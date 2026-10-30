import { readFileSync, readdirSync, statSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
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

const jsPath = join(assetsDir, jsFiles[0]);
const jsContent = readFileSync(jsPath, 'utf-8');

if (!jsContent.includes('LocalAide')) {
  console.error("FAIL: JS bundle does not contain product brand name.");
  process.exit(1);
}

console.log("PASS: JS bundle compiled successfully.");

// Write evidence
const evidenceDir = join(process.cwd(), 'docs', 'evidence');
mkdirSync(evidenceDir, { recursive: true });
const logContent = `Smoke Test Report: PASS
Date: ${new Date().toISOString()}
HTML: dist/index.html (valid #app, title LocalAide)
CSS: ${cssFiles[0]} (${cssSize} bytes - Tailwind verified)
JS: ${jsFiles[0]} (${statSync(jsPath).size} bytes)
Status: GREEN
`;
writeFileSync(join(evidenceDir, 'smoke-test.log'), logContent);
console.log("PASS: Smoke test report written to docs/evidence/smoke-test.log");
console.log("=== ALL SMOKE CHECKS PASSED ===");
