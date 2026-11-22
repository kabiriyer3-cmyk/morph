// Static checks for the vanilla JS frontend (there is no bundler):
// syntax of every module, node catalog vs Rust dispatcher, templates, i18n parity.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { execFileSync } from 'node:child_process';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const errors = [];
const walk = (d) => readdirSync(d).flatMap((f) => {
  const p = join(d, f);
  return statSync(p).isDirectory() ? walk(p) : [p];
});

for (const f of walk(join(root, 'src')).filter((f) => f.endsWith('.js'))) {
  try {
    execFileSync(process.execPath, ['--check', f], { stdio: 'pipe' });
  } catch (e) {
    errors.push(`${f}: ${String(e.stderr || e.message)}`);
  }
}

const load = (p) => import(pathToFileURL(join(root, p)).href);

if (!errors.length) {
  const { NODE_DEFS, TYPES } = await load('src/js/nodes.js');
  const { TEMPLATES } = await load('src/js/templates.js');
  const { STRINGS } = await load('src/js/i18n.js');
  const rust = readFileSync(join(root, 'src-tauri/src/ops/mod.rs'), 'utf8');

  for (const [type, def] of Object.entries(NODE_DEFS)) {
    if (!def.frontendOnly && !rust.includes(`"${type}"`)) errors.push(`node ${type} is not handled in src-tauri/src/ops/mod.rs`);
    for (const s of [...(def.inputs || []), ...(def.outputs || [])]) {
      if (!TYPES[s.type]) errors.push(`${type}: unknown socket type ${s.type}`);
    }
    const names = new Set();
    for (const w of def.widgets || []) {
      if (names.has(w.name)) errors.push(`${type}: duplicate widget ${w.name}`);
      names.add(w.name);
      if (w.kind === 'combo' && !(w.options || []).includes(w.value)) errors.push(`${type}.${w.name}: default ${w.value} is not an option`);
    }
  }

  for (const t of TEMPLATES) {
    let g;
    try { g = t.build(); } catch (e) { errors.push(`template ${t.id}: ${e.message}`); continue; }
    g.nodes.forEach((n) => {
      const def = NODE_DEFS[n.type];
      if (!def) return errors.push(`template ${t.id}: unknown node ${n.type}`);
      for (const k of Object.keys(n.values || {})) {
        const w = (def.widgets || []).find((w) => w.name === k);
        if (!w) errors.push(`template ${t.id}: ${n.type} has no widget ${k}`);
        else if (w.kind === 'combo' && !w.options.includes(n.values[k])) errors.push(`template ${t.id}: ${n.type}.${k}=${n.values[k]} is not an option`);
      }
    });
    for (const [from, out, to, inp] of g.links) {
      const a = NODE_DEFS[g.nodes[from]?.type];
      const b = NODE_DEFS[g.nodes[to]?.type];
      if (!a || !b || !a.outputs[out] || !b.inputs[inp]) errors.push(`template ${t.id}: bad link ${from}:${out} -> ${to}:${inp}`);
    }
  }

  const en = Object.keys(STRINGS.en);
  const ru = new Set(Object.keys(STRINGS.ru));
  for (const k of en) if (!ru.has(k)) errors.push(`i18n: ru is missing "${k}"`);
}

if (errors.length) {
  for (const e of errors) console.log(`::error::${e.replace(/%/g, '%25').replace(/\n/g, '%0A')}`);
  process.exit(1);
}
console.log('frontend OK');
