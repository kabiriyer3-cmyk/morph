use super::batch;
use crate::engine::{Ctx, FileItem};
use crate::kinds;
use crate::util::Params;
use anyhow::{anyhow, bail, Context, Result};
use encoding_rs::Encoding;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Number, Value};
use std::collections::HashSet;

/// Decodes bytes. `label` = auto (BOM / UTF-8 / chardetng guess) or an encoding label.
pub fn decode(bytes: &[u8], label: &str) -> Result<(String, &'static Encoding)> {
    let label = label.trim().to_ascii_lowercase();
    if label.is_empty() || label == "auto" {
        if let Some((enc, bom)) = Encoding::for_bom(bytes) {
            let (s, _) = enc.decode_without_bom_handling(&bytes[bom..]);
            return Ok((s.into_owned(), enc));
        }
        if let Ok(s) = std::str::from_utf8(bytes) {
            return Ok((s.to_string(), encoding_rs::UTF_8));
        }
        let mut det = chardetng::EncodingDetector::new();
        det.feed(bytes, true);
        let enc = det.guess(None, true);
        let (s, _, _) = enc.decode(bytes);
        return Ok((s.into_owned(), enc));
    }
    let lbl = label.trim_end_matches("-bom");
    let enc = Encoding::for_label(lbl.as_bytes()).ok_or_else(|| anyhow!("Unknown encoding {label}"))?;
    let (s, _, _) = enc.decode(bytes);
    Ok((s.into_owned(), enc))
}

pub fn encode(s: &str, label: &str, detected: &'static Encoding) -> Result<Vec<u8>> {
    let label: String = if label == "keep" {
        if detected == encoding_rs::UTF_16LE {
            "utf-16le".into()
        } else if detected == encoding_rs::UTF_16BE {
            "utf-16be".into()
        } else {
            detected.name().to_ascii_lowercase()
        }
    } else {
        label.to_ascii_lowercase()
    };
    Ok(match label.as_str() {
        "utf-8" | "utf8" | "" => s.as_bytes().to_vec(),
        "utf-8-bom" => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(s.as_bytes());
            v
        }
        "utf-16le" => {
            let mut v = vec![0xFF, 0xFE];
            for u in s.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            v
        }
        "utf-16be" => {
            let mut v = vec![0xFE, 0xFF];
            for u in s.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            v
        }
        other => {
            let enc = Encoding::for_label(other.as_bytes()).ok_or_else(|| anyhow!("Unknown encoding {other}"))?;
            let (b, _, _) = enc.encode(s);
            b.into_owned()
        }
    })
}

fn infer(s: &str) -> Value {
    let t = s.trim();
    if t.is_empty() {
        return Value::Null;
    }
    match t {
        "true" | "TRUE" | "True" => return Value::Bool(true),
        "false" | "FALSE" | "False" => return Value::Bool(false),
        _ => {}
    }
    let leading_zero = t.len() > 1 && t.starts_with('0') && !t.starts_with("0.");
    if !leading_zero {
        if let Ok(i) = t.parse::<i64>() {
            return Value::Number(i.into());
        }
        if let Ok(f) = t.parse::<f64>() {
            if f.is_finite() {
                if let Some(n) = Number::from_f64(f) {
                    return Value::Number(n);
                }
            }
        }
    }
    Value::String(s.to_string())
}

fn parse_csv(text: &str, delim: u8, infer_types: bool) -> Result<Value> {
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).flexible(true).from_reader(text.as_bytes());
    let headers = rdr.headers()?.clone();
    let mut rows = vec![];
    for rec in rdr.records() {
        let rec = rec?;
        let mut obj = Map::new();
        for (i, h) in headers.iter().enumerate() {
            let cell = rec.get(i).unwrap_or("");
            obj.insert(h.to_string(), if infer_types { infer(cell) } else { Value::String(cell.to_string()) });
        }
        rows.push(Value::Object(obj));
    }
    Ok(Value::Array(rows))
}

fn parse(text: &str, ext: &str, delim: u8, infer_types: bool) -> Result<Value> {
    let text = text.trim_start_matches('\u{feff}');
    Ok(match ext {
        "json" => serde_json::from_str(text).context("invalid JSON")?,
        "yaml" | "yml" => serde_yaml::from_str(text).context("invalid YAML")?,
        "toml" => toml::from_str(text).context("invalid TOML")?,
        "csv" => parse_csv(text, delim, infer_types)?,
        "tsv" => parse_csv(text, b'\t', infer_types)?,
        "ndjson" | "jsonl" => Value::Array(
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .map(serde_json::from_str)
                .collect::<std::result::Result<Vec<Value>, _>>()
                .context("invalid NDJSON line")?,
        ),
        _ => match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => serde_yaml::from_str(text).context("unsupported data format")?,
        },
    })
}

fn rows(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => {
            if o.len() == 1 {
                if let Some(Value::Array(a)) = o.values().next() {
                    return a.clone();
                }
            }
            vec![v.clone()]
        }
        other => vec![other.clone()],
    }
}

fn cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn to_csv(v: &Value, delim: u8) -> Result<String> {
    let rows = rows(v);
    let mut headers: Vec<String> = vec![];
    let mut seen = HashSet::new();
    for r in &rows {
        if let Value::Object(o) = r {
            for k in o.keys() {
                if seen.insert(k.clone()) {
                    headers.push(k.clone());
                }
            }
        }
    }
    let scalar = headers.is_empty();
    if scalar {
        headers.push("value".into());
    }
    let mut w = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    w.write_record(&headers)?;
    for r in &rows {
        let rec: Vec<String> = if scalar {
            vec![cell(r)]
        } else {
            headers.iter().map(|h| r.get(h.as_str()).map(cell).unwrap_or_default()).collect()
        };
        w.write_record(&rec)?;
    }
    let bytes = w.into_inner().map_err(|e| anyhow!("csv: {e}"))?;
    Ok(String::from_utf8(bytes)?)
}

fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, strip_nulls(v))).collect()),
        Value::Array(a) => Value::Array(a.into_iter().filter(|v| !v.is_null()).map(strip_nulls).collect()),
        other => other,
    }
}

fn render(v: &Value, fmt: &str, pretty: bool, indent: usize, delim: u8) -> Result<(String, &'static str)> {
    Ok(match fmt {
        "yaml" => (serde_yaml::to_string(v)?, "yaml"),
        "toml" => {
            let mut v2 = strip_nulls(v.clone());
            if !v2.is_object() {
                v2 = json!({ "items": v2 });
            }
            (toml::to_string_pretty(&v2)?, "toml")
        }
        "csv" => (to_csv(v, delim)?, "csv"),
        "tsv" => (to_csv(v, b'\t')?, "tsv"),
        "ndjson" => {
            let lines = rows(v).iter().map(serde_json::to_string).collect::<std::result::Result<Vec<_>, _>>()?;
            (lines.join("\n") + "\n", "ndjson")
        }
        _ => {
            let s = if pretty && indent > 0 {
                let ind = " ".repeat(indent.min(8));
                let mut buf = Vec::new();
                let fmtr = serde_json::ser::PrettyFormatter::with_indent(ind.as_bytes());
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmtr);
                v.serialize(&mut ser)?;
                String::from_utf8(buf)?
            } else {
                serde_json::to_string(v)?
            };
            (s + "\n", "json")
        }
    })
}

fn delim_of(s: &str) -> u8 {
    match s {
        "tab" | "\\t" => b'\t',
        ";" => b';',
        "|" => b'|',
        _ => s.bytes().next().unwrap_or(b','),
    }
}

pub fn convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "json");
    let pretty = p.bool("pretty", true);
    let indent = p.i64("indent", 2).clamp(0, 8) as usize;
    let delim = delim_of(&p.str("delimiter", ","));
    let infer_types = p.bool("infer_types", true);
    batch(ctx, files, &["data"], true, |it| {
        let bytes = std::fs::read(&it.path)?;
        let (text, _) = decode(&bytes, "auto")?;
        let value = parse(&text, &it.ext, delim, infer_types)?;
        let (body, ext) = render(&value, &fmt, pretty, indent, delim)?;
        let out = ctx.out_path(it, &it.stem(), ext)?;
        std::fs::write(&out, body)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn text_transform(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let enc_in = p.str("encoding_in", "auto");
    let enc_out = p.str("encoding_out", "utf-8");
    let endings = p.str("line_endings", "keep");
    let trim = p.bool("trim_trailing", false);
    let find = p.str("find", "");
    let replace = p.str("replace", "");
    let use_re = p.bool("regex", false);
    let case = p.str("case", "keep");
    let tabs = p.i64("tabs_to_spaces", 0).clamp(0, 16) as usize;
    let re = if find.is_empty() {
        None
    } else if use_re {
        Some(Regex::new(&find).context("invalid regular expression")?)
    } else {
        Some(Regex::new(&regex::escape(&find))?)
    };
    batch(ctx, files, &[], true, |it| {
        if !kinds::is_textual(&it.kind, &it.ext) {
            return Ok(vec![it.clone()]);
        }
        let bytes = std::fs::read(&it.path)?;
        let (mut s, detected) = decode(&bytes, &enc_in)?;
        if tabs > 0 {
            s = s.replace('\t', &" ".repeat(tabs));
        }
        if trim {
            s = s
                .split_inclusive('\n')
                .map(|line| {
                    let (body, end) = if let Some(b) = line.strip_suffix("\r\n") {
                        (b, "\r\n")
                    } else if let Some(b) = line.strip_suffix('\n') {
                        (b, "\n")
                    } else {
                        (line, "")
                    };
                    format!("{}{}", body.trim_end(), end)
                })
                .collect();
        }
        if let Some(r) = &re {
            s = if use_re {
                r.replace_all(&s, replace.as_str()).into_owned()
            } else {
                r.replace_all(&s, regex::NoExpand(&replace)).into_owned()
            };
        }
        match case.as_str() {
            "lower" => s = s.to_lowercase(),
            "upper" => s = s.to_uppercase(),
            _ => {}
        }
        match endings.as_str() {
            "lf" => s = s.replace("\r\n", "\n"),
            "crlf" => s = s.replace("\r\n", "\n").replace('\n', "\r\n"),
            _ => {}
        }
        let out_bytes = encode(&s, &enc_out, detected)?;
        let out = ctx.out_path(it, &it.stem(), &it.ext)?;
        std::fs::write(&out, out_bytes)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn ensure_text(kind: &str) -> Result<()> {
    if kind == "text" {
        Ok(())
    } else {
        bail!("not a text file")
    }
}
