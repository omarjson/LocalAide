// Experimental "Read answers aloud": the system voice through the browser's
// speechSynthesis. Speech is made on this computer by the operating system.

/** The answer as plain sentences: no code blocks, links, marks or citations. */
export function speakableText(markdown: string): string {
  return markdown
    .replace(/<think>[\s\S]*?<\/think>/gi, " ")
    .replace(/```[\s\S]*?```/g, " Code block. ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\[\d+\]/g, "")
    .replace(/^\s{0,3}#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*>\s?/gm, "")
    .replace(/(\*\*|__|\*|_|~~)(.+?)\1/g, "$2")
    .replace(/\|/g, " ")
    .replace(/\s+/g, " ")
    .replace(/\s+([.,;:!?])/g, "$1")
    .trim();
}

export function speechAvailable(): boolean {
  return typeof window !== "undefined" && "speechSynthesis" in window && typeof SpeechSynthesisUtterance !== "undefined";
}

/** The system's voices, for the picker in Settings → Voice. */
export function systemVoices(): SpeechSynthesisVoice[] {
  return speechAvailable() ? window.speechSynthesis.getVoices() : [];
}

/** Read text aloud, stopping anything already being read. `onDone` runs when it ends or is stopped. */
export function speak(text: string, onDone: () => void, opts: { voiceName?: string | null; rate?: number } = {}): void {
  if (!speechAvailable()) {
    onDone();
    return;
  }
  window.speechSynthesis.cancel();
  const utterance = new SpeechSynthesisUtterance(speakableText(text));
  const chosen = opts.voiceName ? systemVoices().find((v) => v.name === opts.voiceName) : undefined;
  if (chosen) utterance.voice = chosen;
  utterance.rate = Math.min(2, Math.max(0.5, opts.rate ?? 1));
  // Arabic text is read with an Arabic voice when no voice is chosen.
  utterance.lang = chosen?.lang || (/[\u0600-\u06FF]/.test(text) ? "ar" : document.documentElement.lang || navigator.language || "en");
  utterance.onend = onDone;
  utterance.onerror = onDone;
  window.speechSynthesis.speak(utterance);
}

export function stopSpeaking(): void {
  if (speechAvailable()) window.speechSynthesis.cancel();
}
