// src/anim.rs — user animation frames from ~/.config/arcfetch/animations/
//
// A directory holds one text file per frame. Frames are played in filename
// order, so `01.txt`, `02.txt`, … is the easy convention. Frames are used
// verbatim: animfetch's ASCII art may contain `$`, `$$` or `$R` literally, so
// no logo-slot expansion is applied here.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config;

pub struct Animation {
    frames: Vec<Vec<String>>,
}

impl Animation {
    /// Load a named animation from the config directory, or an explicit path.
    pub fn load(arg: &str) -> Result<Self, String> {
        let path = resolve(arg);
        let files = if path.is_dir() {
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
            files
        } else if path.is_file() {
            vec![path]
        } else {
            return Err(format!(
                "animation not found: {} (try `arcfetch --list-animations`)",
                path.display()
            ));
        };

        let mut frames = Vec::with_capacity(files.len());
        for file in files {
            let raw = fs::read_to_string(&file)
                .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
            let lines: Vec<String> = raw
                .lines()
                .map(|l| l.trim_end_matches('\r').to_string())
                .collect();
            if lines.is_empty() {
                return Err(format!("empty animation frame: {}", file.display()));
            }
            frames.push(lines);
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

        Ok(Self { frames })
    }

    pub fn frame(&self, tick: usize, every: usize) -> &[String] {
        &self.frames[(tick / every.max(1)) % self.frames.len()]
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

    #[test]
    fn loads_sorted_frames_and_pads_width() {
        let dir = std::env::temp_dir().join(format!(
            "arcfetch-anim-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("02.txt"), "ab").unwrap();
        fs::write(dir.join("01.txt"), "█").unwrap();

        let anim = Animation::load(dir.to_str().unwrap()).unwrap();
        assert_eq!(anim.frame(0, 1)[0], "█ ");
        assert_eq!(anim.frame(1, 1)[0], "ab");
        assert_eq!(anim.frame(2, 1)[0], "█ ");
        let _ = fs::remove_dir_all(&dir);
    }
}
