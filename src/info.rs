// src/info.rs — arcfetch: zero-copy, ASM CPUID, DRM+mmap GPU, direct-read memory
//
// KEY OPTIMISATION: CollectNeeds drives every branch — only the fields
// actually visible in the current preset/show config are collected.
// Minimal preset: OS + kernel + uptime + memory → ~0 real I/O, no threads.
// Full / hacker preset: threads for GPU + packages run in parallel.

use std::{env, fs, ptr};
use std::ffi::CStr;
use std::thread;

// ──────────────────────────────────────────────────────────────────────────────
//  What the caller actually needs — drives all conditional collection
// ──────────────────────────────────────────────────────────────────────────────

pub struct CollectNeeds {
    pub os:       bool,
    pub kernel:   bool,
    pub uptime:   bool,
    pub res:      bool,
    pub pkgs:     bool,
    pub shell:    bool,
    pub de_wm:    bool,
    pub term:     bool,
    pub cpu:      bool,
    pub gpu:      bool,   // also covers gpu_temp
    pub battery:  bool,
    pub memory:   bool,
    pub disk:     bool,
    pub load:     bool,
    pub locale:   bool,
    pub network:  bool,   // ip + ssh + ports
}

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

/// Single read(2) into a stack buffer — one syscall, no heap.
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
unsafe fn mmap_file(path: &str) -> (*const u8, usize) {
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
}

#[inline]
fn memmem_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    if haystack.len() < needle.len() { return None; }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[inline]
fn memcount(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() { return 0; }
    let mut count = 0usize;
    let mut i = 0usize;
    let end = haystack.len() - needle.len();
    while i <= end {
        if haystack[i..i + needle.len()] == *needle { count += 1; i += needle.len(); }
        else { i += 1; }
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
                // rbx is reserved by LLVM; save/restore around cpuid
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
        // Strip redundant tokens (matches bfetch's brand cleanup)
        let skip_tokens: &[&str] = &[
            "six-core", "eight-core", "quad-core", "twelve-core", "sixteen-core",
            "24-core", "32-core", "64-core", "-core", "processor", "with radeon graphics",
        ];
        let cleaned: Vec<&str> = s.split_ascii_whitespace()
            .filter(|tok| {
                let low = tok.to_lowercase();
                // stop at frequency marker
                if *tok == "@" { return false; }
                !skip_tokens.iter().any(|s| low.contains(s))
            })
            .collect();
        return cleaned.join(" ");
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let mut buf = [0u8; 4096];
        let n = read_proc!("/proc/cpuinfo", buf);
        let raw = std::str::from_utf8(&buf[..n]).unwrap_or("");
        raw.lines()
            .find(|l| l.starts_with("model name") || l.starts_with("Hardware"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.split_ascii_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| "Unknown".into())
    }
}

fn cpu_core_count() -> usize {
    let mut buf = [0u8; 64];
    let n = read_proc!("/sys/devices/system/cpu/present", buf);
    let s = std::str::from_utf8(&buf[..n]).unwrap_or("").trim();
    if s.is_empty() { return 1; }
    s.split(',').fold(0usize, |acc, part| {
        let part = part.trim();
        if let Some((lo, hi)) = part.split_once('-') {
            acc + (hi.parse::<usize>().unwrap_or(0)
                     .saturating_sub(lo.parse::<usize>().unwrap_or(0)) + 1)
        } else { acc + 1 }
    })
}

/// Max CPU frequency from cpufreq sysfs (returns GHz or empty string).
fn cpu_max_ghz() -> String {
    let mut buf = [0u8; 32];
    let n = read_proc!("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq", buf);
    if n == 0 { return String::new(); }
    let khz: u64 = std::str::from_utf8(&buf[..n]).unwrap_or("").trim()
        .parse().unwrap_or(0);
    if khz < 100_000 { return String::new(); }
    format!(" @ {:.2} GHz", khz as f64 / 1_000_000.0)
}

fn collect_cpu() -> String {
    let brand = cpu_brand_string();
    let cores = cpu_core_count();
    let freq  = cpu_max_ghz();
    format!("{} ({}){}", brand, cores, freq)
}

// ──────────────────────────────────────────────────────────────────────────────
//  GPU — DRM sysfs scan + mmap(pci.ids) + mmap(amdgpu.ids)
// ──────────────────────────────────────────────────────────────────────────────

const PCI_IDS_PATHS: &[&str] = &[
    "/usr/share/hwdata/pci.ids",
    "/usr/share/misc/pci.ids",
    "/usr/share/pci.ids",
    "/usr/local/share/hwdata/pci.ids",
];

fn lookup_pci_name(vid: &str, did: &str) -> Option<String> {
    for &path in PCI_IDS_PATHS {
        let (ptr, len) = unsafe { mmap_file(path) };
        if ptr == libc::MAP_FAILED as *const u8 { continue; }
        let data = unsafe { std::slice::from_raw_parts(ptr, len) };
        let vendor_pat = format!("\n{}  ", vid.to_lowercase());
        let device_pat = format!("\n\t{}  ", did.to_lowercase());
        let result = (|| -> Option<String> {
            let vpos = memmem_bytes(data, vendor_pat.as_bytes())?;
            let vsec = &data[vpos + vendor_pat.len()..];
            let send = vsec.windows(2)
                .enumerate()
                .find(|(_, w)| w[0] == b'\n' && w[1] != b'\t' && w[1] != b'#' && w[1] != b'\n')
                .map(|(i, _)| i + 1)
                .unwrap_or(vsec.len());
            let vbody = &vsec[..send];
            let dpos = memmem_bytes(vbody, device_pat.as_bytes())?;
            let after = &vbody[dpos + device_pat.len()..];
            // prefer bracket model name "[RTX 4070]" style
            let name = if let Some(lb) = after.iter().position(|&b| b == b'[') {
                let rb = after[lb..].iter().position(|&b| b == b']').map(|r| lb + r);
                if let Some(rb) = rb {
                    std::str::from_utf8(&after[lb+1..rb]).ok()?.trim()
                } else {
                    let end = after.iter().position(|&b| b == b'\n').unwrap_or(after.len());
                    std::str::from_utf8(&after[..end]).ok()?.trim()
                }
            } else {
                let end = after.iter().position(|&b| b == b'\n').unwrap_or(after.len());
                std::str::from_utf8(&after[..end]).ok()?.trim()
            };
            if name.is_empty() { return None; }
            Some(name.to_string())
        })();
        unsafe { libc::munmap(ptr as *mut libc::c_void, len); }
        if result.is_some() { return result; }
    }
    None
}

/// For AMD GPUs: try amdgpu.ids with device+revision for marketing name.
fn lookup_amd_marketing(did_raw: u32, rev_raw: u32) -> Option<String> {
    let (ptr, len) = unsafe { mmap_file("/usr/share/libdrm/amdgpu.ids") };
    if ptr == libc::MAP_FAILED as *const u8 { return None; }
    let data = unsafe { std::slice::from_raw_parts(ptr, len) };
    // Try "DDDD,\tRR," and "DDDD, RR," variants
    for sep in &[format!("{:04X},\t{:02X},", did_raw, rev_raw),
                 format!("{:04X}, {:02X},", did_raw, rev_raw)] {
        if let Some(pos) = memmem_bytes(data, sep.as_bytes()) {
            let after = &data[pos + sep.len()..];
            let skip = after.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(0);
            let after = &after[skip..];
            let end = after.iter().position(|&b| b == b'\n').unwrap_or(after.len());
            let name = std::str::from_utf8(&after[..end]).ok()?.trim();
            if !name.is_empty() {
                unsafe { libc::munmap(ptr as *mut libc::c_void, len); }
                return Some(name.to_string());
            }
        }
    }
    unsafe { libc::munmap(ptr as *mut libc::c_void, len); }
    None
}

fn detect_gpu() -> (String, String) {
    // 1. NVIDIA proprietary
    if let Ok(dir) = fs::read_dir("/proc/driver/nvidia/gpus") {
        for e in dir.flatten() {
            if let Ok(info) = fs::read_to_string(e.path().join("information")) {
                if let Some(l) = info.lines().find(|l| l.starts_with("Model:")) {
                    return (l[6..].trim().to_string(), String::new());
                }
            }
        }
    }

    // 2. DRM sysfs: vendor+device IDs → pci.ids / amdgpu.ids
    for i in 0..8u8 {
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

        let vendor_id = u32::from_str_radix(vendor.trim_start_matches("0x"), 16).unwrap_or(0);
        let device_id = u32::from_str_radix(device.trim_start_matches("0x"), 16).unwrap_or(0);
        let vid = format!("{:04x}", vendor_id);
        let did = format!("{:04x}", device_id);

        // AMD: try amdgpu.ids first for marketing name
        if vendor_id == 0x1002 {
            let rev_raw = fs::read_to_string(format!("{}/revision", base)).unwrap_or_default();
            let rev = u32::from_str_radix(rev_raw.trim().trim_start_matches("0x"), 16).unwrap_or(0);
            if let Some(name) = lookup_amd_marketing(device_id, rev) {
                let temp = collect_gpu_temp(i);
                return (name, temp);
            }
        }

        if let Some(name) = lookup_pci_name(&vid, &did) {
            let temp = collect_gpu_temp(i);
            return (name, temp);
        }

        // Fallback: vendor-prefixed device ID
        let vname = match vendor_id {
            0x1002 => "AMD",
            0x10de => "NVIDIA",
            0x8086 => "Intel",
            0x1414 => "Microsoft",
            _      => "GPU",
        };
        let temp = collect_gpu_temp(i);
        return (format!("{} [{:04X}]", vname, device_id), temp);
    }

    // 3. Fallback: driver name from uevent
    for i in 0..4u8 {
        let path = format!("/sys/class/drm/card{}/device/uevent", i);
        if let Ok(uevent) = fs::read_to_string(&path) {
            if let Some(p) = uevent.lines().find(|l| l.starts_with("DRIVER=")) {
                let driver = p.trim_start_matches("DRIVER=");
                if !driver.is_empty() { return (driver.to_string(), String::new()); }
            }
        }
    }

    ("Unknown".into(), String::new())
}

fn collect_gpu_temp(card: u8) -> String {
    let base = format!("/sys/class/drm/card{}/device/hwmon", card);
    if let Ok(hwmons) = fs::read_dir(&base) {
        for hw in hwmons.flatten() {
            if let Ok(raw) = fs::read_to_string(hw.path().join("temp1_input")) {
                if let Ok(mc) = raw.trim().parse::<u32>() {
                    return format!("{}°C", mc / 1000);
                }
            }
        }
    }
    "N/A".into()
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
//  Package counting — only runs if needs.pkgs
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
    let mut total = 0usize; let mut found = false;
    for dir in &["/var/lib/flatpak/app".to_string(),
                 format!("{}/.local/share/flatpak/app", home)] {
        if let Ok(d) = fs::read_dir(dir) { total += d.count(); found = true; }
    }
    if found && total > 0 { Some(total) } else { None }
}

fn count_snap() -> Option<usize> {
    // /var/lib/snapd/snaps counts .snap files (matches bfetch)
    let d = fs::read_dir("/var/lib/snapd/snaps").ok()?;
    let n = d.flatten().filter(|e| {
        e.file_name().to_string_lossy().ends_with(".snap")
    }).count();
    if n > 0 { Some(n) } else { None }
}

fn count_dpkg() -> Option<usize> {
    let d = fs::read_dir("/var/lib/dpkg/info").ok()?;
    let n = d.flatten().filter(|e| {
        e.file_name().to_string_lossy().ends_with(".list")
    }).count();
    if n > 0 { Some(n) } else { None }
}

fn collect_packages() -> String {
    let mut parts: Vec<String> = Vec::with_capacity(5);
    if let Some(n) = count_pacman() { parts.push(format!("{} (pacman)", n)); }
    if let Some(n) = count_dpkg()   { parts.push(format!("{} (dpkg)",   n)); }
    if let Some(n) = count_nix()    { parts.push(format!("{} (nix)",    n)); }
    if let Some(n) = count_flatpak(){ parts.push(format!("{} (flatpak)",n)); }
    if let Some(n) = count_snap()   { parts.push(format!("{} (snap)",   n)); }
    if parts.is_empty() { "unknown".into() } else { parts.join(", ") }
}

// ──────────────────────────────────────────────────────────────────────────────
//  Other collectors — only called when needed
// ──────────────────────────────────────────────────────────────────────────────

#[inline]

fn read_uptime() -> (u64, String) {
    // clock_gettime(CLOCK_BOOTTIME) is a vDSO call — no file I/O, no syscall overhead.
    // CLOCK_BOOTTIME = 7 (includes time suspended, matches /proc/uptime semantics).
    let secs: u64 = {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) } == 0 {
            ts.tv_sec as u64
        } else {
            // Fallback: /proc/uptime
            let mut buf = [0u8; 32];
            let n = read_proc!("/proc/uptime", buf);
            std::str::from_utf8(&buf[..n]).unwrap_or("0")
                .split('.').next().and_then(|v| v.parse().ok()).unwrap_or(0)
        }
    };
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    let short = match (d, h, m) {
        (0, 0, m) => format!("{}m", m),
        (0, h, m) => format!("{}h {}m", h, m),
        (d, h, m) => format!("{}d {}h {}m", d, h, m),
    };
    (secs, short)
}

fn read_os_kernel(need_os: bool, need_kernel: bool) -> (String, String) {
    // Kernel: uname() is a vDSO call on Linux — no syscall overhead, ~50ns.
    // Replaces 3 syscalls (open+read+close of /proc/version).
    let kernel = if need_kernel {
        let mut u: libc::utsname = unsafe { std::mem::zeroed() };
        if unsafe { libc::uname(&mut u) } == 0 {
            let ptr = u.release.as_ptr() as *const libc::c_char;
            unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
        } else { "unknown".into() }
    } else { String::new() };

    // OS name: fast-path for common distros via a single access(2) / stat(2)
    // before falling back to the NTFS-bridged /etc/os-release on WSL2.
    let os = if need_os {
        // Arch Linux — /etc/arch-release exists, no read needed
        if unsafe { libc::access(b"/etc/arch-release\0".as_ptr() as *const libc::c_char,
                                 libc::F_OK) } == 0 {
            "Arch Linux".into()
        // Debian/Ubuntu — check /etc/debian_version
        } else if unsafe { libc::access(b"/etc/debian_version\0".as_ptr() as *const libc::c_char,
                                        libc::F_OK) } == 0 {
            let mut buf = [0u8; 256];
            let n = read_proc!("/etc/debian_version", buf);
            let ver = std::str::from_utf8(&buf[..n]).unwrap_or("").trim();
            // Check /etc/os-release for PRETTY_NAME to distinguish Ubuntu vs Debian
            let mut osbuf = [0u8; 2048];
            let on = read_proc!("/etc/os-release", osbuf);
            let ostext = std::str::from_utf8(&osbuf[..on]).unwrap_or("");
            ostext.lines()
                .find(|l| l.starts_with("PRETTY_NAME="))
                .map(|l| l[12..].trim_matches('"').to_string())
                .unwrap_or_else(|| format!("Debian {}", ver))
        // Fedora
        } else if unsafe { libc::access(b"/etc/fedora-release\0".as_ptr() as *const libc::c_char,
                                        libc::F_OK) } == 0 {
            let mut buf = [0u8; 256];
            let n = read_proc!("/etc/fedora-release", buf);
            std::str::from_utf8(&buf[..n]).unwrap_or("Fedora").trim()
                .lines().next().unwrap_or("Fedora").to_string()
        // Generic fallback: read /etc/os-release
        } else {
            let mut buf = [0u8; 2048];
            let n = read_proc!("/etc/os-release", buf);
            let text = std::str::from_utf8(&buf[..n]).unwrap_or("");
            text.lines()
                .find(|l| l.starts_with("PRETTY_NAME="))
                .map(|l| l[12..].trim_matches('"').to_string())
                .unwrap_or_else(|| "Linux".into())
        }
    } else { String::new() };

    (os, kernel)
}

fn read_resolution() -> String {
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
        let blk = st.f_frsize as u64;
        let total = st.f_blocks as u64 * blk;
        let avail = st.f_bavail as u64 * blk;
        let gb = 1_073_741_824.0f64;
        format!("{:.1}G / {:.1}G", total.saturating_sub(avail) as f64 / gb, total as f64 / gb)
    } else { "unknown".into() }
}

fn read_load() -> String {
    let mut buf = [0u8; 64];
    let n = read_proc!("/proc/loadavg", buf);
    std::str::from_utf8(&buf[..n]).unwrap_or("")
        .split_ascii_whitespace().take(3).collect::<Vec<_>>().join("  ")
}

fn read_de_wm() -> String {
         if let Ok(v) = env::var("XDG_CURRENT_DESKTOP")     { v }
    else if let Ok(v) = env::var("DESKTOP_SESSION")         { v }
    else if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() { "Hyprland".into() }
    else if env::var("SWAYSOCK").is_ok()                    { "Sway".into()     }
    else if env::var("WAYLAND_DISPLAY").is_ok()             { "Wayland".into()  }
    else if env::var("DISPLAY").is_ok()                     { "X11".into()      }
    else                                                     { "TTY".into()      }
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
                    if let Some(p) = t.strip_prefix("|-- ").or_else(|| t.strip_prefix("+-- ")) {
                        let s = p.trim();
                        if s != "127.0.0.1" && !s.starts_with("127.") && s != "0.0.0.0" && s.contains('.') {
                            found = s.to_string(); break;
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
                                if let Ok(p) = u16::from_str_radix(hex, 16) {
                                    if !open.contains(&p) { open.push(p); }
                                }
                            }
                        }
                    }
                }
            }
        }
        open.sort_unstable(); open.truncate(8);
        if open.is_empty() { "none".into() }
        else { open.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ") }
    };

    (ip, ssh, ports)
}

// ──────────────────────────────────────────────────────────────────────────────
//  Main entry point — needs-driven collection
// ──────────────────────────────────────────────────────────────────────────────

pub fn collect_all(needs: &CollectNeeds) -> SysInfo {
    // ── Spawn threads ONLY for genuinely slow real-FS collectors ──────────
    // Packages: may scan hundreds of /var/lib/pacman/local entries
    let t_pkgs = if needs.pkgs {
        Some(thread::spawn(collect_packages))
    } else { None };

    // GPU: DRM scan + pci.ids mmap (fast, but keep parallel for pipelining)
    let t_gpu = if needs.gpu {
        Some(thread::spawn(detect_gpu))
    } else { None };

    // ── Fast single-syscall reads on the main thread ───────────────────────
    let (os, kernel)          = read_os_kernel(needs.os, needs.kernel);
    let (uptime_secs, uptime) = if needs.uptime { read_uptime() } else { (0, String::new()) };
    let (mem_used, mem_total) = if needs.memory { read_memory() } else { (0, 0) };

    let cpu     = if needs.cpu     { collect_cpu()       } else { String::new() };
    let disk    = if needs.disk    { read_disk()         } else { String::new() };
    let load    = if needs.load    { read_load()         } else { String::new() };
    let res     = if needs.res     { read_resolution()   } else { String::new() };
    let battery = if needs.battery { read_battery()      } else { String::new() };

    // Zero-I/O: environment variables
    let shell  = if needs.shell  {
        env::var("SHELL").unwrap_or_default()
            .rsplit('/').next().unwrap_or("sh").to_string()
    } else { String::new() };
    let term   = if needs.term   {
        env::var("TERM_PROGRAM").or_else(|_| env::var("TERM")).unwrap_or_else(|_| "unknown".into())
    } else { String::new() };
    let locale = if needs.locale {
        env::var("LANG").unwrap_or_else(|_| "C".into())
    } else { String::new() };
    let de_wm  = if needs.de_wm  { read_de_wm() } else { String::new() };

    let (ip, ssh, ports) = if needs.network { collect_network() }
                           else { (String::new(), String::new(), String::new()) };

    // ── Join threads ────────────────────────────────────────────────────────
    let pkgs = t_pkgs.map(|t| t.join().unwrap_or_else(|_| "unknown".into()))
                     .unwrap_or_default();

    let (gpu, gpu_temp) = t_gpu
        .map(|t| t.join().unwrap_or_else(|_| ("Unknown".into(), String::new())))
        .unwrap_or_default();

    SysInfo {
        os, kernel, uptime_secs, uptime, res, pkgs, shell, de_wm, term,
        cpu, gpu, gpu_temp, battery, disk, load, locale,
        mem_used, mem_total, ip, ssh, ports,
    }
}
