use std::{env, fs, path::Path};

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
    pub battery_key: String,
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
    pub os_version:  String,
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
    pub display_key:   String,
}

// ── Low-level helpers ────────────────────────────────────

#[inline]
fn slurp(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }

/// Run a command, returning trimmed stdout on success.
#[allow(dead_code)]
fn command(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    if !out.status.success() { return None; }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Version hint exported by some shells (ZSH_VERSION is not exported by
/// default), so the fastfetch `shell` row can skip spawning `--version`.
fn shell_version_env(name: &str) -> Option<String> {
    let var = match name {
        "zsh"  => "ZSH_VERSION",
        "bash" => "BASH_VERSION",
        "fish" => "FISH_VERSION",
        "nu"   => "NU_VERSION",
        _      => return None,
    };
    env::var(var).ok().filter(|v| !v.is_empty())
}

/// First parseable value from a list of sysctl keys.
#[allow(dead_code)]
fn sysctl_u64(keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|k| sysctl(k)?.trim().parse::<u64>().ok())
}

/// `uname -r` equivalent, shared by the BSD OS-name fallback.
#[allow(dead_code)]
fn uname_release() -> String {
    let mut uts = std::mem::MaybeUninit::<libc::utsname>::uninit();
    // SAFETY: `uname` fully initializes `uts` when it returns 0.
    if unsafe { libc::uname(uts.as_mut_ptr()) } == 0 {
        // SAFETY: initialized by the successful call above.
        let uts = unsafe { uts.assume_init() };
        // SAFETY: `release` is a NUL-terminated C string.
        unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    }
}

#[cfg(target_os = "macos")]      const NATIVE_OS_ID: &str = "macos";
#[cfg(target_os = "freebsd")]    const NATIVE_OS_ID: &str = "freebsd";
#[cfg(target_os = "openbsd")]    const NATIVE_OS_ID: &str = "openbsd";
#[cfg(target_os = "netbsd")]     const NATIVE_OS_ID: &str = "netbsd";
#[cfg(target_os = "dragonfly")]  const NATIVE_OS_ID: &str = "dragonfly";
#[cfg(target_os = "android")]    const NATIVE_OS_ID: &str = "android";
#[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd",
              target_os = "netbsd", target_os = "dragonfly", target_os = "android")))]
const NATIVE_OS_ID: &str = "linux";

#[cfg(target_os = "macos")]      const NATIVE_INIT: &str = "launchd";
#[cfg(target_os = "android")]    const NATIVE_INIT: &str = "init";
#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
          target_os = "dragonfly"))]
const NATIVE_INIT: &str = "init";
#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "freebsd",
              target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly")))]
const NATIVE_INIT: &str = "unknown";

/// Human-readable OS name when `/etc/os-release` is absent.
#[cfg(target_os = "macos")]
fn native_os() -> String {
    let name = command("sw_vers", &["-productName"]).unwrap_or_else(|| "macOS".into());
    match command("sw_vers", &["-productVersion"]) {
        Some(v) if !v.is_empty() => format!("{name} {v}"),
        _ => name,
    }
}

#[cfg(target_os = "android")]
fn native_os() -> String {
    let version = command("getprop", &["ro.build.version.release"])
        .or_else(|| command("/system/bin/getprop", &["ro.build.version.release"]));
    match version {
        Some(v) if !v.is_empty() => format!("Android {v}"),
        _ => "Android".into(),
    }
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
          target_os = "dragonfly"))]
fn native_os() -> String {
    let name = if cfg!(target_os = "freebsd") { "FreeBSD" }
        else if cfg!(target_os = "openbsd") { "OpenBSD" }
        else if cfg!(target_os = "netbsd") { "NetBSD" }
        else { "DragonFly" };
    let release = uname_release();
    if release.is_empty() { name.into() } else { format!("{name} {release}") }
}

#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "freebsd",
              target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly")))]
fn native_os() -> String { "Linux".into() }

/// Memory + swap in KiB: `(total, available, swap_total, swap_free)`.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn memory_info() -> (u64, u64, u64, u64) {
    let raw = slurp("/proc/meminfo");
    let (mut total, mut avail) = (0u64, 0u64);
    let (mut swap_total, mut swap_free) = (0u64, 0u64);
    for line in raw.lines() {
        let Some((key, rest)) = line.split_once(':') else { continue };
        let value = rest.split_ascii_whitespace().next()
            .and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        match key {
            "MemTotal"     => total = value,
            "MemAvailable" => avail = value,
            "SwapTotal"    => swap_total = value,
            "SwapFree"     => swap_free = value,
            _ => {}
        }
    }
    (total, avail, swap_total, swap_free)
}

/// macOS/BSD memory via sysctl (`hw.memsize`/`hw.physmem`, VM page counts).
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn memory_info() -> (u64, u64, u64, u64) {
    let total_bytes = sysctl_u64(&[
        "hw.memsize", "hw.physmem", "hw.physmem64", "hw.realmem",
        "hw.realmem64", "hw.usermem",
    ]).unwrap_or(0);
    let page = sysctl_u64(&["hw.pagesize"]).unwrap_or(4096);
    let (mut free, mut inactive) = (
        sysctl_u64(&["vm.page_free_count", "vm.stats.vm.v_free_count", "vm.uvmexp.free"]).unwrap_or(0),
        sysctl_u64(&[
            "vm.page_inactive_count", "vm.stats.vm.v_inactive_count",
            "vm.stats.vm.v_cache_count", "vm.uvmexp.inactive",
        ]).unwrap_or(0),
    );
    if free + inactive == 0 {
        if let Some((f, i)) = vmstat_pages() { free = f; inactive = i; }
    }
    let total = total_bytes / 1024;
    let avail = ((free + inactive).saturating_mul(page) / 1024).min(total);
    let (swap_total, swap_used) = swap_usage();
    (total, avail, swap_total, swap_total.saturating_sub(swap_used))
}

/// Free/inactive page counts from `vmstat -s` on BSDs whose sysctl names differ.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn vmstat_pages() -> Option<(u64, u64)> {
    let raw = command("vmstat", &["-s"])?;
    let (mut free, mut inactive) = (0u64, 0u64);
    for line in raw.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 { continue; }
        let Ok(n) = f[0].parse::<u64>() else { continue };
        match (f[1], f[2]) {
            ("pages", "free") => free = n,
            ("pages", "inactive") => inactive = n,
            _ => {}
        }
    }
    (free > 0).then_some((free, inactive))
}

/// Parse `2048.00M`, `512K`, `2G` into KiB.
#[allow(dead_code)]
fn parse_kib(s: &str) -> u64 {
    let s = s.trim().trim_end_matches(|c: char| c == 'B' || c == 'b');
    let (num, unit) = match s.chars().last() {
        Some(c) if c.is_ascii_alphabetic() => (&s[..s.len() - 1], c.to_ascii_uppercase()),
        _ => (s, 'B'),
    };
    let n: f64 = num.parse().unwrap_or(0.0);
    let mult = match unit { 'K' => 1.0, 'M' => 1024.0, 'G' => 1_048_576.0, 'T' => 1_073_741_824.0, _ => 1.0 / 1024.0 };
    (n * mult) as u64
}

#[cfg(target_os = "macos")]
fn swap_usage() -> (u64, u64) {
    let raw = sysctl("vm.swapusage").unwrap_or_default();
    let toks: Vec<&str> = raw.split_whitespace().collect();
    let value = |key: &str| toks.iter().position(|t| *t == key)
        .and_then(|i| toks.get(i + 2))
        .map(|v| parse_kib(v)).unwrap_or(0);
    (value("total"), value("used"))
}

#[cfg(target_os = "freebsd")]
fn swap_usage() -> (u64, u64) {
    let raw = command("swapinfo", &["-k"]).unwrap_or_default();
    let mut total = 0u64;
    let mut used = 0u64;
    for line in raw.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() >= 3 && f[0].eq_ignore_ascii_case("total") {
            total = f[1].parse().unwrap_or(0);
            used = f[2].parse().unwrap_or(0);
        }
    }
    (total, used)
}

#[cfg(not(any(target_os = "macos", target_os = "freebsd")))]
#[allow(dead_code)]
fn swap_usage() -> (u64, u64) { (0, 0) }

/// Load averages from `/proc/loadavg`, else `sysctl vm.loadavg`.
fn load_average() -> String {
    let raw = slurp("/proc/loadavg");
    let raw = if raw.trim().is_empty() {
        sysctl("vm.loadavg").unwrap_or_default()
    } else { raw };
    raw.split_ascii_whitespace()
        .filter(|v| v.parse::<f64>().is_ok())
        .take(3)
        .collect::<Vec<_>>()
        .join("  ")
}

/// Process count via `ps` when `/proc` is unavailable.
#[allow(dead_code)]
fn platform_process_count() -> usize {
    command("ps", &["-ax", "-o", "pid="]).map(|out| {
        out.lines().filter(|l| l.trim().parse::<u32>().is_ok()).count()
    }).unwrap_or(0)
}

/// GPU name from the platform's own tool (macOS/FreeBSD).
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn platform_gpu() -> Option<String> {
    // Apple Silicon: the GPU is the SoC, no need to run system_profiler.
    let brand = sysctl("machdep.cpu.brand_string")
        .or_else(|| sysctl("hw.model"))?;
    Some(format!("{brand} GPU"))
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn platform_gpu() -> Option<String> {
    let out = crate::modules::run_timeout("system_profiler", &["SPDisplaysDataType"], 5000)?;
    for line in out.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("Chipset Model:").or_else(|| l.strip_prefix("Model:")) {
            let v = v.trim();
            if !v.is_empty() { return Some(v.to_string()); }
        }
    }
    None
}

#[cfg(target_os = "freebsd")]
fn platform_gpu() -> Option<String> {
    let out = crate::modules::run_timeout("pciconf", &["-lv"], 1500)?;
    let mut display = false;
    let mut vendor = String::new();
    for line in out.lines() {
        if !line.starts_with(char::is_whitespace) {
            display = line.contains("class=0x0300");
            vendor.clear();
            continue;
        }
        if !display { continue; }
        let l = line.trim();
        let value = l.split_once('=')
            .map(|(_, v)| v.trim().trim_matches('\''))
            .unwrap_or("");
        if l.starts_with("vendor") {
            vendor = value.to_string();
        } else if l.starts_with("device") {
            let name = format!("{} {}", vendor, value).trim().to_string();
            if !name.is_empty() { return Some(name); }
        }
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "freebsd")))]
fn platform_gpu() -> Option<String> { None }

/// Battery percentage from `pmset` (macOS) or `acpiconf` (FreeBSD).
#[cfg(target_os = "macos")]
fn platform_battery() -> Option<String> {
    let out = command("pmset", &["-g", "batt"])?;
    let cap = out.split_whitespace().find(|w| w.ends_with('%'))?.to_string();
    let icon = if out.contains("charging") { "↑" }
        else if out.contains("discharging") { "↓" } else { "" };
    Some(format!("{cap} {icon}").trim().to_string())
}

#[cfg(target_os = "freebsd")]
fn platform_battery() -> Option<String> {
    let out = command("acpiconf", &["-i", "0"])?;
    let cap = out.lines()
        .find(|l| l.trim().starts_with("Remaining capacity:"))
        .and_then(|l| l.split_whitespace().find(|w| w.ends_with('%')))?
        .to_string();
    let low = out.to_ascii_lowercase();
    let icon = if low.contains("charging") { "↑" }
        else if low.contains("discharging") { "↓" } else { "" };
    Some(format!("{cap} {icon}").trim().to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "freebsd")))]
fn platform_battery() -> Option<String> { None }

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
#[cfg(any(target_os = "linux", target_os = "android"))]
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

/// Seconds since boot from `kern.boottime` (macOS/BSD), monotonic fallback.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn system_uptime_secs() -> u64 {
    let boot = sysctl("kern.boottime").and_then(|raw| {
        let tail = raw.split("sec").nth(1)?;
        tail.split(|c: char| !c.is_ascii_digit())
            .find(|s| !s.is_empty())
            .and_then(|s| s.parse::<u64>().ok())
    });
    if let Some(boot) = boot {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > boot { return now - boot; }
    }
    let mut ts = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: `clock_gettime` initializes `ts` on success.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, ts.as_mut_ptr()) } == 0 {
        // SAFETY: initialized by the successful call above.
        unsafe { ts.assume_init() }.tv_sec.max(0) as u64
    } else {
        0
    }
}

/// Run `sysctl -n <key>`; used by the macOS/BSD fallbacks.
#[allow(dead_code)]
fn sysctl(key: &str) -> Option<String> {
    for program in ["sysctl", "/usr/sbin/sysctl", "/sbin/sysctl"] {
        let Ok(out) = std::process::Command::new(program)
            .args(["-n", key])
            .output()
        else { continue; };
        if !out.status.success() { continue; }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() { return Some(s); }
    }
    None
}

/// CPU brand string via CPUID (~1µs) — just the name, no frequency/cores
#[cfg(target_arch = "x86_64")]
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
        for kw in &["with Radeon Graphics", "with Graphics"] {
            if let Some(pos) = s.find(kw) {
                s.truncate(pos);
            }
        }
        s.trim().to_string()
    } else {
        "Unknown".into()
    }
}

/// CPU brand on non-x86_64: `/proc/cpuinfo`, then sysctl, then uname.
#[cfg(not(target_arch = "x86_64"))]
fn cpu_brand() -> String {
    if let Ok(raw) = fs::read_to_string("/proc/cpuinfo") {
        for line in raw.lines() {
            let Some((key, value)) = line.split_once(':') else { continue };
            if matches!(key.trim(), "model name" | "Hardware" | "Processor") {
                let v = value.trim();
                if !v.is_empty() { return v.to_string(); }
            }
        }
    }
    for key in ["machdep.cpu.brand_string", "hw.model"] {
        if let Some(v) = sysctl(key) {
            return v;
        }
    }
    let mut uts = std::mem::MaybeUninit::<libc::utsname>::uninit();
    // SAFETY: `uname` fully initializes `uts` when it returns 0.
    if unsafe { libc::uname(uts.as_mut_ptr()) } == 0 {
        // SAFETY: initialized by the successful call above.
        let uts = unsafe { uts.assume_init() };
        // SAFETY: `machine` is a NUL-terminated C string.
        let machine = unsafe { std::ffi::CStr::from_ptr(uts.machine.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if !machine.is_empty() { return machine; }
    }
    "Unknown".into()
}

/// Integrated/Discrete marker for fastfetch's GPU row. Sysfs vendor IDs are
/// stable; a small AMD VRAM pool means an APU (shared GTT memory).
pub(crate) fn gpu_type() -> Option<&'static str> {
    for i in 0..4u8 {
        let base = format!("/sys/class/drm/card{}/device", i);
        let vendor = slurp(&format!("{base}/vendor")).trim().to_ascii_lowercase();
        match vendor.as_str() {
            "0x8086" => return Some("Integrated"),
            "0x10de" => return Some("Discrete"),
            "0x1002" => {
                let vram = slurp(&format!("{base}/mem_info_vram_total"))
                    .trim().parse::<u64>().unwrap_or(0);
                if vram > 0 {
                    return Some(if vram <= 1 << 30 { "Integrated" } else { "Discrete" });
                }
            }
            _ => {}
        }
    }
    None
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
    if let Some(name) = platform_gpu() { return name; }
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
pub fn collect_all(need_network: bool, show: &crate::config::Show, fastfetch: bool) -> SysInfo {
    // ── fast sequential: all reads on main thread ──────────

    // memory + swap — one shared read (`/proc/meminfo` or sysctl)
    let (mem_used, mem_total, swap) = if show.memory || show.swap {
        let (mem_total_kib, mem_avail_kib, swap_total_kib, swap_free_kib) = memory_info();

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
        // Fastfetch shows the CPU's maximum clock. One sysfs read is cheap
        // enough to do live, and avoids a stale 24 h cache after a boost change.
        let freq = slurp("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq");
        let ghz = freq.trim().parse::<f64>().ok().map(|khz| khz / 1_000_000.0);
        if fastfetch {
            match ghz {
                Some(ghz) if ghz > 0.1 => format!("{} ({}) @ {:.2} GHz", brand, cores, ghz),
                _ => format!("{} ({})", brand, cores),
            }
        } else if let Some(ghz) = ghz {
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
    let os_raw = Some(slurp("/etc/os-release"));
    let os_id = os_raw.as_deref()
        .and_then(|r| r.lines().find(|l| l.starts_with("ID=")))
        .map(|l| l[3..].trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| NATIVE_OS_ID.to_string());
    let os_id_like = os_raw.as_deref()
        .and_then(|r| r.lines().find(|l| l.starts_with("ID_LIKE=")))
        .map(|l| l[8..].trim().trim_matches('"').to_string())
        .unwrap_or_default();
    let os_version = os_raw.as_deref()
        .and_then(|r| r.lines().find(|l| l.starts_with("VERSION_ID="))
            .or_else(|| r.lines().find(|l| l.starts_with("VERSION="))))
        .and_then(|l| l.split_once('='))
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
        .unwrap_or_default();

    let os = if show.os {
        crate::cache::get_or_compute("os", 86400, || {
            os_raw.as_deref()
                .and_then(|r| r.lines().find(|l| l.starts_with("PRETTY_NAME=")))
                .map(|l| l[12..].trim_matches('"').to_string())
                .or_else(|| os_raw.as_deref()
                    .and_then(|r| r.lines().find(|l| l.starts_with("NAME=")))
                    .map(|l| l[5..].trim_matches('"').to_string()))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(native_os)
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

    // battery — /sys/class/power_supply (Android uses `battery`)
    let (battery_key, battery) = if show.battery {
        if fastfetch {
            if let Some((key, value)) = modules::battery_fastfetch() {
                (key, value)
            } else {
                (String::new(), platform_battery().unwrap_or_else(|| "N/A".into()))
            }
        } else {
            let mut b = String::new();
            for bat in &["BAT0", "BAT1", "BAT", "battery"] {
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
            if b.is_empty() {
                (String::new(), platform_battery().unwrap_or_else(|| "N/A".into()))
            } else { (String::new(), b) }
        }
    } else { (String::new(), String::new()) };

    let load = if show.load {
        load_average()
    } else { String::new() };

    // disk — one statvfs syscall
    let disk = if show.disk {
        let mut st = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: `c"/"` is a valid NUL-terminated path; `statvfs` fills `st` on success.
        if unsafe { libc::statvfs(c"/".as_ptr(), st.as_mut_ptr()) } == 0 {
            // SAFETY: initialized by the successful call above.
            let st = unsafe { st.assume_init() };
            let blk   = st.f_frsize as u64;
            let total = st.f_blocks as u64 * blk;
            // Fastfetch counts reserved blocks as used; the compact arcfetch
            // default keeps showing what a normal user can actually use.
            let free  = (if fastfetch { st.f_bfree } else { st.f_bavail }) as u64 * blk;
            let gb    = 1_073_741_824.0f64;
            format!("{:.1}G / {:.1}G",
                total.saturating_sub(free) as f64 / gb, total as f64 / gb)
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
    let shell  = if show.shell  {
        let name = env::var("SHELL").ok()
            .and_then(|s| s.rsplit('/').next().map(str::to_string))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "sh".into());
        if fastfetch {
            // Fastfetch prints `name version`. Spawning a shell for `--version`
            // costs milliseconds on every prompt, so cache the answer and use
            // an exported version first (ZSH_VERSION is not exported by default).
            let ver = crate::cache::get_or_compute(&format!("shell_version_{name}"), 86400, || {
                shell_version_env(&name)
                    .or_else(|| command(&name, &["--version"]))
                    .and_then(|out| {
                        out.lines().next()?.split_whitespace()
                            .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
                            .map(|t| t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>())
                    })
                    .unwrap_or_default()
            });
            if ver.is_empty() { name } else { format!("{name} {ver}") }
        } else {
            name
        }
    } else { String::new() };
    let term   = if show.term   {
        // Fastfetch walks the process tree; herdr exposes itself in the env.
        if fastfetch && env::var_os("HERDR_ENV").is_some() {
            "herdr".into()
        } else {
            env::var("TERM_PROGRAM").or_else(|_| env::var("TERM")).unwrap_or_else(|_| "unknown".into())
        }
    } else { String::new() };
    let locale = if show.locale { env::var("LANG").unwrap_or_else(|_| "C".into()) } else { String::new() };
    let de_wm  = if show.de_wm {
             if let Ok(v) = env::var("XDG_CURRENT_DESKTOP")         { v }
        else if let Ok(v) = env::var("DESKTOP_SESSION")             { v }
        else if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()     { "Hyprland".into() }
        else if env::var("SWAYSOCK").is_ok()                        { "Sway".into() }
        else if env::var("WAYLAND_DISPLAY").is_ok()                 { "Wayland".into() }
        else if env::var("DISPLAY").is_ok()                         { "X11".into() }
        else if cfg!(target_os = "macos")                           { "Aqua".into() }
        else if cfg!(target_os = "android")                         { "Android".into() }
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
            _ if comm.is_empty() => NATIVE_INIT,
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
        if n == 0 { platform_process_count() } else { n }.to_string()
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
        let tcp = ["/proc/net/tcp", "/proc/net/tcp6"].iter()
            .filter_map(|p| fs::read_to_string(p).ok())
            .collect::<Vec<_>>()
            .join("\n");
        // SSH: port 22 (0x0016) in LISTEN (0A)
        let ssh = {
            let up = tcp.lines().skip(1).any(|l| {
                let c: Vec<&str> = l.split_whitespace().collect();
                c.get(1).map(|a| a.ends_with(":0016")).unwrap_or(false)
                    && c.get(3).map(|s| *s == "0A").unwrap_or(false)
            });
            if up { "active".into() } else { "inactive".into() }
        };
        // ports — count listening TCP ports
        let ports = tcp.lines().skip(1)
            .filter(|l| l.split_whitespace().nth(3).map(|s| s == "0A").unwrap_or(false))
            .count().to_string();
        (ip, ssh, ports)
    } else {
        ("N/A".into(), "inactive".into(), "0".into())
    };

    // ── GPU — cached 24h, DRM-first probe ────────────────
    let gpu = if show.gpu || show.gpu_temp {
        let mut g = crate::cache::get_or_compute("gpu", 86400, gpu_probe);
        if fastfetch && show.gpu && !g.is_empty() {
            if let Some(t) = gpu_type() {
                g.push_str(&format!(" [{t}]"));
            }
        }
        g
    } else { String::new() };

    // ── sequential pkgs ─────────────────────────────────
    let pkgs = if show.pkgs {
        let fastfetch = fastfetch;
        let key = if fastfetch { "pkgs-all" } else { "pkgs" };
        crate::cache::get_or_compute(key, 3600, move || {
            if fastfetch { crate::pkgs::count_all() } else { crate::pkgs::count() }
        })
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
        else if cfg!(target_os = "macos")                                { "CoreAudio".into() }
        else if cfg!(target_os = "android")                              { "AudioFlinger".into() }
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
        // Whole disks (nvme0n1, md0) are already in /sys/block. Otherwise
        // strip only the partition suffix: nvme0n1p3 -> nvme0n1, sda2 -> sda.
        let base_dev = if Path::new(&format!("/sys/block/{dev_name}")).exists() {
            dev_name.to_string()
        } else {
            let trimmed = dev_name.trim_end_matches(|c: char| c.is_ascii_digit());
            let with_p = trimmed.strip_suffix('p').unwrap_or(trimmed);
            if ["nvme", "mmcblk", "pmem", "md"].iter().any(|p| dev_name.starts_with(p)) {
                with_p.to_string()
            } else {
                trimmed.to_string()
            }
        };
        let rot_path = format!("/sys/block/{base_dev}/queue/rotational");
        match slurp(&rot_path).trim() {
            "0" => {
                if base_dev.starts_with("nvme") { "NVMe SSD".into() }
                else { "SSD".into() }
            }
            "1" => "HDD".into(),
            _   => "N/A".into(),
        }
    } else { String::new() };

    // pkg_updates — pending pacman updates, cached for an hour. It runs with
    // the other slow collectors below, so its `pacman -Qu` does not add to the
    // warm `--all` latency.

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
            "systemd" => "systemd".into(),
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
    let cursor        = if show.cursor        { modules::cursor() } else { String::new() };
    let term_size     = if show.term_size     { modules::term_size() } else { String::new() };
    let camera        = if show.camera        { modules::camera() } else { String::new() };
    let mouse         = if show.mouse         { modules::mouse() } else { String::new() };
    let keyboard      = if show.keyboard      { modules::keyboard() } else { String::new() };
    let tpm           = if show.tpm           { modules::tpm() } else { String::new() };
    let (display_key, monitor) = if show.monitor {
        if fastfetch {
            modules::display_fastfetch()
        } else {
            (String::new(), modules::monitor())
        }
    } else {
        (String::new(), String::new())
    };

    // The slow collectors — subprocess spawns (`glxinfo`, `playerctl`, `vainfo`,
    // `hciconfig`, …), network fetches and `/proc` walks — run concurrently, so
    // `--all` costs ~max(collector) instead of the sum. Results are cached.
    let cpu_start = std::time::Instant::now();
    let cpu0      = show.cpu_usage.then(modules::cpu_snapshot);
    let (pkg_updates, wallpaper, media, public_ip, weather, opengl, vulkan, codec,
         bluetooth, physical_disk, top_processes, term_theme, de_wm_ver, wifi_h) =
        std::thread::scope(|s| {
        let pkg_updates = show.pkg_updates.then(|| s.spawn(|| {
            if Path::new("/var/lib/pacman/sync").is_dir() {
                crate::cache::get_or_compute_ttl("pkg_updates", 3600, || {
                    match modules::run_timeout("pacman", &["-Qu"], 1500) {
                        Some(s) => s.lines().filter(|l| !l.trim().is_empty()).count().to_string(),
                        None => "N/A".into(),
                    }
                })
            } else { "N/A".into() }
        }));
        let wallpaper = show.wallpaper.then(|| s.spawn(|| {
            crate::cache::get_or_compute_ttl("wallpaper", 60, modules::wallpaper)
        }));
        let media     = show.media.then(|| s.spawn(|| {
            crate::cache::get_or_compute_ttl("media", 15, modules::media)
        }));
        let wifi_h    = show.wifi.then(|| s.spawn(|| {
            crate::cache::get_or_compute_ttl("wifi", 3, modules::wifi)
        }));
        let public_ip = show.public_ip.then(|| s.spawn(|| {
            crate::cache::get_or_compute_swr("public_ip", 3600, net::public_ip)
        }));
        let weather   = show.weather.then(|| s.spawn(|| {
            crate::cache::get_or_compute_swr("weather", 900, || net::weather(""))
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
            pkg_updates.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
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
            wifi_h.map(|h| h.join().unwrap_or_default()).unwrap_or_default(),
        )
    });

    let wifi = if show.wifi { wifi_h } else { String::new() };

    // CPU usage spans the work above; only pad the window if it finished fast.
    let cpu_usage = match cpu0 {
        None => String::new(),
        Some(None) => "N/A".into(),
        Some(Some((idle0, total0))) => {
            // 15 ms ≈ 1–2 USER_HZ ticks (HZ=100); rough but usable, and keeps
            // `--all` well under 30 ms instead of forcing a 60 ms stall.
            const MIN_MS: u64 = 15;
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
              cpu, gpu, gpu_temp, battery, battery_key, disk, load, locale, mem_used, mem_total,
              ip, ssh, ports,
              init, cpu_temp, processes, container, session,
              swap, sound, gpu_driver, bios, board, disk_type,
              pkg_updates, theme, icons, term_font, de_wm_ver, init_ver, local_ip,
              os_id, os_id_like, os_version,
              datetime, editor, dns, host, chassis, bootmgr, cpu_cache, login_manager,
              brightness, cpu_usage, diskio, netio, physmem, gpu_mem, btrfs, wifi,
              users, wallpaper, media, public_ip, weather, opengl, vulkan,
              cursor, term_size, term_theme, codec, bluetooth, camera, mouse,
              keyboard, tpm, physical_disk, top_processes, monitor, display_key }
}

#[cfg(test)]
mod platform_tests {
    use super::parse_kib;

    #[test]
    fn sysctl_sizes_convert_to_kib() {
        assert_eq!(parse_kib("512K"), 512);
        assert_eq!(parse_kib("2M"), 2048);
        assert_eq!(parse_kib("1.5G"), 1_572_864);
        assert_eq!(parse_kib("1024"), 1); // bare bytes
    }
}


