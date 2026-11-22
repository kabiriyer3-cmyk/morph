//! Locates optional external tools (FFmpeg, LibreOffice, Pandoc).
use crate::util::command;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ToolSettings {
    pub ffmpeg: String,
    pub ffprobe: String,
    pub soffice: String,
    pub pandoc: String,
}

#[derive(Default, Clone, Debug)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub soffice: Option<PathBuf>,
    pub pandoc: Option<PathBuf>,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn search_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Ok(cur) = std::env::current_exe() {
        if let Some(d) = cur.parent() {
            v.push(d.to_path_buf());
            v.push(d.join("tools"));
            v.push(d.join("ffmpeg").join("bin"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        v.extend(std::env::split_paths(&path));
    }
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        for k in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(p) = env(k) {
                v.push(p.join("ffmpeg").join("bin"));
                v.push(p.join("LibreOffice").join("program"));
                v.push(p.join("Pandoc"));
            }
        }
        if let Some(p) = env("LOCALAPPDATA") {
            v.push(p.join("Microsoft").join("WinGet").join("Links"));
            v.push(p.join("Pandoc"));
            v.push(p.join("Programs").join("ffmpeg").join("bin"));
        }
        if let Some(p) = env("USERPROFILE") {
            v.push(p.join("scoop").join("shims"));
        }
        v.push(PathBuf::from(r"C:\ffmpeg\bin"));
        v.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
    } else {
        for d in [
            "/usr/bin",
            "/usr/local/bin",
            "/snap/bin",
            "/var/lib/flatpak/exports/bin",
            "/usr/lib/libreoffice/program",
            "/opt/libreoffice/program",
        ] {
            v.push(PathBuf::from(d));
        }
        if let Some(h) = env("HOME") {
            v.push(h.join(".local").join("bin"));
        }
    }
    v
}

pub fn find(names: &[&str], custom: &str) -> Option<PathBuf> {
    let c = custom.trim().trim_matches('"');
    if !c.is_empty() {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
        if p.is_dir() {
            for n in names {
                for q in [p.join(exe(n)), p.join("bin").join(exe(n))] {
                    if q.is_file() {
                        return Some(q);
                    }
                }
            }
        }
    }
    let dirs = search_dirs();
    for n in names {
        for d in &dirs {
            let q = d.join(exe(n));
            if q.is_file() {
                return Some(q);
            }
        }
    }
    None
}

impl Tools {
    pub fn resolve(s: &ToolSettings) -> Tools {
        let ffmpeg = find(&["ffmpeg"], &s.ffmpeg);
        let ffprobe = find(&["ffprobe"], &s.ffprobe).or_else(|| {
            ffmpeg.as_ref().and_then(|f| {
                let q = f.with_file_name(exe("ffprobe"));
                if q.is_file() {
                    Some(q)
                } else {
                    None
                }
            })
        });
        Tools {
            ffmpeg,
            ffprobe,
            soffice: find(&["soffice", "libreoffice"], &s.soffice),
            pandoc: find(&["pandoc"], &s.pandoc),
        }
    }

    pub fn ffmpeg(&self) -> Result<&PathBuf> {
        self.ffmpeg.as_ref().ok_or_else(|| {
            anyhow!("FFmpeg was not found. Install it (Windows: winget install Gyan.FFmpeg, Linux: sudo apt install ffmpeg) or set its path in Settings.")
        })
    }

    pub fn ffprobe(&self) -> Result<&PathBuf> {
        self.ffprobe
            .as_ref()
            .ok_or_else(|| anyhow!("ffprobe was not found (it ships with FFmpeg). Set its path in Settings."))
    }

    pub fn soffice(&self) -> Result<&PathBuf> {
        self.soffice.as_ref().ok_or_else(|| {
            anyhow!("LibreOffice was not found. Install it from libreoffice.org (or sudo apt install libreoffice) or set the soffice path in Settings.")
        })
    }

    pub fn pandoc(&self) -> Result<&PathBuf> {
        self.pandoc
            .as_ref()
            .ok_or_else(|| anyhow!("Pandoc was not found. Install it from pandoc.org or set its path in Settings."))
    }
}

pub fn version(path: &Path, arg: &str) -> Option<String> {
    let out = command(path).arg(arg).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines().next().map(|l| l.trim().chars().take(120).collect())
}
