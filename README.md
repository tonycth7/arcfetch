# arcfetch

> *this is for you if you use arch. but it works everywhere else too.*

arch linux sysinfo that's so fast it's almost rude — ~1.9ms default, ~2.6ms `--full`. one `libc::write` and done. catppuccin mocha. 10+ logos. zero overthinking. only dep is `libc`.

---

## what it looks like

```
        /\        tony@arch
       /  \       ─────────
      /    \      os        Arch Linux
     /      \     uptime    1h 46m
    /   ,,   \    pkgs      1288 (pacman)
   /   |  |   \   shell     zsh
  /_-''    ''-_\  term      alacritty
                  battery   26% ↓
                  memory   ███░░░░░░░░░░░  4.2G / 15.0G
                  disk      112.4G / 455.9G
```

or with block arch (default):

```
         ▟████▙               tony@arch
        ▟██████▙              ─────────
       ▟████████▙             os        Arch Linux
      ▟██████████▙            uptime    1h 46m
     ▂▔▀▜██████████▙          pkgs      1288 (pacman)
    ▟██▅▂▝▜█████████▙        shell     zsh
   ▟█████████████████▙       term      alacritty
  ▟███████████████████▙      battery   26% ↓
 ▟█████████▛▀▀▜██████████    memory   ███░░░░░░░░░░░  4.2G / 15.0G
▟████████▛      ▜█████████▙  disk      112.4G / 455.9G
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

| run          | time     |
|--------------|----------|
| arcfetch     | ~1.9ms   |
| fastfetch    | ~31ms    |
| neofetch     | ~300ms   |

no threads, no subprocesses, no waiting. cpuid for the cpu (zero i/o). `libc::sysinfo` + `libc::uname` for uptime and kernel — one syscall each. two-tier cache (`/dev/shm` + `~/.cache`) means the second run is even faster than the first.

default fields: os, kernel, uptime, pkgs, shell, term, cpu, memory, battery, disk. configurable via `[show]` in config or `--full` / `--preset`.

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
arcfetch --preset minimal         # os kernel uptime pkgs shell term cpu memory
arcfetch --preset hacker          # kernel uptime cpu gpu gpu_temp mem disk load ip
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
arcfetch --logo pi                # raspberry pi
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
arcfetch --quantum --t 5          # 5s quantum animation
arcfetch --cosmic                 # starfield + moon phase
arcfetch --cosmic --t 10          # 10s cosmic animation
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
values = subtext1
sep    = overlay0
bar    = blue

# custom logo file colors ($1..$9 placeholders)
# defaults: c1..c7 + accent + text
logo1 = blue
logo2 = sapphire
logo3 = sky
logo4 = teal
logo5 = green
logo6 = yellow
logo7 = peach
logo8 = mauve
logo9 = text

[show]
header      = both         # both | user | host | none
os          = true
kernel      = true
uptime      = true
pkgs        = true
shell       = true
de_wm       = true
term        = true
cpu         = true
gpu         = true
memory      = true
disk        = false
battery     = false

# extra fields (hidden by default)
res         = false        # screen resolution
load        = false        # system load
locale      = false        # system locale
ip          = false        # local IP
swatches    = false        # color palette row
color_bar   = false        # neofetch-style 8-color bar
swap        = false        # swap usage
sound       = false        # audio server (pipewire/pulse/alsa)
gpu_driver  = false        # gpu driver version
gpu_temp    = false        # gpu temperature
bios        = false        # bios version + date
board       = false        # motherboard vendor + model
disk_type   = false        # ssd / nvme / hdd
pkg_updates = false        # available updates (pacman -Qu)
theme       = false        # gtk theme
icons       = false        # icon theme
term_font   = false        # terminal font
de_wm_ver   = false        # desktop environment version
init_ver    = false        # init system version
local_ip    = false        # interface + local ip
ssh         = false        # ssh sessions
ports       = false        # listening ports

[logo]
# name = arch          # arch | ascii | tux | nix | gentoo | mini | pi | auto
# file = <path>        # custom ascii file (overrides name)

[template]
preset = full           # full | minimal | hacker | science
```

logos and presets can be overridden via CLI — handy for one-off flexes:

```bash
arcfetch --logo nix               # nixos snowflake even if config says arch
arcfetch --preset science          # physicist quote even in minimal config
```

---

## all flags

```
-h,  --help               this
-V,  --version            version + e=mc²
     --config             config reference + write sample
     --full               show most fields
     --preset <n>         full | minimal | hacker | science
     --logo <name>        arch | mini | ascii | tux | nix | gentoo | pi | auto | custom
     --logo-file <path>   ascii art or image file (supports $1..$9 color tokens)
     --accent <color>     hex (#rrggbb) or catppuccin name
     --color <scheme>     logo color: name | #hex | random
     --no-color           strip ansi
     --blackhole          m87 accretion disk animation
     --mandelbrot [iter]  mandelbrot fractal (default 64)
     --quantum            wave-function collapse
     --cosmic             starfield + moon
     --t <secs>           0=forever, n=n secs (blackhole, quantum, cosmic)
```

---

## custom logo colors

your custom ASCII art (via `--logo-file` or `[logo] file =`) can use inline color tokens:

```
$1  → logo1 (default: blue)
$2  → logo2 (default: sapphire)
$3  → logo3 (default: sky)
... up to
$9  → logo9 (default: text)
```

no tokens = default accent color. tokens expand at load time — zero per-frame cost.

---

## logo colors

built-in logos use official distro colors:

| logo    | scheme                     |
|---------|----------------------------|
| nix     | dark blue + light blue     |
| gentoo  | dark purple + light purple |
| mini    | cycling c1-c7 label colors |
| others  | accent color (configurable) |

override the accent for single-color logos:

```bash
arcfetch --color red            # solid red logo
arcfetch --color "#00ff00"      # hex green
arcfetch --color random         # random palette each run
arcfetch --color blue --logo nix # nix keeps its official scheme
```

---

## files

everything in `src/`:

| file              | what                                  |
|-------------------|---------------------------------------|
| `main.rs`         | cli, render, anims, entrypoint, color bar |
| `info.rs`         | 25+ collectors — cpu, gpu, kernel, mem, swap, bios, board, ip … |
| `pkgs.rs`         | pacman, dpkg, rpm, nix, apk, portage  |
| `cache.rs`        | `/dev/shm` + `~/.cache` with ttl      |
| `config.rs`       | catppuccin palette + toml parser      |
| `logos.rs`        | 10+ logos + auto-detect + custom loader |
| `mandelbrot.rs`   | mandelbrot set in rust                |
| `cosmic.rs`       | starfield + moon phase                |
| `flake.nix`       | nix flake for reproducible builds     |

---

## requirements

- any linux — package count is the only arch-specific thing
- true-color terminal (anything from 2015 onwards)
- curiosity about what a terminal can do

---

*built for arch. runs on anything. written in rust. themed in catppuccin. zero bloat.*
