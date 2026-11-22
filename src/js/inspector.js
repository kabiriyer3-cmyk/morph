// Right panel: node details, output lists and rich file previews.
import { el, icon, kindIcon, fmtBytes, fmtTime, fmtPct, fmtDur, dirname, extOf, toast } from './ui.js';
import { t, nodeTitle, nodeDesc, sockLabel, catLabel } from './i18n.js';
import { invoke, fileSrc, loadThumb, DEMO, toolSettings } from './api.js';
import { createViewer } from './viewer3d.js';
import { CATEGORIES } from './nodes.js';

const WEB_IMG = new Set(['jpg', 'jpeg', 'png', 'webp', 'gif', 'svg', 'bmp', 'avif', 'ico']);
const WEB_VID = new Set(['mp4', 'webm', 'mov', 'm4v', 'ogv']);
const WEB_AUD = new Set(['mp3', 'm4a', 'aac', 'ogg', 'oga', 'opus', 'wav', 'flac']);
const TOOL_NAMES = { ffmpeg: 'FFmpeg', soffice: 'LibreOffice', pandoc: 'Pandoc' };

export class Inspector {
  constructor(root, ctx) {
    this.root = root;
    this.ctx = ctx;
    this.viewer = null;
    this.token = 0;
    this.current = null;
    this.showEmpty();
  }

  reset() {
    this.token++;
    if (this.viewer) {
      try {
        this.viewer.dispose();
      } catch {
        // ignore
      }
      this.viewer = null;
    }
    this.root.textContent = '';
    this.root.scrollTop = 0;
    return this.token;
  }

  showEmpty() {
    this.reset();
    this.current = null;
    const tips = [['upload', 'insp.tip1'], ['wire', 'insp.tip2'], ['search', 'insp.tip3'], ['play', 'insp.tip4']];
    this.root.append(el('div', { class: 'insp-empty' },
      el('img', { src: 'assets/logo.svg', alt: '' }),
      el('div', { text: t('insp.empty') }),
      el('ul', {}, tips.map(([ic, k]) => el('li', {}, icon(ic, 14), el('span', { text: t(k) }))))));
  }

  refresh() {
    const c = this.current;
    if (!c || c.kind !== 'node') return;
    const n = this.ctx.editor.get(c.id);
    if (n) this.showNode(n);
    else this.showEmpty();
  }

  showNode(n) {
    this.reset();
    this.current = { kind: 'node', id: n.id };
    const def = n.def;
    const cat = CATEGORIES[def.cat] || CATEGORIES.util;
    this.root.append(el('div', { class: 'insp-head' },
      el('span', { class: 'dot', style: { '--c': cat.color } }),
      el('div', { class: 't' }, el('b', { text: n.title || nodeTitle(n.type) }), el('small', { text: catLabel(def.cat) + ' · ' + n.type }))));
    const about = el('div', { class: 'insp-sec' }, el('p', { text: nodeDesc(n.type) }));
    if (def.needs && this.ctx.missing && this.ctx.missing().has(def.needs)) {
      about.append(el('div', { class: 'box warn', style: { marginTop: '10px' }, text: t('insp.needs', { tool: TOOL_NAMES[def.needs] || def.needs }) }));
    }
    this.root.append(about);
    if (n.error) this.root.append(el('div', { class: 'insp-sec' }, el('h4', { text: t('insp.error') }), el('div', { class: 'box err', text: n.error })));
    const ev = n.result;
    const warnings = (ev && ev.warnings) || n.warnings || [];
    if (warnings.length) {
      this.root.append(el('div', { class: 'insp-sec' },
        el('h4', {}, t('insp.warnings'), el('span', { text: String(warnings.length) })),
        el('ul', { class: 'warn-list scroll' }, warnings.slice(0, 100).map((w) => el('li', { text: w })))));
    }
    if (!ev) {
      if (!def.frontendOnly && !n.error) this.root.append(el('div', { class: 'insp-sec' }, el('p', { class: 'muted', text: t('insp.no_result') })));
      return;
    }
    (ev.outputs || []).forEach((list, i) => {
      const s = def.outputs[i];
      const count = (ev.counts || [])[i] || 0;
      const total = list.reduce((a, f) => a + (f.size || 0), 0);
      const head = def.outputs.length > 1 ? sockLabel(s ? s.name : String(i)) : t('insp.outputs');
      const rows = el('div', { class: 'frows scroll' });
      const shown = list.slice(0, 300);
      for (const f of shown) rows.append(this.fileRow(f, n));
      if (count > shown.length) rows.append(el('div', { class: 'frow muted', text: t('insp.more', { n: count - shown.length }) }));
      this.root.append(el('div', { class: 'insp-sec' }, el('h4', {}, head, el('span', { text: t('nd.files', { n: count }) + ' · ' + fmtBytes(total) })), rows));
    });
    this.root.append(el('div', { class: 'insp-sec' }, el('p', { class: 'muted', text: t('insp.time') + ': ' + fmtTime(ev.ms || 0) })));
  }

  fileRow(f, n) {
    const p = f.origin && f.path !== f.origin && f.origin_size ? fmtPct(f.size, f.origin_size) : null;
    return el('div', { class: 'frow', title: f.path, onclick: () => this.showFile(f, n) },
      el('span', { class: 'k-' + f.kind }, icon(kindIcon(f.kind), 14)),
      el('span', { class: 'nm', text: (f.rel ? f.rel + '/' : '') + f.name }),
      el('span', { class: 'sz', text: fmtBytes(f.size) }),
      p ? el('span', { class: 'pc ' + p.cls, text: p.text }) : null);
  }

  showFile(f, n) {
    const tok = this.reset();
    this.current = { kind: 'file', path: f.path, node: n ? n.id : null };
    const head = el('div', { class: 'insp-head' });
    if (n) {
      head.append(el('button', {
        class: 'icon-btn',
        title: t('insp.back'),
        onclick: () => {
          const m = this.ctx.editor.get(n.id);
          if (m) this.showNode(m);
          else this.showEmpty();
        },
      }, icon('chevron-left', 16)));
    }
    head.append(el('span', { class: 'k-' + f.kind }, icon(kindIcon(f.kind), 18)),
      el('div', { class: 't' }, el('b', { text: f.name, title: f.name }), el('small', { text: f.rel || dirname(f.path), title: f.path })));
    this.root.append(head);
    const pv = el('div', { class: 'pv' }, el('div', { class: 'pv-msg', text: t('insp.loading') }));
    this.root.append(pv);
    this.root.append(el('div', { class: 'insp-sec' }, el('div', { class: 'actions' },
      el('button', { class: 'btn sm', onclick: () => this.ctx.open(f.path) }, icon('external', 12), t('insp.open')),
      el('button', { class: 'btn sm', onclick: () => this.ctx.reveal(f.path) }, icon('folder', 12), t('insp.reveal')),
      el('button', { class: 'btn sm ghost', onclick: () => this.copy(f.path) }, icon('copy', 12), t('insp.copy_path')))));
    const meta = el('table', { class: 'kv' });
    this.root.append(el('div', { class: 'insp-sec' }, el('h4', { text: t('insp.details') }), meta));
    const kv = (k, v) => {
      if (v == null || v === '') return;
      meta.append(el('tr', {}, el('td', { text: k }), el('td', { text: String(v) })));
    };
    kv(t('insp.type'), f.kind + (f.ext ? ' · .' + f.ext : ''));
    const changed = f.origin && f.path !== f.origin && f.origin_size;
    kv(t('insp.size'), fmtBytes(f.size) + (changed ? '   (' + t('insp.before') + ': ' + fmtBytes(f.origin_size) + ')' : ''));
    kv(t('insp.path'), f.path);
    this.renderPreview(f, pv, tok).catch((e) => {
      if (tok !== this.token) return;
      pv.textContent = '';
      pv.append(el('div', { class: 'pv-msg', text: String((e && e.message) || e) }));
    });
    invoke('probe', { path: f.path, settings: toolSettings() })
      .then((p) => {
        if (tok === this.token && p) this.fillMeta(p, kv);
      })
      .catch(() => {});
  }

  async imageSrc(f) {
    if (DEMO) return fileSrc(f.path);
    if (WEB_IMG.has((f.ext || '').toLowerCase()) && (f.size || 0) < 40e6) return fileSrc(f.path);
    const r = await invoke('thumbnail', { path: f.path, size: 1024, settings: toolSettings() }).catch(() => null);
    return (r && r.thumb) || null;
  }

  async renderPreview(f, pv, tok) {
    const ext = (f.ext || '').toLowerCase();
    const live = () => tok === this.token;
    const set = (...nodes) => {
      if (!live()) return false;
      pv.textContent = '';
      pv.append(...nodes);
      return true;
    };
    const msg = (k) => el('div', { class: 'pv-msg' }, icon(kindIcon(f.kind), 30), el('div', { text: t(k) }));

    if (f.kind === 'image') {
      const src = await this.imageSrc(f);
      if (!src) return set(msg('insp.no_preview'));
      if (f.origin && f.origin !== f.path) {
        const before = await this.imageSrc({ ...f, path: f.origin, ext: extOf(f.origin), size: f.origin_size });
        if (before) return set(this.compare(before, src, f));
      }
      return set(el('img', { src, alt: f.name }));
    }
    if (f.kind === 'video') {
      const th = await loadThumb(f, 512);
      if (!live()) return false;
      if (!DEMO && WEB_VID.has(ext)) {
        const v = el('video', { controls: true, preload: 'metadata', src: fileSrc(f.path) });
        if (th) v.poster = th;
        v.onerror = () => {
          if (th && v.isConnected) v.replaceWith(el('img', { src: th, alt: '' }));
        };
        return set(v);
      }
      return set(th ? el('img', { src: th, alt: '' }) : msg('insp.no_preview'));
    }
    if (f.kind === 'audio') {
      const th = await loadThumb(f, 512);
      const wrap = el('div', { style: { width: '100%' } });
      if (th) wrap.append(el('img', { src: th, alt: '', style: { width: '100%', display: 'block' } }));
      if (!DEMO && WEB_AUD.has(ext)) wrap.append(el('audio', { controls: true, preload: 'metadata', src: fileSrc(f.path) }));
      if (!wrap.childNodes.length) wrap.append(msg('insp.no_preview'));
      return set(wrap);
    }
    if (f.kind === 'mesh') {
      const data = await invoke('mesh_preview', { path: f.path });
      const host = el('div', { style: { width: '100%' } });
      if (!set(host)) return false;
      try {
        this.viewer = createViewer(host, data);
      } catch {
        host.append(msg('insp.webgl'));
        return true;
      }
      let on = false;
      const wb = el('button', {
        class: 'btn sm',
        onclick: () => {
          on = !on;
          wb.classList.toggle('on', on);
          if (this.viewer) this.viewer.setWire(on);
        },
      }, t('insp.wire'));
      pv.append(el('div', { class: 'pv-tools' }, wb, el('button', { class: 'btn sm', onclick: () => this.viewer && this.viewer.reset() }, t('insp.reset'))));
      if (data.simplified) pv.append(el('div', { class: 'lbl-simpl', text: t('insp.simplified') }));
      return true;
    }
    if (f.kind === 'archive') {
      const r = await invoke('list_archive', { path: f.path });
      const entries = (r && r.entries) || [];
      const list = el('div', { class: 'scroll', style: { maxHeight: '380px', overflow: 'auto', padding: '0 14px 10px' } });
      for (const x of entries.slice(0, 500)) list.append(el('div', { class: 'ent' }, el('span', { text: x.name }), el('span', { text: x.dir ? '' : fmtBytes(x.size) })));
      const total = r && r.total != null ? r.total : entries.length;
      return set(el('div', { style: { width: '100%' } }, el('div', { class: 'muted', style: { padding: '10px 14px 6px', fontSize: '11.5px' }, text: t('insp.entries', { n: total }) }), list));
    }
    const texty = f.kind === 'data' || f.kind === 'text' || ['md', 'html', 'htm', 'xml', 'log', 'ini', 'cfg'].includes(ext);
    if (texty || (f.kind === 'other' && (f.size || 0) < 4e6)) {
      const r = await invoke('read_text', { path: f.path, limit: 200000 });
      let text = (r && r.text) || '';
      if (!texty) {
        let bad = 0;
        const lim = Math.min(text.length, 4000);
        for (let i = 0; i < lim; i++) {
          const c = text.charCodeAt(i);
          if (c === 0 || c === 65533) bad++;
        }
        if (bad > 8) return set(msg('insp.no_preview'));
      }
      if (ext === 'json') {
        try {
          text = JSON.stringify(JSON.parse(text), null, 2);
        } catch {
          // keep raw text
        }
      }
      return set(el('div', { style: { width: '100%', padding: '10px' } },
        el('pre', { class: 'code scroll', text: text.slice(0, 200000) }),
        r && r.truncated ? el('div', { class: 'muted', style: { paddingTop: '6px', fontSize: '11px' }, text: t('insp.truncated') }) : null));
    }
    return set(msg('insp.no_preview'));
  }

  compare(before, after, f) {
    const wrap = el('div', { class: 'compare' });
    const range = el('input', { type: 'range', min: '0', max: '100', value: '50' });
    wrap.append(
      el('img', { src: before, alt: '' }),
      el('div', { class: 'after' }, el('img', { src: after, alt: '' })),
      el('div', { class: 'bar' }),
      el('span', { class: 'lbl l', text: t('insp.before') + ' · ' + fmtBytes(f.origin_size) }),
      el('span', { class: 'lbl r', text: t('insp.after') + ' · ' + fmtBytes(f.size) }),
      range);
    range.addEventListener('input', () => wrap.style.setProperty('--cut', range.value + '%'));
    return wrap;
  }

  fillMeta(p, kv) {
    if (p.modified) kv(t('insp.modified'), p.modified);
    if (p.width && p.height) kv(t('insp.dimensions'), p.width + ' × ' + p.height);
    const m = p.media;
    if (m) {
      const fm = m.format || {};
      if (fm.format_long_name) kv(t('insp.format'), fm.format_long_name);
      if (fm.duration) kv(t('insp.duration'), fmtDur(Number(fm.duration)));
      if (fm.bit_rate) kv(t('insp.bitrate'), Math.round(Number(fm.bit_rate) / 1000) + ' kbps');
      (m.streams || []).forEach((s, i) => {
        let d = s.codec_name || '?';
        if (s.codec_type === 'video') {
          if (s.width) d += ' · ' + s.width + '×' + s.height;
          const [a, b] = String(s.r_frame_rate || '').split('/').map(Number);
          if (a && b) d += ' · ' + Math.round((a / b) * 100) / 100 + ' fps';
        } else if (s.codec_type === 'audio') {
          if (s.sample_rate) d += ' · ' + Math.round(Number(s.sample_rate) / 100) / 10 + ' kHz';
          if (s.channels) d += ' · ' + s.channels + ' ch';
        }
        kv('#' + i + ' ' + (s.codec_type || ''), d);
      });
    }
    if (p.mesh && typeof p.mesh === 'object') {
      for (const [k, v] of Object.entries(p.mesh)) kv(k.split('_').join(' '), typeof v === 'object' ? JSON.stringify(v) : v);
    }
  }

  copy(path) {
    if (!navigator.clipboard) return;
    navigator.clipboard.writeText(path).then(() => toast(t('insp.copied'), 'ok', 1500)).catch(() => {});
  }
}
