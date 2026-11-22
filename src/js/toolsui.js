// External tools manager: status, one-click install (FFmpeg, Pandoc), custom paths.
import { el, icon, toast } from './ui.js';
import { t } from './i18n.js';
import { invoke, DEMO, pickFiles, openExternal } from './api.js';
import { paint } from './colors.js';

export const TOOL_INFO = {
  ffmpeg: { label: 'FFmpeg', icon: 'video', color: '#A78BFA', url: 'https://ffmpeg.org/download.html', setting: 'ffmpeg' },
  pandoc: { label: 'Pandoc', icon: 'code', color: '#F472B6', url: 'https://pandoc.org/installing.html', setting: 'pandoc' },
  soffice: { label: 'LibreOffice', icon: 'doc', color: '#4ADE80', url: 'https://www.libreoffice.org/download/download-libreoffice/', setting: 'soffice' },
  gs: { label: 'Ghostscript', icon: 'pdf', color: '#FF7A90', url: 'https://ghostscript.com/releases/gsdnld.html', setting: 'gs' },
  sevenzip: { label: '7-Zip', icon: 'archive', color: '#F87171', url: 'https://www.7-zip.org/download.html', setting: 'sevenzip' },
  pdftoppm: { label: 'Poppler', icon: 'image', color: '#5AC8FA', url: 'https://poppler.freedesktop.org/', setting: null },
  ffprobe: { label: 'ffprobe', icon: 'info', color: '#A78BFA', url: 'https://ffmpeg.org/download.html', setting: 'ffprobe' },
};
export const TOOL_ORDER = ['ffmpeg', 'pandoc', 'soffice', 'gs', 'sevenzip', 'pdftoppm', 'ffprobe'];
export const toolLabel = (k) => (TOOL_INFO[k] ? TOOL_INFO[k].label : k);

// Install progress per tool: { stage, value, text }
export const installing = new Map();

export function onInstallEvent(ev, refresh) {
  if (!ev || !ev.tool) return;
  if (ev.stage === 'done' || ev.stage === 'error') installing.delete(ev.tool);
  else installing.set(ev.tool, ev);
  refresh(ev);
}

const short = (v) => {
  const s = String(v || '');
  const m = /(\d+\.\d+(\.\d+)?(-[\w.]+)?)/.exec(s);
  return m ? m[1] : s.split(/\r?\n/)[0].slice(0, 60);
};

export function renderToolsPanel(body, ctx) {
  const st = ctx.status() || { tools: {} };
  body.append(el('div', { class: 'lib-hint', style: { padding: '2px 4px 10px' }, text: t('tools.hint') }));
  if (st.dir) {
    body.append(el('div', { class: 'tools-dir', title: st.dir }, icon('folder', 13), el('span', { text: st.dir }),
      el('button', { class: 'btn sm ghost', onclick: () => invoke('open_tools_folder').catch((e) => toast(String(e), 'error')) }, t('tools.open_dir'))));
  }
  for (const key of TOOL_ORDER) {
    const info = TOOL_INFO[key];
    const s = (st.tools || {})[key] || {};
    const ok = !!s.path;
    const prog = installing.get(key);
    const card = paint(el('div', { class: 'tool' + (ok ? ' ok' : '') }), info.color);
    const chip = el('span', { class: 'chip ' + (ok ? (s.managed ? 'managed' : 'ok') : 'bad') }, el('i'), ok ? (s.managed ? t('tools.managed') : t('tools.ok')) : t('tools.missing'));
    card.append(el('div', { class: 'tool-head' },
      el('span', { class: 'ti' }, icon(info.icon, 17)),
      el('div', { class: 'tt' }, el('b', { text: info.label }), el('small', { text: ok ? (s.version ? short(s.version) + ' · ' : '') + s.path : t('tools.missing'), title: ok ? String(s.path) : '' })),
      chip));
    card.append(el('div', { class: 'tool-for', text: t('tools.for.' + key) }));
    if (prog) {
      card.append(el('div', { class: 'tool-prog' },
        el('div', { class: 'bar' }, el('i', { style: { width: Math.round((Number(prog.value) || 0) * 100) + '%' } })),
        el('small', { text: prog.text || t('tools.installing', { tool: info.label }) })));
    }
    const acts = el('div', { class: 'tool-acts' });
    if (s.installable && key !== 'ffprobe') {
      acts.append(el('button', {
        class: 'btn sm ' + (ok ? '' : 'primary'),
        disabled: !!prog,
        onclick: () => ctx.install(key),
      }, icon('download', 12), ok && s.managed ? t('tools.update') : t('tools.install')));
    }
    if (s.managed && key !== 'ffprobe') {
      acts.append(el('button', { class: 'btn sm ghost', disabled: !!prog, onclick: () => ctx.remove(key) }, icon('trash', 12), t('tools.remove')));
    }
    acts.append(el('button', { class: 'btn sm ghost', onclick: () => openExternal(info.url) }, icon('external', 12), t('tools.download')));
    card.append(acts);
    if (info.setting) {
      const field = el('input', { class: 'field mono', placeholder: t('tools.path'), spellcheck: 'false', value: ctx.settings[info.setting] || '' });
      const apply = () => {
        ctx.settings[info.setting] = field.value.trim();
        ctx.save();
      };
      field.addEventListener('change', apply);
      card.append(el('div', { class: 'tool-row' }, field, el('button', {
        class: 'btn sm',
        title: t('wid.browse'),
        onclick: async () => {
          const r = await pickFiles({ multiple: false });
          if (r[0]) {
            field.value = r[0];
            apply();
          }
        },
      }, icon('folder', 12))));
    }
    body.append(card);
  }
  body.append(el('div', { class: 'links-row', style: { padding: '0 4px' } },
    el('button', { class: 'btn sm', onclick: () => ctx.detect() }, icon('refresh', 12), t('tools.detect'))));
  if (DEMO) body.append(el('p', { class: 'lib-hint', style: { padding: '8px 4px' }, text: t('tools.demo') }));
}
