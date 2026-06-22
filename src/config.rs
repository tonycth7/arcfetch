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
pub enum Header {
    Both,        // user@host  (default)
    UserOnly,    // user
    HostOnly,    // host
    None,        // no header line at all
}

impl Default for Header { fn default() -> Self { Header::Both } }

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
        }
    }
}

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
            _ => {}
        }
    }
}

// ── Logo ─────────────────────────────────────────────────

pub struct Logo {
    pub name: Option<String>,  // built-in logo name (arch, ascii, tux, nix, gentoo, mini, auto)
    pub file: Option<String>,  // custom ASCII file path (overrides name if set)
}

impl Default for Logo {
    fn default() -> Self { Logo { name: None, file: None } }
}

// ── Config ───────────────────────────────────────────────

pub struct Config {
    pub colors: Colors,
    pub show:   Show,
    pub preset: Option<String>,
    pub logo:   Logo,
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

pub fn load(cli_accent: Option<&str>, cli_preset: Option<&str>) -> Config {
    let mut colors = Colors::default();
    let mut show   = Show::default();
    let mut preset: Option<String> = None;
    let mut logo   = Logo::default();
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
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }

            // strip inline comment before checking section header
            let header_part = line.split('#').next().unwrap_or("").trim();
            if header_part.starts_with('[') {
                if let Some(close) = header_part.find(']') {
                    let name = header_part[1..close].trim();
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
                "template" => {
                    if key == "preset" { preset = Some(val.to_string()); }
                }
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

    Config { colors, show, preset, logo }
}
