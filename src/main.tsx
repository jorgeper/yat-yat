import React, { Suspense } from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";

// Window routing: the Rust side opens the overlay window with
// index.html?window=overlay; tests may also use #/overlay.
//
// SPEC14 FR-B4: each window lazy-loads only its own app — the always-alive
// overlay webview no longer parses the settings/onboarding/updater code and
// vice versa (Vite splits them into per-window chunks).
const SettingsApp = React.lazy(() => import("./settings/SettingsApp"));
const OverlayApp = React.lazy(() => import("./overlay/OverlayApp"));

const params = new URLSearchParams(window.location.search);
const isOverlay =
  params.get("window") === "overlay" || window.location.hash === "#/overlay";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Suspense fallback={null}>{isOverlay ? <OverlayApp /> : <SettingsApp />}</Suspense>
  </React.StrictMode>,
);
