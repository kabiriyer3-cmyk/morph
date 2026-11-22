//! Locates optional external tools (FFmpeg, LibreOffice, Pandoc, Ghostscript, Poppler, 7-Zip).
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
    pub gs: String,
    pub sevenzip: String,
}

#[derive(Default, Clone, Debug)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub soffice: Option<PathBuf>,
    pub pandoc: Option<PathBuf>,
    pub gs: Option<PathBuf>,
    pub pdftoppm: Option<PathBuf>,
    pub sevenzip: Option<PathBuf>,
}

pub fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Folder for one-click installs (FFmpeg, Pandoc): %LOCALAPPDATA%\Morph\tools or ~/.local/share/morph/tools.
pub fn tools_dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty());
    if cfg!(windows) {
        env("LOCALAPPDATA").unwrap_or_else(crate::util::home_dir).join("Morph").join("tools")
    } else {
        env("XDG_DATA_HOME")
            .unwrap_or_else(|| crate::util::home_dir().join(".local").join("share"))
            .join("morph")
            .join("tools")
    }
}

/// `<root>/<version dir>/<sub>` for every version folder, newest first (e.g. Program Files\gs\gs10.04.0\bin).
fn versioned(root: &Path, sub: &str) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.reverse();
    dirs.into_iter().map(|d| if sub.is_empty() { d } else { d.join(sub) }).collect()
}

fn search_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    let td = tools_dir();
    for s in ["ffmpeg", "pandoc", "7zip", "gs"] {
        v.push(td.join(s));
    }
    v.push(td);
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
        for k in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(p) = env(k) {
                v.push(p.join("ffmpeg").join("bin"));
                v.push(p.join("LibreOffice").join("program"));
                v.push(p.join("Pandoc"));
                v.push(p.join("7-Zip"));
                v.extend(versioned(&p.join("gs"), "bin"));
                v.extend(versioned(&p.join("poppler"), "Library\\bin"));
            }
        }
        if let Some(p) = env("LOCALAPPDATA") {
            v.push(p.join("Microsoft").join("WinGet").join("Links"));
            v.push(p.join("Pandoc"));
            v.push(p.join("Programs").join("ffmpeg").join("bin"));
            v.push(p.join("Programs").join("LibreOffice").join("program"));
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
        v.extend(versioned(Path::new("/opt"), "program").into_iter().filter(|p| p.to_string_lossy().contains("libreoffice")));
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
                for q in [p.join(exe(n)), p.join("bin").join(exe(n)), p.join("program").join(exe(n))] {
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

pub const NO_FFMPEG: &str = "FFmpeg is not installed. Click \u{201c}Install\u{201d} next to FFmpeg in the top bar or in Settings \u{2192} Tools (one click, about 100 MB), or set its path manually.";
pub const NO_PANDOC: &str = "Pandoc is not installed. Click \u{201c}Install\u{201d} next to Pandoc in Settings \u{2192} Tools (one click), or set its path manually.";
pub const NO_SOFFICE: &str = "LibreOffice is not installed. Get it from libreoffice.org (Windows: winget install TheDocumentFoundation.LibreOffice, Linux: sudo apt install libreoffice); Morph finds it automatically.";
pub const NO_RENDERER: &str = "Rendering PDF pages needs Ghostscript or Poppler (Windows: winget install ArtifexSoftware.GhostScript, Linux: sudo apt install ghostscript poppler-utils).";
pub const NO_GS: &str = "This needs Ghostscript (Windows: winget install ArtifexSoftware.GhostScript, Linux: sudo apt install ghostscript).";
pub const NO_7Z: &str = "This archive type needs 7-Zip (Windows: winget install 7zip.7zip, Linux: sudo apt install 7zip or p7zip-full).";

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
            gs: find(&["gswin64c", "gswin32c", "gs"], &s.gs),
            pdftoppm: find(&["pdftoppm"], ""),
            sevenzip: find(&["7z", "7zz", "7za"], &s.sevenzip),
        }
    }

    pub fn ffmpeg(&self) -> Result<&PathBuf> {
        self.ffmpeg.as_ref().ok_or_else(|| anyhow!(NO_FFMPEG))
    }

    pub fn ffprobe(&self) -> Result<&PathBuf> {
        self.ffprobe
            .as_ref()
            .ok_or_else(|| anyhow!("ffprobe was not found (it ships with FFmpeg). {NO_FFMPEG}"))
    }

    pub fn soffice(&self) -> Result<&PathBuf> {
        self.soffice.as_ref().ok_or_else(|| anyhow!(NO_SOFFICE))
    }

    pub fn pandoc(&self) -> Result<&PathBuf> {
        self.pandoc.as_ref().ok_or_else(|| anyhow!(NO_PANDOC))
    }

    pub fn gs(&self) -> Result<&PathBuf> {
        self.gs.as_ref().ok_or_else(|| anyhow!(NO_GS))
    }

    pub fn sevenzip(&self) -> Result<&PathBuf> {
        self.sevenzip.as_ref().ok_or_else(|| anyhow!(NO_7Z))
    }
}

pub fn version(path: &Path, arg: &str) -> Option<String> {
    let out = command(path).arg(arg).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(120).collect())
}
