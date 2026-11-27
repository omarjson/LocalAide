// The system probe: RAM figures, the CPU brand, the graphics processor and
// the memory budget models may use. Ported from commands/system.rs.

import { spawn } from "node:child_process";
import os from "node:os";
import { memoryPercentForModels, loadConfig } from "../core/config.mjs";
import { setGpuName } from "./engine.mjs";

/** 2 GB the system keeps, whatever the models want. */
const OS_RESERVE_BYTES = 2 * 1024 * 1024 * 1024;

let gpuCache;

function execText(command, args, timeoutMs = 4000) {
  return new Promise((resolve) => {
    try {
      const child = spawn(command, args, { windowsHide: true, timeout: timeoutMs });
      let out = "";
      child.stdout?.on("data", (c) => {
        out += c.toString();
      });
      child.on("close", (code) => resolve(code === 0 ? out : ""));
      child.on("error", () => resolve(""));
    } catch {
      resolve("");
    }
  });
}

/** The graphics processor, read once per run: a subprocess is not free. */
export async function gpuName() {
  if (gpuCache !== undefined) return gpuCache;
  let name = null;
  if (process.platform === "darwin") {
    name = process.arch === "arm64" ? "Apple GPU" : null;
  } else {
    const text = await execText("nvidia-smi", ["--query-gpu=name", "--format=csv,noheader"]);
    const names = [];
    for (const line of text.split("\n")) {
      const trimmed = line.trim();
      if (trimmed && !names.includes(trimmed)) names.push(trimmed);
    }
    name = names.length ? names.join(" + ") : null;
  }
  gpuCache = name;
  setGpuName(name);
  return name;
}

/** RAM, CPU and budget figures the setup wizard and Models screen show. */
export async function systemProbe() {
  const cfg = loadConfig();
  const percent = memoryPercentForModels(cfg);
  const totalRamBytes = os.totalmem();
  const availableRamBytes = os.freemem();
  const totalRamGb = totalRamBytes / 1024 ** 3;
  const availableRamGb = availableRamBytes / 1024 ** 3;
  const cpus = os.cpus();
  const cpuBrand = cpus[0]?.model?.trim() || null;
  const cpuCores = os.availableParallelism?.() ?? cpus.length;

  const usableBytes = Math.max(0, totalRamBytes - OS_RESERVE_BYTES);
  const budgetBytes = Math.floor((usableBytes * percent) / 100);
  const budgetGb = budgetBytes / 1024 ** 3;

  const recommendedPreset =
    totalRamGb >= 28.0 ? "advanced" : totalRamGb >= 14.0 ? "balanced" : totalRamGb >= 7.0 ? "small" : "tiny";

  return {
    total_ram_gb: totalRamGb,
    available_ram_gb: availableRamGb,
    gpu_vendor: await gpuName(),
    cpu_brand: cpuBrand,
    cpu_cores: cpuCores,
    recommended_preset: recommendedPreset,
    budget_gb: budgetGb,
  };
}
