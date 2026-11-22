//! One-click downloads of optional tools (FFmpeg, Pandoc) into the app's tools folder,
//! plus a richer tool status for the UI.
use crate::tools::{exe, tools_dir, version, ToolSettings, Tools};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

type CmdResult<T> = std::result::Result<T, String>;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(120))
        .redirects(10)
        .user_agent(concat!("Morph/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn emit(app: &AppHandle, tool: &str, stage: &str, value: f64, text: &str) {
    let _ = app.emit("tool-install", json!({"tool": tool, "stage": stage, "value": value, "text": text}));
}

fn ffmpeg_url() -> Result<String> {
    let base = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest";
    let file = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "aarch64") => "ffmpeg-master-latest-winarm64-gpl.zip",
        ("windows", _) => "ffmpeg-master-latest-win64-gpl.zip",
        ("linux", "aarch64") => "ffmpeg-master-latest-linuxarm64-gpl.tar.xz",
        ("linux", "x86_64") => "ffmpeg-master-latest-linux64-gpl.tar.xz",
        (os, arch) => bail!("No ready-made FFmpeg build for {os}/{arch}. Install it with your package manager."),
    };
    Ok(format!("{base}/{file}"))
}

/// Picks an asset of the latest GitHub release.
fn github_asset(repo: &str, pick: &dyn Fn(&str) -> bool) -> Result<(String, u64)> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| anyhow!("GitHub is not reachable: {e}"))?;
    let v: Value = serde_json::from_reader(resp.into_reader()).context("unexpected GitHub answer")?;
    for a in v["assets"].as_array().cloned().unwrap_or_default() {
        let name = a["name"].as_str().unwrap_or("");
        if pick(name) {
            let url = a["browser_download_url"].as_str().unwrap_or("").to_string();
            return Ok((url, a["size"].as_u64().unwrap_or(0)));
        }
    }
    bail!("No download for this system in the latest {repo} release")
}

fn pandoc_pick(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => n.ends_with("windows-x86_64.zip"),
        ("linux", "aarch64") => n.ends_with("linux-arm64.tar.gz"),
        ("linux", _) => n.ends_with("linux-amd64.tar.gz"),
        _ => false,
    }
}

fn download(app: &AppHandle, tool: &str, url: &str, size_hint: u64, dest: &Path) -> Result<()> {
    let resp = agent().get(url).call().map_err(|e| anyhow!("download failed: {e}"))?;
    let total = resp.header("Content-Length").and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(size_hint);
    let mut reader = resp.into_reader();
    let mut file = File::create(dest).with_context(|| format!("cannot write {}", dest.display()))?;
    let mut buf = vec![0u8; 1 << 16];
    let mut done: u64 = 0;
    let mut last = Instant::now();
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if last.elapsed() > Duration::from_millis(150) {
            last = Instant::now();
            let frac = if total > 0 { done as f64 / total as f64 } else { 0.0 };
            let text = if total > 0 {
                format!("{:.1} / {:.1} MB", done as f64 / 1_048_576.0, total as f64 / 1_048_576.0)
            } else {
                format!("{:.1} MB", done as f64 / 1_048_576.0)
            };
            emit(app, tool, "download", frac * 0.9, &text);
        }
    }
    file.flush()?;
    if total > 0 && done < total {
        bail!("the download was interrupted ({done} of {total} bytes)");
    }
    Ok(())
}

/// Extracts only the wanted executables (by file name, from any sub folder) into `dest`.
fn extract_bins(archive: &Path, wanted: &[&str], dest: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dest)?;
    let names: Vec<String> = wanted.iter().map(|n| exe(n)).collect();
    let is_wanted = |base: &str| names.iter().any(|n| n.eq_ignore_ascii_case(base));
    let lower = archive.to_string_lossy().to_ascii_lowercase();
    let mut found = vec![];
    if lower.ends_with(".zip") {
        let mut z = zip::ZipArchive::new(File::open(archive)?)?;
        for i in 0..z.len() {
            let mut f = z.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let base = f.name().rsplit(['/', '\\']).next().unwrap_or("").to_string();
            if is_wanted(&base) {
                let out = dest.join(&base);
                let mut w = File::create(&out)?;
                std::io::copy(&mut f, &mut w)?;
                found.push(out);
            }
        }
    } else {
        let file = File::open(archive)?;
        let reader: Box<dyn Read> = if lower.ends_with(".xz") {
            Box::new(xz2::read::XzDecoder::new(file))
        } else {
            Box::new(flate2::read::GzDecoder::new(file))
        };
        let mut t = tar::Archive::new(reader);
        for e in t.entries()? {
            let mut e = e?;
            if !e.header().entry_type().is_file() {
                continue;
            }
            let base = e.path()?.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if is_wanted(&base) {
                let out = dest.join(&base);
                e.unpack(&out)?;
                found.push(out);
            }
        }
    }
    #[cfg(unix)]
    for p in &found {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
    }
    Ok(found)
}

fn install(app: &AppHandle, name: &str) -> Result<Value> {
    let root = tools_dir();
    std::fs::create_dir_all(&root).with_context(|| format!("cannot create {}", root.display()))?;
    emit(app, name, "download", 0.0, "Connecting\u{2026}");
    let (url, size, wanted, main): (String, u64, Vec<&str>, &str) = match name {
        "ffmpeg" => (ffmpeg_url()?, 0, vec!["ffmpeg", "ffprobe"], "ffmpeg"),
        "pandoc" => {
            let (u, s) = github_asset("jgm/pandoc", &pandoc_pick)?;
            (u, s, vec!["pandoc"], "pandoc")
        }
        other => bail!("{other} can't be installed automatically. Use the link in Settings \u{2192} Tools."),
    };
    let file_name = url.rsplit('/').next().unwrap_or("download").to_string();
    let archive = root.join(format!("_{file_name}"));
    let res = (|| -> Result<Value> {
        download(app, name, &url, size, &archive)?;
        emit(app, name, "extract", 0.92, "Unpacking\u{2026}");
        let dest = root.join(name);
        if dest.exists() {
            let _ = std::fs::remove_dir_all(&dest);
        }
        let found = extract_bins(&archive, &wanted, &dest)?;
        let bin = found
            .iter()
            .find(|p| crate::util::file_stem(p).eq_ignore_ascii_case(main))
            .cloned()
            .ok_or_else(|| anyhow!("{main} was not found inside the download"))?;
        emit(app, name, "verify", 0.97, "Checking\u{2026}");
        let arg = if main == "ffmpeg" { "-version" } else { "--version" };
        let ver = version(&bin, arg).ok_or_else(|| anyhow!("the downloaded {main} does not start on this system"))?;
        emit(app, name, "done", 1.0, &ver);
        Ok(json!({"path": bin, "version": ver}))
    })();
    let _ = std::fs::remove_file(&archive);
    if let Err(e) = &res {
        emit(app, name, "error", 0.0, &format!("{:#}", e));
    }
    res
}

#[tauri::command]
pub async fn install_tool(app: AppHandle, name: String) -> CmdResult<Value> {
    tauri::async_runtime::spawn_blocking(move || install(&app, &name))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{:#}", e))
}

#[tauri::command]
pub async fn remove_tool(name: String) -> CmdResult<()> {
    let dir = tools_dir().join(crate::util::sanitize(&name));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_tools_folder() -> CmdResult<String> {
    let d = tools_dir();
    let _ = std::fs::create_dir_all(&d);
    crate::util::open_in_os(&d).map_err(|e| e.to_string())?;
    Ok(d.to_string_lossy().into_owned())
}

/// Every optional tool with path, version and whether Morph can install it in one click.
#[tauri::command]
pub async fn tool_status(settings: Option<ToolSettings>) -> CmdResult<Value> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Value> {
        let t = Tools::resolve(&settings.unwrap_or_default());
        let managed_root = tools_dir();
        let info = |p: &Option<PathBuf>, arg: Option<&str>, installable: bool| match p {
            Some(p) => json!({
                "path": p,
                "version": arg.and_then(|a| version(p, a)),
                "managed": p.starts_with(&managed_root),
                "installable": installable
            }),
            None => json!({"path": Value::Null, "version": Value::Null, "managed": false, "installable": installable}),
        };
        Ok(json!({
            "dir": managed_root,
            "tools": {
                "ffmpeg": info(&t.ffmpeg, Some("-version"), true),
                "ffprobe": info(&t.ffprobe, Some("-version"), true),
                "pandoc": info(&t.pandoc, Some("--version"), true),
                "soffice": info(&t.soffice, None, false),
                "gs": info(&t.gs, Some("--version"), false),
                "pdftoppm": info(&t.pdftoppm, None, false),
                "sevenzip": info(&t.sevenzip, None, false)
            }
        }))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{:#}", e))
}
