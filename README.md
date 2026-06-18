# arcfetch

> **this is not for you if you don't use arch btw**

Blazing-fast Arch Linux sysinfo in Rust. Catppuccin Mocha. 7 logos. Single binary, zero heavy deps.

**~1.8ms** release — faster than fastfetch, faster than neofetch. One `libc::write` syscall for all output.

---

## what it looks like

```
                  ▄                    tony@arch
                 ▟█▙                   ─────────
                ▟███▙                  os        Arch Linux
               ▟█████▙                 kernel    7.0.11-arch1-1
              ▟███████▙                uptime    3h 17m
             ▂▔▀▜██████▙               pkgs      1227 (pacman)
            ▟██▅▂▝▜█████▙              shell     zsh
           ▟█████████████▙             de/wm     niri
          ▟███████████████▙            term      alacritty
         ▟█████████████████▙           cpu       AMD Ryzen 5 5625U (12x) @ 2.30 GHz
        ▟███████████████████▙          gpu       AMD GPU (0x15e7)
       ▟█████████▛▀▀▜████████▙         memory   ██████░░░░░░░░  6.9G / 15.0G
      ▟████████▛      ▜███████▙
     ▟█████████        ████████▙
    ▟██████████        █████▆▅▄▃▂
   ▟██████████▛        ▜█████████▙
  ▟██████▀▀▀              ▀▀██████▙
 ▟███▀▘                       ▝▀███▙
▟▛▀                               ▀▜▙
```

Or with the compact mini logo:

```
       /\         tony@arch
      /  \        ─────────
     /    \       os        Arch Linux
    /      \      kernel    7.0.11-arch1-1
   /   ,,   \     uptime    3h 17m
  /   |  |   \    pkgs      1227 (pacman)
  /_-''    ''-_\  shell     zsh
                  de/wm     niri
                  term      alacritty
                  cpu       AMD Ryzen 5 5625U (12x) @ 2.30 GHz
                  gpu       AMD GPU (0x15e7)
                  memory   ██████░░░░░░░░  6.9G / 15.0G
```

---

## speed

| run                    | time      |
|------------------------|-----------|
| arcfetch (release)     | ~1.8ms    |
| arcfetch (debug)       | ~2.0ms    |
| fastfetch              | ~31ms     |
| neofetch               | ~300ms    |

All I/O is fully sequential — no threads needed. CPUID + `sysconf` for CPU (zero I/O), `libc::sysinfo` + `libc::uname` for uptime/kernel (1 syscall each). Two-tier cache (`/dev/shm` + `~/.cache`) puts results in tmpfs so repeated runs hit instant cache.

Default shows: os, kernel, uptime, pkgs, shell, de/wm, term, cpu, gpu, memory. No disk, no swatches, no load — just the essentials. Use `--full` to show everything.

---

## install

**AUR:**
```bash
paru -S arcfetch
# or
yay -S arcfetch
```

**cargo:**
```bash
cargo install --git https://github.com/tonycth7/arcfetch --locked
```

**manual:**
```bash
git clone https://github.com/tonycth7/arcfetch
cd arcfetch
cargo build --release
cp target/release/arcfetch ~/.local/bin/
```

---

## quick start

```bash
arcfetch                     # default — fast, clean
arcfetch --full              # show all fields
arcfetch --logo mini         # compact ASCII arch logo
arcfetch --preset minimal    # os kernel uptime memory battery + mini logo
arcfetch --preset hacker     # cpu gpu mem disk load ip ssh ports
arcfetch --preset science    # random science logo + physicist quote
arcfetch --no-color          # plain text, pipe-friendly
```

---

## logos

```
arcfetch                     # default — block arch  ▟███▙
arcfetch --logo mini         # compact 7-line ASCII arch
arcfetch --logo ascii        # classic dotty arch ascii
arcfetch --logo tux          # linux tux penguin
arcfetch --logo nix          # nixOS hexagonal snowflake
arcfetch --logo gentoo       # gentoo G (fastfetch style)
arcfetch --logo auto         # auto-detect from /etc/os-release
arcfetch --logo custom       # reads ~/.config/arcfetch/logo.txt
arcfetch --logo-file <path>  # any ASCII art file (or image in kitty terminals)
```

---

## display modes

```bash
arcfetch --blackhole          # animated M87 accretion disk (~3s)
arcfetch --blackhole --t 0    # infinite loop
arcfetch --mandelbrot [iter]  # Mandelbrot fractal as logo
arcfetch --quantum            # wave-function collapse animation
arcfetch --cosmic             # starfield + moon phase
```

Mutually exclusive — pick one.

---

## config

```bash
arcfetch --config
```

Writes `~/.config/arcfetch/config.toml` with all options. Full control over colors, field visibility, and presets.

```toml
[show]
os          = true
kernel      = true
uptime      = true
res         = false
pkgs        = true
shell       = true
de_wm       = true
term        = true
cpu         = true
gpu         = true
memory      = true
disk        = false
battery     = false
load        = false
locale      = false
swatches    = false
init        = false
cpu_temp    = false
processes   = false
container   = false
session     = false

[colors]
accent   = blue
username = mauve
values   = subtext1
sep      = overlay0
bar      = blue

[template]
preset = full
```

---

## all flags

```
-h,  --help               usage screen
-V,  --version            version + E=mc² joke
     --config             config reference + write sample file
     --full               show all fields
     --preset <n>         full | minimal | hacker | science
     --logo <name>        arch | mini | ascii | tux | nix | gentoo | auto | custom
     --logo-file <path>   ASCII art file or image (kitty protocol)
     --accent <color>     hex (#RRGGBB) or catppuccin name
     --no-color           strip all ANSI — pipe-friendly
     --blackhole          M87 accretion disk animation
     --mandelbrot [iter]  fractal logo (default 64 iterations)
     --quantum            wave-function collapse
     --cosmic             starfield + moon
     --t <secs>           0=infinite, N=N seconds (blackhole & cosmic)
```

---

## architecture

Everything is in `src/`:

| File | Purpose |
|------|---------|
| `main.rs` | CLI args, rendering, anim modes, entrypoint |
| `info.rs` | System info — CPUID + files + syscalls, lazy eval |
| `pkgs.rs` | Multi-distro package count (pacman, dpkg, rpm, nix, apk, xbps, bedrock) |
| `cache.rs` | Two-tier cache: `/dev/shm` (same-boot) + `~/.cache` (TTL) |
| `config.rs` | Catppuccin palette, TOML-subset parser |
| `logos.rs` | 7 built-in logos + custom file loader |
| `mandelbrot.rs` | Pure-Rust Mandelbrot set renderer |
| `cosmic.rs` | Starfield + moon phase display |

---

## requirements

- Arch Linux (reads `/var/lib/pacman/local` for package count)
- Rust 1.85+ (edition 2024)
- True-color terminal (anything after 2015)
- Kitty/ghostty/wezterm/foot for image support

---

*built for arch. written in rust. themed in catppuccin. zero bloat.*
