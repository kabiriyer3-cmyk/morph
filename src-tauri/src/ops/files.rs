use super::Outs;
use crate::engine::{Ctx, FileItem};
use crate::ops::media;
use crate::util::{file_name, glob_match, glob_set, home_dir, join_rel, link_or_copy, open_in_os, sanitize, unique_path, Params};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Local};
use rayon::prelude::*;
use regex::Regex;
use serde_json::{json, Value};
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

fn is_hidden(e: &walkdir::DirEntry) -> bool {
    if e.file_name().to_string_lossy().starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(m) = e.metadata() {
            return m.file_attributes() & 0x2 != 0;
        }
    }
    false
}

/// Expands files and folders into FileItems. Files inside a dropped folder get
/// `rel = <folder name>/<sub dirs>` so Save can recreate the structure.
pub fn scan(paths: &[String], recursive: bool, include: &str, exclude: &str, skip_hidden: bool) -> Vec<FileItem> {
    let inc = glob_set(include);
    let exc = glob_set(exclude);
    let ok = |name: &str| (inc.is_empty() || glob_match(&inc, name)) && !glob_match(&exc, name);
    let mut out = vec![];
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for raw in paths {
        let p = PathBuf::from(raw.trim());
        if p.is_file() {
            if ok(&file_name(&p)) && seen.insert(p.clone()) {
                out.push(FileItem::from_path(&p, ""));
            }
        } else if p.is_dir() {
            let root_name = file_name(&p);
            let walker = WalkDir::new(&p)
                .follow_links(true)
                .max_depth(if recursive { usize::MAX } else { 1 })
                .sort_by_file_name();
            for e in walker
                .into_iter()
                .filter_entry(|e| !(skip_hidden && e.depth() > 0 && is_hidden(e)))
                .filter_map(|e| e.ok())
            {
                if !e.file_type().is_file() {
                    continue;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                if !ok(&name) {
                    continue;
                }
                let rel_dir = e
                    .path()
                    .parent()
                    .and_then(|d| d.strip_prefix(&p).ok())
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                let rel = join_rel(&root_name, &rel_dir);
                if seen.insert(e.path().to_path_buf()) {
                    out.push(FileItem::from_path(e.path(), &rel));
                }
            }
        }
    }
    out
}

pub fn load(ctx: &Ctx, p: &Params) -> Result<Vec<FileItem>> {
    let paths = p.list("paths");
    if paths.is_empty() {
        bail!("Pick files or a folder in the Load node (or drop them onto the canvas).");
    }
    let files = scan(
        &paths,
        p.bool("recursive", true),
        &p.str("include", ""),
        &p.str("exclude", ""),
        p.bool("skip_hidden", true),
    );
    if files.is_empty() {
        bail!("No files matched in the selected paths.");
    }
    ctx.progress(1, 1, "");
    Ok(files)
}

/// `{name}` `{ext}` `{n}` `{n:3}` `{date}` `{time}` `{parent}` `{kind}` `{orig}`
pub fn render_pattern(pat: &str, it: &FileItem, index: usize, start: usize, pad: usize, now: &DateTime<Local>) -> String {
    let re = Regex::new(r"\{(\w+)(?::(\d+))?\}").expect("pattern regex");
    re.replace_all(pat, |c: &regex::Captures| {
        let w: usize = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(pad);
        match &c[1] {
            "name" => it.stem(),
            "ext" => it.ext.clone(),
            "n" => format!("{:0w$}", index + start, w = w),
            "date" => now.format("%Y-%m-%d").to_string(),
            "time" => now.format("%H-%M-%S").to_string(),
            "parent" => Path::new(&it.origin).parent().map(file_name).unwrap_or_default(),
            "kind" => it.kind.clone(),
            "orig" => crate::util::file_stem(Path::new(&it.origin)),
            _ => c[0].to_string(),
        }
    })
    .into_owned()
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

pub fn save(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let dest_mode = p.str("destination", "folder");
    let folder = p.str("folder", "");
    let pattern = p.str("pattern", "{name}");
    let pattern = if pattern.trim().is_empty() { "{name}".to_string() } else { pattern };
    let keep = p.bool("keep_structure", true);
    let conflict = p.str("on_conflict", "rename");
    let base = if folder.trim().is_empty() {
        home_dir().join("Morph Output")
    } else {
        PathBuf::from(folder.trim())
    };
    let now = Local::now();
    let total = files.len();
    let mut out = vec![];
    let mut first_dir: Option<PathBuf> = None;
    for (i, it) in files.iter().enumerate() {
        ctx.check()?;
        let dir = if dest_mode.starts_with("next") {
            Path::new(&it.origin).parent().map(Path::to_path_buf).unwrap_or_else(|| base.clone())
        } else if keep && !it.rel.is_empty() {
            base.join(crate::util::clean_rel(&it.rel))
        } else {
            base.clone()
        };
        std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let name = render_pattern(&pattern, it, i, 1, 0, &now);
        let file = if pattern.contains("{ext}") || it.ext.is_empty() {
            sanitize(&name)
        } else {
            format!("{}.{}", sanitize(&name), it.ext)
        };
        let src = it.path_buf();
        let mut target = dir.join(file);
        if same_file(&src, &target) {
            out.push(it.clone());
            continue;
        }
        if target.exists() {
            match conflict.as_str() {
                "skip" => {
                    ctx.warn(format!("{}: already exists, skipped", target.display()));
                    continue;
                }
                "overwrite" => {}
                _ => target = unique_path(&target),
            }
        }
        std::fs::copy(&src, &target).with_context(|| format!("cannot write {}", target.display()))?;
        if first_dir.is_none() {
            first_dir = Some(dir.clone());
        }
        out.push(it.derive(&target));
        ctx.progress(i + 1, total, &it.name);
    }
    if p.bool("open_folder", false) {
        if let Some(d) = first_dir {
            let _ = open_in_os(&d);
        }
    }
    Ok(out)
}

pub fn filter(p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let kind = p.str("kind", "any");
    let pattern = p.str("pattern", "");
    let use_re = p.bool("regex", false);
    let min = p.f64("min_kb", 0.0) * 1024.0;
    let max = p.f64("max_kb", 0.0) * 1024.0;
    let re = if use_re && !pattern.trim().is_empty() {
        Some(Regex::new(&pattern).context("invalid regular expression")?)
    } else {
        None
    };
    let globs = if use_re { vec![] } else { glob_set(&pattern) };
    let (a, b): (Vec<FileItem>, Vec<FileItem>) = files.into_iter().partition(|f| {
        let kind_ok = kind == "any" || f.kind == kind;
        let name_ok = pattern.trim().is_empty()
            || match &re {
                Some(r) => r.is_match(&f.name),
                None => glob_match(&globs, &f.name),
            };
        let size_ok = (min <= 0.0 || f.size as f64 >= min) && (max <= 0.0 || f.size as f64 <= max);
        kind_ok && name_ok && size_ok
    });
    Ok(vec![a, b])
}

pub fn split(files: Vec<FileItem>) -> Outs {
    let mut o: Outs = vec![vec![]; 5];
    for f in files {
        let i = match f.kind.as_str() {
            "image" => 0,
            "video" => 1,
            "audio" => 2,
            "mesh" => 3,
            _ => 4,
        };
        o[i].push(f);
    }
    o
}

pub fn merge(files: Vec<FileItem>) -> Vec<FileItem> {
    let mut seen = HashSet::new();
    files.into_iter().filter(|f| seen.insert(f.path.clone())).collect()
}

fn apply_case(s: &str, case: &str) -> String {
    match case {
        "lower" => s.to_lowercase(),
        "upper" => s.to_uppercase(),
        "title" => s
            .split(' ')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        "snake" | "kebab" => {
            let sep = if case == "snake" { '_' } else { '-' };
            let mut out = String::new();
            let mut prev_sep = true;
            for ch in s.chars() {
                if ch.is_alphanumeric() {
                    out.extend(ch.to_lowercase());
                    prev_sep = false;
                } else if !prev_sep {
                    out.push(sep);
                    prev_sep = true;
                }
            }
            out.trim_end_matches(sep).to_string()
        }
        _ => s.to_string(),
    }
}

pub fn rename(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let pattern = p.str("pattern", "{name}");
    let find = p.str("find", "");
    let replace = p.str("replace", "");
    let use_re = p.bool("regex", false);
    let case = p.str("case", "keep");
    let start = p.i64("start", 1).max(0) as usize;
    let digits = p.i64("digits", 3).clamp(0, 12) as usize;
    let re = if find.is_empty() {
        None
    } else if use_re {
        Some(Regex::new(&find).context("invalid regular expression")?)
    } else {
        Some(Regex::new(&regex::escape(&find))?)
    };
    let now = Local::now();
    let total = files.len();
    let mut out = Vec::with_capacity(total);
    for (i, it) in files.iter().enumerate() {
        ctx.check()?;
        let mut name = render_pattern(&pattern, it, i, start, digits, &now);
        if let Some(r) = &re {
            name = if use_re {
                r.replace_all(&name, replace.as_str()).into_owned()
            } else {
                r.replace_all(&name, regex::NoExpand(&replace)).into_owned()
            };
        }
        let name = apply_case(&name, &case);
        let target = ctx.out_path(it, &name, &it.ext)?;
        link_or_copy(&it.path_buf(), &target)?;
        out.push(it.derive(&target));
        ctx.progress(i + 1, total, &it.name);
    }
    Ok(out)
}

pub fn hash_file(path: &Path, alg: &str) -> Result<String> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    macro_rules! digest {
        ($h:expr) => {{
            let mut h = $h;
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                Digest::update(&mut h, &buf[..n]);
            }
            crate::util::hex(&h.finalize())
        }};
    }
    Ok(match alg {
        "md5" => digest!(md5::Md5::new()),
        "sha1" => digest!(sha1::Sha1::new()),
        "sha512" => digest!(sha2::Sha512::new()),
        "blake3" => {
            let mut h = blake3::Hasher::new();
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            h.finalize().to_hex().to_string()
        }
        "crc32" => {
            let mut h = crc32fast::Hasher::new();
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            format!("{:08x}", h.finalize())
        }
        _ => digest!(sha2::Sha256::new()),
    })
}

fn csv_string(w: csv::Writer<Vec<u8>>) -> Result<String> {
    let bytes = w.into_inner().map_err(|e| anyhow!("csv: {e}"))?;
    Ok(String::from_utf8(bytes)?)
}

pub fn hash(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let alg = p.str("algorithm", "sha256");
    let format = p.str("format", "sums");
    let total = files.len();
    let done = AtomicUsize::new(0);
    let hashes: Vec<Result<String>> = files
        .par_iter()
        .map(|it| {
            if ctx.cancelled() {
                return Ok(String::new());
            }
            let r = hash_file(&it.path_buf(), &alg);
            let d = done.fetch_add(1, Ordering::SeqCst) + 1;
            ctx.progress(d, total, &it.name);
            r
        })
        .collect();
    ctx.check()?;
    let mut rows: Vec<(&FileItem, String)> = vec![];
    for (it, h) in files.iter().zip(hashes) {
        match h {
            Ok(h) => rows.push((it, h)),
            Err(e) => ctx.warn(format!("{}: {:#}", it.name, e)),
        }
    }
    let shown = |it: &FileItem| join_rel(&it.rel, &it.name);
    let (body, ext) = match format.as_str() {
        "csv" => {
            let mut w = csv::Writer::from_writer(vec![]);
            w.write_record(["file", "size", "algorithm", "hash"])?;
            for (it, h) in &rows {
                w.write_record([shown(it), it.size.to_string(), alg.clone(), h.clone()])?;
            }
            (csv_string(w)?, "csv".to_string())
        }
        "json" => {
            let list: Vec<Value> = rows
                .iter()
                .map(|(it, h)| json!({"file": shown(it), "path": it.path, "size": it.size, "algorithm": alg, "hash": h}))
                .collect();
            (serde_json::to_string_pretty(&list)?, "json".to_string())
        }
        _ => (
            rows.iter().map(|(it, h)| format!("{}  {}\n", h, shown(it))).collect::<String>(),
            alg.clone(),
        ),
    };
    let out = ctx.reserve(&ctx.out_dir("")?, "checksums", &ext)?;
    std::fs::write(&out, body)?;
    Ok(vec![files, vec![FileItem::from_path(&out, "")]])
}

pub fn dedupe(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let by = p.str("by", "content");
    let mut size_count: HashMap<u64, usize> = HashMap::new();
    for f in &files {
        *size_count.entry(f.size).or_default() += 1;
    }
    let total = files.len();
    let done = AtomicUsize::new(0);
    let keys: Vec<String> = files
        .par_iter()
        .map(|f| {
            let k = match by.as_str() {
                "name" => f.name.to_lowercase(),
                "name+size" => format!("{}|{}", f.name.to_lowercase(), f.size),
                _ => {
                    if size_count.get(&f.size).copied().unwrap_or(0) < 2 {
                        format!("u|{}", f.path)
                    } else {
                        let h = hash_file(&f.path_buf(), "blake3").unwrap_or_else(|_| f.path.clone());
                        format!("{}|{}", f.size, h)
                    }
                }
            };
            let d = done.fetch_add(1, Ordering::SeqCst) + 1;
            ctx.progress(d, total, &f.name);
            k
        })
        .collect();
    ctx.check()?;
    let mut seen = HashSet::new();
    let (mut uniq, mut dup) = (vec![], vec![]);
    for (f, k) in files.into_iter().zip(keys) {
        if seen.insert(k) {
            uniq.push(f);
        } else {
            dup.push(f);
        }
    }
    if !dup.is_empty() {
        ctx.warn(format!("{} duplicate(s) found", dup.len()));
    }
    Ok(vec![uniq, dup])
}

pub fn info(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let format = p.str("format", "csv");
    let total = files.len();
    let done = AtomicUsize::new(0);
    let rows: Vec<Value> = files
        .par_iter()
        .map(|it| {
            let path = it.path_buf();
            let modified = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string());
            let mut v = json!({
                "name": it.name, "folder": it.rel, "path": it.path, "kind": it.kind, "ext": it.ext,
                "size": it.size, "modified": modified, "width": Value::Null, "height": Value::Null, "duration": Value::Null
            });
            if it.kind == "image" && it.ext != "svg" {
                if let Ok((w, h)) = image::image_dimensions(&path) {
                    v["width"] = json!(w);
                    v["height"] = json!(h);
                }
            }
            if (it.kind == "video" || it.kind == "audio") && ctx.tools.ffprobe.is_some() {
                let mi = media::media_info(ctx.tools, &path);
                v["duration"] = json!((mi.duration * 1000.0).round() / 1000.0);
                if mi.width > 0 {
                    v["width"] = json!(mi.width);
                    v["height"] = json!(mi.height);
                }
            }
            let d = done.fetch_add(1, Ordering::SeqCst) + 1;
            ctx.progress(d, total, &it.name);
            v
        })
        .collect();
    ctx.check()?;
    let cols = ["name", "folder", "path", "kind", "ext", "size", "modified", "width", "height", "duration"];
    let (body, ext) = if format == "json" {
        (serde_json::to_string_pretty(&rows)?, "json")
    } else {
        let mut w = csv::Writer::from_writer(vec![]);
        w.write_record(cols)?;
        for r in &rows {
            let rec: Vec<String> = cols
                .iter()
                .map(|c| match &r[*c] {
                    Value::Null => String::new(),
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect();
            w.write_record(&rec)?;
        }
        (csv_string(w)?, "csv")
    };
    let out = ctx.reserve(&ctx.out_dir("")?, "file-info", ext)?;
    std::fs::write(&out, body)?;
    Ok(vec![files, vec![FileItem::from_path(&out, "")]])
}
