// Boot splash: dots scattered in 3D space gather into a rotating sphere, land on the seam
// mark as a halftone and resolve into the solid logo. Purely visual, the app boots underneath.
// Every frame is a pure function of time, so ?bootcap=1 can render it frame by frame.

const MARK = 'M197.2 209.9A35.8 35.8 0 0 1 182.6 214.9A103.4 103.4 0 1 0 297.1 329.4A35.8 35.8 0 0 1 302.1 314.8ZM209.6 197A35.8 35.8 0 0 0 214.3 179.4L214.3 150.6A59.6 59.6 0 0 1 273.9 91L361.4 91A59.6 59.6 0 0 1 421 150.6L421 238.1A59.6 59.6 0 0 1 361.4 297.7L332.6 297.7A35.8 35.8 0 0 0 315 302.4Z';
const VB = { x: 86, y: 86, size: 340 };
// Timeline in ms. The splash holds on the solid logo until the app reports ready.
const T = { fadeIn: 400, gather: 700, morph: 1000, morphLen: 600, solid: 1900, solidLen: 380, word: 2000, wordLen: 520, hold: 2550, exit: 450 };

const clamp01 = (x) => (x < 0 ? 0 : x > 1 ? 1 : x);
const span = (t, a, len) => clamp01((t - a) / len);
const out3 = (x) => 1 - (1 - x) ** 3;
const inOut3 = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);
const lerp = (a, b, k) => a + (b - a) * k;

function rng(seed) {
  let s = seed >>> 0;
  return () => (s = (Math.imul(s, 1664525) + 1013904223) >>> 0) / 4294967296;
}

// Halftone sample of the mark on a hex grid, as offsets from the mark center in [-0.5, 0.5].
function sampleMark(px, step) {
  const c = document.createElement('canvas');
  c.width = c.height = px;
  const g = c.getContext('2d', { willReadFrequently: true });
  g.scale(px / VB.size, px / VB.size);
  g.translate(-VB.x, -VB.y);
  g.fill(new Path2D(MARK));
  const data = g.getImageData(0, 0, px, px).data;
  const pts = [];
  const rowH = step * 0.866;
  for (let r = 0, y = rowH / 2; y < px; r++, y += rowH) {
    for (let x = r % 2 ? step : step / 2; x < px; x += step) {
      if (data[(Math.floor(y) * px + Math.floor(x)) * 4 + 3] > 150) pts.push([x / px - 0.5, y / px - 0.5]);
    }
  }
  return pts;
}

function buildScene() {
  const rnd = rng(20261122);
  const pts = sampleMark(220, 6.2);
  const n = pts.length;
  const golden = Math.PI * (3 - Math.sqrt(5));
  const dots = pts.map(([u, v], i) => {
    const y = 1 - (2 * (i + 0.5)) / n;
    const r = Math.sqrt(1 - y * y);
    const th = golden * i;
    return {
      u, v,
      sx: Math.cos(th) * r, sy: y, sz: Math.sin(th) * r,
      ox: (rnd() * 2 - 1) * 3.2, oy: (rnd() * 2 - 1) * 2.1, oz: 2 + rnd() * 9,
      d0: rnd() * 300,
      d1: Math.min(260, Math.abs(u - v - 0.037) * 620) + rnd() * 60,
      a: 0.7 + rnd() * 0.3,
    };
  });
  const dust = [];
  for (let i = 0; i < 460; i++) {
    const y = rnd() * 2 - 1;
    const r = Math.sqrt(1 - y * y);
    const th = rnd() * Math.PI * 2;
    const d = 2.1 + rnd() * 5.5;
    dust.push({ x: Math.cos(th) * r * d * 1.6, y: y * d * 0.9, z: Math.sin(th) * r * d, a: 0.08 + rnd() * 0.3, s: 0.6 + rnd() * 0.7, blue: rnd() < 0.35 });
  }
  return { dots, dust };
}

export function startBoot(opts = {}) {
  const root = document.getElementById('boot');
  const noop = { step() {}, version() {}, done: () => Promise.resolve(), active: false };
  if (!root) return noop;
  if (document.documentElement.dataset.boot === 'off' && !opts.force) {
    root.remove();
    return noop;
  }
  document.documentElement.dataset.boot = 'on';
  root.style.animation = 'none';
  root.hidden = false;
  const canvas = root.querySelector('#boot-fx');
  const ctx = canvas.getContext('2d');
  const mark = root.querySelector('.boot-mark');
  const word = root.querySelector('.boot-word');
  const logo = root.querySelector('.boot-logo');
  const foot = root.querySelector('.boot-foot');
  const stepEl = root.querySelector('.boot-step');
  const bar = root.querySelector('.boot-bar i');
  const verEl = root.querySelector('.boot-ver');
  const capture = /[?&]bootcap=1/.test(location.search);
  const { dots, dust } = buildScene();
  let W = 0;
  let H = 0;
  let dpr = 1;
  let box = null;
  const measure = () => {
    dpr = Math.min(2, window.devicePixelRatio || 1);
    W = window.innerWidth;
    H = window.innerHeight;
    canvas.width = Math.round(W * dpr);
    canvas.height = Math.round(H * dpr);
    box = mark.getBoundingClientRect();
  };
  measure();
  window.addEventListener('resize', measure);

  let t0 = performance.now();
  let ready = false;
  let exitAt = Infinity;
  let finished = false;
  let raf = 0;
  let resolveDone = null;
  const donePromise = new Promise((r) => (resolveDone = r));

  const render = (t) => {
    const S = box.width || 128;
    const cx = box.left + box.width / 2;
    const cy = box.top + box.height / 2;
    const R = S * 1.05;
    const f = 820;
    const yaw = 0.5 + 1.35 * (t / 1000) - 0.2 * (t / 1000) ** 2;
    const pitch = 0.42 * (1 - inOut3(span(t, T.morph, T.morphLen + 260)));
    const cyw = Math.cos(yaw);
    const syw = Math.sin(yaw);
    const cp = Math.cos(pitch);
    const sp = Math.sin(pitch);
    const solid = out3(span(t, T.solid, T.solidLen));
    const fade = out3(span(t, 0, T.fadeIn));
    const ex = exitAt === Infinity ? 0 : out3(span(t, exitAt, T.exit));

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    // ambient dust drifts with a slower yaw for parallax
    const dy = yaw * 0.3;
    const dc = Math.cos(dy);
    const ds = Math.sin(dy);
    const dustA = fade * (1 - 0.55 * solid) * (1 - ex);
    if (dustA > 0.01) {
      for (const p of dust) {
        const x = p.x * dc - p.z * ds;
        const z = p.x * ds + p.z * dc;
        const zz = z * R + R * 2.2;
        if (zz <= -f + 40) continue;
        const k = f / (f + zz);
        if (k > 2.4) continue;
        const sx = cx + x * R * k;
        const sy = cy + p.y * R * k;
        if (sx < -10 || sy < -10 || sx > W + 10 || sy > H + 10) continue;
        ctx.globalAlpha = p.a * dustA * clamp01(k * 1.4);
        ctx.fillStyle = p.blue ? '#8fb2ff' : '#ffffff';
        ctx.beginPath();
        ctx.arc(sx, sy, Math.min(1.7, Math.max(0.35, p.s * k * 1.25)), 0, 6.2832);
        ctx.fill();
      }
    }
    // the dots that become the mark
    const dotA = fade * (1 - solid * 0.92) * (1 - ex);
    if (dotA > 0.005) {
      ctx.fillStyle = '#ffffff';
      for (const p of dots) {
        const g = out3(span(t, p.d0, T.gather));
        // sphere point rotated by yaw then pitch
        const x1 = p.sx * cyw - p.sz * syw;
        const z1 = p.sx * syw + p.sz * cyw;
        const y2 = p.sy * cp - z1 * sp;
        const z2 = p.sy * sp + z1 * cp;
        let x = lerp(p.ox, x1, g) * R;
        let y = lerp(p.oy, y2, g) * R;
        let z = lerp(p.oz, z2, g) * R;
        const m = inOut3(span(t, T.morph + p.d1, T.morphLen));
        x = lerp(x, p.u * S, m);
        y = lerp(y, p.v * S, m);
        z = lerp(z, 0, m);
        const k = f / (f + z);
        const depth = clamp01(0.5 - z / (2 * R));
        const a = lerp(0.25 + 0.75 * depth, 1, m) * p.a * clamp01(g * 1.6 + 0.08);
        ctx.globalAlpha = a * dotA;
        ctx.beginPath();
        ctx.arc(cx + x * k, cy + y * k, Math.max(0.4, lerp(1.05 + depth * 0.7, S / 128 * 1.18, m) * k), 0, 6.2832);
        ctx.fill();
      }
    }
    ctx.globalAlpha = 1;
    mark.style.opacity = String(solid * (1 - ex));
    mark.style.transform = `scale(${lerp(0.97, 1, solid)})`;
    const w = out3(span(t, T.word, T.wordLen));
    word.style.opacity = String(w * 0.96 * (1 - ex));
    word.style.clipPath = `inset(0 ${((1 - w) * 100).toFixed(2)}% 0 0)`;
    word.style.transform = `translateY(${((1 - w) * 6).toFixed(2)}px)`;
    foot.style.opacity = String(out3(span(t, 250, 500)) * (1 - ex));
    logo.style.transform = `scale(${(1 + ex * 0.05).toFixed(4)})`;
    root.style.opacity = String(1 - ex);
    return ex >= 1;
  };

  const finish = () => {
    if (finished) return;
    finished = true;
    cancelAnimationFrame(raf);
    window.removeEventListener('resize', measure);
    window.removeEventListener('keydown', onKey, true);
    root.remove();
    resolveDone();
  };

  const startExit = (t) => {
    if (exitAt !== Infinity) return;
    exitAt = Math.max(t, 0);
    setTimeout(finish, T.exit + 900);
  };

  const frame = () => {
    const t = performance.now() - t0;
    if (ready && exitAt === Infinity && t >= T.hold) startExit(t);
    if (render(t)) return finish();
    raf = requestAnimationFrame(frame);
  };

  const skip = () => {
    const t = performance.now() - t0;
    if (t < T.hold) t0 -= T.hold - t;
    if (ready) startExit(T.hold);
  };
  const onKey = (e) => {
    e.stopPropagation();
    if (e.key !== 'Tab') e.preventDefault();
    skip();
  };
  root.addEventListener('pointerdown', skip);
  window.addEventListener('keydown', onKey, true);

  if (capture) {
    window.__bootFrame = (ms) => {
      if (ms >= T.hold && exitAt === Infinity) exitAt = T.hold;
      render(ms);
      return ms;
    };
    window.__bootTimeline = T;
  } else {
    raf = requestAnimationFrame(frame);
  }

  const api = {
    active: true,
    version(v) {
      verEl.textContent = v ? 'v' + v : '';
    },
    step(label, p) {
      if (label != null) stepEl.textContent = label;
      if (p != null) bar.style.width = Math.round(clamp01(p) * 100) + '%';
    },
    done() {
      if (!ready) {
        ready = true;
        api.step(null, 1);
        if (!capture) {
          const t = performance.now() - t0;
          if (t >= T.hold) startExit(t);
          // a hidden window never paints, so do not wait for frames forever
          setTimeout(finish, Math.max(0, T.hold - t) + T.exit + 1500);
        }
      }
      return donePromise;
    },
  };
  return api;
}

const BOOT_HTML = `<canvas id='boot-fx'></canvas><div class='boot-logo'><svg class='boot-mark' viewBox='86 86 340 340' aria-hidden='true'><path fill='currentColor' d='${MARK}'/></svg><img class='boot-word' src='assets/wordmark.svg' alt=''></div><div class='boot-foot'><span class='boot-step'></span><div class='boot-bar'><i></i></div><span class='boot-ver'></span></div>`;

// Plays the splash again on top of the running app (Settings, Appearance).
export function replayBoot(version) {
  if (document.getElementById('boot')) return;
  const root = document.createElement('div');
  root.id = 'boot';
  root.setAttribute('aria-hidden', 'true');
  root.innerHTML = BOOT_HTML;
  document.body.prepend(root);
  const b = startBoot({ force: true });
  b.version(version);
  b.step('', 1);
  b.done();
}
