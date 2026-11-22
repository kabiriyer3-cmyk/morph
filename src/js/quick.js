// Quick convert: drop files, pick a target format, press Convert. Runs a tiny
// Load -> Convert Anything -> Save graph without touching the workflow on the canvas.
import { el, icon, toast, fmtBytes, fmtTime, basename, extOf, kindIcon, clamp } from './ui.js';
import { t } from './i18n.js';
import { invoke, pickFiles, DEMO, DEMO_ROOT, toolSettings } from './api.js';
import { CONVERT_TARGETS, NODE_DEFS, CATEGORIES } from './nodes.js';
import { createWidget } from './widgets.js';
import { paint } from './colors.js';

const GROUP_CAT = { Image: 'image', Video: 'video', Audio: 'audio', Document: 'doc', Spreadsheet: 'sheet', Data: 'data', '3D': 'mesh', Archive: 'archive' };
const HINT = {
  png: 'lossless', jpg: 'photos', webp: 'web, small', avif: 'smallest', gif: 'animation', ico: 'icons', tiff: 'print', exr: 'HDR',
  mp4: 'H.264 + AAC', mkv: 'any codec', webm: 'VP9 + Opus', mov: 'Apple', avi: 'classic', gifv: '',
  mp3: 'everywhere', m4a: 'AAC', opus: 'best quality', flac: 'lossless', wav: 'uncompressed', m4r: 'ringtone',
  pdf: 'print, share', docx: 'Word', odt: 'OpenDocument', md: 'Markdown', html: 'web page', txt: 'plain text', epub: 'e-book', pptx: 'PowerPoint',
  xlsx: 'Excel', xls: 'Excel 97', ods: 'LibreOffice', csv: 'comma separated', tsv: 'tab separated', sql: 'INSERTs', xml: 'XML',
  json: 'JSON', yaml: 'YAML', toml: 'TOML', ndjson: 'lines',
  glb: 'web 3D', obj: 'Wavefront', stl: '3D print', ply: 'scans', off: 'OFF',
  zip: 'everywhere', '7z': 'strongest', 'tar.gz': 'Linux', 'tar.zst': 'fastest',
};
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

export class QuickConvert {
  constructor(ctx) {
    this.ctx = ctx;
    this.back = null;
    this.paths = [];
    this.files = [];
    this.to = null;
    this.values = {};
    this.run = null;
    const def = NODE_DEFS['convert.any'];
    for (const w of def.widgets) this.values[w.name] = w.value;
    this.dest = ctx.settings.qc_dest || 'next';
  }

  isOpen() {
    return !!this.back;
  }

  open(o = {}) {
    if (o.to) this.to = o.to;
    if (this.back) {
      if (o.paths) this.addPaths(o.paths);
      this.render();
      return;
    }
    this.back = el('div', { class: 'dialog-back' });
    this.box = el('div', { class: 'dialog qc' });
    this.back.append(this.box);
    this.back.addEventListener('pointerdown', (e) => {
      if (e.target === this.back && !this.run) this.close();
    });
    this.onKey = (e) => {
      if (e.key === 'Escape' && !this.run) {
        e.preventDefault();
        e.stopPropagation();
        this.close();
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
  }

  async addPaths(paths) {
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

  matching(to) {
    const tools = this.ctx.tools();
    return this.files.filter((f) => f.ext !== to && route(f, to, tools));
  }

  render() {
    if (!this.box) return;
    const keepScroll = this.body ? this.body.scrollTop : 0;
    const q = this.search ? this.search.value : '';
    this.box.textContent = '';
    this.box.append(this.left(), this.right(q));
    if (this.body) this.body.scrollTop = keepScroll;
  }

  left() {
    const files = el('div', { class: 'qc-files scroll' });
    if (!this.files.length) {
      files.append(el('div', { class: 'qc-empty' }, icon('upload', 30), el('div', { text: t('qc.drop') })));
    } else {
      const tools = this.ctx.tools();
      for (const f of this.files.slice(0, 400)) {
        const ok = this.to ? !!route(f, this.to, tools) && f.ext !== this.to : true;
        const st = this.result && this.result.done ? (ok ? el('span', { class: 'st ok', text: '✓' }) : el('span', { class: 'st', text: '–' })) : el('span', { class: 'st', text: fmtBytes(f.size || 0) });
        files.append(el('div', { class: 'qc-file', style: { opacity: ok ? '1' : '.45' }, title: f.path },
          el('span', { class: 'k-' + f.kind }, icon(kindIcon(f.kind), 14)), el('span', { class: 'nm', text: f.name }), st));
      }
      if (this.files.length > 400) files.append(el('div', { class: 'lib-hint', text: t('insp.more', { n: this.files.length - 400 }) }));
    }
    const bar = el('div', { class: 'wp-bar' },
      el('button', { class: 'btn sm', onclick: async () => this.addPaths(await pickFiles({ multiple: true })) }, icon('file-plus', 12), t('wid.add_files')),
      el('button', { class: 'btn sm', onclick: async () => this.addPaths(await pickFiles({ folder: true, multiple: true })) }, icon('folder', 12), t('wid.add_folder')),
      el('button', {
        class: 'btn sm ghost',
        onclick: () => {
          this.paths = [];
          this.files = [];
          this.result = null;
          this.render();
        },
      }, t('wid.clear')));
    const kinds = {};
    for (const f of this.files) kinds[f.kind] = (kinds[f.kind] || 0) + 1;
    const kc = el('div', { class: 'qc-kinds' }, Object.entries(kinds).map(([k, n]) => el('span', { style: { '--c': (CATEGORIES[k] || CATEGORIES.files).color }, text: n + ' × ' + k })));
    return el('div', { class: 'qc-left' },
      el('h3', {}, icon('bolt', 18), t('qc.title')),
      el('p', { text: t('qc.text') }),
      el('div', { class: 'qc-drop' }, files, bar),
      this.files.length ? kc : null);
  }

  right(q) {
    const input = el('input', { class: 'search', placeholder: t('qc.search'), spellcheck: 'false', value: q });
    this.search = input;
    input.addEventListener('input', () => this.fill());
    input.addEventListener('keydown', (e) => e.stopPropagation());
    const top = el('div', { class: 'qc-top' }, el('div', { class: 'search-wrap' }, input, icon('search', 14)),
      el('button', { class: 'icon-btn', title: t('dlg.cancel'), onclick: () => !this.run && this.close() }, icon('x', 16)));
    this.body = el('div', { class: 'qc-body scroll' });
    this.fill();
    return el('div', { class: 'qc-right' }, top, this.body, this.options(), this.footer());
  }

  fill() {
    const q = (this.search ? this.search.value : '').trim().toLowerCase().replace(/^\./, '');
    const body = this.body;
    body.textContent = '';
    const tools = this.ctx.tools();
    for (const [g, list] of Object.entries(CONVERT_TARGETS)) {
      const items = list.filter((x) => !q || x.includes(q) || (HINT[x] || '').toLowerCase().includes(q) || g.toLowerCase().includes(q));
      if (!items.length) continue;
      const cat = CATEGORIES[GROUP_CAT[g]] || CATEGORIES.convert;
      const grid = el('div', { class: 'qc-grid' });
      for (const x of items) {
        const n = this.files.length ? this.matching(x).length : 1;
        const need = this.files.length && n ? (() => {
          const r = route(this.matching(x)[0], x, tools);
          const tool = NEEDS[r];
          return tool && !tools[tool] ? tool : null;
        })() : null;
        const b = paint(el('button', {
          class: 'qc-fmt' + (this.to === x ? ' on' : '') + (n ? '' : ' off'),
          title: need ? t('qc.needs', { tool: need }) : this.files.length ? t('qc.match', { n, total: this.files.length, fmt: x }) : x,
          onclick: () => {
            this.to = x;
            this.result = null;
            this.render();
          },
        }, el('b', { text: x.toUpperCase() }), el('small', { text: need ? '⚠ ' + t('qc.needs', { tool: need }) : HINT[x] || g })), cat.color);
        grid.append(b);
      }
      body.append(paint(el('div', { class: 'qc-group' }, el('h5', {}, el('span', { class: 'ci' }, icon(cat.icon, 12)), groupName(g)), grid), cat.color));
    }
  }

  options() {
    const box = el('div', { class: 'qc-opts' });
    const def = NODE_DEFS['convert.any'];
    const g = this.to ? Object.entries(CONVERT_TARGETS).find(([, l]) => l.includes(this.to)) : null;
    const group = g ? g[0] : '';
    const kinds = new Set(this.files.map((f) => f.kind));
    const want = new Set(['quality']);
    if (group === 'Image' || kinds.has('image')) want.add('max_size');
    if (group === 'Image') want.add('lossless');
    if (this.to === 'pdf' && kinds.has('image')) want.add('combine');
    if (group === 'Video') ['video_codec', 'preset', 'hardware'].forEach((k) => want.add(k));
    if (group === 'Video' && kinds.has('image')) want.add('seconds');
    if ((this.to === 'gif' || this.to === 'webp') && kinds.has('video')) want.add('fps');
    if (group === 'Audio' || group === 'Video') want.add('audio_bitrate');
    if (this.to === 'html') want.add('theme');
    if (['png', 'jpg'].includes(this.to) && this.files.some((f) => f.ext === 'pdf')) want.add('dpi');
    for (const w of def.widgets) {
      if (!want.has(w.name)) continue;
      const we = createWidget({ ...w, adv: false }, this.values[w.name], (v) => {
        this.values[w.name] = v;
      });
      box.append(we);
    }
    return box;
  }

  footer() {
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
    const folderLbl = this.dest === 'folder' ? el('span', { class: 'muted', style: { fontSize: '11.5px', maxWidth: '200px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }, title: this.folderPath(), text: basename(this.folderPath()) || this.folderPath() }) : null;
    const n = this.to ? this.matching(this.to).length : 0;
    const foot = el('div', { class: 'qc-foot' }, segc, folderLbl);
    if (this.run) {
      foot.append(el('div', { class: 'qc-prog' }, el('div', { class: 'bar' }, el('i', { style: { width: Math.round((this.run.p || 0) * 100) + '%' } })), el('span', { text: this.run.text || t('qc.running') })));
      foot.append(el('button', { class: 'btn', onclick: () => invoke('cancel_run').catch(() => {}) }, icon('stop', 12), t('top.cancel')));
      return foot;
    }
    if (this.result) {
      const r = this.result;
      foot.append(el('div', { class: 'qc-prog' }, el('span', { style: { color: r.ok ? '#7fe39a' : '#ff9a9a', maxWidth: 'none' }, text: r.ok ? t('qc.done', { n: r.count, time: fmtTime(r.ms) }) : t('qc.failed') + ': ' + (r.error || '') })));
      if (r.ok && r.first) foot.append(el('button', { class: 'btn', onclick: () => this.ctx.reveal(r.first) }, icon('folder', 13), t('qc.open')));
      foot.append(el('button', { class: 'btn ghost', onclick: () => this.toWorkflow() }, icon('templates', 13), t('qc.edit')));
      return foot;
    }
    foot.append(el('div', { class: 'grow' }));
    if (this.to && this.files.length) foot.append(el('span', { class: 'muted', style: { fontSize: '11.5px' }, text: n ? t('qc.match', { n, total: this.files.length, fmt: this.to }) : t('qc.nomatch', { fmt: this.to }) }));
    foot.append(el('button', {
      class: 'btn primary lg',
      disabled: !this.files.length || !this.to || !n,
      onclick: () => this.start(),
    }, icon('bolt', 15), n ? t('qc.convert_n', { n }) : t('qc.convert')));
    return foot;
  }

  folderPath() {
    return this.folder || this.ctx.settings.output || '~/Morph Output';
  }

  graph() {
    const values = { ...this.values, to: this.to };
    const save = this.dest === 'folder'
      ? { destination: 'folder', folder: this.folderPath(), pattern: '{name}', on_conflict: 'rename', keep_structure: true, open_folder: false }
      : { destination: 'next to original', folder: '', pattern: '{name}', on_conflict: 'rename', keep_structure: true, open_folder: false };
    return [
      { type: 'files.load', values: { paths: this.paths, recursive: true, include: '', exclude: '', skip_hidden: true } },
      { type: 'convert.any', values },
      { type: 'files.save', values: save },
    ];
  }

  async start() {
    if (this.run || this.ctx.busy()) return;
    const g = this.graph();
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
    this.run = { p: 0, text: t('qc.running'), t0: performance.now() };
    this.ctx.claim(this);
    this.render();
    try {
      await invoke('run_graph', { graph: { nodes, settings: toolSettings() } });
    } catch (e) {
      this.finish({ ok: false, error: String((e && e.message) || e) });
    }
  }

  // Run events routed here by main.js while a quick run is active.
  onEvent(ev) {
    if (!this.run) return;
    if (ev.type === 'node_start') {
      const stage = { q1: 0, q2: 0.05, q3: 0.95 }[ev.node] || 0;
      this.run.p = stage;
    } else if (ev.type === 'node_progress' && ev.node === 'q2') {
      this.run.p = 0.05 + clamp(Number(ev.value) || 0, 0, 1) * 0.9;
      this.run.text = ev.text || this.run.text;
    } else if (ev.type === 'node_done' && ev.node === 'q3') {
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
    this.ctx.history({ name: t('qc.title') + ' → ' + (this.to || '').toUpperCase(), ok: this.result.ok, error: this.result.ok ? null : this.result.error, ms: this.result.ms, files: this.result.count, cancelled: !!ev.cancelled });
    this.render();
  }

  toWorkflow() {
    const g = this.graph();
    this.ctx.load({
      nodes: g.map((n, i) => ({ type: n.type, x: 40 + i * 380, y: 80, values: n.values })),
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }, t('qc.title') + ' → ' + (this.to || '').toUpperCase());
    this.close();
  }
}

function groupName(g) {
  const map = { Image: 'cat.image', Video: 'cat.video', Audio: 'cat.audio', Document: 'cat.doc', Spreadsheet: 'cat.sheet', Data: 'cat.data', '3D': 'cat.mesh', Archive: 'cat.archive' };
  return t(map[g] || g);
}
