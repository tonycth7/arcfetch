use std::{env, fs};

pub struct SysInfo {
    pub os:          String,
    pub kernel:      String,
    pub uptime_secs: u64,
    pub uptime:      String,
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
    pub ip:          String,
    pub ssh:         String,
    pub ports:       String,
    pub init:        String,
    pub cpu_temp:    String,
    pub processes:   String,
    pub container:   String,
    pub session:     String,
    // fastfetch-inspired fields
    pub swap:        String,
    pub sound:       String,
    pub gpu_driver:  String,
    pub bios:        String,
    pub board:       String,
    pub disk_type:   String,
    pub pkg_updates: String,
    pub theme:       String,
    pub icons:       String,
    pub term_font:   String,
    pub de_wm_ver:   String,
    pub init_ver:    String,
    pub local_ip:    String,
}

// ── Low-level helpers ────────────────────────────────────

#[inline]
fn slurp(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }

/// CPU brand string via CPUID (~1µs) — just the name, no frequency/cores
#[inline]
fn cpu_brand() -> String {
    let max_ext = core::arch::x86_64::__cpuid_count(0x80000000, 0).eax;
    if max_ext >= 0x80000004 {
        let mut buf = [0u8; 48];
        for (i, leaf) in [0x80000002u32, 0x80000003, 0x80000004].iter().copied().enumerate() {
            let res = core::arch::x86_64::__cpuid_count(leaf, 0);
            let off = i * 16;
            buf[off..off+4].copy_from_slice(&res.eax.to_ne_bytes());
            buf[off+4..off+8].copy_from_slice(&res.ebx.to_ne_bytes());
            buf[off+8..off+12].copy_from_slice(&res.ecx.to_ne_bytes());
            buf[off+12..off+16].copy_from_slice(&res.edx.to_ne_bytes());
        }
        let s = std::str::from_utf8(&buf).unwrap_or("Unknown");
        let mut s: String = s.trim().trim_end_matches('\0').trim()
            .replace("(R)", "").replace("(TM)", "")
            .split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        // strip verbose suffixes like bfetch does
        for kw in &["with Radeon Graphics", "with Graphics", "-Core", " Core"] {
            if let Some(pos) = s.find(kw) {
                s.truncate(pos);
            }
        }
        s.trim().to_string()
    } else {
        "Unknown".into()
    }
}

/// GPU probe — DRM first (AMD/Intel), then NVIDIA, then PCI fallback.
fn gpu_probe() -> String {
    // 1. DRM sysfs — covers AMD, Intel, and nouveau
    for i in 0..4u8 {
        let base = format!("/sys/class/drm/card{}/device", i);
        for attr in &["product_name", "label"] {
            if let Ok(v) = fs::read_to_string(format!("{}/{}", base, attr)) {
                let v = v.trim().to_string();
                if !v.is_empty() { return v; }
            }
        }
    }
    // 2. NVIDIA proprietary driver
    if let Ok(dir) = fs::read_dir("/proc/driver/nvidia/gpus") {
        for e in dir.flatten() {
            if let Ok(info) = fs::read_to_string(e.path().join("information")) {
                if let Some(l) = info.lines().find(|l| l.starts_with("Model:")) {
                    return l[6..].trim().to_string();
                }
            }
        }
    }
    // 3. PCI fallback (slow — only when DRM/NVIDIA fail)
    if let Ok(devs) = fs::read_dir("/sys/bus/pci/devices") {
        for e in devs.flatten() {
            let p = e.path();
            let class = fs::read_to_string(p.join("class")).unwrap_or_default();
            match class.trim() {
                "0x030000" | "0x030200" | "0x030001" => {}
                _ => continue,
            }
            let vid = fs::read_to_string(p.join("vendor")).unwrap_or_default();
            let did = fs::read_to_string(p.join("device")).unwrap_or_default();
            return match vid.trim() {
                "0x1002" => format!("AMD GPU ({})", did.trim()),
                "0x10de" => format!("NVIDIA GPU ({})", did.trim()),
                "0x8086" => format!("Intel GPU ({})", did.trim()),
                v        => format!("GPU {}", v),
            };
        }
    }
    "Unknown".into()
}

/// Collect all system info.
/// Only reads fields whose show-flag is true. Pass need_network=true for hacker preset.
#[inline]
pub fn collect_all(need_network: bool, show: &crate::config::Show) -> SysInfo {
    // ── fast sequential: all reads on main thread ──────────

    // /proc/meminfo — single pass (gated on show.memory || show.swap)
    let (mem_used, mem_total) = if show.memory || show.swap {
        let raw = slurp("/proc/meminfo");
        let (mut tot, mut avl) = (0u64, 0u64);
        for line in raw.lines() {
            if      line.starts_with("MemTotal:")     { tot = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); }
            else if line.starts_with("MemAvailable:") { avl = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); break; }
        }
        (tot.saturating_sub(avl) / 1024, tot / 1024)
    } else { (0, 0) };

    // CPUID + sysconf (no I/O) + frequency (cached) — gated on show.cpu
    let cpu = if show.cpu {
        let brand = cpu_brand();
        let cores = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as usize };
        let freq = crate::cache::get_or_compute("cpu_freq", 86400, || {
            slurp("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        });
        let ghz = freq.trim().parse::<f64>().ok().map(|khz| khz / 1_000_000.0);
        if let Some(ghz) = ghz {
            if ghz > 0.1 {
                format!("{} ({}x) @ {:.2} GHz", brand, cores, ghz)
            } else {
                format!("{} ({}x)", brand, cores)
            }
        } else {
            format!("{} ({}x)", brand, cores)
        }
    } else { String::new() };

    let os = if show.os {
        crate::cache::get_or_compute("os", 86400, || {
            slurp("/etc/os-release").lines()
                .find(|l| l.starts_with("PRETTY_NAME="))
                .map(|l| l[12..].trim_matches('"').to_string())
                .unwrap_or_else(|| "Arch Linux".into())
        })
    } else { String::new() };

    let kernel = if show.kernel {
        let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
        if unsafe { libc::uname(&mut uts) } == 0 {
            let rel = unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()) };
            rel.to_string_lossy().to_string()
        } else { "unknown".into() }
    } else { String::new() };

    // uptime via sysinfo (1 syscall, no file I/O)
    let (uptime_secs, uptime) = if show.uptime {
        let secs: u64 = {
            let mut si: libc::sysinfo = unsafe { std::mem::zeroed() };
            if unsafe { libc::sysinfo(&mut si) } == 0 {
                si.uptime as u64
            } else { 0 }
        };
        let (d, h, m) = (secs/86400, (secs%86400)/3600, (secs%3600)/60);
        let short = match (d, h, m) {
            (0, 0, m) => format!("{}m", m),
            (0, h, m) => format!("{}h {}m", h, m),
            (d, h, m) => format!("{}d {}h {}m", d, h, m),
        };
        (secs, short)
    } else { (0, String::new()) };

    // ── conditional reads (gated by show-flags) ───────────

    // gpu temp — walk /sys/class/drm/card*/device/hwmon/hwmon*/temp1_input
    let gpu_temp = if show.gpu_temp {
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
    } else { String::new() };

    // battery — /sys/class/power_supply/BAT0 (or BAT1)
    let battery = if show.battery {
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
    } else { String::new() };

    let load = if show.load {
        slurp("/proc/loadavg")
            .split_ascii_whitespace().take(3).collect::<Vec<_>>().join("  ")
    } else { String::new() };

    // disk — one statvfs syscall
    let disk = if show.disk {
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(b"/\0".as_ptr().cast::<libc::c_char>(), &mut st) } == 0 {
            let blk   = st.f_frsize as u64;
            let total = st.f_blocks as u64 * blk;
            let avail = st.f_bavail as u64 * blk;
            let gb    = 1_073_741_824.0f64;
            format!("{:.1}G / {:.1}G",
                total.saturating_sub(avail) as f64 / gb, total as f64 / gb)
        } else { "unknown".into() }
    } else { String::new() };

    // resolution — sysfs, small dir
    let res = if show.res {
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
    } else { String::new() };

    // env vars — pure memory, zero I/O — gated on show flags
    let shell  = if show.shell  { env::var("SHELL").unwrap_or_default().rsplit('/').next().unwrap_or("sh").to_string() } else { String::new() };
    let term   = if show.term   { env::var("TERM_PROGRAM").or_else(|_| env::var("TERM")).unwrap_or_else(|_| "unknown".into()) } else { String::new() };
    let locale = if show.locale { env::var("LANG").unwrap_or_else(|_| "C".into()) } else { String::new() };
    let de_wm  = if show.de_wm {
             if let Ok(v) = env::var("XDG_CURRENT_DESKTOP")         { v }
        else if let Ok(v) = env::var("DESKTOP_SESSION")             { v }
        else if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()     { "Hyprland".into() }
        else if env::var("SWAYSOCK").is_ok()                        { "Sway".into() }
        else if env::var("WAYLAND_DISPLAY").is_ok()                 { "Wayland".into() }
        else if env::var("DISPLAY").is_ok()                         { "X11".into() }
        else                                                        { "TTY".into() }
    } else { String::new() };

    // init — /proc/1/comm
    let init = if show.init {
        let comm = slurp("/proc/1/comm").trim().to_string();
        match comm.as_str() {
            "systemd" => "systemd",
            "openrc-init" | "openrc" => "OpenRC",
            "runit" => "runit",
            "s6-svscan" => "s6",
            _ if comm.is_empty() => "unknown",
            _ => &comm,
        }.to_string()
    } else { String::new() };

    // cpu_temp — /sys/class/thermal/thermal_zone0/temp
    let cpu_temp = if show.cpu_temp {
        let raw = slurp("/sys/class/thermal/thermal_zone0/temp");
        match raw.trim().parse::<u32>() {
            Ok(mc) if mc > 0 => format!("{}°C", mc / 1000),
            _ => "N/A".into(),
        }
    } else { String::new() };

    // processes — count numeric entries in /proc
    let processes = if show.processes {
        let n = fs::read_dir("/proc")
            .map(|d| d.flatten().filter(|e|
                e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit())
            ).count()).unwrap_or(0);
        n.to_string()
    } else { String::new() };

    // container — check for known container markers
    let container = if show.container {
        if      fs::metadata("/.dockerenv").is_ok()          { "Docker".into() }
        else if fs::metadata("/run/.containerenv").is_ok()   { "Podman".into() }
        else if slurp("/proc/1/cgroup").contains("docker")   { "Docker".into() }
        else if slurp("/proc/1/cgroup").contains("kubepods") { "Kubernetes".into() }
        else if slurp("/proc/1/cgroup").contains("lxc")      { "LXC".into() }
        else                                                  { "none".into() }
    } else { String::new() };

    // session — XDG_SESSION_TYPE
    let session = if show.session {
        env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "tty".into())
    } else { String::new() };

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
            if up { "active".into() } else { "inactive".into() }
        };
        // ports — count listening TCP ports
        let ports = {
            let tcp = fs::read_to_string("/proc/net/tcp").unwrap_or_default();
            tcp.lines().skip(1)
                .filter(|l| l.split_whitespace().nth(3).map(|s| s == "0A").unwrap_or(false))
                .count().to_string()
        };
        (ip, ssh, ports)
    } else {
        ("N/A".into(), "inactive".into(), "0".into())
    };

    // ── GPU — cached 24h, DRM-first probe ────────────────
    let gpu = if show.gpu || show.gpu_temp {
        crate::cache::get_or_compute("gpu", 86400, gpu_probe)
    } else { String::new() };

    // ── sequential pkgs ─────────────────────────────────
    let pkgs = if show.pkgs {
        crate::cache::get_or_compute("pkgs", 3600, crate::pkgs::count)
    } else { String::new() };

    // ── new fastfetch-inspired fields ───────────────────

    // swap — read SwapTotal and SwapFree from /proc/meminfo (already slurped)
    let swap = if show.swap {
        let raw = slurp("/proc/meminfo");
        let (mut total, mut free) = (0u64, 0u64);
        for line in raw.lines() {
            if      line.starts_with("SwapTotal:") { total = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); }
            else if line.starts_with("SwapFree:")  { free  = line.split_ascii_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); }
        }
        if total > 0 {
            let used = total.saturating_sub(free);
            format!("{:.1}G / {:.1}G", used as f64 / 1_048_576.0, total as f64 / 1_048_576.0)
        } else { "N/A".into() }
    } else { String::new() };

    // sound server — check PipeWire, PulseAudio sockets
    let sound = if show.sound {
        let uid = unsafe { libc::getuid() };
        let run = format!("/run/user/{}", uid);
             if fs::metadata(format!("{}/pipewire-0", run)).is_ok()    { "PipeWire".into() }
        else if fs::metadata(format!("{}/pulse/native", run)).is_ok()  { "PulseAudio".into() }
        else if fs::metadata("/run/pipewire-0").is_ok()                { "PipeWire".into() }
        else if fs::metadata("/run/pulse/native").is_ok()              { "PulseAudio".into() }
        else if fs::metadata("/run/pipewire").is_ok()                  { "PipeWire".into() }
        else { "ALSA".into() }
    } else { String::new() };

    // gpu_driver — check DRM driver or NVIDIA version
    let gpu_driver = if show.gpu_driver {
        let mut drv = "Unknown".to_string();
        for i in 0..4u8 {
            let uevent = format!("/sys/class/drm/card{}/device/uevent", i);
            if let Ok(u) = fs::read_to_string(&uevent) {
                if let Some(p) = u.lines().find(|l| l.starts_with("DRIVER=")) {
                    let name = p.trim_start_matches("DRIVER=").to_string();
                    if !name.is_empty() { drv = name; break; }
                }
            }
        }
        // NVIDIA version
        if drv == "nvidia" || drv == "Unknown" {
            if let Ok(v) = fs::read_to_string("/proc/driver/nvidia/version") {
                if let Some(l) = v.lines().next() {
                    let parts: Vec<&str> = l.split_whitespace().collect();
                    if parts.len() >= 2 { drv = format!("nvidia {}", parts[1]); }
                }
            }
        }
        // AMD version
        if drv == "amdgpu" || drv.starts_with("amdgpu") {
            if let Ok(v) = fs::read_to_string("/sys/module/amdgpu/version") {
                let v = v.trim();
                if !v.is_empty() { drv = format!("amdgpu {}", v); }
            }
        }
        drv
    } else { String::new() };

    // bios — /sys/class/dmi/id/bios_version
    let bios = if show.bios {
        let ver  = slurp("/sys/class/dmi/id/bios_version");
        let date = slurp("/sys/class/dmi/id/bios_date");
        let ver  = ver.trim().to_string();
        let date = date.trim().to_string();
        match (ver.is_empty(), date.is_empty()) {
            (false, false) => format!("{} ({})", ver, date),
            (false, true)  => ver,
            _              => "N/A".into(),
        }
    } else { String::new() };

    // board — motherboard name from DMI
    let board = if show.board {
        let name   = slurp("/sys/class/dmi/id/board_name");
        let vendor = slurp("/sys/class/dmi/id/board_vendor");
        let name   = name.trim().to_string();
        let vendor = vendor.trim().to_string();
        match (vendor.is_empty(), name.is_empty()) {
            (false, false) => format!("{} {}", vendor, name),
            (false, true)  => vendor,
            (true, false)  => name,
            _              => "N/A".into(),
        }
    } else { String::new() };

    // disk_type — rotational (HDD) vs SSD/NVMe
    let disk_type = if show.disk_type {
        // find root device from /proc/mounts
        let mounts = slurp("/proc/mounts");
        let root_dev = mounts.lines()
            .find(|l| l.split_whitespace().nth(1) == Some("/"))
            .and_then(|l| l.split_whitespace().nth(0))
            .unwrap_or("").to_string();
        let dev_name = root_dev.rsplit('/').next().unwrap_or("");
        // strip partition number (e.g. nvme0n1p3 -> nvme0n1, sda2 -> sda)
        let base_dev = dev_name.trim_end_matches(|c: char| c.is_ascii_digit());
        let base_dev = base_dev.trim_end_matches('p'); // for nvme0n1p3 -> nvme0n1
        let rot_path = format!("/sys/block/{}/queue/rotational", base_dev);
        match slurp(&rot_path).trim() {
            "0" => {
                if base_dev.starts_with("nvme") { "NVMe SSD".into() }
                else { "SSD".into() }
            }
            "1" => "HDD".into(),
            _   => "N/A".into(),
        }
    } else { String::new() };

    // pkg_updates — pacman updates count (quick dir mtime check)
    let pkg_updates = if show.pkg_updates {
        let sync_dir = "/var/lib/pacman/sync";
        if let Ok(d) = fs::read_dir(sync_dir) {
            let db_files: Vec<_> = d.flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".db"))
                .collect();
            if !db_files.is_empty() {
                // check local vs sync by counting pacman -Qu equivalent
                // simply count .db files as package repos
                format!("{} repos", db_files.len())
            } else { "0".into() }
        } else { "N/A".into() }
    } else { String::new() };

    // theme — GTK theme from settings.ini
    let theme = if show.theme {
        let home = env::var("HOME").unwrap_or_default();
        let gtk3 = format!("{}/.config/gtk-3.0/settings.ini", home);
        let mut t = String::new();
        if let Ok(raw) = fs::read_to_string(&gtk3) {
            for line in raw.lines() {
                if let Some(v) = line.strip_prefix("gtk-theme-name=") {
                    t = v.trim_matches('"').to_string();
                }
            }
        }
        // fallback: gtk2
        if t.is_empty() {
            let gtk2 = format!("{}/.gtkrc-2.0", home);
            if let Ok(raw) = fs::read_to_string(&gtk2) {
                for line in raw.lines() {
                    if let Some(v) = line.strip_prefix("gtk-theme-name=") {
                        t = v.trim_matches('"').trim().to_string();
                    }
                }
            }
        }
        if t.is_empty() { "N/A".into() } else { t }
    } else { String::new() };

    // icons — icon theme from settings.ini
    let icons = if show.icons {
        let home = env::var("HOME").unwrap_or_default();
        let gtk3 = format!("{}/.config/gtk-3.0/settings.ini", home);
        let mut t = String::new();
        if let Ok(raw) = fs::read_to_string(&gtk3) {
            for line in raw.lines() {
                if let Some(v) = line.strip_prefix("gtk-icon-theme-name=") {
                    t = v.trim_matches('"').to_string();
                }
            }
        }
        if t.is_empty() { "N/A".into() } else { t }
    } else { String::new() };

    // term_font — check env vars
    let term_font = if show.term_font {
        let font = env::var("ARC_FONT")
            .or_else(|_| env::var("FONT"))
            .or_else(|_| env::var("TERM_FONT"));
        match font {
            Ok(f) => f,
            Err(_) => "N/A".into(),
        }
    } else { String::new() };

    // de_wm_ver — version string for DE/WM
    let de_wm_ver = if show.de_wm_ver {
        let de = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
        if de.contains("gnome") {
            let raw = slurp("/usr/share/gnome/gnome-version.xml");
            // quick parse: look for <version>...</version>
            if let Some(start) = raw.find("<version>") {
                let after = &raw[start + 9..];
                if let Some(end) = after.find("</version>") {
                    after[..end].to_string()
                } else { "N/A".into() }
            } else { "N/A".into() }
        } else if de.contains("kde") || de.contains("plasma") {
            slurp("/usr/share/plasma/plasma-version").trim().to_string()
        } else if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
            // hyprctl version
            String::new() // would need subprocess — skip for now
        } else {
            String::new()
        }
    } else { String::new() };

    // init_ver — version of init system
    let init_ver = if show.init_ver {
        let comm = slurp("/proc/1/comm").trim().to_string();
        match comm.as_str() {
            "systemd" => {
                let raw = slurp("/etc/os-release");
                raw.lines()
                    .find(|l| l.starts_with("VERSION_ID="))
                    .map(|l| l[11..].trim_matches('"').to_string())
                    .or_else(|| {
                        // try reading systemd version from binary
                        for p in &["/usr/lib/systemd/systemd", "/usr/bin/systemd"] {
                            if fs::metadata(p).is_ok() {
                                // just show "systemd" without version if we can't get it
                                return Some("systemd".into());
                            }
                        }
                        None
                    })
                    .unwrap_or_else(|| "systemd".into())
            }
            other => other.to_string(),
        }
    } else { String::new() };

    // local_ip — interface + IPv4 via getifaddrs
    let local_ip = if show.local_ip {
        let mut result = String::new();
        unsafe {
            let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
            if libc::getifaddrs(&mut ifap) == 0 {
                let mut cur = ifap;
                while !cur.is_null() {
                    let ifa = &*cur;
                    if !ifa.ifa_addr.is_null()
                        && (*ifa.ifa_addr).sa_family as libc::c_int == libc::AF_INET
                    {
                        let name = std::ffi::CStr::from_ptr(ifa.ifa_name)
                            .to_string_lossy().to_string();
                        if name != "lo" {
                            let sa = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                            let octets = sa.sin_addr.s_addr.to_ne_bytes();
                            let ip = format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3]);
                            if !ip.starts_with("127.") {
                                result = format!("{} {}", name, ip);
                                break;
                            }
                        }
                    }
                    cur = ifa.ifa_next;
                }
                libc::freeifaddrs(ifap);
            }
        }
        if result.is_empty() { "N/A".into() } else { result }
    } else { String::new() };

    SysInfo { os, kernel, uptime_secs, uptime, res, pkgs, shell, de_wm, term,
              cpu, gpu, gpu_temp, battery, disk, load, locale, mem_used, mem_total,
              ip, ssh, ports,
              init, cpu_temp, processes, container, session,
              swap, sound, gpu_driver, bios, board, disk_type,
              pkg_updates, theme, icons, term_font, de_wm_ver, init_ver, local_ip }
}


