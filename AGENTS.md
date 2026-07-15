# arcfetch

Single-binary Rust sysinfo tool. Linux-only, x86_64. One dep: `libc`.

## Build & run

```bash
cargo build --release        # LTO=fat, strip, panic=abort
cargo run -- --logo nix      # test with specific args
./target/release/arcfetch     # binary
```

No tests, no CI, no linter/formatter config. `cargo test` always passes (no test code).

## Architecture

- `src/main.rs` — CLI parser (hand-rolled), render loop, animations (blackhole, quantum), single `libc::write(1, ...)` output
- `src/info.rs` — 25+ collectors reading `/proc`, `/sys`, env vars, CPUID, `libc::sysinfo`/`libc::uname`
- `src/config.rs` — Catppuccin Mocha palette, hand-rolled TOML subset parser (no `toml` crate), `config_path()` resolves `$XDG_CONFIG_HOME/arcfetch/config.toml`
- `src/pkgs.rs` — pacman/dpkg/rpm/nix/apk/portage/xbps/bedrock count; contains hand-rolled read-only SQLite B-tree reader
- `src/cache.rs` — two-tier: `/dev/shm/arcfetch/` (same-boot) + `~/.cache/arcfetch/` (persistent, TTL-gated)
- `src/logos.rs` — logo constants
- `src/mandelbrot.rs`, `src/cosmic.rs` — display modes

## Key quirks

- CPU detection uses `core::arch::x86_64::__cpuid_count` — x86_64 only
- GPU probe walks `/sys/class/drm`, falls back to NVIDIA `/proc/driver/nvidia/gpus`, then PCI
- Package DB uses hand-rolled SQLite parser — no `rusqlite` dep
- Cache TTL: GPU 24h, pkgs 1h, CPU freq 24h
- Animations (`--blackhole`, `--quantum`, `--cosmic`) require a TTY; auto-disable on pipe
- `--t 0` means infinite loop (Ctrl+C); `--t N` means N seconds
- Config file at `~/.config/arcfetch/config.toml`, auto-generated via `arcfetch --config`
- `ARCFETCH_ACCENT` env var overrides accent color
- `--preset science` picks random logo + physicist quote from `/proc/uptime` seed
- Kitty image protocol supported for `--logo-file <png|jpg|gif>`
- Only one display mode at a time (`--blackhole`, `--cosmic`, `--quantum`, `--mandelbrot` are mutually exclusive)
