//! Archive extras on top of arch.rs: password protection and RAR/ISO/CAB through 7-Zip, and archive tests.
use super::{batch, run_tool, Outs};
use crate::engine::{Ctx, FileItem};
use crate::ops::arch;
use crate::util::{clean_rel, command, join_rel, link_or_copy, sanitize, Params};
use anyhow::{bail, Result};
use serde_json::json;
use std::path::Path;
use walkdir::WalkDir;

const NEEDS_7Z: &[&str] = &["rar", "iso", "cab"];

fn seven_extract(ctx: &Ctx, it: &FileItem, password: &str, sub: bool) -> Result<Vec<FileItem>> {
    let bin = ctx.tools.sevenzip()?.clone();
    let dest = ctx.reserve(&ctx.work, &format!("_x_{}", sanitize(&it.name)), "")?;
    std::fs::create_dir_all(&dest)?;
    let mut cmd = command(&bin);
    cmd.arg("x").arg("-y").arg(format!("-o{}", dest.display()));
    cmd.arg(format!("-p{password}"));
    cmd.arg(&it.path);
    run_tool(ctx, &mut cmd, "7-Zip")?;
    let stem = sanitize(&arch::archive_stem(&it.name));
    let base_rel = if sub { join_rel(&clean_rel(&it.rel), &stem) } else { clean_rel(&it.rel) };
    let mut out = vec![];
    for e in WalkDir::new(&dest).sort_by_file_name().into_iter().filter_map(|e| e.ok()) {
        if !e.file_type().is_file() {
            continue;
        }
        let inner = e.path().parent().and_then(|d| d.strip_prefix(&dest).ok()).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        out.push(FileItem::from_path(e.path(), &join_rel(&base_rel, &inner)));
    }
    if out.is_empty() {
        bail!("the archive is empty (or the password is wrong)");
    }
    Ok(out)
}

pub fn extract(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let password = p.str("password", "");
    let sub = p.bool("subfolder", true);
    let use_7z = |it: &FileItem| it.kind == "archive" && (NEEDS_7Z.contains(&it.ext.as_str()) || !password.is_empty());
    if !files.iter().any(|f| use_7z(f)) {
        return arch::extract(ctx, p, files);
    }
    let (ext, native): (Vec<FileItem>, Vec<FileItem>) = files.into_iter().partition(|f| use_7z(f));
    let mut out = if native.iter().any(|f| f.kind == "archive") { arch::extract(ctx, p, native)? } else { native };
    out.extend(batch(ctx, ext, &["archive"], false, |it| seven_extract(ctx, it, &password, sub))?);
    Ok(out)
}

pub fn create(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let password = p.str("password", "");
    if password.is_empty() {
        return arch::create(ctx, p, files);
    }
    if files.is_empty() {
        bail!("No files to archive");
    }
    let bin = ctx.tools.sevenzip()?.clone();
    let fmt = p.str("format", "zip");
    let (ty, ext) = match fmt.as_str() {
        "7z" => ("7z", "7z"),
        "zip" => ("zip", "zip"),
        other => bail!("Passwords work with ZIP and 7z archives, not .{other}"),
    };
    let level = p.i64("level", 6).clamp(0, 9);
    let per_file = p.str("mode", "single") == "per-file";
    let name = p.str("name", "archive");
    let groups: Vec<(String, Vec<FileItem>)> = if per_file {
        files.iter().map(|f| (f.stem(), vec![f.clone()])).collect()
    } else {
        vec![(if name.trim().is_empty() { "archive".into() } else { name.trim().to_string() }, files.clone())]
    };
    let total = groups.len();
    let mut out = vec![];
    for (i, (gname, items)) in groups.iter().enumerate() {
        ctx.check()?;
        let stage = ctx.reserve(&ctx.work, &format!("_stage_{}", sanitize(gname)), "")?;
        for it in items {
            let rel = if per_file { String::new() } else { clean_rel(&it.rel) };
            let dst = if rel.is_empty() { stage.join(&it.name) } else { stage.join(&rel).join(&it.name) };
            link_or_copy(&it.path_buf(), &crate::util::unique_path(&dst))?;
        }
        let target = ctx.reserve(&ctx.out_dir(&items[0].rel)?, gname, ext)?;
        let mut cmd = command(&bin);
        cmd.current_dir(&stage)
            .arg("a")
            .arg(format!("-t{ty}"))
            .arg(format!("-mx={level}"))
            .arg(format!("-p{password}"))
            .arg("-y");
        if ty == "zip" {
            cmd.arg("-mem=AES256");
        } else {
            cmd.arg("-mhe=on");
        }
        cmd.arg(&target).arg(".");
        run_tool(ctx, &mut cmd, "7-Zip")?;
        let _ = std::fs::remove_dir_all(&stage);
        out.push(FileItem::from_path(&target, &items[0].rel));
        ctx.progress(i + 1, total, gname);
    }
    Ok(out)
}

pub fn test(ctx: &Ctx, _p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let list: Vec<&FileItem> = files.iter().filter(|f| f.kind == "archive").collect();
    if list.is_empty() {
        bail!("No archives in the input");
    }
    let (mut ok, mut bad) = (vec![], vec![]);
    let mut rows = vec![];
    for (i, it) in list.iter().enumerate() {
        ctx.check()?;
        let res: Result<usize> = if let Some(bin) = ctx.tools.sevenzip.clone() {
            let mut cmd = command(&bin);
            cmd.arg("t").arg("-p").arg(&it.path);
            run_tool(ctx, &mut cmd, "7-Zip").map(|_| 0)
        } else if NEEDS_7Z.contains(&it.ext.as_str()) {
            Err(anyhow::anyhow!(crate::tools::NO_7Z))
        } else {
            arch::list(Path::new(&it.path)).map(|v| v["entries"].as_array().map(|a| a.len()).unwrap_or(0))
        };
        match res {
            Ok(n) => {
                rows.push(json!({"file": it.name, "ok": true, "entries": n}));
                ok.push((*it).clone());
            }
            Err(e) => {
                rows.push(json!({"file": it.name, "ok": false, "error": format!("{:#}", e)}));
                bad.push((*it).clone());
            }
        }
        ctx.progress(i + 1, list.len(), &it.name);
    }
    if !bad.is_empty() {
        ctx.warn(format!("{} archive(s) are damaged or unreadable", bad.len()));
    }
    let rp = ctx.reserve(&ctx.out_dir("")?, "archive-test", "json")?;
    std::fs::write(&rp, serde_json::to_string_pretty(&rows)?)?;
    Ok(vec![ok, bad, vec![FileItem::from_path(&rp, "")]])
}
