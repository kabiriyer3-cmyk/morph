// Collects every compiler error from `cargo check --message-format=json`
// and prints them as a single GitHub annotation so they can be read in one go.
import { readFileSync, existsSync } from 'node:fs';

const [jsonPath = 'cargo.json', errPath = 'cargo.err', codeArg = '0'] = process.argv.slice(2);
const code = Number(codeArg) || 0;
const errors = [];
let warnings = 0;

for (const line of (existsSync(jsonPath) ? readFileSync(jsonPath, 'utf8') : '').split('\n')) {
  if (!line.startsWith('{')) continue;
  let msg;
  try { msg = JSON.parse(line); } catch { continue; }
  if (msg.reason !== 'compiler-message') continue;
  const m = msg.message;
  if (m.level === 'error') errors.push(m.rendered || m.message);
  else if (m.level === 'warning') warnings++;
}

console.log(`cargo check: ${errors.length} error(s), ${warnings} warning(s), exit ${code}`);
for (const e of errors) console.log(e);

let body = errors.join('\n');
if (!body && code !== 0 && existsSync(errPath)) {
  body = readFileSync(errPath, 'utf8').split('\n').slice(-80).join('\n');
  console.log(body);
}
if (body) {
  if (body.length > 60000) body = body.slice(0, 60000) + '\n... truncated';
  const esc = body.replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
  console.log(`::error title=cargo check ${process.env.RUNNER_OS || ''}::${esc}`);
}
