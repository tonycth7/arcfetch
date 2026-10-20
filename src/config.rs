// src/config.rs — Catppuccin Mocha palette + Config struct + file loader
//
// Config file (~/.config/arcfetch/config.toml):
//
//   [colors]
//   accent   = mauve
//   username = mauve
//   hostname = blue
//   c1 = blue       # label slot 1  (cycles: os, term, disk …)
//   c2 = sapphire
//   c3 = sky
//   c4 = teal
//   c5 = green
//   c6 = yellow
//   c7 = peach
//   values = subtext1
//   sep    = overlay0
//   bar    = blue
//
//   [show]
//   os=true  kernel=true  uptime=true  res=false  pkgs=true
//   shell=true  de_wm=true  term=true  cpu=true  gpu=true
//   memory=true  disk=true  load=false  locale=false  swatches=true
//
//   [logo]
//   name = arch      # built-in: arch | ascii | tux | nix | gentoo | mini | auto
//   file = ~/.config/arcfetch/logo.txt   # custom ASCII file (overrides name)
//
//   [template]
//   preset = full    # full | minimal | hacker | science

use std::{env, fs};

// ── Catppuccin Mocha — 24-bit true-colour ────────────────
pub const ROSEWATER: &str = "\x1b[38;2;245;224;220m";
pub const FLAMINGO:  &str = "\x1b[38;2;242;205;205m";
pub const PINK:      &str = "\x1b[38;2;245;194;231m";
pub const MAUVE:     &str = "\x1b[38;2;203;166;247m";
pub const RED:       &str = "\x1b[38;2;243;139;168m";
pub const MAROON:    &str = "\x1b[38;2;235;160;172m";
pub const PEACH:     &str = "\x1b[38;2;250;179;135m";
pub const YELLOW:    &str = "\x1b[38;2;249;226;175m";
pub const GREEN:     &str = "\x1b[38;2;166;227;161m";
pub const TEAL:      &str = "\x1b[38;2;148;226;213m";
pub const SKY:       &str = "\x1b[38;2;137;220;235m";
pub const SAPPHIRE:  &str = "\x1b[38;2;116;199;236m";
pub const BLUE:      &str = "\x1b[38;2;137;180;250m";
pub const LAVENDER:  &str = "\x1b[38;2;180;190;254m";
pub const TEXT:      &str = "\x1b[38;2;205;214;244m";
pub const SUBTEXT1:  &str = "\x1b[38;2;186;194;222m";
pub const OVERLAY0:  &str = "\x1b[38;2;108;112;134m";
pub const BOLD:      &str = "\x1b[1m";
pub const RESET:     &str = "\x1b[0m";

// ── Official distro colors ────────────────────────────────
pub const NIX_DARK:   &str = "\x1b[38;2;82;119;195m";    // #5277C3
pub const NIX_LIGHT:  &str = "\x1b[38;2;126;186;228m";   // #7EBAE4
pub const GENTOO_DARK:  &str = "\x1b[38;2;84;72;122m";     // #54487A
pub const GENTOO_LIGHT: &str = "\x1b[38;2;155;114;176m";   // #9B72B0

// ── Catppuccin lookup (static) ───────────────────────────
fn catppuccin_ansi(name: &str) -> Option<&'static str> {
    Some(match name {
        "rosewater" => ROSEWATER,
        "flamingo"  => FLAMINGO,
        "pink"      => PINK,
        "mauve"     => MAUVE,
        "red"       => RED,
        "maroon"    => MAROON,
        "peach"     => PEACH,
        "yellow"    => YELLOW,
        "green"     => GREEN,
        "teal"      => TEAL,
        "sky"       => SKY,
        "sapphire"  => SAPPHIRE,
        "blue"      => BLUE,
        "lavender"  => LAVENDER,
        "text"      => TEXT,
        "subtext1"  => SUBTEXT1,
        "overlay0"  => OVERLAY0,
        _ => return None,
    })
}

/// Legacy lookup used for logo colours; unknown names fall back to blue.
#[allow(dead_code)]
pub fn name_to_ansi(name: &str) -> &'static str {
    catppuccin_ansi(&name.trim().to_ascii_lowercase()).unwrap_or(BLUE)
}

#[inline]
fn hex_byte(s: &str) -> Option<u8> { u8::from_str_radix(s, 16).ok() }

/// Resolve a colour spec to an ANSI escape string.
///
/// Accepts: catppuccin names (`mauve`), extra names (`black`, `white`,
/// `gray`, `orange`, `purple`, `cyan`, …), `ansi_*` names, `#RRGGBB` / `#RGB`,
/// `rgb(r,g,b)`, and `colorN` or a bare `N` (0–255) for the xterm-256 palette.
pub fn resolve(name: &str) -> String {
    let name = name.trim().to_ascii_lowercase();

    // #RRGGBB or #RGB
    if let Some(hex) = name.strip_prefix('#') {
        let rgb = match hex.len() {
            6 => (hex_byte(&hex[0..2]), hex_byte(&hex[2..4]), hex_byte(&hex[4..6])),
            3 => {
                let f = |i: usize| hex_byte(&hex[i..i + 1]).map(|v| v.saturating_mul(17));
                (f(0), f(1), f(2))
            }
            _ => (None, None, None),
        };
        if let (Some(r), Some(g), Some(b)) = rgb {
            return format!("\x1b[38;2;{};{};{}m", r, g, b);
        }
    }

    // rgb(r,g,b)
    if let Some(inner) = name.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].parse::<u8>(), parts[1].parse::<u8>(), parts[2].parse::<u8>(),
            ) {
                return format!("\x1b[38;2;{};{};{}m", r, g, b);
            }
        }
    }

    // xterm-256: colorN or a bare N
    let idx = name.strip_prefix("color").unwrap_or(&name);
    if let Ok(n) = idx.parse::<u8>() {
        return format!("\x1b[38;5;{}m", n);
    }

    // catppuccin palette names
    if let Some(a) = catppuccin_ansi(&name) { return a.into(); }

    // extra named colours (none of these collide with catppuccin names)
    let named = match name.as_str() {
        "black" => Some(0),  "white" => Some(7),  "gray" | "grey" => Some(8),
        "silver" => Some(7), "orange" => Some(208), "navy" => Some(17),
        "lime" => Some(118), "aqua" => Some(14),  "fuchsia" => Some(13),
        "purple" | "magenta" => Some(5), "cyan" => Some(6),
        "ansi_black" => Some(0),   "ansi_red" => Some(1),
        "ansi_green" => Some(2),   "ansi_yellow" => Some(3),
        "ansi_blue" => Some(4),    "ansi_magenta" => Some(5),
        "ansi_cyan" => Some(6),    "ansi_white" => Some(7),
        "ansi_bright_black" => Some(8),   "ansi_bright_red" => Some(9),
        "ansi_bright_green" => Some(10),  "ansi_bright_yellow" => Some(11),
        "ansi_bright_blue" => Some(12),   "ansi_bright_magenta" => Some(13),
        "ansi_bright_cyan" => Some(14),   "ansi_bright_white" => Some(15),
        _ => None,
    };
    if let Some(n) = named { return format!("\x1b[38;5;{}m", n); }

    BLUE.into()
}

// ── Terminal colour depth ────────────────────────────────

/// How many colours the attached terminal can actually display.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorDepth {
    /// 24-bit `38;2;R;G;B`
    True,
    /// xterm-256 `38;5;N`
    Ansi256,
    /// the 16 base colours (`30–37`, `90–97`)
    Ansi16,
}

impl ColorDepth {
    /// Best depth the terminal advertises via `$COLORTERM` / `$TERM`.
    pub fn detect() -> Self {
        let ct = env::var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
        if ct.contains("truecolor") || ct.contains("24bit") {
            return ColorDepth::True;
        }
        let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
        if term.contains("direct") || term.contains("truecolor") {
            return ColorDepth::True;
        }
        if term.contains("256color") || ct.contains("256") {
            return ColorDepth::Ansi256;
        }
        ColorDepth::Ansi16
    }
}

/// Parse the `R;G;B` of a true-colour escape, if the spec is one.
fn parse_truecolor(spec: &str) -> Option<(u8, u8, u8)> {
    let inner = spec.strip_prefix("\x1b[38;2;")?.strip_suffix('m')?;
    let mut it = inner.split(';');
    let r = it.next()?.parse().ok()?;
    let g = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    Some((r, g, b))
}

/// Parse the index of an xterm-256 escape, if the spec is one.
fn parse_256(spec: &str) -> Option<u8> {
    spec.strip_prefix("\x1b[38;5;")?.strip_suffix('m')?.parse().ok()
}

/// RGB for an xterm-256 palette index.
fn xterm_rgb(n: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0),
        (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
        (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0),
        (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
    ];
    match n {
        0..=15 => BASE[n as usize],
        16..=231 => {
            let i = n - 16;
            let lv = |v: u8| -> u8 { if v == 0 { 0 } else { 55 + 40 * v } };
            (lv(i / 36), lv((i % 36) / 6), lv(i % 6))
        }
        _ => {
            let g = 8 + 10 * (n - 232);
            (g, g, g)
        }
    }
}

/// Nearest xterm-256 index for an RGB triple.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        if r < 8 { return 16; }
        if r > 248 { return 231; }
        return 232 + ((r as u16 - 8) * 24 / 247) as u8;
    }
    let q = |v: u8| -> u8 {
        if v < 48 { 0 }
        else if v < 115 { 1 }
        else { ((v as u16 - 35) / 40).min(5) as u8 }
    };
    16 + 36 * q(r) + 6 * q(g) + q(b)
}

/// Nearest of the 16 base colours for an RGB triple.
fn rgb_to_16(r: u8, g: u8, b: u8) -> u8 {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0), (205, 0, 0), (0, 205, 0), (205, 205, 0),
        (0, 0, 238), (205, 0, 205), (0, 205, 205), (229, 229, 229),
        (127, 127, 127), (255, 0, 0), (0, 255, 0), (255, 255, 0),
        (92, 92, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
    ];
    let mut best = 0u8;
    let mut best_d = u32::MAX;
    for (i, &(br, bg, bb)) in BASE.iter().enumerate() {
        let dr = r as i32 - br as i32;
        let dg = g as i32 - bg as i32;
        let db = b as i32 - bb as i32;
        let d = (dr * dr + dg * dg + db * db) as u32;
        if d < best_d { best_d = d; best = i as u8; }
    }
    best
}

/// Rewrite one colour escape to the nearest colour at `depth`.
fn downgrade(spec: &str, depth: ColorDepth) -> String {
    if depth == ColorDepth::True {
        return spec.to_string();
    }
    let (r, g, b) = match parse_truecolor(spec) {
        Some(rgb) => rgb,
        None => match parse_256(spec) {
            Some(n) if depth == ColorDepth::Ansi16 => xterm_rgb(n),
            _ => return spec.to_string(),
        },
    };
    match depth {
        ColorDepth::True => spec.to_string(),
        ColorDepth::Ansi256 => format!("\x1b[38;5;{}m", rgb_to_256(r, g, b)),
        ColorDepth::Ansi16 => {
            let n = rgb_to_16(r, g, b);
            if n < 8 { format!("\x1b[3{n}m") } else { format!("\x1b[9{}m", n - 8) }
        }
    }
}

/// Rewrite every colour in `c` so it renders on a terminal of the given depth.
pub fn apply_depth(c: &mut Colors, depth: ColorDepth) {
    if depth == ColorDepth::True {
        return;
    }
    let d = |s: &str| downgrade(s, depth);
    c.accent   = d(&c.accent);
    c.username = d(&c.username);
    c.hostname = d(&c.hostname);
    for l in c.labels.iter_mut() { *l = d(l); }
    c.values = d(&c.values);
    c.sep    = d(&c.sep);
    c.bar    = d(&c.bar);
    for l in c.logo.iter_mut() { *l = d(l); }
}

/// Byte length of a `38;2;…`/`38;5;…` SGR sequence starting at `s[i]`, if any.
fn sgr_colour_at(s: &str, i: usize) -> Option<usize> {
    let rest = &s[i..];
    if !rest.starts_with("\x1b[38;") {
        return None;
    }
    let b = rest.as_bytes();
    let mut j = 5;
    while j < b.len() && j < 20 {
        match b[j] {
            b'0'..=b'9' | b';' => j += 1,
            b'm' => return Some(j + 1),
            _ => return None,
        }
    }
    None
}

/// Rewrite every colour escape in `s` for `depth`. Used on the final render so
/// hard-coded colours (e.g. the Nix/Gentoo logos) are down-sampled too.
pub fn downgrade_str(s: &str, depth: ColorDepth) -> String {
    if depth == ColorDepth::True || !s.contains("\x1b[38;") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if let Some(len) = sgr_colour_at(s, i) {
            out.push_str(&downgrade(&s[i..i + len], depth));
            i += len;
            continue;
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

// ── Colour schemes ───────────────────────────────────────

pub type Rgb = (u8, u8, u8);

/// RGB triplets for a named scheme, in palette order:
/// rosewater, flamingo, pink, mauve, red, maroon, peach, yellow, green,
/// teal, sky, sapphire, blue, lavender, text, subtext1, overlay0.
pub fn scheme_rgb(name: &str) -> Option<[Rgb; 17]> {
    let v: [Rgb; 17] = match name.trim().to_ascii_lowercase().as_str() {
        "mocha" | "catppuccin" | "catppuccin-mocha" => [
            (245,224,220),(242,205,205),(245,194,231),(203,166,247),(243,139,168),
            (235,160,172),(250,179,135),(249,226,175),(166,227,161),(148,226,213),
            (137,220,235),(116,199,236),(137,180,250),(180,190,254),(205,214,244),
            (186,194,222),(108,112,134),
        ],
        "latte" | "catppuccin-latte" => [
            (220,138,120),(221,120,120),(234,118,203),(136,57,239),(210,15,57),
            (230,69,83),(254,100,11),(223,142,29),(64,160,43),(23,146,153),
            (4,165,229),(32,159,181),(30,102,245),(114,135,253),(76,79,105),
            (92,95,119),(156,160,176),
        ],
        "frappe" | "frappé" | "catppuccin-frappe" => [
            (242,213,207),(238,190,190),(244,184,228),(202,158,230),(231,130,132),
            (234,153,156),(239,159,118),(229,200,144),(166,209,137),(129,200,190),
            (153,209,219),(133,193,220),(140,170,238),(186,187,241),(198,208,245),
            (181,191,226),(115,121,148),
        ],
        "macchiato" | "catppuccin-macchiato" => [
            (244,219,214),(240,198,198),(245,189,230),(198,160,246),(237,135,150),
            (238,153,160),(245,169,127),(238,212,159),(166,218,149),(139,213,202),
            (145,215,227),(125,196,228),(138,173,244),(183,189,248),(202,211,245),
            (184,192,224),(110,115,141),
        ],
        "nord" => [
            (216,222,233),(208,135,112),(180,142,173),(180,142,173),(191,97,106),
            (191,97,106),(208,135,112),(235,203,139),(163,190,140),(143,188,187),
            (136,192,208),(129,161,193),(94,129,172),(129,161,193),(236,239,244),
            (216,222,233),(76,86,106),
        ],
        "dracula" => [
            (248,248,242),(255,121,198),(255,121,198),(189,147,249),(255,85,85),
            (255,85,85),(255,184,108),(241,250,140),(80,250,123),(139,233,253),
            (139,233,253),(98,114,164),(189,147,249),(189,147,249),(248,248,242),
            (248,248,242),(98,114,164),
        ],
        "gruvbox" | "gruvbox-dark" => [
            (235,219,178),(211,134,155),(211,134,155),(211,134,155),(251,73,52),
            (204,36,29),(254,128,25),(250,189,47),(184,187,38),(142,192,124),
            (131,165,152),(69,133,136),(131,165,152),(211,134,155),(235,219,178),
            (213,196,161),(146,131,116),
        ],
        "tokyo" | "tokyo-night" | "tokyonight" => [
            (247,118,142),(255,0,124),(187,154,247),(187,154,247),(247,118,142),
            (219,75,75),(255,158,100),(224,175,104),(158,206,106),(115,218,202),
            (125,207,255),(42,195,222),(122,162,247),(187,154,247),(192,202,245),
            (169,177,214),(86,95,137),
        ],
        "solarized" | "solarized-dark" => [
            (238,232,213),(211,54,130),(211,54,130),(108,113,196),(220,50,47),
            (203,75,22),(203,75,22),(181,137,0),(133,153,0),(42,161,152),
            (42,161,152),(38,139,210),(38,139,210),(108,113,196),(238,232,213),
            (147,161,161),(88,110,117),
        ],
        // Rosé Pine (main): rose, love, gold, pine, foam, iris, text, subtle, muted
        "rose-pine" | "rosepine" | "rose_pine" => [
            (235,188,186),(235,188,186),(235,111,146),(196,167,231),(235,111,146),
            (235,111,146),(246,193,119),(246,193,119),(49,116,143),(156,207,216),
            (156,207,216),(49,116,143),(156,207,216),(196,167,231),(224,222,244),
            (144,140,170),(110,106,134),
        ],
        "onedark" | "one-dark" | "atom" => [
            (171,178,191),(224,108,117),(198,120,221),(198,120,221),(224,108,117),
            (224,108,117),(209,154,102),(229,192,123),(152,195,121),(86,182,194),
            (86,182,194),(97,175,239),(97,175,239),(198,120,221),(171,178,191),
            (171,178,191),(92,99,112),
        ],
        // Kanagawa Wave: fujiWhite, sakuraPink, oniViolet, waveRed, autumnRed,
        // surimiOrange, carpYellow, springGreen, waveAqua2, spring/crystal blue
        "kanagawa" => [
            (220,215,186),(210,126,153),(210,126,153),(149,127,184),(228,104,118),
            (195,64,67),(255,160,102),(230,195,132),(152,187,108),(122,168,159),
            (127,180,202),(126,156,216),(126,156,216),(156,171,202),(220,215,186),
            (200,192,147),(114,113,105),
        ],
        "everforest" => [
            (211,198,170),(230,126,128),(230,126,128),(214,153,182),(230,126,128),
            (230,126,128),(230,152,117),(219,188,127),(167,192,128),(131,192,146),
            (127,187,179),(127,187,179),(127,187,179),(214,153,182),(211,198,170),
            (211,198,170),(133,146,137),
        ],
        // Ayu Dark (official): fg, red, purple, orange, yellow, green, cyan, blue
        "ayu" | "ayu-dark" => [
            (191,189,182),(240,113,120),(240,113,120),(210,166,255),(240,113,120),
            (217,87,87),(255,143,64),(255,180,84),(170,217,76),(149,230,203),
            (149,230,203),(57,186,230),(89,194,255),(210,166,255),(191,189,182),
            (191,189,182),(98,106,115),
        ],
        _ => return None,
    };
    Some(v)
}

/// Build a full [`Colors`] from a named scheme.
///
/// `"auto"` picks a light/dark default (`latte`/`mocha`) from the terminal
/// background, so the output stays legible on either kind of terminal.
pub fn scheme(name: &str) -> Option<Colors> {
    let name = if name.eq_ignore_ascii_case("auto") {
        if crate::modules::term_is_light().unwrap_or(false) { "latte" } else { "mocha" }
    } else {
        name
    };
    let p = scheme_rgb(name)?;
    let c = |i: usize| {
        let (r, g, b) = p[i];
        format!("\x1b[38;2;{};{};{}m", r, g, b)
    };
    Some(Colors {
        accent:   c(12),
        username: c(3),
        hostname: c(12),
        labels:   [c(12), c(11), c(10), c(9), c(8), c(7), c(6)],
        values:   c(15),
        sep:      c(16),
        bar:      c(12),
        logo:     [c(12), c(11), c(10), c(9), c(8), c(7), c(6), c(12), c(14)],
    })
}

/// Scheme names accepted by [`scheme`], for help text.
pub const SCHEMES: &[&str] = &[
    "auto", "mocha", "latte", "frappe", "macchiato", "nord", "dracula",
    "gruvbox", "tokyo-night", "solarized-dark", "rose-pine",
    "onedark", "kanagawa", "everforest", "ayu",
];

// ── Colors ───────────────────────────────────────────────

pub struct Colors {
    pub accent:   String,
    pub username: String,
    pub hostname: String,
    pub labels:   [String; 7],  // c1..c7, cycle via label(i)
    pub values:   String,
    pub sep:      String,
    pub bar:      String,
    pub logo:     [String; 9],  // $1..$9 inline color slots for custom logos
}

impl Colors {
    /// Cycle through c1..c7 by field index.
    pub fn label(&self, idx: usize) -> &str {
        &self.labels[idx % 7]
    }
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            accent:   BLUE.into(),
            username: MAUVE.into(),
            hostname: BLUE.into(),
            labels:   [
                BLUE.into(), SAPPHIRE.into(), SKY.into(), TEAL.into(),
                GREEN.into(), YELLOW.into(), PEACH.into(),
            ],
            values:   SUBTEXT1.into(),
            sep:      OVERLAY0.into(),
            bar:      BLUE.into(),
            logo:     [
                BLUE.into(), SAPPHIRE.into(), SKY.into(), TEAL.into(),
                GREEN.into(), YELLOW.into(), PEACH.into(),
                BLUE.into(), TEXT.into(),
            ],
        }
    }
}

// ── Header display ───────────────────────────────────────

#[derive(Clone, PartialEq)]
#[derive(Default)]
pub enum Header {
    #[default]
    Both,        // user@host  (default)
    UserOnly,    // user
    HostOnly,    // host
    None,        // no header line at all
}


// ── Show (per-field visibility) ──────────────────────────

pub struct Show {
    pub header:       Header,
    pub os:           bool,
    pub kernel:       bool,
    pub uptime:       bool,
    pub uptime_long:  bool,
    pub res:          bool,
    pub pkgs:         bool,
    pub shell:        bool,
    pub de_wm:        bool,
    pub term:         bool,
    pub cpu:          bool,
    pub gpu:          bool,
    pub gpu_temp:     bool,
    pub battery:      bool,
    pub memory:       bool,
    pub disk:         bool,
    pub load:         bool,
    pub locale:       bool,
    pub ip:           bool,
    pub ssh:          bool,
    pub ports:        bool,
    pub swatches:     bool,
    pub init:         bool,
    pub cpu_temp:     bool,
    pub processes:    bool,
    pub container:    bool,
    pub session:      bool,
    // new fastfetch-inspired fields
    pub swap:         bool,
    pub sound:        bool,
    pub gpu_driver:   bool,
    pub bios:         bool,
    pub board:        bool,
    pub disk_type:    bool,
    pub pkg_updates:  bool,
    pub theme:        bool,
    pub icons:        bool,
    pub term_font:    bool,
    pub de_wm_ver:    bool,
    pub init_ver:     bool,
    pub local_ip:     bool,
    pub color_bar:    bool,
    // opt-in modules — all off by default ─────────────────
    pub datetime:      bool,
    pub editor:        bool,
    pub dns:           bool,
    pub host:          bool,
    pub chassis:       bool,
    pub bootmgr:       bool,
    pub cpu_cache:     bool,
    pub login_manager: bool,
    pub brightness:    bool,
    pub cpu_usage:     bool,
    pub diskio:        bool,
    pub netio:         bool,
    pub physmem:       bool,
    pub gpu_mem:       bool,
    pub btrfs:         bool,
    pub wifi:          bool,
    pub users:         bool,
    pub wallpaper:     bool,
    pub media:         bool,
    pub public_ip:     bool,
    pub weather:       bool,
    pub opengl:        bool,
    pub vulkan:        bool,
    pub cursor:        bool,
    pub term_size:     bool,
    pub term_theme:    bool,
    pub codec:         bool,
    pub bluetooth:     bool,
    pub camera:        bool,
    pub mouse:         bool,
    pub keyboard:      bool,
    pub tpm:           bool,
    pub physical_disk: bool,
    pub top_processes: bool,
    pub monitor:       bool,
}

impl Default for Show {
    fn default() -> Self {
        Show {
            header: Header::Both,
            os: true, kernel: true, uptime: true, uptime_long: false,
            res: false, pkgs: true, shell: true, de_wm: true, term: true,
            cpu: true, gpu: true, gpu_temp: false, battery: false,
            memory: true, disk: false, load: false, locale: false,
            ip: false, ssh: false, ports: false,
            swatches: false,
            init: false, cpu_temp: false, processes: false,
            container: false, session: false,
            swap: false, sound: false, gpu_driver: false,
            bios: false, board: false, disk_type: false,
            pkg_updates: false, theme: false, icons: false,
            term_font: false, de_wm_ver: false, init_ver: false,
            local_ip: false, color_bar: false,
            datetime: false, editor: false, dns: false, host: false,
            chassis: false, bootmgr: false, cpu_cache: false, login_manager: false,
            brightness: false, cpu_usage: false, diskio: false, netio: false,
            physmem: false, gpu_mem: false, btrfs: false, wifi: false,
            users: false, wallpaper: false, media: false, public_ip: false,
            weather: false, opengl: false, vulkan: false,
            cursor: false, term_size: false, term_theme: false, codec: false,
            bluetooth: false, camera: false, mouse: false, keyboard: false,
            tpm: false, physical_disk: false, top_processes: false,
            monitor: false,
        }
    }
}

/// Every selectable data field key (matches `set_field`). Used by `--all` / `-s`.
pub const FIELD_KEYS: &[&str] = &[
    "os","kernel","uptime","res","pkgs","shell","de_wm","term","cpu","gpu",
    "gpu_temp","battery","memory","disk","load","locale","ip","ssh","ports",
    "init","cpu_temp","processes","container","session","swap","sound","gpu_driver",
    "bios","board","disk_type","pkg_updates","theme","icons","term_font","de_wm_ver",
    "init_ver","local_ip","datetime","editor","dns","host","chassis","bootmgr",
    "cpu_cache","login_manager","brightness","cpu_usage","diskio","netio","physmem",
    "gpu_mem","btrfs","wifi","users","wallpaper","media","public_ip","weather",
    "opengl","vulkan","cursor","term_size","term_theme","codec","bluetooth",
    "camera","mouse","keyboard","tpm","physical_disk","top_processes","monitor",
];

impl Show {
    fn apply_preset(&mut self, preset: &str) {
        *self = Show {
            header: Header::Both,
            os: false, kernel: false, uptime: false, uptime_long: false,
            res: false, pkgs: false, shell: false, de_wm: false, term: false,
            cpu: false, gpu: false, gpu_temp: false, battery: false,
            memory: false, disk: false, load: false, locale: false,
            ip: false, ssh: false, ports: false,
            swatches: false,
            init: false, cpu_temp: false, processes: false,
            container: false, session: false,
            swap: false, sound: false, gpu_driver: false,
            bios: false, board: false, disk_type: false,
            pkg_updates: false, theme: false, icons: false,
            term_font: false, de_wm_ver: false, init_ver: false,
            local_ip: false, color_bar: false,
            datetime: false, editor: false, dns: false, host: false,
            chassis: false, bootmgr: false, cpu_cache: false, login_manager: false,
            brightness: false, cpu_usage: false, diskio: false, netio: false,
            physmem: false, gpu_mem: false, btrfs: false, wifi: false,
            users: false, wallpaper: false, media: false, public_ip: false,
            weather: false, opengl: false, vulkan: false,
            cursor: false, term_size: false, term_theme: false, codec: false,
            bluetooth: false, camera: false, mouse: false, keyboard: false,
            tpm: false, physical_disk: false, top_processes: false,
            monitor: false,
        };
        match preset.trim() {
            "minimal" => {
                self.os = true; self.kernel = true;
                self.uptime = true; self.memory = true;
                self.battery = true;
                self.swatches = true;
            }
            "hacker" => {
                self.kernel = true; self.uptime = true;
                self.cpu = true; self.gpu = true; self.gpu_temp = true;
                self.memory = true; self.disk = true; self.load = true;
                self.ip = true; self.ssh = true; self.ports = true;
                self.swatches = true;
            }
            "science" => {
                self.os = true; self.kernel = true; self.cpu = true;
                self.memory = true; self.disk = true; self.uptime = true;
                self.swatches = false;
            }
            _ => {
                // "full" or anything else
                *self = Show {
                    header: Header::Both,
                    os: true, kernel: true, uptime: true, uptime_long: false,
                    res: true, pkgs: true, shell: true, de_wm: true, term: true,
                    cpu: true, gpu: true, gpu_temp: false, battery: true,
                    memory: true, disk: true, load: true, locale: true,
                    ip: false, ssh: false, ports: false,
                    swatches: true,
                    init: true, cpu_temp: true, processes: true,
                    container: false, session: true,
                    swap: true, sound: true, gpu_driver: true,
                    bios: false, board: false, disk_type: true,
                    pkg_updates: false, theme: true, icons: false,
                    term_font: false, de_wm_ver: false, init_ver: true,
                    local_ip: true, color_bar: false,
                    datetime: false, editor: false, dns: false, host: false,
                    chassis: false, bootmgr: false, cpu_cache: false, login_manager: false,
                    brightness: false, cpu_usage: false, diskio: false, netio: false,
                    physmem: false, gpu_mem: false, btrfs: false, wifi: false,
                    users: false, wallpaper: false, media: false, public_ip: false,
                    weather: false, opengl: false, vulkan: false,
                    cursor: true, term_size: true, term_theme: true, codec: false,
                    bluetooth: false, camera: true, mouse: true, keyboard: true,
                    tpm: true, physical_disk: true, top_processes: false,
                    monitor: true,
                };
            }
        }
    }

    fn set_field(&mut self, key: &str, val: bool) {
        match key {
            "os"          => self.os         = val,
            "kernel"      => self.kernel     = val,
            "uptime"      => self.uptime     = val,
            "uptime_long" => self.uptime_long= val,
            "res"         => self.res        = val,
            "pkgs"        => self.pkgs       = val,
            "shell"       => self.shell      = val,
            "de_wm"       => self.de_wm      = val,
            "term"        => self.term       = val,
            "cpu"         => self.cpu        = val,
            "gpu"         => self.gpu        = val,
            "gpu_temp"    => self.gpu_temp   = val,
            "battery"     => self.battery    = val,
            "memory"      => self.memory     = val,
            "disk"        => self.disk       = val,
            "load"        => self.load       = val,
            "locale"      => self.locale     = val,
            "ip"          => self.ip         = val,
            "ssh"         => self.ssh        = val,
            "ports"       => self.ports      = val,
            "swatches"    => self.swatches   = val,
            "init"        => self.init       = val,
            "cpu_temp"    => self.cpu_temp   = val,
            "processes"   => self.processes  = val,
            "container"   => self.container  = val,
            "session"     => self.session    = val,
            "swap"        => self.swap       = val,
            "sound"       => self.sound      = val,
            "gpu_driver"  => self.gpu_driver = val,
            "bios"        => self.bios       = val,
            "board"       => self.board      = val,
            "disk_type"   => self.disk_type  = val,
            "pkg_updates" => self.pkg_updates= val,
            "theme"       => self.theme      = val,
            "icons"       => self.icons      = val,
            "term_font"   => self.term_font  = val,
            "de_wm_ver"   => self.de_wm_ver  = val,
            "init_ver"    => self.init_ver   = val,
            "local_ip"    => self.local_ip   = val,
            "color_bar"   => self.color_bar  = val,
            "datetime"       => self.datetime       = val,
            "editor"         => self.editor         = val,
            "dns"            => self.dns            = val,
            "host"           => self.host           = val,
            "chassis"        => self.chassis        = val,
            "bootmgr"        => self.bootmgr        = val,
            "cpu_cache"      => self.cpu_cache      = val,
            "login_manager"  => self.login_manager  = val,
            "brightness"     => self.brightness     = val,
            "cpu_usage"      => self.cpu_usage      = val,
            "diskio"         => self.diskio         = val,
            "netio"          => self.netio          = val,
            "physmem"        => self.physmem        = val,
            "gpu_mem"        => self.gpu_mem        = val,
            "btrfs"          => self.btrfs          = val,
            "wifi"           => self.wifi           = val,
            "users"          => self.users          = val,
            "wallpaper"      => self.wallpaper      = val,
            "media"          => self.media          = val,
            "public_ip"      => self.public_ip      = val,
            "weather"        => self.weather        = val,
            "opengl"         => self.opengl         = val,
            "vulkan"         => self.vulkan         = val,
            "cursor"         => self.cursor         = val,
            "term_size"      => self.term_size      = val,
            "term_theme"     => self.term_theme     = val,
            "codec"          => self.codec          = val,
            "bluetooth"      => self.bluetooth      = val,
            "camera"         => self.camera         = val,
            "mouse"          => self.mouse          = val,
            "keyboard"       => self.keyboard       = val,
            "tpm"            => self.tpm            = val,
            "physical_disk"  => self.physical_disk  = val,
            "top_processes"  => self.top_processes  = val,
            "monitor"        => self.monitor        = val,
            _ => {}
        }
    }

    /// Turn every data field on (used by `--all`).
    pub fn enable_all(&mut self) {
        for k in FIELD_KEYS { self.set_field(k, true); }
    }

    /// Turn every data field off, keeping header/swatches/color_bar settings.
    pub fn clear_fields(&mut self) {
        for k in FIELD_KEYS { self.set_field(k, false); }
    }

    /// `-s a,b,c` / `--structure a,b,c`: show ONLY the listed fields.
    pub fn select(&mut self, list: &str) {
        self.clear_fields();
        for tok in list.split([',', ' ', '\t']) {
            let t = tok.trim();
            if !t.is_empty() { self.set_field(t, true); }
        }
    }
}

// ── Logo ─────────────────────────────────────────────────

#[derive(Default)]
pub struct Logo {
    pub name: Option<String>,  // built-in logo name (arch, ascii, tux, nix, gentoo, mini, auto)
    pub file: Option<String>,  // custom ASCII file path (overrides name if set)
}


// ── Animation ────────────────────────────────────────────

pub struct Animation {
    /// Play a short intro before the static render (TTY only).
    pub enabled:  bool,
    /// Glyph effect: none | shimmer | scanline | boot.
    pub effect:   String,
    /// Intro length in milliseconds.
    pub intro_ms: u64,
    /// Colour-wave the logo while animating.
    pub wave:     bool,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            enabled:  true,
            effect:   "none".into(),
            intro_ms: 700,
            wave:     true,
        }
    }
}

// ── Config ───────────────────────────────────────────────

pub struct Config {
    pub colors:    Colors,
    pub show:      Show,
    pub preset:    Option<String>,
    pub logo:      Logo,
    pub animation: Animation,
}

// ── File path ────────────────────────────────────────────

pub fn config_path() -> String {
    env::var("XDG_CONFIG_HOME")
        .map(|p| format!("{}/arcfetch/config.toml", p))
        .unwrap_or_else(|_|
            env::var("HOME")
                .map(|h| format!("{}/.config/arcfetch/config.toml", h))
                .unwrap_or_else(|_| "~/.config/arcfetch/config.toml".into())
        )
}

// ── Simple TOML-subset parser ────────────────────────────
// Handles [section], key = value, # comments.
// No arrays, no multiline — exactly what we need, zero deps.

/// Strip a trailing `# comment` without eating `#RRGGBB` colour values.
fn strip_comment(line: &str) -> &str {
    match line.find('=') {
        Some(eq) => {
            let rest = &line[eq + 1..];
            let value_start = eq + 1 + (rest.len() - rest.trim_start().len());
            if line[value_start..].starts_with('#') {
                // the first '#' begins a hex value; only a second '#' is a comment
                if let Some(p) = line[value_start + 1..].find('#') {
                    return &line[..value_start + 1 + p];
                }
                return line;
            }
            line.find('#').map(|p| &line[..p]).unwrap_or(line)
        }
        None => line.find('#').map(|p| &line[..p]).unwrap_or(line),
    }
}

pub fn load(cli_accent: Option<&str>, cli_preset: Option<&str>) -> Config {
    let mut colors = Colors::default();
    let mut show   = Show::default();
    let mut preset: Option<String> = None;
    let mut logo   = Logo::default();
    let mut animation = Animation::default();
    let mut header_explicit = false;

    // override accent from env var
    let env_accent = env::var("ARCFETCH_ACCENT").ok();
    let accent_override = cli_accent
        .map(String::from)
        .or(env_accent);

    // try reading config file
    let path = config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        let mut section = "";
        for (lineno, raw_line) in content.lines().enumerate() {
            // strip a trailing comment first, then trim; skip blanks
            let line = strip_comment(raw_line).trim();
            if line.is_empty() { continue; }

            // section header: [name]
            if let Some(rest) = line.strip_prefix('[') {
                if let Some((name, _)) = rest.split_once(']') {
                    let name = name.trim();
                    if !name.is_empty() {
                        section = name;
                        continue;
                    }
                }
                eprintln!("arcfetch: warning: invalid section header at line {}: {}", lineno + 1, raw_line);
                continue;
            }

            let Some((key, val)) = line.split_once('=') else {
                eprintln!("arcfetch: warning: skipping unparseable line {}: {}", lineno + 1, raw_line);
                continue;
            };
            let key = key.trim();
            let val = val.trim();

            match section {
                "colors" => {
                    let ansi = resolve(val);
                    match key {
                        "scheme"   => {
                            match scheme(val) {
                                Some(c) => colors = c,
                                None => eprintln!(
                                    "arcfetch: warning: unknown color scheme '{}' (try: {})",
                                    val, SCHEMES.join(", ")),
                            }
                        }
                        "accent"   => colors.accent   = ansi,
                        "username" => colors.username  = ansi,
                        "hostname" => colors.hostname  = ansi,
                        "values"   => colors.values    = ansi,
                        "sep"      => colors.sep        = ansi,
                        "bar"      => colors.bar        = ansi,
                        "c1"       => colors.labels[0]  = ansi,
                        "c2"       => colors.labels[1]  = ansi,
                        "c3"       => colors.labels[2]  = ansi,
                        "c4"       => colors.labels[3]  = ansi,
                        "c5"       => colors.labels[4]  = ansi,
                        "c6"       => colors.labels[5]  = ansi,
                        "c7"       => colors.labels[6]  = ansi,
                        "logo1"    => colors.logo[0]    = ansi,
                        "logo2"    => colors.logo[1]    = ansi,
                        "logo3"    => colors.logo[2]    = ansi,
                        "logo4"    => colors.logo[3]    = ansi,
                        "logo5"    => colors.logo[4]    = ansi,
                        "logo6"    => colors.logo[5]    = ansi,
                        "logo7"    => colors.logo[6]    = ansi,
                        "logo8"    => colors.logo[7]    = ansi,
                        "logo9"    => colors.logo[8]    = ansi,
                        _ => {}
                    }
                }
                "show" => {
                    if key == "header" {
                        header_explicit = true;
                        show.header = match val {
                            "user"     | "username" => Header::UserOnly,
                            "host"     | "hostname" => Header::HostOnly,
                            "none"     | "false"    => Header::None,
                            _                       => Header::Both,
                        };
                    } else {
                        let enabled = matches!(val, "true" | "yes" | "1" | "on");
                        show.set_field(key, enabled);
                    }
                }
                "logo" => {
                    match key {
                        "name" => logo.name = Some(val.to_string()),
                        "file" => logo.file = Some(val.to_string()),
                        _ => {}
                    }
                }
                "animation" => {
                    let on = matches!(val, "true" | "yes" | "1" | "on");
                    match key {
                        "enabled"  => animation.enabled  = on,
                        "effect"   => animation.effect   = val.to_string(),
                        "intro_ms" => animation.intro_ms = val.parse().unwrap_or(700),
                        "wave"     => animation.wave     = on,
                        _ => {}
                    }
                }
                "template"
                    if key == "preset" => { preset = Some(val.to_string()); }
                _ => {}
            }
        }
    }

    // CLI preset overrides file preset
    if let Some(p) = cli_preset { preset = Some(p.to_string()); }

    // apply preset (overrides [show] if set, but preserves explicit header)
    if let Some(p) = &preset {
        let saved_header = if header_explicit { Some(show.header.clone()) } else { None };
        show.apply_preset(p);
        if let Some(h) = saved_header { show.header = h; }
    }

    // CLI / env accent wins over file
    if let Some(a) = accent_override {
        let ansi = resolve(&a);
        colors.accent   = ansi.clone();
        colors.hostname = ansi;
    }

    Config { colors, show, preset, logo, animation }
}
