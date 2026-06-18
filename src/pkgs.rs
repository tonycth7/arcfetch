use std::{fs, path::Path, process::Command};

pub fn count() -> String {
    // 1. Arch — first, fastest, always prioritized
    if let Ok(d) = fs::read_dir("/var/lib/pacman/local") {
        let n = d.count().saturating_sub(1);
        if n > 0 { return format!("{n} (pacman)"); }
    }

    // 2. Debian / Ubuntu
    if let Ok(c) = fs::read_to_string("/var/lib/dpkg/status") {
        let n = c.lines().filter(|l| l.starts_with("Package:")).count();
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

    let root = find_table_root(&data, ps, table)?;
    count_btree(&data, ps, root)
}

fn page<'a>(data: &'a [u8], ps: u32, n: u32) -> Option<&'a [u8]> {
    if n == 0 { return None; }
    let start = (n as usize - 1) * ps as usize;
    if start + ps as usize > data.len() { return None; }
    if n == 1 {
        Some(&data[start + 100..start + ps as usize])
    } else {
        Some(&data[start..start + ps as usize])
    }
}

fn varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for i in 0..9.min(data.len()) {
        v = (v << 7) | (data[i] & 0x7F) as u64;
        if data[i] & 0x80 == 0 { return Some((v, i + 1)); }
    }
    None
}

fn serial_len(t: u64) -> usize {
    match t {
        0 => 0,
        1 => 1, 2 => 2, 3 => 3, 4 => 4, 5 => 6, 6 => 8, 7 => 8,
        8 | 9 => 0,
        n if n >= 12 => ((n - 12) / 2 + 1) as usize,
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
    if t < 13 || t % 2 == 0 { return None; }
    let len = ((t - 13) / 2) as usize;
    if *pos + len > data.len() { return None; }
    let s = String::from_utf8(data[*pos..*pos + len].to_vec()).ok()?;
    *pos += len;
    Some(s)
}

fn find_table_root(data: &[u8], ps: u32, table: &str) -> Option<u32> {
    // sqlite_schema is always on page 1
    // Walk its B-tree looking for tbl_name == table
    let mut stack = vec![1u32];
    while let Some(pn) = stack.pop() {
        let pg = page(data, ps, pn)?;
        let pt = pg[0];
        let nc = u16::from_be_bytes([pg[3], pg[4]]) as usize;

        match pt {
            0x0D => {
                // leaf — examine cells
                let hdr = 8;
                for i in 0..nc {
                    let off = u16::from_be_bytes([pg[hdr + i * 2], pg[hdr + i * 2 + 1]]) as usize;
                    let cell = &pg[off..];
                    if let Some((name, root)) = parse_schema_cell(cell) {
                        if name == table { return Some(root); }
                    }
                }
            }
            0x05 => {
                // interior — push children to stack
                let hdr = 12;
                for i in (0..nc).rev() {
                    let off = u16::from_be_bytes([pg[hdr + i * 2], pg[hdr + i * 2 + 1]]) as usize;
                    let child = u32::from_be_bytes([pg[off], pg[off + 1], pg[off + 2], pg[off + 3]]);
                    stack.push(child);
                }
                let rc = u32::from_be_bytes([pg[8], pg[9], pg[10], pg[11]]);
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
    let (_, n2) = varint(&cell[pos..])?;
    pos += n2;

    let (hs, n3) = varint(&cell[pos..])?;
    pos += n3;
    let hdr_end = pos + hs as usize - n3;

    let mut types = Vec::new();
    while pos < hdr_end {
        let (t, n) = varint(&cell[pos..])?;
        types.push(t);
        pos += n;
    }

    if types.len() < 5 { return None; }

    skip_value(cell, &mut pos, types[0]); // type
    skip_value(cell, &mut pos, types[1]); // name
    let tbl_name = read_text(cell, &mut pos, types[2])?;
    let rootpage = read_int(cell, &mut pos, types[3])? as u32;

    Some((tbl_name, rootpage))
}

fn skip_value(_data: &[u8], pos: &mut usize, t: u64) {
    *pos += serial_len(t);
}

fn count_btree(data: &[u8], ps: u32, pn: u32) -> Option<usize> {
    let pg = page(data, ps, pn)?;
    let pt = pg[0];
    let nc = u16::from_be_bytes([pg[3], pg[4]]) as usize;

    match pt {
        0x0D => Some(nc),  // leaf — cells are actual rows
        0x05 => {
            // interior — recurse
            let hdr = 12;
            let mut total = 0usize;
            for i in 0..nc {
                let off = u16::from_be_bytes([pg[hdr + i * 2], pg[hdr + i * 2 + 1]]) as usize;
                let child = u32::from_be_bytes([pg[off], pg[off + 1], pg[off + 2], pg[off + 3]]);
                total += count_btree(data, ps, child)?;
            }
            let rc = u32::from_be_bytes([pg[8], pg[9], pg[10], pg[11]]);
            total += count_btree(data, ps, rc)?;
            Some(total)
        }
        _ => None,
    }
}

// ── End hand-rolled SQLite reader ─────────────────────────

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
                total += c.lines().filter(|l| l.starts_with("Package:")).count();
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
