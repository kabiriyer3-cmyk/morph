// App bootstrap: panels, menus, keyboard, OS drops and the run lifecycle.
import './i18n_v4.js';
import './nodes_v4.js';
import { DEMO, SMOKE, DEMO_ROOT, invoke, listen, pickFiles, saveDialog, onDrop, setToolSettings, toolSettings, openExternal, downloadText, pickLocalText } from './api.js';
import { Editor } from './editor.js';
import { Inspector } from './inspector.js';
import { NODE_DEFS, CATEGORIES, compatible } from './nodes.js';
import { NODE_TOTAL as NODE_COUNT } from './nodes_v4.js';
import { TEMPLATES, DEFAULT_TEMPLATE, TEMPLATE_GROUPS } from './templates.js';
import { Minimap } from './minimap.js';
import { QuickConvert } from './quick.js';
import { openPalette } from './palette.js';
import { renderToolsPanel, onInstallEvent, toolLabel, TOOL_ORDER } from './toolsui.js';
import { paint } from './colors.js';
import { t, setLang, detectLang, getLang, applyI18n, nodeTitle, nodeDesc, catLabel, tplText } from './i18n.js';
import { $, $$, el, icon, toast, menu, closeMenu, dialog, fmtTime, debounce, clamp } from './ui.js';
import { startBoot, replayBoot } from './boot.js';
import { renderHome } from './home.js';

const REPO = 'https://github.com/kabiriyer3-cmyk/morph';
const LS = { settings: 'morph.settings', graph: 'morph.graph', history: 'morph.history' };

const readJSON = (k, d) => {
  try {
    const v = JSON.parse(localStorage.getItem(k) || 'null');
    return v == null ? d : v;
  } catch {
    return d;
  }
};
const writeJSON = (k, v) => {
  try {
    localStorage.setItem(k, JSON.stringify(v));
  } catch {
    // storage full or disabled
  }
};

const settings = Object.assign({
  lang: '', ffmpeg: '', ffprobe: '', soffice: '', pandoc: '', gs: '', sevenzip: '', snap: true, panel: 'library',
  accent: 'cobalt', grid: 'dots', motion: true, minimap: true, desc: false, output: '', confirm: true, favs: [], recent: [], closed: [], qc_dest: 'next',
  boot: true, start: 'home',
}, readJSON(LS.settings, {}));
const ACCENTS = [['cobalt', 'Cobalt', '#3b7bff'], ['violet', 'Violet', '#8b5cf6'], ['emerald', 'Emerald', '#10b981'], ['amber', 'Amber', '#f59e0b'], ['rose', 'Rose', '#f43f5e'], ['mono', 'Graphite', '#e4e4e7']];
const MARK_PATH = 'M197.2 209.9A35.8 35.8 0 0 1 182.6 214.9A103.4 103.4 0 1 0 297.1 329.4A35.8 35.8 0 0 1 302.1 314.8ZM209.6 197A35.8 35.8 0 0 0 214.3 179.4L214.3 150.6A59.6 59.6 0 0 1 273.9 91L361.4 91A59.6 59.6 0 0 1 421 150.6L421 238.1A59.6 59.6 0 0 1 361.4 297.7L332.6 297.7A35.8 35.8 0 0 0 315 302.4Z';
const DAY_MS = 86400000;
const saveSettings = () => writeJSON(LS.settings, settings);

let editor = null;
let inspector = null;
let wfName = '';
let tools = {};
let run = null;
let version = '0.4.0';
let panel = null;
let searchBox = null;
let placed = 0;
let toolStatus = null;
let minimap = null;
let quick = null;
let libCat = 'all';
let view = 'editor';
let splash = null;
let canvasLeft = null;

const setMsg = (s) => {
  $('#st-msg').textContent = s || '';
};

const autosave = debounce(() => {
  if (SMOKE || !editor) return;
  writeJSON(LS.graph, { ...editor.serialize(), name: wfName });
}, 400);

function setName(n) {
  wfName = n || '';
  $('#wf-name').value = wfName;
  document.title = (wfName ? wfName + ' · ' : '') + 'Morph';
}

function updateStatus() {
  if (editor) $('#st-nodes').textContent = t('st.nodes', { n: editor.nodes.size });
}

function updateUndo() {
  if (!editor) return;
  $('#btn-undo').disabled = !editor.history.length;
  $('#btn-redo').disabled = !editor.future.length;
}

const safeName = (s) => [...String(s)].filter((c) => {
  const k = c.charCodeAt(0);
  return k > 31 && k !== 34 && k !== 92 && !'<>:|?*/'.includes(c);
}).join('').trim() || 'workflow';

function openPath(p) {
  if (DEMO) return toast(t('wf.demo_file'), 'warn');
  invoke('open_path', { path: p }).catch((e) => toast(String(e), 'error'));
}

function revealPath(p) {
  if (DEMO) return toast(t('wf.demo_file'), 'warn');
  invoke('reveal_path', { path: p }).catch((e) => toast(String(e), 'error'));
}

function missingTools() {
  const s = new Set();
  for (const k of ['ffmpeg', 'soffice', 'pandoc', 'gs', 'sevenzip']) if (!tools[k]) s.add(k);
  return s;
}

function applyLook() {
  const r = document.documentElement;
  r.dataset.accent = settings.accent || 'cobalt';
  r.dataset.grid = settings.grid || 'dots';
  r.dataset.motion = settings.motion === false ? 'off' : 'on';
}

const saveValues = (type) => (type === 'files.save' && settings.output ? { folder: settings.output } : undefined);

function remember(type) {
  settings.recent = [type, ...(settings.recent || []).filter((x) => x !== type)].slice(0, 8);
  saveSettings();
}

function addNodeAt(type, x, y) {
  const n = editor.addNode(type, x, y, saveValues(type));
  if (n) {
    editor.select(n.id);
    remember(type);
  }
  return n;
}

function updateWelcome() {
  const w = $('#welcome');
  if (!w || !editor) return;
  const empty = editor.nodes.size === 0;
  w.hidden = !empty;
  if (!empty || w.childNodes.length) return;
  const act = (ic, title, text, fn) => el('button', { class: 'wc-act', onclick: fn }, icon(ic, 20), el('b', { text: title }), el('span', { text }));
  w.append(el('div', { class: 'wc' },
    markIcon(30),
    el('h1', { text: t('welcome.title') }),
    el('p', { text: t('welcome.text') }),
    el('div', { class: 'wc-actions' },
      act('bolt', t('welcome.quick'), t('welcome.quick_d'), () => quick.open()),
      act('templates', t('welcome.tpl'), t('welcome.tpl_d'), () => togglePanel('templates', { force: true })),
      act('search', t('welcome.node'), t('welcome.node_d', { n: NODE_COUNT }), () => {
        const r = $('#canvas').getBoundingClientRect();
        openSearch(r.left + r.width / 2 - 170, r.top + r.height / 2 - 60, { world: editor.centerWorld() });
      })),
    el('div', { class: 'wc-foot', text: t('welcome.foot', { k: 'Ctrl + K' }) })));
}

function updateSel() {
  const n = editor ? editor.sel.size : 0;
  $('#st-sel').textContent = n > 1 ? t('st.sel', { n }) : '';
}

function toggleMinimap(force) {
  settings.minimap = force != null ? force : !settings.minimap;
  saveSettings();
  $('#minimap').hidden = !settings.minimap;
  $('#btn-minimap').classList.toggle('on', !!settings.minimap);
  $('#zoom-tools').style.bottom = '14px';
  if (minimap) minimap.update();
}

function setAccent(name) {
  settings.accent = name;
  saveSettings();
  applyLook();
  if (panel === 'settings') renderPanel('settings');
}

function paletteCtx() {
  return {
    running: () => !!run,
    run: () => (run ? cancelRun() : startRun()),
    quick: (o) => quick.open(o || {}),
    newWf: () => newWorkflow(),
    home: () => setView('home'),
    open: () => openWorkflow(),
    save: () => saveWorkflow(),
    fit: () => editor.fit(),
    arrange: () => editor.arrange(),
    minimap: () => toggleMinimap(),
    selectAll: () => editor.selectAll(),
    addNode: (type) => addAtCenter(type),
    panel: (name) => togglePanel(name, { force: true }),
    toggleLang: () => {
      settings.lang = getLang() === 'ru' ? 'en' : 'ru';
      saveSettings();
      location.reload();
    },
    help: () => showHelp(),
    clearCache: () => invoke('clear_cache').then(() => toast(t('set.cache_cleared'), 'ok')).catch((e) => toast(String(e), 'error')),
    accents: ACCENTS.map(([k, label]) => [k, label]),
    accent: (k) => setAccent(k),
    template: (tpl) => pickTemplate(tpl),
    recent: () => settings.recent || [],
  };
}

function markIcon(size) {
  const tpl = document.createElement('template');
  tpl.innerHTML = `<svg class='mark' viewBox='86 86 340 340' width='${size}' height='${size}' aria-hidden='true'><path fill='currentColor' d='${MARK_PATH}'/></svg>`;
  return tpl.content.firstElementChild;
}

// ------------------------------------------------------------------ views: dashboard and canvas
function rememberCanvas() {
  const r = $('#canvas').getBoundingClientRect();
  if (r.width) canvasLeft = r.left;
}

function setView(v, o = {}) {
  const next = v === 'home' ? 'home' : 'editor';
  if (next === view && !o.force) {
    if (next === 'home') renderHomeView();
    return;
  }
  if (view === 'editor') rememberCanvas();
  view = next;
  document.body.dataset.view = next;
  syncRail();
  document.body.classList.remove('home-panel');
  $('#home').hidden = next !== 'home';
  $('#tab-home').classList.toggle('on', next === 'home');
  $('#tab-editor').classList.toggle('on', next === 'editor');
  closeSearch();
  if (next === 'home') {
    renderHomeView();
    return;
  }
  $('#home').textContent = '';
  const left = $('#canvas').getBoundingClientRect().left;
  if (canvasLeft != null && left !== canvasLeft && editor) {
    editor.view.x -= left - canvasLeft;
    editor.applyView();
  }
  canvasLeft = left;
  if (editor) editor.redraw();
  if (minimap) minimap.update();
}

const startView = () => {
  const q = new URLSearchParams(location.search).get('view');
  if (q === 'home' || q === 'editor') return q;
  if (SMOKE) return 'editor';
  return settings.start === 'editor' ? 'editor' : 'home';
};

// Sample activity for the browser demo so the dashboard is not empty. Never saved.
function demoHistory() {
  let seed = 11;
  const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const names = ['web-images', 'compress-video', 'excel-csv', 'docs-pdf', 'gif', 'convert-all', 'music', 'pdf-merge']
    .map((id) => TEMPLATES.find((x) => x.id === id)).filter(Boolean).map((x) => tplText(x, 'title'));
  const now = Date.now();
  const out = [];
  for (let i = 0; i < 34; i++) {
    const age = i < 3 ? (i + 1) * 1500000 * rnd() : rnd() ** 0.8 * 13.6 * DAY_MS;
    const fail = rnd() < 0.08;
    const cancelled = !fail && rnd() < 0.05;
    out.push({ at: now - Math.round(age), name: names[Math.floor(rnd() * names.length)], ok: !fail && !cancelled, cancelled, error: fail ? 'ffmpeg exited with code 1' : null, ms: Math.round(700 + rnd() ** 2 * 90000), files: Math.round(2 + rnd() ** 2 * 160) });
  }
  return out.sort((a, b) => b.at - a.at);
}

function homeHistory() {
  const h = readJSON(LS.history, []);
  return h.length || !DEMO ? h : demoHistory();
}

function renderHomeView() {
  if (view !== 'home' || !editor) return;
  renderHome($('#home'), {
    version: () => version,
    demo: DEMO,
    history: () => homeHistory(),
    status: () => toolStatus || { tools: Object.fromEntries(Object.entries(tools).map(([k, v]) => [k, v || {}])) },
    graph: () => ({ name: wfName, nodes: editor.nodes.size, links: editor.links.size }),
    editor: () => setView('editor'),
    quick: () => quick.open(),
    newWorkflow: () => newWorkflow(),
    openWorkflow: () => openWorkflow(),
    palette: () => openPalette(paletteCtx()),
    panel: (name) => togglePanel(name, { force: true }),
    template: (tpl) => pickTemplate(tpl),
    library: (cat) => {
      libCat = cat || 'all';
      setView('editor');
      togglePanel('library', { force: true });
    },
  });
}

async function newWorkflow() {
  if (editor.nodes.size && !(await dialog({ title: t('top.new'), text: t('wf.new') }))) return;
  setView('editor');
  editor.load({ nodes: [], links: [] });
  const a = editor.addNode('files.load', 40, 80, DEMO ? { paths: [DEMO_ROOT] } : {}, { silent: true });
  const b = editor.addNode('files.preview', 420, 80, {}, { silent: true });
  editor.connect(a.id, 0, b.id, 0, { silent: true });
  updateWelcome();
  updateStatus();
  setName('');
  editor.resetHistory();
  requestAnimationFrame(() => editor.fit());
  autosave();
}

async function pickTemplate(tpl) {
  if (settings.confirm !== false && editor.nodes.size && !(await dialog({ title: tplText(tpl, 'title'), text: t('tpl.replace', { name: tplText(tpl, 'title') }) }))) return;
  loadTemplate(tpl);
}

// ------------------------------------------------------------------ boot
async function boot() {
  setLang(settings.lang || detectLang());
  splash = startBoot();
  splash.version(version);
  splash.step(t('boot.ui'), 0.1);
  applyLook();
  applyI18n(document.body);
  $$('[data-icon]').forEach((b) => {
    const size = b.closest('#drop-hint') ? 36 : b.classList.contains('sb-btn') ? 18 : b.closest('.tb-pill, .tab, .dock-add') ? 15 : 16;
    b.prepend(icon(b.dataset.icon, size));
  });
  $$('[data-label-key]').forEach((b) => {
    b.append(el('span', { class: 'sb-l', text: t(b.dataset.labelKey) }));
  });
  $('#st-ver').textContent = 'v' + version + (DEMO ? ' · ' + t('st.demo') : '');
  $('#sb-ver').textContent = 'Studio · v' + version;
  setMsg(t('st.ready'));
  setToolSettings(settings);

  editor = new Editor($('#canvas'), {
    snap: settings.snap !== false,
    onSelect: (n) => {
      if (n) inspector.showNode(n);
      else inspector.showEmpty();
      updateSel();
      if (minimap) minimap.update();
    },
    onChange: () => {
      autosave();
      updateStatus();
      updateUndo();
      updateWelcome();
      refreshEmpty();
      if (minimap) minimap.update();
    },
    onHistory: () => updateUndo(),
    onSearch: (x, y, o) => openSearch(x, y, o),
    onContextMenu: (e, info) => contextMenu(e, info),
    onFile: (f, n) => inspector.showFile(f, n),
    onOpenFile: (f) => openPath(f.path),
    onReveal: (p) => revealPath(p),
    onView: (v) => {
      $('#zoom-label').textContent = Math.round(v.z * 100) + '%';
      if (minimap) minimap.update();
    },
    onAdv: () => {
      autosave();
      if (minimap) minimap.update();
    },
    onPointerDown: () => closeSearch(),
    onRename: (n) => {
      const c = inspector.current;
      if (c && c.kind === 'node' && c.id === n.id) inspector.showNode(n);
    },
  });
  splash.step(t('boot.nodes', { n: NODE_COUNT }), 0.3);
  inspector = new Inspector($('#inspector'), { editor, open: openPath, reveal: revealPath, missing: missingTools, tools: () => togglePanel('tools', { force: true }), name: () => wfName, last: () => homeHistory()[0] || null });
  minimap = new Minimap($('#minimap'), editor);
  quick = new QuickConvert({
    settings,
    save: saveSettings,
    tools: () => tools,
    busy: () => !!run,
    claim: (q) => {
      run = { quick: q, total: 1, done: 0, cur: 0, t0: performance.now(), files: 0 };
    },
    release: () => {
      run = null;
    },
    reveal: (p) => revealPath(p),
    history: (h) => pushHistory(h),
    load: (g, name) => {
      setView('editor');
      editor.loadIndexed(g);
      setName(name);
      editor.resetHistory();
      requestAnimationFrame(() => editor.fit());
      autosave();
    },
  });

  bindTopbar();
  bindSidebar();
  bindKeys();
  bindDrop();
  await listen('run-event', onRunEvent);
  await listen('tool-install', (ev) => onInstallEvent(ev, (e) => {
    if (e.stage === 'done') {
      toast(t('tools.done', { tool: toolLabel(e.tool) }), 'ok');
      detectTools();
    } else if (e.stage === 'error') {
      toast(t('tools.failed', { tool: toolLabel(e.tool), err: e.text || '' }), 'error');
      detectTools();
    } else if (panel === 'tools') renderPanel('tools');
  }));
  restoreGraph();
  splash.step(t('boot.workflow'), 0.5);
  toggleMinimap(settings.minimap !== false);
  updateWelcome();
  updateStatus();
  updateUndo();
  setView(startView(), { force: true });
  invoke('system_info')
    .then((s) => {
      if (s && s.version && !DEMO) version = s.version;
      $('#st-ver').textContent = 'v' + version + (DEMO ? ' · ' + t('st.demo') : '');
      $('#sb-ver').textContent = 'Studio · v' + version;
      splash.version(version);
      if (view === 'home') renderHomeView();
    })
    .catch(() => {});
  splash.step(t('boot.tools'), 0.72);
  await Promise.race([detectTools(), new Promise((r) => setTimeout(r, 1500))]);
  splash.step(t('boot.ready'), 1);
  if (SMOKE) startSmoke();
  else document.body.dataset.ready = '1';
  splash.done();
}

const refreshEmpty = debounce(() => {
  if (inspector && !inspector.current) inspector.showEmpty();
}, 160);

function restoreGraph() {
  const g = SMOKE ? null : readJSON(LS.graph, null);
  if (g && Array.isArray(g.nodes) && g.nodes.length) {
    editor.load(g);
    setName(g.name || '');
    editor.resetHistory();
    if (!g.view) requestAnimationFrame(() => editor.fit());
    return;
  }
  loadTemplate(TEMPLATES.find((x) => x.id === DEFAULT_TEMPLATE) || TEMPLATES[0], true);
}

function loadTemplate(tpl, quiet) {
  if (!quiet) setView('editor');
  const g = tpl.build();
  if (DEMO) {
    for (const n of g.nodes) if (n.type === 'files.load') n.values = { ...n.values, paths: [DEMO_ROOT] };
  }
  editor.loadIndexed(g);
  setName(tplText(tpl, 'title'));
  editor.resetHistory();
  inspector.showEmpty();
  requestAnimationFrame(() => editor.fit());
  if (!quiet) toast(tplText(tpl, 'title'), 'ok', 1800);
  autosave();
}

// ------------------------------------------------------------------ top bar
function bindTopbar() {
  $('#btn-new').onclick = () => newWorkflow();
  $('#brand').onclick = () => setView('home');
  $('#tab-home').onclick = () => setView('home');
  $('#tab-editor').addEventListener('click', () => setView('editor'));
  $('#dock-add').onclick = () => {
    const r = $('#canvas').getBoundingClientRect();
    openSearch(r.left + r.width / 2 - 190, Math.max(r.top + 12, r.bottom - 500), { world: editor.centerWorld() });
  };
  $('#dock-note').onclick = () => addAtCenter('util.note');
  $('#btn-open').onclick = () => openWorkflow();
  $('#btn-save').onclick = () => saveWorkflow();
  $('#btn-undo').onclick = () => editor.undo();
  $('#btn-redo').onclick = () => editor.redo();
  $('#btn-fit').onclick = () => (view === 'editor' ? editor.fit() : setView('editor'));
  $('#btn-arrange').onclick = () => editor.arrange();
  $('#btn-run').onclick = () => (run ? cancelRun() : startRun());
  $('#btn-minimap').onclick = () => toggleMinimap();
  $('#btn-palette').onclick = () => openPalette(paletteCtx());
  $('#btn-quick').onclick = () => quick.open();
  $('#zoom-in').onclick = () => editor.zoomBy(1.2);
  $('#zoom-out').onclick = () => editor.zoomBy(1 / 1.2);
  $('#zoom-label').onclick = () => editor.fit();
  const nm = $('#wf-name');
  nm.addEventListener('input', () => {
    wfName = nm.value;
    document.title = (wfName ? wfName + ' · ' : '') + 'Morph';
    autosave();
  });
  nm.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === 'Escape') nm.blur();
  });
}

// ------------------------------------------------------------------ side panels
function bindSidebar() {
  $$('#sidebar .sb-btn[data-panel]').forEach((b) => b.addEventListener('click', () => togglePanel(b.dataset.panel)));
  $('#btn-help').onclick = () => showHelp();
  $('#sb-quick').onclick = () => quick.open();
  $('#sb-share').onclick = () => quick.open({ cat: 'Share', profile: 's:discord' });
  $('#sb-home').onclick = () => setView('home');
  $('#sb-editor').onclick = () => setView('editor');
  $('#sb-ws').onclick = () => setView('home');
  $('#sb-search').onclick = () => openPalette(paletteCtx());
  $('#sb-engine').onclick = () => togglePanel('tools', { force: true });
  $('#sb-collapse').onclick = () => {
    settings.rail = $('#sidebar').classList.contains('compact') ? 'expanded' : 'compact';
    saveSettings();
    syncRail(true);
  };
  let rz = 0;
  window.addEventListener('resize', () => {
    cancelAnimationFrame(rz);
    rz = requestAnimationFrame(() => syncRail(true));
  });
  $('#panel-close').onclick = () => togglePanel(null);
  togglePanel(settings.panel || null, { initial: true });
}

// Sidebar: full width with labels and counts; compact (icons) next to an open panel in the
// editor, on small windows or when the user collapses it. The pill indicator slides to the
// open panel, or to the current view.
function syncRail(adjust = false) {
  const sb = $('#sidebar');
  if (!sb) return;
  const cv = $('#canvas');
  const before = adjust && view === 'editor' && cv ? cv.getBoundingClientRect().left : null;
  const auto = (view === 'editor' && !!panel) || window.innerWidth < 1180;
  const compact = settings.rail === 'compact' || (settings.rail !== 'expanded' && auto) || (settings.rail === 'expanded' && window.innerWidth < 1000);
  sb.classList.toggle('compact', compact);
  const col = $('#sb-collapse');
  if (col) col.title = t(compact ? 'sb.expand' : 'sb.collapse');
  $('#sb-home').classList.toggle('current', view === 'home');
  $('#sb-editor').classList.toggle('current', view === 'editor');
  const counts = { library: NODE_COUNT, templates: TEMPLATES.length, history: homeHistory().length };
  for (const [k, n] of Object.entries(counts)) {
    const b = sb.querySelector(`.sb-btn[data-panel='${k}']`);
    if (!b) continue;
    let c = b.querySelector('.sb-n');
    if (!c) {
      c = el('span', { class: 'sb-n' });
      b.append(c);
    }
    c.textContent = String(n);
  }
  requestAnimationFrame(placeRailIndicator);
  if (before != null && editor) {
    const after = cv.getBoundingClientRect().left;
    if (after !== before) {
      editor.view.x -= after - before;
      editor.applyView();
    }
    rememberCanvas();
  }
}

function placeRailIndicator() {
  const ind = $('#sidebar .sb-ind');
  if (!ind) return;
  const shown = !!panel && !(view === 'home' && !document.body.classList.contains('home-panel'));
  const target = (shown && $('#sidebar .sb-btn.active')) || $('#sidebar .sb-btn.current');
  if (!target) {
    ind.style.opacity = '0';
    return;
  }
  ind.style.opacity = '1';
  ind.style.height = target.offsetHeight + 'px';
  ind.style.width = target.offsetWidth + 'px';
  ind.style.transform = `translate(${target.offsetLeft}px, ${target.offsetTop}px)`;
}

function togglePanel(name, o = {}) {
  // On the dashboard the side panel stays hidden until a rail button asks for it.
  if (view === 'home' && name && !document.body.classList.contains('home-panel')) {
    document.body.classList.add('home-panel');
    if (name === panel) {
      renderPanel(name);
      syncRail();
      return;
    }
    o = { ...o, force: true };
  }
  let next = name;
  if (!o.initial && !o.force && next === panel) next = null;
  const canvas = $('#canvas');
  const before = canvas.getBoundingClientRect().left;
  panel = next;
  $$('#sidebar .sb-btn[data-panel]').forEach((b) => b.classList.toggle('active', b.dataset.panel === next));
  $('#panel').hidden = !next;
  syncRail();
  if (next) renderPanel(next);
  if (!o.initial) {
    settings.panel = next;
    saveSettings();
    const after = canvas.getBoundingClientRect().left;
    if (after !== before && editor && view === 'editor') {
      editor.view.x -= after - before;
      editor.applyView();
    }
    if (view === 'editor') rememberCanvas();
  }
}

function renderPanel(name) {
  $('#panel-title').textContent = t('side.' + name);
  $('#panel-count').textContent = name === 'library' ? t('lib.count', { n: NODE_COUNT }) : '';
  const body = $('#panel-body');
  const top = body.scrollTop;
  body.textContent = '';
  if (name === 'library') renderLibrary(body);
  else if (name === 'templates') renderTemplates(body);
  else if (name === 'history') renderHistory(body);
  else if (name === 'settings') renderSettings(body);
  else if (name === 'tools') {
    renderToolsPanel(body, {
      status: () => toolStatus,
      settings,
      save: () => {
        saveSettings();
        setToolSettings(settings);
        detectTools();
      },
      detect: () => detectTools(),
      install: (k) => installTool(k),
      remove: (k) => removeTool(k),
    });
    body.scrollTop = top;
    return;
  }
  body.scrollTop = 0;
}

async function installTool(k) {
  if (DEMO) return toast(t('tools.demo'), 'warn');
  onInstallEvent({ tool: k, stage: 'download', value: 0, text: t('tools.installing', { tool: toolLabel(k) }) }, () => {});
  if (panel === 'tools') renderPanel('tools');
  try {
    await invoke('install_tool', { name: k });
  } catch (e) {
    onInstallEvent({ tool: k, stage: 'error', value: 0, text: String(e) }, () => {});
    toast(t('tools.failed', { tool: toolLabel(k), err: String(e) }), 'error');
  }
  detectTools();
}

async function removeTool(k) {
  if (!(await dialog({ title: t('tools.remove') + ' ' + toolLabel(k), text: t('tools.remove') + ' ' + toolLabel(k) + '?', danger: true, okText: t('tools.remove') }))) return;
  try {
    await invoke('remove_tool', { name: k });
    toast(t('tools.removed', { tool: toolLabel(k) }), 'ok');
  } catch (e) {
    toast(String(e), 'error');
  }
  detectTools();
}

function matches(q, type, def) {
  if (!q) return true;
  const hay = [nodeTitle(type), def.title, type, nodeDesc(type), def.keywords || ''].join(' ').toLowerCase();
  return q.split(' ').filter(Boolean).every((w) => hay.includes(w));
}

function addAtCenter(type) {
  setView('editor');
  const c = editor.centerWorld();
  const off = (placed++ % 6) * 24;
  addNodeAt(type, c.x - 140 + off, c.y - 100 + off);
}

function libItem(type) {
  const d = NODE_DEFS[type];
  const c = CATEGORIES[d.cat] || CATEGORIES.util;
  const fav = (settings.favs || []).includes(type);
  const star = el('button', {
    class: 'fav' + (fav ? ' on' : ''),
    title: fav ? t('lib.fav_remove') : t('lib.fav_add'),
    onpointerdown: (e) => e.stopPropagation(),
    onclick: (e) => {
      e.stopPropagation();
      const list = settings.favs || [];
      settings.favs = list.includes(type) ? list.filter((x) => x !== type) : [...list, type];
      saveSettings();
      renderPanel('library');
    },
  }, icon(fav ? 'star-fill' : 'star', 14));
  const need = d.needs && !tools[d.needs] ? el('span', { class: 'need', text: toolLabel(d.needs) }) : null;
  const it = paint(el('div', { class: 'lib-item', dataset: { type }, title: nodeDesc(type) },
    el('span', { class: 'li' }, icon(d.icon || c.icon, 16)),
    el('div', { class: 'lt' }, el('b', { text: nodeTitle(type) }), el('span', { text: nodeDesc(type) })),
    need, star), d.color || c.color);
  it.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || e.target.closest('.fav')) return;
    e.preventDefault();
    const x0 = e.clientX;
    const y0 = e.clientY;
    let ghost = null;
    try {
      it.setPointerCapture(e.pointerId);
    } catch {
      // ignore
    }
    const move = (ev) => {
      if (!ghost && Math.hypot(ev.clientX - x0, ev.clientY - y0) < 5) return;
      if (!ghost) {
        ghost = paint(el('div', { class: 'drag-ghost' }, el('span', { class: 'dot' }), nodeTitle(type)), c.color);
        document.body.append(ghost);
      }
      ghost.style.left = ev.clientX + 'px';
      ghost.style.top = ev.clientY + 'px';
    };
    const up = (ev) => {
      it.removeEventListener('pointermove', move);
      it.removeEventListener('pointerup', up);
      it.removeEventListener('pointercancel', up);
      if (!ghost) {
        if (ev.type === 'pointerup') addAtCenter(type);
        return;
      }
      ghost.remove();
      const hit = document.elementFromPoint(ev.clientX, ev.clientY);
      if (hit && hit.closest && hit.closest('#canvas')) {
        const w = editor.toWorld(ev.clientX, ev.clientY);
        addNodeAt(type, w.x - 20, w.y - 15);
      }
    };
    it.addEventListener('pointermove', move);
    it.addEventListener('pointerup', up);
    it.addEventListener('pointercancel', up);
  });
  return it;
}

function renderLibrary(body) {
  const input = el('input', { class: 'search', placeholder: t('lib.search'), spellcheck: 'false' });
  const chips = el('div', { class: 'lib-cats' });
  const list = el('div', {});
  const closed = new Set(settings.closed || []);
  const chip = (key, label, color) => {
    const b = el('button', {
      class: libCat === key ? 'on' : '',
      onclick: () => {
        libCat = key;
        renderPanel('library');
      },
    }, color ? el('span', { class: 'dot', style: { '--c': color } }) : null, label);
    if (color) b.style.setProperty('--c', color);
    return b;
  };
  chips.append(chip('all', t('lib.all')));
  if ((settings.favs || []).length) chips.append(chip('fav', '★ ' + t('lib.favorites'), '#fbbf24'));
  for (const [cat, c] of Object.entries(CATEGORIES)) chips.append(chip(cat, catLabel(cat), c.color));
  const section = (key, label, color, ic, types, q) => {
    if (!types.length) return 0;
    const isClosed = !q && closed.has(key);
    const head = paint(el('div', {
      class: 'lib-cat' + (isClosed ? ' closed' : ''),
      onclick: () => {
        if (closed.has(key)) closed.delete(key);
        else closed.add(key);
        settings.closed = [...closed];
        saveSettings();
        renderPanel('library');
      },
    }, el('span', { class: 'ci' }, icon(ic, 12)), label, el('span', { class: 'n', text: String(types.length) }), el('span', { class: 'chev' }, icon('chevron-down', 13))), color);
    list.append(head);
    if (!isClosed) for (const type of types) list.append(libItem(type));
    return types.length;
  };
  const fill = () => {
    const q = input.value.trim().toLowerCase();
    list.textContent = '';
    let count = 0;
    const all = Object.entries(NODE_DEFS).filter(([type, d]) => matches(q, type, d));
    if (!q && libCat === 'all') {
      const favs = (settings.favs || []).filter((x) => NODE_DEFS[x]);
      count += section('_fav', t('lib.favorites'), '#fbbf24', 'star', favs, q);
      const rec = (settings.recent || []).filter((x) => NODE_DEFS[x] && !favs.includes(x)).slice(0, 5);
      count += section('_recent', t('lib.recent'), '#8b93a7', 'history', rec, q);
    }
    if (libCat === 'fav') {
      count += section('_fav', t('lib.favorites'), '#fbbf24', 'star', all.filter(([type]) => (settings.favs || []).includes(type)).map(([type]) => type), q);
    } else {
      for (const [cat, c] of Object.entries(CATEGORIES)) {
        if (libCat !== 'all' && libCat !== cat) continue;
        count += section(cat, catLabel(cat), c.color, c.icon, all.filter(([, d]) => d.cat === cat).map(([type]) => type), q);
      }
    }
    if (!count) list.append(el('div', { class: 'lib-empty', text: t('lib.none') }));
  };
  input.addEventListener('input', fill);
  input.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter') return;
    const first = list.querySelector('.lib-item');
    if (first) addAtCenter(first.dataset.type);
  });
  body.append(el('div', { class: 'search-wrap' }, input, icon('search', 14)), chips, el('div', { class: 'lib-hint', text: t('lib.hint') }), list);
  fill();
  setTimeout(() => input.focus({ preventScroll: true }), 0);
}

function renderTemplates(body) {
  const list = el('div', { class: 'tpl-list' }, el('div', { class: 'lib-hint', style: { padding: '0 2px' }, text: t('tpl.hint') }));
  for (const g of TEMPLATE_GROUPS) {
    const tpls = TEMPLATES.filter((x) => (x.group || 'files') === g);
    if (!tpls.length) continue;
    list.append(el('div', { class: 'tpl-group', text: t('tpl.g.' + g) }));
    for (const tpl of tpls) {
      const gr = tpl.build();
      const main = gr.nodes.find((n) => !['files.load', 'files.save', 'files.preview'].includes(n.type)) || gr.nodes[0];
      const mc = CATEGORIES[NODE_DEFS[main.type].cat] || CATEGORIES.util;
      const chain = el('div', { class: 'tpl-chain' }, gr.nodes.filter((n) => n.type !== 'files.load').map((n) => el('span', {
        style: { '--c': (CATEGORIES[NODE_DEFS[n.type].cat] || CATEGORIES.util).color },
        text: nodeTitle(n.type),
      })));
      list.append(paint(el('div', { class: 'tpl-card', onclick: () => pickTemplate(tpl) },
        el('h4', {}, icon(tpl.icon || 'templates', 15), tplText(tpl, 'title')), el('p', { text: tplText(tpl, 'desc') }), chain), mc.color));
    }
  }
  body.append(list);
}

function renderHistory(body) {
  const items = readJSON(LS.history, []);
  if (!items.length) {
    body.append(el('div', { class: 'lib-empty', text: t('queue.empty') }));
    return;
  }
  body.append(el('div', { class: 'pad' }, el('button', {
    class: 'btn sm ghost',
    onclick: () => {
      writeJSON(LS.history, []);
      renderPanel('history');
    },
  }, icon('trash', 12), t('queue.clear'))));
  for (const h of items) {
    const when = new Date(h.at).toLocaleString();
    const extra = (h.files != null ? ' · ' + t('queue.files', { n: h.files }) : '') + (h.error ? ' · ' + h.error : '');
    body.append(el('div', { class: 'hist-item' + (h.cancelled ? ' cancel' : h.ok ? '' : ' fail'), title: h.error || '' },
      el('i'), el('b', { text: h.name || t('top.untitled') }), el('em', { text: fmtTime(h.ms || 0) }), el('small', { text: when + extra })));
  }
}



function renderSettings(body) {
  const row = (label, ctl, hint) => el('label', { class: 'set-row' }, el('span', {}, label, hint ? el('small', { text: hint }) : null), ctl);
  const sw = (on, fn) => {
    const inp = el('input', { type: 'checkbox' });
    inp.checked = !!on;
    inp.addEventListener('change', () => fn(inp.checked));
    return el('span', { class: 'switch' }, inp, el('i'));
  };
  const langSel = el('select', { class: 'select' }, el('option', { value: 'en', text: 'English' }), el('option', { value: 'ru', text: 'Русский' }));
  langSel.value = getLang();
  langSel.addEventListener('change', () => {
    settings.lang = langSel.value;
    saveSettings();
    location.reload();
  });
  body.append(el('div', { class: 'set-group' }, el('h3', {}, icon('globe', 13), t('set.language')), row(t('set.language'), langSel, t('set.lang_hint'))));

  const swatches = el('div', { class: 'swatches' }, ACCENTS.map(([k, label, c]) => el('button', {
    class: settings.accent === k ? 'on' : '',
    title: label,
    style: { '--g': c },
    onclick: () => setAccent(k),
  })));
  const grid = el('div', { class: 'seg-ctl', style: { width: '190px' } }, ['dots', 'lines', 'none'].map((g) => el('button', {
    class: (settings.grid || 'dots') === g ? 'on' : '',
    text: t('set.grid_' + g),
    onclick: () => {
      settings.grid = g;
      saveSettings();
      applyLook();
      renderPanel('settings');
    },
  })));
  const startSeg = el('div', { class: 'seg-ctl', style: { width: '170px' } }, ['home', 'editor'].map((g) => el('button', {
    class: (settings.start || 'home') === g ? 'on' : '',
    text: t('set.start_' + g),
    onclick: (e) => {
      e.preventDefault();
      settings.start = g;
      saveSettings();
      renderPanel('settings');
    },
  })));
  body.append(el('div', { class: 'set-group' },
    el('h3', {}, icon('sun', 13), t('set.appearance')),
    row(t('set.accent'), swatches),
    row(t('set.grid'), grid),
    row(t('set.minimap'), sw(settings.minimap !== false, (v) => toggleMinimap(v))),
    row(t('set.motion'), sw(settings.motion !== false, (v) => {
      settings.motion = v;
      saveSettings();
      applyLook();
    })),
    row(t('set.boot'), el('span', { class: 'set-ctl' },
      sw(settings.boot !== false, (v) => {
        settings.boot = v;
        saveSettings();
      }),
      el('button', { class: 'btn sm ghost', title: t('set.boot_play'), onclick: (e) => {
        e.preventDefault();
        replayBoot(version);
      } }, icon('play', 11), t('set.boot_play'))), t('set.boot_hint')),
    row(t('set.start'), startSeg, t('set.start_hint'))));

  const out = el('input', { class: 'field', placeholder: '~/Morph Output', spellcheck: 'false', value: settings.output || '' });
  out.addEventListener('change', () => {
    settings.output = out.value.trim();
    saveSettings();
  });
  const browse = el('button', {
    class: 'btn sm',
    title: t('wid.browse'),
    onclick: async () => {
      const r = await pickFiles({ folder: true, multiple: false });
      if (r[0]) {
        out.value = r[0];
        settings.output = r[0];
        saveSettings();
      }
    },
  }, icon('folder', 12));
  body.append(el('div', { class: 'set-group' },
    el('h3', {}, icon('arrange', 13), t('set.editor')),
    row(t('set.snap'), sw(settings.snap !== false, (v) => {
      settings.snap = v;
      editor.snap = v;
      saveSettings();
    })),
    row(t('set.confirm'), sw(settings.confirm !== false, (v) => {
      settings.confirm = v;
      saveSettings();
    })),
    el('p', { style: { margin: '10px 0 6px' }, text: t('set.output_hint') }),
    el('div', { class: 'tool-row', style: { marginTop: '0' } }, out, browse)));

  const missing = TOOL_ORDER.filter((k) => !(toolStatus && toolStatus.tools && toolStatus.tools[k] && toolStatus.tools[k].path));
  body.append(el('div', { class: 'set-group' },
    el('h3', {}, icon('package', 13), t('set.tools')),
    el('p', { text: t('tools.hint') }),
    el('div', { class: 'links-row' },
      el('button', { class: 'btn sm primary', onclick: () => togglePanel('tools', { force: true }) }, icon('package', 12), t('cmd.tools') + (missing.length ? ' · ' + missing.length : '')))));

  body.append(el('div', { class: 'set-group' },
    el('h3', {}, icon('trash', 13), t('set.cache')),
    el('p', { text: t('set.cache_hint') }),
    el('button', {
      class: 'btn sm',
      onclick: () => invoke('clear_cache').then(() => toast(t('set.cache_cleared'), 'ok')).catch((e) => toast(String(e), 'error')),
    }, icon('trash', 12), t('set.clear_cache'))));

  body.append(el('div', { class: 'set-group' },
    el('h3', {}, icon('info', 13), t('set.about')),
    el('div', { class: 'about' }, el('img', { src: 'assets/logo.svg', alt: '' }), el('div', {}, el('b', { text: 'Morph' }), el('div', { class: 'muted', text: 'v' + version + ' · ' + t('app.tagline') }), el('div', { class: 'muted', style: { fontSize: '11px' }, text: t('lib.count', { n: NODE_COUNT }) }))),
    DEMO ? el('p', { style: { marginTop: '10px' }, text: t('set.demo') }) : null,
    el('div', { class: 'links-row' },
      el('button', { class: 'btn sm', onclick: () => openExternal(REPO) }, icon('git', 12), 'GitHub'),
      el('button', { class: 'btn sm', onclick: () => openExternal(REPO + '/releases') }, icon('download', 12), 'Releases'))));
}

async function detectTools() {
  try {
    toolStatus = await invoke('tool_status', { settings: toolSettings() });
    tools = {};
    for (const [k, v] of Object.entries((toolStatus && toolStatus.tools) || {})) tools[k] = v && v.path ? v : null;
  } catch {
    toolStatus = null;
    try {
      tools = (await invoke('detect_tools', { settings: toolSettings() })) || {};
    } catch {
      tools = {};
    }
  }
  renderToolChips();
  editor.setMissing(missingTools());
  if (view === 'home') renderHomeView();
  if (panel === 'tools' || panel === 'library') renderPanel(panel);
  inspector.refresh();
}

function renderToolChips() {
  const box = $('#st-tools');
  box.textContent = '';
  if (DEMO) box.append(el('span', { class: 'chip demo', text: 'browser demo' }));
  for (const k of ['ffmpeg', 'pandoc', 'soffice', 'gs', 'sevenzip']) {
    const ok = !!tools[k];
    const v = tools[k] && tools[k].version ? String(tools[k].version).split(/\r?\n/)[0] : '';
    const c = el('span', { class: 'chip ' + (ok ? (tools[k].managed ? 'managed' : 'ok') : 'bad'), title: ok ? v || String(tools[k].path) : t('st.missing', { tool: toolLabel(k) }) }, el('i'), toolLabel(k));
    c.style.cursor = 'pointer';
    c.addEventListener('click', () => togglePanel('tools', { force: true }));
    box.append(c);
  }
  const miss = ['ffmpeg', 'pandoc', 'soffice'].filter((k) => !tools[k]).length;
  const have = ['ffmpeg', 'pandoc', 'soffice', 'gs', 'sevenzip'].filter((k) => !!tools[k]).length;
  const es = $('#sb-engine-s');
  if (es) es.textContent = t('sb.engine_n', { n: have, total: 5 });
  const ring = $('#sb-engine .fg');
  if (ring) ring.style.strokeDashoffset = String(94.25 * (1 - have / 5));
  const eng = $('#sb-engine');
  if (eng) eng.classList.toggle('warn', have < 3);
  const b = document.querySelector('.sb-btn[data-panel="tools"]');
  if (b) {
    let dot = b.querySelector('.sb-badge');
    if (miss && !dot) b.append(el('span', { class: 'sb-badge' }));
    if (!miss && dot) dot.remove();
  }
}

function showHelp() {
  const keys = [
    ['Ctrl + Enter', 'help.k_run'], ['Ctrl + K', 'help.k_palette'], ['Tab', 'help.k_search'], ['Ctrl + Shift + C', 'help.k_quick'],
    ['Del', 'help.k_delete'], ['Ctrl + C / V', 'help.k_copy'], ['Ctrl + D', 'help.k_dup'], ['Ctrl + Z / Y', 'help.k_undo'],
    ['Ctrl + B', 'help.k_bypass'], ['F', 'help.k_fit'], ['M', 'help.k_minimap'], ['Space + drag', 'help.k_pan'],
    ['Wheel', 'help.k_zoom'], ['Ctrl + drag', 'help.k_box'], ['Ctrl + S / O', 'help.k_save'],
  ];
  const body = el('div', {},
    el('h4', { text: t('help.start') }),
    el('ol', {}, ['help.s1', 'help.s2', 'help.s3', 'help.s4'].map((k) => el('li', { text: t(k) }))),
    el('h4', { text: t('help.keys') }),
    el('div', { class: 'keys' }, keys.flatMap(([k, d]) => [el('kbd', { text: k }), el('span', { text: t(d) })])));
  dialog({ title: t('help.title'), body, cancel: false, wide: true });
}

// ------------------------------------------------------------------ node search popup
function searchOutside(e) {
  if (searchBox && !searchBox.contains(e.target)) closeSearch();
}

function closeSearch() {
  if (!searchBox) return;
  searchBox.remove();
  searchBox = null;
  document.removeEventListener('pointerdown', searchOutside, true);
}

function openSearch(cx, cy, o = {}) {
  closeSearch();
  closeMenu();
  const input = el('input', { placeholder: t('menu.search'), spellcheck: 'false' });
  const list = el('div', { class: 'sp-list' });
  const box = el('div', { class: 'search-pop' }, input, list);
  let items = [];
  let idx = 0;
  const fits = (d) => {
    const l = o.link;
    if (!l) return true;
    return l.dir === 'out' ? d.inputs.some((s) => compatible(l.type, s.type)) : d.outputs.some((s) => compatible(s.type, l.type));
  };
  const pick = (type) => {
    closeSearch();
    const w = o.world || editor.centerWorld();
    const n = editor.addNode(type, w.x - (o.link && o.link.dir === 'in' ? 300 : 0), w.y - 15, saveValues(type));
    if (!n) return;
    remember(type);
    if (o.link) editor.autoConnect(o.link, n.id);
    editor.select(n.id);
  };
  const mark = () => list.querySelectorAll('.sp-item').forEach((x, j) => x.classList.toggle('active', j === idx));
  const render = () => {
    list.textContent = '';
    if (!items.length) {
      list.append(el('div', { class: 'sp-empty', text: t('lib.none') }));
      return;
    }
    items.forEach(([type, d], i) => {
      const c = CATEGORIES[d.cat] || CATEGORIES.util;
      const row = paint(el('div', { class: 'sp-item' + (i === idx ? ' active' : ''), onclick: () => pick(type) },
        el('span', { class: 'si' }, icon(d.icon || c.icon, 14)), el('b', { text: nodeTitle(type) }), el('small', { text: catLabel(d.cat) })), c.color);
      row.addEventListener('pointerenter', () => {
        idx = i;
        mark();
      });
      list.append(row);
    });
  };
  const fill = () => {
    const q = input.value.trim().toLowerCase();
    items = Object.entries(NODE_DEFS).filter(([type, d]) => fits(d) && matches(q, type, d));
    idx = 0;
    render();
  };
  input.addEventListener('input', fill);
  input.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      idx = clamp(idx + (e.key === 'ArrowDown' ? 1 : -1), 0, Math.max(0, items.length - 1));
      mark();
      const a = list.querySelector('.sp-item.active');
      if (a) a.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (items[idx]) pick(items[idx][0]);
    } else if (e.key === 'Escape') {
      closeSearch();
    }
  });
  document.body.append(box);
  box.style.left = clamp(cx - 20, 8, window.innerWidth - 350) + 'px';
  box.style.top = clamp(cy - 20, 8, window.innerHeight - 420) + 'px';
  searchBox = box;
  fill();
  input.focus();
  setTimeout(() => document.addEventListener('pointerdown', searchOutside, true), 0);
}

// ------------------------------------------------------------------ context menus
async function addPaths(n, folder) {
  const r = await pickFiles({ folder, multiple: true });
  if (r.length) editor.setValue(n.id, 'paths', [...new Set([...(n.values.paths || []), ...r])]);
}

function contextMenu(e, info) {
  const x = e.clientX;
  const y = e.clientY;
  if (info.kind === 'canvas') {
    const cats = Object.entries(CATEGORIES).map(([cat, c]) => ({
      label: catLabel(cat),
      dot: c.color,
      sub: Object.entries(NODE_DEFS).filter(([, d]) => d.cat === cat).map(([type, d]) => ({
        label: nodeTitle(type),
        icon: d.icon || c.icon,
        onClick: () => addNodeAt(type, info.world.x, info.world.y),
      })),
    }));
    menu([
      { label: t('menu.add_node'), icon: 'plus', sub: cats },
      { label: t('menu.search'), icon: 'search', hint: 'Tab', onClick: () => openSearch(x, y, { world: info.world }) },
      { label: t('top.quick'), icon: 'bolt', hint: 'Ctrl+⇧+C', onClick: () => quick.open() },
      { label: t('cmd.note'), icon: 'note', onClick: () => addNodeAt('util.note', info.world.x, info.world.y) },
      { sep: true },
      { label: t('menu.paste'), hint: 'Ctrl+V', disabled: !editor.hasClip(), onClick: () => editor.paste(info.world) },
      { label: t('menu.select_all'), hint: 'Ctrl+A', onClick: () => editor.selectAll() },
      { label: t('menu.arrange'), icon: 'arrange', onClick: () => editor.arrange() },
      { label: t('menu.fit'), icon: 'fit', hint: 'F', onClick: () => editor.fit() },
    ], x, y);
    return;
  }
  if (info.kind === 'link') {
    menu([{ label: t('menu.delete_link'), icon: 'trash', danger: true, onClick: () => editor.removeLink(info.id) }], x, y);
    return;
  }
  const n = editor.get(info.id);
  if (!n) return;
  const items = [
    { label: t('menu.rename'), onClick: () => editor.startRename(n.id) },
    { label: t('menu.duplicate'), icon: 'copy', hint: 'Ctrl+D', onClick: () => editor.duplicate() },
    { label: t('menu.copy'), hint: 'Ctrl+C', onClick: () => editor.copy() },
  ];
  if (!n.def.frontendOnly) items.push({ label: n.bypass ? t('menu.unbypass') : t('menu.bypass'), icon: 'bypass', hint: 'Ctrl+B', onClick: () => editor.toggleBypass() });
  items.push({ label: n.collapsed ? t('menu.expand') : t('menu.collapse'), onClick: () => editor.toggleCollapse(n.id) });
  if (n.type === 'files.load') {
    items.push({ sep: true }, { label: t('menu.add_files'), icon: 'file-plus', onClick: () => addPaths(n, false) }, { label: t('menu.add_folder'), icon: 'folder', onClick: () => addPaths(n, true) });
  }
  const saved = n.type === 'files.save' && n.result && n.result.outputs && n.result.outputs[0] && n.result.outputs[0][0];
  if (saved) items.push({ sep: true }, { label: t('menu.show_output'), icon: 'folder', onClick: () => revealPath(saved.path) });
  items.push({ sep: true }, { label: t('menu.delete'), icon: 'trash', hint: 'Del', danger: true, onClick: () => editor.removeNodes([...editor.sel]) });
  menu(items, x, y);
}

// ------------------------------------------------------------------ keyboard
function bindKeys() {
  window.addEventListener('keydown', (e) => {
    const tg = e.target;
    const typing = !!tg && (tg.tagName === 'INPUT' || tg.tagName === 'TEXTAREA' || tg.tagName === 'SELECT' || tg.isContentEditable);
    const mod = e.ctrlKey || e.metaKey;
    const k = (e.key || '').toLowerCase();
    if (mod && k === 'enter') {
      e.preventDefault();
      if (run) cancelRun();
      else startRun();
      return;
    }
    if (k === 'escape') {
      closeMenu();
      closeSearch();
      return;
    }
    if (mod && k === 'k') {
      e.preventDefault();
      if (!document.querySelector('.dialog-back')) openPalette(paletteCtx());
      return;
    }
    if (mod && e.shiftKey && k === 'c') {
      e.preventDefault();
      quick.open();
      return;
    }
    if (typing || document.querySelector('.dialog-back')) return;
    if (view === 'home') {
      if (mod && k === 's') {
        e.preventDefault();
        saveWorkflow();
      } else if (mod && k === 'o') {
        e.preventDefault();
        openWorkflow();
      } else if (!mod && (k === '?' || (k === '/' && e.shiftKey))) showHelp();
      else if (!mod && k === 'tab') {
        e.preventDefault();
        setView('editor');
        const r = $('#canvas').getBoundingClientRect();
        openSearch(r.left + r.width / 2 - 190, r.top + r.height / 2 - 160, { world: editor.centerWorld() });
      }
      return;
    }
    if (mod && k === 'z' && !e.shiftKey) {
      e.preventDefault();
      editor.undo();
    } else if (mod && (k === 'y' || (k === 'z' && e.shiftKey))) {
      e.preventDefault();
      editor.redo();
    } else if (mod && k === 'c') {
      if (!String(window.getSelection() || '').trim()) editor.copy();
    } else if (mod && k === 'v') {
      editor.paste();
    } else if (mod && k === 'd') {
      e.preventDefault();
      editor.duplicate();
    } else if (mod && k === 'a') {
      e.preventDefault();
      editor.selectAll();
    } else if (mod && k === 's') {
      e.preventDefault();
      saveWorkflow();
    } else if (mod && k === 'o') {
      e.preventDefault();
      openWorkflow();
    } else if (mod && k === 'b') {
      e.preventDefault();
      editor.toggleBypass();
    } else if (!mod && (k === 'delete' || k === 'backspace')) {
      if (editor.sel.size) {
        e.preventDefault();
        editor.removeNodes([...editor.sel]);
      }
    } else if (!mod && k === 'f') {
      editor.fit();
    } else if (!mod && k === 'm') {
      toggleMinimap();
    } else if (!mod && (k === '?' || (k === '/' && e.shiftKey))) {
      showHelp();
    } else if (!mod && k === 'tab') {
      e.preventDefault();
      const m = editor.mouse || { x: window.innerWidth / 2, y: window.innerHeight / 2 };
      openSearch(m.x, m.y, { world: editor.mouseWorld() });
    } else if (k === ' ') {
      e.preventDefault();
    }
  });
}

// ------------------------------------------------------------------ OS drops
function bindDrop() {
  const hint = $('#drop-hint');
  onDrop({
    enter: () => {
      hint.hidden = false;
    },
    over: () => {
      hint.hidden = false;
    },
    leave: () => {
      hint.hidden = true;
    },
    drop: (paths, pos) => {
      hint.hidden = true;
      if (paths.length) addDropped(paths, pos);
    },
  });
}

function addDropped(paths, pos) {
  if (quick && quick.isOpen()) {
    quick.addPaths(paths);
    return;
  }
  if (view === 'home' && quick) {
    quick.open();
    quick.addPaths(paths);
    return;
  }
  const hit = document.elementFromPoint(pos.x, pos.y);
  const nodeEl = hit && hit.closest ? hit.closest('.node') : null;
  let target = nodeEl ? editor.get(nodeEl.dataset.id) : null;
  if (!target || target.type !== 'files.load') {
    const loads = [...editor.nodes.values()].filter((n) => n.type === 'files.load');
    target = loads.length === 1 && !(loads[0].values.paths || []).length ? loads[0] : null;
  }
  if (target) {
    editor.setValue(target.id, 'paths', [...new Set([...(target.values.paths || []), ...paths])]);
  } else {
    const onCanvas = hit && hit.closest && hit.closest('#canvas');
    const w = onCanvas ? editor.toWorld(pos.x, pos.y) : editor.centerWorld();
    target = editor.addNode('files.load', w.x - 150, w.y - 40, { paths });
  }
  if (target) editor.select(target.id);
  toast(t('drop.added', { n: paths.length }), 'ok', 2000);
}

// ------------------------------------------------------------------ workflows on disk
async function saveWorkflow() {
  const data = JSON.stringify({ app: 'morph', name: wfName, ...editor.serialize() }, null, 2);
  const file = safeName(wfName || 'workflow') + '.morph.json';
  if (DEMO) {
    downloadText(file, data);
    return;
  }
  const path = await saveDialog({ defaultPath: file, filters: [{ name: 'Morph workflow', extensions: ['json'] }] });
  if (!path) return;
  try {
    await invoke('write_text', { path, content: data });
    toast(t('wf.saved'), 'ok');
  } catch (e) {
    toast(String(e), 'error');
  }
}

async function openWorkflow() {
  try {
    let text = null;
    if (DEMO) text = await pickLocalText('.json,application/json');
    else {
      const r = await pickFiles({ multiple: false, filters: [{ name: 'Morph workflow', extensions: ['json'] }] });
      if (!r[0]) return;
      const res = await invoke('read_text', { path: r[0], limit: 50000000 });
      text = res && res.text;
    }
    if (!text) return;
    const g = JSON.parse(text);
    if (!g || !Array.isArray(g.nodes)) throw new Error(t('wf.bad'));
    setView('editor');
    editor.load(g);
    setName(g.name || '');
    editor.resetHistory();
    if (!g.view) requestAnimationFrame(() => editor.fit());
    autosave();
    toast(t('wf.opened'), 'ok');
  } catch (e) {
    toast(t('wf.bad') + ': ' + ((e && e.message) || e), 'error');
  }
}

// ------------------------------------------------------------------ running
function setRunning(on) {
  const b = $('#btn-run');
  b.classList.toggle('running', on);
  const ico = b.querySelector('.ico');
  ico.textContent = '';
  ico.append(icon(on ? 'stop' : 'play', 17));
  b.querySelector('.lbl').textContent = on ? t('top.cancel') : t('top.run');
  $('#run-progress').hidden = !on;
  document.body.classList.toggle('is-running', on);
  if (on) setProgress(0, '');
}

function setProgress(p, label) {
  $('#run-progress .bar i').style.width = Math.round(clamp(p, 0, 1) * 100) + '%';
  $('#run-progress .label').textContent = label || Math.round(p * 100) + '%';
}

async function startRun() {
  if (run) return;
  closeMenu();
  closeSearch();
  const spec = editor.toSpec();
  if (!spec.nodes.some((n) => n.is_output)) {
    toast(t('run.no_output'), 'warn');
    return;
  }
  const ids = editor.reachable();
  editor.resetStatus(ids);
  run = { total: Math.max(1, ids.size), done: 0, cur: 0, t0: performance.now(), files: 0 };
  setRunning(true);
  try {
    await invoke('run_graph', { graph: { nodes: spec.nodes, settings: toolSettings() } });
  } catch (e) {
    run = null;
    setRunning(false);
    setMsg(t('run.failed'));
    toast(String((e && e.message) || e), 'error');
  }
}

function cancelRun() {
  if (!run) return;
  run.cancelling = true;
  setMsg(t('run.cancelling'));
  invoke('cancel_run').catch(() => {});
}

function tick() {
  if (!run) return;
  setProgress((run.done + run.cur) / run.total, run.done + ' / ' + run.total);
}

function refreshInspector(id) {
  const c = inspector.current;
  if (c && c.kind === 'node' && String(c.id) === String(id)) inspector.refresh();
}

function onRunEvent(ev) {
  if (!ev || !ev.type) return;
  if (run && run.quick) {
    run.quick.onEvent(ev);
    return;
  }
  const n = ev.node != null ? editor.get(ev.node) : null;
  switch (ev.type) {
    case 'started':
      if (run) run.id = ev.run;
      break;
    case 'node_start':
      editor.setStatus(ev.node, 'running');
      if (run) run.cur = 0;
      setMsg(t('run.node', { name: n ? n.title || nodeTitle(n.type) : ev.node }));
      tick();
      break;
    case 'node_progress':
      editor.setProgress(ev.node, ev.value, ev.text);
      if (run) run.cur = Number(ev.value) || 0;
      tick();
      break;
    case 'node_done':
      editor.setStatus(ev.node, 'done');
      editor.setResult(ev.node, ev);
      if (run) {
        run.done++;
        run.cur = 0;
        if (n && n.def.output) run.files += (ev.counts && ev.counts[0]) || 0;
      }
      tick();
      refreshInspector(ev.node);
      break;
    case 'node_error':
      editor.setStatus(ev.node, 'error');
      editor.setError(ev.node, ev.error, ev.warnings);
      refreshInspector(ev.node);
      break;
    case 'finished':
      finish(ev);
      break;
    default:
      break;
  }
}

function finish(ev) {
  const r = run;
  run = null;
  setRunning(false);
  const ms = ev.ms || (r ? performance.now() - r.t0 : 0);
  if (ev.cancelled) {
    toast(t('run.cancelled'), 'warn');
    setMsg(t('run.cancelled'));
  } else if (ev.ok) {
    toast(t('run.done', { time: fmtTime(ms) }), 'ok');
    setMsg(t('run.done', { time: fmtTime(ms) }));
  } else {
    toast(t('run.failed') + ': ' + (ev.error || ''), 'error');
    setMsg(t('run.failed'));
  }
  pushHistory({ name: wfName, ok: !!ev.ok, cancelled: !!ev.cancelled, error: ev.ok ? null : ev.error || null, ms, files: r ? r.files : null });
  if (SMOKE) smokeDone();
}

function pushHistory(h) {
  const hist = readJSON(LS.history, []);
  hist.unshift({ at: Date.now(), ...h });
  if (!SMOKE) writeJSON(LS.history, hist.slice(0, 100));
  if (panel === 'history') renderPanel('history');
  if (view === 'home') renderHomeView();
  if (inspector && !inspector.current) refreshEmpty();
}

// ------------------------------------------------------------------ CI smoke test (index.html?demo=1&smoke=1)
function markReady() {
  document.body.dataset.ready = '1';
}

function startSmoke() {
  setTimeout(markReady, 9000);
  setTimeout(() => startRun(), 400);
}

function smokeDone() {
  const prev = [...editor.nodes.values()].find((n) => n.def.preview && n.result);
  if (prev) {
    editor.sel = new Set([prev.id]);
    editor.refreshSel();
    const f = prev.result.outputs && prev.result.outputs[0] && prev.result.outputs[0][0];
    if (f) inspector.showFile(f, prev);
  }
  setTimeout(markReady, 800);
}

boot().catch((e) => {
  if (window.__err) window.__err('Boot failed: ' + ((e && e.stack) || e));
  document.body.dataset.ready = '1';
});
