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
  convert: `<path d='M4 10a8 8 0 0 1 14.3-4.9L20 7'/><path d='M20 3v4h-4'/><path d='M20 14a8 8 0 0 1-14.3 4.9L4 17'/><path d='M4 21v-4h4'/>`,
  bolt: `<path d='M13 2.5L4.5 13.5H11l-1 8 8.5-11H12z'/>`,
  command: `<path d='M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z'/>`,
  star: `<path d='M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z'/>`,
  'star-fill': `<path d='M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z' fill='currentColor'/>`,
  pdf: FILE + `<path d='M8 18v-4.5h1.4a1.3 1.3 0 0 1 0 2.6H8M12.5 18v-4.5h1a2.25 2.25 0 0 1 0 4.5zM17 13.5h-1.5V18M15.5 15.8H17'/>`,
  sheet: `<rect x='3' y='4' width='18' height='16' rx='2'/><path d='M3 9.5h18M3 14.8h18M9 4v16'/>`,
  type: `<path d='M5 7V5h14v2M12 5v14M9 19h6'/>`,
  qr: `<rect x='3.5' y='3.5' width='6.5' height='6.5' rx='1'/><rect x='14' y='3.5' width='6.5' height='6.5' rx='1'/><rect x='3.5' y='14' width='6.5' height='6.5' rx='1'/><path d='M14 14h2.5v2.5H14zM20.5 14v.01M18 20.5h2.5M20.5 17.5v3M14 20.5v-1'/>`,
  palette: `<path d='M12 3a9 9 0 1 0 0 18c1.2 0 1.7-.9 1.3-1.8-.5-1.1.3-2.2 1.5-2.2H17a4 4 0 0 0 4-4c0-5.5-4-10-9-10z'/><circle cx='7.5' cy='11.5' r='1.1'/><circle cx='10' cy='7.2' r='1.1'/><circle cx='15' cy='7.5' r='1.1'/>`,
  crop: `<path d='M6 2.5V16a2 2 0 0 0 2 2h13.5'/><path d='M2.5 6H16a2 2 0 0 1 2 2v13.5'/>`,
  resize: `<path d='M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7'/>`,
  rotate: `<path d='M20 12a8 8 0 1 1-2.6-5.9'/><path d='M20 4v5h-5'/>`,
  wand: `<path d='M4 20L15 9'/><path d='M14 3v3M14 12v-1M9.5 7.5H11M17 7.5h3M17.6 4.4l-1.4 1.4M17.6 10.6l-1.4-1.4'/>`,
  frame: `<rect x='3' y='3' width='18' height='18' rx='3'/><rect x='7.5' y='7.5' width='9' height='9' rx='1.5'/>`,
  collage: `<rect x='3' y='3' width='8' height='10' rx='1.5'/><rect x='13' y='3' width='8' height='6' rx='1.5'/><rect x='13' y='11' width='8' height='10' rx='1.5'/><rect x='3' y='15' width='8' height='6' rx='1.5'/>`,
  dashboard: `<rect x='3.5' y='3.5' width='7' height='9' rx='1.6'/><rect x='13.5' y='3.5' width='7' height='5' rx='1.6'/><rect x='13.5' y='11.5' width='7' height='9' rx='1.6'/><rect x='3.5' y='15.5' width='7' height='5' rx='1.6'/>`,
  home: `<path d='M4 10.5L12 4l8 6.5V19a1 1 0 0 1-1 1h-4.5v-5.5h-5V20H5a1 1 0 0 1-1-1z'/>`,
  grid: `<rect x='3' y='3' width='18' height='18' rx='2'/><path d='M3 9h18M3 15h18M9 3v18M15 3v18'/>`,
  scissors: `<circle cx='6' cy='7' r='2.6'/><circle cx='6' cy='17' r='2.6'/><path d='M8.2 8.6L20 19M8.2 15.4L20 5'/>`,
  merge: `<path d='M7 3.5v4c0 3.5 5 4.5 5 8.5v4.5M17 3.5v4c0 3.5-5 4.5-5 8.5'/>`,
  split: `<path d='M3 12h6l4-5.5h7M9 12l4 5.5h7'/><path d='M17.5 4l2.5 2.5-2.5 2.5M17.5 15l2.5 2.5-2.5 2.5'/>`,
  sort: `<path d='M7 4v16M4 17l3 3 3-3M13 6h8M13 11h6M13 16h4'/>`,
  tag: `<path d='M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9z'/><circle cx='7.5' cy='7.5' r='1.5'/>`,
  shield: `<path d='M12 3l8 3v6c0 4.5-3.5 8-8 9-4.5-1-8-4.5-8-9V6z'/><path d='M8.5 12l2.5 2.5 4.5-5'/>`,
  chart: `<path d='M4 20V11M10 20V5M16 20v-7M21 20H3'/>`,
  hash: `<path d='M5 9h14M5 15h14M10 4L8 20M16 4l-2 16'/>`,
  funnel: `<path d='M3.5 5h17l-6.5 8v6l-4 2v-8z'/>`,
  cursor: `<path d='M12 4v16M9 4h6M9 20h6'/><path d='M5 8.5L3 12l2 3.5M19 8.5l2 3.5-2 3.5'/>`,
  film: `<rect x='3' y='3' width='18' height='18' rx='2'/><path d='M7.5 3v18M16.5 3v18M3 8h4.5M3 12h4.5M3 16h4.5M16.5 8H21M16.5 12H21M16.5 16H21'/>`,
  wave: `<path d='M3 12h1.5M6.5 8.5v7M10 5v14M13.5 7.5v9M17 10v4M20.5 12H21'/>`,
  cc: `<rect x='3' y='5' width='18' height='14' rx='2.5'/><path d='M10.5 10.3a2.2 2.2 0 1 0 0 3.4M17 10.3a2.2 2.2 0 1 0 0 3.4'/>`,
  gauge: `<path d='M12 14.5l4.5-4.5'/><path d='M3.5 18.5a9 9 0 1 1 17 0'/>`,
  compress: `<path d='M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7'/>`,
  lock: `<rect x='5' y='11' width='14' height='10' rx='2'/><path d='M8 11V7.5a4 4 0 0 1 8 0V11'/>`,
  code: `<path d='M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 4l-3 16'/>`,
  list: `<path d='M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01'/>`,
  columns: `<rect x='3' y='4' width='18' height='16' rx='2'/><path d='M9 4v16M15 4v16'/>`,
  sparkle: `<path d='M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z'/><path d='M18.5 15.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z'/>`,
  eraser: `<path d='M20 20H8.5l-5-5a2 2 0 0 1 0-2.8l8.7-8.7a2 2 0 0 1 2.8 0l5.2 5.2a2 2 0 0 1 0 2.8L13 18.7'/><path d='M7.5 10.5l6.5 6.5'/>`,
  layers: `<path d='M12 3l9 5-9 5-9-5z'/><path d='M3 12.5l9 5 9-5'/><path d='M3 16.5l9 5 9-5'/>`,
  package: `<path d='M12 3l8 4.5v9L12 21l-8-4.5v-9z'/><path d='M4 7.5l8 4.5 8-4.5M12 12v9M8 5.2l8 4.6'/>`,
  refresh: `<path d='M20 11a8 8 0 0 0-14.9-3.5M4 4v4h4'/><path d='M4 13a8 8 0 0 0 14.9 3.5M20 20v-4h-4'/>`,
  switch: `<circle cx='5.5' cy='12' r='2'/><path d='M7.5 12H11l5-5.5M11 12l5 5.5M16 6.5h4M16 17.5h4'/>`,
  stamp: `<path d='M5 21h14M6 17.5h12V15a2 2 0 0 0-2-2h-2.4l.6-4.2a2.4 2.4 0 1 0-4.4 0l.6 4.2H8a2 2 0 0 0-2 2z'/>`,
  pages: `<rect x='7.5' y='3' width='12' height='15' rx='2'/><path d='M4.5 7v12a2 2 0 0 0 2 2H15'/>`,
  volume: `<path d='M4 9.5v5h3.5L12 18.5v-13L7.5 9.5z'/><path d='M15.5 9a4.5 4.5 0 0 1 0 6M18.3 6.5a8 8 0 0 1 0 11'/>`,
  silence: `<path d='M4 9.5v5h3.5L12 18.5v-13L7.5 9.5z'/><path d='M16 9.5l5 5M21 9.5l-5 5'/>`,
  eq: `<path d='M5 21v-6M5 11V3M12 21v-9M12 8V3M19 21v-4M19 13V3M2.5 15h5M9.5 8h5M16.5 17h5'/>`,
  aspect: `<rect x='3' y='6' width='18' height='12' rx='2'/><path d='M7 10v-1.5h2.5M17 14v1.5h-2.5'/>`,
  slideshow: `<rect x='3' y='4' width='18' height='12.5' rx='2'/><path d='M10 7.8v5l4.2-2.5z'/><path d='M7.5 20.5h9'/>`,
  mux: `<rect x='2.5' y='5' width='11' height='10' rx='2'/><path d='M13.5 9l4-2.2v6.4l-4-2.2'/><path d='M15 20v-2M18 20v-4.5M21 20v-2.5'/>`,
  key: `<circle cx='8' cy='15' r='4'/><path d='M11 12L20 3M16.5 6.5l2.5 2.5M14.2 8.8l2 2'/>`,
  folders: `<path d='M3 7.5A1.5 1.5 0 0 1 4.5 6H8l1.7 2h7.8A1.5 1.5 0 0 1 19 9.5v8a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 3 17.5z'/><path d='M6.5 3.5h4l1.6 2'/>`,
  clean: `<path d='M14.5 3.5l6 6M17 7l-8.5 8.5'/><path d='M8 13l3 3-4.5 4.5H3.5V17.5z'/>`,
  puzzle: `<path d='M10 3.5h4v2.2a1.8 1.8 0 1 0 3.6 0V3.5H20.5V9h-2.2a1.8 1.8 0 1 0 0 3.6h2.2v7.9H15v-2.2a1.8 1.8 0 1 0-3.6 0v2.2H3.5V15h2.2a1.8 1.8 0 1 0 0-3.6H3.5V3.5z'/>`,
  more: `<circle cx='5' cy='12' r='1.2' fill='currentColor'/><circle cx='12' cy='12' r='1.2' fill='currentColor'/><circle cx='19' cy='12' r='1.2' fill='currentColor'/>`,
  'chevron-up': `<path d='M6 15l6-6 6 6'/>`,
  map: `<path d='M9 4.5L3 6.5v13l6-2 6 2 6-2v-13l-6 2z'/><path d='M9 4.5v13M15 6.5v13'/>`,
  monitor: `<rect x='2.5' y='4' width='19' height='13' rx='2'/><path d='M8 21h8M12 17v4'/>`,
  pin: `<path d='M9 3.5h6M10 3.5v6l-3.5 4h11L14 9.5v-6M12 13.5v7'/>`,
  sun: `<circle cx='12' cy='12' r='4'/><path d='M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4'/>`,
  drop: `<path d='M12 3.5c3.5 4.2 6 7.6 6 10.5a6 6 0 0 1-12 0c0-2.9 2.5-6.3 6-10.5z'/>`,
  speed: `<path d='M5 18a8 8 0 1 1 14 0'/><path d='M12 14l3.5-5'/><path d='M8 18h8'/>`,
  check2: `<circle cx='12' cy='12' r='9'/><path d='M8 12.5l2.8 2.8L16.5 9.5'/>`,
  'arrow-right': `<path d='M5 12h14M13 6l6 6-6 6'/>`,
  'arrow-up-right': `<path d='M7 17L17 7M8 7h9v9'/>`,
  cpu: `<rect x='6' y='6' width='12' height='12' rx='2'/><rect x='9.5' y='9.5' width='5' height='5' rx='.8'/><path d='M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3'/>`,
};

export function icon(name, size = 16) {
  const tpl = document.createElement('template');
  tpl.innerHTML = `<svg class='ic' viewBox='0 0 24 24' width='${size}' height='${size}' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>${ICONS[name] || FILE}</svg>`;
  return tpl.content.firstElementChild;
}

const KIND_ICON = { image: 'image', video: 'video', audio: 'audio', mesh: 'mesh', archive: 'archive', data: 'data', doc: 'doc', text: 'text', sheet: 'sheet', pdf: 'pdf' };
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
