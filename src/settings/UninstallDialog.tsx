// SPEC11 §3: the consented, itemized uninstall. Everything the app ever
// created is listed with sizes before the user confirms; keep-data preserves
// settings/history (models always go — they're huge and re-downloadable).

import { useEffect, useState } from "react";
import { api } from "../ipc/api";
import type { UninstallPlanItem } from "../ipc/types";
import { formatBytes } from "../lib/format";

const KIND_LABELS: Record<UninstallPlanItem["kind"], string> = {
  app_data: "Settings & history",
  models: "Downloaded models",
  preferences: "Preferences",
  webkit: "Web storage",
  caches: "Caches",
  saved_state: "Saved window state",
};

export default function UninstallDialog({ onClose }: { onClose(): void }) {
  const [keepData, setKeepData] = useState(false);
  const [plan, setPlan] = useState<UninstallPlanItem[] | null>(null);
  const [removing, setRemoving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .getUninstallPlan(keepData)
      .then((items) => {
        if (!cancelled) setPlan(items);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [keepData]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !removing) onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose, removing]);

  const confirm = async () => {
    setRemoving(true);
    try {
      await api.uninstallApp(keepData);
      // On macOS the app quits moments later; nothing left to do here.
    } catch (e) {
      console.error("uninstall failed:", e);
      setRemoving(false);
    }
  };

  return (
    <div
      className="modal-overlay"
      onMouseDown={(e) => e.target === e.currentTarget && !removing && onClose()}
    >
      <div className="modal update-modal" data-testid="uninstall-dialog">
        <h2>Uninstall Yat Yat</h2>
        {removing ? (
          <p className="update-line">Removing… Yat Yat will quit by itself.</p>
        ) : (
          <>
            <p className="update-line">This removes the app and everything it stored:</p>
            <div className="uninstall-items">
              {(plan ?? []).map((item) => (
                <div className="row uninstall-row" data-testid="uninstall-item" key={item.path}>
                  <span>{KIND_LABELS[item.kind] ?? item.kind}</span>
                  <span className="uninstall-size">{formatBytes(item.bytes)}</span>
                </div>
              ))}
              {plan?.length === 0 && (
                <p className="update-line">Nothing stored yet — only the app itself goes.</p>
              )}
            </div>
            <label className="row" style={{ gap: 8 }}>
              <input
                type="checkbox"
                data-testid="uninstall-keep-data"
                checked={keepData}
                onChange={(e) => setKeepData(e.target.checked)}
              />
              <span>Keep my settings &amp; history (models are still removed)</span>
            </label>
            <p className="update-line" style={{ opacity: 0.75 }}>
              Two things macOS won't let apps remove: the Menu Bar allowance (System
              Settings → Menu Bar) and a Dock pin, if you added one. The app itself
              moves to the Trash.
            </p>
            <div className="actions">
              <button className="btn" data-testid="uninstall-cancel" onClick={onClose}>
                Cancel
              </button>
              <button className="btn danger" data-testid="uninstall-confirm" onClick={confirm}>
                Uninstall
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
