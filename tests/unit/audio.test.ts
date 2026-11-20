import assert from "node:assert/strict";
import { test } from "node:test";
import { bytesToBase64, concat, downsample, encodeWav, level, toInt16 } from "../../src/lib/audio.ts";

test("48 kHz audio becomes 16 kHz with the same shape", () => {
  const input = new Float32Array(48_000).map((_, i) => Math.sin((2 * Math.PI * 440 * i) / 48_000));
  const out = downsample(input, 48_000);
  assert.equal(out.length, 16_000);
  assert.ok(Math.max(...out) > 0.9 && Math.min(...out) < -0.9);
  assert.equal(downsample(new Float32Array(10), 16_000).length, 10);
});

test("samples clip to 16-bit", () => {
  assert.deepEqual([...toInt16(new Float32Array([0, 1, -1, 2, -2]))], [0, 32767, -32768, 32767, -32768]);
});

test("the WAV header is what the backend checks", () => {
  const wav = encodeWav(new Int16Array([1, -1, 2]));
  const v = new DataView(wav.buffer);
  assert.equal(String.fromCharCode(...wav.slice(0, 4)), "RIFF");
  assert.equal(String.fromCharCode(...wav.slice(8, 12)), "WAVE");
  assert.equal(v.getUint16(20, true), 1, "PCM");
  assert.equal(v.getUint16(22, true), 1, "mono");
  assert.equal(v.getUint32(24, true), 16_000);
  assert.equal(v.getUint16(34, true), 16);
  assert.equal(v.getUint32(40, true), 6);
  assert.equal(wav.length, 50);
});

test("base64 matches Node's encoder, also for large recordings", () => {
  const bytes = new Uint8Array(200_000).map((_, i) => (i * 7) % 256);
  assert.equal(bytesToBase64(bytes), Buffer.from(bytes).toString("base64"));
});

test("levels and joining", () => {
  assert.equal(level(new Float32Array(100)), 0);
  assert.ok(level(new Float32Array(100).fill(0.5)) > 0.9);
  assert.deepEqual([...concat([new Int16Array([1, 2]), new Int16Array([3])])], [1, 2, 3]);
});
