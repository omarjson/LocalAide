import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import { AppRoot } from "./AppRoot";

/**
 * The entry point.
 *
 * Vendored from GOAT (`apps/web/src/main.tsx`, MIT). GOAT started about
 * twenty services and loaded Clerk's sign-in shell when it had a cloud
 * account; LocalAide runs on the user's own computer, so the app starts
 * straight away.
 */
const root = document.getElementById("app");

if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <AppRoot />
    </React.StrictMode>,
  );
  // The splash has done its job once the app is on screen.
  document.getElementById("boot-shell")?.remove();
}
