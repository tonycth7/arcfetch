use std::{env, fs};

use crate::{modules, net};

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
    pub os_id:       String,
    pub os_id_like:  String,
    // opt-in modules
    pub datetime:      String,
    pub editor:        String,
    pub dns:           String,
    pub host:          String,
    pub chassis:       String,
    pub bootmgr:       String,
    pub cpu_cache:     String,
    pub login_manager: String,
    pub brightness:    String,
    pub cpu_usage:     String,
    pub diskio:        String,
    pub netio:         String,
    pub physmem:       String,
    pub gpu_mem:       String,
    pub btrfs:         String,
    pub wifi:          String,
    pub users:         String,
    pub wallpaper:     String,
    pub media:         String,
    pub public_ip:     String,
    pub weather:       String,
    pub opengl:        String,
    pub vulkan:        String,
    pub cursor:        String,
    pub term_size:     String,
    pub term_theme:    String,
    pub codec:         String,
    pub bluetooth:     String,
    pub camera:        String,
    pub mouse:         String,
    pub keyboard:      String,
    pub tpm:           String,
    pub physical_disk: String,
    pub top_processes: String,
    pub monitor:       String,
}

// ── Low-level helpers ────────────────────────────────────

#[inline]
fn slurp(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }

/// CPU package temperature in whole °C.
///
/// Prefers hwmon drivers (`k10temp`/`zenpower` on AMD, `coretemp` on Intel,
/// generic SoC thermal on ARM) and falls back to ACPI thermal zones. Returns
/// `None` when no CPU sensor is exposed.
fn cpu_temp_c() -> Option<u32> {
    const CPU_SENSORS: &[&str] = &[
        "k10temp", "zenpower", "coretemp", "k8temp", "cpu_thermal", "cpu-thermal",
        "soc_thermal", "soc-thermal", "arm_thermal", "x86_pkg_temp", "acpitz",
    ];
    // 1. hwmon — name match, then read the package sensor (temp1 = Tctl /
    //    "Package id 0"); fall back to any positive tempN_input.
    if let Ok(dir) = fs::read_dir("/sys/class/hwmon") {
        for e in dir.flatten() {
            let base = e.path();
            let name = fs::read_to_string(base.join("name")).unwrap_or_default();
            let name = name.trim().to_ascii_lowercase();
            if !CPU_SENSORS.iter().any(|s| name == *s) { continue; }
            for cand in ["temp1_input", "temp2_input", "temp3_input"] {
                if let Ok(mc) = fs::read_to_string(base.join(cand)).unwrap_or_default()
                    .trim().parse::<u32>()
                {
                    if mc > 0 { return Some(mc / 1000); }
                }
            }
        }
    }
    // 2. ACPI / SoC thermal zones whose type names a CPU.
    if let Ok(dir) = fs::read_dir("/sys/class/thermal") {
        for e in dir.flatten() {
            let base = e.path();
            let ty = fs::read_to_string(base.join("type")).unwrap_or_default();
            let ty = ty.trim().to_ascii_lowercase();
            if !(ty.contains("cpu") || ty.contains("pkg") || ty.contains("soc") || ty.contains("x86")) {
                continue;
            }
            if let Ok(mc) = fs::read_to_string(base.join("temp")).unwrap_or_default()
                .trim().parse::<u32>()
            {
                if mc > 0 { return Some(mc / 1000); }
            }
        }
    }
    None
}

/// Seconds since boot via `sysinfo(2)`, or 0 on failure.
pub fn system_uptime_secs() -> u64 {
    let mut info = std::mem::MaybeUninit::<libc::sysinfo>::uninit();
    // SAFETY: `sysinfo` initializes the whole struct when it returns 0.
    if unsafe { libc::sysinfo(info.as_mut_ptr()) } == 0 {
        // SAFETY: initialized by the successful call above.
        unsafe { info.assume_init() }.uptime as u64
    } else {
        0
    }
}

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
    // 3. PCI fallback (only when DRM/NVIDIA fail) — the name is resolved from
    //    `pci.ids` and cached, since this path runs on every invocation on
    //    machines (e.g. AMD APUs) that expose no `product_name`.
    if let Ok(devs) = fs::read_dir("/sys/bus/pci/devices") {
        for e in devs.flatten() {
            let p = e.path();
            let class = fs::read_to_string(p.join("class")).unwrap_or_default();
            match class.trim() {
                "0x030000" | "0x030200" | "0x030001" => {}
                _ => continue,
            }
            let vid = fs::read_to_string(p.join("vendor")).unwrap_or_default();
            let vid = vid.trim().to_string();
            let did = fs::read_to_string(p.join("device")).unwrap_or_default();
            let did = did.trim().to_string();
            let key = format!("gpu_pci_{vid}_{did}");
            return crate::cache::get_or_compute_ttl(&key, 86_400, || gpu_pci_name(&vid, &did));
        }
    }
    "Unknown".into()
}

/// Human-readable GPU name for a `("0x1002", "0x15e7")` PCI vendor/device pair.
/// Uses the system `pci.ids` database when present, else a vendor label.
fn gpu_pci_name(vid: &str, did: &str) -> String {
    let short = match vid {
        "0x1002" => "AMD",
        "0x10de" => "NVIDIA",
        "0x8086" => "Intel",
        _ => "",
    };
    let vid_n = u16::from_str_radix(vid.trim_start_matches("0x"), 16).unwrap_or(0);
    let did_n = u16::from_str_radix(did.trim_start_matches("0x"), 16).unwrap_or(0);
    if let Some(name) = pci_ids_name(vid_n, did_n) {
        return format!("{short} {name}").trim().to_string();
    }
    if short.is_empty() {
        format!("GPU {vid}")
    } else {
        format!("{short} GPU ({did})")
    }
}

/// Look up a PCI device name in the system `pci.ids` database.
fn pci_ids_name(vid: u16, did: u16) -> Option<String> {
    const PATHS: &[&str] = &[
        "/usr/share/hwdata/pci.ids",
        "/usr/share/misc/pci.ids",
        "/usr/share/pci.ids",
        "/usr/share/pciutils/pci.ids",
        "/var/lib/pciutils/pci.ids",
    ];
    let want_v = format!("{vid:04x}");
    let want_d = format!("{did:04x}");
    for path in PATHS {
        let Ok(raw) = fs::read_to_string(path) else { continue };
        let mut in_vendor = false;
        for line in raw.lines() {
            if line.is_empty() || line.starts_with('#') { continue; }
            if !line.starts_with('\t') {
                // vendor line: "1002  Advanced Micro Devices, Inc. [AMD/ATI]"
                in_vendor = line.get(..4)
                    .map(|v| v.eq_ignore_ascii_case(&want_v))
                    .unwrap_or(false);
                continue;
            }
            if !in_vendor { continue; }
            // device lines have one tab; subsystem lines have two
            let Some(rest) = line.strip_prefix('\t') else { continue };
            if rest.starts_with('\t') { continue; }
            if rest.get(..4).map(|d| d.eq_ignore_ascii_case(&want_d)).unwrap_or(false) {
                let name = rest[4..].trim();
                if !name.is_empty() { return Some(name.to_string()); }
            }
        }
    }
    None
}

/// A `getifaddrs(3)` result that frees itself on drop.
struct IfAddrs(*mut libc::ifaddrs);

impl Drop for IfAddrs {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a successful `getifaddrs` call.
        unsafe { libc::freeifaddrs(self.0) };
    }
}

/// Interface carrying the IPv4 default route, from `/proc/net/route`.
fn default_route_iface() -> Option<String> {
    let route = slurp("/proc/net/route");
    for line in route.lines().skip(1) {
        // Iface Destination Gateway Flags …
        let mut f = line.split_whitespace();
        let iface = f.next()?;
        let dest = f.next().unwrap_or("");
        let _gw = f.next();
        let flags = f.next().unwrap_or("");
        if dest == "00000000"
            && u32::from_str_radix(flags, 16).map(|v| v & 0x1 != 0).unwrap_or(false)
        {
            return Some(iface.to_string());
        }
    }
    None
}

/// Primary IPv4 address as `("iface", "a.b.c.d")`.
///
/// Prefers the interface that carries the default route so bridges, VM
/// adapters and drained interfaces can't shadow the real one; skips
/// down/loopback/point-to-point links and link-local addresses.
fn local_ipv4() -> Option<(String, String)> {
    let want = default_route_iface();

    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: `list` is a valid out-pointer; on success it points at an owned list.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return None;
    }
    // SAFETY: the call succeeded, so `list` is valid and owned; the guard frees it.
    let list = IfAddrs(list);

    let mut best: Option<(String, String)> = None;
    let mut cur = list.0;
    while !cur.is_null() {
        // SAFETY: `cur` always points at a node in the list owned by `list`.
        let ifa = unsafe { &*cur };
        cur = ifa.ifa_next;
        if ifa.ifa_addr.is_null() { continue; }
        // SAFETY: `ifa_addr` is non-null and points at a `sockaddr`.
        let family = unsafe { (*ifa.ifa_addr).sa_family } as libc::c_int;
        if family != libc::AF_INET { continue; }

        let flags = ifa.ifa_flags as libc::c_int;
        if flags & libc::IFF_UP == 0 { continue; }
        if flags & (libc::IFF_LOOPBACK | libc::IFF_POINTOPOINT) != 0 { continue; }

        // SAFETY: `ifa_name` is a NUL-terminated C string.
        let name = unsafe { std::ffi::CStr::from_ptr(ifa.ifa_name) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: `family` is `AF_INET`, so the address is a `sockaddr_in`.
        let sa = unsafe { &*(ifa.ifa_addr as *const libc::sockaddr_in) };
        let octets = sa.sin_addr.s_addr.to_ne_bytes();
        let ip = format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3]);
        if ip.starts_with("127.") || ip.starts_with("169.254.") || ip == "0.0.0.0" { continue; }

        if want.as_deref() == Some(name.as_str()) {
            return Some((name, ip));
        }
        if best.is_none() {
            best = Some((name, ip));
        }
    }
    best
}

/// Collect all system info.
/// Only reads fields whose show-flag is true. Pass need_network=true for hacker preset.
#[inline]
pub fn collect_all(need_network: bool, show: &crate::config::Show) -> SysInfo {
    // ── fast sequential: all reads on main thread ──────────

    // /proc/meminfo — single pass shared by memory + swap (was read twice)
    let (mem_used, mem_total, swap) = if show.memory || show.swap {
        let raw = slurp("/proc/meminfo");
        let (mut mem_total_kib, mut mem_avail_kib) = (0u64, 0u64);
        let (mut swap_total_kib, mut swap_free_kib) = (0u64, 0u64);
        for line in raw.lines() {
            let Some((key, rest)) = line.split_once(':') else { continue };
            let value = rest
                .split_ascii_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            match key {
                "MemTotal"     => mem_total_kib = value,
                "MemAvailable" => mem_avail_kib = value,
                "SwapTotal"    => swap_total_kib = value,
                "SwapFree"     => swap_free_kib = value,
                _ => {}
            }
        }

        let (used, total) = if show.memory {
            (
                mem_total_kib.saturating_sub(mem_avail_kib) / 1024,
                mem_total_kib / 1024,
            )
        } else {
            (0, 0)
        };

        let swap = if show.swap {
            if swap_total_kib > 0 {
                let swap_used = swap_total_kib.saturating_sub(swap_free_kib);
                format!(
                    "{:.1}G / {:.1}G",
                    swap_used as f64 / 1_048_576.0,
                    swap_total_kib as f64 / 1_048_576.0
                )
            } else {
                "N/A".into()
            }
        } else {
            String::new()
        };

        (used, total, swap)
    } else {
        (0, 0, String::new())
    };

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

    // /etc/os-release — read once (pretty name + IDs for logo detection)
    let os_raw = if show.os { Some(slurp("/etc/os-release")) } else { None };
    let os_id = os_raw.as_deref()
        .and_then(|r| r.lines().find(|l| l.starts_with("ID=")))
        .map(|l| l[3..].trim().trim_matches('"').to_string()).unwrap_or_default();
    let os_id_like = os_raw.as_deref()
        .and_then(|r| r.lines().find(|l| l.starts_with("ID_LIKE=")))
        .map(|l| l[8..].trim().trim_matches('"').to_string()).unwrap_or_default();

    let os = if show.os {
        crate::cache::get_or_compute("os", 86400, || {
            os_raw.as_deref()
                .and_then(|r| r.lines().find(|l| l.starts_with("PRETTY_NAME=")))
                .map(|l| l[12..].trim_matches('"').to_string())
                .unwrap_or_else(|| "Arch Linux".into())
        })
    } else { String::new() };

    let kernel = if show.kernel {
        let mut uts = std::mem::MaybeUninit::<libc::utsname>::uninit();
        // SAFETY: `uname` fully initializes `uts` when it returns 0.
        if unsafe { libc::uname(uts.as_mut_ptr()) } == 0 {
            // SAFETY: initialized by the successful call above.
            let uts = unsafe { uts.assume_init() };
            // SAFETY: `uts.release` is a NUL-terminated C string.
            unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        } else {
            "unknown".into()
        }
    } else { String::new() };

    // uptime via sysinfo (1 syscall, no file I/O)
    let (uptime_secs, uptime) = if show.uptime {
        let secs = system_uptime_secs();
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
        let mut st = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: `c"/"` is a valid NUL-terminated path; `statvfs` fills `st` on success.
        if unsafe { libc::statvfs(c"/".as_ptr(), st.as_mut_ptr()) } == 0 {
            // SAFETY: initialized by the successful call above.
            let st = unsafe { st.assume_init() };
            let blk   = st.f_frsize;
            let total = st.f_blocks * blk;
            let avail = st.f_bavail * blk;
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

    // cpu_temp — hwmon (k10temp/coretemp/…) with ACPI thermal-zone fallback
    let cpu_temp = if show.cpu_temp {
        cpu_temp_c().map(|c| format!("{}°C", c)).unwrap_or_else(|| "N/A".into())
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
        // primary interface address via getifaddrs(3) — no subprocess, and it
        // follows the default route instead of fib_trie's broadcast /32 entries.
        let ip = local_ipv4()
            .map(|(_, ip)| ip)
            .unwrap_or_else(|| "N/A".into());
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
            .and_then(|l| l.split_whitespace().next())
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

    // term_font — env, then the terminal's own config file
    let term_font = if show.term_font {
        modules::term_font()
    } else { String::new() };

    // de_wm_ver — resolved in the parallel batch below (may spawn `<wm> --version`)

    // de_wm_ver — version string for DE/WM (computed in the parallel batch)

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
        match local_ipv4() {
            Some((name, ip)) => format!("{name} {ip}"),
            None => "N/A".into(),
        }
    } else { String::new() };

    // ── opt-in modules (off by default) ─────────────────
    // Cheap collectors run inline.
    let datetime      = if show.datetime      { modules::datetime() } else { String::new() };
    let editor        = if show.editor        { modules::editor() } else { String::new() };
    let dns           = if show.dns           { modules::dns() } else { String::new() };
    let host          = if show.host          { modules::host() } else { String::new() };
    let chassis       = if show.chassis       { modules::chassis() } else { String::new() };
    let bootmgr       = if show.bootmgr       { modules::bootmgr() } else { String::new() };
    let cpu_cache     = if show.cpu_cache     { modules::cpu_cache() } else { String::new() };
    let login_manager = if show.login_manager { modules::login_manager() } else { String::new() };
    let brightness    = if show.brightness    { modules::brightness() } else { String::new() };
    let diskio        = if show.diskio        { modules::diskio() } else { String::new() };
    let netio         = if show.netio         { modules::netio() } else { String::new() };
    let physmem       = if show.physmem       { modules::physmem() } else { String::new() };
    let gpu_mem       = if show.gpu_mem       { modules::gpu_mem() } else { String::new() };
    let btrfs         = if show.btrfs         { modules::btrfs() } else { String::new() };
    let users         = if show.users         { modules::users() } else { String::new() };
    let wifi          = if show.wifi          { modules::wifi() } else { String::new() };
    let cursor        = if show.cursor        { modules::cursor() } else { String::new() };
    let term_size     = if show.term_size     { modules::term_size() } else { String::new() };
    let camera        = if show.camera        { modules::camera() } else { String::new() };
    let mouse         = if show.mouse         { modules::mouse() } else { String::new() };
    let keyboard      = if show.keyboard      { modules::keyboard() } else { String::new() };
    let tpm           = if show.tpm           { modules::tpm() } else { String::new() };
    let monitor       = if show.monitor       { modules::monitor() } else { String::new() };

    // The slow collectors — subprocess spawns (`glxinfo`, `playerctl`, `vainfo`,
    // `hciconfig`, …), network fetches and `/proc` walks — run concurrently, so
    // `--all` costs ~max(collector) instead of the sum. Results are cached.
    let cpu_start = std::time::Instant::now();
    let cpu0      = show.cpu_usage.then(modules::cpu_snapshot);
    let (wallpaper, media, public_ip, weather, opengl, vulkan, codec, bluetooth,
         physical_disk, top_processes, term_theme, de_wm_ver) =
        std::thread::scope(|s| {
        let wallpaper = show.wallpaper.then(|| s.spawn(modules::wallpaper));
        let media     = show.media.then(|| s.spawn(modules::media));
        let public_ip = show.public_ip.then(|| s.spawn(|| {
            crate::cache::get_or_compute_swr("public_ip", 3600, ||
                net::public_ip().unwrap_or_else(|| "N/A".into()))
        }));
        let weather   = show.weather.then(|| s.spawn(|| {
            crate::cache::get_or_compute_swr("weather", 900, ||
                net::weather("").unwrap_or_else(|| "N/A".into()))
        }));
        let opengl    = show.opengl.then(|| s.spawn(|| {
            crate::cache::get_or_compute("opengl", 86400, modules::opengl)
        }));
        let vulkan    = show.vulkan.then(|| s.spawn(|| {
            crate::cache::get_or_compute("vulkan", 86400, modules::vulkan)
        }));
        let codec     = show.codec.then(|| s.spawn(|| {
            crate::cache::get_or_compute("codec", 86400, modules::codec)
        }));
        let bluetooth = show.bluetooth.then(|| s.spawn(|| {
            crate::cache::get_or_compute("bluetooth", 86400, modules::bluetooth)
        }));
        let physical_disk = show.physical_disk.then(|| s.spawn(modules::physical_disk));
        let top_processes = show.top_processes.then(|| s.spawn(modules::top_processes));
        let term_theme = show.term_theme.then(|| s.spawn(|| {
            crate::cache::get_or_compute_ttl("term_theme", 300, modules::term_theme)
        }));
        let de_wm_ver = show.de_wm_ver.then(|| s.spawn(modules::de_wm_version));
        (
            wallpaper.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            media.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            public_ip.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            weather.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            opengl.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            vulkan.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            codec.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            bluetooth.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            physical_disk.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            top_processes.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            term_theme.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
            de_wm_ver.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
        )
    });

    // CPU usage spans the work above; only pad the window if it finished fast.
    let cpu_usage = match cpu0 {
        None => String::new(),
        Some(None) => "N/A".into(),
        Some(Some((idle0, total0))) => {
            // 60 ms ≈ 6 USER_HZ ticks (HZ=100), enough for a stable reading
            // while keeping the warm `--all` path well under 100 ms.
            const MIN_MS: u64 = 60;
            if let Some(rest) = std::time::Duration::from_millis(MIN_MS)
                .checked_sub(cpu_start.elapsed())
            {
                std::thread::sleep(rest);
            }
            match modules::cpu_snapshot() {
                Some((idle1, total1)) => modules::cpu_usage_between(idle0, total0, idle1, total1),
                None => "N/A".into(),
            }
        }
    };

    SysInfo { os, kernel, uptime_secs, uptime, res, pkgs, shell, de_wm, term,
              cpu, gpu, gpu_temp, battery, disk, load, locale, mem_used, mem_total,
              ip, ssh, ports,
              init, cpu_temp, processes, container, session,
              swap, sound, gpu_driver, bios, board, disk_type,
              pkg_updates, theme, icons, term_font, de_wm_ver, init_ver, local_ip,
              os_id, os_id_like,
              datetime, editor, dns, host, chassis, bootmgr, cpu_cache, login_manager,
              brightness, cpu_usage, diskio, netio, physmem, gpu_mem, btrfs, wifi,
              users, wallpaper, media, public_ip, weather, opengl, vulkan,
              cursor, term_size, term_theme, codec, bluetooth, camera, mouse,
              keyboard, tpm, physical_disk, top_processes, monitor }
}


