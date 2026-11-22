//! Node implementations. Every node takes the files from its inputs and returns
//! one list of files per output socket.
pub mod arch;
pub mod arch2;
pub mod audiofx;
pub mod av;
pub mod convert;
pub mod data;
pub mod docs;
pub mod files;
pub mod files2;
pub mod img;
pub mod img2;
pub mod media;
pub mod mesh3d;
pub mod pdftools;
pub mod sheet;
pub mod text2;

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
    let input = |i: usize| inputs.get(i).cloned().unwrap_or_default();
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
        "files.organize" => one(files2::organize(ctx, &p, all)?),
        "files.sort" => one(files2::sort(ctx, &p, all)?),
        "files.clean_names" => one(files2::clean_names(ctx, &p, all)?),
        "files.parts" => one(files2::parts(ctx, &p, all)?),
        "files.verify" => files2::verify(ctx, &p, all),
        "files.report" => files2::report(ctx, &p, all),

        "convert.any" => one(convert::convert_any(ctx, &p, all)?),

        "image.resize" => one(img::resize(ctx, &p, all)?),
        "image.crop" => one(img::crop(ctx, &p, all)?),
        "image.transform" => one(img::transform(ctx, &p, all)?),
        "image.adjust" => one(img::adjust(ctx, &p, all)?),
        "image.convert" => one(img::convert(ctx, &p, all)?),
        "image.watermark" => one(img::watermark(ctx, &p, all)?),
        "image.sheet" => one(img::sheet(ctx, &p, all)?),
        "image.icons" => one(img::icons(ctx, &p, all)?),
        "image.pdf" => one(img::to_pdf(ctx, &p, all)?),
        "image.frame" => one(img2::frame(ctx, &p, all)?),
        "image.effects" => one(img2::effects(ctx, &p, all)?),
        "image.text" => one(img2::text(ctx, &p, all)?),
        "image.metadata" => img2::metadata(ctx, &p, all),
        "image.tiles" => one(img2::tiles(ctx, &p, all)?),
        "image.collage" => one(img2::collage(ctx, &p, all)?),
        "image.palette" => one(img2::palette(ctx, &p, all)?),
        "image.qr" => one(img2::qr(ctx, &p, all)?),
        "image.base64" => one(img2::base64(ctx, &p, all)?),
        "image.similar" => img2::similar(ctx, &p, all),
        "image.remove_bg" => one(img2::remove_bg(ctx, &p, all)?),
        "image.animate" => one(img2::animate(ctx, &p, all)?),
        "image.frames" => one(img2::frames(ctx, &p, all)?),

        "video.convert" => one(av::video_convert(ctx, &p, all)?),
        "video.trim" => one(media::video_trim(ctx, &p, all)?),
        "video.transform" => one(media::video_transform(ctx, &p, all)?),
        "video.gif" => one(media::video_gif(ctx, &p, all)?),
        "video.frames" => one(media::video_frames(ctx, &p, all)?),
        "video.audio" => one(media::video_audio(ctx, &p, all)?),
        "video.concat" => one(media::video_concat(ctx, &p, all)?),
        "video.split" => one(av::video_split(ctx, &p, all)?),
        "video.fx" => one(av::video_fx(ctx, &p, all)?),
        "video.filters" => one(av::video_filters(ctx, &p, all)?),
        "video.framerate" => one(av::video_framerate(ctx, &p, all)?),
        "video.watermark" => one(av::video_watermark(ctx, &p, all)?),
        "video.subtitles" => one(av::video_subtitles(ctx, &p, all)?),
        "video.reframe" => one(av::video_reframe(ctx, &p, all)?),
        "video.thumbs" => one(av::video_thumbs(ctx, &p, all)?),
        "video.slideshow" => one(av::video_from_images(ctx, &p, all)?),
        "video.mux" => one(av::video_mux(ctx, &p, input(0), input(1))?),
        "video.compress" => one(av::video_compress(ctx, &p, all)?),
        "media.info" => av::media_report(ctx, &p, all),

        "audio.convert" => one(av::audio_convert(ctx, &p, all)?),
        "audio.process" => one(media::audio_process(ctx, &p, all)?),
        "audio.join" => one(audiofx::join(ctx, &p, all)?),
        "audio.split" => one(audiofx::split(ctx, &p, all)?),
        "audio.silence" => one(audiofx::silence(ctx, &p, all)?),
        "audio.fx" => one(audiofx::fx(ctx, &p, all)?),
        "audio.visual" => one(audiofx::visual(ctx, &p, all)?),
        "audio.tags" => one(audiofx::tags(ctx, &p, all)?),

        "mesh.convert" => one(mesh3d::convert(ctx, &p, all)?),
        "mesh.transform" => one(mesh3d::transform(ctx, &p, all)?),
        "mesh.optimize" => one(mesh3d::optimize(ctx, &p, all)?),

        "archive.create" => one(arch2::create(ctx, &p, all)?),
        "archive.extract" => one(arch2::extract(ctx, &p, all)?),
        "archive.compress" => one(arch::compress(ctx, &p, all)?),
        "archive.test" => arch2::test(ctx, &p, all),

        "data.convert" => one(data::convert(ctx, &p, all)?),
        "data.json" => one(text2::json_tools(ctx, &p, all)?),
        "data.encode" => one(text2::encode_node(ctx, &p, all)?),
        "text.transform" => one(data::text_transform(ctx, &p, all)?),
        "text.lines" => one(text2::lines(ctx, &p, all)?),
        "text.merge" => one(text2::merge(ctx, &p, all)?),
        "text.split" => one(text2::split(ctx, &p, all)?),
        "text.extract" => one(text2::extract(ctx, &p, all)?),
        "text.stats" => text2::stats(ctx, &p, all),

        "sheet.convert" => one(sheet::convert(ctx, &p, all)?),
        "sheet.tools" => one(sheet::tools(ctx, &p, all)?),
        "sheet.merge" => one(sheet::merge(ctx, &p, all)?),
        "sheet.split" => one(sheet::split(ctx, &p, all)?),

        "doc.office" => one(docs::office(ctx, &p, all)?),
        "doc.pandoc" => one(docs::pandoc(ctx, &p, all)?),
        "doc.markdown" => one(docs::markdown(ctx, &p, all)?),
        "doc.text" => one(docs::to_text(ctx, &p, all)?),
        "pdf.merge" => one(pdftools::merge(ctx, &p, all)?),
        "pdf.split" => one(pdftools::split(ctx, &p, all)?),
        "pdf.pages" => one(pdftools::pages(ctx, &p, all)?),
        "pdf.meta" => pdftools::meta(ctx, &p, all),
        "pdf.compress" => one(pdftools::compress(ctx, &p, all)?),
        "pdf.text" => one(pdftools::text(ctx, &p, all)?),
        "pdf.render" => one(pdftools::render(ctx, &p, all)?),

        "util.note" => Ok(vec![]),
        "util.reroute" => one(all),
        "util.switch" => {
            let pick = if p.str("use", "a") == "b" { 1 } else { 0 };
            let v = input(pick);
            let v = if v.is_empty() && p.bool("fallback", true) { input(1 - pick) } else { v };
            one(v)
        }
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
