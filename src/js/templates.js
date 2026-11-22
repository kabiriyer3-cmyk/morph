// Ready-made workflows. build() returns nodes by index and links as [from, out, to, in].
const N = (type, x, y, values = {}) => ({ type, x, y, values });

export const DEFAULT_TEMPLATE = 'web-images';

export const TEMPLATE_GROUPS = ['media', 'docs', 'files'];

export const TEMPLATES = [
  {
    id: 'web-images', group: 'media', icon: 'image', title: 'Photos for the web',
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
    id: 'compress-video', group: 'media', icon: 'video', title: 'Compress videos',
    desc: 'Re-encode to H.264 with balanced quality. Great for sharing.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('video.convert', 400, 80, { quality: 26, preset: 'slow' }), N('files.save', 760, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'gif', group: 'media', icon: 'video', title: 'Video to GIF',
    desc: 'Turn clips into small looping GIFs.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('video.gif', 400, 80, { fps: 12, width: 480 }), N('files.preview', 740, 40), N('files.save', 740, 470)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'sort-media', group: 'files', icon: 'files', title: 'Convert everything',
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
    id: '3d-models', group: 'files', icon: 'mesh', title: '3D for the web',
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
    id: 'backup', group: 'files', icon: 'archive', title: 'Backup with checksums',
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
    id: 'photos-pdf', group: 'docs', icon: 'doc', title: 'Photos to PDF',
    desc: 'Scans or photos into one A4 PDF document.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('image.pdf', 400, 80, { page: 'A4' }), N('files.save', 740, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'app-icons', group: 'media', icon: 'image', title: 'App icons',
    desc: 'One logo to an .ico file plus PNGs from 16 to 256 px.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('image.icons', 400, 80), N('files.preview', 740, 40), N('files.save', 740, 470)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'data', group: 'docs', icon: 'data', title: 'JSON to YAML',
    desc: 'Convert config and data files between formats.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('data.convert', 400, 80, { format: 'yaml' }), N('files.preview', 740, 40), N('files.save', 740, 470)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'dedupe', group: 'files', icon: 'files', title: 'Find duplicates',
    desc: 'Keep unique files and preview the duplicates.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('files.dedupe', 400, 80, { by: 'content' }), N('files.save', 740, 40), N('files.preview', 740, 330)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 1, 3, 0]],
    }),
  },
  {
    id: 'convert-all', group: 'files', icon: 'convert', title: 'Convert anything',
    desc: 'One universal node: files of any type to the format you pick.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('convert.any', 400, 80, { to: 'pdf' }), N('files.preview', 760, 20), N('files.save', 760, 430)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'any-to-mp4', group: 'media', icon: 'video', title: 'Any video to MP4',
    desc: 'MKV, AVI, MOV, WebM and more to universal MP4 (H.264 + AAC).',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('video.convert', 400, 80, { container: 'mp4', video_codec: 'h264', audio_codec: 'aac', quality: 22 }), N('files.save', 780, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'discord', group: 'media', icon: 'compress', title: 'Video for Discord',
    desc: 'Squeezes clips under the 10 MB limit with an automatic bitrate.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('video.compress', 400, 80, { target: 'discord' }), N('files.save', 760, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'shorts', group: 'media', icon: 'aspect', title: 'Vertical 9:16 video',
    desc: 'Landscape clips to Shorts / Reels format with a blurred background.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('video.reframe', 400, 80, { aspect: '9:16', mode: 'blur' }), N('files.save', 760, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'music', group: 'media', icon: 'audio', title: 'Music to MP3 with tags',
    desc: 'FLAC, WAV, M4A and others to 320 kbps MP3 with loudness normalization.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('audio.process', 400, 80, { normalize: true, lufs: -14 }), N('audio.convert', 760, 80, { format: 'mp3', bitrate: 320 }), N('files.save', 1120, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [2, 0, 3, 0]],
    }),
  },
  {
    id: 'podcast', group: 'media', icon: 'eq', title: 'Clean up a podcast',
    desc: 'Noise reduction, compressor, pause removal and loudness normalization.',
    build: () => ({
      nodes: [
        N('files.load', 40, 80),
        N('audio.fx', 400, 80, { denoise: 'medium', compressor: true, highpass: 80 }),
        N('audio.silence', 780, 80, { mode: 'all', min_silence: 0.8 }),
        N('audio.process', 1140, 80, { normalize: true, lufs: -16 }),
        N('files.save', 1500, 80),
      ],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [2, 0, 3, 0], [3, 0, 4, 0]],
    }),
  },
  {
    id: 'social', group: 'media', icon: 'frame', title: 'Images for social media',
    desc: 'Square 1:1 crop, a rounded frame and a caption, saved as JPG.',
    build: () => ({
      nodes: [
        N('files.load', 40, 120),
        N('image.crop', 400, 120, { mode: 'aspect', aspect: '1:1' }),
        N('image.frame', 760, 120, { padding: 48, radius: 28, shadow: 24, background: '#f2f3f7' }),
        N('image.text', 1120, 120, { text: '{name}', position: 'bottom', size: 4 }),
        N('image.convert', 1480, 120, { format: 'jpeg', quality: 88 }),
        N('files.preview', 1840, 40),
        N('files.save', 1840, 470),
      ],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [2, 0, 3, 0], [3, 0, 4, 0], [4, 0, 5, 0], [4, 0, 6, 0]],
    }),
  },
  {
    id: 'clean-photos', group: 'media', icon: 'layers', title: 'Remove similar photos',
    desc: 'Finds duplicates and near identical shots and keeps the largest ones.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('image.similar', 400, 80, { threshold: 6, keep: 'largest' }), N('files.save', 760, 20), N('files.preview', 760, 330)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 1, 3, 0]],
    }),
  },
  {
    id: 'organize-photos', group: 'files', icon: 'folders', title: 'Sort photos by date',
    desc: 'Puts photos into year/month folders by date taken and cleans up names.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('files.organize', 400, 80, { by: 'taken', date_format: 'yyyy/mm' }), N('files.clean_names', 780, 80), N('files.save', 1140, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [2, 0, 3, 0]],
    }),
  },
  {
    id: 'excel-csv', group: 'docs', icon: 'sheet', title: 'Excel to CSV',
    desc: 'Every sheet of a workbook to its own UTF-8 CSV file.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('sheet.convert', 400, 80, { format: 'csv', multi: 'separate', encoding: 'utf-8' }), N('files.preview', 780, 20), N('files.save', 780, 430)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0], [1, 0, 3, 0]],
    }),
  },
  {
    id: 'csv-excel', group: 'docs', icon: 'sheet', title: 'CSV to a polished Excel',
    desc: 'A table with a bold header, filter buttons and fitted column widths.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('sheet.convert', 400, 80, { format: 'xlsx', bold_header: true, freeze_header: true, autofilter: true, autofit: true }), N('files.save', 780, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'pdf-merge', group: 'docs', icon: 'pdf', title: 'Merge PDFs',
    desc: 'Joins all PDFs, sorted by file name, into one document.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('pdf.merge', 400, 80, { order: 'name', name: 'merged' }), N('files.save', 760, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
  {
    id: 'docs-pdf', group: 'docs', icon: 'doc', title: 'Documents to PDF',
    desc: 'Word, Excel, PowerPoint, Markdown and images to PDF in one pass.',
    build: () => ({
      nodes: [N('files.load', 40, 80), N('convert.any', 400, 80, { to: 'pdf' }), N('files.save', 760, 80)],
      links: [[0, 0, 1, 0], [1, 0, 2, 0]],
    }),
  },
];
