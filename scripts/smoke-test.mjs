import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join } from 'node:path';

console.log("=== LocalAide Build & Startup Smoke Test ===");

const distDir = join(process.cwd(), 'dist');
const assetsDir = join(distDir, 'assets');

if (!existsSync(distDir)) {
  console.error("FAIL: dist/ directory does not exist. Run 'npm run build' first.");
  process.exit(1);
}

const indexHtmlPath = join(distDir, 'index.html');
if (!existsSync(indexHtmlPath)) {
  console.error("FAIL: dist/index.html is missing.");
  process.exit(1);
}

const indexHtml = readFileSync(indexHtmlPath, 'utf-8');

// 1. Verify mount target and title
if (!indexHtml.includes('<div id="app"></div>')) {
  console.error("FAIL: dist/index.html is missing #app mount container.");
  process.exit(1);
}

if (!indexHtml.includes('<title>LocalAide</title>')) {
  console.error("FAIL: dist/index.html title is not 'LocalAide'.");
  process.exit(1);
}

if (indexHtml.includes('vite.svg')) {
  console.error("FAIL: dist/index.html references dead /vite.svg asset.");
  process.exit(1);
}

console.log("PASS: index.html has valid mount container, product title, and assets.");

// The CSP allows scripts from the app itself only, so an inline script would
// be blocked in the real window. Every <script> needs a src.
const inlineScripts = [...indexHtml.matchAll(/<script\b([^>]*)>/g)].filter(([, attrs]) => !/\bsrc=/.test(attrs));
if (inlineScripts.length > 0) {
  console.error("FAIL: dist/index.html has an inline <script>, which the CSP blocks.");
  process.exit(1);
}

// The theme script must load before the page draws, or a light theme flashes dark.
if (!indexHtml.includes('<script src="/theme-init.js"></script>') || !existsSync(join(distDir, 'theme-init.js'))) {
  console.error("FAIL: dist/theme-init.js is missing or not loaded by index.html.");
  process.exit(1);
}
console.log("PASS: no inline scripts, and the theme loads before the first paint.");

// 2. Verify CSS bundle
const assetFiles = readdirSync(assetsDir);
const cssFiles = assetFiles.filter(f => f.endsWith('.css'));
if (cssFiles.length === 0) {
  console.error("FAIL: No CSS asset found in dist/assets/.");
  process.exit(1);
}

const cssPath = join(assetsDir, cssFiles[0]);
const cssSize = statSync(cssPath).size;
console.log(`CSS bundle: ${cssFiles[0]} (${cssSize} bytes)`);

if (cssSize < 10000) {
  console.error(`FAIL: CSS bundle size is only ${cssSize} bytes. Tailwind utilities missing!`);
  process.exit(1);
}
console.log("PASS: CSS bundle contains compiled Tailwind utilities (>10KB).");

// 3. Verify JS bundle
const jsFiles = assetFiles.filter(f => f.endsWith('.js'));
if (jsFiles.length === 0) {
  console.error("FAIL: No JS asset found in dist/assets/.");
  process.exit(1);
}

// The entry script index.html loads; diagrams and other lazy chunks are
// separate files.
const entry = /<script[^>]+src="\/assets\/([^"]+\.js)"/.exec(indexHtml)?.[1] ?? jsFiles[0];
const jsPath = join(assetsDir, entry);
const jsContent = readFileSync(jsPath, 'utf-8');

if (!jsContent.includes('LocalAide')) {
  console.error("FAIL: JS bundle does not contain product brand name.");
  process.exit(1);
}

console.log(`PASS: JS bundle compiled successfully (${entry}, ${statSync(jsPath).size} bytes).`);
console.log("=== ALL SMOKE CHECKS PASSED ===");
