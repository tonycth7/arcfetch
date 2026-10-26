//! Fastfetch JSONC adapter.
//!
//! Fastfetch and arcfetch share the idea of ordered modules but not the
//! schema. This module detects a fastfetch config and translates the parts
//! that have an arcfetch equivalent: logo, display, per-module overrides and
//! the common default formats/tokens. Anything without a collector is
//! dropped instead of leaking a literal `{placeholder}` into the output.

use std::path::Path;
use std::process::Command;

use crate::config::{install_modules, Config, Header, Module};
use crate::jsonc::Value;

pub fn is_fastfetch(root: &Value) -> bool {
    if root.get("$schema")
        .and_then(Value::as_str)
        .is_some_and(|s| s.to_ascii_lowercase().contains("fastfetch"))
    {
        return true;
    }
    // Fallbacks for configs copied without `$schema`. `display.key.type`
    // is deliberately not used: several shipped arcfetch presets use it too.
    if root.get("general").is_some() {
        return true;
    }
    if let Some(t) = root.get("logo").and_then(|l| l.get("type")).and_then(Value::as_str) {
        if matches!(
            t.to_ascii_lowercase().as_str(),
            "kitty" | "kitty-icat" | "kitty-direct" | "sixel" | "iterm" | "chafa"
                | "file-raw" | "data-raw" | "command-raw"
        ) {
            return true;
        }
    }
    false
}

pub fn apply(cfg: &mut Config, root: &Value) -> Result<(), String> {
    cfg.fastfetch = true;
    cfg.show.header = Header::None;
    // Fastfetch's default duration format is the long one.
    cfg.show.uptime_long = true;
    // Fastfetch prints `key: value` without padding and aligns only long keys
    // by their own natural width.
    cfg.display.key_width = Some(0);
    cfg.display.separator = Some(": ".into());

    apply_logo(cfg, root.get("logo"));
    apply_display(cfg, root.get("display"));

    let detect_version = root
        .get("general")
        .and_then(|g| g.get("detectVersion"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    apply_modules(cfg, root, detect_version);
    Ok(())
}

// ── Logo ─────────────────────────────────────────────────

fn apply_logo(cfg: &mut Config, logo: Option<&Value>) {
    let Some(logo) = logo else { return };
    match logo {
        Value::Null => {
            cfg.logo.name = Some("none".into());
            cfg.logo.file = None;
            cfg.logo.data = None;
        }
        Value::Str(s) => apply_logo_string(cfg, s),
        Value::Obj(_) => {
            let ty = logo
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("auto")
                .trim()
                .to_ascii_lowercase();
            let source = logo.get("source").and_then(Value::as_str);

            match ty.as_str() {
                "none" => {
                    cfg.logo.name = Some("none".into());
                    cfg.logo.file = None;
                    cfg.logo.data = None;
                }
                "small" => {
                    cfg.logo.name = Some("mini".into());
                    cfg.logo.file = None;
                    cfg.logo.data = None;
                }
                "data" | "data-raw" => {
                    cfg.logo.name = None;
                    cfg.logo.file = None;
                    cfg.logo.data = source.map(String::from);
                }
                "command-raw" => {
                    cfg.logo.name = None;
                    cfg.logo.file = None;
                    cfg.logo.data = source.and_then(run_command);
                }
                "file" | "file-raw" => {
                    cfg.logo.name = None;
                    cfg.logo.data = None;
                    cfg.logo.file = source.map(String::from);
                }
                "builtin" | "auto" => match source {
                    Some(s) => apply_logo_string(cfg, s),
                    None => {
                        cfg.logo.name = Some("auto".into());
                        cfg.logo.file = None;
                        cfg.logo.data = None;
                    }
                },
                // Image protocols render from `source`; the distro logo is
                // the fallback when no image was given.
                "kitty" | "kitty-icat" | "kitty-direct" | "iterm" | "iterm2"
                | "sixel" | "chafa" | "raw" => {
                    cfg.logo.name = Some("auto".into());
                    cfg.logo.data = None;
                    cfg.logo.file = source.map(String::from);
                }
                other => match source {
                    Some(s) => apply_logo_string(cfg, s),
                    None => cfg.logo.name = Some(other.to_string()),
                },
            }

            cfg.logo.width = logo.get("width").and_then(Value::as_usize);
            cfg.logo.height = logo.get("height").and_then(Value::as_usize);

            if let Some(pad) = logo.get("padding") {
                if let Some(top) = pad.get("top").and_then(Value::as_usize) {
                    cfg.logo.padding_top = top;
                }
                if let Some(right) = pad.get("right").and_then(Value::as_usize) {
                    cfg.layout.gap = right.min(32);
                }
                if let Some(left) = pad.get("left").and_then(Value::as_usize) {
                    cfg.layout.indent = left.min(32);
                }
            }

            if let Some(pos) = logo.get("position").and_then(Value::as_str) {
                let pos = pos.trim().to_ascii_lowercase();
                cfg.logo.position = if pos == "auto" { "left".into() } else { pos };
            }

            if let Some(color) = logo.get("color") {
                for i in 1..=9 {
                    if let Some(v) = color.get(&i.to_string()).and_then(Value::as_str) {
                        cfg.colors.logo[i - 1] = ff_color(v);
                    }
                }
            }
        }
        _ => {}
    }
}

fn apply_logo_string(cfg: &mut Config, s: &str) {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        "none" => {
            cfg.logo.name = Some("none".into());
            cfg.logo.file = None;
            cfg.logo.data = None;
            return;
        }
        "small" => {
            cfg.logo.name = Some("mini".into());
            cfg.logo.file = None;
            cfg.logo.data = None;
            return;
        }
        "auto" | "builtin" => {
            cfg.logo.name = Some("auto".into());
            cfg.logo.file = None;
            cfg.logo.data = None;
            return;
        }
        _ => {}
    }

    if t.contains('\n') {
        cfg.logo.name = None;
        cfg.logo.file = None;
        cfg.logo.data = Some(s.to_string());
    } else if t.contains('/')
        || t.starts_with('~')
        || lower.ends_with(".txt")
        || is_image(&lower)
        || Path::new(t).exists()
    {
        cfg.logo.name = None;
        cfg.logo.data = None;
        cfg.logo.file = Some(s.to_string());
    } else if crate::logos::all_names().iter().any(|n| n.eq_ignore_ascii_case(t)) {
        cfg.logo.name = Some(lower);
        cfg.logo.file = None;
        cfg.logo.data = None;
    } else {
        // Fastfetch has distro logos arcfetch does not; use auto detection.
        cfg.logo.name = Some("auto".into());
        cfg.logo.file = None;
        cfg.logo.data = None;
    }
}

fn run_command(cmd: &str) -> Option<String> {
    let out = Command::new("sh").args(["-c", cmd]).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

fn is_image(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(""),
        "png" | "jpg" | "jpeg" | "gif"
    )
}

fn title_width() -> usize {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default();
    1 + user.chars().count() + crate::hostname().chars().count()
}

fn fill_separator(s: &str, width: usize) -> String {
    let w = s.chars().count();
    if w == 0 { return String::new(); }
    let mut out = String::with_capacity(width * w);
    let mut remaining = width;
    while remaining >= w {
        out.push_str(s);
        remaining -= w;
    }
    if remaining > 0 {
        out.extend(s.chars().take(remaining));
    }
    out
}

// ── Display ──────────────────────────────────────────────

fn apply_display(cfg: &mut Config, display: Option<&Value>) {
    let Some(d) = display else { return };

    if let Some(v) = d.get("separator").and_then(Value::as_str) {
        cfg.display.separator = Some(v.to_string());
    }

    if let Some(color) = d.get("color") {
        match color {
            Value::Str(s) => {
                let c = ff_color(s);
                cfg.display.key_color = Some(c.clone());
                cfg.colors.accent = c;
            }
            Value::Obj(_) => {
                if let Some(v) = color.get("keys").and_then(Value::as_str) {
                    cfg.display.key_color = Some(ff_color(v));
                }
                if let Some(v) = color.get("output").and_then(Value::as_str) {
                    cfg.colors.values = ff_color(v);
                }
                if let Some(v) = color.get("separator").and_then(Value::as_str) {
                    cfg.display.separator_color = Some(ff_color(v));
                }
                if let Some(v) = color.get("title").and_then(Value::as_str) {
                    cfg.colors.accent = ff_color(v);
                }
            }
            _ => {}
        }
    }

    if let Some(key) = d.get("key") {
        if let Some(w) = key.get("width").and_then(Value::as_usize) {
            cfg.display.key_width = Some(w);
        }
        if let Some(p) = key.get("paddingLeft").and_then(Value::as_usize) {
            cfg.display.key_padding_left = p;
        }
        if let Some(t) = key.get("type").and_then(Value::as_str) {
            // arcfetch has no built-in icon map, so icon-only configs still
            // get their string keys instead of losing the label entirely.
            cfg.display.labels = !t.eq_ignore_ascii_case("none");
        }
    }

    if let Some(bar) = d.get("bar") {
        if let Some(w) = bar.get("width").and_then(Value::as_usize) {
            cfg.display.bar_width = w.clamp(1, 200);
        }
        if let Some(ch) = bar.get("char") {
            if let Some(v) = ch.get("elapsed").and_then(Value::as_str) {
                cfg.display.bar_elapsed = v.to_string();
            }
            if let Some(v) = ch.get("total").and_then(Value::as_str) {
                cfg.display.bar_total = v.to_string();
            }
        }
    }

    if let Some(arr) = d.get("constants").and_then(Value::as_array) {
        cfg.display.constants = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    }

    if let Some(abbr) = d.get("duration")
        .and_then(|x| x.get("abbreviation"))
        .and_then(Value::as_bool)
    {
        cfg.show.uptime_long = !abbr;
    }
}

// ── Modules ──────────────────────────────────────────────

fn apply_modules(cfg: &mut Config, root: &Value, detect_version: bool) {
    let Some(arr) = root.get("modules").and_then(Value::as_array) else { return };

    let mut mods = Vec::with_capacity(arr.len());
    let mut need_wm_version = false;
    let has_display = arr.iter().any(|it| module_type(it).as_deref() == Some("display"));
    let has_bluetooth = arr.iter().any(|it| module_type(it).as_deref() == Some("bluetooth"));
    let has_de = desktop_environment_present();

    for item in arr {
        let (ty, obj) = match item {
            Value::Str(s) => (s.trim().to_ascii_lowercase(), None),
            Value::Obj(_) => {
                let Some(ty) = item.get("type").and_then(Value::as_str) else { continue };
                (ty.trim().to_ascii_lowercase(), Some(item))
            }
            _ => continue,
        };
        if ty.is_empty() { continue; }
        if let Some(cond) = obj.and_then(|o| o.get("condition")) {
            if !condition_ok(cond) { continue; }
        }
        // arcfetch detects a single DE/WM name, so a WM-only session must not
        // grow a "DE" row, and Monitor would just repeat Display's data.
        if ty == "de" && !has_de { continue; }
        if ty == "monitor" && has_display { continue; }
        // arcfetch has one Bluetooth collector (radio info), so the paired
        // `bluetoothradio` row would just repeat it.
        if ty == "bluetoothradio" && has_bluetooth { continue; }

        let Some(mut m) = make_module(&ty, obj, detect_version) else { continue };
        if m.kind == "de" && detect_version {
            need_wm_version = true;
        }
        if obj.is_some_and(|o| o.get("combined").and_then(Value::as_bool) == Some(true)) {
            m.combined = true;
        }
        if let Some(Value::Bool(want)) = obj.and_then(|o| o.get("condition")).and_then(|c| c.get("succeeded")) {
            m.condition_succeeded = Some(*want);
        }
        mods.push(m);
    }

    install_modules(cfg, mods);
    if need_wm_version {
        cfg.show.de_wm_ver = true;
    }
}

fn module_type(item: &Value) -> Option<String> {
    match item {
        Value::Str(s) => Some(s.trim().to_ascii_lowercase()),
        _ => item.get("type").and_then(Value::as_str).map(|s| s.trim().to_ascii_lowercase()),
    }
}

/// Whether the session names a real desktop environment (not just a WM).
fn desktop_environment_present() -> bool {
    let de = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["gnome", "kde", "plasma", "xfce", "cinnamon", "mate", "lxde", "lxqt",
     "budgie", "pantheon", "unity", "deepin", "cosmic", "enlightenment",
     "trinity", "cde", "ukui", "nebi"]
        .iter()
        .any(|d| de.contains(d))
}

fn make_module(ty: &str, item: Option<&Value>, detect_version: bool) -> Option<Module> {
    let kind = map_kind(ty)?;
    let mut m = Module { kind, ..Module::default() };

    if let Some(item) = item {
        m.key = item.get("key").and_then(Value::as_str).map(String::from);
        m.format = item.get("format").and_then(Value::as_str).map(String::from);
        m.key_color = item.get("keyColor").and_then(Value::as_str).map(ff_color);
        m.output_color = item.get("outputColor").and_then(Value::as_str).map(ff_color);
        m.key_icon = item.get("keyIcon").and_then(Value::as_str).map(String::from);
        m.padding_left = item.get("paddingLeft").and_then(Value::as_usize).unwrap_or(0);
        m.key_width = item.get("keyWidth").and_then(Value::as_usize);
        m.text = item.get("text").and_then(Value::as_str).map(String::from);
        m.symbol = item.get("symbol").and_then(Value::as_str).map(String::from);
        m.value = item.get("value").and_then(Value::as_str).map(String::from);
        m.command = item.get("command").and_then(Value::as_str).map(String::from);
        if let Some(arr) = item.get("disabled").and_then(Value::as_array) {
            m.disabled = arr.iter().filter_map(Value::as_str).map(String::from).collect();
        }

        if ty == "separator" {
            let s = item.get("string").and_then(Value::as_str).unwrap_or("-");
            let times = item.get("times").and_then(Value::as_usize).unwrap_or(0);
            // Fastfetch's default separator fills the user@host title width;
            // `times` repeats the string verbatim instead.
            m.format = Some(if times > 0 {
                s.repeat(times)
            } else {
                fill_separator(s, title_width())
            });
        }
        // Fastfetch's custom module prints `key <separator> format`; arcfetch
        // normally treats `format` as the whole line. Keep the format as the
        // value when a key was supplied.
        if ty == "custom" && m.key.is_some() {
            if let Some(f) = m.format.take() {
                m.value = Some(f);
                m.value_format = true;
            }
        }
    }

    if ty == "separator" && m.format.is_none() {
        m.format = Some(fill_separator("-", title_width()));
    }
    if m.format.is_none() {
        m.format = default_format(ty, detect_version);
    }
    if m.key.is_none() {
        if let Some(k) = default_key(ty) {
            m.key = Some(k.to_string());
        }
    }
    Some(m)
}

fn map_kind(ty: &str) -> Option<String> {
    let mapped = match ty {
        "wm" => "de",
        "terminal" => "term",
        "packages" => "pkgs",
        "initsystem" => "init",
        "physicalmemory" => "physmem",
        "gpumemory" => "gpumem",
        "top" => "topprocesses",
        "display" => "monitor",
        "font" | "terminalfont" => "termfont",
        "lm" => "loginmanager",
        "loadavg" => "load",
        "terminalsize" => "termsize",
        "terminaltheme" => "termtheme",
        "bluetoothradio" => "bluetooth",
        // wmtheme is dropped: arcfetch has no WM-theme collector, and mapping
        // it to `theme` printed the same row twice.
        "wmtheme" => return None,
        _ => ty,
    };
    if mapped == ty && !KNOWN_KINDS.contains(&ty) {
        return None;
    }
    Some(mapped.to_string())
}

// Arcfetch module kinds. Fastfetch-only modules (logo, player, zpool, …)
// intentionally fall through and are skipped.
const KNOWN_KINDS: &[&str] = &[
    "os", "kernel", "uptime", "res", "pkgs", "shell", "de", "term", "cpu", "gpu",
    "gputemp", "battery", "memory", "disk", "load", "locale", "ip", "localip", "ssh", "ports",
    "init", "cputemp", "processes", "container", "session", "swap", "sound",
    "gpudriver", "bios", "board", "disktype", "pkgupdates", "theme", "icons",
    "termfont", "dewmver", "initver", "datetime", "editor", "dns", "host",
    "chassis", "bootmgr", "cpucache", "loginmanager", "brightness", "cpuusage",
    "diskio", "netio", "physmem", "gpumem", "btrfs", "wifi", "users", "wallpaper",
    "media", "publicip", "weather", "opengl", "vulkan", "cursor", "termsize",
    "termtheme", "codec", "bluetooth", "camera", "mouse", "keyboard", "tpm",
    "physicaldisk", "topprocesses", "monitor", "custom", "title", "separator",
    "break", "colors", "command", "version",
];

fn default_key(ty: &str) -> Option<&'static str> {
    // Fastfetch's own module keys, so a translated config reads like the
    // original. Dynamic keys (display/battery device names) are built by
    // `module_value` in main.rs.
    match ty {
        "title" | "separator" | "break" | "colors" | "custom" | "command" => None,
        "os" => Some("OS"),
        "host" => Some("Host"),
        "bios" => Some("BIOS"),
        "board" => Some("Board"),
        "bootmgr" => Some("Boot Manager"),
        "chassis" => Some("Chassis"),
        "kernel" => Some("Kernel"),
        "initsystem" | "init" => Some("Init System"),
        "uptime" => Some("Uptime"),
        "loadavg" => Some("Load Average"),
        "processes" => Some("Processes"),
        "packages" => Some("Packages"),
        "shell" => Some("Shell"),
        "de" => Some("Desktop Environment"),
        "wm" => Some("Window Manager"),
        "theme" => Some("Theme"),
        "icons" => Some("Icons"),
        "font" => Some("Font"),
        "terminal" => Some("Terminal"),
        "terminalfont" => Some("Terminal Font"),
        "terminalsize" => Some("Terminal Size"),
        "terminaltheme" => Some("Terminal Theme"),
        "cursor" => Some("Cursor"),
        "wallpaper" => Some("Wallpaper"),
        "lm" => Some("Login Manager"),
        "display" | "battery" => None,
        "brightness" => Some("Brightness"),
        "cpu" => Some("CPU"),
        "cpucache" => Some("CPU Cache"),
        "cpuusage" => Some("CPU Usage"),
        "gpu" => Some("GPU"),
        "gpudriver" => Some("GPU Driver"),
        "gputemp" => Some("GPU Temperature"),
        "gpumemory" => Some("GPU Memory"),
        "codec" => Some("Codec"),
        "memory" => Some("Memory"),
        "physicalmemory" => Some("Physical Memory"),
        "swap" => Some("Swap"),
        "disk" => Some("Disk"),
        "disktype" => Some("Disk Type"),
        "btrfs" => Some("BTRFS"),
        "localip" => Some("Local IP"),
        "publicip" => Some("Public IP"),
        "dns" => Some("DNS"),
        "wifi" => Some("Wi-Fi"),
        "datetime" => Some("Date & Time"),
        "locale" => Some("Locale"),
        "editor" => Some("Editor"),
        "media" => Some("Media"),
        "sound" => Some("Sound"),
        "users" => Some("Users"),
        "opengl" => Some("OpenGL"),
        "vulkan" => Some("Vulkan"),
        "weather" => Some("Weather"),
        "session" => Some("Session"),
        "ssh" => Some("SSH"),
        "ports" => Some("Ports"),
        "bluetooth" | "bluetoothradio" => Some("Bluetooth"),
        "camera" => Some("Camera"),
        "mouse" => Some("Mouse"),
        "keyboard" => Some("Keyboard"),
        "tpm" => Some("TPM"),
        "physicaldisk" => Some("Physical Disk"),
        "top" => Some("Top Processes"),
        "diskio" => Some("Disk I/O"),
        "netio" => Some("Network I/O"),
        "version" => Some("Version"),
        "initver" => Some("Init Version"),
        "dewmver" => Some("DE/WM Version"),
        "pkgupdates" => Some("Package Updates"),
        "container" => Some("Container"),
        _ => None,
    }
}

fn default_format(ty: &str, detect_version: bool) -> Option<String> {
    let f = match ty {
        "title" => "{user-name-colored}@{host-name-colored}",
        "os" => "{pretty-name} {arch}",
        "kernel" => "{sysname} {release}",
        "host" => "{name}{?version} ({version}){?}",
        "battery" => "{capacity}{?status} [{status}]{?}",
        "wm" if detect_version => {
            "{pretty-name}{?version} {version}{?}{?protocol-name} ({protocol-name}){?}"
        }
        "wm" | "de" => "{pretty-name}{?version} {version}{?}",
        "terminal" => "{pretty-name}",
        "disk" => {
            "{size-used} / {size-total} ({size-percentage}){?filesystem} - {filesystem}{?}"
        }
        "memory" => "{size-used} / {size-total} ({size-percentage})",
        "swap" => "{size-used} / {size-total} ({size-percentage})",
        "publicip" => "{ip}",
        "cpu" | "gpu" => "{name}",
        _ => return None,
    };
    Some(f.to_string())
}

// ── Conditions ───────────────────────────────────────────

fn condition_ok(cond: &Value) -> bool {
    match cond {
        Value::Str(s) => condition_match(s),
        Value::Arr(a) => a.iter().any(|v| v.as_str().is_some_and(condition_match)),
        Value::Obj(_) => {
            if let Some(v) = cond.get("system") {
                if !condition_values(v, std::env::consts::OS) { return false; }
            }
            if let Some(v) = cond.get("arch") {
                if !condition_values(v, std::env::consts::ARCH) { return false; }
            }
            true
        }
        _ => true,
    }
}

fn condition_values(v: &Value, actual: &str) -> bool {
    match v {
        Value::Str(s) => s.eq_ignore_ascii_case(actual),
        Value::Arr(a) => a.iter().any(|x| x.as_str().is_some_and(|s| s.eq_ignore_ascii_case(actual))),
        _ => true,
    }
}

fn condition_match(s: &str) -> bool {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('!') {
        return !condition_match(rest);
    }
    // Unknown condition syntax should not hide a module.
    s.eq_ignore_ascii_case(std::env::consts::OS)
        || s.eq_ignore_ascii_case(std::env::consts::ARCH)
        || (s.eq_ignore_ascii_case("linux") && std::env::consts::OS == "linux")
}

// ── Colours ──────────────────────────────────────────────

/// Fastfetch names map to the 16 base SGR colours; anything else falls back
/// to the normal arcfetch colour parser.
fn ff_color(spec: &str) -> String {
    let lower = spec.trim().to_ascii_lowercase();
    if lower.is_empty() || lower == "default" {
        return "\x1b[39m".into();
    }
    if lower == "reset_" {
        return "\x1b[0m".into();
    }

    let (attr, name) = if let Some(n) = lower.strip_prefix("dim_") {
        ("\x1b[2m", n)
    } else if let Some(n) = lower.strip_prefix("italic_") {
        ("\x1b[3m", n)
    } else if let Some(n) = lower.strip_prefix("underline_") {
        ("\x1b[4m", n)
    } else if let Some(n) = lower.strip_prefix("blink_") {
        ("\x1b[5m", n)
    } else if let Some(n) = lower.strip_prefix("inverse_") {
        ("\x1b[7m", n)
    } else if let Some(n) = lower.strip_prefix("hidden_") {
        ("\x1b[8m", n)
    } else if let Some(n) = lower.strip_prefix("strike_") {
        ("\x1b[9m", n)
    } else {
        ("", lower.as_str())
    };

    let bright = name.starts_with("bright_") || name.starts_with("light_");
    let base = name
        .strip_prefix("bright_")
        .or_else(|| name.strip_prefix("light_"))
        .unwrap_or(name);
    let color = match base {
        "black" => crate::config::ansi_code(if bright { 8 } else { 0 }),
        "red" => crate::config::ansi_code(if bright { 9 } else { 1 }),
        "green" => crate::config::ansi_code(if bright { 10 } else { 2 }),
        "yellow" => crate::config::ansi_code(if bright { 11 } else { 3 }),
        "blue" => crate::config::ansi_code(if bright { 12 } else { 4 }),
        "magenta" => crate::config::ansi_code(if bright { 13 } else { 5 }),
        "cyan" => crate::config::ansi_code(if bright { 14 } else { 6 }),
        "white" => crate::config::ansi_code(if bright { 15 } else { 7 }),
        _ => crate::config::resolve_config_color(name),
    };
    format!("{attr}{color}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_fastfetch_schema_and_rejects_arcfetch_jsonc() {
        let ff = crate::jsonc::parse(
            r#"{ "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json" }"#,
        ).unwrap();
        assert!(is_fastfetch(&ff));

        let arc = crate::jsonc::parse(r#"{ "info_style": "boxed", "modules": ["os"] }"#).unwrap();
        assert!(!is_fastfetch(&arc));

        let no_schema = crate::jsonc::parse(
            r#"{ "display": { "key": { "type": "none" } } }"#,
        ).unwrap();
        assert!(!is_fastfetch(&no_schema));

        let general = crate::jsonc::parse(r#"{ "general": { "detectVersion": false } }"#).unwrap();
        assert!(is_fastfetch(&general));
    }

    #[test]
    fn maps_fastfetch_modules_and_formats() {
        let root = crate::jsonc::parse(r#"{
            "$schema": "https://fastfetch-cli/fastfetch/json_schema.json",
            "logo": { "type": "small", "padding": { "top": 1, "right": 4, "left": 0 } },
            "display": {
                "separator": " -> ",
                "key": { "width": 12, "type": "none", "paddingLeft": 1 },
                "bar": { "width": 20, "char": { "elapsed": "=", "total": "-" } },
                "color": { "keys": "bright_cyan", "output": "default" },
                "duration": { "abbreviation": false }
            },
            "modules": [
                "os",
                { "type": "wm", "key": "WM" },
                { "type": "disk", "keyColor": "red" },
                { "type": "packages" },
                { "type": "custom", "key": "X", "format": "hello" },
                { "type": "player" }
            ]
        }"#).unwrap();

        let mut cfg = Config::default();
        apply(&mut cfg, &root).unwrap();

        assert!(cfg.fastfetch);
        assert!(cfg.show.uptime_long);
        assert_eq!(cfg.logo.name.as_deref(), Some("mini"));
        assert_eq!(cfg.logo.padding_top, 1);
        assert_eq!(cfg.layout.gap, 4);
        assert_eq!(cfg.layout.indent, 0);
        assert_eq!(cfg.display.separator.as_deref(), Some(" -> "));
        assert_eq!(cfg.display.key_width, Some(12));
        assert!(!cfg.display.labels);
        assert_eq!(cfg.display.bar_width, 20);
        assert_eq!(cfg.display.bar_elapsed, "=");
        assert_eq!(cfg.display.bar_total, "-");
        assert!(cfg.display.key_color.as_deref().unwrap().contains("96"));

        let mods = cfg.modules.as_ref().unwrap();
        assert_eq!(mods.iter().map(|m| m.kind.as_str()).collect::<Vec<_>>(),
                   ["os", "de", "disk", "pkgs", "custom"]);
        assert_eq!(mods[1].key.as_deref(), Some("WM"));
        assert_eq!(mods[1].format.as_deref(),
                   Some("{pretty-name}{?version} {version}{?}{?protocol-name} ({protocol-name}){?}"));
        assert_eq!(mods[2].format.as_deref(),
                   Some("{size-used} / {size-total} ({size-percentage}){?filesystem} - {filesystem}{?}"));
        assert!(mods[4].value_format);
        assert_eq!(mods[4].value.as_deref(), Some("hello"));
        assert!(cfg.show.de_wm_ver);
        assert!(cfg.show.os && cfg.show.de_wm && cfg.show.disk && cfg.show.pkgs);
    }

    #[test]
    fn preset_defaults_and_conditional_fields_match_fastfetch() {
        let root = crate::jsonc::parse(r#"{
            "$schema": "https://fastfetch-cli/fastfetch/json_schema.json",
            "modules": [
                "title", "separator", "os", "kernel",
                { "type": "host", "format": "{/2}{-}{/}{2}{?3} {3}{?}" },
                { "type": "battery", "format": "{/4}{-}{/}{4}{?5} [{5}]{?}" },
                { "type": "display" }
            ]
        }"#).unwrap();
        let mut cfg = Config::default();
        apply(&mut cfg, &root).unwrap();

        assert_eq!(cfg.display.separator.as_deref(), Some(": "));
        assert_eq!(cfg.display.key_width, Some(0));
        let mods = cfg.modules.as_ref().unwrap();
        assert_eq!(mods.iter().map(|m| m.kind.as_str()).collect::<Vec<_>>(),
                   ["title", "separator", "os", "kernel", "host", "battery", "monitor"]);
        assert_eq!(mods[0].format.as_deref(),
                   Some("{user-name-colored}@{host-name-colored}"));
        assert!(mods[1].format.as_deref().is_some_and(|s| !s.is_empty()
            && s.chars().all(|c| c == '-')));
        assert_eq!(mods[2].format.as_deref(), Some("{pretty-name} {arch}"));
        assert_eq!(mods[3].format.as_deref(), Some("{sysname} {release}"));
        assert_eq!(mods[4].format.as_deref(),
                   Some("{/2}{-}{/}{2}{?3} {3}{?}"));
        assert_eq!(mods[5].format.as_deref(),
                   Some("{/4}{-}{/}{4}{?5} [{5}]{?}"));
        assert_eq!(mods[6].format, None);
    }

    #[test]
    fn aliases_conditions_and_default_keys_match_fastfetch() {
        let root = crate::jsonc::parse(r#"{
            "$schema": "https://github.com/fastfetch-cli/fastfetch/json_schema.json",
            "modules": [
                "localip",
                { "type": "gpumemory" },
                { "type": "wmtheme" },
                { "type": "host", "key": "Machine" },
                { "type": "host", "condition": { "succeeded": false } },
                { "type": "packages", "combined": true, "disabled": ["appimage", "flatpak"] },
                { "type": "init" },
                "display", "monitor", "bluetooth", "bluetoothradio"
            ]
        }"#).unwrap();
        let mut cfg = Config::default();
        apply(&mut cfg, &root).unwrap();

        let mods = cfg.modules.as_ref().unwrap();
        assert_eq!(mods.iter().map(|m| m.kind.as_str()).collect::<Vec<_>>(),
                   ["localip", "gpumem", "host", "host", "pkgs", "init", "monitor", "bluetooth"]);
        assert_eq!(mods[0].key.as_deref(), Some("Local IP"));
        assert_eq!(mods[1].key.as_deref(), Some("GPU Memory"));
        // Explicit keys win; the duplicate host keeps its condition.
        assert_eq!(mods[2].key.as_deref(), Some("Machine"));
        assert_eq!(mods[3].key.as_deref(), Some("Host"));
        assert_eq!(mods[3].condition_succeeded, Some(false));
        assert!(mods[4].combined);
        assert_eq!(mods[4].disabled, ["appimage", "flatpak"]);
        assert!(!mods[2].combined);
        assert_eq!(mods[5].key.as_deref(), Some("Init System"));
    }

    #[test]
    fn separator_fills_the_title_width() {
        assert_eq!(fill_separator("  ---", 4), "  --");
        assert_eq!(fill_separator("-", 3), "---");
        assert_eq!(fill_separator("", 3), "");
    }

    #[test]
    fn fastfetch_colours_use_sgr_for_base_names() {
        assert_eq!(ff_color("red"), "\x1b[31m");
        assert_eq!(ff_color("light_red"), "\x1b[91m");
        assert_eq!(ff_color("dim_cyan"), "\x1b[2m\x1b[36m");
        assert_eq!(ff_color("default"), "\x1b[39m");
    }
}
