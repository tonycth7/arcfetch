// arcfetch — blazing-fast Arch Linux sysinfo 
// arcfetch [-h] [-V] [--blackhole [--t N]] [--logo NAME] [--accent COLOR]
//          [--no-color] [--config]
mod anim;
mod cache;
mod config;
mod cosmic;
mod fastfetch;
mod info;
mod jsonc;
mod logos;
mod mandelbrot;
mod modules;
mod net;
mod pkgs;
mod scheme;

use std::{env, fs, thread};
use std::f64::consts::PI;
use std::io::{Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

/// Colour is off for `--no-color`, `NO_COLOR`, `CLICOLOR=0`, or a non-TTY
/// unless `FORCE_COLOR` is set.
fn colour_disabled(no_color: bool) -> bool {
    let tty = unsafe { libc::isatty(1) } != 0;
    let force = env::var_os("FORCE_COLOR").is_some_and(|v| !v.is_empty());
    no_color
        || (!tty && !force)
        || env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
        || env::var("CLICOLOR").ok().as_deref() == Some("0")
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
pub(crate) fn visible_chars(s: &str) -> usize {
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

fn is_box_style(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(),
        "round" | "square" | "double" | "heavy" | "dashed" | "ascii" | "none")
}

/// Wrap an already-rendered fetch in a border. Only called when `--box` (or
/// `[layout] box`) is set, so the default render path is untouched.
fn boxify(body: &str, style: &str, col: &str, indent: usize) -> String {
    let (tl, tr, bl, br, h, v) = match style.to_ascii_lowercase().as_str() {
        "square" => ('┌', '┐', '└', '┘', '─', '│'),
        "double" => ('╔', '╗', '╚', '╝', '═', '║'),
        "heavy"  => ('┏', '┓', '┗', '┛', '━', '┃'),
        "dashed" => ('╭', '╮', '╰', '╯', '╌', '╎'),
        "ascii"  => ('+', '+', '+', '+', '-', '|'),
        _        => ('╭', '╮', '╰', '╯', '─', '│'),
    };
    fn edge(out: &mut String, l: char, r: char, fill: char, width: usize, col: &str, indent: &str) {
        let paint = !col.is_empty();
        if paint { out.push_str(col); }
        out.push_str(indent); out.push(l);
        for _ in 0..width + 2 { out.push(fill); }
        out.push(r);
        if paint { out.push_str(RESET); }
        out.push('\n');
    }
    let pad = " ".repeat(indent);
    let lines: Vec<&str> = body.trim_matches('\n').split('\n')
        .map(|l| l.strip_prefix(&pad).unwrap_or(l)).collect();
    let width = lines.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
    let mut out = String::with_capacity(body.len() + (width + 6) * (lines.len() + 2) + 32);
    out.push('\n');
    edge(&mut out, tl, tr, h, width, col, &pad);
    let paint = !col.is_empty();
    for l in &lines {
        if paint { out.push_str(col); }
        out.push_str(&pad); out.push(v); out.push(' ');
        out.push_str(l);
        if paint { out.push_str(RESET); }          // stop content colour before padding
        for _ in visible_chars(l)..width { out.push(' '); }
        out.push(' ');
        if paint { out.push_str(col); }
        out.push(v);
        if paint { out.push_str(RESET); }
        out.push('\n');
    }
    edge(&mut out, bl, br, h, width, col, &pad);
    out
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

pub(crate) fn expand_path(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = env::var("HOME") {
            path.replacen('~', &home, 1)
        } else { path.to_string() }
    } else { path.to_string() }
}

/// Pixel dimensions from a PNG/GIF/JPEG header, without an image crate.
fn image_size(data: &[u8]) -> Option<(u32, u32)> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") && data.len() >= 24 {
        let w = u32::from_be_bytes(data[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(data[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        if data.len() < 10 { return None; }
        let w = u16::from_le_bytes(data[6..8].try_into().ok()?) as u32;
        let h = u16::from_le_bytes(data[8..10].try_into().ok()?) as u32;
        return Some((w, h));
    }
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 4 <= data.len() {
            if data[i] != 0xFF { i += 1; continue; }
            let marker = data[i + 1];
            i += 2;
            if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
                continue;
            }
            let len = u16::from_be_bytes(data[i..i + 2].try_into().ok()?) as usize;
            if len < 2 || i + len > data.len() { break; }
            if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF)
                && len >= 7
            {
                let h = u16::from_be_bytes(data[i + 3..i + 5].try_into().ok()?) as u32;
                let w = u16::from_be_bytes(data[i + 5..i + 7].try_into().ok()?) as u32;
                return Some((w, h));
            }
            i += len;
        }
    }
    None
}

/// Cell dimensions for a kitty image. Terminals use roughly 2:1 cells, so a
/// pixel aspect ratio maps to terminal rows without a pixel-size query.
fn image_cells(data: &[u8], width: Option<usize>, height: Option<usize>)
    -> (usize, usize)
{
    let (cols, rows) = match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => {
            let r = image_size(data)
                .map(|(pw, ph)| (w as f64 * ph as f64 / (pw.max(1) as f64 * 2.0)).round())
                .unwrap_or(19.0);
            (w, r.max(1.0) as usize)
        }
        (None, Some(h)) => {
            let c = image_size(data)
                .map(|(pw, ph)| (h as f64 * 2.0 * pw as f64 / ph.max(1) as f64).round())
                .unwrap_or(31.0);
            (c.max(1.0) as usize, h)
        }
        (None, None) => {
            let r = image_size(data)
                .map(|(pw, ph)| (31.0 * ph as f64 / (pw.max(1) as f64 * 2.0)).round())
                .unwrap_or(19.0);
            (31, r.max(1.0) as usize)
        }
    };
    (cols.clamp(1, 200), rows.clamp(1, 200))
}

/// `(escape, columns, rows)`. `hold` keeps the cursor on the image's top-left
/// cell so info can be printed beside it; otherwise the terminal moves past
/// the image (the old above/below layout).
fn kitty_image(path: &str, width: Option<usize>, height: Option<usize>, hold: bool)
    -> Option<(String, usize, usize)>
{
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
    let (cols, rows) = image_cells(&data, width, height);
    let hold = if hold { "C=1," } else { "" };
    Some((format!("\x1b_Ga=T,f={format},{hold}c={cols},r={rows};{b64}\x1b\\"), cols, rows))
}

/// System hostname via `gethostname(2)`, or `"localhost"` on failure.
pub(crate) fn hostname() -> String {
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

fn detect_auto_logo(si: &info::SysInfo) -> String {
    if env::var_os("TERMUX_VERSION").is_some() {
        return "termux".into();
    }
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
        // keyframes (atom/dna/arcreactor) and the generated aurora art speak
        // for themselves.
        "none" | "off" | "frames" | "keyframes" | "wave" | "rainbow"
                                         => logos::AnimEffect::None,
        "scanline" | "scan" | "crt"      => logos::AnimEffect::Scanline,
        "boot" | "bootreveal" | "reveal" => logos::AnimEffect::BootReveal,
        _                                => logos::AnimEffect::Shimmer,
    }
}

/// Enter the terminal's alternate screen buffer (used by full-screen modes).
/// The user's shell stays untouched underneath and is restored on exit.
pub(crate) fn screen_enter<W: Write>(buf: &mut W) {
    write!(buf, "\x1b[?1049h\x1b[2J\x1b[H\x1b[?25l").ok();
    buf.flush().ok();
}

/// Leave the alternate screen buffer and restore the cursor.
pub(crate) fn screen_leave<W: Write>(buf: &mut W) {
    write!(buf, "{RESET}\x1b[?25h\x1b[?1049l").ok();
    buf.flush().ok();
}

/// Colour one row from the gradient, or fall back to a flowing palette wave.
fn wave_text(
    line: &str,
    row: usize,
    height: usize,
    tick: usize,
    palette: &[String],
    anim: &config::Animation,
) -> String {
    if let Some(stop) = gradient_for_row(anim, row, height, tick) {
        let mut s = String::with_capacity(line.len() + stop.len() + RESET.len());
        s.push_str(stop);
        s.push_str(line);
        s.push_str(RESET);
        return s;
    }
    let last = palette.len().saturating_sub(1);
    let mut s = String::with_capacity(line.len() * 4);
    let mut prev = None;
    for (col, ch) in line.chars().enumerate() {
        if ch == ' ' {
            s.push(' ');
            continue;
        }
        let phase = logos::color_wave_phase(row, col, tick);
        let idx = ((phase * last as f32).round() as usize).min(last);
        if prev != Some(idx) {
            s.push_str(&palette[idx]);
            prev = Some(idx);
        }
        s.push(ch);
    }
    s.push_str(RESET);
    s
}

/// Short, self-overwriting intro played before the normal static render.
/// Animated logos cycle frames; others can shimmer.
fn run_intro(
    info: &[String],
    logo_name: &str,
    cfg: &Config,
    args: &Args,
    anim: &config::Animation,
) {
    use std::fmt::Write as _;
    let frame_ms = anim.frame_ms();

    let frames = (anim.intro_ms / frame_ms).clamp(1, 24);
    let animated_logo = logos::is_animated(logo_name);
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
    let indent = " ".repeat(cfg.layout.indent);
    let gap = " ".repeat(cfg.layout.gap);
    for tick in 0..frames {
        let frame_deadline = Instant::now() + Duration::from_millis(frame_ms);
        if !RUNNING.load(Ordering::Relaxed) { break; }
        let base: Vec<String> = logos::frame(logo_name, tick as usize, 4);
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
                let _ = writeln!(frame, "{indent}{}{}{gap}{}", line, " ".repeat(pad), inf);
            } else if animated_logo {
                let shown = wave_text(&line, i, base.len(), tick as usize, &cfg.colors.logo, anim);
                let _ = writeln!(frame, "{indent}{}{}{gap}{}", shown, " ".repeat(pad), inf);
            } else {
                let lc = logo_line_color(logo_name, i, cfg, args.color.is_some());
                let _ = writeln!(frame, "{indent}{}{}{}{}{gap}{}", lc, line, RESET, " ".repeat(pad), inf);
            }
        }

        prev_nl = frame.matches('\n').count();
        buf.write_all(frame.as_bytes()).ok();
        buf.flush().ok();
        if let Some(left) = frame_deadline.checked_duration_since(Instant::now()) {
            thread::sleep(left);
        }
    }

    if prev_nl > 0 { write!(buf, "\x1b[{prev_nl}A\r").ok(); }
    write!(buf, "\x1b[?25h").ok();
    buf.flush().ok();
}

fn run_animate(
    info: &[String],
    logo_name: &str,
    art: CustomArt,
    cfg: &Config,
    args: &Args,
    effect_name: &str,
    duration: Option<u64>,
) {
    let frame_ms = cfg.animation.frame_ms();
    let default_frames = (3000 / frame_ms).max(1); // ≈3s

    RUNNING.store(true, Ordering::Relaxed);
    unsafe {
        for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(sig, sigint_handler as *const () as libc::sighandler_t);
        }
    }

    let effect = parse_effect(effect_name);
    let max_frames: Option<u64> = match duration {
        None    => Some(default_frames),
        Some(0) => None, // Ctrl+C to stop
        Some(n) => Some((n * 1000 / frame_ms).max(1)),
    };

    let indent = " ".repeat(cfg.layout.indent);
    let gap = " ".repeat(cfg.layout.gap);

    let out = stdout();
    let mut buf = out.lock();
    screen_enter(&mut buf);

    // Fit frame animations to the terminal once, leaving room for the info.
    let (cols, rows) = term_size();
    let info_w = info.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
    let art_max = cols
        .saturating_sub(info_w + cfg.layout.gap + cfg.layout.indent)
        .max(1);
    let frames: Option<Vec<Vec<String>>> =
        art.anim.map(|a| a.fitted(art_max, rows.saturating_sub(2).max(1)));

    let mut tick: u64 = 0;
    loop {
        let frame_deadline = Instant::now() + Duration::from_millis(frame_ms);
        if !RUNNING.load(Ordering::Relaxed) { break; }
        if let Some(max) = max_frames { if tick >= max { break; } }
        let t = tick as usize;

        let owned;
        let base: &[String] = if let Some(frames) = &frames {
            &frames[t % frames.len()]
        } else if let Some(c) = art.lines {
            c
        } else {
            owned = logos::frame(logo_name, t, 4);
            &owned
        };
        let art = panel_text(base, t, effect, &cfg.colors.logo, &cfg.animation, !args.no_color);
        let art_w = art.iter().map(|l| visible_chars(l)).max().unwrap_or(36);
        let max_rows = art.len().max(info.len());

        // Build the whole frame, then emit it in one write. `\x1b[K` clears
        // leftovers when a frame gets shorter, so no full-screen redraw.
        let mut screen = String::with_capacity(8192);
        screen.push_str("\x1b[H\n");
        for i in 0..max_rows {
            screen.push_str(&indent);
            if let Some(line) = art.get(i) {
                screen.push_str(line);
            } else {
                screen.push_str(&" ".repeat(art_w));
            }
            screen.push_str(&gap);
            screen.push_str(info.get(i).map(String::as_str).unwrap_or(""));
            screen.push_str("\x1b[K\n");
        }
        buf.write_all(screen.as_bytes()).ok();
        buf.flush().ok();

        if let Some(left) = frame_deadline.checked_duration_since(Instant::now()) {
            thread::sleep(left);
        }
        tick += 1;
    }

    screen_leave(&mut buf);
}

fn build_info(si: &info::SysInfo, cfg: &Config) -> Vec<String> {
    let c  = &cfg.colors;
    let sh = &cfg.show;
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
            uptime_long(si.uptime_secs)
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

    lines
}

// ═══════════════════════════════════════════════════════════
//  JSONC module renderer
// ═══════════════════════════════════════════════════════════

fn norm(kind: &str) -> String {
    kind.trim().to_ascii_lowercase().replace(['-', '_'], "")
}

fn user_host() -> (String, String) {
    (
        env::var("USER").or_else(|_| env::var("LOGNAME")).unwrap_or_else(|_| "user".into()),
        hostname(),
    )
}

fn title_lines(cfg: &Config) -> Vec<String> {
    use config::Header;
    let (user, host) = user_host();
    let c = &cfg.colors;
    match cfg.show.header {
        Header::None => Vec::new(),
        Header::Both => vec![
            format!("{B}{uc}{u}{OVL}@{R}{B}{hc}{h}{R}",
                B=BOLD, uc=&c.username, u=&user, OVL=OVERLAY0, R=RESET, hc=&c.hostname, h=&host),
            format!("{B}{ac}{l}{R}", B=BOLD, ac=&c.accent,
                l="\u{2500}".repeat(user.len() + 1 + host.len()), R=RESET),
        ],
        Header::UserOnly => vec![
            format!("{B}{uc}{u}{R}", B=BOLD, uc=&c.username, u=&user, R=RESET),
            format!("{B}{ac}{l}{R}", B=BOLD, ac=&c.accent, l="\u{2500}".repeat(user.len()), R=RESET),
        ],
        Header::HostOnly => vec![
            format!("{B}{hc}{h}{R}", B=BOLD, hc=&c.hostname, h=&host, R=RESET),
            format!("{B}{ac}{l}{R}", B=BOLD, ac=&c.accent, l="\u{2500}".repeat(host.len()), R=RESET),
        ],
    }
}

fn bar_cfg(used: u64, total: u64, cfg: &Config) -> String {
    if total == 0 { return String::new(); }
    let w = cfg.display.bar_width;
    let fill = ((used as f64 / total as f64 * w as f64) as usize).min(w);
    format!("{bar}{f}{OVL}{e}{R}",
        bar=&cfg.colors.bar,
        f=cfg.display.bar_elapsed.repeat(fill),
        OVL=OVERLAY0,
        e=cfg.display.bar_total.repeat(w - fill),
        R=RESET)
}

fn module_value(kind: &str, si: &info::SysInfo, cfg: &Config) -> Option<(String, String)> {
    macro_rules! v {
        ($k:expr, $v:expr) => { Some(($k.to_string(), $v.clone())) };
    }
    match norm(kind).as_str() {
        "os" => v!("os", si.os),
        "kernel" => v!("kernel", si.kernel),
        "uptime" => {
            let val = if cfg.fastfetch {
                uptime_fastfetch(si.uptime_secs)
            } else if cfg.show.uptime_long {
                uptime_long(si.uptime_secs)
            } else {
                si.uptime.clone()
            };
            Some(("uptime".into(), val))
        }
        "res" => v!("res", si.res),
        "pkgs" | "packages" => v!("pkgs", si.pkgs),
        "shell" => v!("shell", si.shell),
        "de" | "wm" | "desktop" => v!("de/wm", si.de_wm),
        "terminal" | "term" => v!("term", si.term),
        "cpu" => v!("cpu", si.cpu),
        "gpu" => v!("gpu", si.gpu),
        "gputemp" => v!("gpu °C", si.gpu_temp),
        "battery" => {
            if cfg.fastfetch {
                let key = if si.battery_key.is_empty() { "Battery" } else { &si.battery_key };
                Some((key.to_string(), si.battery.clone()))
            } else {
                v!("battery", si.battery)
            }
        }
        "disk" => v!("disk", si.disk),
        "load" | "loadavg" => v!("load", si.load),
        "locale" => v!("locale", si.locale),
        "memory" | "mem" => {
            let bar = bar_cfg(si.mem_used, si.mem_total, cfg);
            let val = if si.mem_total >= 1024 {
                format!("{bar}{TEXT}  {u:.1}G / {t:.1}G{R}",
                    TEXT=TEXT, u=si.mem_used as f64/1024.0, t=si.mem_total as f64/1024.0, R=RESET)
            } else {
                format!("{bar}{TEXT}  {u}M / {t}M{R}", TEXT=TEXT, u=si.mem_used, t=si.mem_total, R=RESET)
            };
            Some(("memory".into(), val))
        }
        "ip" => v!("ip", si.ip),
        "ssh" => v!("ssh", si.ssh),
        "ports" => v!("ports", si.ports),
        "init" | "initsystem" => v!("init", si.init),
        "cputemp" => v!("cpu °C", si.cpu_temp),
        "processes" => v!("processes", si.processes),
        "container" => v!("container", si.container),
        "session" => v!("session", si.session),
        "swap" => v!("swap", si.swap),
        "sound" => v!("sound", si.sound),
        "gpudriver" => v!("gpu driver", si.gpu_driver),
        "bios" => v!("bios", si.bios),
        "board" => v!("board", si.board),
        "disktype" => v!("disk type", si.disk_type),
        "pkgupdates" => v!("updates", si.pkg_updates),
        "theme" => v!("theme", si.theme),
        "icons" => v!("icons", si.icons),
        "termfont" | "terminalfont" => v!("term font", si.term_font),
        "dewmver" => v!("de/wm ver", si.de_wm_ver),
        "initver" => v!("init ver", si.init_ver),
        "localip" => v!("local ip", si.local_ip),
        "datetime" => v!("datetime", si.datetime),
        "editor" => v!("editor", si.editor),
        "dns" => v!("dns", si.dns),
        "host" => v!("host", si.host),
        "chassis" => v!("chassis", si.chassis),
        "bootmgr" => v!("bootmgr", si.bootmgr),
        "cpucache" => v!("cpu cache", si.cpu_cache),
        "loginmanager" => v!("login mgr", si.login_manager),
        "brightness" => v!("brightness", si.brightness),
        "cpuusage" => v!("cpu usage", si.cpu_usage),
        "diskio" => v!("disk io", si.diskio),
        "netio" => v!("net io", si.netio),
        "physmem" => v!("phys mem", si.physmem),
        "gpumem" => v!("gpu mem", si.gpu_mem),
        "btrfs" => v!("btrfs", si.btrfs),
        "wifi" => v!("wifi", si.wifi),
        "users" => v!("users", si.users),
        "wallpaper" => v!("wallpaper", si.wallpaper),
        "media" => v!("media", si.media),
        "publicip" => v!("public ip", si.public_ip),
        "weather" => v!("weather", si.weather),
        "opengl" => v!("opengl", si.opengl),
        "vulkan" => v!("vulkan", si.vulkan),
        "cursor" => v!("cursor", si.cursor),
        "termsize" | "terminalsize" => v!("term size", si.term_size),
        "termtheme" | "terminaltheme" => v!("term theme", si.term_theme),
        "codec" => v!("codec", si.codec),
        "bluetooth" => v!("bluetooth", si.bluetooth),
        "camera" => v!("camera", si.camera),
        "mouse" => v!("mouse", si.mouse),
        "keyboard" => v!("keyboard", si.keyboard),
        "tpm" => v!("tpm", si.tpm),
        "physicaldisk" => v!("phy disk", si.physical_disk),
        "topprocesses" => v!("top procs", si.top_processes),
        "monitor" => {
            if cfg.fastfetch {
                let key = if si.display_key.is_empty() { "Display" } else { &si.display_key };
                Some((key.to_string(), si.monitor.clone()))
            } else {
                v!("monitor", si.monitor)
            }
        }
        "version" => v!("version", env!("CARGO_PKG_VERSION").to_string()),
        _ => None,
    }
}

fn format_color(spec: &str, key_color: &str) -> Option<String> {
    if spec.is_empty() { return Some(RESET.into()); }
    // Fastfetch spells the terminal default `default` (and `reset_` for a
    // full reset). Without this, `{#default}` used to fall through to blue.
    if spec == "default" || spec == "reset_" { return Some(RESET.into()); }
    if spec == "keys" { return Some(key_color.to_string()); }
    if let Some(n) = spec.strip_prefix('@').and_then(|v| v.parse::<u8>().ok()) {
        return Some(format!("\x1b[38;5;{}m", n));
    }
    if spec.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(n) = spec.parse::<u8>() {
            // Fastfetch uses 1-9 for SGR attributes (bold, dim, …).
            return Some(if n <= 9 { format!("\x1b[{n}m") } else { config::ansi_code(n) });
        }
    }
    if let Some(name) = spec.strip_prefix("bright_") {
        return Some(config::resolve(&format!("ansi_bright_{name}")));
    }
    Some(config::resolve_config_color(spec))
}

/// Parse a `{~start}` / `{~start,end}` slice token against the module value.
fn value_slice(value: &str, spec: &str) -> String {
    let mut it = spec.split(',');
    let start = it.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let end = it.next().and_then(|s| s.parse::<usize>().ok());
    value.chars().skip(start).take(end.map_or(usize::MAX, |e| e.saturating_sub(start))).collect()
}

/// Kernel name for fastfetch's `{sysname}` token.
fn sysname() -> &'static str {
    match std::env::consts::OS {
        "macos" => "Darwin",
        "freebsd" => "FreeBSD",
        "netbsd" => "NetBSD",
        "openbsd" => "OpenBSD",
        _ => "Linux",
    }
}

/// DMI host field, cleaned like fastfetch. Falls back to the collected
/// `host` string when the platform has no DMI files (macOS/BSD).
fn host_field(name: &str, si: &info::SysInfo) -> Option<String> {
    let file = match name {
        "family" => "product_family",
        "name" => "product_name",
        "version" => "product_version",
        "sku" => "product_sku",
        "vendor" => "sys_vendor",
        "serial" => "product_serial",
        "uuid" => "product_uuid",
        _ => return None,
    };
    let raw = std::fs::read_to_string(format!("/sys/class/dmi/id/{file}"))
        .unwrap_or_default();
    let raw = raw.trim();
    let bogus = ["Not Applicable", "Not Specified", "None", "Unknown",
                 "To Be Filled By O.E.M.", "System Product Name"];
    if !raw.is_empty() && !bogus.contains(&raw) {
        return Some(raw.to_string());
    }
    if name == "name" {
        let mut parts = si.host.splitn(2, char::is_whitespace);
        let first = parts.next().unwrap_or("");
        return Some(parts.next().unwrap_or(first).to_string());
    }
    if name == "vendor" {
        return si.host.split_whitespace().next().map(str::to_string);
    }
    Some(String::new())
}

/// `"83% [AC Connected, Charging]"` → (`"83%"`, `"AC Connected, Charging"`).
fn battery_parts(value: &str) -> (String, String) {
    let cap = value.split_whitespace().next().unwrap_or("").to_string();
    let status = value.split_once('[')
        .and_then(|(_, rest)| rest.rsplit_once(']').map(|(s, _)| s.trim().to_string()))
        .unwrap_or_default();
    (cap, status)
}

/// Fastfetch numeric placeholders. `{1}` still means the whole value for
/// custom modules; only host/battery have published field orders.
fn numeric_field(name: &str, kind: &str, value: &str, si: &info::SysInfo) -> Option<String> {
    let n = name.parse::<usize>().ok()?;
    match norm(kind).as_str() {
        "host" => {
            let field = match n {
                1 => "family", 2 => "name", 3 => "version", 4 => "sku",
                5 => "vendor", 6 => "serial", 7 => "uuid", _ => return Some(String::new()),
            };
            host_field(field, si)
        }
        "battery" => match n {
            4 => Some(battery_parts(value).0),
            5 => Some(battery_parts(value).1),
            _ => Some(String::new()),
        },
        "gpu" => match n {
            1 => Some(value.to_string()),
            2 => gpu_driver_value(si),
            3 => info::gpu_type().map(|t| t.to_string()),
            _ => Some(String::new()),
        },
        _ => None,
    }
}

/// Resolve the GPU driver for fastfetch-style `{2}` field references on GPU modules.
/// Computes it on demand when `show.gpu_driver` didn't already populate `si.gpu_driver`.
fn gpu_driver_value(si: &info::SysInfo) -> Option<String> {
    if !si.gpu_driver.is_empty() {
        return Some(si.gpu_driver.clone());
    }
    // Fall back to reading the DRM driver directly (same logic as collect_all).
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
    if drv == "nvidia" || drv == "Unknown" {
        if let Ok(v) = fs::read_to_string("/proc/driver/nvidia/version") {
            if let Some(l) = v.lines().next() {
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 2 { drv = format!("nvidia {}", parts[1]); }
            }
        }
    }
    if drv == "amdgpu" || drv.starts_with("amdgpu") {
        if let Ok(v) = fs::read_to_string("/sys/module/amdgpu/version") {
            let v = v.trim();
            if !v.is_empty() { drv = format!("amdgpu {}", v); }
        }
    }
    Some(drv)
}

fn format_field(
    name: &str, kind: &str, key: &str, value: &str, icon: &str,
    si: &info::SysInfo, cfg: &Config,
) -> Option<String> {
    if name.bytes().all(|b| b.is_ascii_digit()) {
        if let Some(v) = numeric_field(name, kind, value, si) {
            return Some(v);
        }
    }
    let c = &cfg.colors;
    let (user, host) = user_host();
    match name {
        "" | "value" | "1" => Some(value.to_string()),
        "key" => Some(key.to_string()),
        "icon" => Some(icon.to_string()),
        "user-name" | "username" => Some(user),
        "host-name" | "hostname" => Some(host),
        "user-name-colored" => Some(format!("{}{}{}", c.username, user, RESET)),
        "host-name-colored" => Some(format!("{}{}{}", c.hostname, host, RESET)),
        "at-symbol-colored" => Some(format!("{}{}{}", OVERLAY0, "@", RESET)),
        "sysname" => Some(sysname().into()),
        "arch" => Some(std::env::consts::ARCH.into()),
        "family" if norm(kind) == "host" => host_field("family", si),
        "name" if norm(kind) == "host" => host_field("name", si),
        "version" if norm(kind) == "host" => host_field("version", si),
        "sku" if norm(kind) == "host" => host_field("sku", si),
        "vendor" if norm(kind) == "host" => host_field("vendor", si),
        "serial" if norm(kind) == "host" => host_field("serial", si),
        "uuid" if norm(kind) == "host" => host_field("uuid", si),
        "capacity" if norm(kind) == "battery" => Some(battery_parts(value).0),
        "status" if norm(kind) == "battery" => Some(battery_parts(value).1),
        "pretty-name" | "name" if norm(kind) == "os" => Some(si.os.clone()),
        "pretty-name" | "name" if matches!(norm(kind).as_str(), "de" | "wm" | "desktop") => Some(si.de_wm.clone()),
        "pretty-name" | "name" if matches!(norm(kind).as_str(), "term" | "terminal") => Some(si.term.clone()),
        "process-name" | "exe-name" if matches!(norm(kind).as_str(), "de" | "wm" | "desktop") => Some(si.de_wm.clone()),
        "process-name" | "exe-name" if matches!(norm(kind).as_str(), "term" | "terminal") => Some(si.term.clone()),
        "release" | "kernel-release" if norm(kind) == "kernel" => Some(si.kernel.clone()),
        "release" | "kernel-release" if norm(kind) == "os" => {
            if cfg.fastfetch { Some(si.os_version.clone()) } else { Some(si.kernel.clone()) }
        }
        "id" => Some(si.os_id.clone()),
        "id-like" => Some(si.os_id_like.clone()),
        "version" if norm(kind) == "kernel" => Some(si.kernel.clone()),
        "version" if norm(kind) == "os" => {
            if cfg.fastfetch { Some(si.os_version.clone()) } else { Some(si.kernel.clone()) }
        }
        "version" if matches!(norm(kind).as_str(), "de" | "wm" | "desktop") => Some(si.de_wm_ver.clone()),
        "version" if matches!(norm(kind).as_str(), "term" | "terminal") => {
            Some(env::var("TERM_PROGRAM_VERSION").unwrap_or_default())
        }
        "protocol-name" if matches!(norm(kind).as_str(), "de" | "wm" | "desktop") => {
            Some(if env::var_os("WAYLAND_DISPLAY").is_some() {
                "Wayland".into()
            } else if env::var_os("DISPLAY").is_some() {
                "X11".into()
            } else {
                String::new()
            })
        }
        "formatted" | "combined" | "result" => Some(value.to_string()),
        "filesystem" if norm(kind) == "disk" => Some(root_filesystem()),
        "all" | "appimage" | "flatpak-system" | "flatpak-user" | "flatpak-all"
        | "nix-default" | "nix-system" | "nix-user" | "nix-all"
        | "pacman" | "dpkg" | "rpm" | "apk" | "portage" | "xbps" | "bedrock"
            if matches!(norm(kind).as_str(), "pkgs" | "packages") => package_field(value, name),
        "temperature" if norm(kind) == "cpu" => Some(si.cpu_temp.clone()),
        "temperature" if norm(kind) == "gpu" => Some(si.gpu_temp.clone()),
        "usage" | "avg" if matches!(norm(kind).as_str(), "cpu" | "cpuusage") => {
            Some(si.cpu_usage.clone())
        }
        "freq-max" | "frequency" if norm(kind) == "cpu" => {
            let khz = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
                .ok()
                .and_then(|v| v.trim().parse::<f64>().ok())?;
            Some(format!("{:.2} GHz", khz / 1_000_000.0))
        }
        "type" if norm(kind) == "gpu" => info::gpu_type().map(str::to_string),
        "vendor" if norm(kind) == "gpu" => value.split_whitespace().next().map(str::to_string),
        "process-name" | "exe-name" | "pretty-name" if norm(kind) == "shell" => {
            Some(si.shell.split_whitespace().next().unwrap_or(&si.shell).to_string())
        }
        "version" if norm(kind) == "shell" => {
            si.shell.split_whitespace().nth(1).map(str::to_string)
        }
        "user-id" => Some(unsafe { libc::getuid() }.to_string()),
        "mac" if norm(kind) == "localip" => {
            let ifname = si.local_ip.split_whitespace().next().unwrap_or("");
            (ifname != "N/A").then(|| {
                fs::read_to_string(format!("/sys/class/net/{ifname}/address"))
                    .map(|v| v.trim().to_string()).unwrap_or_default()
            })
        }
        "artist" if norm(kind) == "media" => {
            si.media.split_once(" - ").map(|(a, _)| a.to_string())
        }
        "home-dir" => env::var("HOME").ok(),
        "user-shell" => env::var("SHELL").ok(),
        "exe-path" => env::current_exe().ok().map(|p| p.display().to_string()),
        "pid" => Some(std::process::id().to_string()),
        "cwd" => env::current_dir().ok().map(|p| p.display().to_string()),
        "name" => Some(value.to_string()),
        "title" if norm(kind) == "media" => {
            Some(si.media.split_once(" - ").map(|(_, t)| t).unwrap_or(&si.media).to_string())
        }
        "used" | "total" | "percentage" | "size-used" | "size-total" | "size-percentage"
        | "percentage-bar" | "size-percentage-bar" => {
            let (used, total) = match norm(kind).as_str() {
                "memory" | "mem" => (si.mem_used as f64 / 1024.0, si.mem_total as f64 / 1024.0),
                "disk" => parse_pair(&si.disk),
                "swap" => parse_pair(&si.swap),
                _ => parse_pair(value),
            };
            if name.ends_with("percentage-bar") {
                return Some(bar_cfg(used as u64, total as u64, cfg));
            }
            let pct = if total > 0.0 { used / total * 100.0 } else { 0.0 };
            // Fastfetch prints binary units with two digits; arcfetch's own
            // compact layout keeps one digit.
            let val = if cfg.fastfetch {
                match name {
                    "used" => format!("{used:.2} GiB"),
                    "total" => format!("{total:.2} GiB"),
                    "percentage" => format!("{pct:.0}%"),
                    "size-used" => format!("{used:.2} GiB"),
                    "size-total" => format!("{total:.2} GiB"),
                    _ => format!("{pct:.0}%"),
                }
            } else {
                match name {
                    "used" => format!("{used:.1}"),
                    "total" => format!("{total:.1}"),
                    "percentage" => format!("{pct:.0}%"),
                    "size-used" => format!("{used:.1}G"),
                    "size-total" => format!("{total:.1}G"),
                    _ => format!("{pct:.0}%"),
                }
            };
            Some(val)
        }
        "days" | "hours" | "minutes" | "seconds" => {
            let s = si.uptime_secs;
            let n = match name {
                "days" => s / 86400,
                "hours" => (s % 86400) / 3600,
                "minutes" => (s % 3600) / 60,
                _ => s % 60,
            };
            Some(n.to_string())
        }
        "ipv4" | "ip" if matches!(norm(kind).as_str(), "localip" | "publicip" | "ip") => {
            if norm(kind) == "localip" { Some(si.local_ip.split_whitespace().last().unwrap_or("").into()) }
            else { Some(value.to_string()) }
        }
        "ifname" if norm(kind) == "localip" => Some(si.local_ip.split_whitespace().next().unwrap_or("").into()),
        "cores-logical" | "cores-online" | "cores-physical" => {
            Some(unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) }.to_string())
        }
        "driver" => Some(si.gpu_driver.clone()),
        _ => None,
    }
}

fn parse_pair(s: &str) -> (f64, f64) {
    let mut it = s.split('/').map(|v| {
        v.trim().trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse::<f64>().unwrap_or(0.0)
    });
    (it.next().unwrap_or(0.0), it.next().unwrap_or(0.0))
}

fn root_filesystem() -> String {
    std::fs::read_to_string("/proc/mounts").ok()
        .and_then(|s| s.lines().find_map(|line| {
            let mut f = line.split_whitespace();
            let _dev = f.next()?;
            let mount = f.next()?;
            let fs = f.next()?;
            (mount == "/").then(|| fs.to_string())
        }))
        .unwrap_or_default()
}

/// Pull one manager's count out of a `"2302 (pacman), 7 (flatpak-system)"` list.
fn package_field(value: &str, name: &str) -> Option<String> {
    if name == "all" {
        let total: u64 = value.split(',')
            .filter_map(|p| p.trim().split_once('('))
            .filter_map(|(n, _)| n.trim().parse::<u64>().ok())
            .sum();
        return Some(total.to_string());
    }
    if name == "flatpak-all" || name == "nix-all" {
        let names: &[&str] = if name == "flatpak-all" {
            &["flatpak-system", "flatpak-user"]
        } else {
            &["nix-default", "nix-system", "nix-user"]
        };
        let mut total = 0u64;
        for part in value.split(',') {
            let part = part.trim();
            let Some(open) = part.find('(') else { continue };
            let mgr = part[open + 1..].trim_end_matches(')').trim();
            if names.contains(&mgr) {
                total += part[..open].trim().parse::<u64>().unwrap_or(0);
            }
        }
        return Some(total.to_string());
    }
    for part in value.split(',') {
        let part = part.trim();
        let Some(open) = part.find('(') else { continue };
        let count = part[..open].trim();
        let mgr = part[open + 1..].trim_end_matches(')').trim();
        if mgr == name { return Some(count.to_string()); }
    }
    None
}

fn uptime_long(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    match (d, h, m) {
        (0, 0, m) => format!("{} min{}", m, if m == 1 { "" } else { "s" }),
        (0, h, m) => format!("{} hr{}, {} min{}",
            h, if h == 1 { "" } else { "s" }, m, if m == 1 { "" } else { "s" }),
        (d, h, m) => format!("{} day{}, {} hr{}, {} min{}",
            d, if d == 1 { "" } else { "s" },
            h, if h == 1 { "" } else { "s" },
            m, if m == 1 { "" } else { "s" }),
    }
}

/// Fastfetch's own uptime wording: `1 hour, 30 mins`.
fn uptime_fastfetch(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    let mut parts = Vec::new();
    if d > 0 { parts.push(format!("{} day{}", d, if d == 1 { "" } else { "s" })); }
    if h > 0 { parts.push(format!("{} hour{}", h, if h == 1 { "" } else { "s" })); }
    if m > 0 || parts.is_empty() { parts.push(format!("{} min{}", m, if m == 1 { "" } else { "s" })); }
    parts.join(", ")
}

fn format_template(
    tpl: &str, kind: &str, key: &str, value: &str, icon: &str,
    si: &info::SysInfo, cfg: &Config, key_color: &str,
) -> String {
    let mut terminated = false;
    format_template_inner(tpl, kind, key, value, icon, si, cfg, key_color, &mut terminated)
}

fn format_template_inner(
    tpl: &str, kind: &str, key: &str, value: &str, icon: &str,
    si: &info::SysInfo, cfg: &Config, key_color: &str, terminated: &mut bool,
) -> String {
    let mut out = String::with_capacity(tpl.len() + 32);
    let b = tpl.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if *terminated { return out; }
        if b[i] != b'{' {
            // Copy whole UTF-8 runs, not byte-by-byte, so non-ASCII text
            // (box drawing, nerd icons) is not re-encoded as Latin-1.
            let start = i;
            while i < b.len() && b[i] != b'{' { i += 1; }
            out.push_str(&tpl[start..i]);
            continue;
        }
        let Some(rel) = tpl[i + 1..].find('}') else { out.push_str(&tpl[i..]); break; };
        let end = i + 1 + rel;
        let token = &tpl[i + 1..end];
        i = end + 1;

        if token == "-" {
            *terminated = true;
            return out;
        }
        if let Some(idx) = token.strip_prefix('$').and_then(|n| n.parse::<usize>().ok()) {
            if let Some(c) = cfg.display.constants.get(idx.saturating_sub(1)) { out.push_str(c); }
            continue;
        }
        if let Some(spec) = token.strip_prefix('#') {
            if let Some(col) = format_color(spec, key_color) { out.push_str(&col); }
            continue;
        }
        // fastfetch `{/field}...{/}`: render the block only when field is unset.
        if let Some(field) = token.strip_prefix('/') {
            if field.is_empty() { continue; }
            let rest = &tpl[i..];
            if let Some(rel) = rest.find("{/}") {
                let inner = &rest[..rel];
                let unset = format_field(field, kind, key, value, icon, si, cfg)
                    .is_none_or(|v| v.is_empty());
                if unset {
                    out.push_str(&format_template_inner(
                        inner, kind, key, value, icon, si, cfg, key_color, terminated));
                }
                i += rel + 3;
                if *terminated { return out; }
            }
            continue;
        }
        if let Some(field) = token.strip_prefix('?') {
            if field.is_empty() { continue; }
            let rest = &tpl[i..];
            if let Some(rel) = rest.find("{?}") {
                let inner = &rest[..rel];
                let nonempty = format_field(field, kind, key, value, icon, si, cfg)
                    .is_some_and(|v| !v.is_empty());
                if nonempty {
                    out.push_str(&format_template_inner(
                        inner, kind, key, value, icon, si, cfg, key_color, terminated));
                }
                i += rel + 3;
                if *terminated { return out; }
            }
            continue;
        }
        if let Some(spec) = token.strip_prefix('~') {
            out.push_str(&value_slice(value, spec));
            continue;
        }
        if token.is_empty() { out.push_str(value); continue; }
        if let Some(v) = format_field(token, kind, key, value, icon, si, cfg) {
            out.push_str(&v);
        }
        // Unknown placeholders (fastfetch-specific fields we don't collect) vanish
        // instead of leaking literal `{...}` into the rendered output.
    }
    out
}

fn render_module(m: &config::Module, si: &info::SysInfo, cfg: &Config, li: &mut usize) -> Vec<String> {
    let kind = norm(&m.kind);
    let icon = m.key_icon.as_deref().unwrap_or("");
    let key_color = m.key_color.as_deref()
        .or(cfg.display.key_color.as_deref())
        .unwrap_or_else(|| cfg.colors.label(*li));

    match kind.as_str() {
        "break" => return vec![String::new()],
        "separator" => {
            // fastfetch spells the row text `string`; accept it as `format`.
            let s = m.format.as_deref().filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| "\u{2500}".repeat(30));
            let color = m.output_color.as_deref()
                .or(cfg.display.separator_color.as_deref())
                .unwrap_or("");
            return vec![format!("{color}{s}{RESET}")];
        }
        "custom" => {
            if let Some(f) = &m.format {
                return f.split('\n')
                    .map(|line| format_template(line, "custom", "", "", "", si, cfg, key_color))
                    .collect();
            }
            // `[[custom]] label = "..." value = "..."` (or command) is the
            // shorthand for a plain labelled row.
            let value = if let Some(cmd) = &m.command {
                modules::run_timeout("sh", &["-c", cmd], cfg.command_timeout_ms)
                    .unwrap_or_default().trim().to_string()
            } else {
                m.value.clone().unwrap_or_default()
            };
            let key = m.key.clone().unwrap_or_default();
            if key.is_empty() && value.is_empty() { return Vec::new(); }
            let value = if m.value_format {
                format_template(&value, "custom", &key, &value, "", si, cfg, key_color)
            } else {
                value
            };
            let val_color = m.output_color.as_deref().unwrap_or(&cfg.colors.values);
            return vec![row_cfg(&key, &value, key_color, val_color, m, cfg)];
        }
        "title" => {
            let lines = match &m.format {
                Some(f) => vec![format_template(f, "title", "", "", icon, si, cfg, key_color)],
                None => title_lines(cfg),
            };
            return lines;
        }
        "colors" => {
            let sym = match m.symbol.as_deref().unwrap_or("") {
                "circle" => "\u{25cf}",
                _ => "\u{2588}\u{2588}",
            };
            let mut s = String::with_capacity(64);
            for n in 0..16 {
                s.push_str(&format!("\x1b[38;5;{n}m{sym}"));
            }
            s.push_str(RESET);
            return vec![format!("{}{}", " ".repeat(m.padding_left + cfg.display.key_padding_left), s)];
        }
        "command" => {
            let Some(cmd) = &m.text else { return Vec::new() };
            let Some(raw) = modules::run_timeout("sh", &["-c", cmd], cfg.command_timeout_ms) else { return Vec::new() };
            let value = raw.trim();
            if value.is_empty() { return Vec::new(); }
            let key = m.key.clone().unwrap_or_default();
            let key = format!("{icon}{key}");
            let key = format_template(&key, "command", &key, value, icon, si, cfg, key_color);
            let value = match &m.format {
                Some(f) => format_template(f, "command", &key, value, icon, si, cfg, key_color),
                None => value.to_string(),
            };
            let row = row_cfg(&key, &value, key_color,
                m.output_color.as_deref().unwrap_or(&cfg.colors.values), m, cfg);
            *li += 1;
            return row.split('\n').map(String::from).collect();
        }
        _ => {}
    }

    let Some((default_key, mut value)) = module_value(&kind, si, cfg) else { return Vec::new() };
    if (kind == "pkgs" || kind == "packages") && !m.disabled.is_empty() {
        value = crate::pkgs::filter(&value, &m.disabled);
    }
    if (kind == "pkgs" || kind == "packages") && m.combined {
        value = crate::pkgs::combine(&value);
    }
    if let Some(cmd) = &m.command {
        value = modules::run_timeout("sh", &["-c", cmd], cfg.command_timeout_ms)
            .unwrap_or_default().trim().to_string();
    } else if let Some(v) = &m.value {
        value = v.clone();
    }
    if value.is_empty() { return Vec::new(); }
    let key = m.key.clone().unwrap_or(default_key);
    let key = format!("{icon}{key}");
    let key = format_template(&key, &kind, &key, &value, icon, si, cfg, key_color);
    let value = match &m.format {
        Some(f) => {
            // arcfetch's own `{}` shorthand keeps the old label-stripping
            // behaviour; fastfetch configs use named fields that need the
            // labelled list (`{all}`, `{pacman}`, …).
            let v = if !cfg.fastfetch && (kind == "pkgs" || kind == "packages") {
                value.split(',')
                    .map(|part| part.trim().split_whitespace().next().unwrap_or(part.trim()).to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                value
            };
            format_template(f, &kind, &key, &v, icon, si, cfg, key_color)
        }
        None => value,
    };
    let row = row_cfg(&key, &value, key_color,
        m.output_color.as_deref().unwrap_or(&cfg.colors.values), m, cfg);
    *li += 1;
    row.split('\n').map(String::from).collect()
}

fn row_cfg(key: &str, val: &str, key_color: &str, val_color: &str, m: &config::Module, cfg: &Config) -> String {
    let pad = m.padding_left + cfg.display.key_padding_left;
    if !cfg.display.labels {
        let mut s = String::with_capacity(val.len() + pad + 16);
        for _ in 0..pad { s.push(' '); }
        s.push_str(val_color);
        s.push_str(val);
        s.push_str(RESET);
        return s;
    }
    let sep = cfg.display.separator.as_deref().unwrap_or(" ");
    let sep = match &cfg.display.separator_color {
        Some(c) => format!("{c}{sep}{RESET}"),
        None => sep.to_string(),
    };
    let width = if key.contains('\x1b') { 0 } else { m.key_width.or(cfg.display.key_width).unwrap_or(9) };
    let mut s = String::with_capacity(key.len() + val.len() + 32);
    for _ in 0..pad { s.push(' '); }
    s.push_str(BOLD);
    s.push_str(key_color);
    s.push_str(key);
    for _ in visible_chars(key)..width { s.push(' '); }
    s.push_str(RESET);
    s.push_str(&sep);
    s.push_str(val_color);
    s.push_str(val);
    s.push_str(RESET);
    s
}

fn render_modules(si: &info::SysInfo, cfg: &Config) -> Vec<String> {
    let Some(mods) = cfg.modules.as_deref() else { return Vec::new() };
    let mut out = Vec::with_capacity(mods.len() + 4);
    let mut li = 0usize;
    let mut prev_ok = true;
    for m in mods {
        // fastfetch `condition.succeeded` chains to the preceding module.
        if let Some(want) = m.condition_succeeded {
            if want != prev_ok { continue; }
        }
        let rows = render_module(m, si, cfg, &mut li);
        if !matches!(norm(&m.kind).as_str(), "break" | "separator" | "title" | "colors") {
            prev_ok = !rows.is_empty();
        }
        out.extend(rows);
    }
    out
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

/// Play duration for the non-pinned modes: `--t` wins, then `play_seconds`.
fn play_duration(duration: Option<u64>, cfg: &Config) -> Option<u64> {
    duration.or_else(|| {
        (cfg.animation.play_seconds > 0.0)
            .then(|| cfg.animation.play_seconds.round().max(1.0) as u64)
    })
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
                    let lc = if args.color.is_some() {
                        cfg.colors.accent.as_str()
                    } else {
                        cfg.logo.color.as_deref().unwrap_or(cfg.colors.accent.as_str())
                    };
                    writeln!(buf, "  {lc}{ll}{pad}{R}  {inf}",
                        lc = lc, ll = ll,
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
    config_file:  Option<String>,
    no_color:     bool,
    palette:      bool,
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
    box_style:    Option<String>,
    animate:      Option<String>,
    animation:    Option<String>,
    fps:          Option<f32>,
    list_logos:   bool,
    list_schemes: bool,
    list_presets: bool,
    list_animations: bool,
    no_intro:     bool,
    pin:          bool,
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
        config_file: None,
        no_color: false,
        palette: false,
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
        box_style: None,
        animate: None,
        animation: None,
        fps: None,
        list_logos: false,
        list_schemes: false,
        list_presets: false,
        list_animations: false,
        no_intro: false,
        pin: false,
        stop: false,
    };

    // Accept `-preset`/`-config` as aliases for the long forms.
    let mut argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
    for arg in &mut argv {
        if let Some(s) = arg.to_str() {
            if s.len() > 2 && s.starts_with('-') && !s.starts_with("--") {
                *arg = format!("-{s}").into();
            }
        }
    }

    // lexopt cannot peek, so pre-scan for the space-separated `--mandelbrot <n>` form.
    let mut mandel_space: Option<u32> = None;
    let mut animate_space: Option<String> = None;
    let mut box_space: Option<String> = None;
    let mut config_space: Option<String> = None;
    {
        for w in argv.windows(2) {
            if w[0] == "--mandelbrot" {
                if let Some(s) = w[1].to_str() {
                    if let Ok(n) = s.parse::<u32>() { mandel_space = Some(n); }
                }
            } else if w[0] == "--animate" {
                if let Some(s) = w[1].to_str() {
                    if !s.starts_with('-') { animate_space = Some(s.to_string()); }
                }
            } else if w[0] == "--box" {
                if let Some(s) = w[1].to_str() {
                    if is_box_style(s) { box_space = Some(s.to_string()); }
                }
            } else if w[0] == "--config" {
                if let Some(s) = w[1].to_str() {
                    if !s.starts_with('-') { config_space = Some(s.to_string()); }
                }
            }
        }
    }

    let mut parser = lexopt::Parser::from_args(argv);
    while let Some(arg) = parser.next().ok().flatten() {
        match arg {
            Short('h') | Long("help")    => a.help = true,
            Short('V') | Long("version") => a.version = true,
            Long("blackhole")            => a.blackhole = true,
            Long("no-color")             => a.no_color = true,
            Long("palette")              => a.palette = true,
            Long("config") => {
                if let Some(v) = parser.optional_value() {
                    a.config_file = Some(v.to_string_lossy().into_owned());
                } else if let Some(v) = config_space.clone() {
                    a.config_file = Some(v);
                } else {
                    a.show_cfg = true;
                }
            }
            Long("full")                 => a.all = true,
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
            Long("list-presets") | Long("presets") => a.list_presets = true,
            Long("list-animations") | Long("animations") => a.list_animations = true,
            Long("no-animate") | Long("no-intro") => a.no_intro = true,
            Long("foreground") | Long("fg") => {}
            Long("pin") => a.pin = true,
            Long("stop") | Long("unpin") => a.stop = true,
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
            Long("box") => {
                a.box_style = Some(parser.optional_value()
                    .map(|v| v.to_string_lossy().into_owned())
                    .or_else(|| box_space.clone())
                    .unwrap_or_else(|| "round".into()));
            }
            Long("animate") => {
                a.animate = Some(parser.optional_value()
                    .map(|v| v.to_string_lossy().into_owned())
                    .or_else(|| animate_space.clone())
                    .unwrap_or_else(|| "shimmer".into()));
            }
            Long("animation") => {
                if let Ok(v) = parser.value() {
                    a.animation = Some(v.to_string_lossy().into_owned());
                }
            }
            Long("fps") => {
                if let Ok(v) = parser.value() {
                    a.fps = v.to_string_lossy().parse().ok();
                }
            }
            Short('s') | Long("structure") => {
                if let Ok(v) = parser.value() {
                    a.structure = Some(v.to_string_lossy().into_owned());
                }
            }
            _ => {}
        }
    }
    if a.animation.is_some() && a.animate.is_none() {
        a.animate = Some("frames".into());
    }
    if a.pin && a.animate.is_none() && !a.blackhole {
        a.animate = Some("shimmer".into());
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
    let field = |key: &str, val: &str, desc: &str| {
        println!("  {g}{key:<14}{r} = {val:<10} {s}# {desc}{r}",
            g = g, r = r, key = key, val = val, s = s, desc = desc);
    };

    println!();
    println!("  {b}{PEACH}config file{r}  {s}{cfg_path}{r}", b=b, PEACH=PEACH, r=r, s=s, cfg_path=cfg_path);
    println!("  {s}TOML subset: [section] headers, key = value, # comments. Every key is optional.{r}", s=s, r=r);
    println!("  {s}Arrays may span lines; ~ is expanded in logo and palette paths.{r}", s=s, r=r);
    println!("  {s}Unknown keys/sections are ignored. Booleans: true/yes/1/on enable; anything else disables.{r}", s=s, r=r);
    println!();

    println!("  {b}{BLUE}[palette]{r}  {s}wallpaper-derived colours (matugen, pywal, wallust){r}", b=b, BLUE=BLUE, r=r, s=s);
    field("color_source", "config", "config | palette");
    println!("  {g}palette_files{r} = [ … ]  {s}# first usable JSON file wins; ~ expanded{r}", g=g, r=r, s=s);
    println!("  {s}              default: quickshell/matugen, pywal, wallust paths{r}", s=s, r=r);
    field("palette_accent", "primary", "name for hostname, rule, logo and bar");
    field("palette_value", "on_surface", "name for values");
    println!("  {g}palette_gradient{r} = [ … ] {s}# names for logo + animated-art stops; [] leaves it alone{r}", g=g, r=r, s=s);
    println!("  {s}              pywal/wallust fall back to color4, color7 and [color4, color5, color6]{r}", s=s, r=r);
    println!("  {s}              nested names work: {{\"colors\":{{\"color4\":\"#...\"}}}} and special.background{r}", s=s, r=r);
    println!("  {s}              same keys under [palette] as files/accent/value/gradient; --palette forces it{r}", s=s, r=r);
    println!();

    println!("  {b}{BLUE}[colors]{r}  {s}keys apply in file order — put scheme first if you use it{r}", b=b, BLUE=BLUE, r=r, s=s);
    field("scheme", "mocha", "palette preset; default mocha");
    println!("  {s}              values: auto|mocha|latte|frappe|macchiato|nord|dracula|gruvbox|tokyo-night{r}", s=s, r=r);
    println!("  {s}                      solarized-dark|rose-pine|onedark|kanagawa|everforest|ayu{r}", s=s, r=r);
    println!("  {s}              auto = latte on a light terminal, mocha on dark/unknown; aliases: catppuccin,{r}", s=s, r=r);
    println!("  {s}              tokyonight, atom, gruvbox-dark, solarized, rosepine, ayu-dark, …{r}", s=s, r=r);
    field("accent", "blue", "default logo colour, header rule and box border");
    field("username", "mauve", "user part of user@host");
    field("hostname", "blue", "host part of user@host");
    field("c1", "blue", "field-label colour 1");
    field("c2", "sapphire", "field-label colour 2");
    field("c3", "sky", "field-label colour 3");
    field("c4", "teal", "field-label colour 4");
    field("c5", "green", "field-label colour 5");
    field("c6", "yellow", "field-label colour 6");
    field("c7", "peach", "field-label colour 7; c1..c7 repeat after 7 visible fields");
    field("values", "subtext1", "all field values");
    field("sep", "overlay0", "parsed but not used by the current renderer");
    field("bar", "blue", "memory bar fill");
    field("logo1..logo9", "c1..c7, blue, text", "custom-logo $1..$9 colours");
    println!("  {s}# colour specs (any colour key):{r}", s=s, r=r);
    println!("  {s}#   catppuccin  rosewater flamingo pink mauve red maroon peach yellow green teal sky sapphire blue lavender text subtext1 overlay0{r}", s=s, r=r);
    println!("  {s}#   extra       black white gray/grey silver navy lime aqua fuchsia purple/magenta cyan orange{r}", s=s, r=r);
    println!("  {s}#   ANSI        ansi_black … ansi_bright_white (16 base colours){r}", s=s, r=r);
    println!("  {s}#   numeric     #RGB | #RRGGBB | rgb(r,g,b) | color0..color255 | bare 0..255; unknown = blue{r}", s=s, r=r);
    println!();
    println!("  {b}{BLUE}[layout]{r}  {s}box and spacing for normal output{r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  {g}box{r}       = none     {s}# none | round | square | double | heavy | dashed | ascii{r}", g=g, r=r, s=s);
    println!("  {g}box_color{r} = accent   {s}# border colour; none = uncoloured{r}", g=g, r=r, s=s);
    println!("  {g}indent{r}    = 2        {s}# left margin in columns{r}", g=g, r=r, s=s);
    println!("  {g}gap{r}       = 2        {s}# spaces between logo and info{r}", g=g, r=r, s=s);
    println!();
    println!("  {b}{BLUE}rows{r}  {s}root keys: pick a layout, then override any row{r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  {g}info_style{r} = classic  {s}# classic | minimal | boxed | sections | tree | iconic | card | chips (card/chips = boxed){r}", g=g, r=r, s=s);
    println!("  {g}modules{r}    = [\"os\", \"host\", \"cpu\", \"memory\", \"disk\"]  {s}# ordered rows{r}", g=g, r=r, s=s);
    println!("  {s}  [[modules]] gives one row: type, key, keyIcon, format, keyColor, outputColor{r}", s=s, r=r);
    println!("  {s}  [[custom]] gives a line of your own: format = \"...\"{r}", s=s, r=r);
    println!("  {s}  [overrides.cpu] changes a built-in row: label | value | command{r}", s=s, r=r);
    println!("  {g}command_timeout{r} = 1.0  {s}# seconds a command row may take (0.1–10){r}", g=g, r=r, s=s);
    println!();
    println!("  {b}{BLUE}[display]{r}  {s}row separator and value-only output{r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  {g}separator{r} = \" \"  {g}labels{r} = true  {g}key_width{r} = 9  {g}bar_width{r} = 14", g=g, r=r);
    println!("  {s}labels = false prints values only (minimal/iconic styles do this){r}", s=s, r=r);
    println!();
    println!("  {b}{BLUE}[show]{r}  {s}per-field visibility; true/yes/1/on enables, anything else disables{r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  {s}# --all enables every data field below except header, uptime_long, swatches and color_bar.{r}", s=s, r=r);
    field("header", "both", "both | user/username | host/hostname | none/false");
    println!("  {s}# core fields (on by default):{r}", s=s, r=r);
    field("os", "true", "distro PRETTY_NAME from /etc/os-release");
    field("kernel", "true", "kernel release from uname -r");
    field("uptime", "true", "short uptime, e.g. 3d 4h 5m");
    field("pkgs", "true", "installed package count (cached 1 h)");
    field("shell", "true", "basename of $SHELL");
    field("de_wm", "true", "$XDG_CURRENT_DESKTOP / $DESKTOP_SESSION / Hyprland / Sway / X11 / TTY");
    field("term", "true", "$TERM_PROGRAM, fallback $TERM");
    field("cpu", "true", "CPU brand, core count and max GHz (cached 24 h)");
    field("gpu", "true", "GPU name from DRM/NVIDIA/PCI (cached 24 h)");
    field("memory", "true", "used / total RAM with a 14-cell bar");
    println!("  {s}# optional core fields:{r}", s=s, r=r);
    field("uptime_long", "false", "true = 3 days, 4 hrs, 5 mins (only changes uptime)");
    field("res", "false", "primary output resolution from /sys/class/drm");
    field("gpu_temp", "false", "GPU hwmon temperature in °C, or N/A");
    field("battery", "false", "BAT0/BAT1/BAT capacity + charging icon, or N/A");
    field("disk", "false", "root filesystem used / total");
    field("load", "false", "1, 5 and 15 minute load averages");
    field("locale", "false", "$LANG");
    println!("  {s}# network fields (collected together when any of ip/ssh/ports is on):{r}", s=s, r=r);
    field("ip", "false", "primary interface IPv4 address");
    field("ssh", "false", "port 22 listening: active / inactive");
    field("ports", "false", "count of listening TCP ports");
    println!("  {s}# extra rows:{r}", s=s, r=r);
    field("swatches", "false", "14-dot Catppuccin palette row (fixed colours)");
    field("color_bar", "false", "8-block neofetch-style colour bar");
    field("init", "false", "PID 1 init name (systemd/OpenRC/runit/s6/…)");
    field("cpu_temp", "false", "CPU package temperature from hwmon or ACPI, or N/A");
    field("processes", "false", "number of processes in /proc");
    field("container", "false", "Docker / Podman / Kubernetes / LXC / none");
    field("session", "false", "$XDG_SESSION_TYPE, or tty");
    println!("  {s}# fastfetch-inspired fields (off by default):{r}", s=s, r=r);
    field("swap", "false", "swap used / total, or N/A");
    field("sound", "false", "PipeWire / PulseAudio / ALSA");
    field("gpu_driver", "false", "DRM driver and version (nvidia/amdgpu)");
    field("bios", "false", "DMI BIOS version and date");
    field("board", "false", "DMI motherboard vendor and model");
    field("disk_type", "false", "SSD / NVMe SSD / HDD from the root device");
    field("pkg_updates", "false", "pacman sync repos found (lightweight, not a full update check)");
    field("theme", "false", "GTK theme from gtk-3.0/settings.ini (gtk2 fallback)");
    field("icons", "false", "GTK icon theme");
    field("term_font", "false", "terminal font from the environment or terminal config");
    field("de_wm_ver", "false", "desktop environment / WM version (may spawn <wm> --version)");
    field("init_ver", "false", "init version (systemd shows distro VERSION_ID)");
    field("local_ip", "false", "interface + local IPv4 address");
    println!("  {s}# opt-in modules (all off by default; or use --all):{r}", s=s, r=r);
    field("datetime", "false", "local date and time");
    field("editor", "false", "$VISUAL or $EDITOR");
    field("dns", "false", "nameservers from /etc/resolv.conf");
    field("host", "false", "DMI system vendor and product");
    field("chassis", "false", "DMI chassis type (Laptop/Desktop/…)");
    field("bootmgr", "false", "UEFI or BIOS");
    field("cpu_cache", "false", "L1 / L2 / L3 cache sizes");
    field("login_manager", "false", "display-manager.service");
    field("brightness", "false", "backlight percentage");
    field("cpu_usage", "false", "/proc/stat sample (at least 60 ms)");
    field("diskio", "false", "totals from /proc/diskstats");
    field("netio", "false", "totals from /proc/net/dev");
    field("physmem", "false", "DIMM slots from DMI (root may be required)");
    field("gpu_mem", "false", "VRAM used / total");
    field("btrfs", "false", "btrfs volumes");
    field("wifi", "false", "wireless interface, SSID and signal");
    field("users", "false", "logged-in users");
    field("wallpaper", "false", "current wallpaper (Hyprland/Sway/GNOME)");
    field("media", "false", "now playing via playerctl");
    field("public_ip", "false", "public IP (network; 1 h cache, stale-while-revalidate)");
    field("weather", "false", "weather from wttr.in (network; 15 min cache)");
    field("opengl", "false", "OpenGL renderer via glxinfo (24 h cache)");
    field("vulkan", "false", "Vulkan drivers (24 h cache)");
    field("cursor", "false", "cursor theme and size");
    field("term_size", "false", "terminal columns x rows");
    field("term_theme", "false", "terminal FG/BG from $COLORFGBG (5 min cache)");
    field("codec", "false", "VA-API encoders/decoders via vainfo (24 h cache)");
    field("bluetooth", "false", "Bluetooth radio and HCI version (24 h cache)");
    field("camera", "false", "connected cameras");
    field("mouse", "false", "pointing devices");
    field("keyboard", "false", "keyboards");
    field("tpm", "false", "TPM version");
    field("physical_disk", "false", "disks: model, size, type and temperature");
    field("top_processes", "false", "top 5 processes by CPU");
    field("monitor", "false", "EDID: name, resolution, Hz, size and PPI");
    println!();

    println!("  {b}{BLUE}[logo]{r}  {s}pick a built-in logo or a custom file{r}", b=b, BLUE=BLUE, r=r, s=s);
    field("name", "arch", "built-in name; default arch (minimal preset uses mini; science picks random)");
    println!("  {s}              names: arch, mini, ascii, tux, nix, gentoo, bsd, android, termux, auto,{r}", s=s, r=r);
    println!("  {s}                      atom, dna, arcreactor,{r}", s=s, r=r);
    println!("  {s}                      aurora, bedrock, guix, ubuntu, debian, fedora, …{r}", s=s, r=r);
    println!("  {s}              _small suffixes are compact variants; --list-logos prints every name{r}", s=s, r=r);
    println!("  {s}              auto detects /etc/os-release ID/ID_LIKE; unknown IDs fall back to arch{r}", s=s, r=r);
    field("file", "<path>", "custom ASCII or png/jpg/gif (kitty protocol); overrides name");
    println!("  {s}              text files support $1..$9 = logo1..logo9, $R = reset, $$ = literal ${r}", s=s, r=r);
    field("position", "left", "image logos: left|right = info beside, top = info below");
    field("width", "31", "image logo width in terminal columns");
    field("height", "auto", "image logo height in rows; omit to keep the aspect ratio");
    println!();

    println!("  {b}{BLUE}[animation]{r}  {s}intro, play and pin behaviour{r}", b=b, BLUE=BLUE, r=r, s=s);
    field("effect", "none", "none|shimmer|scanline|boot");
    println!("  {s}              aliases: off/frames/keyframes/wave/rainbow = none, scan/crt = scanline,{r}", s=s, r=r);
    println!("  {s}                      bootreveal/reveal = boot, else shimmer{r}", s=s, r=r);
    field("intro_ms", "700", "normal-mode intro length; 0 disables it; capped at 24 frames");
    field("play_seconds", "3", "how long --animate/--blackhole play before the shell returns");
    field("fps", "30", "animation frame rate (1–120)");
    field("gradient", "animfetch ramp", "vertical #RGB/#RRGGBB stops; [] = scheme palette wave");
    field("gradient_speed", "0", "gradient rows per frame when scrolling; 0 keeps it still");
    field("art_color", "own", "own = keep frame ANSI; theme = force the gradient");
    println!("  {s}              authored ANSI in a frame keeps its own colours{r}", s=s, r=r);
    println!("  {s}              the intro is skipped for custom/image logos, piped output and --no-animate{r}", s=s, r=r);
    println!("  {s}              animated logos play their frames while intro_ms > 0{r}", s=s, r=r);
    println!("  {s}              bundled frames: arcfetch --list-animations; play: --animation <name>{r}", s=s, r=r);
    println!("  {s}              custom frames: ~/.config/arcfetch/animations/<name>/01.txt, 02.txt, …{r}", s=s, r=r);
    println!();

    println!("  {b}{TEAL}precedence{r}", b=b, TEAL=TEAL, r=r);
    println!("  {s}[colors] keys apply in file order; a palette overrides accent/value/gradient after them;{r}", s=s, r=r);
    println!("  {s}--scheme replaces the palette; --accent/ARCFETCH_ACCENT then overrides accent + hostname;{r}", s=s, r=r);
    println!("  {s}--color overrides the logo accent. A file accent does not set hostname.{r}", s=s, r=r);
    println!("  {s}CLI --preset replaces the config file; --full/--all run after it, then -s/--structure wins.{r}", s=s, r=r);
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
    println!("    [layout]");
    println!("    box = round");
    println!("    [logo]");
    println!("    name = arch");
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

    let sample = r##"# arcfetch config — ~/.config/arcfetch/config.toml
# all values are optional — delete lines to use defaults

# ── rows ─────────────────────────────────────────────────
# Start from a built-in layout. Every row can still be overridden below.
# classic | minimal | boxed | sections | tree | iconic | card | chips
# card and chips currently use the boxed layout.
# info_style = "classic"

# Rows in order. Leave this out to use the style's defaults.
# modules = ["os", "host", "kernel", "uptime", "packages", "shell", "de", "term", "cpu", "gpu", "memory", "disk", "colors"]

# One table per row; the same keys JSONC presets use.
# [[modules]]
# type = "os"
# key = "distro"
# keyIcon = ""
# format = "{key} {value}"
# keyColor = "blue"
# outputColor = "text"

# Package rows: hide managers you do not want, and compact the rest.
# [[modules]]
# type = "packages"
# disabled = ["appimage", "nix"]
# combined = true

# A line of your own:
# [[custom]]
# format = "{#accent}── {user-name} ──{#}"

# Or a labelled row:
# [[custom]]
# label = "Owner"
# value = "POKKITA"

# A command row (for example a public IP):
# [[modules]]
# type = "command"
# key = "Public IP"
# text = "curl -s --max-time 1 ifconfig.me"

# Change a built-in row. value and command are alternatives.
# [overrides.os]
# label = "Distro"
# [overrides.uptime]
# command = "uptime -p"
# [overrides.kernel]
# value = "my-kernel 1.0"

# seconds a command row may take (0.1-10)
# command_timeout = 1.0

# ── palette ──────────────────────────────────────────────
# Follow the colours of your wallpaper/desktop theme. With
# color_source = "palette", the first usable file is read and its
# accent/value/gradient names replace the matching colours below. Anything it
# does not define keeps its value, so this can only add colour.
#
# The tool only reads the file; it does not generate a palette. Matugen,
# pywal and wallust all write one of these JSON files.
#
# color_source = "palette"
# palette_files = [
#   "~/.local/state/quickshell/user/generated/colors.json",  # matugen
#   "~/.cache/wal/colors.json",                              # pywal
#   "~/.cache/wallust/colors.json",                          # wallust
# ]
#
# Names are looked up at the top level and inside nested objects, so all of
# these work:
#   matugen:  {"primary": {"color": "#..."}}
#   pywal:    {"colors": {"color4": "#..."}}
#   dotted:   "special.background"
#
# Defaults are Material You role names (matugen). If a name is missing, the
# pywal/wallust equivalent is tried automatically (color4, color7, and the
# color4..color6 gradient), so those files work without changing these names.
# You can still set them explicitly:
#   palette_accent   = "color4"
#   palette_value    = "color7"
#   palette_gradient = ["color4", "color5", "color6"]
#
# palette_accent   = "primary"       # hostname, rule, logo and bar
# palette_value    = "on_surface"    # values
# palette_gradient = ["primary", "secondary", "tertiary"]  # logo + animated art
#
# The same keys also work under [palette] (files/accent/value/gradient) and
# [colors] (source/palette_files/...). Palette colours are applied after the
# [colors] block, so they win there; --scheme, --accent and --color still win.
# Try it for one run without editing this file: arcfetch --palette

# Blank columns between the logo and the info (same as [layout] gap).
# gap = 2

[colors]
# keys apply in file order — keep scheme first if you use it
# palette preset (mocha is the default; uncomment to switch):
#   auto | mocha | latte | frappe | macchiato | nord | dracula | gruvbox |
#   tokyo-night | solarized-dark | rose-pine | onedark | kanagawa |
#   everforest | ayu   (auto = light/dark from the terminal)
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
# defaults: c1..c7 + blue + text
logo1 = blue
logo2 = sapphire
logo3 = sky
logo4 = teal
logo5 = green
logo6 = yellow
logo7 = peach
logo8 = blue
logo9 = text

[show]
# header: both | user/username | host/hostname | none/false
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

# system fields (hidden by default)
init        = false   # PID 1 init name (systemd/OpenRC/runit/s6/…)
cpu_temp    = false   # CPU package temperature (hwmon or ACPI)
processes   = false   # process count from /proc
container   = false   # Docker / Podman / Kubernetes / LXC / none
session     = false   # XDG session type (x11/wayland/tty)

# fastfetch-inspired fields (all hidden by default)
swap        = false   # swap usage
sound       = false   # audio server (PipeWire/PulseAudio/ALSA)
gpu_driver  = false   # GPU driver name + version
bios        = false   # BIOS version + date
board       = false   # motherboard vendor + model
disk_type   = false   # SSD / NVMe / HDD
pkg_updates = false   # pacman sync repos found (not a full update count)
theme       = false   # GTK theme name
icons       = false   # icon theme name
term_font   = false   # terminal font name
de_wm_ver   = false   # desktop environment version
init_ver    = false   # init version (systemd uses distro VERSION_ID)
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
cpu_usage     = false  # /proc/stat sample (at least 60 ms)
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

[display]
# space between a row's key and its value
# separator = " "
# set false to print values only (used by minimal/iconic)
# labels = true
# key column width; 0 removes the gap
# key_width = 9
# key_padding_left = 0
# bar_width = 14

[layout]
# box around the normal one-shot output:
#   none | round | square | double | heavy | dashed | ascii
box       = none
# border colour (any colour spec above); none = uncoloured; default = accent
box_color = accent
# left margin in columns
indent    = 2
# spaces between the logo and the info column
gap       = 2

[logo]
# any built-in name from `arcfetch --list-logos`
# logo name: arch (default) | mini | ascii | tux | nix | gentoo | auto |
#   atom | dna | arcreactor | aurora | ubuntu | debian | fedora | opensuse | manjaro |
#   endeavouros | cachyos | kali | alpine | void | pop | linuxmint | garuda |
#   artix | rocky | almalinux | centos | steamos | raspbian | wsl | neon |
#   deepin | mx | blackarch | redhat | zorin | elementary | nobara | bazzite |
#   bedrock | guix | macos | windows | freebsd | bsd | android | termux | rust
# append _small for compact variants where available
# name = arch

# custom ASCII file path (overrides name if set)
# file = ~/.config/arcfetch/logo.txt

# image logos: left/right put the info beside the image, top puts it below
# position = left
# image cell size; leave height unset to keep the image aspect (2:1 cells)
# width = 31
# height = 19

# uniform logo colour; omit to keep each logo's built-in colours
# color = blue

[animation]
# glyph effect: none | shimmer | scanline | boot
# aliases: off/frames/keyframes/wave/rainbow = none, scan/crt = scanline,
#          bootreveal/reveal = boot, else shimmer
effect   = none
# normal-mode intro length in milliseconds (0 disables; capped at 24 frames)
intro_ms = 700
# how long --animate / --blackhole play before the shell comes back (seconds)
play_seconds = 3
# animation frame rate (1-120; default 30)
fps      = 30

# vertical gradient for plain art; authored ANSI keeps its own colours.
# any #RGB/#RRGGBB; [] falls back to the scheme's logo palette wave.
gradient = ["#f5c95c", "#f17f5b", "#f45c82", "#b07cf4", "#74a4ff"]
# gradient rows per frame; 0 keeps the ramp still
gradient_speed = 0.0
# "own" keeps a frame's ANSI colours; "theme" forces the gradient on it
art_color = "own"

# bundled frame animations: arcfetch --list-animations
# play one: arcfetch --animation saturn
# pin one:  arcfetch --pin --animation saturn
# custom frames: ~/.config/arcfetch/animations/<name>/01.txt, 02.txt, …
"##;
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
    let flag = |syntax: &str, desc: &str| {
        println!("    {b}{g}{syntax:<27}{r} {s}{desc}{r}",
            b = b, g = g, syntax = syntax, r = r, s = s, desc = desc);
    };

    println!();
    let ver = env!("CARGO_PKG_VERSION");
    println!("  {b}{MAUVE}arcfetch{r}  v{ver}  blazing-fast Arch sysinfo", b=b, MAUVE=MAUVE, ver=ver, r=r);
    println!();
    println!("  {b}{BLUE}usage{r}   arcfetch [OPTIONS]", b=b, BLUE=BLUE, r=r);
    println!();
    println!("  {b}{SAPPHIRE}general{r}", b=b, SAPPHIRE=SAPPHIRE, r=r);
    flag("-h, --help", "this screen");
    flag("-V, --version", "version + E=mc² joke");
    flag("--config [file]", "config reference + create sample; or load TOML/JSONC/fastfetch JSONC");
    flag("--all", "enable every data field (header, uptime_long, swatches, color_bar unchanged)");
    flag("-s, --structure <list>", "only listed data fields (comma/space); keeps header/swatches/color_bar");
    flag("--preset <name|file>", "load a shipped JSONC preset or a .jsonc/.json/fastfetch file");
    flag("--list-presets", "list JSONC presets shipped with arcfetch (alias: --presets)");
    flag("--full", "show all data fields (same as --all)");
    flag("--json", "flat JSON object of enabled fields (uptime always included); no colour");
    flag("--watch", "clear and re-run every second; Ctrl+C stops");
    flag("--no-color", "plain text; also NO_COLOR, CLICOLOR=0, or non-TTY unless FORCE_COLOR");
    println!();
    println!("  {b}{SAPPHIRE}appearance{r}", b=b, SAPPHIRE=SAPPHIRE, r=r);
    flag("--scheme <name>", "palette preset (default mocha); replaces the whole palette");
    println!("    {s}   auto|mocha|latte|frappe|macchiato|nord|dracula|gruvbox|tokyo-night{r}", s=s, r=r);
    println!("    {s}   solarized-dark|rose-pine|onedark|kanagawa|everforest|ayu{r}", s=s, r=r);
    println!("    {s}   auto picks latte on a light terminal, mocha on dark/unknown{r}", s=s, r=r);
    flag("--accent <color>", "colour spec (name/hex/rgb/colorN); accent + hostname; wins over scheme/config");
    flag("--palette", "read the first usable configured palette file for this run (wallpaper colours)");
    flag("--color <spec|random>", "logo/accent colour only; spec or random palette colour each run");
    flag("--box [style]", "frame output; bare --box = round, else round|square|double|heavy|dashed|ascii|none");
    flag("--logo <name|?|list>", "built-in logo, auto, or list; default arch");
    flag("--logo-file <path>", "ASCII text or kitty png/jpg/gif; overrides logo name; ~ expanded");
    flag("--list-logos, --logos", "every built-in name with a preview");
    flag("--list-schemes, --schemes", "every palette with role swatches and exact hex");
    flag("--list-animations", "built-in and custom frame sets (alias: --animations)");
    println!();
    println!("  {b}{SAPPHIRE}display & animation{r}", b=b, SAPPHIRE=SAPPHIRE, r=r);
    flag("--animate [effect]", "play a live logo, then return to the shell; shimmer (default)|scanline|boot|frames");
    flag("--pin", "pin the animation above the shell until --stop");
    flag("--animation <name|path>", "play bundled or ~/.config/arcfetch/animations frames; implies --animate");
    flag("--fps <n>", "animation frame rate override (1-120; default 30)");
    flag("--blackhole", "M87 accretion disk; plays for play_seconds, then exits");
    flag("--quantum", "wave-function collapse; fixed 8 frames; --t ignored");
    flag("--cosmic", "full-screen starfield + moon + shooting stars; ~6 s blocking");
    flag("--mandelbrot [iter]", "static fractal logo; default 64 iterations");
    flag("-t, --t <secs>", "play/pin duration; 0 = until Ctrl+C (play) or --stop (pin)");
    flag("--no-animate, --no-intro", "skip the normal-mode intro");
    flag("--stop, --unpin", "remove the pinned animation and reset the terminal");
    println!("    {s}--animate/--blackhole play in place; add --pin to keep them above the shell{r}", s=s, r=r);
    println!("    {s}play duration: [animation] play_seconds (default 3 s); --t overrides it{r}", s=s, r=r);
    println!("    {s}pinned runs last until --stop or --t N; one display mode at a time{r}", s=s, r=r);
    println!("    {s}--blackhole and --animate need a TTY; piped output exits with an error{r}", s=s, r=r);
    println!();
    println!("  {b}{TEAL}logos{r}", b=b, TEAL=TEAL, r=r);

    println!("    {b}{g}arch{r}         block {s}\u{259f}\u{2588}\u{2588}\u{2588}\u{2599}{r} (default)", b=b, g=g, r=r, s=s);
    println!("    {b}{g}mini{r}         compact 7-line ASCII Arch", b=b, g=g, r=r);
    println!("    {b}{g}ascii{r}        dotty Arch ASCII", b=b, g=g, r=r);
    println!("    {b}{g}tux{r}          Linux penguin", b=b, g=g, r=r);
    println!("    {b}{g}nix{r}          NixOS hexagonal snowflake", b=b, g=g, r=r);
    println!("    {b}{g}gentoo{r}       Gentoo G (fastfetch style)", b=b, g=g, r=r);
    println!("    {b}{g}auto{r}         detect from {s}/etc/os-release (unknown falls back to arch){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}atom{r}, {g}dna{r}, {g}arcreactor{r}, {g}aurora{r}  animated logos", b=b, g=g, r=r);
    println!("    {b}{g}custom{r}  {s}~/.config/arcfetch/logo.txt{r}", b=b, g=g, r=r, s=s);
    println!("    {s}{n} built-in names incl. distros, _small variants and mascots — try {g}--logo ubuntu{r}", s=s, n=logos::all_names().len(), g=g, r=r);
    println!();
    println!("  {b}{TEAL}presets{r}", b=b, TEAL=TEAL, r=r);
    println!("    {g}--list-presets{r}      {s}{} shipped JSONC layouts{r}", config::preset_names().len(), g=g, r=r, s=s);
    println!("    {g}--preset 2{r}          {s}load one by number/name (or pass a .jsonc path){r}", g=g, r=r, s=s);
    println!("    {s}presets live in presets/examples; copy one into ~/.config/arcfetch to edit it{r}", s=s, r=r);
    println!();
    println!("  {b}{PEACH}config{r}  {s}{cfg_path}{r}", b=b, PEACH=PEACH, r=r, s=s, cfg_path=cfg_path);
    println!("    run {b}{g}arcfetch --config{r} for the full reference and a sample file", b=b, g=g, r=r);
    println!("    {s}[colors] | [show] | [logo] | [animation] | [display] | [layout] — all keys optional{r}", s=s, r=r);
    println!();
    println!("  {b}{SKY}environment{r}", b=b, SKY=SKY, r=r);
    println!("    {g}ARCFETCH_ACCENT{r}   {s}same as --accent (logo + hostname){r}", g=g, r=r, s=s);
    println!("    {g}NO_COLOR{r}          {s}disable colour; CLICOLOR=0 does the same{r}", g=g, r=r, s=s);
    println!("    {g}FORCE_COLOR{r}       {s}keep colour when stdout is piped{r}", g=g, r=r, s=s);
    println!("    {g}XDG_CONFIG_HOME{r}   {s}base for arcfetch/config.toml (default ~/.config){r}", g=g, r=r, s=s);
    println!();
    println!("  {b}{OVERLAY0}shell startup (e.g. ~/.zshrc){r}", b=b, OVERLAY0=OVERLAY0, r=r);
    println!("    arcfetch                       {s}# minimal default (fast){r}", s=s, r=r);
    println!("    arcfetch --full               {s}# every preset field{r}", s=s, r=r);
    println!("    arcfetch --scheme auto         {s}# follow the terminal light/dark background{r}", s=s, r=r);
    println!("    arcfetch --mandelbrot          {s}# fractal as logo{r}", s=s, r=r);
    println!("    arcfetch --blackhole           {s}# play the M87 disk, then return to the shell{r}", s=s, r=r);
    println!("    arcfetch --pin --blackhole     {s}# keep it pinned above the shell (stop: --unpin){r}", s=s, r=r);
    println!("    arcfetch --logo nix --cosmic   {s}# cosmic + nix logo{r}", s=s, r=r);
    println!("    arcfetch --animation pulse     {s}# your own frame animation{r}", s=s, r=r);
    println!();
}

fn logo_line_color<'a>(logo_name: &str, i: usize, cfg: &'a Config, color_override: bool) -> &'a str {
    if color_override {
        cfg.colors.accent.as_str()
    } else if let Some(ref color) = cfg.logo.color {
        color
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

/// `--list-schemes` — preview every verified palette as role swatches.
fn print_schemes(no_color: bool) {
    use std::fmt::Write as _;
    let color = !colour_disabled(no_color);
    let depth = if color { config::ColorDepth::detect() } else { config::ColorDepth::Ansi16 };
    let cols = term_size().0.max(40);
    let per_row = (cols.saturating_sub(6) / 21).max(1);
    let paint = |line: String| {
        let line = if color {
            config::downgrade_str(&line, depth)
        } else {
            strip_ansi(&line)
        };
        println!("{line}");
    };

    println!();
    paint(format!(
        "  {BOLD}{BLUE}arcfetch schemes{R}  {OVL}use: arcfetch --scheme <name>{R}",
        BOLD = BOLD, BLUE = BLUE, OVL = OVERLAY0, R = RESET
    ));
    paint(format!(
        "  {OVL}verified upstream palettes · role, swatch, exact hex{R}",
        OVL = OVERLAY0, R = RESET
    ));
    println!();

    for s in scheme::SCHEMES.iter().copied() {
        paint(format!(
            "  {BOLD}{GREEN}{}{R}  {OVL}· {}{R}",
            s.palette.name,
            s.palette.variant,
            BOLD = BOLD, GREEN = GREEN, OVL = OVERLAY0, R = RESET
        ));
        for chunk in s.roles.all().chunks(per_row) {
            let mut line = String::from("    ");
            for (label, role) in chunk {
                let rgb = s.rgb(role);
                let _ = write!(line, " {label:<7} ");
                if color {
                    let _ = write!(line, "{}", scheme::swatch(rgb));
                } else {
                    line.push_str("██");
                }
                let _ = write!(line, " {}  ", scheme::hex(rgb));
            }
            paint(line);
        }
        println!();
    }

    paint(format!(
        "  {OVL}auto{R} picks latte on a light terminal, mocha on dark or unknown backgrounds.",
        OVL = OVERLAY0, R = RESET
    ));
    println!();
}

fn print_logos() {
    let names = logos::all_names();

    let is_fun = |n: &str| matches!(
        n,
        "ascii" | "tux" | "nix" | "gentoo" | "mini" | "dna" | "atom"
            | "aurora" | "wave" | "emc2" | "pi" | "batman" | "batman_small" | "ironman"
            | "ironman_small" | "arcreactor"
    );
    let is_animated = logos::is_animated;

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
            let preview = if matches!(*name, "aurora") {
                art.last().map(|line| line.trim()).unwrap_or("")
            } else {
                art.iter()
                    .filter(|l| !l.trim().is_empty())
                    .max_by_key(|l| visible_chars(l))
                    .map(|l| l.trim())
                    .unwrap_or("")
            };
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

fn print_animations() {
    println!();
    println!("  {BOLD}{BLUE}arcfetch animations{R}", BOLD = BOLD, BLUE = BLUE, R = RESET);
    println!();
    println!("  {BOLD}{MAUVE}procedural logos{R}  {GREEN}atom{R}, {GREEN}dna{R}, {GREEN}arcreactor{R}, {GREEN}aurora{R}",
        BOLD = BOLD, MAUVE = MAUVE, GREEN = GREEN, R = RESET);
    println!("  {OVERLAY0}play: arcfetch --animate --logo aurora{R}", OVERLAY0 = OVERLAY0, R = RESET);
    println!();

    let bundled = anim::Animation::bundled_list();
    if !bundled.is_empty() {
        println!("  {BOLD}{MAUVE}bundled frame sets{R}", BOLD = BOLD, MAUVE = MAUVE, R = RESET);
        for chunk in bundled.chunks(3) {
            let mut line = String::from("   ");
            for (name, frames) in chunk {
                line.push_str(&format!(" {GREEN}{:<16}{R} {OVERLAY0}{frames:>3} frames{R}",
                    name, OVERLAY0 = OVERLAY0, GREEN = GREEN, R = RESET, frames = frames));
            }
            println!("{line}");
        }
        println!();
    }

    let custom = anim::Animation::list();
    if custom.is_empty() {
        println!("  {OVERLAY0}no custom animations yet{R}", OVERLAY0 = OVERLAY0, R = RESET);
    } else {
        println!("  {BOLD}{MAUVE}custom{R}", BOLD = BOLD, MAUVE = MAUVE, R = RESET);
        for (name, frames) in custom {
            println!("    {GREEN}{:<16}{R} {OVERLAY0}{frames} frame(s){R}",
                name, OVERLAY0 = OVERLAY0, GREEN = GREEN, R = RESET);
        }
    }
    println!();
    println!("  {OVERLAY0}play: arcfetch --animation saturn          pin: arcfetch --pin --animation saturn{R}",
        OVERLAY0 = OVERLAY0, R = RESET);
    println!("  {OVERLAY0}put your own frames in ~/.config/arcfetch/animations/<name>/01.txt, 02.txt, …{R}",
        OVERLAY0 = OVERLAY0, R = RESET);
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

/// Art source for the pinned/animate paths: a static custom logo or a frame
/// directory loaded by [`anim::Animation`].
#[derive(Clone, Copy)]
struct CustomArt<'a> {
    lines: &'a Option<Vec<String>>,
    anim: Option<&'a anim::Animation>,
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

/// One vertical gradient stop for `row`, or `None` when the gradient is off.
/// This is animfetch's model: plain art gets the ramp, authored ANSI keeps its
/// own colours.
fn gradient_for_row(
    anim: &config::Animation,
    row: usize,
    height: usize,
    tick: usize,
) -> Option<&str> {
    if anim.gradient.is_empty() {
        return None;
    }
    let span = height.max(1);
    let offset = if anim.gradient_scroll && anim.gradient_speed > 0.0 {
        (tick as f32 * anim.gradient_speed) as usize
    } else {
        0
    };
    let t = ((row + offset) % span) as f32 / span as f32;
    let last = anim.gradient.len() - 1;
    let idx = ((t * last as f32).round() as usize).min(last);
    Some(&anim.gradient[idx])
}

/// Build one art frame, with the gradient (or palette wave) when `color` is
/// set. Lines are padded to a common width so the info column never jitters.
fn panel_text(
    lines: &[String],
    tick: usize,
    effect: logos::AnimEffect,
    palette: &[String],
    anim: &config::Animation,
    color: bool,
) -> Vec<String> {
    let widths: Vec<usize> = lines.iter().map(|l| visible_chars(l)).collect();
    let w = widths.iter().max().copied().unwrap_or(36);
    let last = palette.len().saturating_sub(1);

    lines.iter().enumerate().map(|(i, raw)| {
        let stripped;
        // Frames may carry their own SGR colours (animfetch's `art_color =
        // "own"`). Pass those through; `theme` strips them so the gradient wins.
        let keep_ansi = color && (anim.art_color || !raw.contains('\x1b'));
        let raw = if keep_ansi {
            raw.as_str()
        } else {
            stripped = strip_ansi(raw);
            stripped.as_str()
        };
        let authored = keep_ansi && raw.contains('\x1b');
        let line = if authored {
            raw.to_string()
        } else {
            logos::animate_line(raw, i, tick, effect)
        };
        let mut s = String::with_capacity(line.len() * 4 + 16);
        if color {
            if authored {
                s.push_str(&line);
                s.push_str(RESET);
            } else if let Some(stop) = gradient_for_row(anim, i, lines.len(), tick) {
                // One escape per row: cheap, and the vertical ramp reads the
                // same way animfetch's does.
                s.push_str(stop);
                s.push_str(&line);
                s.push_str(RESET);
            } else {
                // Fallback: one escape per run of the same palette slot, and
                // never colour a space.
                let mut prev = None;
                for (col, ch) in line.chars().enumerate() {
                    if ch == ' ' {
                        s.push(' ');
                        continue;
                    }
                    let phase = logos::color_wave_phase(i, col, tick);
                    let idx = ((phase * last as f32).round() as usize).min(last);
                    if prev != Some(idx) {
                        s.push_str(&palette[idx]);
                        prev = Some(idx);
                    }
                    s.push(ch);
                }
                s.push_str(RESET);
            }
        } else {
            s.push_str(&line);
        }
        s.push_str(&" ".repeat(w.saturating_sub(widths[i])));
        s
    }).collect()
}

/// Build one logo frame with the gradient / palette wave.
fn panel_logo(
    logo_name: &str,
    tick: usize,
    effect: logos::AnimEffect,
    palette: &[String],
    anim: &config::Animation,
) -> Vec<String> {
    let base = logos::frame(logo_name, tick, 4);
    panel_text(&base, tick, effect, palette, anim, true)
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

/// The terminal's current line discipline, or `None` when fd 1 is not a tty.
fn tty_termios() -> Option<libc::termios> {
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    (unsafe { libc::tcgetattr(1, &mut t) } == 0).then_some(t)
}

/// Line discipline field: Linux/Android expose `c_line`, BSD/macOS do not.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn termios_line(t: &libc::termios) -> libc::cc_t { t.c_line }
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn termios_line(_: &libc::termios) -> libc::cc_t { 0 }

fn termios_eq(a: &libc::termios, b: &libc::termios) -> bool {
    a.c_iflag == b.c_iflag
        && a.c_oflag == b.c_oflag
        && a.c_cflag == b.c_cflag
        && a.c_lflag == b.c_lflag
        && termios_line(a) == termios_line(b)
        && a.c_cc == b.c_cc
}

/// Decides when the shell's line editor owns the terminal, by remembering the
/// line discipline the shell sets up at its prompt. A foreground command makes
/// the shell hand the terminal over and changes that mode, so a change means
/// the pin must give the terminal back. This also catches programs the shell
/// runs in its own process group, which `tcgetpgrp()` cannot see (fzf-tab runs
/// fzf that way). A background child does not touch the mode, so prompt
/// helpers and completions no longer make the pin pause and repaint.
struct TerminalWatch {
    shell_pgid: i32,
    prompt_mode: Option<libc::termios>,
    settle: Option<Instant>,
    was_shell: bool,
}

impl TerminalWatch {
    fn new(shell_pgid: i32) -> Self {
        Self { shell_pgid, prompt_mode: None, settle: None, was_shell: false }
    }

    /// `None` once the tty is gone; otherwise `true` while the pin may draw.
    fn holds(&mut self) -> Option<bool> {
        let foreground = unsafe { libc::tcgetpgrp(1) };
        if foreground < 0 {
            return None;
        }
        let shell_fg = foreground == self.shell_pgid;
        let now = Instant::now();
        if shell_fg && !self.was_shell {
            // The shell just took the terminal back. Give the line editor a
            // moment to install its own mode before we remember it.
            self.settle = Some(now + Duration::from_millis(60));
        }
        if !shell_fg {
            self.settle = None;
        } else if self.settle.is_some_and(|t| now >= t) {
            self.prompt_mode = tty_termios();
            self.settle = None;
        }
        self.was_shell = shell_fg;

        if !shell_fg {
            return Some(false);
        }
        let taken = match (self.prompt_mode, tty_termios()) {
            (Some(prompt), Some(now)) => !termios_eq(&prompt, &now),
            // Mode not learned yet: stay out of the way until it is.
            _ => true,
        };
        Some(!taken)
    }
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

fn stop_background() -> bool {
    let Some(path) = bg_pid_path() else { return false };
    let mut found = false;
    let mut stopped = false;

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
                        stopped = true;
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
                stopped = true;
            }
        }
    }
    let _ = std::fs::remove_file(&path);
    if found {
        reset_pin();
    }
    stopped
}

/// Clip a line to `max` visible cells, keeping ANSI escapes intact.
fn truncate_visible(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max * 2 + 16));
    let mut vis = 0usize;
    let mut esc = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            esc = true;
            out.push(ch);
            continue;
        }
        if esc {
            out.push(ch);
            if ch.is_ascii_alphabetic() {
                esc = false;
            }
            continue;
        }
        if vis >= max {
            break;
        }
        out.push(ch);
        vis += 1;
    }
    out.push_str(RESET);
    out
}

/// Height of the art itself, before the info column is considered.
fn art_height(mode: PanelMode, first_lines: Option<&[String]>, rows: usize) -> usize {
    match mode {
        PanelMode::Blackhole => (rows * 55 / 100).max(1),
        PanelMode::Animate(_) => first_lines.map(|l| l.len()).unwrap_or(1).max(1),
    }
}

/// Pane rows: the taller of art and info, capped so the shell keeps four rows.
fn pin_height(art_h: usize, info_len: usize, rows: usize) -> usize {
    let usable = rows.saturating_sub(5).max(1);
    art_h.max(info_len).min(usable).max(1)
}

/// Geometry for one pinned pane frame.
struct PaneGeom {
    cols: usize,
    art_cols: usize,
    art_h: usize,
    pane_h: usize,
}

/// Build one frame for the pinned pane: art on the left, info on the right,
/// with the same two-space indent and gap as the normal one-shot render.
fn pane_frame(
    mode: PanelMode,
    logo_name: &str,
    art: CustomArt,
    cfg: &Config,
    info: &[String],
    geom: PaneGeom,
    frame: u64,
) -> Vec<String> {
    let info_w = info.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
    let art_max = geom
        .cols
        .saturating_sub(info_w + cfg.layout.gap + cfg.layout.indent)
        .max(1);

    let art: Vec<String> = match mode {
        PanelMode::Blackhole => panel_blackhole(geom.art_cols, geom.art_h.min(geom.pane_h), frame),
        PanelMode::Animate(effect) => {
            let effect = parse_effect(effect);
            if let Some(a) = art.anim {
                let frames = a.fitted(art_max, geom.art_h.min(geom.pane_h));
                let lines = &frames[frame as usize % frames.len()];
                panel_text(lines, frame as usize, effect, &cfg.colors.logo, &cfg.animation, true)
            } else if let Some(lines) = art.lines {
                panel_text(lines, frame as usize, effect, &cfg.colors.logo, &cfg.animation, true)
            } else {
                panel_logo(logo_name, frame as usize, effect, &cfg.colors.logo, &cfg.animation)
            }
        }
    };

    let art_w = art.iter().map(|l| visible_chars(l)).max().unwrap_or(geom.art_cols);
    let indent = " ".repeat(cfg.layout.indent);
    let gap = " ".repeat(cfg.layout.gap);
    let mut block = Vec::with_capacity(geom.pane_h);
    for i in 0..geom.pane_h {
        let a = art.get(i).map(String::as_str).unwrap_or("");
        let pad = art_w.saturating_sub(visible_chars(a));
        let inf = info.get(i).map(String::as_str).unwrap_or("");
        let mut line = String::with_capacity(indent.len() + art_w + gap.len() + inf.len());
        line.push_str(&indent);
        line.push_str(a);
        line.push_str(RESET);
        if pad > 0 {
            line.push_str(&" ".repeat(pad));
        }
        line.push_str(&gap);
        line.push_str(inf);
        block.push(truncate_visible(&line, geom.cols));
    }
    block
}

/// Put the fetch in a pinned pane above the shell and return immediately.
///
/// The terminal's scroll region is set below the pane, so the shell's prompt
/// and all command output scroll under it and never push the fetch away. The
/// animation runs in a child in its own process group (but the shell's
/// session), so it can tell when a foreground job owns the terminal. While a
/// command runs the child stops painting and drops origin mode so TUIs (fzf,
/// vim, less, …) can use absolute cursor addressing; the region and origin
/// mode are restored when the shell prompt comes back. Programs the shell runs
/// in its own process group (fzf-tab) are caught by the change they make to the
/// terminal's line discipline, so job control and widgets both work.
fn run_background(
    mode: PanelMode,
    logo_name: &str,
    art: CustomArt,
    cfg: &Config,
    info: &[String],
    duration: Option<u64>,
    no_color: bool,
) {
    let (cols, rows) = term_size();
    if cols < 20 || rows < 8 {
        return;
    }

    let info_len = info.len();
    let info_w = info.iter().map(|l| visible_chars(l)).max().unwrap_or(0);

    // A frame at tick 0 gives the logo its natural height.
    let first_lines = match mode {
        PanelMode::Animate(effect) => Some(if let Some(a) = art.anim {
            a.frame(0, 1).to_vec()
        } else if let Some(lines) = art.lines {
            lines.clone()
        } else {
            panel_logo(logo_name, 0, parse_effect(effect), &cfg.colors.logo, &cfg.animation)
        }),
        PanelMode::Blackhole => None,
    };

    let art_h = art_height(mode, first_lines.as_deref(), rows);
    let pane_h = pin_height(art_h, info_len, rows);
    let art_cols = match mode {
        PanelMode::Blackhole => cols.saturating_sub(info_w + 6).clamp(20, 46),
        PanelMode::Animate(_) => cols,
    };
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
    let mut watch = TerminalWatch::new(shell_pgid);

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
    // Remember the pid path now: once the session dies the tty may be gone,
    // but the file still has to be cleaned up.
    let pid_path = bg_pid_path();

    let start = std::time::Instant::now();
    let limit = match duration {
        None | Some(0) => None,
        Some(n) => Some(Duration::from_secs(n)),
    };

    let mut cur_cols = cols;
    let mut cur_rows = rows;
    let mut cur_art_h = art_h;
    let mut cur_pane_h = pane_h;
    let mut cur_art_cols = art_cols;
    let mut cur_scroll_top = scroll_top;
    let mut painted_h = 0usize;
    let mut held = true;
    let mut invalidate = true;
    let mut frame: u64 = 0;
    let mut last_block: Vec<String> = Vec::new();
    let frame_ms = match mode {
        PanelMode::Blackhole => 50,
        PanelMode::Animate(_) => cfg.animation.frame_ms(),
    };

    loop {
        let frame_deadline = Instant::now() + Duration::from_millis(frame_ms);
        if !RUNNING.load(Ordering::Relaxed) {
            break;
        }
        if let Some(lim) = limit {
            if start.elapsed() >= lim {
                break;
            }
        }

        // The shell that owns the pin went away: take the region with us.
        if unsafe { libc::kill(shell_pid, 0) } != 0 {
            break;
        }

        let Some(holds) = watch.holds() else {
            break;
        };

        // A command just took the terminal. Keep the pane's rows reserved,
        // but drop origin mode so a TUI (fzf, vim, less, …) can use absolute
        // cursor addressing. It is restored when the shell comes back.
        if !holds && held {
            write_tty(b"\x1b7\x1b[?6l\x1b8");
        }

        if holds {
            // A resize drops the scroll region. Put it back before painting.
            let (w, h) = term_size();
            if (w, h) != (cur_cols, cur_rows) {
                cur_cols = w;
                cur_rows = h;
                cur_art_cols = match mode {
                    PanelMode::Blackhole => w.saturating_sub(info_w + 6).clamp(20, 46),
                    PanelMode::Animate(_) => w,
                };
                // Keep the pane height, and with it the shell's prompt row,
                // stable across ordinary resizes. Only shrink when the
                // terminal can no longer hold the pane above the shell with
                // four rows to spare.
                if cur_pane_h > h.saturating_sub(5).max(1) {
                    cur_art_h = art_height(mode, first_lines.as_deref(), h);
                    cur_pane_h = pin_height(cur_art_h, info_len, h);
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
                mode,
                logo_name,
                art,
                cfg,
                info,
                PaneGeom {
                    cols: cur_cols,
                    art_cols: cur_art_cols,
                    art_h: cur_art_h,
                    pane_h: cur_pane_h,
                },
                frame,
            );
            let block: Vec<String> = if no_color {
                block.iter().map(|l| strip_ansi(l)).collect()
            } else {
                block
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
                last_block.clear();
                invalidate = false;
            }

            let visible = block.len().min(cur_pane_h);
            // Row-level diff: static info rows and unchanged keyframes cost
            // nothing, which keeps the pinned pane responsive at the configured
            // frame rate.
            for (y, line) in block.iter().take(visible).enumerate() {
                if last_block.get(y) == Some(line) {
                    continue;
                }
                scr.push_str(&format!("\x1b[{};1H\x1b[2K", y + 1));
                scr.push_str(line);
            }
            // If the frame got shorter, erase the rows it used to occupy.
            for y in visible..clear_to {
                if last_block.get(y).is_some_and(|line| !line.is_empty()) {
                    scr.push_str(&format!("\x1b[{};1H\x1b[2K", y + 1));
                }
            }
            last_block = block;
            last_block.truncate(visible);

            scr.push_str(RESET);
            scr.push_str("\x1b[?6h");
            scr.push_str("\x1b8");
            scr.push_str("\x1b[?2026l");

            let mut out = stdout().lock();
            if out.write_all(scr.as_bytes()).is_err() {
                break;
            }
            if out.flush().is_err() {
                break;
            }
            drop(out);

            painted_h = cur_pane_h;
            frame += 1;
        }

        held = holds;
        // Sleep in small slices so a command taking the terminal is noticed
        // before it starts drawing, not up to a whole frame later.
        while Instant::now() < frame_deadline {
            let left = frame_deadline.saturating_duration_since(Instant::now());
            thread::sleep(left.min(Duration::from_millis(5)));
            if !RUNNING.load(Ordering::Relaxed) {
                break;
            }
            match watch.holds() {
                Some(now_holds) if now_holds != holds => break,
                None => break,
                _ => {}
            }
        }
    }

    // Leave the shell a normal terminal again. A timer expiry leaves the last
    // frame as a static picture; an explicit --stop takes it away, even when
    // the stop command itself currently owns the terminal.
    if !RUNNING.load(Ordering::Relaxed) {
        clear_pane(cur_pane_h);
    } else {
        reset_pin();
    }

    if let Some(path) = pid_path {
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
    if let Some(ip) = net::public_ip() { crate::cache::set("public_ip", &ip); }
    if let Some(w) = net::weather("") { crate::cache::set("weather", &w); }
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
    if args.list_schemes { print_schemes(args.no_color); return; }
    if args.list_presets {
        for name in config::preset_names() { println!("{}", name); }
        return;
    }
    if args.list_animations { print_animations(); return; }
    if args.stop {
        let stopped = stop_background();
        // Also repair a region left behind by a crash that took its pid file.
        if tty_name().is_some() {
            reset_pin();
        }
        if !stopped {
            eprintln!("arcfetch: nothing pinned to this terminal");
        }
        return;
    }

    // mutual exclusion for display modes
    let mode_count = [args.blackhole, args.cosmic, args.quantum, args.mandelbrot, args.animate.is_some()]
        .iter().filter(|&&m| m).count();
    if mode_count > 1 {
        eprintln!("arcfetch: only one display mode at a time (--blackhole, --cosmic, --quantum, --mandelbrot, --animate)");
        return;
    }

    let mut cfg = config::load(args.accent.as_deref(), args.preset.as_deref(), args.config_file.as_deref(), args.palette);
    if let Some(fps) = args.fps {
        cfg.animation.fps = fps.clamp(1.0, 120.0);
    }

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
    args.no_color = colour_disabled(args.no_color);

    // Down-sample colours to what the terminal can actually show (truecolor →
    // 256 → 16), so the tool stays legible on any terminal.
    let depth = if args.no_color {
        config::ColorDepth::True
    } else {
        config::ColorDepth::detect()
    };
    config::apply_depth(&mut cfg.colors, depth);
    config::apply_depth_animation(&mut cfg.animation, depth);

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

    // --animation <name|path>: load user frames once; it implies --animate.
    let custom_anim = match args.animation.as_deref() {
        Some(name) => match anim::Animation::load(name) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("arcfetch: {e}");
                return;
            }
        },
        None => None,
    };

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
    let need_net = cfg.show.ip || cfg.show.ssh || cfg.show.ports
        || cfg.show.public_ip || cfg.show.weather;
    let si = info::collect_all(need_net, &cfg.show, cfg.fastfetch);

    // --json: emit machine-readable output and exit
    if args.json {
        print_json(&si, &cfg);
        return;
    }

    // build info column
    let info_lines = if cfg.modules.is_some() {
        render_modules(&si, &cfg)
    } else {
        build_info(&si, &cfg)
    };

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
        if args.pin {
            let no_custom: Option<Vec<String>> = None;
            let art = CustomArt { lines: &no_custom, anim: None };
            run_background(PanelMode::Blackhole, "arch", art, &cfg, &out_lines, args.duration, args.no_color);
        } else {
            stop_background();
            run_blackhole(&out_lines, play_duration(args.duration, &cfg));
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
            raw.lines()
                .map(|line| config::expand_logo_slots(line, &cfg.colors.logo))
                .collect()
        })
    };

    // figure out which logo to use
    let image_escape: Option<(String, usize, usize)> = if args.no_color {
        None
    } else if let Some(ref path) = args.logo_file {
        let ext = path.rsplit('.').next().map(|e| e.to_lowercase());
        match ext.as_deref() {
            Some("png" | "jpg" | "jpeg" | "gif") => {
                let hold = cfg.logo.position != "top";
                let r = kitty_image(path, cfg.logo.width, cfg.logo.height, hold);
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
    } else if let Some(ref data) = cfg.logo.data {
        Some(data.lines().map(|line| {
            // raw data already contains ANSI; keep as-is
            line.to_string()
        }).collect())
    } else if let Some(ref path) = args.logo_file {
        let image = matches!(
            path.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref(),
            Some("png" | "jpg" | "jpeg" | "gif")
        );
        if image {
            // No TTY/colour: fall back to the distro logo instead of trying
            // to read the image as a text logo.
            None
        } else {
            load_file(path).or_else(|| {
                eprintln!("arcfetch: could not read logo file: {}", path);
                None
            })
        }
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
                   else if cfg.logo.name.as_deref() == Some("none") { "none".into() }
                   else { args.logo.clone() };
        if cfg.logo.name.as_deref() == Some("none") {
            "none".into()
        } else {
            base
        }
    };

    // inline logo padding
    let custom_lines = custom_lines.map(|mut lines| {
        for _ in 0..cfg.logo.padding_top { lines.insert(0, String::new()); }
        lines
    });
    let custom_lines = if custom_lines.is_none() && logo_name == "none" {
        Some(Vec::new())
    } else { custom_lines };

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
            eprintln!("arcfetch: --animate/--pin requires a terminal");
            return;
        }
        let art = CustomArt { lines: &custom_lines, anim: custom_anim.as_ref() };
        if args.pin {
            run_background(PanelMode::Animate(eff), &logo_name, art, &cfg, &out_lines, args.duration, args.no_color);
        } else {
            stop_background();
            run_animate(&out_lines, &logo_name, art, &cfg, &args, eff, play_duration(args.duration, &cfg));
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
        let animated_logo = logos::is_animated(&logo_name);
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
    if let Some((ref img, cols, rows)) = image_escape {
        match cfg.logo.position.as_str() {
            "top" => {
                out.push_str(img);
                out.push('\n');
                let indent = " ".repeat(cfg.layout.indent);
                for line in &out_lines {
                    let _ = writeln!(out, "{indent}{}", line);
                }
            }
            "right" => {
                // Info on the left, image on the right: place the image at the
                // top of the block, return to column 0, then print the rows.
                let info_w = out_lines.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
                let pad = cfg.layout.indent + info_w + cfg.layout.gap;
                out.push_str(&" ".repeat(pad));
                out.push_str(img);
                out.push('\r');
                let indent = " ".repeat(cfg.layout.indent);
                for line in &out_lines {
                    let _ = writeln!(out, "{indent}{}", line);
                }
            }
            _ => {
                // Default: image on the left, info beside it.
                let indent = " ".repeat(cfg.layout.indent);
                out.push_str(&indent);
                out.push_str(img);
                for (i, line) in out_lines.iter().enumerate() {
                    if i == 0 {
                        out.push_str(&" ".repeat(cols + cfg.layout.gap));
                    } else if i < rows {
                        out.push_str(&" ".repeat(cfg.layout.indent + cols + cfg.layout.gap));
                    } else {
                        out.push_str(&indent);
                    }
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        if cfg.logo.position != "top" && out_lines.len() < rows {
            out.push_str(&format!("\x1b[{}B", rows - out_lines.len()));
        }
        out.push('\n');
        unsafe { libc::write(1, out.as_ptr().cast::<core::ffi::c_void>(), out.len()); }
        return;
    }

    let indent = " ".repeat(cfg.layout.indent);
    let gap = " ".repeat(cfg.layout.gap);
    if cfg.logo.position == "top" {
        let logo_lines: Vec<String> = if let Some(ref lines) = custom_lines {
            lines.clone()
        } else {
            logos::from_name(&logo_name).iter().map(|s| s.to_string()).collect()
        };
        for _ in 0..cfg.logo.padding_top { out.push('\n'); }
        for (i, ll) in logo_lines.iter().enumerate() {
            if args.no_color {
                let stripped = if ll.contains('\x1b') { strip_ansi(ll) } else { ll.to_string() };
                let _ = writeln!(out, "{indent}{}", stripped);
            } else {
                let lc = if custom_lines.is_some() {
                    cfg.logo.color.as_deref().unwrap_or(cfg.colors.accent.as_str())
                } else {
                    logo_line_color(&logo_name, i, &cfg, args.color.is_some())
                };
                let _ = writeln!(out, "{indent}{}{}{}", lc, ll, RESET);
            }
        }
        out.push('\n');
        for line in &out_lines {
            let _ = writeln!(out, "{indent}{}", line);
        }
        let box_style = args.box_style.as_deref().unwrap_or(&cfg.layout.box_style);
        if !box_style.eq_ignore_ascii_case("none") {
            let border = if args.no_color { "" } else { cfg.layout.box_color.as_deref().unwrap_or(cfg.colors.accent.as_str()) };
            out = boxify(&out, box_style, border, cfg.layout.indent);
        }
        out.push('\n');
        let out = if depth == config::ColorDepth::True { out } else { config::downgrade_str(&out, depth) };
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
                let _ = writeln!(out, "{indent}{}{:pad$}{gap}{}", stripped, "", inf, pad = pad);
            } else {
                let lc = if args.color.is_some() {
                    cfg.colors.accent.as_str()
                } else {
                    cfg.logo.color.as_deref().unwrap_or(cfg.colors.accent.as_str())
                };
                let _ = writeln!(out, "{indent}{}{}{:pad$}{}{gap}{}",
                    lc, ll, "", RESET, inf, pad = pad);
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
                let _ = writeln!(out, "{indent}{}{:pad$}{gap}{}", ll, "", inf, pad = pad);
            } else {
                let lc = if logos::is_animated(&logo_name) {
                    gradient_for_row(&cfg.animation, i, logo.len(), 0)
                        .unwrap_or_else(|| logo_line_color(&logo_name, i, &cfg, args.color.is_some()))
                } else {
                    logo_line_color(&logo_name, i, &cfg, args.color.is_some())
                };
                let _ = writeln!(out, "{indent}{}{}{:pad$}{}{gap}{}",
                    lc, ll, "", RESET, inf, pad = pad);
            }
        }
    }

    let box_style = args.box_style.as_deref().unwrap_or(&cfg.layout.box_style);
    if !box_style.eq_ignore_ascii_case("none") {
        let border = if args.no_color {
            ""
        } else {
            cfg.layout.box_color.as_deref().unwrap_or(cfg.colors.accent.as_str())
        };
        out = boxify(&out, box_style, border, cfg.layout.indent);
    }

    out.push('\n');
    let out = if depth == config::ColorDepth::True {
        out
    } else {
        config::downgrade_str(&out, depth)
    };
    unsafe { libc::write(1, out.as_ptr().cast::<core::ffi::c_void>(), out.len()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_headers_and_cell_aspect_are_parsed() {
        let mut png = [0u8; 24];
        png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[16..20].copy_from_slice(&256u32.to_be_bytes());
        png[20..24].copy_from_slice(&128u32.to_be_bytes());
        assert_eq!(image_size(&png), Some((256, 128)));
        assert_eq!(image_cells(&png, None, None), (31, 8));
        assert_eq!(image_cells(&png, Some(20), None), (20, 5));

        let mut gif = [0u8; 10];
        gif[..6].copy_from_slice(b"GIF89a");
        gif[6..8].copy_from_slice(&100u16.to_le_bytes());
        gif[8..10].copy_from_slice(&50u16.to_le_bytes());
        assert_eq!(image_size(&gif), Some((100, 50)));
    }
}
