# arcfetch

> **this is not for you if you don't use arch btw**

a blazing fast system info tool written in rust. no subprocesses, no python, no waiting.
just your machine's stats, catppuccin mocha colors, and a black hole if you're feeling dramatic.

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

## why does this exist

because neofetch is dead, fastfetch is written in C like it's 1992, and i wanted something that:

- starts faster than your brain processes the command
- looks good with catppuccin mocha
- has a black hole mode (you'll understand when you see it)
- is written in rust because of course it is

**speed comparison:**

| tool      | time    |
|-----------|---------|
| arcfetch  | ~0.009s |
| fastfetch | ~0.031s |
| neofetch  | ~0.3s   |
| screenfetch | lol   |

---

## install

you need rust. if you don't have rust yet:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

then:

```bash
git clone https://github.com/yourname/arcfetch
cd arcfetch
chmod +x install.sh
./install.sh
```

or manually if you don't trust install scripts (fair):

```bash
cargo build --release
cp target/release/arcfetch ~/.local/bin/arcfetch
```

make sure `~/.local/bin` is in your PATH:

```bash
# add this to your .zshrc / .bashrc / config.fish
export PATH="$HOME/.local/bin:$PATH"
```

---

## auto-run on terminal open

the whole point is to see this every time you open a terminal. add one line to your shell config:

**zsh** (`~/.zshrc`):
```bash
arcfetch
# or with a specific logo you like:
arcfetch --logo wave
arcfetch --logo dna --accent mauve
```

**bash** (`~/.bashrc`):
```bash
arcfetch --logo ascii
```

**fish** (`~/.config/fish/config.fish`):
```fish
arcfetch --logo atom --accent sapphire
```

pick a logo, set an accent, forget about it. every new terminal will greet you with your stats.

---

## logos

six to choose from:

```bash
arcfetch                    # default block arch logo ▟███▙
arcfetch --logo ascii       # classic dotty arch ascii
arcfetch --logo tux         # linux tux penguin
arcfetch --logo dna         # DNA double helix (A═T, G═C base pairs)
arcfetch --logo atom        # bohr atom diagram (iron, Fe)
arcfetch --logo wave        # schrödinger wave ψ(x,t) = Ae^i(kx−ωt)
```

if you're putting it in your shell config, pick the one that doesn't get old. `wave` and `dna` are the most aesthetic imo but you do you.

---

## colors / accent

your logo renders in whatever accent color you set. default is catppuccin mocha blue.

**quick change (inline):**
```bash
arcfetch --accent mauve
arcfetch --accent '#CBA6F7'
arcfetch --accent teal
```

**permanent (config file) — recommended:**
```bash
mkdir -p ~/.config/arcfetch
echo "mauve" > ~/.config/arcfetch/accent
# or a hex color:
echo "#CBA6F7" > ~/.config/arcfetch/accent
```

**or via env var (useful for testing):**
```bash
ARCFETCH_ACCENT=pink arcfetch
```

**all catppuccin mocha accent names:**

| name       | vibe              |
|------------|-------------------|
| rosewater  | warm pink-white   |
| flamingo   | soft pink         |
| pink       | hot pink          |
| mauve      | purple (popular)  |
| red        | bright red        |
| maroon     | dark red          |
| peach      | warm orange       |
| yellow     | golden            |
| green      | mint green        |
| teal       | cyan-green        |
| sky        | light blue        |
| sapphire   | deep blue         |
| blue       | mocha blue (default) |
| lavender   | soft purple       |

---

## --blackhole

the reason arcfetch exists honestly.

```bash
arcfetch --blackhole              # runs for ~3 seconds
arcfetch --blackhole --t 0        # infinite loop. your terminal is now a screensaver
arcfetch --blackhole --t 20       # runs for exactly 20 seconds
arcfetch --blackhole --t 60       # one minute. tell people you're busy
```

it renders a real M87-style accretion disk simulation:
- event horizon in the center (pure void)
- photon sphere glowing faint cyan
- accretion disk with actual doppler brightening — approaching side is yellow/warm, receding side is red/cool
- corona wisps in mauve and lavender

for `--t 0`, hit **Ctrl+C** to exit. cursor always comes back, no broken terminal.

if you have a dual monitor setup, put `arcfetch --blackhole --t 0` in a tmux pane and leave it running. you're welcome.

---

## all flags

```
-h, --help                  usage + config file path on your system
-V, --version               version info (with a physics joke)
--blackhole                 animated M87 black hole
  --t <secs>                0 = infinite, N = N seconds
--logo <name>               arch / ascii / tux / dna / atom / wave
--accent <color>            hex (#RRGGBB) or catppuccin name
--no-color                  plain text, no ANSI — good for piping
```

---

## pipe-friendly

```bash
# save your sysinfo to a file
arcfetch --no-color > sysinfo.txt

# share it or grep it
arcfetch --no-color | grep cpu
```

---

## version

```bash
arcfetch -V
```

prints version, theme info, and `E = mc²  where E = startup time ≈ 0`. because why not.

---

## how is it this fast

- reads everything from `/proc`, `/sys`, and env vars directly — zero subprocess spawns
- all 14 system readers run in parallel via `thread::scope`
- entire output written in one `BufWriter` flush
- release build: `lto=thin`, `strip=true`, `codegen-units=1`, `panic=abort`

the binary ends up around 200KB stripped. your bash history file is probably bigger.

---

## requirements

- arch linux (or close enough — it reads `/var/lib/pacman/local` for package count)
- rust 1.63+ for `thread::scope`
- a terminal with true-color support (basically everything made after 2015)
- a nerd font recommended but not required

---

## not for you if

- you use ubuntu
- you use manjaro and call it arch
- you think neofetch is fast enough
- you're satisfied with your terminal looking boring

---

*built for arch, written in rust, themed in catppuccin. what more do you want.*
