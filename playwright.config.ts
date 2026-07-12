import { defineConfig } from "@playwright/test";

// E2E runs against the Vite build served locally; the app detects the absence of
// Tauri and swaps in the mock IPC shim (src/ipc/mock.ts).
export default defineConfig({
  testDir: "tests/e2e",
  // Shared CI runners are slow and oversubscribed: fewer parallel browsers
  // and a longer per-test budget there (assertions unchanged — E9e, the
  // longest flow, was timing out under 5-way parallelism on 3-core runners).
  timeout: process.env.CI ? 60_000 : 30_000,
  workers: process.env.CI ? 2 : undefined,
  retries: 0,
  use: {
    baseURL: "http://localhost:1421",
    headless: true,
    // CI-only flight recorder: a failure leaves a full trace in
    // test-results/ (uploaded as a workflow artifact) instead of a bare
    // assertion message we can't reproduce locally.
    trace: process.env.CI ? "retain-on-failure" : "off",
  },
  webServer: {
    command: "vite preview --port 1421 --strictPort",
    port: 1421,
    reuseExistingServer: false,
  },
});
