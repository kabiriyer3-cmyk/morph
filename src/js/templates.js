// Ready-made workflows. build() returns nodes by index and links as [from, out, to, in].
const N = (type, x, y, values = {}) => ({ type, x, y, values });

export const DEFAULT_TEMPLATE = 'web-images';

export const TEMPLATES = [
  {
    id: 'web-images', icon: 'image', title: 'Photos for the web',
    desc: 'Resize to Full HD and convert to WebP. Usually 70-90% smaller.',
    build: () => ({
      nodes: [
        N('files.load', 40, 80),
        N('image.resize', 400, 80, { mode: 'fit', width: 1920, height: 1080 }),
        N('image.convert', 740, 80, { format: 'webp', quality: 80 }),
        N('files.preview', 1090, 40),
        N('files.save', 1090, 470),
      ],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [2, 0, 3, 0], [2, 0, 4, 0]],
    }),
  },
  {
    id: 'compress-video', icon: 'video', title: 'Compress videos',
    desc: 'Re-encode to H.264 with balanced quality. Great for sharing.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('video.convert', 400, 80, { quality: 26, preset: 'slow' }), N('files.save', 760, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'gif', icon: 'video', title: 'Video to GIF',
    desc: 'Turn clips into small looping GIFs.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('video.gif', 400, 80, { fps: 12, width: 480 }), N('files.preview', 740, 40), N('files.save', 740, 470)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'sort-media', icon: 'files', title: 'Convert everything',
    desc: 'Photos to WebP, videos to MP4, music to MP3, all in one pass.',
    build: () => ({
      nodes: [
        N('files.load', 40, 220),
        N('files.split', 400, 220),
        N('image.convert', 740, 0, { format: 'webp' }),
        N('video.convert', 740, 300, {}),
        N('audio.convert', 740, 760, { format: 'mp3' }),
        N('files.merge', 1100, 300),
        N('files.save', 1420, 300),
      ],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 1, 3, 0], [1, 2, 4, 0], [2, 0, 5, 0], [3, 0, 5, 1], [4, 0, 5, 2], [1, 4, 5, 3], [5, 0, 6, 0]],
    }),
  },
  {
    id: '3d-models', icon: 'mesh', title: '3D for the web',
    desc: 'Halve the triangle count and pack models into GLB.',
    build: () => ({
      nodes: [
        N('files.load', 40, 80),
        N('mesh.optimize', 400, 80, { keep_percent: 50, normals: 'smooth' }),
        N('mesh.convert', 740, 80, { format: 'glb' }),
        N('files.preview', 1080, 40),
        N('files.save', 1080, 470),
      ],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [2, 0, 3, 0], [2, 0, 4, 0]],
    }),
  },
  {
    id: 'backup', icon: 'archive', title: 'Backup with checksums',
    desc: 'Pack files into a ZIP and save a SHA-256 list next to it.',
    build: () => ({
      nodes: [
        N('files.load', 40, 140),
        N('archive.create', 400, 0, { format: 'zip', name: 'backup' }),
        N('files.hash', 400, 300, { algorithm: 'sha256' }),
        N('files.merge', 740, 140),
        N('files.save', 1060, 140),
      ],
      links: [[0, 0, 1, 0], [0, 0, 2, 0], [1, 0, 3, 0], [2, 1, 3, 1], [3, 0, 4, 0]],
    }),
  },
  {
    id: 'photos-pdf', icon: 'doc', title: 'Photos to PDF',
    desc: 'Scans or photos into one A4 PDF document.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('image.pdf', 400, 80, { page: 'A4' }), N('files.save', 740, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'app-icons', icon: 'image', title: 'App icons',
    desc: 'One logo to an .ico file plus PNGs from 16 to 256 px.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('image.icons', 400, 80), N('files.preview', 740, 40), N('files.save', 740, 470)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'data', icon: 'data', title: 'JSON to YAML',
    desc: 'Convert config and data files between formats.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('data.convert', 400, 80, { format: 'yaml' }), N('files.preview', 740, 40), N('files.save', 740, 470)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'dedupe', icon: 'files', title: 'Find duplicates',
    desc: 'Keep unique files and preview the duplicates.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('files.dedupe', 400, 80, { by: 'content' }), N('files.save', 740, 40), N('files.preview', 740, 330)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 1, 3, 0]],
    }),
  },
];
