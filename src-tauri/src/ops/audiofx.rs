//! Audio nodes: join, split, silence removal, effects, waveform/spectrogram images, tags and cover art.
use super::split_kinds;
use crate::engine::{Ctx, FileItem};
use crate::ops::av::{audio_target, each, ff, ff_log, ffcolor, ps, sv};
use crate::ops::media::{ffprobe_json, media_info};
use crate::util::{parse_time, Params};
use anyhow::{bail, Result};
use std::path::Path;

/// Keeps the input format when re-encoding filtered audio.
fn keep_fmt(ext: &str) -> &str {
    match ext {
        "m4a" | "m4b" | "aac" | "mp3" | "opus" | "ogg" | "flac" | "wav" | "aiff" | "wma" | "ac3" | "amr" | "m4r" => ext,
        "aif" => "aiff",
        "oga" => "ogg",
        "alac" => "alac",
        _ => "mp3",
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
    parts.push(format!("atempo={s:.5}"));
    parts.join(",")
}

fn sample_rate(ctx: &Ctx, path: &Path) -> f64 {
    ffprobe_json(ctx.tools, path)
        .ok()
        .and_then(|j| {
            j["streams"].as_array().and_then(|s| {
                s.iter()
                    .find(|x| x["codec_type"] == "audio")
                    .and_then(|x| x["sample_rate"].as_str().and_then(|v| v.parse::<f64>().ok()))
            })
        })
        .unwrap_or(44100.0)
}

/// Runs an audio filter chain on audio files (re-encoded in their format) or on the audio of videos.
fn filter_audio(ctx: &Ctx, files: Vec<FileItem>, chain: &dyn Fn(&FileItem) -> String, kbps: i64) -> Result<Vec<FileItem>> {
    each(ctx, files, &["audio", "video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if !info.has_audio {
            bail!("no audio track");
        }
        let af = chain(it);
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        let out = if it.kind == "video" {
            let ext = match it.ext.as_str() {
                "mp4" | "mkv" | "mov" | "m4v" | "webm" => it.ext.clone(),
                _ => "mp4".to_string(),
            };
            a.extend(sv(&["-map", "0:v:0?", "-map", "0:a:0", "-c:v", "copy", "-af"]));
            a.push(af);
            if ext == "webm" {
                a.extend(sv(&["-c:a", "libopus", "-b:a", "160k"]));
            } else {
                a.extend(sv(&["-c:a", "aac", "-b:a", "192k"]));
            }
            ctx.out_path(it, &it.stem(), &ext)?
        } else {
            let (ext, codec) = audio_target(keep_fmt(&it.ext), kbps, &info.audio_codec, false, "16");
            a.extend(sv(&["-map", "0:a:0", "-map_metadata", "0", "-af"]));
            a.push(af);
            a.extend(codec);
            ctx.out_path(it, &it.stem(), &ext)?
        };
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn join(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let (list, pass) = split_kinds(files, &["audio", "video"]);
    if list.is_empty() {
        bail!("No audio files in the input");
    }
    ctx.tools.ffmpeg()?;
    let fmt = p.str("format", "mp3");
    let kbps = p.i64("bitrate", 192).clamp(8, 1024);
    let xf = p.f64("crossfade", 0.0).max(0.0);
    let gap = p.f64("gap", 0.0).max(0.0);
    let name = p.str("name", "joined");
    let n = list.len();
    let mut a: Vec<String> = vec![];
    let mut total = 0.0;
    for it in &list {
        a.push("-i".into());
        a.push(it.path.clone());
        total += media_info(ctx.tools, &it.path_buf()).duration;
    }
    let mut fc: Vec<String> = (0..n)
        .map(|i| {
            let pad = if gap > 0.0 && xf == 0.0 && i + 1 < n { format!(",apad=pad_dur={gap}") } else { String::new() };
            format!("[{i}:a:0]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo{pad}[n{i}]")
        })
        .collect();
    let last = if n == 1 {
        "n0".to_string()
    } else if xf > 0.0 {
        let mut cur = "n0".to_string();
        for i in 1..n {
            let o = format!("x{i}");
            fc.push(format!("[{cur}][n{i}]acrossfade=d={xf}:c1=tri:c2=tri[{o}]"));
            cur = o;
        }
        cur
    } else {
        fc.push(format!("{}concat=n={n}:v=0:a=1[cat]", (0..n).map(|i| format!("[n{i}]")).collect::<String>()));
        "cat".to_string()
    };
    a.push("-filter_complex".into());
    a.push(fc.join(";"));
    a.push("-map".into());
    a.push(format!("[{last}]"));
    let (ext, codec) = audio_target(&fmt, kbps, "", false, "16");
    a.extend(codec);
    let out = ctx.reserve(&ctx.out_dir(&list[0].rel)?, &name, &ext)?;
    a.push(ps(&out));
    ff(ctx, &a, total, &|f: f64| ctx.report(f, ""), None)?;
    let mut v = vec![FileItem::from_path(&out, &list[0].rel)];
    v.extend(pass);
    Ok(v)
}

fn silences(log: &str) -> Vec<(f64, f64)> {
    let num = |s: &str| -> Option<f64> { s.trim().split(|c: char| c == ' ' || c == '|').next().and_then(|x| x.trim().parse().ok()) };
    let mut out = vec![];
    let mut start: Option<f64> = None;
    for line in log.lines() {
        if let Some(i) = line.find("silence_start:") {
            start = num(&line[i + 14..]);
        } else if let Some(i) = line.find("silence_end:") {
            if let (Some(s), Some(e)) = (start.take(), num(&line[i + 12..])) {
                out.push((s, e));
            }
        }
    }
    out
}

pub fn split(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "silence");
    let seg = parse_time(&p.str("duration", "5:00")).unwrap_or(300.0).max(1.0);
    let parts = p.i64("parts", 2).clamp(2, 1000) as usize;
    let thr = p.f64("threshold", -35.0).clamp(-90.0, 0.0);
    let min_sil = p.f64("min_silence", 0.7).max(0.05);
    let min_len = p.f64("min_length", 2.0).max(0.0);
    each(ctx, files, &["audio"], |it, on| {
        let d = media_info(ctx.tools, &it.path_buf()).duration;
        if d <= 0.0 {
            bail!("unknown duration");
        }
        let raw: Vec<f64> = match mode.as_str() {
            "duration" => (1..1_000_000).map(|i| i as f64 * seg).take_while(|t| *t < d - 0.2).collect(),
            "parts" => (1..parts).map(|i| d * i as f64 / parts as f64).collect(),
            _ => {
                let mut a = sv(&["-i"]);
                a.push(it.path.clone());
                a.push("-af".into());
                a.push(format!("silencedetect=noise={thr}dB:d={min_sil}"));
                a.extend(sv(&["-f", "null", "-"]));
                let log = ff_log(ctx, &a)?;
                silences(&log).iter().map(|(s, e)| (s + e) / 2.0).filter(|t| *t > 0.1 && *t < d - 0.1).collect()
            }
        };
        let mut cuts = vec![];
        let mut last = 0.0;
        for c in raw {
            if c - last >= min_len {
                cuts.push(c);
                last = c;
            }
        }
        let mut bounds = vec![0.0];
        bounds.extend(cuts);
        bounds.push(d);
        let n = bounds.len() - 1;
        if n < 2 {
            ctx.warn(format!("{}: nothing to split", it.name));
        }
        let width = n.to_string().len().max(2);
        let mut out = vec![];
        for i in 0..n {
            ctx.check()?;
            let (s, e) = (bounds[i], bounds[i + 1]);
            let path = ctx.out_path(it, &format!("{}_{:0w$}", it.stem(), i + 1, w = width), &it.ext)?;
            let mut a = vec!["-ss".to_string(), format!("{s:.3}"), "-i".into(), it.path.clone(), "-t".into(), format!("{:.3}", e - s)];
            a.extend(sv(&["-map", "0:a:0", "-c", "copy", "-map_metadata", "0"]));
            a.push(ps(&path));
            ff(ctx, &a, e - s, &|f: f64| on((i as f64 + f) / n as f64), None)?;
            out.push(it.derive(&path));
        }
        Ok(out)
    })
}

pub fn silence(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "edges");
    let thr = p.f64("threshold", -45.0).clamp(-90.0, 0.0);
    let min = p.f64("min_silence", 0.5).max(0.05);
    let keep = p.f64("keep", 0.1).max(0.0);
    let chain = if mode == "all" {
        format!("silenceremove=start_periods=1:start_threshold={thr}dB:start_silence={keep}:stop_periods=-1:stop_threshold={thr}dB:stop_duration={min}:stop_silence={keep}")
    } else {
        let one = format!("silenceremove=start_periods=1:start_threshold={thr}dB:start_silence={keep}");
        format!("{one},areverse,{one},areverse")
    };
    filter_audio(ctx, files, &|_| chain.clone(), 192)
}

pub fn fx(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let bass = p.f64("bass", 0.0).clamp(-30.0, 30.0);
    let treble = p.f64("treble", 0.0).clamp(-30.0, 30.0);
    let hp = p.f64("highpass", 0.0).max(0.0);
    let lp = p.f64("lowpass", 0.0).max(0.0);
    let denoise = p.str("denoise", "off");
    let comp = p.bool("compressor", false);
    let limiter = p.bool("limiter", false);
    let pitch = p.f64("pitch", 0.0).clamp(-24.0, 24.0);
    let echo = p.bool("echo", false);
    let reverb = p.bool("reverb", false);
    let mono = p.bool("mono", false);
    let karaoke = p.bool("karaoke", false);
    let swap = p.bool("swap_channels", false);
    let widen = p.f64("widen", 0.0).clamp(0.0, 5.0);
    let vol = p.f64("volume_db", 0.0).clamp(-60.0, 60.0);
    let kbps = p.i64("bitrate", 192).clamp(8, 1024);
    let base: Vec<String> = {
        let mut af = vec![];
        if hp > 0.0 {
            af.push(format!("highpass=f={hp}"));
        }
        if lp > 0.0 {
            af.push(format!("lowpass=f={lp}"));
        }
        match denoise.as_str() {
            "light" => af.push("afftdn=nr=10".into()),
            "medium" => af.push("afftdn=nr=20".into()),
            "strong" => af.push("afftdn=nr=33".into()),
            _ => {}
        }
        if bass != 0.0 {
            af.push(format!("bass=g={bass}"));
        }
        if treble != 0.0 {
            af.push(format!("treble=g={treble}"));
        }
        if karaoke {
            af.push("pan=stereo|c0=c0-c1|c1=c1-c0".into());
        }
        if swap {
            af.push("pan=stereo|c0=c1|c1=c0".into());
        }
        if widen > 0.0 {
            af.push(format!("extrastereo=m={}", 1.0 + widen));
        }
        if echo {
            af.push("aecho=0.8:0.88:60:0.4".into());
        }
        if reverb {
            af.push("aecho=0.8:0.9:1000|1800:0.3|0.25".into());
        }
        if comp {
            af.push("acompressor=threshold=-18dB:ratio=3:attack=20:release=250:makeup=2".into());
        }
        if vol != 0.0 {
            af.push(format!("volume={vol}dB"));
        }
        if mono {
            af.push("pan=mono|c0=0.5*c0+0.5*c1".into());
        }
        if limiter {
            af.push("alimiter=limit=0.95".into());
        }
        af
    };
    if base.is_empty() && pitch == 0.0 {
        bail!("No effect is enabled");
    }
    filter_audio(
        ctx,
        files,
        &|it| {
            let mut chain = vec![];
            if pitch != 0.0 {
                let sr = sample_rate(ctx, &it.path_buf());
                let r = 2f64.powf(pitch / 12.0);
                chain.push(format!("asetrate={:.0},aresample={:.0},{}", sr * r, sr, atempo(1.0 / r)));
            }
            chain.extend(base.iter().cloned());
            chain.join(",")
        },
        kbps,
    )
}

pub fn visual(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let mode = p.str("mode", "waveform");
    let w = p.i64("width", 1600).clamp(64, 8192);
    let h = p.i64("height", 400).clamp(32, 4096);
    let fg = ffcolor(&p.str("color", "#4a92ff"));
    let bg = ffcolor(&p.str("background", "#0b0e14"));
    let transparent = p.bool("transparent", false);
    let scale = p.str("scale", "sqrt");
    let split = p.bool("split_channels", false);
    let scheme = p.str("palette", "magma");
    let legend = p.bool("legend", true);
    let fmt = if p.str("format", "png") == "jpg" { "jpg" } else { "png" };
    each(ctx, files, &["audio", "video"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        if !info.has_audio {
            bail!("no audio track");
        }
        let suffix = if mode == "spectrogram" { "spectrogram" } else { "waveform" };
        let out = ctx.out_path(it, &format!("{}_{suffix}", it.stem()), fmt)?;
        let fc = if mode == "spectrogram" {
            format!("[0:a:0]showspectrumpic=s={w}x{h}:legend={}:color={scheme}:scale=log[v]", if legend { 1 } else { 0 })
        } else {
            let wave = format!("[0:a:0]showwavespic=s={w}x{h}:colors={fg}:scale={scale}:split_channels={}", if split { 1 } else { 0 });
            if transparent && fmt == "png" {
                format!("{wave}[v]")
            } else {
                format!("{wave}[wv];color=c={bg}:s={w}x{h}[bg];[bg][wv]overlay=format=auto,format=rgb24[v]")
            }
        };
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        a.push("-filter_complex".into());
        a.push(fc);
        a.extend(sv(&["-map", "[v]", "-frames:v", "1"]));
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}

pub fn tags(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let keys = ["title", "artist", "album", "album_artist", "date", "genre", "track", "composer", "comment"];
    let values: Vec<(&str, String)> = keys.iter().map(|k| (*k, p.str(k, ""))).filter(|(_, v)| !v.trim().is_empty()).collect();
    let cover = p.str("cover", "");
    let strip = p.bool("strip", false);
    let cover_ok = !cover.trim().is_empty() && Path::new(cover.trim()).is_file();
    if !cover.trim().is_empty() && !cover_ok {
        bail!("The cover image was not found");
    }
    each(ctx, files, &["audio"], |it, on| {
        let info = media_info(ctx.tools, &it.path_buf());
        let pic_ok = matches!(it.ext.as_str(), "mp3" | "m4a" | "m4b" | "flac" | "aac");
        let mut a = sv(&["-i"]);
        a.push(it.path.clone());
        let with_cover = cover_ok && pic_ok;
        if cover_ok && !pic_ok {
            ctx.warn(format!("{}: .{} can't hold cover art here, the cover was skipped", it.name, it.ext));
        }
        if with_cover {
            a.push("-i".into());
            a.push(cover.trim().to_string());
        }
        a.extend(sv(&["-map", "0:a"]));
        if with_cover {
            a.extend(sv(&["-map", "1:0", "-c:v", "mjpeg", "-disposition:v:0", "attached_pic", "-metadata:s:v", "title=Album cover", "-metadata:s:v", "comment=Cover (front)"]));
        } else if !strip && pic_ok {
            a.extend(sv(&["-map", "0:v?", "-c:v", "copy"]));
        }
        a.extend(sv(&["-c:a", "copy", "-map_metadata", if strip { "-1" } else { "0" }]));
        for (k, v) in &values {
            a.push("-metadata".into());
            a.push(format!("{k}={}", v.trim()));
        }
        if it.ext == "mp3" {
            a.extend(sv(&["-id3v2_version", "3"]));
        }
        let out = ctx.out_path(it, &it.stem(), &it.ext)?;
        a.push(ps(&out));
        ff(ctx, &a, info.duration, on, None)?;
        Ok(vec![it.derive(&out)])
    })
}
