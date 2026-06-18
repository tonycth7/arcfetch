# AGENTS.md — arcfetch

## Commands

```sh
cargo build               # debug
cargo build --release     # release (only path to verification — no tests)
```

No lint, format, typecheck, or test commands exist.

## Speed contract

- Target: **sub-2ms** release (warm cache). Currently ~1.2ms.
- All output buffers into one `String`, flushed via a single `libc::write(1, …)` — never wrap stdout in `BufWriter`/`StdoutLock`.
- All I/O is **sequential** (was threaded before; revert would break the contract).
- Every field is lazy-evaluated: `info.rs` only probes files when `show.xxx` is true.

## Cache

Two-tier write-through:
1. `/dev/shm/arcfetch/` — tmpfs, lost on reboot (fast)
2. `~/.cache/arcfetch/` — persistent, TTL-gated

Cache keys: `pkgs`, `gpu`, `os`, `cpu_freq`. Each file stores a plain value; `cache.rs` manages invalidation.

## Architecture

| File | Purpose |
|------|---------|
| `main.rs` | CLI, render loop, display modes, `build_info()` wiring |
| `info.rs` | System info — CPUID, `/proc`, `/sys`, libc syscalls |
| `config.rs` | TOML config parser + Catppuccin Mocha palette + `Show` defaults |
| `logos.rs` | 7 built-in logo arrays, `from_name()` dispatcher |
| `cache.rs` | Two-tier cache core |
| `pkgs.rs` | Multi-distro package counting (pacman, dpkg, rpm+nix SQLite, etc.) |
| `cosmic.rs` | Cosmic display mode — standalone, threaded sleep loop |
| `mandelbrot.rs` | Pure Rust Mandelbrot render, 19×31 grid |

- `config.rs` and `logos.rs` are tuned together: label colors cycle c1–c7; mini logo per-line color uses `cfg.colors.label(i)`.
- Default `Show`: os, kernel, uptime, pkgs, shell, de/wm, term, cpu, gpu, memory. No disk, no swatches.
- GPU probe order: DRM sysfs (card0) → NVIDIA → PCI fallback (in `info.rs`).

## Conventions

- **Rust edition 2024** — requires Rust 1.85+. Regression risk: edition 2024 changed trait resolution and `unsafe` rules.
- **No proc macros, no build script, no bindgen** — zero codegen steps.
- **`Show` struct drives field filter** — add a field? Add it to `Show`, `config.rs` defaults, and `build_info()`.
- **Config at `~/.config/arcfetch/config.toml`** — `config::load()`.
- **Release profile** (`Cargo.toml`): `lto = "thin"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.
- **PKGBUILD** at repo root — bump version in both `Cargo.toml` and `PKGBUILD`.
- **Single binary**, no workspace, no features.

## Display modes (mutually exclusive in CLI)

- `--blackhole` — starfield → singularity animation
- `--cosmic` — starfield + moon phase (sleep loop)
- `--quantum` — identity matrix counter
- `--mandelbrot` — Mandelbrot render

## Gotchas

- Edition 2024 `unsafe` blocks may need revision if porting older Rust code.
- `info.rs` uses raw `libc::syscall` + `CPUID` — not portable to non-x86-64 without work.
- `pkgs.rs` spawns short-lived `Command` processes for each package manager found.
- `cosmic.rs` *does* use `std::thread` (it's an animation, not a fetch), but the main fetch path (`build_info`) is thread-free.
