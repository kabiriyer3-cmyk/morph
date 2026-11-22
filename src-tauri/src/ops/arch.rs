//! Archive nodes: zip, 7z, tar(.gz/.zst/.xz/.bz2) and single-file compressors.
use super::batch;
use crate::engine::{Ctx, FileItem};
use crate::util::{clean_rel, file_name, join_rel, link_or_copy, sanitize, Params};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SUFFIXES: &[&str] = &[
    ".tar.gz", ".tar.zst", ".tar.xz", ".tar.bz2", ".tgz", ".tzst", ".txz", ".tbz2", ".tbz", ".zip", ".7z", ".tar", ".gz",
    ".zst", ".xz", ".bz2",
];

fn strip_suffix_ci<'a>(name: &'a str, suf: &str) -> Option<&'a str> {
    if name.len() < suf.len() {
        return None;
    }
    let cut = name.len() - suf.len();
    if !name.is_char_boundary(cut) {
        return None;
    }
    if name[cut..].eq_ignore_ascii_case(suf) {
        Some(&name[..cut])
    } else {
        None
    }
}

pub fn archive_stem(name: &str) -> String {
    for suf in SUFFIXES {
        if let Some(s) = strip_suffix_ci(name, suf) {
            return s.to_string();
        }
    }
    name.to_string()
}

fn ends(name: &str, sufs: &[&str]) -> bool {
    sufs.iter().any(|s| strip_suffix_ci(name, s).is_some())
}

fn zstd_level(level: i64) -> i32 {
    if level <= 0 {
        1
    } else {
        ((level * 2) + 1).min(19) as i32
    }
}

fn dedupe_name(base: &str, k: usize) -> String {
    let p = Path::new(base);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let parent = p.parent().map(|d| d.to_string_lossy().replace('\\', "/")).unwrap_or_default();
    let name = match p.extension() {
        Some(e) => format!("{stem} ({k}).{}", e.to_string_lossy()),
        None => format!("{stem} ({k})"),
    };
    join_rel(&parent, &name)
}

fn write_tar<W: Write>(w: W, entries: &[(PathBuf, String)]) -> Result<W> {
    let mut b = tar::Builder::new(w);
    b.follow_symlinks(true);
    for (p, n) in entries {
        b.append_path_with_name(p, n)?;
    }
    Ok(b.into_inner()?)
}

fn write_zip(target: &Path, entries: &[(PathBuf, String)], level: i64) -> Result<()> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    let mut zw = zip::ZipWriter::new(BufWriter::new(File::create(target)?));
    for (p, n) in entries {
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let base = SimpleFileOptions::default().large_file(size >= 0xFFFF_FFFF);
        let opts = if level == 0 {
            base.compression_method(CompressionMethod::Stored)
        } else {
            base.compression_method(CompressionMethod::Deflated).compression_level(Some(level))
        };
        zw.start_file(n.as_str(), opts)?;
        let mut f = File::open(p)?;
        std::io::copy(&mut f, &mut zw)?;
    }
    let mut w = zw.finish()?;
    w.flush()?;
    Ok(())
}

fn write_7z(ctx: &Ctx, target: &Path, entries: &[(PathBuf, String)]) -> Result<()> {
    let stage = ctx.reserve(&ctx.work, "_7z_stage", "")?;
    for (p, n) in entries {
        link_or_copy(p, &stage.join(n))?;
    }
    sevenz_rust2::compress_to_path(&stage, target).map_err(|e| anyhow!("7z: {:?}", e))?;
    let _ = std::fs::remove_dir_all(&stage);
    Ok(())
}

fn write_archive(ctx: &Ctx, fmt: &str, level: i64, target: &Path, entries: &[(PathBuf, String)]) -> Result<()> {
    let lvl = level.clamp(0, 9) as u32;
    let file = || -> Result<BufWriter<File>> { Ok(BufWriter::new(File::create(target)?)) };
    match fmt {
        "zip" => write_zip(target, entries, level),
        "7z" => write_7z(ctx, target, entries),
        "tar" => {
            let mut w = write_tar(file()?, entries)?;
            w.flush()?;
            Ok(())
        }
        "tar.gz" => {
            let enc = write_tar(flate2::write::GzEncoder::new(file()?, flate2::Compression::new(lvl)), entries)?;
            enc.finish()?.flush()?;
            Ok(())
        }
        "tar.zst" => {
            let enc = write_tar(zstd::stream::write::Encoder::new(file()?, zstd_level(level))?, entries)?;
            enc.finish()?.flush()?;
            Ok(())
        }
        "tar.xz" => {
            let enc = write_tar(xz2::write::XzEncoder::new(file()?, lvl), entries)?;
            enc.finish()?.flush()?;
            Ok(())
        }
        "tar.bz2" => {
            let enc = write_tar(bzip2::write::BzEncoder::new(file()?, bzip2::Compression::new(lvl.max(1))), entries)?;
            enc.finish()?.flush()?;
            Ok(())
        }
        other => bail!("Unknown archive format {other}"),
    }
}

pub fn create(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    if files.is_empty() {
        bail!("No files to archive");
    }
    let fmt = p.str("format", "zip");
    let level = p.i64("level", 6).clamp(0, 9);
    let per_file = p.str("mode", "single") == "per-file";
    let name = p.str("name", "archive");
    let groups: Vec<(String, Vec<FileItem>)> = if per_file {
        files.iter().map(|f| (f.stem(), vec![f.clone()])).collect()
    } else {
        let n = if name.trim().is_empty() { "archive".to_string() } else { name.trim().to_string() };
        vec![(n, files)]
    };
    let total = groups.len();
    let mut out = vec![];
    for (i, (gname, items)) in groups.iter().enumerate() {
        ctx.check()?;
        let mut used: HashSet<String> = HashSet::new();
        let entries: Vec<(PathBuf, String)> = items
            .iter()
            .map(|f| {
                let base = if per_file { sanitize(&f.name) } else { join_rel(&clean_rel(&f.rel), &sanitize(&f.name)) };
                let mut n = base.clone();
                let mut k = 1;
                while !used.insert(n.to_lowercase()) {
                    n = dedupe_name(&base, k);
                    k += 1;
                }
                (f.path_buf(), n)
            })
            .collect();
        let rel = if per_file { items[0].rel.clone() } else { String::new() };
        let dir = ctx.out_dir(&rel)?;
        let target = ctx.reserve(&dir, gname, &fmt)?;
        write_archive(ctx, &fmt, level, &target, &entries)?;
        let mut item = FileItem::from_path(&target, &rel);
        item.origin = Path::new(&items[0].origin).with_file_name(file_name(&target)).to_string_lossy().into_owned();
        out.push(item);
        ctx.progress(i + 1, total, gname);
    }
    Ok(out)
}

fn decompress_single<R: Read>(mut r: R, dest: &Path, name: &str) -> Result<()> {
    let out = dest.join(sanitize(&archive_stem(name)));
    let mut w = BufWriter::new(File::create(&out)?);
    std::io::copy(&mut r, &mut w)?;
    w.flush()?;
    Ok(())
}

fn extract_to(src: &Path, name: &str, dest: &Path) -> Result<()> {
    let open = || -> Result<BufReader<File>> { Ok(BufReader::new(File::open(src)?)) };
    if ends(name, &[".zip"]) {
        zip::ZipArchive::new(open()?)?.extract(dest)?;
    } else if ends(name, &[".7z"]) {
        sevenz_rust2::decompress_file(src, dest).map_err(|e| anyhow!("7z: {:?}", e))?;
    } else if ends(name, &[".tar.gz", ".tgz"]) {
        tar::Archive::new(flate2::read::GzDecoder::new(open()?)).unpack(dest)?;
    } else if ends(name, &[".tar.zst", ".tzst"]) {
        tar::Archive::new(zstd::stream::read::Decoder::new(open()?)?).unpack(dest)?;
    } else if ends(name, &[".tar.xz", ".txz"]) {
        tar::Archive::new(xz2::read::XzDecoder::new(open()?)).unpack(dest)?;
    } else if ends(name, &[".tar.bz2", ".tbz2", ".tbz"]) {
        tar::Archive::new(bzip2::read::BzDecoder::new(open()?)).unpack(dest)?;
    } else if ends(name, &[".tar"]) {
        tar::Archive::new(open()?).unpack(dest)?;
    } else if ends(name, &[".gz"]) {
        decompress_single(flate2::read::GzDecoder::new(open()?), dest, name)?;
    } else if ends(name, &[".zst"]) {
        decompress_single(zstd::stream::read::Decoder::new(open()?)?, dest, name)?;
    } else if ends(name, &[".xz"]) {
        decompress_single(xz2::read::XzDecoder::new(open()?), dest, name)?;
    } else if ends(name, &[".bz2"]) {
        decompress_single(bzip2::read::BzDecoder::new(open()?), dest, name)?;
    } else {
        bail!("Unsupported archive type");
    }
    Ok(())
}

pub fn extract(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let sub = p.bool("subfolder", true);
    batch(ctx, files, &["archive"], false, |it| {
        let dest = ctx.reserve(&ctx.work, &format!("_x_{}", sanitize(&it.name)), "")?;
        std::fs::create_dir_all(&dest)?;
        extract_to(&it.path_buf(), &it.name, &dest)?;
        let stem = sanitize(&archive_stem(&it.name));
        let base_rel = if sub { join_rel(&clean_rel(&it.rel), &stem) } else { clean_rel(&it.rel) };
        let origin_dir = Path::new(&it.origin).parent().map(Path::to_path_buf).unwrap_or_default();
        let mut out = vec![];
        for e in WalkDir::new(&dest).sort_by_file_name().into_iter().filter_map(|e| e.ok()) {
            if !e.file_type().is_file() {
                continue;
            }
            let inner = e
                .path()
                .parent()
                .and_then(|d| d.strip_prefix(&dest).ok())
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let mut f = FileItem::from_path(e.path(), &join_rel(&base_rel, &inner));
            let mut o = origin_dir.clone();
            if sub {
                o.push(&stem);
            }
            if !inner.is_empty() {
                o.push(&inner);
            }
            o.push(&f.name);
            f.origin = o.to_string_lossy().into_owned();
            out.push(f);
        }
        if out.is_empty() {
            bail!("the archive is empty");
        }
        Ok(out)
    })
}

pub fn compress(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "zst");
    let level = p.i64("level", 6).clamp(0, 9);
    batch(ctx, files, &[], true, |it| {
        let out = ctx.out_path(it, &it.name, &fmt)?;
        let mut input = BufReader::new(File::open(&it.path)?);
        let w = BufWriter::new(File::create(&out)?);
        match fmt.as_str() {
            "gz" => {
                let mut e = flate2::write::GzEncoder::new(w, flate2::Compression::new(level as u32));
                std::io::copy(&mut input, &mut e)?;
                e.finish()?.flush()?;
            }
            "xz" => {
                let mut e = xz2::write::XzEncoder::new(w, level as u32);
                std::io::copy(&mut input, &mut e)?;
                e.finish()?.flush()?;
            }
            "bz2" => {
                let mut e = bzip2::write::BzEncoder::new(w, bzip2::Compression::new((level as u32).max(1)));
                std::io::copy(&mut input, &mut e)?;
                e.finish()?.flush()?;
            }
            _ => {
                let mut e = zstd::stream::write::Encoder::new(w, zstd_level(level))?;
                std::io::copy(&mut input, &mut e)?;
                e.finish()?.flush()?;
            }
        }
        Ok(vec![it.derive(&out)])
    })
}

/// Lists archive entries for the inspector.
pub fn list(path: &Path) -> Result<Value> {
    let name = file_name(path);
    let mut items: Vec<Value> = vec![];
    if ends(&name, &[".zip"]) {
        let mut z = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
        let total = z.len();
        for i in 0..total.min(5000) {
            let f = z.by_index(i)?;
            items.push(json!({"name": f.name(), "size": f.size(), "dir": f.is_dir()}));
        }
        return Ok(json!({"entries": items, "total": total}));
    }
    let open = || -> Result<BufReader<File>> { Ok(BufReader::new(File::open(path)?)) };
    let reader: Option<Box<dyn Read>> = if ends(&name, &[".tar.gz", ".tgz"]) {
        Some(Box::new(flate2::read::GzDecoder::new(open()?)))
    } else if ends(&name, &[".tar.zst", ".tzst"]) {
        Some(Box::new(zstd::stream::read::Decoder::new(open()?)?))
    } else if ends(&name, &[".tar.xz", ".txz"]) {
        Some(Box::new(xz2::read::XzDecoder::new(open()?)))
    } else if ends(&name, &[".tar.bz2", ".tbz2", ".tbz"]) {
        Some(Box::new(bzip2::read::BzDecoder::new(open()?)))
    } else if ends(&name, &[".tar"]) {
        Some(Box::new(open()?))
    } else {
        None
    };
    let Some(r) = reader else {
        bail!("Listing is not available for this archive type");
    };
    let mut a = tar::Archive::new(r);
    let mut total = 0usize;
    for e in a.entries()? {
        let e = e?;
        total += 1;
        if items.len() < 5000 {
            let n = e.path()?.to_string_lossy().into_owned();
            items.push(json!({"name": n, "size": e.header().size().unwrap_or(0), "dir": e.header().entry_type().is_dir()}));
        }
    }
    Ok(json!({"entries": items, "total": total}))
}
