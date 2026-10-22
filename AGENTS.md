# arcfetch

Single-binary Rust sysinfo tool. Linux-only, x86_64. Deps: `libc`, plus `lexopt` for CLI parsing.

## Build & run

```bash
cargo build --release        # LTO=fat, strip, panic=abort
cargo run -- --logo nix      # test with specific args
./target/release/arcfetch     # binary
```

No CI or linter/formatter config. `cargo test` runs a few unit tests (config comment/colour/layout parsing, subprocess timeout).

## Architecture

- `src/main.rs` — CLI parser (`lexopt`), render loop, animations (blackhole, quantum), JSON output, single `libc::write(1, ...)` output
- `src/info.rs` — 25+ collectors reading `/proc`, `/sys`, env vars, CPUID, `libc::sysinfo`/`libc::uname`; gates all optional collectors behind `Show` flags
- `src/modules.rs` — opt-in collectors (datetime, dns, host, chassis, bootmgr, cpu cache, login manager, brightness, terminal, terminal size/theme, cursor, cpu usage, disk/net io, physmem, VRAM, btrfs, wifi, users, wallpaper, media, opengl, vulkan, codec, bluetooth, camera, mouse, keyboard, TPM, physical disks, top processes, monitor/EDID)
- `src/net.rs` — std-only HTTP/1.0 client (64 KiB cap, timeouts) for `public_ip` / `weather`
- `src/config.rs` — Catppuccin palette, hand-rolled zero-dependency TOML subset parser (no `toml` crate) and the shared ordered-module render model; `config_path()` resolves `$XDG_CONFIG_HOME/arcfetch/config.toml`, JSONC via `config.jsonc`/`--config <file.jsonc>`; supports `modules = [...]`, `[[modules]]`, `[[custom]]`, `[overrides.<module>]` (`label`/`value`/`command`), `[display]` (`separator`, `labels`, `key_width`, `bar_width`) and `info_style` presets (`classic`, `minimal`, `boxed`, `sections`, `tree`, `iconic`, `card`, `chips`); `Show::enable_all` / `clear_fields` / `select` back `--all` and `-s`; `Layout` holds box style/colour, indent and logo/info gap
- `src/pkgs.rs` — pacman/dpkg/rpm/nix/apk/portage/xbps/bedrock count; contains hand-rolled read-only SQLite B-tree reader
- `src/cache.rs` — two-tier: `/dev/shm/arcfetch-<uid>/` (same-boot, private) + `$XDG_CACHE_HOME/arcfetch/` or `~/.cache/arcfetch/` (persistent, TTL-gated); writes are temp+rename; a fresh persistent hit is promoted into tmpfs for the rest of the boot
- `src/logos.rs` — 40+ distro logo constants; `from_name`, `all_names`, `detect(id, id_like)`
- `src/mandelbrot.rs`, `src/cosmic.rs` — display modes

## Key quirks

- CPU detection uses `core::arch::x86_64::__cpuid_count` — x86_64 only
- GPU probe walks `/sys/class/drm`, falls back to NVIDIA `/proc/driver/nvidia/gpus`, then PCI
- Package DB uses hand-rolled SQLite parser — no `rusqlite` dep
- Cache TTL: GPU 24h, pkgs 1h, CPU freq 24h
- Animations (`--blackhole`, `--quantum`, `--cosmic`) require a TTY; auto-disable on pipe
- `--t 0` means infinite; `--t N` means N seconds (background animations stop with `--stop`, not Ctrl+C)
- Config file at `~/.config/arcfetch/config.toml`, auto-generated via `arcfetch --config`; `[layout]` sets `box`, `box_color`, `indent` and `gap`, `[logo] color` gives a uniform logo colour
- `ARCFETCH_ACCENT` env var overrides accent color
- `--preset <name|file>` loads a JSONC preset from `presets/examples` (or a path); `--list-presets` lists them
- TOML rows: `modules = [...]` sets order, `[[modules]]` / `[[custom]]` give per-row control, `[overrides.<module>]` changes a built-in row, `info_style` picks a starting layout. `info_style` is scanned before the TOML pass, so an explicit `modules`/`[colors]`/`[layout]` still wins
- Only when `cfg.modules` is set does rendering switch from `build_info` to `render_modules`; the default path is untouched, and command rows use `command_timeout` (0.1–10 s)
- UTF-8 rule: never build display strings with `byte as char`; copy whole UTF-8 runs (`format_template`, JSONC strings, logo `$1` colorizing). Box drawing and Nerd Font icons otherwise turn into mojibake
- Kitty image protocol supported for `--logo-file <png|jpg|gif>`
- Only one display mode at a time (`--blackhole`, `--cosmic`, `--quantum`, `--mandelbrot`, `--animate` are mutually exclusive)
- `--animate [shimmer|scanline|boot|frames]` renders a live logo using `logos::animate_line` + `logos::color_wave_phase`; animated logos (`atom`, `dna`, `arcreactor`) cycle via `logos::from_name_frame`
- Normal mode plays a short self-overwriting intro on a TTY (`[animation] enabled/effect/intro_ms/wave`, default 700 ms `none`); animated logos always play. `--no-animate` skips it. Piped output stays static
- `--list-logos` / `--logo ?` prints every built-in name grouped with a one-line preview and `[animated]` marker
- `--list-schemes` prints every colour scheme as a strip of its full 17-colour palette (rosewater … overlay0)
- `--blackhole` uses truecolor temperature gradients, Doppler beaming, turbulence and a lensed photon ring
- `--animate` / `--blackhole` pin the **full fetch** above the **real shell**: art on the left, info on the right, with the same 2-space indent and gap as one-shot output. The terminal's scroll region (DECSTBM) is set below the pane and origin mode (DECOM) is enabled, so the prompt, aliases, history and starship keep working and all command output scrolls under the fetch. The painter forks into its own process group (same session), returns immediately, pauses while a foreground job owns the terminal (`tcgetpgrp`, plus the terminal's line discipline changing, which is how fzf-tab catches even when fzf runs in the shell's own pgrp), drops origin mode for that command so TUIs (fzf, vim, less) can use absolute cursor addressing (full-screen apps reset the region themselves), then restores the region/origin and repaints when the shell returns. It also reasserts the region on resize and stores its PID per tty in `$XDG_RUNTIME_DIR/arcfetch-<tty>.pid`.
- `--stop` kills the pin on the current terminal, clears the fetch pane and resets DECOM/DECSTBM (and cleans the old global `arcfetch-anim.pid`); `--t N` bounds one (`0`/none = until stopped) and leaves the last frame as a static picture. Pinning clears the screen (scrollback is kept). `--foreground` uses the alternate screen (`?1049h`/`?1049l`) for the old blocking full-screen version.
- Colour specs accept catppuccin names, `black`/`white`/`gray`/`orange`/`purple`/`cyan`, `ansi_*`, `#RGB`, `#RRGGBB`, `rgb(r,g,b)`, `colorN`/`N` (xterm-256)
- `--scheme` / `[colors] scheme` presets: `auto`, `mocha` (default), `latte`, `frappe`, `macchiato`, `nord`, `dracula`, `gruvbox`, `tokyo-night`, `solarized-dark`, `rose-pine`, `onedark`, `kanagawa`, `everforest`, `ayu`
- `--scheme auto` queries the terminal background (OSC 11, `$COLORFGBG` fallback) and picks `latte` for light / `mocha` for dark
- `--box [round|square|double|heavy|dashed|ascii]` frames the one-shot output; `[layout] box` does the same from the config (CLI wins); it post-processes the finished buffer, so the default path is unchanged when neither is set
- Colours are down-sampled to the terminal's real depth (`$COLORTERM`/`$TERM`): truecolor `38;2`, xterm-256 `38;5`, or the 16 base colours, so output stays legible anywhere
- Colour precedence: scheme < file `[colors]` < `--scheme` < `--accent`/`ARCFETCH_ACCENT` < `--color`
- Honours `NO_COLOR`, `CLICOLOR=0`, and `FORCE_COLOR` (default: colour off when stdout is not a TTY)
- All newer modules (`datetime` … `monitor`) are **opt-in and off by default**: enable individually in `[show]`, via `-s/--structure`, or all at once with `--all`
- `--json` emits a flat JSON object of enabled fields; `--watch` re-runs the binary every second (Ctrl+C stops)
- `--logo auto` maps `/etc/os-release` `ID`/`ID_LIKE` through `logos::detect`; unknown IDs fall back to `arch`
- Expensive collectors stay off in the default fast path. Under `--all` the slow ones (network, `pkg_updates`, `glxinfo`/`playerctl`/`vainfo`/`bluetoothctl`, `<wm> --version`, `/proc` walks) run concurrently via `std::thread::scope`, results are cached (public_ip 1 h, weather 15 min, `pkg_updates` 1 h, term_theme 5 min, codec/opengl/vulkan/bluetooth 24 h), and `cpu_usage` is measured across that work instead of sleeping 200 ms. `--all` is ~0.48 s cold / ~0.08 s warm; the default path is ~1–2 ms
- `public_ip`/`weather` use **stale-while-revalidate**: once cached they are returned instantly, and a throttled detached `ARCFETCH_REFRESH_CACHE` child refreshes them in the background for the next run — only the very first (uncached) run blocks on the network
