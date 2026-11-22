//! Compress for services: fits any file under an upload limit (Discord, Telegram,
//! WhatsApp, e-mail…). Every kind gets its own strategy: videos are re-encoded to a
//! bitrate budget (or cut into playable parts), images to a byte budget, audio to a
//! bitrate budget, GIFs are shrunk or turned into MP4, PDFs are recompressed and
//! everything else is zipped and, as a last resort, split into parts that fit.
use super::{arch, av, files2, img, pdftools};
use crate::engine::{Ctx, FileItem};
use crate::ops::media::media_info;
use crate::util::Params;
use anyhow::Result;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use serde_json::{json, Map, Value};
use std::path::Path;

/// Upload limit in MB (MiB) of a service preset; unknown ids and `custom` use `custom_mb`.
/// Keep in sync with SERVICE_MB in src/js/nodes.js.
pub fn service_mb(service: &str, custom_mb: f64) -> f64 {
    let mb = match service {
        "discord" => 10.0,
        "discord_basic" => 50.0,
        "discord_nitro" => 500.0,
        "telegram" => 2000.0,
        "telegram_premium" => 4000.0,
        "whatsapp" => 16.0,
        "whatsapp_doc" => 2000.0,
        "signal" => 100.0,
        "imessage" => 100.0,
        "messenger" => 25.0,
        "slack" => 1000.0,
        "vk" => 200.0,
        "gmail" => 25.0,
        "outlook" => 20.0,
        "yahoo" => 25.0,
        "email" => 10.0,
        _ => custom_mb,
    };
    mb.max(0.05)
}

/// Already compressed formats: zipping them again gains nothing.
const PACKED: &[&str] = &[
    "zip", "7z", "rar", "gz", "tgz", "xz", "zst", "bz2", "lz4", "cab", "jar", "apk", "docx", "xlsx", "pptx", "odt", "ods", "odp", "epub",
    "jpg", "jpeg", "png", "webp", "avif", "heic", "gif", "mp4", "mkv", "webm", "mov", "mp3", "m4a", "aac", "opus", "ogg", "flac", "glb",
    "pdf",
];

fn vals(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX)
}

fn warn_big(ctx: &Ctx, f: &FileItem, limit: u64) {
    if f.size > limit {
        ctx.warn(format!("{}: {:.1} MB is still over the {:.1} MB limit", f.name, mb(f.size), mb(limit)));
    }
}

/// Keeps the originals when a whole group fails (FFmpeg missing and so on).
fn or_keep(ctx: &Ctx, what: &str, keep: Vec<FileItem>, r: Result<Vec<FileItem>>) -> Result<Vec<FileItem>> {
    match r {
        Ok(v) => Ok(v),
        Err(e) => {
            ctx.check()?;
            ctx.warn(format!("{what}: {e:#}. The original file(s) were kept"));
            Ok(keep)
        }
    }
}

/// Last resort: plain .001/.002… parts (7-Zip and Morph's Split / Join Parts join them back).
fn split_parts(ctx: &Ctx, f: FileItem, budget: u64) -> Vec<FileItem> {
    let name = f.name.clone();
    let m = vals(&[("mode", json!("split")), ("size_mb", json!(mb(budget)))]);
    match files2::parts(ctx, &Params(&m), vec![f.clone()]) {
        Ok(parts) => {
            if parts.len() > 1 {
                ctx.warn(format!(
                    "{name}: split into {} parts (.001, .002…). Join them with 7-Zip or Morph (Split / Join Parts)",
                    parts.len()
                ));
            }
            parts
        }
        Err(e) => {
            ctx.warn(format!("{name}: could not split ({e:#})"));
            vec![f]
        }
    }
}

fn split_videos(ctx: &Ctx, videos: Vec<FileItem>, budget: u64) -> Result<Vec<FileItem>> {
    let mut out = vec![];
    for it in videos {
        ctx.check()?;
        let parts = ((it.size as f64 * 1.12) / budget.max(1) as f64).ceil().max(2.0);
        let m = vals(&[("mode", json!("parts")), ("parts", json!(parts)), ("reencode", json!(false))]);
        out.extend(av::video_split(ctx, &Params(&m), vec![it])?);
    }
    Ok(out)
}

/// Highest quality whose encode fits `target` bytes (binary search, like img::convert's target size).
fn fit_quality(im: &DynamicImage, ext: &str, o: &img::EncodeOpts, target: usize) -> Result<(Vec<u8>, bool)> {
    let mut o2 = o.clone();
    o2.lossless = false;
    let (mut lo, mut hi) = (5u8, 95u8);
    let mut best: Option<Vec<u8>> = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        o2.quality = mid;
        let b = img::encode(im, ext, &o2)?;
        if b.len() <= target {
            best = Some(b);
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    match best {
        Some(b) => Ok((b, true)),
        None => {
            o2.quality = 5;
            Ok((img::encode(im, ext, &o2)?, false))
        }
    }
}

/// Re-encodes an image to fit `budget` bytes, scaling it down step by step if needed.
fn shrink_image(ctx: &Ctx, it: &FileItem, fmt: &str, budget: u64) -> Result<FileItem> {
    let ext = match fmt {
        "jpeg" | "jpg" => "jpg",
        "webp" => "webp",
        _ if matches!(it.ext.as_str(), "png" | "webp" | "avif" | "tif" | "tiff" | "tga" | "qoi" | "ico") => "webp",
        _ => "jpg",
    };
    let opts = img::EncodeOpts {
        quality: 88,
        lossless: false,
        png_optimize: "off".into(),
        png_colors: 0,
        avif_speed: 8,
        background: [255, 255, 255],
    };
    let src = img::open(&it.path_buf())?;
    let (w, h) = src.dimensions();
    let mut bytes: Vec<u8> = vec![];
    for scale in [1.0f64, 0.8, 0.62, 0.48, 0.36, 0.25] {
        ctx.check()?;
        let small;
        let im = if scale < 1.0 {
            small = src.resize(((w as f64 * scale) as u32).max(1), ((h as f64 * scale) as u32).max(1), FilterType::Lanczos3);
            &small
        } else {
            &src
        };
        let (b, fits) = fit_quality(im, ext, &opts, budget as usize)?;
        bytes = b;
        if fits {
            if scale < 1.0 {
                ctx.warn(format!("{}: scaled down to {:.0}% to fit", it.name, scale * 100.0));
            }
            break;
        }
    }
    let out = ctx.out_path(it, &it.stem(), ext)?;
    std::fs::write(&out, &bytes)?;
    Ok(it.derive(&out))
}

/// Shrinks an animated GIF (smaller size, fewer frames and colors) or turns it into MP4.
fn shrink_gif(ctx: &Ctx, it: &FileItem, to_mp4: bool, budget: u64, on: &dyn Fn(f64)) -> Result<Vec<FileItem>> {
    let info = media_info(ctx.tools, &it.path_buf());
    if to_mp4 {
        let out = ctx.out_path(it, &it.stem(), "mp4")?;
        let mut a = av::sv(&["-i"]);
        a.push(it.path.clone());
        a.extend(av::sv(&[
            "-vf",
            "scale=trunc(iw/2)*2:trunc(ih/2)*2",
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-an",
        ]));
        a.push(av::ps(&out));
        av::ff(ctx, &a, info.duration, on, None)?;
        return Ok(vec![it.derive(&out)]);
    }
    let w0 = if info.width > 0 { info.width as f64 } else { 480.0 };
    let src_fps = if info.fps > 0.0 { info.fps } else { 25.0 };
    let out = ctx.out_path(it, &it.stem(), "gif")?;
    let steps: [(f64, f64, u32); 6] = [(1.0, 24.0, 256), (0.8, 18.0, 192), (0.64, 15.0, 128), (0.5, 12.0, 96), (0.38, 10.0, 64), (0.28, 8.0, 48)];
    let n = steps.len() as f64;
    for (i, &(scale, fps, colors)) in steps.iter().enumerate() {
        ctx.check()?;
        let w = ((w0 * scale / 2.0).round() * 2.0).max(16.0) as i64;
        let fps = fps.min(src_fps);
        let vf = format!(
            "fps={fps:.3},scale={w}:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors={colors}:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3"
        );
        let mut a = av::sv(&["-i"]);
        a.push(it.path.clone());
        a.push("-vf".into());
        a.push(vf);
        a.extend(av::sv(&["-loop", "0"]));
        a.push(av::ps(&out));
        let base = i as f64;
        av::ff(ctx, &a, info.duration, &|f| on((base + f) / n), None)?;
        if file_size(&out) <= budget {
            break;
        }
    }
    Ok(vec![it.derive(&out)])
}

pub fn compress(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let limit_mb = service_mb(&p.str("service", "discord"), p.f64("size_mb", 25.0));
    let margin = p.f64("headroom", 4.0).clamp(0.0, 40.0) / 100.0;
    let limit = (limit_mb * 1_048_576.0) as u64;
    let budget = (limit as f64 * (1.0 - margin)) as u64;
    let skip_small = p.bool("skip_small", true);
    let split = p.bool("split", true);

    let mut out: Vec<FileItem> = vec![];
    let mut videos: Vec<FileItem> = vec![];
    let mut images: Vec<FileItem> = vec![];
    let mut gifs: Vec<FileItem> = vec![];
    let mut audio: Vec<FileItem> = vec![];
    let mut pdfs: Vec<FileItem> = vec![];
    let mut other: Vec<FileItem> = vec![];
    for f in files {
        if skip_small && f.size <= limit {
            out.push(f);
            continue;
        }
        let group = match (f.kind.as_str(), f.ext.as_str()) {
            (_, "gif") => 2,
            ("video", _) => 0,
            ("image", _) => 1,
            ("audio", _) => 3,
            (_, "pdf") => 4,
            _ => 5,
        };
        match group {
            0 => videos.push(f),
            1 => images.push(f),
            2 => gifs.push(f),
            3 => audio.push(f),
            4 => pdfs.push(f),
            _ => other.push(f),
        }
    }
    let fit_already = out.len();
    if fit_already > 0 && videos.is_empty() && images.is_empty() && gifs.is_empty() && audio.is_empty() && pdfs.is_empty() && other.is_empty() {
        ctx.warn(format!("All {fit_already} file(s) already fit into {limit_mb} MB and were kept as they are"));
        return Ok(out);
    }

    if !videos.is_empty() {
        let keep = videos.clone();
        let r = if p.str("video_mode", "compress") == "split" {
            split_videos(ctx, videos, budget)
        } else {
            let m = vals(&[
                ("target", json!("custom")),
                ("size_mb", json!(limit_mb * (1.0 - margin) / 0.96)),
                ("max_height", json!(p.str("max_height", "1080"))),
                ("codec", json!(p.str("codec", "h264"))),
                ("audio_bitrate", json!(p.i64("audio_bitrate", 96))),
                ("max_fps", json!(p.f64("max_fps", 30.0))),
                ("preset", json!(p.str("preset", "medium"))),
                ("skip_small", json!(false)),
            ]);
            av::video_compress(ctx, &Params(&m), videos)
        };
        out.extend(or_keep(ctx, "Videos", keep, r)?);
    }

    if !images.is_empty() {
        let fmt = p.str("image_format", "auto");
        let total = images.len();
        for (i, it) in images.into_iter().enumerate() {
            ctx.check()?;
            match shrink_image(ctx, &it, &fmt, budget) {
                Ok(f) => {
                    warn_big(ctx, &f, limit);
                    out.push(f);
                }
                Err(e) => {
                    ctx.warn(format!("{}: {e:#}", it.name));
                    out.push(it);
                }
            }
            ctx.progress(i + 1, total, "images");
        }
    }

    if !gifs.is_empty() {
        let keep = gifs.clone();
        let to_mp4 = p.str("gif_mode", "shrink") == "mp4";
        let r = av::each(ctx, gifs, &[], |it, on| shrink_gif(ctx, it, to_mp4, budget, on));
        let r = or_keep(ctx, "GIF", keep, r)?;
        for f in &r {
            warn_big(ctx, f, limit);
        }
        out.extend(r);
    }

    if !audio.is_empty() {
        if let Err(e) = ctx.tools.ffprobe() {
            ctx.warn(format!("Audio: {e:#}. The original file(s) were kept"));
            out.extend(audio);
        } else {
            let fmt = p.str("audio_format", "m4a");
            let total = audio.len();
            for (i, it) in audio.into_iter().enumerate() {
                ctx.check()?;
                let info = media_info(ctx.tools, &it.path_buf());
                if info.duration <= 0.0 {
                    ctx.warn(format!("{}: unknown duration, kept as is", it.name));
                    out.push(it);
                    continue;
                }
                let cap = if fmt == "opus" { 256.0 } else { 320.0 };
                let kbps = (budget as f64 * 8.0 / 1000.0 / info.duration * 0.97).floor().clamp(12.0, cap);
                if kbps < 32.0 {
                    ctx.warn(format!("{}: {:.0} s of audio is long for this limit, quality will be low", it.name, info.duration));
                }
                let m = vals(&[
                    ("format", json!(fmt)),
                    ("bitrate", json!(kbps)),
                    ("channels", json!(if kbps < 64.0 { "mono" } else { "keep" })),
                    ("keep_cover", json!(false)),
                ]);
                match av::audio_convert(ctx, &Params(&m), vec![it.clone()]) {
                    Ok(v) => {
                        for f in &v {
                            warn_big(ctx, f, limit);
                        }
                        out.extend(v);
                    }
                    Err(e) => {
                        ctx.check()?;
                        ctx.warn(format!("{}: {e:#}", it.name));
                        out.push(it);
                    }
                }
                ctx.progress(i + 1, total, "audio");
            }
        }
    }

    if !pdfs.is_empty() {
        let mode = p.str("pdf_mode", "auto");
        let gs = ctx.tools.gs().is_ok();
        let modes: Vec<String> = match mode.as_str() {
            "auto" if gs => vec!["ebook".into(), "screen".into()],
            "lossless" | "auto" => vec!["lossless".into()],
            m if gs => vec![m.to_string()],
            _ => {
                ctx.warn("Ghostscript is not installed, PDFs were only optimized losslessly".into());
                vec!["lossless".into()]
            }
        };
        for it in pdfs {
            ctx.check()?;
            let mut cur: Option<FileItem> = None;
            let mut first: Option<String> = None;
            for m in &modes {
                let mm = vals(&[("mode", json!(m)), ("keep_smaller", json!(true))]);
                match pdftools::compress(ctx, &Params(&mm), vec![it.clone()]) {
                    Ok(v) => {
                        if let Some(f) = v.into_iter().next() {
                            if first.is_none() {
                                first = Some(f.path.clone());
                            }
                            if let Some(prev) = cur.take() {
                                let _ = std::fs::remove_file(&prev.path);
                            }
                            let fits = f.size <= limit;
                            cur = Some(f);
                            if fits {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        ctx.check()?;
                        ctx.warn(format!("{}: {e:#}", it.name));
                    }
                }
            }
            let mut f = cur.unwrap_or_else(|| it.clone());
            if let Some(fp) = first {
                if f.path != fp && !Path::new(&fp).exists() && std::fs::rename(&f.path, &fp).is_ok() {
                    f = it.derive(Path::new(&fp));
                }
            }
            if f.size > limit && split {
                out.extend(split_parts(ctx, f, budget));
            } else {
                warn_big(ctx, &f, limit);
                out.push(f);
            }
        }
    }

    if !other.is_empty() {
        let zip = p.bool("zip", true);
        let total = other.len();
        for (i, it) in other.into_iter().enumerate() {
            ctx.check()?;
            let mut f = it.clone();
            if zip && it.kind != "archive" && !PACKED.contains(&it.ext.as_str()) {
                let m = vals(&[
                    ("format", json!("zip")),
                    ("level", json!(9)),
                    ("mode", json!("per-file")),
                    ("name", json!(it.stem())),
                    ("password", json!("")),
                ]);
                match arch::create(ctx, &Params(&m), vec![it.clone()]) {
                    Ok(v) => {
                        if let Some(z) = v.into_iter().next() {
                            if z.size < it.size {
                                f = z;
                            } else {
                                let _ = std::fs::remove_file(&z.path);
                            }
                        }
                    }
                    Err(e) => {
                        ctx.check()?;
                        ctx.warn(format!("{}: {e:#}", it.name));
                    }
                }
            }
            if f.size > limit && split {
                out.extend(split_parts(ctx, f, budget));
            } else {
                warn_big(ctx, &f, limit);
                out.push(f);
            }
            ctx.progress(i + 1, total, &it.name);
        }
    }
    Ok(out)
}
