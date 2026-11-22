// Minimal WebGL mesh viewer: orbit (left drag), pan (right drag or shift), zoom (wheel), reset (double click).
const VS = `attribute vec3 aPos;
attribute vec3 aNor;
attribute vec3 aCol;
uniform mat4 uMVP;
varying vec3 vNor;
varying vec3 vCol;
void main() {
  vNor = aNor;
  vCol = aCol;
  gl_Position = uMVP * vec4(aPos, 1.0);
}`;

const FS = `precision mediump float;
varying vec3 vNor;
varying vec3 vCol;
uniform vec3 uLight;
uniform vec4 uFlat;
void main() {
  if (uFlat.a > 0.5) {
    gl_FragColor = vec4(uFlat.rgb, 1.0);
    return;
  }
  vec3 n = normalize(vNor);
  if (!gl_FrontFacing) n = -n;
  float d = max(dot(n, normalize(uLight)), 0.0);
  float hemi = 0.5 + 0.5 * n.y;
  vec3 c = vCol * (0.22 + 0.33 * hemi + 0.62 * d);
  gl_FragColor = vec4(c, 1.0);
}`;

const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a) => {
  const l = Math.hypot(a[0], a[1], a[2]) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
};
const clampN = (v, a, b) => Math.min(b, Math.max(a, v));

function persp(fov, asp, n, f) {
  const t = 1 / Math.tan(fov / 2);
  const m = new Float32Array(16);
  m[0] = t / asp;
  m[5] = t;
  m[10] = (f + n) / (n - f);
  m[11] = -1;
  m[14] = (2 * f * n) / (n - f);
  return m;
}

function lookAt(e, c, u) {
  const z = norm(sub(e, c));
  const x = norm(cross(u, z));
  const y = cross(z, x);
  return new Float32Array([x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot(x, e), -dot(y, e), -dot(z, e), 1]);
}

function mul(a, b) {
  const o = new Float32Array(16);
  for (let i = 0; i < 4; i++) {
    for (let j = 0; j < 4; j++) {
      let s = 0;
      for (let k = 0; k < 4; k++) s += a[k * 4 + j] * b[i * 4 + k];
      o[i * 4 + j] = s;
    }
  }
  return o;
}

function computeNormals(pos, idx) {
  const nor = new Float32Array(pos.length);
  const count = idx ? idx.length : pos.length / 3;
  const at = (k) => (idx ? idx[k] : k);
  for (let k = 0; k + 2 < count; k += 3) {
    const a = at(k) * 3;
    const b = at(k + 1) * 3;
    const c = at(k + 2) * 3;
    const ux = pos[b] - pos[a];
    const uy = pos[b + 1] - pos[a + 1];
    const uz = pos[b + 2] - pos[a + 2];
    const vx = pos[c] - pos[a];
    const vy = pos[c + 1] - pos[a + 1];
    const vz = pos[c + 2] - pos[a + 2];
    const nx = uy * vz - uz * vy;
    const ny = uz * vx - ux * vz;
    const nz = ux * vy - uy * vx;
    for (const p of [a, b, c]) {
      nor[p] += nx;
      nor[p + 1] += ny;
      nor[p + 2] += nz;
    }
  }
  for (let i = 0; i < nor.length; i += 3) {
    const l = Math.hypot(nor[i], nor[i + 1], nor[i + 2]) || 1;
    nor[i] /= l;
    nor[i + 1] /= l;
    nor[i + 2] /= l;
  }
  return nor;
}

export function createViewer(host, data) {
  const canvas = document.createElement('canvas');
  canvas.className = 'gl';
  host.append(canvas);
  const attrs = { antialias: true, alpha: true, premultipliedAlpha: false };
  let gl = canvas.getContext('webgl2', attrs);
  const v2 = !!gl;
  if (!gl) gl = canvas.getContext('webgl', attrs) || canvas.getContext('experimental-webgl', attrs);
  if (!gl) {
    canvas.remove();
    throw new Error('webgl');
  }
  const uint = v2 || !!gl.getExtension('OES_element_index_uint');

  const shader = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) || 'shader');
    return s;
  };
  const prog = gl.createProgram();
  gl.attachShader(prog, shader(gl.VERTEX_SHADER, VS));
  gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, FS));
  gl.bindAttribLocation(prog, 0, 'aPos');
  gl.bindAttribLocation(prog, 1, 'aNor');
  gl.bindAttribLocation(prog, 2, 'aCol');
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog) || 'link');
  const U = { mvp: gl.getUniformLocation(prog, 'uMVP'), light: gl.getUniformLocation(prog, 'uLight'), flat: gl.getUniformLocation(prog, 'uFlat') };

  // geometry, normalized to a 2-unit box around the origin
  let pos = Float32Array.from(data.positions || []);
  let idx = data.indices && data.indices.length ? data.indices : null;
  const mn = [Infinity, Infinity, Infinity];
  const mx = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < pos.length; i += 3) {
    for (let k = 0; k < 3; k++) {
      const v = pos[i + k];
      if (v < mn[k]) mn[k] = v;
      if (v > mx[k]) mx[k] = v;
    }
  }
  if (!pos.length) {
    mn.fill(-1);
    mx.fill(1);
  }
  const c = [0, 1, 2].map((k) => (mn[k] + mx[k]) / 2);
  const s = 2 / (Math.max(mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]) || 1);
  for (let i = 0; i < pos.length; i += 3) for (let k = 0; k < 3; k++) pos[i + k] = (pos[i + k] - c[k]) * s;
  const floorY = (mn[1] - c[1]) * s - 0.002;
  let nor = data.normals && data.normals.length === pos.length ? Float32Array.from(data.normals) : computeNormals(pos, idx);
  let col = data.colors && data.colors.length === pos.length ? Float32Array.from(data.colors) : null;
  if (!col) {
    col = new Float32Array(pos.length);
    for (let i = 0; i < col.length; i += 3) {
      col[i] = 0.8;
      col[i + 1] = 0.82;
      col[i + 2] = 0.88;
    }
  }
  const vcount = pos.length / 3;
  const big = vcount > 65535;
  if (idx && big && !uint) {
    // no 32-bit indices: de-index
    const p2 = new Float32Array(idx.length * 3);
    const n2 = new Float32Array(idx.length * 3);
    const c2 = new Float32Array(idx.length * 3);
    for (let k = 0; k < idx.length; k++) {
      const j = idx[k] * 3;
      for (let q = 0; q < 3; q++) {
        p2[k * 3 + q] = pos[j + q];
        n2[k * 3 + q] = nor[j + q];
        c2[k * 3 + q] = col[j + q];
      }
    }
    pos = p2;
    nor = n2;
    col = c2;
    idx = null;
  }

  const buf = (target, arr) => {
    const b = gl.createBuffer();
    gl.bindBuffer(target, b);
    gl.bufferData(target, arr, gl.STATIC_DRAW);
    return b;
  };
  const bPos = buf(gl.ARRAY_BUFFER, pos);
  const bNor = buf(gl.ARRAY_BUFFER, nor);
  const bCol = buf(gl.ARRAY_BUFFER, col);
  let bIdx = null;
  let idxType = 0;
  let triCount = pos.length / 3;
  const wide = (pos.length / 3) > 65535;
  if (idx) {
    const ta = wide ? Uint32Array.from(idx) : Uint16Array.from(idx);
    bIdx = buf(gl.ELEMENT_ARRAY_BUFFER, ta);
    idxType = wide ? gl.UNSIGNED_INT : gl.UNSIGNED_SHORT;
    triCount = ta.length;
  }

  let bWire = null;
  let wireCount = 0;
  let wireType = 0;
  const buildWire = () => {
    if (bWire || (wide && !uint)) return;
    const tris = idx ? idx.length : pos.length / 3;
    const lines = wide ? new Uint32Array(tris * 2) : new Uint16Array(tris * 2);
    for (let k = 0; k + 2 < tris; k += 3) {
      const a = idx ? idx[k] : k;
      const b = idx ? idx[k + 1] : k + 1;
      const cc = idx ? idx[k + 2] : k + 2;
      lines.set([a, b, b, cc, cc, a], k * 2);
    }
    bWire = buf(gl.ELEMENT_ARRAY_BUFFER, lines);
    wireCount = lines.length;
    wireType = wide ? gl.UNSIGNED_INT : gl.UNSIGNED_SHORT;
  };

  const grid = [];
  const N = 10;
  const step = 0.3;
  for (let i = -N; i <= N; i++) grid.push(i * step, floorY, -N * step, i * step, floorY, N * step, -N * step, floorY, i * step, N * step, floorY, i * step);
  const bGrid = buf(gl.ARRAY_BUFFER, new Float32Array(grid));
  const gridCount = grid.length / 3;

  const cam = { yaw: 0.7, pitch: 0.42, dist: 3.4, tx: 0, ty: 0, tz: 0 };
  const reset = () => Object.assign(cam, { yaw: 0.7, pitch: 0.42, dist: 3.4, tx: 0, ty: 0, tz: 0 });
  let wire = false;
  let raf = 0;
  let disposed = false;

  const bindAttr = (loc, b) => {
    gl.bindBuffer(gl.ARRAY_BUFFER, b);
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0);
  };

  const draw = () => {
    raf = 0;
    if (disposed) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
    const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    gl.viewport(0, 0, w, h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    gl.useProgram(prog);
    const cp = Math.cos(cam.pitch);
    const eye = [cam.tx + cam.dist * cp * Math.sin(cam.yaw), cam.ty + cam.dist * Math.sin(cam.pitch), cam.tz + cam.dist * cp * Math.cos(cam.yaw)];
    const mvp = mul(persp(0.8, w / h, 0.02, 100), lookAt(eye, [cam.tx, cam.ty, cam.tz], [0, 1, 0]));
    gl.uniformMatrix4fv(U.mvp, false, mvp);
    gl.uniform3fv(U.light, new Float32Array(norm([eye[0] - cam.tx + 1.2, eye[1] - cam.ty + 2, eye[2] - cam.tz + 0.6])));

    gl.uniform4f(U.flat, 0.24, 0.24, 0.28, 1);
    bindAttr(0, bGrid);
    gl.disableVertexAttribArray(1);
    gl.disableVertexAttribArray(2);
    gl.vertexAttrib3f(1, 0, 1, 0);
    gl.vertexAttrib3f(2, 1, 1, 1);
    gl.drawArrays(gl.LINES, 0, gridCount);

    gl.uniform4f(U.flat, 0, 0, 0, 0);
    bindAttr(0, bPos);
    bindAttr(1, bNor);
    bindAttr(2, bCol);
    gl.enable(gl.POLYGON_OFFSET_FILL);
    gl.polygonOffset(1, 1);
    if (bIdx) {
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, bIdx);
      gl.drawElements(gl.TRIANGLES, triCount, idxType, 0);
    } else {
      gl.drawArrays(gl.TRIANGLES, 0, triCount);
    }
    gl.disable(gl.POLYGON_OFFSET_FILL);
    if (wire) {
      buildWire();
      if (bWire) {
        gl.uniform4f(U.flat, 0.55, 0.65, 1, 1);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, bWire);
        gl.drawElements(gl.LINES, wireCount, wireType, 0);
      }
    }
  };
  const frame = () => {
    if (!raf && !disposed) raf = requestAnimationFrame(draw);
  };

  let drag = null;
  canvas.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    try {
      canvas.setPointerCapture(e.pointerId);
    } catch {
      // ignore
    }
    drag = { x: e.clientX, y: e.clientY, pan: e.button !== 0 || e.shiftKey };
  });
  canvas.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    drag.x = e.clientX;
    drag.y = e.clientY;
    if (drag.pan) {
      const k = cam.dist * 0.0018;
      cam.tx -= Math.cos(cam.yaw) * dx * k;
      cam.tz += Math.sin(cam.yaw) * dx * k;
      cam.ty += dy * k;
    } else {
      cam.yaw -= dx * 0.008;
      cam.pitch = clampN(cam.pitch + dy * 0.008, -1.45, 1.45);
    }
    frame();
  });
  const end = () => {
    drag = null;
  };
  canvas.addEventListener('pointerup', end);
  canvas.addEventListener('pointercancel', end);
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    e.stopPropagation();
    cam.dist = clampN(cam.dist * Math.exp(e.deltaY * 0.0012), 0.3, 30);
    frame();
  }, { passive: false });
  canvas.addEventListener('dblclick', () => {
    reset();
    frame();
  });
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  const ro = new ResizeObserver(() => frame());
  ro.observe(canvas);
  frame();

  return {
    setWire(v) {
      wire = !!v;
      frame();
    },
    reset() {
      reset();
      frame();
    },
    dispose() {
      disposed = true;
      ro.disconnect();
      if (raf) cancelAnimationFrame(raf);
      const lose = gl.getExtension('WEBGL_lose_context');
      if (lose) lose.loseContext();
      canvas.remove();
    },
  };
}
