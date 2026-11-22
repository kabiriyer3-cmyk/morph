<p align='center'>
  <img src='assets/banner.svg' alt='Morph' width='100%'>
</p>

<p align='center'>
  <a href='https://github.com/kabiriyer3-cmyk/morph/releases/latest'><img alt='Release' src='https://img.shields.io/github/v/release/kabiriyer3-cmyk/morph?style=flat-square&color=2f6ff2'></a>
  <a href='https://github.com/kabiriyer3-cmyk/morph/actions/workflows/ci.yml'><img alt='Build' src='https://img.shields.io/github/actions/workflow/status/kabiriyer3-cmyk/morph/ci.yml?branch=main&style=flat-square&label=build'></a>
  <a href='https://github.com/kabiriyer3-cmyk/morph/releases'><img alt='Downloads' src='https://img.shields.io/github/downloads/kabiriyer3-cmyk/morph/total?style=flat-square&color=1259e0'></a>
  <img alt='Platforms' src='https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-0b1530?style=flat-square'>
  <a href='LICENSE'><img alt='License' src='https://img.shields.io/badge/license-MIT-4a92ff?style=flat-square'></a>
</p>

<p align='center'><b>English</b> · <a href='README.ru.md'>Русский</a></p>

**Morph** is a free, open-source desktop app that compresses, converts and batch-processes your files with a node graph, the way ComfyUI does it for AI images, but for everyday files: photos, video, audio, 3D models, archives, data and documents. Wire a few nodes together, drop a folder onto the canvas and press **Run**. Your originals are never touched.

<p align='center'><img src='docs/screenshot.png' alt='Morph screenshot' width='100%'></p>

## Why Morph

- **Node workflows.** Chain steps visually: resize, convert, watermark, save. Branch, merge, bypass and reuse them. Workflows are saved as `.morph.json` files you can share.
- **One tool for every format.** Images, video, audio, 3D, archives, data, office documents and markup in one window.
- **Flexible settings per format.** WebP/AVIF quality and target size, PNG palette and optimization, CRF and presets, GPU encoding, loudness normalization, mesh decimation and much more.
- **Previews everywhere.** Thumbnails right inside the graph, a before/after slider for images, a video and audio player and an interactive 3D viewer for models.
- **Fast and private.** Rust backend, multi-core batch processing, nothing leaves your computer. No accounts, no telemetry.
- **Safe by design.** Every node writes into a temporary work folder. Only the Save node writes to your disk, and it never overwrites anything unless you tell it to.

## Download

| Platform | Package | |
|---|---|---|
| Windows 10/11 x64 | Installer `Morph_x.y.z_x64-setup.exe` | [Latest release](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Windows 10/11 x64 | MSI `Morph_x.y.z_x64_en-US.msi` | [Latest release](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Windows 10/11 x64 | Portable `Morph_x.y.z_x64_portable.exe`, no install needed | [Latest release](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Linux x64 | AppImage, runs on most distros | [Latest release](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Debian / Ubuntu / Mint | `.deb` | [Latest release](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Fedora / openSUSE | `.rpm` | [Latest release](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |

Install the `.deb` with `sudo apt install ./Morph_*_amd64.deb`, the `.rpm` with `sudo dnf install ./Morph-*.x86_64.rpm`, or make the AppImage executable with `chmod +x Morph_*.AppImage`.

## Quick start

1. Open Morph. The **Photos for the web** template is loaded.
2. Drop a folder or some photos onto the canvas. They go straight into the **Load Files** node.
3. Press **Run** or `Ctrl+Enter`. Thumbnails appear in **Preview** and the results land in `~/Morph Output` (change it in the **Save** node).

Pick another template in the sidebar or double-click the canvas to search for any node.

## Nodes

| Category | Nodes |
|---|---|
| **Files** | Load Files, Save, Preview, Filter (type, glob, regex, size), Split by Type, Merge, Rename (patterns like `{name}` `{n:3}` `{date}`, find and replace, case), Checksums (SHA-256, SHA-1, SHA-512, MD5, BLAKE3, CRC32), Remove Duplicates, File Info (CSV or JSON report) |
| **Image** | Resize (fit, fill, stretch, percent), Crop (aspect, box, auto-trim), Rotate / Flip, Adjust (brightness, contrast, saturation, hue, gamma, blur, sharpen, grayscale, invert), Convert (WebP, AVIF, JPEG, PNG, GIF, BMP, TIFF, ICO, TGA, QOI with quality, lossless, target size in KB, PNG optimization and palette), Watermark, Contact Sheet, App Icons (.ico and PNGs), Images to PDF |
| **Video** | Convert (MP4, MKV, WebM, MOV with H.264, H.265, VP9, AV1, ProRes, CRF, presets, target size, resolution, fps, NVENC, QSV, AMF), Trim, Transform (rotate, flip, crop, speed, mute), Video to GIF or animated WebP, Extract Frames, Extract Audio, Join Videos |
| **Audio** | Convert (MP3, AAC, Opus, OGG, FLAC, WAV, ALAC with bitrate, sample rate, channels), Process (volume, loudness normalization, cutting, fades, speed) |
| **3D** | Convert (GLB / glTF, OBJ, STL, PLY, OFF), Transform (scale, units, rotate, move, center, put on ground, fit size, up axis, flip normals), Optimize (weld, decimate, smooth or flat normals) |
| **Archive** | Create (ZIP, 7z, tar, tar.gz, tar.zst, tar.xz, tar.bz2, one archive or one per file), Extract, Compress single files (zstd, gzip, xz, bzip2) |
| **Data & Docs** | Convert Data (JSON, YAML, TOML, CSV, TSV, NDJSON), Text Tools (encodings including Windows-1251 and KOI8-R, line endings, trimming, find and replace), Office Convert (DOCX, XLSX, PPTX, ODT, PDF and more through LibreOffice), Pandoc Convert (Markdown, HTML, DOCX, EPUB, LaTeX, RST) |
| **Utility** | Note |

Every node accepts any files. Files of a kind a node does not handle pass through unchanged, so a whole mixed folder can flow through a chain.

## Templates

Photos for the web · Compress videos · Video to GIF · Convert everything · 3D for the web · Backup with checksums · Photos to PDF · App icons · JSON to YAML · Find duplicates

## Shortcuts

| Keys | Action |
|---|---|
| `Ctrl+Enter` | Run or cancel |
| `Tab` or double-click on the canvas | Search and add a node |
| `Delete` | Delete selected nodes |
| `Ctrl+C` / `Ctrl+V` / `Ctrl+D` | Copy / paste / duplicate |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo |
| `Ctrl+B` | Bypass selected nodes |
| `F` | Fit the graph to the screen |
| `Space` + drag or middle mouse button | Pan |
| Mouse wheel | Zoom |
| `Ctrl` + drag | Box select |
| `Ctrl+S` / `Ctrl+O` | Save / open a workflow |

## Optional tools

Images, 3D, archives, data and text work out of the box. A few node groups use well-known free tools. Morph finds them automatically (PATH, standard install folders or a `tools` folder next to `Morph.exe`), and you can also set the paths in **Settings**.

| Tool | Needed for | Windows | Linux |
|---|---|---|---|
| [FFmpeg](https://ffmpeg.org) | Video and Audio nodes, video and audio thumbnails | `winget install Gyan.FFmpeg` | `sudo apt install ffmpeg` |
| [LibreOffice](https://www.libreoffice.org) | Office Convert | `winget install TheDocumentFoundation.LibreOffice` | `sudo apt install libreoffice` |
| [Pandoc](https://pandoc.org) | Pandoc Convert | `winget install JohnMacFarlane.Pandoc` | `sudo apt install pandoc` |

On Fedora use `sudo dnf install ...` (FFmpeg comes from [RPM Fusion](https://rpmfusion.org/Configuration)), on Arch `sudo pacman -S ffmpeg libreoffice-fresh pandoc`.

## Build from source

Requirements: [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 20 or newer and, on Linux, the WebKitGTK development packages:

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev build-essential
```

On Windows you need the Visual Studio C++ Build Tools and WebView2 (preinstalled on Windows 10 and 11).

```bash
git clone https://github.com/kabiriyer3-cmyk/morph.git
cd morph
npm install
npm run dev     # start the app
npm run build   # installers land in src-tauri/target/release/bundle
```

The UI is plain HTML, CSS and JavaScript in `src/` with no bundler. You can try it in a browser with built-in demo data:

```bash
python3 -m http.server 8080 --directory src
# then open http://localhost:8080/?demo=1
```

`npm run check` runs the static frontend checks and `npm run smoke` renders the demo in headless Chrome.

## Project layout

```
src/                 UI: index.html, css/, js/ (editor, widgets, inspector, 3D viewer, i18n)
src-tauri/           Rust backend (Tauri 2)
  src/engine.rs      graph executor, progress events, work folders
  src/ops/           node implementations: files, img, media (FFmpeg), mesh3d, arch, data, docs
  src/mesh.rs        mesh loading, writing, welding and decimation
assets/              logo, icon and banner (app icons are generated from assets/icon.svg at build time)
.github/actions/     CI and release steps used by the workflows
scripts/             frontend checks, smoke test, cargo error annotations
```

## Releasing

1. Bump `version` in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`.
2. Merge into `main`. The **Release** workflow builds the Windows and Linux packages on GitHub Actions and publishes the release `vX.Y.Z` with the installer, MSI, portable exe, AppImage, deb and rpm.

You can also push a `vX.Y.Z` tag or start **Release** manually in the Actions tab. Release notes come from `.github/release-notes.md`.

## FAQ

**Windows SmartScreen says the app comes from an unknown publisher.** The builds are not code-signed yet. Click *More info*, then *Run anyway*, or use the portable exe.

**The AppImage does not start.** Make it executable with `chmod +x`. Some distros also need `libfuse2` (`sudo apt install libfuse2`).

**Video nodes say FFmpeg was not found.** Install FFmpeg (see [Optional tools](#optional-tools)) and press *Detect again* in Settings, or point Morph to the ffmpeg executable manually.

**Where do intermediate files go?** Into `morph/runs` inside the system temp folder. Only the last few runs are kept, and *Settings, Clear cache* removes them.

**Does Morph upload my files anywhere?** No. Everything runs locally and nothing is sent over the network.

**Can it handle thousands of files?** Yes. Work is spread over all CPU cores. Previews show the first files and the inspector lists up to 500 results per output.

## Contributing

Issues and pull requests are welcome. A new node needs two pieces: an entry in `src/js/nodes.js` (sockets and widgets) and a handler in `src-tauri/src/ops/` registered in `ops/mod.rs`. CI checks that both sides match.

## License

[MIT](LICENSE) © Morph contributors
