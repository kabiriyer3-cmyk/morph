// Serves src/ over HTTP, opens it in headless Chrome in demo mode, verifies the app booted
// without errors and saves docs/screenshot.png (canvas) and docs/dashboard.png (dashboard).
// Uses the DevTools protocol for exact 1600x1000 shots; falls back to --screenshot on old Node.
import http from 'node:http';
import { readFileSync, existsSync, statSync, mkdirSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, dirname, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';

const run = promisify(execFile);
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'src');
const docs = join(root, 'docs');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png', '.json': 'application/json', '.woff2': 'font/woff2' };
const W = 1600;
const H = 1000;

const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p === '/') p = '/index.html';
  const f = normalize(join(src, p));
  if (!f.startsWith(src) || !existsSync(f) || statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': mime[extname(f)] || 'application/octet-stream' });
  res.end(readFileSync(f));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}/index.html?demo=1&smoke=1`;

const chrome = ['google-chrome', 'google-chrome-stable', 'chromium', 'chromium-browser'].find((c) => {
  try { execFileSync('which', [c], { stdio: 'pipe' }); return true; } catch { return false; }
});
if (!chrome) { console.log('::warning::Chrome not found, smoke test skipped'); server.close(); process.exit(0); }
mkdirSync(docs, { recursive: true });

const errors = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function withCdp() {
  const profile = mkdtempSync(join(tmpdir(), 'morph-smoke-'));
  const proc = spawn(chrome, ['--headless=new', '--no-sandbox', '--disable-gpu', '--hide-scrollbars', '--force-device-scale-factor=1', `--window-size=${W},${H}`, '--remote-debugging-port=0', '--user-data-dir=' + profile, 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
  try {
    const port = await new Promise((resolve, reject) => {
      let buf = '';
      const tm = setTimeout(() => reject(new Error('Chrome did not open a DevTools port')), 30000);
      proc.stderr.on('data', (d) => {
        buf += d;
        const m = /DevTools listening on ws:\/\/[^:]+:(\d+)\//.exec(buf);
        if (m) { clearTimeout(tm); resolve(Number(m[1])); }
      });
      proc.on('exit', () => reject(new Error('Chrome exited early')));
    });
    let page = null;
    for (let i = 0; i < 50 && !page; i++) {
      try { page = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t) => t.type === 'page'); } catch { /* not yet */ }
      if (!page) await sleep(200);
    }
    if (!page) throw new Error('no page target');
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((r, j) => { ws.addEventListener('open', r); ws.addEventListener('error', j); });
    let id = 0;
    const pending = new Map();
    ws.addEventListener('message', (m) => {
      const d = JSON.parse(m.data);
      if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
    });
    const send = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
    const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true })).result?.result?.value;
    await send('Emulation.setDeviceMetricsOverride', { width: W, height: H, deviceScaleFactor: 1, mobile: false });
    await send('Page.enable');

    const shoot = async (query, file, label) => {
      await send('Page.navigate', { url: base + query });
      let ready = false;
      for (let i = 0; i < 120 && !ready; i++) {
        await sleep(250);
        ready = (await evaluate("document.body && document.body.dataset.ready === '1'")) === true;
      }
      if (!ready) errors.push(`${label}: app did not finish booting (body[data-ready] missing)`);
      await sleep(600);
      const boot = await evaluate("(document.getElementById('boot-errors') || {}).textContent || ''");
      if (boot && boot.trim()) errors.push(`${label}: runtime errors:\n${boot.trim()}`);
      const nodes = await evaluate("document.querySelectorAll('.node').length");
      console.log(`smoke ${label}: ${nodes} nodes rendered`);
      if (!(nodes >= 3)) errors.push(`${label}: expected the demo graph to render nodes, found ${nodes}`);
      const shot = await send('Page.captureScreenshot', { format: 'png' });
      writeFileSync(join(docs, file), Buffer.from(shot.result.data, 'base64'));
    };
    await shoot('', 'screenshot.png', 'canvas');
    await shoot('&view=home', 'dashboard.png', 'dashboard');
    ws.close();
  } finally {
    proc.kill();
    await sleep(300);
    try { rmSync(profile, { recursive: true, force: true }); } catch { /* still locked */ }
  }
}

async function legacy() {
  const flags = ['--headless=new', '--no-sandbox', '--disable-gpu', '--hide-scrollbars', '--force-device-scale-factor=1', `--window-size=${W},${H}`, '--virtual-time-budget=15000'];
  const { stdout: dom } = await run(chrome, [...flags, '--dump-dom', base], { maxBuffer: 64 << 20, timeout: 120000 });
  if (!/data-ready="1"/.test(dom)) errors.push('app did not finish booting (body[data-ready] missing)');
  const boot = /<pre id="boot-errors"[^>]*>([\s\S]*?)<\/pre>/.exec(dom);
  if (boot && boot[1].trim()) errors.push(`runtime errors:\n${boot[1].trim()}`);
  const nodes = (dom.match(/class="node[ "]/g) || []).length;
  if (nodes < 3) errors.push(`expected the demo graph to render nodes, found ${nodes}`);
  console.log(`smoke: ${nodes} nodes rendered`);
  await run(chrome, [...flags, `--screenshot=${join(docs, 'screenshot.png')}`, base], { timeout: 120000 });
}

try {
  if (typeof WebSocket === 'function') await withCdp();
  else await legacy();
} catch (e) {
  errors.push(`chrome failed: ${e.message}`);
}
server.close();

if (errors.length) {
  for (const e of errors) console.log(`::error::${e.replace(/%/g, '%25').replace(/\n/g, '%0A')}`);
  process.exit(1);
}
console.log('smoke OK');
