// Builds the installer artwork from the LocalAide ghost in public/logo.svg:
//
//   src-tauri/installer/nsis-header.bmp    150x57   Windows installer header
//   src-tauri/installer/nsis-sidebar.bmp   164x314  Windows welcome/finish pages
//   src-tauri/installer/dmg-background.png 660x400  macOS disk image window
//   src-tauri/installer/icon-macos.svg     macOS icon source (Apple grid padding)
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
const out = join(root, "src-tauri", "installer");
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
// sits at (180, 190) and the Applications link at (480, 190) (tauri.conf.json).
const dmg = `<svg xmlns="http://www.w3.org/2000/svg" width="660" height="400" viewBox="0 0 660 400">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#fbf6ec"/>
      <stop offset="1" stop-color="${BONE}"/>
    </linearGradient>
  </defs>
  <rect width="660" height="400" fill="url(#bg)"/>
  ${ghostAt(24, 22, 34, true)}
  <text x="68" y="45" font-family="${FONT}" font-size="17" font-weight="600" fill="${INK}">LocalAide</text>
  <text x="330" y="88" text-anchor="middle" font-family="${FONT}" font-size="15" fill="${INK}" fill-opacity="0.75">Drag LocalAide into Applications to install or update</text>
  <path d="M262 190 H392" stroke="${EMBER}" stroke-width="4" stroke-dasharray="10 9" stroke-linecap="round"/>
  <path d="M388 176 L408 190 L388 204" fill="none" stroke="${EMBER}" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
  <text x="330" y="360" text-anchor="middle" font-family="${FONT}" font-size="12" fill="${INK}" fill-opacity="0.55">Your chats, notes and models stay in ~/.aiassistant when you update.</text>
</svg>`;

// macOS icons sit on an 824px rounded square inside a 1024px canvas.
const macIcon = `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <g transform="translate(100 100) scale(${824 / 512})">
    <rect width="512" height="512" rx="115" fill="${INK}"/>
    ${ghost}
  </g>
</svg>`;

writeFileSync(join(out, "nsis-header.bmp"), toBmp(render(header, 150)));
writeFileSync(join(out, "nsis-sidebar.bmp"), toBmp(render(sidebar, 164)));
writeFileSync(join(out, "dmg-background.png"), render(dmg, 660).asPng());
writeFileSync(join(out, "icon-macos.svg"), macIcon);
console.log(`Wrote installer art to ${out}`);
