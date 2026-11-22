//! Documents: LibreOffice and Pandoc conversion, native Markdown to HTML and text extraction.
use super::{batch, run_tool};
use crate::engine::{Ctx, FileItem};
use crate::util::{command, temp_root, Params};
use anyhow::{anyhow, bail, Result};
use regex::Regex;
use std::io::Read;
use std::path::{Path, PathBuf};

const OFFICE: &[&str] = &[
    "doc", "docx", "odt", "rtf", "xls", "xlsx", "xlsm", "xlsb", "ods", "ppt", "pptx", "odp", "odg", "html", "htm", "txt",
    "csv", "md", "epub", "fodt", "fods", "wpd", "pages", "vsd", "pub",
];
const PANDOC_IN: &[&str] = &[
    "md", "markdown", "html", "htm", "docx", "odt", "epub", "rst", "tex", "org", "txt", "rtf", "ipynb", "csv", "tsv",
    "json", "textile", "adoc", "typ", "xml",
];

fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    format!("file:///{}", s.trim_start_matches('/').replace(' ', "%20"))
}

/// LibreOffice filter string for a target format.
fn lo_target(fmt: &str, input_ext: &str) -> String {
    match fmt {
        "csv" => "csv:Text - txt - csv (StarCalc):44,34,76,1".into(),
        "txt" => "txt:Text (encoded):UTF8".into(),
        "pdf" if matches!(input_ext, "html" | "htm") => "pdf:writer_web_pdf_Export".into(),
        "html" if matches!(input_ext, "xls" | "xlsx" | "ods" | "csv") => "html:HTML (StarCalc)".into(),
        other => other.to_string(),
    }
}

/// Converts `input` with LibreOffice; returns the produced file (in a temp folder of this node).
pub fn soffice_raw(ctx: &Ctx, input: &Path, fmt: &str) -> Result<PathBuf> {
    let bin = ctx.tools.soffice()?.clone();
    let profile = file_url(&temp_root().join("lo-profile"));
    let tmp = ctx.reserve(&ctx.work, "_lo", "")?;
    std::fs::create_dir_all(&tmp)?;
    let target = lo_target(fmt, &crate::kinds::ext_of(input));
    let mut cmd = command(&bin);
    cmd.arg(format!("-env:UserInstallation={profile}"))
        .args(["--headless", "--norestore", "--nolockcheck", "--convert-to", target.as_str(), "--outdir"])
        .arg(&tmp)
        .arg(input);
    run_tool(ctx, &mut cmd, "LibreOffice")?;
    std::fs::read_dir(&tmp)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow!("LibreOffice produced no output (is .{} \u{2192} {} supported?)", crate::kinds::ext_of(input), fmt))
}

fn place(ctx: &Ctx, it: &FileItem, produced: &Path) -> Result<FileItem> {
    let ext = crate::kinds::ext_of(produced);
    let out = ctx.out_path(it, &it.stem(), &ext)?;
    if std::fs::rename(produced, &out).is_err() {
        std::fs::copy(produced, &out)?;
    }
    if let Some(d) = produced.parent() {
        let _ = std::fs::remove_dir_all(d);
    }
    Ok(it.derive(&out))
}

pub fn office(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "pdf");
    let custom = p.str("filter", "");
    ctx.tools.soffice()?;
    batch(ctx, files, &[], false, |it| {
        if !OFFICE.contains(&it.ext.as_str()) {
            return Ok(vec![it.clone()]);
        }
        let f = if custom.trim().is_empty() { fmt.clone() } else { custom.trim().to_string() };
        let produced = soffice_raw(ctx, &it.path_buf(), &f)?;
        Ok(vec![place(ctx, it, &produced)?])
    })
}

fn pandoc_target(fmt: &str) -> Result<(&'static str, Option<&'static str>)> {
    Ok(match fmt {
        "md" => ("md", Some("gfm")),
        "commonmark" => ("md", Some("commonmark")),
        "html" => ("html", Some("html5")),
        "docx" => ("docx", None),
        "odt" => ("odt", None),
        "rtf" => ("rtf", Some("rtf")),
        "epub" => ("epub", Some("epub3")),
        "pptx" => ("pptx", None),
        "rst" => ("rst", None),
        "tex" => ("tex", Some("latex")),
        "txt" => ("txt", Some("plain")),
        "pdf" => ("pdf", None),
        "asciidoc" => ("adoc", Some("asciidoc")),
        "org" => ("org", Some("org")),
        "mediawiki" => ("wiki", Some("mediawiki")),
        "typst" => ("typ", Some("typst")),
        "ipynb" => ("ipynb", Some("ipynb")),
        "textile" => ("textile", Some("textile")),
        "docbook" => ("xml", Some("docbook5")),
        "man" => ("1", Some("man")),
        "jira" => ("jira", Some("jira")),
        "json" => ("json", Some("json")),
        other => bail!("Unsupported Pandoc target {other}"),
    })
}

pub fn pandoc(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "docx");
    let standalone = p.bool("standalone", true);
    let toc = p.bool("toc", false);
    let numbered = p.bool("number_sections", false);
    let wrap = p.str("wrap", "auto");
    let embed = p.bool("embed_resources", true);
    let reference = p.str("reference_doc", "");
    let bin = ctx.tools.pandoc()?.clone();
    let (ext, to) = pandoc_target(&fmt)?;
    batch(ctx, files, &[], false, |it| {
        if !PANDOC_IN.contains(&it.ext.as_str()) {
            return Ok(vec![it.clone()]);
        }
        let out = ctx.out_path(it, &it.stem(), ext)?;
        let mut cmd = command(&bin);
        cmd.arg(&it.path).arg("-o").arg(&out);
        if it.ext == "txt" {
            cmd.arg("-f").arg("markdown");
        }
        if let Some(t) = to {
            cmd.arg("-t").arg(t);
        }
        if standalone {
            cmd.arg("-s");
        }
        if toc {
            cmd.arg("--toc");
        }
        if numbered {
            cmd.arg("--number-sections");
        }
        if wrap != "auto" {
            cmd.arg(format!("--wrap={wrap}"));
        }
        if embed && ext == "html" {
            cmd.arg("--embed-resources");
        }
        if !reference.trim().is_empty() && matches!(ext, "docx" | "odt" | "pptx") {
            cmd.arg(format!("--reference-doc={}", reference.trim()));
        }
        if let Some(dir) = Path::new(&it.path).parent() {
            cmd.arg("--resource-path").arg(dir);
        }
        run_tool(ctx, &mut cmd, "Pandoc")?;
        Ok(vec![it.derive(&out)])
    })
}

fn theme_css(theme: &str) -> &'static str {
    match theme {
        "dark" => "body{background:#0d1117;color:#e6edf3}a{color:#58a6ff}code,pre{background:#161b22}th,td{border-color:#30363d}blockquote{color:#9da7b3;border-color:#30363d}hr{border-color:#30363d}",
        "serif" => "body{font-family:Georgia,'Times New Roman',serif;font-size:18px;line-height:1.7;color:#222}h1,h2,h3{font-family:Georgia,serif}",
        _ => "",
    }
}

pub fn md_html(md: &str, title: &str, theme: &str, smart: bool) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut o = Options::empty();
    o.insert(Options::ENABLE_TABLES);
    o.insert(Options::ENABLE_STRIKETHROUGH);
    o.insert(Options::ENABLE_TASKLISTS);
    o.insert(Options::ENABLE_FOOTNOTES);
    o.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    if smart {
        o.insert(Options::ENABLE_SMART_PUNCTUATION);
    }
    let mut body = String::new();
    html::push_html(&mut body, Parser::new_ext(md, o));
    if theme == "none" {
        return body;
    }
    let base = "body{font:16px/1.65 system-ui,'Segoe UI',Roboto,'Helvetica Neue',sans-serif;color:#1f2328;max-width:860px;margin:40px auto;padding:0 24px}h1,h2{border-bottom:1px solid #d1d9e0;padding-bottom:.3em}code{font-family:ui-monospace,Consolas,monospace;font-size:.9em;background:#f3f5f7;padding:.15em .35em;border-radius:5px}pre{background:#f6f8fa;padding:14px 16px;border-radius:8px;overflow:auto}pre code{background:none;padding:0}table{border-collapse:collapse}th,td{border:1px solid #d1d9e0;padding:6px 12px}blockquote{margin:0;padding:0 1em;color:#59636e;border-left:4px solid #d1d9e0}img{max-width:100%}hr{border:0;border-top:1px solid #d1d9e0}";
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>{}{}</style></head><body>\n{}</body></html>\n",
        title.replace('&', "&amp;").replace('<', "&lt;"),
        base,
        theme_css(theme),
        body
    )
}

pub fn markdown(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let theme = p.str("theme", "light");
    let smart = p.bool("smart_quotes", true);
    batch(ctx, files, &[], true, |it| {
        if !matches!(it.ext.as_str(), "md" | "markdown" | "txt") {
            return Ok(vec![it.clone()]);
        }
        let bytes = std::fs::read(&it.path)?;
        let (md, _) = crate::ops::data::decode(&bytes, "auto")?;
        let title = md.lines().find(|l| l.starts_with("# ")).map(|l| l[2..].trim().to_string()).unwrap_or_else(|| it.stem());
        let html = md_html(&md, &title, &theme, smart);
        let out = ctx.out_path(it, &it.stem(), "html")?;
        std::fs::write(&out, html)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn decode_entities(s: &str) -> String {
    let re = Regex::new(r"&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);").expect("entity regex");
    re.replace_all(s, |c: &regex::Captures| {
        let e = &c[1];
        let ch = if let Some(h) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
            u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
        } else if let Some(d) = e.strip_prefix('#') {
            d.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            match e {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "mdash" => Some('\u{2014}'),
                "ndash" => Some('\u{2013}'),
                "hellip" => Some('\u{2026}'),
                "laquo" => Some('\u{ab}'),
                "raquo" => Some('\u{bb}'),
                "copy" => Some('\u{a9}'),
                "reg" => Some('\u{ae}'),
                "trade" => Some('\u{2122}'),
                "bull" => Some('\u{2022}'),
                "euro" => Some('\u{20ac}'),
                _ => None,
            }
        };
        ch.map(|c| c.to_string()).unwrap_or_else(|| c[0].to_string())
    })
    .into_owned()
}

fn tidy(s: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in s.lines() {
        let l = line.trim_end();
        if l.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(l);
        out.push('\n');
    }
    out.trim().to_string() + "\n"
}

fn strip_tags(s: &str) -> String {
    Regex::new(r"(?s)<[^>]*>").expect("tag regex").replace_all(s, "").into_owned()
}

pub fn html_text(html: &str) -> String {
    let mut s = Regex::new(r"(?is)<(script|style|head)\b.*?</(script|style|head)>").expect("re").replace_all(html, "").into_owned();
    s = Regex::new(r"(?i)<br\s*/?>").expect("re").replace_all(&s, "\n").into_owned();
    s = Regex::new(r"(?i)<li[^>]*>").expect("re").replace_all(&s, "\n\u{2022} ").into_owned();
    s = Regex::new(r"(?i)</(p|div|h[1-6]|tr|table|section|article|blockquote|pre|ul|ol)>").expect("re").replace_all(&s, "\n\n").into_owned();
    s = Regex::new(r"(?i)</t[dh]>").expect("re").replace_all(&s, "\t").into_owned();
    tidy(&decode_entities(&strip_tags(&s)))
}

fn office_xml_text(xml: &str, kind: &str) -> String {
    let mut s = xml.to_string();
    let rules: &[(&str, &str)] = match kind {
        "docx" => &[(r"<w:tab[^>]*/>", "\t"), (r"<w:br[^>]*/>", "\n"), (r"</w:p>", "\n")],
        "pptx" => &[(r"<a:br[^>]*/>", "\n"), (r"</a:p>", "\n")],
        _ => &[(r"<text:tab[^>]*/>", "\t"), (r"<text:line-break[^>]*/>", "\n"), (r"<text:s[^>]*/>", " "), (r"</text:(p|h)>", "\n")],
    };
    for (pat, rep) in rules {
        s = Regex::new(pat).expect("re").replace_all(&s, *rep).into_owned();
    }
    tidy(&decode_entities(&strip_tags(&s)))
}

fn zip_entry(path: &Path, name: &str) -> Result<String> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut f = z.by_name(name)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(s)
}

fn zip_entries(path: &Path, pick: &dyn Fn(&str) -> bool) -> Result<Vec<(String, String)>> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let names: Vec<String> = (0..z.len()).filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string())).filter(|n| pick(n)).collect();
    let mut out = vec![];
    for n in names {
        let mut f = z.by_name(&n)?;
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            out.push((n, s));
        }
    }
    Ok(out)
}

fn rtf_text(src: &str) -> String {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = String::new();
    let mut stack: Vec<bool> = vec![];
    let mut skip = false;
    let mut pending: Vec<u8> = vec![];
    let mut cp = 1252u32;
    const DEST: [&str; 16] = [
        "\\fonttbl", "\\colortbl", "\\stylesheet", "\\info", "\\pict", "\\header", "\\footer", "\\listtable",
        "\\listoverridetable", "\\rsidtbl", "\\generator", "\\xmlnstbl", "\\themedata", "\\colorschememapping", "\\latentstyles", "\\datastore",
    ];
    let flush = |pending: &mut Vec<u8>, out: &mut String, cp: u32| {
        if pending.is_empty() {
            return;
        }
        let enc = encoding_rs::Encoding::for_label(format!("windows-{cp}").as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252);
        let (s, _, _) = enc.decode(pending);
        out.push_str(&s);
        pending.clear();
    };
    while i < b.len() {
        match b[i] {
            b'{' => {
                flush(&mut pending, &mut out, cp);
                stack.push(skip);
                i += 1;
                let rest = &b[i..];
                if rest.starts_with(b"\\*") || DEST.iter().any(|d| rest.starts_with(d.as_bytes())) {
                    skip = true;
                }
            }
            b'}' => {
                flush(&mut pending, &mut out, cp);
                skip = stack.pop().unwrap_or(false);
                i += 1;
            }
            b'\\' => {
                i += 1;
                if i >= b.len() {
                    break;
                }
                let c = b[i];
                if c == b'\'' {
                    if let Some(h) = b.get(i + 1..i + 3).and_then(|x| std::str::from_utf8(x).ok()).and_then(|x| u8::from_str_radix(x, 16).ok()) {
                        if !skip {
                            pending.push(h);
                        }
                    }
                    i += 3;
                    continue;
                }
                flush(&mut pending, &mut out, cp);
                if !c.is_ascii_alphabetic() {
                    if !skip {
                        match c {
                            b'\\' | b'{' | b'}' => out.push(c as char),
                            b'~' => out.push('\u{a0}'),
                            b'_' => out.push('-'),
                            _ => {}
                        }
                    }
                    i += 1;
                    continue;
                }
                let start = i;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word = std::str::from_utf8(&b[start..i]).unwrap_or("");
                let ns = i;
                if i < b.len() && (b[i] == b'-' || b[i].is_ascii_digit()) {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let num: Option<i64> = std::str::from_utf8(&b[ns..i]).ok().and_then(|x| x.parse().ok());
                if i < b.len() && b[i] == b' ' {
                    i += 1;
                }
                if word == "ansicpg" {
                    if let Some(n) = num {
                        cp = n as u32;
                    }
                }
                if skip {
                    continue;
                }
                match word {
                    "par" | "line" | "row" | "sect" | "page" => out.push('\n'),
                    "tab" | "cell" => out.push('\t'),
                    "emdash" => out.push('\u{2014}'),
                    "endash" => out.push('\u{2013}'),
                    "bullet" => out.push('\u{2022}'),
                    "lquote" => out.push('\u{2018}'),
                    "rquote" => out.push('\u{2019}'),
                    "ldblquote" => out.push('\u{201c}'),
                    "rdblquote" => out.push('\u{201d}'),
                    "u" => {
                        if let Some(n) = num {
                            let n = if n < 0 { n + 65536 } else { n };
                            if let Some(ch) = char::from_u32(n as u32) {
                                out.push(ch);
                            }
                            if i < b.len() && b[i] == b'?' {
                                i += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                flush(&mut pending, &mut out, cp);
                let ch = src[i..].chars().next().unwrap_or(' ');
                if !skip {
                    out.push(ch);
                }
                i += ch.len_utf8().max(1);
            }
        }
    }
    flush(&mut pending, &mut out, cp);
    tidy(&out)
}

/// Plain text of a document (DOCX, ODT, PPTX, XLSX, EPUB, HTML, RTF, PDF, Markdown...).
pub fn extract_text(path: &Path, ext: &str) -> Result<String> {
    Ok(match ext {
        "docx" => office_xml_text(&zip_entry(path, "word/document.xml")?, "docx"),
        "odt" | "odp" | "ods" => office_xml_text(&zip_entry(path, "content.xml")?, "odt"),
        "pptx" => {
            let mut slides = zip_entries(path, &|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))?;
            slides.sort_by_key(|(n, _)| n.trim_start_matches("ppt/slides/slide").trim_end_matches(".xml").parse::<u32>().unwrap_or(0));
            slides
                .iter()
                .enumerate()
                .map(|(i, (_, x))| format!("--- {} ---\n{}", i + 1, office_xml_text(x, "pptx")))
                .collect::<Vec<_>>()
                .join("\n")
        }
        "epub" => {
            let mut parts = zip_entries(path, &|n| {
                let l = n.to_lowercase();
                l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm")
            })?;
            parts.sort_by(|a, b| crate::ops::pdftools::natural_cmp(&a.0, &b.0));
            parts.iter().map(|(_, h)| html_text(h)).collect::<Vec<_>>().join("\n")
        }
        "html" | "htm" => {
            let (s, _) = crate::ops::data::decode(&std::fs::read(path)?, "auto")?;
            html_text(&s)
        }
        "rtf" => rtf_text(&String::from_utf8_lossy(&std::fs::read(path)?)),
        "pdf" => crate::ops::pdftools::pdf_text(path, false)?,
        e if crate::ops::sheet::is_sheet(e) => {
            let tables = crate::ops::sheet::read_tables(path, e, &crate::ops::sheet::ReadOpts::preview())?;
            let mut s = String::new();
            for t in tables {
                if !t.name.is_empty() {
                    s.push_str(&format!("== {} ==\n", t.name));
                }
                for r in &t.rows {
                    s.push_str(&r.iter().map(|c| c.text()).collect::<Vec<_>>().join("\t"));
                    s.push('\n');
                }
                s.push('\n');
            }
            s
        }
        _ => {
            let (s, _) = crate::ops::data::decode(&std::fs::read(path)?, "auto")?;
            s
        }
    })
}

pub fn to_text(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let combine = p.bool("combine", false);
    let accepts = |it: &FileItem| {
        matches!(it.kind.as_str(), "doc" | "text" | "data") || crate::ops::sheet::is_sheet(&it.ext)
    };
    if combine {
        let mut s = String::new();
        let list: Vec<&FileItem> = files.iter().filter(|f| accepts(f)).collect();
        if list.is_empty() {
            bail!("No documents in the input");
        }
        for (i, it) in list.iter().enumerate() {
            ctx.check()?;
            match extract_text(&it.path_buf(), &it.ext) {
                Ok(t) => s.push_str(&format!("===== {} =====\n{}\n", it.name, t.trim())),
                Err(e) => ctx.warn(format!("{}: {:#}", it.name, e)),
            }
            ctx.progress(i + 1, list.len(), &it.name);
        }
        let out = ctx.reserve(&ctx.out_dir("")?, "text", "txt")?;
        std::fs::write(&out, s)?;
        return Ok(vec![FileItem::from_path(&out, "")]);
    }
    batch(ctx, files, &[], true, |it| {
        if !accepts(it) {
            return Ok(vec![it.clone()]);
        }
        let s = extract_text(&it.path_buf(), &it.ext)?;
        let out = ctx.out_path(it, &it.stem(), "txt")?;
        std::fs::write(&out, s)?;
        Ok(vec![it.derive(&out)])
    })
}
