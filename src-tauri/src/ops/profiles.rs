//! Device and format profiles for Convert Video: the "Profile" list of classic
//! converters (Bigasoft, Format Factory, Freemake…). A profile fills in the container,
//! codecs, picture size, frame rate and bitrates; preset, hardware encoder, subtitles
//! and metadata stay as set on the node. Generated from one table together with
//! src/js/profiles.js, keep both in sync.
use super::{av, split_kinds};
use crate::engine::{Ctx, FileItem};
use crate::ops::media::media_info;
use crate::util::Params;
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

/// id, container, video codec, CRF, video kbps (0 = CRF), width, height, max side, fit,
/// fps (0 = keep), max fps (0 = any), audio codec, audio kbps, sample rate, channels
type Row = (
    &'static str,
    &'static str,
    &'static str,
    i64,
    i64,
    i64,
    i64,
    i64,
    &'static str,
    f64,
    f64,
    &'static str,
    i64,
    i64,
    &'static str,
);

#[rustfmt::skip]
static PROFILES: &[Row] = &[
    ("mp4", "mp4", "h264", 21, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 192, 0, "keep"),
    ("mp4_hevc", "mp4", "h265", 24, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 192, 0, "keep"),
    ("mkv", "mkv", "h264", 20, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 224, 0, "keep"),
    ("mov", "mov", "h264", 20, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 224, 0, "keep"),
    ("avi", "avi", "mpeg4", 22, 0, 0, 0, 0, "fit", 0.0, 0.0, "mp3", 192, 0, "keep"),
    ("wmv", "wmv", "wmv2", 0, 3000, 0, 0, 0, "fit", 0.0, 0.0, "wma", 160, 0, "stereo"),
    ("flv", "flv", "h264", 23, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 128, 44100, "stereo"),
    ("webm", "webm", "vp9", 25, 0, 0, 0, 0, "fit", 0.0, 0.0, "opus", 128, 0, "keep"),
    ("ogv", "ogv", "theora", 23, 0, 0, 0, 0, "fit", 0.0, 0.0, "vorbis", 160, 0, "keep"),
    ("mpeg2", "mpg", "mpeg2", 0, 6000, 0, 0, 0, "fit", 0.0, 0.0, "mp2", 224, 48000, "stereo"),
    ("mpeg1", "mpg", "mpeg1", 0, 2000, 0, 0, 0, "fit", 0.0, 0.0, "mp2", 224, 44100, "stereo"),
    ("ts", "ts", "h264", 21, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 192, 0, "keep"),
    ("m4v", "m4v", "h264", 21, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 192, 0, "stereo"),
    ("3gp", "3gp", "h264", 28, 0, 0, 0, 480, "fit", 0.0, 30.0, "aac", 64, 22050, "mono"),
    ("hd720", "mp4", "h264", 21, 0, 1280, 720, 0, "fit", 0.0, 0.0, "aac", 192, 0, "keep"),
    ("hd1080", "mp4", "h264", 20, 0, 1920, 1080, 0, "fit", 0.0, 0.0, "aac", 192, 0, "keep"),
    ("hd1080_hevc", "mp4", "h265", 23, 0, 1920, 1080, 0, "fit", 0.0, 0.0, "aac", 192, 0, "keep"),
    ("uhd4k", "mp4", "h265", 22, 0, 3840, 2160, 0, "fit", 0.0, 0.0, "aac", 256, 0, "keep"),
    ("uhd4k_h264", "mp4", "h264", 20, 0, 3840, 2160, 0, "fit", 0.0, 0.0, "aac", 256, 0, "keep"),
    ("hd_mkv", "mkv", "h264", 19, 0, 1920, 1080, 0, "fit", 0.0, 0.0, "ac3", 448, 48000, "keep"),
    ("avchd", "m2ts", "h264", 18, 0, 1920, 1080, 0, "fit", 0.0, 0.0, "ac3", 384, 48000, "keep"),
    ("hd_ts", "ts", "h264", 20, 0, 1920, 1080, 0, "fit", 0.0, 0.0, "ac3", 384, 48000, "keep"),
    ("hd_mov", "mov", "h264", 18, 0, 1920, 1080, 0, "fit", 0.0, 0.0, "aac", 256, 0, "keep"),
    ("hd_wmv", "wmv", "wmv2", 0, 8000, 1280, 720, 0, "fit", 0.0, 0.0, "wma", 192, 0, "stereo"),
    ("iphone", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("iphone_hevc", "mp4", "h265", 23, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("ipad", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("ipad_pro", "mp4", "h265", 22, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 256, 0, "stereo"),
    ("apple_tv", "mp4", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 256, 0, "keep"),
    ("apple_tv_4k", "mp4", "h265", 21, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 256, 0, "keep"),
    ("ipod", "m4v", "h264", 22, 0, 0, 0, 640, "fit", 0.0, 30.0, "aac", 128, 44100, "stereo"),
    ("final_cut", "mov", "prores", 0, 0, 0, 0, 0, "fit", 0.0, 0.0, "pcm", 0, 48000, "keep"),
    ("imovie", "mov", "h264", 18, 0, 0, 0, 3840, "fit", 0.0, 0.0, "aac", 256, 48000, "keep"),
    ("android", "mp4", "h264", 22, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 160, 0, "stereo"),
    ("android_720", "mp4", "h264", 24, 0, 0, 0, 1280, "fit", 0.0, 30.0, "aac", 128, 0, "stereo"),
    ("android_tablet", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("galaxy", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("galaxy_4k", "mp4", "h265", 23, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("galaxy_tab", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("pixel", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("xiaomi", "mp4", "h264", 22, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 160, 0, "stereo"),
    ("huawei", "mp4", "h264", 22, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 160, 0, "stereo"),
    ("android_old", "mp4", "h264", 24, 0, 0, 0, 854, "fit", 0.0, 30.0, "aac", 96, 44100, "stereo"),
    ("ps5", "mp4", "h265", 22, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 256, 0, "keep"),
    ("ps4", "mp4", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("ps3", "mp4", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 30.0, "aac", 192, 48000, "stereo"),
    ("psp", "mp4", "h264", 22, 0, 480, 272, 0, "pad", 0.0, 30.0, "aac", 128, 48000, "stereo"),
    ("ps_vita", "mp4", "h264", 22, 0, 960, 544, 0, "fit", 0.0, 30.0, "aac", 128, 48000, "stereo"),
    ("xbox_series", "mp4", "h265", 22, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 256, 0, "keep"),
    ("xbox_one", "mp4", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("xbox_360", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 30.0, "aac", 160, 48000, "stereo"),
    ("switch", "mp4", "h264", 22, 0, 0, 0, 1280, "fit", 0.0, 60.0, "aac", 128, 48000, "stereo"),
    ("steam_deck", "mp4", "h264", 21, 0, 1280, 800, 0, "fit", 0.0, 60.0, "aac", 160, 0, "stereo"),
    ("tv_4k", "mp4", "h265", 21, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 256, 0, "keep"),
    ("tv_1080", "mp4", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 256, 0, "keep"),
    ("tv_mkv", "mkv", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 60.0, "ac3", 448, 48000, "keep"),
    ("chromecast", "mp4", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("roku", "mp4", "h264", 20, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 192, 0, "stereo"),
    ("dvd_ntsc", "mpg", "mpeg2", 0, 6000, 720, 480, 0, "pad", 29.97, 0.0, "ac3", 192, 48000, "stereo"),
    ("dvd_pal", "mpg", "mpeg2", 0, 6000, 720, 576, 0, "pad", 25.0, 0.0, "ac3", 192, 48000, "stereo"),
    ("svcd_ntsc", "mpg", "mpeg2", 0, 2200, 480, 480, 0, "pad", 29.97, 0.0, "mp2", 224, 44100, "stereo"),
    ("svcd_pal", "mpg", "mpeg2", 0, 2200, 480, 576, 0, "pad", 25.0, 0.0, "mp2", 224, 44100, "stereo"),
    ("vcd_ntsc", "mpg", "mpeg1", 0, 1150, 352, 240, 0, "pad", 29.97, 0.0, "mp2", 224, 44100, "stereo"),
    ("vcd_pal", "mpg", "mpeg1", 0, 1150, 352, 288, 0, "pad", 25.0, 0.0, "mp2", 224, 44100, "stereo"),
    ("youtube", "mp4", "h264", 18, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 320, 48000, "stereo"),
    ("youtube_4k", "mp4", "h264", 18, 0, 0, 0, 3840, "fit", 0.0, 60.0, "aac", 320, 48000, "stereo"),
    ("vimeo", "mp4", "h264", 18, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 320, 48000, "stereo"),
    ("shorts", "mp4", "h264", 20, 0, 1080, 1920, 0, "pad", 0.0, 60.0, "aac", 192, 48000, "stereo"),
    ("ig_feed", "mp4", "h264", 20, 0, 1080, 1350, 0, "pad", 0.0, 60.0, "aac", 192, 48000, "stereo"),
    ("square", "mp4", "h264", 20, 0, 1080, 1080, 0, "pad", 0.0, 60.0, "aac", 192, 48000, "stereo"),
    ("x_twitter", "mp4", "h264", 22, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 128, 48000, "stereo"),
    ("facebook", "mp4", "h264", 21, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 160, 48000, "stereo"),
    ("telegram", "mp4", "h264", 23, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 128, 0, "stereo"),
    ("web_mp4", "mp4", "h264", 23, 0, 0, 0, 1920, "fit", 0.0, 60.0, "aac", 128, 0, "stereo"),
    ("web_webm", "webm", "vp9", 24, 0, 0, 0, 1920, "fit", 0.0, 60.0, "opus", 128, 0, "stereo"),
    ("web_av1", "webm", "av1", 26, 0, 0, 0, 1920, "fit", 0.0, 60.0, "opus", 128, 0, "stereo"),
    ("prores", "mov", "prores", 0, 0, 0, 0, 0, "fit", 0.0, 0.0, "pcm", 0, 48000, "keep"),
    ("dnxhr", "mov", "dnxhd", 0, 0, 0, 0, 0, "fit", 0.0, 0.0, "pcm", 0, 48000, "keep"),
    ("ffv1", "mkv", "ffv1", 0, 0, 0, 0, 0, "fit", 0.0, 0.0, "flac", 0, 0, "keep"),
    ("mjpeg", "avi", "mjpeg", 16, 0, 0, 0, 0, "fit", 0.0, 0.0, "pcm", 0, 48000, "keep"),
    ("h264_master", "mp4", "h264", 14, 0, 0, 0, 0, "fit", 0.0, 0.0, "aac", 320, 48000, "keep"),
];

/// All profile ids (used by the frontend check).
pub fn ids() -> Vec<&'static str> {
    PROFILES.iter().map(|r| r.0).collect()
}

/// Writes the settings of profile `id` into node values. Returns false for unknown ids.
pub fn apply(id: &str, m: &mut Map<String, Value>) -> bool {
    let Some(&(_, cont, vc, crf, kbps, w, h, max, fit, fps, max_fps, ac, abr, ar, ch)) = PROFILES.iter().find(|r| r.0 == id) else {
        return false;
    };
    let mut set = |k: &str, v: Value| {
        m.insert(k.to_string(), v);
    };
    set("container", json!(cont));
    set("video_codec", json!(vc));
    if kbps > 0 {
        set("rate_mode", json!("bitrate"));
        set("video_bitrate", json!(kbps));
    } else {
        set("rate_mode", json!("quality"));
        set("quality", json!(crf));
    }
    set("width", json!(w));
    set("height", json!(h));
    set("max_size", json!(max));
    set("fit", json!(fit));
    set("fps", json!(fps));
    set("max_fps", json!(max_fps));
    set("two_pass", json!(false));
    set("tune", json!("none"));
    set("audio_codec", json!(ac));
    if abr > 0 {
        set("audio_bitrate", json!(abr));
    }
    set("sample_rate", json!(ar));
    set("channels", json!(ch));
    true
}

/// Convert Video: applies the profile (if any), then caps the frame rate per file so
/// slower sources keep their own rate.
pub fn convert(ctx: &Ctx, p: &Params, files: Vec<FileItem>) -> Result<Vec<FileItem>> {
    let id = p.str("profile", "custom");
    if id.is_empty() || id == "custom" {
        return av::video_convert(ctx, p, files);
    }
    let mut m = p.0.clone();
    if !apply(&id, &mut m) {
        bail!("Unknown profile '{id}'");
    }
    let max_fps = m.get("max_fps").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let forced = m.get("fps").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if max_fps <= 0.0 || forced > 0.0 {
        return av::video_convert(ctx, &Params(&m), files);
    }
    let (videos, pass) = split_kinds(files, &["video"]);
    let total = videos.len();
    let mut out = vec![];
    for it in videos {
        ctx.check()?;
        let info = media_info(ctx.tools, &it.path_buf());
        let mut one = m.clone();
        if info.fps > max_fps + 0.5 {
            one.insert("fps".into(), json!(max_fps));
        }
        match av::video_convert(ctx, &Params(&one), vec![it.clone()]) {
            Ok(v) => out.extend(v),
            Err(e) => {
                ctx.check()?;
                ctx.warn(format!("{}: {e:#}", it.name));
            }
        }
    }
    if total > 0 && out.is_empty() {
        bail!("All {total} file(s) failed. {}", ctx.first_warning());
    }
    out.extend(pass);
    Ok(out)
}
