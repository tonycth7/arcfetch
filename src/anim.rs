// src/anim.rs — animation frames from assets/animations/ and the user config
//
// A directory holds one text file per frame. Frames are played in filename
// order, so `01.txt`, `02.txt`, … is the easy convention. Frames are used
// verbatim: animfetch's ASCII art may contain `$`, `$$` or `$R` literally, so
// no logo-slot expansion is applied here.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config;

include!(concat!(env!("OUT_DIR"), "/bundled_anims.rs"));

pub struct Animation {
    frames: Vec<Vec<String>>,
    width:  usize,
    height: usize,
}

impl Animation {
    /// Load a named animation from the config directory, a bundled name, or an
    /// explicit path. A user animation shadows a bundled one of the same name.
    pub fn load(arg: &str) -> Result<Self, String> {
        let path = resolve(arg);
        if path.is_dir() {
            let mut files: Vec<PathBuf> = fs::read_dir(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_frame(p))
                .collect();
            files.sort();
            if files.is_empty() {
                return Err(format!(
                    "no .txt or .ansi frames in {} (add 01.txt, 02.txt, …)",
                    path.display()
                ));
            }
            Self::from_files(files)
        } else if path.is_file() {
            Self::from_files(vec![path])
        } else if let Some((_, frames)) = BUNDLED.iter().find(|(n, _)| *n == arg) {
            Self::from_texts(frames.iter().map(|s| (*s).to_string()))
        } else {
            Err(format!(
                "animation not found: {arg} (try `arcfetch --list-animations`)"
            ))
        }
    }

    fn from_files(files: Vec<PathBuf>) -> Result<Self, String> {
        let mut texts = Vec::with_capacity(files.len());
        for file in files {
            texts.push(
                fs::read_to_string(&file)
                    .map_err(|e| format!("cannot read {}: {e}", file.display()))?,
            );
        }
        Self::from_texts(texts)
    }

    fn from_texts(texts: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut frames: Vec<Vec<String>> = Vec::new();
        for raw in texts {
            let lines: Vec<String> = raw
                .lines()
                .map(|l| l.trim_end_matches('\r').to_string())
                .collect();
            if lines.is_empty() {
                return Err("empty animation frame".into());
            }
            frames.push(lines);
        }
        if frames.is_empty() {
            return Err("animation has no frames".into());
        }

        let height = frames.iter().map(Vec::len).max().unwrap_or(1);
        let width = frames
            .iter()
            .flat_map(|f| f.iter())
            .map(|l| crate::visible_chars(l))
            .max()
            .unwrap_or(1)
            .max(1);

        for frame in &mut frames {
            for line in frame.iter_mut() {
                let pad = width.saturating_sub(crate::visible_chars(line));
                if pad > 0 {
                    line.push_str(&" ".repeat(pad));
                }
            }
            while frame.len() < height {
                frame.push(" ".repeat(width));
            }
        }

        Ok(Self { frames, width, height })
    }

    pub fn frame(&self, tick: usize, every: usize) -> &[String] {
        &self.frames[(tick / every.max(1)) % self.frames.len()]
    }

    /// Shrink to fit `max_w` x `max_h` cells, keeping the aspect ratio. Frames
    /// already small enough are returned unchanged.
    pub fn fitted(&self, max_w: usize, max_h: usize) -> Vec<Vec<String>> {
        let (max_w, max_h) = (max_w.max(1), max_h.max(1));
        if self.width <= max_w && self.height <= max_h {
            return self.frames.clone();
        }
        let scale = (max_w as f32 / self.width as f32)
            .min(max_h as f32 / self.height as f32);
        let w = ((self.width as f32 * scale).round() as usize).max(1).min(max_w);
        let h = ((self.height as f32 * scale).round() as usize).max(1).min(max_h);

        self.frames
            .iter()
            .map(|frame| {
                (0..h)
                    .map(|y| {
                        let sy = (y * self.height / h).min(frame.len().saturating_sub(1));
                        sample_line(frame.get(sy).map(String::as_str).unwrap_or(""), w, self.width)
                    })
                    .collect()
            })
            .collect()
    }

    /// Bundled animations, as `(name, frame count)`.
    pub fn bundled_list() -> Vec<(&'static str, usize)> {
        BUNDLED.iter().map(|(name, frames)| (*name, frames.len())).collect()
    }

    /// Custom animations in `~/.config/arcfetch/animations/<name>/`.
    pub fn list() -> Vec<(String, usize)> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(animations_dir()) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let frames = fs::read_dir(&path)
                .map(|r| {
                    r.flatten()
                        .filter(|e| e.path().is_file() && is_frame(&e.path()))
                        .count()
                })
                .unwrap_or(0);
            if frames > 0 {
                out.push((entry.file_name().to_string_lossy().into_owned(), frames));
            }
        }
        out.sort();
        out
    }
}

/// Take `w` evenly spaced visible cells from `line`, keeping the ANSI colour
/// escapes attached to the cells that survive.
fn sample_line(line: &str, w: usize, src_w: usize) -> String {
    let mut cells: Vec<(String, char)> = Vec::with_capacity(src_w);
    let mut escapes = String::new();
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            escapes.push(ch);
            for c in chars.by_ref() {
                escapes.push(c);
                if c.is_ascii_alphabetic() { break; }
            }
        } else {
            cells.push((std::mem::take(&mut escapes), ch));
        }
    }
    if cells.is_empty() {
        return " ".repeat(w);
    }

    let mut out = String::with_capacity(w * 4);
    let mut coloured = false;
    for x in 0..w {
        let src = (x * cells.len() / w).min(cells.len() - 1);
        let (esc, ch) = &cells[src];
        out.push_str(esc);
        coloured |= !esc.is_empty();
        out.push(*ch);
    }
    if coloured {
        out.push_str(config::RESET);
    }
    out
}

fn is_frame(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()),
        Some(ref e) if e == "txt" || e == "ansi"
    )
}

fn animations_dir() -> PathBuf {
    Path::new(&config::config_path())
        .parent()
        .unwrap_or(Path::new("."))
        .join("animations")
}

fn resolve(arg: &str) -> PathBuf {
    let expanded = crate::expand_path(arg);
    let path = PathBuf::from(&expanded);
    if path.exists() {
        path
    } else {
        animations_dir().join(arg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "arcfetch-anim-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn loads_sorted_frames_and_pads_width() {
        let dir = temp_dir("sort");
        fs::write(dir.join("02.txt"), "ab").unwrap();
        fs::write(dir.join("01.txt"), "█").unwrap();

        let anim = Animation::load(dir.to_str().unwrap()).unwrap();
        assert_eq!(anim.frame(0, 1)[0], "█ ");
        assert_eq!(anim.frame(1, 1)[0], "ab");
        assert_eq!(anim.frame(2, 1)[0], "█ ");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundled_frames_and_scaling_work() {
        let anim = Animation::load("saturn").unwrap();
        assert!(!anim.frame(0, 1).is_empty());
        let fitted = anim.fitted(20, 5);
        assert!(fitted[0].len() <= 5);
        assert!(fitted[0].iter().all(|line| crate::visible_chars(line) <= 20));
    }
}
