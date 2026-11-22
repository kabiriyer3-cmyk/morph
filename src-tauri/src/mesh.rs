//! Minimal triangle-mesh toolkit: OBJ / STL / PLY / OFF / glTF+GLB in, GLB / OBJ / STL / PLY / OFF out,
//! plus welding, cleanup, normals and vertex-clustering simplification.
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

#[derive(Clone, Default, Debug)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    /// Empty or one per position.
    pub normals: Vec<[f32; 3]>,
    /// Empty or one per position (linear 0..1).
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-20 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

impl Mesh {
    pub fn tri_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn bbox(&self) -> ([f32; 3], [f32; 3]) {
        if self.positions.is_empty() {
            return ([0.0; 3], [0.0; 3]);
        }
        let mut mn = [f32::MAX; 3];
        let mut mx = [f32::MIN; 3];
        for p in &self.positions {
            for k in 0..3 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }
        (mn, mx)
    }

    pub fn stats(&self) -> Value {
        let (mn, mx) = self.bbox();
        json!({
            "vertices": self.positions.len(),
            "triangles": self.tri_count(),
            "size": [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]],
            "min": mn,
            "max": mx,
            "has_normals": !self.normals.is_empty(),
            "has_colors": !self.colors.is_empty()
        })
    }

    pub fn validate(&mut self) {
        let n = self.positions.len() as u32;
        if self.normals.len() != self.positions.len() {
            self.normals.clear();
        }
        if self.colors.len() != self.positions.len() {
            self.colors.clear();
        }
        let mut idx = Vec::with_capacity(self.indices.len());
        for t in self.indices.chunks_exact(3) {
            if t.iter().all(|&i| i < n) {
                idx.extend_from_slice(t);
            }
        }
        self.indices = idx;
    }

    /// Applies a linear map (rotation / axis swap) to positions and normals.
    pub fn apply_linear<F: Fn([f32; 3]) -> [f32; 3]>(&mut self, f: F) {
        for p in self.positions.iter_mut() {
            *p = f(*p);
        }
        for n in self.normals.iter_mut() {
            *n = normalize(f(*n));
        }
    }

    pub fn rotate(&mut self, axis: usize, deg: f32) {
        let (s, c) = deg.to_radians().sin_cos();
        match axis {
            0 => self.apply_linear(|p| [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]),
            1 => self.apply_linear(|p| [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]),
            _ => self.apply_linear(|p| [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]),
        }
    }

    pub fn scale(&mut self, s: f32) {
        for p in self.positions.iter_mut() {
            *p = [p[0] * s, p[1] * s, p[2] * s];
        }
    }

    pub fn translate(&mut self, t: [f32; 3]) {
        for p in self.positions.iter_mut() {
            *p = add(*p, t);
        }
    }

    pub fn flip(&mut self) {
        for t in self.indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
        for n in self.normals.iter_mut() {
            *n = [-n[0], -n[1], -n[2]];
        }
    }

    /// Removes unreferenced vertices.
    pub fn compact(&mut self) {
        let n = self.positions.len();
        let has_n = !self.normals.is_empty();
        let has_c = !self.colors.is_empty();
        let mut remap = vec![u32::MAX; n];
        let (mut pos, mut nor, mut col) = (vec![], vec![], vec![]);
        for i in self.indices.iter_mut() {
            let o = *i as usize;
            if remap[o] == u32::MAX {
                remap[o] = pos.len() as u32;
                pos.push(self.positions[o]);
                if has_n {
                    nor.push(self.normals[o]);
                }
                if has_c {
                    col.push(self.colors[o]);
                }
            }
            *i = remap[o];
        }
        self.positions = pos;
        self.normals = nor;
        self.colors = col;
    }

    /// Merges vertices closer than `tol` (0 = exact duplicates only).
    pub fn weld(&mut self, tol: f32) {
        let n = self.positions.len();
        let has_n = !self.normals.is_empty();
        let has_c = !self.colors.is_empty();
        let mut map: HashMap<[i64; 3], u32> = HashMap::with_capacity(n);
        let mut remap = vec![0u32; n];
        let (mut pos, mut nor, mut col) = (vec![], vec![], vec![]);
        for i in 0..n {
            let p = self.positions[i];
            let key = if tol > 0.0 {
                [(p[0] / tol).round() as i64, (p[1] / tol).round() as i64, (p[2] / tol).round() as i64]
            } else {
                [(p[0] + 0.0).to_bits() as i64, (p[1] + 0.0).to_bits() as i64, (p[2] + 0.0).to_bits() as i64]
            };
            let id = match map.get(&key) {
                Some(&id) => id,
                None => {
                    let id = pos.len() as u32;
                    pos.push(p);
                    if has_n {
                        nor.push(self.normals[i]);
                    }
                    if has_c {
                        col.push(self.colors[i]);
                    }
                    map.insert(key, id);
                    id
                }
            };
            remap[i] = id;
        }
        for i in self.indices.iter_mut() {
            *i = remap[*i as usize];
        }
        self.positions = pos;
        self.normals = nor;
        self.colors = col;
        self.remove_degenerate();
    }

    /// Drops zero-area and duplicate triangles, then unused vertices.
    pub fn remove_degenerate(&mut self) {
        let mut seen: HashSet<[u32; 3]> = HashSet::new();
        let mut idx = Vec::with_capacity(self.indices.len());
        for t in self.indices.chunks_exact(3) {
            let (a, b, c) = (t[0], t[1], t[2]);
            if a == b || b == c || a == c {
                continue;
            }
            let pa = self.positions[a as usize];
            let nrm = cross(sub(self.positions[b as usize], pa), sub(self.positions[c as usize], pa));
            if dot(nrm, nrm) <= 1e-30 {
                continue;
            }
            let mut k = [a, b, c];
            k.sort_unstable();
            if !seen.insert(k) {
                continue;
            }
            idx.extend_from_slice(&[a, b, c]);
        }
        self.indices = idx;
        self.compact();
    }

    /// Smooth, area-weighted vertex normals.
    pub fn compute_normals(&mut self) {
        let mut acc = vec![[0f32; 3]; self.positions.len()];
        for t in self.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = cross(sub(self.positions[b], self.positions[a]), sub(self.positions[c], self.positions[a]));
            for v in [a, b, c] {
                acc[v] = add(acc[v], n);
            }
        }
        self.normals = acc.into_iter().map(normalize).collect();
    }

    /// Faceted look: every triangle gets its own vertices.
    pub fn flat_normals(&mut self) {
        let has_c = !self.colors.is_empty();
        let (mut pos, mut nor, mut col) = (vec![], vec![], vec![]);
        for t in self.indices.chunks_exact(3) {
            let p = [self.positions[t[0] as usize], self.positions[t[1] as usize], self.positions[t[2] as usize]];
            let n = normalize(cross(sub(p[1], p[0]), sub(p[2], p[0])));
            for k in 0..3 {
                pos.push(p[k]);
                nor.push(n);
                if has_c {
                    col.push(self.colors[t[k] as usize]);
                }
            }
        }
        self.indices = (0..pos.len() as u32).collect();
        self.positions = pos;
        self.normals = nor;
        self.colors = col;
    }

    fn cluster(&self, res: usize, mn: [f32; 3], ext: f32) -> Mesh {
        let cell = ext / res as f32;
        let has_n = !self.normals.is_empty();
        let has_c = !self.colors.is_empty();
        let mut map: HashMap<(i32, i32, i32), u32> = HashMap::new();
        let mut sums: Vec<[f64; 3]> = vec![];
        let mut cnt: Vec<u32> = vec![];
        let mut nsum: Vec<[f32; 3]> = vec![];
        let mut csum: Vec<[f32; 3]> = vec![];
        let mut remap = Vec::with_capacity(self.positions.len());
        for (i, p) in self.positions.iter().enumerate() {
            let key = (
                ((p[0] - mn[0]) / cell) as i32,
                ((p[1] - mn[1]) / cell) as i32,
                ((p[2] - mn[2]) / cell) as i32,
            );
            let id = match map.get(&key) {
                Some(&id) => id,
                None => {
                    let id = sums.len() as u32;
                    map.insert(key, id);
                    sums.push([0.0; 3]);
                    cnt.push(0);
                    if has_n {
                        nsum.push([0.0; 3]);
                    }
                    if has_c {
                        csum.push([0.0; 3]);
                    }
                    id
                }
            };
            let k = id as usize;
            sums[k][0] += p[0] as f64;
            sums[k][1] += p[1] as f64;
            sums[k][2] += p[2] as f64;
            cnt[k] += 1;
            if has_n {
                nsum[k] = add(nsum[k], self.normals[i]);
            }
            if has_c {
                csum[k] = add(csum[k], self.colors[i]);
            }
            remap.push(id);
        }
        let positions: Vec<[f32; 3]> = sums
            .iter()
            .zip(&cnt)
            .map(|(s, &c)| {
                let c = c.max(1) as f64;
                [(s[0] / c) as f32, (s[1] / c) as f32, (s[2] / c) as f32]
            })
            .collect();
        let normals: Vec<[f32; 3]> = nsum.into_iter().map(normalize).collect();
        let colors: Vec<[f32; 3]> = csum
            .iter()
            .zip(&cnt)
            .map(|(s, &c)| {
                let c = c.max(1) as f32;
                [s[0] / c, s[1] / c, s[2] / c]
            })
            .collect();
        let mut seen: HashSet<[u32; 3]> = HashSet::new();
        let mut indices = vec![];
        for t in self.indices.chunks_exact(3) {
            let (a, b, c) = (remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]);
            if a == b || b == c || a == c {
                continue;
            }
            let mut k = [a, b, c];
            k.sort_unstable();
            if seen.insert(k) {
                indices.extend_from_slice(&[a, b, c]);
            }
        }
        let mut m = Mesh { positions, normals, colors, indices };
        m.compact();
        m
    }

    /// Vertex-clustering decimation down to at most `target` triangles.
    pub fn simplify(&mut self, target: usize) {
        if self.tri_count() <= target || self.positions.is_empty() {
            return;
        }
        let (mn, mx) = self.bbox();
        let ext = (mx[0] - mn[0]).max(mx[1] - mn[1]).max(mx[2] - mn[2]).max(1e-9) * 1.0001;
        let (mut lo, mut hi) = (2usize, 4096usize);
        let mut best: Option<Mesh> = None;
        while lo <= hi {
            let mid = (lo + hi) / 2;
            let c = self.cluster(mid, mn, ext);
            if c.tri_count() <= target {
                best = Some(c);
                lo = mid + 1;
            } else {
                hi = mid - 1;
            }
        }
        *self = match best {
            Some(b) => b,
            None => self.cluster(2, mn, ext),
        };
    }
}

// ---------------------------------------------------------------- loading

pub fn load(path: &Path) -> Result<Mesh> {
    let ext = crate::kinds::ext_of(path);
    let mut m = match ext.as_str() {
        "obj" => load_obj(path)?,
        "stl" => load_stl(path)?,
        "ply" => load_ply(path)?,
        "off" => load_off(path)?,
        "glb" | "gltf" => load_gltf(path)?,
        e => bail!("Unsupported 3D format .{e}"),
    };
    m.validate();
    if m.positions.is_empty() || m.indices.is_empty() {
        bail!("No triangle geometry found");
    }
    Ok(m)
}

fn parse_idx(s: Option<&str>, len: usize) -> Option<usize> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    let i: i64 = s.parse().ok()?;
    let r = if i > 0 { i - 1 } else { len as i64 + i };
    if r >= 0 && (r as usize) < len {
        Some(r as usize)
    } else {
        None
    }
}

fn floats(it: std::str::SplitWhitespace) -> Vec<f32> {
    it.filter_map(|s| s.parse().ok()).collect()
}

fn load_obj(path: &Path) -> Result<Mesh> {
    let text = String::from_utf8_lossy(&std::fs::read(path)?).into_owned();
    let mut v: Vec<[f32; 3]> = vec![];
    let mut vc: Vec<Option<[f32; 3]>> = vec![];
    let mut vn: Vec<[f32; 3]> = vec![];
    let mut m = Mesh::default();
    let mut map: HashMap<(usize, usize), u32> = HashMap::new();
    let (mut any_n, mut any_c) = (false, false);
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let n = floats(it);
                if n.len() >= 3 {
                    v.push([n[0], n[1], n[2]]);
                    if n.len() >= 6 {
                        vc.push(Some([n[3], n[4], n[5]]));
                        any_c = true;
                    } else {
                        vc.push(None);
                    }
                }
            }
            Some("vn") => {
                let n = floats(it);
                if n.len() >= 3 {
                    vn.push([n[0], n[1], n[2]]);
                }
            }
            Some("f") => {
                let mut face: Vec<u32> = vec![];
                for tok in it {
                    let mut parts = tok.split('/');
                    let Some(vi) = parse_idx(parts.next(), v.len()) else { continue };
                    let _vt = parts.next();
                    let ni = parse_idx(parts.next(), vn.len());
                    let key = (vi, ni.map(|x| x + 1).unwrap_or(0));
                    let idx = match map.get(&key) {
                        Some(&i) => i,
                        None => {
                            let id = m.positions.len() as u32;
                            m.positions.push(v[vi]);
                            m.normals.push(ni.map(|i| vn[i]).unwrap_or([0.0, 0.0, 0.0]));
                            m.colors.push(vc[vi].unwrap_or([0.8, 0.8, 0.8]));
                            if ni.is_some() {
                                any_n = true;
                            }
                            map.insert(key, id);
                            id
                        }
                    };
                    face.push(idx);
                }
                for k in 1..face.len().saturating_sub(1) {
                    m.indices.extend_from_slice(&[face[0], face[k], face[k + 1]]);
                }
            }
            _ => {}
        }
    }
    if !any_n {
        m.normals.clear();
    }
    if !any_c {
        m.colors.clear();
    }
    Ok(m)
}

fn f32le(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn load_stl(path: &Path) -> Result<Mesh> {
    let data = std::fs::read(path)?;
    let binary_size = data.len() >= 84 && {
        let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        84 + n * 50 == data.len()
    };
    let mut tris: Vec<[f32; 3]> = vec![];
    if binary_size || !data.starts_with(b"solid") {
        if data.len() < 84 {
            bail!("Invalid STL file");
        }
        let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        for i in 0..n {
            let o = 84 + i * 50 + 12;
            if o + 36 > data.len() {
                break;
            }
            for k in 0..3 {
                let b = o + k * 12;
                tris.push([f32le(&data, b), f32le(&data, b + 4), f32le(&data, b + 8)]);
            }
        }
    } else {
        let text = String::from_utf8_lossy(&data);
        for line in text.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("vertex") {
                let n = floats(rest.split_whitespace());
                if n.len() >= 3 {
                    tris.push([n[0], n[1], n[2]]);
                }
            }
        }
    }
    let len = tris.len() / 3 * 3;
    tris.truncate(len);
    let mut m = Mesh { indices: (0..len as u32).collect(), positions: tris, ..Default::default() };
    m.weld(0.0);
    Ok(m)
}

struct PlyProp {
    name: String,
    ty: String,
    list: Option<String>,
}

struct PlyElem {
    name: String,
    count: usize,
    props: Vec<PlyProp>,
}

enum Src<'a> {
    Ascii(std::str::SplitAsciiWhitespace<'a>),
    Bin { d: &'a [u8], o: usize, be: bool },
}

impl<'a> Src<'a> {
    fn read(&mut self, ty: &str) -> Result<f64> {
        match self {
            Src::Ascii(it) => it
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| anyhow!("Unexpected end of PLY data")),
            Src::Bin { d, o, be } => {
                let size = match ty {
                    "char" | "int8" | "uchar" | "uint8" => 1,
                    "short" | "int16" | "ushort" | "uint16" => 2,
                    "int" | "int32" | "uint" | "uint32" | "float" | "float32" => 4,
                    "double" | "float64" => 8,
                    other => bail!("Unknown PLY type {other}"),
                };
                if *o + size > d.len() {
                    bail!("Unexpected end of PLY data");
                }
                let mut a = [0u8; 8];
                a[..size].copy_from_slice(&d[*o..*o + size]);
                *o += size;
                if *be {
                    a[..size].reverse();
                }
                Ok(match ty {
                    "char" | "int8" => a[0] as i8 as f64,
                    "uchar" | "uint8" => a[0] as f64,
                    "short" | "int16" => i16::from_le_bytes([a[0], a[1]]) as f64,
                    "ushort" | "uint16" => u16::from_le_bytes([a[0], a[1]]) as f64,
                    "int" | "int32" => i32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f64,
                    "uint" | "uint32" => u32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f64,
                    "float" | "float32" => f32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f64,
                    _ => f64::from_le_bytes(a),
                })
            }
        }
    }
}

fn load_ply(path: &Path) -> Result<Mesh> {
    let data = std::fs::read(path)?;
    let marker = b"end_header";
    let pos = data
        .windows(marker.len())
        .position(|w| w == marker)
        .ok_or_else(|| anyhow!("PLY header not found"))?;
    let mut body_start = pos + marker.len();
    while body_start < data.len() && data[body_start] != b'\n' {
        body_start += 1;
    }
    body_start = (body_start + 1).min(data.len());
    let header = String::from_utf8_lossy(&data[..pos]).into_owned();
    let mut format = "ascii".to_string();
    let mut elems: Vec<PlyElem> = vec![];
    for line in header.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            Some("format") if t.len() > 1 => format = t[1].to_string(),
            Some("element") if t.len() > 2 => elems.push(PlyElem {
                name: t[1].to_string(),
                count: t[2].parse().unwrap_or(0),
                props: vec![],
            }),
            Some("property") if t.len() > 2 => {
                if let Some(e) = elems.last_mut() {
                    if t[1] == "list" && t.len() > 4 {
                        e.props.push(PlyProp { name: t[4].to_string(), ty: t[3].to_string(), list: Some(t[2].to_string()) });
                    } else {
                        e.props.push(PlyProp { name: t[2].to_string(), ty: t[1].to_string(), list: None });
                    }
                }
            }
            _ => {}
        }
    }
    let body = &data[body_start..];
    let ascii = format == "ascii";
    let text = if ascii { String::from_utf8_lossy(body).into_owned() } else { String::new() };
    let mut src = if ascii {
        Src::Ascii(text.split_ascii_whitespace())
    } else {
        Src::Bin { d: body, o: 0, be: format.contains("big") }
    };
    let mut m = Mesh::default();
    for e in &elems {
        let is_v = e.name == "vertex";
        let is_f = e.name == "face";
        let idx = |n: &str| e.props.iter().position(|p| p.name == n);
        let (ix, iy, iz) = (idx("x"), idx("y"), idx("z"));
        let (nx, ny, nz) = (idx("nx"), idx("ny"), idx("nz"));
        let r = idx("red").or_else(|| idx("r"));
        let g = idx("green").or_else(|| idx("g"));
        let b = idx("blue").or_else(|| idx("b"));
        let color_scale = match r {
            Some(i) if e.props[i].ty.starts_with("float") || e.props[i].ty == "double" => 1.0,
            _ => 1.0 / 255.0,
        };
        let mut vals = vec![0f64; e.props.len()];
        for _ in 0..e.count {
            let mut face: Vec<u32> = vec![];
            for (k, p) in e.props.iter().enumerate() {
                if let Some(ct) = &p.list {
                    let n = src.read(ct)?.max(0.0) as usize;
                    let mut items = Vec::with_capacity(n);
                    for _ in 0..n {
                        items.push(src.read(&p.ty)?);
                    }
                    if is_f && (p.name == "vertex_indices" || p.name == "vertex_index") {
                        face = items.iter().map(|&v| v as u32).collect();
                    }
                } else {
                    vals[k] = src.read(&p.ty)?;
                }
            }
            if is_v {
                if let (Some(x), Some(y), Some(z)) = (ix, iy, iz) {
                    m.positions.push([vals[x] as f32, vals[y] as f32, vals[z] as f32]);
                }
                if let (Some(a), Some(bb), Some(c)) = (nx, ny, nz) {
                    m.normals.push([vals[a] as f32, vals[bb] as f32, vals[c] as f32]);
                }
                if let (Some(rr), Some(gg), Some(bb)) = (r, g, b) {
                    m.colors.push([
                        (vals[rr] * color_scale) as f32,
                        (vals[gg] * color_scale) as f32,
                        (vals[bb] * color_scale) as f32,
                    ]);
                }
            }
            if is_f && face.len() >= 3 {
                for k in 1..face.len() - 1 {
                    m.indices.extend_from_slice(&[face[0], face[k], face[k + 1]]);
                }
            }
        }
    }
    Ok(m)
}

fn load_off(path: &Path) -> Result<Mesh> {
    let text = String::from_utf8_lossy(&std::fs::read(path)?).into_owned();
    let mut lines = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty());
    let first = lines.next().ok_or_else(|| anyhow!("Empty OFF file"))?;
    let mut head: Vec<&str> = first.split_whitespace().collect();
    if head.first().map(|h| h.ends_with("OFF")).unwrap_or(false) {
        head.remove(0);
    }
    let counts: Vec<usize> = if head.len() >= 2 {
        head.iter().filter_map(|s| s.parse().ok()).collect()
    } else {
        lines
            .next()
            .ok_or_else(|| anyhow!("OFF counts missing"))?
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    if counts.len() < 2 {
        bail!("Invalid OFF header");
    }
    let mut m = Mesh::default();
    for _ in 0..counts[0] {
        let l = lines.next().ok_or_else(|| anyhow!("OFF vertex list is truncated"))?;
        let v = floats(l.split_whitespace());
        if v.len() < 3 {
            bail!("Invalid OFF vertex");
        }
        m.positions.push([v[0], v[1], v[2]]);
    }
    for _ in 0..counts[1] {
        let Some(l) = lines.next() else { break };
        let v: Vec<u32> = l.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        if v.is_empty() {
            continue;
        }
        let n = v[0] as usize;
        if n < 3 || v.len() < n + 1 {
            continue;
        }
        let f = &v[1..=n];
        for k in 1..n - 1 {
            m.indices.extend_from_slice(&[f[0], f[k], f[k + 1]]);
        }
    }
    Ok(m)
}

type Mat4 = [[f32; 4]; 4];
const IDENTITY: Mat4 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// Column-major multiply: a * b.
fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut r = [[0f32; 4]; 4];
    for c in 0..4 {
        for row in 0..4 {
            r[c][row] = (0..4).map(|k| a[k][row] * b[c][k]).sum();
        }
    }
    r
}

fn xform_point(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

fn xform_dir(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    normalize([
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2],
    ])
}

fn add_gltf_mesh(m: &mut Mesh, mesh: &gltf::Mesh, buffers: &[gltf::buffer::Data], world: &Mat4, any_n: &mut bool) {
    for prim in mesh.primitives() {
        if prim.mode() != gltf::mesh::Mode::Triangles {
            continue;
        }
        let reader = prim.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));
        let Some(pos) = reader.read_positions() else { continue };
        let pos: Vec<[f32; 3]> = pos.collect();
        let n = pos.len();
        let base = m.positions.len() as u32;
        let normals: Option<Vec<[f32; 3]>> = reader.read_normals().map(|i| i.collect());
        let colors: Vec<[f32; 3]> = match reader.read_colors(0) {
            Some(c) => c.into_rgb_f32().collect(),
            None => {
                let f = prim.material().pbr_metallic_roughness().base_color_factor();
                vec![[f[0], f[1], f[2]]; n]
            }
        };
        for (i, p) in pos.iter().enumerate() {
            m.positions.push(xform_point(world, *p));
            match &normals {
                Some(nv) if i < nv.len() => {
                    m.normals.push(xform_dir(world, nv[i]));
                    *any_n = true;
                }
                _ => m.normals.push([0.0, 0.0, 0.0]),
            }
            m.colors.push(colors.get(i).copied().unwrap_or([0.8, 0.8, 0.8]));
        }
        match reader.read_indices() {
            Some(ix) => m.indices.extend(ix.into_u32().map(|x| x + base)),
            None => m.indices.extend((0..n as u32).map(|x| x + base)),
        }
    }
}

fn load_gltf(path: &Path) -> Result<Mesh> {
    let (doc, buffers, _images) = gltf::import(path).map_err(|e| anyhow!("glTF: {e}"))?;
    let mut m = Mesh::default();
    let mut any_n = false;
    match doc.default_scene().or_else(|| doc.scenes().next()) {
        Some(scene) => {
            let mut stack: Vec<(gltf::Node, Mat4)> = scene.nodes().map(|n| (n, IDENTITY)).collect();
            while let Some((node, parent)) = stack.pop() {
                let world = mat_mul(&parent, &node.transform().matrix());
                if let Some(mesh) = node.mesh() {
                    add_gltf_mesh(&mut m, &mesh, &buffers, &world, &mut any_n);
                }
                for c in node.children() {
                    stack.push((c, world));
                }
            }
        }
        None => {
            for mesh in doc.meshes() {
                add_gltf_mesh(&mut m, &mesh, &buffers, &IDENTITY, &mut any_n);
            }
        }
    }
    if !any_n {
        m.normals.clear();
    }
    let colored = m.colors.iter().any(|c| (c[0] - 1.0).abs() > 1e-3 || (c[1] - 1.0).abs() > 1e-3 || (c[2] - 1.0).abs() > 1e-3);
    if !colored {
        m.colors.clear();
    }
    Ok(m)
}

// ---------------------------------------------------------------- saving

pub fn save(m: &Mesh, path: &Path, binary: bool) -> Result<()> {
    let ext = crate::kinds::ext_of(path);
    let bytes = match ext.as_str() {
        "glb" => write_glb(m),
        "obj" => write_obj(m).into_bytes(),
        "stl" => write_stl(m, binary),
        "ply" => write_ply(m, binary),
        "off" => write_off(m).into_bytes(),
        e => bail!("Can't write .{e} (use glb, obj, stl, ply or off)"),
    };
    std::fs::write(path, bytes)?;
    Ok(())
}

fn face_normal(m: &Mesh, t: &[u32]) -> [f32; 3] {
    let p0 = m.positions[t[0] as usize];
    normalize(cross(sub(m.positions[t[1] as usize], p0), sub(m.positions[t[2] as usize], p0)))
}

fn write_obj(m: &Mesh) -> String {
    let mut s = String::from("# Exported by Morph\n");
    let has_c = !m.colors.is_empty();
    for (i, p) in m.positions.iter().enumerate() {
        if has_c {
            let c = m.colors[i];
            let _ = writeln!(s, "v {} {} {} {:.4} {:.4} {:.4}", p[0], p[1], p[2], c[0], c[1], c[2]);
        } else {
            let _ = writeln!(s, "v {} {} {}", p[0], p[1], p[2]);
        }
    }
    let has_n = !m.normals.is_empty();
    for n in &m.normals {
        let _ = writeln!(s, "vn {:.5} {:.5} {:.5}", n[0], n[1], n[2]);
    }
    for t in m.indices.chunks_exact(3) {
        let (a, b, c) = (t[0] + 1, t[1] + 1, t[2] + 1);
        if has_n {
            let _ = writeln!(s, "f {a}//{a} {b}//{b} {c}//{c}");
        } else {
            let _ = writeln!(s, "f {a} {b} {c}");
        }
    }
    s
}

fn write_stl(m: &Mesh, binary: bool) -> Vec<u8> {
    if binary {
        let tris = m.tri_count();
        let mut out = vec![0u8; 80];
        out[..18].copy_from_slice(b"Exported by Morph ");
        out.extend_from_slice(&(tris as u32).to_le_bytes());
        for t in m.indices.chunks_exact(3) {
            let n = face_normal(m, t);
            for v in n {
                out.extend_from_slice(&v.to_le_bytes());
            }
            for &i in t {
                for v in m.positions[i as usize] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            out.extend_from_slice(&[0, 0]);
        }
        out
    } else {
        let mut s = String::from("solid morph\n");
        for t in m.indices.chunks_exact(3) {
            let n = face_normal(m, t);
            let _ = writeln!(s, "  facet normal {} {} {}\n    outer loop", n[0], n[1], n[2]);
            for &i in t {
                let p = m.positions[i as usize];
                let _ = writeln!(s, "      vertex {} {} {}", p[0], p[1], p[2]);
            }
            s.push_str("    endloop\n  endfacet\n");
        }
        s.push_str("endsolid morph\n");
        s.into_bytes()
    }
}

fn write_ply(m: &Mesh, binary: bool) -> Vec<u8> {
    let has_n = !m.normals.is_empty();
    let has_c = !m.colors.is_empty();
    let mut h = String::from("ply\n");
    h.push_str(if binary { "format binary_little_endian 1.0\n" } else { "format ascii 1.0\n" });
    h.push_str("comment Exported by Morph\n");
    let _ = writeln!(h, "element vertex {}", m.positions.len());
    h.push_str("property float x\nproperty float y\nproperty float z\n");
    if has_n {
        h.push_str("property float nx\nproperty float ny\nproperty float nz\n");
    }
    if has_c {
        h.push_str("property uchar red\nproperty uchar green\nproperty uchar blue\n");
    }
    let _ = writeln!(h, "element face {}", m.tri_count());
    h.push_str("property list uchar int vertex_indices\nend_header\n");
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if binary {
        let mut out = h.into_bytes();
        for i in 0..m.positions.len() {
            for v in m.positions[i] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            if has_n {
                for v in m.normals[i] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            if has_c {
                let c = m.colors[i];
                out.extend_from_slice(&[to8(c[0]), to8(c[1]), to8(c[2])]);
            }
        }
        for t in m.indices.chunks_exact(3) {
            out.push(3);
            for &i in t {
                out.extend_from_slice(&(i as i32).to_le_bytes());
            }
        }
        out
    } else {
        for i in 0..m.positions.len() {
            let p = m.positions[i];
            let _ = write!(h, "{} {} {}", p[0], p[1], p[2]);
            if has_n {
                let n = m.normals[i];
                let _ = write!(h, " {} {} {}", n[0], n[1], n[2]);
            }
            if has_c {
                let c = m.colors[i];
                let _ = write!(h, " {} {} {}", to8(c[0]), to8(c[1]), to8(c[2]));
            }
            h.push('\n');
        }
        for t in m.indices.chunks_exact(3) {
            let _ = writeln!(h, "3 {} {} {}", t[0], t[1], t[2]);
        }
        h.into_bytes()
    }
}

fn write_off(m: &Mesh) -> String {
    let mut s = String::from("OFF\n");
    let _ = writeln!(s, "{} {} 0", m.positions.len(), m.tri_count());
    for p in &m.positions {
        let _ = writeln!(s, "{} {} {}", p[0], p[1], p[2]);
    }
    for t in m.indices.chunks_exact(3) {
        let _ = writeln!(s, "3 {} {} {}", t[0], t[1], t[2]);
    }
    s
}

fn write_glb(m: &Mesh) -> Vec<u8> {
    let mut bin: Vec<u8> = vec![];
    let mut views: Vec<Value> = vec![];
    let mut accessors: Vec<Value> = vec![];
    let push_vec3 = |bin: &mut Vec<u8>, data: &[[f32; 3]]| -> (usize, usize) {
        let off = bin.len();
        for v in data {
            for c in v {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        (off, bin.len() - off)
    };
    let count = m.positions.len();
    let (mn, mx) = m.bbox();
    let (o, l) = push_vec3(&mut bin, &m.positions);
    views.push(json!({"buffer": 0, "byteOffset": o, "byteLength": l, "target": 34962}));
    accessors.push(json!({"bufferView": 0, "componentType": 5126, "count": count, "type": "VEC3", "min": mn, "max": mx}));
    let mut attrs = json!({"POSITION": 0});
    if !m.normals.is_empty() {
        let (o, l) = push_vec3(&mut bin, &m.normals);
        views.push(json!({"buffer": 0, "byteOffset": o, "byteLength": l, "target": 34962}));
        accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5126, "count": count, "type": "VEC3"}));
        attrs["NORMAL"] = json!(accessors.len() - 1);
    }
    if !m.colors.is_empty() {
        let (o, l) = push_vec3(&mut bin, &m.colors);
        views.push(json!({"buffer": 0, "byteOffset": o, "byteLength": l, "target": 34962}));
        accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5126, "count": count, "type": "VEC3"}));
        attrs["COLOR_0"] = json!(accessors.len() - 1);
    }
    let off = bin.len();
    for i in &m.indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    views.push(json!({"buffer": 0, "byteOffset": off, "byteLength": bin.len() - off, "target": 34963}));
    accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5125, "count": m.indices.len(), "type": "SCALAR"}));
    let index_accessor = accessors.len() - 1;
    let base_color = if m.colors.is_empty() { [0.8, 0.8, 0.8, 1.0] } else { [1.0, 1.0, 1.0, 1.0] };
    let byte_length = bin.len();
    let doc = json!({
        "asset": {"version": "2.0", "generator": "Morph"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": attrs, "indices": index_accessor, "mode": 4, "material": 0}]}],
        "materials": [{"pbrMetallicRoughness": {"baseColorFactor": base_color, "metallicFactor": 0.0, "roughnessFactor": 0.8}, "doubleSided": true}],
        "buffers": [{"byteLength": byte_length}],
        "bufferViews": views,
        "accessors": accessors
    });
    let mut js = serde_json::to_vec(&doc).unwrap_or_default();
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&js);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}
