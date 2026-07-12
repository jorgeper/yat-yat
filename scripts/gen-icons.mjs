// Generates the three tray-state icons as PNGs, with no image dependencies
// (raw RGBA -> zlib -> PNG). The APP icons come from the designed assets in
// icon-assets/ (copied into src-tauri/icons); the tray glyph here mirrors
// that design's microphone — same geometry as icon-assets/source/
// yatyat_icon.svg, same -6 degree tilt, face omitted (unreadable at 22 pt).
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function crc32(buf) {
  let table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function encodePng(width, height, rgba) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  // filter 0 per scanline
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    rgba.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([
    sig,
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// Anti-aliased signed-distance drawing: sample 4x4 per pixel.
function draw(width, height, sdf, color) {
  const rgba = Buffer.alloc(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let cover = 0;
      for (let sy = 0; sy < 4; sy++)
        for (let sx = 0; sx < 4; sx++)
          if (sdf(x + (sx + 0.5) / 4, y + (sy + 0.5) / 4) <= 0) cover++;
      const a = (cover / 16) * 255;
      const i = (y * width + x) * 4;
      const [r, g, b] = color(x, y);
      rgba[i] = r;
      rgba[i + 1] = g;
      rgba[i + 2] = b;
      rgba[i + 3] = Math.round(a);
    }
  }
  return rgba;
}

const sdRoundRect = (px, py, cx, cy, w, h, r) => {
  const dx = Math.abs(px - cx) - (w / 2 - r);
  const dy = Math.abs(py - cy) - (h / 2 - r);
  const ox = Math.max(dx, 0);
  const oy = Math.max(dy, 0);
  return Math.hypot(ox, oy) + Math.min(Math.max(dx, dy), 0) - r;
};
const sdCircle = (px, py, cx, cy, r) => Math.hypot(px - cx, py - cy) - r;
const union = (...ds) => Math.min(...ds);
const subtract = (a, b) => Math.max(a, -b);

// Distance from (px,py) to the segment (ax,ay)-(bx,by).
const sdSegment = (px, py, ax, ay, bx, by) => {
  const abx = bx - ax;
  const aby = by - ay;
  const t = Math.max(
    0,
    Math.min(1, ((px - ax) * abx + (py - ay) * aby) / (abx * abx + aby * aby)),
  );
  return Math.hypot(px - (ax + abx * t), py - (ay + aby * t));
};

// The Yat Yat microphone from icon-assets/source/yatyat_icon.svg, face
// omitted, in the SVG's own 240-viewbox coordinates: capsule (95,42 50x88
// r25), U-arc (r47 about 120,106, stroke 11, lead-ins from y=96), stem
// (120,153-170) and base (96-144,176), all tilted -6° like the app icon.
// Sized to fill the 44 px tray box with ~0.5 px margin.
function yatMicSdf(px, py, extraTiltDeg = 0) {
  const K = 0.2872; // source units -> screen px
  const THETA = ((6 + extraTiltDeg) * Math.PI) / 180;
  const cos = Math.cos(THETA);
  const sin = Math.sin(THETA);
  // Screen -> source: un-rotate the -6° tilt about the box centre, then
  // un-scale about the glyph's bounding-box centre.
  const dx = px - 22;
  const dy = py - 22;
  const sx = (dx * cos - dy * sin) / K + 120;
  const sy = (dx * sin + dy * cos) / K + 111.75;

  const capsule = sdRoundRect(sx, sy, 120, 86, 50, 88, 25);
  const arc = sy >= 106 ? Math.abs(Math.hypot(sx - 120, sy - 106) - 47) - 5.5 : 1e9;
  const leadL = sdSegment(sx, sy, 73, 96, 73, 106) - 5.5;
  const leadR = sdSegment(sx, sy, 167, 96, 167, 106) - 5.5;
  const stem = sdSegment(sx, sy, 120, 153, 120, 170) - 5.5;
  const base = sdSegment(sx, sy, 96, 176, 144, 176) - 5.5;
  return K * union(capsule, arc, leadL, leadR, stem, base);
}

// --- Tray icons: 44x44. Idle/processing are monochrome (black + alpha,
// rendered as template); recording is a COLOR icon (SPEC7 FR-T1): mid-gray
// mic (legible on light and dark menu bars) + red dot, rendered non-template.
function trayIcon(name, sdf, color = () => [0, 0, 0]) {
  const S = 44;
  const rgba = draw(S, S, sdf, color);
  mkdirSync(join(root, "src-tauri/resources"), { recursive: true });
  writeFileSync(join(root, `src-tauri/resources/${name}`), encodePng(S, S, rgba));
  console.log(`wrote src-tauri/resources/${name}`);
}

trayIcon("tray-idle.png", (px, py) => yatMicSdf(px, py));
// Recording is template too: macOS 26 wraps the actively-recording app's
// status item in the system's orange privacy capsule and refuses color
// icons there (it substitutes a generic mic) — template alpha renders OUR
// glyph white inside the capsule. The dot keeps the state readable on
// macOS versions without the capsule.
trayIcon("tray-recording.png", (px, py) =>
  union(yatMicSdf(px, py), sdCircle(px, py, 36.5, 7.5, 5.5)),
);
trayIcon("tray-processing.png", (px, py) =>
  union(
    sdCircle(px, py, 11, 22, 3.4),
    sdCircle(px, py, 22, 22, 3.4),
    sdCircle(px, py, 33, 22, 3.4),
  ),
);
// Dance-egg wiggle frames (SPEC12 §2): the recording glyph with the mic
// rocked around its base tilt; the dot stays put. Rust steps 1→4 then
// restores the live state icon.
[-8, 8, -4, 0].forEach((delta, i) => {
  trayIcon(`tray-wiggle-${i + 1}.png`, (px, py) =>
    union(yatMicSdf(px, py, delta), sdCircle(px, py, 36.5, 7.5, 5.5)),
  );
});
