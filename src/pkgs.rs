use std::{collections::HashMap, fs, path::Path, process::Command};

/// Fastfetch-style aggregate: every detected manager, not just the first one.
/// Mirrors `packages_linux.c` closely enough that the common counts match.
pub fn count_all() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut parts = Vec::new();

    let appimage = appimage_count(Path::new(&home));
    if appimage > 0 { parts.push(format!("{appimage} (appimage)")); }

    let flatpak_system = flatpak_count(Path::new("/var/lib"));
    if flatpak_system > 0 { parts.push(format!("{flatpak_system} (flatpak-system)")); }

    let flatpak_user = flatpak_count(Path::new(&format!("{home}/.local/share")));
    if flatpak_user > 0 { parts.push(format!("{flatpak_user} (flatpak-user)")); }

    let nix_default = nix_count(Path::new("/nix/var/nix/profiles/default"));
    if nix_default > 0 { parts.push(format!("{nix_default} (nix-default)")); }

    let nix_system = nix_count(Path::new("/run/current-system"));
    if nix_system > 0 { parts.push(format!("{nix_system} (nix-system)")); }

    let state = std::env::var("XDG_STATE_HOME")
        .unwrap_or_else(|_| format!("{home}/.local/state"));
    let user = std::env::var("USER").unwrap_or_default();
    let nix_user = nix_count(Path::new(&format!("{home}/.nix-profile")))
        + nix_count(Path::new(&format!("{state}/nix/profile")))
        + nix_count(Path::new(&format!("/etc/profiles/per-user/{user}")));
    if nix_user > 0 { parts.push(format!("{nix_user} (nix-user)")); }

    let pacman = pacman_count();
    if pacman > 0 { parts.push(format!("{pacman} (pacman)")); }

    if let Ok(c) = fs::read_to_string("/var/lib/dpkg/status") {
        let n = dpkg_installed(&c);
        if n > 0 { parts.push(format!("{n} (dpkg)")); }
    }
    if Path::new("/var/lib/rpm/rpmdb.sqlite").exists() {
        let n = count_sqlite("/var/lib/rpm/rpmdb.sqlite", "Sigmd5")
            .or_else(|| count_sqlite("/var/lib/rpm/rpmdb.sqlite", "Packages"));
        if let Some(n) = n.filter(|n| *n > 0) { parts.push(format!("{n} (rpm)")); }
    }
    if let Ok(c) = fs::read_to_string("/lib/apk/db/installed") {
        let n = c.lines().filter(|l| l.starts_with("P:")).count();
        if n > 0 { parts.push(format!("{n} (apk)")); }
    }
    if let Some(n) = count_gentoo() { parts.push(format!("{n} (portage)")); }
    if let Some(n) = count_xbps() { parts.push(format!("{n} (xbps)")); }
    if let Some(n) = count_bedrock() { parts.push(format!("{n} (bedrock)")); }
    if let Some((n, manager)) = platform_count() { parts.push(format!("{n} ({manager})")); }

    if parts.is_empty() { "unknown".into() } else { parts.join(", ") }
}

/// Package count from the native manager on macOS/BSD/Android.
#[cfg(not(target_os = "linux"))]
fn platform_count() -> Option<(usize, &'static str)> {
    #[allow(unused_variables)]
    let dirs = |base: &str, label: &'static str| -> Option<(usize, &'static str)> {
        let rd = fs::read_dir(base).ok()?;
        let n = rd.flatten().filter(|e| e.path().is_dir()).count();
        (n > 0).then_some((n, label))
    };

    #[cfg(target_os = "macos")]
    for (base, label) in [
        ("/opt/homebrew/Cellar", "brew"),
        ("/usr/local/Cellar", "brew"),
        ("/opt/local/var/macports/software", "macports"),
    ] {
        if let Some(found) = dirs(base, label) { return Some(found); }
    }

    #[cfg(any(target_os = "freebsd", target_os = "openbsd",
              target_os = "netbsd", target_os = "dragonfly"))]
    if let Some(found) = dirs("/var/db/pkg", "pkg") { return Some(found); }

    let count = |program: &str, args: &[&str], label: &'static str,
                 keep: fn(&str) -> bool| -> Option<(usize, &'static str)> {
        let out = Command::new(program).args(args).output().ok()?;
        if !out.status.success() { return None; }
        let n = String::from_utf8_lossy(&out.stdout).lines().filter(|l| keep(l)).count();
        (n > 0).then_some((n, label))
    };
    count("brew", &["list", "--formula"], "brew", |l| !l.trim().is_empty())
        .or_else(|| count("pkg", &["info"], "pkg", |l| !l.trim().is_empty()))
        .or_else(|| count("pkg_info", &[], "pkg_info", |l| !l.trim().is_empty()))
        .or_else(|| count("pm", &["list", "packages"], "pm", |l| l.starts_with("package:")))
        .or_else(|| count("/system/bin/pm", &["list", "packages"], "pm", |l| l.starts_with("package:")))
        .or_else(|| count("port", &["installed"], "macports", |l| l.contains('@')))
}

/// Linux keeps its existing count paths; no extra manager probes.
#[cfg(target_os = "linux")]
fn platform_count() -> Option<(usize, &'static str)> { None }

fn appimage_count(home: &Path) -> usize {
    let mut n = 0;
    for dir in ["AppImages", "Applications"] {
        if let Ok(rd) = fs::read_dir(home.join(dir)) {
            n += rd.flatten().filter(|e| {
                let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                !name.starts_with('.') && name.ends_with(".appimage")
            }).count();
        }
    }
    n
}

fn flatpak_count(base: &Path) -> usize {
    let root = base.join("flatpak");
    let mut n = 0;

    if let Ok(rd) = fs::read_dir(root.join("app")) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with('.') { continue; }
            if e.path().join("current").exists() { n += 1; }
        }
    }

    if let Ok(rd) = fs::read_dir(root.join("runtime")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') { continue; }
            let Some(dot) = name.rfind('.') else { continue; };
            let kind = &name[dot + 1..];
            if kind == "Locale" || kind == "Debug" { continue; }
            let Ok(archs) = fs::read_dir(e.path()) else { continue };
            for arch in archs.flatten() {
                if !arch.path().is_dir() || arch.file_name().to_string_lossy().starts_with('.') { continue; }
                if let Ok(entries) = fs::read_dir(arch.path()) {
                    n += entries.flatten().filter(|x| {
                        x.path().is_dir() && !x.file_name().to_string_lossy().starts_with('.')
                    }).count();
                }
            }
        }
    }
    n
}

fn nix_count(path: &Path) -> usize {
    if !path.is_dir() { return 0; }
    let Ok(out) = Command::new("nix-store")
        .args(["--query", "--requisites"])
        .arg(path)
        .output() else { return 0 };
    if !out.status.success() { return 0; }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| is_valid_nix_pkg(line))
        .count()
}

fn is_valid_nix_pkg(path: &str) -> bool {
    let p = Path::new(path);
    if !p.is_dir() { return false; }
    let Some(name) = p.file_name().and_then(|n| n.to_str()) else { return false; };
    if name.starts_with("nixos-system-nixos-")
        || ["-doc", "-man", "-info", "-dev", "-bin"].iter().any(|s| name.ends_with(s))
    {
        return false;
    }
    // Version marker: a digit run followed by a dot and another digit.
    let b = name.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() { j += 1; }
            if j + 1 < b.len() && b[j] == b'.' && b[j + 1].is_ascii_digit() { return true; }
        }
        i += 1;
    }
    false
}

fn pacman_count() -> usize {
    fs::read_dir("/var/lib/pacman/local")
        .map(|d| d.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0)
}

pub fn count() -> String {
    // 1. Arch — first, fastest, always prioritized
    if let Ok(d) = fs::read_dir("/var/lib/pacman/local") {
        let n = d.count().saturating_sub(1);
        if n > 0 { return format!("{n} (pacman)"); }
    }

    // 2. Debian / Ubuntu
    if let Ok(c) = fs::read_to_string("/var/lib/dpkg/status") {
        let n = dpkg_installed(&c);
        if n > 0 { return format!("{n} (dpkg)"); }
    }

    // 3. Fedora / RHEL
    if Path::new("/var/lib/rpm/rpmdb.sqlite").exists() {
        if let Some(n) = count_sqlite("/var/lib/rpm/rpmdb.sqlite", "Packages") {
            if n > 0 { return format!("{n} (rpm)"); }
        }
    }

    // 4. NixOS
    if Path::new("/nix/var/nix/db/db.sqlite").exists() {
        if let Some(n) = count_sqlite("/nix/var/nix/db/db.sqlite", "ValidPaths") {
            if n > 0 { return format!("{n} (nix)"); }
        }
    }

    // 5. Alpine
    if let Ok(c) = fs::read_to_string("/lib/apk/db/installed") {
        let n = c.lines().filter(|l| l.starts_with("P:")).count();
        if n > 0 { return format!("{n} (apk)"); }
    }

    // 6. Gentoo
    if Path::new("/var/db/pkg").is_dir() {
        if let Some(n) = count_gentoo() {
            if n > 0 { return format!("{n} (portage)"); }
        }
    }

    // 7. Void
    if let Some(n) = count_xbps() {
        if n > 0 { return format!("{n} (xbps)"); }
    }

    // 8. Bedrock
    if Path::new("/bedrock/strata").is_dir() {
        if let Some(n) = count_bedrock() {
            if n > 0 { return format!("{n} (bedrock)"); }
        }
    }

    // 9. macOS / BSD / Android
    if let Some((n, manager)) = platform_count() {
        return format!("{n} ({manager})");
    }

    "unknown".into()
}

// ── Hand-rolled SQLite COUNT reader ──────────────────────
// Only handles read-only COUNT queries — no SQL parsing, no external deps.

fn count_sqlite(path: &str, table: &str) -> Option<usize> {
    let data = fs::read(path).ok()?;
    if data.len() < 100 || &data[..16] != b"SQLite format 3\0" { return None; }

    let ps = match u16::from_be_bytes([data[16], data[17]]) {
        1 => 65536,
        n => n as u32,
    };
    if !(512..=65536).contains(&ps) || !ps.is_power_of_two() { return None; }

    let main_pages = (data.len() / ps as usize) as u32;
    let (wal, wal_pages) = read_wal(path, ps);
    let db = Db {
        data: &data,
        ps,
        wal,
        pages: if wal_pages == 0 { main_pages } else { wal_pages },
    };
    let root = find_table_root(&db, table)?;
    count_btree(&db, root, 0)
}

struct Db<'a> {
    data: &'a [u8],
    ps: u32,
    wal: HashMap<u32, Vec<u8>>,
    pages: u32,
}

// Page 1 includes the 100-byte database header; cell offsets are still from
// the start of the page, so callers offset their b-tree header by 100 there.
fn page<'a>(db: &'a Db<'_>, n: u32) -> Option<&'a [u8]> {
    if n == 0 { return None; }
    if let Some(p) = db.wal.get(&n) { return Some(p); }
    if n > db.pages { return None; }
    let start = (n as usize - 1).checked_mul(db.ps as usize)?;
    let end = start.checked_add(db.ps as usize)?;
    db.data.get(start..end)
}

// Read committed frames from the write-ahead log. Frames are only applied up
// to the last commit marker; checksum/salt failures mean the tail is unusable.
fn read_wal(path: &str, ps: u32) -> (HashMap<u32, Vec<u8>>, u32) {
    let data = match fs::read(format!("{path}-wal")) { Ok(d) => d, Err(_) => return Default::default() };
    if data.len() < 32 { return Default::default(); }

    let be32 = |o: usize| u32::from_be_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    let magic = be32(0);
    if magic != 0x377f0682 && magic != 0x377f0683 { return Default::default(); }
    if be32(8) != ps { return Default::default(); }
    let big = magic == 0x377f0683;
    let salt1 = be32(16);
    let salt2 = be32(20);
    if wal_cksum(&data[..24], 0, 0, big) != (be32(24), be32(28)) {
        return Default::default();
    }

    let mut state = (be32(24), be32(28));
    let mut frames: Vec<(u32, u32, usize)> = Vec::new();
    let mut off = 32usize;
    while off + 24 + ps as usize <= data.len() {
        let pno = be32(off);
        let dbsize = be32(off + 4);
        if be32(off + 8) != salt1 || be32(off + 12) != salt2 { break; }
        let page_off = off + 24;
        let ck = wal_cksum(&data[off..off + 8], state.0, state.1, big);
        let ck = wal_cksum(&data[page_off..page_off + ps as usize], ck.0, ck.1, big);
        if ck != (be32(off + 16), be32(off + 20)) { break; }
        state = ck;
        frames.push((pno, dbsize, page_off));
        off = page_off + ps as usize;
    }

    let Some(last) = frames.iter().rposition(|f| f.1 != 0) else {
        return Default::default();
    };
    let pages = frames[last].1;
    let mut wal = HashMap::new();
    for &(pno, _, page_off) in &frames[..=last] {
        if pno > 0 && pno <= pages {
            wal.insert(pno, data[page_off..page_off + ps as usize].to_vec());
        }
    }
    (wal, pages)
}

fn wal_cksum(data: &[u8], mut s0: u32, mut s1: u32, big: bool) -> (u32, u32) {
    for c in data.chunks_exact(8) {
        let (x0, x1) = if big {
            (u32::from_be_bytes([c[0], c[1], c[2], c[3]]),
             u32::from_be_bytes([c[4], c[5], c[6], c[7]]))
        } else {
            (u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
             u32::from_le_bytes([c[4], c[5], c[6], c[7]]))
        };
        s0 = s0.wrapping_add(x0).wrapping_add(s1);
        s1 = s1.wrapping_add(x1).wrapping_add(s0);
    }
    (s0, s1)
}

fn varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for (i, &byte) in data.iter().enumerate().take(9) {
        v = (v << 7) | (byte & 0x7F) as u64;
        if byte & 0x80 == 0 { return Some((v, i + 1)); }
    }
    None
}

fn serial_len(t: u64) -> usize {
    match t {
        0 => 0,
        1 => 1, 2 => 2, 3 => 3, 4 => 4, 5 => 6, 6 => 8, 7 => 8,
        8 | 9 => 0,
        n if n >= 12 => ((n - 12) / 2) as usize,
        _ => 0,
    }
}

fn read_int(data: &[u8], pos: &mut usize, t: u64) -> Option<u64> {
    let len = serial_len(t);
    if *pos + len > data.len() { return None; }
    let v = match t {
        0 => 0,
        1 => data[*pos] as u64,
        2 => u16::from_be_bytes([data[*pos], data[*pos + 1]]) as u64,
        3 => u32::from_be_bytes([0, data[*pos], data[*pos + 1], data[*pos + 2]]) as u64,
        4 => u32::from_be_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]) as u64,
        5 => u64::from_be_bytes([0, 0, data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3], data[*pos + 4], data[*pos + 5]]),
        6 => u64::from_be_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3], data[*pos + 4], data[*pos + 5], data[*pos + 6], data[*pos + 7]]),
        8 => 0,
        9 => 1,
        _ => 0,
    };
    *pos += len;
    Some(v)
}

fn read_text(data: &[u8], pos: &mut usize, t: u64) -> Option<String> {
    if t < 13 || t.is_multiple_of(2) { return None; }
    let len = ((t - 13) / 2) as usize;
    if *pos + len > data.len() { return None; }
    let s = String::from_utf8(data[*pos..*pos + len].to_vec()).ok()?;
    *pos += len;
    Some(s)
}

fn find_table_root(db: &Db, table: &str) -> Option<u32> {
    // sqlite_schema is always on page 1
    // Walk its B-tree looking for tbl_name == table
    let mut stack = vec![1u32];
    let mut seen = std::collections::HashSet::new();
    while let Some(pn) = stack.pop() {
        if !seen.insert(pn) { continue; }
        let pg = page(db, pn)?;
        let h = if pn == 1 { 100 } else { 0 };
        let pt = *pg.get(h)?;
        let nc = u16::from_be_bytes([*pg.get(h + 3)?, *pg.get(h + 4)?]) as usize;

        match pt {
            0x0D => {
                // leaf — examine cells
                let hdr = h + 8;
                for i in 0..nc {
                    let p = hdr + i * 2;
                    let off = u16::from_be_bytes([*pg.get(p)?, *pg.get(p + 1)?]) as usize;
                    if let Some((name, root)) = parse_schema_cell(pg.get(off..)?) {
                        if name == table { return Some(root); }
                    }
                }
            }
            0x05 => {
                // interior — push children to stack
                let hdr = h + 12;
                for i in (0..nc).rev() {
                    let p = hdr + i * 2;
                    let off = u16::from_be_bytes([*pg.get(p)?, *pg.get(p + 1)?]) as usize;
                    let cell = pg.get(off..off + 4)?;
                    stack.push(u32::from_be_bytes([cell[0], cell[1], cell[2], cell[3]]));
                }
                let rc = u32::from_be_bytes([
                    *pg.get(h + 8)?, *pg.get(h + 9)?,
                    *pg.get(h + 10)?, *pg.get(h + 11)?,
                ]);
                stack.push(rc);
            }
            _ => {}
        }
    }
    None
}

fn parse_schema_cell(cell: &[u8]) -> Option<(String, u32)> {
    let mut pos = 0;
    let (_, n1) = varint(cell)?;
    pos += n1;
    let (_, n2) = varint(cell.get(pos..)?)?;
    pos += n2;

    let (hs, n3) = varint(cell.get(pos..)?)?;
    pos += n3;
    let hdr_end = pos.checked_add(hs as usize)?.checked_sub(n3)?;
    if hdr_end > cell.len() { return None; }

    let mut types = Vec::new();
    while pos < hdr_end {
        let (t, n) = varint(cell.get(pos..)?)?;
        types.push(t);
        pos += n;
    }

    if types.len() < 5 { return None; }

    skip_value(cell, &mut pos, types[0])?; // type
    skip_value(cell, &mut pos, types[1])?; // name
    let tbl_name = read_text(cell, &mut pos, types[2])?;
    let rootpage = read_int(cell, &mut pos, types[3])? as u32;

    Some((tbl_name, rootpage))
}

fn skip_value(data: &[u8], pos: &mut usize, t: u64) -> Option<()> {
    *pos = pos.checked_add(serial_len(t))?;
    (*pos <= data.len()).then_some(())
}

fn count_btree(db: &Db, pn: u32, depth: usize) -> Option<usize> {
    if depth > 64 { return None; }
    let pg = page(db, pn)?;
    let h = if pn == 1 { 100 } else { 0 };
    let pt = *pg.get(h)?;
    let nc = u16::from_be_bytes([*pg.get(h + 3)?, *pg.get(h + 4)?]) as usize;

    match pt {
        0x0D => Some(nc),  // leaf — cells are actual rows
        0x05 => {
            // interior — recurse
            let hdr = h + 12;
            let mut total = 0usize;
            for i in 0..nc {
                let p = hdr + i * 2;
                let off = u16::from_be_bytes([*pg.get(p)?, *pg.get(p + 1)?]) as usize;
                let cell = pg.get(off..off + 4)?;
                let child = u32::from_be_bytes([cell[0], cell[1], cell[2], cell[3]]);
                total += count_btree(db, child, depth + 1)?;
            }
            let rc = u32::from_be_bytes([
                *pg.get(h + 8)?, *pg.get(h + 9)?,
                *pg.get(h + 10)?, *pg.get(h + 11)?,
            ]);
            total += count_btree(db, rc, depth + 1)?;
            Some(total)
        }
        _ => None,
    }
}

// ── End hand-rolled SQLite reader ─────────────────────────

fn dpkg_installed(status: &str) -> usize {
    status.lines().filter(|l| {
        l.strip_prefix("Status:")
            .and_then(|s| s.split_whitespace().nth(2)) == Some("installed")
    }).count()
}

#[cfg(target_os = "linux")]
fn count_gentoo() -> Option<usize> {
    let mut total = 0usize;
    if let Ok(cats) = fs::read_dir("/var/db/pkg") {
        for cat in cats.flatten() {
            if cat.path().is_dir() {
                if let Ok(pkgs) = fs::read_dir(cat.path()) {
                    total += pkgs.count();
                }
            }
        }
    }
    if total > 0 { Some(total) } else { None }
}

/// `/var/db/pkg` is the BSD package database, not Gentoo's category tree.
#[cfg(not(target_os = "linux"))]
fn count_gentoo() -> Option<usize> { None }

fn count_xbps() -> Option<usize> {
    let out = Command::new("xbps-query").args(["-l"]).output().ok()?;
    if out.status.success() {
        let s = String::from_utf8(out.stdout).ok()?;
        Some(s.lines().count())
    } else {
        None
    }
}

fn count_bedrock() -> Option<usize> {
    let mut total = 0usize;
    for entry in fs::read_dir("/bedrock/strata").ok()?.flatten() {
        let s = entry.path();
        if !s.is_dir() { continue; }
        let p = s.join("var/lib/pacman/local");
        if p.is_dir() {
            if let Ok(d) = fs::read_dir(&p) {
                total += d.count().saturating_sub(1);
                continue;
            }
        }
        let p = s.join("var/lib/dpkg/status");
        if p.is_file() {
            if let Ok(c) = fs::read_to_string(&p) {
                total += dpkg_installed(&c);
                continue;
            }
        }
        let p = s.join("var/db/pkg");
        if p.is_dir() {
            if let Ok(cats) = fs::read_dir(&p) {
                for cat in cats.flatten() {
                    if let Ok(pkgs) = fs::read_dir(cat.path()) {
                        total += pkgs.count();
                    }
                }
            }
        }
    }
    if total > 0 { Some(total) } else { None }
}

/// Keep only managers not listed in fastfetch's `packages.disabled`.
/// A plain family name (`flatpak`, `nix`) also drops its sub-managers.
pub fn filter(list: &str, disabled: &[String]) -> String {
    if disabled.is_empty() { return list.to_string(); }
    let keep = |mgr: &str| {
        let mgr = mgr.replace('_', "-");
        !disabled.iter().any(|d| {
            let d = d.trim().to_ascii_lowercase().replace('_', "-");
            // fastfetch names the portage counter `emerge`
            let d = if d == "emerge" { "portage".to_string() } else { d };
            mgr == d || mgr.starts_with(&format!("{d}-"))
        })
    };
    let out: Vec<&str> = list.split(',')
        .filter_map(|part| {
            let part = part.trim();
            let open = part.find('(')?;
            let mgr = part[open + 1..].trim_end_matches(')').trim();
            keep(mgr).then_some(part)
        })
        .collect();
    out.join(", ")
}

/// Merge package-manager families for fastfetch's `packages.combined`:
/// `flatpak-system`/`flatpak-user` → `flatpak`, `nix-*` → `nix`.
pub fn combine(list: &str) -> String {
    let mut out: Vec<(String, u64)> = Vec::new();
    for part in list.split(',') {
        let Some(open) = part.find('(') else { continue };
        let Some(count) = part[..open].trim().parse::<u64>().ok() else { continue };
        let mgr = part[open + 1..].trim_end_matches(')').trim();
        let family = if mgr.starts_with("flatpak") {
            "flatpak"
        } else if mgr.starts_with("nix") {
            "nix"
        } else {
            mgr
        };
        match out.iter_mut().find(|(n, _)| n == family) {
            Some((_, c)) => *c += count,
            None => out.push((family.to_string(), count)),
        }
    }
    out.iter().map(|(n, c)| format!("{c} ({n})")).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::combine;

    #[test]
    fn combine_merges_flatpak_and_nix_families() {
        assert_eq!(
            combine("6 (appimage), 12 (flatpak-system), 19 (flatpak-user), 61 (nix-default), 2309 (pacman)"),
            "6 (appimage), 31 (flatpak), 61 (nix), 2309 (pacman)"
        );
    }

    #[test]
    fn disabled_hides_managers_and_families() {
        use super::filter;
        let all = "6 (appimage), 12 (flatpak-system), 19 (flatpak-user), 61 (nix-default), 2 (portage), 2309 (pacman)";
        assert_eq!(filter(all, &["appimage".into(), "nix".into()]),
                   "12 (flatpak-system), 19 (flatpak-user), 2 (portage), 2309 (pacman)");
        assert_eq!(filter(all, &["flatpak".into(), "emerge".into()]),
                   "6 (appimage), 61 (nix-default), 2309 (pacman)");
        assert_eq!(filter(all, &["pacman".into()]), "6 (appimage), 12 (flatpak-system), 19 (flatpak-user), 61 (nix-default), 2 (portage)");
        assert_eq!(filter(all, &[]), all);
    }
}
