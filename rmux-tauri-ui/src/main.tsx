/**
 * rmux — Tauri v2 UI entry point.
 *
 * This is a minimal shell; the full App component is the central
 * orchestrator (created by another task). This file just mounts
 * the React tree into the DOM so `npm run build` succeeds with the
 * component files on disk.
 */

import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
