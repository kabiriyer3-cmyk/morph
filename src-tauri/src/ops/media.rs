//! Video and audio nodes (FFmpeg CLI).
use super::split_kinds;
use crate::engine::{Ctx, FileItem};
use crate::tools::Tools;
use crate::util::{clean_rel, command, file_name, join_rel, parse_time, Params};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn sv(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[derive(Default, Clone, Debug)]
pub struct MediaInfo {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub audio_codec: String,
}

pub fn ffprobe_json(tools: &Tools, path: &Path) -> Result<Value> {
    let bin = tools.ffprobe()?;
    let out = command(bin)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(path)
        .output()?;
    if !out.status.success() {
        bail!("ffprobe: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

fn parse_rate(s: &str) -> f64 {
    let mut it = s.split('/');
    let a: f64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    let b: f64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(1.0);
    if b > 0.0 {
        a / b
    } else {
        0.0
    }
}

pub fn media_info(tools: &Tools, path: &Path) -> MediaInfo {
    let mut m = MediaInfo::default();
    let Ok(j) = ffprobe_json(tools, path) else { return m };
    m.duration = j["format"]["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    if let Some(streams) = j["streams"].as_array() {
        for s in streams {
            match s["codec_type"].as_str() {
                Some("video") if !m.has_video && s["disposition"]["attached_pic"].as_i64() != Some(1) => {
                    m.has_video = true;
                    m.width = s["width"].as_u64().unwrap_or(0) as u32;
                    m.height = s["height"].as_u64().unwrap_or(0) as u32;
                    m.fps = parse_rate(s["avg_frame_rate"].as_str().or(s["r_frame_rate"].as_str()).unwrap_or("0"));
                    if m.duration <= 0.0 {
                        m.duration = s["duration"].as_str().and_then(|x| x.parse().ok()).unwrap_or(0.0);
                    }
                }
                Some("audio") if !m.has_audio => {
                    m.has_audio = true;
                    m.audio_codec = s["codec_name"].as_str().unwrap_or("").to_string();
                    if m.duration <= 0.0 {
                        m.duration = s["duration"].as_str().and_then(|x| x.parse().ok()).unwrap_or(0.0);
                    }
                }
                _ => {}
            }
        }
    }
    m
}

/// Runs ffmpeg with `-progress` parsing; `on` receives 0..1 for the current file.
pub fn ffmpeg_run(ctx: &Ctx, args: &[String], duration: f64, on: &dyn Fn(f64)) -> Result<()> {
    let bin = ctx.tools.ffmpeg()?;
    let mut cmd = command(bin);
    cmd.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-progress", "pipe:1", "-nostats"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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

/// Sequential per-file loop for FFmpeg nodes (FFmpeg is multi-threaded itself).
fn media_batch<F>(ctx: &Ctx, files: Vec<FileItem>, kinds: &[&str], f: F) -> Result<Vec<FileItem>>
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

fn out_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn scale_filter(w: i64, h: i64) -> Option<String> {
    match (w > 0, h > 0) {
        (true, true) => Some(format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,scale=trunc(iw/2)*2:trunc(ih/2)*2"
        )),
        (true, false) => Some(format!("scale={w}:-2")),
        (false, true) => Some(format!("scale=-2:{h}")),
        _ => None,
    }
}

fn atempo(speed: f64) -> String {
    let mut s = speed;
    let mut parts = vec![];
    while s > 2.0 {
        parts.push("atempo=2.0".to_string());
        s /= 2.0;
    }
    while s < 0.5 {
        parts.push("atempo=0.5".to_string());
        s /= 0.5;
    }
    parts.push(format!("atempo={s:.4}"));
    parts.join(",")
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

fn video_codec_args(codec: &str, hw: &str, q: i64, preset: &str, kbps: Option<f64>, container: &str) -> Vec<String> {
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
        ("prores", _) => "prores_ks",
        _ => "libx264",
    };
    let mp4ish = matches!(container, "mp4" | "mov" | "m4v");
    let mut v = sv(&["-c:v", enc]);
    match enc {
        "libx264" | "libx265" => {
            v.push("-preset".into());
            v.push(if PRESETS.contains(&preset) { preset.to_string() } else { "medium".into() });
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
                    v.push(q.clamp(0, 63).to_string());
                }
            }
            v.extend(sv(&["-pix_fmt", "yuv420p"]));
        }
        "libvpx-vp9" => {
            v.extend(sv(&["-row-mt", "1", "-deadline", "good", "-cpu-used"]));
            v.push(preset_num(preset, [8, 7, 6, 5, 4, 2, 1, 1, 0]).to_string());
            match kbps {
                Some(k) => push_rate(&mut v, k),
                None => {
                    v.push("-crf".into());
                    v.push(q.clamp(0, 63).to_string());
                    v.extend(sv(&["-b:v", "0"]));
                }
            }
            v.extend(sv(&["-pix_fmt", "yuv420p"]));
        }
        "prores_ks" => v.extend(sv(&["-profile:v", "3", "-pix_fmt", "yuv422p10le"])),
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

fn audio_codec_args(codec: &str, kbps: i64) -> Vec<String> {
    let b = format!("{kbps}k");
    match codec {
        "none" => sv(&["-an"]),
        "copy" => sv(&["-c:a", "copy"]),
        "opus" => sv(&["-c:a", "libopus", "-b:a", &b]),
        "mp3" => sv(&["-c:a", "libmp3lame", "-b:a", &b]),
        "flac" => sv(&["-c:a", "flac"]),
        _ => sv(&["-c:a", "aac", "-b:a", &b]),
    }
}

/// (extension, codec args) for an audio target format.
fn audio_target(fmt: &str, kbps: i64, src_codec: &str) -> (String, Vec<String>) {
    let b = format!("{kbps}k");
    let (ext, args) = match fmt {
        "m4a" | "aac" => ("m4a", sv(&["-c:a", "aac", "-b:a", &b])),
        "opus" => ("opus", sv(&["-c:a", "libopus", "-b:a", &b])),
        "ogg" => ("ogg", sv(&["-c:a", "libvorbis", "-b:a", &b])),
        "flac" => ("flac", sv(&["-c:a", "flac"])),
        "wav" => ("wav", sv(&["-c:a", "pcm_s16le"])),
        "alac" => ("m4a", sv(&["-c:a", "alac"])),
        "copy" => {
            let ext = match src_codec {
                "aac" | "alac" => "m4a",
                "mp3" => "mp3",
                "opus" => "opus",
                "vorbis" => "ogg",
                "flac" => "flac",
                c if c.starts_with("pcm_") => "wav",
                _ => "mka",
            };
            (ext, sv(&["-c:a", "copy"]))
        }
        _ => ("mp3", sv(&["-c:a", "libmp3lame", "-b:a", &b])),
    };
    (ext.to_string(), args)
}

pub fn video_convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let container = p.str("container", "mp4");
    let vcodec = p.str("video_codec", "h264");
    let q = p.i64("quality", 23);
    let preset = p.str("preset", "medium");
    let target_mb = p.f64("target_mb", 0.0);
    let w = p.i64("width", 0);
    let h = p.i64("height", 0);
    let fps = p.f64("fps", 0.0);
    let acodec = p.str("audio_codec", "aac");
    let abr = p.i64("audio_bitrate", 160).clamp(8, 1024);
    let hw = p.str("hardware", "none");
    media_batch(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let mut cont = if container == "keep" { it.ext.clone() } else { container.clone() };
        let mut vc = vcodec.clone();
        let mut ac = acodec.clone();
        if vc == "prores" {
            cont = "mov".into();
        }
        if cont == "webm" {
            if !matches!(vc.as_str(), "vp9" | "av1") {
                vc = "vp9".into();
            }
            if !matches!(ac.as_str(), "opus" | "none") {
                ac = "opus".into();
            }
        }
        let mut a = sv(&["-i", &it.path, "-map", "0:v:0?", "-map", "0:a?", "-map_metadata", "0"]);
        let mut vf: Vec<String> = vec![];
        if let Some(s) = scale_filter(w, h) {
            vf.push(s);
        }
        if fps > 0.0 {
            vf.push(format!("fps={fps}"));
        }
        if vc == "copy" {
            if !vf.is_empty() {
                ctx.warn(format!("{}: resize/fps are ignored when the video stream is copied", it.name));
            }
            a.extend(sv(&["-c:v", "copy"]));
        } else {
            if !vf.is_empty() {
                a.push("-vf".into());
                a.push(vf.join(","));
            }
            let kbps = if target_mb > 0.0 && info.duration > 0.0 {
                let audio = match ac.as_str() {
                    "none" => 0.0,
                    "copy" => 192.0,
                    _ => abr as f64,
                };
                Some((target_mb * 8192.0 / info.duration - audio).max(64.0))
            } else {
                None
            };
            a.extend(video_codec_args(&vc, &hw, q, &preset, kbps, &cont));
        }
        a.extend(audio_codec_args(&ac, abr));
        if matches!(cont.as_str(), "mp4" | "mov" | "m4v") {
            a.extend(sv(&["-movflags", "+faststart"]));
        }
        let out = ctx.out_path(it, &it.stem(), &cont)?;
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, info.duration, on)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_trim(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let start = parse_time(&p.str("start", "0")).unwrap_or(0.0).max(0.0);
    let end = parse_time(&p.str("end", "")).filter(|e| *e > start);
    let precise = p.bool("precise", false);
    media_batch(ctx, files, &["video", "audio"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let dur = match end {
            Some(e) => e - start,
            None => (info.duration - start).max(0.0),
        };
        let mut a = vec!["-ss".to_string(), format!("{start:.3}"), "-i".into(), it.path.clone()];
        if end.is_some() {
            a.push("-t".into());
            a.push(format!("{dur:.3}"));
        }
        if precise {
            if it.kind == "audio" {
                a.extend(sv(&["-map", "0:a"]));
            } else if it.ext == "webm" {
                a.extend(sv(&["-map", "0:v:0?", "-map", "0:a?", "-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0", "-row-mt", "1", "-c:a", "libopus"]));
            } else {
                a.extend(sv(&[
                    "-map", "0:v:0?", "-map", "0:a?", "-c:v", "libx264", "-crf", "18", "-preset", "medium", "-pix_fmt", "yuv420p",
                    "-c:a", "aac", "-b:a", "192k",
                ]));
            }
        } else {
            a.extend(sv(&["-map", "0:v?", "-map", "0:a?", "-c", "copy", "-avoid_negative_ts", "make_zero"]));
        }
        let out = ctx.out_path(it, &it.stem(), &it.ext)?;
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, dur, on)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_transform(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let rotate = p.i64("rotate", 0).rem_euclid(360);
    let flip_h = p.bool("flip_h", false);
    let flip_v = p.bool("flip_v", false);
    let speed = p.f64("speed", 1.0).clamp(0.1, 16.0);
    let mute = p.bool("mute", false);
    let crop = p.str("crop", "");
    let q = p.i64("quality", 20);
    media_batch(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let mut vf: Vec<String> = vec![];
        if !crop.trim().is_empty() {
            vf.push(format!("crop={}", crop.trim()));
        }
        match rotate {
            90 => vf.push("transpose=1".into()),
            180 => vf.push("hflip,vflip".into()),
            270 => vf.push("transpose=2".into()),
            _ => {}
        }
        if flip_h {
            vf.push("hflip".into());
        }
        if flip_v {
            vf.push("vflip".into());
        }
        let fast = (speed - 1.0).abs() > 1e-3;
        if fast {
            vf.push(format!("setpts=PTS/{speed}"));
        }
        let ext = if matches!(it.ext.as_str(), "mp4" | "mkv" | "mov" | "webm") { it.ext.clone() } else { "mp4".to_string() };
        let mut a = sv(&["-i", &it.path]);
        if !vf.is_empty() {
            a.push("-vf".into());
            a.push(vf.join(","));
        }
        if mute {
            a.push("-an".into());
        } else if fast {
            a.push("-af".into());
            a.push(atempo(speed));
        }
        if ext == "webm" {
            a.extend(sv(&["-c:v", "libvpx-vp9", "-b:v", "0", "-row-mt", "1", "-crf"]));
            a.push(q.clamp(0, 63).to_string());
            if !mute {
                a.extend(sv(&["-c:a", "libopus", "-b:a", "128k"]));
            }
        } else {
            a.extend(sv(&["-c:v", "libx264", "-preset", "medium", "-pix_fmt", "yuv420p", "-crf"]));
            a.push(q.clamp(0, 51).to_string());
            if !mute {
                a.extend(sv(&["-c:a", "aac", "-b:a", "192k"]));
            }
        }
        if matches!(ext.as_str(), "mp4" | "mov") {
            a.extend(sv(&["-movflags", "+faststart"]));
        }
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, info.duration / speed, on)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_gif(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "gif");
    let fps = p.f64("fps", 12.0).clamp(1.0, 60.0);
    let width = p.i64("width", 480);
    let start = parse_time(&p.str("start", "0")).unwrap_or(0.0).max(0.0);
    let duration = p.f64("duration", 0.0).max(0.0);
    let colors = p.i64("colors", 256).clamp(2, 256);
    let dither = p.str("dither", "sierra2_4a");
    let looped = p.bool("loop", true);
    media_batch(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let dur = if duration > 0.0 { duration } else { (info.duration - start).max(0.0) };
        let mut a: Vec<String> = vec![];
        if start > 0.0 {
            a.push("-ss".into());
            a.push(format!("{start:.3}"));
        }
        if duration > 0.0 {
            a.push("-t".into());
            a.push(format!("{duration:.3}"));
        }
        a.push("-i".into());
        a.push(it.path.clone());
        let scale = if width > 0 { format!(",scale={width}:-1:flags=lanczos") } else { String::new() };
        let ext = if fmt == "webp" {
            a.push("-vf".into());
            a.push(format!("fps={fps}{scale}"));
            a.extend(sv(&["-c:v", "libwebp", "-lossless", "0", "-q:v", "75", "-compression_level", "4", "-an", "-loop"]));
            a.push(if looped { "0".into() } else { "1".into() });
            "webp"
        } else {
            let d = if dither == "bayer" { "bayer:bayer_scale=3".to_string() } else { dither.clone() };
            a.push("-filter_complex".into());
            a.push(format!(
                "[0:v]fps={fps}{scale},split[a][b];[a]palettegen=max_colors={colors}:stats_mode=diff[p];[b][p]paletteuse=dither={d}"
            ));
            a.push("-loop".into());
            a.push(if looped { "0".into() } else { "-1".into() });
            "gif"
        };
        let out = ctx.out_path(it, &it.stem(), ext)?;
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, dur, on)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_frames(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "fps");
    let value = p.f64("value", 1.0);
    let time = parse_time(&p.str("time", "0")).unwrap_or(0.0);
    let fmt = p.str("format", "png");
    let width = p.i64("width", 0);
    media_batch(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let parent = ctx.out_dir(&it.rel)?;
        let dir = ctx.reserve(&parent, &format!("{}_frames", it.stem()), "")?;
        std::fs::create_dir_all(&dir)?;
        let rel = join_rel(&clean_rel(&it.rel), &file_name(&dir));
        let stem = it.stem().replace('%', "_");
        let ext = match fmt.as_str() {
            "jpg" | "webp" => fmt.clone(),
            _ => "png".to_string(),
        };
        let quality: Vec<String> = match ext.as_str() {
            "jpg" => sv(&["-q:v", "2"]),
            "webp" => sv(&["-c:v", "libwebp", "-q:v", "85"]),
            _ => vec![],
        };
        let mut a: Vec<String> = vec![];
        if mode == "single" {
            a.extend(["-ss".to_string(), format!("{time:.3}"), "-i".into(), it.path.clone(), "-frames:v".into(), "1".into()]);
            if width > 0 {
                a.push("-vf".into());
                a.push(format!("scale={width}:-2"));
            }
            a.extend(quality);
            a.push(out_str(&dir.join(format!("{stem}.{ext}"))));
        } else {
            let rate = match mode.as_str() {
                "interval" => 1.0 / value.max(0.01),
                "count" => {
                    if info.duration > 0.0 {
                        value.max(1.0) / info.duration
                    } else {
                        1.0
                    }
                }
                _ => value.max(0.01),
            };
            let mut vf = format!("fps={rate:.6}");
            if width > 0 {
                vf.push_str(&format!(",scale={width}:-2"));
            }
            a.extend(["-i".to_string(), it.path.clone(), "-vf".into(), vf]);
            a.extend(quality);
            a.push(out_str(&dir.join(format!("{stem}_%05d.{ext}"))));
        }
        ffmpeg_run(ctx, &a, if mode == "single" { 0.0 } else { info.duration }, on)?;
        let mut frames: Vec<PathBuf> = std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file()).collect();
        frames.sort();
        if frames.is_empty() {
            bail!("no frames were extracted");
        }
        Ok(frames
            .iter()
            .map(|f| {
                let mut x = it.derive(f);
                x.rel = rel.clone();
                x
            })
            .collect())
    })
}

pub fn video_audio(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "mp3");
    let br = p.i64("bitrate", 192).clamp(8, 1024);
    media_batch(ctx, files, &["video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if !info.has_audio && ctx.tools.ffprobe.is_some() {
            bail!("the video has no audio track");
        }
        let (ext, codec) = audio_target(&fmt, br, &info.audio_codec);
        let mut a = sv(&["-i", &it.path, "-vn", "-map", "0:a:0"]);
        a.extend(codec);
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, info.duration, on)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn video_concat(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    ctx.tools.ffmpeg()?;
    let (vids, _) = split_kinds(files, &["video", "audio"]);
    if vids.len() < 2 {
        bail!("Concat needs at least two video or audio files");
    }
    let name = p.str("name", "joined");
    let reencode = p.bool("reencode", true);
    let infos: Vec<MediaInfo> = vids.iter().map(|v| media_info(ctx.tools, &v.path_buf())).collect();
    let total: f64 = infos.iter().map(|i| i.duration).sum();
    let dir = ctx.out_dir("")?;
    let on = |f: f64| ctx.report(f, "joining");
    let mut a: Vec<String> = vec![];
    if !reencode {
        let list = dir.join("_concat.txt");
        let body: String = vids
            .iter()
            .map(|v| format!("file '{}'\n", v.path.replace('\\', "/").replace('\'', "'\\''")))
            .collect();
        std::fs::write(&list, body)?;
        let out = ctx.reserve(&dir, &name, &vids[0].ext)?;
        a.extend(sv(&["-f", "concat", "-safe", "0", "-i"]));
        a.push(out_str(&list));
        a.extend(sv(&["-map", "0", "-c", "copy"]));
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, total, &on)?;
        return Ok(vec![FileItem::from_path(&out, "")]);
    }
    let video = infos.iter().all(|i| i.has_video);
    let audio = infos.iter().all(|i| i.has_audio);
    if !video && !audio {
        bail!("The inputs have no common video or audio stream");
    }
    let (w, h, fps) = infos
        .iter()
        .find(|i| i.has_video)
        .map(|i| {
            let fps = if i.fps > 0.0 && i.fps < 240.0 { i.fps } else { 30.0 };
            (i.width.max(2) & !1, i.height.max(2) & !1, fps)
        })
        .unwrap_or((1280, 720, 30.0));
    for v in &vids {
        a.push("-i".into());
        a.push(v.path.clone());
    }
    let mut fc = String::new();
    let mut cat = String::new();
    for i in 0..vids.len() {
        if video {
            fc.push_str(&format!(
                "[{i}:v:0]scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps:.3},format=yuv420p[v{i}];"
            ));
            cat.push_str(&format!("[v{i}]"));
        }
        if audio {
            fc.push_str(&format!("[{i}:a:0]aresample=48000,aformat=channel_layouts=stereo[a{i}];"));
            cat.push_str(&format!("[a{i}]"));
        }
    }
    fc.push_str(&format!("{cat}concat=n={}:v={}:a={}", vids.len(), video as u8, audio as u8));
    if video {
        fc.push_str("[v]");
    }
    if audio {
        fc.push_str("[a]");
    }
    a.push("-filter_complex".into());
    a.push(fc);
    let first_ext = vids[0].ext.as_str();
    let ext = if !video {
        "m4a"
    } else if first_ext == "webm" {
        "webm"
    } else if first_ext == "mkv" {
        "mkv"
    } else {
        "mp4"
    };
    if video {
        a.extend(sv(&["-map", "[v]"]));
        if ext == "webm" {
            a.extend(sv(&["-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0", "-row-mt", "1"]));
        } else {
            a.extend(sv(&["-c:v", "libx264", "-crf", "20", "-preset", "medium"]));
        }
    }
    if audio {
        a.extend(sv(&["-map", "[a]"]));
        if ext == "webm" {
            a.extend(sv(&["-c:a", "libopus", "-b:a", "128k"]));
        } else {
            a.extend(sv(&["-c:a", "aac", "-b:a", "192k"]));
        }
    }
    if ext == "mp4" {
        a.extend(sv(&["-movflags", "+faststart"]));
    }
    let out = ctx.reserve(&dir, &name, ext)?;
    a.push(out_str(&out));
    ffmpeg_run(ctx, &a, total, &on)?;
    Ok(vec![FileItem::from_path(&out, "")])
}

pub fn audio_convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let fmt = p.str("format", "mp3");
    let br = p.i64("bitrate", 192).clamp(8, 1024);
    let sr = p.i64("sample_rate", 0);
    let ch = p.i64("channels", 0);
    media_batch(ctx, files, &["audio", "video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let (ext, codec) = audio_target(&fmt, br, &info.audio_codec);
        let mut a = sv(&["-i", &it.path, "-vn", "-map", "0:a:0", "-map_metadata", "0"]);
        a.extend(codec);
        if sr > 0 {
            a.push("-ar".into());
            a.push(sr.to_string());
        }
        if ch > 0 {
            a.push("-ac".into());
            a.push(ch.to_string());
        }
        let out = ctx.out_path(it, &it.stem(), &ext)?;
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, info.duration, on)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn audio_process(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let vol = p.f64("volume_db", 0.0);
    let normalize = p.bool("normalize", false);
    let lufs = p.f64("lufs", -14.0).clamp(-70.0, -5.0);
    let ts = p.f64("trim_start", 0.0).max(0.0);
    let te = p.f64("trim_end", 0.0).max(0.0);
    let fi = p.f64("fade_in", 0.0).max(0.0);
    let fo = p.f64("fade_out", 0.0).max(0.0);
    let speed = p.f64("speed", 1.0).clamp(0.25, 8.0);
    media_batch(ctx, files, &["audio"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let total = info.duration;
        let eff = if total > 0.0 { (total - ts - te).max(0.1) } else { 0.0 };
        let mut a: Vec<String> = vec![];
        if ts > 0.0 {
            a.push("-ss".into());
            a.push(format!("{ts:.3}"));
        }
        a.push("-i".into());
        a.push(it.path.clone());
        if te > 0.0 && total > 0.0 {
            a.push("-t".into());
            a.push(format!("{eff:.3}"));
        }
        let mut af: Vec<String> = vec![];
        if vol != 0.0 {
            af.push(format!("volume={vol}dB"));
        }
        if normalize {
            af.push(format!("loudnorm=I={lufs}:TP=-1.5:LRA=11"));
        }
        if fi > 0.0 {
            af.push(format!("afade=t=in:st=0:d={fi}"));
        }
        if fo > 0.0 && eff > 0.0 {
            af.push(format!("afade=t=out:st={:.3}:d={fo}", (eff - fo).max(0.0)));
        }
        if (speed - 1.0).abs() > 1e-3 {
            af.push(atempo(speed));
        }
        a.extend(sv(&["-map", "0:a:0"]));
        if !af.is_empty() {
            a.push("-af".into());
            a.push(af.join(","));
        }
        if normalize {
            a.extend(sv(&["-ar", "48000"]));
        }
        a.extend(match it.ext.as_str() {
            "mp3" => sv(&["-c:a", "libmp3lame", "-q:a", "2"]),
            "m4a" | "aac" => sv(&["-c:a", "aac", "-b:a", "192k"]),
            "ogg" | "oga" => sv(&["-c:a", "libvorbis", "-q:a", "6"]),
            "opus" => sv(&["-c:a", "libopus", "-b:a", "128k"]),
            "flac" => sv(&["-c:a", "flac"]),
            "wav" => sv(&["-c:a", "pcm_s16le"]),
            _ => vec![],
        });
        let out = ctx.out_path(it, &it.stem(), &it.ext)?;
        a.push(out_str(&out));
        ffmpeg_run(ctx, &a, eff / speed, on)?;
        Ok(vec![it.derive(&out)])
    })
}
