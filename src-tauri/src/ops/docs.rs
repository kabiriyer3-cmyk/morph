use super::{batch, run_tool};
use crate::engine::{Ctx, FileItem};
use crate::util::{command, temp_root, Params};
use anyhow::{bail, Result};
use std::path::Path;

const OFFICE: &[&str] = &[
    "doc", "docx", "odt", "rtf", "xls", "xlsx", "ods", "ppt", "pptx", "odp", "html", "htm", "txt", "csv", "md", "epub",
];
const PANDOC_IN: &[&str] = &[
    "md", "markdown", "html", "htm", "docx", "odt", "epub", "rst", "tex", "org", "txt", "rtf", "ipynb", "csv",
];

fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    format!("file:///{}", s.trim_start_matches('/').replace(' ', "%20"))
}

pub fn office(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "pdf");
    let bin = ctx.tools.soffice()?.clone();
    let profile = file_url(&temp_root().join("lo-profile"));
    batch(ctx, files, &[], false, |it| {
        if !OFFICE.contains(&it.ext.as_str()) {
            return Ok(vec![it.clone()]);
        }
        let tmp = ctx.reserve(&ctx.work, "_lo", "")?;
        std::fs::create_dir_all(&tmp)?;
        let mut cmd = command(&bin);
        cmd.arg(format!("-env:UserInstallation={profile}"))
            .args(["--headless", "--norestore", "--nolockcheck", "--convert-to", fmt.as_str(), "--outdir"])
            .arg(&tmp)
            .arg(&it.path);
        run_tool(ctx, &mut cmd, "LibreOffice")?;
        let produced = std::fs::read_dir(&tmp)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.is_file())
            .ok_or_else(|| anyhow::anyhow!("LibreOffice produced no output (is .{} -> {} supported?)", it.ext, fmt))?;
        let ext = crate::kinds::ext_of(&produced);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        std::fs::rename(&produced, &out).or_else(|_| std::fs::copy(&produced, &out).map(|_| ()))?;
        let _ = std::fs::remove_dir_all(&tmp);
        Ok(vec![it.derive(&out)])
    })
}

pub fn pandoc(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "docx");
    let standalone = p.bool("standalone", true);
    let bin = ctx.tools.pandoc()?.clone();
    let (ext, to): (&str, Option<&str>) = match fmt.as_str() {
        "md" => ("md", Some("gfm")),
        "html" => ("html", Some("html5")),
        "docx" => ("docx", None),
        "odt" => ("odt", None),
        "epub" => ("epub", None),
        "rst" => ("rst", None),
        "tex" => ("tex", Some("latex")),
        "txt" => ("txt", Some("plain")),
        "pdf" => ("pdf", None),
        other => bail!("Unsupported Pandoc target {other}"),
    };
    batch(ctx, files, &[], false, |it| {
        if !PANDOC_IN.contains(&it.ext.as_str()) {
            return Ok(vec![it.clone()]);
        }
        let out = ctx.out_path(it, &it.stem(), ext)?;
        let mut cmd = command(&bin);
        cmd.arg(&it.path).arg("-o").arg(&out);
        if let Some(t) = to {
            cmd.arg("-t").arg(t);
        }
        if standalone {
            cmd.arg("-s");
        }
        if let Some(dir) = Path::new(&it.path).parent() {
            cmd.arg("--resource-path").arg(dir);
        }
        run_tool(ctx, &mut cmd, "Pandoc")?;
        Ok(vec![it.derive(&out)])
    })
}
