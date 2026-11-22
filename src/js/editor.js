// Node graph editor: DOM nodes on a pannable, zoomable canvas with SVG links drawn in screen space.
import { NODE_DEFS, CATEGORIES, typeColor, compatible } from './nodes.js';
import { createWidget, isVisible } from './widgets.js';
import { el, svgEl, icon, kindIcon, fmtBytes, fmtTime, fmtPct, clamp, debounce, toast, NL } from './ui.js';
import { tint } from './colors.js';
import { t, nodeTitle, sockLabel, groupLabel } from './i18n.js';
import { loadThumb, invoke } from './api.js';

// Must match .node-head / .node-slots / .slot-row in app.css.
const HEAD = 36;
const SLOT_TOP = 6;
const ROW = 24;
const GRID = 10;
const PREVIEW_MAX = 48;

const clone = (v) => (v === undefined ? v : JSON.parse(JSON.stringify(v)));
const isTyping = (e) => {
  const tg = e.target;
  return !!tg && (tg.tagName === 'INPUT' || tg.tagName === 'TEXTAREA' || tg.tagName === 'SELECT' || tg.isContentEditable);
};

function curve(a, b, z) {
  const dx = Math.max(40 * z, Math.abs(b.x - a.x) * 0.5);
  const f = (n) => n.toFixed(1);
  return `M${f(a.x)},${f(a.y)} C${f(a.x + dx)},${f(a.y)} ${f(b.x - dx)},${f(b.y)} ${f(b.x)},${f(b.y)}`;
}

const sock = (dir, i, type) => el('span', { class: 'sock', title: type, dataset: { dir, i: String(i) }, style: { '--t': typeColor(type) } });

export class Editor {
  constructor(root, opts = {}) {
    this.root = root;
    this.opts = opts;
    this.nodes = new Map();
    this.links = new Map();
    this.sel = new Set();
    this.nextId = 1;
    this.nextLink = 1;
    this.view = { x: 0, y: 0, z: 1 };
    this.snap = opts.snap !== false;
    this.history = [];
    this.future = [];
    this.snapshot = null;
    this.muted = 0;
    this.drag = null;
    this.space = false;
    this.mouse = null;
    this.clip = null;
    this.missing = new Set();
    this.svg = svgEl('svg', { class: 'links' });
    this.world = el('div', { class: 'world' });
    this.over = svgEl('svg', { class: 'links drag' });
    this.temp = svgEl('path', { class: 'link temp' });
    this.temp.style.display = 'none';
    this.over.append(this.temp);
    this.box = el('div', { class: 'select-box', hidden: true });
    root.append(this.svg, this.world, this.over, this.box);
    this.commitSoon = debounce(() => this.commit(), 300);
    this.bind();
    this.applyView();
  }

  // ------------------------------------------------------------ events
  bind() {
    const r = this.root;
    r.addEventListener('pointerdown', (e) => this.onDown(e));
    r.addEventListener('pointermove', (e) => {
      this.mouse = { x: e.clientX, y: e.clientY };
      if (this.drag) this.onMove(e);
    });
    r.addEventListener('pointerup', (e) => this.onUp(e));
    r.addEventListener('pointercancel', (e) => this.onUp(e));
    r.addEventListener('wheel', (e) => this.onWheel(e), { passive: false });
    r.addEventListener('dblclick', (e) => this.onDbl(e));
    r.addEventListener('contextmenu', (e) => this.onContext(e));
    window.addEventListener('keydown', (e) => {
      if (e.code === 'Space' && !isTyping(e)) this.space = true;
    });
    window.addEventListener('keyup', (e) => {
      if (e.code === 'Space') this.space = false;
    });
    window.addEventListener('blur', () => {
      this.space = false;
    });
    new ResizeObserver(() => this.redraw()).observe(r);
  }

  begin(e, d) {
    this.drag = d;
    try {
      this.root.setPointerCapture(e.pointerId);
    } catch {
      // ignore
    }
    if (d.mode === 'pan') this.root.classList.add('panning');
  }

  onDown(e) {
    if (this.opts.onPointerDown) this.opts.onPointerDown();
    const tg = e.target;
    const nodeEl = tg.closest('.node');
    const n = nodeEl ? this.nodes.get(Number(nodeEl.dataset.id)) : null;
    if (e.button === 1 || (e.button === 0 && this.space)) {
      e.preventDefault();
      this.begin(e, { mode: 'pan', x: e.clientX, y: e.clientY, vx: this.view.x, vy: this.view.y });
      return;
    }
    if (e.button !== 0) return;
    if (tg.closest('.w, .tile, button, input, textarea, select, [contenteditable=true]')) {
      if (n && !this.sel.has(n.id)) this.select(n.id);
      return;
    }
    const ae = document.activeElement;
    if (ae && ae !== document.body && ae.blur && (ae.tagName === 'INPUT' || ae.tagName === 'TEXTAREA' || ae.isContentEditable)) ae.blur();
    const sk = tg.closest('.sock');
    if (sk && n) {
      e.preventDefault();
      this.beginLink(e, n, sk.dataset.dir, Number(sk.dataset.i));
      return;
    }
    if (n && tg.closest('.resize')) {
      e.preventDefault();
      this.begin(e, { mode: 'resize', id: n.id, x: e.clientX, w: n.w });
      return;
    }
    if (n) {
      e.preventDefault();
      if (e.shiftKey || e.ctrlKey || e.metaKey) this.toggleSel(n.id);
      else if (!this.sel.has(n.id)) this.select(n.id);
      else this.primary(n.id);
      if (!this.sel.has(n.id)) return;
      const starts = [...this.sel].map((id) => {
        const m = this.nodes.get(id);
        return { id, x: m.x, y: m.y };
      });
      this.begin(e, { mode: 'move', x: e.clientX, y: e.clientY, starts, moved: false });
      return;
    }
    if (tg.closest('[data-link]')) return;
    e.preventDefault();
    if (this.root.focus) this.root.focus({ preventScroll: true });
    if (e.shiftKey || e.ctrlKey || e.metaKey) {
      const p = this.rel(e);
      this.begin(e, { mode: 'box', x0: p.x, y0: p.y });
      return;
    }
    this.begin(e, { mode: 'pan', x: e.clientX, y: e.clientY, vx: this.view.x, vy: this.view.y, click: true });
  }

  onMove(e) {
    const d = this.drag;
    const z = this.view.z;
    if (d.mode === 'pan') {
      const dx = e.clientX - d.x;
      const dy = e.clientY - d.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) d.moved = true;
      this.view.x = d.vx + dx;
      this.view.y = d.vy + dy;
      this.applyView();
    } else if (d.mode === 'move') {
      const dx = (e.clientX - d.x) / z;
      const dy = (e.clientY - d.y) / z;
      if (!d.moved && Math.abs(dx) + Math.abs(dy) < 3 / z) return;
      d.moved = true;
      for (const s of d.starts) {
        const n = this.nodes.get(s.id);
        if (!n) continue;
        let x = s.x + dx;
        let y = s.y + dy;
        if (this.snap && !e.altKey) {
          x = Math.round(x / GRID) * GRID;
          y = Math.round(y / GRID) * GRID;
        }
        n.x = x;
        n.y = y;
        this.place(n);
      }
      this.redraw();
    } else if (d.mode === 'resize') {
      const n = this.nodes.get(d.id);
      if (!n) return;
      n.w = clamp(Math.round(d.w + (e.clientX - d.x) / z), 200, 900);
      this.place(n);
      this.redraw();
    } else if (d.mode === 'link') {
      d.cur = this.rel(e);
      this.drawTemp();
    } else if (d.mode === 'box') {
      const p = this.rel(e);
      d.x1 = p.x;
      d.y1 = p.y;
      Object.assign(this.box.style, {
        left: Math.min(d.x0, p.x) + 'px',
        top: Math.min(d.y0, p.y) + 'px',
        width: Math.abs(p.x - d.x0) + 'px',
        height: Math.abs(p.y - d.y0) + 'px',
      });
      this.box.hidden = false;
    }
  }

  onUp(e) {
    const d = this.drag;
    if (!d) return;
    this.drag = null;
    try {
      this.root.releasePointerCapture(e.pointerId);
    } catch {
      // ignore
    }
    this.root.classList.remove('panning');
    if (d.mode === 'pan') {
      if (d.click && !d.moved) this.clearSel();
    } else if (d.mode === 'move') {
      if (d.moved) this.changed();
    } else if (d.mode === 'resize') {
      this.changed();
    } else if (d.mode === 'box') {
      this.box.hidden = true;
      if (d.x1 != null) this.boxSelect(d);
    } else if (d.mode === 'link') {
      this.endLink(e, d);
    }
  }

  onWheel(e) {
    const sc = e.target.closest ? e.target.closest('.scroll') : null;
    if (sc && !e.ctrlKey && sc.scrollHeight > sc.clientHeight + 1) return;
    e.preventDefault();
    const b = this.root.getBoundingClientRect();
    const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
    this.zoomAt(e.clientX - b.left, e.clientY - b.top, Math.exp(-dy * 0.0015));
  }

  onDbl(e) {
    const tg = e.target;
    const title = tg.closest('.title');
    if (title) {
      const n = this.nodes.get(Number(title.closest('.node').dataset.id));
      if (n) this.startRename(n.id);
      return;
    }
    if (tg.closest('.node') || tg.closest('[data-link]')) return;
    if (this.opts.onSearch) this.opts.onSearch(e.clientX, e.clientY, { world: this.toWorld(e.clientX, e.clientY) });
  }

  onContext(e) {
    const tg = e.target;
    if (tg.closest('input, textarea')) return;
    e.preventDefault();
    const cb = this.opts.onContextMenu;
    if (!cb) return;
    const world = this.toWorld(e.clientX, e.clientY);
    const nodeEl = tg.closest('.node');
    if (nodeEl) {
      const id = Number(nodeEl.dataset.id);
      if (!this.sel.has(id)) this.select(id);
      cb(e, { kind: 'node', id, world });
      return;
    }
    const lk = tg.closest('[data-link]');
    if (lk) {
      cb(e, { kind: 'link', id: Number(lk.getAttribute('data-link')), world });
      return;
    }
    cb(e, { kind: 'canvas', world });
  }

  // ------------------------------------------------------------ geometry
  rel(e) {
    const b = this.root.getBoundingClientRect();
    return { x: e.clientX - b.left, y: e.clientY - b.top };
  }
  screenToWorld(sx, sy) {
    return { x: (sx - this.view.x) / this.view.z, y: (sy - this.view.y) / this.view.z };
  }
  toWorld(cx, cy) {
    const b = this.root.getBoundingClientRect();
    return this.screenToWorld(cx - b.left, cy - b.top);
  }
  centerWorld() {
    const b = this.root.getBoundingClientRect();
    return this.screenToWorld(b.width / 2, b.height / 2);
  }
  mouseWorld() {
    const b = this.root.getBoundingClientRect();
    const m = this.mouse;
    if (m && m.x >= b.left && m.x <= b.right && m.y >= b.top && m.y <= b.bottom) return this.toWorld(m.x, m.y);
    return this.centerWorld();
  }
  screen(p) {
    return { x: this.view.x + p.x * this.view.z, y: this.view.y + p.y * this.view.z };
  }
  nodeW(n) {
    return n.collapsed ? Math.min(n.w, 220) : n.w;
  }
  sockPos(n, dir, i) {
    return {
      x: n.x + (dir === 'out' ? this.nodeW(n) : 0),
      y: n.y + (n.collapsed ? HEAD / 2 : HEAD + SLOT_TOP + ROW / 2 + i * ROW),
    };
  }

  applyView() {
    const { x, y, z } = this.view;
    this.world.style.transform = `translate(${x}px, ${y}px) scale(${z})`;
    const s = this.root.style;
    s.setProperty('--g1', 20 * z + 'px');
    s.setProperty('--g2', 100 * z + 'px');
    s.setProperty('--gx', x + 'px');
    s.setProperty('--gy', y + 'px');
    this.redraw();
    if (this.opts.onView) this.opts.onView(this.view);
  }

  zoomAt(cx, cy, f) {
    const z0 = this.view.z;
    const z1 = clamp(z0 * f, 0.15, 2.5);
    if (z1 === z0) return;
    this.view.x = cx - (cx - this.view.x) * (z1 / z0);
    this.view.y = cy - (cy - this.view.y) * (z1 / z0);
    this.view.z = z1;
    this.applyView();
  }

  zoomBy(f) {
    const b = this.root.getBoundingClientRect();
    this.zoomAt(b.width / 2, b.height / 2, f);
  }

  fit() {
    const list = [...this.nodes.values()];
    const b = this.root.getBoundingClientRect();
    if (!list.length || !b.width || !b.height) {
      this.view = { x: Math.round((b.width || 800) / 2 - 140), y: Math.round((b.height || 600) / 3), z: 1 };
      this.applyView();
      return;
    }
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const n of list) {
      x0 = Math.min(x0, n.x);
      y0 = Math.min(y0, n.y);
      x1 = Math.max(x1, n.x + this.nodeW(n));
      y1 = Math.max(y1, n.y + (n.el.offsetHeight || 160));
    }
    const pad = 50;
    const z = clamp(Math.min((b.width - pad * 2) / (x1 - x0 || 1), (b.height - pad * 2) / (y1 - y0 || 1)), 0.2, 1.15);
    this.view = { z, x: (b.width - (x1 - x0) * z) / 2 - x0 * z, y: (b.height - (y1 - y0) * z) / 2 - y0 * z };
    this.applyView();
  }

  // ------------------------------------------------------------ links
  drawLink(l) {
    const a = this.nodes.get(l.from);
    const b = this.nodes.get(l.to);
    if (!a || !b) return;
    const d = curve(this.screen(this.sockPos(a, 'out', l.out)), this.screen(this.sockPos(b, 'in', l.in)), this.view.z);
    l.path.setAttribute('d', d);
    l.hit.setAttribute('d', d);
    l.path.setAttribute('stroke-width', String(Math.max(1.5, 3 * this.view.z)));
    l.path.classList.toggle('running', b.status === 'running');
    l.path.classList.toggle('dim', !!(a.bypass || b.bypass));
  }

  redraw() {
    for (const l of this.links.values()) this.drawLink(l);
    if (this.drag && this.drag.mode === 'link') this.drawTemp();
  }

  inputLink(id, i) {
    for (const l of this.links.values()) if (l.to === id && l.in === i) return l;
    return null;
  }

  // true when `target` can be reached by following links downstream from `from`
  reaches(from, target) {
    const stack = [from];
    const seen = new Set();
    while (stack.length) {
      const c = stack.pop();
      if (c === target) return true;
      if (seen.has(c)) continue;
      seen.add(c);
      for (const l of this.links.values()) if (l.from === c) stack.push(l.to);
    }
    return false;
  }

  connect(from, out, to, inp, o = {}) {
    const a = this.nodes.get(from);
    const b = this.nodes.get(to);
    if (!a || !b || from === to) return false;
    const so = a.def.outputs[out];
    const si = b.def.inputs[inp];
    if (!so || !si || !compatible(so.type, si.type)) return false;
    if (this.reaches(to, from)) {
      if (!o.silent) toast(t('wf.loop'), 'warn');
      return false;
    }
    const old = this.inputLink(to, inp);
    if (old) this.removeLink(old.id, true);
    const id = this.nextLink++;
    const g = svgEl('g', { 'data-link': id });
    const hit = svgEl('path', { class: 'link-hit' });
    const path = svgEl('path', { class: 'link', stroke: typeColor(so.type) });
    path.style.setProperty('--t', typeColor(so.type));
    g.append(hit, path);
    this.svg.append(g);
    const l = { id, from, out, to, in: inp, g, hit, path };
    this.links.set(id, l);
    this.refreshSocks(a);
    this.refreshSocks(b);
    this.drawLink(l);
    if (!o.silent) this.changed();
    return true;
  }

  removeLink(id, silent) {
    const l = this.links.get(id);
    if (!l) return;
    l.g.remove();
    this.links.delete(id);
    const a = this.nodes.get(l.from);
    const b = this.nodes.get(l.to);
    if (a) this.refreshSocks(a);
    if (b) this.refreshSocks(b);
    if (!silent) this.changed();
  }

  refreshSocks(n) {
    n.el.querySelectorAll('.sock').forEach((s) => {
      const i = Number(s.dataset.i);
      let on = false;
      if (s.dataset.dir === 'in') on = !!this.inputLink(n.id, i);
      else for (const l of this.links.values()) if (l.from === n.id && l.out === i) on = true;
      s.classList.toggle('on', on);
    });
  }

  beginLink(e, n, dir, i) {
    if (dir === 'in') {
      const ex = this.inputLink(n.id, i);
      if (ex) {
        this.removeLink(ex.id, true);
        const src = this.nodes.get(ex.from);
        if (src) this.startLinkDrag(e, src, 'out', ex.out, true);
        return;
      }
    }
    this.startLinkDrag(e, n, dir, i, false);
  }

  startLinkDrag(e, n, dir, i, detached) {
    const s = dir === 'out' ? n.def.outputs[i] : n.def.inputs[i];
    if (!s) return;
    this.begin(e, { mode: 'link', id: n.id, dir, i, type: s.type, detached, cur: this.rel(e) });
    this.root.classList.add('linking');
    for (const m of this.nodes.values()) {
      if (m.id === n.id) continue;
      m.el.querySelectorAll('.sock').forEach((sk) => {
        const sd = sk.dataset.dir;
        if (sd === dir) return;
        const si = Number(sk.dataset.i);
        const st = sd === 'in' ? m.def.inputs[si].type : m.def.outputs[si].type;
        sk.classList.toggle('ok', dir === 'out' ? compatible(s.type, st) : compatible(st, s.type));
      });
    }
    this.temp.setAttribute('stroke', typeColor(s.type));
    this.temp.setAttribute('stroke-width', String(Math.max(1.5, 3 * this.view.z)));
    this.temp.style.display = '';
    this.drawTemp();
  }

  drawTemp() {
    const d = this.drag;
    if (!d || d.mode !== 'link') return;
    const n = this.nodes.get(d.id);
    if (!n) return;
    const a = this.screen(this.sockPos(n, d.dir, d.i));
    this.temp.setAttribute('d', d.dir === 'out' ? curve(a, d.cur, this.view.z) : curve(d.cur, a, this.view.z));
  }

  endLink(e, d) {
    this.root.classList.remove('linking');
    this.root.querySelectorAll('.sock.ok').forEach((s) => s.classList.remove('ok'));
    this.temp.style.display = 'none';
    const hit = document.elementFromPoint(e.clientX, e.clientY);
    const sk = hit && hit.closest ? hit.closest('.sock') : null;
    const nodeEl = hit && hit.closest ? hit.closest('.node') : null;
    const target = nodeEl ? this.nodes.get(Number(nodeEl.dataset.id)) : null;
    if (sk && target && target.id !== d.id && sk.dataset.dir !== d.dir) {
      const i = Number(sk.dataset.i);
      if (d.dir === 'out') this.connect(d.id, d.i, target.id, i);
      else this.connect(target.id, i, d.id, d.i);
      return;
    }
    if (target && target.id !== d.id) {
      if (!this.autoConnect(d, target.id) && d.detached) this.changed();
      return;
    }
    if (!target && !d.detached && this.opts.onSearch) {
      this.opts.onSearch(e.clientX, e.clientY, { world: this.toWorld(e.clientX, e.clientY), link: { id: d.id, dir: d.dir, i: d.i, type: d.type } });
    }
    if (d.detached) this.changed();
  }

  // Connects a dangling link (from a drag or the search popup) to the best socket of a node.
  autoConnect(link, targetId) {
    const m = this.nodes.get(targetId);
    if (!m) return false;
    if (link.dir === 'out') {
      const ins = m.def.inputs.map((s, i) => ({ s, i })).filter((x) => compatible(link.type, x.s.type));
      ins.sort((a, b) => Number(b.s.type === link.type) - Number(a.s.type === link.type));
      const free = ins.find((x) => !this.inputLink(m.id, x.i)) || ins[0];
      return free ? this.connect(link.id, link.i, m.id, free.i) : false;
    }
    const outs = m.def.outputs.map((s, i) => ({ s, i })).filter((x) => compatible(x.s.type, link.type));
    outs.sort((a, b) => Number(b.s.type === link.type) - Number(a.s.type === link.type));
    return outs[0] ? this.connect(m.id, outs[0].i, link.id, link.i) : false;
  }

  // ------------------------------------------------------------ nodes
  get(id) {
    return this.nodes.get(Number(id));
  }

  addNode(type, x, y, values, o = {}) {
    const def = NODE_DEFS[type];
    if (!def) return null;
    const id = o.id && !this.nodes.has(o.id) ? o.id : this.nextId;
    this.nextId = Math.max(this.nextId, id + 1);
    const vals = {};
    for (const w of def.widgets) vals[w.name] = clone(w.value);
    if (values) for (const [k, v] of Object.entries(values)) if (k in vals && v !== undefined) vals[k] = clone(v);
    const n = {
      id, type, def,
      x: Math.round(Number(x) || 0), y: Math.round(Number(y) || 0),
      w: clamp(Number(o.w) || def.width || 280, 200, 900),
      values: vals, bypass: !!o.bypass, collapsed: !!o.collapsed, title: o.title || '',
      status: null, result: null, error: null, warnings: [], widgets: new Map(), showAdv: !!o.adv,
    };
    this.build(n);
    this.nodes.set(id, n);
    this.world.append(n.el);
    this.place(n);
    if (!o.silent) this.changed();
    return n;
  }

  build(n) {
    const def = n.def;
    const cat = CATEGORIES[def.cat] || CATEGORIES.util;
    const cls = 'node' + (def.frontendOnly ? ' note' : '') + ' cat-' + def.cat;
    const e = el('div', { class: cls, dataset: { id: String(n.id), type: n.type } });
    const c = def.color || cat.color;
    e.style.setProperty('--c', c);
    for (const a of [8, 12, 16, 20, 24, 30, 40, 60]) e.style.setProperty('--ca' + String(a).padStart(2, '0'), tint(c, a / 100));
    n.titleEl = el('span', { class: 'title', text: n.title || nodeTitle(n.type) });
    n.badgeEl = el('span', { class: 'badge' });
    const dot = el('span', { class: 'dot', title: t('menu.collapse') }, icon(def.icon || cat.icon, 13));
    dot.addEventListener('pointerdown', (ev) => ev.stopPropagation());
    dot.addEventListener('click', (ev) => {
      ev.stopPropagation();
      this.toggleCollapse(n.id);
    });
    n.warnEl = el('span', { class: 'warn-ic', title: def.needs ? t('insp.needs', { tool: def.needs }) : '' }, icon('alert', 13));
    e.append(el('div', { class: 'node-head' }, dot, n.titleEl, el('span', { class: 'tag', text: t('nd.bypassed') }), n.warnEl, el('span', { class: 'done-ic' }, icon('check', 13)), n.badgeEl));
    const rows = Math.max(def.inputs.length, def.outputs.length);
    n.cntEls = [];
    if (rows) {
      const slots = el('div', { class: 'node-slots' });
      for (let i = 0; i < rows; i++) {
        const a = def.inputs[i];
        const b = def.outputs[i];
        const cnt = b ? el('span', { class: 'cnt' }) : null;
        n.cntEls.push(cnt);
        slots.append(el('div', { class: 'slot-row' },
          a ? el('div', { class: 'slot in' }, sock('in', i, a.type), el('span', { class: 'sl', text: sockLabel(a.name) })) : el('div', { class: 'slot' }),
          b ? el('div', { class: 'slot out' }, cnt, el('span', { class: 'sl', text: sockLabel(b.name) }), sock('out', i, b.type)) : null));
      }
      e.append(slots);
    }
    const wbox = el('div', { class: 'node-widgets' });
    n.seps = [];
    let group = null;
    let advBox = null;
    const advCount = def.widgets.filter((w) => w.adv).length;
    const addTo = (w, we) => {
      if (w.adv) {
        if (!advBox) {
          n.moreBtn = el('button', {
            class: 'w-more',
            onpointerdown: (ev) => ev.stopPropagation(),
            onclick: (ev) => {
              ev.stopPropagation();
              n.showAdv = !n.showAdv;
              this.updateVis(n);
              this.redraw();
              if (this.opts.onAdv) this.opts.onAdv(n);
            },
          }, icon('chevron-down', 13), el('span', { text: t('nd.advanced') }), el('span', { class: 'n', text: String(advCount) }));
          wbox.append(n.moreBtn);
          advBox = true;
        }
      }
      if (w.group && w.group !== group) {
        group = w.group;
        const sep = el('div', { class: 'w-sep', text: groupLabel(w.group) });
        sep.dataset.group = w.group;
        sep.dataset.adv = w.adv ? '1' : '';
        n.seps.push(sep);
        wbox.append(sep);
      }
      wbox.append(we);
    };
    for (const w of def.widgets) {
      const we = createWidget(w, n.values[w.name], (v) => {
        n.values[w.name] = v;
        this.updateVis(n);
        this.changed();
        if (this.opts.onValue) this.opts.onValue(n, w.name);
      });
      we.dataset.name = w.name;
      n.widgets.set(w.name, we);
      addTo(w, we);
    }
    e.append(wbox);
    if (def.preview) {
      n.previewEl = el('div', { class: 'node-preview' }, el('div', { class: 'empty', text: t('nd.empty') }));
      e.append(n.previewEl);
    }
    n.footEl = el('div', { class: 'node-foot' });
    n.progEl = el('div', { class: 'node-progress' }, el('i'));
    e.append(n.footEl, n.progEl, el('div', { class: 'resize' }));
    n.el = e;
    this.updateVis(n);
    this.applyFlags(n);
  }

  updateVis(n) {
    const shown = new Set();
    for (const w of n.def.widgets) {
      const we = n.widgets.get(w.name);
      if (!we) continue;
      const vis = isVisible(w, n.values) && (!w.adv || !!n.showAdv);
      we.hidden = !vis;
      if (vis && w.group) shown.add(w.group + (w.adv ? ':a' : ''));
    }
    for (const sep of n.seps || []) sep.hidden = !shown.has(sep.dataset.group + (sep.dataset.adv ? ':a' : ''));
    if (n.moreBtn) n.moreBtn.classList.toggle('open', !!n.showAdv);
  }

  applyFlags(n) {
    n.el.classList.toggle('bypass', n.bypass);
    n.el.classList.toggle('collapsed', n.collapsed);
    n.el.classList.toggle('missing', !!(n.def.needs && this.missing.has(n.def.needs)));
  }

  place(n) {
    n.el.style.left = n.x + 'px';
    n.el.style.top = n.y + 'px';
    n.el.style.width = this.nodeW(n) + 'px';
  }

  setValue(id, name, v) {
    const n = this.get(id);
    if (!n || !(name in n.values)) return;
    n.values[name] = clone(v);
    const we = n.widgets.get(name);
    if (we && we.setValue) we.setValue(n.values[name]);
    this.updateVis(n);
    this.changed();
  }

  setMissing(set) {
    this.missing = set || new Set();
    for (const n of this.nodes.values()) this.applyFlags(n);
  }

  removeNodes(ids) {
    const list = [...ids].map(Number).filter((id) => this.nodes.has(id));
    if (!list.length) return;
    for (const id of list) {
      for (const l of [...this.links.values()]) if (l.from === id || l.to === id) this.removeLink(l.id, true);
      this.nodes.get(id).el.remove();
      this.nodes.delete(id);
      this.sel.delete(id);
    }
    if (this.opts.onSelect) this.opts.onSelect(null);
    this.changed();
  }

  toggleBypass(ids) {
    const list = [...(ids || this.sel)].map((id) => this.get(id)).filter((n) => n && !n.def.frontendOnly);
    if (!list.length) return;
    const on = !list.every((n) => n.bypass);
    for (const n of list) {
      n.bypass = on;
      this.applyFlags(n);
    }
    this.redraw();
    this.changed();
  }

  toggleCollapse(id) {
    const n = this.get(id);
    if (!n) return;
    n.collapsed = !n.collapsed;
    this.applyFlags(n);
    this.place(n);
    this.redraw();
    this.changed();
  }

  startRename(id) {
    const n = this.get(id);
    if (!n) return;
    const tEl = n.titleEl;
    const before = tEl.textContent;
    tEl.contentEditable = 'true';
    tEl.focus();
    const range = document.createRange();
    range.selectNodeContents(tEl);
    const s = window.getSelection();
    s.removeAllRanges();
    s.addRange(range);
    let key = null;
    let blur = null;
    const finish = (commit) => {
      tEl.removeEventListener('keydown', key);
      tEl.removeEventListener('blur', blur);
      tEl.contentEditable = 'false';
      if (!commit) {
        tEl.textContent = before;
        return;
      }
      const v = tEl.textContent.trim();
      n.title = v && v !== nodeTitle(n.type) ? v : '';
      tEl.textContent = n.title || nodeTitle(n.type);
      this.changed();
      if (this.opts.onRename) this.opts.onRename(n);
    };
    key = (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') {
        e.preventDefault();
        finish(true);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        finish(false);
      }
    };
    blur = () => finish(true);
    tEl.addEventListener('keydown', key);
    tEl.addEventListener('blur', blur);
  }

  // ------------------------------------------------------------ selection
  select(id) {
    this.sel = new Set([id]);
    this.refreshSel();
    if (this.opts.onSelect) this.opts.onSelect(this.nodes.get(id) || null);
  }
  primary(id) {
    if (this.opts.onSelect) this.opts.onSelect(this.nodes.get(id) || null);
  }
  toggleSel(id) {
    if (this.sel.has(id)) this.sel.delete(id);
    else this.sel.add(id);
    this.refreshSel();
    if (this.opts.onSelect) this.opts.onSelect(this.sel.has(id) ? this.nodes.get(id) : null);
  }
  clearSel() {
    this.sel.clear();
    this.refreshSel();
    if (this.opts.onSelect) this.opts.onSelect(null);
  }
  selectAll() {
    this.sel = new Set(this.nodes.keys());
    this.refreshSel();
  }
  refreshSel() {
    for (const n of this.nodes.values()) n.el.classList.toggle('selected', this.sel.has(n.id));
  }
  boxSelect(d) {
    const a = this.screenToWorld(Math.min(d.x0, d.x1), Math.min(d.y0, d.y1));
    const b = this.screenToWorld(Math.max(d.x0, d.x1), Math.max(d.y0, d.y1));
    for (const n of this.nodes.values()) {
      const h = n.el.offsetHeight || 60;
      if (n.x < b.x && n.x + this.nodeW(n) > a.x && n.y < b.y && n.y + h > a.y) this.sel.add(n.id);
    }
    this.refreshSel();
  }

  // ------------------------------------------------------------ clipboard
  copy() {
    const ids = [...this.sel].filter((id) => this.nodes.has(id));
    if (!ids.length) return false;
    const set = new Set(ids);
    const nodes = ids.map((id) => {
      const n = this.nodes.get(id);
      return { id: n.id, type: n.type, x: n.x, y: n.y, w: n.w, values: clone(n.values), title: n.title, bypass: n.bypass };
    });
    const links = [...this.links.values()].filter((l) => set.has(l.from) && set.has(l.to)).map((l) => [l.from, l.out, l.to, l.in]);
    this.clip = { nodes, links };
    try {
      localStorage.setItem('morph.clip', JSON.stringify(this.clip));
    } catch {
      // storage full or disabled
    }
    return true;
  }
  hasClip() {
    if (this.clip) return true;
    try {
      return !!localStorage.getItem('morph.clip');
    } catch {
      return false;
    }
  }
  paste(at) {
    let clip = this.clip;
    if (!clip) {
      try {
        clip = JSON.parse(localStorage.getItem('morph.clip') || 'null');
      } catch {
        clip = null;
      }
    }
    if (!clip || !clip.nodes || !clip.nodes.length) return;
    const minX = Math.min(...clip.nodes.map((n) => n.x));
    const minY = Math.min(...clip.nodes.map((n) => n.y));
    const p = at || this.mouseWorld();
    const map = new Map();
    this.sel = new Set();
    for (const c of clip.nodes) {
      const n = this.addNode(c.type, p.x + c.x - minX, p.y + c.y - minY, c.values, { w: c.w, title: c.title, bypass: c.bypass, silent: true });
      if (n) {
        map.set(c.id, n.id);
        this.sel.add(n.id);
      }
    }
    for (const [a, o, b, i] of clip.links || []) if (map.has(a) && map.has(b)) this.connect(map.get(a), o, map.get(b), i, { silent: true });
    this.refreshSel();
    this.changed();
  }
  duplicate() {
    if (!this.copy()) return;
    const minX = Math.min(...this.clip.nodes.map((n) => n.x));
    const minY = Math.min(...this.clip.nodes.map((n) => n.y));
    this.paste({ x: minX + 30, y: minY + 30 });
  }

  // ------------------------------------------------------------ layout
  arrange() {
    const depth = new Map();
    const preds = (id) => [...this.links.values()].filter((l) => l.to === id).map((l) => l.from);
    const d = (id, stack = new Set()) => {
      if (depth.has(id)) return depth.get(id);
      if (stack.has(id)) return 0;
      stack.add(id);
      const p = preds(id);
      const v = p.length ? Math.max(...p.map((q) => d(q, stack))) + 1 : 0;
      stack.delete(id);
      depth.set(id, v);
      return v;
    };
    for (const id of this.nodes.keys()) d(id);
    const cols = new Map();
    for (const [id, dep] of depth) {
      if (!cols.has(dep)) cols.set(dep, []);
      cols.get(dep).push(this.nodes.get(id));
    }
    let x = 40;
    for (const dep of [...cols.keys()].sort((a, b) => a - b)) {
      const col = cols.get(dep).sort((a, b) => a.y - b.y);
      let y = 40;
      let maxW = 0;
      for (const n of col) {
        n.x = x;
        n.y = y;
        this.place(n);
        y += (n.el.offsetHeight || 150) + 36;
        maxW = Math.max(maxW, this.nodeW(n));
      }
      x += maxW + 80;
    }
    this.redraw();
    this.changed();
    this.fit();
  }

  // ------------------------------------------------------------ (de)serialization and history
  serialize(withView = true) {
    const g = { version: 1, nodes: [], links: [] };
    if (withView) g.view = { x: Math.round(this.view.x), y: Math.round(this.view.y), z: Number(this.view.z.toFixed(3)) };
    for (const n of this.nodes.values()) {
      const o = { id: n.id, type: n.type, x: Math.round(n.x), y: Math.round(n.y), w: n.w, values: clone(n.values) };
      if (n.bypass) o.bypass = true;
      if (n.collapsed) o.collapsed = true;
      if (n.title) o.title = n.title;
      if (n.showAdv) o.adv = true;
      g.nodes.push(o);
    }
    for (const l of this.links.values()) g.links.push([l.from, l.out, l.to, l.in]);
    return g;
  }

  clear() {
    for (const l of this.links.values()) l.g.remove();
    this.links.clear();
    for (const n of this.nodes.values()) n.el.remove();
    this.nodes.clear();
    this.sel.clear();
    this.nextId = 1;
    this.nextLink = 1;
  }

  load(g, keepView = false) {
    this.muted++;
    try {
      this.clear();
      for (const nd of g.nodes || []) {
        this.addNode(nd.type, nd.x, nd.y, nd.values, { id: Number(nd.id) || undefined, w: nd.w, bypass: nd.bypass, collapsed: nd.collapsed, title: nd.title, adv: nd.adv, silent: true });
      }
      for (const l of g.links || []) {
        if (Array.isArray(l)) this.connect(Number(l[0]), Number(l[1]), Number(l[2]), Number(l[3]), { silent: true });
      }
      if (g.view && !keepView) this.view = { x: Number(g.view.x) || 0, y: Number(g.view.y) || 0, z: clamp(Number(g.view.z) || 1, 0.15, 2.5) };
    } finally {
      this.muted--;
    }
    this.applyView();
    if (this.opts.onSelect) this.opts.onSelect(null);
    if (this.opts.onChange) this.opts.onChange();
  }

  // Templates: nodes are addressed by index.
  loadIndexed(g) {
    this.muted++;
    try {
      this.clear();
      const ids = g.nodes.map((nd) => {
        const n = this.addNode(nd.type, nd.x, nd.y, nd.values, { silent: true });
        return n ? n.id : null;
      });
      for (const [a, o, b, i] of g.links || []) if (ids[a] && ids[b]) this.connect(ids[a], o, ids[b], i, { silent: true });
    } finally {
      this.muted--;
    }
    if (this.opts.onSelect) this.opts.onSelect(null);
    if (this.opts.onChange) this.opts.onChange();
  }

  changed() {
    if (this.muted) return;
    this.commitSoon();
    if (this.opts.onChange) this.opts.onChange();
  }

  commit() {
    const s = JSON.stringify(this.serialize(false));
    if (s === this.snapshot) return;
    if (this.snapshot !== null) {
      this.history.push(this.snapshot);
      if (this.history.length > 100) this.history.shift();
    }
    this.snapshot = s;
    this.future = [];
    if (this.opts.onHistory) this.opts.onHistory();
  }

  resetHistory() {
    this.commitSoon.cancel();
    this.history = [];
    this.future = [];
    this.snapshot = JSON.stringify(this.serialize(false));
    if (this.opts.onHistory) this.opts.onHistory();
  }

  undo() {
    this.commitSoon.flush();
    if (!this.history.length) return;
    this.future.push(this.snapshot);
    this.snapshot = this.history.pop();
    this.load(JSON.parse(this.snapshot), true);
    if (this.opts.onHistory) this.opts.onHistory();
  }

  redo() {
    this.commitSoon.flush();
    if (!this.future.length) return;
    this.history.push(this.snapshot);
    this.snapshot = this.future.pop();
    this.load(JSON.parse(this.snapshot), true);
    if (this.opts.onHistory) this.opts.onHistory();
  }

  // ------------------------------------------------------------ running
  reachable() {
    const seen = new Set();
    const stack = [...this.nodes.values()].filter((n) => n.def.output && !n.bypass).map((n) => n.id);
    while (stack.length) {
      const id = stack.pop();
      if (seen.has(id)) continue;
      seen.add(id);
      for (const l of this.links.values()) if (l.to === id) stack.push(l.from);
    }
    return seen;
  }

  toSpec() {
    const nodes = [];
    for (const n of this.nodes.values()) {
      if (n.def.frontendOnly) continue;
      nodes.push({
        id: String(n.id),
        type: n.type,
        values: clone(n.values),
        inputs: n.def.inputs.map((_, i) => {
          const l = this.inputLink(n.id, i);
          return l ? { node: String(l.from), slot: l.out } : null;
        }),
        outputs: n.def.outputs.length,
        bypass: n.bypass,
        is_output: !!n.def.output && !n.bypass,
        title: n.title || nodeTitle(n.type),
      });
    }
    return { nodes };
  }

  resetStatus(ids) {
    for (const n of this.nodes.values()) {
      if (ids && !ids.has(n.id)) continue;
      n.status = null;
      n.result = null;
      n.error = null;
      n.warnings = [];
      n.el.classList.remove('running', 'done', 'error');
      n.badgeEl.textContent = '';
      n.footEl.textContent = '';
      (n.cntEls || []).forEach((c) => {
        if (c) c.textContent = '';
      });
      n.el.style.setProperty('--prog', '0');
      if (n.previewEl) {
        n.previewEl.textContent = '';
        n.previewEl.append(el('div', { class: 'empty', text: t('nd.empty') }));
      }
    }
    this.redraw();
  }

  setStatus(id, st) {
    const n = this.get(id);
    if (!n) return;
    n.status = st;
    for (const c of ['running', 'done', 'error']) n.el.classList.toggle(c, st === c);
    if (st === 'running') {
      n.el.style.setProperty('--prog', '0');
      n.footEl.textContent = '';
    }
    this.redraw();
  }

  setProgress(id, v, text) {
    const n = this.get(id);
    if (!n) return;
    const p = clamp(Number(v) || 0, 0, 1);
    n.el.style.setProperty('--prog', String(p));
    if (text) {
      n.footEl.textContent = '';
      n.footEl.append(el('span', { text: Math.round(p * 100) + '% · ' + text }));
    }
  }

  setError(id, msg, warnings) {
    const n = this.get(id);
    if (!n) return;
    n.error = String(msg || '');
    n.warnings = warnings || [];
    n.footEl.textContent = '';
    n.footEl.append(el('span', { class: 'err', title: n.error, text: n.error }));
  }

  setResult(id, ev) {
    const n = this.get(id);
    if (!n) return;
    n.result = ev;
    n.error = null;
    n.warnings = ev.warnings || [];
    n.badgeEl.textContent = fmtTime(ev.ms || 0);
    (n.cntEls || []).forEach((c, i) => {
      if (c) c.textContent = ev.counts && ev.counts[i] != null && n.def.outputs.length > 1 ? String(ev.counts[i]) : '';
    });
    this.renderFoot(n);
    if (n.previewEl) this.renderPreview(n, (ev.outputs && ev.outputs[0]) || [], (ev.counts && ev.counts[0]) || 0);
  }

  renderFoot(n) {
    const ev = n.result;
    const f = n.footEl;
    f.textContent = '';
    if (!ev) return;
    const outs = ev.outputs || [];
    const counts = ev.counts || [];
    if (n.def.outputs.length > 1) {
      f.append(el('span', { text: n.def.outputs.map((s, i) => sockLabel(s.name) + ': ' + (counts[i] || 0)).join(' · ') }));
    } else {
      const list = outs[0] || [];
      const total = list.reduce((s, x) => s + (x.size || 0), 0);
      f.append(el('span', { text: t('nd.files', { n: counts[0] || 0 }) + ' · ' + fmtBytes(total) }));
      const changed = list.filter((x) => x.origin && x.path !== x.origin && x.origin_size > 0);
      if (changed.length && n.type !== 'files.load') {
        const a = changed.reduce((s, x) => s + (x.size || 0), 0);
        const o = changed.reduce((s, x) => s + x.origin_size, 0);
        const p = fmtPct(a, o);
        if (p) f.append(el('span', { class: p.cls, text: p.text }));
      }
    }
    const warns = ev.warnings || [];
    if (warns.length) f.append(el('span', { class: 'wn', title: warns.slice(0, 20).join(NL), text: '⚠ ' + warns.length }));
    const first = n.type === 'files.save' && outs[0] && outs[0][0];
    if (first) {
      f.append(el('button', {
        class: 'btn sm',
        onpointerdown: (e) => e.stopPropagation(),
        onclick: (e) => {
          e.stopPropagation();
          if (this.opts.onReveal) this.opts.onReveal(first.path);
        },
      }, icon('folder', 12), t('nd.reveal')));
    }
  }

  renderPreview(n, files, total) {
    const box = n.previewEl;
    box.textContent = '';
    if (!files.length) {
      box.append(el('div', { class: 'empty', text: t('nd.empty') }));
      return;
    }
    const textual = files.length <= 2 && files.every((f) => f.kind === 'data' || f.kind === 'text');
    if (textual) {
      const f = files[0];
      const pre = el('pre', { class: 'scroll', text: t('insp.loading') });
      pre.addEventListener('pointerdown', (e) => e.stopPropagation());
      box.append(pre);
      invoke('read_text', { path: f.path, limit: 8192 })
        .then((r) => {
          let s = (r && r.text) || '';
          if (f.ext === 'json') {
            try {
              s = JSON.stringify(JSON.parse(s), null, 2);
            } catch {
              // keep raw text
            }
          }
          pre.textContent = s.slice(0, 6000);
        })
        .catch((e) => {
          pre.textContent = String((e && e.message) || e);
        });
      const grid = el('div', { class: 'grid', style: { marginTop: '6px' } });
      for (const x of files) grid.append(this.tile(n, x));
      box.append(grid);
      return;
    }
    const grid = el('div', { class: 'grid scroll' });
    const shown = files.slice(0, PREVIEW_MAX);
    for (const f of shown) grid.append(this.tile(n, f));
    if (total > shown.length) grid.append(el('div', { class: 'tile more', text: '+' + (total - shown.length) }));
    box.append(grid);
  }

  tile(n, f) {
    const tl = el('div', { class: 'tile k-' + f.kind, title: f.name + ' · ' + fmtBytes(f.size) }, icon(kindIcon(f.kind), 22));
    if (['image', 'video', 'audio'].includes(f.kind)) {
      loadThumb(f, 256).then((url) => {
        if (!url) return;
        const img = el('img', { src: url, alt: '', draggable: 'false' });
        img.onerror = () => img.remove();
        tl.prepend(img);
      });
    }
    tl.append(el('span', { class: 'ext', text: f.ext || '?' }), el('span', { class: 'nm', text: f.name }));
    tl.addEventListener('pointerdown', (e) => e.stopPropagation());
    tl.addEventListener('click', (e) => {
      e.stopPropagation();
      if (!this.sel.has(n.id)) {
        this.sel = new Set([n.id]);
        this.refreshSel();
      }
      if (this.opts.onFile) this.opts.onFile(f, n);
    });
    tl.addEventListener('dblclick', (e) => {
      e.stopPropagation();
      if (this.opts.onOpenFile) this.opts.onOpenFile(f);
    });
    return tl;
  }
}
