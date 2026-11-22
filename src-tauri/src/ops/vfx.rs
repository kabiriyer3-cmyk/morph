//! Extra looks for Color & Cleanup, the "Effect" list of classic converters: emboss,
//! sketch, old film and mosaic. Every other setting of the node still runs through
//! av::video_filters first.
use super::av;
use crate::engine::{Ctx, FileItem};
use crate::ops::media::media_info;
use crate::util::{clean_rel, Params};
use anyhow::Result;
use serde_json::json;

fn look_filter(look: &str) -> Option<&'static str> {
    Some(match look {
        "emboss" => "convolution=-2 -1 0 -1 1 1 0 1 2:-2 -1 0 -1 1 1 0 1 2:-2 -1 0 -1 1 1 0 1 2:-2 -1 0 -1 1 1 0 1 2",
        "sketch" => "edgedetect=low=0.08:high=0.3,negate",
        "old_film" => "curves=preset=vintage,noise=alls=18:allf=t+u,vignette=PI/4",
        "mosaic" => "scale=iw/16:ih/16:flags=area,scale=iw*16:ih*16:flags=neighbor",
        _ => return None,
    })
}

pub fn filters(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let look = p.str("look", "none");
    let Some(chain) = look_filter(&look) else {
        return av::video_filters(ctx, p, files);
    };
    let mut m = p.0.clone();
    m.insert("look".into(), json!("none"));
    let files = match av::video_filters(ctx, &Params(&m), files.clone()) {
        Ok(v) => v,
        Err(e) if format!("{e:#}").contains("No filter is enabled") => files,
        Err(e) => return Err(e),
    };
    let q = p.i64("quality", 20).clamp(0, 51);
    av::each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let ext = if matches!(it.ext.as_str(), "mp4" | "m4v" | "mov") { "mp4" } else { "mkv" };
        // A separate folder keeps the names clean when the color pass ran first.
        let dir = ctx.work.join("_looks").join(clean_rel(&it.rel));
        std::fs::create_dir_all(&dir)?;
        let out = ctx.reserve(&dir, &it.stem(), ext)?;
        let mut a = av::sv(&["-i"]);
        a.push(it.path.clone());
        a.extend(av::sv(&["-map", "0:v:0", "-map", "0:a?", "-vf"]));
        a.push(chain.to_string());
        a.extend(av::sv(&["-c:v", "libx264", "-preset", "medium", "-crf"]));
        a.push(q.to_string());
        a.extend(av::sv(&["-pix_fmt", "yuv420p"]));
        if info.has_audio {
            a.extend(av::sv(&["-c:a", "aac", "-b:a", "192k"]));
        }
        if ext == "mp4" {
            a.extend(av::sv(&["-movflags", "+faststart"]));
        }
        a.push(av::ps(&out));
        av::ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}
