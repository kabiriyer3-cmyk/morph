//! Node implementations. Every node takes the files from its inputs and returns
//! one list of files per output socket.
pub mod arch;
pub mod data;
pub mod docs;
pub mod files;
pub mod img;
pub mod media;
pub mod mesh3d;

use crate::engine::{Ctx, FileItem, NodeSpec};
use crate::util::Params;
use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

pub type Outs = Vec<Vec<FileItem>>;

pub fn run(ctx: &Ctx, node: &NodeSpec, inputs: Vec<Vec<FileItem>>) -> Result<Outs> {
    let p = Params(&node.values);
    let all: Vec<FileItem> = inputs.iter().flatten().cloned().collect();
    let one = |v: Vec<FileItem>| -> Result<Outs> { Ok(vec![v]) };
    match node.kind.as_str() {
        "files.load" => one(files::load(ctx, &p)?),
        "files.save" => one(files::save(ctx, &p, all)?),
        "files.preview" => one(all),
        "files.filter" => files::filter(&p, all),
        "files.split" => Ok(files::split(all)),
        "files.merge" => one(files::merge(all)),
        "files.rename" => one(files::rename(ctx, &p, all)?),
        "files.hash" => files::hash(ctx, &p, all),
        "files.dedupe" => files::dedupe(ctx, &p, all),
        "files.info" => files::info(ctx, &p, all),

        "image.resize" => one(img::resize(ctx, &p, all)?),
        "image.crop" => one(img::crop(ctx, &p, all)?),
        "image.transform" => one(img::transform(ctx, &p, all)?),
        "image.adjust" => one(img::adjust(ctx, &p, all)?),
        "image.convert" => one(img::convert(ctx, &p, all)?),
        "image.watermark" => one(img::watermark(ctx, &p, all)?),
        "image.sheet" => one(img::sheet(ctx, &p, all)?),
        "image.icons" => one(img::icons(ctx, &p, all)?),
        "image.pdf" => one(img::to_pdf(ctx, &p, all)?),

        "video.convert" => one(media::video_convert(ctx, &p, all)?),
        "video.trim" => one(media::video_trim(ctx, &p, all)?),
        "video.transform" => one(media::video_transform(ctx, &p, all)?),
        "video.gif" => one(media::video_gif(ctx, &p, all)?),
        "video.frames" => one(media::video_frames(ctx, &p, all)?),
        "video.audio" => one(media::video_audio(ctx, &p, all)?),
        "video.concat" => one(media::video_concat(ctx, &p, all)?),
        "audio.convert" => one(media::audio_convert(ctx, &p, all)?),
        "audio.process" => one(media::audio_process(ctx, &p, all)?),

        "mesh.convert" => one(mesh3d::convert(ctx, &p, all)?),
        "mesh.transform" => one(mesh3d::transform(ctx, &p, all)?),
        "mesh.optimize" => one(mesh3d::optimize(ctx, &p, all)?),

        "archive.create" => one(arch::create(ctx, &p, all)?),
        "archive.extract" => one(arch::extract(ctx, &p, all)?),
        "archive.compress" => one(arch::compress(ctx, &p, all)?),

        "data.convert" => one(data::convert(ctx, &p, all)?),
        "text.transform" => one(data::text_transform(ctx, &p, all)?),
        "doc.office" => one(docs::office(ctx, &p, all)?),
        "doc.pandoc" => one(docs::pandoc(ctx, &p, all)?),

        "util.note" => Ok(vec![]),
        other => bail!("Unknown node type '{other}'"),
    }
}

/// Splits files into (matching `kinds`, everything else). Empty `kinds` matches all.
pub fn split_kinds(files: Vec<FileItem>, kinds: &[&str]) -> (Vec<FileItem>, Vec<FileItem>) {
    files.into_iter().partition(|it| kinds.is_empty() || kinds.contains(&it.kind.as_str()))
}

/// Runs `f` for every file of the given kinds (optionally in parallel).
/// Files of other kinds pass through untouched; per-file failures become warnings.
pub fn batch<F>(ctx: &Ctx, files: Vec<FileItem>, kinds: &[&str], parallel: bool, f: F) -> Result<Vec<FileItem>>
where
    F: Fn(&FileItem) -> Result<Vec<FileItem>> + Sync + Send,
{
    let (work, pass) = split_kinds(files, kinds);
    let total = work.len();
    if total == 0 {
        if !pass.is_empty() {
            ctx.warn(format!(
                "No {} files in the input: {} file(s) passed through unchanged",
                kinds.join("/"),
                pass.len()
            ));
        }
        return Ok(pass);
    }
    let done = AtomicUsize::new(0);
    let one = |it: &FileItem| -> Option<Vec<FileItem>> {
        if ctx.cancelled() {
            return None;
        }
        let r = f(it);
        let d = done.fetch_add(1, Ordering::SeqCst) + 1;
        ctx.progress(d, total, &it.name);
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                ctx.warn(format!("{}: {:#}", it.name, e));
                None
            }
        }
    };
    let res: Vec<Option<Vec<FileItem>>> = if parallel {
        work.par_iter().map(one).collect()
    } else {
        work.iter().map(one).collect()
    };
    ctx.check()?;
    let mut out: Vec<FileItem> = res.into_iter().flatten().flatten().collect();
    if out.is_empty() {
        bail!("All {} file(s) failed. {}", total, ctx.first_warning());
    }
    out.extend(pass);
    Ok(out)
}

/// Runs an external tool, killing it when the run is cancelled.
pub fn run_tool(ctx: &Ctx, cmd: &mut Command, what: &str) -> Result<()> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().with_context(|| format!("failed to start {what}"))?;
    let mut err = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    loop {
        if let Some(status) = child.try_wait()? {
            let e = reader.join().unwrap_or_default();
            if !status.success() {
                let tail: Vec<&str> = e.lines().filter(|l| !l.trim().is_empty()).collect();
                let msg = tail[tail.len().saturating_sub(3)..].join(" | ");
                bail!("{what} failed: {}", if msg.is_empty() { format!("exit code {:?}", status.code()) } else { msg });
            }
            return Ok(());
        }
        if ctx.cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Cancelled");
        }
        std::thread::sleep(Duration::from_millis(80));
    }
}
