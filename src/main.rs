// arcfetch v0.3 — sub-ms Arch sysinfo · Catppuccin Mocha
// flags: [-h] [--blackhole [--t <secs>]] [--logo arch|ascii|tux|dna|atom|wave]
//        [--accent <color>] [--no-color]
#![allow(dead_code)]

use std::{env, fs, thread};
use std::f64::consts::PI;
use std::io::{BufWriter, Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// ── Catppuccin Mocha ─────────────────────────────────────
const ROSEWATER: &str = "\x1b[38;2;245;224;220m";
const FLAMINGO:  &str = "\x1b[38;2;242;205;205m";
const PINK:      &str = "\x1b[38;2;245;194;231m";
const MAUVE:     &str = "\x1b[38;2;203;166;247m";
const RED:       &str = "\x1b[38;2;243;139;168m";
const MAROON:    &str = "\x1b[38;2;235;160;172m";
const PEACH:     &str = "\x1b[38;2;250;179;135m";
const YELLOW:    &str = "\x1b[38;2;249;226;175m";
const GREEN:     &str = "\x1b[38;2;166;227;161m";
const TEAL:      &str = "\x1b[38;2;148;226;213m";
const SKY:       &str = "\x1b[38;2;137;220;235m";
const SAPPHIRE:  &str = "\x1b[38;2;116;199;236m";
const BLUE:      &str = "\x1b[38;2;137;180;250m";
const LAVENDER:  &str = "\x1b[38;2;180;190;254m";
const TEXT:      &str = "\x1b[38;2;205;214;244m";
const SUBTEXT1:  &str = "\x1b[38;2;186;194;222m";
const OVERLAY0:  &str = "\x1b[38;2;108;112;134m";
const BOLD:      &str = "\x1b[1m";
const RESET:     &str = "\x1b[0m";

// ═══════════════════════════════════════════════════════════
//  LOGOS  (19 lines each)
// ═══════════════════════════════════════════════════════════

// Arch block ─────────────────────────────────────────────
const LOGO_ARCH: [&str; 19] = [
    "                  \u{2584}                  ",
    "                 \u{259f}\u{2588}\u{2599}                 ",
    "                \u{259f}\u{2588}\u{2588}\u{2588}\u{2599}                ",
    "               \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}               ",
    "              \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}              ",
    "             \u{2582}\u{2594}\u{2580}\u{259c}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}             ",
    "            \u{259f}\u{2588}\u{2588}\u{2585}\u{2582}\u{259d}\u{259c}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}            ",
    "           \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}           ",
    "          \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}          ",
    "         \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}         ",
    "        \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}        ",
    "       \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{259b}\u{2580}\u{2580}\u{259c}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}       ",
    "      \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{259b}      \u{259c}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}      ",
    "     \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}        \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}     ",
    "    \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}        \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2586}\u{2585}\u{2584}\u{2583}\u{2582}    ",
    "   \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{259b}        \u{259c}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}   ",
    "  \u{259f}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2580}\u{2580}\u{2580}              \u{2580}\u{2580}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2599}  ",
    " \u{259f}\u{2588}\u{2588}\u{2588}\u{2580}\u{2598}                       \u{259d}\u{2580}\u{2588}\u{2588}\u{2588}\u{2599} ",
    "\u{259f}\u{259b}\u{2580}                               \u{2580}\u{259c}\u{2599}",
];

// Arch dotty ASCII — improved (user-provided) ─────────────
const LOGO_ASCII: [&str; 19] = [
    "                   -`                    ",
    "                  .o+`                   ",
    "                 `ooo/                   ",
    "                `+oooo:                  ",
    "               `+oooooo:                 ",
    "               -+oooooo+:                ",
    "             `/:-:++oooo+:               ",
    "            `/++++/+++++++:              ",
    "           `/++++++++++++++:             ",
    "          `/+++ooooooooooooo/`           ",
    "         ./ooosssso++osssssso+`          ",
    "        .oossssso-````/ossssss+`         ",
    "       -osssssso.      :ssssssso.        ",
    "      :osssssss/        osssso+++.       ",
    "     /ossssssss/        +ssssooo/-       ",
    "   `/ossssso+/:-        -:/+osssso+-     ",
    "  `+sso+:-`                 `.-/+oso:    ",
    " `++:.                           `-/+/   ",
    " .`                                 `/   ",
];

// Tux ─────────────────────────────────────────────────────
const LOGO_TUX: [&str; 19] = [
    "          .--.             ",
    "         |o_o |            ",
    "         |:_/ |            ",
    "        //   \\ \\           ",
    "       (|     | )          ",
    "      /'\\_ _/`\\          ",
    "      \\___)=(___/          ",
    "                           ",
    "   penguin says:           ",
    "  .--------------------.   ",
    "  | just use arch btw  |   ",
    "  '--------------------'   ",
    "             \\             ",
    "              \\            ",
    "               \\           ",
    "                           ",
    "     Linux  2.6 → 6.x      ",
    "    Torvalds · 1991        ",
    "                           ",
];

// DNA double helix ────────────────────────────────────────
const LOGO_DNA: [&str; 19] = [
    "    \u{2502}   A\u{2550}\u{2550}\u{2550}T   \u{2502}     ",
    "     \\          /    ",
    "      \\  G\u{2550}\u{2550}\u{2550}C  /     ",
    "       \\        /    ",
    "        \u{2573}            ",
    "       /        \\    ",
    "      /  T\u{2550}\u{2550}\u{2550}A  \\     ",
    "     /          \\    ",
    "    \u{2502}   C\u{2550}\u{2550}\u{2550}G   \u{2502}     ",
    "     \\          /    ",
    "      \\  A\u{2550}\u{2550}\u{2550}T  /     ",
    "       \\        /    ",
    "        \u{2573}            ",
    "       /        \\    ",
    "      /  G\u{2550}\u{2550}\u{2550}C  \\     ",
    "     /          \\    ",
    "    \u{2502}   T\u{2550}\u{2550}\u{2550}A   \u{2502}     ",
    "   5'           3'   ",
    "   3'           5'   ",
];

// Bohr atom ───────────────────────────────────────────────
const LOGO_ATOM: [&str; 19] = [
    "          \u{00b7}   e   \u{00b7}          ",
    "    \u{00b7}  \u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}  \u{00b7}   ",
    "   e \u{2502}  \u{00b7}     \u{00b7}  \u{2502} e  ",
    "    \u{00b7} \u{2502} \u{00b7} \u{256d}\u{2500}\u{2500}\u{2500}\u{256e} \u{00b7} \u{2502} \u{00b7}  ",
    "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}  \u{2502} Fe\u{2502}  \u{251c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500} ",
    "   e \u{2502} \u{00b7} \u{2570}\u{2500}\u{2500}\u{2500}\u{256f} \u{00b7} \u{2502} e  ",
    "    \u{00b7} \u{2502}  \u{00b7}     \u{00b7}  \u{2502} \u{00b7}  ",
    "    \u{00b7}  \u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}  \u{00b7}   ",
    "          \u{00b7}   e   \u{00b7}          ",
    "                           ",
    "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}  ",
    "   26p  \u{00b7}  30n  \u{00b7}  26e   ",
    "    1s\u{00b2} 2s\u{00b2} 2p\u{2076} 3s\u{00b2} 3p\u{2076}   ",
    "           3d\u{2076} 4s\u{00b2}          ",
    "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}  ",
    "                           ",
    "     Iron  \u{00b7}  Fe  \u{00b7}  26    ",
    "    Period 4  \u{00b7}  Group 8   ",
    "                           ",
];

// Quantum wave \u{03c8}(x) ────────────────────────────────────────
const LOGO_WAVE: [&str; 19] = [
    "  \u{03c8}(x,t) = Ae^i(kx\u{2212}\u{03c9}t)    ",
    "                           ",
    "   \u{2502}       \u{256d}\u{2500}\u{2500}\u{256e}            ",
    "   \u{2502}      \u{2571}    \u{2572}    \u{256d}\u{2500}\u{2500}\u{256e}  ",
    "   \u{2502}  \u{256d}\u{2500}\u{2500}\u{256f}      \u{2572}  \u{2571}    \u{2572} ",
    "   \u{2502} \u{2571}            \u{2572}\u{2571}      \u{2572}",
    "   \u{2502}\u{2571}                      \u{2500}",
    "   \u{256b}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2192}x",
    "   \u{2502}\\                      \u{2500}",
    "   \u{2502} \\            /\\      /",
    "   \u{2502}  \u{2570}\u{2500}\u{2500}\u{256e}      /  \u{2572}  \u{2571} ",
    "   \u{2502}      \u{2572}    \u{2571}    \u{2570}\u{2500}\u{2500}\u{256f}  ",
    "   \u{2502}       \u{2570}\u{2500}\u{2500}\u{256f}            ",
    "                           ",
    "   |\u{03c8}|\u{00b2} = probability density  ",
    "   \u{222b}|\u{03c8}|\u{00b2}dx = 1  (normalised)  ",
    "                           ",
    "   Schr\u{00f6}dinger  \u{00b7}  1926    ",
    "                           ",
];

// E = mc² ─────────────────────────────────────────────────
const LOGO_EMC2: [&str; 19] = [
    "                           ",
    "   \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}          ",
    "   \u{2502}             \u{2502}          ",
    "   \u{2502}   E=mc\u{00b2}   \u{2502}          ",
    "   \u{2502}             \u{2502}          ",
    "   \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}          ",
    "                           ",
    "   E  \u{2192}  energy  (J)      ",
    "   m  \u{2192}  mass    (kg)     ",
    "   c  \u{2192}  3\u{00d7}10\u{2078} m/s        ",
    "                           ",
    "   \u{0394}E = \u{0394}mc\u{00b2}               ",
    "                           ",
    "   \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}   ",
    "   1 kg \u{2248} 9\u{00d7}10\u{00b9}\u{2076} J       ",
    "   \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}   ",
    "                           ",
    "   Einstein  \u{00b7}  1905       ",
    "                           ",
];

// ── Config ───────────────────────────────────────────────
fn config_path() -> String {
    env::var("XDG_CONFIG_HOME")
        .map(|p| format!("{}/arcfetch/accent", p))
        .unwrap_or_else(|_|
            env::var("HOME")
                .map(|h| format!("{}/.config/arcfetch/accent", h))
                .unwrap_or_else(|_| "~/.config/arcfetch/accent".into())
        )
}

fn load_accent(cli: Option<&str>) -> String {
    let cfg = config_path();
    let raw = cli.map(String::from)
        .or_else(|| env::var("ARCFETCH_ACCENT").ok())
        .or_else(|| fs::read_to_string(&cfg).ok())
        .unwrap_or_default();
    let raw = raw.trim();
    if raw.starts_with('#') && raw.len() == 7 {
        let r = u8::from_str_radix(&raw[1..3], 16).unwrap_or(137);
        let g = u8::from_str_radix(&raw[3..5], 16).unwrap_or(180);
        let b = u8::from_str_radix(&raw[5..7], 16).unwrap_or(250);
        return format!("\x1b[38;2;{};{};{}m", r, g, b);
    }
    match raw {
        "rosewater" => ROSEWATER, "flamingo"  => FLAMINGO,
        "pink"      => PINK,      "mauve"     => MAUVE,
        "red"       => RED,       "maroon"    => MAROON,
        "peach"     => PEACH,     "yellow"    => YELLOW,
        "green"     => GREEN,     "teal"      => TEAL,
        "sky"       => SKY,       "sapphire"  => SAPPHIRE,
        "lavender"  => LAVENDER,
        _           => BLUE,
    }.into()
}

// ── Collectors ───────────────────────────────────────────
#[inline] fn slurp(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }

fn get_username() -> String {
    env::var("USER").or_else(|_| env::var("LOGNAME")).unwrap_or_else(|_| "user".into())
}
fn get_hostname() -> String { slurp("/etc/hostname").trim().to_string() }
fn get_os() -> String {
    slurp("/etc/os-release").lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| l[12..].trim_matches('"').to_string())
        .unwrap_or_else(|| "Arch Linux".into())
}
fn get_kernel() -> String {
    slurp("/proc/version").split_whitespace().nth(2).unwrap_or("unknown").to_string()
}
fn get_uptime() -> String {
    let s: u64 = slurp("/proc/uptime").split_ascii_whitespace().next()
        .and_then(|v| v.split('.').next()).and_then(|v| v.parse().ok()).unwrap_or(0);
    match (s/86400, (s%86400)/3600, (s%3600)/60) {
        (0,0,m) => format!("{}m", m),
        (0,h,m) => format!("{}h {}m", h, m),
        (d,h,m) => format!("{}d {}h {}m", d, h, m),
    }
}
fn get_shell() -> String {
    env::var("SHELL").unwrap_or_default().rsplit('/').next().unwrap_or("sh").to_string()
}
fn get_terminal() -> String {
    env::var("TERM_PROGRAM").or_else(|_| env::var("TERM")).unwrap_or_else(|_| "unknown".into())
}
fn get_de_wm() -> String {
    if let Ok(v) = env::var("XDG_CURRENT_DESKTOP")    { return v; }
    if let Ok(v) = env::var("DESKTOP_SESSION")         { return v; }
    if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() { return "Hyprland".into(); }
    if env::var("SWAYSOCK").is_ok()                    { return "Sway".into(); }
    if env::var("WAYLAND_DISPLAY").is_ok()             { return "Wayland".into(); }
    if env::var("DISPLAY").is_ok()                     { return "X11".into(); }
    "TTY".into()
}
fn get_cpu() -> String {
    let raw = slurp("/proc/cpuinfo");
    let name = raw.lines().find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().replace("(R)","").replace("(TM)","")
            .split_ascii_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "Unknown".into());
    let cores = raw.lines().filter(|l| l.starts_with("processor")).count();
    format!("{} ({}x)", name, cores)
}
fn get_gpu() -> String {
    if let Ok(dir) = fs::read_dir("/proc/driver/nvidia/gpus") {
        for e in dir.flatten() {
            if let Ok(info) = fs::read_to_string(e.path().join("information")) {
                if let Some(l) = info.lines().find(|l| l.starts_with("Model:")) {
                    return l[6..].trim().into();
                }
            }
        }
    }
    for i in 0..4u8 {
        let base = format!("/sys/class/drm/card{}/device", i);
        for attr in &["product_name","label"] {
            if let Ok(v) = fs::read_to_string(format!("{}/{}", base, attr)) {
                let v = v.trim().to_string();
                if !v.is_empty() { return v; }
            }
        }
    }
    if let Ok(devs) = fs::read_dir("/sys/bus/pci/devices") {
        for e in devs.flatten() {
            let p = e.path();
            let class = fs::read_to_string(p.join("class")).unwrap_or_default();
            match class.trim() {
                "0x030000"|"0x030200"|"0x030001" => {
                    let vid = fs::read_to_string(p.join("vendor")).unwrap_or_default();
                    let did = fs::read_to_string(p.join("device")).unwrap_or_default();
                    let vname = match vid.trim() {
                        "0x1002" => "AMD", "0x10de" => "NVIDIA",
                        "0x8086" => "Intel", v => return format!("GPU {}", v),
                    };
                    return format!("{} GPU ({})", vname, did.trim());
                }
                _ => {}
            }
        }
    }
    "Unknown".into()
}
fn get_memory() -> (u64, u64) {
    let raw = slurp("/proc/meminfo");
    let (mut tot, mut avl) = (0u64, 0u64);
    for line in raw.lines() {
        if      line.starts_with("MemTotal:")     { tot = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); }
        else if line.starts_with("MemAvailable:") { avl = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); break; }
    }
    (tot.saturating_sub(avl)/1024, tot/1024)
}
fn get_disk() -> String {
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(b"/\0".as_ptr().cast::<libc::c_char>(), &mut st) } == 0 {
        let blk   = st.f_frsize as u64;
        let total = (st.f_blocks as u64) * blk;
        let avail = (st.f_bavail as u64) * blk;
        let gb    = 1_073_741_824.0f64;
        return format!("{:.1}G / {:.1}G",
            total.saturating_sub(avail) as f64/gb, total as f64/gb);
    }
    "unknown".into()
}
fn get_packages() -> String {
    match fs::read_dir("/var/lib/pacman/local") {
        Ok(d)  => format!("{} (pacman)", d.count().saturating_sub(1)),
        Err(_) => "unknown".into(),
    }
}
fn get_resolution() -> String {
    if let Ok(dir) = fs::read_dir("/sys/class/drm") {
        for e in dir.flatten() {
            let n = e.file_name(); let ns = n.to_string_lossy();
            if ns.starts_with("card") && ns.contains('-') {
                if let Ok(m) = fs::read_to_string(e.path().join("modes")) {
                    if let Some(line) = m.lines().next() { return line.to_string(); }
                }
            }
        }
    }
    "N/A".into()
}
fn get_load() -> String {
    slurp("/proc/loadavg").split_ascii_whitespace().take(3).collect::<Vec<_>>().join("  ")
}
fn get_locale() -> String { env::var("LANG").unwrap_or_else(|_| "C".into()) }

// ── Render helpers ───────────────────────────────────────
fn mem_bar(used: u64, total: u64, w: usize, accent: &str) -> String {
    if total == 0 { return String::new(); }
    let fill = ((used as f64 / total as f64 * w as f64) as usize).min(w);
    format!("{a}{f}{OVL}{e}{R}",
        a=accent, f="\u{2588}".repeat(fill),
        OVL=OVERLAY0, e="\u{2591}".repeat(w-fill), R=RESET)
}
fn row(kc: &str, key: &str, val: &str) -> String {
    let mut s = String::with_capacity(120);
    s.push_str(BOLD); s.push_str(kc); s.push_str(key);
    for _ in key.len()..9 { s.push(' '); }
    s.push_str(RESET); s.push(' ');
    s.push_str(SUBTEXT1); s.push_str(val); s.push_str(RESET);
    s
}
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut esc = false;
    for ch in s.chars() {
        if ch == '\x1b'  { esc = true;  continue; }
        if esc { if ch == 'm' { esc = false; } continue; }
        out.push(ch);
    }
    out
}

// ═══════════════════════════════════════════════════════════
//  Black Hole — animated M87 accretion disk
//
//  --t 0   infinite loop (Ctrl+C to exit, cursor restored)
//  --t N   run for N seconds
//  (none)  default 48 frames (~3 s)
//
//  Physics:
//    r < 3.6  → event horizon / void
//    r < 4.9  → photon sphere  (SKY shimmer)
//    r < 10.4 → accretion disk (Doppler: approach=YELLOW, recede=RED)
//    r < 13.2 → corona wisps   (MAUVE/LAVENDER)
// ═══════════════════════════════════════════════════════════

// atomic flag — set by SIGINT handler for graceful infinite-loop exit
static RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn sigint_handler(_: libc::c_int) {
    RUNNING.store(false, Ordering::Relaxed);
}

fn bh_cell(col: usize, row_idx: usize, cx: f64, cy: f64, rot: f64)
    -> (&'static str, char)
{
    let dx   = (col as f64 - cx) * 0.52;
    let dy   = row_idx as f64 - cy;
    let dist = (dx*dx + dy*dy).sqrt();

    if dist < 3.6 { return (RESET, ' '); }

    let ra = ((dy.atan2(dx) + rot) % (2.0*PI) + 2.0*PI) % (2.0*PI);

    if dist < 4.9 {
        let b = (ra * 1.3).sin() * 0.4 + 0.35;
        return (SKY, if b > 0.45 { '\u{2591}' } else { '\u{00b7}' });
    }

    if dist < 10.4 {
        let b = ((ra.sin() * 0.5 + 0.5)
            * (1.0 - (dist - 4.9) / (10.4 - 4.9) * 0.38))
            .clamp(0.0, 1.0);
        let ch = match (b * 4.8) as u8 {
            4|5 => '\u{2588}', 3 => '\u{2593}',
            2   => '\u{2592}', 1 => '\u{2591}', _ => '\u{00b7}',
        };
        let col: &'static str = match (b * 4.0) as u8 {
            3|4           => YELLOW, 2 => PEACH, 1 => RED,
            _ if b > 0.06 => MAROON, _ => OVERLAY0,
        };
        return (col, ch);
    }

    if dist < 13.2 {
        let w = ((ra * 4.0 + dist * 0.85 + rot * 0.25).sin() + 1.0) * 0.5;
        if w > 0.74 { return (MAUVE,    '\u{00b7}'); }
        if w > 0.60 { return (LAVENDER, '\u{00b7}'); }
    }

    (RESET, ' ')
}

// duration: None=48 frames, Some(0)=infinite, Some(n)=n seconds
fn run_blackhole(info: &[String], duration: Option<u64>) {
    const FRAME_MS: u64 = 62;
    const BH_ROWS:  usize = 19;
    const BH_COLS:  usize = 46;
    const CX:       f64   = 23.0;
    const CY:       f64   = 9.0;

    // register SIGINT handler so cursor is always restored
    RUNNING.store(true, Ordering::Relaxed);
    unsafe { libc::signal(libc::SIGINT, sigint_handler as *const () as libc::sighandler_t); }

    let max_frames: Option<u64> = match duration {
        None      => Some(48),
        Some(0)   => None,
        Some(n)   => Some(n * 1000 / FRAME_MS),
    };

    // label for header
    let mode_lbl = match duration {
        None      => format!("~3s"),
        Some(0)   => format!("\u{221e}  (Ctrl+C to exit)"),
        Some(n)   => format!("{}s", n),
    };

    let out     = stdout();
    let mut buf = BufWriter::new(out.lock());

    write!(buf, "\x1b[?25l").ok();
    writeln!(buf).ok();
    writeln!(buf,
        "  {B}{MV}\u{25cf}  arcfetch{R}  {OVL}\u{00b7}\u{00b7}\u{00b7}  {MV}SINGULARITY MODE{R}  {OVL}[{lbl}]{R}",
        B=BOLD, MV=MAUVE, OVL=OVERLAY0, R=RESET, lbl=mode_lbl).ok();
    writeln!(buf, "  {OVL}{sep}{R}",
        OVL=OVERLAY0, sep="\u{2500}".repeat(62), R=RESET).ok();
    buf.flush().ok();

    let mut frame = 0u64;

    loop {
        // stop conditions
        if !RUNNING.load(Ordering::Relaxed) { break; }
        if let Some(max) = max_frames { if frame >= max { break; } }

        let rot = (frame as f64 * PI / 6.0) % (2.0*PI);

        if frame > 0 { write!(buf, "\x1b[{}A", BH_ROWS).ok(); }

        for r in 0..BH_ROWS {
            write!(buf, "  ").ok();
            let mut prev: &str = "";
            for c in 0..BH_COLS {
                let (color, ch) = bh_cell(c, r, CX, CY, rot);
                if color != prev {
                    write!(buf, "{}{}", RESET, color).ok();
                    prev = color;
                }
                write!(buf, "{}", ch).ok();
            }
            write!(buf, "{}  ", RESET).ok();
            if let Some(line) = info.get(r) { write!(buf, "{}", line).ok(); }
            writeln!(buf).ok();
        }

        buf.flush().ok();
        frame += 1;
        thread::sleep(Duration::from_millis(FRAME_MS));
    }

    // restore cursor
    write!(buf, "\x1b[?25h").ok();
    writeln!(buf).ok();
    buf.flush().ok();
}

// ── CLI ──────────────────────────────────────────────────
enum LogoKind { Arch, Ascii, Tux, Dna, Atom, Wave, Emc2 }

struct Args {
    logo:      LogoKind,
    accent:    Option<String>,
    no_color:  bool,
    blackhole: bool,
    duration:  Option<u64>,   // --t flag (only used with --blackhole)
    help:      bool,
    version:   bool,
}

fn parse_args() -> Args {
    let mut a = Args {
        logo: LogoKind::Arch, accent: None,
        no_color: false, blackhole: false,
        duration: None, help: false, version: false,
    };
    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help"    => a.help      = true,
            "-V" | "--version" => a.version   = true,
            "--blackhole"   => a.blackhole  = true,
            "--no-color"    => a.no_color   = true,
            "--logo"  => if let Some(n) = it.next() {
                a.logo = match n.as_str() {
                    "ascii" => LogoKind::Ascii, "tux"  => LogoKind::Tux,
                    "dna"   => LogoKind::Dna,   "atom" => LogoKind::Atom,
                    "wave"  => LogoKind::Wave,  "emc2" => LogoKind::Emc2,
                    _       => LogoKind::Arch,
                };
            },
            "--accent" => { a.accent   = it.next(); }
            "--t"      => { a.duration = it.next().and_then(|v| v.parse().ok()); }
            _          => {}
        }
    }
    a
}

fn print_help(cfg: &str) {
    let (b, r, g, s) = (BOLD, RESET, GREEN, SUBTEXT1);
    println!();
    println!("  {b}{MAUVE}arcfetch{r}  v0.7  blazing-fast Arch sysinfo", b=b, MAUVE=MAUVE, r=r);
    println!();
    println!("  {b}{BLUE}Usage{r}   arcfetch [OPTIONS]", b=b, BLUE=BLUE, r=r);
    println!();
    println!("  {b}{SAPPHIRE}Options{r}", b=b, SAPPHIRE=SAPPHIRE, r=r);
    println!("    {b}{g}-h, --help{r}                   this screen + config path",b=b,g=g,r=r);
    println!("    {b}{g}-V, --version{r}                print version",b=b,g=g,r=r);
    println!("    {b}{g}--blackhole{r}                  {MAUVE}animated M87 black hole{r}",b=b,g=g,r=r,MAUVE=MAUVE);
    println!("    {b}{g}--t <secs>{r}                   {s}0=infinite  N=N seconds{r}",b=b,g=g,r=r,s=s);
    println!("    {b}{g}--logo <n>{r}                   switch logo (see below)",b=b,g=g,r=r);
    println!("    {b}{g}--accent <color>{r}             hex {s}(#RRGGBB){r} or name",b=b,g=g,r=r,s=s);
    println!("    {b}{g}--no-color{r}                   strip all ANSI",b=b,g=g,r=r);
    println!();
    println!("  {b}{TEAL}Logos{r}", b=b, TEAL=TEAL, r=r);
    println!("    {b}{g}arch{r}   {s}block logo (default){r}",b=b,g=g,s=s,r=r);
    println!("    {b}{g}ascii{r}  {s}classic dotty Arch{r}",b=b,g=g,s=s,r=r);
    println!("    {b}{g}tux{r}    {s}Linux Tux penguin{r}",b=b,g=g,s=s,r=r);
    println!("    {b}{g}dna{r}    {s}DNA double helix{r}",b=b,g=g,s=s,r=r);
    println!("    {b}{g}atom{r}   {s}Bohr atom model (Fe){r}",b=b,g=g,s=s,r=r);
    println!("    {b}{g}wave{r}   {s}Schrodinger wave psi(x){r}",b=b,g=g,s=s,r=r);
    println!("    {b}{g}emc2{r}   {s}E = mc² — Einstein 1905{r}",b=b,g=g,s=s,r=r);
    println!();
    println!("  {b}{YELLOW}Accent names{r}", b=b, YELLOW=YELLOW, r=r);
    println!("    {RW}rosewater{r}  {FL}flamingo{r}  {PK}pink{r}  {MV}mauve{r}  {RD}red{r}  {MR}maroon{r}",
        RW=ROSEWATER,FL=FLAMINGO,PK=PINK,MV=MAUVE,RD=RED,MR=MAROON,r=r);
    println!("    {PE}peach{r}  {YW}yellow{r}  {GN}green{r}  {TL}teal{r}  {SK}sky{r}  {SP}sapphire{r}  {BL}blue{r} {s}(default){r}  {LV}lavender{r}",
        PE=PEACH,YW=YELLOW,GN=GREEN,TL=TEAL,SK=SKY,SP=SAPPHIRE,
        BL=BLUE,LV=LAVENDER,s=s,r=r);
    println!();
    println!("  {b}{PEACH}Config{r}  {s}{cfg}{r}", b=b, PEACH=PEACH, s=s, cfg=cfg, r=r);
    println!("    mkdir -p $(dirname {cfg})", cfg=cfg);
    println!("    echo \"mauve\" > {cfg}", cfg=cfg);
    println!();
    println!("  {b}{SKY}Examples{r}", b=b, SKY=SKY, r=r);
    println!("    arcfetch");
    println!("    arcfetch --blackhole --t 0          {s}# infinite loop{r}",s=s,r=r);
    println!("    arcfetch --blackhole --t 30         {s}# 30 seconds{r}",s=s,r=r);
    println!("    arcfetch --logo dna --accent mauve");
    println!("    arcfetch --logo wave --accent '#CBA6F7'");
    println!("    arcfetch --no-color | tee sysinfo.txt");
    println!();
}

// ── main ─────────────────────────────────────────────────
fn main() {
    let args = parse_args();
    let cfg  = config_path();
    if args.version {
        println!();
        println!("  {B}{MAUVE}arcfetch{R}", B=BOLD, MAUVE=MAUVE, R=RESET);
        println!("  {OVL}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{R}", OVL=OVERLAY0, R=RESET);
        println!("  {S}version  {B}{BLUE}0.7.0{R}", S=SUBTEXT1, B=BOLD, BLUE=BLUE, R=RESET);
        println!("  {S}theme    {MAUVE}Catppuccin Mocha{R}", S=SUBTEXT1, MAUVE=MAUVE, R=RESET);
        println!();
        println!("  {Y}E {R}= {G}m{R}{S}c{R}\u{00b2}   {OVL}where E = startup time \u{2248} 0{R}",
            Y=YELLOW, R=RESET, G=GREEN, S=SUBTEXT1, OVL=OVERLAY0);
        println!();
        return;
    }
    if args.help { print_help(&cfg); return; }

    let accent = load_accent(args.accent.as_deref());
    let acc    = accent.as_str();
    let user   = get_username();
    let host   = get_hostname();

    let (os,kern,upt,res,pkgs,sh,wm,term,cpu,gpu,disk,load,loc,(mu,mt)) =
        thread::scope(|s| {
            let t_os   = s.spawn(get_os);
            let t_kern = s.spawn(get_kernel);
            let t_upt  = s.spawn(get_uptime);
            let t_res  = s.spawn(get_resolution);
            let t_pkgs = s.spawn(get_packages);
            let t_sh   = s.spawn(get_shell);
            let t_wm   = s.spawn(get_de_wm);
            let t_term = s.spawn(get_terminal);
            let t_cpu  = s.spawn(get_cpu);
            let t_gpu  = s.spawn(get_gpu);
            let t_disk = s.spawn(get_disk);
            let t_load = s.spawn(get_load);
            let t_loc  = s.spawn(get_locale);
            let t_mem  = s.spawn(get_memory);
            (
                t_os.join().unwrap_or_else(  |_| "?".into()),
                t_kern.join().unwrap_or_else(|_| "?".into()),
                t_upt.join().unwrap_or_else( |_| "?".into()),
                t_res.join().unwrap_or_else( |_| "?".into()),
                t_pkgs.join().unwrap_or_else(|_| "?".into()),
                t_sh.join().unwrap_or_else(  |_| "?".into()),
                t_wm.join().unwrap_or_else(  |_| "?".into()),
                t_term.join().unwrap_or_else(|_| "?".into()),
                t_cpu.join().unwrap_or_else( |_| "?".into()),
                t_gpu.join().unwrap_or_else( |_| "?".into()),
                t_disk.join().unwrap_or_else(|_| "?".into()),
                t_load.join().unwrap_or_else(|_| "?".into()),
                t_loc.join().unwrap_or_else( |_| "?".into()),
                t_mem.join().unwrap_or((0,0)),
            )
        });

    let bar = mem_bar(mu, mt, 14, acc);
    let mem_str = if mt >= 1024 {
        format!("{bar}{TEXT}  {u:.1}G / {t:.1}G{R}",
            bar=bar, TEXT=TEXT, u=mu as f64/1024.0, t=mt as f64/1024.0, R=RESET)
    } else {
        format!("{bar}{TEXT}  {u}M / {t}M{R}", bar=bar, TEXT=TEXT, u=mu, t=mt, R=RESET)
    };

    let pw  = user.len() + 1 + host.len();
    let uh  = format!("{B}{MAUVE}{u}{OVL}@{R}{B}{acc}{h}{R}",
        B=BOLD, MAUVE=MAUVE, u=user, OVL=OVERLAY0, R=RESET, acc=acc, h=host);
    let sep = format!("{OVL}{l}{R}", OVL=OVERLAY0, l="\u{2500}".repeat(pw), R=RESET);

    let info: [String; 19] = [
        uh,
        sep,
        row(BLUE,      "os",      &os),
        row(SAPPHIRE,  "kernel",  &kern),
        row(SKY,       "uptime",  &upt),
        row(TEAL,      "res",     &res),
        row(GREEN,     "pkgs",    &pkgs),
        row(YELLOW,    "shell",   &sh),
        row(PEACH,     "de/wm",   &wm),
        row(MAROON,    "term",    &term),
        row(RED,       "cpu",     &cpu),
        row(PINK,      "gpu",     &gpu),
        {
            let mut s = String::with_capacity(140);
            s.push_str(BOLD); s.push_str(FLAMINGO); s.push_str("memory");
            s.push_str("   "); s.push_str(RESET); s.push_str(&mem_str);
            s
        },
        row(MAUVE,     "disk",    &disk),
        row(LAVENDER,  "load",    &load),
        row(ROSEWATER, "locale",  &loc),
        String::new(),
        format!(
            "{rw}\u{25cf} {fl}\u{25cf} {pk}\u{25cf} {mv}\u{25cf} {rd}\u{25cf} {mr}\u{25cf} {pe}\u{25cf} {yw}\u{25cf} {gn}\u{25cf} {tl}\u{25cf} {sk}\u{25cf} {sp}\u{25cf} {bl}\u{25cf} {lv}\u{25cf}{R}",
            rw=ROSEWATER, fl=FLAMINGO, pk=PINK,    mv=MAUVE,
            rd=RED,       mr=MAROON,   pe=PEACH,   yw=YELLOW,
            gn=GREEN,     tl=TEAL,     sk=SKY,     sp=SAPPHIRE,
            bl=BLUE,      lv=LAVENDER, R=RESET
        ),
        String::new(),
    ];

    let out_lines: Vec<String> = if args.no_color {
        info.iter().map(|s| strip_ansi(s)).collect()
    } else {
        info.iter().map(String::clone).collect()
    };

    if args.blackhole { run_blackhole(&out_lines, args.duration); return; }

    let logo: &[&str] = match args.logo {
        LogoKind::Arch  => &LOGO_ARCH,
        LogoKind::Ascii => &LOGO_ASCII,
        LogoKind::Tux   => &LOGO_TUX,
        LogoKind::Dna   => &LOGO_DNA,
        LogoKind::Atom  => &LOGO_ATOM,
        LogoKind::Wave  => &LOGO_WAVE,
        LogoKind::Emc2  => &LOGO_EMC2,
    };

    let logo_w = logo.iter().map(|l| l.chars().count()).max().unwrap_or(36);
    let out    = stdout();
    let mut buf = BufWriter::new(out.lock());
    writeln!(buf).ok();

    for (i, ll) in logo.iter().enumerate() {
        let pad   = logo_w.saturating_sub(ll.chars().count());
        let iline = out_lines.get(i).map(String::as_str).unwrap_or("");
        if args.no_color {
            writeln!(buf, "  {}{}  {}", ll, " ".repeat(pad), iline).ok();
        } else {
            writeln!(buf, "  {acc}{ll}{pad}{R}  {i}",
                acc=acc, ll=ll, pad=" ".repeat(pad), R=RESET, i=iline).ok();
        }
    }
    writeln!(buf).ok();
}
