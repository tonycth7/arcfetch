# arcfetch user guide

This is the long-form companion to the [README](README.md): configuration,
animations, and the small details that do not fit in the quick start.

Everything here is optional. arcfetch runs with sane defaults and no config.

---

## configuration

### where it lives

```
$XDG_CONFIG_HOME/arcfetch/config.toml      # default: ~/.config/arcfetch/config.toml
$XDG_CONFIG_HOME/arcfetch/config.jsonc     # JSONC / fastfetch config
$XDG_CONFIG_HOME/arcfetch/animations/      # your frame animations
```

`arcfetch --config` prints a reference and creates `config.toml` if it does not
exist. The annotated sample is also in
[`config.toml.example`](config.toml.example).

`config.toml` wins over `config.jsonc`. A path passed with `--config` or
`--preset` replaces the file for that run. JSONC presets can be shorthand or
full fastfetch configs; fastfetch-only fields are translated where they have an
arcfetch equivalent and skipped where they do not.

### precedence

From weakest to strongest:

1. built-in defaults
2. `config.toml` (or the selected JSONC preset)
3. `--scheme` (replaces the palette)
4. `--accent` / `ARCFETCH_ACCENT` (accent + hostname)
5. `--color` (logo accent only)

`--full` / `--all` and `-s/--structure` run after the file is loaded.
`--palette` forces the wallpaper palette for one run.

### the short version

Most people only touch one or two of these:

```toml
# start from a layout
info_style = "classic"   # classic | minimal | boxed | sections | tree | iconic | card | chips

# or choose the rows yourself
modules = ["os", "host", "kernel", "uptime", "packages", "shell", "de", "term",
           "cpu", "gpu", "memory", "disk", "colors"]

[colors]
accent   = blue
username = mauve
hostname = blue
values   = subtext1

[display]
labels    = true
key_width = 9

[logo]
name = arch              # or auto, mini, nix, tux, …

[animation]
effect       = none      # none | shimmer | scanline | boot
intro_ms     = 700       # normal-mode intro; 0 disables it
play_seconds = 3         # how long --animate / --blackhole play
fps          = 30
gradient     = ["#f5c95c", "#f17f5b", "#f45c82", "#b07cf4", "#74a4ff"]
gradient_speed = 0.0     # > 0 lets the gradient crawl
art_color    = "own"     # theme = force the gradient
```

### layouts and rows

`info_style` chooses a starting layout. `modules`, `[[modules]]`, `[[custom]]`
and `[overrides.<row>]` then shape it row by row:

```toml
[[modules]]
type = "os"
key = "distro"
format = "{key}: {value}"

[[custom]]
label = "Owner"
value = "POKKITA"

[overrides.uptime]
label = "Up"
command = "uptime -p"
```

This is the same model the JSONC presets use. A preset can be a small layout
(`--preset 2`) or a full fastfetch config (`--config ~/.config/fastfetch/config.jsonc`).

### choosing package managers

The `packages`/`pkgs` row lists every detected manager by default. Hide the
ones you do not want and merge the rest with fastfetch's `disabled` and
`combined` options:

```jsonc
{ "type": "packages", "disabled": ["appimage", "nix"], "combined": true }
```

Names: `appimage`, `apk`, `dpkg`, `flatpak`, `nix`, `pacman`, `portage`
(alias `emerge`), `rpm`, `xbps`, `pkg`, `brew`, `macports`. A family name hides
all its sub-managers, so `flatpak` drops system and user apps together.
`combined = true` merges `flatpak-system`/`flatpak-user` into `flatpak` and
`nix-*` into `nix`.

The TOML form is the same:

```toml
[[modules]]
type = "packages"
disabled = ["appimage"]
combined = true
```

With nothing disabled, every manager that exists is shown automatically.

### palette files

To follow your wallpaper/desktop theme:

```toml
color_source = "palette"
palette_files = [
  "~/.local/state/quickshell/user/generated/colors.json",  # matugen
  "~/.cache/wal/colors.json",                              # pywal
  "~/.cache/wallust/colors.json",                          # wallust
]
palette_accent   = "primary"       # pywal/wallust: color4
palette_value    = "on_surface"    # pywal/wallust: color7
palette_gradient = ["primary", "secondary", "tertiary"]
```

The first file that exists and parses wins. Names it does not define keep their
configured value. Matugen role names, pywal/wallust `colorN`, and dotted names
(`special.background`) all resolve. `--palette` forces it for one run.

The same keys work under `[palette]` as `files`, `accent`, `value` and
`gradient`.

---

## animations

arcfetch has three kinds of art:

| kind | examples | how to run |
|------|----------|------------|
| procedural logo animation | `atom`, `dna`, `arcreactor`, `aurora` | `arcfetch --animate --logo aurora` |
| frame animation | bundled sets + your frames | `arcfetch --animation saturn` |
| display mode | blackhole, cosmic, quantum, mandelbrot | `arcfetch --blackhole` |

### play vs pin

`--animate`, `--animation` and `--blackhole` **play in place** by default: the
alternate screen is used, the animation runs for `[animation] play_seconds`
(default 3 s), and the terminal is handed back to your shell.

Add `--pin` to keep the art in a pane above the shell while you work:

```bash
arcfetch --animate --pin              # pin the logo animation
arcfetch --animation cat-run --pin    # pin a frame set
arcfetch --blackhole --pin            # pin the M87 disk
arcfetch --unpin                      # remove the pin
```

`--t N` sets the duration for one run. `--t 0` means until Ctrl+C in play mode,
or until `--unpin`/`--stop` in pin mode. Pinned art pauses while a foreground
program owns the terminal (vim, fzf, less) and repaints when the prompt returns.

### bundled frame animations

`arcfetch --list-animations` prints every bundled and custom set. Bundled names:

```
animals   butterfly cat-run cat-tail chicken-run deer-run dolphin-swim
          fox-run horse-run rabbit-run squirrel-run suba-cat
space     earth saturn blackhole
objects   tree yin-yang icosahedron
coloured  bat gojo-dance2 mew pikachu
other     girl-dance
```

They are the frame sets from
[animfetch](https://github.com/Andrew-Velox/animfetch), embedded at build time
and credited in [`assets/animations/README.md`](assets/animations/README.md).
Large frames are scaled down to the terminal, keeping their aspect ratio.

A user animation with the same name shadows the bundled one.

### write your own

A directory under `~/.config/arcfetch/animations/<name>/`, one text file per
frame, filename order. `.txt` and `.ansi` are both accepted.

```bash
mkdir -p ~/.config/arcfetch/animations/pulse
printf '   ▄▄▄▄▄\n  █  ●  █\n   ▀▀▀▀▀\n' > ~/.config/arcfetch/animations/pulse/01.txt
printf '   ▄▄▄▄▄\n  █  ○  █\n   ▀▀▀▀▀\n' > ~/.config/arcfetch/animations/pulse/02.txt

arcfetch --animation pulse
arcfetch --pin --animation pulse
```

Rules that matter:

- Frames play in filename order. Zero-pad (`01`, `02`, …) so `10` does not sort
  before `2`.
- All frames are padded to the widest line and tallest frame. Keep the art the
  same size for a steady layout.
- Art is used verbatim. `$`, `$$` and `$R` are literal, so animfetch prints and
  shell art survive.
- ANSI colours in a frame are kept (`art_color = "own"`). Escape-free art gets
  the vertical gradient. `art_color = "theme"` strips authored ANSI and lets the
  gradient win.
- Any animfetch folder works directly:
  `arcfetch --animation ~/Projects/animfetch/assets/anim/animals/cat-run`
- Relative and absolute paths work. A bare name is looked up in your animation
  directory first, then the bundled sets.

### animation config

```toml
[animation]
# glyph effect for logo animation
effect       = none      # none | shimmer | scanline | boot

# normal-mode intro (a short self-overwriting animation before the static fetch)
intro_ms     = 700       # 0 disables; capped at 24 frames

# how long play mode runs before returning to the shell
play_seconds = 3

# frame rate for the intro, play mode and pinned pane
fps          = 30        # 1-120

# vertical gradient for plain art; [] = scheme palette wave
gradient       = ["#f5c95c", "#f17f5b", "#f45c82", "#b07cf4", "#74a4ff"]
gradient_speed = 0.0     # rows per frame; 0 keeps the ramp still

# own = keep frame ANSI; theme = force the gradient
art_color = "own"
```

One-key shortcuts that used to do the same job are gone from the generated
config:

| old | new |
|-----|-----|
| `enabled = false` | `intro_ms = 0` |
| `gradient_scroll = true` | `gradient_speed = 1.0` (any value above 0 scrolls) |

`enabled` and `gradient_scroll` are still read when present, so an existing file
keeps the same behaviour. `wave` is no longer used: an empty `gradient` already
selects the scheme palette wave.

---

## see also

- [`config.toml.example`](config.toml.example) — every key, annotated
- `arcfetch --help` — flags
- `arcfetch --config` — terminal reference and sample
- [`assets/animations/README.md`](assets/animations/README.md) — animation credits
