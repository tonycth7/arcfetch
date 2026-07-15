// src/cosmic.rs — starfield + moon phase + shooting stars

use std::io::Write;
use std::thread;
use std::time::Duration;

use crate::config::*;

// ── Moon phase ─────────────────────────────────────────────
fn moon_phase() -> &'static str {
    let uptime_secs: u64 = {
        let mut si: libc::sysinfo = unsafe { std::mem::zeroed() };
        if unsafe { libc::sysinfo(&mut si) } == 0 { si.uptime as u64 } else { 0 }
    };
    let boot_approx = std::fs::metadata("/proc/1")
        .and_then(|m| m.created()).map(|t| {
            use std::time::SystemTime;
            t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs_f64()
        }).unwrap_or(0.0);
    let now = boot_approx + uptime_secs as f64;
    let jd = now / 86400.0 + 2440587.5;
    let phase_days = (jd - 2451543.75) % 29.53058867;
    let phase = (phase_days / 29.53058867 * 8.0) as usize % 8;
    MOON_PHASES[phase]
}

const MOON_PHASES: &[&str; 8] = &[
    "\u{1F311}", "\u{1F312}", "\u{1F313}", "\u{1F314}",
    "\u{1F315}", "\u{1F316}", "\u{1F317}", "\u{1F318}",
];

fn star_char(b: u8) -> char {
    match b { 0 => ' ', 1 => '\u{00B7}', 2 => '.', 3 => '\u{2219}', 4 => '*', _ => '\u{2726}' }
}

fn rng_next(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s >> 33
}

pub fn run(out_lines: &[String], _logo_name: &str, _cfg: &Config, args: &super::Args) {
    const ROWS: usize = 19;
    const COLS: usize = 31;
    const STARS: usize = 40;
    const FRAME_MS: u64 = 120;

    let moon = moon_phase();
    let max_frames: Option<u64> = match args.duration {
        None    => Some(48),
        Some(0) => None,
        Some(n) => Some(n * 1000 / FRAME_MS),
    };
    let lbl = match args.duration {
        None    => "~6s".into(),
        Some(0) => "\u{221e}  (Ctrl+C)".into(),
        Some(n) => format!("{}s", n),
    };

    let seed: u64 = {
        let mut si: libc::sysinfo = unsafe { std::mem::zeroed() };
        if unsafe { libc::sysinfo(&mut si) } == 0 { si.uptime as u64 } else { 12345 }
    };
    let mut rng = seed;

    let mut stars = [[0u8; COLS]; ROWS];
    for _ in 0..STARS {
        let col = rng_next(&mut rng) as usize % COLS;
        let row = rng_next(&mut rng) as usize % ROWS;
        let bri = (rng_next(&mut rng) as u8 % 5) + 1;
        if stars[row][col] == 0 { stars[row][col] = bri; }
    }

    let mut shooting: Option<(f64, f64, f64, u64)> = None;

    let out     = std::io::stdout();
    let mut buf = out.lock();
    write!(buf, "\x1b[?25l").ok();
    unsafe { libc::signal(libc::SIGINT, super::sigint_handler as *const () as libc::sighandler_t); }

    let mut frame = 0u64;
    loop {
        if !super::RUNNING.load(std::sync::atomic::Ordering::Relaxed) { break; }
        if let Some(max) = max_frames { if frame >= max { break; } }

        write!(buf, "\x1b[H").ok();

        write!(buf, "\x1b[K  {B}{MAUVE}\u{25cf}  arcfetch  {OVL}\u{00B7}\u{00B7}\u{00B7}  COSMIC MODE  [{moon}]  {OVL}[{lbl}]{R}",
            B=BOLD, MAUVE=MAUVE, OVL=OVERLAY0, R=RESET, moon=moon, lbl=lbl).ok();
        write!(buf, "\x1b[K\n").ok();
        write!(buf, "\x1b[K  {OVL}{s}{R}", OVL=OVERLAY0, s="\u{2500}".repeat(62), R=RESET).ok();
        write!(buf, "\x1b[K\n").ok();

        if shooting.is_none() && rng_next(&mut rng) % 60 == 0 {
            let sx = rng_next(&mut rng) as f64 % COLS as f64;
            let sy = rng_next(&mut rng) as f64 % ROWS as f64;
            let angle = (rng_next(&mut rng) % 3 + 1) as f64 * 0.3 + 0.2;
            shooting = Some((sx, sy, angle, 12 + rng_next(&mut rng) % 8));
        }

        for r in 0..ROWS {
            write!(buf, "\x1b[K").ok();
            let mut sl = [' '; COLS];
            for (c, &sb) in stars[r].iter().enumerate() {
                if sb > 0 {
                    let t = ((frame.wrapping_add(r as u64 * 7).wrapping_add(c as u64 * 13)) % 8) as u8;
                    sl[c] = star_char(sb.saturating_sub(t / 3));
                }
            }
            if let Some((sx, sy, _, _)) = shooting {
                let (xi, yi) = (sx as usize, sy as usize);
                if yi == r && xi < COLS {
                    sl[xi] = '\u{2726}';
                    for t in 1..4 { let tx = xi.saturating_sub(t); if tx < COLS { sl[tx] = '\u{00B7}'; } }
                }
            }
            let ss: String = sl.iter().collect();
            write!(buf, "  {OVL}{ss}{R}  ", OVL=OVERLAY0, ss=ss, R=RESET).ok();
            if let Some(inf) = out_lines.get(r) { write!(buf, "{}", inf).ok(); }
            write!(buf, "\x1b[K\n").ok();
        }

        write!(buf, "\x1b[K  {OVL}phase  {moon}", OVL=OVERLAY0, moon=moon).ok();
        buf.flush().ok();
        frame += 1;

        if let Some((x, y, dx, rem)) = shooting {
            let (nx, ny) = (x + dx * 2.0, y + 1.5);
            if nx >= COLS as f64 || ny >= ROWS as f64 || rem == 0 {
                shooting = None;
            } else {
                shooting = Some((nx, ny, dx, rem - 1));
            }
        }

        thread::sleep(Duration::from_millis(FRAME_MS));
    }
    write!(buf, "\x1b[?25h").ok();
    writeln!(buf).ok();
    buf.flush().ok();
}
