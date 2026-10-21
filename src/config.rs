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
// A fastfetch-style JSONC config (config.jsonc or --config <file>) is also
// accepted. TOML wins when both config.toml and config.jsonc exist.

use std::{env, fs, path::Path};
use crate::jsonc;

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
            6 if hex.is_ascii() => (hex_byte(&hex[0..2]), hex_byte(&hex[2..4]), hex_byte(&hex[4..6])),
            3 if hex.is_ascii() => {
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

/// Fastfetch colour syntax: `31`, `90`, `#red`, `#@196`, `#ff0000`.
pub fn ansi_code(n: u8) -> String {
    match n {
        0..=7   => format!("\x1b[{}m", 30 + n),
        8..=15  => format!("\x1b[{}m", 90 + n - 8),
        30..=37 | 90..=97 => format!("\x1b[{}m", n),
        _       => format!("\x1b[38;5;{}m", n),
    }
}

/// Resolve a colour value used in a JSONC `display`/module block.
pub fn resolve_config_color(spec: &str) -> String {
    let s = spec.trim();
    if let Some(n) = s.strip_prefix("#@").and_then(|v| v.parse::<u8>().ok()) {
        return format!("\x1b[38;5;{}m", n);
    }
    if let Some(rest) = s.strip_prefix('#') {
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(n) = rest.parse::<u8>() { return ansi_code(n); }
        }
        return resolve(s);
    }
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(n) = s.parse::<u8>() { return ansi_code(n); }
    }
    resolve(s)
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
    /// Turn on the collector needed by a fastfetch-style module type.
    pub fn enable_module(&mut self, kind: &str) {
        let k = kind.trim().to_ascii_lowercase().replace(['-', '_'], "");
        match k.as_str() {
            "title" | "header" => self.header = Header::Both,
            "os" => self.os = true,
            "kernel" => self.kernel = true,
            "uptime" => self.uptime = true,
            "packages" | "pkgs" => self.pkgs = true,
            "shell" => self.shell = true,
            "de" | "wm" | "desktop" => self.de_wm = true,
            "terminal" | "term" => self.term = true,
            "cpu" => self.cpu = true,
            "gpu" => self.gpu = true,
            "gputemp" => self.gpu_temp = true,
            "battery" => self.battery = true,
            "memory" | "mem" => self.memory = true,
            "disk" => self.disk = true,
            "load" | "loadavg" => self.load = true,
            "locale" => self.locale = true,
            "localip" => self.local_ip = true,
            "ip" => self.ip = true,
            "ssh" => self.ssh = true,
            "ports" => self.ports = true,
            "init" | "initsystem" => self.init = true,
            "cputemp" => self.cpu_temp = true,
            "processes" => self.processes = true,
            "container" => self.container = true,
            "session" => self.session = true,
            "swap" => self.swap = true,
            "sound" => self.sound = true,
            "gpudriver" => self.gpu_driver = true,
            "bios" => self.bios = true,
            "board" => self.board = true,
            "disktype" => self.disk_type = true,
            "pkgsupdates" | "pkgupdates" => self.pkg_updates = true,
            "theme" => self.theme = true,
            "icons" => self.icons = true,
            "terminalfont" | "termfont" => self.term_font = true,
            "dewmver" => self.de_wm_ver = true,
            "initver" => self.init_ver = true,
            "datetime" => self.datetime = true,
            "editor" => self.editor = true,
            "dns" => self.dns = true,
            "host" => self.host = true,
            "chassis" => self.chassis = true,
            "bootmgr" => self.bootmgr = true,
            "cpucache" => self.cpu_cache = true,
            "loginmanager" => self.login_manager = true,
            "brightness" => self.brightness = true,
            "cpuusage" => self.cpu_usage = true,
            "diskio" => self.diskio = true,
            "netio" => self.netio = true,
            "physmem" => self.physmem = true,
            "gpumem" => self.gpu_mem = true,
            "btrfs" => self.btrfs = true,
            "wifi" => self.wifi = true,
            "users" => self.users = true,
            "wallpaper" => self.wallpaper = true,
            "media" => self.media = true,
            "publicip" => self.public_ip = true,
            "weather" => self.weather = true,
            "opengl" => self.opengl = true,
            "vulkan" => self.vulkan = true,
            "cursor" => self.cursor = true,
            "terminalsize" | "termsize" => self.term_size = true,
            "terminaltheme" | "termtheme" => self.term_theme = true,
            "codec" => self.codec = true,
            "bluetooth" => self.bluetooth = true,
            "camera" => self.camera = true,
            "mouse" => self.mouse = true,
            "keyboard" => self.keyboard = true,
            "tpm" => self.tpm = true,
            "physicaldisk" => self.physical_disk = true,
            "topprocesses" => self.top_processes = true,
            "monitor" => self.monitor = true,
            "res" => self.res = true,
            "swatches" => self.swatches = true,
            "colorbar" => self.color_bar = true,
            _ => {}
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

pub struct Logo {
    pub name: Option<String>,  // built-in logo name (arch, ascii, tux, nix, gentoo, mini, auto)
    pub file: Option<String>,  // custom ASCII file path (overrides name if set)
    pub color: Option<String>, // uniform logo colour (same syntax as --color)
    pub data: Option<String>,  // inline `data` / `data-raw` logo
    pub position: String,      // left | right | top
    pub padding_top: usize,
    pub padding_right: usize,
}

impl Default for Logo {
    fn default() -> Self {
        Self {
            name: None, file: None, color: None, data: None,
            position: "left".into(), padding_top: 0, padding_right: 2,
        }
    }
}

// ── JSONC display/module settings ───────────────────────

pub struct Display {
    pub separator: Option<String>,
    pub separator_color: Option<String>,
    pub key_color: Option<String>,
    pub constants: Vec<String>,
    pub key_width: Option<usize>,
    pub key_padding_left: usize,
    pub bar_width: usize,
    pub bar_elapsed: String,
    pub bar_total: String,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            separator: None,
            separator_color: None,
            key_color: None,
            constants: Vec::new(),
            key_width: None,
            key_padding_left: 0,
            bar_width: 14,
            bar_elapsed: "\u{2588}".into(),
            bar_total: "\u{2591}".into(),
        }
    }
}

pub struct Module {
    pub kind: String,
    pub key: Option<String>,
    pub format: Option<String>,
    pub key_color: Option<String>,
    pub output_color: Option<String>,
    pub key_icon: Option<String>,
    pub padding_left: usize,
    pub key_width: Option<usize>,
    pub text: Option<String>,   // command module
    pub symbol: Option<String>, // colors module
}

// ── Layout ───────────────────────────────────────────────

pub struct Layout {
    /// Border style: none | round | square | double | heavy | dashed | ascii.
    pub box_style: String,
    /// Border colour; `None` follows the accent, `Some("")` means uncoloured.
    pub box_color: Option<String>,
    /// Left margin in columns.
    pub indent: usize,
    /// Spaces between the logo and the info column.
    pub gap: usize,
}

impl Default for Layout {
    fn default() -> Self {
        Self { box_style: "none".into(), box_color: None, indent: 2, gap: 2 }
    }
}

impl Layout {
    fn set(&mut self, key: &str, val: &str) {
        match key {
            "box" => match val.trim().to_ascii_lowercase().as_str() {
                "true" | "on" | "yes" => self.box_style = "round".into(),
                "false" | "off" | "no" | "none" => self.box_style = "none".into(),
                "round" | "square" | "double" | "heavy" | "dashed" | "ascii" => {
                    self.box_style = val.trim().to_ascii_lowercase();
                }
                other => eprintln!(
                    "arcfetch: warning: unknown box style '{}' (try: round|square|double|heavy|dashed|ascii|none)",
                    other),
            },
            "box_color" => match val.trim().to_ascii_lowercase().as_str() {
                "none" | "off" | "false" => self.box_color = Some(String::new()),
                "accent" | "default" | "auto" => self.box_color = None,
                _ => self.box_color = Some(resolve(val)),
            },
            "indent" => self.indent = val.trim().parse::<usize>().unwrap_or(2).min(32),
            "gap"    => self.gap    = val.trim().parse::<usize>().unwrap_or(2).min(32),
            _ => {}
        }
    }
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
    pub logo:      Logo,
    pub layout:    Layout,
    pub animation: Animation,
    pub display:   Display,
    pub modules:   Option<Vec<Module>>,
    pub source:    String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            colors: Colors::default(),
            show: Show::default(),
            logo: Logo::default(),
            layout: Layout::default(),
            animation: Animation::default(),
            display: Display::default(),
            modules: None,
            source: String::new(),
        }
    }
}

// ── File path ────────────────────────────────────────────

fn config_dir() -> String {
    env::var("XDG_CONFIG_HOME")
        .map(|p| format!("{}/arcfetch", p))
        .unwrap_or_else(|_|
            env::var("HOME")
                .map(|h| format!("{}/.config/arcfetch", h))
                .unwrap_or_else(|_| "~/.config/arcfetch".into())
        )
}

pub fn config_path() -> String {
    format!("{}/config.toml", config_dir())
}

pub fn jsonc_path() -> String {
    format!("{}/config.jsonc", config_dir())
}

/// Where `--preset <name>` looks for fastfetch-style examples.
pub fn presets_dir() -> Option<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(p) = env::var("XDG_DATA_HOME") {
        dirs.push(format!("{p}/arcfetch/presets/examples"));
    }
    if let Ok(h) = env::var("HOME") {
        dirs.push(format!("{h}/.local/share/arcfetch/presets/examples"));
        dirs.push(format!("{h}/.config/arcfetch/examples"));
    }
    dirs.push("/usr/share/arcfetch/presets/examples".into());
    dirs.push("presets/examples".into());
    dirs.push(format!("{}/presets/examples", env!("CARGO_MANIFEST_DIR")));
    dirs.into_iter().map(std::path::PathBuf::from).find(|p| p.is_dir())
}

pub fn resolve_preset(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() { return None; }
    let p = Path::new(name);
    if p.is_file() { return Some(name.to_string()); }
    let dir = presets_dir()?;
    for candidate in [format!("{name}.jsonc"), format!("{name}.json"), name.to_string()] {
        let path = dir.join(candidate);
        if path.is_file() { return Some(path.to_string_lossy().into_owned()); }
    }
    None
}

pub fn preset_names() -> Vec<String> {
    let Some(dir) = presets_dir() else { return Vec::new() };
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_str()?;
            if ext != "jsonc" && ext != "json" { return None; }
            Some(p.file_stem()?.to_string_lossy().into_owned())
        }).collect())
        .unwrap_or_default();
    names.sort();
    names
}

// ── Simple TOML-subset parser ────────────────────────────
// Handles [section], key = value, # comments.
// No arrays, no multiline — exactly what we need, zero deps.

/// Index of a `#` that starts a comment: outside quotes and at the start of the
/// line or after whitespace. Keeps `#` inside paths/hex values.
fn comment_start(line: &str, from: usize) -> Option<usize> {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        if i < from { continue; }
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted && (i == 0 || line.as_bytes()[i - 1].is_ascii_whitespace()) => {
                return Some(i);
            }
            _ => {}
        }
    }
    None
}

/// Strip a trailing `# comment` without eating `#RRGGBB` colour values.
fn strip_comment(line: &str) -> &str {
    match line.find('=') {
        Some(eq) => {
            let rest = &line[eq + 1..];
            let value_start = eq + 1 + (rest.len() - rest.trim_start().len());
            if line[value_start..].starts_with('#') {
                // the first '#' begins a hex value; only a later comment is removed
                return comment_start(line, value_start + 1)
                    .map(|p| &line[..p])
                    .unwrap_or(line);
            }
            comment_start(line, value_start)
                .map(|p| &line[..p])
                .unwrap_or(line)
        }
        None => comment_start(line, 0).map(|p| &line[..p]).unwrap_or(line),
    }
}

pub fn load(cli_accent: Option<&str>, preset: Option<&str>, config_file: Option<&str>) -> Config {
    let mut cfg = Config::default();

    // CLI --config wins over --preset; otherwise TOML wins over JSONC.
    let explicit = config_file
        .map(String::from)
        .or_else(|| preset.and_then(resolve_preset).or_else(|| preset.map(String::from)));

    let path = explicit.or_else(|| {
        for candidate in [config_path(), jsonc_path()] {
            if Path::new(&candidate).is_file() { return Some(candidate); }
        }
        None
    });

    if let Some(path) = path {
        match fs::read_to_string(&path) {
            Ok(content) => {
                cfg.source = path.clone();
                let is_json = path.ends_with(".jsonc")
                    || path.ends_with(".json")
                    || content.trim_start().starts_with('{');
                if is_json {
                    if let Err(e) = apply_jsonc(&mut cfg, &content) {
                        eprintln!("arcfetch: warning: {path}: {e}");
                    }
                } else {
                    apply_toml(&mut cfg, &content);
                }
            }
            Err(e) => eprintln!("arcfetch: warning: {path}: {e}"),
        }
    }

    // CLI / env accent wins over file
    let accent_override = cli_accent.map(String::from).or_else(|| env::var("ARCFETCH_ACCENT").ok());
    if let Some(a) = accent_override {
        let ansi = resolve(&a);
        cfg.colors.accent = ansi.clone();
        cfg.colors.hostname = ansi;
    }

    cfg
}

fn apply_toml(cfg: &mut Config, content: &str) {
    let mut section = "";
    for (lineno, raw_line) in content.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() { continue; }

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
        let val = val.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(val);

        match section {
            "colors" => {
                let ansi = resolve(val);
                match key {
                    "scheme" => match scheme(val) {
                        Some(c) => cfg.colors = c,
                        None => eprintln!(
                            "arcfetch: warning: unknown color scheme '{}' (try: {})",
                            val, SCHEMES.join(", ")),
                    },
                    "accent" => cfg.colors.accent = ansi,
                    "username" => cfg.colors.username = ansi,
                    "hostname" => cfg.colors.hostname = ansi,
                    "values" => cfg.colors.values = ansi,
                    "sep" => cfg.colors.sep = ansi,
                    "bar" => cfg.colors.bar = ansi,
                    "c1" => cfg.colors.labels[0] = ansi,
                    "c2" => cfg.colors.labels[1] = ansi,
                    "c3" => cfg.colors.labels[2] = ansi,
                    "c4" => cfg.colors.labels[3] = ansi,
                    "c5" => cfg.colors.labels[4] = ansi,
                    "c6" => cfg.colors.labels[5] = ansi,
                    "c7" => cfg.colors.labels[6] = ansi,
                    "logo1" => cfg.colors.logo[0] = ansi,
                    "logo2" => cfg.colors.logo[1] = ansi,
                    "logo3" => cfg.colors.logo[2] = ansi,
                    "logo4" => cfg.colors.logo[3] = ansi,
                    "logo5" => cfg.colors.logo[4] = ansi,
                    "logo6" => cfg.colors.logo[5] = ansi,
                    "logo7" => cfg.colors.logo[6] = ansi,
                    "logo8" => cfg.colors.logo[7] = ansi,
                    "logo9" => cfg.colors.logo[8] = ansi,
                    _ => {}
                }
            }
            "show" => {
                if key == "header" {
                    cfg.show.header = match val {
                        "user" | "username" => Header::UserOnly,
                        "host" | "hostname" => Header::HostOnly,
                        "none" | "false" => Header::None,
                        _ => Header::Both,
                    };
                } else {
                    let enabled = matches!(val, "true" | "yes" | "1" | "on");
                    cfg.show.set_field(key, enabled);
                }
            }
            "logo" => match key {
                "name" => cfg.logo.name = Some(val.to_string()),
                "file" => cfg.logo.file = Some(val.to_string()),
                "color" => cfg.logo.color = match val.trim().to_ascii_lowercase().as_str() {
                    "accent" | "default" | "auto" | "none" => None,
                    _ => Some(resolve(val)),
                },
                _ => {}
            },
            "layout" => cfg.layout.set(key, val),
            "animation" => {
                let on = matches!(val, "true" | "yes" | "1" | "on");
                match key {
                    "enabled" => cfg.animation.enabled = on,
                    "effect" => cfg.animation.effect = val.to_string(),
                    "intro_ms" => cfg.animation.intro_ms = val.parse().unwrap_or(700),
                    "wave" => cfg.animation.wave = on,
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn apply_jsonc(cfg: &mut Config, text: &str) -> Result<(), String> {
    let root = jsonc::parse(text)?;

    if let Some(logo) = root.get("logo") {
        match logo {
            jsonc::Value::Null => cfg.logo.name = Some("none".into()),
            jsonc::Value::Obj(_) => {
                if let Some(t) = logo.get("type").and_then(jsonc::Value::as_str) {
                    match t.trim().to_ascii_lowercase().as_str() {
                        "none" => cfg.logo.name = Some("none".into()),
                        "small" => cfg.logo.name = Some("mini".into()),
                        "data" | "data-raw" | "raw" => cfg.logo.name = None,
                        "file" => cfg.logo.file = logo.get("source").and_then(jsonc::Value::as_str).map(String::from),
                        other => cfg.logo.name = Some(other.to_string()),
                    }
                }
                if let Some(src) = logo.get("source").and_then(jsonc::Value::as_str) {
                    match logo.get("type").and_then(jsonc::Value::as_str).unwrap_or("").to_ascii_lowercase().as_str() {
                        "data" | "data-raw" | "raw" => cfg.logo.data = Some(src.to_string()),
                        "file" => cfg.logo.file = Some(src.to_string()),
                        _ if !src.contains('\n') => cfg.logo.file = Some(src.to_string()),
                        _ => cfg.logo.data = Some(src.to_string()),
                    }
                }
                if let Some(c) = logo.get("color").and_then(jsonc::Value::as_str) {
                    cfg.logo.color = match c.trim().to_ascii_lowercase().as_str() {
                        "accent" | "default" | "auto" | "none" => None,
                        _ => Some(resolve_config_color(c)),
                    };
                }
                if let Some(pos) = logo.get("position").and_then(jsonc::Value::as_str) {
                    cfg.logo.position = pos.trim().to_ascii_lowercase();
                }
                if let Some(pad) = logo.get("padding") {
                    cfg.logo.padding_top = pad.get("top").and_then(jsonc::Value::as_usize).unwrap_or(0);
                    cfg.logo.padding_right = pad.get("right").and_then(jsonc::Value::as_usize).unwrap_or(2);
                }
            }
            _ => {}
        }
    }

    if let Some(d) = root.get("display") {
        if let Some(v) = d.get("separator").and_then(jsonc::Value::as_str) {
            cfg.display.separator = Some(v.to_string());
        }
        if let Some(color) = d.get("color") {
            if let Some(v) = color.get("separator").and_then(jsonc::Value::as_str) {
                cfg.display.separator_color = Some(resolve_config_color(v));
            }
            if let Some(v) = color.get("keys").and_then(jsonc::Value::as_str) {
                cfg.display.key_color = Some(resolve_config_color(v));
            }
        }
        if let Some(arr) = d.get("constants").and_then(jsonc::Value::as_array) {
            cfg.display.constants = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        }
        if let Some(key) = d.get("key") {
            cfg.display.key_width = key.get("width").and_then(jsonc::Value::as_usize);
            cfg.display.key_padding_left = key.get("paddingLeft").and_then(jsonc::Value::as_usize).unwrap_or(0);
        }
        if let Some(bar) = d.get("bar") {
            cfg.display.bar_width = bar.get("width").and_then(jsonc::Value::as_usize).unwrap_or(14).clamp(1, 200);
            if let Some(ch) = bar.get("char") {
                if let Some(v) = ch.get("elapsed").and_then(jsonc::Value::as_str) { cfg.display.bar_elapsed = v.to_string(); }
                if let Some(v) = ch.get("total").and_then(jsonc::Value::as_str) { cfg.display.bar_total = v.to_string(); }
            }
        }
    }

    if let Some(arr) = root.get("modules").and_then(jsonc::Value::as_array) {
        let mut mods = Vec::with_capacity(arr.len());
        for item in arr {
            let mut m = Module {
                kind: String::new(), key: None, format: None,
                key_color: None, output_color: None, key_icon: None,
                padding_left: 0, key_width: None, text: None, symbol: None,
            };
            match item {
                jsonc::Value::Str(s) => m.kind = s.trim().to_ascii_lowercase(),
                jsonc::Value::Obj(_) => {
                    let Some(kind) = item.get("type").and_then(jsonc::Value::as_str) else { continue };
                    m.kind = kind.trim().to_ascii_lowercase();
                    m.key = item.get("key").and_then(jsonc::Value::as_str).map(String::from);
                    m.format = item.get("format").and_then(jsonc::Value::as_str).map(String::from);
                    m.key_color = item.get("keyColor").and_then(jsonc::Value::as_str).map(resolve_config_color);
                    m.output_color = item.get("outputColor").and_then(jsonc::Value::as_str).map(resolve_config_color);
                    m.key_icon = item.get("keyIcon").and_then(jsonc::Value::as_str).map(String::from);
                    m.padding_left = item.get("paddingLeft").and_then(jsonc::Value::as_usize).unwrap_or(0);
                    m.key_width = item.get("keyWidth").and_then(jsonc::Value::as_usize);
                    m.text = item.get("text").and_then(jsonc::Value::as_str).map(String::from);
                    m.symbol = item.get("symbol").and_then(jsonc::Value::as_str).map(String::from);
                }
                _ => continue,
            }
            if !m.kind.is_empty() { mods.push(m); }
        }
        cfg.show.clear_fields();
        cfg.show.header = Header::None;
        for m in &mods { cfg.show.enable_module(&m.kind); }
        cfg.modules = Some(mods);
    }

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_ascii_hex_does_not_panic() {
        assert_eq!(resolve("#aébcd"), BLUE);
        assert!(resolve("#ff0000").contains("38;2;255;0;0"));
    }

    #[test]
    fn comments_keep_hashes_in_values_and_quotes() {
        assert_eq!(strip_comment("file = /tmp/a#b"), "file = /tmp/a#b");
        assert_eq!(strip_comment("accent = red # note").trim(), "accent = red");
        assert_eq!(strip_comment("file = \"a # b\""), "file = \"a # b\"");
        assert_eq!(strip_comment("accent = #ff0000 # red").trim(), "accent = #ff0000");
    }

    #[test]
    fn layout_parses_and_clamps() {
        let mut l = Layout::default();
        l.set("box", "double");
        l.set("box_color", "red");
        l.set("indent", "0");
        l.set("gap", "999");
        assert_eq!(l.box_style, "double");
        assert!(l.box_color.as_deref().is_some_and(|c| !c.is_empty()));
        assert_eq!((l.indent, l.gap), (0, 32));

        l.set("box", "true");
        l.set("box_color", "none");
        assert_eq!(l.box_style, "round");
        assert_eq!(l.box_color.as_deref(), Some(""));
    }
}
