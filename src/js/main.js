// App bootstrap: panels, menus, keyboard, OS drops and the run lifecycle.
import { DEMO, SMOKE, DEMO_ROOT, invoke, listen, pickFiles, saveDialog, onDrop, setToolSettings, toolSettings, openExternal, downloadText, pickLocalText } from './api.js';
import { Editor } from './editor.js';
import { Inspector } from './inspector.js';
import { NODE_DEFS, CATEGORIES, compatible } from './nodes.js';
import { TEMPLATES, DEFAULT_TEMPLATE } from './templates.js';
import { t, setLang, detectLang, getLang, applyI18n, nodeTitle, nodeDesc, catLabel, tplText } from './i18n.js';
import { $, $$, el, icon, toast, menu, closeMenu, dialog, fmtTime, debounce, clamp } from './ui.js';

const REPO = 'https://github.com/kabiriyer3-cmyk/morph';
const LS = { settings: 'morph.settings', graph: 'morph.graph', history: 'morph.history' };
const TOOL_LABEL = { ffmpeg: 'FFmpeg', ffprobe: 'ffprobe', soffice: 'LibreOffice', pandoc: 'Pandoc' };

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

const settings = Object.assign({ lang: '', ffmpeg: '', ffprobe: '', soffice: '', pandoc: '', snap: true, panel: 'library' }, readJSON(LS.settings, {}));
const saveSettings = () => writeJSON(LS.settings, settings);

let editor = null;
let inspector = null;
let wfName = '';
let tools = {};
let run = null;
let version = '0.1.0';
let panel = null;
let searchBox = null;
let placed = 0;

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
  for (const k of ['ffmpeg', 'soffice', 'pandoc']) if (!tools[k]) s.add(k);
  return s;
}

// ------------------------------------------------------------------ boot
async function boot() {
  setLang(settings.lang || detectLang());
  applyI18n(document.body);
  $$('[data-icon]').forEach((b) => {
    const size = b.closest('#drop-hint') ? 40 : b.classList.contains('sb-btn') ? 20 : 17;
    b.prepend(icon(b.dataset.icon, size));
  });
  $('#st-ver').textContent = 'v' + version + (DEMO ? ' · ' + t('st.demo') : '');
  setMsg(t('st.ready'));
  setToolSettings(settings);

  editor = new Editor($('#canvas'), {
    snap: settings.snap !== false,
    onSelect: (n) => {
      if (n) inspector.showNode(n);
      else inspector.showEmpty();
    },
    onChange: () => {
      autosave();
      updateStatus();
      updateUndo();
    },
    onHistory: () => updateUndo(),
    onSearch: (x, y, o) => openSearch(x, y, o),
    onContextMenu: (e, info) => contextMenu(e, info),
    onFile: (f, n) => inspector.showFile(f, n),
    onOpenFile: (f) => openPath(f.path),
    onReveal: (p) => revealPath(p),
    onView: (v) => {
      $('#zoom-label').textContent = Math.round(v.z * 100) + '%';
    },
    onPointerDown: () => closeSearch(),
    onRename: (n) => {
      const c = inspector.current;
      if (c && c.kind === 'node' && c.id === n.id) inspector.showNode(n);
    },
  });
  inspector = new Inspector($('#inspector'), { editor, open: openPath, reveal: revealPath, missing: missingTools });

  bindTopbar();
  bindSidebar();
  bindKeys();
  bindDrop();
  await listen('run-event', onRunEvent);
  restoreGraph();
  updateStatus();
  updateUndo();
  invoke('system_info')
    .then((s) => {
      if (s && s.version) version = s.version;
      $('#st-ver').textContent = 'v' + version + (DEMO ? ' · ' + t('st.demo') : '');
    })
    .catch(() => {});
  detectTools();
  if (SMOKE) startSmoke();
  else document.body.dataset.ready = '1';
}

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
  $('#btn-new').onclick = async () => {
    if (editor.nodes.size && !(await dialog({ title: t('top.new'), text: t('wf.new') }))) return;
    editor.load({ nodes: [], links: [] });
    const a = editor.addNode('files.load', 40, 80, DEMO ? { paths: [DEMO_ROOT] } : {}, { silent: true });
    const b = editor.addNode('files.preview', 400, 80, {}, { silent: true });
    editor.connect(a.id, 0, b.id, 0, { silent: true });
    setName('');
    editor.resetHistory();
    requestAnimationFrame(() => editor.fit());
    autosave();
  };
  $('#btn-open').onclick = () => openWorkflow();
  $('#btn-save').onclick = () => saveWorkflow();
  $('#btn-undo').onclick = () => editor.undo();
  $('#btn-redo').onclick = () => editor.redo();
  $('#btn-fit').onclick = () => editor.fit();
  $('#btn-arrange').onclick = () => editor.arrange();
  $('#btn-run').onclick = () => (run ? cancelRun() : startRun());
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
  $('#panel-close').onclick = () => togglePanel(null);
  togglePanel(settings.panel || null, { initial: true });
}

function togglePanel(name, o = {}) {
  let next = name;
  if (!o.initial && !o.force && next === panel) next = null;
  const canvas = $('#canvas');
  const before = canvas.getBoundingClientRect().left;
  panel = next;
  $$('#sidebar .sb-btn[data-panel]').forEach((b) => b.classList.toggle('active', b.dataset.panel === next));
  $('#panel').hidden = !next;
  if (next) renderPanel(next);
  if (!o.initial) {
    settings.panel = next;
    saveSettings();
    const after = canvas.getBoundingClientRect().left;
    if (after !== before && editor) {
      editor.view.x -= after - before;
      editor.applyView();
    }
  }
}

function renderPanel(name) {
  $('#panel-title').textContent = t('side.' + name);
  const body = $('#panel-body');
  body.textContent = '';
  body.scrollTop = 0;
  if (name === 'library') renderLibrary(body);
  else if (name === 'templates') renderTemplates(body);
  else if (name === 'history') renderHistory(body);
  else if (name === 'settings') renderSettings(body);
}

function matches(q, type, def) {
  if (!q) return true;
  const hay = [nodeTitle(type), def.title, type, nodeDesc(type), def.keywords || ''].join(' ').toLowerCase();
  return q.split(' ').filter(Boolean).every((w) => hay.includes(w));
}

function addAtCenter(type) {
  const c = editor.centerWorld();
  const off = (placed++ % 6) * 24;
  const n = editor.addNode(type, c.x - 140 + off, c.y - 100 + off);
  if (n) editor.select(n.id);
}

function libItem(type) {
  const it = el('div', { class: 'lib-item', dataset: { type }, title: nodeDesc(type) }, el('b', { text: nodeTitle(type) }), el('span', { text: nodeDesc(type) }));
  it.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
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
        const c = CATEGORIES[NODE_DEFS[type].cat] || CATEGORIES.util;
        ghost = el('div', { class: 'drag-ghost' }, el('span', { class: 'dot', style: { '--c': c.color } }), nodeTitle(type));
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
        const n = editor.addNode(type, w.x - 20, w.y - 15);
        if (n) editor.select(n.id);
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
  const list = el('div', {});
  const fill = () => {
    const q = input.value.trim().toLowerCase();
    list.textContent = '';
    let count = 0;
    for (const [cat, c] of Object.entries(CATEGORIES)) {
      const items = Object.entries(NODE_DEFS).filter(([type, d]) => d.cat === cat && matches(q, type, d));
      if (!items.length) continue;
      list.append(el('div', { class: 'lib-cat' }, el('span', { class: 'dot', style: { '--c': c.color } }), catLabel(cat)));
      for (const [type] of items) {
        list.append(libItem(type));
        count++;
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
  body.append(el('div', { class: 'search-wrap' }, input, icon('search', 14)), el('div', { class: 'lib-hint', text: t('lib.hint') }), list);
  fill();
}

function renderTemplates(body) {
  const list = el('div', { class: 'tpl-list' }, el('div', { class: 'lib-hint', style: { padding: '0 2px' }, text: t('tpl.hint') }));
  for (const tpl of TEMPLATES) {
    const g = tpl.build();
    const chain = el('div', { class: 'tpl-chain' }, g.nodes.filter((n) => n.type !== 'files.load').map((n) => el('span', {
      style: { '--c': (CATEGORIES[NODE_DEFS[n.type].cat] || CATEGORIES.util).color },
      text: nodeTitle(n.type),
    })));
    list.append(el('div', {
      class: 'tpl-card',
      onclick: async () => {
        if (editor.nodes.size && !(await dialog({ title: tplText(tpl, 'title'), text: t('tpl.replace', { name: tplText(tpl, 'title') }) }))) return;
        loadTemplate(tpl);
      },
    }, el('h4', {}, icon(tpl.icon || 'templates', 15), tplText(tpl, 'title')), el('p', { text: tplText(tpl, 'desc') }), chain));
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

function renderTools(box) {
  box.textContent = '';
  for (const key of ['ffmpeg', 'ffprobe', 'soffice', 'pandoc']) {
    const info = tools[key];
    const field = el('input', { class: 'field', placeholder: t('set.auto'), spellcheck: 'false', value: settings[key] || '' });
    const apply = () => {
      settings[key] = field.value.trim();
      saveSettings();
      setToolSettings(settings);
      detectTools();
    };
    field.addEventListener('change', apply);
    const browse = el('button', {
      class: 'btn sm',
      title: t('wid.browse'),
      onclick: async () => {
        const r = await pickFiles({ multiple: false });
        if (r[0]) {
          field.value = r[0];
          apply();
        }
      },
    }, icon('folder', 12));
    box.append(el('div', { class: 'tool' },
      el('div', { class: 'tool-head' },
        el('span', { class: 'chip ' + (info ? 'ok' : 'bad') }, el('i'), TOOL_LABEL[key]),
        el('small', { text: info ? info.version || info.path : t('set.missing'), title: info ? String(info.path) : '' })),
      el('div', { class: 'tool-row' }, field, browse)));
  }
}

function renderSettings(body) {
  const langSel = el('select', { class: 'select' }, el('option', { value: 'en', text: 'English' }), el('option', { value: 'ru', text: 'Русский' }));
  langSel.value = getLang();
  langSel.addEventListener('change', () => {
    settings.lang = langSel.value;
    saveSettings();
    location.reload();
  });
  body.append(el('div', { class: 'set-group' }, el('h3', { text: t('set.language') }), langSel));

  const toolsBox = el('div', { id: 'tools-box' });
  body.append(el('div', { class: 'set-group' },
    el('h3', { text: t('set.tools') }),
    el('p', { text: t('set.tools_hint') }),
    toolsBox,
    el('div', { class: 'links-row' },
      el('button', { class: 'btn sm', onclick: () => detectTools() }, icon('search', 12), t('set.detect')),
      el('button', { class: 'btn sm ghost', onclick: () => openExternal(REPO + '#optional-tools') }, icon('external', 12), t('set.install')))));
  renderTools(toolsBox);

  const snap = el('input', { type: 'checkbox' });
  snap.checked = settings.snap !== false;
  snap.addEventListener('change', () => {
    settings.snap = snap.checked;
    editor.snap = snap.checked;
    saveSettings();
  });
  body.append(el('div', { class: 'set-group' }, el('h3', { text: t('set.editor') }), el('label', { class: 'set-row' }, el('span', { text: t('set.snap') }), snap)));

  body.append(el('div', { class: 'set-group' },
    el('h3', { text: t('set.cache') }),
    el('p', { text: t('set.cache_hint') }),
    el('button', {
      class: 'btn sm',
      onclick: () => invoke('clear_cache').then(() => toast(t('set.cache_cleared'), 'ok')).catch((e) => toast(String(e), 'error')),
    }, icon('trash', 12), t('set.clear_cache'))));

  body.append(el('div', { class: 'set-group' },
    el('h3', { text: t('set.about') }),
    el('div', { class: 'about' }, el('img', { src: 'assets/logo.svg', alt: '' }), el('div', {}, el('b', { text: 'Morph' }), el('div', { class: 'muted', text: 'v' + version + ' · ' + t('app.tagline') }))),
    DEMO ? el('p', { style: { marginTop: '10px' }, text: t('set.demo') }) : null,
    el('div', { class: 'links-row' },
      el('button', { class: 'btn sm', onclick: () => openExternal(REPO) }, icon('git', 12), 'GitHub'),
      el('button', { class: 'btn sm', onclick: () => openExternal(REPO + '/releases') }, icon('download', 12), 'Releases'))));
}

async function detectTools() {
  try {
    tools = (await invoke('detect_tools', { settings: toolSettings() })) || {};
  } catch {
    tools = {};
  }
  renderToolChips();
  editor.setMissing(missingTools());
  const box = document.getElementById('tools-box');
  if (box) renderTools(box);
  inspector.refresh();
}

function renderToolChips() {
  const box = $('#st-tools');
  box.textContent = '';
  if (DEMO) box.append(el('span', { class: 'chip demo', text: 'browser demo' }));
  for (const k of ['ffmpeg', 'soffice', 'pandoc']) {
    const ok = !!tools[k];
    const c = el('span', { class: 'chip ' + (ok ? 'ok' : 'bad'), title: ok ? tools[k].version || String(tools[k].path) : t('st.missing', { tool: TOOL_LABEL[k] }) }, el('i'), TOOL_LABEL[k]);
    if (!ok) c.addEventListener('click', () => togglePanel('settings', { force: true }));
    box.append(c);
  }
}

function showHelp() {
  const keys = [
    ['Ctrl + Enter', 'help.k_run'], ['Tab', 'help.k_search'], ['Del', 'help.k_delete'], ['Ctrl + C / V', 'help.k_copy'],
    ['Ctrl + D', 'help.k_dup'], ['Ctrl + Z / Y', 'help.k_undo'], ['Ctrl + B', 'help.k_bypass'], ['F', 'help.k_fit'],
    ['Space + drag', 'help.k_pan'], ['Wheel', 'help.k_zoom'], ['Ctrl + drag', 'help.k_box'], ['Ctrl + S / O', 'help.k_save'],
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
    const n = editor.addNode(type, w.x - (o.link && o.link.dir === 'in' ? 300 : 0), w.y - 15);
    if (!n) return;
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
      const row = el('div', { class: 'sp-item' + (i === idx ? ' active' : ''), onclick: () => pick(type) },
        el('span', { class: 'dot', style: { '--c': c.color } }), el('b', { text: nodeTitle(type) }), el('small', { text: catLabel(d.cat) }));
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
      sub: Object.entries(NODE_DEFS).filter(([, d]) => d.cat === cat).map(([type]) => ({
        label: nodeTitle(type),
        onClick: () => {
          const n = editor.addNode(type, info.world.x, info.world.y);
          if (n) editor.select(n.id);
        },
      })),
    }));
    menu([
      { label: t('menu.add_node'), icon: 'plus', sub: cats },
      { label: t('menu.search'), icon: 'search', hint: 'Tab', onClick: () => openSearch(x, y, { world: info.world }) },
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
    if (typing || document.querySelector('.dialog-back')) return;
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
  const hist = readJSON(LS.history, []);
  hist.unshift({ at: Date.now(), name: wfName, ok: !!ev.ok, cancelled: !!ev.cancelled, error: ev.ok ? null : ev.error || null, ms, files: r ? r.files : null });
  if (!SMOKE) writeJSON(LS.history, hist.slice(0, 100));
  if (panel === 'history') renderPanel('history');
  if (SMOKE) smokeDone();
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
