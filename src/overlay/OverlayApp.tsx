// The overlay pill (SPEC FR-2 + FR-1.6 + SPEC7 FR-G3). States: recording
// (live waveform + timer), transcribing (thinking shimmer), nothing-heard,
// no-model (Choose Model button), focus-changed (paste-target prompt).
// Driven entirely by events from the Rust side (or the mock).

import { useEffect, useRef, useState } from "react";
import { api, listen } from "../ipc/api";
import { danceMatches, SleepTracker, SLEEP_LEVEL } from "../lib/eggs";
import { EMPTY_LIVE, stabilize, type LiveText } from "../lib/liveText";
import { BAR_COUNT } from "../lib/waveform";
import { applyTheme } from "./applyTheme";
import { advance, filledBars } from "./progress";
import { EffectEngine } from "./effects/engine";
import { DEFAULT_EFFECT, getEffect } from "./effects";
import FocusPrompt from "./FocusPrompt";

type OverlayState =
  | "hidden"
  | "recording"
  | "transcribing"
  | "nothing-heard"
  | "no-model"
  | "focus-changed";

export default function OverlayApp() {
  const [state, setState] = useState<OverlayState>("hidden");
  const [live, setLive] = useState(false);
  const [effect, setEffect] = useState(DEFAULT_EFFECT);
  const [focusApps, setFocusApps] = useState<{ from: string; to: string }>({ from: "", to: "" });
  // Easter eggs (SPEC11 §5, SPEC12) — cosmetic only: nothing here touches
  // recording. The SPEC12 §3 switch gates all of them; the flag refreshes
  // from settings at every recording start.
  const [wiggling, setWiggling] = useState(false);
  const [asleep, setAsleep] = useState(false);
  const eggsRef = useRef(true);
  const danceCountRef = useRef(0);
  const sleepRef = useRef<SleepTracker | null>(null);
  const reducedMotionRef = useRef(
    typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  const [liveText, setLiveText] = useState<LiveText>(EMPTY_LIVE);
  // Estimated transcription progress (SPEC13 FR-O2): forward-only within
  // one transcribing session, reset on each new one, ignored elsewhere.
  const [progress, setProgress] = useState(0);
  const progressRef = useRef(0);
  const stateRef = useRef<OverlayState>("hidden");
  const [elapsed, setElapsed] = useState(0);
  const [overflowing, setOverflowing] = useState(false);
  const startedRef = useRef<number>(0);
  const liveTextRef = useRef<LiveText>(EMPTY_LIVE);
  const liveRegionRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const engineRef = useRef<EffectEngine | null>(null);

  useEffect(() => {
    document.body.classList.add("overlay-body");
    // Apply the persisted theme once at startup; every show refreshes it.
    api
      .getSettings()
      .then((s) => {
        setEffect(s.overlay_effect || DEFAULT_EFFECT);
        eggsRef.current = s.easter_eggs !== false;
        return applyTheme(s.overlay_theme || "");
      })
      .catch(() => applyTheme(""));
    const unlisteners: Array<() => void> = [];
    (async () => {
      unlisteners.push(
        await listen<{
          state: string;
          live?: boolean;
          effect?: string;
          theme?: string;
          from_app?: string;
          to_app?: string;
        }>(
          "show-overlay",
          ({ state, live, effect, theme, from_app, to_app }) => {
            if (state === "recording") {
              startedRef.current = Date.now();
              setElapsed(0);
              // New recording: reset the stabilizer (SPEC3 FR-L4).
              liveTextRef.current = EMPTY_LIVE;
              setLiveText(EMPTY_LIVE);
              // Eggs reset per recording; the switch re-reads too.
              danceCountRef.current = 0;
              setWiggling(false);
              sleepRef.current = new SleepTracker(Date.now);
              setAsleep(false);
              api
                .getSettings()
                .then((s) => {
                  eggsRef.current = s.easter_eggs !== false;
                })
                .catch(() => {});
            }
            if (state === "focus-changed") {
              setFocusApps({ from: from_app ?? "", to: to_app ?? "" });
            }
            if (state === "transcribing") {
              progressRef.current = 0;
              setProgress(0);
            }
            if (effect) setEffect(effect);
            if (theme !== undefined) applyTheme(theme);
            setLive(state === "recording" && live === true);
            stateRef.current = state as OverlayState;
            setState(state as OverlayState);
          },
        ),
        await listen("hide-overlay", () => {
          stateRef.current = "hidden";
          setState("hidden");
        }),
        await listen<number>("transcribe-progress", (fraction) => {
          if (stateRef.current !== "transcribing") return;
          progressRef.current = advance(progressRef.current, fraction);
          setProgress(progressRef.current);
        }),
        await listen<number>("mic-level", (level) => {
          engineRef.current?.feed(level);
          // Sleepy-waveform egg: any loud sample wakes instantly.
          sleepRef.current?.feed(level);
          if (level >= SLEEP_LEVEL) setAsleep(false);
        }),
        await listen<{ text: string }>("stream-text", ({ text }) => {
          liveTextRef.current = stabilize(liveTextRef.current, text);
          setLiveText(liveTextRef.current);
          // "Dance" wiggle (SPEC12 §1–2): detected on the RAW stream text.
          // Cosmetic only; reduced motion and the eggs switch suppress both
          // the pill class and the tray call at the source.
          const matches = danceMatches(text);
          if (matches > danceCountRef.current) {
            danceCountRef.current = matches;
            if (!reducedMotionRef.current && eggsRef.current) {
              setWiggling(true);
              api.wiggleTray().catch(() => {});
            }
          }
        }),
      );
    })();
    return () => unlisteners.forEach((u) => u());
  }, []);

  // Effect engine lifecycle: runs while the recording canvas is mounted,
  // pauses (stop) the moment the overlay leaves the recording state.
  useEffect(() => {
    if (state !== "recording" || !canvasRef.current) return;
    const engine = new EffectEngine(canvasRef.current);
    engineRef.current = engine;
    engine.setEffect(effect);
    engine.start();
    return () => {
      engineRef.current = null;
      engine.dispose();
    };
  }, [state, effect, live]);

  useEffect(() => {
    if (state !== "recording") return;
    const timer = setInterval(
      () => setElapsed(Math.floor((Date.now() - startedRef.current) / 1000)),
      500,
    );
    return () => clearInterval(timer);
  }, [state]);

  // Sleepy-waveform egg (SPEC11 §5.3): poll the tracker while recording.
  // CSS-only — the engine and recording never notice.
  useEffect(() => {
    if (state !== "recording") {
      setAsleep(false);
      return;
    }
    const timer = setInterval(() => {
      setAsleep(eggsRef.current && (sleepRef.current?.isAsleep() ?? false));
    }, 500);
    return () => clearInterval(timer);
  }, [state]);

  // Esc dismisses the focus prompt (SPEC7 FR-G3). In the real app the global
  // Esc hotkey reaches the pipeline through Rust (the panel never has key
  // focus); this listener covers the browser shim the E2E suite drives.
  useEffect(() => {
    if (state !== "focus-changed") return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") api.cancelDictation();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [state]);

  // Fade the top edge only when the text really wraps past the visible area
  // (a single line stays fully solid).
  useEffect(() => {
    const el = liveRegionRef.current;
    if (!el) {
      setOverflowing(false);
      return;
    }
    setOverflowing(el.scrollHeight > el.clientHeight + 2);
  }, [liveText, live, state]);

  const mm = String(Math.floor(elapsed / 60)).padStart(1, "0");
  const ss = String(elapsed % 60).padStart(2, "0");

  const hasLiveText = liveText.stable !== "" || liveText.tentative !== "";

  return (
    <div
      className={`pill nh-theme ${state !== "hidden" ? "visible" : ""} ${live ? "live" : ""} ${
        wiggling ? "pill-wiggle" : ""
      } ${asleep ? "pill-asleep" : ""}`}
      data-testid="overlay-pill"
      data-state={state}
      data-progress={state === "transcribing" ? Math.round(progress * 100) : undefined}
      data-live={live ? "true" : "false"}
      data-effect={getEffect(effect).id}
      onAnimationEnd={(e) => {
        if (e.animationName === "pill-wiggle") setWiggling(false);
      }}
    >
      {state === "recording" && (
        <>
          {live && (
            <div
              className={`live-text ${overflowing ? "masked" : ""}`}
              data-testid="live-text"
              ref={liveRegionRef}
            >
              {hasLiveText ? (
                <p>
                  <span data-testid="live-stable">{liveText.stable}</span>
                  {liveText.stable && liveText.tentative ? " " : ""}
                  <span className="live-tentative" data-testid="live-tentative">
                    {liveText.tentative}
                  </span>
                </p>
              ) : (
                <p className="live-placeholder" data-testid="live-placeholder">
                  Listening…
                </p>
              )}
            </div>
          )}
          <div className="pill-row">
            <span className="rec-dot" data-testid="rec-dot" />
            {asleep && (
              <span className="sleep-zzz" data-testid="sleep-zzz" aria-hidden>
                💤
              </span>
            )}
            <canvas className="fx-canvas" data-testid="waveform" ref={canvasRef} />
            <span className="pill-timer" data-testid="timer">
              {mm}:{ss}
            </span>
            <button
              className="pill-cancel"
              data-testid="cancel-btn"
              title="Cancel (Esc)"
              onClick={() => api.cancelDictation()}
            >
              ✕
            </button>
          </div>
        </>
      )}
      {state === "transcribing" && (
        <div className="wave thinking" data-testid="thinking-wave">
          {Array.from({ length: BAR_COUNT }, (_, i) => (
            <i key={i} className={i < filledBars(progress, BAR_COUNT) ? "fill" : ""} />
          ))}
        </div>
      )}
      {state === "nothing-heard" && <span className="pill-label">Nothing heard</span>}
      {state === "focus-changed" && (
        <FocusPrompt
          fromApp={focusApps.from}
          toApp={focusApps.to}
          onPaste={() => api.resolveFocusPrompt("paste")}
          onCopy={() => api.resolveFocusPrompt("copy")}
        />
      )}
      {state === "no-model" && (
        <>
          <span className="pill-label">Pick a model to start dictating</span>
          <button
            className="btn primary"
            data-testid="choose-model-btn"
            onClick={() => api.openSettings("models")}
          >
            Choose Model
          </button>
        </>
      )}
    </div>
  );
}
