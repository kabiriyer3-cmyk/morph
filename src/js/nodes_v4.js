// Node library additions for 0.4: Compress for Services (any file kind under an upload limit),
// device profiles for Convert Video and more video effects. main.js imports this module right
// after the strings, so every other module already sees the full library.
import { NODE_DEFS } from './nodes.js';
import { PROFILE_IDS, PROFILE_LBL, PROFILE_GROUPS } from './profiles.js';

// Same widget helpers as in nodes.js.
const num = (name, value, o = {}) => ({ kind: 'number', name, value, step: 1, ...o });
const slider = (name, value, min, max, o = {}) => ({ kind: 'slider', name, value, min, max, step: 1, ...o });
const combo = (name, value, options, o = {}) => ({ kind: 'combo', name, value, options, ...o });
const seg = (name, value, options, o = {}) => ({ kind: 'combo', name, value, options, seg: true, ...o });
const toggle = (name, value, o = {}) => ({ kind: 'toggle', name, value, ...o });
const when = (key, ...values) => ({ show: { [key]: values } });
const G = (group, list, adv = false) => list.map((w) => ({ ...w, group, ...(adv ? { adv: true } : {}) }));
const ADV = (list) => list.map((w) => ({ ...w, adv: true }));
const FIL = [{ name: 'files', type: 'FILES' }];
const PRESETS = ['ultrafast', 'superfast', 'veryfast', 'faster', 'fast', 'medium', 'slow', 'slower', 'veryslow'];

// Upload limits of chat apps and mail services (must match service_mb() in src-tauri/src/ops/share.rs).
export const SERVICES = {
  Chat: ['discord', 'discord_basic', 'discord_nitro', 'telegram', 'telegram_premium', 'whatsapp', 'whatsapp_doc', 'signal', 'imessage', 'messenger', 'slack', 'vk'],
  'E-mail': ['gmail', 'outlook', 'yahoo', 'email'],
  Other: ['custom'],
};
export const SERVICE_MB = { discord: 10, discord_basic: 50, discord_nitro: 500, telegram: 2000, telegram_premium: 4000, whatsapp: 16, whatsapp_doc: 2000, signal: 100, imessage: 100, messenger: 25, slack: 1000, vk: 200, gmail: 25, outlook: 20, yahoo: 25, email: 10 };
export const SERVICE_LBL = {
  discord: 'Discord (10 MB)', discord_basic: 'Discord Nitro Basic (50 MB)', discord_nitro: 'Discord Nitro (500 MB)', telegram: 'Telegram (2 GB)', telegram_premium: 'Telegram Premium (4 GB)',
  whatsapp: 'WhatsApp media (16 MB)', whatsapp_doc: 'WhatsApp document (2 GB)', signal: 'Signal (100 MB)', imessage: 'iMessage (100 MB)', messenger: 'Messenger (25 MB)', slack: 'Slack (1 GB)', vk: 'VK (200 MB)',
  gmail: 'Gmail (25 MB)', outlook: 'Outlook (20 MB)', yahoo: 'Yahoo Mail (25 MB)', email: 'Any e-mail (10 MB)', custom: 'custom size',
};
const HEIGHTS = ['0', '2160', '1440', '1080', '720', '480', '360'];
const HEIGHT_LBL = { 0: 'keep', 2160: '2160p (4K)', 1440: '1440p', 1080: '1080p', 720: '720p', 480: '480p', 360: '360p' };
// Convert Video: widgets that a device profile fills in are only shown for "custom".
const PROFILE_KEEP = new Set(['preset', 'hardware', 'keep_subtitles', 'strip_metadata']);
export const withProfile = (list) => [
  combo('profile', 'custom', PROFILE_IDS, { labels: PROFILE_LBL, groups: { Custom: ['custom'], ...PROFILE_GROUPS } }),
  ...list.map((w) => (PROFILE_KEEP.has(w.name) ? w : { ...w, show: { ...(w.show || {}), profile: ['custom'] } })),
];

NODE_DEFS['share.compress'] = {
  cat: 'convert', title: 'Compress for Services', icon: 'send', width: 300, keywords: 'discord telegram whatsapp gmail outlook slack signal imessage email mail size limit upload share send compress shrink fit 10mb 25mb nitro',
  desc: 'Fits any files under an upload limit: videos, images, GIFs, audio, PDFs, documents and archives. Every kind gets its own strategy, splitting into parts is the last resort.',
  inputs: FIL, outputs: FIL,
  widgets: [
    combo('service', 'discord', Object.values(SERVICES).flat(), { labels: SERVICE_LBL, groups: SERVICES }),
    num('size_mb', 25, { min: 0.05, step: 1, unit: 'MB', label: 'size limit', ...when('service', 'custom') }),
    toggle('skip_small', true, { label: 'keep files that already fit' }),
    ...G('Video', [
      seg('video_mode', 'compress', ['compress', 'split'], { label: 'videos', labels: { compress: 'compress', split: 'cut into parts' } }),
      combo('max_height', '1080', HEIGHTS, { label: 'max height', labels: HEIGHT_LBL, ...when('video_mode', 'compress') }),
      seg('codec', 'h264', ['h264', 'h265'], { labels: { h264: 'H.264', h265: 'HEVC' }, ...when('video_mode', 'compress') }),
    ]),
    ...G('Images', [seg('image_format', 'auto', ['auto', 'jpeg', 'webp'], { label: 'images as', labels: { auto: 'auto', jpeg: 'JPG', webp: 'WebP' } })]),
    ...G('Audio', [seg('audio_format', 'm4a', ['m4a', 'mp3', 'opus'], { label: 'audio as', labels: { m4a: 'M4A', mp3: 'MP3', opus: 'Opus' } })]),
    ...G('GIF', [seg('gif_mode', 'shrink', ['shrink', 'mp4'], { label: 'GIFs', labels: { shrink: 'smaller GIF', mp4: 'to MP4' } })]),
    ...G('Documents', [
      combo('pdf_mode', 'auto', ['auto', 'ebook', 'screen', 'printer', 'lossless'], { label: 'PDF', labels: { auto: 'auto', ebook: 'ebook (150 dpi)', screen: 'screen (72 dpi)', printer: 'printer (300 dpi)', lossless: 'lossless only' } }),
      toggle('zip', true, { label: 'zip compressible files' }),
      toggle('split', true, { label: 'split into parts if still too big' }),
    ]),
    ...ADV([
      slider('headroom', 4, 0, 20, { unit: '%', label: 'safety margin' }),
      num('audio_bitrate', 96, { min: 24, max: 320, unit: 'kbps', label: 'audio in videos' }),
      num('max_fps', 30, { min: 0, max: 240, label: 'max fps (0 = keep)' }),
      combo('preset', 'medium', PRESETS),
    ]),
  ],
};

// Convert Video: device profiles (iPhone, Android, consoles, TV, DVD, YouTube…) on top of the manual settings.
const vc = NODE_DEFS['video.convert'];
vc.keywords += ' profile device iphone ipad android samsung playstation xbox switch tv dvd vcd youtube tiktok';
vc.desc = 'Pick a device profile (iPhone, Android, PlayStation, TV, DVD, YouTube…) or set any container and codec yourself: MP4, MKV, WebM, MOV, AVI… H.264, HEVC, AV1, VP9, ProRes.';
vc.widgets = withProfile(vc.widgets);

// Video Filters: the classic converter effects (old film, emboss, sketch, mosaic).
const look = NODE_DEFS['video.filters'].widgets.find((w) => w.name === 'look');
look.options = ['none', 'grayscale', 'sepia', 'invert', 'vintage', 'old_film', 'emboss', 'sketch', 'mosaic', 'cross_process', 'strong_contrast', 'lighter', 'darker', 'increase_contrast', 'negative'];
look.label = 'effect';
look.labels = { ...(look.labels || {}), old_film: 'old film' };

export const NODE_TOTAL = Object.keys(NODE_DEFS).length;

// Deep link for the browser demo and the website: ?view=converter opens the Converter window.
if (typeof window !== 'undefined' && new URLSearchParams(location.search).get('view') === 'converter') {
  let tries = 0;
  const open = () => {
    const b = document.getElementById('btn-quick');
    const booting = (() => { const r = document.getElementById('boot'); return r && !r.hidden; })();
    if (b && b.offsetParent !== null && !booting) { b.click(); return; }
    if (++tries < 60) setTimeout(open, 200);
  };
  window.addEventListener('load', () => setTimeout(open, 300));
}
