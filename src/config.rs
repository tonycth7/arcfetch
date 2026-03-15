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

// ── name → ANSI escape ───────────────────────────────────
pub fn name_to_ansi(name: &str) -> &'static str {
    // also accepts #RRGGBB — but we return a static str so hex is handled
    // separately via owned String in Colors
    match name.trim() {
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
        _           => BLUE,
    }
}

/// Resolve a color name or #RRGGBB hex to an owned ANSI escape string.
fn resolve(name: &str) -> String {
    let name = name.trim();
    if name.starts_with('#') && name.len() == 7 {
        let r = u8::from_str_radix(&name[1..3], 16).unwrap_or(137);
        let g = u8::from_str_radix(&name[3..5], 16).unwrap_or(180);
        let b = u8::from_str_radix(&name[5..7], 16).unwrap_or(250);
        return format!("\x1b[38;2;{};{};{}m", r, g, b);
    }
    name_to_ansi(name).into()
}

// ── Colors ───────────────────────────────────────────────

pub struct Colors {
    pub accent:   String,
    pub username: String,
    pub hostname: String,
    pub labels:   [String; 7],  // c1..c7, cycle via label(i)
    pub values:   String,
    pub sep:      String,
    pub bar:      String,
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
        }
    }
}

// ── Show (per-field visibility) ──────────────────────────

pub struct Show {
    pub os:      bool,
    pub kernel:  bool,
    pub uptime:  bool,
    pub res:     bool,
    pub pkgs:    bool,
    pub shell:   bool,
    pub de_wm:   bool,
    pub term:    bool,
    pub cpu:     bool,
    pub gpu:     bool,
    pub memory:  bool,
    pub disk:    bool,
    pub load:    bool,
    pub locale:  bool,
    pub swatches: bool,
}

impl Default for Show {
    fn default() -> Self {
        Show {
            os: true, kernel: true, uptime: true, res: false,
            pkgs: true, shell: true, de_wm: true, term: true,
            cpu: true, gpu: true, memory: true, disk: true,
            load: false, locale: false, swatches: true,
        }
    }
}

impl Show {
    fn apply_preset(&mut self, preset: &str) {
        // reset all to false first, then enable the preset fields
        *self = Show {
            os: false, kernel: false, uptime: false, res: false,
            pkgs: false, shell: false, de_wm: false, term: false,
            cpu: false, gpu: false, memory: false, disk: false,
            load: false, locale: false, swatches: false,
        };
        match preset.trim() {
            "minimal" => {
                self.os = true; self.kernel = true;
                self.uptime = true; self.memory = true;
                self.swatches = true;
            }
            "hacker" => {
                self.cpu = true; self.gpu = true; self.memory = true;
                self.disk = true; self.load = true;
                self.swatches = true;
            }
            "science" => {
                self.os = true; self.kernel = true; self.cpu = true;
                self.memory = true; self.disk = true;
                self.swatches = true;
            }
            _ => {
                // "full" or anything else — show everything
                *self = Show {
                    os: true, kernel: true, uptime: true, res: true,
                    pkgs: true, shell: true, de_wm: true, term: true,
                    cpu: true, gpu: true, memory: true, disk: true,
                    load: true, locale: true, swatches: true,
                };
            }
        }
    }

    fn set_field(&mut self, key: &str, val: bool) {
        match key {
            "os"       => self.os      = val,
            "kernel"   => self.kernel  = val,
            "uptime"   => self.uptime  = val,
            "res"      => self.res     = val,
            "pkgs"     => self.pkgs    = val,
            "shell"    => self.shell   = val,
            "de_wm"    => self.de_wm   = val,
            "term"     => self.term    = val,
            "cpu"      => self.cpu     = val,
            "gpu"      => self.gpu     = val,
            "memory"   => self.memory  = val,
            "disk"     => self.disk    = val,
            "load"     => self.load    = val,
            "locale"   => self.locale  = val,
            "swatches" => self.swatches= val,
            _ => {}
        }
    }
}

// ── Config ───────────────────────────────────────────────

pub struct Config {
    pub colors: Colors,
    pub show:   Show,
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

pub fn load(cli_accent: Option<&str>) -> Config {
    let mut colors = Colors::default();
    let mut show   = Show::default();
    let mut preset: Option<String> = None;

    // override accent from env var
    let env_accent = env::var("ARCFETCH_ACCENT").ok();
    let accent_override = cli_accent
        .map(String::from)
        .or(env_accent);

    // try reading config file
    let path = config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        let mut section = "";
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }

            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len()-1];
                continue;
            }

            let Some((key, val)) = line.split_once('=') else { continue };
            let key = key.trim();
            let val = val.split('#').next().unwrap_or("").trim(); // strip inline comment

            match section {
                "colors" => {
                    let ansi = resolve(val);
                    match key {
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
                        _ => {}
                    }
                }
                "show" => {
                    let enabled = matches!(val, "true" | "yes" | "1" | "on");
                    show.set_field(key, enabled);
                }
                "template" => {
                    if key == "preset" { preset = Some(val.to_string()); }
                }
                _ => {}
            }
        }
    }

    // apply preset (overrides [show] if set)
    if let Some(p) = preset { show.apply_preset(&p); }

    // CLI / env accent wins over file
    if let Some(a) = accent_override {
        let ansi = resolve(&a);
        colors.accent   = ansi.clone();
        colors.hostname = ansi;
    }

    Config { colors, show }
}
