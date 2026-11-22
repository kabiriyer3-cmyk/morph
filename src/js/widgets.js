// Widgets rendered inside nodes (ComfyUI style pills).
import { el, icon, clamp, basename, extOf, menu, closeMenu } from './ui.js';
import { t, wLabel, optLabel, groupLabel } from './i18n.js';
import { pickFiles } from './api.js';

const decimals = (s) => {
  const str = String(s);
  const i = str.indexOf('.');
  return i < 0 ? 0 : str.length - i - 1;
};

export function fmtNum(v, step = 1) {
  const n = Number(v);
  if (!Number.isFinite(n)) return '0';
  let s = n.toFixed(Math.min(decimals(step), 6));
  if (s.includes('.')) s = s.replace(/0+$/, '').replace(/[.]$/, '');
  return s;
}

function roundStep(v, w) {
  const st = w.step || 1;
  let x = Math.round(v / st) * st;
  x = Number(x.toFixed(Math.min(decimals(st), 6)));
  if (w.min != null) x = Math.max(w.min, x);
  if (w.max != null) x = Math.min(w.max, x);
  return x;
}

export function isVisible(w, values) {
  if (w.show) for (const [k, list] of Object.entries(w.show)) if (!list.includes(values[k])) return false;
  if (w.hide) for (const [k, list] of Object.entries(w.hide)) if (list.includes(values[k])) return false;
  return true;
}

export function createWidget(w, value, onChange) {
  switch (w.kind) {
    case 'number':
    case 'slider':
      return numberWidget(w, value, onChange);
    case 'combo':
      return w.seg ? segWidget(w, value, onChange) : comboWidget(w, value, onChange);
    case 'toggle':
      return toggleWidget(w, value, onChange);
    case 'textarea':
      return areaWidget(w, value, onChange);
    case 'path':
      return pathWidget(w, value, onChange);
    case 'paths':
      return pathsWidget(w, value, onChange);
    case 'color':
      return colorWidget(w, value, onChange);
    default:
      return textWidget(w, value, onChange);
  }
}

function arrows(onStep) {
  const mk = (d, ic) => el('button', {
    class: 'arr',
    tabindex: '-1',
    onpointerdown: (e) => e.stopPropagation(),
    onclick: (e) => {
      e.stopPropagation();
      onStep(d);
    },
  }, icon(ic, 12));
  return [mk(-1, 'chevron-left'), mk(1, 'chevron-right')];
}

function inlineEdit(root, valEl, initial, commit, placeholder = '') {
  root.classList.add('editing');
  const inp = el('input', { class: 'w-input', value: initial, placeholder, spellcheck: 'false' });
  valEl.replaceWith(inp);
  inp.focus();
  inp.select();
  const finish = (ok) => {
    if (!inp.isConnected) return;
    inp.replaceWith(valEl);
    root.classList.remove('editing');
    commit(ok ? inp.value : null);
  };
  inp.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter') finish(true);
    else if (e.key === 'Escape') finish(false);
  });
  inp.addEventListener('blur', () => finish(true));
  inp.addEventListener('pointerdown', (e) => e.stopPropagation());
}

function numberWidget(w, value, onChange) {
  const isSlider = w.kind === 'slider';
  let v = Number(value);
  if (!Number.isFinite(v)) v = Number(w.value) || 0;
  const valEl = el('span', { class: 'wv' });
  const root = el('div', { class: 'w w-num' + (isSlider ? ' w-slider' : '') });
  const show = () => {
    valEl.textContent = fmtNum(v, w.step) + (w.unit ? ' ' + w.unit : '');
    if (isSlider) root.style.setProperty('--p', String(clamp((v - w.min) / (w.max - w.min || 1), 0, 1)));
  };
  const set = (nv) => {
    const x = roundStep(nv, w);
    if (x === v) return;
    v = x;
    show();
    onChange(v);
  };
  const [l, r] = arrows((d) => set(v + d * (w.step || 1)));
  root.append(l, el('span', { class: 'wl', text: wLabel(w) }), valEl, r);
  const edit = () => inlineEdit(root, valEl, fmtNum(v, w.step), (s) => {
    if (s != null) {
      const n = parseFloat(String(s).replace(',', '.'));
      if (Number.isFinite(n)) set(n);
    }
    show();
  });
  root.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || e.target.closest('.arr') || root.classList.contains('editing')) return;
    e.preventDefault();
    const x0 = e.clientX;
    const v0 = v;
    const box = root.getBoundingClientRect();
    let moved = false;
    try { root.setPointerCapture(e.pointerId); } catch { /* ignore */ }
    const atPos = (cx) => set(w.min + clamp((cx - box.left) / box.width, 0, 1) * (w.max - w.min));
    const move = (ev) => {
      const dx = ev.clientX - x0;
      if (!moved && Math.abs(dx) < 3) return;
      moved = true;
      if (isSlider) atPos(ev.clientX);
      else {
        const f = Math.abs(dx) / 5;
        const steps = Math.round(f + (f * f) / 30) * Math.sign(dx);
        set(v0 + steps * (w.step || 1) * (ev.shiftKey ? 0.1 : 1));
      }
    };
    const up = (ev) => {
      root.removeEventListener('pointermove', move);
      root.removeEventListener('pointerup', up);
      root.removeEventListener('pointercancel', up);
      if (moved || ev.type === 'pointercancel') return;
      if (isSlider && ev.detail < 2) atPos(ev.clientX);
      else edit();
    };
    root.addEventListener('pointermove', move);
    root.addEventListener('pointerup', up);
    root.addEventListener('pointercancel', up);
  });
  if (isSlider) root.addEventListener('dblclick', (e) => {
    e.stopPropagation();
    if (!root.classList.contains('editing')) edit();
  });
  root.setValue = (nv) => {
    v = Number(nv);
    show();
  };
  show();
  return root;
}

function comboWidget(w, value, onChange) {
  const norm = (x) => (w.options.includes(x) ? x : w.options.includes(String(x)) ? String(x) : w.value);
  let v = norm(value);
  const valEl = el('span', { class: 'wv' });
  const root = el('div', { class: 'w w-combo' });
  const show = () => {
    valEl.textContent = optLabel(w, v);
  };
  const set = (nv) => {
    if (nv === v) return;
    v = nv;
    show();
    onChange(v);
  };
  const [l, r] = arrows((d) => {
    const i = w.options.indexOf(v);
    set(w.options[(i + d + w.options.length) % w.options.length]);
  });
  root.append(l, el('span', { class: 'wl', text: wLabel(w) }), valEl, r);
  root.addEventListener('click', (e) => {
    if (e.target.closest('.arr')) return;
    const b = root.getBoundingClientRect();
    if (w.groups || w.options.length > 14) {
      formatPicker(w, v, b, set);
      return;
    }
    menu(w.options.map((o) => ({ label: optLabel(w, o), checked: o === v, onClick: () => set(o) })), b.left, b.bottom + 3, { minWidth: b.width });
  });
  root.setValue = (nv) => {
    v = norm(nv);
    show();
  };
  show();
  return root;
}

function toggleWidget(w, value, onChange) {
  let v = !!value;
  const root = el('div', { class: 'w w-toggle' }, el('span', { class: 'wl', text: wLabel(w) }), el('span', { class: 'sw' }, el('i')));
  const show = () => root.classList.toggle('on', v);
  root.addEventListener('click', () => {
    v = !v;
    show();
    onChange(v);
  });
  root.setValue = (nv) => {
    v = !!nv;
    show();
  };
  show();
  return root;
}

function textWidget(w, value, onChange) {
  const inp = el('input', { class: 'w-input', spellcheck: 'false', placeholder: w.placeholder || '' });
  inp.value = value == null ? '' : String(value);
  const root = el('div', { class: 'w w-text', title: w.placeholder || '' }, el('span', { class: 'wl', text: wLabel(w) }), inp);
  inp.addEventListener('input', () => onChange(inp.value));
  inp.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter' || e.key === 'Escape') inp.blur();
  });
  inp.addEventListener('pointerdown', (e) => e.stopPropagation());
  root.addEventListener('click', () => inp.focus());
  root.setValue = (nv) => {
    inp.value = nv == null ? '' : String(nv);
  };
  return root;
}

function areaWidget(w, value, onChange) {
  const ta = el('textarea', { spellcheck: 'false', placeholder: w.placeholder || '', rows: String(w.rows || 4) });
  ta.value = value == null ? '' : String(value);
  const root = el('div', { class: 'w w-area' }, w.label === '' ? null : el('span', { class: 'wl', text: wLabel(w) }), ta);
  ta.addEventListener('input', () => onChange(ta.value));
  ta.addEventListener('keydown', (e) => e.stopPropagation());
  ta.addEventListener('pointerdown', (e) => e.stopPropagation());
  ta.addEventListener('wheel', (e) => e.stopPropagation(), { passive: true });
  root.setValue = (nv) => {
    ta.value = nv == null ? '' : String(nv);
  };
  return root;
}

function pathWidget(w, value, onChange) {
  let v = value == null ? '' : String(value);
  const valEl = el('span', { class: 'wv' });
  const root = el('div', { class: 'w w-path' });
  const show = () => {
    valEl.textContent = v ? basename(v) || v : t('wid.not_set');
    root.title = v || w.placeholder || '';
    root.classList.toggle('empty', !v);
  };
  const set = (nv) => {
    v = nv;
    show();
    onChange(v);
  };
  const pick = el('button', {
    class: 'pick',
    title: t('wid.browse'),
    onpointerdown: (e) => e.stopPropagation(),
    onclick: async (e) => {
      e.stopPropagation();
      const r = await pickFiles({ folder: !!w.folder, multiple: false, filters: w.filters });
      if (r.length) set(r[0]);
    },
  }, icon(w.folder ? 'folder' : 'file', 11));
  root.append(el('span', { class: 'wl', text: wLabel(w) }), valEl, pick);
  root.addEventListener('click', (e) => {
    if (e.target.closest('.pick') || root.classList.contains('editing')) return;
    inlineEdit(root, valEl, v, (s) => {
      if (s != null && s.trim() !== v) set(s.trim());
      else show();
    }, w.placeholder || '');
  });
  root.setValue = (nv) => {
    v = nv == null ? '' : String(nv);
    show();
  };
  show();
  return root;
}

function pathsWidget(w, value, onChange) {
  let list = Array.isArray(value) ? value.slice() : String(value || '').split(String.fromCharCode(10)).map((s) => s.trim()).filter(Boolean);
  const rows = el('div', { class: 'wp-list scroll' });
  const root = el('div', { class: 'w w-paths' });
  const emit = () => onChange(list.slice());
  const render = () => {
    rows.textContent = '';
    if (!list.length) rows.append(el('div', { class: 'wp-empty' }, icon('upload', 14), t('wid.empty')));
    list.forEach((p, i) => {
      rows.append(el('div', { class: 'wp-row', title: p },
        icon(extOf(p) ? 'file' : 'folder', 13),
        el('span', { class: 'wp-name', text: basename(p) || p }),
        el('button', {
          class: 'wp-x',
          onpointerdown: (e) => e.stopPropagation(),
          onclick: (e) => {
            e.stopPropagation();
            list.splice(i, 1);
            render();
            emit();
          },
        }, icon('x', 12))));
    });
  };
  const add = async (folder) => {
    const r = await pickFiles({ folder, multiple: true });
    if (!r.length) return;
    list = [...new Set([...list, ...r])];
    render();
    emit();
  };
  const btn = (ic, label, fn, cls = '') => el('button', {
    class: 'btn sm ' + cls,
    onpointerdown: (e) => e.stopPropagation(),
    onclick: (e) => {
      e.stopPropagation();
      fn();
    },
  }, ic ? icon(ic, 12) : null, label);
  root.append(rows, el('div', { class: 'wp-bar' },
    btn('file-plus', t('wid.add_files'), () => add(false)),
    btn('folder', t('wid.add_folder'), () => add(true)),
    btn(null, t('wid.clear'), () => {
      list = [];
      render();
      emit();
    }, 'ghost')));
  root.setValue = (nv) => {
    list = Array.isArray(nv) ? nv.slice() : [];
    render();
  };
  render();
  return root;
}

function colorWidget(w, value, onChange) {
  let v = value || w.value || '#ffffff';
  const valEl = el('span', { class: 'wv' });
  const sw = el('span', { class: 'swatch' });
  const inp = el('input', { type: 'color' });
  const root = el('div', { class: 'w w-color' }, el('span', { class: 'wl', text: wLabel(w) }), valEl, sw, inp);
  const show = () => {
    valEl.textContent = v;
    sw.style.background = v;
    inp.value = v;
  };
  inp.addEventListener('input', () => {
    v = inp.value;
    show();
    onChange(v);
  });
  inp.addEventListener('pointerdown', (e) => e.stopPropagation());
  root.setValue = (nv) => {
    v = nv || '#ffffff';
    show();
  };
  show();
  return root;
}

// Segmented control for combos with a few short options.
function segWidget(w, value, onChange) {
  const norm = (x) => (w.options.includes(x) ? x : w.options.includes(String(x)) ? String(x) : w.value);
  let v = norm(value);
  const segs = el('div', { class: 'segs' });
  const root = el('div', { class: 'w w-seg' }, el('span', { class: 'wl', text: wLabel(w) }), segs);
  const btns = w.options.map((o) => {
    const b = el('button', {
      class: 'seg',
      text: optLabel(w, o),
      title: optLabel(w, o),
      onpointerdown: (e) => e.stopPropagation(),
      onclick: (e) => {
        e.stopPropagation();
        if (o === v) return;
        v = o;
        show();
        onChange(v);
      },
    });
    segs.append(b);
    return b;
  });
  const show = () => btns.forEach((b, i) => b.classList.toggle('on', w.options[i] === v));
  root.setValue = (nv) => {
    v = norm(nv);
    show();
  };
  show();
  return root;
}

// Searchable grid popup for long option lists (formats, codecs). w.groups = { label: [options] }.
let picker = null;
function closePicker() {
  if (!picker) return;
  picker.remove();
  picker = null;
  document.removeEventListener('pointerdown', pickerOutside, true);
}
function pickerOutside(e) {
  if (picker && !picker.contains(e.target)) closePicker();
}
export function formatPicker(w, current, rect, onPick) {
  closeMenu();
  closePicker();
  const input = el('input', { placeholder: t('wid.filter'), spellcheck: 'false' });
  const body = el('div', { class: 'fp-body' });
  const box = el('div', { class: 'fmt-pop' }, input, body);
  const groups = w.groups ? Object.entries(w.groups) : [['', w.options]];
  const known = new Set(groups.flatMap(([, l]) => l));
  const rest = w.options.filter((o) => !known.has(o));
  if (rest.length) groups.push(['', rest]);
  const render = () => {
    const q = input.value.trim().toLowerCase();
    body.textContent = '';
    let first = null;
    for (const [g, list] of groups) {
      const items = list.filter((o) => w.options.includes(o) && (!q || String(o).toLowerCase().includes(q) || optLabel(w, o).toLowerCase().includes(q)));
      if (!items.length) continue;
      const grid = el('div', { class: 'fp-grid' });
      for (const o of items) {
        const b = el('button', {
          class: o === current ? 'on' : '',
          text: optLabel(w, o),
          title: optLabel(w, o),
          onclick: () => {
            closePicker();
            onPick(o);
          },
        });
        if (!first) first = o;
        grid.append(b);
      }
      body.append(el('div', { class: 'fp-group' }, g ? el('h5', { text: groupLabel(g) }) : null, grid));
    }
    if (!first) body.append(el('div', { class: 'sp-empty', text: t('lib.none') }));
    return first;
  };
  input.addEventListener('input', render);
  input.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Escape') closePicker();
    if (e.key === 'Enter') {
      const f = render();
      if (f != null) {
        closePicker();
        onPick(f);
      }
    }
  });
  document.body.append(box);
  const bw = 360;
  box.style.left = clamp(rect.left, 8, window.innerWidth - bw - 8) + 'px';
  const top = rect.bottom + 4;
  box.style.top = (top + 420 > window.innerHeight ? Math.max(8, rect.top - 424) : top) + 'px';
  picker = box;
  render();
  input.focus();
  setTimeout(() => document.addEventListener('pointerdown', pickerOutside, true), 0);
}
