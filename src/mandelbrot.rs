// src/mandelbrot.rs — pure Rust Mandelbrot set renderer

const ROWS: usize = 19;
const COLS: usize = 31;

/// Render Mandelbrot set as 19 ASCII strings, each 31 chars wide.
pub fn render(max_iter: u32) -> Vec<String> {
    let x_min = -2.0f64;
    let x_max = 0.5;
    let y_min = -1.0;
    let y_max = 1.0;
    let dx = (x_max - x_min) / COLS as f64;
    let dy = (y_max - y_min) / ROWS as f64;

    let mut out: Vec<String> = Vec::with_capacity(ROWS);
    for row in 0..ROWS {
        let mut s = String::with_capacity(COLS);
        let cy = y_min + row as f64 * dy;
        for col in 0..COLS {
            let cx = x_min + col as f64 * dx;
            let mut x = 0.0f64;
            let mut y = 0.0f64;
            let mut iter = 0u32;
            while iter < max_iter {
                let x2 = x * x;
                let y2 = y * y;
                if x2 + y2 > 4.0 { break; }
                y = 2.0 * x * y + cy;
                x = x2 - y2 + cx;
                iter += 1;
            }
            let ch = match iter {
                0..=1   => ' ',
                2..=3   => '\u{2591}',
                4..=6   => '\u{2592}',
                7..=10  => '\u{2593}',
                11..=15 => '\u{2584}',
                16..=24 => '\u{258C}',
                25..=39 => '\u{2590}',
                40..=63 => '\u{2580}',
                _       => '\u{2588}',
            };
            s.push(ch);
        }
        out.push(s);
    }
    out
}
