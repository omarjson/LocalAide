// Voice settings shared by the chat box, answers (read aloud) and Settings.
import { api } from "./api/client";
import type { VoiceSettings, VoiceUpdate } from "./api/bindings";

export const voice = $state<{ settings: VoiceSettings | null }>({ settings: null });

export async function loadVoice(): Promise<void> {
  try {
    voice.settings = await api.voiceSettings();
  } catch (e: unknown) {
    console.error("Voice settings could not load", e);
  }
}

export async function updateVoice(update: VoiceUpdate): Promise<VoiceSettings> {
  voice.settings = await api.setVoiceSettings(update);
  return voice.settings;
}

/** "online:groq:whisper-large-v3-turbo:batch" → live or batch, or "local". */
export function deliveryOf(model: string): "local" | "live" | "batch" {
  if (!model.startsWith("online:")) return "local";
  return model.endsWith(":live") ? "live" : "batch";
}
