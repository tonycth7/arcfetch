// src/info.rs — hybrid: one thread for slow real-FS reads, rest sequential
use std::{env, fs, thread};

pub struct SysInfo {
    pub os:          String,
    pub kernel:      String,
    pub uptime_secs: u64,     // raw seconds — formatted in build_info per config
    pub uptime:      String,  // pre-formatted short form
    pub res:         String,
    pub pkgs:        String,
    pub shell:       String,
    pub de_wm:       String,
    pub term:        String,
    pub cpu:         String,
    pub gpu:         String,
    pub gpu_temp:    String,
    pub battery:     String,
    pub disk:        String,
    pub load:        String,
    pub locale:      String,
    pub mem_used:    u64,
    pub mem_total:   u64,
    // hacker fields
    pub ip:          String,
    pub ssh:         String,
    pub ports:       String,
}

#[inline] fn slurp(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }

/// Collect all system info.
/// Pass need_network=true only for hacker preset — those /proc/net reads add ~5ms.
#[inline]
pub fn collect_all(need_network: bool) -> SysInfo {
    // ── spawn threads ONLY for real-FS collectors ─────────
    // packages: 686 dirs on real WSL disk  (~15ms)
    let t_pkgs = thread::spawn(|| {
        match fs::read_dir("/var/lib/pacman/local") {
            Ok(d)  => format!("{} (pacman)", d.count().saturating_sub(1)),
            Err(_) => "unknown".into(),
        }
    });

    // gpu: may walk /sys/bus/pci/devices (many entries)
    let t_gpu = thread::spawn(|| {
        // NVIDIA
        if let Ok(dir) = fs::read_dir("/proc/driver/nvidia/gpus") {
            for e in dir.flatten() {
                if let Ok(info) = fs::read_to_string(e.path().join("information")) {
                    if let Some(l) = info.lines().find(|l| l.starts_with("Model:")) {
                        return l[6..].trim().to_string();
                    }
                }
            }
        }
        // DRM sysfs (usually 1-4 entries, fast)
        for i in 0..4u8 {
            let base = format!("/sys/class/drm/card{}/device", i);
            for attr in &["product_name", "label"] {
                if let Ok(v) = fs::read_to_string(format!("{}/{}", base, attr)) {
                    let v = v.trim().to_string();
                    if !v.is_empty() { return v; }
                }
            }
        }
        // PCI fallback (slow on WSL — many entries)
        if let Ok(devs) = fs::read_dir("/sys/bus/pci/devices") {
            for e in devs.flatten() {
                let p = e.path();
                let class = fs::read_to_string(p.join("class")).unwrap_or_default();
                match class.trim() {
                    "0x030000" | "0x030200" | "0x030001" => {
                        let vid = fs::read_to_string(p.join("vendor")).unwrap_or_default();
                        let did = fs::read_to_string(p.join("device")).unwrap_or_default();
                        return match vid.trim() {
                            "0x1002" => format!("AMD GPU ({})", did.trim()),
                            "0x10de" => format!("NVIDIA GPU ({})", did.trim()),
                            "0x8086" => format!("Intel GPU ({})", did.trim()),
                            v        => format!("GPU {}", v),
                        };
                    }
                    _ => {}
                }
            }
        }
        "Unknown".into()
    });

    // ── everything else: kernel virtual FS or env → sequential ──

    // /proc/meminfo — single pass
    let (mem_used, mem_total) = {
        let raw = slurp("/proc/meminfo");
        let (mut tot, mut avl) = (0u64, 0u64);
        for line in raw.lines() {
            if      line.starts_with("MemTotal:")     { tot = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); }
            else if line.starts_with("MemAvailable:") { avl = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); break; }
        }
        (tot.saturating_sub(avl) / 1024, tot / 1024)
    };

    // /proc/cpuinfo — single pass for name + core count
    let cpu = {
        let raw = slurp("/proc/cpuinfo");
        let name = raw.lines().find(|l| l.starts_with("model name"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim().replace("(R)", "").replace("(TM)", "")
                .split_ascii_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| "Unknown".into());
        let cores = raw.lines().filter(|l| l.starts_with("processor")).count();
        format!("{} ({}x)", name, cores)
    };

    let os = slurp("/etc/os-release").lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| l[12..].trim_matches('"').to_string())
        .unwrap_or_else(|| "Arch Linux".into());

    let kernel = slurp("/proc/version")
        .split_whitespace().nth(2).unwrap_or("unknown").to_string();

    // uptime — keep raw secs too for long-format rendering
    let uptime_secs: u64 = slurp("/proc/uptime").split_ascii_whitespace().next()
        .and_then(|v| v.split('.').next()).and_then(|v| v.parse().ok()).unwrap_or(0);
    let uptime = {
        let (d, h, m) = (uptime_secs/86400, (uptime_secs%86400)/3600, (uptime_secs%3600)/60);
        match (d, h, m) {
            (0, 0, m) => format!("{}m", m),
            (0, h, m) => format!("{}h {}m", h, m),
            (d, h, m) => format!("{}d {}h {}m", d, h, m),
        }
    };

    // gpu temp — walk /sys/class/drm/card*/device/hwmon/hwmon*/temp1_input
    let gpu_temp = {
        let mut temp = String::new();
        'outer: for i in 0..4u8 {
            let hwmon_base = format!("/sys/class/drm/card{}/device/hwmon", i);
            if let Ok(hwmons) = fs::read_dir(&hwmon_base) {
                for hw in hwmons.flatten() {
                    // temp1_input is millidegrees Celsius
                    if let Ok(raw) = fs::read_to_string(hw.path().join("temp1_input")) {
                        if let Ok(mc) = raw.trim().parse::<u32>() {
                            temp = format!("{}°C", mc / 1000);
                            break 'outer;
                        }
                    }
                }
            }
        }
        if temp.is_empty() { "N/A".into() } else { temp }
    };

    // battery — /sys/class/power_supply/BAT0 (or BAT1)
    let battery = {
        let mut b = String::new();
        for bat in &["BAT0", "BAT1", "BAT"] {
            let base = format!("/sys/class/power_supply/{}", bat);
            if let Ok(cap) = fs::read_to_string(format!("{}/capacity", base)) {
                let cap = cap.trim();
                let status = fs::read_to_string(format!("{}/status", base))
                    .unwrap_or_default();
                let status = status.trim();
                let icon = match status {
                    "Charging"    => "↑",
                    "Discharging" => "↓",
                    "Full"        => "✓",
                    _             => "",
                };
                b = format!("{}% {}", cap, icon).trim().to_string();
                break;
            }
        }
        if b.is_empty() { "N/A".into() } else { b }
    };

    let load = slurp("/proc/loadavg")
        .split_ascii_whitespace().take(3).collect::<Vec<_>>().join("  ");

    // disk — one statvfs syscall
    let disk = {
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(b"/\0".as_ptr().cast::<libc::c_char>(), &mut st) } == 0 {
            let blk   = st.f_frsize as u64;
            let total = st.f_blocks as u64 * blk;
            let avail = st.f_bavail as u64 * blk;
            let gb    = 1_073_741_824.0f64;
            format!("{:.1}G / {:.1}G",
                total.saturating_sub(avail) as f64 / gb, total as f64 / gb)
        } else { "unknown".into() }
    };

    // resolution — sysfs, small dir
    let res = {
        let mut r = "N/A".to_string();
        if let Ok(dir) = fs::read_dir("/sys/class/drm") {
            for e in dir.flatten() {
                let n = e.file_name(); let ns = n.to_string_lossy();
                if ns.starts_with("card") && ns.contains('-') {
                    if let Ok(m) = fs::read_to_string(e.path().join("modes")) {
                        if let Some(line) = m.lines().next() { r = line.to_string(); break; }
                    }
                }
            }
        }
        r
    };

    // env vars — pure memory, zero I/O
    let shell  = env::var("SHELL").unwrap_or_default().rsplit('/').next().unwrap_or("sh").to_string();
    let term   = env::var("TERM_PROGRAM").or_else(|_| env::var("TERM")).unwrap_or_else(|_| "unknown".into());
    let locale = env::var("LANG").unwrap_or_else(|_| "C".into());
    let de_wm  = {
             if let Ok(v) = env::var("XDG_CURRENT_DESKTOP")         { v }
        else if let Ok(v) = env::var("DESKTOP_SESSION")             { v }
        else if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()     { "Hyprland".into() }
        else if env::var("SWAYSOCK").is_ok()                        { "Sway".into() }
        else if env::var("WAYLAND_DISPLAY").is_ok()                 { "Wayland".into() }
        else if env::var("DISPLAY").is_ok()                         { "X11".into() }
        else                                                        { "TTY".into() }
    };

    // ── network: only collected for hacker preset ─────────
    let (ip, ssh, ports) = if need_network {
        // local IP from /proc/net/fib_trie (no subprocess)
        let ip = {
            let mut found = String::new();
            if let Ok(raw) = fs::read_to_string("/proc/net/fib_trie") {
                let mut local = false;
                for line in raw.lines() {
                    let t = line.trim();
                    if t.ends_with("LOCAL") { local = true; continue; }
                    if local && t.starts_with("32 host") { local = false; continue; }
                    if local {
                        if let Some(ip_part) = t.strip_prefix("|-- ").or_else(|| t.strip_prefix("+-- ")) {
                            let ip_str = ip_part.trim();
                            if ip_str != "127.0.0.1" && !ip_str.starts_with("127.")
                                && ip_str != "0.0.0.0" && ip_str.contains('.') {
                                found = ip_str.to_string();
                                break;
                            }
                        }
                    }
                }
            }
            if found.is_empty() { "N/A".into() } else { found }
        };
        // SSH: port 22 (0x0016) in LISTEN (0A) in /proc/net/tcp
        let ssh = {
            let tcp = fs::read_to_string("/proc/net/tcp").unwrap_or_default();
            let up  = tcp.lines().skip(1).any(|l| {
                let c: Vec<&str> = l.split_whitespace().collect();
                c.get(1).map(|a| a.ends_with(":0016")).unwrap_or(false)
                    && c.get(3).map(|s| *s == "0A").unwrap_or(false)
            });
            if up { "running (port 22)".into() } else { "not running".into() }
        };
        // open TCP ports from /proc/net/tcp + tcp6
        let ports = {
            let mut open: Vec<u16> = Vec::new();
            for path in &["/proc/net/tcp", "/proc/net/tcp6"] {
                if let Ok(raw) = fs::read_to_string(path) {
                    for line in raw.lines().skip(1) {
                        let cols: Vec<&str> = line.split_whitespace().collect();
                        if cols.get(3).map(|s| *s == "0A").unwrap_or(false) {
                            if let Some(local) = cols.get(1) {
                                if let Some(hex) = local.split(':').nth(1) {
                                    if let Ok(port) = u16::from_str_radix(hex, 16) {
                                        if !open.contains(&port) { open.push(port); }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            open.sort_unstable();
            open.truncate(8);
            if open.is_empty() { "none".into() }
            else { open.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ") }
        };
        (ip, ssh, ports)
    } else {
        (String::new(), String::new(), String::new())
    };

    // ── join threads — by now they've had time to finish ──
    let pkgs = t_pkgs.join().unwrap_or_else(|_| "unknown".into());
    let gpu  = t_gpu.join().unwrap_or_else(|_| "Unknown".into());

    SysInfo { os, kernel, uptime_secs, uptime, res, pkgs, shell, de_wm, term,
              cpu, gpu, gpu_temp, battery, disk, load, locale, mem_used, mem_total,
              ip, ssh, ports }
}
