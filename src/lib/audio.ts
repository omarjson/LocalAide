// Microphone audio for voice input: 16 kHz mono 16-bit PCM, the format
// every transcription model here accepts. Pure helpers are exported for tests.

export const TARGET_RATE = 16_000;

/** Average-downsample to 16 kHz (input rates are 44.1 or 48 kHz). */
export function downsample(input: Float32Array, inRate: number, outRate = TARGET_RATE): Float32Array {
  if (inRate === outRate) return input.slice();
  const ratio = inRate / outRate;
  const out = new Float32Array(Math.floor(input.length / ratio));
  for (let i = 0; i < out.length; i++) {
    const start = Math.floor(i * ratio);
    const end = Math.min(input.length, Math.floor((i + 1) * ratio));
    let sum = 0;
    for (let j = start; j < end; j++) sum += input[j];
    out[i] = end > start ? sum / (end - start) : 0;
  }
  return out;
}

export function toInt16(input: Float32Array): Int16Array {
  const out = new Int16Array(input.length);
  for (let i = 0; i < input.length; i++) {
    const s = Math.max(-1, Math.min(1, input[i]));
    out[i] = s < 0 ? Math.round(s * 0x8000) : Math.round(s * 0x7fff);
  }
  return out;
}

/** A WAV file (RIFF, PCM 16-bit, mono) around the samples. */
export function encodeWav(samples: Int16Array, rate = TARGET_RATE): Uint8Array {
  const data = samples.length * 2;
  const buf = new ArrayBuffer(44 + data);
  const v = new DataView(buf);
  const str = (o: number, s: string) => [...s].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  str(0, "RIFF");
  v.setUint32(4, 36 + data, true);
  str(8, "WAVE");
  str(12, "fmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  str(36, "data");
  v.setUint32(40, data, true);
  new Int16Array(buf, 44).set(samples);
  return new Uint8Array(buf);
}

export function bytesToBase64(bytes: Uint8Array): string {
  let bin = "";
  const step = 0x8000;
  for (let i = 0; i < bytes.length; i += step) bin += String.fromCharCode(...bytes.subarray(i, i + step));
  return btoa(bin);
}

/** Loudness of a block, 0 to 1, for the waveform. */
export function level(block: Float32Array): number {
  let sum = 0;
  for (let i = 0; i < block.length; i++) sum += block[i] * block[i];
  return Math.min(1, Math.sqrt(sum / Math.max(1, block.length)) * 4);
}

export interface Recording {
  wav: Uint8Array;
  seconds: number;
}

/** Records from the microphone until `stop()`. */
export class Recorder {
  private ctx: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private node: ScriptProcessorNode | null = null;
  private chunks: Int16Array[] = [];
  private pending: Int16Array[] = [];
  private flushTimer: ReturnType<typeof setInterval> | null = null;

  /**
   * `onLevel` gets a loudness value about 20 times a second. `onPcm`, when
   * given, gets new 16 kHz PCM every 250 ms for live transcription.
   */
  async start(onLevel: (v: number) => void, onPcm?: (pcm: Int16Array) => void): Promise<void> {
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
    });
    const AC = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    this.ctx = new AC();
    const source = this.ctx.createMediaStreamSource(this.stream);
    this.node = this.ctx.createScriptProcessor(2048, 1, 1);
    const rate = this.ctx.sampleRate;
    this.node.onaudioprocess = (e) => {
      const input = e.inputBuffer.getChannelData(0);
      onLevel(level(input));
      const pcm = toInt16(downsample(input, rate));
      this.chunks.push(pcm);
      if (onPcm) this.pending.push(pcm);
    };
    source.connect(this.node);
    // Some engines only run the processor while it is connected to output;
    // a zero gain keeps the speakers silent.
    const mute = this.ctx.createGain();
    mute.gain.value = 0;
    this.node.connect(mute);
    mute.connect(this.ctx.destination);
    if (onPcm) {
      this.flushTimer = setInterval(() => {
        if (this.pending.length === 0) return;
        onPcm(concat(this.pending));
        this.pending = [];
      }, 250);
    }
  }

  async stop(onPcm?: (pcm: Int16Array) => void): Promise<Recording> {
    if (this.flushTimer) clearInterval(this.flushTimer);
    if (onPcm && this.pending.length) onPcm(concat(this.pending));
    this.pending = [];
    this.node?.disconnect();
    this.stream?.getTracks().forEach((t) => t.stop());
    await this.ctx?.close().catch(() => {});
    const all = concat(this.chunks);
    this.chunks = [];
    return { wav: encodeWav(all), seconds: all.length / TARGET_RATE };
  }

  cancel(): void {
    void this.stop();
  }
}

export function concat(parts: Int16Array[]): Int16Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Int16Array(total);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}
