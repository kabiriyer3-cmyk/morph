//! Spreadsheets and tables. Reads XLSX/XLSM/XLSB/XLS/ODS natively (calamine) plus CSV/TSV/JSON/NDJSON/YAML,
//! writes XLSX natively (rust_xlsxwriter) and CSV/TSV/JSON/NDJSON/YAML/HTML/Markdown/XML/SQL/text;
//! ODS, XLS and PDF output go through LibreOffice when it is installed.
use super::batch;
use crate::engine::{Ctx, FileItem};
use crate::ops::data::{decode, encode};
use crate::util::{sanitize, Params};
use anyhow::{anyhow, bail, Context, Result};
use calamine::{open_workbook_auto, Data, Reader};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const BOOK: &[&str] = &["xlsx", "xlsm", "xlsb", "xls", "xla", "xlam", "ods"];
pub const PLAIN: &[&str] = &["csv", "tsv", "tab", "json", "ndjson", "jsonl", "yaml", "yml"];
pub const OUT: &[&str] = &["xlsx", "csv", "tsv", "json", "ndjson", "yaml", "html", "md", "xml", "sql", "txt", "ods", "xls", "pdf"];

pub fn is_sheet(ext: &str) -> bool {
    BOOK.contains(&ext) || PLAIN.contains(&ext)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Empty,
    Str(String),
    Int(i64),
    Num(f64),
    Bool(bool),
}

fn fmt_num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{}", f)
    }
}

impl Cell {
    pub fn text(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Str(s) => s.clone(),
            Cell::Int(i) => i.to_string(),
            Cell::Num(f) => fmt_num(*f),
            Cell::Bool(b) => b.to_string(),
        }
    }
    pub fn json(&self) -> Value {
        match self {
            Cell::Empty => Value::Null,
            Cell::Str(s) => Value::String(s.clone()),
            Cell::Int(i) => Value::from(*i),
            Cell::Num(f) => serde_json::Number::from_f64(*f).map(Value::Number).unwrap_or(Value::Null),
            Cell::Bool(b) => Value::Bool(*b),
        }
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Cell::Empty => true,
            Cell::Str(s) => s.trim().is_empty(),
            _ => false,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            Cell::Int(i) => Some(*i as f64),
            Cell::Num(f) => Some(*f),
            Cell::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Cell::Str(s) => s.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite()),
            Cell::Empty => None,
        }
    }
}

/// Guesses a typed cell from text (keeps leading-zero codes and phone numbers as text).
pub fn infer(s: &str) -> Cell {
    let t = s.trim();
    if t.is_empty() {
        return Cell::Empty;
    }
    let digits = t.trim_start_matches('-');
    let leading_zero = digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0.");
    if !leading_zero && !t.starts_with('+') {
        if let Ok(i) = t.parse::<i64>() {
            return Cell::Int(i);
        }
        if t.bytes().any(|b| b.is_ascii_digit()) && t.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'e' | b'E' | b'+')) {
            if let Ok(f) = t.parse::<f64>() {
                if f.is_finite() {
                    return Cell::Num(f);
                }
            }
        }
    }
    match t.to_ascii_lowercase().as_str() {
        "true" => Cell::Bool(true),
        "false" => Cell::Bool(false),
        _ => Cell::Str(s.to_string()),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub name: String,
    pub rows: Vec<Vec<Cell>>,
}

impl Table {
    pub fn width(&self) -> usize {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0)
    }
    pub fn pad(&mut self) {
        let w = self.width();
        for r in &mut self.rows {
            r.resize(w, Cell::Empty);
        }
    }
}

fn excel_date(v: f64, fmt: &str) -> String {
    let Some(base) = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).and_then(|d| d.and_hms_opt(0, 0, 0)) else {
        return fmt_num(v);
    };
    let dt = base + chrono::Duration::milliseconds((v * 86_400_000.0).round() as i64);
    let secs = ((v.fract().abs() * 86_400.0).round() as i64) % 86_400;
    let d = match fmt {
        "dmy" => "%d.%m.%Y",
        "mdy" => "%m/%d/%Y",
        _ => "%Y-%m-%d",
    };
    if v.abs() < 1.0 {
        dt.format("%H:%M:%S").to_string()
    } else if secs == 0 {
        dt.format(d).to_string()
    } else {
        dt.format(&format!("{d} %H:%M:%S")).to_string()
    }
}

fn from_data(d: &Data, date_fmt: &str) -> Cell {
    match d {
        Data::Empty => Cell::Empty,
        Data::String(s) => {
            if s.is_empty() {
                Cell::Empty
            } else {
                Cell::Str(s.clone())
            }
        }
        Data::Int(i) => Cell::Int(*i),
        Data::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 9.0e15 {
                Cell::Int(*f as i64)
            } else {
                Cell::Num(*f)
            }
        }
        Data::Bool(b) => Cell::Bool(*b),
        Data::DateTime(dt) => {
            let v = dt.as_f64();
            if date_fmt == "serial" {
                Cell::Num(v)
            } else {
                Cell::Str(excel_date(v, date_fmt))
            }
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Str(s.clone()),
        Data::Error(e) => Cell::Str(format!("#{e:?}")),
    }
}

fn json_cell(v: &Value) -> Cell {
    match v {
        Value::Null => Cell::Empty,
        Value::Bool(b) => Cell::Bool(*b),
        Value::Number(n) => n.as_i64().map(Cell::Int).unwrap_or_else(|| Cell::Num(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => Cell::Str(s.clone()),
        other => Cell::Str(other.to_string()),
    }
}

fn flatten_into(prefix: &str, v: &Value, out: &mut Map<String, Value>) {
    match v {
        Value::Object(o) if !o.is_empty() => {
            for (k, x) in o {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten_into(&key, x, out);
            }
        }
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

fn value_rows(v: &Value, flatten: bool) -> Vec<Vec<Cell>> {
    let items: Vec<Value> = match v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => {
            if o.len() == 1 && o.values().next().map(|x| x.is_array()).unwrap_or(false) {
                o.values().next().and_then(|x| x.as_array()).cloned().unwrap_or_default()
            } else if !o.is_empty() && o.values().all(|x| x.is_array()) {
                let keys: Vec<&String> = o.keys().collect();
                let n = o.values().filter_map(|x| x.as_array()).map(|a| a.len()).max().unwrap_or(0);
                let mut rows = vec![keys.iter().map(|k| Cell::Str((*k).clone())).collect::<Vec<_>>()];
                for i in 0..n {
                    rows.push(keys.iter().map(|k| o[k.as_str()].get(i).map(json_cell).unwrap_or(Cell::Empty)).collect());
                }
                return rows;
            } else {
                vec![v.clone()]
            }
        }
        other => vec![other.clone()],
    };
    if !items.is_empty() && items.iter().all(|x| x.is_array()) {
        return items.iter().map(|r| r.as_array().map(|a| a.iter().map(json_cell).collect()).unwrap_or_default()).collect();
    }
    let items: Vec<Value> = if flatten {
        items
            .into_iter()
            .map(|x| {
                if x.is_object() {
                    let mut m = Map::new();
                    flatten_into("", &x, &mut m);
                    Value::Object(m)
                } else {
                    x
                }
            })
            .collect()
    } else {
        items
    };
    let mut keys: Vec<String> = vec![];
    let mut seen = HashSet::new();
    for it in &items {
        if let Value::Object(o) = it {
            for k in o.keys() {
                if seen.insert(k.clone()) {
                    keys.push(k.clone());
                }
            }
        }
    }
    if keys.is_empty() {
        let mut rows = vec![vec![Cell::Str("value".into())]];
        rows.extend(items.iter().map(|x| vec![json_cell(x)]));
        return rows;
    }
    let mut rows = vec![keys.iter().map(|k| Cell::Str(k.clone())).collect::<Vec<_>>()];
    for it in &items {
        rows.push(keys.iter().map(|k| it.get(k.as_str()).map(json_cell).unwrap_or(Cell::Empty)).collect());
    }
    rows
}

fn sniff(text: &str) -> u8 {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut best = (b',', 0usize);
    for d in [b',', b';', b'\t', b'|'] {
        let n = line.bytes().filter(|b| *b == d).count();
        if n > best.1 {
            best = (d, n);
        }
    }
    best.0
}

pub fn delim_of(s: &str) -> u8 {
    match s {
        "tab" | "\\t" => b'\t',
        ";" => b';',
        "|" => b'|',
        "space" => b' ',
        _ => s.bytes().next().unwrap_or(b','),
    }
}

fn read_csv(text: &str, delim: u8, infer_types: bool) -> Result<Vec<Vec<Cell>>> {
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(text.as_bytes());
    let mut rows = vec![];
    for rec in rdr.records() {
        let rec = rec?;
        rows.push(
            rec.iter()
                .map(|c| {
                    if infer_types {
                        infer(c)
                    } else if c.is_empty() {
                        Cell::Empty
                    } else {
                        Cell::Str(c.to_string())
                    }
                })
                .collect(),
        );
    }
    Ok(rows)
}

#[derive(Clone, Debug)]
pub struct ReadOpts {
    pub sheets: String,
    pub sheet_name: String,
    pub skip_rows: usize,
    pub delimiter: String,
    pub encoding: String,
    pub date_format: String,
    pub trim: bool,
    pub drop_empty: bool,
    pub infer: bool,
    pub flatten: bool,
}

impl ReadOpts {
    pub fn from(p: &Params) -> Self {
        ReadOpts {
            sheets: p.str("sheets", "all"),
            sheet_name: p.str("sheet_name", ""),
            skip_rows: p.i64("skip_rows", 0).max(0) as usize,
            delimiter: p.str("input_delimiter", "auto"),
            encoding: p.str("input_encoding", "auto"),
            date_format: p.str("date_format", "iso"),
            trim: p.bool("trim", true),
            drop_empty: p.bool("drop_empty", true),
            infer: p.bool("infer_types", true),
            flatten: p.bool("flatten", true),
        }
    }
    pub fn preview() -> Self {
        ReadOpts {
            sheets: "first".into(),
            sheet_name: String::new(),
            skip_rows: 0,
            delimiter: "auto".into(),
            encoding: "auto".into(),
            date_format: "iso".into(),
            trim: false,
            drop_empty: false,
            infer: false,
            flatten: true,
        }
    }
}

fn pick_sheets(names: &[String], o: &ReadOpts) -> Vec<String> {
    match o.sheets.as_str() {
        "first" => names.iter().take(1).cloned().collect(),
        "name" => {
            let want = o.sheet_name.trim();
            if let Some(n) = names.iter().find(|n| n.trim().to_lowercase() == want.to_lowercase()) {
                return vec![n.clone()];
            }
            let mut v = vec![];
            for tok in want.split([',', ';']) {
                let tok = tok.trim();
                if tok.is_empty() {
                    continue;
                }
                if let Some(n) = names.iter().find(|n| n.trim().to_lowercase() == tok.to_lowercase()) {
                    v.push(n.clone());
                } else if let Ok(i) = tok.parse::<usize>() {
                    if i >= 1 && i <= names.len() {
                        v.push(names[i - 1].clone());
                    }
                }
            }
            v
        }
        _ => names.to_vec(),
    }
}

fn read_book(path: &Path, o: &ReadOpts) -> Result<Vec<Table>> {
    let mut wb = open_workbook_auto(path).map_err(|e| anyhow!("cannot open the workbook: {e}"))?;
    let names: Vec<String> = wb.sheet_names().to_vec();
    let wanted = pick_sheets(&names, o);
    if wanted.is_empty() {
        bail!("sheet \"{}\" not found (sheets: {})", o.sheet_name, names.join(", "));
    }
    let mut out = vec![];
    for n in wanted {
        let range = wb.worksheet_range(&n).map_err(|e| anyhow!("sheet {n}: {e}"))?;
        let rows: Vec<Vec<Cell>> = range.rows().map(|r| r.iter().map(|d| from_data(d, &o.date_format)).collect()).collect();
        out.push(Table { name: n, rows });
    }
    Ok(out)
}

/// Reads every table (sheet) of a spreadsheet or data file.
pub fn read_tables(path: &Path, ext: &str, o: &ReadOpts) -> Result<Vec<Table>> {
    let stem = crate::util::file_stem(path);
    let mut tables = if BOOK.contains(&ext) {
        read_book(path, o)?
    } else {
        let bytes = std::fs::read(path)?;
        let (text, _) = decode(&bytes, &o.encoding)?;
        let text = text.trim_start_matches('\u{feff}');
        let rows = match ext {
            "json" | "geojson" => value_rows(&serde_json::from_str::<Value>(text).context("invalid JSON")?, o.flatten),
            "yaml" | "yml" => value_rows(&serde_yaml::from_str::<Value>(text).context("invalid YAML")?, o.flatten),
            "ndjson" | "jsonl" => {
                let items = text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(serde_json::from_str::<Value>)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("invalid NDJSON")?;
                value_rows(&Value::Array(items), o.flatten)
            }
            _ => {
                let d = match o.delimiter.as_str() {
                    "auto" | "" => {
                        if ext == "tsv" || ext == "tab" {
                            b'\t'
                        } else {
                            sniff(text)
                        }
                    }
                    other => delim_of(other),
                };
                read_csv(text, d, o.infer)?
            }
        };
        vec![Table { name: stem, rows }]
    };
    for t in &mut tables {
        if o.skip_rows > 0 {
            let n = o.skip_rows.min(t.rows.len());
            t.rows.drain(..n);
        }
        if o.trim {
            for r in &mut t.rows {
                for c in r.iter_mut() {
                    if let Cell::Str(s) = c {
                        let tr = s.trim();
                        *c = if tr.is_empty() { Cell::Empty } else { Cell::Str(tr.to_string()) };
                    }
                }
            }
        }
        if o.drop_empty {
            t.rows.retain(|r| r.iter().any(|c| !c.is_empty()));
        }
        t.pad();
    }
    Ok(tables)
}

#[derive(Clone, Debug)]
pub struct WriteOpts {
    pub header: bool,
    pub delimiter: u8,
    pub encoding: String,
    pub json_shape: String,
    pub pretty: bool,
    pub bold_header: bool,
    pub freeze: bool,
    pub autofilter: bool,
    pub autofit: bool,
    pub zebra: bool,
    pub sql_table: String,
    pub multi: String,
}

impl WriteOpts {
    pub fn from(p: &Params) -> Self {
        WriteOpts {
            header: p.bool("header", true),
            delimiter: delim_of(&p.str("delimiter", ",")),
            encoding: p.str("encoding", "utf-8"),
            json_shape: p.str("json_shape", "objects"),
            pretty: p.bool("pretty", true),
            bold_header: p.bool("bold_header", true),
            freeze: p.bool("freeze_header", true),
            autofilter: p.bool("autofilter", true),
            autofit: p.bool("autofit", true),
            zebra: p.bool("zebra", false),
            sql_table: p.str("sql_table", ""),
            multi: p.str("multi", "separate"),
        }
    }
}

fn col_letter(mut i: usize) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (i % 26) as u8) as char);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s
}

fn letters_index(s: &str) -> Option<usize> {
    let mut n = 0usize;
    for c in s.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        n = n * 26 + (c.to_ascii_uppercase() as u8 - b'A' + 1) as usize;
    }
    if n == 0 {
        None
    } else {
        Some(n - 1)
    }
}

fn header_names(t: &Table, header: bool) -> Vec<String> {
    let w = t.width();
    if !header || t.rows.is_empty() {
        return (0..w).map(col_letter).collect();
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    (0..w)
        .map(|i| {
            let raw = t.rows[0].get(i).map(Cell::text).unwrap_or_default();
            let base = if raw.trim().is_empty() { format!("column_{}", i + 1) } else { raw.trim().to_string() };
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                format!("{base}_{n}")
            } else {
                base
            }
        })
        .collect()
}

fn body(t: &Table, header: bool) -> &[Vec<Cell>] {
    if header && !t.rows.is_empty() {
        &t.rows[1..]
    } else {
        &t.rows[..]
    }
}

fn table_value(t: &Table, o: &WriteOpts) -> Value {
    let heads = header_names(t, o.header);
    let rows = body(t, o.header);
    match o.json_shape.as_str() {
        "arrays" => Value::Array(t.rows.iter().map(|r| Value::Array(r.iter().map(Cell::json).collect())).collect()),
        "columns" => {
            let mut m = Map::new();
            for (i, h) in heads.iter().enumerate() {
                m.insert(h.clone(), Value::Array(rows.iter().map(|r| r.get(i).map(Cell::json).unwrap_or(Value::Null)).collect()));
            }
            Value::Object(m)
        }
        "keyed" => {
            let mut m = Map::new();
            for r in rows {
                let key = r.first().map(Cell::text).unwrap_or_default();
                let mut obj = Map::new();
                for (i, h) in heads.iter().enumerate().skip(1) {
                    obj.insert(h.clone(), r.get(i).map(Cell::json).unwrap_or(Value::Null));
                }
                m.insert(key, Value::Object(obj));
            }
            Value::Object(m)
        }
        _ => Value::Array(
            rows.iter()
                .map(|r| {
                    let mut m = Map::new();
                    for (i, h) in heads.iter().enumerate() {
                        m.insert(h.clone(), r.get(i).map(Cell::json).unwrap_or(Value::Null));
                    }
                    Value::Object(m)
                })
                .collect(),
        ),
    }
}

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xml_name(s: &str) -> String {
    let mut n: String = s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' { c } else { '_' }).collect();
    if n.is_empty() || !n.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
        n.insert(0, '_');
    }
    n
}

fn sql_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn render_text(ts: &[Table], fmt: &str, o: &WriteOpts, title: &str) -> Result<(String, &'static str)> {
    Ok(match fmt {
        "csv" | "tsv" => {
            let d = if fmt == "tsv" { b'\t' } else { o.delimiter };
            let mut w = csv::WriterBuilder::new().delimiter(d).from_writer(vec![]);
            for t in ts {
                for r in &t.rows {
                    w.write_record(r.iter().map(Cell::text))?;
                }
            }
            let bytes = w.into_inner().map_err(|e| anyhow!("csv: {e}"))?;
            (String::from_utf8(bytes)?, if fmt == "tsv" { "tsv" } else { "csv" })
        }
        "json" | "yaml" => {
            let v = if ts.len() == 1 {
                table_value(&ts[0], o)
            } else {
                let mut m = Map::new();
                for t in ts {
                    m.insert(t.name.clone(), table_value(t, o));
                }
                Value::Object(m)
            };
            if fmt == "yaml" {
                (serde_yaml::to_string(&v)?, "yaml")
            } else if o.pretty {
                (serde_json::to_string_pretty(&v)? + "\n", "json")
            } else {
                (serde_json::to_string(&v)? + "\n", "json")
            }
        }
        "ndjson" => {
            let mut s = String::new();
            for t in ts {
                let mut o2 = o.clone();
                o2.json_shape = "objects".into();
                if let Value::Array(a) = table_value(t, &o2) {
                    for x in a {
                        s.push_str(&serde_json::to_string(&x)?);
                        s.push('\n');
                    }
                }
            }
            (s, "ndjson")
        }
        "html" => {
            let mut s = format!(
                "<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>body{{font:14px/1.45 system-ui,'Segoe UI',Roboto,sans-serif;margin:28px;color:#1b1f24;background:#fff}}h2{{font-size:15px;margin:26px 0 10px;color:#3d4450}}table{{border-collapse:collapse;margin:0 0 24px;font-variant-numeric:tabular-nums}}th,td{{border:1px solid #dfe3ea;padding:6px 10px;text-align:left;vertical-align:top}}th{{background:#f1f4f9;font-weight:600;position:sticky;top:0}}tr:nth-child(even) td{{background:#fafbfd}}td.n{{text-align:right}}</style></head><body>\n",
                esc_html(title)
            );
            for t in ts {
                if ts.len() > 1 {
                    s.push_str(&format!("<h2>{}</h2>\n", esc_html(&t.name)));
                }
                s.push_str("<table>\n");
                for (i, r) in t.rows.iter().enumerate() {
                    let head = o.header && i == 0;
                    s.push_str("<tr>");
                    for c in r {
                        let num = matches!(c, Cell::Int(_) | Cell::Num(_));
                        if head {
                            s.push_str(&format!("<th>{}</th>", esc_html(&c.text())));
                        } else if num {
                            s.push_str(&format!("<td class=\"n\">{}</td>", esc_html(&c.text())));
                        } else {
                            s.push_str(&format!("<td>{}</td>", esc_html(&c.text()).replace('\n', "<br>")));
                        }
                    }
                    s.push_str("</tr>\n");
                }
                s.push_str("</table>\n");
            }
            s.push_str("</body></html>\n");
            (s, "html")
        }
        "md" => {
            let mut s = String::new();
            for t in ts {
                if ts.len() > 1 {
                    s.push_str(&format!("## {}\n\n", t.name));
                }
                let heads = header_names(t, o.header);
                let cellmd = |c: &str| c.replace('|', "\\|").replace('\n', "<br>");
                s.push_str(&format!("| {} |\n", heads.iter().map(|h| cellmd(h)).collect::<Vec<_>>().join(" | ")));
                s.push_str(&format!("|{}|\n", heads.iter().map(|_| " --- ").collect::<Vec<_>>().join("|")));
                for r in body(t, o.header) {
                    s.push_str(&format!("| {} |\n", r.iter().map(|c| cellmd(&c.text())).collect::<Vec<_>>().join(" | ")));
                }
                s.push('\n');
            }
            (s, "md")
        }
        "xml" => {
            let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<workbook>\n");
            for t in ts {
                let heads: Vec<String> = header_names(t, o.header).iter().map(|h| xml_name(h)).collect();
                s.push_str(&format!("  <sheet name=\"{}\">\n", esc_html(&t.name)));
                for r in body(t, o.header) {
                    s.push_str("    <row>");
                    for (i, c) in r.iter().enumerate() {
                        let h = heads.get(i).cloned().unwrap_or_else(|| format!("c{}", i + 1));
                        s.push_str(&format!("<{h}>{}</{h}>", esc_html(&c.text())));
                    }
                    s.push_str("</row>\n");
                }
                s.push_str("  </sheet>\n");
            }
            s.push_str("</workbook>\n");
            (s, "xml")
        }
        "sql" => {
            let mut s = String::new();
            for t in ts {
                let name = if !o.sql_table.trim().is_empty() && ts.len() == 1 { o.sql_table.trim().to_string() } else { t.name.clone() };
                let heads = header_names(t, o.header);
                let rows = body(t, o.header);
                let types: Vec<&str> = (0..heads.len())
                    .map(|i| {
                        let vals: Vec<&Cell> = rows.iter().filter_map(|r| r.get(i)).filter(|c| !c.is_empty()).collect();
                        if !vals.is_empty() && vals.iter().all(|c| matches!(c, Cell::Int(_) | Cell::Bool(_))) {
                            "INTEGER"
                        } else if !vals.is_empty() && vals.iter().all(|c| matches!(c, Cell::Int(_) | Cell::Num(_))) {
                            "REAL"
                        } else {
                            "TEXT"
                        }
                    })
                    .collect();
                let cols: Vec<String> = heads.iter().zip(&types).map(|(h, ty)| format!("  {} {}", sql_ident(h), ty)).collect();
                s.push_str(&format!("CREATE TABLE IF NOT EXISTS {} (\n{}\n);\n", sql_ident(&name), cols.join(",\n")));
                let names = heads.iter().map(|h| sql_ident(h)).collect::<Vec<_>>().join(", ");
                for chunk in rows.chunks(500) {
                    s.push_str(&format!("INSERT INTO {} ({}) VALUES\n", sql_ident(&name), names));
                    let lines: Vec<String> = chunk
                        .iter()
                        .map(|r| {
                            let vals: Vec<String> = r
                                .iter()
                                .map(|c| match c {
                                    Cell::Empty => "NULL".to_string(),
                                    Cell::Int(i) => i.to_string(),
                                    Cell::Num(f) => fmt_num(*f),
                                    Cell::Bool(b) => (if *b { "1" } else { "0" }).to_string(),
                                    Cell::Str(x) => format!("'{}'", x.replace('\'', "''")),
                                })
                                .collect();
                            format!("  ({})", vals.join(", "))
                        })
                        .collect();
                    s.push_str(&lines.join(",\n"));
                    s.push_str(";\n");
                }
                s.push('\n');
            }
            (s, "sql")
        }
        _ => {
            let mut s = String::new();
            for t in ts {
                if ts.len() > 1 {
                    s.push_str(&format!("== {} ==\n", t.name));
                }
                let w = t.width();
                let widths: Vec<usize> = (0..w)
                    .map(|i| t.rows.iter().map(|r| r.get(i).map(|c| c.text().chars().count()).unwrap_or(0)).max().unwrap_or(0).min(60))
                    .collect();
                for (ri, r) in t.rows.iter().enumerate() {
                    let cells: Vec<String> = (0..w)
                        .map(|i| {
                            let tx: String = r.get(i).map(Cell::text).unwrap_or_default().replace('\n', " ").chars().take(60).collect();
                            let pad = widths[i].saturating_sub(tx.chars().count());
                            format!("{tx}{}", " ".repeat(pad))
                        })
                        .collect();
                    s.push_str(cells.join(" | ").trim_end());
                    s.push('\n');
                    if ri == 0 && o.header {
                        s.push_str(&widths.iter().map(|n| "-".repeat(*n)).collect::<Vec<_>>().join("-+-"));
                        s.push('\n');
                    }
                }
                s.push('\n');
            }
            (s, "txt")
        }
    })
}

fn unique_sheet(name: &str, used: &mut HashSet<String>) -> String {
    let clean: String = name.chars().filter(|c| !"[]:*?/\\".contains(*c)).collect();
    let clean = clean.trim().trim_matches('\'').to_string();
    let base: String = if clean.is_empty() { "Sheet".to_string() } else { clean.chars().take(31).collect() };
    let mut n = base.clone();
    let mut i = 2;
    while used.contains(&n.to_lowercase()) {
        let suffix = format!(" ({i})");
        n = format!("{}{}", base.chars().take(31 - suffix.len()).collect::<String>(), suffix);
        i += 1;
    }
    used.insert(n.to_lowercase());
    n
}

fn write_xlsx(path: &Path, ts: &[Table], o: &WriteOpts) -> Result<()> {
    use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook};
    let mut wb = Workbook::new();
    let head = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE8EEF9))
        .set_border_bottom(FormatBorder::Thin);
    let band = Format::new().set_background_color(Color::RGB(0xF6F8FC));
    let mut used = HashSet::new();
    for t in ts {
        let ws = wb.add_worksheet();
        ws.set_name(unique_sheet(&t.name, &mut used))?;
        let w = t.width().min(16_384);
        for (r, row) in t.rows.iter().enumerate().take(1_048_576) {
            let r32 = r as u32;
            let is_head = o.header && r == 0;
            let zebra = o.zebra && !is_head && r % 2 == 0;
            for (c, cell) in row.iter().enumerate().take(16_384) {
                let c16 = c as u16;
                if is_head && o.bold_header {
                    ws.write_string_with_format(r32, c16, cell.text(), &head)?;
                    continue;
                }
                match cell {
                    Cell::Empty => {
                        if zebra {
                            ws.write_blank(r32, c16, &band)?;
                        }
                    }
                    Cell::Str(s) => {
                        let s: String = s.chars().take(32_767).collect();
                        if zebra {
                            ws.write_string_with_format(r32, c16, s, &band)?;
                        } else {
                            ws.write_string(r32, c16, s)?;
                        }
                    }
                    Cell::Int(i) => {
                        if zebra {
                            ws.write_number_with_format(r32, c16, *i as f64, &band)?;
                        } else {
                            ws.write_number(r32, c16, *i as f64)?;
                        }
                    }
                    Cell::Num(f) => {
                        if f.is_finite() {
                            if zebra {
                                ws.write_number_with_format(r32, c16, *f, &band)?;
                            } else {
                                ws.write_number(r32, c16, *f)?;
                            }
                        }
                    }
                    Cell::Bool(b) => {
                        if zebra {
                            ws.write_boolean_with_format(r32, c16, *b, &band)?;
                        } else {
                            ws.write_boolean(r32, c16, *b)?;
                        }
                    }
                }
            }
        }
        if o.header && t.rows.len() > 1 && w > 0 {
            if o.freeze {
                ws.set_freeze_panes(1, 0)?;
            }
            if o.autofilter {
                ws.autofilter(0, 0, (t.rows.len().min(1_048_576) - 1) as u32, (w - 1) as u16)?;
            }
        }
        if o.autofit {
            ws.autofit();
        }
    }
    wb.save(path)?;
    Ok(())
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    if std::fs::rename(from, to).is_err() {
        std::fs::copy(from, to)?;
        let _ = std::fs::remove_file(from);
    }
    Ok(())
}

/// Writes `ts` as one file `<dir>/<stem>.<fmt>`.
pub fn write_file(ctx: &Ctx, dir: &Path, stem: &str, ts: &[Table], fmt: &str, o: &WriteOpts) -> Result<PathBuf> {
    match fmt {
        "xlsx" => {
            let out = ctx.reserve(dir, stem, "xlsx")?;
            write_xlsx(&out, ts, o)?;
            Ok(out)
        }
        "ods" | "xls" | "pdf" => {
            let tmp_dir = ctx.reserve(&ctx.work, "_xlsx", "")?;
            std::fs::create_dir_all(&tmp_dir)?;
            let tmp = tmp_dir.join(format!("{}.xlsx", sanitize(stem)));
            write_xlsx(&tmp, ts, o)?;
            let produced = crate::ops::docs::soffice_raw(ctx, &tmp, fmt)?;
            let out = ctx.reserve(dir, stem, fmt)?;
            move_file(&produced, &out)?;
            let _ = std::fs::remove_dir_all(&tmp_dir);
            Ok(out)
        }
        _ => {
            let (body, ext) = render_text(ts, fmt, o, stem)?;
            let bytes = if matches!(ext, "csv" | "tsv" | "txt") { encode(&body, &o.encoding, encoding_rs::UTF_8)? } else { body.into_bytes() };
            let out = ctx.reserve(dir, stem, ext)?;
            std::fs::write(&out, bytes)?;
            Ok(out)
        }
    }
}

fn merge_tables(tables: Vec<Table>, header: bool) -> Table {
    let mut it = tables.into_iter();
    let Some(mut first) = it.next() else { return Table::default() };
    if !header {
        for t in it {
            first.rows.extend(t.rows);
        }
        first.pad();
        return first;
    }
    let mut heads: Vec<String> = header_names(&first, true);
    let mut rest: Vec<Table> = it.collect();
    for t in &rest {
        for h in header_names(t, true) {
            if !heads.iter().any(|x| x.eq_ignore_ascii_case(&h)) {
                heads.push(h);
            }
        }
    }
    let mut rows: Vec<Vec<Cell>> = vec![heads.iter().map(|h| Cell::Str(h.clone())).collect()];
    let mut push = |t: &Table| {
        let th = header_names(t, true);
        let map: Vec<Option<usize>> = heads.iter().map(|h| th.iter().position(|x| x.eq_ignore_ascii_case(h))).collect();
        for r in body(t, true) {
            rows.push(map.iter().map(|m| m.and_then(|i| r.get(i).cloned()).unwrap_or(Cell::Empty)).collect());
        }
    };
    push(&first);
    for t in rest.drain(..) {
        push(&t);
    }
    Table { name: first.name, rows }
}

fn side_by_side(tables: Vec<Table>) -> Table {
    let n = tables.iter().map(|t| t.rows.len()).max().unwrap_or(0);
    let mut rows: Vec<Vec<Cell>> = vec![vec![]; n];
    let name = tables.first().map(|t| t.name.clone()).unwrap_or_default();
    for t in tables {
        let w = t.width();
        for (i, r) in rows.iter_mut().enumerate() {
            let mut src = t.rows.get(i).cloned().unwrap_or_default();
            src.resize(w, Cell::Empty);
            r.extend(src);
        }
    }
    Table { name, rows }
}

/// Writes the tables of one input, honouring `multi` for formats that hold a single table.
pub fn write_out(ctx: &Ctx, it: &FileItem, mut tables: Vec<Table>, fmt: &str, o: &WriteOpts) -> Result<Vec<FileItem>> {
    if tables.is_empty() {
        bail!("no tables found");
    }
    match o.multi.as_str() {
        "first" => tables.truncate(1),
        "merge" => tables = vec![merge_tables(tables, o.header)],
        _ => {}
    }
    let multi_ok = matches!(fmt, "xlsx" | "ods" | "xls" | "pdf" | "html" | "md" | "txt" | "sql" | "xml" | "json" | "yaml");
    let dir = ctx.out_dir(&it.rel)?;
    let mut out = vec![];
    if tables.len() > 1 && !multi_ok {
        for t in tables {
            let stem = format!("{} - {}", it.stem(), t.name);
            let path = write_file(ctx, &dir, &stem, std::slice::from_ref(&t), fmt, o)?;
            out.push(it.derive(&path));
        }
    } else {
        let path = write_file(ctx, &dir, &it.stem(), &tables, fmt, o)?;
        out.push(it.derive(&path));
    }
    Ok(out)
}

fn keep_fmt(ext: &str) -> String {
    match ext {
        "xls" | "xlsb" | "xlsm" | "xla" | "xlam" | "ods" => "xlsx".into(),
        "yml" => "yaml".into(),
        "jsonl" => "ndjson".into(),
        "tab" => "tsv".into(),
        e => e.into(),
    }
}

fn native_par(fmt: &str) -> bool {
    !matches!(fmt, "ods" | "xls" | "pdf")
}

pub fn convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "xlsx");
    let ro = ReadOpts::from(p);
    let wo = WriteOpts::from(p);
    batch(ctx, files, &[], native_par(&fmt), |it| {
        if !is_sheet(&it.ext) {
            return Ok(vec![it.clone()]);
        }
        let tables = read_tables(&it.path_buf(), &it.ext, &ro)?;
        write_out(ctx, it, tables, &fmt, &wo)
    })
}

fn col_index(tok: &str, names: &[String], width: usize) -> Option<usize> {
    let t = tok.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(i) = names.iter().position(|n| n.trim().to_lowercase() == t.to_lowercase()) {
        return Some(i);
    }
    if let Ok(n) = t.parse::<usize>() {
        if n >= 1 && n <= width {
            return Some(n - 1);
        }
    }
    if t.len() <= 3 {
        if let Some(i) = letters_index(t) {
            if i < width {
                return Some(i);
            }
        }
    }
    None
}

/// `name, B, 3, D-F, 2-4` -> column indexes.
fn resolve_cols(spec: &str, names: &[String], width: usize) -> Vec<usize> {
    let mut out = vec![];
    for tok in spec.split([',', ';']) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        if let Some(i) = col_index(tok, names, width) {
            out.push(i);
            continue;
        }
        if let Some((a, b)) = tok.split_once('-') {
            if let (Some(x), Some(y)) = (col_index(a, names, width), col_index(b, names, width)) {
                out.extend(x.min(y)..=x.max(y));
            }
        }
    }
    let mut seen = HashSet::new();
    out.retain(|i| seen.insert(*i));
    out
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn apply_op(t: &mut Table, op: &str, p: &Params, header: bool, re: Option<&Regex>) -> Result<()> {
    t.pad();
    let h = if header && !t.rows.is_empty() { 1 } else { 0 };
    let names: Vec<String> = if h == 1 { t.rows[0].iter().map(Cell::text).collect() } else { vec![] };
    let width = t.width();
    let cols_of = |key: &str| resolve_cols(&p.str(key, ""), &names, width);
    let one_col = |key: &str| cols_of(key).first().copied();
    match op {
        "select" | "drop" => {
            let sel = cols_of("columns");
            if sel.is_empty() {
                bail!("no matching columns for \"{}\"", p.str("columns", ""));
            }
            let keep: Vec<usize> = if op == "select" { sel } else { (0..width).filter(|i| !sel.contains(i)).collect() };
            for r in &mut t.rows {
                *r = keep.iter().map(|&i| r.get(i).cloned().unwrap_or(Cell::Empty)).collect();
            }
        }
        "rename" => {
            if h == 0 {
                bail!("renaming needs a header row");
            }
            for line in p.str("mapping", "").lines() {
                if let Some((a, b)) = line.split_once('=') {
                    if let Some(i) = col_index(a, &names, width) {
                        t.rows[0][i] = Cell::Str(b.trim().to_string());
                    }
                }
            }
        }
        "filter" => {
            let col = one_col("column");
            let cmp = p.str("cmp", "contains");
            let val = p.str("value", "");
            let lv = val.trim().to_lowercase();
            let num = val.trim().replace(',', ".").parse::<f64>().ok();
            let test = |c: &Cell| -> bool {
                let tx = c.text().trim().to_lowercase();
                let ord = match (c.num(), num) {
                    (Some(a), Some(b)) => a.partial_cmp(&b),
                    _ => Some(tx.cmp(&lv)),
                };
                match cmp.as_str() {
                    "eq" => ord == Some(std::cmp::Ordering::Equal),
                    "ne" => ord != Some(std::cmp::Ordering::Equal),
                    "gt" => ord == Some(std::cmp::Ordering::Greater),
                    "lt" => ord == Some(std::cmp::Ordering::Less),
                    "ge" => matches!(ord, Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)),
                    "le" => matches!(ord, Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)),
                    "not_contains" => !tx.contains(&lv),
                    "starts" => tx.starts_with(&lv),
                    "ends" => tx.ends_with(&lv),
                    "regex" => re.map(|r| r.is_match(&c.text())).unwrap_or(false),
                    "empty" => c.is_empty(),
                    "not_empty" => !c.is_empty(),
                    _ => tx.contains(&lv),
                }
            };
            let kept: Vec<Vec<Cell>> = t.rows.drain(h..).filter(|r| match col { Some(i) => test(&r[i]), None => r.iter().any(test) }).collect();
            t.rows.extend(kept);
        }
        "sort" => {
            let col = one_col("column").unwrap_or(0);
            let desc = p.str("order", "asc") == "desc";
            let mut rows: Vec<Vec<Cell>> = t.rows.drain(h..).collect();
            let numeric = rows.iter().all(|r| r.get(col).map(|c| c.is_empty() || c.num().is_some()).unwrap_or(true));
            if numeric {
                rows.sort_by(|a, b| {
                    let (x, y) = (a.get(col).and_then(Cell::num), b.get(col).and_then(Cell::num));
                    let o = match (x, y) {
                        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    };
                    if desc {
                        o.reverse()
                    } else {
                        o
                    }
                });
            } else {
                rows.sort_by_cached_key(|r| r.get(col).map(|c| c.text().to_lowercase()).unwrap_or_default());
                if desc {
                    rows.reverse();
                }
            }
            t.rows.extend(rows);
        }
        "dedupe" => {
            let sel = cols_of("columns");
            let mut seen = HashSet::new();
            let rows: Vec<Vec<Cell>> = t
                .rows
                .drain(h..)
                .filter(|r| {
                    let key: Vec<String> = if sel.is_empty() { r.iter().map(Cell::text).collect() } else { sel.iter().map(|&i| r.get(i).map(Cell::text).unwrap_or_default()).collect() };
                    seen.insert(key.join("\u{1f}").to_lowercase())
                })
                .collect();
            t.rows.extend(rows);
        }
        "transpose" => {
            let n = t.rows.len();
            let mut out = vec![vec![Cell::Empty; n]; width];
            for (r, row) in t.rows.iter().enumerate() {
                for (c, cell) in row.iter().enumerate() {
                    out[c][r] = cell.clone();
                }
            }
            t.rows = out;
        }
        "limit" => {
            let off = p.i64("offset", 0).max(0) as usize;
            let cnt = p.i64("count", 100).max(0) as usize;
            let rows: Vec<Vec<Cell>> = t.rows.drain(h..).skip(off).take(if cnt == 0 { usize::MAX } else { cnt }).collect();
            t.rows.extend(rows);
        }
        "fill" => {
            let sel = cols_of("columns");
            let cols: Vec<usize> = if sel.is_empty() { (0..width).collect() } else { sel };
            let down = p.str("fill_mode", "down") == "down";
            let value = infer(&p.str("value", ""));
            let mut last: Vec<Cell> = vec![Cell::Empty; width];
            for r in t.rows.iter_mut().skip(h) {
                for &c in &cols {
                    if r[c].is_empty() {
                        r[c] = if down { last[c].clone() } else { value.clone() };
                    } else {
                        last[c] = r[c].clone();
                    }
                }
            }
        }
        "replace" => {
            let sel = cols_of("columns");
            let find = p.str("find", "");
            let rep = p.str("replace", "");
            if find.is_empty() && re.is_none() {
                bail!("type the text to find");
            }
            for r in t.rows.iter_mut().skip(h) {
                for (c, cell) in r.iter_mut().enumerate() {
                    if !sel.is_empty() && !sel.contains(&c) {
                        continue;
                    }
                    if cell.is_empty() {
                        continue;
                    }
                    let s = cell.text();
                    let n = match re {
                        Some(x) => x.replace_all(&s, rep.as_str()).into_owned(),
                        None => s.replace(&find, &rep),
                    };
                    if n != s {
                        *cell = infer(&n);
                    }
                }
            }
        }
        "case" => {
            let sel = cols_of("columns");
            let case = p.str("case", "upper");
            for r in t.rows.iter_mut().skip(h) {
                for (c, cell) in r.iter_mut().enumerate() {
                    if !sel.is_empty() && !sel.contains(&c) {
                        continue;
                    }
                    if let Cell::Str(s) = cell {
                        *s = match case.as_str() {
                            "lower" => s.to_lowercase(),
                            "title" => title_case(&s.to_lowercase()),
                            "clean" => s.split_whitespace().collect::<Vec<_>>().join(" "),
                            _ => s.to_uppercase(),
                        };
                    }
                }
            }
        }
        "index" => {
            let name = p.str("name", "#");
            for (i, r) in t.rows.iter_mut().enumerate() {
                let v = if i < h { Cell::Str(name.clone()) } else { Cell::Int((i + 1 - h) as i64) };
                r.insert(0, v);
            }
        }
        "clean" => {
            t.rows.retain(|r| r.iter().any(|c| !c.is_empty()));
            let w = t.width();
            let keep: Vec<usize> = (0..w).filter(|&c| t.rows.iter().skip(h).any(|r| r.get(c).map(|x| !x.is_empty()).unwrap_or(false))).collect();
            for r in &mut t.rows {
                *r = keep.iter().map(|&i| r.get(i).cloned().unwrap_or(Cell::Empty)).collect();
            }
        }
        "split" => {
            let col = one_col("column").ok_or_else(|| anyhow!("pick the column to split"))?;
            let sep = p.str("separator", ",");
            let sep = if sep.is_empty() { " ".to_string() } else { sep };
            let parts: Vec<Vec<String>> = t.rows.iter().map(|r| r[col].text().split(sep.as_str()).map(|s| s.trim().to_string()).collect()).collect();
            let n = parts.iter().skip(h).map(|v| v.len()).max().unwrap_or(1);
            for (ri, r) in t.rows.iter_mut().enumerate() {
                let mut new: Vec<Cell> = if ri < h {
                    (0..n).map(|k| Cell::Str(format!("{}_{}", names.get(col).cloned().unwrap_or_default(), k + 1))).collect()
                } else {
                    (0..n).map(|k| parts[ri].get(k).map(|s| infer(s)).unwrap_or(Cell::Empty)).collect()
                };
                let tail = r.split_off(col + 1);
                r.pop();
                r.append(&mut new);
                r.extend(tail);
            }
        }
        "math" => {
            let a = one_col("column").ok_or_else(|| anyhow!("pick the column"))?;
            let fop = p.str("math_op", "multiply");
            let v = p.f64("value", 1.0);
            for r in t.rows.iter_mut().skip(h) {
                if let Some(x) = r[a].num() {
                    let y = match fop.as_str() {
                        "add" => x + v,
                        "subtract" => x - v,
                        "divide" => {
                            if v == 0.0 {
                                x
                            } else {
                                x / v
                            }
                        }
                        "round" => {
                            let k = 10f64.powi(v.clamp(0.0, 10.0) as i32);
                            (x * k).round() / k
                        }
                        "percent" => x * v / 100.0,
                        _ => x * v,
                    };
                    r[a] = if y.fract() == 0.0 && y.abs() < 9e15 { Cell::Int(y as i64) } else { Cell::Num(y) };
                }
            }
        }
        other => bail!("unknown table operation {other}"),
    }
    t.pad();
    Ok(())
}

pub fn tools(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let op = p.str("op", "filter");
    let fmt = p.str("format", "keep");
    let ro = ReadOpts::from(p);
    let wo = WriteOpts::from(p);
    let re = if (op == "filter" && p.str("cmp", "") == "regex") || (op == "replace" && p.bool("regex", false)) {
        let pat = if op == "filter" { p.str("value", "") } else { p.str("find", "") };
        Some(Regex::new(&pat).context("invalid regular expression")?)
    } else {
        None
    };
    batch(ctx, files, &[], native_par(&fmt), |it| {
        if !is_sheet(&it.ext) {
            return Ok(vec![it.clone()]);
        }
        let mut tables = read_tables(&it.path_buf(), &it.ext, &ro)?;
        for t in &mut tables {
            apply_op(t, &op, p, wo.header, re.as_ref())?;
        }
        let f = if fmt == "keep" { keep_fmt(&it.ext) } else { fmt.clone() };
        write_out(ctx, it, tables, &f, &wo)
    })
}

pub fn merge(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "rows");
    let fmt = p.str("format", "xlsx");
    let name = p.str("name", "merged");
    let add_source = p.bool("add_source", false);
    let ro = ReadOpts::from(p);
    let mut wo = WriteOpts::from(p);
    let (work, pass): (Vec<FileItem>, Vec<FileItem>) = files.into_iter().partition(|f| is_sheet(&f.ext));
    if work.is_empty() {
        bail!("No spreadsheets or CSV/JSON tables in the input");
    }
    let total = work.len();
    let mut tables = vec![];
    for (i, it) in work.iter().enumerate() {
        ctx.check()?;
        match read_tables(&it.path_buf(), &it.ext, &ro) {
            Ok(ts) => {
                let many = ts.len() > 1;
                for mut t in ts {
                    if add_source {
                        for (ri, r) in t.rows.iter_mut().enumerate() {
                            r.insert(0, if ri == 0 && wo.header { Cell::Str("source".into()) } else { Cell::Str(it.name.clone()) });
                        }
                    }
                    t.name = if many { format!("{} - {}", it.stem(), t.name) } else { it.stem() };
                    tables.push(t);
                }
            }
            Err(e) => ctx.warn(format!("{}: {:#}", it.name, e)),
        }
        ctx.progress(i + 1, total, &it.name);
    }
    if tables.is_empty() {
        bail!("No readable tables. {}", ctx.first_warning());
    }
    let merged = match mode.as_str() {
        "sheets" => tables,
        "columns" => vec![side_by_side(tables)],
        _ => vec![merge_tables(tables, wo.header)],
    };
    wo.multi = "separate".into();
    let fmt = if mode == "sheets" && !matches!(fmt.as_str(), "xlsx" | "ods" | "xls" | "pdf" | "html" | "md" | "txt" | "json" | "yaml" | "xml" | "sql") { "xlsx".to_string() } else { fmt };
    let dir = ctx.out_dir(&work[0].rel)?;
    let path = write_file(ctx, &dir, &name, &merged, &fmt, &wo)?;
    let mut v = vec![FileItem::from_path(&path, &work[0].rel)];
    v.extend(pass);
    Ok(v)
}

pub fn split(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "rows");
    let n = p.i64("rows", 1000).max(1) as usize;
    let fmt = p.str("format", "keep");
    let ro = ReadOpts::from(p);
    let mut wo = WriteOpts::from(p);
    wo.multi = "separate".into();
    batch(ctx, files, &[], native_par(&fmt), |it| {
        if !is_sheet(&it.ext) {
            return Ok(vec![it.clone()]);
        }
        let tables = read_tables(&it.path_buf(), &it.ext, &ro)?;
        let f = if fmt == "keep" { keep_fmt(&it.ext) } else { fmt.clone() };
        let dir = ctx.out_dir(&it.rel)?;
        let many = tables.len() > 1;
        let mut out = vec![];
        for t in tables {
            let prefix = if many { format!("{} - {}", it.stem(), t.name) } else { it.stem() };
            let h = if wo.header && !t.rows.is_empty() { 1 } else { 0 };
            let head: Vec<Vec<Cell>> = t.rows[..h].to_vec();
            let parts: Vec<(String, Table)> = match mode.as_str() {
                "sheets" => vec![(prefix.clone(), t)],
                "column" => {
                    let names: Vec<String> = if h == 1 { t.rows[0].iter().map(Cell::text).collect() } else { vec![] };
                    let col = col_index(&p.str("column", ""), &names, t.width()).ok_or_else(|| anyhow!("pick the column to split by"))?;
                    let mut groups: Vec<(String, Vec<Vec<Cell>>)> = vec![];
                    for r in &t.rows[h..] {
                        let key = r.get(col).map(Cell::text).unwrap_or_default();
                        let key = if key.trim().is_empty() { "(empty)".to_string() } else { key.trim().to_string() };
                        match groups.iter_mut().find(|g| g.0 == key) {
                            Some(g) => g.1.push(r.clone()),
                            None => groups.push((key, vec![r.clone()])),
                        }
                    }
                    groups
                        .into_iter()
                        .map(|(k, rows)| {
                            let mut all = head.clone();
                            all.extend(rows);
                            (format!("{prefix} - {}", sanitize(&k)), Table { name: k, rows: all })
                        })
                        .collect()
                }
                _ => t.rows[h..]
                    .chunks(n)
                    .enumerate()
                    .map(|(i, c)| {
                        let mut all = head.clone();
                        all.extend(c.iter().cloned());
                        (format!("{prefix}_{:03}", i + 1), Table { name: t.name.clone(), rows: all })
                    })
                    .collect(),
            };
            for (stem, part) in parts {
                let path = write_file(ctx, &dir, &stem, std::slice::from_ref(&part), &f, &wo)?;
                out.push(it.derive(&path));
            }
        }
        Ok(out)
    })
}

/// First rows of a sheet for the inspector.
#[tauri::command]
pub async fn sheet_preview(path: String, sheet: Option<String>, limit: Option<usize>) -> std::result::Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Value> {
        let p = PathBuf::from(&path);
        let ext = crate::kinds::ext_of(&p);
        let limit = limit.unwrap_or(200).clamp(1, 5000);
        let mut o = ReadOpts::preview();
        let mut names: Vec<String> = vec![];
        if BOOK.contains(&ext.as_str()) {
            let wb = open_workbook_auto(&p).map_err(|e| anyhow!("{e}"))?;
            names = wb.sheet_names().to_vec();
            o.sheets = "name".into();
            o.sheet_name = sheet.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| names.first().cloned().unwrap_or_default());
        }
        let tables = read_tables(&p, &ext, &o)?;
        let t = tables.into_iter().next().unwrap_or_default();
        let rows: Vec<Vec<String>> = t.rows.iter().take(limit).map(|r| r.iter().map(Cell::text).collect()).collect();
        Ok(json!({"sheets": names, "sheet": t.name, "rows": rows, "total_rows": t.rows.len(), "cols": t.width()}))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{:#}", e))
}
