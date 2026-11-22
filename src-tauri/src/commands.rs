//! Tauri commands invoked from the UI.
use crate::engine::{self, FileItem, GraphSpec, RunState};
use crate::tools::{version, ToolSettings, Tools};
use crate::util::{self, file_name};
use crate::{kinds, mesh, ops};
use anyhow::{anyhow, bail};
use image::GenericImageView;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

type CmdResult<T> = Result<T, String>;

async fn blocking<T, F>(f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{:#}", e))
}

#[tauri::command]
pub fn system_info() -> Value {
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "version": env!("CARGO_PKG_VERSION"),
        "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        "temp": util::temp_root(),
        "home": util::home_dir(),
    })
}

#[tauri::command]
pub async fn detect_tools(settings: Option<ToolSettings>) -> CmdResult<Value> {
    blocking(move || {
        let t = Tools::resolve(&settings.unwrap_or_default());
        let info = |p: &Option<PathBuf>, arg: Option<&str>| match p {
            Some(p) => json!({"path": p, "version": arg.and_then(|a| version(p, a))}),
            None => Value::Null,
        };
        Ok(json!({
            "ffmpeg": info(&t.ffmpeg, Some("-version")),
            "ffprobe": info(&t.ffprobe, Some("-version")),
            "soffice": info(&t.soffice, None),
            "pandoc": info(&t.pandoc, Some("--version")),
        }))
    })
    .await
}

#[tauri::command]
pub async fn scan_paths(
    paths: Vec<String>,
    recursive: Option<bool>,
    include: Option<String>,
    exclude: Option<String>,
) -> CmdResult<Vec<FileItem>> {
    blocking(move || {
        Ok(ops::files::scan(
            &paths,
            recursive.unwrap_or(true),
            &include.unwrap_or_default(),
            &exclude.unwrap_or_default(),
            true,
        ))
    })
    .await
}

fn ffmpeg_capture(tools: &Tools, args: &[String]) -> anyhow::Result<Vec<u8>> {
    let out = util::command(tools.ffmpeg()?).args(args).output()?;
    if out.stdout.is_empty() {
        bail!("ffmpeg: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("no output"));
    }
    Ok(out.stdout)
}

fn make_thumb(p: &Path, size: u32, settings: &ToolSettings) -> anyhow::Result<Value> {
    let ext = kinds::ext_of(p);
    let kind = kinds::kind_of(&ext);
    match kind {
        "image" => {
            let img = if ext == "svg" { ops::img::render_svg(p, size * 2)? } else { ops::img::open(p)? };
            let (w, h) = img.dimensions();
            let t = img.thumbnail(size, size).to_rgba8();
            let mem = webp::Encoder::from_rgba(t.as_raw(), t.width(), t.height())
                .encode_simple(false, 82.0)
                .map_err(|e| anyhow!("webp: {:?}", e))?;
            Ok(json!({"kind": kind, "thumb": util::data_url("image/webp", &mem), "width": w, "height": h}))
        }
        "video" => {
            let tools = Tools::resolve(settings);
            let info = ops::media::media_info(&tools, p);
            let at = if info.duration > 0.0 { (info.duration * 0.1).min(5.0) } else { 0.0 };
            let args: Vec<String> = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-ss".into(),
                format!("{at:.2}"),
                "-i".into(),
                p.to_string_lossy().into_owned(),
                "-frames:v".into(),
                "1".into(),
                "-vf".into(),
                format!("scale='min({size},iw)':-2"),
                "-f".into(),
                "image2pipe".into(),
                "-vcodec".into(),
                "mjpeg".into(),
                "pipe:1".into(),
            ];
            let jpg = ffmpeg_capture(&tools, &args)?;
            Ok(json!({
                "kind": "video",
                "thumb": util::data_url("image/jpeg", &jpg),
                "width": info.width,
                "height": info.height,
                "duration": info.duration
            }))
        }
        "audio" => {
            let tools = Tools::resolve(settings);
            let info = ops::media::media_info(&tools, p);
            let args: Vec<String> = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-i".into(),
                p.to_string_lossy().into_owned(),
                "-filter_complex".into(),
                format!("showwavespic=s={}x{}:colors=0x81C784", size * 2, size / 2),
                "-frames:v".into(),
                "1".into(),
                "-f".into(),
                "image2pipe".into(),
                "-vcodec".into(),
                "png".into(),
                "pipe:1".into(),
            ];
            let png = ffmpeg_capture(&tools, &args)?;
            Ok(json!({"kind": "audio", "thumb": util::data_url("image/png", &png), "duration": info.duration}))
        }
        _ => Ok(json!({"kind": kind, "thumb": Value::Null})),
    }
}

#[tauri::command]
pub async fn thumbnail(path: String, size: Option<u32>, settings: Option<ToolSettings>) -> CmdResult<Value> {
    blocking(move || {
        let s = size.unwrap_or(256).clamp(32, 1024);
        make_thumb(Path::new(&path), s, &settings.unwrap_or_default())
    })
    .await
}

#[tauri::command]
pub async fn probe(path: String, settings: Option<ToolSettings>) -> CmdResult<Value> {
    blocking(move || {
        let p = PathBuf::from(&path);
        let meta = std::fs::metadata(&p)?;
        let ext = kinds::ext_of(&p);
        let kind = kinds::kind_of(&ext);
        let modified = meta
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M").to_string());
        let mut v = json!({
            "path": path,
            "name": file_name(&p),
            "ext": ext,
            "kind": kind,
            "size": meta.len(),
            "modified": modified
        });
        match kind {
            "image" => {
                if ext != "svg" {
                    if let Ok((w, h)) = image::image_dimensions(&p) {
                        v["width"] = json!(w);
                        v["height"] = json!(h);
                    }
                }
            }
            "video" | "audio" => {
                let tools = Tools::resolve(&settings.unwrap_or_default());
                if let Ok(j) = ops::media::ffprobe_json(&tools, &p) {
                    v["media"] = j;
                }
            }
            "mesh" => {
                if let Ok(m) = mesh::load(&p) {
                    v["mesh"] = m.stats();
                }
            }
            _ => {}
        }
        Ok(v)
    })
    .await
}

#[tauri::command]
pub async fn mesh_preview(path: String, limit: Option<usize>) -> CmdResult<Value> {
    blocking(move || {
        let mut m = mesh::load(Path::new(&path))?;
        let original = m.stats();
        let limit = limit.unwrap_or(400_000).max(1000);
        let simplified = m.tri_count() > limit;
        if simplified {
            m.simplify(limit);
        }
        if m.normals.len() != m.positions.len() {
            m.compute_normals();
        }
        let flat = |v: &Vec<[f32; 3]>| v.iter().flat_map(|p| p.iter().copied()).collect::<Vec<f32>>();
        Ok(json!({
            "positions": flat(&m.positions),
            "normals": flat(&m.normals),
            "colors": flat(&m.colors),
            "indices": m.indices,
            "stats": original,
            "simplified": simplified
        }))
    })
    .await
}

#[tauri::command]
pub async fn read_text(path: String, limit: Option<usize>) -> CmdResult<Value> {
    blocking(move || {
        use std::io::Read;
        let limit = limit.unwrap_or(256 * 1024);
        let f = std::fs::File::open(&path)?;
        let total = f.metadata().map(|m| m.len()).unwrap_or(0);
        let mut buf = Vec::new();
        f.take(limit as u64).read_to_end(&mut buf)?;
        let (text, enc) = ops::data::decode(&buf, "auto")?;
        Ok(json!({"text": text, "truncated": total > buf.len() as u64, "encoding": enc.name(), "size": total}))
    })
    .await
}

#[tauri::command]
pub async fn write_text(path: String, content: String) -> CmdResult<()> {
    blocking(move || {
        std::fs::write(&path, content)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_archive(path: String) -> CmdResult<Value> {
    blocking(move || ops::arch::list(Path::new(&path))).await
}

#[tauri::command]
pub fn open_path(path: String) -> CmdResult<()> {
    util::open_in_os(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_path(path: String) -> CmdResult<()> {
    util::reveal_in_os(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_cache(state: State<'_, RunState>) -> CmdResult<()> {
    if state.running.load(Ordering::SeqCst) {
        return Err("Wait until the current run finishes".into());
    }
    blocking(move || {
        let root = util::temp_root();
        if root.exists() {
            std::fs::remove_dir_all(&root)?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn run_graph(app: AppHandle, state: State<'_, RunState>, graph: GraphSpec) -> CmdResult<String> {
    engine::start(app, &state, graph).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
pub fn cancel_run(state: State<'_, RunState>) {
    state.cancel.store(true, Ordering::SeqCst);
}
