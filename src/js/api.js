// Bridge to the Rust backend (Tauri). In a plain browser it runs a built-in demo mock,
// which also powers the CI smoke test and the README screenshot.
import { basename, extOf, hashStr, NL } from './ui.js';

const T = typeof window !== 'undefined' ? window.__TAURI__ : undefined;
const params = new URLSearchParams(typeof location !== 'undefined' ? location.search : '');
export const DEMO = !(T && T.core) || params.has('demo');
export const SMOKE = params.has('smoke');
export const DEMO_ROOT = 'C:/Users/demo/Desktop/Morph demo';

const TOOL_KEYS = ['ffmpeg', 'ffprobe', 'soffice', 'pandoc', 'gs', 'sevenzip'];
let tools = Object.fromEntries(TOOL_KEYS.map((k) => [k, '']));
export function setToolSettings(s) {
  tools = Object.fromEntries(TOOL_KEYS.map((k) => [k, (s && s[k]) || '']));
}
export const toolSettings = () => ({ ...tools });

export async function invoke(cmd, args = {}) {
  if (DEMO) return mock(cmd, args);
  return T.core.invoke(cmd, args);
}

const bus = new Map();
function emit(event, payload) {
  const set = bus.get(event);
  if (!set) return;
  for (const cb of set) {
    try {
      cb(payload);
    } catch (e) {
      console.error(e);
      if (window.__err) window.__err(String((e && e.stack) || e));
    }
  }
}

export async function listen(event, cb) {
  if (DEMO) {
    if (!bus.has(event)) bus.set(event, new Set());
    bus.get(event).add(cb);
    return () => bus.get(event).delete(cb);
  }
  return T.event.listen(event, (e) => cb(e.payload));
}

export async function pickFiles({ folder = false, multiple = true, filters } = {}) {
  try {
    if (DEMO) return folder ? [DEMO_ROOT] : DEMO_FILES.slice(0, multiple ? 3 : 1).map((f) => f.path);
    const options = { directory: folder, multiple };
    if (filters && !folder) options.filters = filters;
    const r = T.dialog && T.dialog.open ? await T.dialog.open(options) : await T.core.invoke('plugin:dialog|open', { options });
    if (!r) return [];
    return (Array.isArray(r) ? r : [r]).map((x) => (typeof x === 'string' ? x : x && x.path)).filter(Boolean);
  } catch (e) {
    console.error(e);
    return [];
  }
}

export async function saveDialog({ defaultPath, filters } = {}) {
  try {
    if (DEMO) return null;
    const options = { defaultPath, filters };
    const r = T.dialog && T.dialog.save ? await T.dialog.save(options) : await T.core.invoke('plugin:dialog|save', { options });
    return r || null;
  } catch (e) {
    console.error(e);
    return null;
  }
}

export function fileSrc(path) {
  if (DEMO) return demoThumb(path);
  return T.core.convertFileSrc(path);
}

export function openExternal(url) {
  if (DEMO) {
    window.open(url, '_blank', 'noopener');
    return;
  }
  T.core.invoke('open_path', { path: url }).catch(() => window.open(url, '_blank'));
}

export function downloadText(name, text) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
  a.download = name;
  document.body.append(a);
  a.click();
  setTimeout(() => {
    URL.revokeObjectURL(a.href);
    a.remove();
  }, 1000);
}

export function pickLocalText(accept) {
  return new Promise((resolve) => {
    const inp = document.createElement('input');
    inp.type = 'file';
    inp.accept = accept || '';
    inp.onchange = () => {
      const f = inp.files && inp.files[0];
      if (!f) return resolve(null);
      const r = new FileReader();
      r.onload = () => resolve(String(r.result || ''));
      r.onerror = () => resolve(null);
      r.readAsText(f);
    };
    inp.click();
  });
}

// OS file drops (Tauri intercepts them, HTML5 drop events do not fire).
export function onDrop(h) {
  if (DEMO) return;
  const conv = (p) => (p ? { x: p.x / (window.devicePixelRatio || 1), y: p.y / (window.devicePixelRatio || 1) } : { x: 0, y: 0 });
  const handle = (p) => {
    if (!p) return;
    if (p.type === 'enter') h.enter && h.enter(conv(p.position));
    else if (p.type === 'over') h.over && h.over(conv(p.position));
    else if (p.type === 'drop') h.drop && h.drop(p.paths || [], conv(p.position));
    else h.leave && h.leave();
  };
  try {
    const wv = T.webview && T.webview.getCurrentWebview ? T.webview.getCurrentWebview() : null;
    if (wv && wv.onDragDropEvent) {
      wv.onDragDropEvent((e) => handle(e.payload)).catch((e) => console.error(e));
      return;
    }
  } catch (e) {
    console.error(e);
  }
  const on = (name, type) => T.event.listen(name, (e) => handle({ ...(e.payload || {}), type })).catch(() => {});
  on('tauri://drag-enter', 'enter');
  on('tauri://drag-over', 'over');
  on('tauri://drag-drop', 'drop');
  on('tauri://drag-leave', 'leave');
}

// Thumbnails: cached, at most 4 backend requests in flight.
const WEB_IMG = new Set(['jpg', 'jpeg', 'png', 'webp', 'gif', 'svg', 'bmp', 'avif', 'ico']);
const thumbs = new Map();
const waiting = [];
let active = 0;
function pump() {
  while (active < 4 && waiting.length) {
    const job = waiting.shift();
    active++;
    thumbFor(job.file, job.size)
      .then(job.resolve, () => job.resolve(null))
      .finally(() => {
        active--;
        pump();
      });
  }
}
async function thumbFor(f, size) {
  if (!['image', 'video', 'audio'].includes(f.kind)) return null;
  if (DEMO) return demoThumb(f.path);
  if (f.kind === 'image' && WEB_IMG.has((f.ext || '').toLowerCase()) && (f.size || 0) < 8e6) return fileSrc(f.path);
  const r = await invoke('thumbnail', { path: f.path, size, settings: toolSettings() });
  return (r && r.thumb) || null;
}
export function loadThumb(file, size = 256) {
  const key = file.path + '|' + size;
  if (thumbs.has(key)) return thumbs.get(key);
  const p = new Promise((resolve) => {
    waiting.push({ file, size, resolve });
    pump();
  });
  thumbs.set(key, p);
  if (thumbs.size > 3000) thumbs.delete(thumbs.keys().next().value);
  return p;
}

// ---------------------------------------------------------------- demo mock

const KIND = {
  jpg: 'image', jpeg: 'image', png: 'image', svg: 'image', webp: 'image', avif: 'image', gif: 'image', bmp: 'image', tif: 'image', tiff: 'image', ico: 'image', tga: 'image', qoi: 'image',
  mp4: 'video', mov: 'video', mkv: 'video', webm: 'video', avi: 'video',
  m4a: 'audio', mp3: 'audio', opus: 'audio', ogg: 'audio', flac: 'audio', wav: 'audio', alac: 'audio',
  glb: 'mesh', gltf: 'mesh', obj: 'mesh', stl: 'mesh', ply: 'mesh', off: 'mesh',
  zip: 'archive', '7z': 'archive', tar: 'archive', gz: 'archive', zst: 'archive', xz: 'archive', bz2: 'archive',
  json: 'data', yaml: 'data', yml: 'data', toml: 'data', csv: 'data', tsv: 'data', ndjson: 'data', xml: 'data',
  xls: 'doc', xlsm: 'doc', doc: 'doc', rtf: 'doc', ppt: 'doc', md: 'doc', aiff: 'audio', wma: 'audio', aac: 'audio', ac3: 'audio', m4r: 'audio',
  m4v: 'video', flv: 'video', wmv: 'video', mpg: 'video', ts: 'video', m2ts: 'video', '3gp': 'video', ogv: 'video', mxf: 'video',
  heic: 'image', jxl: 'image', psd: 'image', ppm: 'image', exr: 'image', rar: 'archive', iso: 'archive', srt: 'text', sql: 'text', css: 'text', b64: 'text',
  docx: 'doc', odt: 'doc', pdf: 'doc', xlsx: 'doc', ods: 'doc', pptx: 'doc', odp: 'doc', html: 'doc', epub: 'doc',
  md: 'text', txt: 'text', rst: 'text', tex: 'text',
};
const kindOf = (ext) => KIND[ext] || 'other';
const stemOf = (name) => {
  const i = name.lastIndexOf('.');
  return i > 0 ? name.slice(0, i) : name;
};

const DEMO_LIST = [
  ['Photos', 'beach.jpg', 4812000], ['Photos', 'mountains.png', 9634000], ['Photos', 'city-night.jpg', 3921000],
  ['Photos', 'forest.jpg', 5230000], ['Photos', 'sunset.jpg', 2875000], ['Photos', 'logo.svg', 14200],
  ['Clips', 'trip.mp4', 184300000], ['Clips', 'drone.mov', 412700000], ['Audio', 'voice-memo.m4a', 5320000],
  ['Models', 'helmet.glb', 3720000], ['Docs', 'report.docx', 842000], ['Docs', 'budget.xlsx', 48200], ['Docs', 'manual.pdf', 2380000],
  ['Data', 'config.json', 4200],
];
const DEMO_FILES = DEMO_LIST.map(([dir, name, size]) => {
  const path = DEMO_ROOT + '/' + dir + '/' + name;
  const ext = extOf(name);
  return { path, name, ext, rel: 'Morph demo/' + dir, kind: kindOf(ext), size, origin: path, origin_size: size };
});
const sizes = new Map(DEMO_FILES.map((f) => [f.path, f.size]));
const WORK = 'C:/Users/demo/AppData/Local/Temp/morph/runs/demo';
const OUT = 'C:/Users/demo/Morph Output';

const demoThumbs = new Map();
export function demoThumb(path) {
  if (demoThumbs.has(path)) return demoThumbs.get(path);
  const name = basename(path);
  const kind = kindOf(extOf(name));
  let stem = stemOf(name);
  const us = stem.lastIndexOf('_');
  if (us > 0 && /^[0-9]+$/.test(stem.slice(us + 1))) stem = stem.slice(0, us);
  const h = hashStr(stem);
  const hue = h % 360;
  const hue2 = (hue + 40 + ((h >> 8) % 80)) % 360;
  let body;
  if (kind === 'audio') {
    body = `<rect width='300' height='200' fill='#1d2620'/>`;
    for (let i = 0; i < 58; i++) {
      const a = Math.abs(Math.sin(i * 0.37 + (h % 7))) * (0.35 + 0.65 * Math.abs(Math.sin(i * 0.11 + 1)));
      const hh = 8 + a * 120;
      body += `<rect x='${6 + i * 5}' y='${(100 - hh / 2).toFixed(1)}' width='3' height='${hh.toFixed(1)}' rx='1.5' fill='#81C784'/>`;
    }
  } else {
    const sx = 40 + (h % 200);
    const sy = 45 + ((h >> 4) % 45);
    const p1 = [0, 200, 0, 140, 60, 90 + (h % 30), 120, 128, 190, 70 + ((h >> 3) % 40), 300, 132, 300, 200].join(' ');
    const p2 = [0, 200, 0, 172, 80, 136, 150, 164, 230, 120, 300, 158, 300, 200].join(' ');
    body = `<defs><linearGradient id='s' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='hsl(${hue},62%,56%)'/><stop offset='1' stop-color='hsl(${hue2},70%,78%)'/></linearGradient></defs>`
      + `<rect width='300' height='200' fill='url(#s)'/><circle cx='${sx}' cy='${sy}' r='22' fill='hsl(${(hue + 180) % 360},90%,86%)' opacity='.9'/>`
      + `<polygon points='${p1}' fill='hsl(${hue2},32%,30%)'/><polygon points='${p2}' fill='hsl(${hue},30%,18%)'/>`;
    if (kind === 'video') body += `<circle cx='150' cy='100' r='26' fill='rgba(0,0,0,.45)'/><path d='M141 86v28l24-14z' fill='#fff'/>`;
  }
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 300 200' width='600' height='400'>${body}</svg>`;
  const url = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
  demoThumbs.set(path, url);
  return url;
}

function demoProbe(path) {
  const name = basename(path);
  const ext = extOf(name);
  const kind = kindOf(ext);
  const v = { path, name, ext, kind, size: sizes.get(path) || 1048576, modified: '2026-11-20 14:32' };
  if (kind === 'image') Object.assign(v, { width: 4032, height: 3024 });
  if (kind === 'video') {
    v.media = {
      format: { format_long_name: 'QuickTime / MOV', duration: '42.500000', bit_rate: '34690000' },
      streams: [{ codec_type: 'video', codec_name: 'h264', width: 3840, height: 2160, r_frame_rate: '30/1' }, { codec_type: 'audio', codec_name: 'aac', sample_rate: '48000', channels: 2 }],
    };
  }
  if (kind === 'audio') {
    v.media = { format: { format_long_name: 'QuickTime / MOV', duration: '184.200000', bit_rate: '256000' }, streams: [{ codec_type: 'audio', codec_name: 'aac', sample_rate: '44100', channels: 2 }] };
  }
  if (kind === 'mesh') v.mesh = { vertices: 14336, triangles: 28672, materials: 1 };
  return v;
}

function hexOf(s) {
  let out = '';
  let h = hashStr(s);
  while (out.length < 64) {
    h = hashStr(out + h);
    out += h.toString(16).padStart(8, '0');
  }
  return out.slice(0, 64);
}

function demoText(path) {
  const ext = extOf(path);
  let text;
  if (ext === 'json') text = JSON.stringify({ name: 'morph-demo', version: 3, features: ['images', 'video', 'audio', '3d'], output: { folder: 'Morph Output', overwrite: false } });
  else if (ext === 'yaml' || ext === 'yml') text = ['name: morph-demo', 'version: 3', 'features:', '  - images', '  - video', '  - audio', '  - 3d', 'output:', '  folder: Morph Output', '  overwrite: false'].join(NL);
  else if (ext === 'toml') text = ['name = ' + JSON.stringify('morph-demo'), 'version = 3', '', '[output]', 'folder = ' + JSON.stringify('Morph Output'), 'overwrite = false'].join(NL);
  else if (ext === 'csv') text = ['name,folder,kind,size', ...DEMO_FILES.map((f) => [f.name, f.rel, f.kind, f.size].join(','))].join(NL);
  else if (['sha256', 'sha1', 'sha512', 'md5', 'blake3', 'crc32'].includes(ext)) text = DEMO_FILES.map((f) => hexOf(f.path) + '  ' + f.rel + '/' + f.name).join(NL);
  else text = 'Morph demo file' + NL;
  return { text, truncated: false, encoding: 'UTF-8', size: text.length };
}

function torusKnot() {
  const P = 2;
  const Q = 3;
  const seg = 240;
  const rad = 16;
  const tube = 0.32;
  const pos = [];
  const nor = [];
  const idx = [];
  const curve = (u) => {
    const cs = Math.cos((Q / P) * u);
    return [(2 + cs) * 0.5 * Math.cos(u), (2 + cs) * 0.5 * Math.sin(u), Math.sin((Q / P) * u) * 0.5];
  };
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const nrm = (a) => {
    const l = Math.hypot(a[0], a[1], a[2]) || 1;
    return [a[0] / l, a[1] / l, a[2] / l];
  };
  for (let i = 0; i <= seg; i++) {
    const u = (i / seg) * P * Math.PI * 2;
    const p1 = curve(u);
    const p2 = curve(u + 0.01);
    const tt = sub(p2, p1);
    const nn = [p2[0] + p1[0], p2[1] + p1[1], p2[2] + p1[2]];
    const b = nrm(cross(tt, nn));
    const n = nrm(cross(b, tt));
    for (let j = 0; j <= rad; j++) {
      const v = (j / rad) * Math.PI * 2;
      const cx = -tube * Math.cos(v);
      const cy = tube * Math.sin(v);
      const d = [cx * n[0] + cy * b[0], cx * n[1] + cy * b[1], cx * n[2] + cy * b[2]];
      pos.push(p1[0] + d[0], p1[1] + d[1], p1[2] + d[2]);
      nor.push(...nrm(d));
    }
  }
  for (let i = 0; i < seg; i++) {
    for (let j = 0; j < rad; j++) {
      const a = (rad + 1) * i + j;
      const b = (rad + 1) * (i + 1) + j;
      idx.push(a, b, a + 1, b, b + 1, a + 1);
    }
  }
  return { positions: pos, normals: nor, colors: [], indices: idx, stats: { vertices: pos.length / 3, triangles: idx.length / 3 }, simplified: false };
}

function derive(f, node, name, kind, size) {
  const ext = extOf(name);
  const path = WORK + '/' + node + '/' + (f.rel ? f.rel + '/' : '') + name;
  const it = { path, name, ext, rel: f.rel, kind: kind || kindOf(ext), size: Math.max(1, Math.round(size)), origin: f.origin, origin_size: f.origin_size };
  sizes.set(path, it.size);
  return it;
}

function single(node, name, kind, size) {
  const path = WORK + '/' + node + '/' + name;
  const s = Math.max(1, Math.round(size));
  sizes.set(path, s);
  return { path, name, ext: extOf(name), rel: '', kind, size: s, origin: path, origin_size: s };
}

const IMG_FACTOR = { webp: 0.3, avif: 0.22, jpeg: 0.55, png: 0.95, gif: 0.6, bmp: 3, tiff: 2.4, ico: 0.04, tga: 2.8, qoi: 1.4, keep: 0.8 };
// type: [kinds handled, new extension, size factor, output kind]
const MAP = {
  'image.resize': ['image', null, () => 0.42],
  'image.crop': ['image', null, () => 0.8],
  'image.transform': ['image', null, () => 1],
  'image.adjust': ['image', null, () => 1.02],
  'image.watermark': ['image', null, () => 1.03],
  'image.convert': ['image', (v, f) => (v.format === 'keep' ? f.ext : v.format === 'jpeg' ? 'jpg' : v.format), (v) => (IMG_FACTOR[v.format] || 0.8) * (0.6 + (Number(v.quality) || 80) / 250)],
  'video.convert': ['video', (v, f) => (v.container === 'keep' ? f.ext : v.container), (v) => (v.video_codec === 'copy' ? 1 : 0.3)],
  'video.trim': ['video', null, () => 0.4],
  'video.transform': ['video', null, () => 0.9],
  'video.gif': ['video', (v) => v.format || 'gif', () => 0.03, 'image'],
  'video.audio': ['video', (v) => (v.format === 'copy' ? 'm4a' : v.format), () => 0.02, 'audio'],
  'audio.convert': ['audio', (v) => v.format, () => 0.7],
  'audio.process': ['audio', null, () => 1],
  'mesh.convert': ['mesh', (v) => v.format, () => 0.85],
  'mesh.transform': ['mesh', null, () => 1],
  'mesh.optimize': ['mesh', null, (v) => Math.max(0.05, (Number(v.keep_percent) || 100) / 100)],
  'archive.compress': ['', (v, f) => f.ext + '.' + (v.format || 'zst'), () => 0.35, 'archive'],
  'data.convert': ['data', (v) => v.format, () => 1.15],
  'text.transform': ['text,data', null, () => 1],
  'doc.office': ['doc', (v) => v.format, () => 0.7],
  'doc.pandoc': ['doc,text', (v) => v.format, () => 0.9],
  'files.rename': ['', null, () => 1],
  'convert.any': ['', (v, f) => v.to || f.ext, (v, f) => (f.kind === 'video' ? 0.35 : f.kind === 'audio' ? 0.6 : 0.8)],
  'video.compress': ['video', () => 'mp4', (v, f) => Math.min(1, 9.5e6 / Math.max(1, f.size))],
  'video.reframe': ['video', null, () => 0.6],
  'video.filters': ['video', null, () => 0.95],
  'video.fx': ['video', null, () => 1],
  'video.framerate': ['video', null, () => 1.3],
  'video.watermark': ['video', null, () => 1.02],
  'video.subtitles': ['video', null, () => 1],
  'audio.fx': ['audio', null, () => 1],
  'audio.silence': ['audio', null, () => 0.85],
  'audio.tags': ['audio', null, () => 1],
  'image.effects': ['image', null, () => 0.9],
  'image.text': ['image', null, () => 1.01],
  'image.frame': ['image', null, () => 1.05],
  'image.remove_bg': ['image', () => 'png', () => 1.3],
  'files.clean_names': ['', null, () => 1],
  'files.organize': ['', null, () => 1],
  'files.sort': ['', null, () => 1],
  'sheet.convert': ['doc,data', (v) => v.format || 'xlsx', () => 1.1],
  'sheet.tools': ['doc,data', null, () => 0.8],
  'doc.markdown': ['doc,text', () => 'html', () => 1.6],
  'doc.text': ['doc', () => 'txt', () => 0.1],
  'pdf.compress': ['doc', null, () => 0.45],
  'pdf.pages': ['doc', null, () => 0.7],
  'text.lines': ['text,data', null, () => 0.9],
  'data.json': ['data', null, () => 0.9],
  'data.encode': ['', null, () => 1.34],
};

function mockOp(n, inputs) {
  const v = n.values || {};
  const all = inputs.flat();
  const uniq = (list) => {
    const s = new Set();
    return list.filter((f) => !s.has(f.path) && s.add(f.path));
  };
  const sum = (list) => list.reduce((s, f) => s + (f.size || 0), 0);
  switch (n.type) {
    case 'files.load':
      return [DEMO_FILES.map((f) => ({ ...f }))];
    case 'files.preview':
    case 'files.merge':
      return [uniq(all)];
    case 'files.save':
      return [all.map((f) => {
        const dir = v.destination === 'next to original'
          ? f.origin.slice(0, f.origin.lastIndexOf('/'))
          : (v.folder || OUT) + (v.keep_structure !== false && f.rel ? '/' + f.rel : '');
        const path = dir + '/' + f.name;
        sizes.set(path, f.size);
        return { ...f, path };
      })];
    case 'files.filter': {
      const k = v.kind || 'any';
      const m = all.filter((f) => k === 'any' || f.kind === k);
      return [m, all.filter((f) => !m.includes(f))];
    }
    case 'files.split': {
      const ks = ['image', 'video', 'audio', 'mesh'];
      return [...ks.map((k) => all.filter((f) => f.kind === k)), all.filter((f) => !ks.includes(f.kind))];
    }
    case 'files.hash': {
      const ext = v.format === 'csv' || v.format === 'json' ? v.format : v.algorithm || 'sha256';
      return [all, [single(n.id, 'checksums.' + ext, ext === 'csv' || ext === 'json' ? 'data' : 'other', 90 * all.length)]];
    }
    case 'files.info':
      return [all, [single(n.id, 'file-info.' + (v.format || 'csv'), 'data', 160 * all.length)]];
    case 'files.dedupe':
      return [uniq(all), []];
    case 'image.sheet':
      return [[single(n.id, (v.name || 'contact-sheet') + '.png', 'image', 380000)]];
    case 'image.pdf': {
      const imgs = all.filter((f) => f.kind === 'image');
      if (!imgs.length) throw new Error('No images to put into the PDF');
      return [[single(n.id, (v.name || 'images') + '.pdf', 'doc', sum(imgs) * 0.35)]];
    }
    case 'image.icons': {
      const list = String(v.sizes || '16,32,48,256').split(',').map((s) => s.trim()).filter(Boolean);
      return [all.flatMap((f) => (f.kind !== 'image' ? [f] : [
        ...list.map((s) => derive(f, n.id, stemOf(f.name) + '_' + s + '.png', 'image', Number(s) * Number(s) * 0.9)),
        derive(f, n.id, stemOf(f.name) + '.ico', 'image', 120000),
      ]))];
    }
    case 'video.frames':
      return [all.flatMap((f) => (f.kind !== 'video' ? [f] : [1, 2, 3, 4, 5, 6].map((i) => derive(f, n.id, stemOf(f.name) + '_' + String(i).padStart(4, '0') + '.' + (v.format || 'png'), 'image', 900000))))];
    case 'video.concat':
      return [[single(n.id, (v.name || 'joined') + '.mp4', 'video', sum(all.filter((f) => f.kind === 'video')))]];
    case 'archive.create': {
      const fmt = v.format || 'zip';
      if (v.mode === 'per-file') return [all.map((f) => derive(f, n.id, stemOf(f.name) + '.' + fmt, 'archive', f.size * 0.7))];
      return [[single(n.id, (v.name || 'archive') + '.' + fmt, 'archive', sum(all) * 0.72)]];
    }
    case 'pdf.merge': {
      const pdfs = all.filter((f) => f.ext === 'pdf');
      return [[single(n.id, (v.name || 'merged') + '.pdf', 'doc', sum(pdfs) || 1200000)]];
    }
    case 'pdf.render':
      return [all.flatMap((f) => (f.ext !== 'pdf' ? [] : [1, 2, 3, 4].map((i) => derive(f, n.id, stemOf(f.name) + '_' + String(i).padStart(3, '0') + '.' + (v.format || 'png'), 'image', 380000))))];
    case 'sheet.merge':
      return [[single(n.id, (v.name || 'merged') + '.' + (v.format || 'xlsx'), 'doc', sum(all) * 0.9)]];
    case 'image.collage':
      return [[single(n.id, (v.name || 'collage') + '.' + (v.format || 'jpg'), 'image', 900000)]];
    case 'video.thumbs':
      return [all.filter((f) => f.kind === 'video').map((f) => derive(f, n.id, stemOf(f.name) + '_thumbs.' + (v.format || 'jpg'), 'image', 520000))];
    case 'video.slideshow':
      return [[single(n.id, (v.name || 'slideshow') + '.' + (v.format || 'mp4'), 'video', 24000000)]];
    case 'media.info':
      return [all, [single(n.id, 'media-report.' + (v.format || 'csv'), 'data', 2200)]];
    case 'archive.extract':
      return [all.flatMap((f) => (f.kind === 'archive' ? DEMO_FILES.slice(0, 4).map((d) => derive(f, n.id, d.name, d.kind, d.size)) : [f]))];
    default: {
      const m = MAP[n.type];
      if (!m) return Array.from({ length: Math.max(1, n.outputs || 1) }, () => all);
      const [kinds, extFn, factor, outKind] = m;
      const ks = kinds ? kinds.split(',') : null;
      return [all.map((f) => {
        if (ks && !ks.includes(f.kind)) return f;
        const ext = (extFn && extFn(v, f)) || f.ext;
        return derive(f, n.id, stemOf(f.name) + '.' + ext, outKind || null, f.size * factor(v, f));
      })];
    }
  }
}

let cancelFlag = false;
function mockRun(graph) {
  const nodes = (graph && graph.nodes) || [];
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const order = [];
  const state = new Map();
  const visit = (id) => {
    const st = state.get(id);
    if (st === 2) return;
    if (st === 1) throw new Error('The graph contains a loop');
    const n = byId.get(id);
    if (!n) return;
    state.set(id, 1);
    for (const r of n.inputs || []) if (r) visit(r.node);
    state.set(id, 2);
    order.push(id);
  };
  const targets = nodes.filter((n) => n.is_output);
  if (!targets.length) throw new Error('Nothing to run: add a Save or Preview node and connect it.');
  targets.forEach((n) => visit(n.id));
  cancelFlag = false;
  const runId = 'demo-' + Date.now();
  const t0 = Date.now();
  const results = new Map();
  const ev = (p) => setTimeout(() => emit('run-event', p), 0);
  const finish = (ok, error) => ev({ type: 'finished', run: runId, ok, error: error || null, cancelled: cancelFlag, ms: Date.now() - t0 });
  let i = 0;
  const next = () => {
    if (cancelFlag) return finish(false, 'Cancelled');
    if (i >= order.length) return finish(true);
    const n = byId.get(order[i++]);
    const inputs = (n.inputs || []).map((r) => (r ? (results.get(r.node) || [])[r.slot] || [] : []));
    const flat = inputs.flat();
    ev({ type: 'node_start', node: n.id });
    const dur = 260 + (hashStr(n.type) % 420);
    let k = 0;
    const tick = () => {
      if (cancelFlag) return finish(false, 'Cancelled');
      k++;
      if (k < 4) {
        const f = flat[Math.min(flat.length - 1, Math.floor((k / 4) * flat.length))];
        ev({ type: 'node_progress', node: n.id, value: k / 4, text: f ? f.name : '' });
        setTimeout(tick, dur / 4);
        return;
      }
      let outs;
      try {
        outs = n.bypass ? Array.from({ length: Math.max(1, n.outputs || 1) }, () => flat) : mockOp(n, inputs);
      } catch (e) {
        ev({ type: 'node_error', node: n.id, error: e.message, warnings: [] });
        finish(false, (n.title || n.type) + ': ' + e.message);
        return;
      }
      results.set(n.id, outs);
      ev({ type: 'node_done', node: n.id, ms: Math.round(dur), counts: outs.map((o) => o.length), outputs: outs.map((o) => o.slice(0, 500)), warnings: [] });
      setTimeout(next, 40);
    };
    setTimeout(tick, dur / 4);
  };
  ev({ type: 'started', run: runId });
  setTimeout(next, 30);
  return runId;
}

async function mock(cmd, a) {
  await new Promise((r) => setTimeout(r, 10));
  switch (cmd) {
    case 'system_info':
      return { os: 'demo', arch: 'web', version: '0.2.0', cpus: navigator.hardwareConcurrency || 8, temp: 'C:/Users/demo/AppData/Local/Temp/morph', home: 'C:/Users/demo' };
    case 'detect_tools':
      return {
        ffmpeg: { path: 'ffmpeg', version: 'ffmpeg version 7.1 (demo)' },
        ffprobe: { path: 'ffprobe', version: 'ffprobe version 7.1 (demo)' },
        soffice: null,
        pandoc: { path: 'pandoc', version: 'pandoc 3.5 (demo)' },
      };
    case 'tool_status': {
      const dir = 'C:/Users/demo/AppData/Local/Morph/tools';
      const tool = (path, version, managed, installable) => ({ path, version, managed, installable });
      return {
        dir,
        tools: {
          ffmpeg: tool(dir + '/ffmpeg/bin/ffmpeg.exe', 'ffmpeg version 7.1-full (demo)', true, true),
          ffprobe: tool(dir + '/ffmpeg/bin/ffprobe.exe', 'ffprobe version 7.1-full (demo)', true, true),
          pandoc: tool('C:/Program Files/Pandoc/pandoc.exe', 'pandoc 3.5', false, true),
          soffice: tool(null, null, false, false),
          gs: tool(null, null, false, false),
          pdftoppm: tool(null, null, false, false),
          sevenzip: tool('C:/Program Files/7-Zip/7z.exe', null, false, false),
        },
      };
    }
    case 'install_tool':
    case 'remove_tool':
    case 'open_tools_folder':
      return null;
    case 'sheet_preview': {
      const head = ['Month', 'Revenue', 'Costs', 'Profit', 'Growth'];
      const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
      const rows = [head, ...months.map((m, i) => {
        const r = 42000 + i * 3100 + (i % 3) * 1800;
        const c = 27000 + i * 1400;
        return [m, String(r), String(c), String(r - c), (i ? ((r / (42000 + (i - 1) * 3100) - 1) * 100).toFixed(1) + '%' : '')];
      })];
      return { sheets: ['2026', 'Forecast', 'Notes'], sheet: a.sheet || '2026', rows, total_rows: rows.length, cols: head.length };
    }
    case 'pdf_info':
      return { pages: 24, version: '1.7', encrypted: false, page_mm: [210, 297], title: 'User manual', author: 'Morph demo' };
    case 'scan_paths':
      return DEMO_FILES.map((f) => ({ ...f }));
    case 'thumbnail': {
      const k = kindOf(extOf(a.path));
      return { kind: k, thumb: ['image', 'video', 'audio'].includes(k) ? demoThumb(a.path) : null, width: 1920, height: 1280, duration: k === 'video' ? 42.5 : k === 'audio' ? 184.2 : null };
    }
    case 'probe':
      return demoProbe(a.path);
    case 'mesh_preview':
      return torusKnot();
    case 'read_text':
      return demoText(a.path);
    case 'list_archive':
      return { entries: DEMO_FILES.slice(0, 8).map((f) => ({ name: f.rel + '/' + f.name, size: f.size, dir: false })), total: 8 };
    case 'write_text':
    case 'open_path':
    case 'reveal_path':
    case 'clear_cache':
      return null;
    case 'run_graph':
      return mockRun(a.graph);
    case 'cancel_run':
      cancelFlag = true;
      return null;
    default:
      throw new Error('Unknown command ' + cmd);
  }
}
