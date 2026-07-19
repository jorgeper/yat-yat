// SPEC9 §2: the Check for Updates… dialog — a straight port of marky-mark's
// UpdateDialog (same state machine, same testids). Walks checking →
// up-to-date / available → downloading → restart, with honest dismissable
// errors. All network happens behind the updates seam (Rust-side,
// user-initiated). SPEC15 FR-D: while the download runs the dialog cannot be
// dismissed (no silent background install); the ready phase warns macOS
// users about the post-restart Accessibility re-key; restart failures land
// in the error phase instead of a dead button.

import { useEffect, useRef, useState } from "react";
import type { UpdatesApi } from "../ipc/updates";

type Phase =
  | { kind: "checking" }
  | { kind: "none" }
  | { kind: "available"; version: string; notes: string }
  | { kind: "progress"; pct: number }
  | { kind: "ready" }
  | { kind: "error"; message: string };

export default function UpdateDialog({
  currentVersion,
  platform,
  updates,
  onClose,
}: {
  currentVersion: string;
  platform: string;
  updates: UpdatesApi;
  onClose(): void;
}) {
  const [phase, setPhase] = useState<Phase>({ kind: "checking" });
  const disposedRef = useRef(false);
  // SPEC15 FR-D1: the close-lock reads the live phase — the Escape listener
  // is registered once and must not close a mid-download dialog.
  const phaseRef = useRef(phase);
  phaseRef.current = phase;

  useEffect(() => {
    disposedRef.current = false;
    void (async () => {
      try {
        const found = await updates.check();
        if (disposedRef.current) return;
        setPhase(
          found ? { kind: "available", version: found.version, notes: found.notes } : { kind: "none" },
        );
      } catch (err) {
        if (!disposedRef.current)
          setPhase({
            kind: "error",
            message: `Couldn't check for updates: ${err instanceof Error ? err.message : String(err)}`,
          });
      }
    })();
    return () => {
      disposedRef.current = true;
    };
  }, [updates]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && phaseRef.current.kind !== "progress") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const install = () => {
    setPhase({ kind: "progress", pct: 0 });
    void (async () => {
      try {
        await updates.downloadAndInstall((pct) => {
          if (!disposedRef.current) setPhase({ kind: "progress", pct });
        });
        if (!disposedRef.current) setPhase({ kind: "ready" });
      } catch (err) {
        // A failed download must still land in the dismissable error phase —
        // the FR-D1 close-lock applies to the progress phase alone.
        if (!disposedRef.current)
          setPhase({
            kind: "error",
            message: `Update failed: ${err instanceof Error ? err.message : String(err)}`,
          });
      }
    })();
  };

  // SPEC15 FR-D3: a rejecting restart() surfaces instead of a dead button.
  const restart = () => {
    void updates.restart().catch((err) => {
      if (!disposedRef.current)
        setPhase({
          kind: "error",
          message: `Restart failed: ${err instanceof Error ? err.message : String(err)}`,
        });
    });
  };

  return (
    <div
      className="modal-overlay"
      onMouseDown={(e) =>
        e.target === e.currentTarget && phase.kind !== "progress" && onClose()
      }
    >
      <div className="modal update-modal" data-testid="update-dialog">
        <h2>Check for Updates</h2>

        {phase.kind === "checking" && (
          <p className="update-line" data-testid="update-checking">
            Checking for updates…
          </p>
        )}

        {phase.kind === "none" && (
          <p className="update-line" data-testid="update-none">
            You're up to date — Yat Yat v{currentVersion} is the latest version.
          </p>
        )}

        {phase.kind === "available" && (
          <div data-testid="update-available">
            <p className="update-line">
              <strong>Yat Yat v{phase.version}</strong> is available (you have v{currentVersion}).
            </p>
            {phase.notes && <p className="update-notes">{phase.notes}</p>}
            <div className="actions">
              <button className="btn" data-testid="update-later" onClick={onClose}>
                Later
              </button>
              <button className="btn primary" data-testid="update-install" onClick={install}>
                Install Update
              </button>
            </div>
          </div>
        )}

        {phase.kind === "progress" && (
          <div data-testid="update-progress" data-pct={phase.pct}>
            <p className="update-line">Downloading… {phase.pct}%</p>
            <div className="update-bar">
              <div className="update-bar-fill" style={{ width: `${phase.pct}%` }} />
            </div>
          </div>
        )}

        {phase.kind === "ready" && (
          <div>
            <p className="update-line">Update installed. Restart to finish.</p>
            {platform === "macos" && (
              <p className="update-warning" data-testid="update-ax-warning">
                After the restart, macOS will treat the new build as a new app
                and the dictation hotkey stays off until Accessibility is
                re-granted. Yat Yat will guide you through the re-grant on its
                next launch.
              </p>
            )}
            <div className="actions">
              <button className="btn primary" data-testid="update-restart" onClick={restart}>
                Restart Yat Yat
              </button>
            </div>
          </div>
        )}

        {phase.kind === "error" && (
          <div data-testid="update-error">
            <p className="update-line">{phase.message}</p>
            <div className="actions">
              <button className="btn" onClick={onClose}>
                Close
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
