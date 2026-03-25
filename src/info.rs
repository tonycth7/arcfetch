// src/info.rs — arcfetch: zero-copy, ASM CPUID, DRM+mmap GPU, direct-read memory
//
// Key techniques:
//   CPU    : cpuid inline ASM (x86_64) — no /proc/cpuinfo read for brand string
//   GPU    : /sys/class/drm scan → vendor+device IDs → mmap(pci.ids) substring match
//   Memory : O_RDONLY + read() into stack buffer → parse MemTotal / MemAvailable
//   Pkgs   : pacman dir-count | nix mmap manifest | flatpak dir-count | snap dir-count
//   Output : single BufWriter flush (caller's responsibility)

use std::{env, fs, ptr};
use std::ffi::CStr;

use std::thread;

// ──────────────────────────────────────────────────────────────────────────────
//  Public SysInfo struct
// ──────────────────────────────────────────────────────────────────────────────

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
}

// ──────────────────────────────────────────────────────────────────────────────
//  Low-level helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Read a small /proc or /sys file into a stack buffer via a single read(2).
macro_rules! read_proc {
    ($path:expr, $buf:expr) => {{
        let path = concat!($path, "\0");
        let fd = unsafe {
            libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDONLY)
        };
        let n = if fd < 0 {
            0usize
        } else {
            let n = unsafe {
                libc::read(fd, $buf.as_mut_ptr() as *mut libc::c_void, $buf.len())
            };
            unsafe { libc::close(fd); }
            if n < 0 { 0usize } else { n as usize }
        };
        n
    }};
}

/// mmap a file read-only.  Returns (ptr, len) or (MAP_FAILED, 0).
unsafe fn mmap_file(path: &str) -> (*const u8, usize) { unsafe {
    let cpath = format!("{}\0", path);
    let fd = libc::open(cpath.as_ptr() as *const libc::c_char, libc::O_RDONLY);
    if fd < 0 { return (libc::MAP_FAILED as *const u8, 0); }

    let mut st: libc::stat = std::mem::zeroed();
    if libc::fstat(fd, &mut st) < 0 { libc::close(fd); return (libc::MAP_FAILED as *const u8, 0); }
    let len = st.st_size as usize;
    if len == 0 { libc::close(fd); return (libc::MAP_FAILED as *const u8, 0); }

    let p = libc::mmap(ptr::null_mut(), len, libc::PROT_READ, libc::MAP_PRIVATE, fd, 0);
    libc::close(fd);
    if p == libc::MAP_FAILED { (libc::MAP_FAILED as *const u8, 0) }
    else { (p as *const u8, len) }
}}

/// Fast substring search — returns index of first occurrence.
#[inline]
fn memmem(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    if haystack.len() < needle.len() { return None; }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Count non-overlapping occurrences of needle in haystack.
#[inline]
fn memcount(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() { return 0; }
    let mut count = 0usize;
    let mut i = 0usize;
    let end = haystack.len() - needle.len();
    while i <= end {
        if haystack[i..i + needle.len()] == *needle {
            count += 1;
            i += needle.len();
        } else {
            i += 1;
        }
    }
    count
}

// ──────────────────────────────────────────────────────────────────────────────
//  CPU — cpuid inline assembly (x86_64)
// ──────────────────────────────────────────────────────────────────────────────

fn cpu_brand_string() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let mut brand = [0u8; 48];
        for (i, leaf) in [0x8000_0002u32, 0x8000_0003, 0x8000_0004].iter().enumerate() {
            let (eax, ebx, ecx, edx): (u32, u32, u32, u32);
            unsafe {
                // rbx is reserved by LLVM for position-independent code; save/restore it.
                std::arch::asm!(
                    "push rbx",
                    "cpuid",
                    "mov {ebx_out:e}, ebx",
                    "pop rbx",
                    inout("eax") *leaf => eax,
                    out("ecx")   ecx,
                    out("edx")   edx,
                    ebx_out = out(reg) ebx,
                    options(nostack),
                );
            }
            let base = i * 16;
            brand[base      ..base +  4].copy_from_slice(&eax.to_le_bytes());
            brand[base +  4 ..base +  8].copy_from_slice(&ebx.to_le_bytes());
            brand[base +  8 ..base + 12].copy_from_slice(&ecx.to_le_bytes());
            brand[base + 12 ..base + 16].copy_from_slice(&edx.to_le_bytes());
        }
        let s = CStr::from_bytes_until_nul(&brand)
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_else(|_| {
                String::from_utf8_lossy(&brand).trim_end_matches('\0').to_string()
            });
        return s.split_ascii_whitespace()
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let mut buf = [0u8; 4096];
        let n = read_proc!("/proc/cpuinfo", buf);
        let raw = std::str::from_utf8(&buf[..n]).unwrap_or("");
        raw.lines()
            .find(|l| l.starts_with("model name") || l.starts_with("Model name"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.split_ascii_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| "Unknown".into())
    }
}

/// Read CPU logical core count from /sys/devices/system/cpu/present.
fn cpu_core_count() -> usize {
    let mut buf = [0u8; 64];
    let n = read_proc!("/sys/devices/system/cpu/present", buf);
    let s = std::str::from_utf8(&buf[..n]).unwrap_or("").trim();
    if s.is_empty() { return 1; }
    s.split(',').fold(0usize, |acc, part| {
        let part = part.trim();
        if let Some((lo, hi)) = part.split_once('-') {
            let lo: usize = lo.parse().unwrap_or(0);
            let hi: usize = hi.parse().unwrap_or(lo);
            acc + (hi - lo + 1)
        } else {
            acc + 1
        }
    })
}

// ──────────────────────────────────────────────────────────────────────────────
//  GPU — DRM sysfs + mmap(pci.ids)
// ──────────────────────────────────────────────────────────────────────────────

const PCI_IDS_PATHS: &[&str] = &[
    "/usr/share/hwdata/pci.ids",
    "/usr/share/misc/pci.ids",
    "/usr/share/pci.ids",
    "/usr/local/share/hwdata/pci.ids",
];

/// Look up vendor+device in pci.ids using mmap + substring search.
/// vid and did are lowercase 4-hex strings (no "0x" prefix).
fn lookup_pci_name(vid: &str, did: &str) -> Option<String> {
    for &path in PCI_IDS_PATHS {
        let (ptr, len) = unsafe { mmap_file(path) };
        if ptr == libc::MAP_FAILED as *const u8 { continue; }
        let data = unsafe { std::slice::from_raw_parts(ptr, len) };

        let vendor_pat = format!("\n{}  ", vid.to_lowercase());
        let device_pat = format!("\n\t{}  ", did.to_lowercase());

        let result = (|| -> Option<String> {
            let vpos = memmem(data, vendor_pat.as_bytes())?;
            let vendor_section = &data[vpos + vendor_pat.len()..];
            // vendor section ends at next non-indented line
            let section_end = vendor_section.windows(2)
                .enumerate()
                .find(|(_, w)| w[0] == b'\n' && w[1] != b'\t' && w[1] != b'#' && w[1] != b'\n')
                .map(|(i, _)| i + 1)
                .unwrap_or(vendor_section.len());
            let vendor_body = &vendor_section[..section_end];

            let dpos = memmem(vendor_body, device_pat.as_bytes())?;
            let after = &vendor_body[dpos + device_pat.len()..];
            let name_end = after.iter().position(|&b| b == b'\n').unwrap_or(after.len());
            let name = std::str::from_utf8(&after[..name_end]).ok()?.trim();
            if name.is_empty() { return None; }
            Some(name.to_string())
        })();

        unsafe { libc::munmap(ptr as *mut libc::c_void, len); }
        if result.is_some() { return result; }
    }
    None
}

fn detect_gpu() -> String {
    // 1. NVIDIA proprietary
    if let Ok(dir) = fs::read_dir("/proc/driver/nvidia/gpus") {
        for e in dir.flatten() {
            if let Ok(info) = fs::read_to_string(e.path().join("information")) {
                if let Some(l) = info.lines().find(|l| l.starts_with("Model:")) {
                    return l[6..].trim().to_string();
                }
            }
        }
    }

    // 2. DRM sysfs: vendor+device IDs → mmap pci.ids
    for i in 0..4u8 {
        let base = format!("/sys/class/drm/card{}/device", i);

        // Only display controllers (class 0x03xxxx)
        let class = fs::read_to_string(format!("{}/class", base)).unwrap_or_default();
        let class = class.trim();
        if !class.is_empty() && !class.starts_with("0x03") { continue; }

        let vendor_raw = fs::read_to_string(format!("{}/vendor", base)).unwrap_or_default();
        let device_raw = fs::read_to_string(format!("{}/device", base)).unwrap_or_default();
        let vendor = vendor_raw.trim();
        let device = device_raw.trim();
        if vendor.is_empty() || device.is_empty() { continue; }

        let vid = vendor.strip_prefix("0x").unwrap_or(vendor).to_lowercase();
        let did = device.strip_prefix("0x").unwrap_or(device).to_lowercase();

        if let Some(name) = lookup_pci_name(&vid, &did) {
            return name;
        }

        let vendor_name = match vendor {
            "0x1002" => "AMD",
            "0x10de" => "NVIDIA",
            "0x8086" => "Intel",
            "0x1414" => "Microsoft",
            _        => "GPU",
        };
        return format!("{} [{}]", vendor_name, did.to_uppercase());
    }

    // 3. product_name / label sysfs fallback
    for i in 0..4u8 {
        let base = format!("/sys/class/drm/card{}/device", i);
        for attr in &["product_name", "label"] {
            if let Ok(v) = fs::read_to_string(format!("{}/{}", base, attr)) {
                let v = v.trim().to_string();
                if !v.is_empty() { return v; }
            }
        }
    }

    "Unknown".into()
}

// ──────────────────────────────────────────────────────────────────────────────
//  Memory — single read() into stack buffer
// ──────────────────────────────────────────────────────────────────────────────

fn read_memory() -> (u64, u64) {
    let mut buf = [0u8; 2048];
    let n = read_proc!("/proc/meminfo", buf);
    let text = match std::str::from_utf8(&buf[..n]) { Ok(s) => s, Err(_) => return (0, 0) };
    let (mut total_kb, mut avail_kb) = (0u64, 0u64);
    for line in text.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = line.split_ascii_whitespace().nth(1)
                .and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            avail_kb = line.split_ascii_whitespace().nth(1)
                .and_then(|v| v.parse().ok()).unwrap_or(0);
            break;
        }
    }
    (total_kb.saturating_sub(avail_kb) / 1024, total_kb / 1024)
}

// ──────────────────────────────────────────────────────────────────────────────
//  Package counting
// ──────────────────────────────────────────────────────────────────────────────

fn count_nix() -> Option<usize> {
    let home = env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{}/.local/state/nix/profiles/home-manager/manifest.json", home),
        format!("{}/.nix-profile/manifest.json", home),
        "/nix/var/nix/profiles/default/manifest.json".to_string(),
        "/nix/var/nix/profiles/system/manifest.json".to_string(),
    ];
    for path in &candidates {
        let (ptr, len) = unsafe { mmap_file(path) };
        if ptr == libc::MAP_FAILED as *const u8 { continue; }
        let data = unsafe { std::slice::from_raw_parts(ptr, len) };
        let count = memcount(data, b"\"name\":");
        unsafe { libc::munmap(ptr as *mut libc::c_void, len); }
        if count > 0 { return Some(count); }
    }
    None
}

fn count_pacman() -> Option<usize> {
    let d = fs::read_dir("/var/lib/pacman/local").ok()?;
    let n = d.count().saturating_sub(1);
    if n > 0 { Some(n) } else { None }
}

fn count_flatpak() -> Option<usize> {
    let home = env::var("HOME").unwrap_or_default();
    let dirs = [
        "/var/lib/flatpak/app".to_string(),
        format!("{}/.local/share/flatpak/app", home),
    ];
    let mut total = 0usize;
    let mut found = false;
    for dir in &dirs {
        if let Ok(d) = fs::read_dir(dir) { total += d.count(); found = true; }
    }
    if found && total > 0 { Some(total) } else { None }
}

fn count_snap() -> Option<usize> {
    let d = fs::read_dir("/snap").ok()?;
    let n = d.flatten()
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s != "bin" && !s.starts_with('.')
        })
        .count();
    if n > 0 { Some(n) } else { None }
}

fn collect_packages() -> String {
    let mut parts: Vec<String> = Vec::with_capacity(4);
    if let Some(n) = count_pacman()  { parts.push(format!("{} (pacman)", n)); }
    if let Some(n) = count_nix()     { parts.push(format!("{} (nix)",    n)); }
    if let Some(n) = count_flatpak() { parts.push(format!("{} (flatpak)",n)); }
    if let Some(n) = count_snap()    { parts.push(format!("{} (snap)",   n)); }
    if parts.is_empty() { "unknown".into() } else { parts.join(", ") }
}

// ──────────────────────────────────────────────────────────────────────────────
//  Other collectors
// ──────────────────────────────────────────────────────────────────────────────

#[inline]
fn slurp(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }

fn read_uptime() -> (u64, String) {
    let mut buf = [0u8; 32];
    let n = read_proc!("/proc/uptime", buf);
    let s = std::str::from_utf8(&buf[..n]).unwrap_or("0");
    let secs: u64 = s.split('.').next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    let short = match (d, h, m) {
        (0, 0, m) => format!("{}m", m),
        (0, h, m) => format!("{}h {}m", h, m),
        (d, h, m) => format!("{}d {}h {}m", d, h, m),
    };
    (secs, short)
}

fn read_os_kernel() -> (String, String) {
    let os_raw = slurp("/etc/os-release");
    let os = os_raw.lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| l[12..].trim_matches('"').to_string())
        .unwrap_or_else(|| "Linux".into());
    let mut buf = [0u8; 256];
    let n = read_proc!("/proc/version", buf);
    let kver = std::str::from_utf8(&buf[..n]).unwrap_or("")
        .split_whitespace().nth(2).unwrap_or("unknown").to_string();
    (os, kver)
}

fn read_resolution() -> String {
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

fn read_gpu_temp() -> String {
    for i in 0..4u8 {
        let base = format!("/sys/class/drm/card{}/device/hwmon", i);
        if let Ok(hwmons) = fs::read_dir(&base) {
            for hw in hwmons.flatten() {
                if let Ok(raw) = fs::read_to_string(hw.path().join("temp1_input")) {
                    if let Ok(mc) = raw.trim().parse::<u32>() {
                        return format!("{}°C", mc / 1000);
                    }
                }
            }
        }
    }
    "N/A".into()
}

fn read_battery() -> String {
    for bat in &["BAT0", "BAT1", "BAT"] {
        let base = format!("/sys/class/power_supply/{}", bat);
        if let Ok(cap) = fs::read_to_string(format!("{}/capacity", base)) {
            let status = fs::read_to_string(format!("{}/status", base)).unwrap_or_default();
            let icon = match status.trim() {
                "Charging"    => "↑",
                "Discharging" => "↓",
                "Full"        => "✓",
                _             => "",
            };
            return format!("{}% {}", cap.trim(), icon).trim().to_string();
        }
    }
    "N/A".into()
}

fn read_disk() -> String {
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(b"/\0".as_ptr().cast::<libc::c_char>(), &mut st) } == 0 {
        let blk   = st.f_frsize as u64;
        let total = st.f_blocks as u64 * blk;
        let avail = st.f_bavail as u64 * blk;
        let gb    = 1_073_741_824.0f64;
        format!("{:.1}G / {:.1}G", total.saturating_sub(avail) as f64 / gb, total as f64 / gb)
    } else { "unknown".into() }
}

fn read_load() -> String {
    let mut buf = [0u8; 64];
    let n = read_proc!("/proc/loadavg", buf);
    std::str::from_utf8(&buf[..n]).unwrap_or("")
        .split_ascii_whitespace().take(3).collect::<Vec<_>>().join("  ")
}

// ──────────────────────────────────────────────────────────────────────────────
//  Network (hacker preset only)
// ──────────────────────────────────────────────────────────────────────────────

fn collect_network() -> (String, String, String) {
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

    let ssh = {
        let tcp = fs::read_to_string("/proc/net/tcp").unwrap_or_default();
        let up  = tcp.lines().skip(1).any(|l| {
            let c: Vec<&str> = l.split_whitespace().collect();
            c.get(1).map(|a| a.ends_with(":0016")).unwrap_or(false)
                && c.get(3).map(|s| *s == "0A").unwrap_or(false)
        });
        if up { "running (port 22)".into() } else { "not running".into() }
    };

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
}

// ──────────────────────────────────────────────────────────────────────────────
//  Main entry point
// ──────────────────────────────────────────────────────────────────────────────

pub fn collect_all(need_network: bool) -> SysInfo {
    // Slow real-FS collectors run in parallel threads
    let t_pkgs = thread::spawn(collect_packages);
    let t_gpu  = thread::spawn(detect_gpu);

    // Fast virtual-FS + syscall reads on main thread
    let (os, kernel)            = read_os_kernel();
    let (uptime_secs, uptime)   = read_uptime();
    let (mem_used, mem_total)   = read_memory();

    let cpu = {
        let brand = cpu_brand_string();
        let cores = cpu_core_count();
        format!("{} ({}x)", brand, cores)
    };

    let disk     = read_disk();
    let load     = read_load();
    let res      = read_resolution();
    let gpu_temp = read_gpu_temp();
    let battery  = read_battery();

    let shell  = env::var("SHELL").unwrap_or_default()
        .rsplit('/').next().unwrap_or("sh").to_string();
    let term   = env::var("TERM_PROGRAM")
        .or_else(|_| env::var("TERM"))
        .unwrap_or_else(|_| "unknown".into());
    let locale = env::var("LANG").unwrap_or_else(|_| "C".into());
    let de_wm  = {
             if let Ok(v) = env::var("XDG_CURRENT_DESKTOP")     { v }
        else if let Ok(v) = env::var("DESKTOP_SESSION")         { v }
        else if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() { "Hyprland".into() }
        else if env::var("SWAYSOCK").is_ok()                    { "Sway".into()     }
        else if env::var("WAYLAND_DISPLAY").is_ok()             { "Wayland".into()  }
        else if env::var("DISPLAY").is_ok()                     { "X11".into()      }
        else                                                     { "TTY".into()      }
    };

    let (ip, ssh, ports) = if need_network { collect_network() }
                           else { (String::new(), String::new(), String::new()) };

    let pkgs = t_pkgs.join().unwrap_or_else(|_| "unknown".into());
    let gpu  = t_gpu.join().unwrap_or_else(|_| "Unknown".into());

    SysInfo {
        os, kernel, uptime_secs, uptime, res, pkgs, shell, de_wm, term,
        cpu, gpu, gpu_temp, battery, disk, load, locale,
        mem_used, mem_total, ip, ssh, ports,
    }
}
