// Node catalog. Pure data: imported by the UI and by scripts/check-frontend.mjs.
// Widget names and option values must match what src-tauri/src/ops reads.
// Widget flags: group (section title), adv (hidden under "Advanced"), seg (segmented control),
// groups (searchable grouped picker for long option lists), when/unless (conditional visibility).

export const TYPES = {
  FILES: '#9AA4B8',
  IMAGE: '#5AC8FA',
  VIDEO: '#A78BFA',
  AUDIO: '#2DD4BF',
  MESH: '#FB923C',
  ARCHIVE: '#F87171',
  DATA: '#FBBF24',
  DOC: '#F472B6',
  SHEET: '#4ADE80',
  PDF: '#FF7A90',
  TEXT: '#CBD5E1',
};

export const CATEGORIES = {
  files: { color: '#9AA4B8', icon: 'files' },
  convert: { color: '#4A92FF', icon: 'convert' },
  image: { color: '#5AC8FA', icon: 'image' },
  video: { color: '#A78BFA', icon: 'video' },
  audio: { color: '#2DD4BF', icon: 'audio' },
  doc: { color: '#F472B6', icon: 'doc' },
  pdf: { color: '#FF7A90', icon: 'pdf' },
  sheet: { color: '#4ADE80', icon: 'sheet' },
  data: { color: '#FBBF24', icon: 'data' },
  text: { color: '#CBD5E1', icon: 'text' },
  mesh: { color: '#FB923C', icon: 'mesh' },
  archive: { color: '#F87171', icon: 'archive' },
  util: { color: '#8B93A7', icon: 'puzzle' },
};

export const typeColor = (type) => TYPES[type] || TYPES.FILES;
// FILES is the wildcard: any list of files can go anywhere, nodes skip kinds they do not handle.
// Related document types can be wired into each other too.
const COMPAT = { SHEET: ['DATA', 'DOC'], DATA: ['SHEET', 'TEXT'], DOC: ['SHEET', 'PDF', 'TEXT'], PDF: ['DOC'], TEXT: ['DATA', 'DOC'] };
export const compatible = (from, to) => from === to || from === 'FILES' || to === 'FILES' || (COMPAT[to] || []).includes(from);

const io = (name, type) => ({ name, type });
const num = (name, value, o = {}) => ({ kind: 'number', name, value, step: 1, ...o });
const slider = (name, value, min, max, o = {}) => ({ kind: 'slider', name, value, min, max, step: 1, ...o });
const combo = (name, value, options, o = {}) => ({ kind: 'combo', name, value, options, ...o });
const seg = (name, value, options, o = {}) => ({ kind: 'combo', name, value, options, seg: true, ...o });
const toggle = (name, value, o = {}) => ({ kind: 'toggle', name, value, ...o });
const text = (name, value = '', o = {}) => ({ kind: 'text', name, value, ...o });
const area = (name, value = '', o = {}) => ({ kind: 'textarea', name, value, ...o });
const path = (name, o = {}) => ({ kind: 'path', name, value: '', ...o });
const color = (name, value, o = {}) => ({ kind: 'color', name, value, ...o });
const when = (key, ...values) => ({ show: { [key]: values } });
const unless = (key, ...values) => ({ hide: { [key]: values } });
// Merges visibility rules: all(when('a', 1), when('b', 2))
const all = (...rules) => {
  const r = {};
  for (const x of rules) {
    for (const k of ['show', 'hide']) if (x[k]) r[k] = { ...(r[k] || {}), ...x[k] };
  }
  return r;
};
// Puts a list of widgets into a section and/or behind "Advanced".
const G = (group, list, adv = false) => list.map((w) => ({ ...w, group, ...(adv ? { adv: true } : {}) }));
const ADV = (list) => list.map((w) => ({ ...w, adv: true }));

const FIL = [io('files', 'FILES')];
const IMG = [io('images', 'IMAGE')];
const VID = [io('videos', 'VIDEO')];
const AUD = [io('audio', 'AUDIO')];
const MSH = [io('models', 'MESH')];
const DAT = [io('data', 'DATA')];
const DOC = [io('docs', 'DOC')];
const PDF = [io('pdfs', 'PDF')];
const SHT = [io('tables', 'SHEET')];
const TXT = [io('texts', 'TEXT')];
const REPORT = io('report', 'DATA');

const ANCHORS = ['center', 'top-left', 'top', 'top-right', 'left', 'right', 'bottom-left', 'bottom', 'bottom-right'];
const PRESETS = ['ultrafast', 'superfast', 'veryfast', 'faster', 'fast', 'medium', 'slow', 'slower', 'veryslow'];
const DEG = { 0: '0°', 90: '90°', 180: '180°', 270: '270°' };
const HW = { none: 'CPU', nvidia: 'NVIDIA NVENC', intel: 'Intel QSV', amd: 'AMD AMF' };
const FONTS = ['sans-serif', 'serif', 'monospace', 'Arial', 'Segoe UI', 'Inter', 'Roboto', 'Helvetica', 'Times New Roman', 'Georgia', 'Verdana', 'Impact', 'Courier New', 'DejaVu Sans', 'Noto Sans'];
const IMG_OUT = ['keep', 'png', 'jpg', 'webp', 'avif', 'tiff', 'bmp', 'gif', 'tga', 'qoi'];
const IMG_OUT_LBL = { keep: 'keep format' };
const Q = (v = 92, o = {}) => slider('quality', v, 1, 100, o);

export const VIDEO_CONTAINERS = ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'flv', 'wmv', 'mpg', 'ts', 'm2ts', '3gp', 'ogv', 'mxf', 'keep'];
const VCODECS = ['auto', 'h264', 'h265', 'av1', 'vp9', 'vp8', 'prores', 'dnxhd', 'mpeg4', 'mpeg2', 'wmv2', 'theora', 'mjpeg', 'ffv1', 'copy'];
const VCODEC_LBL = { auto: 'auto (best for container)', h264: 'H.264 / AVC', h265: 'H.265 / HEVC', av1: 'AV1', vp9: 'VP9', vp8: 'VP8', prores: 'Apple ProRes', dnxhd: 'Avid DNxHD', mpeg4: 'MPEG-4 Part 2', mpeg2: 'MPEG-2', wmv2: 'Windows Media 8', theora: 'Theora', mjpeg: 'Motion JPEG', ffv1: 'FFV1 (lossless)', copy: 'copy (no re-encode)' };
const ACODECS = ['auto', 'aac', 'opus', 'mp3', 'vorbis', 'flac', 'alac', 'pcm', 'ac3', 'mp2', 'wma', 'copy', 'none'];
const ACODEC_LBL = { auto: 'auto (best for container)', aac: 'AAC', opus: 'Opus', mp3: 'MP3', vorbis: 'Vorbis', flac: 'FLAC', alac: 'Apple Lossless', pcm: 'PCM (uncompressed)', ac3: 'Dolby AC-3', mp2: 'MP2', wma: 'WMA', copy: 'copy', none: 'no audio' };
export const AUDIO_FORMATS = ['mp3', 'm4a', 'aac', 'opus', 'ogg', 'flac', 'wav', 'alac', 'aiff', 'wma', 'ac3', 'eac3', 'mp2', 'amr', 'm4r', 'm4b', 'copy'];
const AUDIO_FMT_LBL = { m4a: 'm4a (AAC)', alac: 'alac (Apple Lossless)', m4r: 'm4r (iPhone ringtone)', m4b: 'm4b (audiobook)', copy: 'copy (no re-encode)' };
const RATES = ['0', '8000', '16000', '22050', '32000', '44100', '48000', '88200', '96000', '192000'];
const RATE_LBL = { 0: 'keep', 8000: '8 kHz', 16000: '16 kHz', 22050: '22.05 kHz', 32000: '32 kHz', 44100: '44.1 kHz', 48000: '48 kHz', 88200: '88.2 kHz', 96000: '96 kHz', 192000: '192 kHz' };

// Formats offered by convert.any (must match src-tauri/src/ops/convert.rs).
export const CONVERT_TARGETS = {
  Image: ['png', 'jpg', 'webp', 'avif', 'gif', 'bmp', 'tiff', 'ico', 'tga', 'qoi', 'ppm', 'exr'],
  Video: ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'flv', 'wmv', 'mpg', 'ts', 'm2ts', '3gp', 'ogv', 'mxf'],
  Audio: ['mp3', 'm4a', 'aac', 'opus', 'ogg', 'flac', 'wav', 'alac', 'aiff', 'wma', 'ac3', 'amr', 'm4r'],
  Document: ['pdf', 'docx', 'doc', 'odt', 'rtf', 'html', 'md', 'txt', 'epub', 'rst', 'tex', 'org', 'pptx', 'ppt', 'odp'],
  Spreadsheet: ['xlsx', 'xls', 'ods', 'csv', 'tsv', 'sql', 'xml'],
  Data: ['json', 'yaml', 'toml', 'ndjson'],
  '3D': ['glb', 'obj', 'stl', 'ply', 'off'],
  Archive: ['zip', '7z', 'tar', 'tar.gz', 'tar.xz', 'tar.zst', 'tar.bz2'],
};
const CONVERT_ALL = [...new Set(Object.values(CONVERT_TARGETS).flat())];

export const NODE_DEFS = {
  // ---------------------------------------------------------------- files
  'files.load': {
    cat: 'files', title: 'Load Files', icon: 'folder-open', width: 300, keywords: 'open input folder add import',
    desc: 'Pick files or whole folders, or simply drop them onto the canvas.',
    inputs: [], outputs: FIL,
    widgets: [
      { kind: 'paths', name: 'paths', value: [] },
      toggle('recursive', true, { label: 'include subfolders' }),
      ...ADV([
        text('include', '', { placeholder: '*.jpg, *.png' }),
        text('exclude', '', { placeholder: '*.tmp, Thumbs.db' }),
        toggle('skip_hidden', true, { label: 'skip hidden' }),
      ]),
    ],
  },
  'files.save': {
    cat: 'files', title: 'Save', icon: 'save', output: true, keywords: 'export write output folder',
    desc: 'Copies results into a folder. Originals are never modified.',
    inputs: FIL, outputs: [io('saved', 'FILES')],
    widgets: [
      seg('destination', 'folder', ['folder', 'next to original'], { labels: { 'next to original': 'next to source' } }),
      path('folder', { folder: true, placeholder: '~/Morph Output', ...when('destination', 'folder') }),
      text('pattern', '{name}', { label: 'file name', placeholder: '{name}_{n:3}' }),
      combo('on_conflict', 'rename', ['rename', 'overwrite', 'skip'], { label: 'if exists' }),
      ...ADV([
        toggle('keep_structure', true, { label: 'keep folders', ...when('destination', 'folder') }),
        toggle('open_folder', false, { label: 'open folder after' }),
      ]),
    ],
  },
  'files.preview': {
    cat: 'files', title: 'Preview', icon: 'eye', output: true, preview: true, width: 330, keywords: 'view show thumbnails gallery',
    desc: 'Shows thumbnails of the files that reach it. Click one to inspect it.',
    inputs: FIL, outputs: FIL, widgets: [],
  },
  'files.filter': {
    cat: 'files', title: 'Filter', icon: 'funnel', keywords: 'select match glob size type',
    desc: 'Splits files by type, name pattern or size.',
    inputs: FIL, outputs: [io('match', 'FILES'), io('rest', 'FILES')],
    widgets: [
      combo('kind', 'any', ['any', 'image', 'video', 'audio', 'mesh', 'archive', 'data', 'doc', 'text', 'other']),
      text('pattern', '', { label: 'name', placeholder: '*.jpg; IMG_*' }),
      toggle('regex', false, { label: 'regular expression' }),
      ...ADV([num('min_kb', 0, { min: 0, unit: 'KB', label: 'min size' }), num('max_kb', 0, { min: 0, unit: 'KB', label: 'max size' })]),
    ],
  },
  'files.split': {
    cat: 'files', title: 'Split by Type', icon: 'split', keywords: 'route sort kind separate',
    desc: 'Routes images, videos, audio and 3D models to separate outputs.',
    inputs: FIL,
    outputs: [io('images', 'IMAGE'), io('videos', 'VIDEO'), io('audio', 'AUDIO'), io('models', 'MESH'), io('other', 'FILES')],
    widgets: [],
  },
  'files.merge': {
    cat: 'files', title: 'Merge', icon: 'merge', keywords: 'join combine union',
    desc: 'Joins up to four file lists into one.',
    inputs: [io('a', 'FILES'), io('b', 'FILES'), io('c', 'FILES'), io('d', 'FILES')], outputs: FIL, widgets: [],
  },
  'files.rename': {
    cat: 'files', title: 'Rename', icon: 'cursor', keywords: 'batch rename number pattern',
    desc: 'Batch rename with patterns, numbering, find and replace and case changes.',
    inputs: FIL, outputs: FIL,
    widgets: [
      text('pattern', '{name}', { placeholder: '{name}_{n}' }),
      text('find', ''),
      text('replace', '', { label: 'replace with' }),
      toggle('regex', false, { label: 'regular expression' }),
      combo('case', 'keep', ['keep', 'lower', 'upper', 'title', 'snake', 'kebab']),
      ...ADV([num('start', 1, { min: 0, label: 'first number' }), num('digits', 3, { min: 0, max: 12 })]),
    ],
  },
  'files.sort': {
    cat: 'files', title: 'Sort & Pick', icon: 'sort', keywords: 'order sort limit first last every random shuffle',
    desc: 'Sorts the list and keeps a range: first N, every Nth or a random sample.',
    inputs: FIL, outputs: FIL,
    widgets: [
      combo('by', 'name', ['name', 'path', 'size', 'modified', 'ext', 'kind', 'random', 'none'], { label: 'sort by' }),
      seg('order', 'asc', ['asc', 'desc'], { labels: { asc: 'A → Z', desc: 'Z → A' }, ...unless('by', 'random', 'none') }),
      num('limit', 0, { min: 0, label: 'keep (0 = all)' }),
      ...ADV([
        num('offset', 0, { min: 0, label: 'skip first' }),
        num('every', 1, { min: 1, label: 'every Nth' }),
        num('seed', 0, { min: 0, label: 'random seed', ...when('by', 'random') }),
      ]),
    ],
  },
  'files.organize': {
    cat: 'files', title: 'Organize into Folders', icon: 'folders', width: 300, keywords: 'sort folders date year month type extension photos',
    desc: 'Puts files into sub folders by type, extension, date taken, size, first letter or your own pattern.',
    inputs: FIL, outputs: FIL,
    widgets: [
      combo('by', 'kind', ['kind', 'ext', 'date', 'taken', 'size', 'letter', 'pattern', 'flatten'], { label: 'group by', labels: { kind: 'file type', ext: 'extension', date: 'date modified', taken: 'date taken (EXIF)', size: 'size', letter: 'first letter', pattern: 'custom pattern', flatten: 'flatten (no folders)' } }),
      combo('date_format', 'yyyy/mm', ['yyyy', 'yyyy-mm', 'yyyy/mm', 'yyyy/mm/dd', 'yyyy-mm-dd'], { label: 'folders', ...when('by', 'date', 'taken') }),
      text('pattern', '{kind}/{yyyy}', { placeholder: '{kind}/{yyyy}/{mm}', ...when('by', 'pattern') }),
      seg('folder_names', 'en', ['en', 'ru'], { label: 'names', labels: { en: 'English', ru: 'Русский' }, ...when('by', 'kind', 'pattern') }),
      ...ADV([toggle('keep_root', false, { label: 'keep top folder' })]),
    ],
  },
  'files.clean_names': {
    cat: 'files', title: 'Clean File Names', icon: 'clean', keywords: 'transliterate latin spaces special characters safe names',
    desc: 'Makes names safe for the web and any OS: transliteration, no spaces or special characters.',
    inputs: FIL, outputs: FIL,
    widgets: [
      toggle('transliterate', true, { label: 'transliterate (Привет → Privet)' }),
      combo('spaces', '_', ['_', '-', 'remove', 'keep'], { label: 'spaces to', labels: { _: 'underscore _', '-': 'dash -', remove: 'remove', keep: 'keep' } }),
      seg('case', 'keep', ['keep', 'lower', 'upper']),
      toggle('remove_special', true, { label: 'remove special characters' }),
      ...ADV([num('max_length', 0, { min: 0, max: 255, label: 'max length (0 = any)' }), toggle('lowercase_ext', true, { label: 'lowercase extension' })]),
    ],
  },
  'files.hash': {
    cat: 'files', title: 'Checksums', icon: 'hash', keywords: 'hash sha md5 verify integrity',
    desc: 'Computes SHA-256, BLAKE3, MD5 and more and writes a report file.',
    inputs: FIL, outputs: [io('files', 'FILES'), REPORT],
    widgets: [
      combo('algorithm', 'sha256', ['sha256', 'sha1', 'md5', 'blake3', 'crc32', 'sha512']),
      seg('format', 'sums', ['sums', 'csv', 'json'], { labels: { sums: 'sum file' } }),
    ],
  },
  'files.verify': {
    cat: 'files', title: 'Verify Checksums', icon: 'shield', keywords: 'check sha256sum md5sum sfv integrity corrupted',
    desc: 'Checks files against .sha256 / .md5 / .sfv checksum lists that come with them.',
    inputs: FIL, outputs: [io('ok', 'FILES'), io('bad', 'FILES'), REPORT], widgets: [],
  },
  'files.dedupe': {
    cat: 'files', title: 'Remove Duplicates', icon: 'copy', keywords: 'duplicate same identical',
    desc: 'Finds identical files by content, name or size.',
    inputs: FIL, outputs: [io('unique', 'FILES'), io('duplicates', 'FILES')],
    widgets: [combo('by', 'content', ['content', 'name', 'name+size'], { label: 'compare' })],
  },
  'files.info': {
    cat: 'files', title: 'File Info', icon: 'info', keywords: 'report list table metadata',
    desc: 'Writes a CSV or JSON table with sizes, dates, dimensions and durations.',
    inputs: FIL, outputs: [io('files', 'FILES'), REPORT],
    widgets: [seg('format', 'csv', ['csv', 'json'])],
  },
  'files.report': {
    cat: 'files', title: 'File Report', icon: 'chart', keywords: 'html report catalog gallery list inventory',
    desc: 'A beautiful HTML catalog (with thumbnails) or a CSV / JSON / Markdown list of all files.',
    inputs: FIL, outputs: [io('files', 'FILES'), REPORT],
    widgets: [
      seg('format', 'html', ['html', 'csv', 'json', 'md']),
      text('title', 'Files'),
      toggle('thumbnails', true, { label: 'image thumbnails', ...when('format', 'html') }),
    ],
  },
  'files.parts': {
    cat: 'files', title: 'Split / Join Parts', icon: 'scissors', keywords: 'split large file parts chunks join 001',
    desc: 'Cuts big files into parts of a given size (file.001, file.002…) or joins such parts back.',
    inputs: FIL, outputs: FIL,
    widgets: [
      seg('mode', 'split', ['split', 'join']),
      num('size_mb', 100, { min: 0.01, step: 10, unit: 'MB', label: 'part size', ...when('mode', 'split') }),
    ],
  },

  // ---------------------------------------------------------------- universal converter
  'convert.any': {
    cat: 'convert', title: 'Convert Anything', icon: 'convert', width: 300, keywords: 'convert format any universal mp4 mp3 pdf docx xlsx csv png jpg',
    desc: 'One node for every format: pick the target and Morph chooses the right engine for each file.',
    inputs: FIL, outputs: FIL,
    widgets: [
      combo('to', 'mp4', CONVERT_ALL, { label: 'convert to', groups: CONVERT_TARGETS }),
      slider('quality', 80, 1, 100),
      num('max_size', 0, { min: 0, unit: 'px', label: 'max image side (0 = keep)' }),
      ...G('Images', [toggle('lossless', false), toggle('combine', false, { label: 'images to one PDF' })], true),
      ...G('Video', [
        combo('video_codec', 'auto', VCODECS, { label: 'video codec', labels: VCODEC_LBL }),
        combo('preset', 'medium', PRESETS),
        combo('hardware', 'none', ['none', 'nvidia', 'intel', 'amd'], { labels: HW }),
        num('seconds', 3, { min: 0.1, step: 0.5, unit: 's', label: 'seconds per image' }),
        num('fps', 12, { min: 1, max: 60, label: 'GIF fps' }),
      ], true),
      ...G('Audio', [num('audio_bitrate', 192, { min: 8, max: 1024, unit: 'kbps', label: 'audio bitrate' })], true),
      ...G('Documents', [seg('theme', 'light', ['light', 'dark', 'serif', 'none'], { label: 'HTML theme' }), num('dpi', 150, { min: 36, max: 1200, label: 'PDF render dpi' })], true),
    ],
  },

  // ---------------------------------------------------------------- images
  'image.resize': {
    cat: 'image', title: 'Resize', icon: 'resize', keywords: 'scale shrink downscale',
    desc: 'Fit, fill, stretch or scale images with high quality filters.',
    inputs: IMG, outputs: IMG,
    widgets: [
      combo('mode', 'fit', ['fit', 'fill', 'stretch', 'percent', 'width', 'height']),
      num('width', 1920, { min: 1, max: 16384, unit: 'px', ...when('mode', 'fit', 'fill', 'stretch', 'width') }),
      num('height', 1080, { min: 1, max: 16384, unit: 'px', ...when('mode', 'fit', 'fill', 'stretch', 'height') }),
      slider('percent', 50, 1, 400, { unit: '%', ...when('mode', 'percent') }),
      toggle('only_shrink', true, { label: 'never upscale' }),
      ...ADV([combo('filter', 'lanczos3', ['lanczos3', 'catmull-rom', 'triangle', 'gaussian', 'nearest'])]),
    ],
  },
  'image.crop': {
    cat: 'image', title: 'Crop', icon: 'crop', keywords: 'cut aspect trim borders',
    desc: 'Crop to an aspect ratio, an exact box or trim empty borders.',
    inputs: IMG, outputs: IMG,
    widgets: [
      seg('mode', 'aspect', ['aspect', 'manual', 'trim']),
      combo('aspect', '1:1', ['1:1', '4:3', '3:2', '16:9', '21:9', '4:5', '3:4', '2:3', '9:16'], when('mode', 'aspect')),
      combo('anchor', 'center', ANCHORS, when('mode', 'aspect')),
      num('x', 0, { min: 0, unit: 'px', ...when('mode', 'manual') }),
      num('y', 0, { min: 0, unit: 'px', ...when('mode', 'manual') }),
      num('width', 512, { min: 1, unit: 'px', ...when('mode', 'manual') }),
      num('height', 512, { min: 1, unit: 'px', ...when('mode', 'manual') }),
      slider('tolerance', 10, 0, 100, { unit: '%', ...when('mode', 'trim') }),
    ],
  },
  'image.transform': {
    cat: 'image', title: 'Rotate / Flip', icon: 'rotate', keywords: 'rotate mirror flip orientation',
    desc: 'Rotate in 90° steps and mirror images.',
    inputs: IMG, outputs: IMG,
    widgets: [
      seg('rotate', '0', ['0', '90', '180', '270'], { labels: DEG }),
      toggle('flip_h', false, { label: 'flip horizontal' }),
      toggle('flip_v', false, { label: 'flip vertical' }),
    ],
  },
  'image.adjust': {
    cat: 'image', title: 'Adjust', icon: 'settings', keywords: 'brightness contrast color blur sharpen grayscale',
    desc: 'Brightness, contrast, saturation, hue, gamma, blur and sharpen.',
    inputs: IMG, outputs: IMG,
    widgets: [
      slider('brightness', 0, -100, 100),
      slider('contrast', 0, -100, 100),
      slider('saturation', 0, -100, 100),
      slider('hue', 0, -180, 180, { unit: '°' }),
      ...ADV([
        slider('gamma', 1, 0.2, 3, { step: 0.05 }),
        slider('blur', 0, 0, 20, { step: 0.5 }),
        slider('sharpen', 0, 0, 10, { step: 0.5 }),
        toggle('grayscale', false),
        toggle('invert', false),
      ]),
    ],
  },
  'image.effects': {
    cat: 'image', title: 'Effects & Filters', icon: 'wand', width: 290, keywords: 'filter sepia grayscale duotone pixelate vignette noise emboss edges posterize',
    desc: 'Sepia, duotone, pixelate, vignette, film grain, emboss, edge detection and more.',
    inputs: IMG, outputs: IMG,
    widgets: [
      combo('effect', 'duotone', ['grayscale', 'sepia', 'invert', 'solarize', 'posterize', 'threshold', 'duotone', 'tint', 'pixelate', 'blur', 'sharpen', 'emboss', 'edges', 'vignette', 'noise', 'flatten', 'alpha-mask'], { labels: { noise: 'film grain', flatten: 'flatten transparency', 'alpha-mask': 'alpha to mask', edges: 'edge detect' } }),
      slider('amount', 100, 0, 100, { unit: '%', ...unless('effect', 'flatten', 'alpha-mask') }),
      num('size', 8, { min: 0, max: 200, step: 1, label: 'size / levels', ...when('effect', 'posterize', 'pixelate', 'blur', 'sharpen') }),
      color('color1', '#1b2b6b', { label: 'shadows', ...when('effect', 'duotone', 'tint', 'flatten') }),
      color('color2', '#ffd9a0', { label: 'highlights', ...when('effect', 'duotone') }),
      ...ADV([combo('format', 'keep', IMG_OUT, { labels: IMG_OUT_LBL }), Q(92, unless('format', 'png', 'bmp', 'tiff', 'gif', 'tga', 'qoi'))]),
    ],
  },
  'image.convert': {
    cat: 'image', title: 'Convert Image', icon: 'convert', width: 290, keywords: 'webp avif jpeg png compress optimize format',
    desc: 'WebP, AVIF, JPEG, PNG and more, with quality, target size and PNG optimization.',
    inputs: IMG, outputs: IMG,
    widgets: [
      combo('format', 'webp', ['webp', 'jpeg', 'png', 'avif', 'gif', 'bmp', 'tiff', 'ico', 'tga', 'qoi', 'keep']),
      slider('quality', 82, 1, 100, unless('format', 'png', 'gif', 'bmp', 'tiff', 'ico', 'tga', 'qoi')),
      toggle('lossless', false, when('format', 'webp', 'avif')),
      num('target_kb', 0, { min: 0, unit: 'KB', label: 'target size', ...when('format', 'webp', 'jpeg', 'avif') }),
      ...ADV([
        combo('png_optimize', 'balanced', ['off', 'fast', 'balanced', 'max'], { label: 'png optimize', ...when('format', 'png', 'keep') }),
        num('png_colors', 0, { min: 0, max: 256, label: 'png colors (0 = all)', ...when('format', 'png', 'keep') }),
        slider('avif_speed', 6, 1, 10, { label: 'avif speed', ...when('format', 'avif') }),
        color('background', '#ffffff', when('format', 'jpeg', 'keep')),
      ]),
    ],
  },
  'image.watermark': {
    cat: 'image', title: 'Logo Watermark', icon: 'stamp', keywords: 'logo stamp overlay watermark',
    desc: 'Stamps a logo onto every image, in a corner or tiled.',
    inputs: IMG, outputs: IMG,
    widgets: [
      path('watermark', { label: 'logo', filters: [{ name: 'Images', extensions: ['png', 'webp', 'svg', 'jpg', 'jpeg'] }] }),
      combo('position', 'bottom-right', [...ANCHORS, 'tile']),
      slider('scale', 20, 1, 100, { unit: '%' }),
      slider('opacity', 70, 0, 100, { unit: '%' }),
      num('margin', 24, { min: 0, unit: 'px' }),
    ],
  },
  'image.text': {
    cat: 'image', title: 'Text Overlay', icon: 'type', width: 300, keywords: 'caption text label title copyright watermark date',
    desc: 'Writes a caption on every image. Use {name}, {date}, {w}×{h} and \\n for new lines.',
    inputs: IMG, outputs: IMG,
    widgets: [
      text('text', '{name}', { placeholder: '© {date} {name}' }),
      slider('size', 6, 0.5, 30, { step: 0.5, unit: '%', label: 'size (% of height)' }),
      color('color', '#ffffff'),
      combo('position', 'bottom', ANCHORS),
      ...G('Style', [
        combo('font', 'sans-serif', FONTS),
        toggle('bold', true),
        toggle('shadow', true),
        toggle('box', false, { label: 'background box' }),
        color('box_color', '#000000', { label: 'box color', ...when('box', true) }),
        slider('opacity', 100, 0, 100, { unit: '%' }),
        slider('margin', 4, 0, 45, { unit: '%' }),
      ], true),
      ...ADV([combo('format', 'keep', IMG_OUT, { labels: IMG_OUT_LBL }), Q(92)]),
    ],
  },
  'image.frame': {
    cat: 'image', title: 'Frame & Corners', icon: 'frame', width: 290, keywords: 'border padding rounded corners shadow frame canvas',
    desc: 'Padding, border, rounded corners and a soft drop shadow, like a polished screenshot.',
    inputs: IMG, outputs: IMG,
    widgets: [
      seg('unit', 'px', ['px', '%']),
      num('padding', 40, { min: 0 }),
      num('radius', 24, { min: 0, label: 'corner radius' }),
      num('border', 0, { min: 0 }),
      color('border_color', '#ffffff', { label: 'border color' }),
      num('shadow', 0, { min: 0, max: 200 }),
      toggle('transparent', false, { label: 'transparent background' }),
      color('background', '#ffffff', unless('transparent', true)),
      ...ADV([combo('format', 'keep', IMG_OUT, { labels: IMG_OUT_LBL }), Q(92)]),
    ],
  },
  'image.remove_bg': {
    cat: 'image', title: 'Remove Background', icon: 'eraser', keywords: 'transparent background remove cutout chroma key white',
    desc: 'Makes a plain background transparent (product shots, logos, scans, green screen).',
    inputs: IMG, outputs: IMG,
    widgets: [
      seg('mode', 'auto', ['auto', 'color'], { labels: { auto: 'detect color', color: 'pick color' } }),
      color('color', '#ffffff', when('mode', 'color')),
      slider('tolerance', 12, 0, 100, { unit: '%' }),
      slider('feather', 4, 0, 100, { label: 'soft edge' }),
      toggle('connected', true, { label: 'only from the edges' }),
      toggle('trim', false, { label: 'trim to content' }),
      seg('format', 'png', ['png', 'webp']),
    ],
  },
  'image.collage': {
    cat: 'image', title: 'Collage', icon: 'collage', width: 290, keywords: 'collage grid combine join images strip mosaic',
    desc: 'Combines images into one picture: a grid, a horizontal strip or a vertical stack.',
    inputs: IMG, outputs: [io('collage', 'IMAGE')],
    widgets: [
      seg('layout', 'grid', ['grid', 'horizontal', 'vertical']),
      num('columns', 3, { min: 1, max: 50, ...when('layout', 'grid') }),
      num('cell', 600, { min: 16, max: 8192, unit: 'px', label: 'cell size' }),
      num('gap', 16, { min: 0, max: 1000, unit: 'px' }),
      seg('fit', 'cover', ['cover', 'contain'], when('layout', 'grid')),
      num('radius', 0, { min: 0, unit: 'px', label: 'corner radius' }),
      color('background', '#ffffff', unless('transparent', true)),
      text('name', 'collage'),
      ...ADV([toggle('transparent', false), combo('format', 'jpg', ['jpg', 'png', 'webp', 'avif']), Q(90)]),
    ],
  },
  'image.sheet': {
    cat: 'image', title: 'Contact Sheet', icon: 'grid', keywords: 'grid collage overview thumbnails',
    desc: 'Puts all images onto one overview grid.',
    inputs: IMG, outputs: [io('sheet', 'IMAGE')],
    widgets: [
      num('columns', 6, { min: 1, max: 64 }),
      num('cell', 256, { min: 16, max: 4096, unit: 'px' }),
      num('padding', 8, { min: 0, unit: 'px' }),
      color('background', '#1e1e22'),
      text('name', 'contact-sheet'),
    ],
  },
  'image.tiles': {
    cat: 'image', title: 'Cut into Tiles', icon: 'grid', keywords: 'slice split tiles grid instagram carousel puzzle',
    desc: 'Slices images into a grid (e.g. 3×3 for Instagram) or tiles of a fixed size.',
    inputs: IMG, outputs: IMG,
    widgets: [
      seg('mode', 'grid', ['grid', 'size'], { labels: { grid: 'rows × columns', size: 'tile size' } }),
      num('columns', 3, { min: 1, max: 100, ...when('mode', 'grid') }),
      num('rows', 3, { min: 1, max: 100, ...when('mode', 'grid') }),
      num('tile_width', 512, { min: 1, unit: 'px', label: 'tile width', ...when('mode', 'size') }),
      num('tile_height', 512, { min: 1, unit: 'px', label: 'tile height', ...when('mode', 'size') }),
      ...ADV([combo('format', 'keep', IMG_OUT, { labels: IMG_OUT_LBL }), Q(92)]),
    ],
  },
  'image.icons': {
    cat: 'image', title: 'App Icons', icon: 'sparkle', keywords: 'ico favicon icon sizes',
    desc: 'Makes an .ico file and PNG icons in every size from one picture.',
    inputs: IMG, outputs: [io('icons', 'IMAGE')],
    widgets: [
      text('sizes', '16,24,32,48,64,128,256'),
      toggle('ico', true, { label: '.ico file' }),
      toggle('png', true, { label: 'png files' }),
    ],
  },
  'image.palette': {
    cat: 'image', title: 'Color Palette', icon: 'palette', keywords: 'colors palette extract dominant swatches css gimp',
    desc: 'Extracts the dominant colors as a swatch image, CSS variables, JSON or a GIMP palette.',
    inputs: IMG, outputs: [io('palettes', 'FILES')],
    widgets: [
      slider('colors', 8, 2, 32),
      combo('format', 'png', ['png', 'svg', 'json', 'css', 'gpl', 'txt'], { labels: { gpl: 'gpl (GIMP / Inkscape)', css: 'css variables' } }),
    ],
  },
  'image.qr': {
    cat: 'image', title: 'QR Code', icon: 'qr', width: 290, keywords: 'qr code barcode link generate',
    desc: 'Generates QR codes from the text below or from incoming text files (one per line if you like).',
    inputs: FIL, outputs: [io('codes', 'IMAGE')],
    widgets: [
      area('text', 'https://github.com/kabiriyer3-cmyk/morph', { rows: 3 }),
      toggle('per_line', false, { label: 'one code per line' }),
      num('size', 512, { min: 64, max: 8192, unit: 'px' }),
      seg('format', 'png', ['png', 'svg', 'jpg', 'webp']),
      ...ADV([
        num('margin', 4, { min: 0, max: 40, label: 'quiet zone' }),
        seg('ec', 'M', ['L', 'M', 'Q', 'H'], { label: 'error correction' }),
        color('color', '#000000'),
        color('background', '#ffffff', unless('transparent', true)),
        toggle('transparent', false),
      ]),
    ],
  },
  'image.metadata': {
    cat: 'image', title: 'Photo Metadata', icon: 'tag', keywords: 'exif gps camera metadata strip privacy remove',
    desc: 'Reads EXIF (camera, lens, GPS…) into a report, or strips all metadata for privacy.',
    inputs: IMG, outputs: [io('images', 'IMAGE'), REPORT],
    widgets: [
      seg('mode', 'report', ['report', 'strip'], { labels: { report: 'read', strip: 'remove' } }),
      seg('format', 'csv', ['csv', 'json', 'txt'], when('mode', 'report')),
      Q(95, when('mode', 'strip')),
    ],
  },
  'image.similar': {
    cat: 'image', title: 'Similar Images', icon: 'layers', keywords: 'duplicate similar photos near identical cleanup',
    desc: 'Finds visually similar photos (resized, recompressed, near duplicates) and keeps the best one.',
    inputs: IMG, outputs: [io('unique', 'IMAGE'), io('similar', 'IMAGE'), REPORT],
    widgets: [
      slider('threshold', 6, 0, 32, { label: 'tolerance' }),
      seg('keep', 'largest', ['largest', 'smallest', 'first']),
    ],
  },
  'image.animate': {
    cat: 'image', title: 'Images to GIF', icon: 'film', keywords: 'gif animation frames sequence boomerang',
    desc: 'Turns a sequence of images into an animated GIF.',
    inputs: IMG, outputs: [io('gif', 'IMAGE')],
    widgets: [
      num('delay', 200, { min: 10, max: 60000, step: 10, unit: 'ms', label: 'frame time' }),
      num('width', 480, { min: 0, unit: 'px', label: 'width (0 = keep)' }),
      toggle('loop', true),
      toggle('boomerang', false),
      text('name', 'animation'),
    ],
  },
  'image.frames': {
    cat: 'image', title: 'GIF to Frames', icon: 'pages', keywords: 'split gif frames apng webp animation extract',
    desc: 'Extracts the frames of animated GIF, WebP or APNG images.',
    inputs: IMG, outputs: [io('frames', 'IMAGE')],
    widgets: [
      num('every', 1, { min: 1, label: 'every Nth frame' }),
      num('max_frames', 0, { min: 0, label: 'max frames (0 = all)' }),
      seg('format', 'png', ['png', 'jpg', 'webp']),
    ],
  },
  'image.base64': {
    cat: 'image', title: 'Image to Base64', icon: 'code', keywords: 'base64 data uri css html embed inline',
    desc: 'Encodes images as data URIs for CSS, HTML, Markdown or JSON.',
    inputs: IMG, outputs: [io('code', 'TEXT')],
    widgets: [
      combo('format', 'datauri', ['datauri', 'css', 'html', 'md', 'json', 'raw'], { labels: { datauri: 'data URI', raw: 'raw base64', md: 'Markdown' } }),
      toggle('combine', true, { label: 'one file for all' }),
    ],
  },
  'image.pdf': {
    cat: 'image', title: 'Images to PDF', icon: 'pdf', keywords: 'pdf scan document pages',
    desc: 'Combines images into a PDF document, one image per page.',
    inputs: IMG, outputs: [io('pdf', 'PDF')],
    widgets: [
      text('name', 'images'),
      combo('page', 'A4', ['A4', 'Letter', 'A3', 'A5', 'image'], { labels: { image: 'fit image' } }),
      seg('orientation', 'auto', ['auto', 'portrait', 'landscape'], unless('page', 'image')),
      num('margin_mm', 10, { min: 0, max: 100, unit: 'mm', label: 'margin' }),
      ...ADV([slider('quality', 88, 1, 100), toggle('per_image', false, { label: 'one pdf per image' })]),
    ],
  },

  // ---------------------------------------------------------------- video
  'video.convert': {
    cat: 'video', title: 'Convert Video', icon: 'convert', width: 310, needs: 'ffmpeg', keywords: 'mp4 mkv webm mov avi h264 h265 hevc av1 vp9 prores compress encode',
    desc: 'Any container and codec: MP4, MKV, WebM, MOV, AVI… H.264, HEVC, AV1, VP9, ProRes, with full quality control.',
    inputs: VID, outputs: VID,
    widgets: [
      combo('container', 'mp4', VIDEO_CONTAINERS, { labels: { keep: 'keep container' } }),
      combo('video_codec', 'auto', VCODECS, { label: 'video codec', labels: VCODEC_LBL }),
      ...G('Quality', [
        seg('rate_mode', 'quality', ['quality', 'bitrate', 'size'], { label: 'mode', labels: { quality: 'CRF', bitrate: 'bitrate', size: 'file size' }, ...unless('video_codec', 'copy') }),
        slider('quality', 23, 0, 51, { label: 'quality (lower = better)', ...all(when('rate_mode', 'quality'), unless('video_codec', 'copy')) }),
        num('video_bitrate', 4000, { min: 50, max: 200000, step: 100, unit: 'kbps', label: 'video bitrate', ...all(when('rate_mode', 'bitrate'), unless('video_codec', 'copy')) }),
        num('target_mb', 25, { min: 0.5, step: 1, unit: 'MB', label: 'target size', ...all(when('rate_mode', 'size'), unless('video_codec', 'copy')) }),
        combo('preset', 'medium', PRESETS, unless('video_codec', 'copy', 'prores', 'dnxhd', 'ffv1', 'mjpeg')),
        combo('hardware', 'none', ['none', 'nvidia', 'intel', 'amd'], { labels: HW, ...when('video_codec', 'auto', 'h264', 'h265', 'av1') }),
      ]),
      ...G('Picture', [
        num('max_size', 0, { min: 0, max: 7680, unit: 'px', label: 'max side (0 = keep)' }),
        num('width', 0, { min: 0, max: 7680, unit: 'px', label: 'width (0 = auto)' }),
        num('height', 0, { min: 0, max: 4320, unit: 'px', label: 'height (0 = auto)' }),
        combo('fit', 'fit', ['fit', 'crop', 'pad', 'stretch']),
        num('fps', 0, { min: 0, max: 240, label: 'fps (0 = keep)' }),
        toggle('two_pass', false, { label: 'two-pass encode', ...when('rate_mode', 'bitrate', 'size') }),
        combo('tune', 'none', ['none', 'film', 'animation', 'grain', 'stillimage', 'fastdecode', 'zerolatency'], when('video_codec', 'auto', 'h264', 'h265')),
      ].map((w) => ({ ...w, ...(w.show || w.hide ? {} : unless('video_codec', 'copy')) })), true),
      ...G('Audio', [
        combo('audio_codec', 'auto', ACODECS, { label: 'audio codec', labels: ACODEC_LBL }),
        num('audio_bitrate', 160, { min: 8, max: 1024, unit: 'kbps', label: 'audio bitrate', ...unless('audio_codec', 'copy', 'none', 'flac', 'alac', 'pcm') }),
        combo('sample_rate', '0', RATES, { labels: RATE_LBL, ...unless('audio_codec', 'copy', 'none') }),
        seg('channels', 'keep', ['keep', 'mono', 'stereo', '5.1'], unless('audio_codec', 'copy', 'none')),
      ], true),
      ...G('Extras', [toggle('keep_subtitles', true, { label: 'keep subtitles' }), toggle('strip_metadata', false, { label: 'strip metadata' })], true),
    ],
  },
  'video.compress': {
    cat: 'video', title: 'Compress for Sharing', icon: 'compress', width: 290, needs: 'ffmpeg', keywords: 'compress shrink discord whatsapp telegram email size limit 10mb 25mb',
    desc: 'Fits videos under a size limit: Discord, WhatsApp, Telegram, e-mail or your own number.',
    inputs: VID, outputs: VID,
    widgets: [
      combo('target', 'discord', ['discord', 'discord-50', 'whatsapp', 'telegram', 'email', 'gmail', 'custom'], { labels: { discord: 'Discord (10 MB)', 'discord-50': 'Discord Nitro (50 MB)', whatsapp: 'WhatsApp (16 MB)', telegram: 'Telegram (2 GB)', email: 'E-mail (25 MB)', gmail: 'Gmail (25 MB)', custom: 'custom size' } }),
      num('size_mb', 25, { min: 0.5, step: 1, unit: 'MB', label: 'size limit', ...when('target', 'custom') }),
      combo('max_height', '1080', ['0', '2160', '1440', '1080', '720', '480', '360'], { label: 'max height', labels: { 0: 'keep', 2160: '2160p (4K)', 1440: '1440p', 1080: '1080p', 720: '720p', 480: '480p', 360: '360p' } }),
      seg('codec', 'h264', ['h264', 'h265'], { labels: { h264: 'H.264', h265: 'HEVC' } }),
      ...ADV([
        num('audio_bitrate', 96, { min: 24, max: 320, unit: 'kbps', label: 'audio bitrate' }),
        num('max_fps', 30, { min: 0, max: 240, label: 'max fps (0 = keep)' }),
        combo('preset', 'medium', PRESETS),
        toggle('skip_small', true, { label: 'skip files already small' }),
      ]),
    ],
  },
  'video.trim': {
    cat: 'video', title: 'Trim Video', icon: 'scissors', needs: 'ffmpeg', keywords: 'cut clip range',
    desc: 'Cuts a time range out of a video.',
    inputs: VID, outputs: VID,
    widgets: [
      text('start', '0:00', { placeholder: '0:00' }),
      text('end', '', { placeholder: 'empty = to the end' }),
      toggle('precise', false, { label: 'frame accurate (re-encode)' }),
    ],
  },
  'video.split': {
    cat: 'video', title: 'Split Video', icon: 'split', needs: 'ffmpeg', keywords: 'split parts segments chapters cut pieces',
    desc: 'Cuts videos into N equal parts, chunks of fixed length or at the times you list.',
    inputs: VID, outputs: VID,
    widgets: [
      seg('mode', 'parts', ['parts', 'duration', 'times'], { labels: { parts: 'N parts', duration: 'every…', times: 'at times' } }),
      num('parts', 2, { min: 2, max: 1000, ...when('mode', 'parts') }),
      text('duration', '60', { label: 'segment length', placeholder: 'seconds or 1:30', ...when('mode', 'duration') }),
      text('times', '', { placeholder: '0:30, 1:45, 3:00', ...when('mode', 'times') }),
      toggle('reencode', false, { label: 'frame accurate (re-encode)' }),
    ],
  },
  'video.transform': {
    cat: 'video', title: 'Transform Video', icon: 'rotate', width: 290, needs: 'ffmpeg', keywords: 'rotate flip speed mute crop',
    desc: 'Rotate, flip, crop, change speed or remove the sound.',
    inputs: VID, outputs: VID,
    widgets: [
      seg('rotate', '0', ['0', '90', '180', '270'], { labels: DEG }),
      toggle('flip_h', false, { label: 'flip horizontal' }),
      toggle('flip_v', false, { label: 'flip vertical' }),
      slider('speed', 1, 0.25, 4, { step: 0.05, unit: '×' }),
      toggle('mute', false, { label: 'remove sound' }),
      ...ADV([text('crop', '', { placeholder: 'w:h:x:y' }), slider('quality', 23, 0, 51, { label: 'quality (CRF)' })]),
    ],
  },
  'video.reframe': {
    cat: 'video', title: 'Reframe (9:16, 1:1…)', icon: 'aspect', needs: 'ffmpeg', keywords: 'vertical shorts reels tiktok square aspect blur background',
    desc: 'Changes the aspect ratio for Shorts, Reels or TikTok with a blurred background, bars or a crop.',
    inputs: VID, outputs: VID,
    widgets: [
      seg('aspect', '9:16', ['9:16', '1:1', '4:5', '16:9', '4:3', '21:9']),
      seg('mode', 'blur', ['blur', 'color', 'crop'], { labels: { blur: 'blurred', color: 'bars', crop: 'crop' } }),
      color('color', '#000000', when('mode', 'color')),
      num('width', 0, { min: 0, max: 4320, unit: 'px', label: 'width (0 = auto)' }),
      ...ADV([slider('quality', 20, 0, 51, { label: 'quality (CRF)' })]),
    ],
  },
  'video.filters': {
    cat: 'video', title: 'Color & Cleanup', icon: 'sun', width: 300, needs: 'ffmpeg', keywords: 'color grading brightness contrast saturation denoise stabilize deinterlace sharpen look lut',
    desc: 'Color correction, looks, denoise, stabilization, deinterlacing, sharpening, grain and vignette.',
    inputs: VID, outputs: VID,
    widgets: [
      ...G('Color', [
        slider('brightness', 0, -1, 1, { step: 0.02 }),
        slider('contrast', 1, 0, 3, { step: 0.05 }),
        slider('saturation', 1, 0, 3, { step: 0.05 }),
        slider('gamma', 1, 0.1, 4, { step: 0.05 }),
        slider('hue', 0, -180, 180, { unit: '°' }),
        slider('temperature', 6500, 2000, 12000, { step: 100, unit: 'K' }),
        combo('look', 'none', ['none', 'grayscale', 'sepia', 'invert', 'vintage', 'cross_process', 'strong_contrast', 'lighter', 'darker', 'increase_contrast', 'negative'], { labels: { cross_process: 'cross process', strong_contrast: 'strong contrast', increase_contrast: 'more contrast' } }),
      ]),
      ...G('Cleanup', [
        combo('denoise', 'off', ['off', 'light', 'medium', 'strong']),
        toggle('stabilize', false, { label: 'stabilize (deshake)' }),
        toggle('deinterlace', false),
        slider('sharpen', 0, 0, 3, { step: 0.1 }),
        slider('blur', 0, 0, 50, { step: 0.5 }),
        toggle('vignette', false),
        toggle('grain', false, { label: 'film grain' }),
      ], true),
      ...ADV([slider('quality', 20, 0, 51, { label: 'quality (CRF)' })]),
    ],
  },
  'video.fx': {
    cat: 'video', title: 'Fades, Reverse, Loop', icon: 'sparkle', needs: 'ffmpeg', keywords: 'fade in out reverse boomerang loop repeat',
    desc: 'Fade in and out, play backwards, boomerang or repeat a clip.',
    inputs: VID, outputs: VID,
    widgets: [
      num('fade_in', 0, { min: 0, step: 0.5, unit: 's', label: 'fade in' }),
      num('fade_out', 0, { min: 0, step: 0.5, unit: 's', label: 'fade out' }),
      seg('fade_color', 'black', ['black', 'white'], { label: 'fade to' }),
      toggle('reverse', false, { label: 'play backwards' }),
      toggle('boomerang', false),
      num('loop', 1, { min: 1, max: 100, label: 'repeat ×' }),
      ...ADV([slider('quality', 20, 0, 51, { label: 'quality (CRF)' })]),
    ],
  },
  'video.framerate': {
    cat: 'video', title: 'Change Frame Rate', icon: 'gauge', needs: 'ffmpeg', keywords: 'fps framerate 60fps slow motion interpolate smooth',
    desc: 'Converts to a new frame rate: drop frames, blend them or interpolate smooth motion.',
    inputs: VID, outputs: VID,
    widgets: [
      combo('fps', 60, [24, 25, 30, 48, 50, 60, 90, 120, 144, 240].map(String).map(Number), { label: 'target fps' }),
      seg('mode', 'blend', ['drop', 'blend', 'interpolate'], { labels: { drop: 'drop', blend: 'blend', interpolate: 'smooth' } }),
      ...ADV([slider('quality', 20, 0, 51, { label: 'quality (CRF)' })]),
    ],
  },
  'video.watermark': {
    cat: 'video', title: 'Video Watermark', icon: 'stamp', width: 300, needs: 'ffmpeg', keywords: 'logo text watermark overlay copyright channel',
    desc: 'Puts a text or logo watermark on videos.',
    inputs: VID, outputs: VID,
    widgets: [
      seg('mode', 'text', ['text', 'image'], { labels: { image: 'logo' } }),
      text('text', '© Morph', when('mode', 'text')),
      path('image', { label: 'logo', filters: [{ name: 'Images', extensions: ['png', 'webp', 'svg', 'jpg', 'jpeg'] }], ...when('mode', 'image') }),
      combo('position', 'bottom-right', ANCHORS),
      slider('size', 5, 0.5, 40, { step: 0.5, unit: '%' }),
      slider('opacity', 80, 0, 100, { unit: '%' }),
      ...G('Style', [
        color('color', '#ffffff', when('mode', 'text')),
        combo('font', 'sans-serif', FONTS, when('mode', 'text')),
        toggle('bold', true, when('mode', 'text')),
        toggle('shadow', true, when('mode', 'text')),
        slider('margin', 3, 0, 50, { unit: '%' }),
        slider('quality', 20, 0, 51, { label: 'quality (CRF)' }),
      ], true),
    ],
  },
  'video.subtitles': {
    cat: 'video', title: 'Subtitles', icon: 'cc', width: 290, needs: 'ffmpeg', keywords: 'subtitles srt ass vtt burn hardsub softsub extract captions',
    desc: 'Burns subtitles into the picture, adds them as a track, or extracts them to SRT / ASS / VTT.',
    inputs: VID, outputs: VID,
    widgets: [
      seg('mode', 'burn', ['burn', 'add', 'extract'], { labels: { burn: 'burn in', add: 'add track', extract: 'extract' } }),
      path('file', { label: 'subtitle file', placeholder: 'same name .srt next to video', filters: [{ name: 'Subtitles', extensions: ['srt', 'ass', 'ssa', 'vtt', 'sub'] }], ...unless('mode', 'extract') }),
      num('font_size', 0, { min: 0, max: 200, label: 'font size (0 = auto)', ...when('mode', 'burn') }),
      color('color', '#ffffff', when('mode', 'burn')),
      text('language', '', { placeholder: 'eng, rus…', ...when('mode', 'add') }),
      seg('format', 'srt', ['srt', 'ass', 'vtt'], when('mode', 'extract')),
      num('stream', 1, { min: 1, max: 99, label: 'subtitle track #', ...when('mode', 'extract') }),
      ...ADV([slider('quality', 20, 0, 51, { label: 'quality (CRF)', ...when('mode', 'burn') })]),
    ],
  },
  'video.thumbs': {
    cat: 'video', title: 'Thumbnails', icon: 'grid', needs: 'ffmpeg', keywords: 'thumbnails preview contact sheet screenshots storyboard',
    desc: 'A storyboard sheet of evenly spaced frames, or a set of separate preview images.',
    inputs: VID, outputs: [io('images', 'IMAGE')],
    widgets: [
      seg('mode', 'sheet', ['sheet', 'images'], { labels: { sheet: 'one sheet', images: 'separate' } }),
      num('columns', 4, { min: 1, max: 20, ...when('mode', 'sheet') }),
      num('rows', 4, { min: 1, max: 30, ...when('mode', 'sheet') }),
      num('count', 8, { min: 1, max: 500, ...when('mode', 'images') }),
      num('width', 320, { min: 32, max: 4096, unit: 'px', label: 'frame width' }),
      seg('format', 'jpg', ['jpg', 'png', 'webp']),
      ...ADV([color('background', '#111111', when('mode', 'sheet'))]),
    ],
  },
  'video.frames': {
    cat: 'video', title: 'Extract Frames', icon: 'pages', needs: 'ffmpeg', keywords: 'frames screenshots thumbnails stills',
    desc: 'Saves frames as images: every N seconds, N per second or a single frame.',
    inputs: VID, outputs: [io('frames', 'IMAGE')],
    widgets: [
      combo('mode', 'fps', ['fps', 'interval', 'count', 'single'], { labels: { fps: 'frames per second', interval: 'every N seconds', count: 'N frames total', single: 'single frame' } }),
      num('value', 1, { min: 0.01, step: 0.5, ...unless('mode', 'single') }),
      text('time', '0', { placeholder: 'seconds or 0:05', ...when('mode', 'single') }),
      seg('format', 'png', ['png', 'jpg', 'webp']),
      num('width', 0, { min: 0, unit: 'px', label: 'width (0 = keep)' }),
    ],
  },
  'video.gif': {
    cat: 'video', title: 'Video to GIF', icon: 'film', needs: 'ffmpeg', keywords: 'gif animation webp loop',
    desc: 'High quality animated GIF or WebP with a tuned palette.',
    inputs: VID, outputs: IMG,
    widgets: [
      seg('format', 'gif', ['gif', 'webp']),
      num('fps', 12, { min: 1, max: 60 }),
      num('width', 480, { min: 16, max: 3840, unit: 'px' }),
      text('start', '0', { placeholder: 'seconds or 0:05' }),
      text('duration', '', { placeholder: 'empty = whole clip' }),
      ...ADV([
        num('colors', 256, { min: 2, max: 256, ...when('format', 'gif') }),
        combo('dither', 'sierra2_4a', ['sierra2_4a', 'floyd_steinberg', 'bayer', 'none'], when('format', 'gif')),
        toggle('loop', true),
      ]),
    ],
  },
  'video.audio': {
    cat: 'video', title: 'Extract Audio', icon: 'audio', needs: 'ffmpeg', keywords: 'soundtrack mp3 rip',
    desc: 'Pulls the soundtrack out of videos.',
    inputs: VID, outputs: AUD,
    widgets: [
      combo('format', 'mp3', ['mp3', 'm4a', 'opus', 'flac', 'wav', 'copy']),
      num('bitrate', 192, { min: 32, max: 512, unit: 'kbps', ...unless('format', 'flac', 'wav', 'copy') }),
    ],
  },
  'video.concat': {
    cat: 'video', title: 'Join Videos', icon: 'merge', needs: 'ffmpeg', keywords: 'concat merge join combine',
    desc: 'Concatenates clips into one video in input order.',
    inputs: VID, outputs: [io('video', 'VIDEO')],
    widgets: [text('name', 'joined'), toggle('reencode', false, { label: 're-encode (mixed formats)' })],
  },
  'video.mux': {
    cat: 'video', title: 'Add Music / Voice', icon: 'mux', needs: 'ffmpeg', keywords: 'music soundtrack replace audio mix voiceover dub',
    desc: 'Puts an audio file onto videos: replace the sound or mix it with the original.',
    inputs: [io('videos', 'VIDEO'), io('audio', 'AUDIO')], outputs: VID,
    widgets: [
      seg('mode', 'replace', ['replace', 'mix']),
      slider('volume', 100, 0, 300, { unit: '%', label: 'new audio volume' }),
      slider('original_volume', 100, 0, 300, { unit: '%', label: 'original volume', ...when('mode', 'mix') }),
      toggle('shortest', true, { label: 'stop at the shorter one' }),
      ...ADV([num('offset', 0, { min: 0, step: 0.5, unit: 's', label: 'audio starts at' }), toggle('loop_audio', false, { label: 'loop audio' })]),
    ],
  },
  'video.slideshow': {
    cat: 'video', title: 'Slideshow Video', icon: 'slideshow', width: 290, needs: 'ffmpeg', keywords: 'slideshow photos to video images music',
    desc: 'Makes a video from images, optionally with a soundtrack.',
    inputs: IMG, outputs: [io('video', 'VIDEO')],
    widgets: [
      num('seconds', 2, { min: 0.04, step: 0.5, unit: 's', label: 'seconds per image' }),
      combo('format', 'mp4', ['mp4', 'webm', 'mov', 'mkv', 'gif']),
      seg('fit', 'pad', ['pad', 'crop'], { labels: { pad: 'fit', crop: 'fill' } }),
      path('audio', { label: 'music', filters: [{ name: 'Audio', extensions: ['mp3', 'm4a', 'aac', 'wav', 'flac', 'ogg', 'opus'] }] }),
      text('name', 'slideshow'),
      ...ADV([
        num('width', 1920, { min: 16, max: 7680, unit: 'px' }),
        num('height', 1080, { min: 16, max: 7680, unit: 'px' }),
        num('fps', 30, { min: 1, max: 120 }),
        color('background', '#000000'),
        slider('quality', 20, 0, 51, { label: 'quality (CRF)' }),
      ]),
    ],
  },
  'media.info': {
    cat: 'video', title: 'Media Report', icon: 'info', needs: 'ffmpeg', keywords: 'ffprobe codec bitrate duration resolution report',
    desc: 'A table with codecs, resolution, frame rate, bitrate, duration and audio of every clip.',
    inputs: FIL, outputs: [io('files', 'FILES'), REPORT],
    widgets: [seg('format', 'csv', ['csv', 'json', 'md', 'txt'])],
  },

  // ---------------------------------------------------------------- audio
  'audio.convert': {
    cat: 'audio', title: 'Convert Audio', icon: 'convert', width: 290, needs: 'ffmpeg', keywords: 'mp3 aac opus ogg flac wav alac aiff wma m4r ringtone music',
    desc: 'MP3, AAC, Opus, OGG, FLAC, WAV, ALAC, AIFF, WMA, AC-3, ringtones and audiobooks.',
    inputs: AUD, outputs: AUD,
    widgets: [
      combo('format', 'mp3', AUDIO_FORMATS, { labels: AUDIO_FMT_LBL }),
      num('bitrate', 192, { min: 8, max: 1024, unit: 'kbps', ...unless('format', 'flac', 'wav', 'alac', 'aiff', 'copy') }),
      toggle('vbr', false, { label: 'variable bitrate', ...when('format', 'mp3', 'ogg', 'opus') }),
      ...G('Sound', [
        combo('sample_rate', '0', RATES, { labels: RATE_LBL, ...unless('format', 'copy') }),
        seg('channels', 'keep', ['keep', 'mono', 'stereo'], unless('format', 'copy')),
        seg('bit_depth', '16', ['16', '24', '32', '32f'], { label: 'bit depth', labels: { '32f': '32 float' }, ...when('format', 'wav', 'flac', 'aiff', 'alac') }),
      ], true),
      ...G('Tags', [toggle('keep_cover', true, { label: 'keep cover art' }), toggle('strip_metadata', false, { label: 'strip tags' })], true),
    ],
  },
  'audio.process': {
    cat: 'audio', title: 'Volume & Trim', icon: 'volume', width: 290, needs: 'ffmpeg', keywords: 'volume normalize loudness fade trim speed',
    desc: 'Volume, loudness normalization, trimming, fades and speed.',
    inputs: AUD, outputs: AUD,
    widgets: [
      slider('volume_db', 0, -30, 30, { step: 0.5, unit: 'dB', label: 'volume' }),
      toggle('normalize', false, { label: 'normalize loudness' }),
      num('lufs', -16, { min: -40, max: -5, unit: 'LUFS', label: 'target', ...when('normalize', true) }),
      num('fade_in', 0, { min: 0, step: 0.5, unit: 's', label: 'fade in' }),
      num('fade_out', 0, { min: 0, step: 0.5, unit: 's', label: 'fade out' }),
      ...ADV([
        num('trim_start', 0, { min: 0, step: 0.5, unit: 's', label: 'cut from start' }),
        num('trim_end', 0, { min: 0, step: 0.5, unit: 's', label: 'cut from end' }),
        slider('speed', 1, 0.5, 2, { step: 0.05, unit: '×' }),
      ]),
    ],
  },
  'audio.fx': {
    cat: 'audio', title: 'Audio Effects', icon: 'eq', width: 300, needs: 'ffmpeg', keywords: 'equalizer bass treble denoise compressor limiter pitch echo reverb karaoke vocal remove',
    desc: 'EQ, noise reduction, compressor, pitch, echo, reverb, stereo widening and vocal removal.',
    inputs: AUD, outputs: AUD,
    widgets: [
      ...G('Tone', [
        slider('bass', 0, -20, 20, { step: 0.5, unit: 'dB' }),
        slider('treble', 0, -20, 20, { step: 0.5, unit: 'dB' }),
        num('highpass', 0, { min: 0, max: 20000, step: 10, unit: 'Hz', label: 'low cut' }),
        num('lowpass', 0, { min: 0, max: 22000, step: 100, unit: 'Hz', label: 'high cut' }),
        slider('volume_db', 0, -30, 30, { step: 0.5, unit: 'dB', label: 'volume' }),
      ]),
      ...G('Clean', [
        combo('denoise', 'off', ['off', 'light', 'medium', 'strong'], { label: 'noise reduction' }),
        toggle('compressor', false),
        toggle('limiter', false),
      ]),
      ...G('Creative', [
        slider('pitch', 0, -12, 12, { step: 0.5, label: 'pitch (semitones)' }),
        toggle('echo', false),
        toggle('reverb', false),
        slider('widen', 0, 0, 5, { step: 0.1, label: 'stereo widen' }),
        toggle('karaoke', false, { label: 'remove vocals' }),
        toggle('mono', false, { label: 'to mono' }),
        toggle('swap_channels', false, { label: 'swap L / R' }),
      ], true),
      ...ADV([num('bitrate', 192, { min: 8, max: 1024, unit: 'kbps' })]),
    ],
  },
  'audio.join': {
    cat: 'audio', title: 'Join Audio', icon: 'merge', needs: 'ffmpeg', keywords: 'join merge concat crossfade mix tape playlist',
    desc: 'Joins tracks into one file, with optional crossfades or pauses between them.',
    inputs: AUD, outputs: [io('audio', 'AUDIO')],
    widgets: [
      combo('format', 'mp3', ['mp3', 'm4a', 'aac', 'opus', 'ogg', 'flac', 'wav']),
      num('crossfade', 0, { min: 0, step: 0.5, unit: 's' }),
      num('gap', 0, { min: 0, step: 0.5, unit: 's', label: 'pause between', ...when('crossfade', 0) }),
      text('name', 'joined'),
      ...ADV([num('bitrate', 192, { min: 8, max: 1024, unit: 'kbps' })]),
    ],
  },
  'audio.split': {
    cat: 'audio', title: 'Split Audio', icon: 'scissors', needs: 'ffmpeg', keywords: 'split tracks silence chapters segments cut',
    desc: 'Splits long recordings at silences, into equal parts or chunks of a fixed length.',
    inputs: AUD, outputs: AUD,
    widgets: [
      seg('mode', 'silence', ['silence', 'duration', 'parts'], { labels: { silence: 'at silences', duration: 'every…', parts: 'N parts' } }),
      text('duration', '5:00', { label: 'segment length', ...when('mode', 'duration') }),
      num('parts', 2, { min: 2, max: 1000, ...when('mode', 'parts') }),
      num('threshold', -35, { min: -90, max: 0, unit: 'dB', label: 'silence level', ...when('mode', 'silence') }),
      num('min_silence', 0.7, { min: 0.05, step: 0.1, unit: 's', label: 'min silence', ...when('mode', 'silence') }),
      num('min_length', 2, { min: 0, step: 0.5, unit: 's', label: 'min piece', ...when('mode', 'silence') }),
    ],
  },
  'audio.silence': {
    cat: 'audio', title: 'Remove Silence', icon: 'silence', needs: 'ffmpeg', keywords: 'silence trim pauses podcast dead air',
    desc: 'Trims silence at the start and end, or removes every pause (great for voice and podcasts).',
    inputs: AUD, outputs: AUD,
    widgets: [
      seg('mode', 'edges', ['edges', 'all'], { labels: { edges: 'start & end', all: 'all pauses' } }),
      num('threshold', -45, { min: -90, max: 0, unit: 'dB', label: 'silence level' }),
      num('min_silence', 0.5, { min: 0.05, step: 0.1, unit: 's', label: 'min pause' }),
      num('keep', 0.1, { min: 0, step: 0.05, unit: 's', label: 'keep around' }),
    ],
  },
  'audio.visual': {
    cat: 'audio', title: 'Waveform / Spectrogram', icon: 'wave', width: 290, needs: 'ffmpeg', keywords: 'waveform spectrogram visualize image picture',
    desc: 'Draws a waveform or spectrogram image of every track.',
    inputs: AUD, outputs: [io('images', 'IMAGE')],
    widgets: [
      seg('mode', 'waveform', ['waveform', 'spectrogram']),
      num('width', 1600, { min: 64, max: 8192, unit: 'px' }),
      num('height', 400, { min: 32, max: 4096, unit: 'px' }),
      color('color', '#4a92ff', when('mode', 'waveform')),
      combo('palette', 'magma', ['magma', 'viridis', 'plasma', 'cividis', 'fire', 'rainbow', 'cool', 'green', 'intensity', 'channel'], when('mode', 'spectrogram')),
      ...ADV([
        color('background', '#0b0e14', unless('transparent', true)),
        toggle('transparent', false, when('mode', 'waveform')),
        seg('scale', 'sqrt', ['lin', 'log', 'sqrt', 'cbrt']),
        toggle('split_channels', false, { label: 'channels separately' }),
        toggle('legend', true, when('mode', 'spectrogram')),
        seg('format', 'png', ['png', 'jpg']),
      ]),
    ],
  },
  'audio.tags': {
    cat: 'audio', title: 'Edit Tags', icon: 'tag', width: 290, needs: 'ffmpeg', keywords: 'id3 tags metadata title artist album cover art',
    desc: 'Sets title, artist, album, cover art and more. Empty fields stay unchanged. {name} = file name.',
    inputs: AUD, outputs: AUD,
    widgets: [
      text('title', '', { placeholder: '{name}' }),
      text('artist', ''),
      text('album', ''),
      path('cover', { label: 'cover art', filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png'] }] }),
      ...ADV([
        text('album_artist', '', { label: 'album artist' }),
        text('date', '', { label: 'year' }),
        text('genre', ''),
        text('track', '', { placeholder: '{n}' }),
        text('composer', ''),
        text('comment', ''),
        toggle('strip', false, { label: 'remove other tags' }),
      ]),
    ],
  },

  // ---------------------------------------------------------------- documents
  'doc.office': {
    cat: 'doc', title: 'Office Convert', icon: 'doc', needs: 'soffice', keywords: 'docx xlsx pptx odt pdf libreoffice word excel powerpoint doc rtf',
    desc: 'Word, Excel, PowerPoint and OpenDocument to PDF or any other office format (LibreOffice).',
    inputs: DOC, outputs: DOC,
    widgets: [
      combo('format', 'pdf', ['pdf', 'docx', 'doc', 'odt', 'rtf', 'txt', 'html', 'epub', 'xlsx', 'xls', 'ods', 'csv', 'pptx', 'ppt', 'odp', 'png', 'jpg', 'svg'], {
        groups: { Document: ['pdf', 'docx', 'doc', 'odt', 'rtf', 'txt', 'html', 'epub'], Spreadsheet: ['xlsx', 'xls', 'ods', 'csv'], Presentation: ['pptx', 'ppt', 'odp'], Image: ['png', 'jpg', 'svg'] },
      }),
      ...ADV([text('filter', '', { label: 'custom filter', placeholder: 'pdf:writer_pdf_Export' })]),
    ],
  },
  'doc.pandoc': {
    cat: 'doc', title: 'Pandoc Convert', icon: 'code', width: 290, needs: 'pandoc', keywords: 'markdown md html epub latex rst docx typst asciidoc org wiki',
    desc: 'Markdown, HTML, DOCX, EPUB, LaTeX, Typst, AsciiDoc, wiki formats and more (Pandoc).',
    inputs: DOC, outputs: DOC,
    widgets: [
      combo('format', 'docx', ['md', 'commonmark', 'html', 'docx', 'odt', 'rtf', 'epub', 'pptx', 'pdf', 'txt', 'rst', 'tex', 'typst', 'asciidoc', 'org', 'mediawiki', 'textile', 'jira', 'docbook', 'man', 'ipynb', 'json'], {
        labels: { md: 'Markdown (GitHub)', commonmark: 'CommonMark', tex: 'LaTeX', txt: 'plain text', ipynb: 'Jupyter notebook', json: 'Pandoc JSON', man: 'man page' },
        groups: { Common: ['md', 'html', 'docx', 'odt', 'pdf', 'epub', 'txt', 'pptx', 'rtf'], Markup: ['commonmark', 'rst', 'tex', 'typst', 'asciidoc', 'org', 'textile'], Wiki: ['mediawiki', 'jira', 'docbook', 'man'], Other: ['ipynb', 'json'] },
      }),
      toggle('standalone', true, { label: 'standalone document' }),
      toggle('toc', false, { label: 'table of contents' }),
      ...ADV([
        toggle('number_sections', false, { label: 'number sections' }),
        seg('wrap', 'auto', ['auto', 'none', 'preserve'], { label: 'line wrap' }),
        toggle('embed_resources', true, { label: 'embed images', ...when('format', 'html') }),
        path('reference_doc', { label: 'style template', filters: [{ name: 'Templates', extensions: ['docx', 'odt', 'pptx'] }], ...when('format', 'docx', 'odt', 'pptx') }),
      ]),
    ],
  },
  'doc.markdown': {
    cat: 'doc', title: 'Markdown to HTML', icon: 'code', keywords: 'markdown md html page readme render',
    desc: 'Renders Markdown into a clean, styled HTML page (no extra tools needed).',
    inputs: DOC, outputs: DOC,
    widgets: [seg('theme', 'light', ['light', 'dark', 'serif', 'none']), toggle('smart_quotes', true, { label: 'smart quotes' })],
  },
  'doc.text': {
    cat: 'doc', title: 'Extract Text', icon: 'text', keywords: 'text extract docx pdf html epub plain txt',
    desc: 'Pulls plain text out of DOCX, ODT, PPTX, EPUB, HTML, PDF and more.',
    inputs: DOC, outputs: [io('texts', 'TEXT')],
    widgets: [toggle('combine', false, { label: 'one file for all' })],
  },

  // ---------------------------------------------------------------- pdf
  'pdf.merge': {
    cat: 'pdf', title: 'Merge PDFs', icon: 'merge', keywords: 'pdf merge combine join',
    desc: 'Joins PDF files into one document.',
    inputs: PDF, outputs: [io('pdf', 'PDF')],
    widgets: [seg('order', 'input', ['input', 'name'], { label: 'order', labels: { input: 'as connected', name: 'by name' } }), text('name', 'merged')],
  },
  'pdf.split': {
    cat: 'pdf', title: 'Split PDF', icon: 'scissors', keywords: 'pdf split pages extract ranges',
    desc: 'One PDF per page, per N pages or per page range.',
    inputs: PDF, outputs: PDF,
    widgets: [
      seg('mode', 'pages', ['pages', 'every', 'ranges'], { labels: { pages: 'each page', every: 'every N', ranges: 'ranges' } }),
      num('every', 2, { min: 1, label: 'pages per file', ...when('mode', 'every') }),
      text('ranges', '1-3, 4-', { placeholder: '1-3, 4-6, 7-', ...when('mode', 'ranges') }),
    ],
  },
  'pdf.pages': {
    cat: 'pdf', title: 'Pages: Keep / Delete / Rotate', icon: 'pages', keywords: 'pdf pages delete remove rotate reorder reverse odd even',
    desc: 'Keep, delete or rotate pages, reverse the order or take odd / even pages.',
    inputs: PDF, outputs: PDF,
    widgets: [
      combo('op', 'keep', ['keep', 'delete', 'rotate', 'reverse', 'odd', 'even'], { labels: { keep: 'keep pages', delete: 'delete pages', rotate: 'rotate pages', reverse: 'reverse order', odd: 'odd pages only', even: 'even pages only' } }),
      text('pages', '1-3', { placeholder: '1-3, 5, 8-, last', ...when('op', 'keep', 'delete', 'rotate') }),
      seg('angle', 90, [90, 180, 270], { labels: { 90: '90°', 180: '180°', 270: '270°' }, ...when('op', 'rotate') }),
    ],
  },
  'pdf.meta': {
    cat: 'pdf', title: 'PDF Info & Metadata', icon: 'tag', width: 290, keywords: 'pdf metadata title author info pages report',
    desc: 'Reads page count and metadata into a report, sets title / author, or strips metadata.',
    inputs: PDF, outputs: [io('pdfs', 'PDF'), REPORT],
    widgets: [
      seg('mode', 'report', ['report', 'set', 'strip'], { labels: { report: 'read', set: 'set', strip: 'remove' } }),
      seg('format', 'json', ['json', 'csv'], when('mode', 'report')),
      text('title', '', when('mode', 'set')),
      text('author', '', when('mode', 'set')),
      text('subject', '', when('mode', 'set')),
      text('keywords', '', when('mode', 'set')),
      text('creator', '', when('mode', 'set')),
    ],
  },
  'pdf.compress': {
    cat: 'pdf', title: 'Compress PDF', icon: 'compress', keywords: 'pdf compress shrink reduce size ghostscript',
    desc: 'Shrinks PDFs: lossless clean-up, or Ghostscript presets for screen, e-book or print.',
    inputs: PDF, outputs: PDF,
    widgets: [
      combo('mode', 'ebook', ['lossless', 'screen', 'ebook', 'printer', 'prepress'], { labels: { lossless: 'lossless (built-in)', screen: 'smallest (72 dpi)', ebook: 'balanced (150 dpi)', printer: 'print (300 dpi)', prepress: 'prepress' } }),
      toggle('keep_smaller', true, { label: 'keep original if not smaller' }),
    ],
  },
  'pdf.text': {
    cat: 'pdf', title: 'PDF to Text', icon: 'text', keywords: 'pdf text extract copy',
    desc: 'Extracts the text layer of PDFs into .txt files.',
    inputs: PDF, outputs: [io('texts', 'TEXT')],
    widgets: [toggle('page_marks', true, { label: 'mark page breaks' })],
  },
  'pdf.render': {
    cat: 'pdf', title: 'PDF to Images', icon: 'image', keywords: 'pdf to png jpg render pages images',
    desc: 'Renders PDF pages to PNG or JPG images.',
    inputs: PDF, outputs: [io('images', 'IMAGE')],
    widgets: [
      seg('format', 'png', ['png', 'jpg']),
      num('dpi', 150, { min: 36, max: 1200, step: 10 }),
      num('first', 1, { min: 1, label: 'first page' }),
      num('last', 0, { min: 0, label: 'last page (0 = end)' }),
    ],
  },

  // ---------------------------------------------------------------- spreadsheets
  'sheet.convert': {
    cat: 'sheet', title: 'Convert Spreadsheet', icon: 'sheet', width: 310, keywords: 'excel xlsx xls ods csv tsv json sql html markdown table convert',
    desc: 'Excel, ODS, CSV, TSV, JSON, YAML, HTML, Markdown, XML and SQL tables in any direction.',
    inputs: SHT, outputs: SHT,
    widgets: [
      combo('format', 'xlsx', ['xlsx', 'csv', 'tsv', 'json', 'ndjson', 'yaml', 'html', 'md', 'xml', 'sql', 'txt', 'ods', 'xls', 'pdf'], {
        labels: { xlsx: 'Excel (xlsx)', xls: 'Excel 97 (xls)', ods: 'OpenDocument (ods)', md: 'Markdown table', sql: 'SQL inserts', txt: 'text table', pdf: 'PDF' },
        groups: { Spreadsheet: ['xlsx', 'xls', 'ods', 'csv', 'tsv'], Data: ['json', 'ndjson', 'yaml', 'xml', 'sql'], Document: ['html', 'md', 'txt', 'pdf'] },
      }),
      seg('multi', 'separate', ['separate', 'merge', 'first'], { label: 'many sheets', labels: { separate: 'file each', merge: 'merge', first: 'first' } }),
      ...G('Read', [
        seg('sheets', 'all', ['all', 'first', 'name'], { labels: { all: 'all sheets', first: 'first sheet', name: 'by name' } }),
        text('sheet_name', '', { label: 'sheet name', placeholder: 'Sheet1', ...when('sheets', 'name') }),
        num('skip_rows', 0, { min: 0, label: 'skip rows' }),
        combo('input_delimiter', 'auto', ['auto', ',', ';', 'tab', '|', 'space'], { label: 'CSV delimiter' }),
        combo('input_encoding', 'auto', ['auto', 'utf-8', 'windows-1251', 'windows-1252', 'koi8-r', 'utf-16le', 'iso-8859-1', 'shift_jis', 'gbk'], { label: 'CSV encoding' }),
        combo('date_format', 'iso', ['iso', 'dmy', 'mdy', 'serial'], { label: 'dates', labels: { iso: '2026-01-31', dmy: '31.01.2026', mdy: '01/31/2026', serial: 'Excel number' } }),
        toggle('trim', true, { label: 'trim spaces' }),
        toggle('drop_empty', true, { label: 'drop empty rows' }),
        toggle('infer_types', true, { label: 'detect numbers' }),
        toggle('flatten', true, { label: 'flatten nested JSON' }),
      ], true),
      ...G('Write', [
        toggle('header', true, { label: 'first row is header' }),
        combo('delimiter', ',', [',', ';', 'tab', '|'], { label: 'CSV delimiter', ...when('format', 'csv', 'txt') }),
        combo('encoding', 'utf-8', ['utf-8', 'utf-8-bom', 'windows-1251', 'windows-1252', 'utf-16le'], { label: 'CSV encoding', labels: { 'utf-8-bom': 'utf-8 + BOM (Excel)' }, ...when('format', 'csv', 'tsv', 'txt') }),
        seg('json_shape', 'objects', ['objects', 'arrays', 'columns', 'keyed'], { label: 'JSON shape', ...when('format', 'json', 'ndjson', 'yaml') }),
        toggle('pretty', true, when('format', 'json', 'xml')),
        text('sql_table', '', { label: 'SQL table', placeholder: 'file name', ...when('format', 'sql') }),
        toggle('bold_header', true, { label: 'bold header', ...when('format', 'xlsx') }),
        toggle('freeze_header', true, { label: 'freeze header', ...when('format', 'xlsx') }),
        toggle('autofilter', true, { label: 'filter buttons', ...when('format', 'xlsx') }),
        toggle('autofit', true, { label: 'fit column widths', ...when('format', 'xlsx') }),
        toggle('zebra', false, { label: 'striped rows', ...when('format', 'xlsx', 'html') }),
      ], true),
    ],
  },
  'sheet.tools': {
    cat: 'sheet', title: 'Table Tools', icon: 'columns', width: 310, keywords: 'filter sort columns rename dedupe transpose replace math formula clean excel',
    desc: 'Select, rename, filter, sort, dedupe, transpose, fill, replace, split columns or do math, for every sheet.',
    inputs: SHT, outputs: SHT,
    widgets: [
      combo('op', 'filter', ['select', 'drop', 'rename', 'filter', 'sort', 'dedupe', 'transpose', 'limit', 'fill', 'replace', 'case', 'index', 'clean', 'split', 'math'], {
        label: 'action',
        labels: { select: 'keep columns', drop: 'remove columns', rename: 'rename columns', filter: 'filter rows', sort: 'sort rows', dedupe: 'remove duplicate rows', transpose: 'transpose', limit: 'take rows', fill: 'fill empty cells', replace: 'find & replace', case: 'change case', index: 'add row number', clean: 'clean text', split: 'split column', math: 'math on column' },
      }),
      text('columns', '', { placeholder: 'A, C:E, Name, Price', ...when('op', 'select', 'drop', 'dedupe', 'fill', 'replace', 'case', 'clean', 'math') }),
      area('mapping', '', { placeholder: 'old name = new name', rows: 3, ...when('op', 'rename') }),
      text('column', '', { placeholder: 'B or Price', ...when('op', 'filter', 'sort', 'split') }),
      combo('cmp', 'contains', ['eq', 'ne', 'gt', 'lt', 'ge', 'le', 'contains', 'not_contains', 'starts', 'ends', 'regex', 'empty', 'not_empty'], { label: 'condition', labels: { eq: '=', ne: '≠', gt: '>', lt: '<', ge: '≥', le: '≤', not_contains: 'does not contain', starts: 'starts with', ends: 'ends with', not_empty: 'not empty' }, ...when('op', 'filter') }),
      text('value', '', { ...when('op', 'filter', 'fill', 'math') }),
      seg('order', 'asc', ['asc', 'desc'], { labels: { asc: 'A → Z', desc: 'Z → A' }, ...when('op', 'sort') }),
      num('offset', 0, { min: 0, label: 'skip rows', ...when('op', 'limit') }),
      num('count', 100, { min: 0, label: 'rows', ...when('op', 'limit') }),
      seg('fill_mode', 'down', ['down', 'value'], { label: 'fill with', labels: { down: 'value above', value: 'fixed value' }, ...when('op', 'fill') }),
      text('find', '', when('op', 'replace')),
      text('replace', '', { label: 'replace with', ...when('op', 'replace') }),
      toggle('regex', false, { label: 'regular expression', ...when('op', 'replace') }),
      seg('case', 'upper', ['upper', 'lower', 'title', 'clean'], when('op', 'case')),
      text('name', '#', { label: 'column name', ...when('op', 'index') }),
      text('separator', ',', when('op', 'split')),
      combo('math_op', 'multiply', ['multiply', 'add', 'subtract', 'divide', 'round', 'percent'], { label: 'operation', labels: { multiply: '× multiply', add: '+ add', subtract: '− subtract', divide: '÷ divide', round: 'round to N digits', percent: 'as percent' }, ...when('op', 'math') }),
      ...ADV([
        toggle('header', true, { label: 'first row is header' }),
        combo('format', 'keep', ['keep', 'xlsx', 'csv', 'tsv', 'json', 'ods', 'html', 'md'], { label: 'save as', labels: { keep: 'same format' } }),
      ]),
    ],
  },
  'sheet.merge': {
    cat: 'sheet', title: 'Merge Tables', icon: 'merge', keywords: 'merge combine append stack tables sheets workbook excel',
    desc: 'Appends rows of all tables, puts each file on its own sheet, or joins columns side by side.',
    inputs: SHT, outputs: [io('table', 'SHEET')],
    widgets: [
      seg('mode', 'rows', ['rows', 'sheets', 'columns'], { labels: { rows: 'stack rows', sheets: 'one workbook', columns: 'side by side' } }),
      combo('format', 'xlsx', ['xlsx', 'csv', 'tsv', 'json', 'ods', 'html', 'md']),
      text('name', 'merged'),
      toggle('add_source', false, { label: 'add source column', ...when('mode', 'rows') }),
    ],
  },
  'sheet.split': {
    cat: 'sheet', title: 'Split Table', icon: 'split', keywords: 'split table rows chunks by column sheets excel',
    desc: 'Splits a table every N rows, by the values of a column, or into one file per sheet.',
    inputs: SHT, outputs: SHT,
    widgets: [
      seg('mode', 'rows', ['rows', 'column', 'sheets'], { labels: { rows: 'every N rows', column: 'by column', sheets: 'per sheet' } }),
      num('rows', 1000, { min: 1, ...when('mode', 'rows') }),
      text('column', '', { placeholder: 'B or City', ...when('mode', 'column') }),
      combo('format', 'keep', ['keep', 'xlsx', 'csv', 'tsv', 'json', 'ods'], { labels: { keep: 'same format' } }),
    ],
  },

  // ---------------------------------------------------------------- data
  'data.convert': {
    cat: 'data', title: 'Convert Data', icon: 'data', keywords: 'json yaml toml csv tsv ndjson config',
    desc: 'JSON, YAML, TOML, CSV, TSV and NDJSON in any direction.',
    inputs: DAT, outputs: DAT,
    widgets: [
      combo('format', 'json', ['json', 'yaml', 'toml', 'csv', 'tsv', 'ndjson']),
      toggle('pretty', true, when('format', 'json')),
      num('indent', 2, { min: 0, max: 8, ...when('format', 'json') }),
      combo('delimiter', ',', [',', ';', '|', 'tab'], when('format', 'csv')),
      toggle('infer_types', true, { label: 'detect numbers' }),
    ],
  },
  'data.json': {
    cat: 'data', title: 'JSON Tools', icon: 'code', keywords: 'json pretty minify sort keys flatten schema extract jq path',
    desc: 'Pretty print, minify, sort keys, flatten / unflatten, extract a path, list keys or infer a schema.',
    inputs: DAT, outputs: DAT,
    widgets: [
      combo('op', 'pretty', ['pretty', 'minify', 'sort_keys', 'flatten', 'unflatten', 'extract', 'keys', 'schema'], { labels: { sort_keys: 'sort keys', extract: 'extract path', keys: 'list keys', schema: 'JSON schema' } }),
      num('indent', 2, { min: 0, max: 8, ...when('op', 'pretty', 'sort_keys', 'flatten', 'unflatten', 'extract', 'schema') }),
      text('path', '', { placeholder: 'users.0.name or items[*].id', ...when('op', 'extract') }),
      text('separator', '.', when('op', 'flatten', 'unflatten')),
    ],
  },
  'data.encode': {
    cat: 'data', title: 'Encode / Decode', icon: 'key', keywords: 'base64 hex url encode decode escape html rot13 unicode',
    desc: 'Base64, hex, URL, HTML entities, Unicode escapes and ROT13, both ways.',
    inputs: FIL, outputs: FIL,
    widgets: [
      combo('op', 'base64-encode', ['base64-encode', 'base64-decode', 'hex-encode', 'hex-decode', 'url-encode', 'url-decode', 'html-escape', 'html-unescape', 'unicode-escape', 'unicode-unescape', 'rot13']),
    ],
  },

  // ---------------------------------------------------------------- text
  'text.transform': {
    cat: 'text', title: 'Text Tools', icon: 'text', width: 300, keywords: 'encoding utf8 cp1251 line endings replace',
    desc: 'Fix encodings, line endings, whitespace and case, find and replace.',
    inputs: [io('files', 'TEXT')], outputs: [io('files', 'TEXT')],
    widgets: [
      combo('encoding_in', 'auto', ['auto', 'utf-8', 'utf-16le', 'utf-16be', 'windows-1251', 'koi8-r', 'windows-1252', 'iso-8859-1', 'shift_jis', 'gbk'], { label: 'read as' }),
      combo('encoding_out', 'utf-8', ['keep', 'utf-8', 'utf-8-bom', 'utf-16le', 'utf-16be', 'windows-1251', 'koi8-r', 'windows-1252'], { label: 'write as' }),
      text('find', ''),
      text('replace', '', { label: 'replace with' }),
      toggle('regex', false, { label: 'regular expression' }),
      ...ADV([
        combo('line_endings', 'keep', ['keep', 'lf', 'crlf'], { label: 'line endings', labels: { lf: 'LF (Linux)', crlf: 'CRLF (Windows)' } }),
        toggle('trim_trailing', false, { label: 'trim trailing spaces' }),
        seg('case', 'keep', ['keep', 'lower', 'upper']),
        num('tabs_to_spaces', 0, { min: 0, max: 8, label: 'tabs to spaces (0 = off)' }),
      ]),
    ],
  },
  'text.lines': {
    cat: 'text', title: 'Line Tools', icon: 'list', width: 290, keywords: 'lines sort unique dedupe reverse shuffle number prefix wrap head tail grep',
    desc: 'Sort, dedupe, reverse, shuffle, number, filter, wrap, join or cut lines of text files.',
    inputs: TXT, outputs: TXT,
    widgets: [
      combo('op', 'sort', ['sort', 'sort_length', 'unique', 'reverse', 'shuffle', 'trim', 'remove_empty', 'number', 'prefix', 'suffix', 'keep', 'remove', 'wrap', 'join', 'split', 'head', 'tail'], {
        labels: { sort_length: 'sort by length', unique: 'remove duplicates', remove_empty: 'remove empty lines', number: 'number lines', prefix: 'add prefix', suffix: 'add suffix', keep: 'keep matching', remove: 'remove matching', wrap: 'wrap long lines', join: 'join into one line', split: 'split into lines', head: 'first N lines', tail: 'last N lines' },
      }),
      seg('order', 'asc', ['asc', 'desc'], { labels: { asc: 'A → Z', desc: 'Z → A' }, ...when('op', 'sort', 'sort_length') }),
      toggle('natural', true, { label: 'natural order (2 < 10)', ...when('op', 'sort') }),
      toggle('ignore_case', true, { label: 'ignore case', ...when('op', 'sort', 'unique', 'keep', 'remove') }),
      text('text', '', { label: 'text', ...when('op', 'prefix', 'suffix') }),
      text('pattern', '', { ...when('op', 'keep', 'remove') }),
      toggle('regex', false, { label: 'regular expression', ...when('op', 'keep', 'remove') }),
      num('count', 10, { min: 0, label: 'lines', ...when('op', 'head', 'tail') }),
      num('width', 80, { min: 1, label: 'width', ...when('op', 'wrap') }),
      text('separator', ', ', { ...when('op', 'join', 'split') }),
    ],
  },
  'text.merge': {
    cat: 'text', title: 'Merge Text Files', icon: 'merge', keywords: 'merge concat join text files combine',
    desc: 'Joins text files into one, with optional headers between them.',
    inputs: TXT, outputs: [io('text', 'TEXT')],
    widgets: [
      text('name', 'merged'),
      toggle('headers', true, { label: 'file name headers' }),
      text('header', '===== {name} =====', when('headers', true)),
      seg('separator', 'blank', ['none', 'newline', 'blank', 'rule'], { labels: { blank: 'empty line', rule: '-----' } }),
    ],
  },
  'text.split': {
    cat: 'text', title: 'Split Text File', icon: 'scissors', keywords: 'split text lines chunks size regex',
    desc: 'Splits text files every N lines, at a regex match or by size.',
    inputs: TXT, outputs: TXT,
    widgets: [
      seg('mode', 'lines', ['lines', 'regex', 'size'], { labels: { regex: 'at match' } }),
      num('lines', 1000, { min: 1, ...when('mode', 'lines') }),
      text('pattern', '^-{3,}$', when('mode', 'regex')),
      num('size_kb', 512, { min: 1, unit: 'KB', label: 'part size', ...when('mode', 'size') }),
    ],
  },
  'text.extract': {
    cat: 'text', title: 'Extract Emails, Links…', icon: 'search', keywords: 'extract emails urls links phones numbers hashtags ip dates regex grep',
    desc: 'Pulls e-mails, links, phone numbers, hashtags, IPs, dates or any regex out of text.',
    inputs: TXT, outputs: [io('found', 'TEXT')],
    widgets: [
      combo('what', 'emails', ['emails', 'urls', 'phones', 'numbers', 'hashtags', 'mentions', 'ips', 'dates', 'words', 'custom'], { labels: { urls: 'links', ips: 'IP addresses', custom: 'custom regex' } }),
      text('pattern', '', { placeholder: '[A-Z]{3}-\\d+', ...when('what', 'custom') }),
      toggle('unique', true, { label: 'unique only' }),
      seg('format', 'txt', ['txt', 'csv']),
      toggle('combine', true, { label: 'one file for all' }),
    ],
  },
  'text.stats': {
    cat: 'text', title: 'Text Statistics', icon: 'chart', keywords: 'word count characters lines statistics reading time',
    desc: 'Counts lines, words, characters, reading time and the most common words.',
    inputs: TXT, outputs: [io('texts', 'TEXT'), REPORT],
    widgets: [seg('format', 'csv', ['csv', 'json']), num('top_words', 10, { min: 0, max: 100, label: 'top words' })],
  },

  // ---------------------------------------------------------------- 3D
  'mesh.convert': {
    cat: 'mesh', title: 'Convert 3D', icon: 'convert', keywords: 'glb gltf obj stl ply off model',
    desc: 'Converts between GLB, OBJ, STL, PLY and OFF.',
    inputs: MSH, outputs: MSH,
    widgets: [combo('format', 'glb', ['glb', 'obj', 'stl', 'ply', 'off']), toggle('binary', true, when('format', 'stl', 'ply'))],
  },
  'mesh.transform': {
    cat: 'mesh', title: 'Transform 3D', icon: 'rotate', width: 300, keywords: 'scale units rotate center axis',
    desc: 'Scale, change units, rotate, move, center and fix the up axis.',
    inputs: MSH, outputs: MSH,
    widgets: [
      num('scale', 1, { min: 0.0001, step: 0.1 }),
      combo('units', 'none', ['none', 'mm to m', 'm to mm', 'cm to m', 'm to cm', 'inch to mm', 'mm to inch']),
      combo('center', 'none', ['none', 'center', 'center + ground']),
      combo('up_axis', 'keep', ['keep', 'Z-up to Y-up', 'Y-up to Z-up'], { label: 'up axis' }),
      ...G('Rotate & move', [
        num('rotate_x', 0, { step: 15, unit: '°', label: 'rotate X' }),
        num('rotate_y', 0, { step: 15, unit: '°', label: 'rotate Y' }),
        num('rotate_z', 0, { step: 15, unit: '°', label: 'rotate Z' }),
        num('translate_x', 0, { step: 0.1, label: 'move X' }),
        num('translate_y', 0, { step: 0.1, label: 'move Y' }),
        num('translate_z', 0, { step: 0.1, label: 'move Z' }),
        num('fit_size', 0, { min: 0, step: 0.1, label: 'fit to size (0 = off)' }),
        toggle('flip_normals', false, { label: 'flip normals' }),
      ], true),
    ],
  },
  'mesh.optimize': {
    cat: 'mesh', title: 'Optimize 3D', icon: 'sparkle', keywords: 'decimate simplify reduce weld normals',
    desc: 'Weld vertices, reduce the triangle count and rebuild normals.',
    inputs: MSH, outputs: MSH,
    widgets: [
      slider('keep_percent', 100, 1, 100, { unit: '%', label: 'keep triangles' }),
      seg('normals', 'keep', ['keep', 'smooth', 'flat']),
      toggle('weld', true, { label: 'weld vertices' }),
      ...ADV([num('weld_tolerance', 0.0001, { min: 0, step: 0.0001, label: 'weld tolerance', ...when('weld', true) }), toggle('remove_degenerate', true, { label: 'drop degenerate' })]),
    ],
  },

  // ---------------------------------------------------------------- archives
  'archive.create': {
    cat: 'archive', title: 'Create Archive', icon: 'archive', keywords: 'zip 7z tar pack compress backup password encrypt',
    desc: 'ZIP, 7z or tar.gz/zst/xz/bz2, one archive or one per file, optionally with a password.',
    inputs: FIL, outputs: [io('archive', 'ARCHIVE')],
    widgets: [
      combo('format', 'zip', ['zip', '7z', 'tar', 'tar.gz', 'tar.zst', 'tar.xz', 'tar.bz2']),
      slider('level', 6, 0, 9, unless('format', 'tar')),
      seg('mode', 'single', ['single', 'per-file'], { labels: { single: 'one archive', 'per-file': 'one per file' } }),
      text('name', 'archive', when('mode', 'single')),
      text('password', '', { placeholder: 'optional (zip / 7z)', ...when('format', 'zip', '7z') }),
    ],
  },
  'archive.extract': {
    cat: 'archive', title: 'Extract Archive', icon: 'package', keywords: 'unzip unpack extract rar 7z iso password',
    desc: 'Unpacks ZIP, 7z, RAR, ISO, CAB and tar archives, including password protected ones.',
    inputs: [io('archives', 'ARCHIVE')], outputs: FIL,
    widgets: [toggle('subfolder', true, { label: 'folder per archive' }), text('password', '', { placeholder: 'if protected' })],
  },
  'archive.compress': {
    cat: 'archive', title: 'Compress File', icon: 'compress', keywords: 'zstd gzip xz bzip2',
    desc: 'Compresses each file on its own with zstd, gzip, xz or bzip2.',
    inputs: FIL, outputs: [io('archives', 'ARCHIVE')],
    widgets: [seg('format', 'zst', ['zst', 'gz', 'xz', 'bz2']), slider('level', 6, 1, 9)],
  },
  'archive.test': {
    cat: 'archive', title: 'Test Archives', icon: 'shield', keywords: 'test verify archive integrity corrupted crc',
    desc: 'Checks that archives open and every entry is intact.',
    inputs: [io('archives', 'ARCHIVE')], outputs: [io('ok', 'ARCHIVE'), io('bad', 'ARCHIVE'), REPORT], widgets: [],
  },

  // ---------------------------------------------------------------- utility
  'util.note': {
    cat: 'util', title: 'Note', icon: 'note', frontendOnly: true, width: 260, keywords: 'comment sticky text',
    desc: 'A sticky note for comments. It does not run.',
    inputs: [], outputs: [],
    widgets: [area('text', '', { label: '', placeholder: 'Write a note…', rows: 5 })],
  },
  'util.reroute': {
    cat: 'util', title: 'Reroute', icon: 'wire', width: 200, keywords: 'reroute dot wire tidy pass',
    desc: 'Passes files through unchanged. Handy to tidy up long links.',
    inputs: [io('in', 'FILES')], outputs: [io('out', 'FILES')], widgets: [],
  },
  'util.switch': {
    cat: 'util', title: 'Switch', icon: 'switch', width: 220, keywords: 'switch choose toggle branch a b',
    desc: 'Passes either input A or input B, so you can flip between two branches.',
    inputs: [io('a', 'FILES'), io('b', 'FILES')], outputs: [io('out', 'FILES')],
    widgets: [seg('use', 'a', ['a', 'b'], { labels: { a: 'A', b: 'B' } }), toggle('fallback', true, { label: 'use the other if empty' })],
  },
};

export const NODE_COUNT = Object.keys(NODE_DEFS).length;
