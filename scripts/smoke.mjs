// Serves src/ over HTTP, opens it in headless Chrome in demo mode,
// verifies the app booted without errors and saves docs/screenshot.png.
import http from 'node:http';
import { readFileSync, existsSync, statSync, mkdirSync } from 'node:fs';
import { join, extname, dirname, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';

const run = promisify(execFile);
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'src');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png', '.json': 'application/json' };

const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p === '/') p = '/index.html';
  const f = normalize(join(src, p));
  if (!f.startsWith(src) || !existsSync(f) || statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': mime[extname(f)] || 'application/octet-stream' });
  res.end(readFileSync(f));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const url = `http://127.0.0.1:${server.address().port}/index.html?demo=1&smoke=1`;

const chrome = ['google-chrome', 'google-chrome-stable', 'chromium', 'chromium-browser'].find((c) => {
  try { execFileSync('which', [c], { stdio: 'pipe' }); return true; } catch { return false; }
});
if (!chrome) { console.log('::warning::Chrome not found, smoke test skipped'); server.close(); process.exit(0); }

const flags = ['--headless=new', '--no-sandbox', '--disable-gpu', '--hide-scrollbars', '--force-device-scale-factor=1', '--window-size=1600,1000', '--virtual-time-budget=15000'];
const errors = [];
try {
  const { stdout: dom } = await run(chrome, [...flags, '--dump-dom', url], { maxBuffer: 64 << 20, timeout: 120000 });
  if (!/data-ready="1"/.test(dom)) errors.push('app did not finish booting (body[data-ready] missing)');
  const boot = /<pre id="boot-errors"[^>]*>([\s\S]*?)<\/pre>/.exec(dom);
  if (boot && boot[1].trim()) errors.push(`runtime errors:\n${boot[1].trim()}`);
  const nodes = (dom.match(/class="node[ "]/g) || []).length;
  if (nodes < 3) errors.push(`expected the demo graph to render nodes, found ${nodes}`);
  console.log(`smoke: ${nodes} nodes rendered`);
  mkdirSync(join(root, 'docs'), { recursive: true });
  await run(chrome, [...flags, `--screenshot=${join(root, 'docs', 'screenshot.png')}`, url], { timeout: 120000 });
} catch (e) {
  errors.push(`chrome failed: ${e.message}`);
}
server.close();

if (errors.length) {
  for (const e of errors) console.log(`::error::${e.replace(/%/g, '%25').replace(/\n/g, '%0A')}`);
  process.exit(1);
}
console.log('smoke OK');
