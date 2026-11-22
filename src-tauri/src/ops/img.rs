use super::{batch, split_kinds};
use crate::engine::{Ctx, FileItem};
use crate::pdf;
use crate::util::{parse_color, write_ico, Params};
use anyhow::{anyhow, bail, Result};
use image::codecs::avif::AvifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Rgb, RgbImage,
    Rgba, RgbaImage,
};
use rayon::prelude::*;
use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

const KINDS: &[&str] = &["image"];

static FONTDB: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();

/// Renders an SVG. `max_side` 0 = natural size (at least 1024 px on the long side).
pub fn render_svg(path: &Path, max_side: u32) -> Result<DynamicImage> {
    let data = std::fs::read(path)?;
    let mut opt = resvg::usvg::Options::default();
    opt.resources_dir = path.parent().map(|p| p.to_path_buf());
    opt.fontdb = FONTDB
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone();
    let tree = resvg::usvg::Tree::from_data(&data, &opt)?;
    let size = tree.size();
    let (w, h) = (size.width().max(1.0), size.height().max(1.0));
    let long = w.max(h);
    let scale = if max_side > 0 {
        max_side as f32 / long
    } else if long < 1024.0 {
        1024.0 / long
    } else {
        1.0
    };
    let pw = ((w * scale).round() as u32).clamp(1, 16384);
    let ph = ((h * scale).round() as u32).clamp(1, 16384);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph).ok_or_else(|| anyhow!("SVG is too large"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let mut px = pixmap.take();
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in p.iter_mut().take(3) {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    let img = RgbaImage::from_raw(pw, ph, px).ok_or_else(|| anyhow!("bad SVG buffer"))?;
    Ok(DynamicImage::ImageRgba8(img))
}

/// Opens any supported image, applying EXIF orientation.
pub fn open(path: &Path) -> Result<DynamicImage> {
    if crate::kinds::ext_of(path) == "svg" {
        return render_svg(path, 0);
    }
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    reader.no_limits();
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(img)
}

#[derive(Clone, Debug)]
pub struct EncodeOpts {
    pub quality: u8,
    pub lossless: bool,
    pub png_optimize: String,
    pub png_colors: u32,
    pub avif_speed: u8,
    pub background: [u8; 3],
}

impl EncodeOpts {
    /// High quality settings used for intermediate results of non-convert nodes.
    pub fn keep() -> Self {
        EncodeOpts {
            quality: 92,
            lossless: false,
            png_optimize: "off".into(),
            png_colors: 0,
            avif_speed: 8,
            background: [255, 255, 255],
        }
    }
}

pub fn encodable(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "webp" | "avif" | "gif" | "bmp" | "tif" | "tiff" | "ico" | "tga" | "qoi" | "pnm" | "ppm" | "exr"
    )
}

pub fn keep_ext(it: &FileItem) -> String {
    match it.ext.as_str() {
        "jpeg" | "jpe" | "jfif" => "jpg".into(),
        e if encodable(e) => e.into(),
        _ => "png".into(),
    }
}

pub fn flatten(img: &DynamicImage, bg: [u8; 3]) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let mix = |c: u8, b: u8| ((c as u32 * a + b as u32 * (255 - a) + 127) / 255) as u8;
        out.put_pixel(x, y, Rgb([mix(p[0], bg[0]), mix(p[1], bg[1]), mix(p[2], bg[2])]));
    }
    out
}

fn write_fmt(img: &DynamicImage, fmt: ImageFormat) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), fmt)?;
    Ok(buf)
}

fn quantized_png(rgba: &RgbaImage, colors: u32) -> Result<Vec<u8>> {
    let (w, h) = rgba.dimensions();
    let nq = color_quant::NeuQuant::new(10, colors.clamp(2, 256) as usize, rgba.as_raw());
    let map = nq.color_map_rgba();
    let indices: Vec<u8> = rgba.as_raw().chunks_exact(4).map(|px| nq.index_of(px) as u8).collect();
    let mut palette = Vec::with_capacity(map.len() / 4 * 3);
    let mut trns = Vec::with_capacity(map.len() / 4);
    for c in map.chunks_exact(4) {
        palette.extend_from_slice(&c[..3]);
        trns.push(c[3]);
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Indexed);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_palette(palette);
        if trns.iter().any(|&a| a < 255) {
            enc.set_trns(trns);
        }
        let mut wr = enc.write_header()?;
        wr.write_image_data(&indices)?;
    }
    Ok(out)
}

fn encode_png(img: &DynamicImage, o: &EncodeOpts) -> Result<Vec<u8>> {
    let mut buf = if (2..=256).contains(&o.png_colors) {
        quantized_png(&img.to_rgba8(), o.png_colors)?
    } else {
        match img {
            DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
                write_fmt(&DynamicImage::ImageRgba16(img.to_rgba16()), ImageFormat::Png)?
            }
            _ => write_fmt(img, ImageFormat::Png)?,
        }
    };
    let preset = match o.png_optimize.as_str() {
        "fast" => Some(1),
        "balanced" => Some(3),
        "max" => Some(6),
        _ => None,
    };
    if let Some(level) = preset {
        let opts = oxipng::Options::from_preset(level);
        if let Ok(b) = oxipng::optimize_from_memory(&buf, &opts) {
            if b.len() < buf.len() {
                buf = b;
            }
        }
    }
    Ok(buf)
}

pub fn encode(img: &DynamicImage, ext: &str, o: &EncodeOpts) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    let q = o.quality.clamp(1, 100);
    Ok(match ext {
        "jpg" | "jpeg" => {
            let rgb = flatten(img, o.background);
            let mut buf = Vec::new();
            JpegEncoder::new_with_quality(&mut buf, q).write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)?;
            buf
        }
        "webp" => {
            let rgba = img.to_rgba8();
            let mem = webp::Encoder::from_rgba(rgba.as_raw(), w, h)
                .encode_simple(o.lossless, q as f32)
                .map_err(|e| anyhow!("WebP encoding failed: {:?}", e))?;
            mem.to_vec()
        }
        "avif" => {
            let rgba = img.to_rgba8();
            let mut buf = Vec::new();
            AvifEncoder::new_with_speed_quality(&mut buf, o.avif_speed.clamp(1, 10), if o.lossless { 100 } else { q })
                .write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)?;
            buf
        }
        "png" => encode_png(img, o)?,
        "ico" => {
            let im = if w > 256 || h > 256 { img.thumbnail(256, 256) } else { img.clone() };
            write_fmt(&DynamicImage::ImageRgba8(im.to_rgba8()), ImageFormat::Ico)?
        }
        "gif" => write_fmt(&DynamicImage::ImageRgba8(img.to_rgba8()), ImageFormat::Gif)?,
        "bmp" => write_fmt(&DynamicImage::ImageRgba8(img.to_rgba8()), ImageFormat::Bmp)?,
        "tif" | "tiff" => {
            let im = if img.color().has_alpha() {
                DynamicImage::ImageRgba8(img.to_rgba8())
            } else {
                DynamicImage::ImageRgb8(img.to_rgb8())
            };
            write_fmt(&im, ImageFormat::Tiff)?
        }
        "tga" => write_fmt(&DynamicImage::ImageRgba8(img.to_rgba8()), ImageFormat::Tga)?,
        "qoi" => write_fmt(&DynamicImage::ImageRgba8(img.to_rgba8()), ImageFormat::Qoi)?,
        "pnm" | "ppm" => write_fmt(&DynamicImage::ImageRgb8(flatten(img, o.background)), ImageFormat::Pnm)?,
        "exr" => write_fmt(&DynamicImage::ImageRgba32F(img.to_rgba32f()), ImageFormat::OpenExr)?,
        other => bail!("Can't encode .{other}"),
    })
}

/// Binary-searches the quality so the result fits into `target` bytes.
fn fit_size(img: &DynamicImage, ext: &str, o: &EncodeOpts, target: usize) -> Result<(Vec<u8>, bool)> {
    let mut o2 = o.clone();
    o2.lossless = false;
    let (mut lo, mut hi) = (5u8, 95u8);
    let mut best: Option<Vec<u8>> = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        o2.quality = mid;
        let b = encode(img, ext, &o2)?;
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
            Ok((encode(img, ext, &o2)?, false))
        }
    }
}

pub fn save(ctx: &Ctx, it: &FileItem, img: &DynamicImage, ext: &str, o: &EncodeOpts) -> Result<FileItem> {
    let ext = if encodable(ext) { ext.to_string() } else { "png".to_string() };
    let bytes = encode(img, &ext, o)?;
    let out = ctx.out_path(it, &it.stem(), &ext)?;
    std::fs::write(&out, bytes)?;
    Ok(it.derive(&out))
}

fn filter_type(s: &str) -> FilterType {
    match s {
        "nearest" => FilterType::Nearest,
        "triangle" => FilterType::Triangle,
        "catmull-rom" => FilterType::CatmullRom,
        "gaussian" => FilterType::Gaussian,
        _ => FilterType::Lanczos3,
    }
}

fn anchor_frac(s: &str) -> (f64, f64) {
    match s {
        "top-left" => (0.0, 0.0),
        "top" => (0.5, 0.0),
        "top-right" => (1.0, 0.0),
        "left" => (0.0, 0.5),
        "right" => (1.0, 0.5),
        "bottom-left" => (0.0, 1.0),
        "bottom" => (0.5, 1.0),
        "bottom-right" => (1.0, 1.0),
        _ => (0.5, 0.5),
    }
}

pub fn resize(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "fit");
    let w = p.u32("width", 1920).max(1);
    let h = p.u32("height", 1080).max(1);
    let pct = p.f64("percent", 50.0).clamp(1.0, 1000.0);
    let filter = filter_type(&p.str("filter", "lanczos3"));
    let only_shrink = p.bool("only_shrink", true);
    batch(ctx, files, KINDS, true, |it| {
        let img = open(&it.path_buf())?;
        let (iw, ih) = img.dimensions();
        let (fw, fh) = (iw as f64, ih as f64);
        let out = match mode.as_str() {
            "fill" => {
                if only_shrink && w >= iw && h >= ih {
                    img
                } else {
                    img.resize_to_fill(w, h, filter)
                }
            }
            "stretch" => img.resize_exact(w, h, filter),
            _ => {
                let (tw, th) = match mode.as_str() {
                    "percent" => (fw * pct / 100.0, fh * pct / 100.0),
                    "width" => (w as f64, fh * w as f64 / fw),
                    "height" => (fw * h as f64 / fh, h as f64),
                    _ => {
                        let s = (w as f64 / fw).min(h as f64 / fh);
                        (fw * s, fh * s)
                    }
                };
                let (tw, th) = ((tw.round() as u32).max(1), (th.round() as u32).max(1));
                if only_shrink && tw >= iw && th >= ih {
                    img
                } else {
                    img.resize_exact(tw, th, filter)
                }
            }
        };
        Ok(vec![save(ctx, it, &out, &keep_ext(it), &EncodeOpts::keep())?])
    })
}

fn parse_aspect(s: &str) -> f64 {
    let parts: Vec<f64> = s.split([':', 'x', '/']).filter_map(|v| v.trim().parse().ok()).collect();
    match parts.as_slice() {
        [a, b] if *a > 0.0 && *b > 0.0 => a / b,
        [a] if *a > 0.0 => *a,
        _ => 1.0,
    }
}

fn trim_box(img: &RgbaImage, tol: f64) -> (u32, u32, u32, u32) {
    let (w, h) = img.dimensions();
    let bg = *img.get_pixel(0, 0);
    let t = (tol.clamp(0.0, 100.0) * 2.55 * 4.0) as i32;
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    let mut any = false;
    for (x, y, p) in img.enumerate_pixels() {
        let d: i32 = (0..4).map(|i| (p[i] as i32 - bg[i] as i32).abs()).sum();
        if d > t {
            any = true;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if !any {
        return (0, 0, w, h);
    }
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

pub fn crop(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "aspect");
    let aspect = parse_aspect(&p.str("aspect", "1:1"));
    let anchor = p.str("anchor", "center");
    let (x, y, cw, ch) = (p.u32("x", 0), p.u32("y", 0), p.u32("width", 512), p.u32("height", 512));
    let tol = p.f64("tolerance", 10.0);
    batch(ctx, files, KINDS, true, |it| {
        let img = open(&it.path_buf())?;
        let (iw, ih) = img.dimensions();
        let (bx, by, bw, bh) = match mode.as_str() {
            "manual" => {
                let bx = x.min(iw.saturating_sub(1));
                let by = y.min(ih.saturating_sub(1));
                (bx, by, cw.min(iw - bx).max(1), ch.min(ih - by).max(1))
            }
            "trim" => trim_box(&img.to_rgba8(), tol),
            _ => {
                let cur = iw as f64 / ih as f64;
                let (w2, h2) = if cur > aspect {
                    (((ih as f64) * aspect).round() as u32, ih)
                } else {
                    (iw, ((iw as f64) / aspect).round() as u32)
                };
                let (w2, h2) = (w2.clamp(1, iw), h2.clamp(1, ih));
                let (fx, fy) = anchor_frac(&anchor);
                (((iw - w2) as f64 * fx).round() as u32, ((ih - h2) as f64 * fy).round() as u32, w2, h2)
            }
        };
        let out = img.crop_imm(bx, by, bw, bh);
        Ok(vec![save(ctx, it, &out, &keep_ext(it), &EncodeOpts::keep())?])
    })
}

pub fn transform(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let rotate = p.i64("rotate", 0).rem_euclid(360);
    let fh = p.bool("flip_h", false);
    let fv = p.bool("flip_v", false);
    batch(ctx, files, KINDS, true, |it| {
        let mut img = open(&it.path_buf())?;
        img = match rotate {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => img,
        };
        if fh {
            img = img.fliph();
        }
        if fv {
            img = img.flipv();
        }
        Ok(vec![save(ctx, it, &img, &keep_ext(it), &EncodeOpts::keep())?])
    })
}

pub fn adjust(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let brightness = p.f64("brightness", 0.0);
    let contrast = p.f64("contrast", 0.0);
    let saturation = p.f64("saturation", 0.0);
    let hue = p.f64("hue", 0.0);
    let gamma = p.f64("gamma", 1.0).clamp(0.05, 10.0);
    let blur = p.f64("blur", 0.0);
    let sharpen = p.f64("sharpen", 0.0);
    let gray = p.bool("grayscale", false);
    let invert = p.bool("invert", false);
    batch(ctx, files, KINDS, true, |it| {
        let mut img = open(&it.path_buf())?;
        if brightness != 0.0 {
            img = img.brighten((brightness * 2.55).round() as i32);
        }
        if contrast != 0.0 {
            img = img.adjust_contrast(contrast as f32);
        }
        if saturation != 0.0 || (gamma - 1.0).abs() > 1e-3 {
            let mut rgba = img.to_rgba8();
            let s = 1.0 + saturation / 100.0;
            let g = 1.0 / gamma;
            let lut: Vec<u8> = (0..256)
                .map(|i| ((i as f64 / 255.0).powf(g) * 255.0).round().clamp(0.0, 255.0) as u8)
                .collect();
            for px in rgba.pixels_mut() {
                let (r, gg, b) = (px[0] as f64, px[1] as f64, px[2] as f64);
                let l = 0.299 * r + 0.587 * gg + 0.114 * b;
                let f = |c: f64| (l + (c - l) * s).round().clamp(0.0, 255.0) as usize;
                px[0] = lut[f(r)];
                px[1] = lut[f(gg)];
                px[2] = lut[f(b)];
            }
            img = DynamicImage::ImageRgba8(rgba);
        }
        if hue != 0.0 {
            img = img.huerotate(hue.round() as i32);
        }
        if blur > 0.0 {
            img = img.blur(blur as f32);
        }
        if sharpen > 0.0 {
            img = img.unsharpen(sharpen as f32, 1);
        }
        if gray {
            img = img.grayscale();
        }
        if invert {
            img.invert();
        }
        Ok(vec![save(ctx, it, &img, &keep_ext(it), &EncodeOpts::keep())?])
    })
}

pub fn convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "webp");
    let opts = EncodeOpts {
        quality: p.u32("quality", 82).clamp(1, 100) as u8,
        lossless: p.bool("lossless", false),
        png_optimize: p.str("png_optimize", "balanced"),
        png_colors: p.u32("png_colors", 0),
        avif_speed: p.u32("avif_speed", 6).clamp(1, 10) as u8,
        background: parse_color(&p.str("background", "#ffffff")),
    };
    let target_kb = p.f64("target_kb", 0.0);
    batch(ctx, files, KINDS, true, |it| {
        let ext = if fmt == "keep" { keep_ext(it) } else if fmt == "jpeg" { "jpg".to_string() } else { fmt.clone() };
        let img = open(&it.path_buf())?;
        let bytes = if target_kb > 0.0 && matches!(ext.as_str(), "jpg" | "webp" | "avif") {
            let (b, fits) = fit_size(&img, &ext, &opts, (target_kb * 1024.0) as usize)?;
            if !fits {
                ctx.warn(format!("{}: could not reach {} KB even at the lowest quality", it.name, target_kb));
            }
            b
        } else {
            encode(&img, &ext, &opts)?
        };
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        std::fs::write(&out, &bytes)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn watermark(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let wm_path = p.str("watermark", "");
    if wm_path.trim().is_empty() {
        bail!("Choose a watermark image (a PNG with transparency works best).");
    }
    let wm = open(Path::new(wm_path.trim()))?.to_rgba8();
    let position = p.str("position", "bottom-right");
    let scale = p.f64("scale", 20.0).clamp(1.0, 100.0);
    let opacity = (p.f64("opacity", 70.0).clamp(0.0, 100.0) / 100.0) as f32;
    let margin = p.u32("margin", 24);
    batch(ctx, files, KINDS, true, |it| {
        let mut base = open(&it.path_buf())?.to_rgba8();
        let (bw, bh) = base.dimensions();
        let tw = ((bw as f64 * scale / 100.0).round() as u32).max(1);
        let th = ((wm.height() as f64 * tw as f64 / wm.width().max(1) as f64).round() as u32).max(1);
        let mut mark = imageops::resize(&wm, tw, th, FilterType::Lanczos3);
        for px in mark.pixels_mut() {
            px[3] = (px[3] as f32 * opacity).round() as u8;
        }
        if position == "tile" {
            let gap = margin.max(8) as i64 * 2;
            let (sx, sy) = (tw as i64 + gap, th as i64 + gap);
            let mut y = 0i64;
            let mut row = 0;
            while y < bh as i64 {
                let mut x = if row % 2 == 1 { -(sx / 2) } else { 0 };
                while x < bw as i64 {
                    imageops::overlay(&mut base, &mark, x, y);
                    x += sx;
                }
                y += sy;
                row += 1;
            }
        } else {
            let (fx, fy) = anchor_frac(&position);
            let m = margin as f64;
            let x = m + (bw as f64 - tw as f64 - 2.0 * m) * fx;
            let y = m + (bh as f64 - th as f64 - 2.0 * m) * fy;
            imageops::overlay(&mut base, &mark, x.round() as i64, y.round() as i64);
        }
        Ok(vec![save(ctx, it, &DynamicImage::ImageRgba8(base), &keep_ext(it), &EncodeOpts::keep())?])
    })
}

pub fn sheet(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let (imgs, _) = split_kinds(files, KINDS);
    if imgs.is_empty() {
        bail!("No images to put on the contact sheet");
    }
    let cols = p.u32("columns", 6).max(1);
    let cell = p.u32("cell", 256).clamp(16, 4096);
    let pad = p.u32("padding", 8);
    let bg = parse_color(&p.str("background", "#1e1e22"));
    let name = p.str("name", "contact-sheet");
    let total = imgs.len();
    let done = AtomicUsize::new(0);
    let thumbs: Vec<Option<RgbaImage>> = imgs
        .par_iter()
        .map(|it| {
            let r = open(&it.path_buf()).ok().map(|i| i.thumbnail(cell, cell).to_rgba8());
            let d = done.fetch_add(1, Ordering::SeqCst) + 1;
            ctx.progress(d, total, &it.name);
            r
        })
        .collect();
    ctx.check()?;
    let n = thumbs.len() as u32;
    let rows = n.div_ceil(cols);
    let used = cols.min(n);
    let w = used * cell + (used + 1) * pad;
    let h = rows * cell + (rows + 1) * pad;
    let mut canvas = RgbaImage::from_pixel(w, h, Rgba([bg[0], bg[1], bg[2], 255]));
    for (i, t) in thumbs.iter().enumerate() {
        if let Some(t) = t {
            let (c, r) = (i as u32 % cols, i as u32 / cols);
            let x = pad + c * (cell + pad) + (cell - t.width()) / 2;
            let y = pad + r * (cell + pad) + (cell - t.height()) / 2;
            imageops::overlay(&mut canvas, t, x as i64, y as i64);
        }
    }
    let out = ctx.reserve(&ctx.out_dir("")?, &name, "png")?;
    let o = EncodeOpts { png_optimize: "fast".into(), ..EncodeOpts::keep() };
    std::fs::write(&out, encode(&DynamicImage::ImageRgba8(canvas), "png", &o)?)?;
    Ok(vec![FileItem::from_path(&out, "")])
}

fn square(img: &DynamicImage) -> DynamicImage {
    let (w, h) = img.dimensions();
    let s = w.max(h);
    let mut c = RgbaImage::new(s, s);
    imageops::overlay(&mut c, &img.to_rgba8(), ((s - w) / 2) as i64, ((s - h) / 2) as i64);
    DynamicImage::ImageRgba8(c)
}

pub fn icons(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mut sizes: Vec<u32> = p
        .str("sizes", "16,24,32,48,64,128,256")
        .split([',', ' ', ';'])
        .filter_map(|s| s.trim().parse().ok())
        .filter(|&s| s > 0 && s <= 2048)
        .collect();
    sizes.sort_unstable();
    sizes.dedup();
    if sizes.is_empty() {
        bail!("No icon sizes given");
    }
    let want_ico = p.bool("ico", true);
    let want_png = p.bool("png", true);
    batch(ctx, files, KINDS, true, |it| {
        let sq = square(&open(&it.path_buf())?);
        let mut out = vec![];
        let mut entries = vec![];
        for &s in &sizes {
            let r = sq.resize_exact(s, s, FilterType::Lanczos3);
            let bytes = encode(&r, "png", &EncodeOpts::keep())?;
            if want_png {
                let path = ctx.out_path(it, &format!("{}_{}", it.stem(), s), "png")?;
                std::fs::write(&path, &bytes)?;
                out.push(it.derive(&path));
            }
            if s <= 256 {
                entries.push((s, bytes));
            }
        }
        if want_ico && !entries.is_empty() {
            let path = ctx.out_path(it, &it.stem(), "ico")?;
            std::fs::write(&path, write_ico(&entries))?;
            out.push(it.derive(&path));
        }
        Ok(out)
    })
}

pub fn to_pdf(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let (imgs, _) = split_kinds(files, KINDS);
    if imgs.is_empty() {
        bail!("No images to put into the PDF");
    }
    let name = p.str("name", "images");
    let orientation = p.str("orientation", "auto");
    let margin = p.f64("margin_mm", 10.0);
    let q = p.u32("quality", 88).clamp(1, 100) as u8;
    let per_image = p.bool("per_image", false);
    let page_mm = match p.str("page", "A4").as_str() {
        "A4" => Some((210.0, 297.0)),
        "A3" => Some((297.0, 420.0)),
        "A5" => Some((148.0, 210.0)),
        "Letter" => Some((215.9, 279.4)),
        _ => None,
    };
    let total = imgs.len();
    let done = AtomicUsize::new(0);
    let pages: Vec<Result<pdf::PdfImage>> = imgs
        .par_iter()
        .map(|it| {
            let r = (|| -> Result<pdf::PdfImage> {
                let img = open(&it.path_buf())?;
                let rgb = flatten(&img, [255, 255, 255]);
                let (w, h) = rgb.dimensions();
                let mut jpeg = Vec::new();
                JpegEncoder::new_with_quality(&mut jpeg, q).write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)?;
                Ok(pdf::PdfImage { jpeg, width: w, height: h })
            })();
            let d = done.fetch_add(1, Ordering::SeqCst) + 1;
            ctx.progress(d, total, &it.name);
            r
        })
        .collect();
    ctx.check()?;
    let mut ok: Vec<(&FileItem, pdf::PdfImage)> = vec![];
    for (it, r) in imgs.iter().zip(pages) {
        match r {
            Ok(pg) => ok.push((it, pg)),
            Err(e) => ctx.warn(format!("{}: {:#}", it.name, e)),
        }
    }
    if ok.is_empty() {
        bail!("No image could be read. {}", ctx.first_warning());
    }
    let mut out = vec![];
    if per_image {
        for (it, pg) in ok {
            let path = ctx.out_path(it, &it.stem(), "pdf")?;
            std::fs::write(&path, pdf::build(std::slice::from_ref(&pg), page_mm, &orientation, margin))?;
            out.push(it.derive(&path));
        }
    } else {
        let pgs: Vec<pdf::PdfImage> = ok.into_iter().map(|(_, pg)| pg).collect();
        let path = ctx.reserve(&ctx.out_dir("")?, &name, "pdf")?;
        std::fs::write(&path, pdf::build(&pgs, page_mm, &orientation, margin))?;
        out.push(FileItem::from_path(&path, ""));
    }
    Ok(out)
}
