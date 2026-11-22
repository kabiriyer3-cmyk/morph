use std::path::Path;

pub const IMAGE: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "jfif", "webp", "gif", "bmp", "tif", "tiff", "ico", "tga", "qoi", "avif", "hdr",
    "exr", "pnm", "pbm", "pgm", "ppm", "pam", "dds", "ff", "svg",
];
pub const VIDEO: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts", "mts", "3gp", "ogv", "vob",
];
pub const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "oga", "opus", "m4a", "aac", "wma", "aiff", "aif", "alac", "ape", "mka", "ac3",
];
pub const MESH: &[&str] = &["obj", "stl", "ply", "off", "glb", "gltf"];
pub const ARCHIVE: &[&str] = &["zip", "7z", "tar", "gz", "tgz", "zst", "tzst", "xz", "txz", "bz2", "tbz2", "tbz"];
pub const DATA: &[&str] = &["json", "yaml", "yml", "toml", "csv", "tsv", "ndjson", "jsonl", "xml"];
pub const DOC: &[&str] = &[
    "pdf", "doc", "docx", "odt", "rtf", "xls", "xlsx", "ods", "ppt", "pptx", "odp", "md", "markdown", "html", "htm",
    "epub", "tex", "rst", "org",
];
pub const TEXT: &[&str] = &[
    "txt", "log", "ini", "cfg", "conf", "env", "srt", "vtt", "ass", "rs", "js", "mjs", "ts", "tsx", "jsx", "py", "c",
    "h", "cpp", "hpp", "cs", "java", "kt", "go", "rb", "php", "lua", "css", "scss", "sh", "bat", "ps1", "sql",
];

/// Text-like document formats that can be edited as plain text.
pub const TEXTUAL_DOC: &[&str] = &["md", "markdown", "html", "htm", "tex", "rst", "org"];

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
