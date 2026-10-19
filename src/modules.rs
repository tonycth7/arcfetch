//! Opt-in extra sysinfo collectors (off by default).
//!
//! Self-contained: only `std` + `libc`, no `crate::` references. Every public
//! function returns a human-readable `String` or `"N/A"` when the data cannot
//! be determined. Nothing here is allowed to panic.

use std::{env, fs, path::Path, process::Command, thread, time::Duration};

// ── Small helpers ────────────────────────────────────────

/// Read a file to a trimmed `String`, empty on any error.
#[allow(dead_code)]
fn read_str(p: &str) -> String {
    fs::read_to_string(p)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Read a path to a trimmed `String`, `None` on any error.
#[allow(dead_code)]
fn read_path(p: &Path) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// Little-endian u16 with bounds checking.
#[allow(dead_code)]
fn le_u16(d: &[u8], off: usize) -> u16 {
    match (d.get(off), d.get(off + 1)) {
        (Some(a), Some(b)) => u16::from_le_bytes([*a, *b]),
        _ => 0,
    }
}

/// Little-endian u32 with bounds checking.
#[allow(dead_code)]
fn le_u32(d: &[u8], off: usize) -> u32 {
    match (d.get(off), d.get(off + 1), d.get(off + 2), d.get(off + 3)) {
        (Some(a), Some(b), Some(c), Some(e)) => u32::from_le_bytes([*a, *b, *c, *e]),
        _ => 0,
    }
}

/// Human byte count: `512B`, `1.2G`, `345M`, …
#[allow(dead_code)]
fn human(bytes: u64) -> String {
    const U: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut v = bytes as f64;
    let mut i = 0usize;
    while v >= 1024.0 && i + 1 < U.len() {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{}B", bytes)
    } else if v >= 10.0 {
        format!("{:.0}{}", v, U[i])
    } else {
        format!("{:.1}{}", v, U[i])
    }
}

/// Normalize a cache-size string to the largest whole unit, e.g. `16384K` → `16M`.
#[allow(dead_code)]
fn norm_cache_size(s: &str) -> String {
    let s = s.trim();
    let (num, unit) = match s.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, "K"),
    };
    let n: u64 = match num.trim().parse() {
        Ok(n) => n,
        Err(_) => return s.to_string(),
    };
    let mult: u64 = match unit.chars().next() {
        Some('K') | Some('k') => 1024,
        Some('M') | Some('m') => 1024 * 1024,
        Some('G') | Some('g') => 1024 * 1024 * 1024,
        _ => 1,
    };
    let bytes = n.saturating_mul(mult);
    if bytes == 0 {
        return "0K".into();
    }
    if bytes.is_multiple_of(1024 * 1024 * 1024) {
        format!("{}G", bytes / (1024 * 1024 * 1024))
    } else if bytes.is_multiple_of(1024 * 1024) {
        format!("{}M", bytes / (1024 * 1024))
    } else if bytes.is_multiple_of(1024) {
        format!("{}K", bytes / 1024)
    } else {
        format!("{}K", n)
    }
}

/// NUL-terminated C char array → trimmed `String`.
#[allow(dead_code)]
fn cstr(buf: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// `$HOME`, if set and non-empty.
#[allow(dead_code)]
fn home() -> Option<String> {
    env::var("HOME").ok().filter(|s| !s.is_empty())
}

/// Run a command, killing it if it exceeds `ms`. Returns its stdout as text.
fn run_timeout(program: &str, args: &[&str], ms: u64) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + Duration::from_millis(ms);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child.wait_with_output().ok()?;
                return Some(String::from_utf8_lossy(&out.stdout).into_owned());
            }
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => return None,
        }
    }
}

// ── Collectors ───────────────────────────────────────────

/// Local time as `YYYY-MM-DD HH:MM:SS` via pure libc.
#[allow(dead_code)]
pub fn datetime() -> String {
    unsafe {
        let mut t: libc::time_t = 0;
        if libc::time(&mut t) == -1 {
            return "N/A".into();
        }
        let mut tm: libc::tm = core::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return "N/A".into();
        }
        let mut buf = [0u8; 64];
        let fmt = b"%Y-%m-%d %H:%M:%S\0";
        let n = libc::strftime(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            fmt.as_ptr() as *const libc::c_char,
            &tm,
        );
        if n == 0 {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                tm.tm_year + 1900,
                tm.tm_mon + 1,
                tm.tm_mday,
                tm.tm_hour,
                tm.tm_min,
                tm.tm_sec
            )
        } else {
            String::from_utf8_lossy(&buf[..n]).into_owned()
        }
    }
}

/// Basename of `$VISUAL`, else `$EDITOR`.
#[allow(dead_code)]
pub fn editor() -> String {
    let raw = env::var("VISUAL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| env::var("EDITOR").ok().filter(|s| !s.is_empty()));
    match raw {
        Some(s) => {
            let first = s.split_whitespace().next().unwrap_or(&s);
            let base = first.rsplit('/').next().unwrap_or(first);
            if base.is_empty() {
                "N/A".into()
            } else {
                base.to_string()
            }
        }
        None => "N/A".into(),
    }
}

/// Nameservers from `/etc/resolv.conf`, joined `", "`.
#[allow(dead_code)]
pub fn dns() -> String {
    let s = match fs::read_to_string("/etc/resolv.conf") {
        Ok(s) => s,
        Err(_) => return "N/A".into(),
    };
    let mut ns: Vec<String> = Vec::new();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let mut it = line.split_whitespace();
        if it.next() == Some("nameserver") {
            if let Some(a) = it.next() {
                ns.push(a.to_string());
            }
        }
    }
    if ns.is_empty() {
        "N/A".into()
    } else {
        ns.join(", ")
    }
}

/// DMI vendor + product name, e.g. `LENOVO 82XX`.
#[allow(dead_code)]
pub fn host() -> String {
    let vendor = read_str("/sys/class/dmi/id/sys_vendor");
    let product = read_str("/sys/class/dmi/id/product_name");
    let parts: Vec<&str> = [vendor.as_str(), product.as_str()]
        .iter()
        .copied()
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        "N/A".into()
    } else {
        parts.join(" ")
    }
}

/// SMBIOS chassis type number → human name.
#[allow(dead_code)]
pub fn chassis() -> String {
    let n: u32 = match read_str("/sys/class/dmi/id/chassis_type").parse() {
        Ok(n) => n,
        Err(_) => return "N/A".into(),
    };
    let name = match n {
        1 => "Other",
        2 => "Unknown",
        3 => "Desktop",
        4 => "Low Profile Desktop",
        5 => "Pizza Box",
        6 => "Mini Tower",
        7 => "Tower",
        8 => "Portable",
        9 => "Laptop",
        10 => "Notebook",
        11 => "Hand Held",
        12 => "Docking Station",
        13 => "All in One",
        14 => "Sub Notebook",
        15 => "Space-saving",
        16 => "Lunch Box",
        17 => "Main Server Chassis",
        18 => "Expansion Chassis",
        19 => "Sub Chassis",
        20 => "Bus Expansion Chassis",
        21 => "Peripheral Chassis",
        22 => "RAID Chassis",
        23 => "Rack Mount Chassis",
        24 => "Sealed-case PC",
        25 => "Multi-system",
        26 => "CompactPCI",
        27 => "AdvancedTCA",
        28 => "Blade",
        29 => "Blade Enclosure",
        30 => "Tablet",
        31 => "Convertible",
        32 => "Detachable",
        33 => "IoT Gateway",
        34 => "Embedded PC",
        35 => "Mini PC",
        36 => "Stick PC",
        _ => return "N/A".into(),
    };
    name.into()
}

/// `UEFI` if `/sys/firmware/efi` exists, else `BIOS`.
#[allow(dead_code)]
pub fn bootmgr() -> String {
    if Path::new("/sys/firmware/efi").exists() {
        "UEFI".into()
    } else {
        "BIOS".into()
    }
}

/// CPU0 cache summary, e.g. `L1d 32K, L2 512K, L3 16M`.
#[allow(dead_code)]
pub fn cpu_cache() -> String {
    let dir = match fs::read_dir("/sys/devices/system/cpu/cpu0/cache") {
        Ok(d) => d,
        Err(_) => return "N/A".into(),
    };
    // (level, type letter, size)
    let mut items: Vec<(u64, u8, String)> = Vec::new();
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("index") {
            continue;
        }
        let level = read_path(&e.path().join("level"))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let ty = read_path(&e.path().join("type")).unwrap_or_default();
        let size = read_path(&e.path().join("size")).unwrap_or_default();
        if level == 0 || size.is_empty() {
            continue;
        }
        let letter = match ty.chars().next() {
            Some('D') | Some('d') => b'd',
            Some('I') | Some('i') => b'i',
            Some('U') | Some('u') => b'u',
            _ => b'?',
        };
        items.push((level, letter, norm_cache_size(&size)));
    }
    items.sort_by_key(|x| (x.0, x.1));
    if items.is_empty() {
        return "N/A".into();
    }
    let parts: Vec<String> = items
        .iter()
        .map(|(l, c, s)| format!("L{}{} {}", l, *c as char, s))
        .collect();
    parts.join(", ")
}

/// Basename of the systemd display-manager symlink, else `N/A`.
#[allow(dead_code)]
pub fn login_manager() -> String {
    match fs::read_link("/etc/systemd/system/display-manager.service") {
        Ok(p) => p
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "N/A".into()),
        Err(_) => "N/A".into(),
    }
}

/// Backlight brightness as `NN%`.
#[allow(dead_code)]
pub fn brightness() -> String {
    let dir = match fs::read_dir("/sys/class/backlight") {
        Ok(d) => d,
        Err(_) => return "N/A".into(),
    };
    for e in dir.flatten() {
        let b = read_path(&e.path().join("brightness")).and_then(|s| s.parse::<u64>().ok());
        let m = read_path(&e.path().join("max_brightness")).and_then(|s| s.parse::<u64>().ok());
        if let (Some(b), Some(m)) = (b, m) {
            let rounded = b.saturating_mul(100).saturating_add(m / 2);
            if let Some(pct) = rounded.checked_div(m) {
                return format!("{pct}%");
            }
        }
    }
    "N/A".into()
}

/// Pretty terminal name from environment variables.
#[allow(dead_code)]
pub fn terminal() -> String {
    let has = |k: &str| env::var_os(k).map(|v| !v.is_empty()).unwrap_or(false);
    if has("KITTY_WINDOW_ID") {
        return "kitty".into();
    }
    if has("WEZTERM_PANE") {
        return "WezTerm".into();
    }
    if has("ALACRITTY_WINDOW_ID") || has("ALACRITTY_LOG") {
        return "Alacritty".into();
    }
    if has("KONSOLE_VERSION") {
        return "Konsole".into();
    }
    if has("VTE_VERSION") {
        return "VTE".into();
    }
    if has("WT_SESSION") {
        return "Windows Terminal".into();
    }
    if let Ok(tp) = env::var("TERM_PROGRAM") {
        if !tp.is_empty() {
            return tp;
        }
    }
    if has("GHOSTTY_RESOURCES_DIR") {
        return "Ghostty".into();
    }
    if has("TERMINOLOGY") {
        return "Terminology".into();
    }
    if has("KITTY_PID") {
        return "kitty".into();
    }
    match env::var("TERM") {
        Ok(t) if !t.is_empty() => t,
        _ => "unknown".into(),
    }
}

/// Read aggregate CPU jiffies from `/proc/stat` → `(idle, total)`.
#[allow(dead_code)]
fn cpu_times() -> Option<(u64, u64)> {
    let s = fs::read_to_string("/proc/stat").ok()?;
    let line = s.lines().next()?;
    if !line.starts_with("cpu ") {
        return None;
    }
    let mut total: u64 = 0;
    let mut idle: u64 = 0;
    for (i, tok) in line.split_whitespace().skip(1).enumerate() {
        let v: u64 = tok.parse().ok()?;
        total = total.saturating_add(v);
        // user nice system idle iowait irq softirq steal
        if i == 3 || i == 4 {
            idle = idle.saturating_add(v);
        }
    }
    Some((idle, total))
}

/// Snapshot aggregate CPU jiffies (`(idle, total)`) for later comparison.
#[allow(dead_code)]
pub fn cpu_snapshot() -> Option<(u64, u64)> {
    cpu_times()
}

/// CPU usage between two [`cpu_snapshot`]s as `NN.N%`.
pub fn cpu_usage_between(idle0: u64, total0: u64, idle1: u64, total1: u64) -> String {
    let dt = total1.saturating_sub(total0);
    if dt == 0 {
        return "0.0%".into();
    }
    let di = idle1.saturating_sub(idle0);
    let busy = dt.saturating_sub(di);
    format!("{:.1}%", busy as f64 / dt as f64 * 100.0)
}

/// CPU usage over `sample_ms` (0 → 200ms) as `NN.N%`.
#[allow(dead_code)]
pub fn cpu_usage(sample_ms: u64) -> String {
    let ms = if sample_ms == 0 { 200 } else { sample_ms };
    let (idle0, total0) = match cpu_times() {
        Some(v) => v,
        None => return "N/A".into(),
    };
    thread::sleep(Duration::from_millis(ms));
    let (idle1, total1) = match cpu_times() {
        Some(v) => v,
        None => return "N/A".into(),
    };
    cpu_usage_between(idle0, total0, idle1, total1)
}

/// `/proc/diskstats` totals since boot, e.g. `read 1.2G, write 3.4G`.
#[allow(dead_code)]
pub fn diskio() -> String {
    let s = match fs::read_to_string("/proc/diskstats") {
        Ok(s) => s,
        Err(_) => return "N/A".into(),
    };
    let mut rd: u64 = 0;
    let mut wr: u64 = 0;
    for line in s.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        // field 6 → sectors read (idx 5), field 10 → sectors written (idx 9)
        let r: u64 = f.get(5).and_then(|x| x.parse().ok()).unwrap_or(0);
        let w: u64 = f.get(9).and_then(|x| x.parse().ok()).unwrap_or(0);
        rd = rd.saturating_add(r);
        wr = wr.saturating_add(w);
    }
    if rd == 0 && wr == 0 {
        return "N/A".into();
    }
    format!(
        "read {}, write {}",
        human(rd.saturating_mul(512)),
        human(wr.saturating_mul(512))
    )
}

/// `/proc/net/dev` totals since boot, e.g. `down 1.2G, up 345M`.
#[allow(dead_code)]
pub fn netio() -> String {
    let s = match fs::read_to_string("/proc/net/dev") {
        Ok(s) => s,
        Err(_) => return "N/A".into(),
    };
    let mut down: u64 = 0;
    let mut up: u64 = 0;
    for line in s.lines().skip(2) {
        let (name, rest) = match line.split_once(':') {
            Some(x) => x,
            None => continue,
        };
        let name = name.trim();
        if name.is_empty() || name == "lo" {
            continue;
        }
        let f: Vec<&str> = rest.split_whitespace().collect();
        let rx: u64 = f.first().and_then(|x| x.parse().ok()).unwrap_or(0);
        let tx: u64 = f.get(8).and_then(|x| x.parse().ok()).unwrap_or(0);
        down = down.saturating_add(rx);
        up = up.saturating_add(tx);
    }
    if down == 0 && up == 0 {
        return "N/A".into();
    }
    format!("down {}, up {}", human(down), human(up))
}

/// SMBIOS memory type byte → short name.
#[allow(dead_code)]
fn mem_type_name(t: u8) -> &'static str {
    match t {
        0x01 => "Other",
        0x02 => "Unknown",
        0x03 => "DRAM",
        0x04 => "EDRAM",
        0x05 => "VRAM",
        0x06 => "SRAM",
        0x07 => "RAM",
        0x08 => "ROM",
        0x09 => "FLASH",
        0x0A => "EEPROM",
        0x0B => "FEPROM",
        0x0C => "EPROM",
        0x0D => "CDRAM",
        0x0E => "3DRAM",
        0x0F => "SDRAM",
        0x10 => "SGRAM",
        0x11 => "RDRAM",
        0x12 => "DDR",
        0x13 => "DDR2",
        0x14 => "DDR2 FB-DIMM",
        0x18 => "DDR3",
        0x1A => "DDR4",
        0x1B => "LPDDR",
        0x1C => "LPDDR2",
        0x1D => "LPDDR3",
        0x1E => "LPDDR4",
        0x20 => "HBM",
        0x21 => "HBM2",
        0x22 => "DDR5",
        0x23 => "LPDDR5",
        _ => "RAM",
    }
}

/// Parse SMBIOS type-17 DIMM entries from the DMI raw structures.
#[allow(dead_code)]
fn physmem_dmi() -> Option<String> {
    let dir = fs::read_dir("/sys/firmware/dmi/entries").ok()?;
    let mut total_mb: u64 = 0;
    let mut count: u32 = 0;
    let mut first_type: Option<&'static str> = None;
    let mut speed: u16 = 0;

    for e in dir.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("17-") {
            continue;
        }
        let raw = match fs::read(e.path().join("raw")) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if raw.len() < 0x1E || raw.first().copied() != Some(17) {
            continue;
        }
        let d = raw.as_slice();
        let size_raw = le_u16(d, 0x0C);
        let mtype = d.get(0x12).copied().unwrap_or(0);
        let spd = le_u16(d, 0x15);

        let mb: u64 = if size_raw == 0 {
            continue; // empty slot
        } else if size_raw == 0x7FFF {
            le_u32(d, 0x1C) as u64 // extended size, MB
        } else if size_raw == 0xFFFF {
            continue; // unknown
        } else if size_raw & 0x8000 != 0 {
            ((size_raw & 0x7FFF) as u64) / 1024 // KB → MB
        } else {
            size_raw as u64 // MB
        };
        if mb == 0 {
            continue;
        }

        total_mb = total_mb.saturating_add(mb);
        if count == 0 {
            first_type = Some(mem_type_name(mtype));
            speed = spd;
        }
        count += 1;
    }

    if count == 0 {
        return None;
    }
    let size_str = if total_mb.is_multiple_of(1024) {
        format!("{}G", total_mb / 1024)
    } else {
        format!("{:.1}G", total_mb as f64 / 1024.0)
    };
    let ty = first_type.unwrap_or("RAM");
    let unit = if count == 1 { "dimm" } else { "dimms" };
    let out = if speed > 0 {
        format!("{} {}-{} ({} {})", size_str, ty, speed, count, unit)
    } else {
        format!("{} {} ({} {})", size_str, ty, count, unit)
    };
    Some(out)
}

/// Fallback: sum `/sys/devices/system/memory/memory*` blocks.
#[allow(dead_code)]
fn physmem_blocks() -> Option<String> {
    let bs = fs::read_to_string("/sys/devices/system/memory/block_size_bytes").ok()?;
    let bs = u64::from_str_radix(bs.trim(), 16).ok()?;
    if bs == 0 {
        return None;
    }
    let dir = fs::read_dir("/sys/devices/system/memory").ok()?;
    let mut n: u64 = 0;
    for e in dir.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if let Some(rest) = name.strip_prefix("memory") {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                n += 1;
            }
        }
    }
    if n == 0 {
        return None;
    }
    Some(format!("{} ({} blocks)", human(n.saturating_mul(bs)), n))
}

/// Physical memory: DIMM info from SMBIOS, else memory-block sum, else `N/A`.
#[allow(dead_code)]
pub fn physmem() -> String {
    if let Some(s) = physmem_dmi() {
        return s;
    }
    if let Some(s) = physmem_blocks() {
        return s;
    }
    "N/A".into()
}

/// Best-effort VRAM total from DRM sysfs.
#[allow(dead_code)]
pub fn gpu_mem() -> String {
    for i in 0..8 {
        let base = format!("/sys/class/drm/card{}/device", i);
        if let Some(total) = read_path(&Path::new(&base).join("mem_info_vram_total"))
            .and_then(|s| s.parse::<u64>().ok())
        {
            if total > 0 {
                let used = read_path(&Path::new(&base).join("mem_info_vram_used"))
                    .and_then(|s| s.parse::<u64>().ok());
                return match used {
                    Some(u) if u > 0 => format!("{} / {}", human(u), human(total)),
                    _ => human(total),
                };
            }
        }
    }
    "N/A".into()
}

/// btrfs volumes from `/sys/fs/btrfs/*`, joined `", "`.
#[allow(dead_code)]
pub fn btrfs() -> String {
    let looks_like_uuid =
        |s: &str| s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    let dir = match fs::read_dir("/sys/fs/btrfs") {
        Ok(d) => d,
        Err(_) => return "N/A".into(),
    };
    let mut vols: Vec<String> = Vec::new();
    for e in dir.flatten() {
        let uuid = e.file_name().to_string_lossy().into_owned();
        if !looks_like_uuid(&uuid) {
            continue; // skip pseudo entries such as "features"
        }
        let mut devs: Vec<String> = Vec::new();
        if let Ok(dd) = fs::read_dir(e.path().join("devices")) {
            for d in dd.flatten() {
                devs.push(d.file_name().to_string_lossy().into_owned());
            }
        }
        if devs.is_empty() {
            let short = &uuid[..uuid.len().min(8)];
            if !short.is_empty() {
                vols.push(short.to_string());
            }
        } else {
            devs.sort();
            vols.push(devs.join("+"));
        }
    }
    if vols.is_empty() {
        "N/A".into()
    } else {
        vols.join(", ")
    }
}

/// SSID for an interface via `iw`, if present.
#[allow(dead_code)]
fn iw_ssid(iface: &str) -> Option<String> {
    let out = Command::new("iw")
        .args(["dev", iface, "link"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if let Some(rest) = line.trim().strip_prefix("SSID:") {
            let v = rest.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Primary wireless iface + quality/signal from `/proc/net/wireless`.
#[allow(dead_code)]
pub fn wifi() -> String {
    let s = match fs::read_to_string("/proc/net/wireless") {
        Ok(s) => s,
        Err(_) => return "N/A".into(),
    };
    let mut iface = String::new();
    let mut qual: Option<f64> = None;
    let mut level: Option<f64> = None;
    for line in s.lines().skip(2) {
        let (name, rest) = match line.split_once(':') {
            Some(x) => x,
            None => continue,
        };
        let name = name.trim();
        if name.is_empty() || name == "lo" {
            continue;
        }
        iface = name.to_string();
        let f: Vec<&str> = rest.split_whitespace().collect();
        // status link level noise ...
        qual = f
            .get(1)
            .and_then(|x| x.trim_end_matches('.').parse::<f64>().ok());
        level = f
            .get(2)
            .and_then(|x| x.trim_end_matches('.').parse::<f64>().ok());
        break;
    }
    if iface.is_empty() {
        return "N/A".into();
    }
    let mut out = iface.clone();
    if let Some(q) = qual {
        out.push_str(&format!(" {:.0}%", q.clamp(0.0, 100.0)));
    }
    if let Some(l) = level {
        out.push_str(&format!(" {:.0}dBm", l));
    }
    if let Some(ssid) = iw_ssid(&iface) {
        out.push(' ');
        out.push_str(&ssid);
    }
    out
}

/// Logged-in users via libc utmpx, deduped, joined `", "`.
#[allow(dead_code)]
pub fn users() -> String {
    let mut seen: Vec<String> = Vec::new();
    unsafe {
        libc::setutxent();
        loop {
            let p = libc::getutxent();
            if p.is_null() {
                break;
            }
            let u = &*p;
            if u.ut_type != libc::USER_PROCESS {
                continue;
            }
            let name = cstr(&u.ut_user);
            if name.is_empty() {
                continue;
            }
            if !seen.iter().any(|x| x == &name) {
                seen.push(name);
            }
        }
        libc::endutxent();
    }
    if seen.is_empty() {
        "N/A".into()
    } else {
        seen.join(", ")
    }
}

/// Scan a config file for a wallpaper line (hyprpaper / hyprland / sway).
#[allow(dead_code)]
fn find_wallpaper_conf(content: &str) -> Option<String> {
    for raw in content.lines() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let low = l.to_ascii_lowercase();
        if low.starts_with("preload") || low.starts_with("wallpaper") {
            if let Some(val) = l.split_once('=').map(|x| x.1) {
                let val = val.rsplit(',').next().unwrap_or(val).trim().trim_matches('"');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
        if low.starts_with("output") {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if let Some(i) = parts.iter().position(|x| x.eq_ignore_ascii_case("bg")) {
                if let Some(p) = parts.get(i + 1) {
                    let p = p.trim_matches('"');
                    if !p.is_empty() {
                        return Some(p.to_string());
                    }
                }
            }
        }
    }
    None
}

/// GNOME wallpaper via gsettings, if available.
#[allow(dead_code)]
fn gsettings_wallpaper() -> Option<String> {
    let out = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", "picture-uri"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let s = s.trim().trim_matches('\'').trim_matches('"');
    let s = s.strip_prefix("file://").unwrap_or(s);
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Best-effort current wallpaper (Hyprland/sway config, gsettings).
#[allow(dead_code)]
pub fn wallpaper() -> String {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(h) = home() {
        for rel in [
            "/.config/hypr/hyprpaper.conf",
            "/.config/hypr/hyprland.conf",
            "/.config/sway/config",
        ] {
            candidates.push(format!("{}{}", h, rel));
        }
    }
    for p in candidates {
        if let Ok(c) = fs::read_to_string(&p) {
            if let Some(w) = find_wallpaper_conf(&c) {
                return w;
            }
        }
    }
    if let Some(w) = gsettings_wallpaper() {
        return w;
    }
    "N/A".into()
}

/// Best-effort now-playing via `playerctl`.
#[allow(dead_code)]
pub fn media() -> String {
    if let Ok(out) = Command::new("playerctl")
        .args(["metadata", "--format", "{{ artist }} - {{ title }}"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && !s.contains("No players") && s != " - " {
                return s;
            }
        }
    }
    "N/A".into()
}

/// Extract a JSON string value for `key` without a parser.
#[allow(dead_code)]
fn json_string(content: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let mut rest = content;
    while let Some(i) = rest.find(&needle) {
        let after = rest[i + needle.len()..].trim_start();
        if let Some(col) = after.strip_prefix(':') {
            let col = col.trim_start();
            if let Some(v) = col.strip_prefix('"') {
                let val: String = v.chars().take_while(|&c| c != '"').collect();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
        rest = &rest[i + needle.len()..];
    }
    None
}

/// Vulkan ICD `library_path` → friendly driver name.
#[allow(dead_code)]
fn driver_name(lib: &str) -> String {
    let base = lib.rsplit('/').next().unwrap_or(lib);
    let l = base.to_ascii_lowercase();
    if l.contains("radeon") || l.contains("amd") {
        "AMD (RADV)".into()
    } else if l.contains("nvidia") || l.starts_with("libnv") {
        "NVIDIA".into()
    } else if l.contains("intel") || l.contains("anv") {
        "Intel".into()
    } else if l.contains("lvp") || l.contains("lavapipe") {
        "lavapipe".into()
    } else if l.contains("swrast") {
        "swrast".into()
    } else {
        base.trim_start_matches("lib")
            .trim_end_matches(".so")
            .to_string()
    }
}

/// Vulkan driver names discovered from `/usr/share/vulkan/icd.d/*.json`.
#[allow(dead_code)]
fn vulkan_driver() -> Option<String> {
    let dir = fs::read_dir("/usr/share/vulkan/icd.d").ok()?;
    let mut out: Vec<String> = Vec::new();
    for e in dir.flatten() {
        let p = e.path();
        if !p.extension().map(|x| x == "json").unwrap_or(false) {
            continue;
        }
        if let Ok(s) = fs::read_to_string(&p) {
            if let Some(lib) = json_string(&s, "library_path") {
                out.push(driver_name(&lib));
            } else if let Some(stem) = p.file_stem() {
                out.push(stem.to_string_lossy().into_owned());
            }
        }
    }
    out.retain(|x| !x.is_empty());
    if out.is_empty() {
        None
    } else {
        Some(out.join(", "))
    }
}

/// Best-effort OpenGL renderer via `glxinfo`, else Vulkan ICD driver.
#[allow(dead_code)]
pub fn opengl() -> String {
    for arg in ["-B", ""] {
        let mut cmd = Command::new("glxinfo");
        if !arg.is_empty() {
            cmd.arg(arg);
        }
        if let Ok(out) = cmd.output() {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("OpenGL renderer string:") {
                    let v = rest.trim();
                    if !v.is_empty() {
                        return v.to_string();
                    }
                }
            }
        }
    }
    if let Some(d) = vulkan_driver() {
        return d;
    }
    "N/A".into()
}

/// Vulkan driver from ICD JSON, falling back to `vulkaninfo`.
#[allow(dead_code)]
pub fn vulkan() -> String {
    if let Some(d) = vulkan_driver() {
        return d;
    }
    if let Some(s) = run_timeout("vulkaninfo", &["--summary"], 1500) {
        for line in s.lines() {
            if line.contains("driverName") || line.contains("deviceName") {
                if let Some((_, v)) = line.split_once('=') {
                    let v = v.trim();
                    if !v.is_empty() {
                        return v.to_string();
                    }
                }
            }
        }
    }
    "N/A".into()
}

// ── Terminal / input / hardware extras ──────────────────

/// Terminal size as `162 columns x 47 rows` (via `TIOCGWINSZ`).
#[allow(dead_code)]
pub fn term_size() -> String {
    let mut ws = libc::winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    // SAFETY: `TIOCGWINSZ` writes a `winsize` through the pointer.
    let rc = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) };
    if rc == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
        return format!("{} columns x {} rows", ws.ws_col, ws.ws_row);
    }
    let cols = env::var("COLUMNS").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(0);
    let rows = env::var("LINES").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(0);
    if cols > 0 && rows > 0 {
        format!("{cols} columns x {rows} rows")
    } else {
        "N/A".into()
    }
}

/// The 16 basic ANSI colours as hex, used to decode `$COLORFGBG`.
const ANSI_HEX: [&str; 16] = [
    "#000000", "#800000", "#008000", "#808000", "#000080", "#800080", "#008080", "#c0c0c0",
    "#808080", "#ff0000", "#00ff00", "#ffff00", "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
];

/// Terminal foreground/background, from an OSC 10/11 query if possible,
/// else `$COLORFGBG`.
#[allow(dead_code)]
pub fn term_theme() -> String {
    term_theme_osc().or_else(term_theme_env).unwrap_or_else(|| "N/A".into())
}

fn term_theme_env() -> Option<String> {
    let fgbg = env::var("COLORFGBG").ok()?;
    let mut parts = fgbg.split(';').rev();
    let bg = parts.next().and_then(|s| s.trim().parse::<usize>().ok())?;
    let fg = parts.next().and_then(|s| s.trim().parse::<usize>().ok())?;
    let hex = |i: usize| ANSI_HEX.get(i % 16).copied().unwrap_or("#000000");
    Some(format!("{} (FG) - {} (BG)", hex(fg), hex(bg)))
}

/// Restores the tty's termios and closes it on drop.
struct TtyRaw {
    fd: libc::c_int,
    saved: libc::termios,
}

impl Drop for TtyRaw {
    fn drop(&mut self) {
        // SAFETY: `saved` was captured from this fd; restoring is best-effort.
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.saved);
            libc::close(self.fd);
        }
    }
}

fn term_theme_osc() -> Option<String> {
    let (fg, bg) = query_osc_colors();
    match (fg, bg) {
        (Some(f), Some(b)) => Some(format!("{f} (FG) - {b} (BG)")),
        (Some(f), None) => Some(format!("{f} (FG)")),
        (None, Some(b)) => Some(format!("{b} (BG)")),
        (None, None) => None,
    }
}

/// Terminal background as `#RRGGBB`, from an OSC 11 query with `$COLORFGBG`
/// as a fallback. Used to pick a light/dark scheme automatically.
pub fn term_bg() -> Option<String> {
    if let (_, Some(bg)) = query_osc_colors() {
        return Some(bg);
    }
    let fgbg = env::var("COLORFGBG").ok()?;
    let bg = fgbg.split(';').next_back()?.trim().parse::<usize>().ok()?;
    ANSI_HEX.get(bg % 16).map(|s| (*s).to_string())
}

/// Whether the terminal background is light (for auto light/dark schemes).
pub fn term_is_light() -> Option<bool> {
    let hex = term_bg()?;
    let h = hex.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    let (r, g, b) = (byte(0)?, byte(2)?, byte(4)?);
    Some(0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64 > 128.0)
}

/// Query OSC 10 (foreground) and OSC 11 (background) from the controlling
/// terminal. Returns `(fg, bg)` hex strings, either of which may be `None`.
fn query_osc_colors() -> (Option<String>, Option<String>) {
    // SAFETY: opening `/dev/tty` returns a plain fd; checked below.
    let fd = unsafe {
        libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_NOCTTY)
    };
    if fd < 0 {
        return (None, None);
    }
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: `tcgetattr` fills `saved` for an open tty.
    if unsafe { libc::tcgetattr(fd, &mut saved) } != 0 {
        // SAFETY: `fd` was opened above.
        unsafe { libc::close(fd) };
        return (None, None);
    }
    let mut raw = saved;
    raw.c_lflag &= !(libc::ICANON | libc::ECHO);
    raw.c_cc[libc::VMIN] = 0;
    raw.c_cc[libc::VTIME] = 1; // 100 ms per read
    // SAFETY: applying a termios derived from the current one.
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) };
    let guard = TtyRaw { fd, saved };

    let query = b"\x1b]10;?\x07\x1b]11;?\x07";
    // SAFETY: writing a static buffer to the open tty.
    unsafe { libc::write(fd, query.as_ptr().cast(), query.len()) };

    let mut buf = Vec::new();
    let mut tmp = [0u8; 256];
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(120);
    while std::time::Instant::now() < deadline {
        // SAFETY: reading into a valid buffer from the open tty.
        let n = unsafe { libc::read(fd, tmp.as_mut_ptr().cast(), tmp.len()) };
        if n > 0 {
            buf.extend_from_slice(&tmp[..n as usize]);
            if buf.iter().filter(|&&b| b == 0x1b).count() >= 4 {
                break;
            }
        }
    }
    drop(guard);

    let text = String::from_utf8_lossy(&buf);
    let fg = parse_osc_color(&text, 10);
    let bg = parse_osc_color(&text, 11);
    (fg, bg)
}

fn parse_osc_color(s: &str, code: u8) -> Option<String> {
    let needle = format!("]{code};");
    let rest = &s[s.find(&needle)? + needle.len()..];
    let spec = rest.strip_prefix("rgb:").or_else(|| rest.strip_prefix("rgba:"))?;
    let end = spec.find(['\x07', '\x1b']).unwrap_or(spec.len());
    let mut parts = spec[..end].split('/');
    let r = hex_component(parts.next()?)?;
    let g = hex_component(parts.next()?)?;
    let b = hex_component(parts.next()?)?;
    Some(format!("#{r:02X}{g:02X}{b:02X}"))
}

fn hex_component(s: &str) -> Option<u8> {
    if s.is_empty() || s.len() > 4 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    let max = (1u32 << (4 * s.len() as u32)) - 1;
    Some(((v * 255 + max / 2) / max).min(255) as u8)
}

/// Terminal font family: `$ARC_FONT`/`$FONT` first, then the terminal's own
/// config file (rio, kitty, ghostty, foot, alacritty, wezterm).
pub fn term_font() -> String {
    for var in ["ARC_FONT", "ARCFONT", "TERM_FONT", "FONT"] {
        if let Ok(v) = env::var(var) {
            if !v.trim().is_empty() { return v; }
        }
    }
    let Some(h) = home() else { return "N/A".into() };
    let read = |p: &str| read_str(&format!("{h}/.config/{p}"));

    // rio
    if let Some(v) = toml_section_value(&read("rio/config.toml"), "fonts", "family") {
        return v;
    }
    // kitty
    if let Some(v) = conf_value(&read("kitty/kitty.conf"), "font_family") {
        return v;
    }
    // ghostty
    if let Some(v) = conf_value(&read("ghostty/config"), "font-family") {
        return v;
    }
    // foot — `font=Family:size=11`
    if let Some(v) = conf_value(&read("foot/foot.ini"), "font") {
        let fam = v.split(':').next().unwrap_or(&v).trim().to_string();
        if !fam.is_empty() { return fam; }
    }
    // alacritty — TOML then YAML
    if let Some(v) = toml_section_value(&read("alacritty/alacritty.toml"), "font.normal", "family") {
        return v;
    }
    if let Some(v) = alacritty_yaml_family(&read("alacritty/alacritty.yml")) {
        return v;
    }
    // wezterm
    if let Some(v) = wezterm_font(&read("wezterm/wezterm.lua")) {
        return v;
    }
    "N/A".into()
}

/// Value of `key` inside `[section]` of a small TOML file.
fn toml_section_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        if let Some(rest) = t.strip_prefix('[') {
            in_section = rest.trim_end_matches(']').trim() == section;
            continue;
        }
        if in_section {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == key { return clean_value(v); }
            }
        }
    }
    None
}

/// Value of a `key value` or `key = value` line in a flat config file.
fn conf_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        let (k, v) = match t.split_once('=') {
            Some(kv) => kv,
            None => match t.split_once(char::is_whitespace) {
                Some(kv) => kv,
                None => continue,
            },
        };
        if k.trim() == key { return clean_value(v); }
    }
    None
}

fn clean_value(v: &str) -> Option<String> {
    let v = v.trim().trim_matches(['"', '\'']).trim();
    if v.is_empty() { None } else { Some(v.to_string()) }
}

fn alacritty_yaml_family(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("family:") {
            return clean_value(v);
        }
    }
    None
}

fn wezterm_font(text: &str) -> Option<String> {
    // `font = wezterm.font("Family")` or `wezterm.font_with_fallback({...})`
    let i = text.find("font(")?;
    let after = &text[i + 5..];
    let q = after.find(['"', '\''])?;
    let quote = after.as_bytes()[q];
    let rest = &after[q + 1..];
    let close = rest.find(quote as char)?;
    clean_value(&rest[..close])
}

/// Version string for the running desktop/WM, e.g. `25.05` (niri) or `0.41.2`.
///
/// Reads a version file for GNOME/KDE and asks the compositor binary otherwise;
/// returns `N/A` when the WM exposes no version.
pub fn de_wm_version() -> String {
    let de = env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| env::var("DESKTOP_SESSION"))
        .unwrap_or_default();
    let low = de.to_ascii_lowercase();

    if low.contains("gnome") {
        let raw = read_str("/usr/share/gnome/gnome-version.xml");
        if let (Some(s), Some(e)) = (raw.find("<version>"), raw.find("</version>")) {
            if s + 9 <= e { return raw[s + 9..e].to_string(); }
        }
    }
    if low.contains("kde") || low.contains("plasma") {
        let v = read_str("/usr/share/plasma/plasma-version");
        if !v.trim().is_empty() { return v.trim().to_string(); }
    }

    let bin: Option<&str> =
        if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() { Some("Hyprland") }
        else if env::var("SWAYSOCK").is_ok()                 { Some("sway") }
        else if env::var("NIRI_SOCKET").is_ok()              { Some("niri") }
        else if low.contains("hyprland")                     { Some("Hyprland") }
        else if low.contains("sway")                         { Some("sway") }
        else if low.contains("niri")                         { Some("niri") }
        else if low.contains("i3")                           { Some("i3") }
        else if low.contains("river")                        { Some("river") }
        else if low.contains("wayfire")                      { Some("wayfire") }
        else if low.contains("labwc")                        { Some("labwc") }
        else if low.contains("xfce")                         { Some("xfce4-session") }
        else if low.contains("cinnamon")                     { Some("cinnamon") }
        else if low.contains("mate")                         { Some("mate-session") }
        else                                                 { None };

    match bin.and_then(|b| run_timeout(b, &["--version"], 400)) {
        Some(s) => extract_version(&s),
        None => "N/A".into(),
    }
}

/// First dotted version token in `s` (e.g. `25.05`, `1.9`, `0.41.2`).
fn extract_version(s: &str) -> String {
    for tok in s.split_whitespace() {
        let cleaned = tok.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        if cleaned.contains('.') && cleaned.chars().any(|c| c.is_ascii_digit()) {
            return cleaned.to_string();
        }
    }
    s.lines().next().unwrap_or("").trim().to_string()
}

/// Cursor theme + size from the environment, falling back to GTK settings.
#[allow(dead_code)]
pub fn cursor() -> String {
    let theme = env::var("XCURSOR_THEME").ok().filter(|s| !s.is_empty())
        .or_else(|| gtk_setting("gtk-cursor-theme-name"));
    let size = env::var("XCURSOR_SIZE").ok().filter(|s| !s.is_empty())
        .or_else(|| gtk_setting("gtk-cursor-theme-size"));
    match (theme, size) {
        (Some(t), Some(s)) => format!("{t} ({s}px)"),
        (Some(t), None) => t,
        (None, Some(s)) => format!("{s}px"),
        (None, None) => "N/A".into(),
    }
}

/// Read one `key=value` from the user's GTK 3/4 settings.
fn gtk_setting(key: &str) -> Option<String> {
    let h = home()?;
    for sub in ["gtk-3.0", "gtk-4.0"] {
        let s = read_str(&format!("{h}/.config/{sub}/settings.ini"));
        for line in s.lines() {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key && !v.trim().is_empty() {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Connected cameras from `/sys/class/video4linux`.
#[allow(dead_code)]
pub fn camera() -> String {
    let mut names: Vec<String> = Vec::new();
    if let Ok(rd) = fs::read_dir("/sys/class/video4linux") {
        for e in rd.flatten() {
            if let Some(name) = read_path(&e.path().join("name")) {
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    if names.is_empty() { "N/A".into() } else { names.join(", ") }
}

/// Keyboards from `/proc/bus/input/devices`.
#[allow(dead_code)]
pub fn keyboard() -> String {
    let (kbs, _) = input_devices();
    if kbs.is_empty() { "N/A".into() } else { kbs.join(", ") }
}

/// Mice / touchpads from `/proc/bus/input/devices`.
#[allow(dead_code)]
pub fn mouse() -> String {
    let (_, mice) = input_devices();
    if mice.is_empty() { "N/A".into() } else { mice.join(", ") }
}

fn input_devices() -> (Vec<String>, Vec<String>) {
    let s = read_str("/proc/bus/input/devices");
    let (mut kbs, mut mice): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for block in s.split("\n\n") {
        let mut name = String::new();
        let mut handlers = String::new();
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("N: Name=") {
                name = rest.trim().trim_matches('"').to_string();
            } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
                handlers = rest.to_string();
            }
        }
        if name.is_empty() || handlers.is_empty() {
            continue;
        }
        const NOT_A_KEYBOARD: &[&str] = &[
            "Button", "Video Bus", "PC Speaker", "Lid Switch", "HD-Audio", "HDMI", "Headphone",
        ];
        if NOT_A_KEYBOARD.iter().any(|pat| name.contains(pat)) {
            continue;
        }
        let is_kbd = handlers.split_whitespace().any(|h| h == "kbd");
        let is_mouse = handlers.split_whitespace().any(|h| h.starts_with("mouse"));
        if is_kbd && !kbs.contains(&name) {
            kbs.push(name);
        } else if is_mouse && !mice.contains(&name) {
            mice.push(name);
        }
    }
    (kbs, mice)
}

/// TPM version from `/sys/class/tpm/*/tpm_version_major`.
#[allow(dead_code)]
pub fn tpm() -> String {
    let mut out: Vec<String> = Vec::new();
    if let Ok(rd) = fs::read_dir("/sys/class/tpm") {
        for e in rd.flatten() {
            if let Some(major) = read_path(&e.path().join("tpm_version_major")) {
                if !major.is_empty() {
                    let v = format!("{major}.0");
                    if !out.contains(&v) { out.push(v); }
                }
            }
        }
    }
    if out.is_empty() { "N/A".into() } else { out.join(", ") }
}

/// Physical (block) disks: model, size, type and temperature.
#[allow(dead_code)]
pub fn physical_disk() -> String {
    let mut out: Vec<String> = Vec::new();
    let Ok(rd) = fs::read_dir("/sys/block") else { return "N/A".into() };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-")
            || name.starts_with("md") || name.starts_with("sr")
        {
            continue;
        }
        let p = e.path();
        let sectors: u64 = read_path(&p.join("size")).and_then(|s| s.parse().ok()).unwrap_or(0);
        if sectors == 0 { continue; }
        let bytes = sectors.saturating_mul(512);
        let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let model = read_path(&p.join("device/model")).filter(|m| !m.is_empty())
            .unwrap_or_else(|| name.clone());
        let virtual_disk = name.starts_with("zram") || name.starts_with("nbd");
        let rotational = read_path(&p.join("queue/rotational")).unwrap_or_default();
        let kind = if virtual_disk { "Virtual" }
            else if rotational.trim() == "0" { "SSD" }
            else { "HDD" };
        let temp = disk_temp(&name).map(|t| format!(" - {t:.1}°C")).unwrap_or_default();
        out.push(format!("{model}: {gib:.2} GiB [{kind}, Fixed]{temp}"));
    }
    if out.is_empty() { "N/A".into() } else { out.join(", ") }
}

fn disk_temp(name: &str) -> Option<f64> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    // `nvme0n1` → controller `nvme0`, whose `hwmonN` node holds the temperature.
    if let Some(rest) = name.strip_prefix("nvme") {
        let ctrl: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !ctrl.is_empty() {
            let base = std::path::PathBuf::from(format!("/sys/class/nvme/nvme{ctrl}"));
            dirs.push(base.join("device/hwmon"));
            if let Ok(rd) = fs::read_dir(&base) {
                for e in rd.flatten() {
                    if e.file_name().to_string_lossy().starts_with("hwmon") {
                        dirs.push(e.path());
                    }
                }
            }
        }
    }
    dirs.push(std::path::PathBuf::from(format!("/sys/class/block/{name}/device/hwmon")));
    for dir in dirs {
        if let Some(t) = temp_in(&dir) {
            return Some(t);
        }
    }
    None
}

/// Read `temp1_input` from `dir`, or from a `hwmonN` child of `dir`.
fn temp_in(dir: &std::path::Path) -> Option<f64> {
    if let Some(v) = read_path(&dir.join("temp1_input")) {
        if let Ok(milli) = v.parse::<f64>() {
            return Some(milli / 1000.0);
        }
    }
    let rd = fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        if let Some(v) = read_path(&e.path().join("temp1_input")) {
            if let Ok(milli) = v.parse::<f64>() {
                return Some(milli / 1000.0);
            }
        }
    }
    None
}

/// Codec support (encoder/decoder): `vainfo` if installed, else libva via dlopen.
#[allow(dead_code)]
pub fn codec() -> String {
    codec_vainfo().or_else(codec_libva).unwrap_or_else(|| "N/A".into())
}

fn codec_vainfo() -> Option<String> {
    let s = run_timeout("vainfo", &[], 1000)?;
    let (mut enc, mut dec): (Vec<&'static str>, Vec<&'static str>) = (Vec::new(), Vec::new());
    for line in s.lines() {
        let Some((prof, entry)) = line.split_once(':') else { continue };
        if !prof.trim().starts_with("VAProfile") { continue; }
        let Some(name) = va_codec_name(prof) else { continue };
        let entry = entry.trim();
        if entry.contains("EncSlice") || entry.contains("EncPicture") || entry.contains("EncJPEG") {
            if !enc.contains(&name) { enc.push(name); }
        } else if entry.contains("VLD") && !dec.contains(&name) {
            dec.push(name);
        }
    }
    match (enc.is_empty(), dec.is_empty()) {
        (true, true) => None,
        (false, true) => Some(format!("enc: {}", enc.join(", "))),
        (true, false) => Some(format!("dec: {}", dec.join(", "))),
        (false, false) => Some(format!("enc: {} | dec: {}", enc.join(", "), dec.join(", "))),
    }
}

// ── libva (optional, loaded at runtime) ─────────────────

type VaDisplay = *mut core::ffi::c_void;
type VaStatus = core::ffi::c_int;
type VaProfile = core::ffi::c_int;
type VaEntrypoint = core::ffi::c_int;
type VaGetDisplayDrm = unsafe extern "C" fn(core::ffi::c_int) -> VaDisplay;
type VaInitialize = unsafe extern "C" fn(VaDisplay, *mut core::ffi::c_int, *mut core::ffi::c_int) -> VaStatus;
type VaTerminate = unsafe extern "C" fn(VaDisplay) -> VaStatus;
type VaQueryProfiles = unsafe extern "C" fn(VaDisplay, *mut VaProfile, *mut core::ffi::c_int) -> VaStatus;
type VaQueryEntrypoints = unsafe extern "C" fn(VaDisplay, VaProfile, *mut VaEntrypoint, *mut core::ffi::c_int) -> VaStatus;
const VA_STATUS_SUCCESS: VaStatus = 0;

struct VaApi {
    core: *mut core::ffi::c_void,
    drm: *mut core::ffi::c_void,
    get_display_drm: unsafe extern "C" fn(core::ffi::c_int) -> VaDisplay,
    initialize: unsafe extern "C" fn(VaDisplay, *mut core::ffi::c_int, *mut core::ffi::c_int) -> VaStatus,
    terminate: unsafe extern "C" fn(VaDisplay) -> VaStatus,
    query_profiles: unsafe extern "C" fn(VaDisplay, *mut VaProfile, *mut core::ffi::c_int) -> VaStatus,
    query_entrypoints: unsafe extern "C" fn(VaDisplay, VaProfile, *mut VaEntrypoint, *mut core::ffi::c_int) -> VaStatus,
}

impl VaApi {
    fn load() -> Option<Self> {
        // SAFETY: plain dlopen/dlsym; every symbol is null-checked before use.
        unsafe {
            let core = libc::dlopen(c"libva.so.2".as_ptr(), libc::RTLD_NOW);
            if core.is_null() {
                return None;
            }
            // `vaGetDisplayDRM` lives in libva-drm, not libva itself.
            let drm = libc::dlopen(c"libva-drm.so.2".as_ptr(), libc::RTLD_NOW);
            if drm.is_null() {
                libc::dlclose(core);
                return None;
            }
            let get_display_drm = libc::dlsym(drm, c"vaGetDisplayDRM".as_ptr());
            let initialize = libc::dlsym(core, c"vaInitialize".as_ptr());
            let terminate = libc::dlsym(core, c"vaTerminate".as_ptr());
            let query_profiles = libc::dlsym(core, c"vaQueryConfigProfiles".as_ptr());
            let query_entrypoints = libc::dlsym(core, c"vaQueryConfigEntrypoints".as_ptr());
            if [get_display_drm, initialize, terminate, query_profiles, query_entrypoints]
                .iter().any(|p| p.is_null())
            {
                libc::dlclose(drm);
                libc::dlclose(core);
                return None;
            }
            Some(Self {
                core,
                drm,
                get_display_drm: std::mem::transmute::<*mut core::ffi::c_void, VaGetDisplayDrm>(get_display_drm),
                initialize: std::mem::transmute::<*mut core::ffi::c_void, VaInitialize>(initialize),
                terminate: std::mem::transmute::<*mut core::ffi::c_void, VaTerminate>(terminate),
                query_profiles: std::mem::transmute::<*mut core::ffi::c_void, VaQueryProfiles>(query_profiles),
                query_entrypoints: std::mem::transmute::<*mut core::ffi::c_void, VaQueryEntrypoints>(query_entrypoints),
            })
        }
    }
}

impl Drop for VaApi {
    fn drop(&mut self) {
        // SAFETY: both handles came from successful `dlopen` calls above.
        unsafe {
            libc::dlclose(self.drm);
            libc::dlclose(self.core);
        }
    }
}

fn va_profile_name(p: VaProfile) -> Option<&'static str> {
    match p {
        0 | 1 => Some("MPEG-2"),
        2..=4 => Some("MPEG-4"),
        5..=7 | 13 | 15 | 16 => Some("H.264"),
        8..=10 => Some("VC-1"),
        12 => Some("JPEG"),
        14 => Some("VP8"),
        17 | 18 | 23..=31 => Some("HEVC / H.265"),
        19..=22 => Some("VP9"),
        32 | 33 => Some("AV1"),
        _ => None,
    }
}

fn codec_libva() -> Option<String> {
    let api = VaApi::load()?;
    let mut fd = -1;
    for i in 128..136 {
        let path = std::ffi::CString::new(format!("/dev/dri/renderD{i}")).ok()?;
        // SAFETY: `open` on a valid C string; the result is checked.
        fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR) };
        if fd >= 0 {
            break;
        }
    }
    if fd < 0 {
        return None;
    }
    // SAFETY: `fd` is an open DRM render node and the fn pointer is valid.
    let display = unsafe { (api.get_display_drm)(fd) };
    if display.is_null() {
        // SAFETY: `fd` is still open here.
        unsafe { libc::close(fd) };
        return None;
    }
    let mut major = 0 as core::ffi::c_int;
    let mut minor = 0 as core::ffi::c_int;
    // SAFETY: valid display handle from `vaGetDisplayDRM`.
    if unsafe { (api.initialize)(display, &mut major, &mut minor) } != VA_STATUS_SUCCESS {
        unsafe { libc::close(fd) };
        return None;
    }
    let mut profiles = [0 as VaProfile; 128];
    let mut nprofiles: core::ffi::c_int = profiles.len() as core::ffi::c_int;
    let ok = unsafe { (api.query_profiles)(display, profiles.as_mut_ptr(), &mut nprofiles) }
        == VA_STATUS_SUCCESS;
    let mut enc: Vec<&'static str> = Vec::new();
    let mut dec: Vec<&'static str> = Vec::new();
    if ok {
        for &profile in profiles.iter().take(nprofiles.max(0) as usize) {
            let Some(codec) = va_profile_name(profile) else { continue };
            let mut eps = [0 as VaEntrypoint; 32];
            let mut nep: core::ffi::c_int = eps.len() as core::ffi::c_int;
            // SAFETY: valid display handle and a correctly sized output buffer.
            let q = unsafe {
                (api.query_entrypoints)(display, profile, eps.as_mut_ptr(), &mut nep)
            };
            if q != VA_STATUS_SUCCESS {
                continue;
            }
            for &ep in eps.iter().take(nep.max(0) as usize) {
                match ep {
                    1 if !dec.contains(&codec) => dec.push(codec),
                    6..=8 if !enc.contains(&codec) => enc.push(codec),
                    _ => {}
                }
            }
        }
    }
    // SAFETY: teardown of the handles created above.
    unsafe {
        (api.terminate)(display);
        libc::close(fd);
    }
    match (enc.is_empty(), dec.is_empty()) {
        (true, true) => None,
        (false, true) => Some(format!("enc: {}", enc.join(", "))),
        (true, false) => Some(format!("dec: {}", dec.join(", "))),
        (false, false) => Some(format!("enc: {} | dec: {}", enc.join(", "), dec.join(", "))),
    }
}

fn va_codec_name(profile: &str) -> Option<&'static str> {
    let p = profile.trim();
    if p.contains("H264") || p.contains("AVC") { Some("H.264") }
    else if p.contains("HEVC") || p.contains("H265") { Some("HEVC / H.265") }
    else if p.contains("VP9") { Some("VP9") }
    else if p.contains("AV1") { Some("AV1") }
    else if p.contains("VP8") { Some("VP8") }
    else if p.contains("MPEG2") { Some("MPEG-2") }
    else if p.contains("MPEG4") { Some("MPEG-4") }
    else if p.contains("VC1") { Some("VC-1") }
    else if p.contains("JPEG") { Some("JPEG") }
    else { None }
}

/// Bluetooth radios from `/sys/class/bluetooth` (+ `hciconfig` for the version).
#[allow(dead_code)]
pub fn bluetooth() -> String {
    if fs::read_dir("/sys/class/bluetooth").map(|mut d| d.next().is_none()).unwrap_or(true) {
        return "N/A".into();
    }
    let (version, alias) = bluetooth_info();
    match (version, alias) {
        (Some(v), Some(a)) => format!("Bluetooth {v} ({a})"),
        (Some(v), None) => format!("Bluetooth {v}"),
        (None, Some(a)) => format!("Bluetooth ({a})"),
        (None, None) => "Bluetooth".into(),
    }
}

/// Map an HCI version code (from `bluetoothctl show`) to `major.minor`.
const HCI_VERSIONS: &[&str] = &[
    "1.0b", "1.1", "1.2", "2.0", "2.1", "3.0", "4.0", "4.1",
    "4.2", "5.0", "5.1", "5.2", "5.3", "5.4", "5.5", "6.0",
];

fn bluetooth_info() -> (Option<String>, Option<String>) {
    let Some(s) = run_timeout("bluetoothctl", &["show"], 800) else {
        return (None, None);
    };
    let mut version = None;
    let mut alias = None;
    for line in s.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("Version:") {
            if let Some((_, num)) = rest.split_once('(') {
                if let Ok(code) = num.trim_end_matches(')').trim().parse::<usize>() {
                    version = HCI_VERSIONS.get(code).map(|v| (*v).to_string());
                }
            }
        } else if let Some(rest) = l.strip_prefix("Alias:") {
            let a = rest.trim();
            if !a.is_empty() {
                alias = Some(a.to_string());
            }
        }
    }
    (version, alias)
}

/// Uptime in seconds (for per-process CPU averaging).
fn uptime_secs() -> u64 {
    let mut info = std::mem::MaybeUninit::<libc::sysinfo>::uninit();
    // SAFETY: `sysinfo` fills the struct on success.
    if unsafe { libc::sysinfo(info.as_mut_ptr()) } == 0 {
        // SAFETY: initialized by the successful call above.
        unsafe { info.assume_init() }.uptime as u64
    } else {
        0
    }
}

/// Top 5 processes by average CPU use since they started.
#[allow(dead_code)]
pub fn top_processes() -> String {
    let hz = {
        // SAFETY: `sysconf` is always safe to call.
        let v = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if v > 0 { v as u64 } else { 100 }
    };
    let page = {
        // SAFETY: `sysconf` is always safe to call.
        let v = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if v > 0 { v as u64 } else { 4096 }
    };
    let uptime_ticks = uptime_secs().saturating_mul(hz);
    let Ok(rd) = fs::read_dir("/proc") else { return "N/A".into() };
    let mut procs: Vec<(f64, u64, String, u64)> = Vec::new();
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u64>().ok()) else { continue };
        let Ok(stat) = fs::read_to_string(e.path().join("stat")) else { continue };
        let Some(close) = stat.rfind(')') else { continue };
        let comm = stat[..close].split_once('(').map(|(_, c)| c).unwrap_or("");
        let rest: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        if rest.len() < 20 { continue; }
        let utime: u64 = rest[11].parse().unwrap_or(0);
        let stime: u64 = rest[12].parse().unwrap_or(0);
        let start: u64 = rest[19].parse().unwrap_or(0);
        let elapsed = uptime_ticks.saturating_sub(start);
        if elapsed == 0 { continue; }
        let cpu = (utime + stime) as f64 / elapsed as f64 * 100.0;
        if cpu < 0.05 { continue; }
        let rss_pages: u64 = read_str(&format!("/proc/{pid}/statm"))
            .split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        procs.push((cpu, pid, comm.to_string(), rss_pages.saturating_mul(page)));
    }
    procs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    procs.truncate(5);
    if procs.is_empty() { return "N/A".into(); }
    let mut out = Vec::with_capacity(procs.len());
    for (cpu, pid, name, rss) in procs {
        let io = read_str(&format!("/proc/{pid}/io"));
        let rd_b = io.lines().find_map(|l| l.strip_prefix("read_bytes:"))
            .and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0);
        let wr_b = io.lines().find_map(|l| l.strip_prefix("write_bytes:"))
            .and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0);
        out.push(format!("{name} ({pid}) {cpu:.1}% {} R{} W{}", human(rss), human(rd_b), human(wr_b)));
    }
    out.join(", ")
}

/// Detailed monitor info for the first connected DRM connector (via EDID).
#[allow(dead_code)]
pub fn monitor() -> String {
    let Ok(rd) = fs::read_dir("/sys/class/drm") else { return "N/A".into() };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let dir = e.path();
        if read_path(&dir.join("status")).as_deref() != Some("connected") {
            continue;
        }
        let mode = read_path(&dir.join("modes")).unwrap_or_default();
        let Some((w, h)) = mode.lines().next().and_then(|m| {
            let (a, b) = m.trim().split_once('x')?;
            Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))
        }) else {
            continue;
        };
        let edid = fs::read(dir.join("edid")).unwrap_or_default();
        let (name, size_cm, refresh) = edid_info(&edid);
        let name = name.unwrap_or_else(|| {
            e.file_name().to_string_lossy()
                .split_once('-').map(|(_, n)| n.to_string())
                .unwrap_or_else(|| e.file_name().to_string_lossy().into_owned())
        });
        let mut out = format!("{name}: {w}x{h} px");
        if let Some(r) = refresh {
            out.push_str(&format!(" @ {r:.3} Hz"));
        }
        if let Some((wc, hc)) = size_cm {
            let inches = ((wc as f64 / 2.54).powi(2) + (hc as f64 / 2.54).powi(2)).sqrt();
            let ppi = ((w * w + h * h) as f64).sqrt() / inches;
            out.push_str(&format!(
                " - {}x{} mm ({inches:.2} inches, {ppi:.2} ppi)",
                wc * 10, hc * 10
            ));
        }
        return out;
    }
    "N/A".into()
}

fn edid_info(edid: &[u8]) -> (Option<String>, Option<(u32, u32)>, Option<f64>) {
    if edid.len() < 128 || edid[..8] != [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00] {
        return (None, None, None);
    }
    let (w, h) = (edid[21] as u32, edid[22] as u32);
    let size = if w > 0 && h > 0 { Some((w, h)) } else { None };
    let name = edid_descriptor_name(edid).or_else(|| edid_pnp_name(edid));
    (name, size, preferred_refresh(edid))
}

/// `CMN15F5`-style name from the EDID PnP id + product code.
fn edid_pnp_name(edid: &[u8]) -> Option<String> {
    if edid.len() < 12 {
        return None;
    }
    let m = u16::from_be_bytes([edid[8], edid[9]]);
    let letter = |shift: u16| (((m >> shift) & 0x1f) as u8 + b'A' - 1) as char;
    let pnp: String = [letter(10), letter(5), letter(0)].into_iter().collect();
    if !pnp.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let product = u16::from_le_bytes([edid[10], edid[11]]);
    Some(format!("{pnp}{product:04X}"))
}

fn edid_descriptor_name(edid: &[u8]) -> Option<String> {
    for off in [54usize, 72, 90, 108] {
        if off + 18 > edid.len() {
            continue;
        }
        let d = &edid[off..off + 18];
        if d[0] == 0 && d[1] == 0 && d[3] == 0xfc {
            let name: String = d[5..]
                .iter()
                .take_while(|&&b| b != 0x0a && b != 0)
                .map(|&b| b as char)
                .collect();
            let name = name.trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Refresh rate of the EDID's preferred detailed timing, in Hz.
fn preferred_refresh(edid: &[u8]) -> Option<f64> {
    if edid.len() < 72 {
        return None;
    }
    let d = &edid[54..72];
    let pclk = u16::from_le_bytes([d[0], d[1]]) as u64 * 10_000;
    if pclk == 0 {
        return None;
    }
    let ha = d[2] as u64 | ((d[4] as u64 & 0xf0) << 4);
    let hb = d[3] as u64 | ((d[4] as u64 & 0x0f) << 8);
    let va = d[5] as u64 | ((d[7] as u64 & 0xf0) << 4);
    let vb = d[6] as u64 | ((d[7] as u64 & 0x0f) << 8);
    let total = (ha + hb).saturating_mul(va + vb);
    if total == 0 { None } else { Some(pclk as f64 / total as f64) }
}
