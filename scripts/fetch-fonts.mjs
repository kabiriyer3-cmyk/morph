// Downloads the Inter variable font (SIL Open Font License) into src/fonts so the desktop
// build and the README screenshots use it. The UI falls back to system fonts without it.
import { existsSync, mkdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const dir = join(root, 'src', 'fonts');
const font = join(dir, 'InterVariable.woff2');
const FONT_URLS = [
  'https://rsms.me/inter/font-files/InterVariable.woff2?v=4.1',
  'https://cdn.jsdelivr.net/gh/rsms/inter@v4.1/docs/font-files/InterVariable.woff2',
  'https://raw.githubusercontent.com/rsms/inter/v4.1/docs/font-files/InterVariable.woff2',
  'https://raw.githubusercontent.com/rsms/inter/master/docs/font-files/InterVariable.woff2',
];
const LICENSE_URLS = [
  'https://raw.githubusercontent.com/rsms/inter/v4.1/LICENSE.txt',
  'https://raw.githubusercontent.com/rsms/inter/master/LICENSE.txt',
  'https://cdn.jsdelivr.net/gh/rsms/inter@v4.1/LICENSE.txt',
];

async function get(urls, check) {
  for (const url of urls) {
    try {
      const res = await fetch(url, { redirect: 'follow', signal: AbortSignal.timeout(30000) });
      if (!res.ok) throw new Error('HTTP ' + res.status);
      const buf = Buffer.from(await res.arrayBuffer());
      check(buf);
      return { buf, host: new URL(url).host };
    } catch (e) {
      console.log(`font: ${url} failed (${e.message})`);
    }
  }
  return null;
}

mkdirSync(dir, { recursive: true });
if (existsSync(font) && statSync(font).size > 100000 && !process.argv.includes('--force')) {
  console.log('font: Inter is already in src/fonts');
} else {
  const f = await get(FONT_URLS, (b) => {
    if (b.length < 100000 || b.toString('latin1', 0, 4) !== 'wOF2') throw new Error('not a woff2 file');
  });
  if (f) {
    writeFileSync(font, f.buf);
    console.log(`font: Inter ${Math.round(f.buf.length / 1024)} KB from ${f.host}`);
  } else {
    console.log('::warning::Inter could not be downloaded, the UI will use system fonts');
  }
}
const lic = join(dir, 'Inter-LICENSE.txt');
if (!existsSync(lic)) {
  const l = await get(LICENSE_URLS, (b) => {
    if (!/Open Font License/i.test(b.toString('utf8'))) throw new Error('unexpected license text');
  });
  writeFileSync(lic, l ? l.buf : 'Inter is copyright (c) The Inter Project Authors and is licensed under the SIL Open Font License 1.1.\nhttps://github.com/rsms/inter/blob/master/LICENSE.txt\n');
}
