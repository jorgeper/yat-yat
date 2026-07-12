// Generates the README hero shots: the live-dictation pill mid-recording in
// three theme × effect combinations, as transparent retina PNGs with the
// pill's own soft shadow, under docs/screenshots/. Drives the REAL overlay
// (built app + mock IPC shim) through Playwright — run `npm run build`
// first; this script serves dist/ itself.
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "docs/screenshots");
const PORT = 1431;

// Variety on purpose: the default look, a CRT, and something loud.
const SHOTS = [
  {
    file: "pill-indigo-bars.png",
    theme: "indigo",
    effect: "classic-bars",
    text: "Ship the release notes on Wednesday, then call the dentist.",
  },
  {
    file: "pill-phosphor-heartbeat.png",
    theme: "phosphor",
    effect: "heartbeat",
    text: "Remind me that the quarterly numbers look better than expected.",
  },
  {
    file: "pill-vaporwave-mirror.png",
    theme: "vaporwave",
    effect: "mirror-wave",
    text: "Draft an intro paragraph for the design doc before standup.",
  },
];

// Pill is 420×110 (live mode); give it a halo so the shadow breathes.
const HALO = 28;
const WIDTH = 420 + HALO * 2;
const HEIGHT = 110 + HALO * 2;

const server = spawn("npx", ["vite", "preview", "--port", String(PORT), "--strictPort"], {
  cwd: root,
  stdio: "ignore",
});
try {
  await new Promise((resolve, reject) => {
    const t0 = Date.now();
    const poll = async () => {
      try {
        await fetch(`http://localhost:${PORT}/`);
        resolve();
      } catch {
        if (Date.now() - t0 > 15000) reject(new Error("vite preview never came up"));
        else setTimeout(poll, 200);
      }
    };
    poll();
  });

  const browser = await chromium.launch();
  const page = await browser.newPage({
    viewport: { width: WIDTH, height: HEIGHT },
    deviceScaleFactor: 2,
  });
  mkdirSync(outDir, { recursive: true });

  for (const shot of SHOTS) {
    await page.goto(`http://localhost:${PORT}/?window=overlay`);
    await page.waitForSelector("[data-testid=overlay-pill]");
    // Loosen the pill from the viewport edges so its shadow isn't clipped.
    await page.addStyleTag({ content: `.pill { margin: ${HALO}px !important; }` });
    // Drop-in user-theme override — BEFORE show-overlay: the effect engine
    // snapshots theme colors when it starts. (applyTheme's own tag is
    // replaced on show, so the override tag must simply come later in the
    // head, which addStyleTag guarantees for equal-specificity rules.)
    if (shot.css) await page.addStyleTag({ content: shot.css });
    await page.evaluate(
      ([effect, theme]) =>
        window.__mock.emit("show-overlay", { state: "recording", live: true, effect, theme }),
      [shot.effect, shot.theme],
    );
    // Two agreeing passes promote the words to stable (solid) text.
    for (let i = 0; i < 2; i++) {
      await page.evaluate((text) => window.__mock.emit("stream-text", { text }), shot.text);
    }
    // Let the effect build up some voice history, then freeze a lively frame.
    for (let i = 0; i < 30; i++) {
      await page.evaluate(
        (lvl) => window.__mock.emit("mic-level", lvl),
        0.25 + 0.7 * Math.abs(Math.sin(i / 2.5)),
      );
      await page.waitForTimeout(33);
    }
    await page.screenshot({ path: join(outDir, shot.file), omitBackground: true });
    console.log(`wrote docs/screenshots/${shot.file}`);
  }
  await browser.close();
} finally {
  server.kill();
}
