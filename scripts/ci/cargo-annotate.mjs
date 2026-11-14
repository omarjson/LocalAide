#!/usr/bin/env node
// Runs a cargo command and turns its failures into GitHub annotations.
//
// Job logs need a signed-in user, but annotations are public through the
// check-run API, so a failed CI run stays readable without log access.
//
// Usage: node scripts/ci/cargo-annotate.mjs <label> -- <command> [args...]
// Pass --message-format=json to build commands (check, clippy) so each
// compiler message lands in the annotation for its file.

import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

// GitHub keeps 10 error annotations per step. One slot is saved for stderr.
const MAX_ANNOTATIONS = 9;
const MAX_MESSAGE = 30000;
const STATUS_LINE =
  /^\s*(Compiling|Checking|Downloaded|Downloading|Fresh|Finished|Running|Locking|Adding|Updating|Blocking|Doc-tests|Executable)\b/;

const [label, separator, command, ...args] = process.argv.slice(2);
if (!label || separator !== '--' || !command) {
  console.error('usage: node scripts/ci/cargo-annotate.mjs <label> -- <command> [args...]');
  process.exit(2);
}

const escapeData = (s) => s.replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
const escapeProperty = (s) => escapeData(s).replace(/:/g, '%3A').replace(/,/g, '%2C');
const clip = (s) => (s.length > MAX_MESSAGE ? `${s.slice(0, MAX_MESSAGE)}\n[cut]` : s);

const diagnostics = [];
const plainLines = [];
const stderrLines = [];

function readJson(line) {
  if (!line.startsWith('{')) return null;
  try {
    const value = JSON.parse(line);
    return value && typeof value === 'object' && 'reason' in value ? value : null;
  } catch {
    return null;
  }
}

function onStdoutLine(line) {
  const message = readJson(line);
  if (!message) {
    plainLines.push(line);
    process.stdout.write(`${line}\n`);
    return;
  }
  if (message.reason !== 'compiler-message' || !message.message) return;
  const m = message.message;
  if (typeof m.rendered === 'string') process.stdout.write(m.rendered);
  const spans = Array.isArray(m.spans) ? m.spans : [];
  if ((m.level === 'error' || m.level === 'warning') && spans.length > 0) {
    const span = spans.find((s) => s.is_primary) ?? spans[0];
    diagnostics.push({
      file: String(span.file_name).replace(/\\/g, '/'),
      line: Number(span.line_start) || 1,
      text: typeof m.rendered === 'string' ? m.rendered : String(m.message),
    });
  }
}

function buildAnnotations() {
  const main = [];

  // Compiler messages: one annotation per file with all of its messages.
  const seen = new Set();
  const byFile = new Map();
  for (const d of diagnostics) {
    if (seen.has(d.text)) continue;
    seen.add(d.text);
    const entry = byFile.get(d.file) ?? { line: d.line, texts: [] };
    entry.texts.push(d.text);
    byFile.set(d.file, entry);
  }
  for (const [file, { line, texts }] of byFile) {
    const absolute = file.startsWith('/') || /^[A-Za-z]:\//.test(file);
    const path = absolute ? file : `src-tauri/${file}`;
    const title = `${label}: ${texts.length} in ${file}`;
    main.push(
      `::error file=${escapeProperty(path)},line=${line},title=${escapeProperty(title)}::${escapeData(clip(texts.join('\n')))}`,
    );
  }

  // Test failures: the "---- name stdout ----" blocks of the test harness.
  let name = null;
  let body = [];
  const flush = () => {
    if (name) {
      main.push(`::error title=${escapeProperty(`${label}: ${name}`)}::${escapeData(clip(body.join('\n').trim()))}`);
    }
    name = null;
    body = [];
  };
  for (const line of plainLines) {
    const header = /^---- (.+) stdout ----$/.exec(line);
    if (header) {
      flush();
      name = header[1];
    } else if (line === 'failures:' || line.startsWith('test result:')) {
      flush();
    } else if (name) {
      body.push(line);
    }
  }
  flush();

  const annotations = main.slice(0, MAX_ANNOTATIONS - 1);

  // Tools other than cargo (the Tauri bundler, for one) may report on
  // stdout, so its last lines go in too when nothing else explained the failure.
  if (annotations.length === 0) {
    const outTail = plainLines.filter((l) => l.trim() && !STATUS_LINE.test(l)).slice(-40);
    if (outTail.length > 0) {
      annotations.push(`::error title=${escapeProperty(`${label}: stdout`)}::${escapeData(clip(outTail.join('\n')))}`);
    }
  }

  // The rest of stderr: build scripts, the linker and cargo itself.
  const tail = stderrLines.filter((l) => !STATUS_LINE.test(l)).slice(-80);
  if (tail.length > 0) {
    annotations.push(`::error title=${escapeProperty(`${label}: stderr`)}::${escapeData(clip(tail.join('\n')))}`);
  }
  return annotations;
}

const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'pipe'] });
const outReader = createInterface({ input: child.stdout });
const errReader = createInterface({ input: child.stderr });
outReader.on('line', onStdoutLine);
errReader.on('line', (line) => {
  stderrLines.push(line);
  process.stderr.write(`${line}\n`);
});

const closed = (reader) => new Promise((resolve) => reader.on('close', resolve));
const exited = new Promise((resolve) => {
  child.on('error', (err) => {
    stderrLines.push(`could not start ${command}: ${err.message}`);
    resolve(1);
  });
  child.on('close', (code) => resolve(code ?? 1));
});

const [code] = await Promise.all([exited, closed(outReader), closed(errReader)]);
if (code !== 0) {
  for (const annotation of buildAnnotations()) console.log(annotation);
}
process.exit(code);
