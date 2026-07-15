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
    let _ = fs::create_dir_all(&pdir);
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
