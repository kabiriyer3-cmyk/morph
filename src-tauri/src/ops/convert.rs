//! Universal converter: picks the right engine for every input file and the chosen target format.
use crate::engine::{Ctx, FileItem};
use crate::ops::{arch, av, data, docs, img, img2, media, mesh3d, pdftools, sheet};
use crate::util::Params;
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const IMAGE_OUT: &[&str] = &["png", "jpg", "jpeg", "webp", "avif", "gif", "bmp", "tiff", "tif", "ico", "tga", "qoi", "ppm", "exr"];
pub const VIDEO_OUT: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "m4v", "flv", "wmv", "mpg", "ts", "m2ts", "3gp", "ogv", "mxf"];
pub const AUDIO_OUT: &[&str] = &["mp3", "m4a", "aac", "opus", "ogg", "flac", "wav", "alac", "aiff", "wma", "ac3", "amr", "m4r"];
pub const DATA_OUT: &[&str] = &["json", "yaml", "toml", "csv", "tsv", "ndjson"];
pub const MESH_OUT: &[&str] = &["glb", "gltf", "obj", "stl", "ply", "off"];
pub const ARCH_OUT: &[&str] = &["zip", "7z", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"];

fn one_of(v: &str, list: &[&str]) -> bool {
    list.contains(&v)
}

fn route(it: &FileItem, to: &str, office: bool, pandoc: bool) -> &'static str {
    let k = it.kind.as_str();
    let e = it.ext.as_str();
    if one_of(to, ARCH_OUT) {
        return if k == "archive" { "repack" } else { "pack" };
    }
    match k {
        "image" => {
            if one_of(to, IMAGE_OUT) {
                "image"
            } else if to == "pdf" {
                "image-pdf"
            } else if one_of(to, VIDEO_OUT) {
                "image-video"
            } else {
                ""
            }
        }
        "video" => {
            if one_of(to, VIDEO_OUT) {
                "video"
            } else if one_of(to, AUDIO_OUT) {
                "video-audio"
            } else if to == "gif" || to == "webp" {
                "video-anim"
            } else if one_of(to, IMAGE_OUT) {
                "video-frame"
            } else {
                ""
            }
        }
        "audio" => {
            if one_of(to, AUDIO_OUT) {
                "audio"
            } else if one_of(to, &["mp4", "webm", "mkv", "mov"]) {
                "audio-video"
            } else {
                ""
            }
        }
        "mesh" => {
            if one_of(to, MESH_OUT) {
                "mesh"
            } else {
                ""
            }
        }
        _ => {
            if one_of(e, &["json", "yaml", "yml", "toml", "ndjson", "jsonl", "geojson"]) && one_of(to, DATA_OUT) {
                return "data";
            }
            if sheet::is_sheet(e) && one_of(to, sheet::OUT) {
                return if one_of(to, &["ods", "xls", "pdf"]) && !office { "" } else { "sheet" };
            }
            if e == "pdf" {
                return match to {
                    "txt" => "pdf-text",
                    "png" | "jpg" | "jpeg" => "pdf-image",
                    _ => "",
                };
            }
            let markdown = one_of(e, &["md", "markdown"]);
            if markdown {
                return match to {
                    "html" => "md-html",
                    "pdf" if office => "md-pdf",
                    "txt" => "text",
                    _ if pandoc => "pandoc",
                    _ if office && one_of(to, &["docx", "odt", "rtf", "doc"]) => "md-office",
                    _ => "",
                };
            }
            let txt = one_of(k, &["text"]) || e == "txt";
            if txt {
                return match to {
                    "html" => "md-html",
                    "pdf" | "docx" | "odt" | "rtf" | "doc" if office => "office",
                    _ if pandoc && one_of(to, &["docx", "odt", "epub", "rtf", "md"]) => "pandoc",
                    _ => "",
                };
            }
            if to == "txt" && one_of(e, &["docx", "odt", "odp", "pptx", "epub", "html", "htm", "rtf"]) && !office {
                return "text";
            }
            if one_of(e, &["html", "htm"]) && to == "md" && pandoc {
                return "pandoc";
            }
            if one_of(e, &["docx", "odt", "rtf", "epub", "html", "htm", "ipynb", "rst", "tex", "org"]) && one_of(to, &["md", "epub", "rst", "tex", "org"]) && pandoc {
                return "pandoc";
            }
            if one_of(k, &["doc"]) && office && one_of(to, &["pdf", "docx", "doc", "odt", "rtf", "html", "txt", "epub", "pptx", "ppt", "odp", "png", "jpg"]) {
                return "office";
            }
            if one_of(e, &["docx", "odt", "epub", "html", "htm", "rtf"]) && pandoc && one_of(to, &["docx", "odt", "html", "txt", "pdf", "epub"]) {
                return "pandoc";
            }
            if to == "txt" && k == "doc" {
                return "text";
            }
            ""
        }
    }
}

fn params(pairs: &[(&str, Value)]) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    m
}

fn image_route(ctx: &Ctx, to: &str, q: i64, max: i64, lossless: bool, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let ext = match to {
        "jpeg" => "jpg",
        "tif" => "tiff",
        e => e,
    };
    super::batch(ctx, files, &[], true, |it| {
        let mut im = img2::open_any(ctx, &it.path_buf())?;
        if max > 0 {
            use image::GenericImageView;
            let (w, h) = im.dimensions();
            if w.max(h) as i64 > max {
                im = im.resize(max as u32, max as u32, image::imageops::FilterType::Lanczos3);
            }
        }
        let mut o = img::EncodeOpts::keep();
        o.quality = q.clamp(1, 100) as u8;
        o.lossless = lossless;
        if ext == "png" {
            o.png_optimize = "fast".into();
        }
        Ok(vec![img::save(ctx, it, &im, ext, &o)?])
    })
}

fn run_route(ctx: &Ctx, p: &Params, route: &str, to: &str, items: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let q = p.i64("quality", 80).clamp(1, 100);
    let max = p.i64("max_size", 0).max(0);
    let crf = (51.0 - q as f64 * 0.33).round() as i64;
    let abr = p.i64("audio_bitrate", 192).clamp(8, 1024);
    match route {
        "image" => image_route(ctx, to, q, max, p.bool("lossless", false), items),
        "image-pdf" => {
            let combine = p.bool("combine", false);
            let m = params(&[("page", json!("fit")), ("per_image", json!(!combine)), ("quality", json!(q)), ("name", json!("images"))]);
            img::to_pdf(ctx, &Params(&m), items)
        }
        "image-video" => {
            let m = params(&[("format", json!(to)), ("seconds", json!(p.f64("seconds", 3.0))), ("name", json!("slideshow"))]);
            av::video_from_images(ctx, &Params(&m), items)
        }
        "video" => {
            let m = params(&[
                ("container", json!(to)),
                ("video_codec", json!(p.str("video_codec", "auto"))),
                ("quality", json!(crf)),
                ("preset", json!(p.str("preset", "medium"))),
                ("max_size", json!(max)),
                ("audio_codec", json!("auto")),
                ("audio_bitrate", json!(abr.min(320))),
                ("hardware", json!(p.str("hardware", "none"))),
            ]);
            av::video_convert(ctx, &Params(&m), items)
        }
        "video-audio" | "audio" => {
            let m = params(&[("format", json!(to)), ("bitrate", json!(abr))]);
            av::audio_convert(ctx, &Params(&m), items)
        }
        "video-anim" => {
            let w = if max > 0 { max } else { 480 };
            let m = params(&[("format", json!(to)), ("fps", json!(p.f64("fps", 12.0))), ("width", json!(w))]);
            media::video_gif(ctx, &Params(&m), items)
        }
        "video-frame" => {
            let ext = if to == "jpeg" { "jpg" } else { to };
            av::each(ctx, items, &["video"], |it, _| Ok(vec![av::grab_frame(ctx, it, ext, 0.1, max)?]))
        }
        "audio-video" => av::audio_to_video(ctx, items, to, "#4a92ff", "#0b0e14"),
        "sheet" => {
            let m = params(&[("format", json!(to))]);
            sheet::convert(ctx, &Params(&m), items)
        }
        "data" => {
            let m = params(&[("format", json!(to)), ("pretty", json!(true)), ("indent", json!(2))]);
            data::convert(ctx, &Params(&m), items)
        }
        "md-html" => {
            let m = params(&[("theme", json!(p.str("theme", "light")))]);
            docs::markdown(ctx, &Params(&m), items)
        }
        "md-pdf" | "md-office" => {
            let m = params(&[("theme", json!("light"))]);
            let html = docs::markdown(ctx, &Params(&m), items)?;
            let fmt = if route == "md-pdf" { "pdf" } else { to };
            super::batch(ctx, html, &[], false, |it| {
                let produced = docs::soffice_raw(ctx, &it.path_buf(), fmt)?;
                let ext = crate::kinds::ext_of(&produced);
                let out = ctx.out_path(it, &it.stem(), &ext)?;
                if std::fs::rename(&produced, &out).is_err() {
                    std::fs::copy(&produced, &out)?;
                }
                Ok(vec![it.derive(&out)])
            })
        }
        "office" => {
            let m = params(&[("format", json!(to))]);
            docs::office(ctx, &Params(&m), items)
        }
        "pandoc" => {
            let m = params(&[("format", json!(to))]);
            docs::pandoc(ctx, &Params(&m), items)
        }
        "text" => docs::to_text(ctx, &Params(&Map::new()), items),
        "pdf-text" => pdftools::text(ctx, &Params(&Map::new()), items),
        "pdf-image" => {
            let m = params(&[("format", json!(if to == "jpeg" { "jpg" } else { to })), ("dpi", json!(p.i64("dpi", 150)))]);
            pdftools::render(ctx, &Params(&m), items)
        }
        "mesh" => {
            let m = params(&[("format", json!(to)), ("binary", json!(true))]);
            mesh3d::convert(ctx, &Params(&m), items)
        }
        "pack" => {
            let m = params(&[("format", json!(to)), ("mode", json!("per-file")), ("level", json!(6))]);
            arch::create(ctx, &Params(&m), items)
        }
        "repack" => {
            let mut out = vec![];
            for it in items {
                ctx.check()?;
                let x = params(&[("subfolder", json!(false))]);
                let files = arch::extract(ctx, &Params(&x), vec![it.clone()])?;
                let c = params(&[("format", json!(to)), ("mode", json!("single")), ("level", json!(6)), ("name", json!(arch::archive_stem(&it.name)))]);
                out.extend(arch::create(ctx, &Params(&c), files)?);
            }
            Ok(out)
        }
        _ => {
            for it in &items {
                ctx.warn(format!("{}: .{} \u{2192} .{} is not supported", it.name, it.ext, to));
            }
            Ok(vec![])
        }
    }
}

pub fn convert_any(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let to = p.str("to", "png").trim().trim_start_matches('.').to_lowercase();
    if files.is_empty() {
        bail!("No files to convert");
    }
    let office = ctx.tools.soffice.is_some();
    let pandoc = ctx.tools.pandoc.is_some();
    let mut buckets: BTreeMap<&'static str, Vec<FileItem>> = BTreeMap::new();
    for it in files {
        let r = if it.ext == to && !matches!(it.kind.as_str(), "image" | "video" | "audio") { "same" } else { route(&it, &to, office, pandoc) };
        buckets.entry(r).or_default().push(it);
    }
    let mut out = vec![];
    for (r, items) in buckets {
        ctx.check()?;
        if r == "same" {
            out.extend(items);
            continue;
        }
        if r.is_empty() {
            for it in &items {
                let hint = if it.kind == "doc" && !office {
                    " (install LibreOffice for office documents)"
                } else {
                    ""
                };
                ctx.warn(format!("{}: can't convert .{} to .{}{hint}", it.name, it.ext, to));
            }
            continue;
        }
        match run_route(ctx, p, r, &to, items) {
            Ok(v) => out.extend(v),
            Err(e) => {
                if ctx.cancelled() {
                    return Err(e);
                }
                ctx.warn(format!("{:#}", e));
            }
        }
    }
    if out.is_empty() {
        bail!("Nothing was converted. {}", ctx.first_warning());
    }
    Ok(out)
}
