// Converter: the classic "Add files -> Profile -> Convert" window (Bigasoft, Format Factory...)
// on top of the node engine. Edits (trim, crop, rotate, effect, watermark, subtitles) apply to
// every file ("apply to all"), the profile picks the converter node. It runs a linear graph
// q1..qN without touching the canvas; "Open as workflow" puts the same graph on the canvas.
import { el, icon, toast, fmtBytes, fmtTime, basename, extOf, kindIcon } from './ui.js';
import { t, groupLabel, optLabel } from './i18n.js';
import { invoke, pickFiles, DEMO, DEMO_ROOT, toolSettings, loadThumb } from './api.js';
import { CONVERT_TARGETS, NODE_DEFS, CATEGORIES } from './nodes.js';
import { SERVICES, SERVICE_LBL, SERVICE_MB } from './nodes_v4.js';
import { PROFILE_GROUPS, PROFILE_LBL, PROFILE_INFO } from './profiles.js';
import { createWidget, isVisible } from './widgets.js';
import { paint } from './colors.js';

const IMG = CONVERT_TARGETS.Image;
const VID = CONVERT_TARGETS.Video;
const AUD = CONVERT_TARGETS.Audio;
const MESH = CONVERT_TARGETS['3D'];
const DATA = ['json', 'yaml', 'toml', 'csv', 'tsv', 'ndjson'];
const ARCH = CONVERT_TARGETS.Archive;
const SHEET_IN = ['xlsx', 'xlsm', 'xlsb', 'xls', 'xla', 'xlam', 'ods', 'csv', 'tsv', 'tab', 'json', 'ndjson', 'jsonl', 'yaml', 'yml'];
const SHEET_OUT = ['xlsx', 'csv', 'tsv', 'json', 'ndjson', 'yaml', 'html', 'md', 'xml', 'sql', 'txt', 'ods', 'xls', 'pdf'];


// JS mirror of route() in src-tauri/src/ops/convert.rs: which engine converts f to `to`, '' if none.
export function route(f, to, tools = {}) {
  const k = f.kind;
  const e = (f.ext || '').toLowerCase();
  const office = !!tools.soffice;
  const pandoc = !!tools.pandoc;
  if (ARCH.includes(to)) return k === 'archive' ? 'repack' : 'pack';
  if (k === 'image') return IMG.includes(to) ? 'image' : to === 'pdf' ? 'image-pdf' : VID.includes(to) ? 'image-video' : '';
  if (k === 'video') return VID.includes(to) ? 'video' : AUD.includes(to) ? 'video-audio' : to === 'gif' || to === 'webp' ? 'video-anim' : IMG.includes(to) ? 'video-frame' : '';
  if (k === 'audio') return AUD.includes(to) ? 'audio' : ['mp4', 'webm', 'mkv', 'mov'].includes(to) ? 'audio-video' : '';
  if (k === 'mesh') return MESH.includes(to) ? 'mesh' : '';
  if (['json', 'yaml', 'yml', 'toml', 'ndjson', 'jsonl', 'geojson'].includes(e) && DATA.includes(to)) return 'data';
  if (SHEET_IN.includes(e) && SHEET_OUT.includes(to)) return ['ods', 'xls', 'pdf'].includes(to) && !office ? '' : 'sheet';
  if (e === 'pdf') return to === 'txt' ? 'pdf-text' : ['png', 'jpg', 'jpeg'].includes(to) ? 'pdf-image' : '';
  if (e === 'md' || e === 'markdown') {
    if (to === 'html') return 'md-html';
    if (to === 'pdf' && office) return 'md-pdf';
    if (to === 'txt') return 'text';
    if (pandoc) return 'pandoc';
    return office && ['docx', 'odt', 'rtf', 'doc'].includes(to) ? 'md-office' : '';
  }
  if (k === 'text' || e === 'txt') {
    if (to === 'html') return 'md-html';
    if (['pdf', 'docx', 'odt', 'rtf', 'doc'].includes(to) && office) return 'office';
    return pandoc && ['docx', 'odt', 'epub', 'rtf', 'md'].includes(to) ? 'pandoc' : '';
  }
  if (to === 'txt' && ['docx', 'odt', 'odp', 'pptx', 'epub', 'html', 'htm', 'rtf'].includes(e) && !office) return 'text';
  if (['html', 'htm'].includes(e) && to === 'md' && pandoc) return 'pandoc';
  if (['docx', 'odt', 'rtf', 'epub', 'html', 'htm', 'ipynb', 'rst', 'tex', 'org'].includes(e) && ['md', 'epub', 'rst', 'tex', 'org'].includes(to) && pandoc) return 'pandoc';
  if (k === 'doc' && office && ['pdf', 'docx', 'doc', 'odt', 'rtf', 'html', 'txt', 'epub', 'pptx', 'ppt', 'odp', 'png', 'jpg'].includes(to)) return 'office';
  if (['docx', 'odt', 'epub', 'html', 'htm', 'rtf'].includes(e) && pandoc && ['docx', 'odt', 'html', 'txt', 'pdf', 'epub'].includes(to)) return 'pandoc';
  if (to === 'txt' && k === 'doc') return 'text';
  return '';
}

// Which external tool a route needs (for hints).
const NEEDS = { video: 'ffmpeg', 'video-audio': 'ffmpeg', 'video-anim': 'ffmpeg', 'video-frame': 'ffmpeg', audio: 'ffmpeg', 'audio-video': 'ffmpeg', 'image-video': 'ffmpeg', office: 'soffice', 'md-pdf': 'soffice', 'md-office': 'soffice', pandoc: 'pandoc' };

// ------------------------------------------------------------------ profile catalog
const VGROUP = { General: ['film', 'video'], 'HD & 4K': ['sparkle', 'video'], Apple: ['phone', 'video'], Android: ['phone', 'video'], Consoles: ['gamepad', 'video'], 'TV & disc': ['tv', 'video'], 'Web & social': ['globe', 'video'], Editing: ['layers', 'video'] };
const AUDIO_LIST = [
  ['mp3', 'MP3', 'cv.h_everywhere', 192], ['m4a', 'M4A (AAC)', 'cv.h_apple', 192], ['aac', 'AAC', 'cv.h_raw', 192], ['opus', 'Opus', 'cv.h_opus', 128],
  ['ogg', 'OGG Vorbis', 'cv.h_open', 192], ['flac', 'FLAC', 'cv.h_lossless', 0], ['wav', 'WAV', 'cv.h_uncompressed', 0], ['alac', 'ALAC', 'cv.h_alac', 0],
  ['aiff', 'AIFF', 'cv.h_uncompressed', 0], ['wma', 'WMA', 'cv.h_windows', 192], ['ac3', 'AC-3', 'cv.h_dolby', 384], ['m4r', 'M4R', 'cv.h_ringtone', 192],
  ['amr', 'AMR', 'cv.h_voice', 0], ['mp2', 'MP2', 'cv.h_broadcast', 224],
];
const IMAGE_LIST = [
  ['jpeg', 'JPG', 'cv.h_photos'], ['png', 'PNG', 'cv.h_png'], ['webp', 'WebP', 'cv.h_web'], ['avif', 'AVIF', 'cv.h_smallest'], ['gif', 'GIF', 'cv.h_classic'],
  ['bmp', 'BMP', 'cv.h_uncompressed'], ['tiff', 'TIFF', 'cv.h_print'], ['ico', 'ICO', 'cv.h_icon'], ['tga', 'TGA', 'cv.h_games'], ['qoi', 'QOI', 'cv.h_qoi'],
];
const GIF_LIST = [
  ['gif', 'GIF · 480 px · 12 fps', { format: 'gif', fps: 12, width: 480 }],
  ['gif_hd', 'GIF HD · 720 px · 15 fps', { format: 'gif', fps: 15, width: 720 }],
  ['gif_small', 'GIF · 320 px · 10 fps', { format: 'gif', fps: 10, width: 320, colors: 128 }],
  ['webp', 'WebP · 480 px · 15 fps', { format: 'webp', fps: 15, width: 480 }],
];
const ROUTE_CATS = { Document: ['doc', 'doc'], Spreadsheet: ['sheet', 'sheet'], Data: ['data', 'data'], '3D': ['mesh', 'mesh'], Archive: ['archive', 'archive'] };
const POPULAR = ['v:mp4', 'v:iphone', 'v:android', 'v:youtube', 'v:shorts', 'v:mkv', 'a:mp3', 'a:m4a', 'i:jpeg', 'i:webp', 'g:gif', 's:discord', 's:telegram', 's:gmail', 'r:pdf', 'v:dvd_pal'];

const L = (s) => optLabel({}, s);
const mbLabel = (mb) => (mb >= 1000 ? +(mb / 1000).toFixed(1) + ' GB' : mb + ' MB');

function buildCatalog() {
  const list = [];
  for (const [g, ids] of Object.entries(PROFILE_GROUPS)) {
    for (const id of ids) {
      const i = PROFILE_INFO[id];
      list.push({ key: 'v:' + id, cat: g, label: PROFILE_LBL[id], spec: [i.ext.toUpperCase(), i.v, i.size].join(' · '), ext: i.ext, node: 'video.convert', values: { profile: id }, icon: VGROUP[g][0], c: 'video', accepts: ['video'] });
    }
  }
  for (const [fmt, label, hint, kbps] of AUDIO_LIST) {
    list.push({ key: 'a:' + fmt, cat: 'Audio', label, hintKey: hint, spec: kbps ? kbps + ' kbps' : '', ext: fmt === 'alac' ? 'm4a' : fmt, node: 'audio.convert', values: kbps ? { format: fmt, bitrate: kbps } : { format: fmt }, icon: 'audio', c: 'audio', accepts: ['audio', 'video'] });
  }
  for (const [fmt, label, hint] of IMAGE_LIST) {
    list.push({ key: 'i:' + fmt, cat: 'Image', label, hintKey: hint, ext: fmt === 'jpeg' ? 'jpg' : fmt, node: 'image.convert', values: { format: fmt }, icon: 'image', c: 'image', accepts: ['image'] });
  }
  list.push({ key: 'i:pdf', cat: 'Image', label: 'PDF', hintKey: 'cv.h_imgpdf', ext: 'pdf', node: 'image.pdf', values: {}, icon: 'pdf', c: 'pdf', accepts: ['image'], merges: true });
  for (const [id, label, values] of GIF_LIST) {
    list.push({ key: 'g:' + id, cat: 'GIF', label, hintKey: 'cv.h_gif', ext: values.format, node: 'video.gif', values, icon: 'film', c: 'video', accepts: ['video'] });
  }
  for (const id of [...SERVICES.Chat, ...SERVICES['E-mail']]) {
    const mail = SERVICES['E-mail'].includes(id);
    list.push({ key: 's:' + id, cat: 'Share', label: SERVICE_LBL[id].replace(/ \([^)]*\)$/, ''), spec: t('cv.upto', { size: mbLabel(SERVICE_MB[id]) }), ext: '', node: 'share.compress', values: { service: id }, icon: mail ? 'mail' : 'send', c: 'convert', accepts: null });
  }
  for (const [g, [cat]] of Object.entries(ROUTE_CATS)) {
    for (const to of CONVERT_TARGETS[g]) {
      list.push({ key: 'r:' + to, cat: g, label: to.toUpperCase(), hintKey: 'cv.f_' + to, ext: to, node: 'convert.any', values: { to }, icon: (CATEGORIES[cat] || CATEGORIES.convert).icon, c: cat, route: to });
    }
  }
  return list;
}

const CATS = [
  ['Popular', 'star', 'convert'], ['General', 'film', 'video'], ['HD & 4K', 'sparkle', 'video'], ['Apple', 'phone', 'video'], ['Android', 'phone', 'video'],
  ['Consoles', 'gamepad', 'video'], ['TV & disc', 'tv', 'video'], ['Web & social', 'globe', 'video'], ['Editing', 'layers', 'video'],
  ['Audio', 'audio', 'audio'], ['Image', 'image', 'image'], ['GIF', 'film', 'video'], ['Share', 'send', 'convert'],
  ['Document', 'doc', 'doc'], ['Spreadsheet', 'sheet', 'sheet'], ['Data', 'data', 'data'], ['3D', 'mesh', 'mesh'], ['Archive', 'archive', 'archive'],
];
const catKey = (c) => 'cv.c_' + c.toLowerCase().replace(/[^a-z0-9]+/g, '_');

// ------------------------------------------------------------------ edits ("apply to all")
// Each edit is edited with the widgets of a video node; images get mapped values.
const EDITS = [
  { id: 'trim', icon: 'scissors', node: 'video.trim', widgets: ['start', 'end', 'precise'], kinds: ['video'],
    active: (v) => (v.start && v.start !== '0:00' && v.start !== '0') || !!String(v.end || '').trim() },
  { id: 'crop', icon: 'crop', node: 'video.reframe', widgets: ['aspect', 'mode', 'color'], init: { aspect: '16:9', mode: 'crop' }, kinds: ['video', 'image'],
    image: (v) => ({ type: 'image.crop', values: { mode: 'aspect', aspect: v.aspect, anchor: 'center' } }) },
  { id: 'rotate', icon: 'rotate', node: 'video.transform', widgets: ['rotate', 'flip_h', 'flip_v'], init: { rotate: '90' }, kinds: ['video', 'image'],
    active: (v) => v.rotate !== '0' || v.flip_h || v.flip_v,
    image: (v) => ({ type: 'image.transform', values: { rotate: v.rotate, flip_h: !!v.flip_h, flip_v: !!v.flip_v } }) },
  { id: 'effect', icon: 'sparkle', node: 'video.filters', widgets: ['look', 'brightness', 'contrast', 'saturation', 'deinterlace', 'denoise'], kinds: ['video', 'image'],
    active: (v) => v.look !== 'none' || v.brightness !== 0 || v.contrast !== 1 || v.saturation !== 1 || v.deinterlace || v.denoise !== 'off',
    image: (v) => ({ type: 'image.adjust', values: { brightness: Math.round(v.brightness * 100), contrast: Math.round((v.contrast - 1) * 100), saturation: Math.round((v.saturation - 1) * 100), grayscale: v.look === 'grayscale', invert: v.look === 'invert' || v.look === 'negative' } }) },
  { id: 'watermark', icon: 'stamp', node: 'video.watermark', widgets: ['mode', 'text', 'image', 'position', 'size', 'opacity', 'color'], kinds: ['video', 'image'],
    active: (v) => (v.mode === 'image' ? !!v.image : !!String(v.text || '').trim()),
    image: (v) => (v.mode === 'image'
      ? { type: 'image.watermark', values: { watermark: v.image, position: v.position, opacity: v.opacity, scale: Math.max(5, Math.min(100, v.size * 3)) } }
      : { type: 'image.text', values: { text: v.text, position: v.position, opacity: v.opacity, color: v.color, size: Math.max(0.5, Math.min(30, v.size)) } }) },
  { id: 'subtitles', icon: 'cc', node: 'video.subtitles', widgets: ['mode', 'file', 'font_size', 'color', 'language'], init: { mode: 'burn' }, kinds: ['video'],
    active: (v) => v.mode !== 'extract' },
];

function defaults(type, init = {}) {
  const v = {};
  for (const w of NODE_DEFS[type].widgets) v[w.name] = w.value;
  return { ...v, ...init };
}

export class QuickConvert {
  constructor(ctx) {
    this.ctx = ctx;
    this.back = null;
    this.paths = [];
    this.files = [];
    this.off = new Set();
    this.catalog = buildCatalog();
    this.profile = this.catalog.find((p) => p.key === (ctx.settings.cv_profile || 'v:mp4')) || this.catalog[0];
    this.cat = 'Popular';
    this.opts = {};
    this.edits = {};
    this.sheet = null;
    this.merge = false;
    this.run = null;
    this.result = null;
    this.dest = ctx.settings.qc_dest || 'next';
  }

  isOpen() {
    return !!this.back;
  }

  // o.to: a format (legacy quick convert), o.profile: a catalog key, o.cat: a category, o.paths
  open(o = {}) {
    if (o.to) {
      const keys = ['r:' + o.to, 'v:' + o.to, 'a:' + o.to, 'i:' + (o.to === 'jpg' ? 'jpeg' : o.to), 'g:' + o.to];
      this.pick(keys.map((k) => this.catalog.find((p) => p.key === k)).find(Boolean) || this.profile, false);
    }
    if (o.profile) this.pick(this.catalog.find((p) => p.key === o.profile) || this.profile, false);
    if (o.cat) this.cat = o.cat;
    if (this.back) {
      if (o.paths) this.addPaths(o.paths);
      this.render();
      return;
    }
    this.back = el('div', { class: 'dialog-back' });
    this.box = el('div', { class: 'dialog cv' });
    this.back.append(this.box);
    this.back.addEventListener('pointerdown', (e) => {
      if (e.target === this.back && !this.run) this.close();
    });
    this.onKey = (e) => {
      if (e.key === 'Escape' && !this.run) {
        e.preventDefault();
        e.stopPropagation();
        if (this.sheet) {
          this.sheet = null;
          this.render();
        } else this.close();
      }
    };
    document.addEventListener('keydown', this.onKey, true);
    document.body.append(this.back);
    if (DEMO && !this.paths.length) this.paths = [DEMO_ROOT];
    if (o.paths) this.addPaths(o.paths);
    else this.scan();
    this.render();
  }

  close() {
    if (!this.back) return;
    document.removeEventListener('keydown', this.onKey, true);
    this.back.remove();
    this.back = null;
    this.sheet = null;
  }

  async addPaths(paths) {
    if (!paths || !paths.length) return;
    this.paths = [...new Set([...this.paths, ...paths])];
    this.result = null;
    await this.scan();
  }

  async scan() {
    if (!this.paths.length) {
      this.files = [];
      this.render();
      return;
    }
    try {
      this.files = (await invoke('scan_paths', { paths: this.paths, recursive: true, include: null, exclude: null })) || [];
    } catch (e) {
      this.files = this.paths.map((p) => ({ path: p, name: basename(p), ext: extOf(p), kind: 'other', size: 0 }));
    }
    this.render();
  }

  pick(p, render = true) {
    this.profile = p;
    this.result = null;
    this.ctx.settings.cv_profile = p.key;
    this.ctx.save();
    if (render) this.render();
  }

  accepts(p, f) {
    if (this.off.has(f.path)) return false;
    if (p.route) return f.ext !== p.route && !!route(f, p.route, this.ctx.tools());
    if (!p.accepts) return true;
    return p.accepts.includes(f.kind);
  }

  matching(p = this.profile) {
    return this.files.filter((f) => this.accepts(p, f));
  }

  kindsOf(list) {
    return new Set(list.map((f) => f.kind));
  }

  // ---------------------------------------------------------------- render
  render() {
    if (!this.box) return;
    const keep = this.list ? this.list.scrollTop : 0;
    const keepP = this.grid ? this.grid.scrollTop : 0;
    this.box.textContent = '';
    this.box.append(this.head(), el('div', { class: 'cv-main' }, this.left(), this.right()), this.foot());
    if (this.sheet) this.box.append(this.renderSheet());
    if (this.list) this.list.scrollTop = keep;
    if (this.grid) this.grid.scrollTop = keepP;
  }

  head() {
    const q = this.search ? this.search.value : '';
    const input = el('input', { class: 'search', placeholder: t('cv.search'), spellcheck: 'false', value: q });
    this.search = input;
    input.addEventListener('input', () => this.fillProfiles());
    input.addEventListener('keydown', (e) => e.stopPropagation());
    return el('div', { class: 'cv-head' },
      el('div', { class: 'cv-title' }, el('span', { class: 'cv-badge' }, icon('bolt', 16)), el('div', {}, el('h3', { text: t('cv.title') }), el('p', { text: t('cv.sub') }))),
      el('div', { class: 'search-wrap' }, input, icon('search', 14)),
      el('button', { class: 'icon-btn', title: t('dlg.cancel'), onclick: () => !this.run && this.close() }, icon('x', 16)));
  }

  left() {
    const total = this.files.length;
    const match = this.matching();
    const bytes = match.reduce((s, f) => s + (f.size || 0), 0);
    const bar = el('div', { class: 'cv-bar' },
      el('button', { class: 'btn sm primary-soft', onclick: async () => this.addPaths(await pickFiles({ multiple: true })) }, icon('file-plus', 13), t('cv.add_file')),
      el('button', { class: 'btn sm', onclick: async () => this.addPaths(await pickFiles({ folder: true, multiple: true })) }, icon('folder', 13), t('cv.add_folder')),
      total ? el('button', {
        class: 'btn sm ghost',
        onclick: () => {
          this.paths = [];
          this.files = [];
          this.off.clear();
          this.result = null;
          this.render();
        },
      }, icon('trash', 13), t('cv.clear')) : null,
      el('div', { class: 'grow' }),
      total ? el('span', { class: 'cv-count', text: t('cv.count', { n: match.length, total, size: fmtBytes(bytes) }) }) : null);
    const list = el('div', { class: 'cv-list scroll' });
    this.list = list;
    if (!total) {
      list.append(el('div', { class: 'cv-empty' },
        el('div', { class: 'cv-empty-ic' }, icon('upload', 26)),
        el('b', { text: t('cv.drop') }),
        el('span', { text: t('cv.drop_d') })));
    } else {
      list.append(el('div', { class: 'cv-row cv-th' }, el('span'), el('span', { text: t('cv.col_name') }), el('span', { text: t('cv.col_size') }), el('span', { text: t('cv.col_out') }), el('span')));
      const p = this.profile;
      for (const f of this.files.slice(0, 500)) {
        const ok = this.accepts(p, f);
        const skipped = this.off.has(f.path);
        const done = this.result && this.result.ok && ok;
        const th = el('span', { class: 'cv-th-img k-' + f.kind }, icon(kindIcon(f.kind), 15));
        if (['image', 'video'].includes(f.kind)) {
          loadThumb(f, 96).then((url) => {
            if (url && th.isConnected) {
              th.textContent = '';
              th.append(el('img', { src: url, alt: '', loading: 'lazy' }));
            }
          }).catch(() => {});
        }
        const outExt = ok ? (p.ext || f.ext || '').toUpperCase() : '';
        list.append(el('div', { class: 'cv-row' + (ok ? '' : ' off'), title: f.path },
          el('button', {
            class: 'cv-check' + (skipped ? '' : ' on'),
            title: t('cv.include'),
            onclick: () => {
              if (skipped) this.off.delete(f.path);
              else this.off.add(f.path);
              this.result = null;
              this.render();
            },
          }, icon('check', 11)),
          el('span', { class: 'cv-name' }, th, el('span', { class: 'nm', text: f.name }), el('small', { text: (f.ext || '').toUpperCase() })),
          el('span', { class: 'cv-size', text: fmtBytes(f.size || 0) }),
          el('span', { class: 'cv-out' }, outExt ? el('em', { text: outExt }) : el('i', { text: skipped ? t('cv.skipped') : t('cv.unsupported') })),
          el('span', { class: 'cv-st' }, done ? icon('check', 13) : null)));
      }
      if (total > 500) list.append(el('div', { class: 'lib-hint', text: t('insp.more', { n: total - 500 }) }));
    }
    const kinds = this.kindsOf(match);
    const edits = el('div', { class: 'cv-edits' }, el('span', { class: 'cv-lbl', text: t('cv.edit') }));
    for (const e of EDITS) {
      const usable = e.kinds.some((k) => kinds.has(k));
      const st = this.edits[e.id];
      const on = st && st.on;
      edits.append(el('button', {
        class: 'cv-tool' + (on ? ' on' : ''),
        disabled: !usable && !on,
        title: usable ? t('cv.e_' + e.id + '_t') : t('cv.e_na'),
        onclick: () => {
          this.sheet = { kind: 'edit', id: e.id };
          if (!this.edits[e.id]) this.edits[e.id] = { on: false, values: defaults(e.node, e.init) };
          this.render();
        },
      }, icon(e.icon, 14), el('span', { text: t('cv.e_' + e.id) }), on ? el('i') : null));
    }
    edits.append(el('span', { class: 'cv-vsep' }));
    edits.append(el('button', {
      class: 'cv-tool',
      disabled: !this.files.some((f) => f.kind === 'video'),
      title: t('cv.snapshot_t'),
      onclick: () => {
        this.sheet = { kind: 'snapshot' };
        if (!this.snap) this.snap = { time: '0:01', format: 'png', width: 0 };
        this.render();
      },
    }, icon('camera', 14), el('span', { text: t('cv.snapshot') })));
    return el('div', { class: 'cv-left' }, bar, list, edits);
  }

  right() {
    const cats = el('div', { class: 'cv-cats scroll' });
    for (const [c, ic, color] of CATS) {
      const n = c === 'Popular' ? POPULAR.length : this.catalog.filter((p) => p.cat === c).length;
      cats.append(paint(el('button', {
        class: 'cv-cat' + (this.cat === c ? ' on' : ''),
        onclick: () => {
          this.cat = c;
          if (this.search) this.search.value = '';
          this.render();
        },
      }, el('span', { class: 'ci' }, icon(ic, 14)), el('span', { class: 'cl', text: t(catKey(c)) }), el('small', { text: String(n) })), (CATEGORIES[color] || CATEGORIES.convert).color));
    }
    this.grid = el('div', { class: 'cv-grid scroll' });
    this.fillProfiles();
    return el('div', { class: 'cv-right' }, cats, this.grid);
  }

  fillProfiles() {
    const grid = this.grid;
    if (!grid) return;
    grid.textContent = '';
    const q = (this.search ? this.search.value : '').trim().toLowerCase();
    let list;
    if (q) {
      list = this.catalog.filter((p) => [p.label, L(p.label), p.spec || '', p.hintKey ? t(p.hintKey) : '', t(catKey(p.cat)), p.ext || '', p.key].join(' ').toLowerCase().includes(q));
    } else if (this.cat === 'Popular') {
      list = POPULAR.map((k) => this.catalog.find((p) => p.key === k)).filter(Boolean);
    } else {
      list = this.catalog.filter((p) => p.cat === this.cat);
    }
    if (!list.length) {
      grid.append(el('div', { class: 'cv-none', text: t('cv.none') }));
      return;
    }
    const tools = this.ctx.tools();
    for (const p of list) {
      const n = this.files.length ? this.matching(p).length : -1;
      const need = p.route && n > 0 ? NEEDS[route(this.matching(p)[0], p.route, tools)] : null;
      const sub = need && !tools[need] ? '⚠ ' + t('qc.needs', { tool: need }) : p.spec || (p.hintKey ? t(p.hintKey) : '');
      const color = (CATEGORIES[p.c] || CATEGORIES.convert).color;
      grid.append(paint(el('button', {
        class: 'cv-prof' + (this.profile.key === p.key ? ' on' : '') + (n === 0 ? ' dim' : ''),
        title: n >= 0 ? t('cv.match', { n, total: this.files.length }) : L(p.label),
        onclick: () => this.pick(p),
        ondblclick: () => {
          this.pick(p, false);
          this.start();
        },
      },
      el('span', { class: 'pi' }, icon(p.icon, 16)),
      el('span', { class: 'pt' }, el('b', { text: L(p.label) }), el('small', { text: sub })),
      p.ext ? el('em', { text: p.ext.toUpperCase() }) : null), color));
    }
  }

  foot() {
    const p = this.profile;
    const color = (CATEGORIES[p.c] || CATEGORIES.convert).color;
    const summary = paint(el('button', { class: 'cv-sum', title: t('cv.settings_t'), onclick: () => this.openSettings() },
      el('span', { class: 'pi' }, icon(p.icon, 16)),
      el('span', { class: 'pt' }, el('small', { text: t('cv.profile') }), el('b', { text: L(p.label) })),
      el('span', { class: 'cv-gear' }, icon('settings', 14))), color);
    const segc = el('div', { class: 'seg-ctl' },
      ...[['next', t('qc.dest_next')], ['folder', t('qc.dest_folder')]].map(([k, label]) => el('button', {
        class: this.dest === k ? 'on' : '',
        text: label,
        onclick: async () => {
          if (k === 'folder') {
            const r = await pickFiles({ folder: true, multiple: false });
            if (!r[0] && !this.folder) return;
            if (r[0]) this.folder = r[0];
          }
          this.dest = k;
          this.ctx.settings.qc_dest = k;
          this.ctx.save();
          this.render();
        },
      })));
    const dest = el('div', { class: 'cv-dest' }, el('small', { text: t('cv.dest') }), segc,
      this.dest === 'folder' ? el('span', { class: 'cv-folder', title: this.folderPath(), text: basename(this.folderPath()) || this.folderPath() }) : null);
    const canMerge = this.canMerge();
    const merge = el('button', {
      class: 'cv-merge' + (this.merge && canMerge ? ' on' : ''),
      disabled: !canMerge,
      title: t('cv.merge_t'),
      onclick: () => {
        this.merge = !this.merge;
        this.render();
      },
    }, el('span', { class: 'sw' }, el('i')), el('span', { text: t('cv.merge') }));
    const n = this.matching().length;
    const foot = el('div', { class: 'cv-foot' }, summary, dest, merge, el('div', { class: 'grow' }));
    if (this.run) {
      foot.append(el('div', { class: 'qc-prog' }, el('div', { class: 'bar' }, el('i', { style: { width: Math.round((this.run.p || 0) * 100) + '%' } })), el('span', { text: this.run.text || t('qc.running') })));
      foot.append(el('button', { class: 'btn', onclick: () => invoke('cancel_run').catch(() => {}) }, icon('stop', 12), t('top.cancel')));
      return foot;
    }
    if (this.result) {
      const r = this.result;
      foot.append(el('span', { class: 'cv-res ' + (r.ok ? 'ok' : 'bad'), text: r.ok ? t('qc.done', { n: r.count, time: fmtTime(r.ms) }) : t('qc.failed') + ': ' + (r.error || '') }));
      if (r.ok && r.first) foot.append(el('button', { class: 'btn', onclick: () => this.ctx.reveal(r.first) }, icon('folder', 13), t('qc.open')));
    }
    foot.append(el('button', { class: 'btn ghost', title: t('cv.workflow_t'), disabled: !n, onclick: () => this.toWorkflow() }, icon('wire', 13), t('cv.workflow')));
    foot.append(el('button', {
      class: 'btn primary lg',
      disabled: !n,
      onclick: () => this.start(),
    }, icon('play', 13), n ? t('cv.convert_n', { n }) : t('qc.convert')));
    return foot;
  }

  canMerge() {
    const p = this.profile;
    return p.node === 'video.convert' || p.node === 'audio.convert' || (p.node === 'convert.any' && p.route === 'pdf');
  }

  folderPath() {
    return this.folder || this.ctx.settings.output || '~/Morph Output';
  }

  // ---------------------------------------------------------------- sheets (edit, settings, snapshot)
  openSettings() {
    this.sheet = { kind: 'settings' };
    this.render();
  }

  renderSheet() {
    const s = this.sheet;
    const body = el('div', { class: 'cv-sheet-body scroll' });
    let title;
    let sub;
    let ic;
    let actions;
    const close = () => {
      this.sheet = null;
      this.render();
    };
    const fields = (type, values, names, onChange) => {
      const def = NODE_DEFS[type];
      for (const w of def.widgets) {
        if (names && !names.includes(w.name)) continue;
        if (!isVisible(w, values)) continue;
        body.append(createWidget({ ...w, adv: false }, values[w.name], (v) => {
          values[w.name] = v;
          if (onChange) onChange(w.name);
        }));
      }
    };
    if (s.kind === 'edit') {
      const e = EDITS.find((x) => x.id === s.id);
      const st = this.edits[e.id];
      title = t('cv.e_' + e.id);
      sub = t('cv.e_' + e.id + '_t');
      ic = e.icon;
      const redraw = (name) => {
        if (['mode', 'look'].includes(name)) this.render();
      };
      fields(e.node, st.values, e.widgets, redraw);
      body.append(el('p', { class: 'cv-note', text: t('cv.apply_all') }));
      actions = [
        el('button', { class: 'btn ghost', onclick: () => { this.edits[e.id] = { on: false, values: defaults(e.node, e.init) }; close(); } }, t('cv.reset')),
        el('button', { class: 'btn primary', onclick: () => { st.on = e.active ? !!e.active(st.values) : true; this.result = null; close(); } }, icon('check', 13), t('cv.apply')),
      ];
    } else if (s.kind === 'snapshot') {
      title = t('cv.snapshot');
      sub = t('cv.snapshot_t');
      ic = 'camera';
      const def = NODE_DEFS['video.frames'];
      for (const name of ['time', 'format', 'width']) {
        const w = def.widgets.find((x) => x.name === name);
        body.append(createWidget({ ...w, adv: false, show: null, hide: null }, this.snap[name], (v) => {
          this.snap[name] = v;
        }));
      }
      actions = [
        el('button', { class: 'btn ghost', onclick: close }, t('dlg.cancel')),
        el('button', { class: 'btn primary', onclick: () => { this.sheet = null; this.start({ snapshot: true }); } }, icon('camera', 13), t('cv.snapshot_go')),
      ];
    } else {
      const p = this.profile;
      title = t('cv.settings');
      sub = L(p.label);
      ic = 'settings';
      const vals = this.nodeValues(p);
      const primary = { 'video.convert': 'profile', 'audio.convert': 'format', 'image.convert': 'format', 'share.compress': 'service', 'convert.any': 'to', 'video.gif': 'format' }[p.node];
      const store = (this.opts[p.key] = this.opts[p.key] || {});
      const def = NODE_DEFS[p.node];
      for (const w of def.widgets) {
        if (w.name === primary || !isVisible(w, vals)) continue;
        body.append(createWidget({ ...w, adv: false }, vals[w.name], (v) => {
          store[w.name] = v;
          vals[w.name] = v;
          if (w.kind === 'combo' || w.kind === 'toggle') this.render();
        }));
      }
      if (!body.childElementCount) body.append(el('p', { class: 'cv-note', text: t('cv.no_settings') }));
      if (p.node === 'video.convert') body.append(el('p', { class: 'cv-note', text: t('cv.profile_note') }));
      actions = [
        el('button', { class: 'btn ghost', onclick: () => { delete this.opts[p.key]; this.render(); } }, t('cv.reset')),
        el('button', { class: 'btn primary', onclick: close }, icon('check', 13), t('cv.apply')),
      ];
    }
    const sheet = el('div', { class: 'cv-sheet' },
      el('div', { class: 'cv-sheet-head' }, el('span', { class: 'cv-badge' }, icon(ic, 15)), el('div', {}, el('b', { text: title }), el('small', { text: sub })),
        el('button', { class: 'icon-btn', onclick: close }, icon('x', 15))),
      body,
      el('div', { class: 'cv-sheet-foot' }, ...actions));
    return el('div', { class: 'cv-scrim', onpointerdown: (ev) => { if (ev.target.classList.contains('cv-scrim')) close(); } }, sheet);
  }

  nodeValues(p) {
    return { ...defaults(p.node), ...p.values, ...(this.opts[p.key] || {}) };
  }

  // ---------------------------------------------------------------- graph
  graph(o = {}) {
    const p = this.profile;
    const save = this.dest === 'folder'
      ? { destination: 'folder', folder: this.folderPath(), pattern: '{name}', on_conflict: 'rename', keep_structure: true, open_folder: false }
      : { destination: 'next to original', folder: '', pattern: '{name}', on_conflict: 'rename', keep_structure: true, open_folder: false };
    if (o.snapshot) {
      const vids = this.files.filter((f) => f.kind === 'video' && !this.off.has(f.path));
      return [
        { type: 'files.load', values: { paths: vids.map((f) => f.path), recursive: false, include: '', exclude: '', skip_hidden: true } },
        { type: 'video.frames', values: { ...defaults('video.frames'), mode: 'single', ...this.snap } },
        { type: 'files.save', values: save },
      ];
    }
    const files = this.matching(p);
    const all = this.off.size === 0 && files.length === this.files.length;
    const nodes = [{ type: 'files.load', values: { paths: all ? this.paths : files.map((f) => f.path), recursive: all, include: '', exclude: '', skip_hidden: true } }];
    const kinds = this.kindsOf(files);
    for (const e of EDITS) {
      const st = this.edits[e.id];
      if (!st || !st.on) continue;
      if (kinds.has('video') && e.kinds.includes('video')) nodes.push({ type: e.node, values: { ...st.values } });
      if (kinds.has('image') && e.image) nodes.push(e.image(st.values));
    }
    nodes.push({ type: p.node, values: this.nodeValues(p) });
    if (this.merge && this.canMerge()) {
      if (p.node === 'video.convert') nodes.push({ type: 'video.concat', values: { name: 'merged', reencode: false } });
      else if (p.node === 'audio.convert') {
        const fmt = ['mp3', 'm4a', 'aac', 'opus', 'ogg', 'flac', 'wav'].includes(p.values.format) ? p.values.format : 'mp3';
        nodes.push({ type: 'audio.join', values: { format: fmt, name: 'merged', crossfade: 0, gap: 0, bitrate: p.values.bitrate || 192 } });
      } else nodes.push({ type: 'pdf.merge', values: { order: 'input', name: 'merged' } });
    }
    nodes.push({ type: 'files.save', values: save });
    return nodes;
  }

  graphName(o = {}) {
    return o.snapshot ? t('cv.snapshot') : t('cv.title') + ' → ' + L(this.profile.label);
  }

  async start(o = {}) {
    if (this.run || this.ctx.busy()) return;
    const g = this.graph(o);
    if (!g[0].values.paths.length) {
      toast(t('cv.nothing'), 'warn');
      return;
    }
    const nodes = g.map((n, i) => {
      const def = NODE_DEFS[n.type];
      const vals = {};
      for (const w of def.widgets) vals[w.name] = n.values[w.name] !== undefined ? n.values[w.name] : w.value;
      return {
        id: 'q' + (i + 1),
        type: n.type,
        values: vals,
        inputs: def.inputs.map((_, j) => (i > 0 && j === 0 ? { node: 'q' + i, slot: 0 } : null)),
        outputs: def.outputs.length,
        bypass: false,
        is_output: !!def.output,
        title: def.title,
      };
    });
    this.run = { p: 0, text: t('qc.running'), t0: performance.now(), n: nodes.length, name: this.graphName(o) };
    this.result = null;
    this.ctx.claim(this);
    this.render();
    try {
      await invoke('run_graph', { graph: { nodes, settings: toolSettings() } });
    } catch (e) {
      this.finish({ ok: false, error: String((e && e.message) || e) });
    }
  }

  // Run events routed here by main.js while a converter run is active.
  onEvent(ev) {
    if (!this.run) return;
    const n = this.run.n || 3;
    const idx = ev.node ? Math.max(0, parseInt(String(ev.node).slice(1), 10) - 1) : 0;
    if (ev.type === 'node_start') {
      this.run.p = Math.max(this.run.p, idx / n);
    } else if (ev.type === 'node_progress') {
      this.run.p = Math.max(this.run.p, (idx + Math.min(1, Math.max(0, Number(ev.value) || 0))) / n);
      this.run.text = ev.text || this.run.text;
    } else if (ev.type === 'node_done' && idx === n - 1) {
      const outs = (ev.outputs && ev.outputs[0]) || [];
      this.run.count = (ev.counts && ev.counts[0]) || outs.length;
      this.run.first = outs[0] ? outs[0].path : null;
    } else if (ev.type === 'node_error') {
      this.run.error = ev.error;
    } else if (ev.type === 'finished') {
      this.finish(ev);
      return;
    }
    if (this.box) {
      const bar = this.box.querySelector('.qc-prog .bar i');
      const sp = this.box.querySelector('.qc-prog span');
      if (bar) bar.style.width = Math.round(this.run.p * 100) + '%';
      if (sp) sp.textContent = this.run.text;
    }
  }

  finish(ev) {
    const r = this.run || {};
    this.run = null;
    this.ctx.release(this);
    this.result = { ok: !!ev.ok && !ev.cancelled, error: ev.cancelled ? t('run.cancelled') : ev.error || r.error, count: r.count || 0, first: r.first, ms: ev.ms || (r.t0 ? performance.now() - r.t0 : 0), done: true };
    if (this.result.ok) toast(t('qc.done', { n: this.result.count, time: fmtTime(this.result.ms) }), 'ok');
    this.ctx.history({ name: r.name || t('cv.title'), ok: this.result.ok, error: this.result.ok ? null : this.result.error, ms: this.result.ms, files: this.result.count, cancelled: !!ev.cancelled });
    this.render();
  }

  toWorkflow() {
    const g = this.graph();
    this.ctx.load({
      nodes: g.map((n, i) => ({ type: n.type, x: 40 + i * 380, y: 80 + (i % 2) * 40, values: n.values })),
      links: g.slice(1).map((_, i) => [i, 0, i + 1, 0]),
    }, this.graphName());
    this.close();
  }
}

export const groupName = (g) => groupLabel(g);
