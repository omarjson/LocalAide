// Chat attachments: pick files with a dialog, or read files dropped on the
// window. Text becomes text, images become data URLs, everything else is
// refused. Ported from commands/attach.rs.

import { dialog } from "electron";
import fs from "node:fs";
import path from "node:path";
import { AppError } from "../core/errors.mjs";

const MAX_FILES = 8;
const MAX_FILE_BYTES = 25 * 1024 * 1024;
const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
const MAX_TEXT_CHARS = 80_000;

const IMAGE_TYPES = new Map([
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".jpeg", "image/jpeg"],
  [".webp", "image/webp"],
  [".gif", "image/gif"],
]);

/** Reads bytes until they say "text"; binaries are refused, not mangled. */
function looksBinary(bytes) {
  const probe = bytes.subarray(0, Math.min(4096, bytes.length));
  let suspicious = 0;
  for (const b of probe) {
    if (b === 0) return true;
    if (b < 9 || (b > 13 && b < 32)) suspicious += 1;
  }
  return suspicious > probe.length * 0.05;
}

const failed = (name, message) => ({
  name,
  kind: "text",
  text: null,
  data_url: null,
  chars: 0,
  truncated: false,
  error: message,
});

/** Reads one file into a ChatAttachment. */
export function readAttachment(filePath) {
  const name = path.basename(filePath);
  let stat;
  try {
    stat = fs.statSync(filePath);
  } catch {
    return failed(name, "That file is not there any more.");
  }
  if (stat.isDirectory()) return failed(name, "Folders cannot be attached.");

  const ext = path.extname(filePath).toLowerCase();
  const mime = IMAGE_TYPES.get(ext);
  if (mime) {
    if (stat.size > MAX_IMAGE_BYTES) {
      return failed(name, "That image is over the 10 MB limit.");
    }
    try {
      const data = fs.readFileSync(filePath);
      return {
        name,
        kind: "image",
        text: null,
        data_url: `data:${mime};base64,${data.toString("base64")}`,
        chars: 0,
        truncated: false,
        error: null,
      };
    } catch (e) {
      return failed(name, `The image could not be read: ${e}`);
    }
  }

  if (stat.size > MAX_FILE_BYTES) {
    return failed(name, "That file is over the 25 MB limit.");
  }
  let bytes;
  try {
    bytes = fs.readFileSync(filePath);
  } catch (e) {
    return failed(name, `The file could not be read: ${e}`);
  }
  if (looksBinary(bytes)) {
    return failed(name, "That file is a binary, not text or an image.");
  }
  const text = bytes.toString("utf8");
  const truncated = [...text].length > MAX_TEXT_CHARS;
  const kept = truncated ? [...text].slice(0, MAX_TEXT_CHARS).join("") : text;
  return {
    name,
    kind: "text",
    text: kept,
    data_url: null,
    chars: [...kept].length,
    truncated,
    error: null,
  };
}

const pickFiles = async (options) => {
  const { canceled, filePaths } = await dialog.showOpenDialog(options);
  if (canceled) return [];
  return filePaths.slice(0, MAX_FILES).map(readAttachment);
};

/** The system file picker, text and images. */
export function attachPick() {
  return pickFiles({
    title: "Attach files",
    properties: ["openFile", "multiSelections"],
  });
}

/** Files that were just dropped on the window. */
export function attachDropped(paths) {
  if (!Array.isArray(paths)) throw AppError.invalidInput("No files arrived.");
  return paths.slice(0, MAX_FILES).map((p) => readAttachment(String(p)));
}
