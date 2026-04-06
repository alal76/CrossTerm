import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ToastProvider } from "@/components/Shared/Toast";
import "./index.css";
import "./styles/rtl.css";
import "./i18n";

// Stub Tauri IPC internals when running outside the Tauri webview (e.g. vite dev in browser)
if (!(globalThis as Record<string, unknown>).__TAURI_INTERNALS__) {
  (globalThis as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke: () => Promise.resolve(),
    transformCallback: () => 0,
    convertFileSrc: (s: string) => s,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  };
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ToastProvider>
      <App />
    </ToastProvider>
  </React.StrictMode>
);
