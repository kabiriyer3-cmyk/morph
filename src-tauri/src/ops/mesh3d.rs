//! 3D nodes built on the in-house mesh module.
use super::batch;
use crate::engine::{Ctx, FileItem};
use crate::mesh;
use crate::util::Params;
use anyhow::Result;

const KINDS: &[&str] = &["mesh"];
const WRITABLE: &[&str] = &["glb", "obj", "stl", "ply", "off"];

fn out_ext(it: &FileItem) -> String {
    if WRITABLE.contains(&it.ext.as_str()) {
        it.ext.clone()
    } else {
        "glb".into()
    }
}

pub fn convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "glb");
    let binary = p.bool("binary", true);
    batch(ctx, files, KINDS, true, |it| {
        let m = mesh::load(&it.path_buf())?;
        let out = ctx.out_path(it, &it.stem(), &fmt)?;
        mesh::save(&m, &out, binary)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn transform(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let scale = p.f64("scale", 1.0) as f32;
    let units = p.str("units", "none");
    let rot = [p.f64("rotate_x", 0.0) as f32, p.f64("rotate_y", 0.0) as f32, p.f64("rotate_z", 0.0) as f32];
    let tr = [p.f64("translate_x", 0.0) as f32, p.f64("translate_y", 0.0) as f32, p.f64("translate_z", 0.0) as f32];
    let center = p.str("center", "none");
    let fit = p.f64("fit_size", 0.0) as f32;
    let up = p.str("up_axis", "keep");
    let flip = p.bool("flip_normals", false);
    batch(ctx, files, KINDS, true, |it| {
        let mut m = mesh::load(&it.path_buf())?;
        let uf: f32 = match units.as_str() {
            "mm to m" => 0.001,
            "m to mm" => 1000.0,
            "cm to m" => 0.01,
            "m to cm" => 100.0,
            "inch to mm" => 25.4,
            "mm to inch" => 1.0 / 25.4,
            _ => 1.0,
        };
        if uf != 1.0 {
            m.scale(uf);
        }
        match up.as_str() {
            "Z-up to Y-up" => m.apply_linear(|p| [p[0], p[2], -p[1]]),
            "Y-up to Z-up" => m.apply_linear(|p| [p[0], -p[2], p[1]]),
            _ => {}
        }
        for (axis, deg) in rot.iter().enumerate() {
            if *deg != 0.0 {
                m.rotate(axis, *deg);
            }
        }
        if scale != 1.0 && scale > 0.0 {
            m.scale(scale);
        }
        if fit > 0.0 {
            let (mn, mx) = m.bbox();
            let ext = (mx[0] - mn[0]).max(mx[1] - mn[1]).max(mx[2] - mn[2]);
            if ext > 0.0 {
                m.scale(fit / ext);
            }
        }
        if center != "none" {
            let (mn, mx) = m.bbox();
            let c = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
            let ty = if center == "center + ground" { -mn[1] } else { -c[1] };
            m.translate([-c[0], ty, -c[2]]);
        }
        if tr.iter().any(|v| *v != 0.0) {
            m.translate(tr);
        }
        if flip {
            m.flip();
        }
        let out = ctx.out_path(it, &it.stem(), &out_ext(it))?;
        mesh::save(&m, &out, true)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn optimize(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let weld = p.bool("weld", true);
    let tol = p.f64("weld_tolerance", 0.0001).max(0.0) as f32;
    let keep = p.f64("keep_percent", 100.0).clamp(0.1, 100.0);
    let normals = p.str("normals", "keep");
    let degenerate = p.bool("remove_degenerate", true);
    batch(ctx, files, KINDS, true, |it| {
        let mut m = mesh::load(&it.path_buf())?;
        let before = m.tri_count();
        if weld {
            m.weld(tol);
        }
        if degenerate {
            m.remove_degenerate();
        }
        if keep < 100.0 {
            let target = ((m.tri_count() as f64) * keep / 100.0).round().max(4.0) as usize;
            m.simplify(target);
        }
        match normals.as_str() {
            "smooth" => m.compute_normals(),
            "flat" => m.flat_normals(),
            _ => {}
        }
        if m.tri_count() < before {
            ctx.warn(format!("{}: {} -> {} triangles", it.name, before, m.tri_count()));
        }
        let out = ctx.out_path(it, &it.stem(), &out_ext(it))?;
        mesh::save(&m, &out, true)?;
        Ok(vec![it.derive(&out)])
    })
}
