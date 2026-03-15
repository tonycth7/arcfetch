// src/info.rs — system info collectors, parallel via rayon
use std::{env, fs};
use rayon::prelude::*;

pub struct SysInfo {
    pub os:        String,
    pub kernel:    String,
    pub uptime:    String,
    pub res:       String,
    pub pkgs:      String,
    pub shell:     String,
    pub de_wm:     String,
    pub term:      String,
    pub cpu:       String,
    pub gpu:       String,
    pub disk:      String,
    pub load:      String,
    pub locale:    String,
    pub mem_used:  u64,
    pub mem_total: u64,
}

#[inline]
fn slurp(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }

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
    match (s / 86400, (s % 86400) / 3600, (s % 3600) / 60) {
        (0, 0, m) => format!("{}m", m),
        (0, h, m) => format!("{}h {}m", h, m),
        (d, h, m) => format!("{}d {}h {}m", d, h, m),
    }
}

fn get_resolution() -> String {
    if let Ok(dir) = fs::read_dir("/sys/class/drm") {
        for e in dir.flatten() {
            let n = e.file_name();
            let ns = n.to_string_lossy();
            if ns.starts_with("card") && ns.contains('-') {
                if let Ok(m) = fs::read_to_string(e.path().join("modes")) {
                    if let Some(line) = m.lines().next() { return line.to_string(); }
                }
            }
        }
    }
    "N/A".into()
}

fn get_packages() -> String {
    match fs::read_dir("/var/lib/pacman/local") {
        Ok(d)  => format!("{} (pacman)", d.count().saturating_sub(1)),
        Err(_) => "unknown".into(),
    }
}

fn get_shell() -> String {
    env::var("SHELL").unwrap_or_default().rsplit('/').next().unwrap_or("sh").to_string()
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

fn get_terminal() -> String {
    env::var("TERM_PROGRAM").or_else(|_| env::var("TERM")).unwrap_or_else(|_| "unknown".into())
}

fn get_cpu() -> String {
    let raw = slurp("/proc/cpuinfo");
    let name = raw.lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().replace("(R)", "").replace("(TM)", "")
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
        for attr in &["product_name", "label"] {
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
                "0x030000" | "0x030200" | "0x030001" => {
                    let vid = fs::read_to_string(p.join("vendor")).unwrap_or_default();
                    let did = fs::read_to_string(p.join("device")).unwrap_or_default();
                    let v = match vid.trim() {
                        "0x1002" => "AMD", "0x10de" => "NVIDIA",
                        "0x8086" => "Intel", v => return format!("GPU {}", v),
                    };
                    return format!("{} GPU ({})", v, did.trim());
                }
                _ => {}
            }
        }
    }
    "Unknown".into()
}

fn get_disk() -> String {
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(b"/\0".as_ptr().cast::<libc::c_char>(), &mut st) } == 0 {
        let blk   = st.f_frsize as u64;
        let total = st.f_blocks as u64 * blk;
        let avail = st.f_bavail as u64 * blk;
        let gb    = 1_073_741_824.0f64;
        return format!("{:.1}G / {:.1}G",
            total.saturating_sub(avail) as f64 / gb, total as f64 / gb);
    }
    "unknown".into()
}

fn get_load() -> String {
    slurp("/proc/loadavg").split_ascii_whitespace().take(3).collect::<Vec<_>>().join("  ")
}

fn get_locale() -> String {
    env::var("LANG").unwrap_or_else(|_| "C".into())
}

fn get_memory() -> (u64, u64) {
    let raw = slurp("/proc/meminfo");
    let (mut tot, mut avl) = (0u64, 0u64);
    for line in raw.lines() {
        if line.starts_with("MemTotal:") {
            tot = line.split_ascii_whitespace().nth(1)
                .and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            avl = line.split_ascii_whitespace().nth(1)
                .and_then(|v| v.parse().ok()).unwrap_or(0);
            break;
        }
    }
    (tot.saturating_sub(avl) / 1024, tot / 1024)
}

/// Collect all system info in parallel using rayon.
/// String collectors run as a par_iter pool; memory runs alongside via rayon::join.
pub fn collect_all() -> SysInfo {
    type StrFn = fn() -> String;

    let fns: [StrFn; 13] = [
        get_os, get_kernel, get_uptime, get_resolution, get_packages,
        get_shell, get_de_wm, get_terminal, get_cpu, get_gpu,
        get_disk, get_load, get_locale,
    ];

    // rayon::join runs par_iter pool AND get_memory() on the thread pool simultaneously
    let (strings, mem) = rayon::join(
        || fns.par_iter().map(|f| f()).collect::<Vec<_>>(),
        get_memory,
    );

    let [os, kernel, uptime, res, pkgs, shell, de_wm, term, cpu, gpu, disk, load, locale]:
        [String; 13] = strings.try_into().expect("collector count mismatch");

    SysInfo {
        os, kernel, uptime, res, pkgs, shell, de_wm, term,
        cpu, gpu, disk, load, locale,
        mem_used:  mem.0,
        mem_total: mem.1,
    }
}
