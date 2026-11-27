// Builds the installer artwork from the LocalAide ghost in public/logo.svg:
//
//   build/installer/nsis-header.bmp    150x57   Windows installer header
//   build/installer/nsis-sidebar.bmp   164x314  Windows welcome/finish pages
//   build/installer/dmg-background.png 660x400  macOS disk image window (2x, 144 dpi)
//   build/installer/icon-macos.svg     macOS icon source (Apple grid padding)
//
// Run it after changing the logo:
//   npx -y -p @resvg/resvg-js node scripts/gen-installer-art.mjs
// (or install @resvg/resvg-js anywhere on NODE_PATH). The outputs are
// committed, so normal builds do not need this script.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Resvg } = require("@resvg/resvg-js");

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "build", "installer");
mkdirSync(out, { recursive: true });

const logo = readFileSync(join(root, "public", "logo.svg"), "utf8");
// Everything inside <svg>, minus the title, description and background tile.
const ghost = logo
  .replace(/^[\s\S]*?<svg[^>]*>/, "")
  .replace(/<\/svg>\s*$/, "")
  .replace(/<title[\s\S]*?<\/title>/, "")
  .replace(/<desc[\s\S]*?<\/desc>/, "")
  .replace(/<rect width="512" height="512"[^>]*\/>/, "");

const INK = "#120d1a";
const PLUM = "#1f1630";
const BONE = "#f3ead8";
const EMBER = "#ff7a1a";
const FONT = "Segoe UI, Noto Sans, Helvetica, Arial, sans-serif";

/** The ghost at (x, y), size px square, optionally on its dark tile. */
const ghostAt = (x, y, size, tile = false) => `
  <g transform="translate(${x} ${y}) scale(${size / 512})">
    ${tile ? `<rect width="512" height="512" rx="112" fill="${INK}"/>` : ""}
    ${ghost}
  </g>`;

const render = (svg, width) =>
  new Resvg(svg, {
    fitTo: { mode: "width", value: width },
    font: { loadSystemFonts: true, defaultFontFamily: "Noto Sans" },
  }).render();

/** 24-bit bottom-up BMP, the format NSIS expects. Alpha is dropped. */
function toBmp(image) {
  const { width, height } = image;
  const rgba = image.pixels;
  const rowSize = Math.ceil((width * 3) / 4) * 4;
  const size = 54 + rowSize * height;
  const buf = Buffer.alloc(size);
  buf.write("BM", 0, "ascii");
  buf.writeUInt32LE(size, 2);
  buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(width, 18);
  buf.writeInt32LE(height, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(rowSize * height, 34);
  buf.writeInt32LE(2835, 38);
  buf.writeInt32LE(2835, 42);
  for (let y = 0; y < height; y++) {
    const row = 54 + (height - 1 - y) * rowSize;
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      buf[row + x * 3] = rgba[i + 2];
      buf[row + x * 3 + 1] = rgba[i + 1];
      buf[row + x * 3 + 2] = rgba[i];
    }
  }
  return buf;
}

// Windows header: white like the installer page, ghost tile on the right.
const header = `<svg xmlns="http://www.w3.org/2000/svg" width="150" height="57" viewBox="0 0 150 57">
  <rect width="150" height="57" fill="#ffffff"/>
  ${ghostAt(99, 6, 45, true)}
</svg>`;

// Windows sidebar: dark, the ghost, the name and a stitched seam.
const sidebar = `<svg xmlns="http://www.w3.org/2000/svg" width="164" height="314" viewBox="0 0 164 314">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${PLUM}"/>
      <stop offset="1" stop-color="${INK}"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.5" cy="0.3" r="0.5">
      <stop offset="0" stop-color="${EMBER}" stop-opacity="0.28"/>
      <stop offset="1" stop-color="${EMBER}" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="164" height="314" fill="url(#bg)"/>
  <rect width="164" height="200" fill="url(#glow)"/>
  ${ghostAt(32, 46, 100)}
  <text x="82" y="186" text-anchor="middle" font-family="${FONT}" font-size="20" font-weight="600" fill="${BONE}">LocalAide</text>
  <text x="82" y="208" text-anchor="middle" font-family="${FONT}" font-size="11" fill="${BONE}" fill-opacity="0.72">Private AI that runs</text>
  <text x="82" y="223" text-anchor="middle" font-family="${FONT}" font-size="11" fill="${BONE}" fill-opacity="0.72">on your computer</text>
  <path d="M24 246 H140" stroke="${EMBER}" stroke-width="2" stroke-dasharray="6 6" stroke-linecap="round"/>
</svg>`;

// macOS DMG: light, so Finder's dark icon labels stay readable. The app icon
// sits at (180, 190) and the Applications link at (480, 190) (tauri.conf.json);
// a soft well marks each spot and a curved arrow joins them. Drawn in points
// (660x400) and rendered at 2x with 144 dpi, so Retina screens get it sharp.
const well = (cx, cy) => `
  <circle cx="${cx}" cy="${cy + 3}" r="78" fill="${INK}" fill-opacity="0.05" filter="url(#blur)"/>
  <circle cx="${cx}" cy="${cy}" r="76" fill="#fffdf8" stroke="#e7dcc8" stroke-width="1"/>
  <circle cx="${cx}" cy="${cy}" r="68" fill="none" stroke="#efe6d6" stroke-width="1"/>`;
const dmg = `<svg xmlns="http://www.w3.org/2000/svg" width="660" height="400" viewBox="0 0 660 400">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#fffaf2"/>
      <stop offset="1" stop-color="#f1e7d4"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.86" cy="0.02" r="0.7">
      <stop offset="0" stop-color="${EMBER}" stop-opacity="0.16"/>
      <stop offset="1" stop-color="${EMBER}" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="arrow" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0" stop-color="${EMBER}" stop-opacity="0.35"/>
      <stop offset="1" stop-color="${EMBER}"/>
    </linearGradient>
    <filter id="blur" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="6"/></filter>
  </defs>
  <rect width="660" height="400" fill="url(#bg)"/>
  <rect width="660" height="400" fill="url(#glow)"/>
  ${ghostAt(28, 24, 30, true)}
  <text x="68" y="38" font-family="${FONT}" font-size="15" font-weight="700" fill="${INK}">LocalAide</text>
  <text x="68" y="53" font-family="${FONT}" font-size="10.5" fill="${INK}" fill-opacity="0.55">Private AI that runs on your Mac</text>
  ${well(180, 190)}
  ${well(480, 190)}
  <path d="M276 178 Q330 140 384 178" fill="none" stroke="url(#arrow)" stroke-width="3.5" stroke-linecap="round"/>
  <path d="M371 166 L386 180 L367 184" fill="none" stroke="${EMBER}" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"/>
  <text x="330" y="318" text-anchor="middle" font-family="${FONT}" font-size="16" font-weight="600" fill="${INK}">Drag LocalAide to Applications</text>
  <text x="330" y="340" text-anchor="middle" font-family="${FONT}" font-size="11.5" fill="${INK}" fill-opacity="0.6">Updating? Drop it on the old copy. Your chats, notes and models stay in ~/.aiassistant.</text>
  <text x="330" y="380" text-anchor="middle" font-family="${FONT}" font-size="10" fill="${INK}" fill-opacity="0.4">Open source · Runs offline · github.com/omarjson/LocalAide</text>
</svg>`;

/** Mark a PNG as 144 dpi, so Finder shows a 2x render at its point size. */
function retinaPng(png) {
  const { crc32 } = require("node:zlib");
  const data = Buffer.alloc(9);
  data.writeUInt32BE(5669, 0); // 144 dpi in pixels per metre
  data.writeUInt32BE(5669, 4);
  data.writeUInt8(1, 8);
  const type = Buffer.from("pHYs", "ascii");
  const chunk = Buffer.alloc(12 + 9);
  chunk.writeUInt32BE(9, 0);
  type.copy(chunk, 4);
  data.copy(chunk, 8);
  chunk.writeUInt32BE(crc32(Buffer.concat([type, data])) >>> 0, 17);
  // After the signature (8 bytes) and IHDR (8 + 13 + 4 bytes).
  const at = 8 + 25;
  return Buffer.concat([png.subarray(0, at), chunk, png.subarray(at)]);
}

// macOS icons sit on an 824px rounded square inside a 1024px canvas.
const macIcon = `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <g transform="translate(100 100) scale(${824 / 512})">
    <rect width="512" height="512" rx="115" fill="${INK}"/>
    ${ghost}
  </g>
</svg>`;

writeFileSync(join(out, "nsis-header.bmp"), toBmp(render(header, 150)));
writeFileSync(join(out, "nsis-sidebar.bmp"), toBmp(render(sidebar, 164)));
writeFileSync(join(out, "dmg-background.png"), retinaPng(render(dmg, 1320).asPng()));
writeFileSync(join(out, "icon-macos.svg"), macIcon);
console.log(`Wrote installer art to ${out}`);
