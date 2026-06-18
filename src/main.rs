// arcfetch v0.7.0 — sub-ms Arch Linux sysinfo · Catppuccin Mocha
// arcfetch [-h] [-V] [--blackhole [--t N]] [--logo NAME] [--accent COLOR]
//          [--no-color] [--config]
mod cache;
mod config;
mod cosmic;
mod info;
mod logos;
mod mandelbrot;
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
    for ch in s.chars() {
        if ch == '\x1b'  { esc = true; continue; }
        if esc { if ch == 'm' { esc = false; } continue; }
        out.push(ch);
    }
    out
}

/// Count visible characters (ignoring ANSI escape sequences).
fn visible_chars(s: &str) -> usize {
    let mut n = 0;
    let mut esc = false;
    for ch in s.chars() {
        if ch == '\x1b' { esc = true; continue; }
        if esc { if ch == 'm' { esc = false; } continue; }
        n += 1;
    }
    n
}

// ── Kitty image protocol ────────────────────────────────
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
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

fn kitty_image(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?.to_lowercase();
    let format = match ext.as_str() {
        "png" => 100u32,
        "jpg" | "jpeg" => 101,
        "gif" => 102,
        _ => return None,
    };
    let data = std::fs::read(path).ok()?;
    let b64 = base64_encode(&data);
    Some(format!("\x1b_Ga=T,f={},c=31,r=19,d=A;{}\x1b\\", format, b64))
}

/// Build the info column as a Vec<String> respecting show/hide config.
fn science_quote() -> &'static str {
    // deterministic-ish: pick by uptime seconds mod pool size
    let secs = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    let n: usize = secs.split('.').next()
        .and_then(|v| v.parse().ok()).unwrap_or(0);
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
    QUOTES[n % QUOTES.len()]
}

fn science_logo() -> &'static str {
    let secs = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    let n: usize = secs.split('.').next()
        .and_then(|v| v.parse().ok()).unwrap_or(0);
    // science logos: dna atom wave emc2 pi
    ["dna", "atom", "wave", "emc2", "pi"][n % 5]
}

fn detect_auto_logo() -> String {
    let raw = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut id = "";
    for line in raw.lines() {
        if let Some(v) = line.strip_prefix("ID=") { id = v.trim_matches('"'); }
    }
    match id {
        "arch" | "archarm"     => "arch".into(),
        "nixos"                => "nix".into(),
        "debian" | "ubuntu"
        | "linuxmint"
        | "pop" | "elementary" => "tux".into(),
        "void"                 => "tux".into(),
        "gentoo"               => "gentoo".into(),
        _                      => "arch".into(),
    }
}

fn build_info(si: &info::SysInfo, cfg: &Config) -> (Vec<String>, bool) {
    let c      = &cfg.colors;
    let sh     = &cfg.show;
    let is_sci = matches!(cfg.preset.as_deref(), Some("science"));
    let mut lines: Vec<String> = Vec::with_capacity(22);

    // ── header ────────────────────────────────────────────
    use config::Header;
    let username = env::var("USER").or_else(|_| env::var("LOGNAME")).unwrap_or_else(|_| "user".into());
    let hostname = {
        let mut buf = [0i8; 256];
        if unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len()) } == 0 {
            unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
        } else { "localhost".into() }
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

    // use li to avoid unused-assignments warning
    let _ = li;

    // swatches
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

    // science quote — always last if science preset
    if is_sci {
        lines.push(String::new());
        lines.push(format!("{OVL}{q}{R}", OVL=OVERLAY0, q=science_quote(), R=RESET));
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

// ── deterministic starfield seeded from cell position ──────
fn bh_star(c: usize, r: usize, frame: u64) -> Option<(&'static str, char)> {
    let h = (c.wrapping_mul(97) ^ r.wrapping_mul(191) ^ 0xdead_beefu64 as usize)
        .wrapping_mul(0x9e37_79b9);
    let star = (h >> 16) & 0xff;
    let twinkle = (((h >> 8) as u64).wrapping_add(frame / 2)) & 0x1f;
    if star > 230 { return None; }
    let ch = if (h & 0x100) != 0 { '\u{2219}' } else { '\u{00b7}' };
    let colors = [OVERLAY0, LAVENDER, MAUVE, TEXT, SKY, BLUE];
    let ci = (h.wrapping_add(frame as usize)) % colors.len();
    let bright = if twinkle < 4 { colors[(ci + 1) % colors.len()] } else { colors[ci] };
    Some((bright, ch))
}

fn bh_cell(col: usize, row: usize, cx: f64, cy: f64, rot: f64) -> (&'static str, char) {
    let dx   = (col as f64 - cx) * 0.52;
    let dy   = row as f64 - cy;
    let dist = (dx*dx + dy*dy).sqrt();
    let ra   = ((dy.atan2(dx) + rot) % (2.0*PI) + 2.0*PI) % (2.0*PI);

    if dist < 1.8 { return (RESET, ' '); }

    // ── photon ring — intense blueshift inner edge ──────────
    if dist < 2.8 {
        let b = (ra * 1.7).sin() * 0.5 + 0.5;
        return (BLUE, if b > 0.5 { '\u{2591}' } else { '\u{00b7}' });
    }
    // ── inner accretion — blue → white → gold ───────────────
    if dist < 4.5 {
        let b = ((ra * 1.3).sin() * 0.4 + 0.35).clamp(0.0, 1.0);
        let t = (dist - 2.8) / 1.7;
        let col = if t < 0.33 { if b > 0.5 { SAPPHIRE } else { BLUE } }
                  else if t < 0.66 { if b > 0.5 { SKY } else { SAPPHIRE } }
                  else { if b > 0.5 { TEAL } else { SKY } };
        return (col, if b > 0.45 { '\u{2591}' } else { '\u{00b7}' });
    }
    // ── main disk — hottest in, cooler out ──────────────────
    if dist < 10.4 {
        let bright = ((ra.sin() * 0.5 + 0.5)
            * (1.0 - (dist - 4.5) / 5.9 * 0.42)).clamp(0.0, 1.0);
        let ch: char = match (bright * 5.5) as u8 {
            5 => '\u{2588}', 4 => '\u{2593}', 3 => '\u{2592}',
            2 => '\u{2591}', 1 => '\u{00b7}', _ => ' ',
        };
        let t = (dist - 4.5) / 4.0;
        let col: &'static str = match (bright * 5.0) as u8 {
            5|4 if t < 0.5 => YELLOW,
            5|4           => PEACH,
            3             => PEACH,
            2             => RED,
            1 if bright > 0.1 => MAROON,
            _ => OVERLAY0,
        };
        return (col, ch);
    }
    // ── outer halo ──────────────────────────────────────────
    if dist < 13.8 {
        let w = ((ra * 4.0 + dist * 0.85 + rot * 0.25).sin() + 1.0) * 0.5;
        if w > 0.76 { return (MAUVE,    '\u{00b7}'); }
        if w > 0.62 { return (LAVENDER, '\u{00b7}'); }
    }
    (RESET, ' ')
}

fn run_blackhole(info: &[String], duration: Option<u64>) {
    const FRAME_MS: u64  = 54;
    const BH_ROWS:  usize = 19;
    const BH_COLS:  usize = 46;
    const CX:       f64   = 23.0;
    const CY:       f64   = 9.0;


    RUNNING.store(true, Ordering::Relaxed);
    unsafe { libc::signal(libc::SIGINT, sigint_handler as *const () as libc::sighandler_t); }

    let max_frames: Option<u64> = match duration {
        None    => Some(56), Some(0) => None, Some(n) => Some(n * 1000 / FRAME_MS),
    };
    let lbl = match duration {
        None    => "~3s".into(), Some(0) => "\u{221e}  (Ctrl+C)".into(),
        Some(n) => format!("{}s", n),
    };
    let out     = stdout();
    let mut buf = out.lock();
    write!(buf, "\x1b[?25l").ok();

    // ── background star positions — 40 fixed stars ──────────
    let mut stars = [(0usize, 0usize, 0u64); 40];
    for (i, s) in stars.iter_mut().enumerate() {
        let h = i.wrapping_mul(0x9e37_79b9);
        s.0 = 2 + (h & 0x2f) as usize;         // col offset
        s.1 = 1 + ((h >> 8) & 0x1f) as usize;  // row offset
        s.2 = (h >> 16) as u64 & 0x3f;          // phase
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
        let pulse = ((frame as f64 * 0.12).sin() * 0.3 + 0.7);
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

        for r in 0..BH_ROWS {
            write!(buf, "  ").ok();
            let mut prev: &str = "";
            for c in 0..BH_COLS {
                // ── shooting star streak ────────────────────
                if ss_alive && r == ss_row && c >= ss_col && c < ss_col + 6 {
                    let tail = (c - ss_col) as f64;
                    if tail < 1.0 { write!(buf, "{}{}", RESET, TEXT).ok(); write!(buf, "\u{258c}").ok(); prev = ""; continue; }
                    let bright = 1.0 - tail / 6.0;
                    if bright > 0.3 {
                        let ch = if bright > 0.7 { '\u{2590}' } else { '\u{00b7}' };
                        write!(buf, "{}{}", RESET, SKY).ok(); write!(buf, "{}", ch).ok(); prev = "";
                        continue;
                    }
                }
                // ── background star ─────────────────────────
                let mut star_rendered = false;
                for s in &stars {
                    if s.0 == c && s.1 == r {
                        let phase = (frame.wrapping_add(s.2)) & 0x1f;
                        if phase < 22 {
                            let star_colors = [OVERLAY0, LAVENDER, MAUVE, TEXT, SKY, BLUE];
                            let ci = (s.2 as usize + frame as usize) % star_colors.len();
                            let sc = if phase < 4 { star_colors[(ci + 1) % star_colors.len()] }
                                     else { star_colors[ci] };
                            write!(buf, "{}{}",
                                RESET, sc).ok();
                            write!(buf, "{}", '\u{00b7}').ok();
                            prev = "";
                            star_rendered = true;
                        }
                        break;
                    }
                }
                if star_rendered { continue; }
                // ── accretion disk ──────────────────────────
                let (color, ch) = bh_cell(c, r, CX, CY, rot);
                // ── relativistic jet (poles) ────────────────
                let dx_j = c as f64 - CX;
                let dy_j = r as f64 - CY;
                let d_j  = (dx_j*dx_j + dy_j*dy_j).sqrt();
                let jet  = d_j > 2.5 && d_j < 14.0 && dx_j.abs() < 1.5 && dy_j.abs() > 2.5
                    && ((dy_j > 0.0 && ((dx_j * 0.4 + rot * 0.5).sin() > 0.0))
                        || (dy_j < 0.0 && ((dx_j * 0.4 - rot * 0.5).sin() > 0.0)));
                if jet && ch == ' ' {
                    let jet_col = if dy_j < 0.0 { MAUVE } else { LAVENDER };
                    let jet_ch  = if ((dx_j * 2.0 + rot * 1.5).sin() * 0.5 + 0.5) > 0.6
                        { '\u{2591}' } else { '\u{00b7}' };
                    write!(buf, "{}{}", RESET, jet_col).ok();
                    write!(buf, "{}", jet_ch).ok();
                    prev = "";
                    continue;
                }
                if color != prev { write!(buf, "{}{}", RESET, color).ok(); prev = color; }
                write!(buf, "{}", ch).ok();
            }
            write!(buf, "{}  ", RESET).ok();
            if let Some(line) = info.get(r) { write!(buf, "{}{}", RESET, line).ok(); }
            write!(buf, "\x1b[K\n").ok();
        }
        buf.flush().ok();
        frame += 1;
        thread::sleep(Duration::from_millis(FRAME_MS));
    }
    write!(buf, "\x1b[?25h").ok();
    // ── brief afterglow message ─────────────────────────────
    write!(buf, "\x1b[H").ok();
    for _ in 0..BH_ROWS + 4 { write!(buf, "\x1b[K\n").ok(); }
    write!(buf, "\x1b[H").ok();
    writeln!(buf, "  {OVL}the singularity collapsed. you are back.{R}", OVL=OVERLAY0, R=RESET).ok();
    writeln!(buf).ok();
    buf.flush().ok();
}

// ── Quantum collapse — wave-function animation ─────────────
fn run_quantum(info_lines: &[String], logo_name: &str, custom_lines: &Option<Vec<String>>,
               cfg: &Config, args: &Args) {
    const FRAME_MS: u64 = 70;
    const FADES: usize = 8;
    const SUPER: &[char] = &['░', '▒', '▓', '█'];

    let out     = stdout();
    let mut buf = out.lock();
    write!(buf, "\x1b[?25l").ok();

    for frame in 0..FADES {
        write!(buf, "\x1b[H").ok();
        let out_lines: Vec<String> = info_lines.iter().map(|s| if args.no_color { strip_ansi(s) } else { s.clone() }).collect();

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
            let logo_w = lines.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
            for i in 0..lines.len().max(faded.len()) {
                let ll  = lines.get(i).map(String::as_str).unwrap_or("");
                let pad = logo_w.saturating_sub(visible_chars(ll));
                let inf = faded.get(i).map(String::as_str).unwrap_or("");
                if args.no_color {
                    writeln!(buf, "  {}{}  {}", ll, " ".repeat(pad), inf).ok();
                } else {
                    writeln!(buf, "  {acc}{ll}{pad}{R}  {inf}",
                        acc = &cfg.colors.accent, ll = ll,
                        pad = " ".repeat(pad), R = RESET, inf = inf).ok();
                }
            }
        } else {
            let logo   = logos::from_name(logo_name);
            let logo_w = logo.iter().map(|l| visible_chars(l)).max().unwrap_or(36);
            let logo_accent: &str = if logo_name == "gentoo" { config::MAUVE } else { cfg.colors.accent.as_str() };
            for i in 0..logo.len().max(faded.len()) {
                let ll  = logo.get(i).copied().unwrap_or("");
                let pad = logo_w.saturating_sub(visible_chars(ll));
                let inf = faded.get(i).map(String::as_str).unwrap_or("");
                if args.no_color {
                    writeln!(buf, "  {}{}  {}", ll, " ".repeat(pad), inf).ok();
                } else {
                    writeln!(buf, "  {acc}{ll}{pad}{R}  {inf}",
                        acc = logo_accent, ll = ll,
                        pad = " ".repeat(pad), R = RESET, inf = inf).ok();
                }
            }
        }
        buf.flush().ok();
        thread::sleep(Duration::from_millis(FRAME_MS));
    }

    // flash — measurement collapse
    write!(buf, "\x1b[H").ok();
    let final_lines: Vec<String> = info_lines.iter().map(|s| if args.no_color { strip_ansi(s) } else { s.clone() }).collect();
    writeln!(buf).ok();
    if let Some(lines) = custom_lines {
        let logo_w = lines.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
        for i in 0..lines.len().max(final_lines.len()) {
            let ll  = lines.get(i).map(String::as_str).unwrap_or("");
            let pad = logo_w.saturating_sub(visible_chars(ll));
            let inf = final_lines.get(i).map(String::as_str).unwrap_or("");
            write!(buf, "  {}{}  {}\n", ll, " ".repeat(pad), inf).ok();
        }
    } else {
        let logo   = logos::from_name(logo_name);
        let logo_w = logo.iter().map(|l| visible_chars(l)).max().unwrap_or(36);
        let logo_accent: &str = if logo_name == "gentoo" { config::MAUVE } else { cfg.colors.accent.as_str() };
        for i in 0..logo.len().max(final_lines.len()) {
            let ll  = logo.get(i).copied().unwrap_or("");
            let pad = logo_w.saturating_sub(visible_chars(ll));
            let inf = final_lines.get(i).map(String::as_str).unwrap_or("");
            write!(buf, "  {acc}{ll}{pad}{R}  {inf}\n",
                acc = logo_accent, ll = ll,
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
}

fn parse_args() -> Args {
    let mut a = Args {
        logo: "arch".into(), logo_explicit: false, logo_file: None,
        accent: None, preset: None,
        no_color: false, blackhole: false,
        mandelbrot: false, mandel_iter: 64,
        quantum: false, cosmic: false,
        duration: None, help: false, version: false, show_cfg: false,
    };
    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help"    => a.help      = true,
            "-V" | "--version" => a.version   = true,
            "--blackhole"      => a.blackhole  = true,
            "--no-color"       => a.no_color   = true,
            "--config"         => a.show_cfg   = true,
            "--full"           => { a.preset = Some("full".into()); }
            "--mandelbrot"     => { a.mandelbrot = true; a.mandel_iter = it.next().and_then(|v| v.parse().ok()).unwrap_or(64); }
            "--quantum"        => a.quantum    = true,
            "--cosmic"         => a.cosmic     = true,
            "--logo"      => { a.logo         = it.next().unwrap_or_else(|| "arch".into()); a.logo_explicit = true; }
            "--logo-file" => { a.logo_file  = it.next(); }
            "--accent"    => { a.accent     = it.next(); }
            "--preset"    => { a.preset     = it.next(); }
            "--t"         => { a.duration   = it.next().and_then(|v| v.parse().ok()); }
            _             => {}
        }
    }
    a
}

fn print_version() {
    println!();
    println!("  {S}version  {B}{BLUE}0.7.3{R}", S=SUBTEXT1, B=BOLD, BLUE=BLUE, R=RESET);
    println!();
    // E = mc²: energy of startup ≈ 0
    println!("  {Y}E{R} = {G}m{R}{S}c{R}\u{00b2}   {OVL}where E = startup time \u{2248} 0{R}",
        Y=YELLOW, R=RESET, G=GREEN, S=SUBTEXT1, OVL=OVERLAY0);
    println!();
}

fn print_config_help(cfg_path: &str) {
    let (b, r, s, g) = (BOLD, RESET, SUBTEXT1, GREEN);
    println!();
    println!("  {b}{PEACH}config file{r}  {s}{cfg_path}{r}", b=b, PEACH=PEACH, r=r, s=s, cfg_path=cfg_path);
    println!();
    println!("  {b}{BLUE}[colors]{r}", b=b, BLUE=BLUE, r=r);
    println!("  {s}# 7 label slots (cycle through fields), accent, values, sep, bar{r}", s=s, r=r);
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
    println!();
    println!("  {b}{BLUE}[show]{r}  {s}(true/false each field){r}", b=b, BLUE=BLUE, r=r, s=s);
    println!("  os=true  kernel=true  uptime=true  res=false  pkgs=true");
    println!("  shell=true  de_wm=true  term=true  cpu=true  gpu=true");
    println!("  memory=true  disk=false  load=false  locale=false  swatches=false");
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
disk        = true
load        = false
locale      = false

# hacker fields (hidden by default)
ip          = false
ssh         = false
ports       = false

swatches    = false   # color palette swatches row (hidden by default)

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
    println!("  {b}{MAUVE}arcfetch{r}  v0.7  blazing-fast Arch sysinfo", b=b, MAUVE=MAUVE, r=r);
    println!();
    println!("  {b}{BLUE}usage{r}   arcfetch [OPTIONS]", b=b, BLUE=BLUE, r=r);
    println!();
    println!("  {b}{SAPPHIRE}flags{r}", b=b, SAPPHIRE=SAPPHIRE, r=r);
    println!("    {b}{g}-h, --help{r}                 this screen", b=b, g=g, r=r);
    println!("    {b}{g}-V, --version{r}              version + E=mc² joke", b=b, g=g, r=r);
    println!("    {b}{g}--config{r}                   show config ref + write sample file", b=b, g=g, r=r);
    println!("    {b}{g}--blackhole{r}                {MAUVE}M87 accretion disk animation{r}", b=b, g=g, r=r, MAUVE=MAUVE);
    println!("    {b}{g}--t <secs>{r}                 {s}0=infinite  N=N seconds{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--logo <name>{r}              switch logo {s}(arch|mini|nix|ascii|auto…){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--logo-file <path>{r}         {s}ASCII art or image (kitty protocol){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--preset <n>{r}               {s}full|minimal|hacker|science{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--full{r}                     {s}show all fields (overrides minimal default){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--accent <color>{r}           hex or catppuccin name", b=b, g=g, r=r);
    println!("    {b}{g}--no-color{r}                 plain text (pipe-friendly)", b=b, g=g, r=r);
    println!("    {b}{g}--mandelbrot [iter]{r}        {GREEN}Mandelbrot set as logo{r}", b=b, g=g, r=r, GREEN=GREEN);
    println!("    {b}{g}--quantum{r}                  {MAUVE}wave-function collapse animation{r}", b=b, g=g, r=r, MAUVE=MAUVE);
    println!("    {b}{g}--cosmic{r}                   {SKY}starfield + moon + shooting stars{r}", b=b, g=g, r=r, SKY=SKY);
    println!("    {b}{g}--t <secs>{r}                 {s}0=infinite  N=N seconds (blackhole & cosmic){r}", b=b, g=g, r=r, s=s);
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
    println!("    {g}--blackhole{r}   accretion disk (physics)", g=g, r=r);
    println!("    {g}--mandelbrot{r}  fractal logo (math)", g=g, r=r);
    println!("    {g}--quantum{r}     wave collapse (physics)", g=g, r=r);
    println!("    {g}--cosmic{r}      starfield + moon + shooting stars (astronomy)", g=g, r=r);
    println!("    {s}mutually exclusive — pick one{r}", s=s, r=r);
    println!();
    println!("  {b}{OVERLAY0}shell startup (e.g. ~/.zshrc){r}", b=b, OVERLAY0=OVERLAY0, r=r);
    println!("    arcfetch                       {s}# minimal default (fast){r}", s=s, r=r);
    println!("    arcfetch --full               {s}# show all fields{r}", s=s, r=r);
    println!("    arcfetch --mandelbrot          {s}# fractal as logo{r}", s=s, r=r);
    println!("    arcfetch --logo nix --cosmic   {s}# cosmic + nix logo{r}", s=s, r=r);
    println!();
}

// ═══════════════════════════════════════════════════════════
//  main
// ═══════════════════════════════════════════════════════════

fn main() {
    let args    = parse_args();
    let cfg_path = config::config_path();

    if args.version  { print_version();               return; }
    if args.help     { print_help(&cfg_path);          return; }
    if args.show_cfg { print_config_help(&cfg_path); write_sample_config(&cfg_path); return; }

    // mutual exclusion for display modes
    let mode_count = [args.blackhole, args.cosmic, args.quantum, args.mandelbrot].iter().filter(|&&m| m).count();
    if mode_count > 1 {
        eprintln!("arcfetch: only one display mode at a time (--blackhole, --cosmic, --quantum, --mandelbrot)");
        return;
    }

    let cfg = config::load(args.accent.as_deref(), args.preset.as_deref());

    // collect all sysinfo — only read network for hacker preset
    let need_net = cfg.preset.as_deref() == Some("hacker")
        || cfg.show.ip || cfg.show.ssh || cfg.show.ports;
    let si = info::collect_all(need_net, &cfg.show);

    // build info column
    let (info_lines, is_science) = build_info(&si, &cfg);

    // ── blackhole mode: consume info_lines into out_lines ──
    if args.blackhole {
        let out_lines: Vec<String> = if args.no_color {
            info_lines.iter().map(|s| strip_ansi(s)).collect()
        } else {
            info_lines
        };
        run_blackhole(&out_lines, args.duration);
        return;
    }

    // ── resolve logo (shared between quantum, cosmic and normal render) ──
    // priority: --logo-file  >  --logo custom  >  science random  >  named logo

    // default custom logo path: ~/.config/arcfetch/logo.txt
    let default_custom = {
        let cfg_dir = config::config_path()
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_else(|| "~/.config/arcfetch".into());
        format!("{}/logo.txt", cfg_dir)
    };

    // load lines from a file — any number of lines, any width
    let load_file = |path: &str| -> Option<Vec<String>> {
        let expanded = if path.starts_with('~') {
            env::var("HOME").ok()
                .map(|h| path.replacen('~', &h, 1))
                .unwrap_or_else(|| path.to_string())
        } else {
            path.to_string()
        };
        std::fs::read_to_string(&expanded).ok().map(|raw| {
            raw.lines().map(String::from).collect()
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
        let base = if args.logo == "auto" { detect_auto_logo() }
                   else if !args.logo_explicit && cfg.preset.as_deref() == Some("minimal") { "mini".into() }
                   else { args.logo.clone() };
        if custom_lines.is_none() && is_science && base == "arch" {
            science_logo().to_string()
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

    // ── cosmic mode ──
    if args.cosmic {
        cosmic::run(&out_lines, &logo_name, &cfg, &args);
        return;
    }

    // ── single-buffer render (one syscall) ──
    use std::fmt::Write;
    let mut out = String::with_capacity(4096);
    out.push('\n');

    if let Some(ref lines) = custom_lines {
        let logo_w = lines.iter().map(|l| visible_chars(l)).max().unwrap_or(0);
        let max_rows = lines.len().max(out_lines.len());
        for i in 0..max_rows {
            let ll   = lines.get(i).map(String::as_str).unwrap_or("");
            let pad  = logo_w.saturating_sub(visible_chars(ll));
            let inf  = out_lines.get(i).map(String::as_str).unwrap_or("");
            if args.no_color {
                let _ = write!(out, "  {}{:pad$}  {}\n", ll, "", inf, pad = pad);
            } else {
                let lc = if logo_name == "mini" { cfg.colors.label(i) } else { cfg.colors.accent.as_str() };
                let _ = write!(out, "  {}{}{:pad$}{}  {}\n",
                    lc, ll, "", RESET, inf, pad = pad);
            }
        }
    } else {
        let logo   = logos::from_name(&logo_name);
        let logo_w = logo.iter().map(|l| visible_chars(l)).max().unwrap_or(36);
        let logo_accent: &str = if logo_name == "gentoo" { config::MAUVE } else { cfg.colors.accent.as_str() };
        let max_rows = logo.len().max(out_lines.len());
        for i in 0..max_rows {
            let ll   = logo.get(i).copied().unwrap_or("");
            let pad  = logo_w.saturating_sub(visible_chars(ll));
            let inf  = out_lines.get(i).map(String::as_str).unwrap_or("");
            if args.no_color {
                let _ = write!(out, "  {}{:pad$}  {}\n", ll, "", inf, pad = pad);
            } else {
                let lc = if logo_name == "mini" { cfg.colors.label(i) } else { logo_accent };
                let _ = write!(out, "  {}{}{:pad$}{}  {}\n",
                    lc, ll, "", RESET, inf, pad = pad);
            }
        }
    }

    out.push('\n');
    unsafe { libc::write(1, out.as_ptr().cast::<core::ffi::c_void>(), out.len()); }
}
