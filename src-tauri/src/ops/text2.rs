//! Text and data tools: line operations, merge/split, extraction, statistics, encoders and JSON tools.
use super::{batch, Outs};
use crate::engine::{Ctx, FileItem};
use crate::kinds;
use crate::ops::data::{decode, encode};
use crate::util::Params;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

fn read_text(it: &FileItem) -> Result<(String, &'static encoding_rs::Encoding)> {
    decode(&std::fs::read(&it.path)?, "auto")
}

fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    crate::ops::pdftools::natural_cmp(a, b)
}

fn wrap(line: &str, width: usize) -> String {
    if width == 0 || line.chars().count() <= width {
        return line.to_string();
    }
    let mut out = vec![];
    let mut cur = String::new();
    for word in line.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.join("\n")
}

pub fn lines(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let op = p.str("op", "sort");
    let desc = p.str("order", "asc") == "desc";
    let nat = p.bool("natural", true);
    let ci = p.bool("ignore_case", true);
    let text = p.str("text", "");
    let pattern = p.str("pattern", "");
    let use_re = p.bool("regex", false);
    let count = p.i64("count", 10).max(0) as usize;
    let width = p.i64("width", 80).max(0) as usize;
    let sep = p.str("separator", ", ").replace("\\n", "\n").replace("\\t", "\t");
    let re = if matches!(op.as_str(), "keep" | "remove") && !pattern.is_empty() {
        Some(if use_re { Regex::new(&pattern).context("invalid regular expression")? } else { Regex::new(&format!("(?i){}", regex::escape(&pattern)))? })
    } else {
        None
    };
    batch(ctx, files, &[], true, |it| {
        if !kinds::is_textual(&it.kind, &it.ext) {
            return Ok(vec![it.clone()]);
        }
        let (s, enc) = read_text(it)?;
        let crlf = s.contains("\r\n");
        let mut v: Vec<String> = s.lines().map(|l| l.to_string()).collect();
        match op.as_str() {
            "sort" => {
                if nat {
                    v.sort_by(|a, b| natural(a, b));
                } else if ci {
                    v.sort_by_cached_key(|l| l.to_lowercase());
                } else {
                    v.sort();
                }
                if desc {
                    v.reverse();
                }
            }
            "sort_length" => {
                v.sort_by_key(|l| l.chars().count());
                if desc {
                    v.reverse();
                }
            }
            "unique" => {
                let mut seen = HashSet::new();
                v.retain(|l| seen.insert(if ci { l.trim().to_lowercase() } else { l.clone() }));
            }
            "reverse" => v.reverse(),
            "shuffle" => {
                let mut s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7) | 1;
                for i in (1..v.len()).rev() {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    v.swap(i, (s % (i as u64 + 1)) as usize);
                }
            }
            "trim" => v.iter_mut().for_each(|l| *l = l.trim().to_string()),
            "remove_empty" => v.retain(|l| !l.trim().is_empty()),
            "number" => {
                let w = v.len().to_string().len();
                let fmt = if text.is_empty() { ". ".to_string() } else { text.clone() };
                v = v.iter().enumerate().map(|(i, l)| format!("{:>w$}{fmt}{l}", i + 1, w = w)).collect();
            }
            "prefix" => v.iter_mut().for_each(|l| *l = format!("{text}{l}")),
            "suffix" => v.iter_mut().for_each(|l| *l = format!("{l}{text}")),
            "keep" => {
                if let Some(r) = &re {
                    v.retain(|l| r.is_match(l));
                }
            }
            "remove" => {
                if let Some(r) = &re {
                    v.retain(|l| !r.is_match(l));
                }
            }
            "wrap" => v = v.iter().map(|l| wrap(l, width)).collect(),
            "join" => v = vec![v.join(&sep)],
            "split" => {
                let s2 = if sep.is_empty() { ",".to_string() } else { sep.clone() };
                v = v.iter().flat_map(|l| l.split(s2.as_str()).map(|x| x.trim().to_string()).collect::<Vec<_>>()).collect();
            }
            "head" => v.truncate(count),
            "tail" => {
                let n = v.len().saturating_sub(count);
                v.drain(..n);
            }
            other => bail!("unknown operation {other}"),
        }
        let nl = if crlf { "\r\n" } else { "\n" };
        let mut body = v.join(nl);
        if s.ends_with('\n') {
            body.push_str(nl);
        }
        let out = ctx.out_path(it, &it.stem(), &it.ext)?;
        std::fs::write(&out, encode(&body, "keep", enc)?)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn merge(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let name = p.str("name", "merged");
    let headers = p.bool("headers", true);
    let header = p.str("header", "===== {name} =====");
    let sep = match p.str("separator", "blank").as_str() {
        "none" => "",
        "newline" => "\n",
        "rule" => "\n\n----------------------------------------\n\n",
        _ => "\n\n",
    };
    let texts: Vec<&FileItem> = files.iter().filter(|f| kinds::is_textual(&f.kind, &f.ext)).collect();
    if texts.is_empty() {
        bail!("No text files in the input");
    }
    let ext = texts[0].ext.clone();
    let mut parts = vec![];
    for it in &texts {
        ctx.check()?;
        let (s, _) = read_text(it)?;
        let s = s.trim_end().to_string();
        parts.push(if headers { format!("{}\n{}", header.replace("{name}", &it.name).replace("{folder}", &it.rel), s) } else { s });
    }
    let body = parts.join(sep) + "\n";
    let out = ctx.reserve(&ctx.out_dir(&texts[0].rel)?, &name, if ext.is_empty() { "txt" } else { &ext })?;
    std::fs::write(&out, body)?;
    Ok(vec![FileItem::from_path(&out, &texts[0].rel)])
}

pub fn split(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "lines");
    let n = p.i64("lines", 1000).max(1) as usize;
    let pattern = p.str("pattern", "^-{3,}$");
    let kb = (p.f64("size_kb", 512.0).max(1.0) * 1024.0) as usize;
    let re = if mode == "regex" { Some(Regex::new(&format!("(?m){pattern}")).context("invalid regular expression")?) } else { None };
    batch(ctx, files, &[], true, |it| {
        if !kinds::is_textual(&it.kind, &it.ext) {
            return Ok(vec![it.clone()]);
        }
        let (s, _) = read_text(it)?;
        let chunks: Vec<String> = match mode.as_str() {
            "regex" => re.as_ref().map(|r| r.split(&s).map(|x| x.trim_matches('\n').to_string()).filter(|x| !x.trim().is_empty()).collect()).unwrap_or_default(),
            "size" => {
                let mut v = vec![];
                let mut cur = String::new();
                for line in s.split_inclusive('\n') {
                    if cur.len() + line.len() > kb && !cur.is_empty() {
                        v.push(std::mem::take(&mut cur));
                    }
                    cur.push_str(line);
                }
                if !cur.is_empty() {
                    v.push(cur);
                }
                v
            }
            _ => s.lines().collect::<Vec<_>>().chunks(n).map(|c| c.join("\n") + "\n").collect(),
        };
        let width = chunks.len().to_string().len().max(3);
        let mut out = vec![];
        for (i, c) in chunks.iter().enumerate() {
            let path = ctx.out_path(it, &format!("{}_{:0w$}", it.stem(), i + 1, w = width), &it.ext)?;
            std::fs::write(&path, c)?;
            out.push(it.derive(&path));
        }
        Ok(out)
    })
}

pub fn extract(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let preset = p.str("what", "emails");
    let custom = p.str("pattern", "");
    let unique = p.bool("unique", true);
    let fmt = p.str("format", "txt");
    let combine = p.bool("combine", true);
    let pat = match preset.as_str() {
        "emails" => r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}",
        "urls" => r#"https?://[^\s<>"')\]]+"#,
        "phones" => r"\+?\d[\d\s().-]{7,}\d",
        "numbers" => r"-?\d+(?:[.,]\d+)?",
        "hashtags" => r"#[\p{L}\p{N}_]+",
        "mentions" => r"@[\p{L}\p{N}_.]+",
        "ips" => r"\b(?:\d{1,3}\.){3}\d{1,3}\b",
        "dates" => r"\b\d{1,4}[./-]\d{1,2}[./-]\d{1,4}\b",
        "words" => r"[\p{L}\p{N}'-]+",
        _ => custom.as_str(),
    };
    if pat.is_empty() {
        bail!("Type a regular expression");
    }
    let re = Regex::new(pat).context("invalid regular expression")?;
    let texts: Vec<&FileItem> = files.iter().filter(|f| kinds::is_textual(&f.kind, &f.ext) || f.kind == "doc").collect();
    if texts.is_empty() {
        bail!("No text files in the input");
    }
    let mut groups: Vec<(&FileItem, Vec<String>)> = vec![];
    for it in &texts {
        ctx.check()?;
        let s = if kinds::is_textual(&it.kind, &it.ext) { read_text(it)?.0 } else { crate::ops::docs::extract_text(&it.path_buf(), &it.ext).unwrap_or_default() };
        let found: Vec<String> = re.find_iter(&s).map(|m| m.as_str().trim().to_string()).collect();
        groups.push((it, found));
    }
    let render = |items: Vec<String>| -> (String, &'static str) {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut order = vec![];
        for x in items {
            let e = counts.entry(x.clone()).or_insert(0);
            if *e == 0 {
                order.push(x);
            }
            *e += 1;
        }
        if fmt == "csv" {
            let mut s = String::from("value,count\n");
            for x in &order {
                s.push_str(&format!("\"{}\",{}\n", x.replace('"', "\"\""), counts[x]));
            }
            (s, "csv")
        } else if unique {
            (order.join("\n") + "\n", "txt")
        } else {
            let mut v = vec![];
            for x in &order {
                for _ in 0..counts[x] {
                    v.push(x.clone());
                }
            }
            (v.join("\n") + "\n", "txt")
        }
    };
    let mut out = vec![];
    if combine {
        let all: Vec<String> = groups.into_iter().flat_map(|(_, v)| v).collect();
        let (body, ext) = render(all);
        let path = ctx.reserve(&ctx.out_dir("")?, &format!("extracted-{preset}"), ext)?;
        std::fs::write(&path, body)?;
        out.push(FileItem::from_path(&path, ""));
    } else {
        for (it, v) in groups {
            let (body, ext) = render(v);
            let path = ctx.out_path(it, &format!("{}_{preset}", it.stem()), ext)?;
            std::fs::write(&path, body)?;
            out.push(it.derive(&path));
        }
    }
    Ok(out)
}

pub fn stats(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let fmt = p.str("format", "csv");
    let top = p.i64("top_words", 10).clamp(0, 100) as usize;
    let word = Regex::new(r"[\p{L}\p{N}'-]+").expect("word regex");
    let texts: Vec<&FileItem> = files.iter().filter(|f| kinds::is_textual(&f.kind, &f.ext)).collect();
    if texts.is_empty() {
        bail!("No text files in the input");
    }
    let mut rows = vec![];
    for it in &texts {
        ctx.check()?;
        let (s, enc) = read_text(it)?;
        let words: Vec<String> = word.find_iter(&s).map(|m| m.as_str().to_lowercase()).collect();
        let mut freq: HashMap<&str, usize> = HashMap::new();
        for w in &words {
            if w.chars().count() > 3 {
                *freq.entry(w.as_str()).or_default() += 1;
            }
        }
        let mut tops: Vec<(&str, usize)> = freq.into_iter().collect();
        tops.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let minutes = (words.len() as f64 / 200.0 * 10.0).round() / 10.0;
        rows.push(json!({
            "file": it.name, "lines": s.lines().count(), "words": words.len(), "chars": s.chars().count(),
            "chars_no_spaces": s.chars().filter(|c| !c.is_whitespace()).count(), "encoding": enc.name(), "reading_minutes": minutes,
            "top_words": tops.iter().take(top).map(|(w, n)| format!("{w} ({n})")).collect::<Vec<_>>().join(", ")
        }));
    }
    let cols = ["file", "lines", "words", "chars", "chars_no_spaces", "reading_minutes", "encoding", "top_words"];
    let (body, ext) = if fmt == "json" {
        (serde_json::to_string_pretty(&rows)?, "json")
    } else {
        let mut w = csv::Writer::from_writer(vec![]);
        w.write_record(cols)?;
        for r in &rows {
            w.write_record(cols.iter().map(|c| match &r[*c] {
                Value::String(s) => s.clone(),
                o => o.to_string(),
            }))?;
        }
        (String::from_utf8(w.into_inner().map_err(|e| anyhow!("csv: {e}"))?)?, "csv")
    };
    let out = ctx.reserve(&ctx.out_dir("")?, "text-stats", ext)?;
    std::fs::write(&out, body)?;
    Ok(vec![files, vec![FileItem::from_path(&out, "")]])
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 && i + 2 <= b.len() - 1 + 1 {
            if let Some(v) = std::str::from_utf8(&b[i + 1..(i + 3).min(b.len())]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='m' | 'A'..='M' => ((c as u8) + 13) as char,
            'n'..='z' | 'N'..='Z' => ((c as u8) - 13) as char,
            _ => c,
        })
        .collect()
}

pub fn encode_node(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let op = p.str("op", "base64-encode");
    let b64 = base64::engine::general_purpose::STANDARD;
    batch(ctx, files, &[], true, |it| {
        let bytes = std::fs::read(&it.path)?;
        let text = || -> Result<String> { Ok(decode(&bytes, "auto")?.0) };
        let (body, name, ext): (Vec<u8>, String, String) = match op.as_str() {
            "base64-encode" => (b64.encode(&bytes).into_bytes(), it.name.clone(), "b64".into()),
            "hex-encode" => (crate::util::hex(&bytes).into_bytes(), it.name.clone(), "hex".into()),
            "base64-decode" | "hex-decode" => {
                let t = text()?;
                let clean: String = t.chars().filter(|c| !c.is_whitespace()).collect();
                let clean = clean.split_once(";base64,").map(|(_, b)| b.to_string()).unwrap_or(clean);
                let data = if op == "hex-decode" {
                    (0..clean.len() / 2).map(|i| u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16)).collect::<std::result::Result<Vec<u8>, _>>().map_err(|_| anyhow!("not valid hex"))?
                } else {
                    b64.decode(clean.as_bytes()).map_err(|e| anyhow!("not valid Base64: {e}"))?
                };
                let (stem, ext) = match it.stem().rsplit_once('.') {
                    Some((s, e)) => (s.to_string(), e.to_string()),
                    None => (it.stem(), "bin".to_string()),
                };
                (data, stem, ext)
            }
            "url-encode" => (url_encode(&text()?).into_bytes(), it.stem(), it.ext.clone()),
            "url-decode" => (url_decode(&text()?).into_bytes(), it.stem(), it.ext.clone()),
            "html-escape" => (
                text()?.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;").into_bytes(),
                it.stem(),
                it.ext.clone(),
            ),
            "html-unescape" => (crate::ops::docs::decode_entities(&text()?).into_bytes(), it.stem(), it.ext.clone()),
            "rot13" => (rot13(&text()?).into_bytes(), it.stem(), it.ext.clone()),
            "unicode-escape" => (
                text()?.chars().map(|c| if c.is_ascii() { c.to_string() } else { c.encode_utf16(&mut [0u16; 2]).iter().map(|u| format!("\\u{u:04x}")).collect() }).collect::<String>().into_bytes(),
                it.stem(),
                it.ext.clone(),
            ),
            "unicode-unescape" => {
                let t = text()?;
                let re = Regex::new(r"\\u([0-9a-fA-F]{4})").expect("re");
                let units: Vec<u16> = {
                    let mut v = vec![];
                    let mut last = 0;
                    for m in re.captures_iter(&t) {
                        let all = m.get(0).map(|x| (x.start(), x.end())).unwrap_or((0, 0));
                        v.extend(t[last..all.0].encode_utf16());
                        v.push(u16::from_str_radix(&m[1], 16).unwrap_or(0x3f));
                        last = all.1;
                    }
                    v.extend(t[last..].encode_utf16());
                    v
                };
                (String::from_utf16_lossy(&units).into_bytes(), it.stem(), it.ext.clone())
            }
            other => bail!("unknown operation {other}"),
        };
        let out = ctx.out_path(it, &name, &ext)?;
        std::fs::write(&out, body)?;
        Ok(vec![it.derive(&out)])
    })
}

fn flatten(prefix: &str, v: &Value, sep: &str, out: &mut Map<String, Value>) {
    match v {
        Value::Object(o) if !o.is_empty() => {
            for (k, x) in o {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}{sep}{k}") };
                flatten(&key, x, sep, out);
            }
        }
        Value::Array(a) if !a.is_empty() => {
            for (i, x) in a.iter().enumerate() {
                let key = if prefix.is_empty() { i.to_string() } else { format!("{prefix}{sep}{i}") };
                flatten(&key, x, sep, out);
            }
        }
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

fn unflatten(o: &Map<String, Value>, sep: &str) -> Value {
    let mut root = Value::Object(Map::new());
    for (k, v) in o {
        let mut cur = &mut root;
        let parts: Vec<&str> = k.split(sep).collect();
        for (i, part) in parts.iter().enumerate() {
            let last = i + 1 == parts.len();
            if !cur.is_object() {
                *cur = Value::Object(Map::new());
            }
            let m = cur.as_object_mut().expect("object");
            if last {
                m.insert(part.to_string(), v.clone());
                break;
            }
            cur = m.entry(part.to_string()).or_insert_with(|| Value::Object(Map::new()));
        }
    }
    root
}

fn sort_keys(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let mut m = Map::new();
            for k in keys {
                m.insert(k.clone(), sort_keys(&o[k.as_str()]));
            }
            Value::Object(m)
        }
        Value::Array(a) => Value::Array(a.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

/// `a.b[0].c`, `items[*].name`, `users.*.email`
fn select(v: &Value, path: &str) -> Vec<Value> {
    let mut cur = vec![v.clone()];
    let norm = path.trim().trim_start_matches('$').trim_start_matches('.').replace('[', ".").replace(']', "");
    for seg in norm.split('.').filter(|s| !s.is_empty()) {
        let mut next = vec![];
        for x in &cur {
            match (seg, x) {
                ("*", Value::Array(a)) => next.extend(a.iter().cloned()),
                ("*", Value::Object(o)) => next.extend(o.values().cloned()),
                (s, Value::Array(a)) => {
                    if let Ok(i) = s.parse::<i64>() {
                        let idx = if i < 0 { a.len() as i64 + i } else { i };
                        if let Some(y) = a.get(idx.max(0) as usize) {
                            next.push(y.clone());
                        }
                    } else {
                        next.extend(a.iter().filter_map(|y| y.get(s).cloned()));
                    }
                }
                (s, Value::Object(o)) => {
                    if let Some(y) = o.get(s) {
                        next.push(y.clone());
                    }
                }
                _ => {}
            }
        }
        cur = next;
    }
    cur
}

pub fn json_tools(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let op = p.str("op", "pretty");
    let indent = p.i64("indent", 2).clamp(0, 8) as usize;
    let path = p.str("path", "");
    let sep = p.str("separator", ".");
    let sep = if sep.is_empty() { ".".to_string() } else { sep };
    batch(ctx, files, &["data"], true, |it| {
        let (s, _) = read_text(it)?;
        let s = s.trim_start_matches('\u{feff}');
        let v: Value = match it.ext.as_str() {
            "yaml" | "yml" => serde_yaml::from_str(s).context("invalid YAML")?,
            "toml" => toml::from_str(s).context("invalid TOML")?,
            "json" | "geojson" => serde_json::from_str(s).context("invalid JSON")?,
            _ => return Ok(vec![it.clone()]),
        };
        let result = match op.as_str() {
            "minify" | "pretty" => v,
            "sort_keys" => sort_keys(&v),
            "flatten" => {
                let mut m = Map::new();
                flatten("", &v, &sep, &mut m);
                Value::Object(m)
            }
            "unflatten" => match &v {
                Value::Object(o) => unflatten(o, &sep),
                _ => bail!("unflatten needs an object"),
            },
            "extract" => {
                let found = select(&v, &path);
                if found.len() == 1 && !path.contains('*') {
                    found.into_iter().next().unwrap_or(Value::Null)
                } else {
                    Value::Array(found)
                }
            }
            "keys" => match &v {
                Value::Object(o) => json!(o.keys().collect::<Vec<_>>()),
                Value::Array(a) => json!(a.len()),
                _ => json!([]),
            },
            "schema" => schema(&v),
            other => bail!("unknown operation {other}"),
        };
        let body = if op == "minify" {
            serde_json::to_string(&result)?
        } else if indent == 0 {
            serde_json::to_string(&result)?
        } else {
            use serde::Serialize;
            let ind = " ".repeat(indent);
            let mut buf = Vec::new();
            let f = serde_json::ser::PrettyFormatter::with_indent(ind.as_bytes());
            let mut ser = serde_json::Serializer::with_formatter(&mut buf, f);
            result.serialize(&mut ser)?;
            String::from_utf8(buf)?
        };
        let stem = if op == "extract" || op == "schema" || op == "keys" { format!("{}_{op}", it.stem()) } else { it.stem() };
        let out = ctx.out_path(it, &stem, "json")?;
        std::fs::write(&out, body + "\n")?;
        Ok(vec![it.derive(&out)])
    })
}

fn schema(v: &Value) -> Value {
    match v {
        Value::Null => json!({"type": "null"}),
        Value::Bool(_) => json!({"type": "boolean"}),
        Value::Number(n) => json!({"type": if n.is_i64() || n.is_u64() { "integer" } else { "number" }}),
        Value::String(_) => json!({"type": "string"}),
        Value::Array(a) => json!({"type": "array", "items": a.first().map(schema).unwrap_or(json!({}))}),
        Value::Object(o) => {
            let mut props = Map::new();
            for (k, x) in o {
                props.insert(k.clone(), schema(x));
            }
            json!({"type": "object", "properties": props})
        }
    }
}
