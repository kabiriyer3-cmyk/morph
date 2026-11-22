// Color helpers shared by the editor, library and dialogs.
export function rgb(hex) {
  let h = String(hex || '').replace('#', '');
  if (h.length === 3) h = h.split('').map((c) => c + c).join('');
  const n = parseInt(h.slice(0, 6), 16);
  if (!Number.isFinite(n)) return [138, 147, 166];
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function tint(hex, a) {
  const [r, g, b] = rgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

// Sets --c and --caNN (alpha tints) on an element so CSS can use category colors without color-mix().
export function paint(e, hex) {
  e.style.setProperty('--c', hex);
  for (const a of [8, 12, 16, 20, 24, 30, 40, 60]) e.style.setProperty('--ca' + String(a).padStart(2, '0'), tint(hex, a / 100));
  return e;
}
