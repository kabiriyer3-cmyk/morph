//! Executes a node graph sent by the UI on a worker thread and streams events back.
use crate::kinds;
use crate::ops;
use crate::tools::{ToolSettings, Tools};
use crate::util::{clean_rel, sanitize, temp_root};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Deserialize, Clone, Debug)]
pub struct InputRef {
    pub node: String,
    pub slot: usize,
}

fn one() -> usize {
    1
}

#[derive(Deserialize, Clone, Debug)]
pub struct NodeSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub values: Map<String, Value>,
    #[serde(default)]
    pub inputs: Vec<Option<InputRef>>,
    #[serde(default = "one")]
    pub outputs: usize,
    #[serde(default)]
    pub bypass: bool,
    #[serde(default)]
    pub is_output: bool,
    #[serde(default)]
    pub title: String,
}

#[derive(Deserialize, Debug)]
pub struct GraphSpec {
    pub nodes: Vec<NodeSpec>,
    #[serde(default)]
    pub settings: ToolSettings,
}

/// One file flowing through the graph.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FileItem {
    pub path: String,
    pub name: String,
    pub ext: String,
    /// Logical sub folder (relative to the dropped folder), kept by Save.
    pub rel: String,
    pub kind: String,
    pub size: u64,
    /// The original source file this item was derived from.
    pub origin: String,
    pub origin_size: u64,
}

impl FileItem {
    pub fn from_path(p: &Path, rel: &str) -> FileItem {
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let ext = kinds::ext_of(p);
        let path = p.to_string_lossy().into_owned();
        FileItem {
            name: crate::util::file_name(p),
            kind: kinds::kind_of(&ext).to_string(),
            ext,
            rel: rel.to_string(),
            size,
            origin: path.clone(),
            origin_size: size,
            path,
        }
    }

    /// A new file produced from this one: keeps `rel` and `origin`.
    pub fn derive(&self, p: &Path) -> FileItem {
        let mut f = FileItem::from_path(p, &self.rel);
        f.origin = self.origin.clone();
        f.origin_size = self.origin_size;
        f
    }

    pub fn path_buf(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }

    pub fn stem(&self) -> String {
        Path::new(&self.name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.name.clone())
    }
}

#[derive(Default)]
pub struct RunState {
    pub cancel: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
}

/// Per-node execution context.
pub struct Ctx<'a> {
    pub app: &'a AppHandle,
    pub node_id: String,
    pub work: PathBuf,
    pub tools: &'a Tools,
    pub cancel: Arc<AtomicBool>,
    warnings: Mutex<Vec<String>>,
    last: Mutex<Instant>,
    reserved: Mutex<HashSet<PathBuf>>,
}

impl<'a> Ctx<'a> {
    pub fn new(app: &'a AppHandle, id: &str, work: PathBuf, tools: &'a Tools, cancel: Arc<AtomicBool>) -> Self {
        let now = Instant::now();
        Ctx {
            app,
            node_id: id.to_string(),
            work,
            tools,
            cancel,
            warnings: Mutex::new(vec![]),
            last: Mutex::new(now.checked_sub(Duration::from_secs(1)).unwrap_or(now)),
            reserved: Mutex::new(HashSet::new()),
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn check(&self) -> Result<()> {
        if self.cancelled() {
            bail!("Cancelled");
        }
        Ok(())
    }

    pub fn warn(&self, msg: String) {
        if let Ok(mut w) = self.warnings.lock() {
            if w.len() < 200 {
                w.push(msg);
            }
        }
    }

    pub fn first_warning(&self) -> String {
        self.warnings.lock().ok().and_then(|w| w.first().cloned()).unwrap_or_default()
    }

    pub fn take_warnings(&self) -> Vec<String> {
        self.warnings.lock().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default()
    }

    /// Progress of the current node, 0..1 (throttled).
    pub fn report(&self, value: f64, text: &str) {
        let now = Instant::now();
        if let Ok(mut last) = self.last.lock() {
            if value < 0.999 && now.duration_since(*last) < Duration::from_millis(120) {
                return;
            }
            *last = now;
        }
        let _ = self.app.emit(
            "run-event",
            json!({"type": "node_progress", "node": self.node_id, "value": value.clamp(0.0, 1.0), "text": text}),
        );
    }

    pub fn progress(&self, done: usize, total: usize, text: &str) {
        let v = if total == 0 { 1.0 } else { done as f64 / total as f64 };
        self.report(v, text);
    }

    pub fn out_dir(&self, rel: &str) -> Result<PathBuf> {
        let rel = clean_rel(rel);
        let d = if rel.is_empty() { self.work.clone() } else { self.work.join(rel) };
        std::fs::create_dir_all(&d)?;
        Ok(d)
    }

    /// A free output path inside this node's work dir, mirroring the item's sub folder.
    pub fn out_path(&self, item: &FileItem, stem: &str, ext: &str) -> Result<PathBuf> {
        let dir = self.out_dir(&item.rel)?;
        self.reserve(&dir, stem, ext)
    }

    pub fn reserve(&self, dir: &Path, stem: &str, ext: &str) -> Result<PathBuf> {
        let base = sanitize(stem);
        let mut reserved = self.reserved.lock().map_err(|_| anyhow!("internal lock error"))?;
        for i in 0..100_000u32 {
            let name = match (i, ext.is_empty()) {
                (0, true) => base.clone(),
                (0, false) => format!("{base}.{ext}"),
                (_, true) => format!("{base} ({i})"),
                (_, false) => format!("{base} ({i}).{ext}"),
            };
            let p = dir.join(name);
            if !p.exists() && !reserved.contains(&p) {
                reserved.insert(p.clone());
                return Ok(p);
            }
        }
        bail!("could not find a free file name in {}", dir.display())
    }
}

fn emit(app: &AppHandle, v: Value) {
    let _ = app.emit("run-event", v);
}

pub fn start(app: AppHandle, state: &RunState, graph: GraphSpec) -> Result<String> {
    if state.running.swap(true, Ordering::SeqCst) {
        bail!("A run is already in progress");
    }
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let running = state.running.clone();
    let run_id = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f").to_string();
    let id = run_id.clone();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        emit(&app, json!({"type": "started", "run": id}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute(&app, &graph, &id, cancel.clone())));
        let (ok, error) = match res {
            Ok(Ok(())) => (true, None),
            Ok(Err(e)) => (false, Some(format!("{:#}", e))),
            Err(_) => (false, Some("Internal error (panic)".to_string())),
        };
        emit(
            &app,
            json!({
                "type": "finished",
                "run": id,
                "ok": ok,
                "error": error,
                "cancelled": cancel.load(Ordering::SeqCst),
                "ms": t0.elapsed().as_millis() as u64
            }),
        );
        running.store(false, Ordering::SeqCst);
    });
    Ok(run_id)
}

fn cleanup_runs(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut names: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    names.sort();
    names.reverse();
    for old in names.into_iter().skip(keep.saturating_sub(1)) {
        let _ = std::fs::remove_dir_all(old);
    }
}

fn topo<'a>(by_id: &HashMap<&'a str, &'a NodeSpec>, targets: &[&'a str]) -> Result<Vec<&'a str>> {
    fn visit<'a>(
        id: &'a str,
        by_id: &HashMap<&'a str, &'a NodeSpec>,
        state: &mut HashMap<&'a str, u8>,
        order: &mut Vec<&'a str>,
    ) -> Result<()> {
        match state.get(id) {
            Some(2) => return Ok(()),
            Some(1) => bail!("The graph contains a loop"),
            _ => {}
        }
        let node: &'a NodeSpec = by_id.get(id).copied().ok_or_else(|| anyhow!("Missing node {id}"))?;
        state.insert(id, 1);
        for r in node.inputs.iter().flatten() {
            let src: &'a str = r.node.as_str();
            if by_id.contains_key(src) {
                visit(src, by_id, state, order)?;
            }
        }
        state.insert(id, 2);
        order.push(id);
        Ok(())
    }
    let mut order = vec![];
    let mut state = HashMap::new();
    for t in targets {
        visit(t, by_id, &mut state, &mut order)?;
    }
    Ok(order)
}

fn execute(app: &AppHandle, graph: &GraphSpec, run_id: &str, cancel: Arc<AtomicBool>) -> Result<()> {
    let tools = Tools::resolve(&graph.settings);
    let runs = temp_root().join("runs");
    cleanup_runs(&runs, 4);
    let run_dir = runs.join(run_id);
    std::fs::create_dir_all(&run_dir)?;

    let by_id: HashMap<&str, &NodeSpec> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let targets: Vec<&str> = graph.nodes.iter().filter(|n| n.is_output).map(|n| n.id.as_str()).collect();
    if targets.is_empty() {
        bail!("Nothing to run: add a Save or Preview node and connect it.");
    }
    let order = topo(&by_id, &targets)?;
    let mut results: HashMap<String, Vec<Vec<FileItem>>> = HashMap::new();

    for id in order {
        if cancel.load(Ordering::SeqCst) {
            bail!("Cancelled");
        }
        let node = by_id.get(id).copied().ok_or_else(|| anyhow!("Missing node {id}"))?;
        let inputs: Vec<Vec<FileItem>> = node
            .inputs
            .iter()
            .map(|i| match i {
                Some(r) => results.get(&r.node).and_then(|o| o.get(r.slot)).cloned().unwrap_or_default(),
                None => vec![],
            })
            .collect();
        let t = Instant::now();
        emit(app, json!({"type": "node_start", "node": id}));
        let ctx = Ctx::new(app, id, run_dir.join(sanitize(id)), &tools, cancel.clone());
        let out = if node.bypass {
            let all: Vec<FileItem> = inputs.into_iter().flatten().collect();
            (0..node.outputs.max(1)).map(|_| all.clone()).collect::<Vec<_>>()
        } else {
            match ops::run(&ctx, node, inputs) {
                Ok(o) => o,
                Err(e) => {
                    let msg = format!("{:#}", e);
                    emit(app, json!({"type": "node_error", "node": id, "error": msg, "warnings": ctx.take_warnings()}));
                    let title = if node.title.is_empty() { node.kind.clone() } else { node.title.clone() };
                    return Err(anyhow!("{}: {}", title, msg));
                }
            }
        };
        let counts: Vec<usize> = out.iter().map(|o| o.len()).collect();
        let outputs: Vec<Vec<&FileItem>> = out.iter().map(|o| o.iter().take(500).collect()).collect();
        emit(
            app,
            json!({
                "type": "node_done",
                "node": id,
                "ms": t.elapsed().as_millis() as u64,
                "counts": counts,
                "outputs": outputs,
                "warnings": ctx.take_warnings()
            }),
        );
        results.insert(id.to_string(), out);
    }
    Ok(())
}
