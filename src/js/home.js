// Dashboard (0.5, compact): greeting with the main actions, one activity strip,
// templates with live graph thumbnails and the recent runs.
import { el, svgEl, icon, fmtTime } from './ui.js';
import { t, getLang, tplText } from './i18n.js';
import { NODE_DEFS, CATEGORIES, typeColor } from './nodes.js';
import { NODE_TOTAL as NODE_COUNT } from './nodes_v4.js';
import { TEMPLATES } from './templates.js';
import { paint } from './colors.js';
import './i18n_v5.js';

const DAYS = 14;
const DAY_MS = 86400000;
const FEATURED = ['web-images', 'share-discord', 'device-iphone', 'excel-csv', 'docs-pdf', 'convert-all'];

function greeting() {
  const h = new Date().getHours();
  return t(h < 5 ? 'home.hi_night' : h < 12 ? 'home.hi_morning' : h < 18 ? 'home.hi_day' : 'home.hi_evening');
}

export function ago(ts) {
  const s = Math.max(0, (Date.now() - ts) / 1000);
  if (s < 60) return t('home.now');
  if (s < 3600) return t('home.ago_m', { n: Math.floor(s / 60) });
  if (s < 86400) return t('home.ago_h', { n: Math.floor(s / 3600) });
  if (s < 172800) return t('home.yesterday');
  if (s < 7 * 86400) return t('home.ago_d', { n: Math.floor(s / 86400) });
  try {
    return new Date(ts).toLocaleDateString(getLang(), { day: 'numeric', month: 'short' });
  } catch {
    return new Date(ts).toLocaleDateString();
  }
}

const num = (n) => {
  try {
    return Number(n || 0).toLocaleString(getLang());
  } catch {
    return String(n || 0);
  }
};

function buckets(hist, pick) {
  const out = new Array(DAYS).fill(0);
  const end = new Date();
  end.setHours(24, 0, 0, 0);
  const start = end.getTime() - DAYS * DAY_MS;
  for (const h of hist) {
    if (!h || !h.at || h.at < start) continue;
    out[Math.min(DAYS - 1, Math.floor((h.at - start) / DAY_MS))] += pick(h);
  }
  return out;
}

function bars(values) {
  const w = 84;
  const h = 26;
  const gap = 2;
  const bw = (w - gap * (values.length - 1)) / values.length;
  const max = Math.max(1, ...values);
  const svg = svgEl('svg', { class: 'hm-bars', viewBox: `0 0 ${w} ${h}`, width: w, height: h, 'aria-hidden': 'true' });
  values.forEach((v, i) => {
    const bh = v ? Math.max(2, (v / max) * h) : 1.5;
    svg.append(svgEl('rect', { x: (i * (bw + gap)).toFixed(2), y: (h - bh).toFixed(2), width: bw.toFixed(2), height: bh.toFixed(2), rx: 1, class: v ? (i === values.length - 1 ? 'on last' : 'on') : '' }));
  });
  return svg;
}

// A miniature of the template graph: nodes as cards, links colored by socket type.
function thumb(tpl) {
  const g = tpl.build();
  const W = 300;
  const H = 120;
  const pad = 16;
  const boxes = g.nodes.map((n) => {
    const d = NODE_DEFS[n.type] || { inputs: [], outputs: [], widgets: [] };
    const rows = Math.max((d.inputs || []).length, (d.outputs || []).length);
    const ws = (d.widgets || []).filter((w) => !w.adv).length;
    const h = 42 + rows * 24 + Math.min(ws, 5) * 30 + (d.preview ? 150 : 0) + 12;
    return { x: n.x, y: n.y, w: d.width || 280, h, c: (CATEGORIES[d.cat] || CATEGORIES.util).color, d };
  });
  const x0 = Math.min(...boxes.map((b) => b.x));
  const y0 = Math.min(...boxes.map((b) => b.y));
  const x1 = Math.max(...boxes.map((b) => b.x + b.w));
  const y1 = Math.max(...boxes.map((b) => b.y + b.h));
  const s = Math.min((W - pad * 2) / (x1 - x0 || 1), (H - pad * 2) / (y1 - y0 || 1));
  const ox = (W - (x1 - x0) * s) / 2 - x0 * s;
  const oy = (H - (y1 - y0) * s) / 2 - y0 * s;
  const r1 = (v) => Math.round(v * 10) / 10;
  const svg = svgEl('svg', { class: 'hm-thumb', viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: 'xMidYMid meet', 'aria-hidden': 'true' });
  for (const [from, out, to, inp] of g.links) {
    const a = boxes[from];
    const b = boxes[to];
    if (!a || !b) continue;
    const ax = ox + (a.x + a.w) * s;
    const ay = oy + (a.y + 54 + out * 24) * s;
    const bx = ox + b.x * s;
    const by = oy + (b.y + 54 + inp * 24) * s;
    const dx = Math.max(10, Math.abs(bx - ax) * 0.5);
    const so = (a.d.outputs || [])[out];
    svg.append(svgEl('path', { class: 'ln', d: `M${r1(ax)} ${r1(ay)}C${r1(ax + dx)} ${r1(ay)} ${r1(bx - dx)} ${r1(by)} ${r1(bx)} ${r1(by)}`, stroke: typeColor(so ? so.type : 'FILES') }));
  }
  for (const b of boxes) {
    const x = ox + b.x * s;
    const y = oy + b.y * s;
    const w = b.w * s;
    const h = b.h * s;
    svg.append(svgEl('rect', { class: 'nd', x: r1(x), y: r1(y), width: r1(w), height: r1(h), rx: r1(Math.max(2.5, 10 * s)) }));
    svg.append(svgEl('line', { class: 'hd', x1: r1(x), y1: r1(y + 36 * s), x2: r1(x + w), y2: r1(y + 36 * s) }));
    svg.append(svgEl('rect', { x: r1(x + 9 * s), y: r1(y + 10 * s), width: r1(16 * s), height: r1(16 * s), rx: r1(4 * s), fill: b.c }));
    svg.append(svgEl('rect', { class: 'tt', x: r1(x + 32 * s), y: r1(y + 15 * s), width: r1(w * 0.42), height: r1(Math.max(1.5, 6 * s)), rx: 1 }));
    for (let i = 0; i < 3 && 42 + i * 30 + 24 < b.h - 12; i++) {
      svg.append(svgEl('rect', { class: 'wd', x: r1(x + 10 * s), y: r1(y + (48 + i * 30) * s), width: r1(w - 20 * s), height: r1(22 * s), rx: r1(5 * s) }));
    }
  }
  return svg;
}

const head = (title, link) => el('div', { class: 'h5-sec' }, el('h2', { text: title }), link || null);
const lnk = (text, fn) => el('button', { class: 'h5-lnk', onclick: fn }, el('span', { text }), icon('chevron-right', 13));
const pill = (cls, ic, text, fn, title) => el('button', { class: 'h5-btn' + (cls ? ' ' + cls : ''), onclick: fn, title: title || '' }, icon(ic, 15), el('span', { text }));

export function renderHome(root, ctx) {
  const top = root.scrollTop;
  root.textContent = '';
  const hist = (ctx.history() || []).filter((h) => h && h.at);
  const g = ctx.graph();
  const page = el('div', { class: 'h5' });

  // greeting and the four things people come here for
  page.append(el('header', { class: 'h5-head' },
    el('div', { class: 'h5-t' }, el('h1', { text: greeting() }), el('p', { text: t('home.sub5', { n: NODE_COUNT, t: TEMPLATES.length }) })),
    el('div', { class: 'h5-acts' },
      pill('ghost', 'file-plus', t('home.new5'), () => ctx.newWorkflow(), t('home.new')),
      pill('ghost', 'folder-open', t('home.open5'), () => ctx.openWorkflow(), t('home.open')),
      pill('', 'wire', t('home.continue5'), () => ctx.editor(), t('home.continue5_t', { name: g.name || t('top.untitled') })),
      pill('primary', 'bolt', t('home.quick'), () => ctx.quick(), 'Ctrl+Shift+C'))));

  // one activity strip: four numbers, sparklines only when there is something to draw
  const runs14 = buckets(hist, () => 1);
  const files14 = buckets(hist, (h) => Number(h.files) || 0);
  const time14 = buckets(hist, (h) => Number(h.ms) || 0);
  const sum = (a) => a.reduce((x, y) => x + y, 0);
  const recent = hist.filter((h) => h.at >= Date.now() - DAYS * DAY_MS);
  const okN = recent.filter((h) => h.ok).length;
  const failN = recent.filter((h) => !h.ok && !h.cancelled).length;
  const fin = okN + failN;
  const rate = fin ? Math.round((okN / fin) * 100) : null;
  const cell = (k, v, vals, tip) => el('div', { class: 'h5-stat', title: tip || '' },
    el('div', { class: 'tx' }, el('span', { class: 'k', text: k }), el('b', { text: v })),
    vals && sum(vals) ? bars(vals) : null);
  const rateCell = el('div', { class: 'h5-stat', title: t('home.success_s', { ok: okN, fail: failN }) },
    el('div', { class: 'tx' }, el('span', { class: 'k', text: t('home.success5') }), el('b', { text: rate == null ? '—' : rate + '%' })),
    fin ? el('div', { class: 'h5-rate' }, el('i', { style: { width: rate + '%' } })) : null);
  page.append(el('section', { class: 'h5-block' },
    head(t('home.activity'), el('span', { class: 'h5-dim', text: t('home.days5') })),
    el('div', { class: 'h5-stats' },
      cell(t('home.runs5'), num(sum(runs14)), runs14, t('home.runs_s', { n: num(hist.length) })),
      cell(t('home.files5'), num(sum(files14)), files14, t('home.files_s')),
      rateCell,
      cell(t('home.time5'), sum(time14) ? fmtTime(sum(time14)) : '—', time14, t('home.time_s')))));

  // templates: a graph thumbnail and a title, nothing else
  const feat = FEATURED.map((id) => TEMPLATES.find((x) => x.id === id)).filter(Boolean);
  for (const x of TEMPLATES) if (feat.length < 6 && !feat.includes(x)) feat.push(x);
  const tg = el('div', { class: 'h5-tpls' });
  for (const tpl of feat) {
    const gr = tpl.build();
    const main = gr.nodes.find((n) => !['files.load', 'files.save', 'files.preview'].includes(n.type)) || gr.nodes[0];
    const c = (CATEGORIES[(NODE_DEFS[main.type] || {}).cat] || CATEGORIES.util).color;
    tg.append(paint(el('button', { class: 'h5-tpl', onclick: () => ctx.template(tpl), title: tplText(tpl, 'desc') },
      el('div', { class: 'tp' }, thumb(tpl)),
      el('div', { class: 'tb' }, el('span', { class: 'ti' }, icon(tpl.icon || 'templates', 13)), el('b', { text: tplText(tpl, 'title') }))), c));
  }
  page.append(el('section', { class: 'h5-block' }, head(t('home.templates5'), lnk(t('home.all_templates'), () => ctx.panel('templates'))), tg));

  // recent runs
  const rows = el('div', { class: 'h5-runs' });
  if (!hist.length) rows.append(el('div', { class: 'h5-empty', text: t('home.no_runs5') }));
  for (const h of hist.slice(0, 5)) {
    const st = h.cancelled ? 'cancel' : h.ok ? 'ok' : 'fail';
    rows.append(el('div', { class: 'h5-run ' + st, title: h.error || '' },
      el('i'), el('b', { text: h.name || t('top.untitled') }),
      el('span', { text: (h.files != null ? t('nd.files', { n: num(h.files) }) + ' · ' : '') + fmtTime(h.ms || 0) }),
      el('span', { class: 'when', text: ago(h.at) })));
  }
  page.append(el('section', { class: 'h5-block' }, head(t('home.recent5'), hist.length ? lnk(t('home.history'), () => ctx.panel('history')) : null), rows));

  root.append(page);
  root.scrollTop = top;
}
