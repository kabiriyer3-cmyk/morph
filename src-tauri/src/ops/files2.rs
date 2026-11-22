//! More file nodes: organize into folders, sort/limit, clean names (transliteration), split/join parts,
//! checksum verification and HTML/CSV/JSON reports.
use super::{batch, Outs};
use crate::engine::{Ctx, FileItem};
use crate::ops::files::hash_file;
use crate::util::{clean_rel, join_rel, link_or_copy, sanitize, Params};
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::Path;

fn modified(path: &Path) -> Option<DateTime<Local>> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().map(DateTime::<Local>::from)
}

fn taken(path: &Path) -> Option<DateTime<Local>> {
    let s = crate::ops::img2::date_taken(path)?;
    let s = s.replace(':', "-");
    let s: String = s.chars().take(19).collect();
    let dt = NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H-%M-%S").ok()?;
    Local.from_local_datetime(&dt).single()
}

fn kind_folder(kind: &str, ru: bool) -> &'static str {
    match (kind, ru) {
        ("image", false) => "Images",
        ("video", false) => "Videos",
        ("audio", false) => "Audio",
        ("mesh", _) => "3D",
        ("archive", false) => "Archives",
        ("data", false) => "Data",
        ("doc", false) => "Documents",
        ("text", false) => "Text",
        ("image", true) => "\u{418}\u{437}\u{43e}\u{431}\u{440}\u{430}\u{436}\u{435}\u{43d}\u{438}\u{44f}",
        ("video", true) => "\u{412}\u{438}\u{434}\u{435}\u{43e}",
        ("audio", true) => "\u{410}\u{443}\u{434}\u{438}\u{43e}",
        ("archive", true) => "\u{410}\u{440}\u{445}\u{438}\u{432}\u{44b}",
        ("data", true) => "\u{414}\u{430}\u{43d}\u{43d}\u{44b}\u{435}",
        ("doc", true) => "\u{414}\u{43e}\u{43a}\u{443}\u{43c}\u{435}\u{43d}\u{442}\u{44b}",
        ("text", true) => "\u{422}\u{435}\u{43a}\u{441}\u{442}\u{44b}",
        (_, true) => "\u{414}\u{440}\u{443}\u{433}\u{43e}\u{435}",
        _ => "Other",
    }
}

fn size_bucket(n: u64) -> &'static str {
    match n {
        0..=102_399 => "0-100 KB",
        102_400..=1_048_575 => "100 KB-1 MB",
        1_048_576..=10_485_759 => "1-10 MB",
        10_485_760..=104_857_599 => "10-100 MB",
        104_857_600..=1_073_741_823 => "100 MB-1 GB",
        _ => "1 GB+",
    }
}

pub fn organize(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let by = p.str("by", "kind");
    let date_fmt = match p.str("date_format", "yyyy/mm").as_str() {
        "yyyy" => "%Y",
        "yyyy-mm" => "%Y-%m",
        "yyyy/mm/dd" => "%Y/%m/%d",
        "yyyy-mm-dd" => "%Y-%m-%d",
        _ => "%Y/%m",
    };
    let pattern = p.str("pattern", "{kind}/{yyyy}");
    let keep_root = p.bool("keep_root", false);
    let ru = p.str("folder_names", "en") == "ru";
    let total = files.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let out: Vec<FileItem> = files
        .par_iter()
        .map(|it| {
            let path = it.path_buf();
            let date = || {
                if by == "taken" || pattern.contains("{taken") {
                    taken(&path).or_else(|| modified(&path))
                } else {
                    modified(&path)
                }
            };
            let letter = it.stem().chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "#".into());
            let folder = match by.as_str() {
                "ext" => if it.ext.is_empty() { "no extension".to_string() } else { it.ext.to_uppercase() },
                "date" | "taken" => date().map(|d| d.format(date_fmt).to_string()).unwrap_or_else(|| "unknown date".into()),
                "size" => size_bucket(it.size).to_string(),
                "letter" => letter.clone(),
                "flatten" => String::new(),
                "pattern" => {
                    let d = date();
                    let f = |fmt: &str| d.map(|x| x.format(fmt).to_string()).unwrap_or_else(|| "0000".into());
                    pattern
                        .replace("{kind}", kind_folder(&it.kind, ru))
                        .replace("{ext}", &it.ext)
                        .replace("{yyyy}", &f("%Y"))
                        .replace("{mm}", &f("%m"))
                        .replace("{dd}", &f("%d"))
                        .replace("{letter}", &letter)
                        .replace("{size}", size_bucket(it.size))
                        .replace("{parent}", &Path::new(&it.origin).parent().map(crate::util::file_name).unwrap_or_default())
                }
                _ => kind_folder(&it.kind, ru).to_string(),
            };
            let root = if keep_root { it.rel.split('/').next().unwrap_or("").to_string() } else { String::new() };
            let mut f = it.clone();
            f.rel = clean_rel(&join_rel(&root, &folder));
            let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            ctx.progress(d, total, &it.name);
            f
        })
        .collect();
    Ok(out)
}

fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    crate::ops::pdftools::natural_cmp(a, b)
}

pub fn sort(_ctx: &Ctx, p: &Params, mut files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let by = p.str("by", "name");
    let desc = p.str("order", "asc") == "desc";
    let offset = p.i64("offset", 0).max(0) as usize;
    let limit = p.i64("limit", 0).max(0) as usize;
    let every = p.i64("every", 1).max(1) as usize;
    match by.as_str() {
        "size" => files.sort_by(|a, b| a.size.cmp(&b.size)),
        "modified" => files.sort_by_cached_key(|f| modified(&f.path_buf()).map(|d| d.timestamp_millis()).unwrap_or(0)),
        "ext" => files.sort_by(|a, b| a.ext.cmp(&b.ext).then_with(|| natural(&a.name, &b.name))),
        "kind" => files.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| natural(&a.name, &b.name))),
        "path" => files.sort_by(|a, b| natural(&join_rel(&a.rel, &a.name), &join_rel(&b.rel, &b.name))),
        "random" => {
            let mut seed = p.i64("seed", 0) as u64;
            if seed == 0 {
                seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
            }
            let mut s = seed | 1;
            for i in (1..files.len()).rev() {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                files.swap(i, (s % (i as u64 + 1)) as usize);
            }
        }
        "none" => {}
        _ => files.sort_by(|a, b| natural(&a.name, &b.name)),
    }
    if desc && by != "random" {
        files.reverse();
    }
    let out: Vec<FileItem> = files.into_iter().skip(offset).step_by(every).take(if limit == 0 { usize::MAX } else { limit }).collect();
    Ok(out)
}

fn translit(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{430}' => "a", '\u{431}' => "b", '\u{432}' => "v", '\u{433}' => "g", '\u{434}' => "d", '\u{435}' => "e", '\u{451}' => "yo",
        '\u{436}' => "zh", '\u{437}' => "z", '\u{438}' => "i", '\u{439}' => "y", '\u{43a}' => "k", '\u{43b}' => "l", '\u{43c}' => "m",
        '\u{43d}' => "n", '\u{43e}' => "o", '\u{43f}' => "p", '\u{440}' => "r", '\u{441}' => "s", '\u{442}' => "t", '\u{443}' => "u",
        '\u{444}' => "f", '\u{445}' => "kh", '\u{446}' => "ts", '\u{447}' => "ch", '\u{448}' => "sh", '\u{449}' => "shch",
        '\u{44a}' => "", '\u{44b}' => "y", '\u{44c}' => "", '\u{44d}' => "e", '\u{44e}' => "yu", '\u{44f}' => "ya",
        '\u{456}' => "i", '\u{457}' => "yi", '\u{454}' => "ye", '\u{491}' => "g", '\u{45e}' => "u",
        _ => return None,
    })
}

pub fn transliterate(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let lower: Vec<char> = c.to_lowercase().collect();
        match lower.first().and_then(|l| translit(*l)) {
            Some(t) => {
                if c.is_uppercase() {
                    let mut it = t.chars();
                    if let Some(f) = it.next() {
                        out.extend(f.to_uppercase());
                        out.push_str(it.as_str());
                    }
                } else {
                    out.push_str(t);
                }
            }
            None => out.push(c),
        }
    }
    out
}

pub fn clean_names(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let tr = p.bool("transliterate", true);
    let spaces = p.str("spaces", "_");
    let case = p.str("case", "keep");
    let special = p.bool("remove_special", true);
    let max_len = p.i64("max_length", 0).max(0) as usize;
    let lower_ext = p.bool("lowercase_ext", true);
    let total = files.len();
    let mut out = vec![];
    for (i, it) in files.iter().enumerate() {
        ctx.check()?;
        let mut s = it.stem();
        if tr {
            s = transliterate(&s);
        }
        if special {
            s = s.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' { c } else { ' ' }).collect();
        }
        let sep = match spaces.as_str() {
            "-" => "-",
            "remove" => "",
            "keep" => " ",
            _ => "_",
        };
        s = s.split_whitespace().collect::<Vec<_>>().join(sep);
        s = match case.as_str() {
            "lower" => s.to_lowercase(),
            "upper" => s.to_uppercase(),
            _ => s,
        };
        if max_len > 0 && s.chars().count() > max_len {
            s = s.chars().take(max_len).collect();
        }
        let s = s.trim_matches(|c: char| c == '_' || c == '-' || c == '.' || c == ' ').to_string();
        let s = if s.is_empty() { "file".to_string() } else { s };
        let ext = if lower_ext { it.ext.to_lowercase() } else { it.ext.clone() };
        let target = ctx.out_path(it, &s, &ext)?;
        link_or_copy(&it.path_buf(), &target)?;
        out.push(it.derive(&target));
        ctx.progress(i + 1, total, &it.name);
    }
    Ok(out)
}

pub fn parts(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "split");
    let size = (p.f64("size_mb", 100.0).max(0.01) * 1_048_576.0) as u64;
    if mode == "join" {
        let mut groups: BTreeMap<String, Vec<FileItem>> = BTreeMap::new();
        let mut pass = vec![];
        for f in files {
            let is_part = f.ext.len() == 3 && f.ext.chars().all(|c| c.is_ascii_digit());
            if is_part {
                groups.entry(join_rel(&f.rel, &f.stem())).or_default().push(f);
            } else {
                pass.push(f);
            }
        }
        if groups.is_empty() {
            bail!("No .001/.002\u{2026} parts in the input");
        }
        let total = groups.len();
        let mut out = vec![];
        for (i, (_, mut list)) in groups.into_iter().enumerate() {
            ctx.check()?;
            list.sort_by(|a, b| a.ext.cmp(&b.ext));
            let first = list[0].clone();
            let name = first.stem();
            let (stem, ext) = match name.rsplit_once('.') {
                Some((s, e)) => (s.to_string(), e.to_string()),
                None => (name.clone(), String::new()),
            };
            let target = ctx.out_path(&first, &stem, &ext)?;
            let mut w = std::io::BufWriter::new(std::fs::File::create(&target)?);
            for part in &list {
                let mut r = std::fs::File::open(&part.path)?;
                std::io::copy(&mut r, &mut w)?;
            }
            w.flush()?;
            drop(w);
            let mut f = FileItem::from_path(&target, &first.rel);
            f.origin = target.to_string_lossy().into_owned();
            out.push(f);
            ctx.progress(i + 1, total, &name);
        }
        out.extend(pass);
        return Ok(out);
    }
    batch(ctx, files, &[], true, |it| {
        if it.size <= size {
            return Ok(vec![it.clone()]);
        }
        let mut r = std::fs::File::open(&it.path)?;
        let mut buf = vec![0u8; 1 << 20];
        let mut out = vec![];
        let mut idx = 1;
        loop {
            ctx.check()?;
            let target = ctx.out_path(it, &it.name, &format!("{idx:03}"))?;
            let mut w = std::io::BufWriter::new(std::fs::File::create(&target)?);
            let mut written = 0u64;
            while written < size {
                let want = ((size - written) as usize).min(buf.len());
                let n = r.read(&mut buf[..want])?;
                if n == 0 {
                    break;
                }
                w.write_all(&buf[..n])?;
                written += n as u64;
            }
            w.flush()?;
            drop(w);
            if written == 0 {
                let _ = std::fs::remove_file(&target);
                break;
            }
            out.push(it.derive(&target));
            idx += 1;
            if written < size {
                break;
            }
        }
        Ok(out)
    })
}

fn alg_for(hash_len: usize, ext: &str) -> &'static str {
    match (ext, hash_len) {
        ("md5", _) | (_, 32) => "md5",
        ("sha1", _) | (_, 40) => "sha1",
        ("b3" | "blake3", _) => "blake3",
        ("sha512", _) | (_, 128) => "sha512",
        ("crc32" | "sfv", _) | (_, 8) => "crc32",
        _ => "sha256",
    }
}

pub fn verify(ctx: &Ctx, _p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let is_sum = |f: &FileItem| matches!(f.ext.as_str(), "sha256" | "sha1" | "md5" | "sha512" | "b3" | "blake3" | "crc32" | "sums" | "sfv") || f.name.to_lowercase().contains("checksum") || f.name.to_lowercase().ends_with("sums.txt");
    let (sums, data): (Vec<FileItem>, Vec<FileItem>) = files.into_iter().partition(|f| is_sum(f));
    if sums.is_empty() {
        bail!("Add a checksum file (.sha256, .md5, SHA256SUMS\u{2026}) to the input");
    }
    let mut expected: HashMap<String, (String, &'static str)> = HashMap::new();
    for s in &sums {
        let (text, _) = crate::ops::data::decode(&std::fs::read(&s.path)?, "auto")?;
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
                continue;
            }
            let (hash, name) = if s.ext == "sfv" {
                match l.rsplit_once(' ') {
                    Some((n, h)) => (h.trim().to_lowercase(), n.trim().to_string()),
                    None => continue,
                }
            } else {
                match l.split_once(char::is_whitespace) {
                    Some((h, n)) => (h.trim().to_lowercase(), n.trim().trim_start_matches('*').to_string()),
                    None => continue,
                }
            };
            let key = name.replace('\\', "/").rsplit('/').next().unwrap_or(&name).to_lowercase();
            expected.insert(key, (hash.clone(), alg_for(hash.len(), &s.ext)));
        }
    }
    let total = data.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<(FileItem, String)> = data
        .par_iter()
        .map(|f| {
            let status = match expected.get(&f.name.to_lowercase()) {
                None => "not listed".to_string(),
                Some((h, alg)) => match hash_file(&f.path_buf(), alg) {
                    Ok(got) if got.eq_ignore_ascii_case(h) => "ok".to_string(),
                    Ok(_) => "MISMATCH".to_string(),
                    Err(e) => format!("error: {e}"),
                },
            };
            let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            ctx.progress(d, total, &f.name);
            (f.clone(), status)
        })
        .collect();
    ctx.check()?;
    let mut report = String::from("file,status\n");
    let (mut ok, mut bad) = (vec![], vec![]);
    for (f, s) in results {
        report.push_str(&format!("\"{}\",{}\n", f.name.replace('"', "\"\""), s));
        if s == "ok" {
            ok.push(f);
        } else {
            bad.push(f);
        }
    }
    if !bad.is_empty() {
        ctx.warn(format!("{} file(s) failed verification or are not listed", bad.len()));
    }
    let rp = ctx.reserve(&ctx.out_dir("")?, "verify-report", "csv")?;
    std::fs::write(&rp, report)?;
    Ok(vec![ok, bad, vec![FileItem::from_path(&rp, "")]])
}

fn human(n: u64) -> String {
    let f = n as f64;
    if f >= 1_073_741_824.0 {
        format!("{:.2} GB", f / 1_073_741_824.0)
    } else if f >= 1_048_576.0 {
        format!("{:.1} MB", f / 1_048_576.0)
    } else if f >= 1024.0 {
        format!("{:.0} KB", f / 1024.0)
    } else {
        format!("{n} B")
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn report(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let fmt = p.str("format", "html");
    let title = p.str("title", "Files");
    let thumbs = p.bool("thumbnails", true);
    if files.is_empty() {
        bail!("No files in the input");
    }
    let total = files.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let rows: Vec<Value> = files
        .par_iter()
        .map(|it| {
            let path = it.path_buf();
            let mut thumb = String::new();
            if fmt == "html" && thumbs && it.kind == "image" {
                if let Ok(im) = crate::ops::img2::open_any(ctx, &path) {
                    let t = im.thumbnail(240, 240);
                    if let Ok(b) = crate::ops::img::encode(&t, "jpg", &crate::ops::img::EncodeOpts::keep()) {
                        thumb = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(b));
                    }
                }
            }
            let (w, h) = if it.kind == "image" && it.ext != "svg" { image::image_dimensions(&path).unwrap_or((0, 0)) } else { (0, 0) };
            let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            ctx.progress(d, total, &it.name);
            json!({
                "name": it.name, "folder": it.rel, "path": it.path, "kind": it.kind, "ext": it.ext, "size": it.size,
                "modified": modified(&path).map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
                "width": if w > 0 { json!(w) } else { Value::Null }, "height": if h > 0 { json!(h) } else { Value::Null }, "thumb": thumb
            })
        })
        .collect();
    ctx.check()?;
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for f in &files {
        *kinds.entry(f.kind.clone()).or_default() += 1;
    }
    let cell = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    };
    let cols = ["name", "folder", "kind", "ext", "size", "modified", "width", "height", "path"];
    let (body, ext) = match fmt.as_str() {
        "json" => {
            let clean: Vec<Value> = rows.iter().map(|r| { let mut r = r.clone(); if let Some(o) = r.as_object_mut() { o.remove("thumb"); } r }).collect();
            (serde_json::to_string_pretty(&json!({"title": title, "count": total, "total_size": total_size, "kinds": kinds, "files": clean}))?, "json")
        }
        "csv" => {
            let mut w = csv::Writer::from_writer(vec![]);
            w.write_record(cols)?;
            for r in &rows {
                w.write_record(cols.iter().map(|c| cell(&r[*c])))?;
            }
            (String::from_utf8(w.into_inner().map_err(|e| anyhow!("csv: {e}"))?)?, "csv")
        }
        "md" => {
            let mut s = format!("# {title}\n\n{total} files, {}\n\n| name | folder | kind | size | modified |\n|---|---|---|---|---|\n", human(total_size));
            for r in &rows {
                s.push_str(&format!("| {} | {} | {} | {} | {} |\n", cell(&r["name"]).replace('|', "\\|"), cell(&r["folder"]), cell(&r["kind"]), human(r["size"].as_u64().unwrap_or(0)), cell(&r["modified"])));
            }
            (s, "md")
        }
        _ => {
            let chips: String = kinds.iter().map(|(k, n)| format!("<span class=chip>{} \u{b7} {}</span>", esc(k), n)).collect();
            let mut cards = String::new();
            for r in &rows {
                let t = cell(&r["thumb"]);
                let ext = cell(&r["ext"]).to_uppercase();
                let media = if t.is_empty() { format!("<div class=ph>{}</div>", esc(if ext.is_empty() { "FILE" } else { &ext })) } else { format!("<img loading=lazy src=\"{t}\">") };
                let dims = match (r["width"].as_u64(), r["height"].as_u64()) {
                    (Some(w), Some(h)) => format!(" \u{b7} {w}\u{d7}{h}"),
                    _ => String::new(),
                };
                cards.push_str(&format!(
                    "<a class=card href=\"file:///{}\" title=\"{}\">{media}<div class=meta><b>{}</b><span>{}{} \u{b7} {}</span></div></a>",
                    esc(&cell(&r["path"]).replace('\\', "/").trim_start_matches('/').to_string()),
                    esc(&cell(&r["path"])),
                    esc(&cell(&r["name"])),
                    human(r["size"].as_u64().unwrap_or(0)),
                    dims,
                    esc(&cell(&r["modified"]))
                ));
            }
            (format!("<!doctype html><html><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{t}</title><style>body{{margin:0;font:14px/1.4 system-ui,'Segoe UI',Roboto,sans-serif;background:#0b0e14;color:#e6ebf4}}header{{padding:28px 32px 12px}}h1{{margin:0 0 6px;font-size:24px;font-weight:600}}.sub{{color:#8a93a6}}.chip{{display:inline-block;margin:10px 8px 0 0;padding:4px 10px;border-radius:999px;background:#161b24;border:1px solid #2d3544;color:#b9c3d6;font-size:12px}}main{{display:grid;grid-template-columns:repeat(auto-fill,minmax(200px,1fr));gap:14px;padding:16px 32px 40px}}.card{{display:block;background:#11151c;border:1px solid #212733;border-radius:14px;overflow:hidden;color:inherit;text-decoration:none;transition:.15s}}.card:hover{{border-color:#2f6ff2;transform:translateY(-2px)}}.card img,.ph{{width:100%;aspect-ratio:4/3;object-fit:cover;display:block;background:#161b24}}.ph{{display:grid;place-items:center;color:#5a6376;font-weight:700;letter-spacing:.08em}}.meta{{padding:10px 12px}}.meta b{{display:block;font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}.meta span{{color:#8a93a6;font-size:12px}}</style></head><body><header><h1>{t}</h1><div class=sub>{total} files \u{b7} {}</div>{chips}</header><main>{cards}</main></body></html>", human(total_size), t = esc(&title)), "html")
        }
    };
    let out = ctx.reserve(&ctx.out_dir("")?, &sanitize(&title), ext)?;
    std::fs::write(&out, body)?;
    Ok(vec![files, vec![FileItem::from_path(&out, "")]])
}
