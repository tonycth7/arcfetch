# arcfetch

> **this is not for you if you don't use arch btw**

blazing fast system info written in rust. catppuccin mocha. 8 logos.
a black hole. a π symbol made of its own digits. physicist quotes.

---

## what it looks like

```
                  ▄                    tony@DESKTOP
                 ▟█▙                   ────────────
                ▟███▙                  os        Arch Linux
               ▟█████▙                kernel    6.6.87-arch1-1
              ▟███████▙               uptime    4h 20m
             ▂▔▀▜██████▙              res       2560x1440
            ▟██▅▂▝▜█████▙             pkgs      1247 (pacman)
           ▟█████████████▙            shell     zsh
          ▟███████████████▙           de/wm     Hyprland
         ▟█████████████████▙          term      kitty
        ▟███████████████████▙         cpu       AMD Ryzen 5 5625U (12x)
       ▟█████████▛▀▀▜████████▙        gpu       AMD GPU
      ▟████████▛      ▜███████▙       memory    ████████░░░░░░  3.1G / 7.4G
     ▟█████████        ████████▙      disk      64.8G / 1006.9G
    ▟██████████        █████▆▅▄▃▂     load      0.12  0.20  0.18
   ▟██████████▛        ▜█████████▙    locale    en_US.UTF-8
  ▟██████▀▀▀              ▀▀██████▙
 ▟███▀▘                       ▝▀███▙  ● ● ● ● ● ● ● ● ● ● ● ● ● ●
▟▛▀                               ▀▜▙
```

---

## speed

first run after boot is slower (~0.020s) — kernel cold-caches `/proc` and
`/var/lib/pacman`. every subsequent run hits warm cache:

| run            | time      |
|----------------|-----------|
| arcfetch       | ~0.007s   |
| fastfetch      | ~0.031s   |
| neofetch       | ~0.300s   |

on bare metal arch the warm cache number drops to ~0.003s.
hacker preset is ~0.010-0.012s due to `/proc/net/tcp` reads — expected and worth it.

how: two threads for slow real-FS reads (`/var/lib/pacman`, GPU detection),
everything else sequential from `/proc` and `/sys`. zero subprocesses.

---

## install

**via AUR** :
```bash
paru -S arcfetch
# or
yay -S arcfetch
```

**one liner — no clone needed:**

```bash
cargo install --git https://github.com/tonycth7/arcfetch --locked
```

or clone and build manually:

```bash
git clone https://github.com/tonycth7/arcfetch
cd arcfetch
chmod +x install.sh
./install.sh
```

add `~/.local/bin` to PATH if needed:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

need rust?

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## auto-run on terminal open

add one line to your shell config:

**zsh** (`~/.zshrc`):
```bash
arcfetch
# or pin a specific look:
arcfetch --logo pi --accent mauve
arcfetch --preset science
```

**bash** (`~/.bashrc`):
```bash
arcfetch --logo ascii
```

**fish** (`~/.config/fish/config.fish`):
```fish
arcfetch --logo atom --accent teal
```

---

## logos

8 to choose from:

```bash
arcfetch                           # default — block arch  ▟███▙
arcfetch --logo ascii              # classic dotty arch ascii
arcfetch --logo tux                # linux tux penguin
arcfetch --logo dna                # DNA double helix  A═T  G═C  base pairs
arcfetch --logo atom               # bohr atom (Fe) — 26p 30n 26e — 3d⁶4s²
arcfetch --logo wave               # schrödinger  ψ(x,t) = Ae^i(kx−ωt)
arcfetch --logo emc2               # E = mc²  with mass-energy conversion table
arcfetch --logo pi                 # π — digits spiralling around the symbol itself
arcfetch --logo custom             # reads ~/.config/arcfetch/logo.txt
arcfetch --logo-file ~/mylogo.txt  # any file, any path
```

### custom logos

put any ASCII art in `~/.config/arcfetch/logo.txt` and use it with:

```bash
arcfetch --logo custom
```

or point to any file:

```bash
arcfetch --logo-file ~/my-art/dragon.txt
arcfetch --logo-file ~/.config/arcfetch/tux-big.txt
```

the file is read as-is — one line per row, any width, any number of lines.
it renders in your accent color just like built-in logos. no size limit.

example `~/.config/arcfetch/logo.txt`:
```
    /\
   /  \
  / /\ \
 / ____ \
/_/    \_\
```

the `science` preset automatically picks a random science logo every session
(cycles dna / atom / wave / emc2 / pi by uptime — different every terminal open).

---

## presets

```bash
arcfetch --preset full      # everything — all fields visible
arcfetch --preset minimal   # os kernel uptime memory battery
arcfetch --preset hacker    # kernel uptime cpu gpu gpu_temp mem disk load ip ssh ports
arcfetch --preset science   # random science logo + physicist quote at the bottom
```

set permanently in config:

```toml
[template]
preset = science
```

---

## config

```bash
arcfetch --config
```

prints the full reference **and** writes a sample `~/.config/arcfetch/config.toml`
on first run. won't overwrite an existing file.

### header

```toml
[show]
header = both    # user@host  (default)
header = user    # just the username
header = host    # just the hostname
header = none    # no header at all — pure info
```

### show / hide any field

```toml
[show]
os          = true
kernel      = true
uptime      = true
uptime_long = false   # "1 day, 2 hours, 30 mins" vs "1d 2h 30m"
res         = false
pkgs        = true
shell       = true
de_wm       = true
term        = true
cpu         = true
gpu         = true
gpu_temp    = false   # GPU temperature °C — reads /sys/class/drm hwmon
battery     = false   # 85% ↓ / 92% ↑ / 100% ✓ — N/A on desktop
memory      = true
disk        = true
load        = false
locale      = false
ip          = false   # local IP — reads /proc/net/fib_trie
ssh         = false   # SSH running/not — reads /proc/net/tcp
ports       = false   # open TCP ports — reads /proc/net/tcp
swatches    = true
```

### colors

7 cycling label colors + accent + bar + sep. all catppuccin mocha names or hex:

```toml
[colors]
accent   = blue        # logo + hostname
username = mauve       # user part of user@host
c1       = blue        # label slot 1
c2       = sapphire    # label slot 2
c3       = sky         # ... cycles through visible fields
c4       = teal
c5       = green
c6       = yellow
c7       = peach
values   = subtext1    # all field values
sep      = overlay0    # separator line
bar      = blue        # memory bar fill
```

named accents: `rosewater` `flamingo` `pink` `mauve` `red` `maroon` `peach` `yellow`
`green` `teal` `sky` `sapphire` `blue` *(default)* `lavender`

or raw hex: `accent = #CBA6F7`

---

## --blackhole

animated M87-style accretion disk renders alongside your sysinfo.

```bash
arcfetch --blackhole              # runs ~3 seconds
arcfetch --blackhole --t 0        # infinite — your terminal is a screensaver
arcfetch --blackhole --t 30       # exactly 30 seconds
```

physics: real doppler brightening — approaching side glows yellow/warm,
receding side goes red/dark. photon sphere in cyan. corona wisps in mauve.
Ctrl+C always restores cursor cleanly.

---

## all flags

```
-h,  --help                 usage screen
-V,  --version              version + E=mc² joke
     --config               config reference + write sample file
     --preset <n>           full | minimal | hacker | science
     --blackhole            animated M87 accretion disk
     --t <secs>             0 = infinite, N = N seconds
     --logo <n>             arch | ascii | tux | dna | atom | wave | emc2 | pi | custom
     --logo-file <path>     use any ASCII art text file as logo
     --accent <color>       hex (#RRGGBB) or catppuccin name
     --no-color             strip all ANSI — pipe-friendly
```

---

## not for you if

- you use ubuntu
- you use manjaro and call it arch
- you think neofetch is fast enough
- you're satisfied with your terminal looking boring

---

## requirements

- arch linux (reads `/var/lib/pacman/local` for package count — shows `unknown` elsewhere)
- rust 1.63+ for `thread::scope` support (`rustup update`)
- true-color terminal (anything made after 2015)

---

*built for arch. written in rust. themed in catppuccin.*
*feynman said nature uses the longest threads. this binary uses two.*
