# arcfetch

Single-binary Rust sysinfo tool. Linux, Android/Termux, macOS and BSD; Linux is the full-featured fast path. Deps: `libc`, plus `lexopt` for CLI parsing.

## Build & run

```bash
cargo build --release        # LTO=fat, strip, panic=abort, static-pie on x86_64 Linux
cargo run -- --logo nix      # test with specific args
./target/release/arcfetch     # binary
```

No CI or linter/formatter config. `cargo test` runs the config comment/colour/layout/subprocess-timeout tests and the colour-scheme unit tests (upstream hex values, aliases, role resolution, depth fallback).

`build.rs` embeds `assets/animations/<name>/*.txt` into the binary; add a directory and rebuild to add a bundled animation.

Cross-check without a linker: `cargo check --target x86_64-apple-darwin` (also `aarch64-apple-darwin`), `x86_64-unknown-freebsd`, `x86_64-unknown-netbsd`, `x86_64-linux-android` (also `aarch64-linux-android`).

Cross-build all release targets with `./scripts/build-all.sh` (Cargo + Zig + Android NDK); `./scripts/release-check.sh` runs the local pre-flight and writes `dist/` without publishing. Release steps are in `RELEASE.md`.

## Architecture

- `src/main.rs` — CLI parser (`lexopt`), render loop, animations (blackhole, quantum), JSON output, single `libc::write(1, ...)` output
- `src/info.rs` — 25+ collectors reading `/proc`, `/sys`, env vars, CPUID, `libc::sysinfo`/`libc::uname`; gates all optional collectors behind `Show` flags
- `src/modules.rs` — opt-in collectors (datetime, dns, host, chassis, bootmgr, cpu cache, login manager, brightness, terminal, terminal size/theme, cursor, cpu usage, disk/net io, physmem, VRAM, btrfs, wifi, users, wallpaper, media, opengl, vulkan, codec, bluetooth, camera, mouse, keyboard, TPM, physical disks, top processes, monitor/EDID)
- `src/net.rs` — std-only HTTP/1.0 client (64 KiB cap, timeouts) for `public_ip` / `weather`
- `src/config.rs` — Catppuccin palette, hand-rolled zero-dependency TOML subset parser (no `toml` crate) and the shared ordered-module render model; `config_path()` resolves `$XDG_CONFIG_HOME/arcfetch/config.toml`, JSONC via `config.jsonc`/`--config <file.jsonc>`; supports `modules = [...]` (including multiline arrays), `[[modules]]`, `[[custom]]`, `[overrides.<module>]` (`label`/`value`/`command`), `[display]` (`separator`, `labels`, `key_width`, `bar_width`) and `info_style` presets (`classic`, `minimal`, `boxed`, `sections`, `tree`, `iconic`, `card`, `chips`); palette files (`color_source`/`palette_files`/`palette_accent`/`palette_value`/`palette_gradient`, plus `[palette]` and `[colors]` aliases) read the first existing matugen/pywal/wallust JSON and apply accent/value/gradient before CLI colours; `Show::enable_all` / `clear_fields` / `select` back `--all` and `-s`; `Layout` holds box style/colour, indent and logo/info gap
- `src/fastfetch.rs` — detects fastfetch JSONC (`$schema`, `general`, or fastfetch-only image logo types) and translates logo, display, modules, keys/formats/colours and conditions to the arcfetch render model; `Config.fastfetch` switches package aggregation and fastfetch value formatting
- `src/scheme.rs` — canonical upstream palettes for all 14 schemes, semantic role mappings, aliases and `auto` selection; single source of truth for `--scheme` and `--list-schemes`
- `src/pkgs.rs` — pacman/dpkg/rpm/nix/apk/portage/xbps/bedrock plus brew/pkg/pkg_info/pm counts; contains hand-rolled read-only SQLite B-tree reader
- `src/cache.rs` — two-tier: `/dev/shm/arcfetch-<uid>/` (same-boot, private) + `$XDG_CACHE_HOME/arcfetch/` or `~/.cache/arcfetch/` (persistent, TTL-gated); writes are temp+rename; a fresh persistent hit is promoted into tmpfs for the rest of the boot
- `src/logos.rs` — 40+ distro logo constants, keyframe logos and the procedural `aurora`; `frame`, `animate_line`, `color_wave_phase`, `all_names`, `detect(id, id_like)`
- `src/anim.rs` — bundled frame animations (`assets/animations/<name>/`, embedded by `build.rs`) and user frames from `~/.config/arcfetch/animations/<name>/` (sorted `*.txt`/`*.ansi`, used verbatim so animfetch `$` art survives); `fitted()` scales large art to the terminal with aspect kept
- `src/mandelbrot.rs`, `src/cosmic.rs` — display modes

## Key quirks

- CPU detection uses `core::arch::x86_64::__cpuid_count` on x86_64, with `/proc/cpuinfo` + `sysctl` fallbacks on other arches
- GPU probe walks `/sys/class/drm`, falls back to NVIDIA `/proc/driver/nvidia/gpus`, then PCI
- Package DB uses hand-rolled SQLite parser — no `rusqlite` dep
- Cache TTL: GPU 24h, pkgs 1h, shell version 24h (the fastfetch `shell` row would otherwise spawn `--version` on every prompt)
- Animations (`--blackhole`, `--quantum`, `--cosmic`, `--animate`, `--animation`) require a TTY; auto-disable on pipe
- `--t 0` means infinite; `--t N` means N seconds. In play mode 0 runs until Ctrl+C; in pin mode 0 runs until `--stop`.
- Config file at `~/.config/arcfetch/config.toml`, auto-generated via `arcfetch --config`; `[layout]` sets `box`, `box_color`, `indent` and `gap`, `[logo] color` gives a uniform logo colour. `color_source = "palette"` (or `--palette`) reads the first usable `palette_files` entry and overrides accent/value/logo gradient; matugen role names, pywal/wallust `colorN` (with color4/color7 fallback), and dotted names (`special.background`) all resolve, and missing names keep their config value
- `ARCFETCH_ACCENT` env var overrides accent color
- `--preset <name|file>` loads a JSONC preset from `presets/examples` (or a path); `--list-presets` lists them. Fastfetch JSONC files are detected and routed through `src/fastfetch.rs`; `--config <fastfetch.jsonc>` does the same
- TOML rows: `modules = [...]` sets order, `[[modules]]` / `[[custom]]` give per-row control, `[overrides.<module>]` changes a built-in row, `info_style` picks a starting layout. `info_style` is scanned before the TOML pass, so an explicit `modules`/`[colors]`/`[layout]` still wins
- Only when `cfg.modules` is set does rendering switch from `build_info` to `render_modules`; the default path is untouched, and command rows use `command_timeout` (0.1–10 s)
- UTF-8 rule: never build display strings with `byte as char`; copy whole UTF-8 runs (`format_template`, JSONC strings, logo `$1` colorizing). Box drawing and Nerd Font icons otherwise turn into mojibake
- Kitty image protocol supported for `--logo-file <png|jpg|gif>`; images sit beside the info (`[logo] position = left|right`) with rows derived from the PNG/GIF/JPEG header, or above it with `position = "top"`
- Only one display mode at a time (`--blackhole`, `--cosmic`, `--quantum`, `--mandelbrot`, `--animate` are mutually exclusive)
- `--animate [shimmer|scanline|boot|frames]` plays a compact live logo for `[animation] play_seconds` (default 3 s) and returns to the shell; add `--pin` to keep it above the shell. Keyframe logos (`atom`, `dna`, `arcreactor`) cycle via `logos::frame`, while `aurora` is generated per tick (34×15 grid, no frame table). The pinned painter runs at ~30 fps and skips unchanged rows.
- Animated art is painted by a vertical gradient, animfetch-style: plain lines get one escape per row from `[animation] gradient` (default animfetch's five stops), and `gradient_speed > 0` moves the ramp. Lines carrying authored ANSI keep their own colours (`art_color = "own"`, the default); `art_color = "theme"` strips them so the gradient wins. `gradient = []` falls back to the old `logos::color_wave_phase` + `$1..$9` palette wave.
- Bundled frame animations from animfetch live in `assets/animations/` (one dir per set, `*.txt` frames); `build.rs` embeds them, `--animation <name>` resolves user dir first then bundled, and `--list-animations` prints both. `Animation::fitted()` keeps large art inside the pane. `--animation <name|path>` implies `--animate`.
- Normal mode plays a short self-overwriting intro on a TTY (`[animation] effect/intro_ms/gradient`, default 700 ms `none`); animated logos always play. `--no-animate` skips it. Piped output stays static.
- Play vs pin: `--blackhole`, `--animate` and `--animation` play in the alternate screen for `[animation] play_seconds` (or `--t N`) then restore the shell; `--pin` uses the scroll-region pane instead. `--t 0` = Ctrl+C (play) or `--stop` (pin). `--foreground` is accepted as a no-op alias for play mode.
- `--list-logos` / `--logo ?` prints every built-in name grouped with a one-line preview and `[animated]` marker
- `--list-schemes` prints every scheme with its upstream variant and semantic role swatches (bg, fg, accent, red/green/yellow/blue/purple, …) plus exact hex; it wraps to terminal width and down-samples when truecolor is unavailable
- `--blackhole` uses truecolor temperature gradients, Doppler beaming, turbulence and a lensed photon ring
- `--cosmic` is a distinct full-screen immersive scene on the alternate screen, not a pinned pane
- `--animate` / `--blackhole` **play** the full fetch in the alternate screen by default (art left, info right, same indent/gap as one-shot output), then restore the shell. `--pin` switches to the scroll-region pane above the **real shell**: the terminal's scroll region (DECSTBM) is set below the pane and origin mode (DECOM) is enabled, so the prompt, aliases, history and starship keep working and all command output scrolls under the fetch. The painter forks into its own process group (same session), returns immediately, pauses while a foreground job owns the terminal (`tcgetpgrp`, plus the terminal's line discipline changing, which is how fzf-tab catches even when fzf runs in the shell's own pgrp), drops origin mode for that command so TUIs (fzf, vim, less) can use absolute cursor addressing (full-screen apps reset the region themselves), then restores the region/origin and repaints when the shell returns. It also reasserts the region on resize and stores its PID per tty in `$XDG_RUNTIME_DIR/arcfetch-<tty>.pid`.
- `--stop` kills the pin on the current terminal, clears the fetch pane and resets DECOM/DECSTBM (and cleans the old global `arcfetch-anim.pid`); `--t N` bounds one (`0`/none = until stopped) and leaves the last frame as a static picture. Pinning clears the screen (scrollback is kept).
- Colour specs accept catppuccin names, `black`/`white`/`gray`/`orange`/`purple`/`cyan`, `ansi_*`, `#RGB`, `#RRGGBB`, `rgb(r,g,b)`, `colorN`/`N` (xterm-256)
- `--scheme` / `[colors] scheme` presets: `auto`, `mocha` (default), `latte`, `frappe`, `macchiato`, `nord`, `dracula`, `gruvbox`, `tokyo-night`, `solarized-dark`, `rose-pine`, `onedark`, `kanagawa`, `everforest`, `ayu`
- `--scheme auto` picks `latte` for a light terminal / `mocha` for dark: `$COLORFGBG` first, else an OSC 11 query when stdout is a TTY (60 ms cap, and a silent terminal is remembered for 5 min so it is paid at most once)
- `--box [round|square|double|heavy|dashed|ascii]` frames the one-shot output; `[layout] box` does the same from the config (CLI wins); it post-processes the finished buffer, so the default path is unchanged when neither is set
- Colours are down-sampled to the terminal's real depth (`$COLORTERM`/`$TERM`): truecolor `38;2`, xterm-256 `38;5`, or the 16 base colours, so output stays legible anywhere
- Colour precedence: scheme < file `[colors]` < `--scheme` < `--accent`/`ARCFETCH_ACCENT` < `--color`
- Honours `NO_COLOR`, `CLICOLOR=0`, and `FORCE_COLOR` (default: colour off when stdout is not a TTY)
- All newer modules (`datetime` … `monitor`) are **opt-in and off by default**: enable individually in `[show]`, via `-s/--structure`, or all at once with `--all`
- `--json` emits a flat JSON object of enabled fields; `--watch` re-runs the binary every second (Ctrl+C stops)
- `--logo auto` maps `/etc/os-release` `ID`/`ID_LIKE` through `logos::detect`; unknown IDs fall back to `arch`
- Expensive collectors stay off in the default fast path. Under `--all` the slow ones (network, `pkg_updates`, `glxinfo`/`playerctl`/`vainfo`/`bluetoothctl`, `<wm> --version`, `/proc` walks) run concurrently via `std::thread::scope`, results are cached (public_ip 1 h, weather 15 min, `pkg_updates` 1 h, term_theme 5 min, codec/opengl/vulkan/bluetooth 24 h), and `cpu_usage` is measured across that work instead of sleeping 200 ms. `--all` is ~0.48 s cold / ~0.08 s warm; the default path is ~1–2 ms
- `public_ip`/`weather` use **stale-while-revalidate**: once cached they are returned instantly, and a throttled detached `ARCFETCH_REFRESH_CACHE` child refreshes them in the background for the next run — only the very first (uncached) run blocks on the network
