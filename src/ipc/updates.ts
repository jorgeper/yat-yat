// SPEC9 §2.3: the updater seam. App code reaches the updater ONLY through
// this module. Desktop implements it with the official updater/process
// plugins (all network Rust-side, user-initiated, responses verified against
// the baked-in public key); the browser branch is a mock driven by the
// window.__yyUpdate test hook — the e2e seam (port of marky-mark's
// __mmUpdate).

import { isTauri } from "./api";

export interface UpdatesApi {
  /** null ⇒ already up to date. Throws on network/manifest/signature errors. */
  check(): Promise<{ version: string; notes: string } | null>;
  downloadAndInstall(onProgress: (pct: number) => void): Promise<void>;
  restart(): Promise<void>;
}

export interface UpdateHook {
  next: { version: string; notes: string } | { error: string } | null;
  progress: number[];
  installed: boolean;
  restarted: boolean;
}

declare global {
  interface Window {
    __yyUpdate?: UpdateHook;
  }
}

function tauriUpdates(): UpdatesApi {
  // The Update object must survive from check() to downloadAndInstall().
  let pending: import("@tauri-apps/plugin-updater").Update | null = null;
  return {
    async check() {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      pending = update;
      if (!update) return null;
      return { version: update.version, notes: update.body ?? "" };
    },
    async downloadAndInstall(onProgress: (pct: number) => void) {
      if (!pending) throw new Error("no update pending — check first");
      let total = 0;
      let got = 0;
      await pending.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        else if (event.event === "Progress") {
          got += event.data.chunkLength;
          if (total > 0) onProgress(Math.min(99, Math.round((got / total) * 100)));
        } else if (event.event === "Finished") onProgress(100);
      });
    },
    async restart() {
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    },
  };
}

function mockUpdates(): UpdatesApi {
  return {
    async check() {
      const hook = (window.__yyUpdate ??= {
        next: null,
        progress: [],
        installed: false,
        restarted: false,
      });
      const next = hook.next;
      if (next && "error" in next) throw new Error(next.error);
      return next;
    },
    async downloadAndInstall(onProgress) {
      const hook = window.__yyUpdate!;
      for (const pct of [12, 48, 87, 100]) {
        hook.progress.push(pct);
        onProgress(pct);
        await new Promise((r) => setTimeout(r, 20));
      }
      hook.installed = true;
    },
    async restart() {
      window.__yyUpdate!.restarted = true;
    },
  };
}

export const updates: UpdatesApi = isTauri() ? tauriUpdates() : mockUpdates();
