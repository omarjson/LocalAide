export type Screen = "setup" | "chat" | "memory" | "models" | "settings" | "dev" | "search";

export interface SystemProbe {
  totalRam: number;
  usableRam: number;
  cpuCores: number;
  os: string;
}

export interface Model {
  id: string;
  name: string;
  ram: number;
  size: number;
  type: string;
}

export interface Preset {
  id: string;
  name: string;
  ramGb: number;
  fits: string;
  models: string[];
}

export interface IndexerStats {
  doc_count: number;
  total_bytes: number;
  storage_path: string;
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  ts?: number;
}
