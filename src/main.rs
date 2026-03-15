// arcfetch v0.7.0 — sub-ms Arch Linux sysinfo · Catppuccin Mocha
// arcfetch [-h] [-V] [--blackhole [--t N]] [--logo NAME] [--accent COLOR]
//          [--no-color] [--config]
mod config;
mod info;
mod logos;

use std::{env, thread};
use std::f64::consts::PI;
use std::io::{BufWriter, Write, stdout};
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

fn row(label_color: &str, key: &str, val: &str, val_color: &str) -> String {
    let mut s = String::with_capacity(120);
    s.push_str(BOLD); s.push_str(label_color); s.push_str(key);
    for _ in key.len()..9 { s.push(' '); }
    s.push_str(RESET); s.push(' ');
    s.push_str(val_color); s.push_str(val); s.push_str(RESET);
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

/// Build the info column as a Vec<String> respecting show/hide config.
fn build_info(si: &info::SysInfo, cfg: &Config) -> Vec<String> {
    let c  = &cfg.colors;
    let sh = &cfg.show;
    let mut lines: Vec<String> = Vec::with_capacity(19);

    // header — always shown
    let username = env::var("USER").or_else(|_| env::var("LOGNAME")).unwrap_or_else(|_| "user".into());
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default().trim().to_string();
    let pw       = username.len() + 1 + hostname.len();

    lines.push(format!("{B}{uc}{u}{OVL}@{R}{B}{hc}{h}{R}",
        B=BOLD, uc=&c.username, u=username,
        OVL=OVERLAY0, R=RESET, hc=&c.hostname, h=hostname));
    lines.push(format!("{sc}{l}{R}", sc=&c.sep, l="\u{2500}".repeat(pw), R=RESET));

    // macro avoids borrow conflict — closure would hold &mut lines + &mut li
    // simultaneously with the memory block below
    macro_rules! field {
        ($show:expr, $key:expr, $val:expr) => {
            if $show {
                lines.push(row(c.label(li), $key, $val, &c.values));
                li += 1;
            }
        };
    }

    let mut li = 0usize;

    field!(sh.os,     "os",     &si.os);
    field!(sh.kernel, "kernel", &si.kernel);
    field!(sh.uptime, "uptime", &si.uptime);
    field!(sh.res,    "res",    &si.res);
    field!(sh.pkgs,   "pkgs",   &si.pkgs);
    field!(sh.shell,  "shell",  &si.shell);
    field!(sh.de_wm,  "de/wm",  &si.de_wm);
    field!(sh.term,   "term",   &si.term);
    field!(sh.cpu,    "cpu",    &si.cpu);
    field!(sh.gpu,    "gpu",    &si.gpu);

    // memory — special: includes bar
    if sh.memory {
        let bar     = mem_bar(si.mem_used, si.mem_total, 14, &c.bar);
        let mem_str = if si.mem_total >= 1024 {
            format!("{bar}{TEXT}  {u:.1}G / {t:.1}G{R}",
                bar=bar, TEXT=TEXT,
                u=si.mem_used as f64/1024.0,
                t=si.mem_total as f64/1024.0, R=RESET)
        } else {
            format!("{bar}{TEXT}  {u}M / {t}M{R}",
                bar=bar, TEXT=TEXT,
                u=si.mem_used, t=si.mem_total, R=RESET)
        };
        let mut s = String::with_capacity(140);
        s.push_str(BOLD); s.push_str(c.label(li));
        s.push_str("memory"); s.push_str("   ");
        s.push_str(RESET); s.push_str(&mem_str);
        lines.push(s);
        li += 1;
    }

    field!(sh.disk,   "disk",   &si.disk);
    field!(sh.load,   "load",   &si.load);
    field!(sh.locale, "locale", &si.locale);

    // blank line + swatches
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

    lines
}

// ═══════════════════════════════════════════════════════════
//  Black Hole — M87 accretion disk
// ═══════════════════════════════════════════════════════════

static RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn sigint_handler(_: libc::c_int) {
    RUNNING.store(false, Ordering::Relaxed);
}

fn bh_cell(col: usize, row_idx: usize, cx: f64, cy: f64, rot: f64) -> (&'static str, char) {
    let dx   = (col as f64 - cx) * 0.52;
    let dy   = row_idx as f64 - cy;
    let dist = (dx*dx + dy*dy).sqrt();

    if dist < 3.6 { return (RESET, ' '); }

    let ra = ((dy.atan2(dx) + rot) % (2.0*PI) + 2.0*PI) % (2.0*PI);

    if dist < 4.9 {
        let b = (ra * 1.3).sin() * 0.4 + 0.35;
        return (SKY, if b > 0.45 { '\u{2591}' } else { '\u{00b7}' });
    }
    if dist < 10.4 {
        let b = ((ra.sin() * 0.5 + 0.5) * (1.0 - (dist - 4.9) / 5.5 * 0.38)).clamp(0.0, 1.0);
        let ch: char = match (b * 4.8) as u8 {
            4|5 => '\u{2588}', 3 => '\u{2593}', 2 => '\u{2592}',
            1   => '\u{2591}', _ => '\u{00b7}',
        };
        let col: &'static str = match (b * 4.0) as u8 {
            3|4           => YELLOW, 2 => PEACH, 1 => RED,
            _ if b > 0.06 => MAROON, _ => OVERLAY0,
        };
        return (col, ch);
    }
    if dist < 13.2 {
        let w = ((ra * 4.0 + dist * 0.85 + rot * 0.25).sin() + 1.0) * 0.5;
        if w > 0.74 { return (MAUVE,    '\u{00b7}'); }
        if w > 0.60 { return (LAVENDER, '\u{00b7}'); }
    }
    (RESET, ' ')
}

fn run_blackhole(info: &[String], duration: Option<u64>) {
    const FRAME_MS: u64  = 62;
    const BH_ROWS:  usize = 19;
    const BH_COLS:  usize = 46;
    const CX:       f64   = 23.0;
    const CY:       f64   = 9.0;

    RUNNING.store(true, Ordering::Relaxed);
    unsafe { libc::signal(libc::SIGINT, sigint_handler as *const () as libc::sighandler_t); }

    let max_frames: Option<u64> = match duration {
        None    => Some(48), Some(0) => None, Some(n) => Some(n * 1000 / FRAME_MS),
    };
    let lbl = match duration {
        None    => "~3s".into(), Some(0) => "\u{221e}  (Ctrl+C)".into(),
        Some(n) => format!("{}s", n),
    };

    let out     = stdout();
    let mut buf = BufWriter::new(out.lock());
    write!(buf, "\x1b[?25l").ok();
    writeln!(buf).ok();
    writeln!(buf,
        "  {B}{MV}\u{25cf}  arcfetch{R}  {OVL}\u{00b7}\u{00b7}\u{00b7}  {MV}SINGULARITY MODE{R}  {OVL}[{lbl}]{R}",
        B=BOLD, MV=MAUVE, OVL=OVERLAY0, R=RESET, lbl=lbl).ok();
    writeln!(buf, "  {OVL}{sep}{R}",
        OVL=OVERLAY0, sep="\u{2500}".repeat(62), R=RESET).ok();
    buf.flush().ok();

    let mut frame = 0u64;
    loop {
        if !RUNNING.load(Ordering::Relaxed) { break; }
        if let Some(max) = max_frames { if frame >= max { break; } }

        let rot = (frame as f64 * PI / 6.0) % (2.0*PI);
        if frame > 0 { write!(buf, "\x1b[{}A", BH_ROWS).ok(); }

        for r in 0..BH_ROWS {
            write!(buf, "  ").ok();
            let mut prev: &str = "";
            for c in 0..BH_COLS {
                let (color, ch) = bh_cell(c, r, CX, CY, rot);
                if color != prev { write!(buf, "{}{}", RESET, color).ok(); prev = color; }
                write!(buf, "{}", ch).ok();
            }
            write!(buf, "{}  ", RESET).ok();
            if let Some(line) = info.get(r) { write!(buf, "{}", line).ok(); }
            writeln!(buf).ok();
        }
        buf.flush().ok();
        frame += 1;
        thread::sleep(Duration::from_millis(FRAME_MS));
    }
    write!(buf, "\x1b[?25h").ok();
    writeln!(buf).ok();
    buf.flush().ok();
}

// ═══════════════════════════════════════════════════════════
//  CLI
// ═══════════════════════════════════════════════════════════

struct Args {
    logo:      String,
    accent:    Option<String>,
    no_color:  bool,
    blackhole: bool,
    duration:  Option<u64>,
    help:      bool,
    version:   bool,
    show_cfg:  bool,
}

fn parse_args() -> Args {
    let mut a = Args {
        logo: "arch".into(), accent: None,
        no_color: false, blackhole: false,
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
            "--logo"    => { a.logo     = it.next().unwrap_or_else(|| "arch".into()); }
            "--accent"  => { a.accent   = it.next(); }
            "--t"       => { a.duration = it.next().and_then(|v| v.parse().ok()); }
            _           => {}
        }
    }
    a
}

fn print_version() {
    println!();
    println!("  {B}{MAUVE}arcfetch{R}", B=BOLD, MAUVE=MAUVE, R=RESET);
    println!("  {OVL}{sep}{R}", OVL=OVERLAY0, sep="\u{2500}".repeat(13), R=RESET);
    println!("  {S}version  {B}{BLUE}0.7.0{R}", S=SUBTEXT1, B=BOLD, BLUE=BLUE, R=RESET);
    println!("  {S}theme    {MAUVE}Catppuccin Mocha{R}", S=SUBTEXT1, MAUVE=MAUVE, R=RESET);
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
    println!("  memory=true  disk=true  load=false  locale=false  swatches=true");
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
    println!("    {b}{g}--config{r}                   show full config reference", b=b, g=g, r=r);
    println!("    {b}{g}--blackhole{r}                {MAUVE}animated M87 accretion disk{r}", b=b, g=g, r=r, MAUVE=MAUVE);
    println!("    {b}{g}--t <secs>{r}                 {s}0=infinite  N=N seconds{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}--logo <name>{r}              switch logo", b=b, g=g, r=r);
    println!("    {b}{g}--accent <color>{r}           hex or catppuccin name", b=b, g=g, r=r);
    println!("    {b}{g}--no-color{r}                 plain text (pipe-friendly)", b=b, g=g, r=r);
    println!();
    println!("  {b}{TEAL}logos{r}", b=b, TEAL=TEAL, r=r);
    println!("    {b}{g}arch{r}   block {s}▟███▙{r} (default)", b=b, g=g, r=r, s=s);
    println!("    {b}{g}ascii{r}  dotty Arch ASCII", b=b, g=g, r=r);
    println!("    {b}{g}tux{r}    Linux penguin", b=b, g=g, r=r);
    println!("    {b}{g}dna{r}    DNA double helix", b=b, g=g, r=r);
    println!("    {b}{g}atom{r}   Bohr atom (Fe)", b=b, g=g, r=r);
    println!("    {b}{g}wave{r}   {s}Schrödinger ψ(x,t){r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}emc2{r}   {s}E = mc²{r}", b=b, g=g, r=r, s=s);
    println!("    {b}{g}pi{r}     {s}π — irrational & beautiful{r}", b=b, g=g, r=r, s=s);
    println!();
    println!("  {b}{YELLOW}templates{r}  {s}(set in config or just use --accent){r}", b=b, YELLOW=YELLOW, r=r, s=s);
    println!("    {g}full{r}     everything  {g}minimal{r}  os kernel uptime memory", g=g, r=r);
    println!("    {g}hacker{r}   cpu gpu mem disk load  {g}science{r}  os kernel cpu mem disk", g=g, r=r);
    println!();
    println!("  {b}{PEACH}config{r}  {s}{cfg_path}{r}", b=b, PEACH=PEACH, r=r, s=s, cfg_path=cfg_path);
    println!("    run {b}{g}arcfetch --config{r} for full config reference", b=b, g=g, r=r);
    println!();
    println!("  {b}{SKY}examples{r}", b=b, SKY=SKY, r=r);
    println!("    arcfetch");
    println!("    arcfetch --logo pi --accent mauve");
    println!("    arcfetch --logo wave --accent '#CBA6F7'");
    println!("    arcfetch --blackhole --t 0         {s}# infinite loop{r}", s=s, r=r);
    println!("    arcfetch --blackhole --t 30        {s}# 30 seconds{r}", s=s, r=r);
    println!("    arcfetch --no-color | tee info.txt {s}# pipe-friendly{r}", s=s, r=r);
    println!();
    println!("  {b}{OVERLAY0}shell startup (e.g. ~/.zshrc){r}", b=b, OVERLAY0=OVERLAY0, r=r);
    println!("    arcfetch                           {s}# default{r}", s=s, r=r);
    println!("    arcfetch --logo pi                 {s}# pin a logo{r}", s=s, r=r);
    println!("    arcfetch --logo wave --accent teal {s}# logo + color{r}", s=s, r=r);
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
    if args.show_cfg { print_config_help(&cfg_path);   return; }

    let cfg = config::load(args.accent.as_deref());

    // collect all sysinfo in parallel (rayon)
    let si = info::collect_all();

    // build info column
    let mut info_lines = build_info(&si, &cfg);

    // optionally strip colour
    let out_lines: Vec<String> = if args.no_color {
        info_lines.iter().map(|s| strip_ansi(s)).collect()
    } else {
        info_lines.drain(..).collect()
    };

    if args.blackhole { run_blackhole(&out_lines, args.duration); return; }

    let logo   = logos::from_name(&args.logo);
    let logo_w = logo.iter().map(|l| l.chars().count()).max().unwrap_or(36);

    let out     = stdout();
    let mut buf = BufWriter::new(out.lock());
    writeln!(buf).ok();

    let max_rows = logo.len().max(out_lines.len());
    for i in 0..max_rows {
        let ll  = logo.get(i).copied().unwrap_or("");
        let pad = logo_w.saturating_sub(ll.chars().count());
        let inf = out_lines.get(i).map(String::as_str).unwrap_or("");

        if args.no_color {
            writeln!(buf, "  {}{}  {}", ll, " ".repeat(pad), inf).ok();
        } else {
            writeln!(buf, "  {acc}{ll}{pad}{R}  {inf}",
                acc = &cfg.colors.accent,
                ll  = ll,
                pad = " ".repeat(pad),
                R   = RESET,
                inf = inf).ok();
        }
    }
    writeln!(buf).ok();
}
