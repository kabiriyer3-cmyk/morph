//! PDF tools (lopdf): merge, split, select/reorder/rotate pages, metadata, compression, text;
//! page rendering through Ghostscript or Poppler when installed.
use super::{batch, run_tool, Outs};
use crate::engine::{Ctx, FileItem};
use crate::util::{command, Params};
use anyhow::{anyhow, bail, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const INHERIT: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

pub fn load(path: &Path) -> Result<Document> {
    let doc = Document::load(path).map_err(|e| anyhow!("cannot read the PDF: {e}"))?;
    if doc.trailer.get(b"Encrypt").is_ok() {
        bail!("the PDF is password protected");
    }
    Ok(doc)
}

fn save(mut doc: Document, path: &Path) -> Result<()> {
    doc.compress();
    doc.save(path).map_err(|e| anyhow!("cannot write the PDF: {e}"))?;
    Ok(())
}

/// "1-3, 5, 8-" (1-based; "odd", "even", "last" and reversed ranges like "9-1" work too).
pub fn parse_ranges(spec: &str, n: u32) -> Vec<u32> {
    let mut out = vec![];
    for tok in spec.split([',', ';']) {
        let t = tok.trim().to_lowercase().replace(' ', "");
        if t.is_empty() {
            continue;
        }
        match t.as_str() {
            "odd" => {
                out.extend((1..=n).filter(|i| i % 2 == 1));
                continue;
            }
            "even" => {
                out.extend((1..=n).filter(|i| i % 2 == 0));
                continue;
            }
            "all" | "*" => {
                out.extend(1..=n);
                continue;
            }
            _ => {}
        }
        let num = |s: &str| -> Option<u32> {
            match s {
                "" => None,
                "last" | "end" | "z" => Some(n),
                _ => s.parse::<u32>().ok(),
            }
        };
        if let Some((a, b)) = t.split_once('-') {
            let lo = num(a).unwrap_or(1).clamp(1, n.max(1));
            let hi = num(b).unwrap_or(n).clamp(1, n.max(1));
            if lo <= hi {
                out.extend(lo..=hi);
            } else {
                out.extend((hi..=lo).rev());
            }
        } else if let Some(v) = num(&t) {
            if v >= 1 && v <= n {
                out.push(v);
            }
        }
    }
    out
}

fn inherited(doc: &Document, page: ObjectId) -> Dictionary {
    let mut dict = doc.get_object(page).and_then(Object::as_dict).cloned().unwrap_or_default();
    let mut parent = dict.get(b"Parent").and_then(Object::as_reference).ok();
    let mut guard = 0;
    while let Some(pid) = parent {
        guard += 1;
        if guard > 64 {
            break;
        }
        let Ok(pd) = doc.get_object(pid).and_then(Object::as_dict) else { break };
        for key in INHERIT {
            if !dict.has(key) {
                if let Ok(v) = pd.get(key) {
                    dict.set(key.to_vec(), v.clone());
                }
            }
        }
        parent = pd.get(b"Parent").and_then(Object::as_reference).ok();
    }
    dict
}

fn pages_root(doc: &Document) -> Result<ObjectId> {
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|_| anyhow!("the PDF has no catalog"))?;
    doc.get_object(root)
        .and_then(Object::as_dict)
        .and_then(|d| d.get(b"Pages"))
        .and_then(Object::as_reference)
        .map_err(|_| anyhow!("the PDF has no page tree"))
}

/// Rebuilds a flat page tree with exactly `order` (inherited attributes are copied into each page).
fn rebuild(doc: &mut Document, order: &[ObjectId]) -> Result<()> {
    if order.is_empty() {
        bail!("no pages left");
    }
    let root = pages_root(doc)?;
    let dicts: Vec<(ObjectId, Dictionary)> = order.iter().map(|id| (*id, inherited(doc, *id))).collect();
    let mut seen = HashSet::new();
    let mut kids = vec![];
    for (id, mut d) in dicts {
        let id = if seen.insert(id) { id } else { doc.new_object_id() };
        d.set("Parent", Object::Reference(root));
        doc.objects.insert(id, Object::Dictionary(d));
        kids.push(Object::Reference(id));
    }
    let count = kids.len() as i64;
    let rd = doc.get_object_mut(root).and_then(Object::as_dict_mut).map_err(|_| anyhow!("broken page tree"))?;
    rd.set("Kids", Object::Array(kids));
    rd.set("Count", count);
    let _ = doc.prune_objects();
    Ok(())
}

fn type_of(o: &Object) -> Option<Vec<u8>> {
    o.as_dict().ok().and_then(|d| d.get(b"Type").ok()).and_then(|t| t.as_name().ok()).map(|n| n.to_vec())
}

fn merge_docs(mut docs: Vec<Document>) -> Result<Document> {
    if docs.is_empty() {
        bail!("no PDFs to merge");
    }
    let mut base = docs.remove(0);
    let mut order: Vec<ObjectId> = base.get_pages().values().copied().collect();
    for mut d in docs {
        d.renumber_objects_with(base.max_id + 1);
        let pages: Vec<ObjectId> = d.get_pages().values().copied().collect();
        let mats: Vec<(ObjectId, Dictionary)> = pages.iter().map(|id| (*id, inherited(&d, *id))).collect();
        let max = d.max_id;
        for (id, obj) in std::mem::take(&mut d.objects) {
            let t = type_of(&obj);
            if matches!(t.as_deref(), Some(b"Catalog") | Some(b"Pages") | Some(b"Outlines") | Some(b"Outline")) {
                continue;
            }
            base.objects.insert(id, obj);
        }
        for (id, dict) in mats {
            base.objects.insert(id, Object::Dictionary(dict));
            order.push(id);
        }
        base.max_id = base.max_id.max(max);
    }
    rebuild(&mut base, &order)?;
    Ok(base)
}

fn natural_key(s: &str) -> Vec<(u8, u64, String)> {
    let mut out = vec![];
    let mut num = String::new();
    let mut txt = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_digit() {
            if !txt.is_empty() {
                out.push((1, 0, std::mem::take(&mut txt)));
            }
            num.push(c);
        } else {
            if !num.is_empty() {
                out.push((0, num.parse().unwrap_or(0), String::new()));
                num.clear();
            }
            txt.push(c);
        }
    }
    if !num.is_empty() {
        out.push((0, num.parse().unwrap_or(0), String::new()));
    }
    if !txt.is_empty() {
        out.push((1, 0, txt));
    }
    out
}

pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    natural_key(a).cmp(&natural_key(b))
}

pub fn merge(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let (mut pdfs, pass): (Vec<FileItem>, Vec<FileItem>) = files.into_iter().partition(|f| f.ext == "pdf");
    if pdfs.is_empty() {
        bail!("No PDF files in the input");
    }
    if p.str("order", "input") == "name" {
        pdfs.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    }
    let total = pdfs.len();
    let mut docs = vec![];
    for (i, it) in pdfs.iter().enumerate() {
        ctx.check()?;
        match load(&it.path_buf()) {
            Ok(d) => docs.push(d),
            Err(e) => ctx.warn(format!("{}: {:#}", it.name, e)),
        }
        ctx.report(i as f64 / total as f64 * 0.6, &it.name);
    }
    if docs.is_empty() {
        bail!("No readable PDFs. {}", ctx.first_warning());
    }
    let doc = merge_docs(docs)?;
    let name = p.str("name", "merged");
    let out = ctx.reserve(&ctx.out_dir(&pdfs[0].rel)?, &name, "pdf")?;
    save(doc, &out)?;
    ctx.progress(1, 1, "");
    let mut v = vec![FileItem::from_path(&out, &pdfs[0].rel)];
    v.extend(pass);
    Ok(v)
}

pub fn split(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "pages");
    let every = p.i64("every", 2).max(1) as usize;
    let ranges = p.str("ranges", "1-3, 4-");
    batch(ctx, files, &[], false, |it| {
        if it.ext != "pdf" {
            return Ok(vec![it.clone()]);
        }
        let doc = load(&it.path_buf())?;
        let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
        let n = pages.len() as u32;
        let groups: Vec<Vec<u32>> = match mode.as_str() {
            "every" => (1..=n).collect::<Vec<_>>().chunks(every).map(|c| c.to_vec()).collect(),
            "ranges" => ranges.split([',', ';']).map(|r| parse_ranges(r, n)).filter(|g| !g.is_empty()).collect(),
            _ => (1..=n).map(|i| vec![i]).collect(),
        };
        let width = n.to_string().len().max(2);
        let mut out = vec![];
        for g in groups {
            ctx.check()?;
            let mut d = doc.clone();
            let order: Vec<ObjectId> = g.iter().filter_map(|i| pages.get(*i as usize - 1).copied()).collect();
            rebuild(&mut d, &order)?;
            let label = if g.len() == 1 {
                format!("{:0w$}", g[0], w = width)
            } else {
                format!("{:0w$}-{:0w$}", g[0], g[g.len() - 1], w = width)
            };
            let path = ctx.out_path(it, &format!("{}_p{}", it.stem(), label), "pdf")?;
            save(d, &path)?;
            out.push(it.derive(&path));
        }
        Ok(out)
    })
}

pub fn pages(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let op = p.str("op", "keep");
    let spec = p.str("pages", "1-3");
    let angle = p.i64("angle", 90);
    batch(ctx, files, &[], true, |it| {
        if it.ext != "pdf" {
            return Ok(vec![it.clone()]);
        }
        let mut doc = load(&it.path_buf())?;
        let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
        let n = pages.len() as u32;
        if op == "rotate" {
            let sel: HashSet<u32> = if spec.trim().is_empty() { (1..=n).collect() } else { parse_ranges(&spec, n).into_iter().collect() };
            for (i, pid) in pages.iter().enumerate() {
                if !sel.contains(&(i as u32 + 1)) {
                    continue;
                }
                let cur = inherited(&doc, *pid).get(b"Rotate").and_then(Object::as_i64).unwrap_or(0);
                if let Ok(d) = doc.get_object_mut(*pid).and_then(Object::as_dict_mut) {
                    d.set("Rotate", (cur + angle).rem_euclid(360));
                }
            }
        } else {
            let order: Vec<u32> = match op.as_str() {
                "delete" => {
                    let del: HashSet<u32> = parse_ranges(&spec, n).into_iter().collect();
                    (1..=n).filter(|i| !del.contains(i)).collect()
                }
                "reverse" => (1..=n).rev().collect(),
                "odd" => (1..=n).filter(|i| i % 2 == 1).collect(),
                "even" => (1..=n).filter(|i| i % 2 == 0).collect(),
                _ => parse_ranges(&spec, n),
            };
            let ids: Vec<ObjectId> = order.iter().filter_map(|i| pages.get(*i as usize - 1).copied()).collect();
            rebuild(&mut doc, &ids)?;
        }
        let path = ctx.out_path(it, &it.stem(), "pdf")?;
        save(doc, &path)?;
        Ok(vec![it.derive(&path)])
    })
}

fn pdf_string(o: &Object) -> String {
    match o {
        Object::String(b, _) => {
            if b.len() >= 2 && b[0] == 0xFE && b[1] == 0xFF {
                let u: Vec<u16> = b[2..].chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect();
                String::from_utf16_lossy(&u)
            } else {
                b.iter().map(|&c| c as char).collect()
            }
        }
        Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
        Object::Integer(i) => i.to_string(),
        Object::Real(f) => f.to_string(),
        Object::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}

fn text_obj(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut b = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            b.extend_from_slice(&u.to_be_bytes());
        }
        Object::String(b, StringFormat::Hexadecimal)
    }
}

fn info_dict(doc: &Document) -> Option<Dictionary> {
    match doc.trailer.get(b"Info").ok()? {
        Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok().cloned(),
        Object::Dictionary(d) => Some(d.clone()),
        _ => None,
    }
}

fn page_size_mm(doc: &Document, page: ObjectId) -> Option<(f64, f64)> {
    let d = inherited(doc, page);
    let mb = d.get(b"MediaBox").ok()?.as_array().ok()?;
    let v: Vec<f64> = mb
        .iter()
        .filter_map(|o| match o {
            Object::Integer(i) => Some(*i as f64),
            Object::Real(f) => Some(*f as f64),
            _ => None,
        })
        .collect();
    if v.len() < 4 {
        return None;
    }
    let mm = 25.4 / 72.0;
    Some((((v[2] - v[0]).abs() * mm).round(), ((v[3] - v[1]).abs() * mm).round()))
}

pub fn summary(path: &Path) -> Result<Value> {
    let doc = Document::load(path).map_err(|e| anyhow!("cannot read the PDF: {e}"))?;
    let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    let mut v = json!({
        "pages": pages.len(),
        "version": doc.version,
        "encrypted": doc.trailer.get(b"Encrypt").is_ok(),
    });
    if let Some(first) = pages.first() {
        if let Some((w, h)) = page_size_mm(&doc, *first) {
            v["page_mm"] = json!([w, h]);
        }
    }
    if let Some(info) = info_dict(&doc) {
        for (k, key) in [("title", "Title"), ("author", "Author"), ("subject", "Subject"), ("keywords", "Keywords"), ("creator", "Creator"), ("producer", "Producer"), ("created", "CreationDate"), ("modified", "ModDate")] {
            if let Ok(o) = info.get(key.as_bytes()) {
                let s = pdf_string(o);
                if !s.trim().is_empty() {
                    v[k] = json!(s.trim());
                }
            }
        }
    }
    Ok(v)
}

pub fn meta(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let mode = p.str("mode", "report");
    let fmt = p.str("format", "json");
    if mode == "report" {
        let pdfs: Vec<&FileItem> = files.iter().filter(|f| f.ext == "pdf").collect();
        if pdfs.is_empty() {
            bail!("No PDF files in the input");
        }
        let mut rows = vec![];
        for (i, it) in pdfs.iter().enumerate() {
            ctx.check()?;
            match summary(&it.path_buf()) {
                Ok(mut v) => {
                    v["file"] = json!(it.name);
                    v["size"] = json!(it.size);
                    rows.push(v);
                }
                Err(e) => ctx.warn(format!("{}: {:#}", it.name, e)),
            }
            ctx.progress(i + 1, pdfs.len(), &it.name);
        }
        let (body, ext) = if fmt == "csv" {
            let cols = ["file", "pages", "version", "title", "author", "subject", "keywords", "creator", "producer", "created", "modified", "encrypted", "size"];
            let mut w = csv::Writer::from_writer(vec![]);
            w.write_record(cols)?;
            for r in &rows {
                w.write_record(cols.iter().map(|c| match &r[*c] {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    o => o.to_string(),
                }))?;
            }
            (String::from_utf8(w.into_inner().map_err(|e| anyhow!("csv: {e}"))?)?, "csv")
        } else {
            (serde_json::to_string_pretty(&rows)?, "json")
        };
        let out = ctx.reserve(&ctx.out_dir("")?, "pdf-info", ext)?;
        std::fs::write(&out, body)?;
        return Ok(vec![files, vec![FileItem::from_path(&out, "")]]);
    }
    let fields: Vec<(&str, String)> = ["title", "author", "subject", "keywords", "creator"].iter().map(|k| (*k, p.str(k, ""))).collect();
    let out = batch(ctx, files, &[], true, |it| {
        if it.ext != "pdf" {
            return Ok(vec![it.clone()]);
        }
        let mut doc = load(&it.path_buf())?;
        if mode == "strip" {
            doc.trailer.remove(b"Info");
            if let Ok(root) = doc.trailer.get(b"Root").and_then(Object::as_reference) {
                if let Ok(d) = doc.get_object_mut(root).and_then(Object::as_dict_mut) {
                    d.remove(b"Metadata");
                }
            }
            let _ = doc.prune_objects();
        } else {
            let mut info = info_dict(&doc).unwrap_or_default();
            for (k, v) in &fields {
                if !v.trim().is_empty() {
                    let key = match *k {
                        "title" => "Title",
                        "author" => "Author",
                        "subject" => "Subject",
                        "keywords" => "Keywords",
                        _ => "Creator",
                    };
                    info.set(key, text_obj(v.trim()));
                }
            }
            info.set("Producer", text_obj("Morph"));
            let id = doc.add_object(Object::Dictionary(info));
            doc.trailer.set("Info", Object::Reference(id));
        }
        let path = ctx.out_path(it, &it.stem(), "pdf")?;
        save(doc, &path)?;
        Ok(vec![it.derive(&path)])
    })?;
    Ok(vec![out, vec![]])
}

pub fn compress(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "lossless");
    let keep_smaller = p.bool("keep_smaller", true);
    if mode != "lossless" {
        ctx.tools.gs()?;
    }
    batch(ctx, files, &[], mode == "lossless", |it| {
        if it.ext != "pdf" {
            return Ok(vec![it.clone()]);
        }
        let out = ctx.out_path(it, &it.stem(), "pdf")?;
        if mode == "lossless" {
            let mut doc = load(&it.path_buf())?;
            let _ = doc.prune_objects();
            save(doc, &out)?;
        } else {
            let gs = ctx.tools.gs()?;
            let mut cmd = command(gs);
            cmd.arg("-sDEVICE=pdfwrite")
                .arg("-dCompatibilityLevel=1.5")
                .arg(format!("-dPDFSETTINGS=/{mode}"))
                .arg("-dNOPAUSE")
                .arg("-dQUIET")
                .arg("-dBATCH")
                .arg("-dSAFER")
                .arg(format!("-sOutputFile={}", out.display()))
                .arg(&it.path);
            run_tool(ctx, &mut cmd, "Ghostscript")?;
        }
        let new_size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(u64::MAX);
        if keep_smaller && new_size >= it.size {
            std::fs::copy(&it.path, &out)?;
            ctx.warn(format!("{}: could not be made smaller, kept the original", it.name));
        }
        Ok(vec![it.derive(&out)])
    })
}

pub fn pdf_text(path: &Path, marks: bool) -> Result<String> {
    let doc = load(path)?;
    let nums: Vec<u32> = doc.get_pages().keys().copied().collect();
    let mut s = String::new();
    for n in nums {
        let t = doc.extract_text(&[n]).unwrap_or_default();
        if marks {
            s.push_str(&format!("--- {n} ---\n"));
        }
        s.push_str(t.trim_end());
        s.push_str("\n\n");
    }
    if s.replace("---", "").trim().chars().all(|c| c.is_ascii_digit() || c.is_whitespace()) {
        bail!("no text layer found (scanned PDF?)");
    }
    Ok(s)
}

pub fn text(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let marks = p.bool("page_marks", true);
    batch(ctx, files, &[], true, |it| {
        if it.ext != "pdf" {
            return Ok(vec![it.clone()]);
        }
        let s = pdf_text(&it.path_buf(), marks)?;
        let out = ctx.out_path(it, &it.stem(), "txt")?;
        std::fs::write(&out, s)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn render(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = if p.str("format", "png") == "jpg" { "jpg" } else { "png" };
    let dpi = p.i64("dpi", 150).clamp(36, 1200);
    let first = p.i64("first", 1).max(1);
    let last = p.i64("last", 0).max(0);
    if ctx.tools.gs.is_none() && ctx.tools.pdftoppm.is_none() {
        bail!("{}", crate::tools::NO_RENDERER);
    }
    batch(ctx, files, &[], false, |it| {
        if it.ext != "pdf" {
            return Ok(vec![it.clone()]);
        }
        let tmp = ctx.reserve(&ctx.work, "_render", "")?;
        std::fs::create_dir_all(&tmp)?;
        if let Some(gs) = ctx.tools.gs.clone() {
            let dev = if fmt == "jpg" { "jpeg" } else { "png16m" };
            let mut cmd = command(&gs);
            cmd.arg("-dSAFER")
                .arg("-dBATCH")
                .arg("-dNOPAUSE")
                .arg("-dQUIET")
                .arg(format!("-sDEVICE={dev}"))
                .arg(format!("-r{dpi}"))
                .arg("-dTextAlphaBits=4")
                .arg("-dGraphicsAlphaBits=4")
                .arg(format!("-dFirstPage={first}"));
            if last > 0 {
                cmd.arg(format!("-dLastPage={last}"));
            }
            if fmt == "jpg" {
                cmd.arg("-dJPEGQ=92");
            }
            cmd.arg(format!("-sOutputFile={}", tmp.join(format!("page-%04d.{fmt}")).display())).arg(&it.path);
            run_tool(ctx, &mut cmd, "Ghostscript")?;
        } else if let Some(pp) = ctx.tools.pdftoppm.clone() {
            let mut cmd = command(&pp);
            cmd.arg(if fmt == "jpg" { "-jpeg" } else { "-png" }).arg("-r").arg(dpi.to_string()).arg("-f").arg(first.to_string());
            if last > 0 {
                cmd.arg("-l").arg(last.to_string());
            }
            cmd.arg(&it.path).arg(tmp.join("page"));
            run_tool(ctx, &mut cmd, "pdftoppm")?;
        }
        let mut produced: Vec<PathBuf> = std::fs::read_dir(&tmp)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file()).collect();
        produced.sort();
        let mut out = vec![];
        for (i, f) in produced.iter().enumerate() {
            let ext = crate::kinds::ext_of(f);
            let path = ctx.out_path(it, &format!("{}_p{:03}", it.stem(), first as usize + i), &ext)?;
            if std::fs::rename(f, &path).is_err() {
                std::fs::copy(f, &path)?;
            }
            out.push(it.derive(&path));
        }
        let _ = std::fs::remove_dir_all(&tmp);
        if out.is_empty() {
            bail!("no pages were rendered");
        }
        Ok(out)
    })
}

#[tauri::command]
pub async fn pdf_info(path: String) -> std::result::Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || summary(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{:#}", e))
}
