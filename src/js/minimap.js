// Minimap: a small overview of the graph with the visible area. Click or drag to move the view.
import { CATEGORIES } from './nodes.js';

export class Minimap {
  constructor(host, editor) {
    this.host = host;
    this.editor = editor;
    this.cv = host.querySelector('canvas');
    this.ctx = this.cv.getContext('2d');
    this.queued = false;
    this.box = null;
    const go = (e) => {
      if (!this.box) return;
      const r = this.cv.getBoundingClientRect();
      const { s, ox, oy } = this.box;
      const wx = (e.clientX - r.left - ox) / s;
      const wy = (e.clientY - r.top - oy) / s;
      const c = editor.root.getBoundingClientRect();
      const z = editor.view.z;
      editor.view.x = c.width / 2 - wx * z;
      editor.view.y = c.height / 2 - wy * z;
      editor.applyView();
    };
    host.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      e.stopPropagation();
      try {
        host.setPointerCapture(e.pointerId);
      } catch {
        // ignore
      }
      go(e);
      const move = (ev) => go(ev);
      const up = () => {
        host.removeEventListener('pointermove', move);
        host.removeEventListener('pointerup', up);
        host.removeEventListener('pointercancel', up);
      };
      host.addEventListener('pointermove', move);
      host.addEventListener('pointerup', up);
      host.addEventListener('pointercancel', up);
    });
    host.addEventListener('wheel', (e) => {
      e.preventDefault();
      editor.zoomBy(Math.exp(-e.deltaY * 0.0015));
    }, { passive: false });
  }

  update() {
    if (this.queued || this.host.hidden) return;
    this.queued = true;
    requestAnimationFrame(() => {
      this.queued = false;
      this.draw();
    });
  }

  draw() {
    const cv = this.cv;
    const dpr = window.devicePixelRatio || 1;
    const W = this.host.clientWidth;
    const H = this.host.clientHeight;
    if (!W || !H) return;
    if (cv.width !== Math.round(W * dpr)) cv.width = Math.round(W * dpr);
    if (cv.height !== Math.round(H * dpr)) cv.height = Math.round(H * dpr);
    const g = this.ctx;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, W, H);
    const ed = this.editor;
    const nodes = [...ed.nodes.values()];
    const c = ed.root.getBoundingClientRect();
    const z = ed.view.z;
    const vx0 = -ed.view.x / z;
    const vy0 = -ed.view.y / z;
    const vx1 = vx0 + c.width / z;
    const vy1 = vy0 + c.height / z;
    let x0 = vx0;
    let y0 = vy0;
    let x1 = vx1;
    let y1 = vy1;
    for (const n of nodes) {
      x0 = Math.min(x0, n.x);
      y0 = Math.min(y0, n.y);
      x1 = Math.max(x1, n.x + ed.nodeW(n));
      y1 = Math.max(y1, n.y + (n.el.offsetHeight || 120));
    }
    const pad = 8;
    const s = Math.min((W - pad * 2) / (x1 - x0 || 1), (H - pad * 2) / (y1 - y0 || 1));
    const ox = pad + ((W - pad * 2) - (x1 - x0) * s) / 2 - x0 * s;
    const oy = pad + ((H - pad * 2) - (y1 - y0) * s) / 2 - y0 * s;
    this.box = { s, ox, oy };
    g.strokeStyle = 'rgba(160, 175, 205, 0.28)';
    g.lineWidth = 1;
    for (const l of ed.links.values()) {
      const a = ed.nodes.get(l.from);
      const b = ed.nodes.get(l.to);
      if (!a || !b) continue;
      const p = ed.sockPos(a, 'out', l.out);
      const q = ed.sockPos(b, 'in', l.in);
      g.beginPath();
      g.moveTo(ox + p.x * s, oy + p.y * s);
      g.lineTo(ox + q.x * s, oy + q.y * s);
      g.stroke();
    }
    for (const n of nodes) {
      const cat = CATEGORIES[n.def.cat] || CATEGORIES.util;
      const x = ox + n.x * s;
      const y = oy + n.y * s;
      const w = Math.max(3, ed.nodeW(n) * s);
      const h = Math.max(3, (n.el.offsetHeight || 120) * s);
      g.fillStyle = ed.sel.has(n.id) ? 'rgba(110, 162, 255, 0.55)' : 'rgba(40, 47, 60, 0.95)';
      g.fillRect(x, y, w, h);
      g.fillStyle = n.status === 'error' ? '#ff5d5d' : n.def.color || cat.color;
      g.fillRect(x, y, w, Math.max(2, 4 * s * 4));
    }
    g.strokeStyle = 'rgba(232, 237, 246, 0.85)';
    g.lineWidth = 1.5;
    g.fillStyle = 'rgba(110, 162, 255, 0.07)';
    const rx = ox + vx0 * s;
    const ry = oy + vy0 * s;
    g.fillRect(rx, ry, (vx1 - vx0) * s, (vy1 - vy0) * s);
    g.strokeRect(rx + 0.5, ry + 0.5, (vx1 - vx0) * s - 1, (vy1 - vy0) * s - 1);
  }
}
