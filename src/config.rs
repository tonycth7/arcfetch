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

/// Down-sample the animation gradient with the rest of the palette.
pub fn apply_depth_animation(a: &mut Animation, depth: ColorDepth) {
    if depth == ColorDepth::True {
        return;
    }
    for g in &mut a.gradient {
        *g = downgrade(g, depth);
    }
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

/// Build a full [`Colors`] from a verified upstream palette.
///
/// `"auto"` is a selector, not a palette: it queries the terminal background
/// (OSC 11, `$COLORFGBG` fallback) and picks Latte for light or Mocha for
/// dark/unavailable, so the choice stays deterministic.
pub fn scheme(name: &str) -> Option<Colors> {
    let s = if name.eq_ignore_ascii_case("auto") {
        crate::scheme::auto_for(crate::modules::term_is_light().unwrap_or(false))
    } else {
        crate::scheme::find(name)?
    };
    let c = |role: &str| {
        let (r, g, b) = s.rgb(role);
        format!("\x1b[38;2;{};{};{}m", r, g, b)
    };
    Some(Colors {
        accent:   c(s.roles.accent),
        username: c(s.roles.purple),
        hostname: c(s.roles.blue),
        labels:   [c(s.roles.blue), c(s.roles.cyan), c(s.roles.teal), c(s.roles.green), c(s.roles.yellow), c(s.roles.orange), c(s.roles.red)],
        values:   c(s.roles.subtext),
        sep:      c(s.roles.overlay),
        bar:      c(s.roles.blue),
        logo:     [c(s.roles.blue), c(s.roles.cyan), c(s.roles.teal), c(s.roles.green), c(s.roles.yellow), c(s.roles.orange), c(s.roles.red), c(s.roles.accent), c(s.roles.purple)],
    })
}

/// Scheme names accepted by [`scheme`], for help text. `auto` is a selector.
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

/// A vertical colour ramp, sampled by row. Stops are stored as RGB so they
/// can be interpolated; the renderer asks for ANSI strings once.
#[derive(Clone, Debug, Default)]
pub struct Gradient {
    stops: Vec<(u8, u8, u8)>,
}

impl Gradient {
    pub fn new(stops: Vec<(u8, u8, u8)>) -> Self {
        Self { stops }
    }

    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }

    /// Sample at `t` in `0.0..=1.0`, interpolating between adjacent stops.
    pub fn sample(&self, t: f32) -> Option<(u8, u8, u8)> {
        match self.stops.len() {
            0 => None,
            1 => Some(self.stops[0]),
            n => {
                let pos = t.clamp(0.0, 1.0) * (n - 1) as f32;
                let i = (pos.floor() as usize).min(n - 2);
                let a = self.stops[i];
                let b = self.stops[i + 1];
                let f = pos - i as f32;
                let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f).round() as u8;
                Some((mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2)))
            }
        }
    }

    /// A dense set of ANSI foregrounds, so the renderer only has to pick one.
    pub fn ansi(&self, samples: usize) -> Vec<String> {
        if self.is_empty() {
            return Vec::new();
        }
        let n = samples.max(2);
        (0..n)
            .map(|i| {
                let (r, g, b) = self.sample(i as f32 / (n - 1) as f32).unwrap();
                format!("\x1b[38;2;{r};{g};{b}m")
            })
            .collect()
    }
}

/// Parse `#rgb` or `#rrggbb` for a gradient stop.
fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    let hex = s.strip_prefix('#').unwrap_or(s);
    match hex.len() {
        3 => {
            let f = |i: usize| hex_byte(&hex[i..i + 1]).map(|v| v.saturating_mul(17));
            Some((f(0)?, f(1)?, f(2)?))
        }
        6 => Some((
            hex_byte(&hex[0..2])?,
            hex_byte(&hex[2..4])?,
            hex_byte(&hex[4..6])?,
        )),
        _ => None,
    }
}

fn art_color_is_theme(val: &str) -> bool {
    val.trim_matches('"').trim_matches('\'').eq_ignore_ascii_case("theme")
}

/// Parse a single-line array of hex colours. Bad entries are skipped.
fn parse_gradient(val: &str) -> Option<Gradient> {
    let inner = val.trim().strip_prefix('[')?.strip_suffix(']')?;
    let stops = inner
        .split(',')
        .filter_map(parse_hex_rgb)
        .collect();
    Some(Gradient::new(stops))
}

/// Expand the `$1..$9`, `$R` and `$$` colour slots used by custom logos and
/// animation frames. `chars` keeps multi-byte block art intact.
pub fn expand_logo_slots(line: &str, logo: &[String; 9]) -> String {
    let mut out = String::with_capacity(line.len() + 32);
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            if let Some(&next) = chars.peek() {
                if ('1'..='9').contains(&next) {
                    out.push_str(&logo[(next as u8 - b'1') as usize]);
                    chars.next();
                    continue;
                }
                match next {
                    'R' => { out.push_str(RESET); chars.next(); continue; }
                    '$' => { out.push('$'); chars.next(); continue; }
                    _ => {}
                }
            }
        }
        out.push(ch);
    }
    out
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
    pub labels: bool,
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
            labels: true,
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
    pub text: Option<String>,    // command module
    pub symbol: Option<String>,  // colors module
    pub value: Option<String>,   // static value override
    pub command: Option<String>, // value from a command
}

impl Default for Module {
    fn default() -> Self {
        Self {
            kind: String::new(), key: None, format: None,
            key_color: None, output_color: None, key_icon: None,
            padding_left: 0, key_width: None, text: None, symbol: None,
            value: None, command: None,
        }
    }
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
    /// Vertical gradient for plain art. Empty falls back to the logo palette.
    pub gradient: Vec<String>,
    /// Let the gradient crawl through the art as it animates.
    pub gradient_scroll: bool,
    /// Keep a frame's own ANSI colours; `false` forces the gradient on it.
    pub art_color: bool,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            enabled:  true,
            effect:   "none".into(),
            intro_ms: 700,
            wave:     true,
            // animfetch's default ramp. Authored ANSI still wins.
            gradient: Gradient::new(vec![
                (0xf5, 0xc9, 0x5c),
                (0xf1, 0x7f, 0x5b),
                (0xf4, 0x5c, 0x82),
                (0xb0, 0x7c, 0xf4),
                (0x74, 0xa4, 0xff),
            ])
            .ansi(32),
            gradient_scroll: false,
            art_color: true,
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
    pub command_timeout_ms: u64,
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
            command_timeout_ms: 1000,
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
// Handles [section], [[table array]], key = value, single-line string arrays,
// and # comments — enough for config.toml with zero deps. No multiline values.

/// Strip a trailing `# comment` without eating `#RRGGBB` values. Quotes,
/// arrays and paths are respected, so both `path = /a#b` and
/// `gradient = ["#f5c95c", "#f17f5b"] # note` survive.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' if quote == Some(bytes[i]) => quote = None,
            b'"' | b'\'' if quote.is_none() => quote = Some(bytes[i]),
            b'#' if quote.is_none() => {
                let hex = line[i + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .count();
                let before = line[..i].trim_end();
                let is_value = before.ends_with('=')
                    || before.ends_with('[')
                    || before.ends_with(',');
                if is_value && matches!(hex, 3 | 6) {
                    i += 1 + hex;
                    continue;
                }
                if i == 0 || bytes[i - 1].is_ascii_whitespace() {
                    return &line[..i];
                }
            }
            _ => {}
        }
        i += 1;
    }
    line
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
                    if let Some(style) = toml_info_style(&content) {
                        apply_style(&mut cfg, &style);
                    }
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

/// Split a single-line TOML string array (`["os", "cpu"]`).
fn toml_array(val: &str) -> Option<Vec<String>> {
    let inner = val.trim().strip_prefix('[')?.strip_suffix(']')?;
    let mut out = Vec::new();
    for item in inner.split(',') {
        let item = item.trim();
        if item.is_empty() { continue; }
        let item = item.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
            .or_else(|| item.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(item);
        if !item.is_empty() { out.push(item.to_string()); }
    }
    Some(out)
}

fn module_field(m: &mut Module, key: &str, val: &str) {
    let k = key.trim().to_ascii_lowercase().replace(['-', '_'], "");
    match k.as_str() {
        "type" | "kind" => m.kind = val.trim().to_ascii_lowercase(),
        "key" => m.key = Some(val.to_string()),
        "format" => m.format = Some(val.to_string()),
        "keycolor" => m.key_color = Some(resolve_config_color(val)),
        "outputcolor" => m.output_color = Some(resolve_config_color(val)),
        "keyicon" => m.key_icon = Some(val.to_string()),
        "paddingleft" => m.padding_left = val.parse().unwrap_or(0),
        "keywidth" => m.key_width = val.parse().ok(),
        "text" => m.text = Some(val.to_string()),
        "symbol" => m.symbol = Some(val.to_string()),
        "value" => m.value = Some(val.to_string()),
        "command" => m.command = Some(val.to_string()),
        _ => {}
    }
}

fn mods(kinds: &[&str]) -> Vec<Module> {
    kinds.iter().map(|k| Module { kind: (*k).into(), ..Default::default() }).collect()
}

fn push_module(parsed: &mut Vec<Module>, m: Module) {
    // `modules = ["os", "custom"]` defines order; a later [[modules]] entry
    // fills that placeholder instead of adding a duplicate row.
    let bare = |p: &Module| p.format.is_none() && p.key.is_none() && p.key_icon.is_none()
        && p.value.is_none() && p.command.is_none() && p.text.is_none()
        && p.symbol.is_none() && p.key_color.is_none() && p.output_color.is_none();
    match parsed.iter().position(|p| p.kind == m.kind && bare(p)) {
        Some(i) => parsed[i] = m,
        None => parsed.push(m),
    }
}

fn install_modules(cfg: &mut Config, mods: Vec<Module>) {
    // The module list is the source of truth: don't collect rows that are not
    // rendered (this is what keeps custom layouts on the fast path).
    cfg.show.clear_fields();
    for m in &mods {
        // A static value or command replaces the collector's output, so don't
        // pay for it (notably public_ip/weather and other subprocess modules).
        if m.value.is_none() && m.command.is_none() {
            cfg.show.enable_module(&m.kind);
        }
    }
    cfg.modules = Some(mods);
}

/// Built-in starting points for `info_style`. Users can override anything with
/// `[[modules]]`, so these are layouts, not a closed list.
fn apply_style(cfg: &mut Config, name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    match n.as_str() {
        "classic" | "" | "default" => return false,
        "minimal" => {
            cfg.show.header = Header::None;
            cfg.display.labels = false;
            cfg.display.separator = Some(" ".into());
            cfg.display.key_width = Some(0);
            install_modules(cfg, mods(&["os", "kernel", "de", "shell", "term", "cpu", "gpu", "memory", "disk", "colors"]));
        }
        "boxed" | "card" | "chips" => {
            cfg.layout.box_style = "round".into();
            install_modules(cfg, mods(&["os", "host", "kernel", "uptime", "packages", "shell", "de", "term", "cpu", "gpu", "memory", "disk", "colors"]));
        }
        "sections" => {
            let mut ms = Vec::new();
            let heading = |t: &str| Module {
                kind: "custom".into(),
                format: Some(format!("{{#accent}}\u{2500}\u{2500} {t} \u{2500}\u{2500}{{#}}")),
                ..Default::default()
            };
            ms.push(heading("Hardware"));
            ms.extend(mods(&["host", "cpu", "gpu", "memory", "disk"]));
            ms.push(heading("Software"));
            ms.extend(mods(&["os", "kernel", "packages", "shell", "de", "term"]));
            ms.push(heading("System"));
            ms.extend(mods(&["uptime", "battery", "colors"]));
            cfg.display.key_width = Some(11);
            install_modules(cfg, ms);
        }
        "tree" => {
            let mut ms = Vec::new();
            ms.push(Module {
                kind: "custom".into(),
                format: Some("{#accent}{user-name}@{host-name}{#}".into()),
                ..Default::default()
            });
            for (kind, label) in [
                ("os", "\u{251c}\u{2500} os"),
                ("kernel", "\u{251c}\u{2500} kernel"),
                ("packages", "\u{251c}\u{2500} packages"),
                ("shell", "\u{251c}\u{2500} shell"),
                ("de", "\u{251c}\u{2500} wm"),
                ("term", "\u{251c}\u{2500} terminal"),
                ("cpu", "\u{251c}\u{2500} cpu"),
                ("gpu", "\u{251c}\u{2500} gpu"),
                ("memory", "\u{2514}\u{2500} memory"),
            ] {
                ms.push(Module { kind: kind.into(), key: Some(label.into()), ..Default::default() });
            }
            cfg.display.key_width = Some(15);
            cfg.display.separator = Some(" : ".into());
            install_modules(cfg, ms);
        }
        "iconic" => {
            cfg.display.labels = true;
            cfg.display.key_width = Some(2);
            cfg.display.separator = Some(" ".into());
            let mut ms = mods(&["os", "kernel", "de", "shell", "term", "cpu", "gpu", "memory", "disk"]);
            for (m, icon) in ms.iter_mut().zip([
                "\u{f17c}", "\u{f17c}", "\u{f108}", "\u{f120}", "\u{f120}",
                "\u{f2db}", "\u{f108}", "\u{f0a0}", "\u{f0a0}",
            ]) {
                m.key = Some(String::new());
                m.key_icon = Some(icon.into());
            }
            install_modules(cfg, ms);
        }
        other => {
            eprintln!("arcfetch: warning: unknown info_style '{other}' (try: classic, minimal, boxed, sections, tree, iconic, card, chips)");
            return false;
        }
    }
    true
}

/// Read `info_style` from the root of a TOML file without applying anything.
fn toml_info_style(content: &str) -> Option<String> {
    let mut section = "";
    for raw_line in content.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() { continue; }
        if let Some(rest) = line.strip_prefix('[') {
            section = rest.split_once(']').map(|(n, _)| n.trim()).unwrap_or("");
            continue;
        }
        if section.is_empty() {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "info_style" {
                    let v = v.trim();
                    let v = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(v);
                    if !v.is_empty() { return Some(v.to_string()); }
                }
            }
        }
    }
    None
}

fn apply_toml(cfg: &mut Config, content: &str) {
    let mut section = "";
    let mut current: Option<Module> = None;
    let mut parsed: Vec<Module> = Vec::new();
    let mut override_kind: Option<String> = None;
    let mut overrides: Vec<(String, Option<String>, Option<String>, Option<String>)> = Vec::new();
    let mut modules_seen = false;

    macro_rules! flush {
        () => {
            if let Some(m) = current.take() {
                if !m.kind.is_empty() { push_module(&mut parsed, m); }
            }
        };
    }

    for (lineno, raw_line) in content.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() { continue; }

        // [[modules]] / [[custom]]
        if let Some(rest) = line.strip_prefix("[[") {
            if let Some(name) = rest.strip_suffix("]]") {
                flush!();
                let name = name.trim();
                modules_seen = true;
                override_kind = None;
                if name == "modules" || name == "custom" {
                    section = name;
                    let mut m = Module::default();
                    if name == "custom" { m.kind = "custom".into(); }
                    current = Some(m);
                } else {
                    eprintln!("arcfetch: warning: unknown table array [[{name}]] at line {}", lineno + 1);
                }
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix('[') {
            if let Some((name, _)) = rest.split_once(']') {
                let name = name.trim();
                if !name.is_empty() {
                    flush!();
                    section = name;
                    override_kind = name.strip_prefix("overrides.").map(|s| s.to_string());
                    if let Some(kind) = &override_kind {
                        overrides.push((kind.clone(), None, None, None));
                    }
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

        if section == "modules" || section == "custom" {
            if let Some(m) = current.as_mut() { module_field(m, key, val); }
            continue;
        }

        if override_kind.is_some() {
            if let Some(o) = overrides.last_mut() {
                match key.to_ascii_lowercase().as_str() {
                    "label" => o.1 = Some(val.to_string()),
                    "value" => o.2 = Some(val.to_string()),
                    "command" => o.3 = Some(val.to_string()),
                    _ => {}
                }
            }
            continue;
        }

        match section {
            "" => match key {
                "modules" => {
                    if let Some(items) = toml_array(val) {
                        modules_seen = true;
                        parsed = items.into_iter()
                            .map(|k| Module { kind: k.trim().to_ascii_lowercase(), ..Default::default() })
                            .collect();
                    } else {
                        eprintln!("arcfetch: warning: modules must be a single-line array, e.g. [\"os\", \"cpu\"]");
                    }
                }
                "command_timeout" => {
                    let secs = val.parse::<f64>().unwrap_or(1.0);
                    cfg.command_timeout_ms = (secs * 1000.0).clamp(100.0, 10_000.0) as u64;
                }
                // handled before this pass
                "info_style" => {}
                _ => {}
            },
            "display" => match key {
                "separator" => cfg.display.separator = Some(val.to_string()),
                "separator_color" => cfg.display.separator_color = Some(resolve_config_color(val)),
                "key_color" => cfg.display.key_color = Some(resolve_config_color(val)),
                "key_width" => cfg.display.key_width = val.parse().ok(),
                "key_padding_left" => cfg.display.key_padding_left = val.parse().unwrap_or(0),
                "bar_width" => cfg.display.bar_width = val.parse().unwrap_or(14).clamp(1, 200),
                "bar_elapsed" => cfg.display.bar_elapsed = val.to_string(),
                "bar_total" => cfg.display.bar_total = val.to_string(),
                "labels" => cfg.display.labels = matches!(val, "true" | "yes" | "1" | "on"),
                _ => {}
            },
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
                "data" => cfg.logo.data = Some(val.to_string()),
                "position" => cfg.logo.position = val.to_ascii_lowercase(),
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
                    "gradient" => {
                        if let Some(g) = parse_gradient(val) {
                            cfg.animation.gradient = g.ansi(32);
                        }
                    }
                    "gradient_scroll" => cfg.animation.gradient_scroll = on,
                    "art_color" => cfg.animation.art_color = !art_color_is_theme(val),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    flush!();

    let has_overrides = !overrides.is_empty();
    if has_overrides && !modules_seen {
        parsed = mods(&["os", "host", "kernel", "uptime", "packages", "shell", "de", "term", "cpu", "gpu", "memory", "disk", "colors"]);
    }
    for (kind, label, value, command) in overrides {
        let mut target = parsed.iter_mut().find(|m| m.kind == kind);
        if target.is_none() {
            parsed.push(Module { kind: kind.clone(), ..Default::default() });
            target = parsed.last_mut();
        }
        let m = target.unwrap();
        if let Some(v) = label { m.key = Some(v); }
        if let Some(v) = value { m.value = Some(v); }
        if let Some(v) = command { m.command = Some(v); }
    }

    if modules_seen || has_overrides {
        install_modules(cfg, parsed);
    }
}
fn apply_jsonc(cfg: &mut Config, text: &str) -> Result<(), String> {
    let root = jsonc::parse(text)?;

    if let Some(style) = root.get("info_style").and_then(jsonc::Value::as_str) {
        apply_style(cfg, style);
    }

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
        if let Some(v) = d.get("labels").and_then(jsonc::Value::as_bool) {
            cfg.display.labels = v;
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
            let mut m = Module::default();
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
                    m.value = item.get("value").and_then(jsonc::Value::as_str).map(String::from);
                    m.command = item.get("command").and_then(jsonc::Value::as_str).map(String::from);
                }
                _ => continue,
            }
            if !m.kind.is_empty() { mods.push(m); }
        }
        cfg.show.clear_fields();
        cfg.show.header = Header::None;
        install_modules(cfg, mods);
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

    #[test]
    fn toml_modules_overrides_and_styles() {
        let mut cfg = Config::default();
        apply_toml(&mut cfg, r#"
modules = ["os", "custom", "uptime"]
command_timeout = 5
[[custom]]
format = "head"
[[modules]]
type = "uptime"
key = "up"
[overrides.os]
label = "Distro"
"#);
        let mods = cfg.modules.as_ref().unwrap();
        assert_eq!(mods.iter().map(|m| m.kind.as_str()).collect::<Vec<_>>(),
                   ["os", "custom", "uptime"]);
        assert_eq!(mods[0].key.as_deref(), Some("Distro"));
        assert_eq!(mods[1].format.as_deref(), Some("head"));
        assert_eq!(mods[2].key.as_deref(), Some("up"));
        assert_eq!(cfg.command_timeout_ms, 5000);

        let mut cfg = Config::default();
        assert!(apply_style(&mut cfg, "minimal"));
        assert!(!cfg.display.labels);
        assert!(cfg.modules.as_ref().is_some_and(|m| !m.is_empty()));
    }
}

    #[test]
    fn scheme_uses_the_canonical_palette() {
        let c = scheme("mocha").unwrap();
        assert_eq!(c.accent, "\x1b[38;2;137;180;250m");   // blue #89b4fa
        assert_eq!(c.username, "\x1b[38;2;203;166;247m"); // mauve #cba6f7
        assert_eq!(c.values, "\x1b[38;2;186;194;222m");   // subtext1 #bac2de
    }

    #[test]
    fn scheme_aliases_and_invalid_names() {
        assert_eq!(scheme("catppuccin").unwrap().accent, scheme("mocha").unwrap().accent);
        assert_eq!(scheme("tokyonight").unwrap().accent, scheme("tokyo-night").unwrap().accent);
        assert_eq!(scheme("atom").unwrap().accent, scheme("onedark").unwrap().accent);
        assert!(scheme("no-such-scheme").is_none());
    }

    #[test]
    fn scheme_names_match_the_palette_module() {
        assert_eq!(SCHEMES[0], "auto");
        let names: Vec<&str> = crate::scheme::SCHEMES
            .iter()
            .map(|s| s.palette.name)
            .collect();
        assert_eq!(&SCHEMES[1..], names.as_slice());
    }

    #[test]
    fn colour_depth_fallbacks_are_exact() {
        let red = "\x1b[38;2;255;0;0m";
        assert_eq!(downgrade_str(red, ColorDepth::True), red);
        assert_eq!(downgrade_str(red, ColorDepth::Ansi256), "\x1b[38;5;196m");
        assert_eq!(downgrade_str(red, ColorDepth::Ansi16), "\x1b[91m");
    }

    #[test]
    fn preview_swatches_fall_back_without_truecolour() {
        let sw = crate::scheme::swatch((255, 0, 0));
        assert_eq!(downgrade_str(&sw, ColorDepth::Ansi256), "\x1b[38;5;196m██\x1b[0m");
        assert_eq!(downgrade_str(&sw, ColorDepth::Ansi16), "\x1b[91m██\x1b[0m");
    }

    #[test]
    fn logo_slots_expand_without_breaking_unicode() {
        let mut logo = std::array::from_fn(|_| String::new());
        logo[0] = "<red>".into();
        assert_eq!(expand_logo_slots("$1█ $$ $R", &logo), "<red>█ $ \x1b[0m");
    }

    #[test]
    fn gradient_keeps_quoted_hex_and_skips_bad_stops() {
        let line = r##"gradient = ["#f5c95c", "#f17f5b", "nope"] # comment"##;
        let line = strip_comment(line).trim();
        assert_eq!(line, r##"gradient = ["#f5c95c", "#f17f5b", "nope"]"##);
        let val = line.split_once('=').unwrap().1.trim();
        let g = parse_gradient(val).unwrap();
        assert_eq!(g.sample(0.0), Some((0xf5, 0xc9, 0x5c)));
        assert_eq!(g.sample(1.0), Some((0xf1, 0x7f, 0x5b)));
        assert_eq!(g.ansi(2).len(), 2);
    }

    #[test]
    fn art_color_theme_accepts_quotes() {
        assert!(art_color_is_theme("\"theme\""));
        assert!(!art_color_is_theme("\"own\""));
    }

    #[test]
    fn unquoted_hex_survives_comment_stripping() {
        assert_eq!(strip_comment("accent = #89b4fa # blue"), "accent = #89b4fa ");
        assert_eq!(strip_comment("# whole-line comment"), "");
    }
