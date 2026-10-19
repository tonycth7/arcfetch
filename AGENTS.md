# arcfetch

Single-binary Rust sysinfo tool. Linux-only, x86_64. Deps: `libc`, plus `lexopt` for CLI parsing.

## Build & run

```bash
cargo build --release        # LTO=fat, strip, panic=abort
cargo run -- --logo nix      # test with specific args
./target/release/arcfetch     # binary
```

No tests, no CI, no linter/formatter config. `cargo test` always passes (no test code).

## Architecture

- `src/main.rs` — CLI parser (`lexopt`), render loop, animations (blackhole, quantum), JSON output, single `libc::write(1, ...)` output
- `src/info.rs` — 25+ collectors reading `/proc`, `/sys`, env vars, CPUID, `libc::sysinfo`/`libc::uname`; gates all optional collectors behind `Show` flags
- `src/modules.rs` — opt-in collectors (datetime, dns, host, chassis, bootmgr, cpu cache, login manager, brightness, terminal, terminal size/theme, cursor, cpu usage, disk/net io, physmem, VRAM, btrfs, wifi, users, wallpaper, media, opengl, vulkan, codec, bluetooth, camera, mouse, keyboard, TPM, physical disks, top processes, monitor/EDID)
- `src/net.rs` — std-only HTTP/1.0 client (64 KiB cap, timeouts) for `public_ip` / `weather`
- `src/config.rs` — Catppuccin Mocha palette, hand-rolled TOML subset parser (no `toml` crate), `config_path()` resolves `$XDG_CONFIG_HOME/arcfetch/config.toml`; `Show::enable_all` / `clear_fields` / `select` back `--all` and `-s`
- `src/pkgs.rs` — pacman/dpkg/rpm/nix/apk/portage/xbps/bedrock count; contains hand-rolled read-only SQLite B-tree reader
- `src/cache.rs` — two-tier: `/dev/shm/arcfetch/` (same-boot) + `~/.cache/arcfetch/` (persistent, TTL-gated)
- `src/logos.rs` — 40+ distro logo constants; `from_name`, `all_names`, `detect(id, id_like)`
- `src/mandelbrot.rs`, `src/cosmic.rs` — display modes

## Key quirks

- CPU detection uses `core::arch::x86_64::__cpuid_count` — x86_64 only
- GPU probe walks `/sys/class/drm`, falls back to NVIDIA `/proc/driver/nvidia/gpus`, then PCI
- Package DB uses hand-rolled SQLite parser — no `rusqlite` dep
- Cache TTL: GPU 24h, pkgs 1h, CPU freq 24h
- Animations (`--blackhole`, `--quantum`, `--cosmic`) require a TTY; auto-disable on pipe
- `--t 0` means infinite; `--t N` means N seconds (background animations stop with `--stop`, not Ctrl+C)
- Config file at `~/.config/arcfetch/config.toml`, auto-generated via `arcfetch --config`
- `ARCFETCH_ACCENT` env var overrides accent color
- `--preset science` picks random logo + physicist quote from `/proc/uptime` seed
- Kitty image protocol supported for `--logo-file <png|jpg|gif>`
- Only one display mode at a time (`--blackhole`, `--cosmic`, `--quantum`, `--mandelbrot`, `--animate` are mutually exclusive)
- `--animate [shimmer|scanline|boot|frames]` renders a live logo using `logos::animate_line` + `logos::color_wave_phase`; animated logos (`atom`, `dna`, `arcreactor`) cycle via `logos::from_name_frame`
- Normal mode plays a short self-overwriting intro on a TTY (`[animation] enabled/effect/intro_ms/wave`, default 700 ms `none`); animated logos always play. `--no-animate` skips it. Piped output stays static
- `--list-logos` / `--logo ?` prints every built-in name grouped with a one-line preview and `[animated]` marker
- `--blackhole` uses truecolor temperature gradients, Doppler beaming, turbulence and a lensed photon ring
- `--animate` / `--blackhole` pin the art above the **real shell**: the terminal's scroll region (DECSTBM) is set below the pane and origin mode (DECOM) is enabled, so the prompt, aliases, history and starship keep working and all command output scrolls under the art. The painter forks into its own process group (same session), returns immediately, pauses while a foreground command owns the terminal (`tcgetpgrp`), repaints after full-screen apps, reasserts the region on resize, and stores its PID per tty in `$XDG_RUNTIME_DIR/arcfetch-<tty>.pid`.
- `--stop` kills the pin on the current terminal and resets DECOM/DECSTBM (and cleans the old global `arcfetch-anim.pid`); `--t N` bounds one (`0`/none = until stopped). Pinning clears the screen (scrollback is kept) and leaves the last frame behind. `--foreground` uses the alternate screen (`?1049h`/`?1049l`) for the old blocking full-screen version.
- Colour specs accept catppuccin names, `black`/`white`/`gray`/`orange`/`purple`/`cyan`, `ansi_*`, `#RGB`, `#RRGGBB`, `rgb(r,g,b)`, `colorN`/`N` (xterm-256)
- `--scheme` / `[colors] scheme` presets: `mocha` (default), `latte`, `frappe`, `macchiato`, `nord`, `dracula`, `gruvbox`, `tokyo-night`, `solarized-dark`, `rose-pine`
- Colour precedence: scheme < file `[colors]` < `--scheme` < `--accent`/`ARCFETCH_ACCENT` < `--color`
- Honours `NO_COLOR`, `CLICOLOR=0`, and `FORCE_COLOR` (default: colour off when stdout is not a TTY)
- All newer modules (`datetime` … `monitor`) are **opt-in and off by default**: enable individually in `[show]`, via `-s/--structure`, or all at once with `--all`
- `--json` emits a flat JSON object of enabled fields; `--watch` re-runs the binary every second (Ctrl+C stops)
- `--logo auto` maps `/etc/os-release` `ID`/`ID_LIKE` through `logos::detect`; unknown IDs fall back to `arch`
- Expensive collectors stay off in the default fast path. Under `--all` the slow ones (network, `glxinfo`/`playerctl`/`vainfo`/`bluetoothctl`, `/proc` walks) run concurrently via `std::thread::scope`, results are cached (public_ip 1 h, weather 15 min, term_theme 5 min, codec/opengl/vulkan/bluetooth 24 h), and `cpu_usage` is measured across that work instead of sleeping 200 ms. `--all` is ~0.47 s cold / ~0.11 s warm; the default path is ~2.6 ms
