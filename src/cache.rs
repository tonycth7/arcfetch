use std::{env, fs, time::{SystemTime, UNIX_EPOCH}};

const CACHE_DIR: &str = "/dev/shm/arcfetch";

fn persistent_dir() -> &'static str {
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        format!("{home}/.cache/arcfetch")
    })
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub fn get(key: &str, ttl_secs: u64) -> Option<String> {
    // Tier 1: /dev/shm/arcfetch/ (instant, same-boot)
    let shm_path = format!("{CACHE_DIR}/{key}");
    if let Ok(v) = fs::read_to_string(&shm_path) {
        let v = v.trim().to_string();
        if !v.is_empty() { return Some(v); }
    }

    // Tier 2: ~/.cache/arcfetch/ (persistent across reboots)
    let pdir = persistent_dir();
    let cache_path = format!("{pdir}/{key}");
    let meta = fs::metadata(&cache_path).ok()?;
    let modified = meta.modified().ok()?;
    let age = now_secs().saturating_sub(modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
    if age > ttl_secs { return None; }

    fs::read_to_string(&cache_path).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn set(key: &str, value: &str) {
    // Write to /dev/shm/ (tmpfs)
    let _ = fs::create_dir_all(CACHE_DIR);
    let shm_path = format!("{CACHE_DIR}/{key}");
    let _ = fs::write(&shm_path, value);

    // Write to ~/.cache/arcfetch/ (persistent)
    let pdir = persistent_dir();
    let _ = fs::create_dir_all(pdir);
    let cache_path = format!("{pdir}/{key}");
    let _ = fs::write(&cache_path, value);
}

pub fn get_or_compute(key: &str, ttl_secs: u64, f: impl Fn() -> String) -> String {
    if let Some(v) = get(key, ttl_secs) {
        return v;
    }
    let v = f();
    set(key, &v);
    v
}

/// Like [`get_or_compute`], but `ttl_secs` is enforced against the file mtime
/// in **both** tiers. Use this for volatile data (network lookups): the
/// same-boot `/dev/shm` tier must still expire.
#[allow(dead_code)]
pub fn get_or_compute_ttl(key: &str, ttl_secs: u64, f: impl Fn() -> String) -> String {
    if let Some(v) = get_fresh(key, ttl_secs) {
        return v;
    }
    let v = f();
    set(key, &v);
    v
}

/// Any cached value ignoring its TTL, with the age (seconds) of the freshest
/// tier, so callers can decide to serve stale data while refreshing.
fn get_any(key: &str) -> Option<(String, u64)> {
    let mut best: Option<(String, u64)> = None;
    for path in [
        format!("{CACHE_DIR}/{key}"),
        format!("{}/{key}", persistent_dir()),
    ] {
        let Ok(meta) = fs::metadata(&path) else { continue };
        let Ok(modified) = meta.modified() else { continue };
        let age = now_secs()
            .saturating_sub(modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
        if let Ok(v) = fs::read_to_string(&path) {
            let v = v.trim().to_string();
            if !v.is_empty() && best.as_ref().map(|(_, a)| age < *a).unwrap_or(true) {
                best = Some((v, age));
            }
        }
    }
    best
}

/// Stale-while-revalidate: return a cached value immediately, even once it is
/// stale, and kick off a detached background refresh for the next run. Blocks
/// (and computes) only when nothing has been cached yet. This keeps repeated
/// `--all` runs fast without ever showing a stale value forever.
pub fn get_or_compute_swr(key: &str, ttl_secs: u64, f: impl Fn() -> String) -> String {
    if let Some((v, age)) = get_any(key) {
        if age > ttl_secs {
            spawn_refresh();
        }
        return v;
    }
    let v = f();
    set(key, &v);
    v
}

/// Spawn a throttled, detached `arcfetch` that repopulates the slow network
/// caches (selected via `ARCFETCH_REFRESH_CACHE`), so the next run is warm.
fn spawn_refresh() {
    // At most one background refresher every 20 s, so a tight loop of `--all`
    // runs does not fork-bomb the machine.
    let marker = format!("{CACHE_DIR}/.refresh");
    if let Ok(meta) = fs::metadata(&marker) {
        if let Ok(m) = meta.modified() {
            let age = now_secs()
                .saturating_sub(m.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
            if age < 20 { return; }
        }
    }
    let Ok(exe) = env::current_exe() else { return };
    let _ = fs::create_dir_all(CACHE_DIR);
    let _ = fs::write(&marker, "");
    let _ = std::process::Command::new(exe)
        .env("ARCFETCH_REFRESH_CACHE", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[allow(dead_code)]
fn get_fresh(key: &str, ttl_secs: u64) -> Option<String> {
    for path in [
        format!("{CACHE_DIR}/{key}"),
        format!("{}/{key}", persistent_dir()),
    ] {
        let Ok(meta) = fs::metadata(&path) else { continue };
        let Ok(modified) = meta.modified() else { continue };
        let age = now_secs()
            .saturating_sub(modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
        if age > ttl_secs {
            continue;
        }
        if let Ok(v) = fs::read_to_string(&path) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}
