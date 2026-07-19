// First-run wizard, SPEC2 + SPEC5: one screen = one gate = one verified fact,
// now with backward navigation. Steps before the frontier render in review
// mode (full instructions, satisfied status); forward movement past an unmet
// gate remains impossible from every control. Auto-advance fires ONLY when
// the displayed step's own gate transitions unmet -> met — never while
// reviewing an already-met step (the no-yank rule).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, listen } from "../ipc/api";
import { permissions } from "../ipc/permissions";
import type { DownloadProgress, ModelStatus, Settings } from "../ipc/types";
import {
  ALL_FACTS,
  factsForStep,
  firstUnmetStep,
  stepMet,
  stepsFor,
  type GateSnapshot,
  type SnapshotFacts,
  type StepId,
} from "../lib/onboarding";
import { usePageVisible } from "../lib/usePageVisible";
import { backTarget, canNavigateTo, isReview, jumpTarget, nextStep } from "../lib/wizardNav";

const POLL_MS = 1000;
const TRAY_POLL_MS = 1500;

const STEP_NAMES: Record<StepId, string> = {
  welcome: "Welcome",
  microphone: "Microphone",
  accessibility: "Accessibility",
  menubar: "Menu bar",
  model: "Model",
  try: "Try it",
};

async function readSnapshot(
  models: ModelStatus[],
  settings: Settings,
  facts: SnapshotFacts,
  prev: GateSnapshot | null,
): Promise<GateSnapshot> {
  const perms = await permissions();
  // One failing check must never sink the whole snapshot (a rejected
  // Promise.all blanked the wizard on Windows): an unverifiable fact reads
  // as unmet and the poll keeps retrying.
  const orFalse = (p: Promise<boolean>) => p.catch(() => false);
  // SPEC14 FR-S4: re-verify only the requested facts; the rest carry over
  // from the previous snapshot (no prev ⇒ read everything). Notably the
  // tray window-server probe runs only when asked for, and platform is
  // fetched exactly once instead of a getAppInfo per tick.
  const full = prev == null;
  const [microphone, accessibility, captureReady, trayVisible] = await Promise.all([
    full || facts.microphone ? orFalse(perms.checkMicrophone()) : prev.microphone,
    full || facts.accessibility ? orFalse(perms.checkAccessibility()) : prev.accessibility,
    full || facts.captureReady ? orFalse(perms.checkCaptureReady()) : prev.captureReady,
    full || facts.trayVisible ? orFalse(perms.checkTrayVisible()) : prev.trayVisible,
  ]);
  const platform = prev?.platform ?? (await api.getAppInfo()).platform;
  const modelReady =
    settings.active_model != null &&
    models.some((m) => m.id === settings.active_model && m.downloaded);
  return { microphone, accessibility, captureReady, trayVisible, modelReady, platform };
}

function StatusChip({ ok, checking, label }: { ok: boolean; checking?: boolean; label?: string }) {
  return (
    <span className={`badge ${ok ? "active-badge" : ""}`} data-testid="gate-status">
      {checking ? "Checking…" : ok ? (label ?? "Granted ✓") : "Not granted"}
    </span>
  );
}

export default function Onboarding({
  settings,
  models,
  refreshModels,
  startAtWelcome = false,
  onDone,
}: {
  settings: Settings;
  models: ModelStatus[];
  refreshModels: () => void;
  /** Re-run setup walks from the top (SPEC5 §4.3). Session-only intent. */
  startAtWelcome?: boolean;
  onDone: (settings: Settings) => Promise<void>;
}) {
  const [snapshot, setSnapshot] = useState<GateSnapshot | null>(null);
  const [step, setStep] = useState<StepId | null>(null);
  const pageVisible = usePageVisible();
  const skipsRef = useRef<string[]>([...settings.onboarding_skips]);
  const modelsRef = useRef(models);
  modelsRef.current = models;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const snapshotRef = useRef<GateSnapshot | null>(null);
  // Gate-transition tracking for the displayed step (auto-advance trigger).
  const metRef = useRef<{ step: StepId | null; met: boolean }>({ step: null, met: false });

  // facts defaults to a full read; polling passes the displayed step's plan
  // (SPEC14 FR-S4) so ticks only re-verify what the step gates on.
  const refreshSnapshot = useCallback(async (facts: SnapshotFacts = ALL_FACTS) => {
    try {
      const snap = await readSnapshot(
        modelsRef.current,
        settingsRef.current,
        facts,
        facts === ALL_FACTS ? null : snapshotRef.current,
      );
      snapshotRef.current = snap;
      setSnapshot(snap);
      return snap;
    } catch (e) {
      console.error("snapshot failed:", e);
      return null;
    }
  }, []);

  // Open at the frontier — or at Welcome when re-running the whole walk.
  // A failed snapshot must never blank the wizard (the exact failure mode
  // that shipped black on Windows): fall back to Welcome and let the poll
  // recover.
  useEffect(() => {
    refreshSnapshot().then((snap) => {
      if (!snap) {
        setStep("welcome");
        return;
      }
      setStep(startAtWelcome ? "welcome" : firstUnmetStep(snap, skipsRef.current));
    });
  }, [refreshSnapshot, startAtWelcome]);

  // Poll while the page is actually visible (SPEC14 FR-S3 — a hidden
  // settings window runs zero timers); each tick re-verifies only the
  // displayed step's facts (FR-S4). Step navigation and re-show do one full
  // refresh so review chips stay truthful.
  useEffect(() => {
    if (!step || step === "try" || !pageVisible) return;
    refreshSnapshot();
    const facts = factsForStep(step);
    const interval = setInterval(
      () => refreshSnapshot(facts),
      step === "menubar" ? TRAY_POLL_MS : POLL_MS,
    );
    return () => clearInterval(interval);
  }, [step, pageVisible, refreshSnapshot]);

  // capture-ready event refreshes immediately (no waiting for the next tick).
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen("capture-ready", () => {
        refreshSnapshot();
      });
    })();
    return () => unlisten?.();
  }, [refreshSnapshot]);

  // Auto-advance ONLY on the displayed step's own unmet -> met transition
  // (SPEC5 §2.4). Reviewing an already-met step never yanks.
  useEffect(() => {
    if (!step || !snapshot) return;
    const met = stepMet(step, snapshot, skipsRef.current);
    const prev = metRef.current;
    metRef.current = { step, met };
    if (prev.step === step && !prev.met && met && step !== "try") {
      setStep(firstUnmetStep(snapshot, skipsRef.current));
    }
  }, [snapshot, step]);

  const navigate = useCallback(
    (target: StepId) => {
      if (!snapshot) return;
      const frontier = firstUnmetStep(snapshot, skipsRef.current);
      if (canNavigateTo(target, frontier, snapshot.platform)) {
        metRef.current = { step: null, met: false };
        setStep(target);
        refreshSnapshot();
      }
    },
    [snapshot, refreshSnapshot],
  );

  const goNext = useCallback(() => {
    if (!step || !snapshot) return;
    const next = nextStep(step, snapshot.platform);
    if (next) {
      metRef.current = { step: null, met: false };
      setStep(next);
      refreshSnapshot();
    }
  }, [step, snapshot, refreshSnapshot]);

  const skip = useCallback(
    async (id: StepId) => {
      if (!skipsRef.current.includes(id)) {
        skipsRef.current = [...skipsRef.current, id];
        // Persist so a relaunch honors the deferral (SPEC2 §3).
        await api.setSettings({ ...settingsRef.current, onboarding_skips: skipsRef.current });
      }
      goNext();
    },
    [goNext],
  );

  const steps = useMemo(
    () => stepsFor(snapshot?.platform ?? "macos"),
    [snapshot?.platform],
  );

  if (!step || !snapshot) return null;

  const frontier = firstUnmetStep(snapshot, skipsRef.current);
  const review = isReview(step, frontier, snapshot.platform);
  const back = backTarget(step, snapshot.platform);
  const jump = review ? jumpTarget(step, frontier, snapshot.platform) : null;
  const met = stepMet(step, snapshot, skipsRef.current);

  const gateProps = { snapshot, met, review, onNext: goNext };

  return (
    <div className="onboarding" data-testid="onboarding" data-step={step}>
      {back && (
        <button className="wizard-back" data-testid="wizard-back" onClick={() => navigate(back)}>
          ‹ Back
        </button>
      )}
      <div className="steps">
        {steps.map((s) => {
          const reachable = canNavigateTo(s, frontier, snapshot.platform);
          return (
            <button
              key={s}
              type="button"
              title={STEP_NAMES[s]}
              data-testid={`dot-${s}`}
              className={`step-dot ${steps.indexOf(s) <= steps.indexOf(step) ? "done" : ""} ${
                reachable ? "clickable" : ""
              }`}
              onClick={() => reachable && navigate(s)}
            />
          );
        })}
      </div>
      {step === "welcome" && <Welcome onNext={goNext} />}
      {step === "microphone" && <Microphone {...gateProps} />}
      {step === "accessibility" && (
        <Accessibility {...gateProps} onSkip={() => skip("accessibility")} />
      )}
      {step === "menubar" && <MenuBar {...gateProps} onSkip={() => skip("menubar")} />}
      {step === "model" && (
        <ModelChoice models={models} refreshModels={refreshModels} onNext={goNext} />
      )}
      {step === "try" && (
        <TryIt
          snapshot={snapshot}
          // Red rows jump straight to the broken gate — deliberately not
          // frontier-bounded (a lost permission can sit past the frontier).
          goTo={(s) => {
            metRef.current = { step: null, met: false };
            setStep(s);
            refreshSnapshot();
          }}
          onDone={() => onDone({ ...settingsRef.current, onboarding_skips: skipsRef.current })}
        />
      )}
      {jump && (
        <button className="btn" data-testid="wizard-jump" onClick={() => navigate(jump)}>
          Jump to where I left off ›
        </button>
      )}
    </div>
  );
}

interface GateStepProps {
  snapshot: GateSnapshot;
  met: boolean;
  review: boolean;
  onNext: () => void;
}

function Welcome({ onNext }: { onNext: () => void }) {
  return (
    <>
      <h1>Welcome to Yat Yat</h1>
      <p>
        Press a hotkey anywhere, talk, press it again — clean text lands right where your cursor
        is. Everything runs on this machine: your voice never leaves it.
      </p>
      <p>Setup checks each thing it needs and only moves on once it's really working.</p>
      <button className="btn primary" data-testid="onboarding-next" onClick={onNext}>
        Set up Yat Yat
      </button>
    </>
  );
}

function Microphone({ snapshot, met, onNext }: GateStepProps) {
  const [requested, setRequested] = useState(false);
  const granted = snapshot.microphone;

  const request = async () => {
    setRequested(true);
    const perms = await permissions();
    await perms.requestMicrophone();
  };

  return (
    <>
      <h1>Microphone</h1>
      <p>Yat Yat needs your microphone to hear you. Audio is processed locally and discarded.</p>
      <StatusChip ok={granted} />
      {!granted && (
        <button className="btn primary" data-testid="mic-request" onClick={request}>
          {requested ? "Waiting for permission…" : "Allow microphone access"}
        </button>
      )}
      {!granted && requested && (
        <p>
          If no prompt appeared, it was denied before: enable Yat Yat in System Settings →
          Privacy &amp; Security → Microphone. This screen updates by itself.
        </p>
      )}
      <button
        className="btn primary"
        data-testid="onboarding-next"
        disabled={!met}
        onClick={onNext}
      >
        Continue
      </button>
    </>
  );
}

function Accessibility({
  snapshot,
  met,
  review,
  onNext,
  onSkip,
}: GateStepProps & { onSkip: () => void }) {
  // Two facts (SPEC2 FR-O2b): the permission AND the armed hotkey listener.
  const reallyMet = snapshot.accessibility && snapshot.captureReady;

  const request = async () => {
    const perms = await permissions();
    await perms.requestAccessibility();
  };

  return (
    <>
      <h1>Accessibility</h1>
      <p>
        macOS requires this for two things Yat Yat does: watching for your hotkey system-wide,
        and pressing ⌘V to paste the finished text. Yat Yat never reads your screen.
      </p>
      <StatusChip
        ok={reallyMet}
        checking={snapshot.accessibility && !snapshot.captureReady}
        label="Granted ✓ — hotkey armed"
      />
      {snapshot.accessibility && !snapshot.captureReady && (
        <p data-testid="arming-note">Permission granted — arming the hotkey listener…</p>
      )}
      <button className="btn primary" data-testid="ax-request" onClick={request}>
        Open System Settings
      </button>
      {!reallyMet && !review && (
        <>
          <p data-testid="stale-hint">
            Already enabled in the list but still showing "Not granted" here? macOS ties the
            permission to each build of the app — remove Yat Yat from the Accessibility list
            (−), then add it back. This screen updates by itself.
          </p>
          <button className="btn" data-testid="ax-defer" onClick={onSkip}>
            Skip — copy to clipboard instead of pasting (no global hotkey)
          </button>
        </>
      )}
      <button
        className="btn primary"
        data-testid="onboarding-next"
        disabled={!met}
        onClick={onNext}
      >
        Continue
      </button>
    </>
  );
}

function MenuBar({
  snapshot,
  met,
  review,
  onNext,
  onSkip,
}: GateStepProps & { onSkip: () => void }) {
  const open = async () => {
    const perms = await permissions();
    await perms.openMenuBarSettings();
  };

  return (
    <>
      <h1>Menu bar</h1>
      <p>
        macOS decides which apps may show a menu-bar icon. Yat Yat's icon is how you reach
        settings, history, and retry — without it the app still works, but it's invisible.
      </p>
      <p>
        In System Settings, find <b>Yat Yat</b> under <b>Menu Bar</b> and turn on{" "}
        <b>Allow in the Menu Bar</b>. This screen updates by itself when the icon appears.
      </p>
      <StatusChip ok={snapshot.trayVisible} label="Visible ✓" />
      <button className="btn primary" data-testid="menubar-open" onClick={open}>
        Open System Settings
      </button>
      {!snapshot.trayVisible && !review && (
        <>
          <button className="btn" data-testid="menubar-confirm" onClick={onSkip}>
            I can see the icon — continue
          </button>
          <button className="btn" data-testid="menubar-defer" onClick={onSkip}>
            I'll do this later
          </button>
        </>
      )}
      <button
        className="btn primary"
        data-testid="onboarding-next"
        disabled={!met}
        onClick={onNext}
      >
        Continue
      </button>
    </>
  );
}

function ModelChoice({
  models,
  refreshModels,
  onNext,
}: {
  models: ModelStatus[];
  refreshModels: () => void;
  onNext: () => void;
}) {
  const recommended = models.find((m) => m.recommended);
  const quickStart = models.find((m) => m.id === "whisper-tiny");
  const active = models.find((m) => m.active && m.downloaded);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<DownloadProgress>("model-download-progress", setProgress);
    })();
    return () => unlisten?.();
  }, []);

  // The verified gate (SPEC2 FR-O2d): downloaded AND active.
  const ready = active != null;

  const pick = async (m: ModelStatus) => {
    setError(null);
    try {
      if (!m.downloaded) {
        setDownloadingId(m.id);
        await api.downloadModel(m.id);
      }
      await api.setActiveModel(m.id);
      refreshModels();
      setDownloadingId(null);
    } catch (e) {
      setError(String(e));
      setDownloadingId(null);
    }
  };

  return (
    <>
      <h1>Pick a model</h1>
      <p>This is the only download Yat Yat ever makes. You can add or switch models later.</p>
      {ready && (
        <p data-testid="active-model-note">
          Active model: <b>{active.display_name}</b> — change it anytime in Settings → Models.
        </p>
      )}
      {downloadingId ? (
        <div style={{ width: 320 }}>
          <div className="progress-track">
            <div
              className="progress-fill"
              style={{ width: `${progress?.percentage ?? 0}%` }}
              data-testid="onboarding-progress"
            />
          </div>
          <p style={{ marginTop: 8 }}>{Math.round(progress?.percentage ?? 0)}%</p>
        </div>
      ) : (
        !ready && (
          <div style={{ display: "flex", gap: 10 }}>
            {recommended && (
              <button
                className="btn primary"
                data-testid="pick-recommended"
                onClick={() => pick(recommended)}
              >
                {recommended.display_name} — recommended
              </button>
            )}
            {quickStart && (
              <button className="btn" data-testid="pick-quickstart" onClick={() => pick(quickStart)}>
                {quickStart.display_name} — quick start (75 MB)
              </button>
            )}
          </div>
        )
      )}
      {error && <p className="download-error">{error}</p>}
      {ready && (
        <button className="btn primary" data-testid="onboarding-next" onClick={onNext}>
          Continue
        </button>
      )}
    </>
  );
}

function TryIt({
  snapshot,
  goTo,
  onDone,
}: {
  snapshot: GateSnapshot;
  goTo: (s: StepId) => void;
  onDone: () => void;
}) {
  const rows: { label: string; ok: boolean; step: StepId }[] = [
    { label: "Microphone", ok: snapshot.microphone, step: "microphone" },
    {
      label: "Hotkey armed",
      ok: snapshot.accessibility && snapshot.captureReady,
      step: "accessibility",
    },
    ...(snapshot.platform === "macos"
      ? [{ label: "Menu-bar icon", ok: snapshot.trayVisible, step: "menubar" as StepId }]
      : []),
    { label: "Model active", ok: snapshot.modelReady, step: "model" },
  ];

  return (
    <>
      <h1>Try it here</h1>
      <div className="card" style={{ width: 340, textAlign: "left" }} data-testid="readiness">
        {rows.map((r) => (
          <div
            key={r.label}
            className={`row ${r.ok ? "" : "history-item"}`}
            data-testid={`ready-${r.step}`}
            onClick={() => !r.ok && goTo(r.step)}
            title={r.ok ? "" : "Click to fix"}
          >
            <span>{r.label}</span>
            <span className={`badge ${r.ok ? "active-badge" : ""}`}>{r.ok ? "Ready" : "Fix"}</span>
          </div>
        ))}
      </div>
      <p>Click into the box, press your hotkey, and say something.</p>
      <textarea placeholder="Dictate into me…" data-testid="try-box" style={{ maxWidth: 420 }} />
      <button className="btn primary" data-testid="onboarding-finish" onClick={onDone}>
        Finish setup
      </button>
    </>
  );
}
