// arcfetch — blazing-fast Arch Linux sysinfo 
// arcfetch [-h] [-V] [--blackhole [--t N]] [--logo NAME] [--accent COLOR]
//          [--no-color] [--config]
mod cache;
mod config;
mod cosmic;
mod info;
mod logos;
mod mandelbrot;
mod modules;
mod net;
mod pkgs;

use std::{env, thread};
use std::f64::consts::PI;
use std::io::{Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use config::*;

// ═══════════════════════════════════════════════════════════
//  Render helpers
// ═══════════════════════════════════════════════════════════

fn mem_bar(used: u64, total: u64, w: usize, bar_color: &str) -> String {
    if total == 0 { return String::new(); }
    let fill = ((used as f64 / total as f64 * w as f64) as usize).min(w);
    format!("{bar}{f}{OVL}{e}{R}",
        bar = bar_color,
        f   = "\u{2588}".repeat(fill),
        OVL = OVERLAY0,
        e   = "\u{2591}".repeat(w - fill),
        R   = RESET)
}

fn row(label_color: &str, key: &str, val: &str) -> String {
    let mut s = String::with_capacity(120);
    s.push_str(BOLD); s.push_str(label_color); s.push_str(key);
    for _ in key.len()..9 { s.push(' '); }
    s.push_str(RESET); s.push(' ');
    s.push_str(label_color); s.push_str(val); s.push_str(RESET);
    s
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut esc = false;
    let mut osc = false;
    for ch in s.chars() {
        if ch == '\x1b' { esc = true; osc = false; continue; }
        if esc {
            if ch == '[' { continue; }  // CSI introducer
            if ch == ']' { osc = true; continue; }  // OSC introducer
            if osc {
                if ch == '\x1b' { continue; }
                if ch == '\\' || ch == '\x07' { osc = false; esc = false; }
                continue;
            }
            if ch == '\\' { esc = false; continue; }  // ST
            if ('\x40'..='\x7e').contains(&ch) { esc = false; }  // CSI final byte
            continue;
        }
        out.push(ch);
    }
    out
}

/// Count visible characters (ignoring ANSI escape sequences).
fn visible_chars(s: &str) -> usize {
    let mut n = 0;
    let mut esc = false;
    let mut osc = false;
    for ch in s.chars() {
        if ch == '\x1b' { esc = true; osc = false; continue; }
        if esc {
            if ch == '[' { continue; }
            if ch == ']' { osc = true; continue; }
            if osc {
                if ch == '\x1b' { continue; }
                if ch == '\\' || ch == '\x07' { osc = false; esc = false; }
                continue;
            }
            if ch == '\\' { esc = false; continue; }
            if ('\x40'..='\x7e').contains(&ch) { esc = false; }
            continue;
        }
        n += 1;
    }
    n
}

// ── Kitty image protocol ────────────────────────────────
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { B64[((triple >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[(triple & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn expand_path(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = env::var("HOME") {
            path.replacen('~', &home, 1)
        } else { path.to_string() }
    } else { path.to_string() }
}

fn kitty_image(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?.to_lowercase();
    let format = match ext.as_str() {
        "png" => 100u32,
        "jpg" | "jpeg" => 101,
        "gif" => 102,
        _ => return None,
    };
    let expanded = expand_path(path);
    let data = std::fs::read(&expanded).ok()?;
    let b64 = base64_encode(&data);
    Some(format!("\x1b_Ga=T,f={},c=31,r=19,d=A;{}\x1b\\", format, b64))
}

fn uptime_seed() -> usize {
    info::system_uptime_secs() as usize
}

/// System hostname via `gethostname(2)`, or `"localhost"` on failure.
fn hostname() -> String {
    let mut buf = [0 as libc::c_char; 256];
    // SAFETY: `buf` is a valid, writable buffer of `buf.len()` bytes.
    if unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len()) } == 0 {
        // SAFETY: `gethostname` NUL-terminates within `buf` on success.
        unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    } else {
        "localhost".into()
    }
}

fn science_quote(seed: usize) -> &'static str {
    const QUOTES: &[&str] = &[
        "\"imagination is more important than knowledge\" — Einstein",
        "\"if you can't explain it simply, you don't understand it\" — Feynman",
        "\"we are made of star stuff\" — Sagan",
        "\"the universe is under no obligation to make sense to you\" — Tyson",
        "\"an expert is a person who has made all the mistakes\" — Bohr",
        "\"what we know is a drop, what we don't know is an ocean\" — Newton",
        "\"the important thing is not to stop questioning\" — Einstein",
        "\"science is magic that works\" — Vonnegut",
        "\"nature uses only the longest threads to weave her patterns\" — Feynman",
        "\"the cosmos is within us. we are a way for the universe to know itself\" — Sagan",
        "\"not only is the universe stranger than we think, it is stranger than we can think\" — Heisenberg",
        "\"in physics, you don't have to go around making trouble for yourself — nature does it for you\" — Feynman",
    ];
    QUOTES[seed % QUOTES.len()]
}

fn science_logo(seed: usize) -> &'static str {
    ["dna", "atom", "wave", "emc2", "pi"][seed % 5]
}

fn detect_auto_logo(si: &info::SysInfo) -> String {
    if !si.os_id.is_empty() {
        return logos::detect(&si.os_id, &si.os_id_like).to_string();
    }
    let raw = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let field = |k: &str| -> String {
        raw.lines()
            .find_map(|l| l.strip_prefix(k))
            .map(|v| v.trim().trim_matches('"').to_string())
            .unwrap_or_default()
    };
    logos::detect(&field("ID="), &field("ID_LIKE=")).to_string()
}

// ── animated logo mode (procedural engine from logos.rs) ──

fn parse_effect(s: &str) -> logos::AnimEffect {
    match s.trim().to_ascii_lowercase().as_str() {
        // `frames`/`wave` add no glyph distortion — they let the native
        // keyframes (atom/dna/arcreactor) and the colour wave speak for themselves.
        "none" | "off" | "frames" | "keyframes" | "wave" | "rainbow"
                                         => logos::AnimEffect::None,
        "scanline" | "scan" | "crt"      => logos::AnimEffect::Scanline,
        "boot" | "bootreveal" | "reveal" => logos::AnimEffect::BootReveal,
        _                                => logos::AnimEffect::Shimmer,
    }
}

/// Enter the terminal's alternate screen buffer (used by full-screen modes).
/// The user's shell stays untouched underneath and is restored on exit.
fn screen_enter<W: Write>(buf: &mut W) {
    write!(buf, "\x1b[?1049h\x1b[2J\x1b[H\x1b[?25l").ok();
    buf.flush().ok();
}

/// Leave the alternate screen buffer and restore the cursor.
fn screen_leave<W: Write>(buf: &mut W) {
    write!(buf, "{RESET}\x1b[?25h\x1b[?1049l").ok();
    buf.flush().ok();
}

/// Colour each character of `line` from a flowing palette wave.
fn wave_text(line: &str, row: usize, tick: usize, palette: &[String]) -> String {
    let last = palette.len().saturating_sub(1);
    let mut s = String::with_capacity(line.len() * 8);
    for (col, ch) in line.chars().enumerate() {
        let phase = logos::color_wave_phase(row, col, tick);
        let idx = ((phase * last as f32).round() as usize).min(last);
        s.push_str(&palette[idx]);
        s.push(ch);
    }
    s.push_str(RESET);
    s
}

/// Short, self-overwriting intro played before the normal static render.
/// Animated logos (atom/dna/arcreactor) cycle frames; others can shimmer.
fn run_intro(
    info: &[String],
    logo_name: &str,
    cfg: &Config,
    args: &Args,
    anim: &config::Animation,
) {
    use std::fmt::Write as _;
    const FRAME_MS: u64 = 70;

    let frames = (anim.intro_ms / FRAME_MS).clamp(1, 24);
    let animated_logo = matches!(logo_name, "atom" | "dna" | "arcreactor");
    let effect = if animated_logo {
        logos::AnimEffect::None
    } else {
        parse_effect(&anim.effect)
    };

    RUNNING.store(true, Ordering::Relaxed);
    unsafe {
        for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(sig, sigint_handler as *const () as libc::sighandler_t);
        }
    }

    let out = stdout();
    let mut buf = out.lock();
    write!(buf, "\x1b[?25l").ok();

    let mut prev_nl = 0usize;
    for tick in 0..frames {
        if !RUNNING.load(Ordering::Relaxed) { break; }
        let base: Vec<String> = logos::from_name_frame(logo_name, (tick / 3) as usize)
            .iter().map(|s| (*s).to_string()).collect();
        let widths: Vec<usize> = base.iter().map(|l| visible_chars(l)).collect();
        let logo_w = widths.iter().max().copied().unwrap_or(36);
        let rows = base.len().max(info.len());

        let mut frame = String::with_capacity(8192);
        if prev_nl > 0 { let _ = write!(frame, "\x1b[{prev_nl}A\r"); }
        frame.push('\n');

        for i in 0..rows {
            let raw  = base.get(i).map(String::as_str).unwrap_or("");
            let line = logos::animate_line(raw, i, tick as usize, effect);
            let pad  = logo_w.saturating_sub(widths.get(i).copied().unwrap_or(0));
            let inf  = info.get(i).map(String::as_str).unwrap_or("");

            if args.no_color {
                let _ = write!(frame, "  {}{}  {}\n", line, " ".repeat(pad), inf);
            } else if animated_logo && anim.wave {
                let shown = wave_text(&line, i, tick as usize, &cfg.colors.logo);
                let _ = write!(frame, "  {}{}  {}\n", shown, " ".repeat(pad), inf);
            } else {
                let lc = logo_line_color(logo_name, i, cfg, args.color.is_some());
                let _ = write!(frame, "  {}{}{}{}  {}\n", lc, line, RESET, " ".repeat(pad), inf);
            }
        }

        prev_nl = frame.matches('\n').count();
        buf.write_all(frame.as_bytes()).ok();
        buf.flush().ok();
        thread::sleep(Duration::from_millis(FRAME_MS));
    }

    if prev_nl > 0 { write!(buf, "\x1b[{prev_nl}A\r").ok(); }
    write!(buf, "\x1b[?25h").ok();
    buf.flush().ok();
}

fn run_animate(
    info: &[String],
    logo_name: &str,
    custom: &Option<Vec<String>>,
    cfg: &Config,
    args: &Args,
    effect_name: &str,
    duration: Option<u64>,
) {
    use std::fmt::Write;

    const FRAME_MS: u64 = 70;
    const DEFAULT_FRAMES: u64 = 45; // ≈3s

    RUNNING.store(true, Ordering::Relaxed);
    unsafe {
        for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(sig, sigint_handler as *const () as libc::sighandler_t);
        }
    }

    let effect = parse_effect(effect_name);
    let max_frames: Option<u64> = match duration {
        None    => Some(DEFAULT_FRAMES),
        Some(0) => None, // Ctrl+C to stop
        Some(n) => Some((n * 1000 / FRAME_MS).max(1)),
    };

    // colour ramp for the wave; `logo` is a fixed 9-slot palette
    let ramp = &cfg.colors.logo;
    let last = ramp.len().saturating_sub(1);

    let out = stdout();
    let mut buf = out.lock();
    screen_enter(&mut buf);

    let mut tick: u64 = 0;
    loop {
        if !RUNNING.load(Ordering::Relaxed) { break; }
        if let Some(max) = max_frames { if tick >= max { break; } }
        let t = tick as usize;

        // current logo frame: animated logos cycle, everything else stays put
        let base: Vec<String> = match custom {
            Some(c) => c.clone(),
            None => logos::from_name_frame(logo_name, t / 6)
                .iter().map(|s| (*s).to_string()).collect(),
        };
        let widths: Vec<usize> = base.iter().map(|l| visible_chars(l)).collect();
        let logo_w = widths.iter().max().copied().unwrap_or(36);
        let max_rows = base.len().max(info.len());

        // build the whole frame, then emit it in one write (avoids tearing)
        let mut screen = String::with_capacity(8192);
        screen.push_str("\x1b[2J\x1b[H\n");
        for i in 0..max_rows {
            let raw  = base.get(i).map(String::as_str).unwrap_or("");
            let anim = logos::animate_line(raw, i, t, effect);
            let pad  = logo_w.saturating_sub(widths.get(i).copied().unwrap_or(0));
            let inf  = info.get(i).map(String::as_str).unwrap_or("");
            screen.push_str("  ");
            if args.no_color {
                screen.push_str(&anim);
            } else {
                // per-character colour wave across the logo
                for (col, ch) in anim.chars().enumerate() {
                    let phase = logos::color_wave_phase(i, col, t);
                    let idx   = ((phase * last as f32).round() as usize).min(last);
                    let _ = write!(screen, "{}{}", ramp[idx], ch);
                }
                screen.push_str(RESET);
            }
            let _ = writeln!(screen, "{:pad$}  {inf}", "", pad = pad);
        }
        buf.write_all(screen.as_bytes()).ok();
        buf.flush().ok();

        thread::sleep(Duration::from_millis(FRAME_MS));
        tick += 1;
    }

    screen_leave(&mut buf);
}

fn build_info(si: &info::SysInfo, cfg: &Config, sci_seed: usize) -> (Vec<String>, bool) {
    let c      = &cfg.colors;
    let sh     = &cfg.show;
    let is_sci = matches!(cfg.preset.as_deref(), Some("science"));
    let mut lines: Vec<String> = Vec::with_capacity(22);

    // ── header ────────────────────────────────────────────
    use config::Header;
    let (username, hostname) = if sh.header != Header::None {
        (
            env::var("USER").or_else(|_| env::var("LOGNAME")).unwrap_or_else(|_| "user".into()),
            hostname(),
        )
    } else {
        (String::new(), String::new())
    };

    match &sh.header {
        Header::Both => {
            let pw = username.len() + 1 + hostname.len();
            lines.push(format!("{B}{uc}{u}{OVL}@{R}{B}{hc}{h}{R}",
                B=BOLD, uc=&c.username, u=&username,
                OVL=OVERLAY0, R=RESET, hc=&c.hostname, h=&hostname));
            lines.push(format!("{B}{ac}{l}{R}", B=BOLD, ac=&c.accent,
                l="\u{2500}".repeat(pw), R=RESET));
        }
        Header::UserOnly => {
            lines.push(format!("{B}{uc}{u}{R}", B=BOLD, uc=&c.username, u=&username, R=RESET));
            lines.push(format!("{B}{ac}{l}{R}", B=BOLD, ac=&c.accent,
                l="\u{2500}".repeat(username.len()), R=RESET));
        }
        Header::HostOnly => {
            lines.push(format!("{B}{hc}{h}{R}", B=BOLD, hc=&c.hostname, h=&hostname, R=RESET));
            lines.push(format!("{B}{ac}{l}{R}", B=BOLD, ac=&c.accent,
                l="\u{2500}".repeat(hostname.len()), R=RESET));
        }
        Header::None => {}
    }

    macro_rules! field {
        ($li:expr, $lines:expr, $c:expr, $show:expr, $key:expr, $val:expr) => {
            if $show {
                $lines.push(row($c.label($li), $key, $val));
                $li += 1;
            }
        };
    }

    let mut li = 0usize;

    field!(li, lines, c, sh.os,     "os",     &si.os);
    field!(li, lines, c, sh.kernel, "kernel", &si.kernel);

    // uptime — short or long format based on config
    if sh.uptime {
        let upt_str = if sh.uptime_long {
            let s = si.uptime_secs;
            let (d, h, m) = (s/86400, (s%86400)/3600, (s%3600)/60);
            match (d, h, m) {
                (0, 0, m) => format!("{} min{}", m, if m==1 {""} else {"s"}),
                (0, h, m) => format!("{} hr{}, {} min{}", h, if h==1 {""} else {"s"}, m, if m==1 {""} else {"s"}),
                (d, h, m) => format!("{} day{}, {} hr{}, {} min{}", d, if d==1 {""} else {"s"}, h, if h==1 {""} else {"s"}, m, if m==1 {""} else {"s"}),
            }
        } else {
            si.uptime.clone()
        };
        lines.push(row(c.label(li), "uptime", &upt_str));
        li += 1;
    }

    field!(li, lines, c, sh.res,    "res",    &si.res);
    field!(li, lines, c, sh.pkgs,   "pkgs",   &si.pkgs);
    field!(li, lines, c, sh.shell,  "shell",  &si.shell);
    field!(li, lines, c, sh.de_wm,  "de/wm",  &si.de_wm);
    field!(li, lines, c, sh.term,   "term",   &si.term);
    field!(li, lines, c, sh.cpu,    "cpu",    &si.cpu);
    field!(li, lines, c, sh.gpu,    "gpu",    &si.gpu);
    field!(li, lines, c, sh.gpu_temp, "gpu °C", &si.gpu_temp);
    field!(li, lines, c, sh.battery,  "battery", &si.battery);

    // memory bar
    if sh.memory {
        let bar     = mem_bar(si.mem_used, si.mem_total, 14, &c.bar);
        let mem_str = if si.mem_total >= 1024 {
            format!("{bar}{TEXT}  {u:.1}G / {t:.1}G{R}",
                bar=bar, TEXT=TEXT, u=si.mem_used as f64/1024.0,
                t=si.mem_total as f64/1024.0, R=RESET)
        } else {
            format!("{bar}{TEXT}  {u}M / {t}M{R}",
                bar=bar, TEXT=TEXT, u=si.mem_used, t=si.mem_total, R=RESET)
        };
        let mut s = String::with_capacity(140);
        s.push_str(BOLD); s.push_str(c.label(li));
        s.push_str("memory"); s.push_str("   ");
        s.push_str(RESET); s.push_str(&mem_str);
        lines.push(s);
        li += 1;
    }

    field!(li, lines, c, sh.disk,   "disk",   &si.disk);
    field!(li, lines, c, sh.load,   "load",   &si.load);
    field!(li, lines, c, sh.locale, "locale", &si.locale);

    // hacker fields
    field!(li, lines, c, sh.ip,    "ip",    &si.ip);
    field!(li, lines, c, sh.ssh,   "ssh",   &si.ssh);
    field!(li, lines, c, sh.ports, "ports", &si.ports);

    // new fields
    field!(li, lines, c, sh.init,      "init",      &si.init);
    field!(li, lines, c, sh.cpu_temp,  "cpu °C",    &si.cpu_temp);
    field!(li, lines, c, sh.processes, "processes", &si.processes);
    field!(li, lines, c, sh.container, "container", &si.container);
    field!(li, lines, c, sh.session,   "session",   &si.session);

    // fastfetch-inspired fields
    field!(li, lines, c, sh.swap,        "swap",        &si.swap);
    field!(li, lines, c, sh.sound,       "sound",       &si.sound);
    field!(li, lines, c, sh.gpu_driver,  "gpu driver",  &si.gpu_driver);
    field!(li, lines, c, sh.bios,        "bios",        &si.bios);
    field!(li, lines, c, sh.board,       "board",       &si.board);
    field!(li, lines, c, sh.disk_type,   "disk type",   &si.disk_type);
    field!(li, lines, c, sh.pkg_updates, "updates",     &si.pkg_updates);
    field!(li, lines, c, sh.theme,       "theme",       &si.theme);
    field!(li, lines, c, sh.icons,       "icons",       &si.icons);
    field!(li, lines, c, sh.term_font,   "term font",   &si.term_font);
    field!(li, lines, c, sh.de_wm_ver,   "de/wm ver",   &si.de_wm_ver);
    field!(li, lines, c, sh.init_ver,    "init ver",    &si.init_ver);
    field!(li, lines, c, sh.local_ip,    "local ip",    &si.local_ip);

    // opt-in modules
    field!(li, lines, c, sh.datetime,      "datetime",   &si.datetime);
    field!(li, lines, c, sh.editor,        "editor",     &si.editor);
    field!(li, lines, c, sh.dns,           "dns",        &si.dns);
    field!(li, lines, c, sh.host,          "host",       &si.host);
    field!(li, lines, c, sh.chassis,       "chassis",    &si.chassis);
    field!(li, lines, c, sh.bootmgr,       "bootmgr",    &si.bootmgr);
    field!(li, lines, c, sh.cpu_cache,     "cpu cache",  &si.cpu_cache);
    field!(li, lines, c, sh.login_manager, "login mgr",  &si.login_manager);
    field!(li, lines, c, sh.brightness,    "brightness", &si.brightness);
    field!(li, lines, c, sh.cpu_usage,     "cpu usage",  &si.cpu_usage);
    field!(li, lines, c, sh.diskio,        "disk io",    &si.diskio);
    field!(li, lines, c, sh.netio,         "net io",     &si.netio);
    field!(li, lines, c, sh.physmem,       "phys mem",   &si.physmem);
    field!(li, lines, c, sh.gpu_mem,       "gpu mem",    &si.gpu_mem);
    field!(li, lines, c, sh.btrfs,         "btrfs",      &si.btrfs);
    field!(li, lines, c, sh.wifi,          "wifi",       &si.wifi);
    field!(li, lines, c, sh.users,         "users",      &si.users);
    field!(li, lines, c, sh.wallpaper,     "wallpaper",  &si.wallpaper);
    field!(li, lines, c, sh.media,         "media",      &si.media);
    field!(li, lines, c, sh.public_ip,     "public ip",  &si.public_ip);
    field!(li, lines, c, sh.weather,       "weather",    &si.weather);
    field!(li, lines, c, sh.opengl,        "opengl",     &si.opengl);
    field!(li, lines, c, sh.vulkan,        "vulkan",     &si.vulkan);
    field!(li, lines, c, sh.cursor,        "cursor",     &si.cursor);
    field!(li, lines, c, sh.term_size,     "term size",  &si.term_size);
    field!(li, lines, c, sh.term_theme,    "term theme", &si.term_theme);
    field!(li, lines, c, sh.codec,         "codec",      &si.codec);
    field!(li, lines, c, sh.bluetooth,     "bluetooth",  &si.bluetooth);
    field!(li, lines, c, sh.camera,        "camera",     &si.camera);
    field!(li, lines, c, sh.mouse,         "mouse",      &si.mouse);
    field!(li, lines, c, sh.keyboard,      "keyboard",   &si.keyboard);
    field!(li, lines, c, sh.tpm,           "tpm",        &si.tpm);
    field!(li, lines, c, sh.physical_disk, "phy disk",   &si.physical_disk);
    field!(li, lines, c, sh.top_processes, "top procs",  &si.top_processes);
    field!(li, lines, c, sh.monitor,       "monitor",    &si.monitor);

    let _ = li;

    // swatches / color bar
    if sh.swatches {
        lines.push(String::new());
        lines.push(format!(
            "{rw}\u{25cf} {fl}\u{25cf} {pk}\u{25cf} {mv}\u{25cf} {rd}\u{25cf} {mr}\u{25cf} {pe}\u{25cf} {yw}\u{25cf} {gn}\u{25cf} {tl}\u{25cf} {sk}\u{25cf} {sp}\u{25cf} {bl}\u{25cf} {lv}\u{25cf}{R}",
            rw=ROSEWATER, fl=FLAMINGO, pk=PINK,    mv=MAUVE,
            rd=RED,       mr=MAROON,   pe=PEACH,   yw=YELLOW,
            gn=GREEN,     tl=TEAL,     sk=SKY,     sp=SAPPHIRE,
            bl=BLUE,      lv=LAVENDER, R=RESET
        ));
    }
    if sh.color_bar {
        lines.push(String::new());
        lines.push(format!(
            "{bl}\u{2588}{rd}\u{2588}{gn}\u{2588}{yw}\u{2588}{bl2}\u{2588}{mv}\u{2588}{cy}\u{2588}{wh}\u{2588}{R}",
            bl=OVERLAY0, rd=RED, gn=GREEN, yw=YELLOW,
            bl2=BLUE, mv=MAUVE, cy=TEAL, wh=TEXT, R=RESET
        ));
    }

    // science quote — always last if science preset
    if is_sci {
        lines.push(String::new());
        lines.push(format!("{OVL}{q}{R}", OVL=OVERLAY0, q=science_quote(sci_seed), R=RESET));
    }

    (lines, is_sci)
}

// ═══════════════════════════════════════════════════════════
//  Black Hole — M87 accretion disk
// ═══════════════════════════════════════════════════════════

static RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn sigint_handler(_: libc::c_int) {
    RUNNING.store(false, Ordering::Relaxed);
}

#[inline]
fn bh_char(i: f64) -> char {
    if i > 0.88 { '\u{2588}' }        // █
    else if i > 0.70 { '\u{2593}' }   // ▓
    else if i > 0.52 { '\u{2592}' }   // ▒
    else if i > 0.32 { '\u{2591}' }   // ░
    else if i > 0.15 { '\u{00b7}' }   // ·
    else { ' ' }
}

#[inline]
fn rgb_scale(c: (u8, u8, u8), k: f64) -> (u8, u8, u8) {
    let k = k.clamp(0.0, 1.0);
    ((c.0 as f64 * k) as u8, (c.1 as f64 * k) as u8, (c.2 as f64 * k) as u8)
}

#[inline]
fn lerp3(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f64 + (b.0 as f64 - a.0 as f64) * t) as u8,
        (a.1 as f64 + (b.1 as f64 - a.1 as f64) * t) as u8,
        (a.2 as f64 + (b.2 as f64 - a.2 as f64) * t) as u8,
    )
}

fn bh_cell(col: usize, row: usize, cx: f64, cy: f64, rot: f64, frame: u64)
    -> ((u8, u8, u8), char)
{
    let dx   = (col as f64 - cx) * 0.52;
    let dy   = row as f64 - cy;
    let dist = (dx*dx + dy*dy).sqrt();
    let ang  = dy.atan2(dx);

    // ── event horizon: absolute black ───────────────────────
    if dist < 1.9 { return ((0, 0, 0), ' '); }

    // ── differential rotation: inner orbits sweep faster ────
    let ra = (ang + rot * (1.0 + 3.0 / (dist + 1.5))).rem_euclid(2.0 * PI);

    // ── photon ring: razor-thin, bright, gravitationally lensed ──
    if dist < 2.7 {
        let edge = (1.0 - (dist - 2.30).abs() / 0.42).clamp(0.0, 1.0);
        let flick = 0.75 + 0.25 * (ra * 6.0).sin().abs();
        let inten = (0.35 + 0.65 * edge) * flick;
        let col = lerp3((120, 200, 255), (255, 255, 255), edge);
        return (rgb_scale(col, inten), bh_char(inten));
    }

    // ── accretion disk: temperature gradient + Doppler + turbulence ──
    if dist < 11.5 {
        let t = ((dist - 2.7) / 8.8).clamp(0.0, 1.0);
        let temp = if t < 0.22 {
            lerp3((255, 246, 224), (255, 214, 150), t / 0.22)
        } else if t < 0.52 {
            lerp3((255, 214, 150), (255, 132, 58), (t - 0.22) / 0.30)
        } else if t < 0.80 {
            lerp3((255, 132, 58), (198, 58, 30), (t - 0.52) / 0.28)
        } else {
            lerp3((198, 58, 30), (104, 24, 18), (t - 0.80) / 0.20)
        };
        // relativistic beaming: the approaching side is brighter
        let beam = 0.55 + 0.45 * ((ang + 0.35).cos() * 0.5 + 0.5);
        // turbulent filaments co-rotating with the disk
        let turb = 0.60 + 0.40
            * ((ra * 7.0 + dist * 2.6 + frame as f64 * 0.85).sin() * 0.5 + 0.5);
        let inten = (beam * turb * (1.18 - t * 0.55)).clamp(0.0, 1.0);
        return (rgb_scale(temp, inten), bh_char(inten));
    }

    // ── outer halo / lensed dust ────────────────────────────
    if dist < 14.8 {
        let w = (ra * 3.0 + dist * 0.7 + rot * 0.4).sin() * 0.5 + 0.5;
        let inten = (w * 0.5 * (1.0 - (dist - 11.5) / 3.3)).clamp(0.0, 0.4);
        if inten > 0.10 {
            let c = rgb_scale((150, 120, 220), inten * 2.2);
            return (c, if inten > 0.22 { '\u{00b7}' } else { ' ' });
        }
    }
    ((0, 0, 0), ' ')
}

fn run_blackhole(info: &[String], duration: Option<u64>) {
    const FRAME_MS: u64  = 54;
    const BH_ROWS:  usize = 19;
    const BH_COLS:  usize = 46;
    const CX:       f64   = 23.0;
    const CY:       f64   = 9.0;


    RUNNING.store(true, Ordering::Relaxed);
    unsafe {
        for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(sig, sigint_handler as *const () as libc::sighandler_t);
        }
    }

    let max_frames: Option<u64> = match duration {
        None    => Some(56), Some(0) => None, Some(n) => Some(n * 1000 / FRAME_MS),
    };
    let lbl = match duration {
        None    => "~3s".into(), Some(0) => "\u{221e}  (Ctrl+C)".into(),
        Some(n) => format!("{}s", n),
    };
    let out     = stdout();
    let mut buf = out.lock();
    screen_enter(&mut buf);

    // ── background star positions — 40 fixed stars ──────────
    let mut stars = [[0u8; BH_COLS]; BH_ROWS];
    let mut star_phase = [0u64; 40];
    for (i, phase) in star_phase.iter_mut().enumerate() {
        let h = i.wrapping_mul(0x9e37_79b9);
        let col = 2 + (h & 0x2f);
        let row = 1 + ((h >> 8) & 0x1f);
        *phase = ((h >> 16) & 0x3f) as u64;
        if col < BH_COLS && row < BH_ROWS {
            stars[row][col] = (i + 1) as u8;
        }
    }

    let mut frame = 0u64;
    loop {
        if !RUNNING.load(Ordering::Relaxed) { break; }
        if let Some(max) = max_frames { if frame >= max { break; } }

        let rot = (frame as f64 * PI / 6.0) % (2.0*PI);

        // ── shooting star — rare, fast ──────────────────────
        let (ss_col, ss_row, ss_alive) = if frame % 17 == 3 && frame > 4 {
            let h = frame.wrapping_mul(0x6c8e_9cf5);
            let c = (h & 0x1f) as usize + 2;
            let r = ((h >> 8) & 0x0f) as usize + 2;
            (c, r, true)
        } else { (0, 0, false) };

        write!(buf, "\x1b[H").ok();

        // ── header with pulsing dot ─────────────────────────
        let pulse = (frame as f64 * 0.12).sin() * 0.3 + 0.7;
        write!(buf, "\x1b[K").ok();
        writeln!(buf).ok();
        write!(buf, "\x1b[K").ok();
        write!(buf, "  {}", BOLD).ok();
        if pulse > 0.92 { write!(buf, "{}", YELLOW).ok(); }
        else if pulse > 0.75 { write!(buf, "{}", PEACH).ok(); }
        else { write!(buf, "{}", MAUVE).ok(); }
        writeln!(buf, "\u{25cf}{}  arcfetch{R}  {OVL}\u{00b7}\u{00b7}\u{00b7}  {MV}SINGULARITY MODE{R}  {OVL}[{lbl}]{R}",
            RESET, OVL=OVERLAY0, MV=MAUVE, R=RESET, lbl=lbl).ok();
        write!(buf, "\x1b[K").ok();
        writeln!(buf, "  {OVL}{sep}{R}",
            OVL=OVERLAY0, sep="\u{2500}".repeat(62), R=RESET).ok();

        for (r, star_row) in stars.iter().enumerate() {
            write!(buf, "  ").ok();
            let mut prev: Option<(u8, u8, u8)> = None;
            for (c, &cell) in star_row.iter().enumerate() {
                // ── shooting star streak ────────────────────
                if ss_alive && r == ss_row && c >= ss_col && c < ss_col + 6 {
                    let tail = (c - ss_col) as f64;
                    if tail < 1.0 {
                        let cc = (220, 245, 255);
                        if prev != Some(cc) {
                            write!(buf, "\x1b[38;2;{};{};{}m", cc.0, cc.1, cc.2).ok();
                            prev = Some(cc);
                        }
                        write!(buf, "\u{258c}").ok();
                        continue;
                    }
                    let bright = 1.0 - tail / 6.0;
                    if bright > 0.3 {
                        let ch = if bright > 0.7 { '\u{2590}' } else { '\u{00b7}' };
                        let cc = rgb_scale((137, 220, 235), bright);
                        if prev != Some(cc) {
                            write!(buf, "\x1b[38;2;{};{};{}m", cc.0, cc.1, cc.2).ok();
                            prev = Some(cc);
                        }
                        write!(buf, "{}", ch).ok();
                        continue;
                    }
                }
                // ── background star ─────────────────────────
                let si = cell;
                if si > 0 {
                    let idx = (si - 1) as usize;
                    let phase = (frame.wrapping_add(star_phase[idx])) & 0x1f;
                    if phase < 22 {
                        let palette: [(u8, u8, u8); 6] = [
                            (108, 112, 134), (180, 190, 254), (203, 166, 247),
                            (205, 214, 244), (137, 220, 235), (137, 180, 250),
                        ];
                        let ci = (star_phase[idx] as usize + frame as usize) % palette.len();
                        let base = if phase < 4 { palette[(ci + 1) % palette.len()] }
                                   else { palette[ci] };
                        // twinkle: dim toward the middle of the cycle
                        let tw = 0.55 + 0.45 * ((phase as f64 / 22.0) * PI).sin();
                        let cc = rgb_scale(base, tw);
                        if prev != Some(cc) {
                            write!(buf, "\x1b[38;2;{};{};{}m", cc.0, cc.1, cc.2).ok();
                            prev = Some(cc);
                        }
                        write!(buf, "\u{00b7}").ok();
                        continue;
                    }
                }
                // ── accretion disk ──────────────────────────
                let (mut color, mut ch) = bh_cell(c, r, CX, CY, rot, frame);
                // ── relativistic jet (poles) ────────────────
                let dx_j = c as f64 - CX;
                let dy_j = r as f64 - CY;
                let d_j  = (dx_j*dx_j + dy_j*dy_j).sqrt();
                let jet  = d_j > 2.5 && d_j < 14.0 && dx_j.abs() < 1.5 && dy_j.abs() > 2.5
                    && ((dy_j > 0.0 && ((dx_j * 0.4 + rot * 0.5).sin() > 0.0))
                        || (dy_j < 0.0 && ((dx_j * 0.4 - rot * 0.5).sin() > 0.0)));
                if jet && ch == ' ' {
                    let jet_base = if dy_j < 0.0 { (180, 190, 254) } else { (203, 166, 247) };
                    let jf = ((dx_j * 2.0 + rot * 1.5).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                    color = rgb_scale(jet_base, 0.35 + 0.65 * jf);
                    ch = if jf > 0.6 { '\u{2591}' } else { '\u{00b7}' };
                }

                if ch == ' ' && color == (0, 0, 0) {
                    if prev.is_some() { write!(buf, "{}", RESET).ok(); prev = None; }
                    write!(buf, " ").ok();
                } else {
                    if prev != Some(color) {
                        write!(buf, "\x1b[38;2;{};{};{}m", color.0, color.1, color.2).ok();
                        prev = Some(color);
                    }
                    write!(buf, "{}", ch).ok();
                }
            }
            write!(buf, "{}  ", RESET).ok();
            if let Some(line) = info.get(r) { write!(buf, "{}{}", RESET, line).ok(); }
            writeln!(buf, "\x1b[K").ok();
        }
        buf.flush().ok();
        frame += 1;
        thread::sleep(Duration::from_millis(FRAME_MS));
    }
    screen_leave(&mut buf);
}

// ── Quantum collapse — wave-function animation ─────────────
fn run_quantum(info_lines: &[String], logo_name: &str, custom_lines: &Option<Vec<String>>,
               cfg: &Config, args: &Args) {
    const FRAME_MS: u64 = 70;
    const FADES: usize = 8;
    const SUPER: &[char] = &['░', '▒', '▓', '█'];

    // pre-compute logo widths once (all paths)
    let (logo_widths, logo_max_w): (Vec<usize>, usize) = if let Some(lines) = custom_lines {
        let w: Vec<usize> = lines.iter().map(|l| visible_chars(l)).collect();
        let m = w.iter().max().copied().unwrap_or(0);
        (w, m)
    } else {
        let logo = logos::from_name(logo_name);
        let w: Vec<usize> = logo.iter().map(|l| visible_chars(l)).collect();
        let m = w.iter().max().copied().unwrap_or(36);
        (w, m)
    };

    let out     = stdout();
    let mut buf = out.lock();
    write!(buf, "\x1b[?25l").ok();

    let out_lines: Vec<String> = info_lines.iter().map(|s| if args.no_color { strip_ansi(s) } else { s.clone() }).collect();

    for frame in 0..FADES {
        write!(buf, "\x1b[H").ok();

        // wave collapse — ripple envelope per line
        let faded: Vec<String> = out_lines.iter().enumerate().map(|(li, line)| {
            if line.is_empty() { return line.clone(); }
            let bytes = line.as_bytes();
            let val_start = bytes.iter().position(|&b| b == b' ').unwrap_or(0);
            if val_start >= line.len() { return line.clone(); }

            let visible = visible_chars(&line[val_start..]);
            let wave_base = frame as f64 / FADES as f64;
            let offset = (li as f64 * 0.4).sin() * 0.15;
            let wave = (wave_base + offset).clamp(0.0, 1.0);
            let reveal = (wave * visible as f64) as usize;

            let mut s = line[..val_start].to_string();
            let mut pos = 0usize;
            let mut esc = false;
            for ch in line[val_start..].chars() {
                if ch == '\x1b' { esc = true; s.push(ch); continue; }
                if esc { s.push(ch); if ch == 'm' { esc = false; } continue; }
                if pos < reveal {
                    s.push(ch);
                } else if pos == reveal {
                    let o = ((frame as f64 * 4.0 + li as f64 * 1.7 + pos as f64 * 0.7).sin() * 2.0 + 3.0) as usize;
                    s.push(SUPER[o.min(3)]);
                } else {
                    s.push('█');
                }
                pos += 1;
            }
            s
        }).collect();

        writeln!(buf).ok();
        if let Some(lines) = custom_lines {
            let max_rows = lines.len().max(faded.len());
            for i in 0..max_rows {
                let ll  = lines.get(i).map(String::as_str).unwrap_or("");
                let pad = logo_max_w.saturating_sub(logo_widths.get(i).copied().unwrap_or(0));
                let inf = faded.get(i).map(String::as_str).unwrap_or("");
                if args.no_color {
                    let stripped = if ll.contains('\x1b') { strip_ansi(ll) } else { ll.to_string() };
                    writeln!(buf, "  {}{}  {}", stripped, " ".repeat(pad), inf).ok();
                } else {
                    writeln!(buf, "  {acc}{ll}{pad}{R}  {inf}",
                        acc = &cfg.colors.accent, ll = ll,
                        pad = " ".repeat(pad), R = RESET, inf = inf).ok();
                }
            }
        } else {
            let logo   = logos::from_name(logo_name);
            let max_rows = logo.len().max(faded.len());
            for i in 0..max_rows {
                let ll  = logo.get(i).copied().unwrap_or("");
                let pad = logo_max_w.saturating_sub(logo_widths.get(i).copied().unwrap_or(0));
                let inf = faded.get(i).map(String::as_str).unwrap_or("");
                let lc = logo_line_color(logo_name, i, cfg, args.color.is_some());
                if args.no_color {
                    writeln!(buf, "  {}{}  {}", ll, " ".repeat(pad), inf).ok();
                } else {
                    writeln!(buf, "  {lc}{ll}{pad}{R}  {inf}",
                        lc = lc, ll = ll,
                        pad = " ".repeat(pad), R = RESET, inf = inf).ok();
                }
            }
        }
        buf.flush().ok();
        thread::sleep(Duration::from_millis(FRAME_MS));
    }

    // flash — measurement collapse
    write!(buf, "\x1b[H").ok();
    writeln!(buf).ok();
    if let Some(lines) = custom_lines {
        let max_rows = lines.len().max(out_lines.len());
        for i in 0..max_rows {
            let ll  = lines.get(i).map(String::as_str).unwrap_or("");
            let pad = logo_max_w.saturating_sub(logo_widths.get(i).copied().unwrap_or(0));
            let inf = out_lines.get(i).map(String::as_str).unwrap_or("");
            writeln!(buf, "  {}{}  {}", ll, " ".repeat(pad), inf).ok();
        }
    } else {
        let logo   = logos::from_name(logo_name);
        let max_rows = logo.len().max(out_lines.len());
        for i in 0..max_rows {
            let ll  = logo.get(i).copied().unwrap_or("");
            let pad = logo_max_w.saturating_sub(logo_widths.get(i).copied().unwrap_or(0));
            let inf = out_lines.get(i).map(String::as_str).unwrap_or("");
            let lc = logo_line_color(logo_name, i, cfg, args.color.is_some());
            writeln!(buf, "  {lc}{ll}{pad}{R}  {inf}\n",
                lc = lc, ll = ll,
                pad = " ".repeat(pad), R = RESET, inf = inf).ok();
        }
    }
    write!(buf, "\x1b[?25h").ok();
    writeln!(buf).ok();
    buf.flush().ok();
}

// ═══════════════════════════════════════════════════════════
//  CLI
// ═══════════════════════════════════════════════════════════

struct Args {
    logo:         String,
    logo_explicit: bool,
    logo_file:    Option<String>,
    accent:       Option<String>,
    color:        Option<String>,
    preset:       Option<String>,
    no_color:     bool,
    blackhole:    bool,
    mandelbrot:   bool,
    mandel_iter:  u32,
    quantum:      bool,
    cosmic:       bool,
    duration:     Option<u64>,
    help:         bool,
    version:      bool,
    show_cfg:     bool,
    all:          bool,
    json:         bool,
    structure:    Option<String>,
    watch:        bool,
    scheme:       Option<String>,
    animate:      Option<String>,
    list_logos:   bool,
    list_schemes: bool,
    no_intro:     bool,
    foreground:   bool,
    stop:         bool,
}

fn parse_args() -> Args {
    use lexopt::prelude::*;

    let mut a = Args {
        logo: "arch".into(),
        logo_explicit: false,
        logo_file: None,
        accent: None,
        color: None,
        preset: None,
        no_color: false,
        blackhole: false,
        mandelbrot: false,
        mandel_iter: 64,
        quantum: false,
        cosmic: false,
        duration: None,
        help: false,
        version: false,
        show_cfg: false,
        all: false,
        json: false,
        structure: None,
        watch: false,
        scheme: None,
        animate: None,
        list_logos: false,
        list_schemes: false,
        no_intro: false,
        foreground: false,
        stop: false,
    };

    // lexopt cannot peek, so pre-scan for the space-separated `--mandelbrot <n>` form.
    let mut mandel_space: Option<u32> = None;
    let mut animate_space: Option<String> = None;
    {
        let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
        for w in argv.windows(2) {
            if w[0] == "--mandelbrot" {
                if let Some(s) = w[1].to_str() {
                    if let Ok(n) = s.parse::<u32>() { mandel_space = Some(n); }
                }
            } else if w[0] == "--animate" {
                if let Some(s) = w[1].to_str() { animate_space = Some(s.to_string()); }
            }
        }
    }

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next().ok().flatten() {
        match arg {
            Short('h') | Long("help")    => a.help = true,
            Short('V') | Long("version") => a.version = true,
            Long("blackhole")            => a.blackhole = true,
            Long("no-color")             => a.no_color = true,
            Long("config")               => a.show_cfg = true,
            Long("full")                 => a.preset = Some("full".into()),
            Long("quantum")              => a.quantum = true,
            Long("cosmic")               => a.cosmic = true,
            Long("mandelbrot") => {
                a.mandelbrot = true;
                // optional iteration count: `--mandelbrot=N` (joined) or `--mandelbrot N` (pre-scanned)
                if let Some(v) = parser.optional_value() {
                    if let Ok(n) = v.to_string_lossy().parse::<u32>() { a.mandel_iter = n; }
                } else if let Some(n) = mandel_space {
                    a.mandel_iter = n;
                }
            }
            Long("logo") => {
                if let Ok(v) = parser.value() {
                    let name = v.to_string_lossy().into_owned();
                    if name == "?" || name == "list" {
                        a.list_logos = true;
                    } else {
                        a.logo = name;
                        a.logo_explicit = true;
                    }
                }
            }
            Long("list-logos") | Long("logos") => a.list_logos = true,
            Long("list-schemes") | Long("schemes") => a.list_schemes = true,
            Long("no-animate") | Long("no-intro") => a.no_intro = true,
            Long("foreground") | Long("fg") => a.foreground = true,
            Long("stop") => a.stop = true,
            Long("logo-file") => {
                if let Ok(v) = parser.value() {
                    a.logo_file = Some(v.to_string_lossy().into_owned());
                }
            }
            Long("accent") => {
                if let Ok(v) = parser.value() {
                    a.accent = Some(v.to_string_lossy().into_owned());
                }
            }
            Long("color") => {
                if let Ok(v) = parser.value() {
                    a.color = Some(v.to_string_lossy().into_owned());
                }
            }
            Long("preset") => {
                if let Ok(v) = parser.value() {
                    a.preset = Some(v.to_string_lossy().into_owned());
                }
            }
            Short('t') | Long("t") => {
                if let Ok(v) = parser.value() {
                    a.duration = v.to_string_lossy().parse().ok();
                }
            }
            Long("all")     => a.all = true,
            Long("json")    => a.json = true,
            Long("watch")   => a.watch = true,
            Long("scheme")  => {
                if let Ok(v) = parser.value() {
                    a.scheme = Some(v.to_string_lossy().into_owned());
                }
            }
            Long("animate") => {
                a.animate = Some(parser.optional_value()
                    .map(|v| v.to_string_lossy().into_owned())
                    .or_else(|| animate_space.clone())
                    .unwrap_or_else(|| "shimmer".into()));
            }
            Short('s') | Long("structure") => {
                if let Ok(v) = parser.value() {
                    a.structure = Some(v.to_string_lossy().into_owned());
                }
            }
            _ => {}
        }
    }
    a
}

fn print_version() {
    println!();
    let ver = env!("CARGO_PKG_VERSION");
    println!("  {S}version  {B}{BLUE}{ver}{R}", S=SUBTEXT1, B=BOLD, BLUE=BLUE, ver=ver, R=RESET);
    println!();
    // E = mc²: energy of startup ≈ 0
    println!("  {Y}E{R} = {G}m{R}{S}c{R}\u{00b2}   {OVL}where E = startup time \u{2248} 0{R}",
        Y=YELLOW, R=RESET, G=GREEN, S=SUBTEXT1, OVL=OVERLAY0);
    println!();
}

/// Escape a string for embedding in a JSON string literal.
fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"'  => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// `--json`: emit enabled fields as a flat JSON object, one line.
fn print_json(si: &info::SysInfo, cfg: &Config) {
    let sh = &cfg.show;
    let mut out = String::with_capacity(4096);
    out.push('{');
    let mut first = true;
    macro_rules! j {
        ($k:expr, $v:expr) => {{
            if !first { out.push(','); }
            first = false;
            out.push('"'); out.push_str(&json_escape($k)); out.push_str("\":\"");
            out.push_str(&json_escape($v)); out.push('"');
        }};
    }
    macro_rules! jf {
        ($flag:expr, $k:expr, $v:expr) => { if $flag { j!($k, $v); } };
    }

    jf!(sh.os, "os", &si.os);
    jf!(sh.kernel, "kernel", &si.kernel);
    jf!(true, "uptime", &si.uptime);
    jf!(sh.res, "res", &si.res);
    jf!(sh.pkgs, "pkgs", &si.pkgs);
    jf!(sh.shell, "shell", &si.shell);
    jf!(sh.de_wm, "de_wm", &si.de_wm);
    jf!(sh.term, "term", &si.term);
    jf!(sh.cpu, "cpu", &si.cpu);
    jf!(sh.gpu, "gpu", &si.gpu);
    jf!(sh.gpu_temp, "gpu_temp", &si.gpu_temp);
    jf!(sh.battery, "battery", &si.battery);
    jf!(sh.disk, "disk", &si.disk);
    jf!(sh.load, "load", &si.load);
    jf!(sh.locale, "locale", &si.locale);
    jf!(sh.memory, "mem_used", &si.mem_used.to_string());
    jf!(sh.memory, "mem_total", &si.mem_total.to_string());
    jf!(sh.ip, "ip", &si.ip);
    jf!(sh.ssh, "ssh", &si.ssh);
    jf!(sh.ports, "ports", &si.ports);
    jf!(sh.init, "init", &si.init);
    jf!(sh.cpu_temp, "cpu_temp", &si.cpu_temp);
    jf!(sh.processes, "processes", &si.processes);
    jf!(sh.container, "container", &si.container);
    jf!(sh.session, "session", &si.session);
    jf!(sh.swap, "swap", &si.swap);
    jf!(sh.sound, "sound", &si.sound);
    jf!(sh.gpu_driver, "gpu_driver", &si.gpu_driver);
    jf!(sh.bios, "bios", &si.bios);
    jf!(sh.board, "board", &si.board);
    jf!(sh.disk_type, "disk_type", &si.disk_type);
    jf!(sh.pkg_updates, "pkg_updates", &si.pkg_updates);
    jf!(sh.theme, "theme", &si.theme);
    jf!(sh.icons, "icons", &si.icons);
    jf!(sh.term_font, "term_font", &si.term_font);
    jf!(sh.de_wm_ver, "de_wm_ver", &si.de_wm_ver);
    jf!(sh.init_ver, "init_ver", &si.init_ver);
    jf!(sh.local_ip, "local_ip", &si.local_ip);
    jf!(sh.datetime, "datetime", &si.datetime);
    jf!(sh.editor, "editor", &si.editor);
    jf!(sh.dns, "dns", &si.dns);
    jf!(sh.host, "host", &si.host);
    jf!(sh.chassis, "chassis", &si.chassis);
    jf!(sh.bootmgr, "bootmgr", &si.bootmgr);
    jf!(sh.cpu_cache, "cpu_cache", &si.cpu_cache);
    jf!(sh.login_manager, "login_manager", &si.login_manager);
    jf!(sh.brightness, "brightness", &si.brightness);
    jf!(sh.cpu_usage, "cpu_usage", &si.cpu_usage);
    jf!(sh.diskio, "diskio", &si.diskio);
    jf!(sh.netio, "netio", &si.netio);
    jf!(sh.physmem, "physmem", &si.physmem);
    jf!(sh.gpu_mem, "gpu_mem", &si.gpu_mem);
    jf!(sh.btrfs, "btrfs", &si.btrfs);
    jf!(sh.wifi, "wifi", &si.wifi);
    jf!(sh.users, "users", &si.users);
    jf!(sh.wallpaper, "wallpaper", &si.wallpaper);
    jf!(sh.media, "media", &si.media);
    jf!(sh.public_ip, "public_ip", &si.public_ip);
    jf!(sh.weather, "weather", &si.weather);
    jf!(sh.opengl, "opengl", &si.opengl);
    jf!(sh.vulkan, "vulkan", &si.vulkan);
    jf!(sh.cursor, "cursor", &si.cursor);
    jf!(sh.term_size, "term_size", &si.term_size);
    jf!(sh.term_theme, "term_theme", &si.term_theme);
    jf!(sh.codec, "codec", &si.codec);
    jf!(sh.bluetooth, "bluetooth", &si.bluetooth);
    jf!(sh.camera, "camera", &si.camera);
    jf!(sh.mouse, "mouse", &si.mouse);
    jf!(sh.keyboard, "keyboard", &si.keyboard);
    jf!(sh.tpm, "tpm", &si.tpm);
    jf!(sh.physical_disk, "physical_disk", &si.physical_disk);
    jf!(sh.top_processes, "top_processes", &si.top_processes);
    jf!(sh.monitor, "monitor", &si.monitor);

    out.push_str("}\n");
    let _ = first;
    unsafe { libc::write(1, out.as_ptr().cast::<core::ffi::c_void>(), out.len()); }
}

fn print_config_help(cfg_path: &str) {
    let (b, r, s, g) = (BOLD, RESET, SUBTEXT1, GREEN);
    println!();
    println!("  {b}{PEACH}config file{r}  {s}{cfg_path}{r}", b=b, PEACH=PEACH, r=r, s=s, cfg_path=cfg_path);
    println!();
    println!("  {b}{BLUE}[colors]{r}", b=b, BLUE=BLUE, r=r);
    println!("  {s}# 7 label slots (cycle through fields), accent, values, sep, bar{r}", s=s, r=r);
    println!("  {g}scheme{r}   = auto       {s}# auto|{} (auto = light/dark from the terminal){r}", config::SCHEMES.join("|"), g=g, r=r, s=s);
    println!("  {s}# values: catppuccin names | black/white/gray/orange | ansi_* | #RGB | #RRGGBB | rgb(r,g,b) | color0..255{r}", s=s, r=r);
    println!("  {g}accent{r}   = blue       {s}# logo + hostname color{r}", g=g, r=r, s=s);
    println!("  {g}username{r} = mauve      {s}# user part of user@host{r}", g=g, r=r, s=s);
    println!("  {g}hostname{r} = blue", g=g, r=r);
    println!("  {g}c1{r}       = blue       {s}# field label 1 (os, term, disk...){r}", g=g, r=r, s=s);
    println!("  {g}c2{r}       = sapphire", g=g, r=r);
    println!("  {g}c3{r}       = sky", g=g, r=r);
    println!("  {g}c4{r}       = teal", g=g, r=r);
    println!("  {g}c5{r}       = green", g=g, r=r);
    println!("  {g}c6{r}       = yellow", g=g, r=r);
    println!("  {g}c7{r}       = peach      {s}# repeats if more than 7 visible fields{r}", g=g, r=r, s=s);
    println!("  {g}values{r}   = subtext1   {s}# all field values{r}", g=g, r=r, s=s);
    println!("  {g}sep{r}      = overlay0   {s}# separator ────{r}", g=g, r=r, s=s);
    println!("  {g}bar{r}      = blue       {s}# memory bar fill{r}", g=g, r=r, s=s);
    println!("  {s}  logo1..logo9{r}  {s}custom logo inline colors (default: c1..c7 + accent + text){r}", s=s, r=r);
    println!();
    println!("  {b}{BLUE}[show]{r}  {s}(true/false each field){r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  os=true  kernel=true  uptime=true  res=false  pkgs=true");
    println!("  shell=true  de_wm=true  term=true  cpu=true  gpu=true");
    println!("  memory=true  disk=false  load=false  locale=false");
    println!("  swatches=false  color_bar=false");
    println!("  swap=false  sound=false  gpu_driver=false  theme=false");
    println!("  bios=false  board=false  disk_type=false  pkg_updates=false");
    println!("  local_ip=false  init_ver=false  de_wm_ver=false");
    println!("  {s}# opt-in modules (all false by default — or use --all):{r}", s=s, r=r);
    println!("  {s}datetime editor dns host chassis bootmgr cpu_cache login_manager{r}", s=s, r=r);
    println!("  {s}brightness cpu_usage diskio netio physmem gpu_mem btrfs wifi users{r}", s=s, r=r);
    println!("  {s}wallpaper media public_ip weather opengl vulkan{r}", s=s, r=r);
    println!("  {s}cursor term_size term_theme codec bluetooth camera mouse keyboard{r}", s=s, r=r);
    println!("  {s}tpm physical_disk top_processes monitor{r}", s=s, r=r);
    println!();
    println!("  {b}{BLUE}[logo]{r}  {s}choose logo or custom ASCII file{r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  {g}name{r} = arch         {s}# arch | ascii | tux | nix | gentoo | mini | auto{r}", g=g, r=r, s=s);
    println!("  {g}file{r} = <path>       {s}# custom ASCII file (overrides name){r}", g=g, r=r, s=s);
    println!("  {s}list every logo name from the CLI: arcfetch --list-logos{r}", s=s, r=r);
    println!();
    println!("  {b}{BLUE}[animation]{r}  {s}short intro on a real terminal{r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  {g}enabled{r}  = true      {s}# false = always static{r}", g=g, r=r, s=s);
    println!("  {g}effect{r}   = none      {s}# none | shimmer | scanline | boot{r}", g=g, r=r, s=s);
    println!("  {g}intro_ms{r} = 700       {s}# 0 disables; animated logos auto-play{r}", g=g, r=r, s=s);
    println!("  {g}wave{r}     = true      {s}# colour-wave atom/dna/arcreactor{r}", g=g, r=r, s=s);
    println!();
    println!("  {b}{BLUE}[template]{r}  {s}overrides [show] entirely{r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  {g}preset{r} = full         {s}# full | minimal | hacker | science{r}", g=g, r=r, s=s);
    println!();
    println!("  {b}{TEAL}color names{r}  {s}or #RRGGBB hex{r}", b=b, TEAL=TEAL, r=r, s=s);
    println!("  {RW}rosewater{r}  {FL}flamingo{r}  {PK}pink{r}  {MV}mauve{r}  {RD}red{r}  {MR}maroon{r}",
        RW=ROSEWATER, FL=FLAMINGO, PK=PINK, MV=MAUVE, RD=RED, MR=MAROON, r=r);
    println!("  {PE}peach{r}  {YW}yellow{r}  {GN}green{r}  {TL}teal{r}  {SK}sky{r}  {SP}sapphire{r}  {BL}blue{r}  {LV}lavender{r}",
        PE=PEACH, YW=YELLOW, GN=GREEN, TL=TEAL, SK=SKY,
        SP=SAPPHIRE, BL=BLUE, LV=LAVENDER, r=r);
    println!();
    println!("  {b}{SKY}quick start{r}", b=b, SKY=SKY, r=r);
    println!("    mkdir -p $(dirname {cfg_path})", cfg_path=cfg_path);
    println!("    cat > {cfg_path} << 'EOF'", cfg_path=cfg_path);
    println!("    [colors]");
    println!("    accent = mauve");
    println!("    c1 = mauve");
    println!("    c2 = lavender");
    println!("    [show]");
    println!("    res = false");
    println!("    [logo]");
    println!("    name = arch");
    println!("    [template]");
    println!("    preset = full");
    println!("    EOF");
    println!();
}

fn write_sample_config(cfg_path: &str) {
    use std::fs;
    use std::path::Path;

    let path = Path::new(cfg_path);

    // don't overwrite existing config
    if path.exists() {
        println!("  {OVL}config already exists — not overwritten{R}",
            OVL=OVERLAY0, R=RESET);
        println!("  {S}{p}{R}", S=SUBTEXT1, p=cfg_path, R=RESET);
        println!();
        return;
    }

    // create parent dirs
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            println!("  {RED}could not create dir: {e}{R}", RED=RED, e=e, R=RESET);
            return;
        }
    }

    let sample = r#"# arcfetch config — ~/.config/arcfetch/config.toml
# all values are optional — delete lines to use defaults

[colors]
# palette preset (uncomment to switch): auto | mocha | latte | frappe | macchiato |
#   nord | dracula | gruvbox | tokyo-night | solarized-dark | rose-pine |
#   onedark | kanagawa | everforest | ayu   (auto = light/dark from the terminal)
# scheme = auto

# every colour accepts: catppuccin names, black/white/gray/orange/purple/cyan,
# ansi_red / ansi_bright_blue / …, #RGB, #RRGGBB, rgb(r,g,b), color0..color255
accent   = blue
username = mauve
hostname = blue
c1 = blue
c2 = sapphire
c3 = sky
c4 = teal
c5 = green
c6 = yellow
c7 = peach
values = subtext1
sep    = overlay0
bar    = blue

# logo inline colors for custom logo files ($1..$9)
# defaults: c1..c7 + accent + text
logo1 = blue
logo2 = sapphire
logo3 = sky
logo4 = teal
logo5 = green
logo6 = yellow
logo7 = peach
logo8 = mauve
logo9 = text

[show]
# header: both | user | host | none
header      = both

os          = true
kernel      = true
uptime      = true
uptime_long = false   # true = "1 day, 2 hours, 30 mins"
res         = false
pkgs        = true
shell       = true
de_wm       = true
term        = true
cpu         = true
gpu         = true
gpu_temp    = false   # GPU temperature — reads /sys/class/drm hwmon
battery     = false   # battery % + status — N/A on desktop
memory      = true
disk        = false
load        = false
locale      = false

# hacker fields (hidden by default)
ip          = false
ssh         = false
ports       = false

swatches    = false   # color palette swatches row
color_bar   = false   # neofetch-style 8-color bar (■■■■■■■■)

# fastfetch-inspired fields (all hidden by default)
swap        = false   # swap usage
sound       = false   # audio server (PipeWire/PulseAudio/ALSA)
gpu_driver  = false   # GPU driver name + version
bios        = false   # BIOS version + date
board       = false   # motherboard vendor + model
disk_type   = false   # SSD / NVMe / HDD
pkg_updates = false   # available package updates
theme       = false   # GTK theme name
icons       = false   # icon theme name
term_font   = false   # terminal font name
de_wm_ver   = false   # desktop environment version
init_ver    = false   # init system version
local_ip    = false   # interface + local IP

# opt-in modules (all off by default; or just pass --all)
datetime      = false  # local date + time
editor        = false  # $VISUAL / $EDITOR
dns           = false  # nameservers from /etc/resolv.conf
host          = false  # DMI system vendor + product
chassis       = false  # DMI chassis type (Laptop/Desktop/…)
bootmgr       = false  # UEFI or BIOS
cpu_cache     = false  # L1/L2/L3 cache sizes
login_manager = false  # display-manager.service
brightness    = false  # backlight %
cpu_usage     = false  # /proc/stat sample (~200ms)
diskio        = false  # /proc/diskstats totals
netio         = false  # /proc/net/dev totals
physmem       = false  # DIMM info (needs root for DMI)
gpu_mem       = false  # VRAM used / total
btrfs         = false  # btrfs volumes
wifi          = false  # wireless iface + signal + SSID
users         = false  # logged-in users
wallpaper     = false  # current wallpaper (hyprland/sway/gnome)
media         = false  # now playing (playerctl)
public_ip     = false  # public IP (network)
weather       = false  # weather (network, wttr.in)
opengl        = false  # OpenGL renderer
vulkan        = false  # Vulkan drivers
cursor        = false  # cursor theme + size
term_size     = false  # terminal columns x rows
term_theme    = false  # terminal FG/BG ($COLORFGBG)
codec         = false  # VA-API encoders / decoders (vainfo)
bluetooth     = false  # Bluetooth radio + HCI version
camera        = false  # connected cameras
mouse         = false  # pointing devices
keyboard      = false  # keyboards
tpm           = false  # TPM version
physical_disk = false  # disks: model, size, type, temp
top_processes = false  # top 5 processes by CPU
monitor       = false  # EDID: name, resolution, Hz, size, PPI

[logo]
# logo name: arch | ascii | tux | nix | gentoo | mini | auto
# or any built-in distro logo: ubuntu | debian | fedora | opensuse | manjaro |
#   endeavouros | cachyos | kali | alpine | void | pop | linuxmint | garuda |
#   artix | rocky | almalinux | centos | steamos | raspbian | wsl | neon |
#   deepin | mx | blackarch | redhat | zorin  (append _small for compact)
# discover every built-in name: arcfetch --list-logos
# name = arch

# custom ASCII file path (overrides name if set)
# file = ~/.config/arcfetch/logo.txt

[animation]
# short intro played on a real terminal before the static render
enabled  = true
# glyph effect: none | shimmer | scanline | boot
effect   = none
# intro length in milliseconds (0 disables)
intro_ms = 700
# colour-wave animated logos (atom/dna/arcreactor)
wave     = true

[template]
# preset overrides [show] entirely
# options: full | minimal | hacker | science
# preset = full
"#;

    match fs::write(path, sample) {
        Ok(_) => {
            println!("  {GREEN}created{R}  {S}{p}{R}",
                GREEN=GREEN, R=RESET, S=SUBTEXT1, p=cfg_path);
            println!("  {OVL}edit it to customise — all fields are optional{R}",
                OVL=OVERLAY0, R=RESET);
        }
        Err(e) => {
            println!("  {RED}error writing config: {e}{R}", RED=RED, e=e, R=RESET);
        }
    }
    println!();
}

fn print_help(cfg_path: &str) {
    let (b, r, s, g) = (BOLD, RESET, SUBTEXT1, GREEN);
    println!();
    let ver = env!("CARGO_PKG_VERSION");
    println!("  {b}{MAUVE}arcfetch{r}  v{ver}  blazing-fast Arch sysinfo", b=b, MAUVE=MAUVE, ver=ver, r=r);
    println!();
    println!("  {b}{BLUE}usage{r}   arcfetch [OPTIONS]", b=b, BLUE=BLUE, r=r);
    println!();
    println!("  {b}{SAPPHIRE}flags{r}", b=b, SAPPHIRE=SAPPHIRE, r=r);
    println!("    {b}{g}-h, --help{r}                 this screen", b=b, g=g, r=r);
    println!("    {b}{g}-V, --version{r}              version + E=mc² joke", b=b, g=g, r=r);
    println!("    {b}{g}--config{r}                   show config ref + write sample file", b=b, g=g, r=r);
    println!("    {b}{g}--blackhole{r}                {MAUVE}M87 accretion disk, pinned above your shell{r}", b=b, g=g, r=r, MAUVE=MAUVE);
    println!("    {b}{g}-t, --t <secs>{r}            {s}0=infinite  N=N seconds{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--logo <name>{r}              switch logo {s}(arch|ubuntu|debian|fedora|nix|auto…){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--list-logos{r}              {s}show every logo name with a preview{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--list-schemes{r}            {s}show every colour scheme as a swatch strip{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--logo ?{r}                  {s}same as --list-logos{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--logo-file <path>{r}         {s}ASCII art or image (kitty protocol){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--preset <n>{r}               {s}full|minimal|hacker|science{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--full{r}                     {s}show all fields (overrides minimal default){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--accent <color>{r}           name | #RRGGBB | rgb(r,g,b) | colorN", b=b, g=g, r=r);
    println!("    {b}{g}--color <spec>{r}             logo colour: name | hex | random", b=b, g=g, r=r);
    println!("    {b}{g}--scheme <name>{r}            palette: {s}{}{r}", config::SCHEMES.join("|"), b=b, g=g, r=r, s=s);
    println!("    {b}{g}--no-color{r}                 plain text (pipe-friendly)", b=b, g=g, r=r);
    println!("    {b}{g}--all{r}                      {s}enable every optional field{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}-s, --structure <list>{r}     {s}show only listed fields (comma-sep){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--json{r}                     {s}machine-readable JSON output{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--watch{r}                    {s}refresh every second (Ctrl+C stops){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--mandelbrot [iter]{r}        {GREEN}Mandelbrot set as logo{r}", b=b, g=g, r=r, GREEN=GREEN);
    println!("    {b}{g}--quantum{r}                  {MAUVE}wave-function collapse animation{r}", b=b, g=g, r=r, MAUVE=MAUVE);
    println!("    {b}{g}--cosmic{r}                   {SKY}starfield + moon + shooting stars{r}", b=b, g=g, r=r, SKY=SKY);
    println!("    {b}{g}--animate [effect]{r}         {s}animate any logo, pinned above your shell (shimmer|scanline|boot|frames){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--no-animate{r}              {s}skip the short intro in normal mode{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--foreground{r}             {s}blocking full-screen animation (alternate screen){r}", b=b, g=g, r=r, s=s);
    println!();
    println!("  {b}{TEAL}logos{r}", b=b, TEAL=TEAL, r=r);

    println!("    {b}{g}arch{r}         block {s}\u{259f}\u{2588}\u{2588}\u{2588}\u{2599}{r} (default)", b=b, g=g, r=r, s=s);
    println!("    {b}{g}mini{r}         compact 7-line ASCII Arch", b=b, g=g, r=r);
    println!("    {b}{g}ascii{r}        dotty Arch ASCII", b=b, g=g, r=r);
    println!("    {b}{g}tux{r}          Linux penguin", b=b, g=g, r=r);
    println!("    {b}{g}nix{r}          NixOS hexagonal snowflake", b=b, g=g, r=r);
    println!("    {b}{g}gentoo{r}       Gentoo G (fastfetch style)", b=b, g=g, r=r);
    println!("    {b}{g}auto{r}         detect from {s}/etc/os-release{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}custom{r}  {s}~/.config/arcfetch/logo.txt{r}", b=b, g=g, r=r, s=s);
    println!("    {s}{n} built-in logos — try {g}--logo ubuntu{r}", s=s, n=logos::all_names().len(), g=g, r=r);
    println!();
    println!("  {b}{TEAL}presets{r}", b=b, TEAL=TEAL, r=r);
    println!("    {g}full{r}      everything", g=g, r=r);
    println!("    {g}minimal{r}   os kernel uptime memory battery", g=g, r=r);
    println!("    {g}hacker{r}    cpu gpu mem disk load ip ssh ports", g=g, r=r);
    println!("    {g}science{r}   os kernel cpu mem + random science logo + physicist quote", g=g, r=r);
    println!();
    println!("  {b}{PEACH}config{r}  {s}{cfg_path}{r}", b=b, PEACH=PEACH, r=r, s=s, cfg_path=cfg_path);
    println!("    run {b}{g}arcfetch --config{r} for full config reference", b=b, g=g, r=r);
    println!();
    println!("  {b}{SKY}display modes{r}", b=b, SKY=SKY, r=r);
    println!("    {g}--blackhole{r}   accretion disk, pinned above your shell", g=g, r=r);
    println!("    {g}--mandelbrot{r}  fractal logo (math)", g=g, r=r);
    println!("    {g}--quantum{r}     wave collapse (physics)", g=g, r=r);
    println!("    {g}--cosmic{r}      starfield + moon + shooting stars (astronomy)", g=g, r=r);
    println!("    {g}--animate{r}     procedural logo animation, pinned above your shell", g=g, r=r);
    println!("    {g}--stop{r}        remove the pinned animation from this terminal", g=g, r=r);
    println!("    {s}mutually exclusive — pick one{r}", s=s, r=r);
    println!();
    println!("  {b}{OVERLAY0}shell startup (e.g. ~/.zshrc){r}", b=b, OVERLAY0=OVERLAY0, r=r);
    println!("    arcfetch                       {s}# minimal default (fast){r}", s=s, r=r);
    println!("    arcfetch --full               {s}# show all fields{r}", s=s, r=r);
    println!("    arcfetch --mandelbrot          {s}# fractal as logo{r}", s=s, r=r);
    println!("    arcfetch --logo nix --cosmic   {s}# cosmic + nix logo{r}", s=s, r=r);
    println!();
}

fn logo_line_color<'a>(logo_name: &str, i: usize, cfg: &'a Config, color_override: bool) -> &'a str {
    if color_override {
        cfg.colors.accent.as_str()
    } else {
        match logo_name {
            "mini" => cfg.colors.label(i),
            "nix" => if i.is_multiple_of(2) { config::NIX_LIGHT } else { config::NIX_DARK },
            "gentoo" => if i.is_multiple_of(2) { config::GENTOO_DARK } else { config::GENTOO_LIGHT },
            _ => cfg.colors.accent.as_str(),
        }
    }
}

// ═══════════════════════════════════════════════════════════
//  logo catalogue
// ═══════════════════════════════════════════════════════════

/// `--list-schemes` — preview every built-in palette as a swatch strip.
fn print_schemes() {
    let depth = config::ColorDepth::detect();
    println!();
    println!("  {BOLD}{BLUE}arcfetch schemes{R}  {OVL}use: arcfetch --scheme <name>{R}",
        BOLD = BOLD, BLUE = BLUE, OVL = OVERLAY0, R = RESET);
    println!();
    for &name in config::SCHEMES {
        if name == "auto" { continue; }
        let Some(c) = config::scheme(name) else { continue };
        let swatches = [
            &c.accent, &c.labels[1], &c.labels[2], &c.labels[3],
            &c.labels[4], &c.labels[5], &c.labels[6], &c.values,
        ];
        let mut line = format!("    {GREEN}{:<12}{R} ", name, GREEN = GREEN, R = RESET);
        for col in swatches {
            line.push_str(col);
            line.push_str("███");
        }
        line.push_str(RESET);
        println!("{}", config::downgrade_str(&line, depth));
    }
    println!();
    println!("  {OVL}auto{R} picks latte/mocha from the terminal background.", OVL = OVERLAY0, R = RESET);
    println!();
}

fn print_logos() {
    let names = logos::all_names();

    let is_fun = |n: &str| matches!(
        n,
        "ascii" | "tux" | "nix" | "gentoo" | "mini" | "dna" | "atom" | "wave"
            | "emc2" | "pi" | "batman" | "batman_small" | "ironman" | "ironman_small"
            | "arcreactor"
    );
    let is_animated = |n: &str| matches!(n, "atom" | "dna" | "arcreactor");

    println!();
    println!("  {BOLD}{BLUE}arcfetch logos{R}  {OVL}{n} available · use: arcfetch --logo <name>{R}",
        BOLD = BOLD, BLUE = BLUE, OVL = OVERLAY0, R = RESET, n = names.len());
    println!();

    let fun: Vec<&str> = names.iter().copied().filter(|n| is_fun(n)).collect();
    let distros: Vec<&str> = names.iter().copied().filter(|n| !is_fun(n)).collect();

    for (title, list) in [("symbols, mascots & sci-fi", &fun), ("distributions", &distros)] {
        if list.is_empty() { continue; }
        println!("  {BOLD}{MAUVE}{title}{R}", BOLD = BOLD, MAUVE = MAUVE, R = RESET, title = title);

        for name in list {
            let art = logos::from_name(name);
            let preview = art
                .iter()
                .filter(|l| !l.trim().is_empty())
                .max_by_key(|l| visible_chars(l))
                .map(|l| l.trim())
                .unwrap_or("");
            let preview: String = preview.chars().take(30).collect();
            let marker = if is_animated(name) {
                format!(" {TEAL}[animated]{R}", TEAL = TEAL, R = RESET)
            } else {
                String::new()
            };
            println!("    {GREEN}{:<16}{R} {OVL}{}{R}{marker}",
                name, preview, GREEN = GREEN, R = RESET, OVL = OVERLAY0);
        }
        println!();
    }

    println!();
    println!("  {OVL}preview: arcfetch --logo <name>   ·   animate: arcfetch --animate --logo <name>{R}",
        OVL = OVERLAY0, R = RESET);
    println!("  {OVL}colour it: arcfetch --logo <name> --color '#ff8800'{R}", OVL = OVERLAY0, R = RESET);
    println!();
}

// ═══════════════════════════════════════════════════════════
//  background animation (top strip, shell stays usable)
// ═══════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
enum PanelMode<'a> {
    Blackhole,
    Animate(&'a str),
}

fn term_size() -> (usize, usize) {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) } == 0;
    if ok && ws.ws_row > 0 && ws.ws_col > 0 {
        (ws.ws_col as usize, ws.ws_row as usize)
    } else {
        (80, 24)
    }
}

/// Build one blackhole frame sized to the strip's animation area.
fn panel_blackhole(cols: usize, rows: usize, frame: u64) -> Vec<String> {
    let cx = cols as f64 / 2.0;
    let cy = rows as f64 / 2.0;
    let rot = (frame as f64 * std::f64::consts::PI / 6.0)
        % (2.0 * std::f64::consts::PI);
    let mut out = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut line = String::new();
        let mut prev: Option<(u8, u8, u8)> = None;
        for c in 0..cols {
            let (color, ch) = bh_cell(c, r, cx, cy, rot, frame);
            if ch == ' ' && color == (0, 0, 0) {
                if prev.is_some() { line.push_str(RESET); prev = None; }
                line.push(' ');
            } else {
                if prev != Some(color) {
                    line.push_str(&format!("\x1b[38;2;{};{};{}m", color.0, color.1, color.2));
                    prev = Some(color);
                }
                line.push(ch);
            }
        }
        if prev.is_some() { line.push_str(RESET); }
        out.push(line);
    }
    out
}

/// Build one logo frame with the colour wave.
fn panel_logo(
    logo_name: &str,
    tick: usize,
    effect: logos::AnimEffect,
    palette: &[String],
) -> Vec<String> {
    let base = logos::from_name_frame(logo_name, tick / 6);
    let widths: Vec<usize> = base.iter().map(|l| visible_chars(l)).collect();
    let w = widths.iter().max().copied().unwrap_or(36);
    let last = palette.len().saturating_sub(1);

    base.iter().enumerate().map(|(i, raw)| {
        let line = logos::animate_line(raw, i, tick, effect);
        let mut s = String::new();
        for (col, ch) in line.chars().enumerate() {
            let phase = logos::color_wave_phase(i, col, tick);
            let idx = ((phase * last as f32).round() as usize).min(last);
            s.push_str(&palette[idx]);
            s.push(ch);
        }
        s.push_str(RESET);
        s.push_str(&" ".repeat(w.saturating_sub(widths[i])));
        s
    }).collect()
}

/// The terminal device this process is talking to, e.g. `/dev/pts/3`.
fn tty_name() -> Option<String> {
    for fd in [1, 0, 2] {
        let raw = unsafe { libc::ttyname(fd) };
        if raw.is_null() { continue; }
        let name = unsafe { std::ffi::CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        if !name.is_empty() { return Some(name); }
    }
    None
}

/// Pid file keyed by terminal, so two terminals can pin at the same time.
fn bg_pid_path() -> Option<std::path::PathBuf> {
    let name = tty_name()?;
    let dir = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) if !dir.is_empty() => std::path::PathBuf::from(dir),
        _ => std::env::temp_dir(),
    };
    let slug: String = name
        .trim_start_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Some(dir.join(format!("arcfetch-{slug}.pid")))
}

fn pid_is_ours(pid: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim() == "arcfetch")
        .unwrap_or(false)
}

fn wait_for_exit(pid: i32) {
    for _ in 0..50 {
        if unsafe { libc::kill(pid, 0) } != 0 { return; }
        thread::sleep(Duration::from_millis(10));
    }
    if pid_is_ours(pid) {
        unsafe { libc::kill(pid, libc::SIGKILL); }
    }
}

/// Write escape sequences to the terminal even when stdout is redirected, so
/// `arcfetch --stop >log` restores the terminal instead of filling the log.
fn write_tty(seq: &[u8]) {
    if let Ok(mut tty) = std::fs::OpenOptions::new().write(true).open("/dev/tty") {
        let _ = tty.write_all(seq);
        let _ = tty.flush();
    } else {
        let mut out = stdout().lock();
        let _ = out.write_all(seq);
        let _ = out.flush();
    }
}

/// Undo the pin: ordinary full-screen scrolling, no origin mode. The cursor is
/// saved and restored because both escapes home the terminal as a side effect.
fn reset_pin() {
    write_tty(b"\x1b7\x1b[?6l\x1b[r\x1b8");
}

/// On an explicit `--stop`, take the frozen frame away as well as the region.
fn clear_pane(pane_h: usize) {
    if pane_h == 0 { return; }
    let mut seq = String::with_capacity(32 + pane_h * 12);
    seq.push_str("\x1b7\x1b[?6l");
    for y in 0..pane_h {
        seq.push_str(&format!("\x1b[{};1H\x1b[2K", y + 1));
    }
    seq.push_str("\x1b[r\x1b8");
    write_tty(seq.as_bytes());
}

fn stop_background() {
    let Some(path) = bg_pid_path() else { return };
    let mut found = false;

    // Clean up the pre-pin global pid file too, so an animation started by an
    // older build does not keep drawing over the terminal.
    if let Some(dir) = path.parent() {
        let legacy = dir.join("arcfetch-anim.pid");
        if legacy != path {
            if let Ok(text) = std::fs::read_to_string(&legacy) {
                found = true;
                if let Ok(pid) = text.trim().parse::<i32>() {
                    if pid_is_ours(pid) {
                        unsafe { libc::kill(pid, libc::SIGTERM); }
                        wait_for_exit(pid);
                    }
                }
            }
            let _ = std::fs::remove_file(&legacy);
        }
    }

    if let Ok(text) = std::fs::read_to_string(&path) {
        found = true;
        if let Ok(pid) = text.trim().parse::<i32>() {
            if pid_is_ours(pid) {
                unsafe { libc::kill(pid, libc::SIGTERM); }
                wait_for_exit(pid);
            }
        }
    }
    let _ = std::fs::remove_file(&path);
    if found {
        reset_pin();
    }
}

/// Build one frame for the pinned pane.
fn pane_frame(
    mode: PanelMode,
    logo_name: &str,
    custom: &Option<Vec<String>>,
    cfg: &Config,
    pane_cols: usize,
    pane_h: usize,
    frame: u64,
) -> Vec<String> {
    match mode {
        PanelMode::Blackhole => panel_blackhole(pane_cols, pane_h, frame),
        PanelMode::Animate(effect) => {
            let mut block = match custom {
                Some(lines) => lines.clone(),
                None => panel_logo(
                    logo_name,
                    frame as usize,
                    parse_effect(effect),
                    &cfg.colors.logo,
                ),
            };
            block.truncate(pane_h);
            block
        }
    }
}

/// Rows the pinned pane may claim on a terminal of `rows` rows.
fn pane_height(mode: PanelMode, first_lines: Option<&[String]>, rows: usize) -> usize {
    // Leave room for the gap row and at least four rows of shell.
    let usable = rows.saturating_sub(5).max(1);
    match mode {
        PanelMode::Blackhole => (rows * 55 / 100).min(usable).max(1),
        PanelMode::Animate(_) => first_lines
            .map(|l| l.len())
            .unwrap_or(1)
            .min(usable)
            .max(1),
    }
}

/// Put the animation in a pinned pane above the shell and return immediately.
///
/// The terminal's scroll region is set below the pane, so the shell's prompt
/// and all command output scroll under it and never push the art away. The
/// animation runs in a child in its own process group (but the shell's
/// session), so it can tell when a full-screen program owns the terminal and
/// pause instead of painting over it. The shell itself is untouched: aliases,
/// completion and starship keep working.
fn run_background(
    mode: PanelMode,
    logo_name: &str,
    custom: &Option<Vec<String>>,
    cfg: &Config,
    duration: Option<u64>,
    no_color: bool,
) {
    let (cols, rows) = term_size();
    if cols < 20 || rows < 8 {
        return;
    }

    // A frame at tick 0 gives the logo its natural height.
    let first_lines = match mode {
        PanelMode::Animate(effect) => Some(match custom {
            Some(lines) => lines.clone(),
            None => panel_logo(logo_name, 0, parse_effect(effect), &cfg.colors.logo),
        }),
        PanelMode::Blackhole => None,
    };

    let pane_h = pane_height(mode, first_lines.as_deref(), rows);
    let pane_cols = cols.min(120);
    let scroll_top = (pane_h + 2) as u16;
    let bottom = rows as u16;

    // Replace any pin already on this terminal. This also resets a region
    // left behind by a crash before we install the new one.
    stop_background();

    let shell_pid = unsafe { libc::getppid() };
    let mut shell_pgid = unsafe { libc::getpgid(shell_pid) };
    if shell_pgid < 0 {
        shell_pgid = shell_pid;
    }

    // Clear the screen, confine scrolling to the rows below the pane, and home
    // the cursor there. The shell's next prompt lands at scroll_top.
    {
        let mut out = stdout().lock();
        let _ = write!(
            out,
            "\x1b[?6l\x1b[r\x1b[2J\x1b[{};{}r\x1b[?6h\x1b[H",
            scroll_top, bottom
        );
        let _ = out.flush();
    }

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        reset_pin();
        return;
    }
    if pid > 0 {
        // Write it before returning: `arcfetch --stop` straight afterwards must
        // find the process that is actually drawing.
        if let Some(path) = bg_pid_path() {
            let _ = std::fs::write(path, pid.to_string());
        }
        return;
    }

    // ---- child ----
    // Its own process group, so Ctrl+C at the shell prompt does not kill it,
    // but the same session, so tcgetpgrp() can see who owns the terminal.
    unsafe {
        libc::setpgid(0, 0);
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
        libc::signal(libc::SIGTERM, sigint_handler as *const () as libc::sighandler_t);
    }
    RUNNING.store(true, Ordering::Relaxed);

    let start = std::time::Instant::now();
    let limit = match duration {
        None | Some(0) => None,
        Some(n) => Some(Duration::from_secs(n)),
    };

    let mut cur_cols = cols;
    let mut cur_rows = rows;
    let mut cur_pane_h = pane_h;
    let mut cur_pane_cols = pane_cols;
    let mut cur_scroll_top = scroll_top;
    let mut painted_h = 0usize;
    let mut held = true;
    let mut invalidate = true;
    let mut frame: u64 = 0;

    loop {
        if !RUNNING.load(Ordering::Relaxed) { break; }
        if let Some(lim) = limit {
            if start.elapsed() >= lim { break; }
        }

        // The shell that owns the pin went away: take the region with us.
        if unsafe { libc::kill(shell_pid, 0) } != 0 { break; }

        let foreground = unsafe { libc::tcgetpgrp(1) };
        if foreground < 0 { break; }
        let holds = foreground == shell_pgid;

        if holds {
            // A resize drops the scroll region. Put it back before painting.
            let (w, h) = term_size();
            if (w, h) != (cur_cols, cur_rows) {
                cur_cols = w;
                cur_rows = h;
                cur_pane_cols = w.min(120);
                // Keep the pane height, and with it the shell's prompt row,
                // stable across ordinary resizes. Only shrink when the
                // terminal can no longer hold the pane above the shell.
                if cur_scroll_top > h as u16 {
                    cur_pane_h = pane_height(mode, first_lines.as_deref(), h);
                    cur_scroll_top = (cur_pane_h + 2).min(h) as u16;
                }
                let mut out = stdout().lock();
                // Clear the gap row too: a shrink can slide the shell's first
                // prompt line into it, and that row is outside the region, so
                // it would otherwise stay pinned there forever.
                let gap = cur_scroll_top.saturating_sub(1).max(1);
                let _ = write!(
                    out,
                    "\x1b7\x1b[?6l\x1b[{};1H\x1b[2K\x1b[{};{}r\x1b[?6h\x1b8",
                    gap, cur_scroll_top, h as u16
                );
                let _ = out.flush();
                invalidate = true;
            }

            if !held {
                // A full-screen program may have reset the region on exit.
                // Clear the gap row too: the program may have left something
                // there, and it is outside the scrolling region.
                let mut out = stdout().lock();
                let gap = cur_scroll_top.saturating_sub(1).max(1);
                let _ = write!(
                    out,
                    "\x1b7\x1b[?6l\x1b[{};1H\x1b[2K\x1b[{};{}r\x1b[?6h\x1b8",
                    gap, cur_scroll_top, cur_rows as u16
                );
                let _ = out.flush();
                invalidate = true;
            }

            let block = pane_frame(
                mode, logo_name, custom, cfg, cur_pane_cols, cur_pane_h, frame,
            );
            let block: Vec<String> = if no_color {
                block.iter().map(|l| strip_ansi(l)).collect()
            } else {
                block
            };

            // Logos are centred in the pane; the blackhole fills it.
            let left = match mode {
                PanelMode::Animate(_) => {
                    let w = block.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
                    cur_cols.saturating_sub(w) / 2
                }
                PanelMode::Blackhole => cur_cols.saturating_sub(cur_pane_cols) / 2,
            };

            // Never clear below the top of the new scroll region: that is the
            // shell's space, not ours.
            let clear_to = painted_h
                .max(cur_pane_h)
                .min(cur_scroll_top.saturating_sub(1) as usize);
            let mut scr = String::with_capacity(4096);
            scr.push_str("\x1b[?2026h"); // synchronized update, ignored if unsupported
            scr.push_str("\x1b7");
            scr.push_str("\x1b[?6l"); // the pane lives at absolute rows 1..pane_h

            if invalidate {
                for y in 0..clear_to {
                    scr.push_str(&format!("\x1b[{};1H\x1b[2K", y + 1));
                }
                invalidate = false;
            }

            let visible = block.len().min(cur_pane_h);
            for (y, line) in block.iter().take(visible).enumerate() {
                scr.push_str(&format!("\x1b[{};1H\x1b[2K", y + 1));
                if left > 0 {
                    scr.push_str(&" ".repeat(left));
                }
                scr.push_str(line);
            }
            // If the frame got shorter, erase the rows it used to occupy.
            for y in visible..clear_to {
                scr.push_str(&format!("\x1b[{};1H\x1b[2K", y + 1));
            }

            scr.push_str(RESET);
            scr.push_str("\x1b[?6h");
            scr.push_str("\x1b8");
            scr.push_str("\x1b[?2026l");

            let mut out = stdout().lock();
            if out.write_all(scr.as_bytes()).is_err() { break; }
            if out.flush().is_err() { break; }
            drop(out);

            painted_h = cur_pane_h;
            frame += 1;
        }

        held = holds;
        thread::sleep(Duration::from_millis(70));
    }

    // Leave the shell a normal terminal again. A timer expiry leaves the last
    // frame as a static picture; an explicit --stop takes it away too.
    if !RUNNING.load(Ordering::Relaxed) && held {
        clear_pane(cur_pane_h);
    } else {
        reset_pin();
    }

    if let Some(path) = bg_pid_path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if text.trim() == std::process::id().to_string() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    unsafe { libc::_exit(0); }
}

// ═══════════════════════════════════════════════════════════
//  main
// ═══════════════════════════════════════════════════════════

/// Repopulate the slow network caches in the background. Invoked by the
/// detached child that [`cache::get_or_compute_swr`] spawns; produces no output.
fn refresh_cache() {
    crate::cache::set("public_ip", &net::public_ip().unwrap_or_else(|| "N/A".into()));
    crate::cache::set("weather", &net::weather("").unwrap_or_else(|| "N/A".into()));
}

fn main() {
    // behave like standard Unix tools when the reader goes away (e.g. `arcfetch | head`)
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }

    // Stale-while-revalidate worker: refresh the network caches, then exit.
    if env::var_os("ARCFETCH_REFRESH_CACHE").is_some() {
        refresh_cache();
        return;
    }

    let mut args = parse_args();
    let cfg_path = config::config_path();

    if args.version  { print_version();               return; }
    if args.help     { print_help(&cfg_path);          return; }
    if args.show_cfg { print_config_help(&cfg_path); write_sample_config(&cfg_path); return; }
    if args.list_logos { print_logos(); return; }
    if args.list_schemes { print_schemes(); return; }
    if args.stop { stop_background(); return; }

    // mutual exclusion for display modes
    let mode_count = [args.blackhole, args.cosmic, args.quantum, args.mandelbrot, args.animate.is_some()]
        .iter().filter(|&&m| m).count();
    if mode_count > 1 {
        eprintln!("arcfetch: only one display mode at a time (--blackhole, --cosmic, --quantum, --mandelbrot, --animate)");
        return;
    }

    let mut cfg = config::load(args.accent.as_deref(), args.preset.as_deref());

    // --all enables every optional field; -s/--structure shows only the listed ones
    if args.all { cfg.show.enable_all(); }
    if let Some(ref st) = args.structure { cfg.show.select(st); }

    // --scheme replaces the whole palette (then --accent / ARCFETCH_ACCENT re-wins)
    if let Some(ref s) = args.scheme {
        match config::scheme(s) {
            Some(c) => {
                cfg.colors = c;
                if let Some(a) = args.accent.clone().or_else(|| env::var("ARCFETCH_ACCENT").ok()) {
                    let ansi = config::resolve(&a);
                    cfg.colors.accent = ansi.clone();
                    cfg.colors.hostname = ansi;
                }
            }
            None => eprintln!("arcfetch: unknown scheme '{}' (try: {})", s, config::SCHEMES.join(", ")),
        }
    }

    // --color overrides logo/accent colour: name, #RRGGBB, rgb(), colorN, or random
    if let Some(ref color) = args.color {
        if color == "random" {
            use config::*;
            let palette = [BLUE, GREEN, TEAL, YELLOW, PEACH, MAUVE, RED, SKY, SAPPHIRE, LAVENDER];
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default()
                .as_nanos() as usize;
            cfg.colors.accent = palette[seed % palette.len()].into();
        } else {
            cfg.colors.accent = config::resolve(color);
        }
    }

    // auto-disable colour when stdout is not a terminal; honour NO_COLOR / CLICOLOR / FORCE_COLOR
    let is_tty = unsafe { libc::isatty(1) } != 0;
    let force_color = env::var_os("FORCE_COLOR").is_some_and(|v| !v.is_empty());
    if !is_tty && !force_color { args.no_color = true; }
    if env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) { args.no_color = true; }
    if env::var("CLICOLOR").ok().as_deref() == Some("0") { args.no_color = true; }

    // Down-sample colours to what the terminal can actually show (truecolor →
    // 256 → 16), so the tool stays legible on any terminal.
    let depth = if args.no_color {
        config::ColorDepth::True
    } else {
        config::ColorDepth::detect()
    };
    config::apply_depth(&mut cfg.colors, depth);

    // --watch: re-run this binary every second, clearing the screen each time
    if args.watch {
        let exe = std::env::current_exe().unwrap_or_default();
        let child_args: Vec<String> = std::env::args().skip(1)
            .filter(|a| a != "--watch").collect();
        if is_tty { print!("\x1b[?25l"); let _ = stdout().flush(); }
        while RUNNING.load(Ordering::Relaxed) {
            if is_tty { print!("\x1b[2J\x1b[H"); let _ = stdout().flush(); }
            let _ = std::process::Command::new(&exe).args(&child_args).status();
            std::thread::sleep(Duration::from_secs(1));
        }
        if is_tty { print!("\x1b[?25h"); let _ = stdout().flush(); }
        return;
    }

    // apply logo from config (CLI overrides config)
    if !args.logo_explicit {
        if let Some(ref name) = cfg.logo.name {
            args.logo = name.clone();
            // don't set logo_explicit — presets (minimal, science) can still override
        }
    }
    if args.logo_file.is_none() {
        if let Some(ref file) = cfg.logo.file {
            args.logo_file = Some(file.clone());
        }
    }

    // collect all sysinfo — only read network for hacker preset
    let need_net = cfg.preset.as_deref() == Some("hacker")
        || cfg.show.ip || cfg.show.ssh || cfg.show.ports;
    let si = info::collect_all(need_net, &cfg.show);

    // --json: emit machine-readable output and exit
    if args.json {
        print_json(&si, &cfg);
        return;
    }

    // science seed — one /proc/uptime read for both quote & logo
    let sci_seed = uptime_seed();

    // build info column
    let (info_lines, is_science) = build_info(&si, &cfg, sci_seed);

    // ── blackhole mode: consume info_lines into out_lines ──
    if args.blackhole {
        if !is_tty {
            eprintln!("arcfetch: --blackhole requires a terminal");
            return;
        }
        let out_lines: Vec<String> = if args.no_color {
            info_lines.iter().map(|s| strip_ansi(s)).collect()
        } else {
            info_lines
        };
        if args.foreground {
            stop_background();
            run_blackhole(&out_lines, args.duration);
        } else {
            let no_custom: Option<Vec<String>> = None;
            run_background(PanelMode::Blackhole, "arch", &no_custom, &cfg, args.duration, args.no_color);
        }
        return;
    }

    // ── resolve logo (shared between quantum, cosmic and normal render) ──
    // priority: --logo-file  >  --logo custom  >  science random  >  named logo

    // default custom logo path: ~/.config/arcfetch/logo.txt
    let default_custom = {
        let cfg_dir = cfg_path
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_else(|| format!("{}/.config/arcfetch", env::var("HOME").unwrap_or_default()));
        format!("{}/logo.txt", cfg_dir)
    };

    // load lines from a file — any number of lines, any width
    // supports $1..$9 (inline color slots), $R (reset), $$ (literal $)
    let load_file = |path: &str| -> Option<Vec<String>> {
        let expanded = expand_path(path);
        std::fs::read_to_string(&expanded).ok().map(|raw| {
            let colors = &cfg.colors;
            raw.lines().map(|line| {
                let bytes = line.as_bytes();
                let mut out = String::with_capacity(bytes.len() + 64);
                let mut i = 0;
                while i < bytes.len() {
                    if bytes[i] == b'$' && i + 1 < bytes.len() {
                        let next = bytes[i + 1];
                        if (b'1'..=b'9').contains(&next) {
                            out.push_str(&colors.logo[(next - b'1') as usize]);
                            i += 2; continue;
                        }
                        match next {
                            b'R' => { out.push_str(RESET); i += 2; continue; }
                            b'$' => { out.push('$'); i += 2; continue; }
                            _ => {}
                        }
                    }
                    out.push(bytes[i] as char);
                    i += 1;
                }
                out
            }).collect()
        })
    };

    // figure out which logo to use
    let image_escape: Option<String> = if args.no_color {
        None
    } else if let Some(ref path) = args.logo_file {
        let ext = path.rsplit('.').next().map(|e| e.to_lowercase());
        match ext.as_deref() {
            Some("png" | "jpg" | "jpeg" | "gif") => {
                let r = kitty_image(path);
                if r.is_none() {
                    eprintln!("arcfetch: could not load image: {}", path);
                }
                r
            }
            _ => None,
        }
    } else {
        None
    };

    // ── mandelbrot mode overrides the logo ─────────────
    let custom_lines: Option<Vec<String>> = if args.mandelbrot {
        Some(mandelbrot::render(args.mandel_iter))
    } else if image_escape.is_some() {
        None
    } else if let Some(ref path) = args.logo_file {
        // explicit --logo-file path (text file)
        load_file(path).or_else(|| {
            eprintln!("arcfetch: could not read logo file: {}", path);
            None
        })
    } else if args.logo == "custom" {
        // --logo custom → try default path
        load_file(&default_custom).or_else(|| {
            eprintln!("arcfetch: no logo file found at {}", default_custom);
            eprintln!("  create it or use --logo-file <path>");
            None
        })
    } else {
        None
    };

    let logo_name = {
        let base = if args.logo == "auto" { detect_auto_logo(&si) }
                   else if !args.logo_explicit && cfg.preset.as_deref() == Some("minimal") { "mini".into() }
                   else { args.logo.clone() };
        if custom_lines.is_none() && is_science && !args.logo_explicit {
            science_logo(sci_seed).to_string()
        } else {
            base
        }
    };

    // ── quantum mode: borrow info_lines directly ──
    if args.quantum {
        run_quantum(&info_lines, &logo_name, &custom_lines, &cfg, &args);
        return;
    }

    // ── consume info_lines for all remaining paths ──
    let out_lines: Vec<String> = if args.no_color {
        info_lines.into_iter().map(|s| strip_ansi(&s)).collect()
    } else {
        info_lines
    };

    // ── animated logo mode ──
    if let Some(ref eff) = args.animate {
        if !is_tty {
            eprintln!("arcfetch: --animate requires a terminal");
            return;
        }
        if args.foreground {
            stop_background();
            run_animate(&out_lines, &logo_name, &custom_lines, &cfg, &args, eff, args.duration);
        } else {
            run_background(PanelMode::Animate(eff), &logo_name, &custom_lines, &cfg, args.duration, args.no_color);
        }
        return;
    }

    // ── cosmic mode ──
    if args.cosmic {
        cosmic::run(&out_lines, &logo_name, &cfg, &args);
        return;
    }

    // ── short intro animation in normal mode (TTY only) ──
    if is_tty
        && !args.no_intro
        && cfg.animation.enabled
        && cfg.animation.intro_ms > 0
        && custom_lines.is_none()
        && image_escape.is_none()
    {
        let animated_logo = matches!(logo_name.as_str(), "atom" | "dna" | "arcreactor");
        let effect_on = !cfg.animation.effect.eq_ignore_ascii_case("none");
        if animated_logo || effect_on {
            run_intro(&out_lines, &logo_name, &cfg, &args, &cfg.animation);
        }
    }

    // ── single-buffer render (one syscall) ──
    use std::fmt::Write;
    let mut out = String::with_capacity(4096);
    out.push('\n');

    // emit kitty image escape first (the image IS the logo)
    if let Some(ref img) = image_escape {
        out.push_str(img);
        out.push('\n');
        // render info lines below the image, no logo needed
        for line in &out_lines {
            let _ = writeln!(out, "  {}", line);
        }
        out.push('\n');
        unsafe { libc::write(1, out.as_ptr().cast::<core::ffi::c_void>(), out.len()); }
        return;
    }

    if let Some(ref lines) = custom_lines {
        // pre-compute visible widths for pre-processed lines (once, not per-frame)
        let logo_widths: Vec<usize> = lines.iter().map(|l| visible_chars(l)).collect();
        let logo_w = logo_widths.iter().max().copied().unwrap_or(0);
        let max_rows = lines.len().max(out_lines.len());
        for i in 0..max_rows {
            let ll   = lines.get(i).map(String::as_str).unwrap_or("");
            let pad  = logo_w.saturating_sub(logo_widths.get(i).copied().unwrap_or(0));
            let inf  = out_lines.get(i).map(String::as_str).unwrap_or("");
            if args.no_color {
                let stripped = if ll.contains('\x1b') { strip_ansi(ll) } else { ll.to_string() };
                let _ = writeln!(out, "  {}{:pad$}  {}", stripped, "", inf, pad = pad);
            } else {
                let _ = writeln!(out, "  {}{}{:pad$}{}  {}",
                    cfg.colors.accent, ll, "", RESET, inf, pad = pad);
            }
        }
    } else {
        let logo   = logos::from_name(&logo_name);
        // pre-compute visible widths for all logo lines (one pass)
        let logo_widths: Vec<usize> = logo.iter().map(|l| visible_chars(l)).collect();
        let logo_w = logo_widths.iter().max().copied().unwrap_or(36);
        let max_rows = logo.len().max(out_lines.len());
        for i in 0..max_rows {
            let ll   = logo.get(i).copied().unwrap_or("");
            let pad  = logo_w.saturating_sub(logo_widths.get(i).copied().unwrap_or(0));
            let inf  = out_lines.get(i).map(String::as_str).unwrap_or("");
            if args.no_color {
                let _ = writeln!(out, "  {}{:pad$}  {}", ll, "", inf, pad = pad);
            } else {
                let lc = logo_line_color(&logo_name, i, &cfg, args.color.is_some());
                let _ = writeln!(out, "  {}{}{:pad$}{}  {}",
                    lc, ll, "", RESET, inf, pad = pad);
            }
        }
    }

    out.push('\n');
    let out = if depth == config::ColorDepth::True {
        out
    } else {
        config::downgrade_str(&out, depth)
    };
    unsafe { libc::write(1, out.as_ptr().cast::<core::ffi::c_void>(), out.len()); }
}
