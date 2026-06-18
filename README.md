# arcfetch

> *this is for you if you use arch. but it works everywhere else too.*

arch linux sysinfo that's so fast it's almost rude — ~1.2ms in release, one `libc::write` and done. catppuccin mocha. 6 logos. zero overthinking. only dep is `libc`.

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

or with the mini logo (auto-selected by `--preset minimal`):

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

## blackhole — the main feature

this isn't just 'neofetch but rust.' this is a **simulated M87 accretion disk** rendered in real time with actual physics math — gravitational lensing, doppler beaming (the approaching side of the disk is brighter), photon ring glow, and a rotating shadow. it runs for ~3 seconds by default, then exits cleanly back to your prompt.

```bash
arcfetch --blackhole          # 3s animation — M87* accretion disk
arcfetch --blackhole --t 0    # infinite loop (Ctrl+C to stop)
arcfetch --blackhole --t 10   # 10 seconds
```

it pulls your system info alongside the disk, so it's still functional — you get the flex *and* the data. and because it's pure rust with zero dependencies (libc doesn't count), there's no ffmpeg, no opengl, no bullet hell of missing libraries. the terminal is the gpu.

the disk renders as a rotating ring of block characters (`░` `▒` `▓` `█`) around a dark center — approaching side is brighter from doppler beaming, photon ring glows blue, outer halo fades to purple. background stars twinkle, shooting stars streak. all deterministic, all zero-dependency.

---

## speed

| run          | time    |
|--------------|---------|
| arcfetch     | ~1.2ms  |
| fastfetch    | ~31ms   |
| neofetch     | ~300ms  |

no threads, no subprocesses, no waiting. cpuid for the cpu (zero i/o). `libc::sysinfo` + `libc::uname` for uptime and kernel — one syscall each. two-tier cache (`/dev/shm` + `~/.cache`) means the second run is even faster than the first.

only 10 fields by default: os, kernel, uptime, pkgs, shell, de/wm, term, cpu, gpu, memory. no disk, no swatches, no load. just what you actually look at.

---

## install

**nix:**
```bash
nix run github:tonycth7/arcfetch
```

**aur:**
```bash
paru -S arcfetch
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
arcfetch                          # default — clean, fast
arcfetch --logo mini              # compact ascii arch
arcfetch --preset minimal         # os kernel uptime memory battery
arcfetch --preset hacker          # kernel uptime cpu gpu gpu_temp mem disk load ip ssh ports
arcfetch --preset science         # random logo + physicist quote
arcfetch --full                   # most fields
arcfetch --no-color               # plain text, pipe-friendly
```

---
```bash
arcfetch                          # block arch (default)
arcfetch --logo mini              # compact 7-line arch
arcfetch --logo ascii             # dotty neofetch-style
arcfetch --logo tux               # linux penguin
arcfetch --logo nix               # nixos snowflake
arcfetch --logo gentoo            # gentoo G
arcfetch --logo auto              # detect from /etc/os-release
arcfetch --logo custom            # ~/.config/arcfetch/logo.txt
arcfetch --logo-file <path>       # any ascii art or image (kitty protocol)
```

---

## display modes

```bash
arcfetch --blackhole              # m87 accretion disk (3s)
arcfetch --blackhole --t 0        # infinite loop
arcfetch --mandelbrot [iter]      # mandelbrot fractal as logo
arcfetch --quantum                # wave-function collapse
arcfetch --cosmic                 # starfield + moon phase
```

mutually exclusive. pick one.

---

## config

```bash
arcfetch --config
```

writes `~/.config/arcfetch/config.toml` with everything, but you probably don't need it. defaults are sane.

```toml
[colors]
accent   = blue
username = mauve
hostname = blue
c1 = blue
c2 = sapphire
c3 = sky
c4 = teal
c5 = green
c6 = yellow
c7 = peach
values   = subtext1
sep      = overlay0
bar      = blue

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
ip          = false
ssh         = false
ports       = false
swatches    = false

[logo]
# name = arch         # arch | ascii | tux | nix | gentoo | mini | auto
# file = <path>       # custom ascii file (overrides name)

[template]
preset = full
```

---

## all flags

```
-h,  --help               this
-V,  --version            version + e=mc2
     --config             config reference + write sample
     --full               show most fields
     --preset <n>         full | minimal | hacker | science
     --logo <name>        arch | mini | ascii | tux | nix | gentoo | auto | custom
     --logo-file <path>   ascii art or image file
     --accent <color>     hex (#rrggbb) or catppuccin name
     --no-color           strip ansi
     --blackhole          m87 accretion disk animation
     --mandelbrot [iter]  mandelbrot fractal (default 64)
     --quantum            wave-function collapse
     --cosmic             starfield + moon
     --t <secs>           0=forever, n=n secs (blackhole & cosmic)
```

---

## files

everything in `src/`:

| file              | what                                  |
|-------------------|---------------------------------------|
| `main.rs`         | cli, render, anims, entrypoint        |
| `info.rs`         | cpu, gpu, kernel, uptime, memory      |
| `pkgs.rs`         | pacman, dpkg, rpm, nix, apk, portage  |
| `cache.rs`        | `/dev/shm` + `~/.cache` with ttl      |
| `config.rs`       | catppuccin palette + toml parser      |
| `logos.rs`        | 6 logos + auto-detect + custom loader |
| `mandelbrot.rs`   | mandelbrot set in rust                |
| `cosmic.rs`       | starfield + moon phase                |
| `flake.nix`       | nix flake for reproducible builds     |

---

## requirements

- any linux — package count is the only arch-specific thing
- rust 1.85+ (edition 2024)
- true-color terminal (anything from 2015 onwards)
- curiosity about what a terminal can do

---

*built for arch. runs on anything. written in rust. themed in catppuccin. zero bloat.*
