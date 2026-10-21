use std::{env, fs, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};
use std::os::unix::fs::PermissionsExt;

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

fn shm_dir() -> &'static str {
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| format!("/dev/shm/arcfetch-{}", uid()))
}

fn persistent_dir() -> &'static str {
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let base = env::var("XDG_CACHE_HOME").ok().filter(|s| !s.is_empty())
            .or_else(|| env::var("HOME").ok().map(|h| format!("{h}/.cache")))
            .unwrap_or_else(|| format!("{}/arcfetch-{}", env::temp_dir().to_string_lossy(), uid()));
        format!("{base}/arcfetch")
    })
}

fn ensure_dir(path: &str) {
    if fs::create_dir_all(path).is_ok() {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
}

// Temp file + rename, so a concurrent reader never sees a half-written value.
fn write_atomic(path: &str, value: &str, mtime: Option<SystemTime>) {
    let tmp = format!(
        "{path}.{}-{}.tmp",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut f = match fs::File::create(&tmp) { Ok(f) => f, Err(_) => return };
    use std::io::Write;
    if f.write_all(value.as_bytes()).is_err() {
        let _ = fs::remove_file(&tmp);
        return;
    }
    if let Some(m) = mtime { let _ = f.set_modified(m); }
    drop(f);
    if fs::rename(&tmp, path).is_err() { let _ = fs::remove_file(&tmp); }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

// A clock skew can put mtime in the future; treat that as infinitely stale.
fn age_secs(modified: SystemTime) -> u64 {
    let m = modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let now = now_secs();
    if m > now { u64::MAX } else { now - m }
}

pub fn get(key: &str, ttl_secs: u64) -> Option<String> {
    // Tier 1: /dev/shm/arcfetch/ (instant, same-boot)
    let shm_path = format!("{}/{key}", shm_dir());
    if let Ok(v) = fs::read_to_string(&shm_path) {
        let v = v.trim().to_string();
        if !v.is_empty() { return Some(v); }
    }

    // Tier 2: ~/.cache/arcfetch/ (persistent across reboots)
    let cache_path = format!("{}/{key}", persistent_dir());
    let meta = fs::metadata(&cache_path).ok()?;
    let modified = meta.modified().ok()?;
    if age_secs(modified) > ttl_secs { return None; }

    let v = fs::read_to_string(&cache_path).ok()?.trim().to_string();
    if v.is_empty() { return None; }
    // Promote into tmpfs so the rest of this boot skips the persistent stat+read.
    ensure_dir(shm_dir());
    write_atomic(&shm_path, &v, Some(modified));
    Some(v)
}

pub fn set(key: &str, value: &str) {
    // Write to /dev/shm/ (tmpfs)
    ensure_dir(shm_dir());
    let shm_path = format!("{}/{key}", shm_dir());
    write_atomic(&shm_path, value, None);

    // Write to ~/.cache/arcfetch/ (persistent)
    ensure_dir(persistent_dir());
    let cache_path = format!("{}/{key}", persistent_dir());
    write_atomic(&cache_path, value, None);
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
        format!("{}/{key}", shm_dir()),
        format!("{}/{key}", persistent_dir()),
    ] {
        let Ok(meta) = fs::metadata(&path) else { continue };
        let Ok(modified) = meta.modified() else { continue };
        let age = age_secs(modified);
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
/// (and computes) only when nothing has been cached yet. Failures are not
/// cached, so a transient network error does not pin `N/A` forever.
pub fn get_or_compute_swr(key: &str, ttl_secs: u64, f: impl Fn() -> Option<String>) -> String {
    if let Some((v, age)) = get_any(key) {
        if age > ttl_secs {
            spawn_refresh();
        }
        return v;
    }
    match f() {
        Some(v) => { set(key, &v); v }
        None => "N/A".into(),
    }
}

/// Spawn a throttled, detached `arcfetch` that repopulates the slow network
/// caches (selected via `ARCFETCH_REFRESH_CACHE`), so the next run is warm.
fn spawn_refresh() {
    // At most one background refresher every 20 s, so a tight loop of `--all`
    // runs does not fork-bomb the machine.
    let marker = format!("{}/.refresh", shm_dir());
    if let Ok(meta) = fs::metadata(&marker) {
        if let Ok(m) = meta.modified() {
            if age_secs(m) < 20 { return; }
        }
    }
    let Ok(exe) = env::current_exe() else { return };
    ensure_dir(shm_dir());
    write_atomic(&marker, "", None);
    let _ = std::process::Command::new(exe)
        .env("ARCFETCH_REFRESH_CACHE", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[allow(dead_code)]
fn get_fresh(key: &str, ttl_secs: u64) -> Option<String> {
    let shm_path = format!("{}/{key}", shm_dir());

    // Tier 1: /dev/shm/arcfetch/ — TTL enforced here too.
    if let Ok(meta) = fs::metadata(&shm_path) {
        if let Ok(modified) = meta.modified() {
            if age_secs(modified) <= ttl_secs {
                if let Ok(v) = fs::read_to_string(&shm_path) {
                    let v = v.trim().to_string();
                    if !v.is_empty() { return Some(v); }
                }
            }
        }
    }

    // Tier 2: ~/.cache/arcfetch/ — promote into tmpfs for the rest of this boot,
    // preserving the original mtime so the TTL is not stretched.
    let cache_path = format!("{}/{key}", persistent_dir());
    let meta = fs::metadata(&cache_path).ok()?;
    let modified = meta.modified().ok()?;
    if age_secs(modified) > ttl_secs { return None; }
    let v = fs::read_to_string(&cache_path).ok()?.trim().to_string();
    if v.is_empty() { return None; }
    ensure_dir(shm_dir());
    write_atomic(&shm_path, &v, Some(modified));
    Some(v)
}
