// Renders assets/icon.svg into every icon Tauri needs (PNG set + multi-size ICO),
// so no binary files have to live in the repository.
use std::path::{Path, PathBuf};

fn render(tree: &resvg::usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let s = tree.size();
    let scale = size as f32 / s.width().max(s.height());
    resvg::render(tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let mut data = pixmap.take();
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in px.iter_mut().take(3) {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("png header");
        w.write_image_data(&data).expect("png data");
    }
    out
}

fn ico(entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
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

fn write_if_changed(path: &Path, data: &[u8]) {
    if std::fs::read(path).map(|old| old == data).unwrap_or(false) {
        return;
    }
    std::fs::write(path, data).expect("write icon");
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let svg_path = manifest.join("..").join("assets").join("icon.svg");
    println!("cargo:rerun-if-changed={}", svg_path.display());
    let icons = manifest.join("icons");
    std::fs::create_dir_all(&icons).expect("icons dir");
    let svg = std::fs::read(&svg_path).expect("assets/icon.svg is missing");
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).expect("icon.svg is invalid");

    for (name, size) in [
        ("32x32.png", 32u32),
        ("64x64.png", 64),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
        ("icon.png", 512),
    ] {
        write_if_changed(&icons.join(name), &render(&tree, size));
    }
    let entries: Vec<(u32, Vec<u8>)> = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&s| (s, render(&tree, s)))
        .collect();
    write_if_changed(&icons.join("icon.ico"), &ico(&entries));

    tauri_build::build();
}
