//! More image nodes: frame & corners, effects, text, EXIF metadata, tiles, collage, palette,
//! QR codes, Base64, similar images, background removal, GIF animation and frame extraction.
use super::{batch, split_kinds, Outs};
use crate::engine::{Ctx, FileItem};
use crate::ops::img::{self, EncodeOpts};
use crate::util::{command, parse_color, sanitize, Params};
use anyhow::{anyhow, bail, Result};
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, DynamicImage, GenericImageView, Rgba, RgbaImage};
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, OnceLock};

static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();

fn fonts() -> Arc<resvg::usvg::fontdb::Database> {
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_system_fonts();
            let pick = ["Segoe UI", "Inter", "Arial", "Noto Sans", "DejaVu Sans", "Liberation Sans", "Ubuntu", "Cantarell", "Roboto"]
                .iter()
                .find(|c| db.faces().any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(c))))
                .map(|s| s.to_string());
            if let Some(n) = pick {
                db.set_sans_serif_family(n);
            }
            Arc::new(db)
        })
        .clone()
}

pub fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Renders an SVG document to straight-alpha RGBA.
pub fn svg_rgba(svg: &str) -> Result<RgbaImage> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fonts();
    let tree = resvg::usvg::Tree::from_str(svg, &opt)?;
    let size = tree.size();
    let w = (size.width().ceil() as u32).clamp(1, 16384);
    let h = (size.height().ceil() as u32).clamp(1, 16384);
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h).ok_or_else(|| anyhow!("image is too large"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pm.as_mut());
    let mut px = pm.take();
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in p.iter_mut().take(3) {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    RgbaImage::from_raw(w, h, px).ok_or_else(|| anyhow!("bad image buffer"))
}

/// Opens any image; formats the image crate can't read (HEIC, AVIF, JPEG XL, PSD...) go through FFmpeg.
pub fn open_any(ctx: &Ctx, path: &Path) -> Result<DynamicImage> {
    match img::open(path) {
        Ok(i) => Ok(i),
        Err(e) => {
            let Some(ff) = ctx.tools.ffmpeg.as_ref() else {
                return Err(e.context("install FFmpeg to open more image formats (HEIC, AVIF, JPEG XL, PSD)"));
            };
            let dir = ctx.out_dir("_decode")?;
            let tmp = ctx.reserve(&dir, &crate::util::file_stem(path), "png")?;
            let st = command(ff)
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(path)
                .args(["-frames:v", "1"])
                .arg(&tmp)
                .status()?;
            if !st.success() || !tmp.is_file() {
                return Err(e.context("FFmpeg could not decode it either"));
            }
            let im = image::open(&tmp)?;
            let _ = std::fs::remove_file(&tmp);
            Ok(im)
        }
    }
}

fn trim_alpha(img: &RgbaImage, pad: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 6 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 || y0 > y1 {
        return RgbaImage::new(1, 1);
    }
    let x0 = x0.saturating_sub(pad);
    let y0 = y0.saturating_sub(pad);
    let x1 = (x1 + pad).min(w - 1);
    let y1 = (y1 + pad).min(h - 1);
    imageops::crop_imm(img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
}

/// Anti-aliased rounded corners (multiplies alpha).
fn round_corners(img: &mut RgbaImage, r: u32) {
    let (w, h) = img.dimensions();
    let r = r.min(w / 2).min(h / 2);
    if r == 0 {
        return;
    }
    let rf = r as f32;
    for y in 0..h {
        for x in 0..w {
            let cx = if x < r { rf } else if x >= w - r { (w - r) as f32 } else { continue };
            let cy = if y < r { rf } else if y >= h - r { (h - r) as f32 } else { continue };
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let cov = (rf - d + 0.5).clamp(0.0, 1.0);
            let p = img.get_pixel_mut(x, y);
            p[3] = (p[3] as f32 * cov).round() as u8;
        }
    }
}

fn set_opacity(img: &mut RgbaImage, op: f32) {
    if op >= 0.999 {
        return;
    }
    for p in img.pixels_mut() {
        p[3] = (p[3] as f32 * op.clamp(0.0, 1.0)).round() as u8;
    }
}

/// Renders (multi-line) text with system fonts, trimmed to the ink with a little padding.
pub fn text_image(text: &str, px: f32, color: &str, font: &str, bold: bool, shadow: bool, bg: Option<&str>) -> Result<RgbaImage> {
    let lines: Vec<&str> = text.split('\n').collect();
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1).max(1) as f32;
    let pad = (px * 0.6).ceil();
    let w = (longest * px * 0.78 + pad * 2.0 + px).ceil();
    let lh = px * 1.25;
    let h = (lines.len() as f32 * lh + pad * 2.0).ceil();
    let [r, g, b] = parse_color(color);
    let fam = xml_esc(font.trim());
    let weight = if bold { 700 } else { 400 };
    let (defs, fattr) = if shadow {
        (
            format!(
                r#"<filter id="s" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="{:.2}" stdDeviation="{:.2}" flood-color="#000000" flood-opacity="0.55"/></filter>"#,
                px * 0.05,
                px * 0.08
            ),
            r#" filter="url(#s)""#,
        )
    } else {
        (String::new(), "")
    };
    let mut body = String::new();
    for (i, l) in lines.iter().enumerate() {
        let y = pad + px + i as f32 * lh;
        body.push_str(&format!(r#"<text x="{pad}" y="{y:.1}" xml:space="preserve">{}</text>"#, xml_esc(l)));
    }
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><defs>{defs}</defs><g font-family="'{fam}', sans-serif" font-size="{px}" font-weight="{weight}" fill="rgb({r},{g},{b})"{fattr}>{body}</g></svg>"#
    );
    let ink = trim_alpha(&svg_rgba(&svg)?, (px * 0.12) as u32);
    let Some(bgc) = bg else { return Ok(ink) };
    let [br, bgg, bb] = parse_color(bgc);
    let p2 = (px * 0.35).round() as u32;
    let mut boxed = RgbaImage::from_pixel(ink.width() + 2 * p2, ink.height() + 2 * p2, Rgba([br, bgg, bb, 190]));
    round_corners(&mut boxed, (px * 0.3) as u32);
    imageops::overlay(&mut boxed, &ink, p2 as i64, p2 as i64);
    Ok(boxed)
}

fn anchor(pos: &str, cw: u32, ch: u32, w: u32, h: u32, m: u32) -> (i64, i64) {
    let (fx, fy) = match pos {
        "top-left" => (0.0, 0.0),
        "top" => (0.5, 0.0),
        "top-right" => (1.0, 0.0),
        "left" => (0.0, 0.5),
        "center" => (0.5, 0.5),
        "right" => (1.0, 0.5),
        "bottom-left" => (0.0, 1.0),
        "bottom" => (0.5, 1.0),
        _ => (1.0, 1.0),
    };
    let free_w = cw as f64 - w as f64 - 2.0 * m as f64;
    let free_h = ch as f64 - h as f64 - 2.0 * m as f64;
    ((m as f64 + free_w * fx).round() as i64, (m as f64 + free_h * fy).round() as i64)
}

fn out_ext(it: &FileItem, fmt: &str, alpha: bool) -> String {
    let e = if fmt == "keep" || fmt.is_empty() { img::keep_ext(it) } else { fmt.to_string() };
    if alpha && matches!(e.as_str(), "jpg" | "jpeg" | "bmp" | "pnm" | "ppm") {
        "png".into()
    } else {
        e
    }
}

fn opts(q: i64) -> EncodeOpts {
    let mut o = EncodeOpts::keep();
    o.quality = q.clamp(1, 100) as u8;
    o
}

fn save_named(ctx: &Ctx, it: &FileItem, stem: &str, im: &DynamicImage, ext: &str, q: i64) -> Result<FileItem> {
    let ext = if img::encodable(ext) { ext.to_string() } else { "png".to_string() };
    let bytes = img::encode(im, &ext, &opts(q))?;
    let out = ctx.out_path(it, stem, &ext)?;
    std::fs::write(&out, bytes)?;
    Ok(it.derive(&out))
}

pub fn frame(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let unit = p.str("unit", "px");
    let pad = p.f64("padding", 40.0).max(0.0);
    let border = p.f64("border", 0.0).max(0.0);
    let bc = parse_color(&p.str("border_color", "#ffffff"));
    let bg = parse_color(&p.str("background", "#ffffff"));
    let transparent = p.bool("transparent", false);
    let radius = p.f64("radius", 0.0).max(0.0);
    let shadow = p.f64("shadow", 0.0).clamp(0.0, 200.0);
    let fmt = p.str("format", "keep");
    let q = p.i64("quality", 92);
    batch(ctx, files, &["image"], true, |it| {
        let src = open_any(ctx, &it.path_buf())?.to_rgba8();
        let (w, h) = src.dimensions();
        let base = w.min(h) as f64;
        let px = |v: f64| -> u32 { if unit == "%" { (v / 100.0 * base).round() as u32 } else { v.round() as u32 } };
        let (pd, bd, rd, sh) = (px(pad), px(border), px(radius), px(shadow));
        let mut pic = src;
        round_corners(&mut pic, rd.saturating_sub(bd));
        if bd > 0 {
            let mut b = RgbaImage::from_pixel(w + 2 * bd, h + 2 * bd, Rgba([bc[0], bc[1], bc[2], 255]));
            round_corners(&mut b, rd);
            imageops::overlay(&mut b, &pic, bd as i64, bd as i64);
            pic = b;
        }
        let (cw, ch) = (pic.width() + 2 * pd, pic.height() + 2 * pd);
        let fill = if transparent { Rgba([0, 0, 0, 0]) } else { Rgba([bg[0], bg[1], bg[2], 255]) };
        let mut canvas = RgbaImage::from_pixel(cw, ch, fill);
        if sh > 0 {
            let mut mask = RgbaImage::from_pixel(pic.width(), pic.height(), Rgba([0, 0, 0, 0]));
            for (x, y, p) in pic.enumerate_pixels() {
                mask.put_pixel(x, y, Rgba([0, 0, 0, (p[3] as f32 * 0.5) as u8]));
            }
            let (mw, mh) = ((mask.width() / 4).max(1), (mask.height() / 4).max(1));
            let small = imageops::resize(&mask, mw, mh, FilterType::Triangle);
            let mut big = RgbaImage::from_pixel(mw + sh, mh + sh, Rgba([0, 0, 0, 0]));
            imageops::overlay(&mut big, &small, (sh / 2) as i64, (sh / 2) as i64);
            let blurred = imageops::blur(&big, (sh as f32 / 8.0).max(0.5));
            let full = imageops::resize(&blurred, (mw + sh) * 4, (mh + sh) * 4, FilterType::Triangle);
            let off = (sh as f64 * 0.25) as i64;
            imageops::overlay(&mut canvas, &full, pd as i64 - (sh * 2) as i64 + off, pd as i64 - (sh * 2) as i64 + off * 2);
        }
        imageops::overlay(&mut canvas, &pic, pd as i64, pd as i64);
        let alpha = transparent;
        let ext = out_ext(it, &fmt, alpha);
        save_named(ctx, it, &it.stem(), &DynamicImage::ImageRgba8(canvas), &ext, q).map(|f| vec![f])
    })
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn luma(p: &Rgba<u8>) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

fn mix(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

fn rgb_filter(img: &RgbaImage, kernel: &[f32]) -> RgbaImage {
    let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let f = imageops::filter3x3(&rgb, kernel);
    let mut out = img.clone();
    for (x, y, p) in out.enumerate_pixels_mut() {
        let q = f.get_pixel(x, y);
        p[0] = q[0];
        p[1] = q[1];
        p[2] = q[2];
    }
    out
}

pub fn effects(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fx = p.str("effect", "grayscale");
    let amount = (p.f64("amount", 100.0) / 100.0).clamp(0.0, 1.0) as f32;
    let size = p.f64("size", 8.0).max(0.0) as f32;
    let c1 = parse_color(&p.str("color1", "#1b2b6b"));
    let c2 = parse_color(&p.str("color2", "#ffd9a0"));
    let fmt = p.str("format", "keep");
    let q = p.i64("quality", 92);
    batch(ctx, files, &["image"], true, |it| {
        let orig = open_any(ctx, &it.path_buf())?.to_rgba8();
        let (w, h) = orig.dimensions();
        let mut img = orig.clone();
        let mut mixable = true;
        match fx.as_str() {
            "grayscale" => img.pixels_mut().for_each(|p| {
                let l = luma(p).round() as u8;
                p[0] = l;
                p[1] = l;
                p[2] = l;
            }),
            "sepia" => img.pixels_mut().for_each(|p| {
                let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
                p[0] = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0) as u8;
                p[1] = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0) as u8;
                p[2] = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0) as u8;
            }),
            "invert" => img.pixels_mut().for_each(|p| {
                p[0] = 255 - p[0];
                p[1] = 255 - p[1];
                p[2] = 255 - p[2];
            }),
            "solarize" => img.pixels_mut().for_each(|p| {
                for c in 0..3 {
                    if p[c] > 128 {
                        p[c] = 255 - p[c];
                    }
                }
            }),
            "posterize" => {
                let levels = size.clamp(2.0, 64.0);
                let step = 255.0 / (levels - 1.0);
                img.pixels_mut().for_each(|p| {
                    for c in 0..3 {
                        p[c] = ((p[c] as f32 / step).round() * step).clamp(0.0, 255.0) as u8;
                    }
                });
            }
            "threshold" => {
                let t = amount * 255.0;
                mixable = false;
                img.pixels_mut().for_each(|p| {
                    let v = if luma(p) >= t { 255 } else { 0 };
                    p[0] = v;
                    p[1] = v;
                    p[2] = v;
                });
            }
            "duotone" => img.pixels_mut().for_each(|p| {
                let l = luma(p) / 255.0;
                p[0] = mix(c1[0], c2[0], l);
                p[1] = mix(c1[1], c2[1], l);
                p[2] = mix(c1[2], c2[2], l);
            }),
            "tint" => img.pixels_mut().for_each(|p| {
                let l = luma(p) / 255.0;
                p[0] = (c1[0] as f32 * l).round() as u8;
                p[1] = (c1[1] as f32 * l).round() as u8;
                p[2] = (c1[2] as f32 * l).round() as u8;
            }),
            "pixelate" => {
                let b = size.max(2.0) as u32;
                let small = imageops::resize(&img, (w / b).max(1), (h / b).max(1), FilterType::Triangle);
                img = imageops::resize(&small, w, h, FilterType::Nearest);
            }
            "blur" => {
                img = imageops::blur(&img, size.max(0.3));
            }
            "sharpen" => {
                img = imageops::unsharpen(&img, (size / 4.0).clamp(0.3, 10.0), 2);
            }
            "emboss" => img = rgb_filter(&img, &[-2.0, -1.0, 0.0, -1.0, 1.0, 1.0, 0.0, 1.0, 2.0]),
            "edges" => {
                img = rgb_filter(&img, &[-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0]);
            }
            "vignette" => {
                mixable = false;
                let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
                let maxd = (cx * cx + cy * cy).sqrt();
                for (x, y, p) in img.enumerate_pixels_mut() {
                    let d = (((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / maxd).clamp(0.0, 1.0);
                    let t = ((d - 0.35) / 0.65).clamp(0.0, 1.0);
                    let f = 1.0 - amount * t * t * (3.0 - 2.0 * t);
                    for c in 0..3 {
                        p[c] = (p[c] as f32 * f) as u8;
                    }
                }
            }
            "noise" => {
                mixable = false;
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (w as u64) << 20 ^ h as u64);
                let s = amount * 70.0;
                for p in img.pixels_mut() {
                    let n = (rng.next() - 0.5) * 2.0 * s;
                    for c in 0..3 {
                        p[c] = (p[c] as f32 + n).clamp(0.0, 255.0) as u8;
                    }
                }
            }
            "flatten" => {
                mixable = false;
                for p in img.pixels_mut() {
                    let a = p[3] as f32 / 255.0;
                    for c in 0..3 {
                        p[c] = (p[c] as f32 * a + c1[c] as f32 * (1.0 - a)).round() as u8;
                    }
                    p[3] = 255;
                }
            }
            "alpha-mask" => {
                mixable = false;
                for p in img.pixels_mut() {
                    let a = p[3];
                    *p = Rgba([a, a, a, 255]);
                }
            }
            other => bail!("unknown effect {other}"),
        }
        if mixable && amount < 0.999 {
            for (x, y, p) in img.enumerate_pixels_mut() {
                let o = orig.get_pixel(x, y);
                for c in 0..3 {
                    p[c] = mix(o[c], p[c], amount);
                }
            }
        }
        let alpha = img.pixels().any(|p| p[3] < 255);
        let ext = out_ext(it, &fmt, alpha);
        save_named(ctx, it, &it.stem(), &DynamicImage::ImageRgba8(img), &ext, q).map(|f| vec![f])
    })
}

pub fn text(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let pattern = p.str("text", "{name}");
    let size = p.f64("size", 6.0).clamp(0.5, 100.0);
    let color = p.str("color", "#ffffff");
    let font = p.str("font", "sans-serif");
    let bold = p.bool("bold", true);
    let shadow = p.bool("shadow", true);
    let boxed = p.bool("box", false);
    let box_color = p.str("box_color", "#000000");
    let opacity = (p.f64("opacity", 100.0) / 100.0) as f32;
    let pos = p.str("position", "bottom");
    let margin = p.f64("margin", 4.0).clamp(0.0, 45.0);
    let fmt = p.str("format", "keep");
    let q = p.i64("quality", 92);
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    batch(ctx, files, &["image"], true, |it| {
        let mut im = open_any(ctx, &it.path_buf())?.to_rgba8();
        let (w, h) = im.dimensions();
        let t = pattern
            .replace("{name}", &it.stem())
            .replace("{ext}", &it.ext)
            .replace("{w}", &w.to_string())
            .replace("{h}", &h.to_string())
            .replace("{date}", &date)
            .replace("\\n", "\n");
        let px = (h as f64 * size / 100.0).max(6.0) as f32;
        let mut label = text_image(&t, px, &color, &font, bold, shadow, if boxed { Some(box_color.as_str()) } else { None })?;
        let m = (margin / 100.0 * w.min(h) as f64) as u32;
        let maxw = w.saturating_sub(2 * m).max(8);
        if label.width() > maxw {
            let nh = (label.height() as f64 * maxw as f64 / label.width() as f64).max(1.0) as u32;
            label = imageops::resize(&label, maxw, nh, FilterType::Lanczos3);
        }
        set_opacity(&mut label, opacity);
        let (x, y) = anchor(&pos, w, h, label.width(), label.height(), m);
        imageops::overlay(&mut im, &label, x, y);
        let ext = out_ext(it, &fmt, false);
        save_named(ctx, it, &it.stem(), &DynamicImage::ImageRgba8(im), &ext, q).map(|f| vec![f])
    })
}

fn exif_rows(path: &Path) -> Result<Vec<(String, String)>> {
    let file = std::fs::File::open(path)?;
    let mut r = BufReader::new(file);
    let ex = exif::Reader::new().read_from_container(&mut r).map_err(|e| anyhow!("{e}"))?;
    let mut rows = vec![];
    for f in ex.fields() {
        let v = f.display_value().with_unit(&ex).to_string();
        rows.push((f.tag.to_string(), v.trim_matches('"').to_string()));
    }
    let gps = |tag: exif::Tag, rtag: exif::Tag| -> Option<f64> {
        let f = ex.get_field(tag, exif::In::PRIMARY)?;
        let v = match &f.value {
            exif::Value::Rational(v) if v.len() >= 3 => v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0,
            _ => return None,
        };
        let neg = ex
            .get_field(rtag, exif::In::PRIMARY)
            .map(|r| matches!(r.display_value().to_string().trim_matches('"'), "S" | "W"))
            .unwrap_or(false);
        Some(if neg { -v } else { v })
    };
    if let (Some(lat), Some(lon)) = (gps(exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef), gps(exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef)) {
        rows.push(("GPS".into(), format!("{lat:.6}, {lon:.6}")));
    }
    Ok(rows)
}

/// "Date taken" from EXIF as `YYYY-MM-DD HH:MM:SS` (None when missing).
pub fn date_taken(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut r = BufReader::new(file);
    let ex = exif::Reader::new().read_from_container(&mut r).ok()?;
    for tag in [exif::Tag::DateTimeOriginal, exif::Tag::DateTimeDigitized, exif::Tag::DateTime] {
        if let Some(f) = ex.get_field(tag, exif::In::PRIMARY) {
            let s = f.display_value().to_string();
            let s = s.trim_matches('"').trim();
            if s.len() >= 10 {
                return Some(s.to_string());
            }
        }
    }
    None
}

pub fn metadata(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let mode = p.str("mode", "report");
    let fmt = p.str("format", "csv");
    if mode == "strip" {
        let q = p.i64("quality", 95);
        let out = batch(ctx, files, &["image"], true, |it| {
            let im = open_any(ctx, &it.path_buf())?;
            let ext = out_ext(it, "keep", false);
            save_named(ctx, it, &it.stem(), &im, &ext, q).map(|f| vec![f])
        })?;
        return Ok(vec![out, vec![]]);
    }
    let imgs: Vec<&FileItem> = files.iter().filter(|f| f.kind == "image").collect();
    if imgs.is_empty() {
        bail!("No images in the input");
    }
    let rows: Vec<(String, Vec<(String, String)>)> = imgs
        .par_iter()
        .map(|it| {
            let mut r = vec![];
            if let Ok((w, h)) = image::image_dimensions(it.path_buf()) {
                r.push(("Width".to_string(), w.to_string()));
                r.push(("Height".to_string(), h.to_string()));
            }
            r.extend(exif_rows(&it.path_buf()).unwrap_or_default());
            (it.name.clone(), r)
        })
        .collect();
    ctx.check()?;
    let (body, ext) = match fmt.as_str() {
        "json" => {
            let list: Vec<Value> = rows
                .iter()
                .map(|(n, r)| {
                    let mut m = serde_json::Map::new();
                    m.insert("file".into(), json!(n));
                    for (k, v) in r {
                        m.entry(k.clone()).or_insert(json!(v));
                    }
                    Value::Object(m)
                })
                .collect();
            (serde_json::to_string_pretty(&list)?, "json")
        }
        "txt" => {
            let mut s = String::new();
            for (n, r) in &rows {
                s.push_str(&format!("== {n} ==\n"));
                for (k, v) in r {
                    s.push_str(&format!("{k:>28}: {v}\n"));
                }
                s.push('\n');
            }
            (s, "txt")
        }
        _ => {
            let cols = [
                "Width", "Height", "Make", "Model", "LensModel", "DateTimeOriginal", "ExposureTime", "FNumber",
                "PhotographicSensitivity", "FocalLength", "Orientation", "Software", "GPS",
            ];
            let mut w = csv::Writer::from_writer(vec![]);
            let mut head = vec!["file"];
            head.extend(cols);
            w.write_record(&head)?;
            for (n, r) in &rows {
                let map: HashMap<&str, &str> = r.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                let mut rec = vec![n.clone()];
                rec.extend(cols.iter().map(|c| map.get(c).map(|s| s.to_string()).unwrap_or_default()));
                w.write_record(&rec)?;
            }
            let bytes = w.into_inner().map_err(|e| anyhow!("csv: {e}"))?;
            (String::from_utf8(bytes)?, "csv")
        }
    };
    let out = ctx.reserve(&ctx.out_dir("")?, "image-metadata", ext)?;
    std::fs::write(&out, body)?;
    Ok(vec![files, vec![FileItem::from_path(&out, "")]])
}

pub fn tiles(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "grid");
    let cols = p.i64("columns", 3).clamp(1, 100) as u32;
    let rows = p.i64("rows", 3).clamp(1, 100) as u32;
    let tw = p.i64("tile_width", 512).clamp(1, 16384) as u32;
    let th = p.i64("tile_height", 512).clamp(1, 16384) as u32;
    let fmt = p.str("format", "keep");
    let q = p.i64("quality", 92);
    batch(ctx, files, &["image"], true, |it| {
        let im = open_any(ctx, &it.path_buf())?;
        let (w, h) = im.dimensions();
        let (cw, ch) = if mode == "size" { (tw.min(w), th.min(h)) } else { ((w / cols).max(1), (h / rows).max(1)) };
        let (nx, ny) = if mode == "size" { (w.div_ceil(cw), h.div_ceil(ch)) } else { (cols, rows) };
        let ext = out_ext(it, &fmt, im.color().has_alpha());
        let mut out = vec![];
        for r in 0..ny {
            for c in 0..nx {
                let (x, y) = (c * cw, r * ch);
                if x >= w || y >= h {
                    continue;
                }
                let tile = im.crop_imm(x, y, cw.min(w - x), ch.min(h - y));
                out.push(save_named(ctx, it, &format!("{}_r{:02}_c{:02}", it.stem(), r + 1, c + 1), &tile, &ext, q)?);
            }
        }
        Ok(out)
    })
}

pub fn collage(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let (imgs, pass) = split_kinds(files, &["image"]);
    if imgs.is_empty() {
        bail!("No images in the input");
    }
    let layout = p.str("layout", "grid");
    let cols = p.i64("columns", 3).clamp(1, 50) as u32;
    let cell = p.i64("cell", 600).clamp(16, 8192) as u32;
    let gap = p.i64("gap", 16).clamp(0, 1000) as u32;
    let fit = p.str("fit", "cover");
    let transparent = p.bool("transparent", false);
    let bg = parse_color(&p.str("background", "#ffffff"));
    let radius = p.i64("radius", 0).clamp(0, 4000) as u32;
    let name = p.str("name", "collage");
    let fmt = p.str("format", "jpg");
    let q = p.i64("quality", 90);
    let loaded: Vec<Option<RgbaImage>> = imgs
        .par_iter()
        .map(|it| match open_any(ctx, &it.path_buf()) {
            Ok(i) => Some(i.to_rgba8()),
            Err(e) => {
                ctx.warn(format!("{}: {:#}", it.name, e));
                None
            }
        })
        .collect();
    ctx.check()?;
    let pics: Vec<RgbaImage> = loaded.into_iter().flatten().collect();
    if pics.is_empty() {
        bail!("No readable images. {}", ctx.first_warning());
    }
    let fill = if transparent { Rgba([0, 0, 0, 0]) } else { Rgba([bg[0], bg[1], bg[2], 255]) };
    let mut tiles: Vec<RgbaImage> = vec![];
    for pic in &pics {
        let (w, h) = pic.dimensions();
        let mut t = match layout.as_str() {
            "horizontal" => imageops::resize(pic, ((w as f64 * cell as f64 / h as f64).round() as u32).max(1), cell, FilterType::Lanczos3),
            "vertical" => imageops::resize(pic, cell, ((h as f64 * cell as f64 / w as f64).round() as u32).max(1), FilterType::Lanczos3),
            _ => {
                if fit == "contain" {
                    let s = (cell as f64 / w as f64).min(cell as f64 / h as f64);
                    let r = imageops::resize(pic, ((w as f64 * s) as u32).max(1), ((h as f64 * s) as u32).max(1), FilterType::Lanczos3);
                    let mut c = RgbaImage::from_pixel(cell, cell, fill);
                    imageops::overlay(&mut c, &r, ((cell - r.width()) / 2) as i64, ((cell - r.height()) / 2) as i64);
                    c
                } else {
                    let s = (cell as f64 / w as f64).max(cell as f64 / h as f64);
                    let r = imageops::resize(pic, ((w as f64 * s).ceil() as u32).max(cell), ((h as f64 * s).ceil() as u32).max(cell), FilterType::Lanczos3);
                    imageops::crop_imm(&r, (r.width() - cell) / 2, (r.height() - cell) / 2, cell, cell).to_image()
                }
            }
        };
        round_corners(&mut t, radius);
        tiles.push(t);
    }
    let n = tiles.len() as u32;
    let (cw, ch) = match layout.as_str() {
        "horizontal" => (tiles.iter().map(|t| t.width()).sum::<u32>() + gap * (n + 1), cell + 2 * gap),
        "vertical" => (cell + 2 * gap, tiles.iter().map(|t| t.height()).sum::<u32>() + gap * (n + 1)),
        _ => {
            let c = cols.min(n);
            let r = n.div_ceil(c);
            (c * cell + gap * (c + 1), r * cell + gap * (r + 1))
        }
    };
    let mut canvas = RgbaImage::from_pixel(cw, ch, fill);
    let (mut x, mut y) = (gap, gap);
    for (i, t) in tiles.iter().enumerate() {
        match layout.as_str() {
            "horizontal" => {
                imageops::overlay(&mut canvas, t, x as i64, gap as i64);
                x += t.width() + gap;
            }
            "vertical" => {
                imageops::overlay(&mut canvas, t, gap as i64, y as i64);
                y += t.height() + gap;
            }
            _ => {
                let c = cols.min(n);
                let (cx, cy) = (i as u32 % c, i as u32 / c);
                imageops::overlay(&mut canvas, t, (gap + cx * (cell + gap)) as i64, (gap + cy * (cell + gap)) as i64);
            }
        }
    }
    let ext = if transparent && fmt == "jpg" { "png".to_string() } else { fmt.clone() };
    let first = &imgs[0];
    let mut base = first.clone();
    base.name = format!("{name}.{ext}");
    let mut f = save_named(ctx, &base, &name, &DynamicImage::ImageRgba8(canvas), &ext, q)?;
    f.origin = f.path.clone();
    f.origin_size = f.size;
    let mut v = vec![f];
    v.extend(pass);
    Ok(v)
}

pub fn palette(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let n = p.i64("colors", 8).clamp(2, 32) as usize;
    let fmt = p.str("format", "png");
    batch(ctx, files, &["image"], true, |it| {
        let im = open_any(ctx, &it.path_buf())?.thumbnail(220, 220).to_rgba8();
        let px: Vec<u8> = im.pixels().filter(|p| p[3] > 127).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        if px.len() < 4 * n {
            bail!("the image has too few opaque pixels");
        }
        let nq = color_quant::NeuQuant::new(10, n, &px);
        let map = nq.color_map_rgba();
        let mut counts = vec![0usize; n];
        for c in px.chunks_exact(4) {
            let i = nq.index_of(c);
            if i < n {
                counts[i] += 1;
            }
        }
        let mut colors: Vec<([u8; 3], usize)> = (0..n).map(|i| ([map[i * 4], map[i * 4 + 1], map[i * 4 + 2]], counts[i])).filter(|c| c.1 > 0).collect();
        colors.sort_by(|a, b| b.1.cmp(&a.1));
        colors.dedup_by(|a, b| a.0 == b.0);
        let total: usize = colors.iter().map(|c| c.1).sum::<usize>().max(1);
        let hex = |c: &[u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        let stem = format!("{}_palette", it.stem());
        let (body, ext): (Vec<u8>, &str) = match fmt.as_str() {
            "json" => {
                let list: Vec<Value> = colors.iter().map(|(c, k)| json!({"hex": hex(c), "rgb": c, "share": (*k as f64 / total as f64 * 1000.0).round() / 10.0})).collect();
                (serde_json::to_string_pretty(&list)?.into_bytes(), "json")
            }
            "css" => {
                let mut s = String::from(":root {\n");
                for (i, (c, _)) in colors.iter().enumerate() {
                    s.push_str(&format!("  --color-{}: {};\n", i + 1, hex(c)));
                }
                s.push_str("}\n");
                (s.into_bytes(), "css")
            }
            "gpl" => {
                let mut s = format!("GIMP Palette\nName: {}\nColumns: {}\n#\n", it.stem(), colors.len());
                for (c, _) in &colors {
                    s.push_str(&format!("{:3} {:3} {:3}\t{}\n", c[0], c[1], c[2], hex(c)));
                }
                (s.into_bytes(), "gpl")
            }
            "txt" => (colors.iter().map(|(c, _)| hex(c)).collect::<Vec<_>>().join("\n").into_bytes(), "txt"),
            _ => {
                let sw = 160;
                let mut svg = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="220"><rect width="100%" height="100%" fill="#ffffff"/>"#,
                    sw * colors.len()
                );
                for (i, (c, _)) in colors.iter().enumerate() {
                    let x = i * sw;
                    let l = 0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64;
                    let _ = l;
                    svg.push_str(&format!(
                        r#"<rect x="{x}" y="0" width="{sw}" height="170" fill="{h}"/><text x="{tx}" y="203" font-family="monospace" font-size="20" text-anchor="middle" fill="#222222">{h}</text>"#,
                        h = hex(c),
                        tx = x + sw / 2
                    ));
                }
                svg.push_str("</svg>");
                if fmt == "svg" {
                    (svg.into_bytes(), "svg")
                } else {
                    let im = svg_rgba(&svg)?;
                    (img::encode(&DynamicImage::ImageRgba8(im), "png", &EncodeOpts::keep())?, "png")
                }
            }
        };
        let out = ctx.out_path(it, &stem, ext)?;
        std::fs::write(&out, body)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn qr(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    use qrcode::{EcLevel, QrCode};
    let text = p.str("text", "https://github.com/kabiriyer3-cmyk/morph");
    let per_line = p.bool("per_line", false);
    let size = p.i64("size", 512).clamp(64, 8192) as u32;
    let margin = p.i64("margin", 4).clamp(0, 40) as u32;
    let fg = parse_color(&p.str("color", "#000000"));
    let bg = parse_color(&p.str("background", "#ffffff"));
    let transparent = p.bool("transparent", false);
    let ec = match p.str("ec", "M").as_str() {
        "L" => EcLevel::L,
        "Q" => EcLevel::Q,
        "H" => EcLevel::H,
        _ => EcLevel::M,
    };
    let fmt = p.str("format", "png");
    let mut payloads: Vec<String> = vec![];
    for it in files.iter().filter(|f| crate::kinds::is_textual(&f.kind, &f.ext)) {
        let bytes = std::fs::read(&it.path)?;
        let (s, _) = crate::ops::data::decode(&bytes, "auto")?;
        if per_line {
            payloads.extend(s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
        } else if !s.trim().is_empty() {
            payloads.push(s.trim().to_string());
        }
    }
    if payloads.is_empty() {
        if per_line {
            payloads.extend(text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
        } else if !text.trim().is_empty() {
            payloads.push(text.trim().to_string());
        }
    }
    if payloads.is_empty() {
        bail!("Type the text or link for the QR code");
    }
    let dir = ctx.out_dir("")?;
    let total = payloads.len();
    let mut out = vec![];
    for (i, data) in payloads.iter().enumerate() {
        ctx.check()?;
        let code = QrCode::with_error_correction_level(data.as_bytes(), ec).map_err(|e| anyhow!("QR: {e:?} (the text may be too long)"))?;
        let w = code.width() as u32;
        let dark: Vec<bool> = code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect();
        let full = w + 2 * margin;
        let short: String = data.chars().filter(|c| c.is_alphanumeric()).take(24).collect();
        let stem = if total == 1 && short.is_empty() { "qr".to_string() } else { format!("qr_{:03}_{}", i + 1, sanitize(&short)) };
        let path = if fmt == "svg" {
            let mut d = String::new();
            for y in 0..w {
                for x in 0..w {
                    if dark[(y * w + x) as usize] {
                        d.push_str(&format!("M{} {}h1v1h-1z", x + margin, y + margin));
                    }
                }
            }
            let bgr = if transparent { String::new() } else { format!(r#"<rect width="100%" height="100%" fill="rgb({},{},{})"/>"#, bg[0], bg[1], bg[2]) };
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {full} {full}" width="{size}" height="{size}" shape-rendering="crispEdges">{bgr}<path d="{d}" fill="rgb({},{},{})"/></svg>"#,
                fg[0], fg[1], fg[2]
            );
            let path = ctx.reserve(&dir, &stem, "svg")?;
            std::fs::write(&path, svg)?;
            path
        } else {
            let m = (size / full).max(1);
            let side = m * full;
            let back = if transparent { Rgba([0, 0, 0, 0]) } else { Rgba([bg[0], bg[1], bg[2], 255]) };
            let mut im = RgbaImage::from_pixel(side, side, back);
            for y in 0..w {
                for x in 0..w {
                    if dark[(y * w + x) as usize] {
                        for dy in 0..m {
                            for dx in 0..m {
                                im.put_pixel((x + margin) * m + dx, (y + margin) * m + dy, Rgba([fg[0], fg[1], fg[2], 255]));
                            }
                        }
                    }
                }
            }
            let ext = if transparent && fmt == "jpg" { "png" } else { fmt.as_str() };
            let ext = if img::encodable(ext) { ext } else { "png" };
            let path = ctx.reserve(&dir, &stem, ext)?;
            std::fs::write(&path, img::encode(&DynamicImage::ImageRgba8(im), ext, &EncodeOpts::keep())?)?;
            path
        };
        out.push(FileItem::from_path(&path, ""));
        ctx.progress(i + 1, total, "");
    }
    Ok(out)
}

fn mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" | "jfif" | "jpe" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        _ => "application/octet-stream",
    }
}

pub fn base64(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    use base64::Engine;
    let fmt = p.str("format", "datauri");
    let combine = p.bool("combine", true);
    let imgs: Vec<&FileItem> = files.iter().filter(|f| f.kind == "image").collect();
    if imgs.is_empty() {
        bail!("No images in the input");
    }
    let mut entries: Vec<(&FileItem, String)> = vec![];
    for it in &imgs {
        ctx.check()?;
        let bytes = std::fs::read(&it.path)?;
        let b = base64::engine::general_purpose::STANDARD.encode(&bytes);
        entries.push((it, b));
    }
    let class = |it: &FileItem| {
        let s: String = it.stem().chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        format!("img-{}", s.trim_matches('-'))
    };
    let render = |list: &[(&FileItem, String)]| -> (String, &'static str) {
        let uri = |it: &FileItem, b: &str| format!("data:{};base64,{}", mime(&it.ext), b);
        match fmt.as_str() {
            "css" => (
                list.iter().map(|(it, b)| format!(".{} {{ background-image: url(\"{}\"); }}\n", class(it), uri(it, b))).collect(),
                "css",
            ),
            "html" => (
                list.iter().map(|(it, b)| format!("<img src=\"{}\" alt=\"{}\">\n", uri(it, b), xml_esc(&it.stem()))).collect(),
                "html",
            ),
            "md" => (list.iter().map(|(it, b)| format!("![{}]({})\n", it.stem(), uri(it, b))).collect(), "md"),
            "json" => {
                let mut m = serde_json::Map::new();
                for (it, b) in list {
                    m.insert(it.name.clone(), json!(uri(it, b)));
                }
                (serde_json::to_string_pretty(&Value::Object(m)).unwrap_or_default(), "json")
            }
            "raw" => (list.iter().map(|(_, b)| format!("{b}\n")).collect(), "b64"),
            _ => (list.iter().map(|(it, b)| format!("{}\n", uri(it, b))).collect(), "txt"),
        }
    };
    let mut out = vec![];
    if combine {
        let (body, ext) = render(&entries);
        let path = ctx.reserve(&ctx.out_dir("")?, "images-base64", ext)?;
        std::fs::write(&path, body)?;
        out.push(FileItem::from_path(&path, ""));
    } else {
        for e in &entries {
            let (body, ext) = render(std::slice::from_ref(e));
            let path = ctx.out_path(e.0, &e.0.stem(), ext)?;
            std::fs::write(&path, body)?;
            out.push(e.0.derive(&path));
        }
    }
    Ok(out)
}

fn dhash(im: &DynamicImage) -> u64 {
    let g = im.resize_exact(9, 8, FilterType::Triangle).to_luma8();
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            h <<= 1;
            if g.get_pixel(x, y)[0] > g.get_pixel(x + 1, y)[0] {
                h |= 1;
            }
        }
    }
    h
}

pub fn similar(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let threshold = p.i64("threshold", 6).clamp(0, 32) as u32;
    let keep = p.str("keep", "largest");
    let (mut imgs, pass) = split_kinds(files, &["image"]);
    if imgs.is_empty() {
        bail!("No images in the input");
    }
    match keep.as_str() {
        "largest" => imgs.sort_by(|a, b| b.size.cmp(&a.size)),
        "smallest" => imgs.sort_by(|a, b| a.size.cmp(&b.size)),
        _ => {}
    }
    let total = imgs.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let hashes: Vec<Option<u64>> = imgs
        .par_iter()
        .map(|it| {
            let h = open_any(ctx, &it.path_buf()).ok().map(|i| dhash(&i));
            let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            ctx.progress(d, total, &it.name);
            h
        })
        .collect();
    ctx.check()?;
    let mut kept: Vec<(u64, usize)> = vec![];
    let (mut uniq, mut dup) = (vec![], vec![]);
    let mut report = String::from("duplicate,original,distance\n");
    for (i, (it, h)) in imgs.iter().zip(hashes).enumerate() {
        let Some(h) = h else {
            uniq.push(it.clone());
            continue;
        };
        match kept.iter().map(|(k, j)| ((k ^ h).count_ones(), *j)).filter(|(d, _)| *d <= threshold).min() {
            Some((d, j)) => {
                report.push_str(&format!("\"{}\",\"{}\",{}\n", it.path.replace('"', "\"\""), imgs[j].path.replace('"', "\"\""), d));
                dup.push(it.clone());
            }
            None => {
                kept.push((h, i));
                uniq.push(it.clone());
            }
        }
    }
    if !dup.is_empty() {
        ctx.warn(format!("{} similar image(s) found", dup.len()));
    }
    let rp = ctx.reserve(&ctx.out_dir("")?, "similar-images", "csv")?;
    std::fs::write(&rp, report)?;
    uniq.extend(pass);
    Ok(vec![uniq, dup, vec![FileItem::from_path(&rp, "")]])
}

pub fn remove_bg(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "auto");
    let color = parse_color(&p.str("color", "#ffffff"));
    let tol = p.f64("tolerance", 12.0).clamp(0.0, 100.0) as f32 / 100.0 * 441.7;
    let soft = p.f64("feather", 4.0).clamp(0.0, 100.0) as f32 * 4.0;
    let connected = p.bool("connected", true);
    let trim = p.bool("trim", false);
    let fmt = p.str("format", "png");
    batch(ctx, files, &["image"], true, |it| {
        let mut im = open_any(ctx, &it.path_buf())?.to_rgba8();
        let (w, h) = im.dimensions();
        let key = if mode == "color" {
            color
        } else {
            let mut cs: Vec<[u8; 3]> = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1), (w / 2, 0), (w / 2, h - 1), (0, h / 2), (w - 1, h / 2)]
                .iter()
                .map(|(x, y)| {
                    let p = im.get_pixel(*x, *y);
                    [p[0], p[1], p[2]]
                })
                .collect();
            cs.sort_by_key(|c| c[0] as u32 + c[1] as u32 + c[2] as u32);
            cs[cs.len() / 2]
        };
        let dist = |p: &Rgba<u8>| -> f32 {
            let d: f32 = (0..3).map(|c| (p[c] as f32 - key[c] as f32).powi(2)).sum();
            d.sqrt()
        };
        let alpha_for = |d: f32| -> f32 { if d <= tol { 0.0 } else if soft > 0.0 && d < tol + soft { (d - tol) / soft } else { 1.0 } };
        if connected {
            let mut seen = vec![false; (w * h) as usize];
            let mut stack: Vec<(u32, u32)> = vec![];
            for x in 0..w {
                stack.push((x, 0));
                stack.push((x, h - 1));
            }
            for y in 0..h {
                stack.push((0, y));
                stack.push((w - 1, y));
            }
            while let Some((x, y)) = stack.pop() {
                let i = (y * w + x) as usize;
                if seen[i] {
                    continue;
                }
                seen[i] = true;
                let d = dist(im.get_pixel(x, y));
                let a = alpha_for(d);
                if a >= 1.0 {
                    continue;
                }
                let p = im.get_pixel_mut(x, y);
                p[3] = (p[3] as f32 * a) as u8;
                if d <= tol {
                    if x > 0 {
                        stack.push((x - 1, y));
                    }
                    if x + 1 < w {
                        stack.push((x + 1, y));
                    }
                    if y > 0 {
                        stack.push((x, y - 1));
                    }
                    if y + 1 < h {
                        stack.push((x, y + 1));
                    }
                }
            }
        } else {
            for p in im.pixels_mut() {
                let a = alpha_for(dist(p));
                p[3] = (p[3] as f32 * a) as u8;
            }
        }
        if trim {
            im = trim_alpha(&im, 0);
        }
        let ext = if fmt == "webp" { "webp" } else { "png" };
        save_named(ctx, it, &it.stem(), &DynamicImage::ImageRgba8(im), ext, 95).map(|f| vec![f])
    })
}

pub fn animate(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let (imgs, pass) = split_kinds(files, &["image"]);
    if imgs.is_empty() {
        bail!("No images in the input");
    }
    let delay = p.i64("delay", 200).clamp(10, 60_000) as u32;
    let width = p.i64("width", 480).max(0) as u32;
    let looped = p.bool("loop", true);
    let boomerang = p.bool("boomerang", false);
    let name = p.str("name", "animation");
    let first = open_any(ctx, &imgs[0].path_buf())?;
    let (fw, fh) = first.dimensions();
    let (tw, th) = if width > 0 && width < fw { (width, ((fh as f64 * width as f64 / fw as f64).round() as u32).max(1)) } else { (fw, fh) };
    let total = imgs.len();
    let mut frames: Vec<RgbaImage> = vec![];
    for (i, it) in imgs.iter().enumerate() {
        ctx.check()?;
        let im = if i == 0 { first.clone() } else { open_any(ctx, &it.path_buf())? };
        let r = im.resize(tw, th, FilterType::Lanczos3).to_rgba8();
        let mut c = RgbaImage::from_pixel(tw, th, Rgba([0, 0, 0, 0]));
        imageops::overlay(&mut c, &r, ((tw - r.width()) / 2) as i64, ((th - r.height()) / 2) as i64);
        frames.push(c);
        ctx.report(i as f64 / total as f64 * 0.5, &it.name);
    }
    if boomerang && frames.len() > 2 {
        let back: Vec<RgbaImage> = frames[1..frames.len() - 1].iter().rev().cloned().collect();
        frames.extend(back);
    }
    let out = ctx.reserve(&ctx.out_dir(&imgs[0].rel)?, &name, "gif")?;
    {
        let f = std::io::BufWriter::new(std::fs::File::create(&out)?);
        let mut enc = GifEncoder::new_with_speed(f, 10);
        enc.set_repeat(if looped { Repeat::Infinite } else { Repeat::Finite(0) })?;
        let list = frames.into_iter().map(|b| image::Frame::from_parts(b, 0, 0, image::Delay::from_numer_denom_ms(delay, 1)));
        enc.encode_frames(list)?;
    }
    ctx.progress(1, 1, "");
    let mut v = vec![FileItem::from_path(&out, &imgs[0].rel)];
    v.extend(pass);
    Ok(v)
}

pub fn frames(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    use image::codecs::{gif::GifDecoder, png::PngDecoder, webp::WebPDecoder};
    let every = p.i64("every", 1).max(1) as usize;
    let max = p.i64("max_frames", 0).max(0) as usize;
    let fmt = p.str("format", "png");
    batch(ctx, files, &["image"], false, |it| {
        let open = || -> Result<BufReader<std::fs::File>> { Ok(BufReader::new(std::fs::File::open(&it.path)?)) };
        let list: Vec<image::Frame> = match it.ext.as_str() {
            "gif" => GifDecoder::new(open()?)?.into_frames().collect_frames()?,
            "webp" => WebPDecoder::new(open()?)?.into_frames().collect_frames()?,
            "png" | "apng" => PngDecoder::new(open()?)?.apng()?.into_frames().collect_frames()?,
            _ => bail!("only animated GIF, WebP and PNG are supported"),
        };
        if list.len() < 2 {
            bail!("the image is not animated");
        }
        let width = list.len().to_string().len().max(3);
        let mut out = vec![];
        for (i, f) in list.into_iter().enumerate().filter(|(i, _)| i % every == 0) {
            if max > 0 && out.len() >= max {
                break;
            }
            ctx.check()?;
            let im = DynamicImage::ImageRgba8(f.into_buffer());
            let ext = if fmt == "jpg" { "jpg" } else if fmt == "webp" { "webp" } else { "png" };
            out.push(save_named(ctx, it, &format!("{}_{:0w$}", it.stem(), i + 1, w = width), &im, ext, 92)?);
        }
        Ok(out)
    })
}
