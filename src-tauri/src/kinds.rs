use std::path::Path;

pub const IMAGE: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "jfif", "webp", "gif", "bmp", "tif", "tiff", "ico", "tga", "qoi", "avif", "hdr",
    "exr", "pnm", "pbm", "pgm", "ppm", "pam", "dds", "ff", "svg", "heic", "heif", "jxl", "jp2", "psd", "apng",
];
pub const VIDEO: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts", "mts", "3gp", "ogv", "vob",
    "mxf", "f4v", "asf", "rm", "rmvb", "divx", "dv", "y4m", "3g2",
];
pub const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "oga", "opus", "m4a", "aac", "wma", "aiff", "aif", "alac", "ape", "mka", "ac3",
    "amr", "m4r", "m4b", "caf", "dts", "au", "mp2", "wv", "tta", "eac3",
];
pub const MESH: &[&str] = &["obj", "stl", "ply", "off", "glb", "gltf"];
pub const ARCHIVE: &[&str] = &[
    "zip", "7z", "tar", "gz", "tgz", "zst", "tzst", "xz", "txz", "bz2", "tbz2", "tbz", "rar", "iso", "cab",
];
pub const DATA: &[&str] = &["json", "yaml", "yml", "toml", "csv", "tsv", "ndjson", "jsonl", "xml", "geojson"];
pub const DOC: &[&str] = &[
    "pdf", "doc", "docx", "odt", "rtf", "xls", "xlsx", "xlsm", "xlsb", "ods", "ppt", "pptx", "odp", "odg", "md",
    "markdown", "html", "htm", "epub", "tex", "rst", "org", "fodt", "fods", "ipynb",
];
pub const TEXT: &[&str] = &[
    "txt", "log", "ini", "cfg", "conf", "env", "srt", "vtt", "ass", "rs", "js", "mjs", "tsx", "jsx", "py", "c", "h",
    "cpp", "hpp", "cs", "java", "kt", "go", "rb", "php", "lua", "css", "scss", "sh", "bat", "ps1", "sql", "b64",
];

/// Text-like document formats that can be edited as plain text.
pub const TEXTUAL_DOC: &[&str] = &["md", "markdown", "html", "htm", "tex", "rst", "org", "ipynb"];

pub fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

pub fn kind_of(ext: &str) -> &'static str {
    let e = ext.to_ascii_lowercase();
    let e = e.as_str();
    for (k, list) in [
        ("image", IMAGE),
        ("video", VIDEO),
        ("audio", AUDIO),
        ("mesh", MESH),
        ("archive", ARCHIVE),
        ("data", DATA),
        ("doc", DOC),
        ("text", TEXT),
    ] {
        if list.contains(&e) {
            return k;
        }
    }
    "other"
}

pub fn is_textual(kind: &str, ext: &str) -> bool {
    matches!(kind, "text" | "data") || (kind == "doc" && TEXTUAL_DOC.contains(&ext))
}
