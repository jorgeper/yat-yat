// Settings window (SPEC FR-6): General / Hotkey / Models / Cleanup, plus the
// first-run onboarding wizard (FR-7) when onboarding is incomplete.

import { useCallback, useEffect, useRef, useState } from "react";
import { api, listen } from "../ipc/api";
import type { HistoryEntry, ModelStatus, Settings } from "../ipc/types";
import type { StepId } from "../lib/onboarding";
import { usePageVisible } from "../lib/usePageVisible";
import { updates } from "../ipc/updates";
import { konamiProgress, KONAMI } from "../lib/eggs";
import { SECRET_THEME_ID } from "../overlay/secretTheme";
import AppearanceSection from "./AppearanceSection";
import GeneralSection from "./GeneralSection";
import HotkeySection from "./HotkeySection";
import ModelsSection from "./ModelsSection";
import CleanupSection from "./CleanupSection";
import Onboarding from "./Onboarding";
import UpdateDialog from "./UpdateDialog";

export type SectionId = "general" | "appearance" | "hotkey" | "models" | "cleanup";

const SECTIONS: { id: SectionId; label: string; icon: string }[] = [
  { id: "general", label: "General", icon: "⚙︎" },
  { id: "appearance", label: "Appearance", icon: "◐" },
  { id: "hotkey", label: "Hotkey", icon: "⌘" },
  { id: "models", label: "Models", icon: "▣" },
  { id: "cleanup", label: "Cleanup", icon: "✦" },
];

export default function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  // SPEC14 FR-S2: the window is created lazily, so deep links may arrive as
  // URL query params (an emit into a just-created webview races listener
  // registration) — ?section=models and ?updates=1 mirror the events.
  const [section, setSection] = useState<SectionId>(() => {
    const s = new URLSearchParams(window.location.search).get("section");
    return SECTIONS.some((x) => x.id === s) ? (s as SectionId) : "general";
  });
  const [captureDead, setCaptureDead] = useState(false);
  const pageVisible = usePageVisible();
  const pageVisibleRef = useRef(pageVisible);
  pageVisibleRef.current = pageVisible;
  // Session-only intent (SPEC5 §4.3): Re-run setup walks from the top;
  // launch resume and Fix-in-setup open at the frontier.
  const [wizardFromTop, setWizardFromTop] = useState(false);
  // SPEC9: Check for Updates… (native menu / tray) opens the dialog here.
  const [updateOpen, setUpdateOpen] = useState(
    () => new URLSearchParams(window.location.search).get("updates") === "1",
  );
  const [appVersion, setAppVersion] = useState("");
  // SPEC15 FR-D2: the update dialog's pre-restart Accessibility warning is
  // macOS-specific — platform comes from get_app_info, threaded as a prop.
  const [appPlatform, setAppPlatform] = useState("");
  // SPEC11 §5.2: ↑↑↓↓←→←→BA toggles the secret Yat95 theme.
  const [eggToast, setEggToast] = useState<string | null>(null);

  const refreshModels = useCallback(() => {
    api.listModels().then(setModels).catch(console.error);
    // active_model is backend-owned; refetch so our copy never goes stale.
    api.getSettings().then(setSettings).catch(console.error);
  }, []);

  // History refreshes only reach IPC while the page is visible (SPEC14
  // FR-S3): a history-changed after every dictation was waking the hidden
  // webview; the stale flag defers the fetch to the next show.
  const historyStaleRef = useRef(false);
  const refreshHistory = useCallback(() => {
    if (!pageVisibleRef.current) {
      historyStaleRef.current = true;
      return;
    }
    api.getHistory().then(setHistory).catch(console.error);
  }, []);

  useEffect(() => {
    if (pageVisible && historyStaleRef.current) {
      historyStaleRef.current = false;
      api.getHistory().then(setHistory).catch(console.error);
    }
  }, [pageVisible]);

  useEffect(() => {
    api.getSettings().then(setSettings).catch(console.error);
    refreshModels();
    refreshHistory();
    const unlisteners: Array<() => void> = [];
    (async () => {
      unlisteners.push(
        await listen("history-changed", refreshHistory),
        await listen("model-download-complete", refreshModels),
        await listen("model-deleted", refreshModels),
        await listen<string>("navigate-section", (s) => {
          if (SECTIONS.some((x) => x.id === s)) setSection(s as SectionId);
        }),
        await listen("check-updates", () => setUpdateOpen(true)),
      );
    })();
    api
      .getAppInfo()
      .then((info) => {
        setAppVersion(info.version);
        setAppPlatform(info.platform);
      })
      .catch(console.error);
    return () => unlisteners.forEach((u) => u());
  }, [refreshModels, refreshHistory]);

  // Capture-dead banner (SPEC4 FR-F2.1): poll while the normal sections are
  // showing; the banner clears itself when the hotkey listener recovers.
  // Gated on visibility (SPEC14 FR-S3) — re-show runs an immediate check.
  const onboardingActive = settings ? !settings.onboarding_complete : true;
  useEffect(() => {
    if (onboardingActive) {
      setCaptureDead(false);
      return;
    }
    if (!pageVisible) return;
    let cancelled = false;
    const check = async () => {
      try {
        const info = await api.getAppInfo();
        if (!cancelled) {
          setCaptureDead(info.platform === "macos" && !info.capture_ready);
        }
      } catch {
        /* window closing */
      }
    };
    check();
    const poll = setInterval(check, 2000);
    return () => {
      cancelled = true;
      clearInterval(poll);
    };
  }, [onboardingActive, pageVisible]);

  const save = useCallback(async (next: Settings) => {
    setSettings(next);
    try {
      await api.setSettings(next);
    } catch (e) {
      console.error("saving settings failed:", e);
      const fresh = await api.getSettings();
      setSettings(fresh);
      throw e;
    }
  }, []);

  // SPEC11 §5.2: the Konami code toggles the secret Yat95 theme. Cosmetic
  // only — it just writes overlay_theme like the Appearance picker would.
  const settingsEggRef = useRef<Settings | null>(null);
  settingsEggRef.current = settings;
  const prevThemeRef = useRef<string>("indigo");
  useEffect(() => {
    let progress = 0;
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return; // never eat typing in settings fields
      }
      progress = konamiProgress(progress, e.key);
      if (progress < KONAMI.length) return;
      progress = 0;
      const current = settingsEggRef.current;
      // SPEC12 §3: the Easter-eggs switch silences the code entirely.
      if (!current || !current.easter_eggs) return;
      if (current.overlay_theme === SECRET_THEME_ID) {
        void save({ ...current, overlay_theme: prevThemeRef.current });
        setEggToast("Back to normal. The 90s say hi.");
      } else {
        prevThemeRef.current = current.overlay_theme;
        void save({ ...current, overlay_theme: SECRET_THEME_ID });
        setEggToast("🎉 Secret theme unlocked: Yat95");
      }
      window.setTimeout(() => setEggToast(null), 3000);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // SPEC4 FR-F1: the single recovery entry point. Clears the completion flag
  // (and the given deferrals, so a broken-but-deferred gate is re-checked);
  // SPEC2's gate resolution then opens the wizard at the first unmet gate.
  const startSetup = useCallback(
    async (clearDeferrals: StepId[] = []) => {
      const current = await api.getSettings();
      await save({
        ...current,
        onboarding_complete: false,
        onboarding_skips: current.onboarding_skips.filter(
          (s) => !clearDeferrals.includes(s as StepId),
        ),
      });
    },
    [save],
  );

  // Re-run setup (SPEC5 §4.3): full walkthrough from Welcome.
  const rerunSetup = useCallback(async () => {
    setWizardFromTop(true);
    const current = await api.getSettings();
    await save({ ...current, onboarding_complete: false, onboarding_skips: [] });
  }, [save]);

  if (!settings) return null;

  if (!settings.onboarding_complete) {
    return (
      <Onboarding
        settings={settings}
        models={models}
        refreshModels={refreshModels}
        startAtWelcome={wizardFromTop}
        onDone={async (next) => {
          await save({ ...next, onboarding_complete: true });
          setWizardFromTop(false);
        }}
      />
    );
  }

  return (
    <div className="settings" data-testid="settings-root">
      {updateOpen && (
        <UpdateDialog
          currentVersion={appVersion}
          platform={appPlatform}
          updates={updates}
          onClose={() => setUpdateOpen(false)}
        />
      )}
      {eggToast && (
        <div className="egg-toast" data-testid="egg-toast">
          {eggToast}
        </div>
      )}
      <nav className="settings-nav">
        <div className="app-name">Yat Yat</div>
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            className={`nav-item ${section === s.id ? "active" : ""}`}
            data-testid={`nav-${s.id}`}
            onClick={() => setSection(s.id)}
          >
            <span aria-hidden>{s.icon}</span>
            {s.label}
          </button>
        ))}
      </nav>
      <main className="settings-content" data-testid={`section-${section}`}>
        {captureDead && (
          <div className="banner" data-testid="capture-banner">
            <span>
              The dictation hotkey is inactive — Accessibility permission is missing or was
              re-keyed by an update.
            </span>
            <button
              className="btn primary"
              data-testid="capture-banner-fix"
              onClick={() => startSetup(["accessibility"])}
            >
              Fix in setup
            </button>
          </div>
        )}
        {section === "general" && (
          <GeneralSection
            settings={settings}
            save={save}
            history={history}
            refreshHistory={refreshHistory}
            rerunSetup={rerunSetup}
          />
        )}
        {section === "appearance" && <AppearanceSection settings={settings} save={save} />}
        {section === "hotkey" && (
          <HotkeySection settings={settings} save={save} startSetup={startSetup} />
        )}
        {section === "models" && (
          <ModelsSection settings={settings} models={models} refreshModels={refreshModels} />
        )}
        {section === "cleanup" && <CleanupSection settings={settings} save={save} />}
      </main>
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  testId,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  testId?: string;
}) {
  return (
    <label className="switch">
      <input
        type="checkbox"
        checked={checked}
        data-testid={testId}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="knob" />
    </label>
  );
}
