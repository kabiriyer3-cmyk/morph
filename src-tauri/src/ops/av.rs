//! Extended FFmpeg toolkit: universal video and audio conversion plus video nodes
//! (split, effects, filters, watermark, subtitles, reframe, thumbnails, slideshow, mux, compress, info).
use super::{split_kinds, Outs};
use crate::engine::{Ctx, FileItem};
use crate::ops::media::{ffprobe_json, media_info};
use crate::util::{command, parse_color, parse_time, sanitize, Params};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;

pub fn sv(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

pub fn ps(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn one_of(v: &str, list: &[&str]) -> bool {
    list.contains(&v)
}

pub fn ffcolor(s: &str) -> String {
    let [r, g, b] = parse_color(s);
    format!("0x{:02X}{:02X}{:02X}", r, g, b)
}

/// Runs ffmpeg with `-progress` parsing (optionally inside `cwd`); `on` receives 0..1.
pub fn ff(ctx: &Ctx, args: &[String], duration: f64, on: &dyn Fn(f64), cwd: Option<&Path>) -> Result<()> {
    let bin = ctx.tools.ffmpeg()?;
    let mut cmd = command(bin);
    cmd.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-progress", "pipe:1", "-nostats"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let mut child = cmd.spawn().context("failed to start ffmpeg")?;
    let stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if ctx.cancelled() {
                let _ = child.kill();
                break;
            }
            if let Some(v) = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms=")) {
                if let Ok(us) = v.trim().parse::<f64>() {
                    if duration > 0.0 {
                        on((us / 1_000_000.0 / duration).clamp(0.0, 1.0));
                    }
                }
            }
        }
    }
    let status = child.wait()?;
    let err = err_thread.join().unwrap_or_default();
    ctx.check()?;
    if !status.success() {
        let tail: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).collect();
        let msg = tail[tail.len().saturating_sub(4)..].join(" | ");
        bail!("ffmpeg failed: {}", if msg.is_empty() { format!("exit code {:?}", status.code()) } else { msg });
    }
    Ok(())
}

/// Runs ffmpeg and returns its log (for analysis filters such as silencedetect).
pub fn ff_log(ctx: &Ctx, args: &[String]) -> Result<String> {
    let bin = ctx.tools.ffmpeg()?;
    let out = command(bin)
        .args(["-hide_banner", "-nostdin", "-y"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("failed to start ffmpeg")?;
    ctx.check()?;
    Ok(String::from_utf8_lossy(&out.stderr).into_owned())
}

/// Sequential per-file loop for FFmpeg nodes (FFmpeg is multi-threaded itself).
pub fn each<F>(ctx: &Ctx, files: Vec<FileItem>, kinds: &[&str], f: F) -> Result<Vec<FileItem>>
where
    F: Fn(&FileItem, &dyn Fn(f64)) -> Result<Vec<FileItem>>,
{
    let (work, pass) = split_kinds(files, kinds);
    if work.is_empty() {
        if !pass.is_empty() {
            ctx.warn(format!("No {} files in the input: {} file(s) passed through", kinds.join("/"), pass.len()));
        }
        return Ok(pass);
    }
    ctx.tools.ffmpeg()?;
    let total = work.len();
    let mut out = vec![];
    for (i, it) in work.iter().enumerate() {
        ctx.check()?;
        let on = |frac: f64| ctx.report((i as f64 + frac) / total as f64, &it.name);
        match f(it, &on) {
            Ok(v) => out.extend(v),
            Err(e) => {
                if ctx.cancelled() {
                    return Err(e);
                }
                ctx.warn(format!("{}: {:#}", it.name, e));
            }
        }
        ctx.progress(i + 1, total, &it.name);
    }
    if out.is_empty() {
        bail!("All {} file(s) failed. {}", total, ctx.first_warning());
    }
    out.extend(pass);
    Ok(out)
}

const PRESETS: [&str; 9] = ["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"];

fn preset_num(preset: &str, table: [i32; 9]) -> i32 {
    PRESETS.iter().position(|n| *n == preset).map(|i| table[i]).unwrap_or(table[5])
}

fn push_rate(v: &mut Vec<String>, kbps: f64) {
    v.push("-b:v".into());
    v.push(format!("{}k", kbps.round()));
    v.push("-maxrate".into());
    v.push(format!("{}k", (kbps * 1.5).round()));
    v.push("-bufsize".into());
    v.push(format!("{}k", (kbps * 2.0).round()));
}

/// Default video codec for a container.
pub fn auto_vcodec(cont: &str) -> &'static str {
    match cont {
        "webm" => "vp9",
        "avi" | "divx" => "mpeg4",
        "wmv" | "asf" => "wmv2",
        "mpg" | "mpeg" | "vob" | "mxf" => "mpeg2",
        "ogv" => "theora",
        _ => "h264",
    }
}

/// Default audio codec for a container.
pub fn auto_acodec(cont: &str) -> &'static str {
    match cont {
        "webm" => "opus",
        "ogv" => "vorbis",
        "avi" | "divx" => "mp3",
        "wmv" | "asf" => "wma",
        "mpg" | "mpeg" | "vob" => "mp2",
        "mxf" => "pcm",
        _ => "aac",
    }
}

/// Resolves `auto` codecs and fixes combinations the container can't hold.
pub fn fit_codecs(cont: &str, vc: &str, ac: &str) -> (String, String, String) {
    let mut cont = cont.to_string();
    let mut vc = if vc == "auto" || vc.is_empty() { auto_vcodec(&cont).to_string() } else { vc.to_string() };
    let mut ac = if ac == "auto" || ac.is_empty() { auto_acodec(&cont).to_string() } else { ac.to_string() };
    if one_of(&vc, &["prores", "dnxhd"]) && !one_of(&cont, &["mov", "mkv", "mxf"]) {
        cont = "mov".into();
    }
    match cont.as_str() {
        "webm" => {
            if !one_of(&vc, &["vp9", "vp8", "av1", "copy"]) {
                vc = "vp9".into();
            }
            if !one_of(&ac, &["opus", "vorbis", "none", "copy"]) {
                ac = "opus".into();
            }
        }
        "ogv" => {
            if !one_of(&vc, &["theora", "vp8", "copy"]) {
                vc = "theora".into();
            }
            if !one_of(&ac, &["vorbis", "opus", "flac", "none", "copy"]) {
                ac = "vorbis".into();
            }
        }
        "mp4" | "m4v" | "mov" | "3gp" | "3g2" => {
            if one_of(&vc, &["wmv2", "theora", "vp8", "ffv1"]) {
                vc = "h264".into();
            }
            if one_of(&ac, &["wma", "vorbis", "mp2", "pcm"]) && cont != "mov" {
                ac = "aac".into();
            }
        }
        "flv" => {
            if !one_of(&vc, &["h264", "copy"]) {
                vc = "h264".into();
            }
            if !one_of(&ac, &["aac", "mp3", "none", "copy"]) {
                ac = "aac".into();
            }
        }
        "wmv" | "asf" => {
            if !one_of(&vc, &["wmv2", "h264", "mpeg4", "copy"]) {
                vc = "wmv2".into();
            }
            if !one_of(&ac, &["wma", "mp3", "none", "copy"]) {
                ac = "wma".into();
            }
        }
        "mpg" | "mpeg" | "vob" => {
            if !one_of(&vc, &["mpeg2", "mpeg1", "copy"]) {
                vc = "mpeg2".into();
            }
            if !one_of(&ac, &["mp2", "mp3", "ac3", "none", "copy"]) {
                ac = "mp2".into();
            }
        }
        "avi" | "divx" => {
            if one_of(&vc, &["vp9", "av1", "theora"]) {
                vc = "mpeg4".into();
            }
            if one_of(&ac, &["opus", "vorbis"]) {
                ac = "mp3".into();
            }
        }
        _ => {}
    }
    (cont, vc, ac)
}

pub fn vcodec_args(codec: &str, hw: &str, q: i64, preset: &str, kbps: Option<f64>, cont: &str, tune: &str) -> Vec<String> {
    let enc = match (codec, hw) {
        ("h264", "nvidia") => "h264_nvenc",
        ("h264", "intel") => "h264_qsv",
        ("h264", "amd") => "h264_amf",
        ("h265", "nvidia") => "hevc_nvenc",
        ("h265", "intel") => "hevc_qsv",
        ("h265", "amd") => "hevc_amf",
        ("h265", _) => "libx265",
        ("av1", "nvidia") => "av1_nvenc",
        ("av1", "intel") => "av1_qsv",
        ("av1", "amd") => "av1_amf",
        ("av1", _) => "libsvtav1",
        ("vp9", _) => "libvpx-vp9",
        ("vp8", _) => "libvpx",
        ("prores", _) => "prores_ks",
        ("dnxhd", _) => "dnxhd",
        ("mpeg4", _) => "mpeg4",
        ("mpeg2", _) => "mpeg2video",
        ("mpeg1", _) => "mpeg1video",
        ("wmv2", _) => "wmv2",
        ("theora", _) => "libtheora",
        ("mjpeg", _) => "mjpeg",
        ("ffv1", _) => "ffv1",
        _ => "libx264",
    };
    let mp4ish = one_of(cont, &["mp4", "mov", "m4v", "3gp"]);
    let qs = ((q as f64 - 10.0) * 0.6).round().clamp(2.0, 31.0) as i64;
    let mut v = sv(&["-c:v", enc]);
    match enc {
        "libx264" | "libx265" => {
            v.push("-preset".into());
            v.push(if PRESETS.contains(&preset) { preset.to_string() } else { "medium".into() });
            let tune_ok = if enc == "libx264" {
                one_of(tune, &["film", "animation", "grain", "stillimage", "fastdecode", "zerolatency"])
            } else {
                one_of(tune, &["animation", "grain", "fastdecode", "zerolatency"])
            };
            if tune_ok {
                v.push("-tune".into());
                v.push(tune.to_string());
            }
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.push("-crf".into());
                    v.push(q.clamp(0, 51).to_string());
                }
            }
            if enc == "libx265" && mp4ish {
                v.extend(sv(&["-tag:v", "hvc1"]));
            }
            v.extend(sv(&["-pix_fmt", "yuv420p"]));
        }
        "libsvtav1" => {
            v.push("-preset".into());
            v.push(preset_num(preset, [12, 11, 10, 9, 8, 6, 4, 3, 2]).to_string());
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.push("-crf".into());
                    v.push((q + 8).clamp(0, 63).to_string());
                }
            }
            v.extend(sv(&["-pix_fmt", "yuv420p"]));
        }
        "libvpx-vp9" | "libvpx" => {
            v.extend(sv(&["-deadline", "good", "-cpu-used"]));
            v.push(preset_num(preset, [8, 7, 6, 5, 4, 2, 1, 1, 0]).to_string());
            if enc == "libvpx-vp9" {
                v.extend(sv(&["-row-mt", "1"]));
            }
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.push("-crf".into());
                    v.push((q + 8).clamp(4, 63).to_string());
                    v.extend(sv(&["-b:v", if enc == "libvpx" { "20M" } else { "0" }]));
                }
            }
            v.extend(sv(&["-pix_fmt", "yuv420p"]));
        }
        "prores_ks" => v.extend(sv(&["-profile:v", "3", "-pix_fmt", "yuv422p10le"])),
        "dnxhd" => v.extend(sv(&["-profile:v", "dnxhr_hq", "-pix_fmt", "yuv422p"])),
        "mpeg4" | "mpeg2video" | "mpeg1video" | "wmv2" | "mjpeg" => {
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.push("-q:v".into());
                    v.push(qs.to_string());
                }
            }
            if enc == "mpeg4" && one_of(cont, &["avi", "divx"]) {
                v.extend(sv(&["-vtag", "xvid"]));
            }
            v.extend(sv(&["-pix_fmt", if enc == "mjpeg" { "yuvj420p" } else { "yuv420p" }]));
        }
        "libtheora" => {
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.push("-q:v".into());
                    v.push(((10.0 - (q as f64 - 10.0) * 0.3).round().clamp(0.0, 10.0) as i64).to_string());
                }
            }
            v.extend(sv(&["-pix_fmt", "yuv420p"]));
        }
        "ffv1" => v.extend(sv(&["-level", "3", "-g", "1"])),
        e if e.ends_with("_nvenc") => {
            v.push("-preset".into());
            v.push(format!("p{}", preset_num(preset, [1, 2, 3, 3, 4, 5, 6, 7, 7])));
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.extend(sv(&["-rc", "vbr", "-cq"]));
                    v.push(q.clamp(0, 51).to_string());
                    v.extend(sv(&["-b:v", "0"]));
                }
            }
            if e == "hevc_nvenc" && mp4ish {
                v.extend(sv(&["-tag:v", "hvc1"]));
            }
            v.extend(sv(&["-pix_fmt", "yuv420p"]));
        }
        e if e.ends_with("_qsv") => {
            v.push("-preset".into());
            v.push(if PRESETS[2..].contains(&preset) { preset.to_string() } else { "veryfast".into() });
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.push("-global_quality".into());
                    v.push(q.clamp(1, 51).to_string());
                }
            }
        }
        e if e.ends_with("_amf") => {
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.extend(sv(&["-rc", "cqp", "-qp_i"]));
                    v.push(q.clamp(0, 51).to_string());
                    v.push("-qp_p".into());
                    v.push(q.clamp(0, 51).to_string());
                }
            }
            v.extend(sv(&["-quality", "balanced"]));
        }
        _ => {}
    }
    v
}

pub fn acodec_args(codec: &str, kbps: i64) -> Vec<String> {
    let b = format!("{kbps}k");
    match codec {
        "none" => sv(&["-an"]),
        "copy" => sv(&["-c:a", "copy"]),
        "opus" => sv(&["-c:a", "libopus", "-b:a", &b]),
        "mp3" => sv(&["-c:a", "libmp3lame", "-b:a", &b]),
        "vorbis" => sv(&["-c:a", "libvorbis", "-b:a", &b]),
        "flac" => sv(&["-c:a", "flac"]),
        "alac" => sv(&["-c:a", "alac"]),
        "pcm" => sv(&["-c:a", "pcm_s16le"]),
        "ac3" => {
            let k = format!("{}k", kbps.clamp(64, 640));
            sv(&["-c:a", "ac3", "-b:a", &k])
        }
        "mp2" => sv(&["-c:a", "mp2", "-b:a", &b]),
        "wma" => sv(&["-c:a", "wmav2", "-b:a", &b]),
        _ => sv(&["-c:a", "aac", "-b:a", &b]),
    }
}

/// (extension, codec args) for an audio target format.
pub fn audio_target(fmt: &str, kbps: i64, src_codec: &str, vbr: bool, bits: &str) -> (String, Vec<String>) {
    let b = format!("{kbps}k");
    let pcm = |be: bool| -> String {
        let base = match bits {
            "24" => "s24",
            "32" => "s32",
            "32f" => "f32",
            _ => "s16",
        };
        format!("pcm_{base}{}", if be { "be" } else { "le" })
    };
    let mp3q = match kbps {
        k if k >= 245 => 0,
        k if k >= 220 => 1,
        k if k >= 190 => 2,
        k if k >= 170 => 3,
        k if k >= 150 => 4,
        k if k >= 130 => 5,
        k if k >= 115 => 6,
        k if k >= 100 => 7,
        k if k >= 85 => 8,
        _ => 9,
    }
    .to_string();
    let ac3 = format!("{}k", kbps.clamp(64, 640));
    let (ext, args): (&str, Vec<String>) = match fmt {
        "m4a" => ("m4a", sv(&["-c:a", "aac", "-b:a", &b])),
        "aac" => ("aac", sv(&["-c:a", "aac", "-b:a", &b])),
        "m4r" => ("m4r", sv(&["-c:a", "aac", "-b:a", &b, "-f", "ipod"])),
        "m4b" => ("m4b", sv(&["-c:a", "aac", "-b:a", &b, "-f", "ipod"])),
        "opus" => ("opus", sv(&["-c:a", "libopus", "-b:a", &b])),
        "ogg" => ("ogg", sv(&["-c:a", "libvorbis", "-b:a", &b])),
        "flac" => ("flac", sv(&["-c:a", "flac", "-compression_level", "8"])),
        "wav" => ("wav", vec!["-c:a".to_string(), pcm(false)]),
        "aiff" | "aif" => ("aiff", vec!["-c:a".to_string(), pcm(true)]),
        "alac" => ("m4a", sv(&["-c:a", "alac"])),
        "wma" => ("wma", sv(&["-c:a", "wmav2", "-b:a", &b])),
        "ac3" => ("ac3", sv(&["-c:a", "ac3", "-b:a", &ac3])),
        "eac3" => ("eac3", sv(&["-c:a", "eac3", "-b:a", &ac3])),
        "mp2" => ("mp2", sv(&["-c:a", "mp2", "-b:a", &b])),
        "amr" => ("amr", sv(&["-c:a", "libopencore_amrnb", "-ar", "8000", "-ac", "1", "-b:a", "12.2k"])),
        "mka" => ("mka", sv(&["-c:a", "copy"])),
        "copy" => {
            let ext = match src_codec {
                "aac" | "alac" => "m4a",
                "mp3" => "mp3",
                "opus" => "opus",
                "vorbis" => "ogg",
                "flac" => "flac",
                "ac3" => "ac3",
                "wmav2" | "wmav1" => "wma",
                c if c.starts_with("pcm_") => "wav",
                _ => "mka",
            };
            (ext, sv(&["-c:a", "copy"]))
        }
        _ => {
            if vbr {
                ("mp3", sv(&["-c:a", "libmp3lame", "-q:a", &mp3q]))
            } else {
                ("mp3", sv(&["-c:a", "libmp3lame", "-b:a", &b]))
            }
        }
    };
    (ext.to_string(), args)
}

fn scale_expr(w: i64, h: i64, max: i64, fit: &str) -> Option<String> {
    if max > 0 {
        return Some(format!(
            "scale='if(gte(iw,ih),min({max},iw),-2)':'if(gte(iw,ih),-2,min({max},ih))',scale=trunc(iw/2)*2:trunc(ih/2)*2"
        ));
    }
    match (w > 0, h > 0) {
        (true, true) => Some(match fit {
            "crop" => format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}"),
            "stretch" => format!("scale={w}:{h},setsar=1"),
            "pad" => format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1"),
            _ => format!("scale={w}:{h}:force_original_aspect_ratio=decrease,scale=trunc(iw/2)*2:trunc(ih/2)*2"),
        }),
        (true, false) => Some(format!("scale={w}:-2")),
        (false, true) => Some(format!("scale=-2:{h}")),
        _ => None,
    }
}

fn is_sub_error(e: &anyhow::Error) -> bool {
    let m = format!("{:#}", e).to_lowercase();
    m.contains("subtitle") || m.contains("not currently supported in container") || m.contains("codec not supported")
}

pub fn video_convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let container = p.str("container", "mp4");
    let vcodec = p.str("video_codec", "auto");
    let target_mb = p.f64("target_mb", 0.0);
    let rate = if p.raw("rate_mode").is_none() && target_mb > 0.0 { "size".to_string() } else { p.str("rate_mode", "quality") };
    let q = p.i64("quality", 23);
    let vkbps = p.f64("video_bitrate", 4000.0);
    let two_pass = p.bool("two_pass", false);
    let preset = p.str("preset", "medium");
    let tune = p.str("tune", "none");
    let (w, h, max) = (p.i64("width", 0), p.i64("height", 0), p.i64("max_size", 0));
    let fit = p.str("fit", "fit");
    let fps = p.f64("fps", 0.0);
    let acodec = p.str("audio_codec", "auto");
    let abr = p.i64("audio_bitrate", 160).clamp(8, 1024);
    let ar = p.i64("sample_rate", 0);
    let ach = p.str("channels", "keep");
    let hw = p.str("hardware", "none");
    let subs = p.bool("keep_subtitles", true);
    let strip = p.bool("strip_metadata", false);
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let cont0 = if container == "keep" { it.ext.clone() } else { container.clone() };
        let (cont, vc, ac) = fit_codecs(&cont0, &vcodec, &acodec);
        let ac = if info.has_audio { ac } else { "none".to_string() };
        let out = ctx.out_path(it, &it.stem(), &cont)?;
        let passlog = ctx.work.join(format!("2pass-{}", sanitize(&it.stem())));
        let mut vf: Vec<String> = vec![];
        if let Some(s) = scale_expr(w, h, max, &fit) {
            vf.push(s);
        }
        if fps > 0.0 {
            vf.push(format!("fps={fps}"));
        }
        let audio_k = match ac.as_str() {
            "none" => 0.0,
            "copy" => 192.0,
            _ => abr as f64,
        };
        let kbps = match rate.as_str() {
            "bitrate" => Some(vkbps.max(50.0)),
            "size" if target_mb > 0.0 && info.duration > 0.0 => Some((target_mb * 8192.0 / info.duration - audio_k).max(50.0)),
            _ => None,
        };
        let mut video: Vec<String> = vec![];
        if vc == "copy" {
            if !vf.is_empty() {
                ctx.warn(format!("{}: resize/fps are ignored when the video stream is copied", it.name));
            }
            video.extend(sv(&["-c:v", "copy"]));
        } else {
            if !vf.is_empty() {
                video.push("-vf".into());
                video.push(vf.join(","));
            }
            video.extend(vcodec_args(&vc, &hw, q, &preset, kbps, &cont, &tune));
        }
        let mut audio = acodec_args(&ac, abr);
        if ac != "none" && ac != "copy" {
            if ar > 0 {
                audio.push("-ar".into());
                audio.push(ar.to_string());
            }
            match ach.as_str() {
                "mono" => audio.extend(sv(&["-ac", "1"])),
                "stereo" => audio.extend(sv(&["-ac", "2"])),
                "5.1" => audio.extend(sv(&["-ac", "6"])),
                _ => {}
            }
        }
        let sub_codec = if subs {
            match cont.as_str() {
                "mkv" => Some("copy"),
                "mp4" | "m4v" | "mov" => Some("mov_text"),
                "webm" => Some("webvtt"),
                _ => None,
            }
        } else {
            None
        };
        let build = |with_subs: bool, pass: u8| -> Vec<String> {
            let mut a = sv(&["-i"]);
            a.push(it.path.clone());
            a.extend(sv(&["-map", "0:v:0?"]));
            if pass != 1 && ac != "none" {
                a.extend(sv(&["-map", "0:a?"]));
            }
            if with_subs && pass != 1 {
                if let Some(sc) = sub_codec {
                    a.extend(sv(&["-map", "0:s?", "-c:s", sc]));
                }
            }
            a.extend(sv(&["-map_metadata", if strip { "-1" } else { "0" }]));
            if strip {
                a.extend(sv(&["-map_chapters", "-1"]));
            }
            a.extend(video.iter().cloned());
            if pass > 0 {
                a.push("-pass".into());
                a.push(pass.to_string());
                a.push("-passlogfile".into());
                a.push(ps(&passlog));
            }
            if pass == 1 {
                a.extend(sv(&["-an", "-f", "null", "-"]));
                return a;
            }
            a.extend(audio.iter().cloned());
            if one_of(&cont, &["mp4", "mov", "m4v", "3gp"]) {
                a.extend(sv(&["-movflags", "+faststart"]));
            }
            a.push(ps(&out));
            a
        };
        let use_2pass = two_pass && kbps.is_some() && one_of(&vc, &["h264", "vp9", "vp8", "mpeg4"]) && hw == "none";
        let run = |with_subs: bool| -> Result<()> {
            if use_2pass {
                ff(ctx, &build(false, 1), info.duration, &|f| on(f * 0.45), None)?;
                ff(ctx, &build(with_subs, 2), info.duration, &|f| on(0.45 + f * 0.55), None)
            } else {
                ff(ctx, &build(with_subs, 0), info.duration, on, None)
            }
        };
        if let Err(e) = run(sub_codec.is_some()) {
            if sub_codec.is_some() && !ctx.cancelled() && is_sub_error(&e) {
                ctx.warn(format!("{}: subtitles could not be kept and were dropped", it.name));
                run(false)?;
            } else {
                return Err(e);
            }
        }
        for suffix in ["-0.log", "-0.log.mbtree", "-0.log.temp"] {
            let _ = std::fs::remove_file(format!("{}{}", ps(&passlog), suffix));
        }
        Ok(vec![it.derive(&out)])
    })
}

pub fn audio_convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "mp3");
    let kbps = p.i64("bitrate", 192).clamp(8, 1024);
    let sr = p.i64("sample_rate", 0);
    let ch = p.str("channels", "keep");
    let vbr = p.bool("vbr", false);
    let bits = p.str("bit_depth", "16");
    let cover = p.bool("keep_cover", true);
    let strip = p.bool("strip_metadata", false);
    each(ctx, files, &["audio", "video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if !info.has_audio {
            bail!("no audio track");
        }
        let (ext, codec) = audio_target(&fmt, kbps, &info.audio_codec, vbr, &bits);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        let with_cover = cover && !strip && it.kind == "audio" && one_of(&ext, &["mp3", "m4a", "flac", "m4b"]);
        let ringtone_cut = ext == "m4r" && info.duration > 40.0;
        let build = |cov: bool| -> Vec<String> {
            let mut a = sv(&["-i"]);
            a.push(it.path.clone());
            a.extend(sv(&["-map", "0:a:0"]));
            if cov {
                a.extend(sv(&["-map", "0:v?", "-c:v", "copy", "-disposition:v", "attached_pic"]));
            } else {
                a.push("-vn".into());
            }
            a.extend(sv(&["-map_metadata", if strip { "-1" } else { "0" }]));
            a.extend(codec.iter().cloned());
            if sr > 0 && !codec.iter().any(|x| x == "-ar") {
                a.push("-ar".into());
                a.push(sr.to_string());
            }
            if !codec.iter().any(|x| x == "-ac") {
                match ch.as_str() {
                    "mono" => a.extend(sv(&["-ac", "1"])),
                    "stereo" => a.extend(sv(&["-ac", "2"])),
                    _ => {}
                }
            }
            if ext == "mp3" {
                a.extend(sv(&["-id3v2_version", "3"]));
            }
            if ringtone_cut {
                a.extend(sv(&["-t", "40"]));
            }
            a.push(ps(&out));
            a
        };
        if ringtone_cut {
            ctx.warn(format!("{}: ringtones are limited to 40 s, the rest was cut", it.name));
        }
        match ff(ctx, &build(with_cover), info.duration, on, None) {
            Ok(()) => {}
            Err(_) if with_cover && !ctx.cancelled() => ff(ctx, &build(false), info.duration, on, None)?,
            Err(e) => return Err(e),
        }
        Ok(vec![it.derive(&out)])
    })
}

/// Output extension and codec args for nodes that re-encode video through filters.
fn reencode(it: &FileItem, q: i64, has_audio: bool, audio_filtered: bool) -> (String, Vec<String>) {
    let ext = match it.ext.as_str() {
        "mkv" | "mov" | "webm" | "mp4" | "m4v" => it.ext.clone(),
        _ => "mp4".to_string(),
    };
    let mut v = if ext == "webm" {
        let mut v = sv(&["-c:v", "libvpx-vp9", "-row-mt", "1", "-deadline", "good", "-cpu-used", "4", "-crf"]);
        v.push((q + 10).clamp(4, 63).to_string());
        v.extend(sv(&["-b:v", "0"]));
        v
    } else {
        let mut v = sv(&["-c:v", "libx264", "-preset", "medium", "-crf"]);
        v.push(q.clamp(0, 51).to_string());
        v
    };
    v.extend(sv(&["-pix_fmt", "yuv420p"]));
    if has_audio {
        match ext.as_str() {
            "webm" => v.extend(sv(&["-c:a", "libopus", "-b:a", "160k"])),
            "mkv" | "mov" if !audio_filtered => v.extend(sv(&["-c:a", "copy"])),
            _ => v.extend(sv(&["-c:a", "aac", "-b:a", "192k"])),
        }
    }
    if one_of(&ext, &["mp4", "m4v", "mov"]) {
        v.extend(sv(&["-movflags", "+faststart"]));
    }
    (ext, v)
}

fn place(pos: &str, m: i64) -> (String, String) {
    let m = m.max(0);
    let (x, y): (String, String) = match pos {
        "top-left" => (format!("{m}"), format!("{m}")),
        "top" => ("(W-w)/2".into(), format!("{m}")),
        "top-right" => (format!("W-w-{m}"), format!("{m}")),
        "left" => (format!("{m}"), "(H-h)/2".into()),
        "center" => ("(W-w)/2".into(), "(H-h)/2".into()),
        "right" => (format!("W-w-{m}"), "(H-h)/2".into()),
        "bottom-left" => (format!("{m}"), format!("H-h-{m}")),
        "bottom" => ("(W-w)/2".into(), format!("H-h-{m}")),
        _ => (format!("W-w-{m}"), format!("H-h-{m}")),
    };
    (x, y)
}

fn parse_rate(s: &str) -> f64 {
    let mut it = s.split('/');
    let a: f64 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0.0);
    let b: f64 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(1.0);
    if b == 0.0 {
        0.0
    } else {
        a / b
    }
}

pub fn video_split(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "parts");
    let parts = p.i64("parts", 2).clamp(2, 1000) as usize;
    let seg = parse_time(&p.str("duration", "60")).unwrap_or(60.0).max(0.5);
    let times = p.str("times", "");
    let reencode_on = p.bool("reencode", false);
    each(ctx, files, &["video", "audio"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if info.duration <= 0.0 {
            bail!("unknown duration");
        }
        let d = info.duration;
        let mut cuts: Vec<f64> = match mode.as_str() {
            "duration" => (1..100_000).map(|i| i as f64 * seg).take_while(|t| *t < d - 0.05).collect(),
            "times" => times.split([',', ';', ' ', '\n']).filter_map(parse_time).filter(|t| *t > 0.0 && *t < d).collect(),
            _ => (1..parts).map(|i| d * i as f64 / parts as f64).collect(),
        };
        cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        cuts.dedup_by(|a, b| (*a - *b).abs() < 0.05);
        let mut bounds = vec![0.0];
        bounds.extend(cuts);
        bounds.push(d);
        let n = bounds.len() - 1;
        let width = n.to_string().len().max(2);
        let mut out = vec![];
        for i in 0..n {
            ctx.check()?;
            let (s, e) = (bounds[i], bounds[i + 1]);
            if e - s < 0.05 {
                continue;
            }
            let path = ctx.out_path(it, &format!("{}_{:0w$}", it.stem(), i + 1, w = width), &it.ext)?;
            let mut a = vec!["-ss".to_string(), format!("{s:.3}"), "-i".into(), it.path.clone(), "-t".into(), format!("{:.3}", e - s)];
            if reencode_on {
                if it.kind == "video" {
                    a.extend(sv(&["-map", "0:v:0?", "-map", "0:a?", "-c:v", "libx264", "-preset", "fast", "-crf", "20", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k"]));
                }
            } else {
                a.extend(sv(&["-map", "0", "-c", "copy", "-avoid_negative_ts", "make_zero"]));
            }
            a.push(ps(&path));
            ff(ctx, &a, e - s, &|f| on((i as f64 + f) / n as f64), None)?;
            out.push(it.derive(&path));
        }
        Ok(out)
    })
}

pub fn video_fx(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fade_in = p.f64("fade_in", 0.0).max(0.0);
    let fade_out = p.f64("fade_out", 0.0).max(0.0);
    let color = if p.str("fade_color", "black") == "white" { "white" } else { "black" };
    let reverse = p.bool("reverse", false);
    let boomerang = p.bool("boomerang", false);
    let loops = p.i64("loop", 1).clamp(1, 100);
    let q = p.i64("quality", 20);
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let total = info.duration * loops as f64 * if boomerang { 2.0 } else { 1.0 };
        let mut a: Vec<String> = vec![];
        if loops > 1 {
            a.push("-stream_loop".into());
            a.push((loops - 1).to_string());
        }
        a.push("-i".into());
        a.push(it.path.clone());
        let mut parts: Vec<String> = vec![];
        let mut vl = "0:v".to_string();
        let mut al: Option<String> = if info.has_audio { Some("0:a".into()) } else { None };
        if reverse {
            parts.push(format!("[{vl}]reverse[vr]"));
            vl = "vr".into();
            if let Some(x) = al.clone() {
                parts.push(format!("[{x}]areverse[ar]"));
                al = Some("ar".into());
            }
        }
        if boomerang {
            parts.push(format!("[{vl}]split[vb1][vb2];[vb2]reverse[vb3];[vb1][vb3]concat=n=2:v=1:a=0[vb]"));
            vl = "vb".into();
            if let Some(x) = al.clone() {
                parts.push(format!("[{x}]asplit[ab1][ab2];[ab2]areverse[ab3];[ab1][ab3]concat=n=2:v=0:a=1[ab]"));
                al = Some("ab".into());
            }
        }
        let mut vfx = vec![];
        let mut afx = vec![];
        if fade_in > 0.0 {
            vfx.push(format!("fade=t=in:st=0:d={fade_in}:color={color}"));
            afx.push(format!("afade=t=in:st=0:d={fade_in}"));
        }
        if fade_out > 0.0 && total > fade_out {
            let st = total - fade_out;
            vfx.push(format!("fade=t=out:st={st:.3}:d={fade_out}:color={color}"));
            afx.push(format!("afade=t=out:st={st:.3}:d={fade_out}"));
        }
        if !vfx.is_empty() {
            parts.push(format!("[{vl}]{}[vf]", vfx.join(",")));
            vl = "vf".into();
        }
        if let Some(x) = al.clone() {
            if !afx.is_empty() {
                parts.push(format!("[{x}]{}[af]", afx.join(",")));
                al = Some("af".into());
            }
        }
        if parts.is_empty() && loops == 1 {
            bail!("nothing to do: enable an effect");
        }
        let audio_filtered = al.as_deref().map(|x| x != "0:a").unwrap_or(false);
        if !parts.is_empty() {
            a.push("-filter_complex".into());
            a.push(parts.join(";"));
        }
        a.push("-map".into());
        a.push(if vl == "0:v" { "0:v:0".into() } else { format!("[{vl}]") });
        if let Some(x) = &al {
            a.push("-map".into());
            a.push(if x == "0:a" { "0:a:0".into() } else { format!("[{x}]") });
        }
        let (ext, codec) = reencode(it, q, al.is_some(), audio_filtered);
        a.extend(codec);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        a.push(ps(&out));
        ff(ctx, &a, total, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_filters(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mut vf: Vec<String> = vec![];
    if p.bool("deinterlace", false) {
        vf.push("yadif".into());
    }
    match p.str("denoise", "off").as_str() {
        "light" => vf.push("hqdn3d=2:1.5:3:2.25".into()),
        "medium" => vf.push("hqdn3d=4:3:6:4.5".into()),
        "strong" => vf.push("hqdn3d=8:6:12:9".into()),
        _ => {}
    }
    if p.bool("stabilize", false) {
        vf.push("deshake".into());
    }
    let b = p.f64("brightness", 0.0).clamp(-1.0, 1.0);
    let c = p.f64("contrast", 1.0).clamp(0.0, 3.0);
    let s = p.f64("saturation", 1.0).clamp(0.0, 3.0);
    let g = p.f64("gamma", 1.0).clamp(0.1, 10.0);
    if b != 0.0 || c != 1.0 || s != 1.0 || g != 1.0 {
        vf.push(format!("eq=brightness={b}:contrast={c}:saturation={s}:gamma={g}"));
    }
    let hue = p.f64("hue", 0.0);
    if hue != 0.0 {
        vf.push(format!("hue=h={hue}"));
    }
    let temp = p.f64("temperature", 6500.0);
    if (temp - 6500.0).abs() > 1.0 {
        vf.push(format!("colortemperature=temperature={}", temp.clamp(1000.0, 40000.0)));
    }
    match p.str("look", "none").as_str() {
        "grayscale" => vf.push("hue=s=0".into()),
        "sepia" => vf.push("colorchannelmixer=.393:.769:.189:0:.349:.686:.168:0:.272:.534:.131".into()),
        "invert" => vf.push("negate".into()),
        l @ ("vintage" | "cross_process" | "strong_contrast" | "lighter" | "darker" | "increase_contrast" | "negative") => {
            vf.push(format!("curves=preset={l}"))
        }
        _ => {}
    }
    let sharpen = p.f64("sharpen", 0.0).clamp(0.0, 3.0);
    if sharpen > 0.0 {
        vf.push(format!("unsharp=5:5:{sharpen}:5:5:0"));
    }
    let blur = p.f64("blur", 0.0).clamp(0.0, 50.0);
    if blur > 0.0 {
        vf.push(format!("gblur=sigma={blur}"));
    }
    if p.bool("vignette", false) {
        vf.push("vignette=PI/5".into());
    }
    if p.bool("grain", false) {
        vf.push("noise=alls=12:allf=t+u".into());
    }
    let q = p.i64("quality", 20);
    if vf.is_empty() {
        bail!("No filter is enabled");
    }
    let chain = vf.join(",");
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let (ext, codec) = reencode(it, q, info.has_audio, false);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        a.extend(sv(&["-map", "0:v:0", "-map", "0:a?", "-vf"]));
        a.push(chain.clone());
        a.extend(codec);
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_framerate(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fps = p.f64("fps", 60.0).clamp(1.0, 240.0);
    let mode = p.str("mode", "blend");
    let q = p.i64("quality", 20);
    let filter = match mode.as_str() {
        "drop" => format!("fps={fps}"),
        "interpolate" => format!("minterpolate=fps={fps}:mi_mode=mci:mc_mode=aobmc:me_mode=bidir:vsbmc=1"),
        _ => format!("framerate=fps={fps}"),
    };
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let (ext, codec) = reencode(it, q, info.has_audio, false);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        a.extend(sv(&["-map", "0:v:0", "-map", "0:a?", "-vf"]));
        a.push(filter.clone());
        a.extend(codec);
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_watermark(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "text");
    let text = p.str("text", "\u{a9} Morph");
    let size = p.f64("size", 5.0).clamp(0.5, 100.0);
    let color = p.str("color", "#ffffff");
    let opacity = (p.f64("opacity", 80.0) / 100.0).clamp(0.0, 1.0);
    let pos = p.str("position", "bottom-right");
    let margin = p.f64("margin", 3.0).clamp(0.0, 50.0);
    let image = p.str("image", "");
    let font = p.str("font", "sans-serif");
    let bold = p.bool("bold", true);
    let shadow = p.bool("shadow", true);
    let q = p.i64("quality", 20);
    if mode == "image" && !Path::new(image.trim()).is_file() {
        bail!("Pick the watermark image (logo) in the node");
    }
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let (w, h) = (info.width.max(16) as f64, info.height.max(16) as f64);
        let m = (margin / 100.0 * w.min(h)).round() as i64;
        let (x, y) = place(&pos, m);
        let (wm_path, scale) = if mode == "image" {
            let tw = ((w * size / 100.0).round() as i64).max(2);
            (PathBuf::from(image.trim()), format!("scale={tw}:-1,"))
        } else {
            let px = (h * size / 100.0).max(8.0) as f32;
            let img = crate::ops::img2::text_image(&text, px, &color, &font, bold, shadow, None)?;
            let path = ctx.reserve(&ctx.out_dir("")?, &format!("_wm_{}", sanitize(&it.stem())), "png")?;
            img.save(&path)?;
            (path, String::new())
        };
        let fc = format!("[1:v]{scale}format=rgba,colorchannelmixer=aa={opacity:.3}[wm];[0:v][wm]overlay={x}:{y}:format=auto[v]");
        let (ext, codec) = reencode(it, q, info.has_audio, false);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        a.push("-i".into());
        a.push(ps(&wm_path));
        a.push("-filter_complex".into());
        a.push(fc);
        a.extend(sv(&["-map", "[v]", "-map", "0:a?"]));
        a.extend(codec);
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

fn find_subtitle(it: &FileItem, custom: &str) -> Option<PathBuf> {
    let c = custom.trim().trim_matches('"');
    if !c.is_empty() {
        let p = PathBuf::from(c);
        return if p.is_file() { Some(p) } else { None };
    }
    for base in [Path::new(&it.origin), Path::new(&it.path)] {
        for ext in ["srt", "ass", "ssa", "vtt"] {
            let q = base.with_extension(ext);
            if q.is_file() {
                return Some(q);
            }
        }
    }
    None
}

pub fn video_subtitles(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "burn");
    let custom = p.str("file", "");
    let size = p.i64("font_size", 0).clamp(0, 200);
    let color = p.str("color", "#ffffff");
    let lang = p.str("language", "");
    let fmt = p.str("format", "srt");
    let index = p.i64("stream", 1).max(1) - 1;
    let q = p.i64("quality", 20);
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if mode == "extract" {
            let (ext, codec) = match fmt.as_str() {
                "ass" => ("ass", "ass"),
                "vtt" => ("vtt", "webvtt"),
                _ => ("srt", "srt"),
            };
            let out = ctx.out_path(it, &it.stem(), ext)?;
            let mut a = sv(&["-i"]);
            a.push(it.path.clone());
            a.push("-map".into());
            a.push(format!("0:s:{index}"));
            a.extend(sv(&["-c:s", codec]));
            a.push(ps(&out));
            ff(ctx, &a, info.duration, on, None).context("no such subtitle track (or it is a picture-based track)")?;
            return Ok(vec![it.derive(&out)]);
        }
        let sub = find_subtitle(it, &custom).ok_or_else(|| {
            anyhow::anyhow!("no subtitle file: pick one in the node or put a .srt/.ass/.vtt with the same name next to the video")
        })?;
        let sub_ext = crate::kinds::ext_of(&sub);
        if mode == "add" {
            let ext = match it.ext.as_str() {
                "mp4" | "m4v" | "mov" | "webm" | "mkv" => it.ext.clone(),
                _ => "mkv".to_string(),
            };
            let codec = match ext.as_str() {
                "mp4" | "m4v" | "mov" => "mov_text",
                "webm" => "webvtt",
                _ => {
                    if sub_ext == "ass" || sub_ext == "ssa" {
                        "ass"
                    } else {
                        "srt"
                    }
                }
            };
            let existing = ffprobe_json(ctx.tools, &it.path_buf())
                .ok()
                .and_then(|j| j["streams"].as_array().map(|s| s.iter().filter(|x| x["codec_type"] == "subtitle").count()))
                .unwrap_or(0);
            let out = ctx.out_path(it, &it.stem(), &ext)?;
            let mut a = sv(&["-i"]);
            a.push(it.path.clone());
            a.push("-i".into());
            a.push(ps(&sub));
            a.extend(sv(&["-map", "0", "-map", "1:0", "-c", "copy", "-c:s", codec]));
            if !lang.trim().is_empty() {
                a.push(format!("-metadata:s:s:{existing}"));
                a.push(format!("language={}", lang.trim()));
            }
            a.push(format!("-disposition:s:{existing}"));
            a.push("default".into());
            a.push(ps(&out));
            ff(ctx, &a, info.duration, on, None)?;
            return Ok(vec![it.derive(&out)]);
        }
        let dir = ctx.out_dir("")?;
        let local = format!("_sub_{}.{}", sanitize(&it.stem()).replace([' ', '\'', ',', ';', '[', ']', '='], "_"), sub_ext);
        std::fs::copy(&sub, dir.join(&local))?;
        let mut filter = format!("subtitles={local}");
        if size > 0 || color.to_lowercase() != "#ffffff" {
            let [r, g, b] = parse_color(&color);
            let mut style = vec![format!("PrimaryColour=&H00{:02X}{:02X}{:02X}", b, g, r)];
            if size > 0 {
                style.push(format!("FontSize={size}"));
            }
            filter.push_str(&format!(":force_style='{}'", style.join(",")));
        }
        let (ext, codec) = reencode(it, q, info.has_audio, false);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        a.extend(sv(&["-map", "0:v:0", "-map", "0:a?", "-vf"]));
        a.push(filter);
        a.extend(codec);
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, Some(&dir))?;
        let _ = std::fs::remove_file(dir.join(&local));
        Ok(vec![it.derive(&out)])
    })
}

fn even(v: f64) -> i64 {
    ((v / 2.0).round() as i64 * 2).max(2)
}

pub fn video_reframe(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let aspect = p.str("aspect", "9:16");
    let mode = p.str("mode", "blur");
    let width = p.i64("width", 0).max(0);
    let bg = ffcolor(&p.str("color", "#000000"));
    let q = p.i64("quality", 20);
    let (aw, ah) = aspect
        .split_once(':')
        .and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)))
        .filter(|(a, b)| *a > 0.0 && *b > 0.0)
        .unwrap_or((9.0, 16.0));
    each(ctx, files, &["video", "image"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let (sw, sh) = (info.width.max(2) as f64, info.height.max(2) as f64);
        let (tw, th) = if width > 0 {
            (even(width as f64), even(width as f64 * ah / aw))
        } else if sw / sh > aw / ah {
            (even(sw), even(sw * ah / aw))
        } else {
            (even(sh * aw / ah), even(sh))
        };
        let fc = match mode.as_str() {
            "crop" => format!("[0:v]scale={tw}:{th}:force_original_aspect_ratio=increase,crop={tw}:{th},setsar=1[v]"),
            "color" => format!("[0:v]scale={tw}:{th}:force_original_aspect_ratio=decrease,pad={tw}:{th}:(ow-iw)/2:(oh-ih)/2:color={bg},setsar=1[v]"),
            _ => format!("[0:v]split[a][b];[a]scale={tw}:{th}:force_original_aspect_ratio=increase,crop={tw}:{th},gblur=sigma=40,eq=brightness=-0.06[bg];[b]scale={tw}:{th}:force_original_aspect_ratio=decrease[fg];[bg][fg]overlay=(W-w)/2:(H-h)/2,setsar=1[v]"),
        };
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        a.push("-filter_complex".into());
        a.push(fc);
        a.extend(sv(&["-map", "[v]"]));
        let out = if it.kind == "image" {
            let ext = if it.ext == "png" { "png" } else { "jpg" };
            let out = ctx.out_path(it, &it.stem(), ext)?;
            a.extend(sv(&["-frames:v", "1", "-q:v", "2"]));
            out
        } else {
            a.extend(sv(&["-map", "0:a?"]));
            let (ext, codec) = reencode(it, q, info.has_audio, false);
            a.extend(codec);
            ctx.out_path(it, &it.stem(), &ext)?
        };
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

/// Grabs one frame at `pos` (0..1 of the duration).
pub fn grab_frame(ctx: &Ctx, it: &FileItem, ext: &str, pos: f64, width: i64) -> Result<FileItem> {
    let info = media_info(ctx.tools, &it.path_buf());
    let t = (info.duration * pos.clamp(0.0, 1.0)).max(0.0);
    let out = ctx.out_path(it, &it.stem(), ext)?;
    let mut a = vec!["-ss".to_string(), format!("{t:.3}"), "-i".into(), it.path.clone(), "-frames:v".into(), "1".into()];
    if width > 0 {
        a.push("-vf".into());
        a.push(format!("scale={width}:-2"));
    }
    if ext == "jpg" || ext == "jpeg" {
        a.extend(sv(&["-q:v", "2"]));
    }
    a.push(ps(&out));
    ff(ctx, &a, 0.0, &|_| {}, None)?;
    if !out.is_file() {
        bail!("no frame could be read");
    }
    Ok(it.derive(&out))
}

pub fn video_thumbs(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "sheet");
    let cols = p.i64("columns", 4).clamp(1, 20);
    let rows = p.i64("rows", 4).clamp(1, 30);
    let count = p.i64("count", 8).clamp(1, 500);
    let tw = p.i64("width", 320).clamp(32, 4096);
    let fmt = p.str("format", "jpg");
    let bg = ffcolor(&p.str("background", "#111111"));
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if info.duration <= 0.0 {
            bail!("unknown duration");
        }
        let d = info.duration;
        if mode == "images" {
            let mut out = vec![];
            let width = count.to_string().len().max(2);
            for i in 0..count {
                ctx.check()?;
                let t = d * (i as f64 + 0.5) / count as f64;
                let path = ctx.out_path(it, &format!("{}_{:0w$}", it.stem(), i + 1, w = width as usize), &fmt)?;
                let mut a = vec!["-ss".to_string(), format!("{t:.3}"), "-i".into(), it.path.clone(), "-frames:v".into(), "1".into(), "-vf".into(), format!("scale={tw}:-2")];
                if fmt == "jpg" {
                    a.extend(sv(&["-q:v", "2"]));
                }
                a.push(ps(&path));
                ff(ctx, &a, 0.0, &|_| {}, None)?;
                on((i + 1) as f64 / count as f64);
                out.push(it.derive(&path));
            }
            return Ok(out);
        }
        let n = cols * rows;
        let start = d * 0.02;
        let span = (d * 0.96).max(0.1);
        let out = ctx.out_path(it, &format!("{}_sheet", it.stem()), &fmt)?;
        let mut a = vec!["-ss".to_string(), format!("{start:.3}"), "-t".into(), format!("{span:.3}"), "-i".into(), it.path.clone()];
        a.push("-vf".into());
        a.push(format!("fps={n}/{span:.3},scale={tw}:-2,tile={cols}x{rows}:padding=6:margin=8:color={bg}"));
        a.extend(sv(&["-frames:v", "1"]));
        if fmt == "jpg" {
            a.extend(sv(&["-q:v", "2"]));
        }
        a.push(ps(&out));
        ff(ctx, &a, span, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_from_images(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let (imgs, pass) = split_kinds(files, &["image"]);
    if imgs.is_empty() {
        bail!("No images in the input");
    }
    ctx.tools.ffmpeg()?;
    let per = p.f64("seconds", 2.0).max(0.04);
    let fps = p.i64("fps", 30).clamp(1, 120);
    let w = even(p.i64("width", 1920).clamp(16, 7680) as f64);
    let h = even(p.i64("height", 1080).clamp(16, 7680) as f64);
    let fit = p.str("fit", "pad");
    let fmt = p.str("format", "mp4");
    let audio = p.str("audio", "");
    let name = p.str("name", "slideshow");
    let bg = ffcolor(&p.str("background", "#000000"));
    let q = p.i64("quality", 20);
    let dir = ctx.out_dir("_slides")?;
    let mut list = String::from("ffconcat version 1.0\n");
    let total = imgs.len();
    let mut last = String::new();
    for (i, it) in imgs.iter().enumerate() {
        ctx.check()?;
        let src = if one_of(&it.ext, &["jpg", "jpeg", "png", "bmp", "tif", "tiff", "webp", "gif"]) {
            it.path_buf()
        } else {
            let img = crate::ops::img2::open_any(ctx, &it.path_buf())?;
            let q = dir.join(format!("{:05}.png", i));
            img.save(&q)?;
            q
        };
        let s = ps(&src).replace('\\', "/").replace('\'', "'\\''");
        list.push_str(&format!("file '{s}'\nduration {per}\n"));
        last = s;
        ctx.report(i as f64 / total as f64 * 0.1, &it.name);
    }
    list.push_str(&format!("file '{last}'\n"));
    let list_path = dir.join("list.ffconcat");
    std::fs::write(&list_path, list)?;
    let scale = match fit.as_str() {
        "crop" => format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}"),
        _ => format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color={bg}"),
    };
    let ext = match fmt.as_str() {
        "webm" => "webm",
        "gif" => "gif",
        "mov" => "mov",
        "mkv" => "mkv",
        _ => "mp4",
    };
    let out = ctx.reserve(&ctx.out_dir(&imgs[0].rel)?, &name, ext)?;
    let mut a = sv(&["-f", "concat", "-safe", "0", "-i"]);
    a.push(ps(&list_path));
    let has_audio = !audio.trim().is_empty() && Path::new(audio.trim()).is_file() && ext != "gif";
    if has_audio {
        a.push("-i".into());
        a.push(audio.trim().to_string());
    }
    if ext == "gif" {
        a.push("-filter_complex".into());
        a.push(format!("[0:v]{scale},fps={fps},split[x][y];[x]palettegen[pl];[y][pl]paletteuse[v]"));
        a.extend(sv(&["-map", "[v]", "-loop", "0"]));
    } else {
        a.push("-vf".into());
        a.push(format!("{scale},setsar=1,fps={fps},format=yuv420p"));
        a.extend(sv(&["-map", "0:v"]));
        if ext == "webm" {
            a.extend(sv(&["-c:v", "libvpx-vp9", "-row-mt", "1", "-b:v", "0", "-crf"]));
            a.push((q + 12).clamp(4, 63).to_string());
        } else {
            a.extend(sv(&["-c:v", "libx264", "-preset", "medium", "-tune", "stillimage", "-crf"]));
            a.push(q.clamp(0, 51).to_string());
        }
        if has_audio {
            a.extend(sv(&["-map", "1:a:0", "-shortest"]));
            a.extend(sv(if ext == "webm" { &["-c:a", "libopus", "-b:a", "160k"] } else { &["-c:a", "aac", "-b:a", "192k"] }));
        }
        if ext == "mp4" || ext == "mov" {
            a.extend(sv(&["-movflags", "+faststart"]));
        }
    }
    a.push(ps(&out));
    let dur = per * total as f64;
    ff(ctx, &a, dur, &|f| ctx.report(0.1 + f * 0.9, ""), None)?;
    let _ = std::fs::remove_dir_all(&dir);
    let mut v = vec![FileItem::from_path(&out, &imgs[0].rel)];
    v.extend(pass);
    Ok(v)
}

pub fn video_mux(ctx: &Ctx, p: &Params, videos: Vec<FileItem>, audios: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "replace");
    let vol = p.f64("volume", 100.0).max(0.0) / 100.0;
    let orig = p.f64("original_volume", 100.0).max(0.0) / 100.0;
    let shortest = p.bool("shortest", true);
    let offset = p.f64("offset", 0.0).max(0.0);
    let loop_audio = p.bool("loop_audio", false);
    let audios: Vec<FileItem> = audios.into_iter().filter(|f| f.kind == "audio" || f.kind == "video").collect();
    if audios.is_empty() {
        bail!("Connect an audio file to the second input");
    }
    let order: Vec<String> = videos.iter().filter(|v| v.kind == "video").map(|v| v.path.clone()).collect();
    each(ctx, videos, &["video"], |it, on| {
        let idx = order.iter().position(|x| *x == it.path).unwrap_or(0);
        let au = if audios.len() == 1 {
            &audios[0]
        } else {
            audios.iter().find(|a| a.stem() == it.stem()).unwrap_or(&audios[idx % audios.len()])
        };
        let info = media_info(ctx.tools, &it.path_buf());
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        if offset > 0.0 {
            a.push("-itsoffset".into());
            a.push(format!("{offset:.3}"));
        }
        if loop_audio {
            a.extend(sv(&["-stream_loop", "-1"]));
        }
        a.push("-i".into());
        a.push(au.path.clone());
        if mode == "mix" && info.has_audio {
            a.push("-filter_complex".into());
            a.push(format!("[0:a]volume={orig:.3}[a0];[1:a]volume={vol:.3}[a1];[a0][a1]amix=inputs=2:duration=first:dropout_transition=0:normalize=0[a]"));
            a.extend(sv(&["-map", "0:v:0", "-map", "[a]"]));
        } else {
            a.extend(sv(&["-map", "0:v:0", "-map", "1:a:0"]));
            if (vol - 1.0).abs() > 0.001 {
                a.push("-filter:a".into());
                a.push(format!("volume={vol:.3}"));
            }
        }
        let ext = match it.ext.as_str() {
            "mkv" | "mov" | "mp4" | "m4v" => it.ext.clone(),
            "webm" => "webm".into(),
            _ => "mp4".into(),
        };
        a.extend(sv(&["-c:v", "copy"]));
        if ext == "webm" {
            a.extend(sv(&["-c:a", "libopus", "-b:a", "160k"]));
        } else {
            a.extend(sv(&["-c:a", "aac", "-b:a", "192k"]));
        }
        if shortest || loop_audio {
            a.push("-shortest".into());
        }
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_compress(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let target = p.str("target", "discord");
    let size_mb = match target.as_str() {
        "discord" => 10.0,
        "discord-50" => 50.0,
        "whatsapp" => 16.0,
        "telegram" => 2000.0,
        "email" => 25.0,
        "gmail" => 25.0,
        _ => p.f64("size_mb", 25.0),
    }
    .max(0.5);
    let budget = size_mb * 0.96;
    let max_h = p.i64("max_height", 1080).max(0);
    let codec = p.str("codec", "h264");
    let abr = p.i64("audio_bitrate", 96).clamp(24, 320);
    let max_fps = p.f64("max_fps", 30.0).max(0.0);
    let preset = p.str("preset", "medium");
    let skip_small = p.bool("skip_small", true);
    each(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if info.duration <= 0.0 {
            bail!("unknown duration");
        }
        if skip_small && (it.size as f64) <= budget * 1_048_576.0 {
            let out = ctx.out_path(it, &it.stem(), &it.ext)?;
            crate::util::link_or_copy(&it.path_buf(), &out)?;
            ctx.warn(format!("{}: already fits into {size_mb} MB, kept as is", it.name));
            return Ok(vec![it.derive(&out)]);
        }
        let audio_k = if info.has_audio { abr as f64 } else { 0.0 };
        let mut kbps = budget * 8192.0 / info.duration - audio_k;
        if kbps < 80.0 {
            ctx.warn(format!("{}: {:.0} s of video is a lot for {size_mb} MB, quality will be low", it.name, info.duration));
            kbps = kbps.max(40.0);
        }
        let src_h = if info.height > 0 { info.height as i64 } else { 1080 };
        let cap = if max_h > 0 { max_h.min(src_h) } else { src_h };
        let ladder: [(i64, f64); 8] = [(2160, 8000.0), (1440, 5000.0), (1080, 2500.0), (720, 1200.0), (540, 700.0), (480, 500.0), (360, 280.0), (240, 0.0)];
        let mut height = 240;
        for (hh, need) in ladder {
            if hh <= cap && kbps * if codec == "h265" { 1.4 } else { 1.0 } >= need {
                height = hh;
                break;
            }
        }
        if cap < 240 {
            height = cap;
        }
        let mut vf = vec![];
        if height < src_h {
            vf.push(format!("scale=-2:{height}"));
        }
        if max_fps > 0.0 && info.fps > max_fps + 0.5 {
            vf.push(format!("fps={max_fps}"));
        }
        let out = ctx.out_path(it, &it.stem(), "mp4")?;
        let passlog = ctx.work.join(format!("2pass-{}", sanitize(&it.stem())));
        let vcodec = vcodec_args(if codec == "h265" { "h265" } else { "h264" }, "none", 23, &preset, Some(kbps), "mp4", "none");
        let build = |pass: u8| -> Vec<String> {
            let mut a = sv(&["-i"]);
            a.push(it.path.clone());
            a.extend(sv(&["-map", "0:v:0"]));
            if pass != 1 && info.has_audio {
                a.extend(sv(&["-map", "0:a:0"]));
            }
            if !vf.is_empty() {
                a.push("-vf".into());
                a.push(vf.join(","));
            }
            a.extend(vcodec.iter().cloned());
            if pass > 0 {
                a.push("-pass".into());
                a.push(pass.to_string());
                a.push("-passlogfile".into());
                a.push(ps(&passlog));
            }
            if pass == 1 {
                a.extend(sv(&["-an", "-f", "null", "-"]));
                return a;
            }
            if info.has_audio {
                a.extend(sv(&["-c:a", "aac", "-ac", "2", "-b:a"]));
                a.push(format!("{abr}k"));
            }
            a.extend(sv(&["-movflags", "+faststart"]));
            a.push(ps(&out));
            a
        };
        if codec == "h265" {
            ff(ctx, &build(0), info.duration, on, None)?;
        } else {
            ff(ctx, &build(1), info.duration, &|f| on(f * 0.45), None)?;
            ff(ctx, &build(2), info.duration, &|f| on(0.45 + f * 0.55), None)?;
        }
        for suffix in ["-0.log", "-0.log.mbtree", "-0.log.temp"] {
            let _ = std::fs::remove_file(format!("{}{}", ps(&passlog), suffix));
        }
        let got = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) as f64 / 1_048_576.0;
        if got > size_mb {
            ctx.warn(format!("{}: came out at {got:.1} MB (target {size_mb} MB); try a lower max height", it.name));
        }
        Ok(vec![it.derive(&out)])
    })
}

/// Audio to video: a waveform visualizer on a solid background (for sharing audio as video).
pub fn audio_to_video(ctx: &Ctx, files: Vec<FileItem>, ext: &str, color: &str, background: &str) -> Result<Vec<FileItem>> {
    let fg = ffcolor(color);
    let bg = ffcolor(background);
    each(ctx, files, &["audio"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let out = ctx.out_path(it, &it.stem(), ext)?;
        let mut a = sv(&["-f", "lavfi", "-i"]);
        a.push(format!("color=c={bg}:s=1280x720:r=30"));
        a.push("-i".into());
        a.push(it.path.clone());
        a.push("-filter_complex".into());
        a.push(format!("[1:a]showwaves=s=1280x360:mode=cline:rate=30:colors={fg}[w];[0:v][w]overlay=0:180:format=auto,format=yuv420p[v]"));
        a.extend(sv(&["-map", "[v]", "-map", "1:a:0", "-shortest"]));
        if ext == "webm" {
            a.extend(sv(&["-c:v", "libvpx-vp9", "-row-mt", "1", "-b:v", "0", "-crf", "36", "-c:a", "libopus", "-b:a", "160k"]));
        } else {
            a.extend(sv(&["-c:v", "libx264", "-preset", "fast", "-crf", "24", "-c:a", "aac", "-b:a", "192k"]));
        }
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn media_report(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Outs> {
    let fmt = p.str("format", "csv");
    ctx.tools.ffprobe()?;
    let media: Vec<&FileItem> = files.iter().filter(|f| f.kind == "video" || f.kind == "audio").collect();
    if media.is_empty() {
        bail!("No video or audio files in the input");
    }
    let total = media.len();
    let mut rows: Vec<Value> = vec![];
    for (i, it) in media.iter().enumerate() {
        ctx.check()?;
        let j = match ffprobe_json(ctx.tools, &it.path_buf()) {
            Ok(j) => j,
            Err(e) => {
                ctx.warn(format!("{}: {:#}", it.name, e));
                continue;
            }
        };
        let f = &j["format"];
        let num = |v: &Value| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64()).unwrap_or(0.0);
        let streams = j["streams"].as_array().cloned().unwrap_or_default();
        let v = streams.iter().find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"].as_i64() != Some(1));
        let a = streams.iter().find(|s| s["codec_type"] == "audio");
        let subs = streams.iter().filter(|s| s["codec_type"] == "subtitle").count();
        let dur = num(&f["duration"]);
        rows.push(json!({
            "name": it.name,
            "folder": it.rel,
            "container": f["format_long_name"].as_str().or(f["format_name"].as_str()).unwrap_or(""),
            "duration": (dur * 100.0).round() / 100.0,
            "duration_hms": format!("{:02}:{:02}:{:05.2}", (dur / 3600.0) as u64, ((dur % 3600.0) / 60.0) as u64, dur % 60.0),
            "size_mb": (it.size as f64 / 1_048_576.0 * 100.0).round() / 100.0,
            "bitrate_kbps": (num(&f["bit_rate"]) / 1000.0).round(),
            "video_codec": v.and_then(|s| s["codec_name"].as_str()).unwrap_or(""),
            "width": v.and_then(|s| s["width"].as_u64()).unwrap_or(0),
            "height": v.and_then(|s| s["height"].as_u64()).unwrap_or(0),
            "fps": v.map(|s| (parse_rate(s["avg_frame_rate"].as_str().unwrap_or("0")) * 100.0).round() / 100.0).unwrap_or(0.0),
            "pixel_format": v.and_then(|s| s["pix_fmt"].as_str()).unwrap_or(""),
            "audio_codec": a.and_then(|s| s["codec_name"].as_str()).unwrap_or(""),
            "sample_rate": a.map(|s| num(&s["sample_rate"])).unwrap_or(0.0),
            "channels": a.and_then(|s| s["channels"].as_u64()).unwrap_or(0),
            "audio_kbps": a.map(|s| (num(&s["bit_rate"]) / 1000.0).round()).unwrap_or(0.0),
            "subtitles": subs,
            "title": f["tags"]["title"].as_str().unwrap_or(""),
        }));
        ctx.progress(i + 1, total, &it.name);
    }
    let cols = [
        "name", "folder", "container", "duration", "duration_hms", "size_mb", "bitrate_kbps", "video_codec", "width", "height", "fps",
        "pixel_format", "audio_codec", "sample_rate", "channels", "audio_kbps", "subtitles", "title",
    ];
    let cell = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let (body, ext) = match fmt.as_str() {
        "json" => (serde_json::to_string_pretty(&rows)?, "json"),
        "md" => {
            let mut s = format!("| {} |\n|{}|\n", cols.join(" | "), cols.iter().map(|_| "---").collect::<Vec<_>>().join("|"));
            for r in &rows {
                s.push_str(&format!("| {} |\n", cols.iter().map(|c| cell(&r[*c]).replace('|', "\\|")).collect::<Vec<_>>().join(" | ")));
            }
            (s, "md")
        }
        "txt" => {
            let mut s = String::new();
            for r in &rows {
                for c in cols {
                    let v = cell(&r[c]);
                    if !v.is_empty() && v != "0" {
                        s.push_str(&format!("{c:>13}: {v}\n"));
                    }
                }
                s.push('\n');
            }
            (s, "txt")
        }
        _ => {
            let mut w = csv::Writer::from_writer(vec![]);
            w.write_record(cols)?;
            for r in &rows {
                w.write_record(cols.iter().map(|c| cell(&r[*c])))?;
            }
            let bytes = w.into_inner().map_err(|e| anyhow::anyhow!("csv: {e}"))?;
            (String::from_utf8(bytes)?, "csv")
        }
    };
    let out = ctx.reserve(&ctx.out_dir("")?, "media-info", ext)?;
    std::fs::write(&out, body)?;
    Ok(vec![files, vec![FileItem::from_path(&out, "")]])
}
