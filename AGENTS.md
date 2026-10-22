# arcfetch

Single-binary Rust sysinfo tool. Linux-only, x86_64. Deps: `libc`, plus `lexopt` for CLI parsing.

## Build & run

```bash
cargo build --release        # LTO=fat, strip, panic=abort
cargo run -- --logo nix      # test with specific args
./target/release/arcfetch     # binary
```

No CI or linter/formatter config. `cargo test` runs the colour-scheme unit tests (upstream hex values, aliases, role resolution, depth fallback).

## Architecture

- `src/main.rs` — CLI parser (`lexopt`), render loop, animations (blackhole, quantum), JSON output, single `libc::write(1, ...)` output
- `src/info.rs` — 25+ collectors reading `/proc`, `/sys`, env vars, CPUID, `libc::sysinfo`/`libc::uname`; gates all optional collectors behind `Show` flags
- `src/modules.rs` — opt-in collectors (datetime, dns, host, chassis, bootmgr, cpu cache, login manager, brightness, terminal, terminal size/theme, cursor, cpu usage, disk/net io, physmem, VRAM, btrfs, wifi, users, wallpaper, media, opengl, vulkan, codec, bluetooth, camera, mouse, keyboard, TPM, physical disks, top processes, monitor/EDID)
- `src/net.rs` — std-only HTTP/1.0 client (64 KiB cap, timeouts) for `public_ip` / `weather`
- `src/config.rs` — Catppuccin Mocha palette, hand-rolled TOML subset parser (no `toml` crate), `config_path()` resolves `$XDG_CONFIG_HOME/arcfetch/config.toml`; `Show::enable_all` / `clear_fields` / `select` back `--all` and `-s`
- `src/scheme.rs` — canonical upstream palettes for all 14 schemes, semantic role mappings, aliases and `auto` selection; single source of truth for `--scheme` and `--list-schemes`
- `src/pkgs.rs` — pacman/dpkg/rpm/nix/apk/portage/xbps/bedrock count; contains hand-rolled read-only SQLite B-tree reader
- `src/cache.rs` — two-tier: `/dev/shm/arcfetch/` (same-boot) + `~/.cache/arcfetch/` (persistent, TTL-gated); a fresh persistent hit is promoted into tmpfs for the rest of the boot
- `src/logos.rs` — 40+ distro logo constants, keyframe logos and the procedural `aurora`; `frame`, `animate_line`, `color_wave_phase`, `all_names`, `detect(id, id_like)`
- `src/anim.rs` — user frame animations from `~/.config/arcfetch/animations/<name>/` (sorted `*.txt`/`*.ansi`, used verbatim so animfetch `$` art survives)
- `src/mandelbrot.rs`, `src/cosmic.rs` — display modes

## Key quirks

- CPU detection uses `core::arch::x86_64::__cpuid_count` — x86_64 only
- GPU probe walks `/sys/class/drm`, falls back to NVIDIA `/proc/driver/nvidia/gpus`, then PCI
- Package DB uses hand-rolled SQLite parser — no `rusqlite` dep
- Cache TTL: GPU 24h, pkgs 1h, CPU freq 24h
- Animations (`--blackhole`, `--quantum`, `--cosmic`, `--animate`, `--animation`) require a TTY; auto-disable on pipe
- `--t 0` means infinite; `--t N` means N seconds (background animations stop with `--stop`, not Ctrl+C)
- Config file at `~/.config/arcfetch/config.toml`, auto-generated via `arcfetch --config`
- `ARCFETCH_ACCENT` env var overrides accent color
- `--preset science` picks random logo + physicist quote from `/proc/uptime` seed
- Kitty image protocol supported for `--logo-file <png|jpg|gif>`
- Only one display mode at a time (`--blackhole`, `--cosmic`, `--quantum`, `--mandelbrot`, `--animate` are mutually exclusive)
- `--animate [shimmer|scanline|boot|frames]` pins a compact live logo using `logos::animate_line`; keyframe logos (`atom`, `dna`, `arcreactor`) cycle via `logos::frame`, while `aurora` is generated per tick (34×15 grid, no frame table). The pinned painter runs at ~30 fps and skips unchanged rows.
- Animated art is painted by a vertical gradient, animfetch-style: plain lines get one escape per row from `[animation] gradient` (default animfetch's five stops), and `gradient_scroll` moves the ramp. Lines carrying authored ANSI keep their own colours (`art_color = "own"`, the default); `art_color = "theme"` strips them so the gradient wins. `gradient = []` falls back to the old `logos::color_wave_phase` + `$1..$9` palette wave.
- `--animation <name|path>` plays user frames from `~/.config/arcfetch/animations/<name>/` and implies `--animate`; `--list-animations` lists built-in and custom sets. Frames are `*.txt`/`*.ansi` in filename order, padded to a common width.
- Normal mode plays a short self-overwriting intro on a TTY (`[animation] enabled/effect/intro_ms/wave/gradient`, default 700 ms `none`); animated logos always play. `--no-animate` skips it. Piped output stays static.
- `--list-logos` / `--logo ?` prints every built-in name grouped with a one-line preview and `[animated]` marker
- `--list-schemes` prints every scheme with its upstream variant and semantic role swatches (bg, fg, accent, red/green/yellow/blue/purple, …) plus exact hex; it wraps to terminal width and down-samples when truecolor is unavailable
- `--blackhole` uses truecolor temperature gradients, Doppler beaming, turbulence and a lensed photon ring
- `--cosmic` is a distinct full-screen immersive scene on the alternate screen, not a pinned pane
- `--animate` / `--blackhole` pin the **full fetch** above the **real shell**: art on the left, info on the right, with the same 2-space indent and gap as one-shot output. The terminal's scroll region (DECSTBM) is set below the pane and origin mode (DECOM) is enabled, so the prompt, aliases, history and starship keep working and all command output scrolls under the fetch. The painter forks into its own process group (same session), returns immediately, pauses while a foreground command owns the terminal (`tcgetpgrp`), drops origin mode for that command so TUIs (fzf, vim, less) can use absolute cursor addressing (full-screen apps reset the region themselves), then restores the region/origin and repaints when the shell returns. It also reasserts the region on resize and stores its PID per tty in `$XDG_RUNTIME_DIR/arcfetch-<tty>.pid`.
- `--stop` kills the pin on the current terminal, clears the fetch pane and resets DECOM/DECSTBM (and cleans the old global `arcfetch-anim.pid`); `--t N` bounds one (`0`/none = until stopped) and leaves the last frame as a static picture. Pinning clears the screen (scrollback is kept). `--foreground` uses the alternate screen (`?1049h`/`?1049l`) for the old blocking full-screen version.
- Colour specs accept catppuccin names, `black`/`white`/`gray`/`orange`/`purple`/`cyan`, `ansi_*`, `#RGB`, `#RRGGBB`, `rgb(r,g,b)`, `colorN`/`N` (xterm-256)
- `--scheme` / `[colors] scheme` presets: `auto`, `mocha` (default), `latte`, `frappe`, `macchiato`, `nord`, `dracula`, `gruvbox`, `tokyo-night`, `solarized-dark`, `rose-pine`, `onedark`, `kanagawa`, `everforest`, `ayu`
- `--scheme auto` queries the terminal background (OSC 11, `$COLORFGBG` fallback) and picks `latte` for light / `mocha` for dark
- `--box [round|square|double|heavy|dashed|ascii]` frames the one-shot output; it post-processes the finished buffer, so the default path is unchanged when the flag is absent
- Colours are down-sampled to the terminal's real depth (`$COLORTERM`/`$TERM`): truecolor `38;2`, xterm-256 `38;5`, or the 16 base colours, so output stays legible anywhere
- Colour precedence: scheme < file `[colors]` < `--scheme` < `--accent`/`ARCFETCH_ACCENT` < `--color`
- Honours `NO_COLOR`, `CLICOLOR=0`, and `FORCE_COLOR` (default: colour off when stdout is not a TTY)
- All newer modules (`datetime` … `monitor`) are **opt-in and off by default**: enable individually in `[show]`, via `-s/--structure`, or all at once with `--all`
- `--json` emits a flat JSON object of enabled fields; `--watch` re-runs the binary every second (Ctrl+C stops)
- `--logo auto` maps `/etc/os-release` `ID`/`ID_LIKE` through `logos::detect`; unknown IDs fall back to `arch`
- Expensive collectors stay off in the default fast path. Under `--all` the slow ones (network, `glxinfo`/`playerctl`/`vainfo`/`bluetoothctl`, `<wm> --version`, `/proc` walks) run concurrently via `std::thread::scope`, results are cached (public_ip 1 h, weather 15 min, term_theme 5 min, codec/opengl/vulkan/bluetooth 24 h), and `cpu_usage` is measured across that work instead of sleeping 200 ms. `--all` is ~0.48 s cold / ~0.07 s warm; the default path is ~1–2 ms
- `public_ip`/`weather` use **stale-while-revalidate**: once cached they are returned instantly, and a throttled detached `ARCFETCH_REFRESH_CACHE` child refreshes them in the background for the next run — only the very first (uncached) run blocks on the network
