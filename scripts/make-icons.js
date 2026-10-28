'use strict';
// Generates the Tauri bundle icons from a single procedural master.
// No dependencies: zlib + fs are built in. Re-runnable; output is byte-identical.
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const crypto = require('crypto');

const OUT_DIR = path.join(__dirname, '..', 'src-tauri', 'icons');

// ---- design constants (master space, 1024x1024) ----
const MASTER = 1024;
const SS = 4;                    // supersampling factor
const BIG = MASTER * SS;         // 4096 render surface
const BG = [0x0a, 0x84, 0xff];   // #0A84FF
const WHITE = [0xff, 0xff, 0xff];
const RADIUS = 224;              // rounded-square corner radius
const DOT_D = 132;               // dot diameter
const DOT_GAP = 132;             // gap between dots == diameter
const ROW_W = 3 * DOT_D + 2 * DOT_GAP; // 660
const ROW_X = (MASTER - ROW_W) / 2;    // 182

// ---------------------------------------------------------------------------
// raster: render BIG x BIG RGBA, then box-downsample to MASTER
// ---------------------------------------------------------------------------
function renderMaster() {
  const C = BIG / 2;
  const HALF = BIG / 2;
  const R = RADIUS * SS;
  const QR = HALF - R;
  const DOT_R = (DOT_D / 2) * SS;
  const STEP = (DOT_D + DOT_GAP) * SS;
  const firstCx = (ROW_X + DOT_D / 2) * SS;
  const dotR2 = DOT_R * DOT_R;

  const big = Buffer.alloc(BIG * BIG * 4); // alpha 0 unless written
  for (let py = 0; py < BIG; py++) {
    const y = py + 0.5;
    const ay = Math.abs(y - C);
    let o = py * BIG * 4;
    for (let px = 0; px < BIG; px++, o += 4) {
      const x = px + 0.5;
      // dots sit on top of the plate
      let hit = false;
      for (let i = 0; i < 3; i++) {
        const dx = x - (firstCx + i * STEP);
        const dy = y - C;
        if (dx * dx + dy * dy <= dotR2) { hit = true; break; }
      }
      if (hit) {
        big[o] = WHITE[0]; big[o + 1] = WHITE[1]; big[o + 2] = WHITE[2]; big[o + 3] = 255;
        continue;
      }
      const ax = Math.abs(x - C);
      if (ax > HALF || ay > HALF) continue; // transparent outside the canvas square
      const qx = ax - QR;
      const qy = ay - QR;
      const inCorner = qx > 0 && qy > 0;
      if (!inCorner || qx * qx + qy * qy <= R * R) {
        big[o] = BG[0]; big[o + 1] = BG[1]; big[o + 2] = BG[2]; big[o + 3] = 255;
      }
    }
  }
  return big;
}

// Premultiplied box downsample; ratio must be an integer.
function downsample(src, s, t) {
  const r = s / t;
  const inv = 1 / (r * r);
  const out = Buffer.alloc(t * t * 4);
  for (let y = 0; y < t; y++) {
    for (let x = 0; x < t; x++) {
      let sr = 0, sg = 0, sb = 0, sa = 0;
      for (let j = 0; j < r; j++) {
        let o = ((y * r + j) * s + x * r) * 4;
        for (let i = 0; i < r; i++, o += 4) {
          const a = src[o + 3];
          if (a === 0) continue;
          sa += a;
          sr += src[o] * a;
          sg += src[o + 1] * a;
          sb += src[o + 2] * a;
        }
      }
      const o = (y * t + x) * 4;
      out[o + 3] = Math.round(sa * inv);
      if (sa > 0) {
        out[o] = Math.round(sr / sa);
        out[o + 1] = Math.round(sg / sa);
        out[o + 2] = Math.round(sb / sa);
      }
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// encoders
// ---------------------------------------------------------------------------
const CRC_TABLE = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}

function pngChunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const t = Buffer.from(type, 'ascii');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([t, data])), 0);
  return Buffer.concat([len, t, data, crc]);
}

const PNG_SIG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function encodePNG(rgba, w, h) {
  const stride = w * 4;
  const raw = Buffer.alloc(h * (stride + 1));
  for (let y = 0; y < h; y++) {
    raw[y * (stride + 1)] = 0; // filter type 0
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;  // bit depth
  ihdr[9] = 6;  // colour type RGBA
  ihdr[10] = 0; // compression
  ihdr[11] = 0; // filter
  ihdr[12] = 0; // interlace
  return Buffer.concat([
    PNG_SIG,
    pngChunk('IHDR', ihdr),
    pngChunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

function encodeICO(entries) {
  const n = entries.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(n, 4);
  const dir = Buffer.alloc(16 * n);
  let offset = 6 + 16 * n;
  entries.forEach((e, i) => {
    const b = i * 16;
    const dim = e.size >= 256 ? 0 : e.size;
    dir[b] = dim;
    dir[b + 1] = dim;
    dir[b + 2] = 0;              // colour count
    dir[b + 3] = 0;              // reserved
    dir.writeUInt16LE(1, b + 4);   // planes
    dir.writeUInt16LE(32, b + 6);  // bit count
    dir.writeUInt32LE(e.png.length, b + 8);
    dir.writeUInt32LE(offset, b + 12);
    offset += e.png.length;
  });
  return Buffer.concat([header, dir, ...entries.map((e) => e.png)]);
}

function encodeICNS(chunks) {
  const parts = chunks.map(([type, png]) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(png.length + 8, 0);
    return Buffer.concat([Buffer.from(type, 'ascii'), len, png]);
  });
  const body = Buffer.concat(parts);
  const head = Buffer.alloc(8);
  head.write('icns', 0, 'ascii');
  head.writeUInt32BE(body.length + 8, 4);
  return Buffer.concat([head, body]);
}

// ---------------------------------------------------------------------------
// verification helpers
// ---------------------------------------------------------------------------
function fail(msg) { throw new Error('verify: ' + msg); }

function checkPNG(buf, label, w, h) {
  if (buf.length < 33) fail(label + ' too short');
  if (!buf.subarray(0, 8).equals(PNG_SIG)) fail(label + ' bad signature');
  if (buf.readUInt32BE(8) !== 13 || buf.toString('ascii', 12, 16) !== 'IHDR') fail(label + ' missing IHDR');
  const gw = buf.readUInt32BE(16);
  const gh = buf.readUInt32BE(20);
  if (gw !== w || gh !== h) fail(`${label} IHDR ${gw}x${gh} != ${w}x${h}`);
  if (buf[24] !== 8 || buf[25] !== 6) fail(label + ' not 8-bit RGBA');
  return `${gw}x${gh}`;
}

function checkICNS(buf, expected) {
  if (buf.length < 16) fail('icns too short');
  if (buf.toString('ascii', 0, 4) !== 'icns') fail('icns magic');
  const total = buf.readUInt32BE(4);
  if (total !== buf.length) fail(`icns length ${total} != file ${buf.length}`);
  let off = 8;
  const seen = [];
  while (off < buf.length) {
    if (off + 8 > buf.length) fail('icns truncated chunk header');
    const type = buf.toString('ascii', off, off + 4);
    const clen = buf.readUInt32BE(off + 4);
    if (clen < 8 || off + clen > buf.length) fail(`icns bad chunk len ${clen} for ${type}`);
    const payload = buf.subarray(off + 8, off + clen);
    checkPNG(payload, 'icns/' + type, expected[type], expected[type]);
    seen.push(type);
    off += clen;
  }
  if (off !== buf.length) fail('icns chunks do not sum to file size');
  const want = Object.keys(expected);
  if (seen.join(',') !== want.join(',')) fail(`icns chunks ${seen} != ${want}`);
}

function checkICO(buf, sizes) {
  if (buf.length < 6) fail('ico too short');
  if (buf.readUInt16LE(0) !== 0 || buf.readUInt16LE(2) !== 1) fail('ico header type');
  const n = buf.readUInt16LE(4);
  if (n !== sizes.length) fail(`ico count ${n} != ${sizes.length}`);
  for (let i = 0; i < n; i++) {
    const b = 6 + i * 16;
    const bytes = buf.readUInt32LE(b + 8);
    const off = buf.readUInt32LE(b + 12);
    if (off + bytes > buf.length) fail(`ico entry ${i} out of range`);
    if (!buf.subarray(off, off + 4).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47]))) {
      fail(`ico entry ${i} offset ${off} lacks PNG signature`);
    }
    checkPNG(buf.subarray(off, off + bytes), `ico[${i}]`, sizes[i], sizes[i]);
  }
}

function digest(buf) {
  return crypto.createHash('sha256').update(buf).digest('hex');
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });

  const big = renderMaster();
  const master = downsample(big, BIG, MASTER);
  const at = (size) => (size === MASTER ? master : downsample(master, MASTER, size));

  const png = {
    32: encodePNG(at(32), 32, 32),
    128: encodePNG(at(128), 128, 128),
    256: encodePNG(at(256), 256, 256),
    512: encodePNG(at(512), 512, 512),
    1024: encodePNG(at(1024), 1024, 1024),
  };

  const files = [
    ['32x32.png', png[32], [32, 32]],
    ['128x128.png', png[128], [128, 128]],
    ['128x128@2x.png', png[256], [256, 256]],
    ['icon.ico', encodeICO([32, 128, 256].map((s) => ({ size: s, png: png[s] })), [32, 128, 256])],
    ['icon.icns', encodeICNS([['ic07', png[128]], ['ic08', png[256]], ['ic09', png[512]], ['ic10', png[1024]]])],
  ];

  // write (skip identical bytes so reruns are truly no-ops on disk)
  const written = [];
  for (const [name, buf] of files) {
    const file = path.join(OUT_DIR, name);
    let same = false;
    try {
      const prev = fs.readFileSync(file);
      same = prev.length === buf.length && prev.equals(buf);
    } catch { /* missing */ }
    if (!same) fs.writeFileSync(file, buf);
    written.push([name, buf, same]);
  }

  // --- verification (checks 1-4), read back from disk ---
  const want = {
    '32x32.png': [32, 32],
    '128x128.png': [128, 128],
    '128x128@2x.png': [256, 256],
  };
  for (const [name, [w, h]] of Object.entries(want)) {
    const buf = fs.readFileSync(path.join(OUT_DIR, name));
    if (!buf.length) fail(name + ' empty');
    checkPNG(buf, name, w, h);
  }
  checkICNS(fs.readFileSync(path.join(OUT_DIR, 'icon.icns')), { ic07: 128, ic08: 256, ic09: 512, ic10: 1024 });
  checkICO(fs.readFileSync(path.join(OUT_DIR, 'icon.ico')), [32, 128, 256]);
  for (const [name] of files) {
    if (fs.statSync(path.join(OUT_DIR, name)).size === 0) fail(name + ' zero bytes');
  }

  // --- report (check 6) ---
  console.log('file                       dims        bytes        sha256           rerun');
  for (const [name, buf, same] of written) {
    let dims = '';
    if (name.endsWith('.png')) dims = `${buf.readUInt32BE(16)}x${buf.readUInt32BE(20)}`;
    else if (name.endsWith('.icns')) dims = '32..1024';
    else dims = '32/128/256';
    console.log(
      `${name.padEnd(26)} ${dims.padEnd(11)} ${String(buf.length).padStart(10)}  ${digest(buf)}  ${same ? 'unchanged' : 'written'}`
    );
  }
  console.log('verify: PNG signatures+IHDR ok, ICNS chunks ok, ICO dir ok, all non-zero');
}

main();
