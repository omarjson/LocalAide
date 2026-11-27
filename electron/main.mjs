// Electron main process: one window, one preload bridge, one command table.
// The renderer is the existing Svelte app, unchanged, talking through a
// Tauri-compatible shim so not one component had to be rewritten.

import { app, BrowserWindow, ipcMain, shell, dialog, nativeTheme, Menu, protocol, session, net } from "electron";
import path from "node:path";
import fs from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { registerCommands } from "./ipc.mjs";
import { bus } from "./core/channels.mjs";
import { log } from "./core/log.mjs";
import { ensureAppDir } from "./core/config.mjs";
import { openDb, closeDb } from "./core/db.mjs";
import { shutdownEngine } from "./services/engine.mjs";
import { shutdownMcp } from "./services/mcp.mjs";
import { shutdownEmbeddingEngine } from "./services/embedding.mjs";
import { shutdownShare } from "./services/share.mjs";
import { shutdownRerankEngine } from "./services/rerank.mjs";
import { applyUiScale } from "./services/assist.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
// --dev (or LOCALAIDE_DEV_URL / NODE_ENV=development) loads the Vite dev
// server instead of dist/, for renderer work with the real backend.
const devUrl =
  process.env.LOCALAIDE_DEV_URL ||
  (process.env.NODE_ENV === "development" || process.argv.includes("--dev")
    ? "http://localhost:5173"
    : null);

// The custom scheme must be registered before the app is ready, so
// convertFileSrc can serve model images and HTML frames from disk.
protocol.registerSchemesAsPrivileged([
  { scheme: "localaide-file", privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true } },
]);

// The renderer never talks to the network directly (every provider call
// happens in this process over IPC), so the policy is tight: local scripts
// and styles, images from disk, data URLs and the web, loopback only for
// anything that connects.
const CSP = [
  "default-src 'self' localaide-file:",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: localaide-file: https:",
  "font-src 'self'",
  "connect-src 'self' http://127.0.0.1:* http://localhost:* http://[::1]:*",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

// One copy only: a second instance would share the database and start a
// second engine.
if (!app.requestSingleInstanceLock()) {
  app.quit();
}

let mainWindow = null;

const createWindow = () => {
  mainWindow = new BrowserWindow({
    width: 1024,
    height: 720,
    minWidth: 800,
    minHeight: 560,
    show: false,
    title: "LocalAide",
    backgroundColor: "#141414",
    // The app draws its own chrome (TitleBar.svelte), like it did on Tauri.
    frame: false,
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "hidden",
    trafficLightPosition: { x: 14, y: 14 },
    webPreferences: {
      preload: path.join(here, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
      spellcheck: true,
      webSecurity: true,
    },
  });

  mainWindow.once("ready-to-show", () => mainWindow.show());

  if (devUrl) {
    mainWindow.loadURL(devUrl);
  } else {
    mainWindow.loadFile(path.join(here, "..", "dist", "index.html"));
  }

  // Links open in the user's browser, never inside the app window.
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:/.test(url)) shell.openExternal(url);
    return { action: "deny" };
  });

  const send = (channel, payload) => {
    if (mainWindow && !mainWindow.isDestroyed()) mainWindow.webContents.send(channel, payload);
  };
  bus.on("frame", (frame) => send("localaide:frame", frame));
  bus.on("app-event", (event) => send("localaide:event", event));

  // Window state drives the maximize/restore button in the title bar.
  mainWindow.on("maximize", () => send("localaide:window", { maximized: true }));
  mainWindow.on("unmaximize", () => send("localaide:window", { maximized: false }));

  mainWindow.on("closed", () => {
    mainWindow = null;
  });
};

app.on("second-instance", () => {
  if (mainWindow) {
    if (mainWindow.isMinimized()) mainWindow.restore();
    mainWindow.focus();
  }
});

app.whenReady().then(() => {
  ensureAppDir();
  openDb();
  log("info", "LocalAide started", `electron ${process.versions.electron}`);
  registerCommands(ipcMain);

  // convertFileSrc targets: localaide-file://<encoded absolute path>.
  protocol.handle("localaide-file", (request) => {
    const raw = decodeURIComponent(new URL(request.url).pathname).replace(/^\/([A-Za-z]:)/, "$1");
    const file = path.resolve(raw);
    if (!fs.existsSync(file)) {
      return new Response("Not found", { status: 404 });
    }
    return net.fetch(pathToFileURL(file).toString());
  });

  // The CSP ships with every file:// response (dev server stays open for HMR).
  session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
    if (details.url.startsWith("file://") || details.url.startsWith("localaide-file://")) {
      callback({ responseHeaders: { ...details.responseHeaders, "Content-Security-Policy": [CSP] } });
    } else {
      callback({ responseHeaders: details.responseHeaders });
    }
  });

  // The title bar's window controls and the About panel's version read.
  ipcMain.handle("localaide:window-control", (event, action) => {
    const win = BrowserWindow.fromWebContents(event.sender);
    if (!win) return;
    if (action === "minimize") win.minimize();
    else if (action === "toggle-maximize") {
      if (win.isMaximized()) win.unmaximize();
      else win.maximize();
    } else if (action === "close") win.close();
  });
  ipcMain.handle("localaide:window-maximized", (event) => {
    return BrowserWindow.fromWebContents(event.sender)?.isMaximized() ?? false;
  });
  ipcMain.handle("localaide:app-version", () => app.getVersion());

  // No default menu: the app has its own chrome and its own keybindings.
  Menu.setApplicationMenu(null);
  nativeTheme.themeSource = "dark";
  createWindow();
  applyUiScale();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", () => {
  shutdownEngine();
  shutdownMcp();
  shutdownEmbeddingEngine();
  shutdownShare();
  shutdownRerankEngine();
  closeDb();
});

