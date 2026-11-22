// Dashboard: quick actions, activity from the run history, external tools,
// templates with live graph thumbnails and the node library by category.
import { el, svgEl, icon, fmtTime } from './ui.js';
import { t, getLang, catLabel, tplText } from './i18n.js';
import { NODE_DEFS, CATEGORIES, NODE_COUNT, typeColor } from './nodes.js';
import { TEMPLATES } from './templates.js';
import { paint } from './colors.js';
import { TOOL_INFO } from './toolsui.js';

const DAYS = 14;
const DAY_MS = 86400000;
const HOME_TOOLS = ['ffmpeg', 'pandoc', 'soffice', 'gs', 'pdftoppm', 'sevenzip'];
const FEATURED = ['web-images', 'compress-video', 'excel-csv', 'docs-pdf', 'gif', 'convert-all'];

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
  const w = 112;
  const h = 30;
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

const sec = (title, sub, link) => el('div', { class: 'hm-sec-h' }, el('h2', { text: title }), sub ? el('small', { text: sub }) : null, link || null);
const lnk = (text, fn) => el('button', { class: 'lnk', onclick: fn }, el('span', { text }), icon('chevron-right', 13));

export function renderHome(root, ctx) {
  const top = root.scrollTop;
  root.textContent = '';
  const hist = (ctx.history() || []).filter((h) => h && h.at);
  const st = ctx.status() || { tools: {} };
  const tools = st.tools || {};
  const ready = HOME_TOOLS.filter((k) => tools[k] && tools[k].path).length;
  const cats = Object.keys(CATEGORIES);
  const page = el('div', { class: 'hm' });

  // header
  page.append(el('header', { class: 'hm-head' },
    el('div', { class: 'hm-t' },
      el('div', { class: 'hm-over' }, el('span', { class: 'hm-ver', text: 'Morph ' + ctx.version() }), ctx.demo ? el('span', { class: 'hm-tag', text: t('st.demo') }) : null),
      el('h1', { text: greeting() }),
      el('p', { text: t('home.sub', { n: NODE_COUNT, c: cats.length, t: TEMPLATES.length, r: ready, all: HOME_TOOLS.length }) })),
    el('button', { class: 'hm-search', onclick: () => ctx.palette() }, icon('search', 15), el('span', { text: t('home.search') }), el('kbd', { text: 'Ctrl K' }))));

  // quick actions
  const g = ctx.graph();
  const act = (ic, title, text, fn, key, cls) => el('button', { class: 'hm-act' + (cls ? ' ' + cls : ''), onclick: fn },
    el('span', { class: 'ai' }, icon(ic, 16)), key ? el('kbd', { text: key }) : null, el('b', { text: title }), el('span', { class: 'ad', text }));
  page.append(el('div', { class: 'hm-actions' },
    act('wire', t('home.continue'), (g.name || t('top.untitled')) + ' · ' + t('home.nodes_n', { n: g.nodes }), () => ctx.editor(), null, 'primary'),
    act('bolt', t('home.quick'), t('home.quick_d'), () => ctx.quick(), 'Ctrl ⇧ C'),
    act('file-plus', t('home.new'), t('home.new_d'), () => ctx.newWorkflow()),
    act('folder-open', t('home.open'), t('home.open_d'), () => ctx.openWorkflow(), 'Ctrl O')));

  // activity
  const runs14 = buckets(hist, () => 1);
  const files14 = buckets(hist, (h) => Number(h.files) || 0);
  const time14 = buckets(hist, (h) => Number(h.ms) || 0);
  const sum = (a) => a.reduce((x, y) => x + y, 0);
  const recent = hist.filter((h) => h.at >= Date.now() - DAYS * DAY_MS);
  const okN = recent.filter((h) => h.ok).length;
  const failN = recent.filter((h) => !h.ok && !h.cancelled).length;
  const cancelN = recent.filter((h) => h.cancelled).length;
  const fin = okN + failN;
  const rate = fin ? Math.round((okN / fin) * 100) : null;
  const stat = (k, v, s, viz) => el('div', { class: 'hm-stat' }, el('span', { class: 'k', text: k }), el('div', { class: 'row' }, el('span', { class: 'v', text: v }), viz), el('span', { class: 's', text: s }));
  const rateBar = el('div', { class: 'hm-rate', title: `${okN} / ${failN} / ${cancelN}` },
    el('i', { class: 'ok', style: { flexGrow: String(okN || 0) } }), el('i', { class: 'fail', style: { flexGrow: String(failN || 0) } }), el('i', { class: 'cancel', style: { flexGrow: String(cancelN || 0) } }),
    !recent.length ? el('i', { class: 'none', style: { flexGrow: '1' } }) : null);
  page.append(el('section', {},
    sec(t('home.activity'), t('home.last_days', { n: DAYS }), lnk(t('home.all_runs'), () => ctx.panel('history'))),
    el('div', { class: 'hm-stats' },
      stat(t('home.runs'), num(sum(runs14)), t('home.runs_s', { n: num(hist.length) }), bars(runs14)),
      stat(t('home.files'), num(sum(files14)), t('home.files_s'), bars(files14)),
      stat(t('home.success'), rate == null ? '—' : rate + '%', t('home.success_s', { ok: okN, fail: failN }), rateBar),
      stat(t('home.time'), fmtTime(sum(time14)), t('home.time_s'), bars(time14)))));

  // recent runs + tools
  const rows = el('div', { class: 'hm-runs' }, el('div', { class: 'hr th' }, el('span'), el('span', { text: t('home.col_name') }), el('span', { text: t('home.col_files') }), el('span', { text: t('home.col_time') }), el('span', { text: t('home.col_when') })));
  if (!hist.length) rows.append(el('div', { class: 'hm-empty' }, icon('play', 16), el('span', { text: t('home.no_runs') })));
  for (const h of hist.slice(0, 6)) {
    rows.append(el('div', { class: 'hr ' + (h.cancelled ? 'cancel' : h.ok ? 'ok' : 'fail'), title: h.error || '' },
      el('i'), el('b', { text: h.name || t('top.untitled') }),
      el('span', { text: h.files != null ? num(h.files) : '—' }), el('span', { text: fmtTime(h.ms || 0) }), el('span', { class: 'when', text: ago(h.at) })));
  }
  const tl = el('div', { class: 'hm-tools' });
  for (const k of HOME_TOOLS) {
    const info = TOOL_INFO[k];
    const s = tools[k] || {};
    const ok = !!s.path;
    const ver = ok && s.version ? (String(s.version).match(/\d+(?:\.\d+)+/) || [''])[0] : '';
    tl.append(paint(el('div', { class: 'ht' + (ok ? ' ok' : '') },
      el('span', { class: 'ti' }, icon(info.icon, 14)),
      el('b', { text: info.label }),
      el('span', { class: 'hv', text: ok ? (ver ? 'v' + ver : t('tools.ok')) : t('home.tool_missing') }),
      el('i')), info.color));
  }
  page.append(el('div', { class: 'hm-split' },
    el('section', {}, sec(t('home.recent'), null, lnk(t('home.history'), () => ctx.panel('history'))), el('div', { class: 'hm-card' }, rows)),
    el('section', {}, sec(t('home.tools'), t('home.tools_ready', { r: ready, all: HOME_TOOLS.length }), lnk(t('home.manage'), () => ctx.panel('tools'))), el('div', { class: 'hm-card' }, tl))));

  // templates
  const feat = FEATURED.map((id) => TEMPLATES.find((x) => x.id === id)).filter(Boolean);
  for (const x of TEMPLATES) if (feat.length < 6 && !feat.includes(x)) feat.push(x);
  const tg = el('div', { class: 'hm-tpls' });
  for (const tpl of feat) {
    const gr = tpl.build();
    const main = gr.nodes.find((n) => !['files.load', 'files.save', 'files.preview'].includes(n.type)) || gr.nodes[0];
    const c = (CATEGORIES[(NODE_DEFS[main.type] || {}).cat] || CATEGORIES.util).color;
    tg.append(paint(el('button', { class: 'hm-tpl', onclick: () => ctx.template(tpl) },
      el('div', { class: 'tp' }, thumb(tpl)),
      el('div', { class: 'tb' },
        el('b', {}, icon(tpl.icon || 'templates', 13), el('span', { text: tplText(tpl, 'title') })),
        el('span', { class: 'td', text: tplText(tpl, 'desc') }),
        el('span', { class: 'tm', text: t('home.nodes_n', { n: gr.nodes.length }) }))), c));
  }
  page.append(el('section', {}, sec(t('home.templates'), t('home.templates_s', { n: TEMPLATES.length }), lnk(t('home.all_templates'), () => ctx.panel('templates'))), tg));

  // node library by category
  const cl = el('div', { class: 'hm-cats' });
  for (const k of cats) {
    const info = CATEGORIES[k];
    const n = Object.values(NODE_DEFS).filter((d) => d.cat === k).length;
    if (!n) continue;
    cl.append(paint(el('button', { class: 'hm-cat', onclick: () => ctx.library(k) }, el('span', { class: 'ci' }, icon(info.icon, 13)), el('span', { text: catLabel(k) }), el('small', { text: String(n) })), info.color));
  }
  page.append(el('section', {}, sec(t('home.library'), t('lib.count', { n: NODE_COUNT }), lnk(t('home.open_library'), () => ctx.library('all'))), cl));

  page.append(el('footer', { class: 'hm-foot' },
    el('span', {}, el('kbd', { text: 'Ctrl K' }), t('home.k_search')),
    el('span', {}, el('kbd', { text: 'Tab' }), t('home.k_add')),
    el('span', {}, el('kbd', { text: 'Ctrl ⏎' }), t('home.k_run')),
    el('span', {}, el('kbd', { text: '?' }), t('home.k_help'))));
  root.append(page);
  root.scrollTop = top;
}
