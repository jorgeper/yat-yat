// SPEC14 FR-S3: the settings webview lives (hidden) for the whole app
// lifetime — every poll in it must gate on page visibility or a background
// menu-bar utility keeps a permanent timer heartbeat. WKWebView reports
// visibilitychange when the NSWindow hides/shows; plain browsers (the e2e
// mock) are always visible, so gated code behaves exactly as before there.

import { useEffect, useState } from "react";

export function usePageVisible(): boolean {
  const [visible, setVisible] = useState(
    typeof document === "undefined" || document.visibilityState !== "hidden",
  );
  useEffect(() => {
    const onChange = () => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);
  return visible;
}
