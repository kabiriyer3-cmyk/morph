// Node catalog. Pure data: imported by the UI and by scripts/check-frontend.mjs.
// Widget names and option values must match what src-tauri/src/ops reads.

export const TYPES = {
  FILES: '#a0a0a0',
  IMAGE: '#64B5F6',
  VIDEO: '#B39DDB',
  AUDIO: '#81C784',
  MESH: '#FFA931',
  ARCHIVE: '#FF6E6E',
  DATA: '#FFD500',
  DOC: '#FF9CF9',
};

export const CATEGORIES = {
  files: { color: '#a0a0a0', icon: 'files' },
  image: { color: '#64B5F6', icon: 'image' },
  video: { color: '#B39DDB', icon: 'video' },
  audio: { color: '#81C784', icon: 'audio' },
  mesh: { color: '#FFA931', icon: 'mesh' },
  archive: { color: '#FF6E6E', icon: 'archive' },
  data: { color: '#FFD500', icon: 'data' },
  util: { color: '#7d7d86', icon: 'note' },
};

export const typeColor = (type) => TYPES[type] || TYPES.FILES;
// FILES is the wildcard: any list of files can go anywhere, nodes skip kinds they do not handle.
export const compatible = (from, to) => from === to || from === 'FILES' || to === 'FILES';

const io = (name, type) => ({ name, type });
const num = (name, value, o = {}) => ({ kind: 'number', name, value, step: 1, ...o });
const slider = (name, value, min, max, o = {}) => ({ kind: 'slider', name, value, min, max, step: 1, ...o });
const combo = (name, value, options, o = {}) => ({ kind: 'combo', name, value, options, ...o });
const toggle = (name, value, o = {}) => ({ kind: 'toggle', name, value, ...o });
const text = (name, value = '', o = {}) => ({ kind: 'text', name, value, ...o });
const area = (name, value = '', o = {}) => ({ kind: 'textarea', name, value, ...o });
const path = (name, o = {}) => ({ kind: 'path', name, value: '', ...o });
const color = (name, value, o = {}) => ({ kind: 'color', name, value, ...o });
const when = (key, ...values) => ({ show: { [key]: values } });
const unless = (key, ...values) => ({ hide: { [key]: values } });

const FIL = [io('files', 'FILES')];
const IMG = [io('images', 'IMAGE')];
const VID = [io('videos', 'VIDEO')];
const AUD = [io('audio', 'AUDIO')];
const MSH = [io('models', 'MESH')];
const DAT = [io('data', 'DATA')];
const DOC = [io('docs', 'DOC')];

const ANCHORS = ['center', 'top-left', 'top', 'top-right', 'left', 'right', 'bottom-left', 'bottom', 'bottom-right'];
const PRESETS = ['ultrafast', 'superfast', 'veryfast', 'faster', 'fast', 'medium', 'slow', 'slower', 'veryslow'];
const DEG = { 0: '0°', 90: '90°', 180: '180°', 270: '270°' };
const VCODEC = { h264: 'H.264', h265: 'H.265 / HEVC', vp9: 'VP9', av1: 'AV1', prores: 'ProRes', copy: 'copy (no re-encode)' };
const ACODEC = { aac: 'AAC', opus: 'Opus', mp3: 'MP3', flac: 'FLAC', copy: 'copy', none: 'no audio' };
const HW = { none: 'CPU', nvidia: 'NVIDIA NVENC', intel: 'Intel QSV', amd: 'AMD AMF' };

export const NODE_DEFS = {
  // files
  'files.load': {
    cat: 'files', title: 'Load Files', width: 300, keywords: 'open input folder add',
    desc: 'Pick files or whole folders, or simply drop them onto the canvas.',
    inputs: [], outputs: FIL,
    widgets: [
      { kind: 'paths', name: 'paths', value: [] },
      toggle('recursive', true, { label: 'include subfolders' }),
      text('include', '', { placeholder: '*.jpg, *.png' }),
      text('exclude', '', { placeholder: '*.tmp, Thumbs.db' }),
      toggle('skip_hidden', true, { label: 'skip hidden' }),
    ],
  },
  'files.save': {
    cat: 'files', title: 'Save', output: true, keywords: 'export write output folder',
    desc: 'Copies results into a folder. Originals are never modified.',
    inputs: FIL, outputs: [io('saved', 'FILES')],
    widgets: [
      combo('destination', 'folder', ['folder', 'next to original']),
      path('folder', { folder: true, placeholder: '~/Morph Output', ...when('destination', 'folder') }),
      text('pattern', '{name}', { label: 'file name', placeholder: '{name}_{n:3}' }),
      toggle('keep_structure', true, { label: 'keep folders', ...when('destination', 'folder') }),
      combo('on_conflict', 'rename', ['rename', 'overwrite', 'skip'], { label: 'if exists' }),
      toggle('open_folder', false, { label: 'open folder after' }),
    ],
  },
  'files.preview': {
    cat: 'files', title: 'Preview', output: true, preview: true, width: 320, keywords: 'view show thumbnails',
    desc: 'Shows thumbnails of the files that reach it. Click one to inspect it.',
    inputs: FIL, outputs: FIL, widgets: [],
  },
  'files.filter': {
    cat: 'files', title: 'Filter', keywords: 'select match glob size',
    desc: 'Splits files by type, name pattern or size.',
    inputs: FIL, outputs: [io('match', 'FILES'), io('rest', 'FILES')],
    widgets: [
      combo('kind', 'any', ['any', 'image', 'video', 'audio', 'mesh', 'archive', 'data', 'doc', 'text', 'other']),
      text('pattern', '', { label: 'name', placeholder: '*.jpg; IMG_*' }),
      toggle('regex', false, { label: 'regular expression' }),
      num('min_kb', 0, { min: 0, unit: 'KB', label: 'min size' }),
      num('max_kb', 0, { min: 0, unit: 'KB', label: 'max size' }),
    ],
  },
  'files.split': {
    cat: 'files', title: 'Split by Type', keywords: 'route sort kind',
    desc: 'Routes images, videos, audio and 3D models to separate outputs.',
    inputs: FIL,
    outputs: [io('images', 'IMAGE'), io('videos', 'VIDEO'), io('audio', 'AUDIO'), io('models', 'MESH'), io('other', 'FILES')],
    widgets: [],
  },
  'files.merge': {
    cat: 'files', title: 'Merge', keywords: 'join combine union',
    desc: 'Joins up to four file lists into one.',
    inputs: [io('a', 'FILES'), io('b', 'FILES'), io('c', 'FILES'), io('d', 'FILES')], outputs: FIL, widgets: [],
  },
  'files.rename': {
    cat: 'files', title: 'Rename', keywords: 'batch rename number pattern',
    desc: 'Batch rename with patterns, numbering, find and replace and case changes.',
    inputs: FIL, outputs: FIL,
    widgets: [
      text('pattern', '{name}', { placeholder: '{name}_{n}' }),
      text('find', ''),
      text('replace', '', { label: 'replace with' }),
      toggle('regex', false, { label: 'regular expression' }),
      combo('case', 'keep', ['keep', 'lower', 'upper', 'title', 'snake', 'kebab']),
      num('start', 1, { min: 0, label: 'first number' }),
      num('digits', 3, { min: 0, max: 12 }),
    ],
  },
  'files.hash': {
    cat: 'files', title: 'Checksums', keywords: 'hash sha md5 verify integrity',
    desc: 'Computes SHA-256, BLAKE3, MD5 and more and writes a report file.',
    inputs: FIL, outputs: [io('files', 'FILES'), io('report', 'DATA')],
    widgets: [
      combo('algorithm', 'sha256', ['sha256', 'sha1', 'md5', 'blake3', 'crc32', 'sha512']),
      combo('format', 'sums', ['sums', 'csv', 'json'], { labels: { sums: 'checksum file' } }),
    ],
  },
  'files.dedupe': {
    cat: 'files', title: 'Remove Duplicates', keywords: 'duplicate same identical',
    desc: 'Finds identical files by content, name or size.',
    inputs: FIL, outputs: [io('unique', 'FILES'), io('duplicates', 'FILES')],
    widgets: [combo('by', 'content', ['content', 'name', 'name+size'], { label: 'compare' })],
  },
  'files.info': {
    cat: 'files', title: 'File Info', keywords: 'report list table metadata',
    desc: 'Writes a CSV or JSON table with sizes, dates, dimensions and durations.',
    inputs: FIL, outputs: [io('files', 'FILES'), io('report', 'DATA')],
    widgets: [combo('format', 'csv', ['csv', 'json'])],
  },

  // images
  'image.resize': {
    cat: 'image', title: 'Resize', keywords: 'scale shrink downscale',
    desc: 'Fit, fill, stretch or scale images with high quality filters.',
    inputs: IMG, outputs: IMG,
    widgets: [
      combo('mode', 'fit', ['fit', 'fill', 'stretch', 'percent', 'width', 'height']),
      num('width', 1920, { min: 1, max: 16384, unit: 'px', ...when('mode', 'fit', 'fill', 'stretch', 'width') }),
      num('height', 1080, { min: 1, max: 16384, unit: 'px', ...when('mode', 'fit', 'fill', 'stretch', 'height') }),
      slider('percent', 50, 1, 400, { unit: '%', ...when('mode', 'percent') }),
      combo('filter', 'lanczos3', ['lanczos3', 'catmull-rom', 'triangle', 'gaussian', 'nearest']),
      toggle('only_shrink', true, { label: 'never upscale' }),
    ],
  },
  'image.crop': {
    cat: 'image', title: 'Crop', keywords: 'cut aspect trim borders',
    desc: 'Crop to an aspect ratio, an exact box or trim empty borders.',
    inputs: IMG, outputs: IMG,
    widgets: [
      combo('mode', 'aspect', ['aspect', 'manual', 'trim']),
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
    cat: 'image', title: 'Rotate / Flip', keywords: 'rotate mirror flip orientation',
    desc: 'Rotate in 90° steps and mirror images.',
    inputs: IMG, outputs: IMG,
    widgets: [
      combo('rotate', '0', ['0', '90', '180', '270'], { labels: DEG }),
      toggle('flip_h', false, { label: 'flip horizontal' }),
      toggle('flip_v', false, { label: 'flip vertical' }),
    ],
  },
  'image.adjust': {
    cat: 'image', title: 'Adjust', keywords: 'brightness contrast color blur sharpen grayscale',
    desc: 'Brightness, contrast, saturation, hue, gamma, blur and sharpen.',
    inputs: IMG, outputs: IMG,
    widgets: [
      slider('brightness', 0, -100, 100),
      slider('contrast', 0, -100, 100),
      slider('saturation', 0, -100, 100),
      slider('hue', 0, -180, 180, { unit: '°' }),
      slider('gamma', 1, 0.2, 3, { step: 0.05 }),
      slider('blur', 0, 0, 20, { step: 0.5 }),
      slider('sharpen', 0, 0, 10, { step: 0.5 }),
      toggle('grayscale', false),
      toggle('invert', false),
    ],
  },
  'image.convert': {
    cat: 'image', title: 'Convert Image', width: 290, keywords: 'webp avif jpeg png compress optimize format',
    desc: 'WebP, AVIF, JPEG, PNG and more, with quality, target size and PNG optimization.',
    inputs: IMG, outputs: IMG,
    widgets: [
      combo('format', 'webp', ['webp', 'jpeg', 'png', 'avif', 'gif', 'bmp', 'tiff', 'ico', 'tga', 'qoi', 'keep']),
      slider('quality', 82, 1, 100, unless('format', 'png', 'gif', 'bmp', 'tiff', 'ico', 'tga', 'qoi')),
      toggle('lossless', false, when('format', 'webp', 'avif')),
      num('target_kb', 0, { min: 0, unit: 'KB', label: 'target size', ...when('format', 'webp', 'jpeg', 'avif') }),
      combo('png_optimize', 'balanced', ['off', 'fast', 'balanced', 'max'], { label: 'png optimize', ...when('format', 'png', 'keep') }),
      num('png_colors', 0, { min: 0, max: 256, label: 'png colors (0 = all)', ...when('format', 'png', 'keep') }),
      slider('avif_speed', 6, 1, 10, { label: 'avif speed', ...when('format', 'avif') }),
      color('background', '#ffffff', when('format', 'jpeg', 'keep')),
    ],
  },
  'image.watermark': {
    cat: 'image', title: 'Watermark', keywords: 'logo stamp overlay',
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
  'image.sheet': {
    cat: 'image', title: 'Contact Sheet', keywords: 'grid collage overview',
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
  'image.icons': {
    cat: 'image', title: 'App Icons', keywords: 'ico favicon icon sizes',
    desc: 'Makes an .ico file and PNG icons in every size from one picture.',
    inputs: IMG, outputs: [io('icons', 'IMAGE')],
    widgets: [
      text('sizes', '16,24,32,48,64,128,256'),
      toggle('ico', true, { label: '.ico file' }),
      toggle('png', true, { label: 'png files' }),
    ],
  },
  'image.pdf': {
    cat: 'image', title: 'Images to PDF', keywords: 'pdf scan document pages',
    desc: 'Combines images into a PDF document, one image per page.',
    inputs: IMG, outputs: [io('pdf', 'DOC')],
    widgets: [
      text('name', 'images'),
      combo('page', 'A4', ['A4', 'Letter', 'A3', 'A5', 'image'], { labels: { image: 'fit image' } }),
      combo('orientation', 'auto', ['auto', 'portrait', 'landscape'], unless('page', 'image')),
      num('margin_mm', 10, { min: 0, max: 100, unit: 'mm', label: 'margin' }),
      slider('quality', 88, 1, 100),
      toggle('per_image', false, { label: 'one pdf per image' }),
    ],
  },

  // video
  'video.convert': {
    cat: 'video', title: 'Convert Video', width: 300, needs: 'ffmpeg', keywords: 'mp4 h264 h265 hevc av1 vp9 compress encode',
    desc: 'H.264, H.265, VP9, AV1 or ProRes, with quality, size limit and GPU encoding.',
    inputs: VID, outputs: VID,
    widgets: [
      combo('container', 'mp4', ['mp4', 'mkv', 'webm', 'mov', 'keep']),
      combo('video_codec', 'h264', ['h264', 'h265', 'vp9', 'av1', 'prores', 'copy'], { label: 'video codec', labels: VCODEC }),
      slider('quality', 23, 0, 51, { label: 'quality (CRF)', ...unless('video_codec', 'copy', 'prores') }),
      combo('preset', 'medium', PRESETS, unless('video_codec', 'copy', 'prores')),
      num('target_mb', 0, { min: 0, unit: 'MB', label: 'target size', ...unless('video_codec', 'copy', 'prores') }),
      num('width', 0, { min: 0, max: 7680, unit: 'px', label: 'width (0 = keep)', ...unless('video_codec', 'copy') }),
      num('height', 0, { min: 0, max: 4320, unit: 'px', label: 'height (0 = keep)', ...unless('video_codec', 'copy') }),
      num('fps', 0, { min: 0, max: 240, label: 'fps (0 = keep)', ...unless('video_codec', 'copy') }),
      combo('audio_codec', 'aac', ['aac', 'opus', 'mp3', 'flac', 'copy', 'none'], { label: 'audio codec', labels: ACODEC }),
      num('audio_bitrate', 160, { min: 32, max: 512, unit: 'kbps', label: 'audio bitrate', ...unless('audio_codec', 'copy', 'none', 'flac') }),
      combo('hardware', 'none', ['none', 'nvidia', 'intel', 'amd'], { labels: HW, ...when('video_codec', 'h264', 'h265', 'av1') }),
    ],
  },
  'video.trim': {
    cat: 'video', title: 'Trim Video', needs: 'ffmpeg', keywords: 'cut clip range',
    desc: 'Cuts a time range out of a video.',
    inputs: VID, outputs: VID,
    widgets: [
      text('start', '0:00', { placeholder: '0:00' }),
      text('end', '', { placeholder: 'empty = to the end' }),
      toggle('precise', false, { label: 'frame accurate (re-encode)' }),
    ],
  },
  'video.transform': {
    cat: 'video', title: 'Transform Video', width: 290, needs: 'ffmpeg', keywords: 'rotate flip speed mute crop',
    desc: 'Rotate, flip, crop, change speed or remove the sound.',
    inputs: VID, outputs: VID,
    widgets: [
      combo('rotate', '0', ['0', '90', '180', '270'], { labels: DEG }),
      toggle('flip_h', false, { label: 'flip horizontal' }),
      toggle('flip_v', false, { label: 'flip vertical' }),
      slider('speed', 1, 0.25, 4, { step: 0.05, unit: '×' }),
      toggle('mute', false, { label: 'remove sound' }),
      text('crop', '', { placeholder: 'w:h:x:y' }),
      slider('quality', 23, 0, 51, { label: 'quality (CRF)' }),
    ],
  },
  'video.gif': {
    cat: 'video', title: 'Video to GIF', needs: 'ffmpeg', keywords: 'gif animation webp loop',
    desc: 'High quality animated GIF or WebP with a tuned palette.',
    inputs: VID, outputs: IMG,
    widgets: [
      combo('format', 'gif', ['gif', 'webp']),
      num('fps', 12, { min: 1, max: 60 }),
      num('width', 480, { min: 16, max: 3840, unit: 'px' }),
      text('start', '0', { placeholder: 'seconds or 0:05' }),
      text('duration', '', { placeholder: 'empty = whole clip' }),
      num('colors', 256, { min: 2, max: 256, ...when('format', 'gif') }),
      combo('dither', 'sierra2_4a', ['sierra2_4a', 'floyd_steinberg', 'bayer', 'none'], when('format', 'gif')),
      toggle('loop', true),
    ],
  },
  'video.frames': {
    cat: 'video', title: 'Extract Frames', needs: 'ffmpeg', keywords: 'frames screenshots thumbnails stills',
    desc: 'Saves frames as images: every N seconds, N per second or a single frame.',
    inputs: VID, outputs: [io('frames', 'IMAGE')],
    widgets: [
      combo('mode', 'fps', ['fps', 'interval', 'count', 'single'], { labels: { fps: 'frames per second', interval: 'every N seconds', count: 'N frames total', single: 'single frame' } }),
      num('value', 1, { min: 0.01, step: 0.5, ...unless('mode', 'single') }),
      text('time', '0', { placeholder: 'seconds or 0:05', ...when('mode', 'single') }),
      combo('format', 'png', ['png', 'jpg', 'webp']),
      num('width', 0, { min: 0, unit: 'px', label: 'width (0 = keep)' }),
    ],
  },
  'video.audio': {
    cat: 'video', title: 'Extract Audio', needs: 'ffmpeg', keywords: 'soundtrack mp3 rip',
    desc: 'Pulls the soundtrack out of videos.',
    inputs: VID, outputs: AUD,
    widgets: [
      combo('format', 'mp3', ['mp3', 'm4a', 'opus', 'flac', 'wav', 'copy']),
      num('bitrate', 192, { min: 32, max: 512, unit: 'kbps', ...unless('format', 'flac', 'wav', 'copy') }),
    ],
  },
  'video.concat': {
    cat: 'video', title: 'Join Videos', needs: 'ffmpeg', keywords: 'concat merge join combine',
    desc: 'Concatenates clips into one video in input order.',
    inputs: VID, outputs: [io('video', 'VIDEO')],
    widgets: [text('name', 'joined'), toggle('reencode', false, { label: 're-encode (mixed formats)' })],
  },

  // audio
  'audio.convert': {
    cat: 'audio', title: 'Convert Audio', needs: 'ffmpeg', keywords: 'mp3 aac opus flac wav music',
    desc: 'MP3, AAC, Opus, OGG, FLAC, WAV or ALAC, with bitrate, sample rate and channels.',
    inputs: AUD, outputs: AUD,
    widgets: [
      combo('format', 'mp3', ['mp3', 'm4a', 'opus', 'ogg', 'flac', 'wav', 'alac']),
      num('bitrate', 192, { min: 32, max: 512, unit: 'kbps', ...unless('format', 'flac', 'wav', 'alac') }),
      combo('sample_rate', '0', ['0', '22050', '32000', '44100', '48000', '96000'], { labels: { 0: 'keep', 22050: '22.05 kHz', 32000: '32 kHz', 44100: '44.1 kHz', 48000: '48 kHz', 96000: '96 kHz' } }),
      combo('channels', '0', ['0', '1', '2'], { labels: { 0: 'keep', 1: 'mono', 2: 'stereo' } }),
    ],
  },
  'audio.process': {
    cat: 'audio', title: 'Process Audio', width: 290, needs: 'ffmpeg', keywords: 'volume normalize loudness fade trim speed',
    desc: 'Volume, loudness normalization, trimming, fades and speed.',
    inputs: AUD, outputs: AUD,
    widgets: [
      slider('volume_db', 0, -30, 30, { step: 0.5, unit: 'dB', label: 'volume' }),
      toggle('normalize', false, { label: 'normalize loudness' }),
      num('lufs', -16, { min: -40, max: -5, unit: 'LUFS', label: 'target', ...when('normalize', true) }),
      num('trim_start', 0, { min: 0, step: 0.5, unit: 's', label: 'cut from start' }),
      num('trim_end', 0, { min: 0, step: 0.5, unit: 's', label: 'cut from end' }),
      num('fade_in', 0, { min: 0, step: 0.5, unit: 's', label: 'fade in' }),
      num('fade_out', 0, { min: 0, step: 0.5, unit: 's', label: 'fade out' }),
      slider('speed', 1, 0.5, 2, { step: 0.05, unit: '×' }),
    ],
  },

  // 3D
  'mesh.convert': {
    cat: 'mesh', title: 'Convert 3D', keywords: 'glb gltf obj stl ply off model',
    desc: 'Converts between GLB, OBJ, STL, PLY and OFF.',
    inputs: MSH, outputs: MSH,
    widgets: [combo('format', 'glb', ['glb', 'obj', 'stl', 'ply', 'off']), toggle('binary', true, when('format', 'stl', 'ply'))],
  },
  'mesh.transform': {
    cat: 'mesh', title: 'Transform 3D', width: 300, keywords: 'scale units rotate center axis',
    desc: 'Scale, change units, rotate, move, center and fix the up axis.',
    inputs: MSH, outputs: MSH,
    widgets: [
      num('scale', 1, { min: 0.0001, step: 0.1 }),
      combo('units', 'none', ['none', 'mm to m', 'm to mm', 'cm to m', 'm to cm', 'inch to mm', 'mm to inch']),
      num('rotate_x', 0, { step: 15, unit: '°', label: 'rotate X' }),
      num('rotate_y', 0, { step: 15, unit: '°', label: 'rotate Y' }),
      num('rotate_z', 0, { step: 15, unit: '°', label: 'rotate Z' }),
      num('translate_x', 0, { step: 0.1, label: 'move X' }),
      num('translate_y', 0, { step: 0.1, label: 'move Y' }),
      num('translate_z', 0, { step: 0.1, label: 'move Z' }),
      combo('center', 'none', ['none', 'center', 'center + ground']),
      num('fit_size', 0, { min: 0, step: 0.1, label: 'fit to size (0 = off)' }),
      combo('up_axis', 'keep', ['keep', 'Z-up to Y-up', 'Y-up to Z-up'], { label: 'up axis' }),
      toggle('flip_normals', false, { label: 'flip normals' }),
    ],
  },
  'mesh.optimize': {
    cat: 'mesh', title: 'Optimize 3D', keywords: 'decimate simplify reduce weld normals',
    desc: 'Weld vertices, reduce the triangle count and rebuild normals.',
    inputs: MSH, outputs: MSH,
    widgets: [
      toggle('weld', true, { label: 'weld vertices' }),
      num('weld_tolerance', 0.0001, { min: 0, step: 0.0001, label: 'weld tolerance', ...when('weld', true) }),
      slider('keep_percent', 100, 1, 100, { unit: '%', label: 'keep triangles' }),
      combo('normals', 'keep', ['keep', 'smooth', 'flat']),
      toggle('remove_degenerate', true, { label: 'drop degenerate' }),
    ],
  },

  // archives
  'archive.create': {
    cat: 'archive', title: 'Create Archive', keywords: 'zip 7z tar pack compress backup',
    desc: 'ZIP, 7z or tar.gz/zst/xz/bz2, one archive or one per file.',
    inputs: FIL, outputs: [io('archive', 'ARCHIVE')],
    widgets: [
      combo('format', 'zip', ['zip', '7z', 'tar', 'tar.gz', 'tar.zst', 'tar.xz', 'tar.bz2']),
      slider('level', 6, 0, 9, unless('format', 'tar')),
      combo('mode', 'single', ['single', 'per-file'], { labels: { single: 'one archive', 'per-file': 'archive per file' } }),
      text('name', 'archive', when('mode', 'single')),
    ],
  },
  'archive.extract': {
    cat: 'archive', title: 'Extract Archive', keywords: 'unzip unpack extract',
    desc: 'Unpacks ZIP, 7z and tar archives.',
    inputs: [io('archives', 'ARCHIVE')], outputs: FIL,
    widgets: [toggle('subfolder', true, { label: 'folder per archive' })],
  },
  'archive.compress': {
    cat: 'archive', title: 'Compress File', keywords: 'zstd gzip xz bzip2',
    desc: 'Compresses each file on its own with zstd, gzip, xz or bzip2.',
    inputs: FIL, outputs: [io('archives', 'ARCHIVE')],
    widgets: [combo('format', 'zst', ['zst', 'gz', 'xz', 'bz2']), slider('level', 6, 1, 9)],
  },

  // data & documents
  'data.convert': {
    cat: 'data', title: 'Convert Data', keywords: 'json yaml toml csv tsv ndjson config',
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
  'text.transform': {
    cat: 'data', title: 'Text Tools', width: 300, keywords: 'encoding utf8 cp1251 line endings replace',
    desc: 'Fix encodings, line endings, whitespace and case, find and replace.',
    inputs: [io('files', 'DATA')], outputs: [io('files', 'DATA')],
    widgets: [
      combo('encoding_in', 'auto', ['auto', 'utf-8', 'utf-16le', 'utf-16be', 'windows-1251', 'koi8-r', 'windows-1252', 'iso-8859-1', 'shift_jis', 'gbk'], { label: 'read as' }),
      combo('encoding_out', 'utf-8', ['keep', 'utf-8', 'utf-8-bom', 'utf-16le', 'utf-16be', 'windows-1251', 'koi8-r', 'windows-1252'], { label: 'write as' }),
      combo('line_endings', 'keep', ['keep', 'lf', 'crlf'], { label: 'line endings', labels: { lf: 'LF (Linux)', crlf: 'CRLF (Windows)' } }),
      toggle('trim_trailing', false, { label: 'trim trailing spaces' }),
      text('find', ''),
      text('replace', '', { label: 'replace with' }),
      toggle('regex', false, { label: 'regular expression' }),
      combo('case', 'keep', ['keep', 'lower', 'upper']),
      num('tabs_to_spaces', 0, { min: 0, max: 8, label: 'tabs to spaces (0 = off)' }),
    ],
  },
  'doc.office': {
    cat: 'data', title: 'Office Convert', needs: 'soffice', keywords: 'docx xlsx pptx odt pdf libreoffice word excel',
    desc: 'DOCX, XLSX, PPTX, ODT and more to PDF or other formats (LibreOffice).',
    inputs: DOC, outputs: DOC,
    widgets: [combo('format', 'pdf', ['pdf', 'docx', 'odt', 'xlsx', 'ods', 'pptx', 'odp', 'html', 'txt', 'png'])],
  },
  'doc.pandoc': {
    cat: 'data', title: 'Pandoc Convert', needs: 'pandoc', keywords: 'markdown md html epub latex rst',
    desc: 'Markdown, HTML, DOCX, EPUB, LaTeX and more (Pandoc).',
    inputs: DOC, outputs: DOC,
    widgets: [combo('format', 'docx', ['docx', 'md', 'html', 'odt', 'epub', 'rst', 'tex', 'txt', 'pdf']), toggle('standalone', true, { label: 'standalone document' })],
  },

  // utility
  'util.note': {
    cat: 'util', title: 'Note', frontendOnly: true, width: 260, keywords: 'comment sticky text',
    desc: 'A sticky note for comments. It does not run.',
    inputs: [], outputs: [],
    widgets: [area('text', '', { label: '', placeholder: 'Write a note…', rows: 5 })],
  },
};
