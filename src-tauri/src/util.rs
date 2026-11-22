use anyhow::Result;
use base64::Engine;
use regex::Regex;
use serde_json::{Map, Value};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn temp_root() -> PathBuf {
    std::env::temp_dir().join("morph")
}

pub fn home_dir() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// A `Command` that never flashes a console window on Windows.
pub fn command<S: AsRef<OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn file_stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn unique_path(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    let stem = file_stem(p);
    let ext = p.extension().map(|s| s.to_string_lossy().into_owned());
    let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
    for i in 1..100_000 {
        let name = match &ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let c = dir.join(name);
        if !c.exists() {
            return c;
        }
    }
    p.to_path_buf()
}

/// Makes a string safe to use as a single file name on Windows and Linux.
pub fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let t = s.trim().trim_end_matches('.').trim().to_string();
    if t.is_empty() || t == "." || t == ".." {
        "file".into()
    } else {
        t
    }
}

/// Sanitizes every segment of a `a/b/c` relative path and drops `.`/`..`.
pub fn clean_rel(rel: &str) -> String {
    rel.split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .map(sanitize)
        .collect::<Vec<_>>()
        .join("/")
}

pub fn join_rel(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}/{b}"),
    }
}

pub fn link_or_copy(src: &Path, dst: &Path) -> Result<()> {
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    if std::fs::hard_link(src, dst).is_err() {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

/// Parses `90`, `1:30`, `00:01:30.5` into seconds.
pub fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut total = 0.0;
    for part in s.split(':') {
        let v: f64 = part.trim().replace(',', ".").parse().ok()?;
        total = total * 60.0 + v;
    }
    Some(total)
}

pub fn parse_color(s: &str) -> [u8; 3] {
    let h = s.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let Ok(v) = u32::from_str_radix(h, 16) {
            return [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        }
    }
    if h.len() == 3 {
        if let Ok(v) = u32::from_str_radix(h, 16) {
            let r = ((v >> 8) & 0xF) as u8;
            let g = ((v >> 4) & 0xF) as u8;
            let b = (v & 0xF) as u8;
            return [r * 17, g * 17, b * 17];
        }
    }
    [255, 255, 255]
}

/// `*.jpg, *.png; photo_??.*` -> case-insensitive regexes.
pub fn glob_set(patterns: &str) -> Vec<Regex> {
    patterns
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let mut re = String::from("(?i)^");
            for c in p.chars() {
                match c {
                    '*' => re.push_str(".*"),
                    '?' => re.push('.'),
                    c => re.push_str(&regex::escape(&c.to_string())),
                }
            }
            re.push('$');
            Regex::new(&re).ok()
        })
        .collect()
}

pub fn glob_match(set: &[Regex], name: &str) -> bool {
    set.iter().any(|r| r.is_match(name))
}

/// Builds a .ico file from PNG-encoded entries (supported since Windows Vista).
pub fn write_ico(entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0u8, 0, 1, 0];
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * entries.len();
    for (size, png) in entries {
        let b = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[b, b, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in entries {
        out.extend_from_slice(png);
    }
    out
}

pub fn open_in_os(p: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        command("explorer").arg(p).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        command("open").arg(p).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        command("xdg-open").arg(p).spawn()?;
    }
    Ok(())
}

pub fn reveal_in_os(p: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = command("explorer");
        c.raw_arg(format!("/select,\"{}\"", p.display()));
        c.spawn()?;
    }
    #[cfg(not(windows))]
    {
        let dir = if p.is_dir() {
            p.to_path_buf()
        } else {
            p.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        open_in_os(&dir)?;
    }
    Ok(())
}

/// Typed access to the widget values the UI sends for a node.
pub struct Params<'a>(pub &'a Map<String, Value>);

impl<'a> Params<'a> {
    pub fn raw(&self, k: &str) -> Option<&Value> {
        self.0.get(k).filter(|v| !v.is_null())
    }

    pub fn str(&self, k: &str, d: &str) -> String {
        match self.raw(k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            _ => d.to_string(),
        }
    }

    pub fn f64(&self, k: &str, d: f64) -> f64 {
        match self.raw(k) {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(d),
            Some(Value::String(s)) => s.trim().replace(',', ".").parse().unwrap_or(d),
            Some(Value::Bool(b)) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => d,
        }
    }

    pub fn i64(&self, k: &str, d: i64) -> i64 {
        self.f64(k, d as f64).round() as i64
    }

    pub fn u32(&self, k: &str, d: u32) -> u32 {
        self.i64(k, d as i64).clamp(0, u32::MAX as i64) as u32
    }

    pub fn bool(&self, k: &str, d: bool) -> bool {
        match self.raw(k) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
            Some(Value::String(s)) => matches!(s.to_lowercase().as_str(), "true" | "1" | "yes" | "on"),
            _ => d,
        }
    }

    pub fn list(&self, k: &str) -> Vec<String> {
        match self.raw(k) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect(),
            Some(Value::String(s)) => s
                .lines()
                .map(|l| l.trim().trim_matches('"').to_string())
                .filter(|l| !l.is_empty())
                .collect(),
            _ => vec![],
        }
    }
}
