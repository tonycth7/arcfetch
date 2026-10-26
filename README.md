# arcfetch

> *this is for you if you use arch. but it works everywhere else too.*

arch linux sysinfo that's so fast it's almost rude — ~1.2 ms default. one `libc::write` and done. catppuccin mocha. 60+ logos. zero overthinking. only deps are `libc` and `lexopt`.

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
arcfetch --blackhole          # plays ~3s, then returns to your prompt
arcfetch --blackhole --pin    # keep it pinned above the shell (--unpin stops it)
arcfetch --blackhole --t 0    # play until Ctrl+C
arcfetch --blackhole --t 10   # play for 10 seconds
```

it pulls your system info alongside the disk, so it's still functional — you get the flex *and* the data. and because it's pure rust with zero dependencies (libc doesn't count), there's no ffmpeg, no opengl, no bullet hell of missing libraries. the terminal is the gpu.

the disk renders as a rotating ring of block characters (`░` `▒` `▓` `█`) around a dark center — approaching side is brighter from doppler beaming, photon ring glows blue, outer halo fades to purple. background stars twinkle, shooting stars streak. all deterministic, all zero-dependency.

---

## speed

| run          | time     |
|--------------|----------|
| arcfetch     | ~1.2 ms  |
| fastfetch    | ~44 ms   |
| neofetch     | ~300 ms  |

no threads, no subprocesses, no waiting. cpuid for the cpu (zero i/o). `libc::sysinfo` + `libc::uname` for uptime and kernel — one syscall each. two-tier cache (`/dev/shm` + `~/.cache`) means the second run is even faster than the first. the release build is statically linked, so the dynamic loader is out of the hot path.

default fields: os, kernel, uptime, pkgs, shell, de/wm, term, cpu, gpu, memory, plus battery and disk when present. configurable via `[show]` in config or `--full` / `--preset`.

---

## install

**one-liner:**
```bash
curl -fsSL https://raw.githubusercontent.com/tonycth7/arcfetch/main/install.sh | bash
```

**prebuilt:** Linux x86_64, Android arm64/armv7/x86/x86_64, macOS Intel/Apple Silicon, FreeBSD and NetBSD — see [GitHub releases](https://github.com/tonycth7/arcfetch/releases). macOS builds are unsigned; if Gatekeeper blocks one, run `xattr -dr com.apple.quarantine arcfetch`.

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
Binary only; use the one-liner, AUR or manual install for the shipped presets.

**manual:**
```bash
git clone https://github.com/tonycth7/arcfetch
cd arcfetch
./install.sh
```

---

## quick start

```bash
arcfetch                          # default — clean, fast
arcfetch --logo mini              # compact ascii arch
arcfetch --preset 2               # a shipped JSONC layout
arcfetch --preset 20              # another one (--list-presets for all)
arcfetch --preset ~/my.jsonc      # or your own JSONC file
arcfetch --config ~/.config/fastfetch/config.jsonc  # fastfetch JSONC
arcfetch --full                   # every field
arcfetch --no-color               # plain text, pipe-friendly
```

---

## logos

```bash
arcfetch                          # block arch (default)
arcfetch --logo mini              # compact 7-line arch
arcfetch --logo ascii             # dotty neofetch-style
arcfetch --logo tux               # linux penguin
arcfetch --logo nix               # nixos snowflake
arcfetch --logo gentoo            # gentoo G
arcfetch --logo bsd               # BSD daemon
arcfetch --logo android           # android robot
arcfetch --logo termux            # termux terminal
arcfetch --logo pi                # raspberry pi
arcfetch --logo aurora            # northern lights
arcfetch --logo auto              # detect from /etc/os-release
arcfetch --logo custom            # ~/.config/arcfetch/logo.txt
arcfetch --logo-file <path>       # any ascii art or image (kitty protocol)
```

PNG/JPG/GIF logos sit beside the info by default. Set `[logo] position = "right"`
to put the image on the right, or `"top"` for the old above/below layout;
`width`/`height` control its cell size (height keeps the aspect when omitted).

---

## display modes

```bash
arcfetch --blackhole              # m87 accretion disk, ~3s then back to shell
arcfetch --blackhole --pin        # keep the disk pinned above the shell
arcfetch --blackhole --t 0        # play until Ctrl+C
arcfetch --mandelbrot [iter]      # mandelbrot fractal as logo
arcfetch --quantum                # wave-function collapse
arcfetch --quantum --t 5          # 5s quantum animation
arcfetch --cosmic                 # full-screen starfield + moon phase
arcfetch --cosmic --t 10          # 10s cosmic scene
arcfetch --animate --logo aurora  # play the aurora logo, then return
arcfetch --pin                    # pin the default logo animation
arcfetch --unpin                  # remove the pin and reset the terminal
arcfetch --animate shimmer        # play a logo with shimmer
arcfetch --animation saturn       # play a bundled frame animation
arcfetch --pin --animation saturn # pin those frames above the shell
```

`--animate`, `--animation` and `--blackhole` **play in place** by default: they
run for `[animation] play_seconds` (3 s) and hand the terminal back to your
shell. Add `--pin` to keep the art pinned above the shell instead; a pinned run
lasts until `--unpin`/`--stop` or `--t N`. `--cosmic` is a full-screen scene on
the alternate screen, so the two do not compete. Display modes are mutually
exclusive; `--animation` implies `--animate`.

---

## animations

arcfetch ships the frame sets from
[animfetch](https://github.com/Andrew-Velox/animfetch) — cats, foxes, space,
and a few coloured sets:

```bash
arcfetch --list-animations       # every bundled and custom animation
arcfetch --animation cat-run     # play it, then return to the shell
arcfetch --pin --animation earth # pin it above the shell
```

Bundled names: `butterfly`, `cat-run`, `cat-tail`, `chicken-run`, `deer-run`,
`dolphin-swim`, `earth`, `fox-run`, `girl-dance`, `horse-run`, `icosahedron`,
`pikachu`, `rabbit-run`, `saturn`, `squirrel-run`, `suba-cat`, `tree`,
`yin-yang`, plus the coloured `bat`, `gojo-dance2`, `mew`, and the frame-based
`blackhole` (`--animation blackhole`; `--blackhole` is the procedural disk).

---

## make your own animation

No compiler or rebuild needed. Put one text file per frame in
`~/.config/arcfetch/animations/<name>/` and play it:

```bash
mkdir -p ~/.config/arcfetch/animations/pulse
cat > ~/.config/arcfetch/animations/pulse/01.txt <<'EOF'
   ▄▄▄▄▄
  █  ●  █
   ▀▀▀▀▀
EOF
cat > ~/.config/arcfetch/animations/pulse/02.txt <<'EOF'
   ▄▄▄▄▄
  █  ○  █
   ▀▀▀▀▀
EOF

arcfetch --animation pulse       # play, then return to the shell
arcfetch --pin --animation pulse # pin above the shell
arcfetch --animation ./my-frames # or any directory path
arcfetch --list-animations       # show names and frame counts
```

Frames play in filename order (`01.txt`, `02.txt`, …), so zero-pad the names.
Frames are padded to the widest line, and large art is scaled to fit the
terminal. Frames are used verbatim, so animfetch-style art containing `$`, `$$`
or `$R` works as drawn; ANSI colours are kept as they are. Art without its own
colour gets the vertical gradient.

The same format as animfetch works directly: point `--animation` at any
animfetch folder, for example
`arcfetch --animation ~/Projects/animfetch/assets/anim/animals/cat-run`.

Full details, config keys and a copy-paste template are in
[USER_GUIDE.md](USER_GUIDE.md).

The gradient is animfetch's default ramp, independent of `--scheme`. Override it
with any hex colours:

```toml
[animation]
gradient = ["#f5c95c", "#f17f5b", "#f45c82", "#b07cf4", "#74a4ff"]
gradient_speed = 0.0     # rows per frame; > 0 lets the ramp crawl
fps = 30                 # animation frame rate (1-120)
play_seconds = 3         # how long play mode runs before the shell returns
art_color = "own"        # "theme" ignores frame ANSI and forces the gradient
# gradient = []          # fall back to the scheme's logo palette wave
```

---

## config

```bash
arcfetch --config     # print the full reference and create config.toml
```

Everything is optional; defaults are sane. A ready-to-read copy is in
[`config.toml.example`](config.toml.example), and the full walkthrough lives in
[USER_GUIDE.md](USER_GUIDE.md). The short version:

```toml
# rows
# info_style = "classic"   # classic | minimal | boxed | sections | tree | iconic | card | chips
# modules = ["os", "host", "kernel", "uptime", "packages", "shell", "de", "term", "cpu", "gpu", "memory", "disk", "colors"]

[colors]
accent   = blue
username = mauve
hostname = blue
values   = subtext1

[display]
labels    = true
key_width = 9

[logo]
# name = arch

[animation]
effect       = none      # none | shimmer | scanline | boot
intro_ms     = 700       # 0 disables the normal-mode intro
play_seconds = 3         # how long --animate/--blackhole play
fps          = 30
gradient     = ["#f5c95c", "#f17f5b", "#f45c82", "#b07cf4", "#74a4ff"]
gradient_speed = 0.0     # > 0 lets the gradient crawl
art_color    = "own"     # theme = force the gradient
```

Follow your desktop/wallpaper palette instead:

```toml
color_source = "palette"
palette_files = [
  "~/.local/state/quickshell/user/generated/colors.json",  # matugen
  "~/.cache/wal/colors.json",                              # pywal
  "~/.cache/wallust/colors.json",                          # wallust
]
```

`color_source = "palette"` replaces the accent, values and logo/animation
gradient after `[colors]`; names the file does not define keep their value.
`--scheme`, `--accent` and `--color` still win. `arcfetch --palette` forces it
for one run.

`info_style` picks a starting layout; `modules`, `[[modules]]`, `[[custom]]` and
`[overrides.*]` then shape it row by row. A fastfetch JSONC config can be loaded
directly with `--config` or `--preset`; fields arcfetch does not collect
disappear instead of printing literal placeholders. Package rows accept
fastfetch's `disabled` list and `combined` flag to pick or merge managers (see
[`USER_GUIDE.md`](USER_GUIDE.md#choosing-package-managers)).

```bash
arcfetch --logo nix               # nixos snowflake even if config says arch
arcfetch --preset 2               # a shipped JSONC layout (--list-presets)
```

---

## all flags

```
-h,  --help               this
-V,  --version            version + e=mc²
     --config [file]      config reference + sample, or load a config file
     --full, --all        show every data field
     -s, --structure <list>  only the listed data fields
     --json               flat JSON object of the enabled fields
     --watch              clear and re-run every second (Ctrl+C stops)
     --preset <name|file> load a JSONC preset (see --list-presets)
     --list-presets       list shipped JSONC presets
     --scheme <name>      palette preset (auto = light/dark terminal)
     --logo <name>        arch | mini | ascii | tux | nix | gentoo | bsd | android | termux | auto | custom
     --logo-file <path>   ascii art or image file (supports $1..$9 color tokens)
     --list-logos         every built-in logo with a preview
     --list-schemes       every palette with role swatches and hex
     --accent <color>     hex (#rrggbb) or catppuccin name
     --palette            use the first usable configured palette file for this run
     --color <scheme>     logo color: name | #hex | random
     --box [style]        frame output (round | square | double | heavy | dashed | ascii)
     --no-color           strip ansi
     --blackhole          m87 accretion disk; plays then returns to the shell
     --mandelbrot [iter]  mandelbrot fractal (default 64)
     --quantum            wave-function collapse
     --cosmic             full-screen starfield + moon
     --animate [effect]   play a live logo (shimmer | scanline | boot | frames)
     --pin                keep the animation pinned above the shell
     --animation <name>   play bundled or custom frames (implies --animate)
     --fps <n>            animation frame rate override (1-120; default 30)
     --stop, --unpin      remove the pinned animation and reset the terminal
     --list-animations    bundled and custom frame sets
     --no-animate         skip the normal-mode intro
     --t <secs>           play/pin duration; 0 = Ctrl+C (play) or --stop (pin)
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

the parts worth knowing:

| file              | what                                  |
|-------------------|---------------------------------------|
| `main.rs`         | cli, render, anims, entrypoint, color bar |
| `info.rs`         | 25+ collectors — cpu, gpu, kernel, mem, swap, bios, board, ip … |
| `modules.rs`      | opt-in collectors — disk/network io, top processes, edid, wifi … |
| `fastfetch.rs`    | fastfetch JSONC translation           |
| `pkgs.rs`         | pacman, dpkg, rpm, nix, apk, portage, brew, pkg, pm |
| `cache.rs`        | `/dev/shm` + `~/.cache` with ttl      |
| `config.rs`       | catppuccin palette + toml parser      |
| `scheme.rs`       | canonical palettes and semantic roles |
| `net.rs`          | std-only http client (public ip, weather) |
| `logos.rs`        | 60+ logos + procedural aurora + effects |
| `anim.rs`         | bundled + user animation frames, terminal scaling |
| `build.rs`        | embeds `assets/animations/` frame sets |
| `mandelbrot.rs`   | mandelbrot set in rust                |
| `cosmic.rs`       | full-screen starfield + moon phase    |
| `flake.nix`       | nix flake for reproducible builds     |

---

## requirements

- linux, android 7+/termux, macos, freebsd, netbsd or openbsd
- the prebuilt Linux x86_64 binary is static-pie, so there is no glibc version to match (Android, macOS and BSD builds link against their platform libc)
- linux gets every collector; other platforms get the core fields (os, kernel, uptime, packages, shell, desktop/wm, terminal, cpu, gpu, memory) via `sysctl`/`sw_vers`/`pmset`/`pciconf`/`acpiconf`
- linux-only extras (sysfs temps, `/proc` process/network counters, btrfs, monitors) degrade to `N/A`
- cross builds: `./scripts/build-all.sh` (Linux x86_64, Android arm64/armv7/x86/x86_64, macOS both arches, FreeBSD, NetBSD; cargo + zig + Android NDK). Without an NDK, `cargo check --target ...` still works.
- true-color terminal (anything from 2015 onwards)
- curiosity about what a terminal can do

---

*built for arch. runs on anything. written in rust. themed in catppuccin. zero bloat.*

*inspired by [fastfetch](https://github.com/fastfetch-cli/fastfetch) and animfetch.*
