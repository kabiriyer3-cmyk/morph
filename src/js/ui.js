// Small DOM helpers shared by the UI modules.
import { t } from './i18n.js';

export const BS = String.fromCharCode(92);
export const NL = String.fromCharCode(10);
export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
export const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

export function el(tag, attrs, ...children) {
  const e = document.createElement(tag);
  setAttrs(e, attrs);
  add(e, children);
  return e;
}

export function svgEl(tag, attrs) {
  const e = document.createElementNS('http://www.w3.org/2000/svg', tag);
  for (const [k, v] of Object.entries(attrs || {})) if (v != null) e.setAttribute(k, String(v));
  return e;
}

function setAttrs(e, attrs) {
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') e.className = v;
    else if (k === 'text') e.textContent = v;
    else if (k === 'html') e.innerHTML = v;
    else if (k === 'style') {
      for (const [p, val] of Object.entries(v)) {
        if (p.startsWith('--')) e.style.setProperty(p, String(val));
        else e.style[p] = val;
      }
    } else if (k === 'dataset') Object.assign(e.dataset, v);
    else if (k.startsWith('on') && typeof v === 'function') e.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === 'value' && 'value' in e) e.value = v;
    else e.setAttribute(k, v === true ? '' : String(v));
  }
}

function add(e, children) {
  for (const c of children.flat(Infinity)) {
    if (c == null || c === false) continue;
    e.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
}

const FILE = `<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5'/>`;
const FOLDER = `<path d='M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z'/>`;
const ICONS = {
  play: `<path d='M7 4.5v15l12-7.5z' fill='currentColor'/>`,
  stop: `<rect x='6' y='6' width='12' height='12' rx='2' fill='currentColor'/>`,
  plus: `<path d='M12 5v14M5 12h14'/>`,
  minus: `<path d='M5 12h14'/>`,
  x: `<path d='M6 6l12 12M18 6L6 18'/>`,
  check: `<path d='M5 12.5l4.5 4.5L19 7'/>`,
  'chevron-right': `<path d='M9 6l6 6-6 6'/>`,
  'chevron-left': `<path d='M15 6l-6 6 6 6'/>`,
  'chevron-down': `<path d='M6 9l6 6 6-6'/>`,
  file: FILE,
  'file-plus': FILE + `<path d='M12 11v6M9 14h6'/>`,
  folder: FOLDER,
  'folder-open': FOLDER + `<path d='M3 10.5h18'/>`,
  save: `<path d='M5 3h11l3 3v13a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2z'/><path d='M8 3v5h7V3M8 21v-7h8v7'/>`,
  undo: `<path d='M9 14L4 9l5-5'/><path d='M4 9h10.5a5.5 5.5 0 0 1 0 11H11'/>`,
  redo: `<path d='M15 14l5-5-5-5'/><path d='M20 9H9.5a5.5 5.5 0 0 0 0 11H13'/>`,
  fit: `<path d='M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4'/>`,
  arrange: `<rect x='3' y='4' width='7' height='6' rx='1.5'/><rect x='14' y='4' width='7' height='6' rx='1.5'/><rect x='14' y='14' width='7' height='6' rx='1.5'/><path d='M10 7h4M17.5 10v4'/>`,
  library: `<rect x='3' y='3' width='7' height='7' rx='1.5'/><rect x='14' y='3' width='7' height='7' rx='1.5'/><rect x='3' y='14' width='7' height='7' rx='1.5'/><path d='M17.5 14v7M14 17.5h7'/>`,
  templates: `<rect x='3' y='3' width='18' height='18' rx='2'/><path d='M3 9h18M9 21V9'/>`,
  history: `<circle cx='12' cy='12' r='9'/><path d='M12 7v5l3 2'/>`,
  settings: `<path d='M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1'/><circle cx='15' cy='6' r='2'/><circle cx='9' cy='12' r='2'/><circle cx='17' cy='18' r='2'/>`,
  help: `<circle cx='12' cy='12' r='9'/><path d='M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17v.5'/>`,
  info: `<circle cx='12' cy='12' r='9'/><path d='M12 11v6M12 7.5v.5'/>`,
  alert: `<path d='M12 3.5l9.5 16.5h-19z'/><path d='M12 10v4.5M12 17.5v.5'/>`,
  image: `<rect x='3' y='4' width='18' height='16' rx='2'/><circle cx='9' cy='10' r='2'/><path d='M21 16l-5-5-9 9'/>`,
  video: `<rect x='3' y='5' width='13' height='14' rx='2'/><path d='M16 10l5-3v10l-5-3z'/>`,
  audio: `<path d='M9 18V5l11-2v13'/><circle cx='6.5' cy='18' r='2.5'/><circle cx='17.5' cy='16' r='2.5'/>`,
  mesh: `<path d='M12 3l8 4.5v9L12 21l-8-4.5v-9z'/><path d='M4 7.5l8 4.5 8-4.5M12 12v9'/>`,
  archive: `<rect x='3' y='4' width='18' height='5' rx='1'/><path d='M5 9v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V9M10 13h4'/>`,
  data: `<path d='M8 4c-2 0-2 1.5-2 3v2c0 1.5-1 3-2 3 1 0 2 1.5 2 3v2c0 1.5 0 3 2 3M16 4c2 0 2 1.5 2 3v2c0 1.5 1 3 2 3-1 0-2 1.5-2 3v2c0 1.5 0 3-2 3'/>`,
  doc: FILE + `<path d='M9 13h6M9 17h6'/>`,
  text: `<path d='M4 6h16M4 11h16M4 16h10'/>`,
  files: `<path d='M15 3H8a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h9a2 2 0 0 0 2-2V7z'/><path d='M3 7v12a2 2 0 0 0 2 2h9'/>`,
  note: `<path d='M5 4h14v11l-5 5H5z'/><path d='M14 20v-5h5'/>`,
  eye: `<path d='M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z'/><circle cx='12' cy='12' r='3'/>`,
  trash: `<path d='M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3'/>`,
  copy: `<rect x='9' y='9' width='12' height='12' rx='2'/><path d='M5 15V5a2 2 0 0 1 2-2h8'/>`,
  external: `<path d='M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5'/>`,
  search: `<circle cx='11' cy='11' r='7'/><path d='M20 20l-4-4'/>`,
  upload: `<path d='M12 16V4M7 9l5-5 5 5M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3'/>`,
  download: `<path d='M12 4v12M7 11l5 5 5-5M4 20h16'/>`,
  bypass: `<circle cx='12' cy='12' r='9'/><path d='M5.6 5.6l12.8 12.8'/>`,
  git: `<circle cx='6' cy='6' r='2'/><circle cx='6' cy='18' r='2'/><circle cx='18' cy='8' r='2'/><path d='M6 8v8M18 10c0 4-6 3-10.5 6.5'/>`,
  globe: `<circle cx='12' cy='12' r='9'/><path d='M3 12h18M12 3c3 3.5 3 14.5 0 18M12 3c-3 3.5-3 14.5 0 18'/>`,
  wire: `<path d='M4 17c6 0 6-10 12-10h4'/><circle cx='4' cy='17' r='1.5'/>`,
};

export function icon(name, size = 16) {
  const tpl = document.createElement('template');
  tpl.innerHTML = `<svg class='ic' viewBox='0 0 24 24' width='${size}' height='${size}' fill='none' stroke='currentColor' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>${ICONS[name] || FILE}</svg>`;
  return tpl.content.firstElementChild;
}

const KIND_ICON = { image: 'image', video: 'video', audio: 'audio', mesh: 'mesh', archive: 'archive', data: 'data', doc: 'doc', text: 'text' };
export const kindIcon = (kind) => KIND_ICON[kind] || 'file';

export function fmtBytes(n) {
  let v = Number(n) || 0;
  if (v < 1024) return Math.round(v) + ' B';
  const u = ['KB', 'MB', 'GB', 'TB'];
  let i = -1;
  do {
    v /= 1024;
    i++;
  } while (v >= 1024 && i < u.length - 1);
  return (v >= 100 ? v.toFixed(0) : v >= 10 ? v.toFixed(1) : v.toFixed(2)) + ' ' + u[i];
}

export function fmtTime(ms) {
  const v = Number(ms) || 0;
  if (v < 1000) return Math.round(v) + ' ms';
  if (v < 60000) return (v / 1000).toFixed(1) + ' s';
  const m = Math.floor(v / 60000);
  const s = Math.round((v % 60000) / 1000);
  return m + ' min ' + String(s).padStart(2, '0') + ' s';
}

export function fmtDur(sec) {
  const v = Math.max(0, Number(sec) || 0);
  const h = Math.floor(v / 3600);
  const m = Math.floor((v % 3600) / 60);
  const s = Math.floor(v % 60);
  const mm = h ? String(m).padStart(2, '0') : String(m);
  return (h ? h + ':' : '') + mm + ':' + String(s).padStart(2, '0');
}

export function fmtPct(out, orig) {
  if (!orig || out == null) return null;
  const p = ((out - orig) / orig) * 100;
  const a = Math.abs(p);
  const text = a < 0.5 ? '0%' : (p > 0 ? '+' : '−') + a.toFixed(a < 10 ? 1 : 0) + '%';
  return { text, cls: p < -0.5 ? 'good' : p > 0.5 ? 'bad' : '' };
}

const norm = (p) => String(p || '').split(BS).join('/');
export function basename(p) {
  const s = norm(p);
  const t2 = s.endsWith('/') ? s.slice(0, -1) : s;
  return t2.slice(t2.lastIndexOf('/') + 1);
}
export function dirname(p) {
  const s = norm(p);
  const i = s.lastIndexOf('/');
  return i > 0 ? s.slice(0, i) : '';
}
export function extOf(p) {
  const b = basename(p);
  const i = b.lastIndexOf('.');
  return i > 0 ? b.slice(i + 1).toLowerCase() : '';
}

export function hashStr(s) {
  let h = 2166136261;
  const str = String(s);
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

export function debounce(fn, ms = 200) {
  let tm = null;
  let args = [];
  const d = (...a) => {
    args = a;
    clearTimeout(tm);
    tm = setTimeout(() => {
      tm = null;
      fn(...args);
    }, ms);
  };
  d.flush = () => {
    if (tm) {
      clearTimeout(tm);
      tm = null;
      fn(...args);
    }
  };
  d.cancel = () => {
    clearTimeout(tm);
    tm = null;
  };
  return d;
}

let toastBox = null;
export function toast(msg, kind = 'info', ms = 3500) {
  if (!toastBox) {
    toastBox = el('div', { class: 'toasts' });
    document.body.append(toastBox);
  }
  const n = el('div', { class: 'toast ' + kind, text: String(msg) });
  n.addEventListener('click', () => n.remove());
  toastBox.append(n);
  while (toastBox.children.length > 4) toastBox.firstChild.remove();
  setTimeout(() => n.remove(), kind === 'error' ? Math.max(ms, 8000) : ms);
}

let menus = [];
function closeFrom(depth) {
  menus.filter((m) => Number(m.dataset.depth) >= depth).forEach((m) => m.remove());
  menus = menus.filter((m) => Number(m.dataset.depth) < depth);
}
function outside(e) {
  if (!menus.some((m) => m.contains(e.target))) closeMenu();
}
function escKey(e) {
  if (e.key === 'Escape') closeMenu();
}
export function closeMenu() {
  closeFrom(0);
  document.removeEventListener('pointerdown', outside, true);
  document.removeEventListener('keydown', escKey, true);
}
function place(m, x, y, flipX) {
  const r = m.getBoundingClientRect();
  let left = x;
  let top = y;
  if (left + r.width > innerWidth - 4) left = flipX != null ? flipX - r.width : innerWidth - r.width - 4;
  if (top + r.height > innerHeight - 4) top = innerHeight - r.height - 4;
  m.style.left = Math.max(4, left) + 'px';
  m.style.top = Math.max(4, top) + 'px';
}
function build(items, depth, opts) {
  const m = el('div', { class: 'menu', dataset: { depth: String(depth) } });
  if (opts.minWidth) m.style.minWidth = Math.round(opts.minWidth) + 'px';
  for (const it of items) {
    if (!it) continue;
    if (it.sep) {
      m.append(el('div', { class: 'menu-sep' }));
      continue;
    }
    if (it.header) {
      m.append(el('div', { class: 'menu-head', text: it.header }));
      continue;
    }
    const cls = ['menu-item', it.disabled && 'disabled', it.danger && 'danger'].filter(Boolean).join(' ');
    const row = el('div', { class: cls },
      el('span', { class: 'mi-check' }, it.checked ? icon('check', 14) : it.icon ? icon(it.icon, 14) : null),
      it.dot ? el('span', { class: 'dot', style: { '--c': it.dot } }) : null,
      el('span', { class: 'mi-label', text: it.label }),
      it.hint ? el('span', { class: 'mi-hint', text: it.hint }) : null,
      it.sub ? icon('chevron-right', 14) : null);
    row.addEventListener('pointerenter', () => {
      closeFrom(depth + 1);
      if (!it.sub || it.disabled) return;
      const sub = build(it.sub, depth + 1, {});
      document.body.append(sub);
      menus.push(sub);
      const r = row.getBoundingClientRect();
      place(sub, r.right + 2, r.top - 5, r.left - 2);
    });
    if (!it.sub) {
      row.addEventListener('click', () => {
        if (it.disabled) return;
        closeMenu();
        if (it.onClick) it.onClick();
      });
    }
    m.append(row);
  }
  return m;
}
export function menu(items, x, y, opts = {}) {
  closeMenu();
  const m = build(items, 0, opts);
  document.body.append(m);
  menus.push(m);
  place(m, x, y);
  setTimeout(() => {
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('keydown', escKey, true);
  }, 0);
  return m;
}

export function dialog({ title, text, input = false, value = '', okText, danger = false, cancel = true, body = null, wide = false } = {}) {
  return new Promise((resolve) => {
    const field = input ? el('input', { class: 'field', value, spellcheck: 'false' }) : null;
    const back = el('div', { class: 'dialog-back' });
    let onKey = null;
    const done = (v) => {
      document.removeEventListener('keydown', onKey, true);
      back.remove();
      resolve(v);
    };
    const ok = () => done(field ? field.value : true);
    onKey = (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        done(null);
      } else if (e.key === 'Enter' && !(e.target && e.target.tagName === 'TEXTAREA')) {
        e.preventDefault();
        e.stopPropagation();
        ok();
      }
    };
    const okBtn = el('button', { class: 'btn ' + (danger ? 'danger' : 'primary'), text: okText || t('dlg.ok'), onclick: ok });
    const box = el('div', { class: 'dialog' + (wide ? ' help' : '') },
      title ? el('h3', { text: title }) : null,
      text ? el('p', { text }) : null,
      body,
      field,
      el('div', { class: 'dlg-actions' },
        cancel ? el('button', { class: 'btn ghost', text: t('dlg.cancel'), onclick: () => done(null) }) : null,
        okBtn));
    back.append(box);
    back.addEventListener('pointerdown', (e) => {
      if (e.target === back) done(null);
    });
    document.body.append(back);
    document.addEventListener('keydown', onKey, true);
    if (field) {
      field.focus();
      field.select();
    } else okBtn.focus();
  });
}
