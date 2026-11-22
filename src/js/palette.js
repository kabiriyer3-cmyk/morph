// Command palette (Ctrl+K): commands, nodes, templates and quick convert targets in one fuzzy list.
import { el, icon, clamp } from './ui.js';
import { t, nodeTitle, nodeDesc, catLabel, tplText } from './i18n.js';
import { NODE_DEFS, CATEGORIES, CONVERT_TARGETS } from './nodes.js';
import { TEMPLATES } from './templates.js';
import { paint } from './colors.js';

// Subsequence fuzzy score: higher is better, -1 = no match.
export function fuzzy(q, s) {
  if (!q) return 0;
  const hay = s.toLowerCase();
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  let total = 0;
  for (const w of words) {
    const i = hay.indexOf(w);
    if (i >= 0) {
      total += 100 - Math.min(i, 60) + (i === 0 || hay[i - 1] === ' ' ? 30 : 0);
      continue;
    }
    let j = 0;
    let score = 0;
    let last = -2;
    for (const c of w) {
      const k = hay.indexOf(c, j);
      if (k < 0) return -1;
      score += k === last + 1 ? 6 : 1;
      last = k;
      j = k + 1;
    }
    total += score;
  }
  return total;
}

export function openPalette(ctx) {
  const items = [];
  const cmd = (id, ic, label, run, hint) => items.push({ sec: 'commands', icon: ic, label, run, hint, key: id });
  const r = ctx.running();
  cmd('run', r ? 'stop' : 'play', r ? t('cmd.cancel') : t('cmd.run'), ctx.run, 'Ctrl+⏎');
  cmd('quick', 'bolt', t('cmd.quick'), () => ctx.quick(), 'Ctrl+⇧+C');
  cmd('new', 'file-plus', t('cmd.new'), ctx.newWf);
  cmd('open', 'folder-open', t('cmd.open'), ctx.open, 'Ctrl+O');
  cmd('save', 'save', t('cmd.save'), ctx.save, 'Ctrl+S');
  cmd('fit', 'fit', t('cmd.fit'), ctx.fit, 'F');
  cmd('arrange', 'arrange', t('cmd.arrange'), ctx.arrange);
  cmd('minimap', 'map', t('cmd.minimap'), ctx.minimap, 'M');
  cmd('all', 'library', t('cmd.select_all'), ctx.selectAll, 'Ctrl+A');
  cmd('note', 'note', t('cmd.note'), () => ctx.addNode('util.note'));
  cmd('lib', 'library', t('cmd.library'), () => ctx.panel('library'));
  cmd('tpl', 'templates', t('cmd.templates'), () => ctx.panel('templates'));
  cmd('hist', 'history', t('cmd.history'), () => ctx.panel('history'));
  cmd('tools', 'package', t('cmd.tools'), () => ctx.panel('tools'));
  cmd('settings', 'settings', t('cmd.settings'), () => ctx.panel('settings'));
  cmd('lang', 'globe', t('cmd.lang'), ctx.toggleLang);
  cmd('help', 'help', t('cmd.help'), ctx.help, '?');
  cmd('cache', 'trash', t('cmd.clear_cache'), ctx.clearCache);
  for (const [name, label] of ctx.accents) cmd('accent-' + name, 'drop', t('cmd.accent', { name: label }), () => ctx.accent(name));
  for (const [type, d] of Object.entries(NODE_DEFS)) {
    const c = CATEGORIES[d.cat] || CATEGORIES.util;
    items.push({ sec: 'nodes', icon: d.icon || c.icon, color: c.color, label: nodeTitle(type), hint: catLabel(d.cat), extra: [d.title, type, nodeDesc(type), d.keywords || ''].join(' '), run: () => ctx.addNode(type) });
  }
  for (const tpl of TEMPLATES) {
    items.push({ sec: 'templates', icon: tpl.icon || 'templates', label: tplText(tpl, 'title'), hint: t('side.templates'), extra: tplText(tpl, 'desc') + ' ' + tpl.title, run: () => ctx.template(tpl) });
  }
  for (const [g, list] of Object.entries(CONVERT_TARGETS)) {
    for (const x of list) items.push({ sec: 'convert', icon: 'bolt', label: t('pal.convert') + ' ' + x.toUpperCase(), hint: g, extra: 'convert to ' + x + ' ' + g, run: () => ctx.quick({ to: x }) });
  }

  const input = el('input', { placeholder: t('pal.ph'), spellcheck: 'false' });
  const list = el('div', { class: 'pal-list scroll' });
  const box = el('div', { class: 'dialog pal' }, el('div', { class: 'pal-in' }, icon('search', 18), input, el('kbd', { text: 'Esc' })), list,
    el('div', { class: 'pal-foot' }, el('span', { text: t('pal.nav') }), el('span', { text: t('pal.run') }), el('span', { text: t('pal.close') })));
  const back = el('div', { class: 'dialog-back pal-back' }, box);
  let shown = [];
  let idx = 0;
  const close = () => {
    document.removeEventListener('keydown', onKey, true);
    back.remove();
  };
  const exec = (it) => {
    close();
    if (it) setTimeout(() => it.run(), 0);
  };
  const mark = () => {
    list.querySelectorAll('.pal-item').forEach((x, j) => x.classList.toggle('active', j === idx));
    const a = list.querySelector('.pal-item.active');
    if (a) a.scrollIntoView({ block: 'nearest' });
  };
  const SEC = { commands: 'pal.commands', nodes: 'pal.nodes', templates: 'pal.templates', convert: 'pal.convert' };
  const render = () => {
    const q = input.value.trim();
    let res;
    if (!q) res = items.filter((x) => x.sec === 'commands').slice(0, 9).concat(items.filter((x) => x.sec === 'templates').slice(0, 4), ctx.recent().map((type) => items.find((x) => x.sec === 'nodes' && x.label === nodeTitle(type))).filter(Boolean));
    else {
      res = items.map((x) => ({ x, s: Math.max(fuzzy(q, x.label) * 1.4, fuzzy(q, x.label + ' ' + (x.extra || ''))) }))
        .filter((r) => r.s >= 0)
        .sort((a, b) => b.s - a.s)
        .slice(0, 60)
        .map((r) => r.x);
    }
    shown = res;
    idx = clamp(idx, 0, Math.max(0, shown.length - 1));
    list.textContent = '';
    if (!shown.length) {
      list.append(el('div', { class: 'sp-empty', text: t('lib.none') }));
      return;
    }
    let sec = null;
    shown.forEach((it, i) => {
      if (!q && it.sec !== sec) {
        sec = it.sec;
        list.append(el('div', { class: 'pal-sec', text: t(SEC[sec]) }));
      }
      const row = el('div', { class: 'pal-item' + (i === idx ? ' active' : ''), onclick: () => exec(it) },
        el('span', { class: 'pi' }, icon(it.icon, 15)), el('b', { text: it.label }), it.hint ? el('small', { text: it.hint }) : null);
      if (it.color) paint(row.querySelector('.pi'), it.color);
      row.addEventListener('pointermove', () => {
        if (idx === i) return;
        idx = i;
        mark();
      });
      list.append(row);
    });
  };
  const onKey = (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      idx = clamp(idx + (e.key === 'ArrowDown' ? 1 : -1), 0, Math.max(0, shown.length - 1));
      mark();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      exec(shown[idx]);
    }
  };
  input.addEventListener('input', () => {
    idx = 0;
    render();
  });
  back.addEventListener('pointerdown', (e) => {
    if (e.target === back) close();
  });
  document.addEventListener('keydown', onKey, true);
  document.body.append(back);
  render();
  input.focus();
}
