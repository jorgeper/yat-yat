// Appearance (SPEC6 FR-A5, SPEC16 FR-U): live preview running the REAL
// effect engine on synthetic levels; a cinema-mode gallery whose cards set
// the effect+theme pair in one save; an effect picker; a theme swatch grid;
// and user themes with Reload. One scrollable page — the segmented control
// is scroll-anchor sugar, every section stays mounted.

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../ipc/api";
import type { Settings, UserTheme } from "../ipc/types";
import { usePageVisible } from "../lib/usePageVisible";
import { invalidateAppliedTheme } from "../overlay/applyTheme";
import { EFFECTS } from "../overlay/effects";
import { EffectEngine } from "../overlay/effects/engine";
import { MODES, modeFor } from "../overlay/modes";
import { getBuiltinTheme, THEMES } from "../overlay/themes";
import { secretThemeCss } from "../overlay/secretTheme";

const PREVIEW_STYLE_ID = "nh-preview-theme-style";

function applyPreviewTheme(css: string) {
  let tag = document.getElementById(PREVIEW_STYLE_ID) as HTMLStyleElement | null;
  if (!tag) {
    tag = document.createElement("style");
    tag.id = PREVIEW_STYLE_ID;
    document.head.appendChild(tag);
  }
  // Scope to the preview so the settings chrome keeps its own look.
  tag.textContent = css.replaceAll(".nh-theme", ".nh-preview.nh-theme");
}

export default function AppearanceSection({
  settings,
  save,
}: {
  settings: Settings;
  save: (s: Settings) => Promise<void>;
}) {
  const [userThemes, setUserThemes] = useState<UserTheme[]>([]);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const engineRef = useRef<EffectEngine | null>(null);
  const pageVisible = usePageVisible();
  // SPEC16 FR-U4: hovering a mode card retargets the preview WITHOUT
  // touching settings; leave reverts to the saved pair.
  const [previewPair, setPreviewPair] = useState<{ effect: string; theme: string } | null>(null);
  const previewEffect = previewPair?.effect ?? settings.overlay_effect;
  const previewTheme = previewPair?.theme ?? settings.overlay_theme;
  // Scroll anchors for the segmented control (FR-U1).
  const modesRef = useRef<HTMLDivElement | null>(null);
  const effectsRef = useRef<HTMLDivElement | null>(null);
  const themesRef = useRef<HTMLDivElement | null>(null);

  const loadUserThemes = useCallback(() => {
    // Edited theme files may resolve differently now — let the overlay's
    // memoized applyTheme re-fetch on its next show (SPEC14 FR-R6).
    invalidateAppliedTheme();
    api.listUserThemes().then(setUserThemes).catch(console.error);
  }, []);

  useEffect(loadUserThemes, [loadUserThemes]);

  // Resolve + apply the selected (or hover-previewed) theme to the preview.
  useEffect(() => {
    const id = previewTheme;
    const secret = secretThemeCss(id);
    if (secret) {
      applyPreviewTheme(secret);
    } else if (id.startsWith("user:")) {
      const user = userThemes.find((t) => t.id === id && !t.reason && t.css);
      applyPreviewTheme(user ? user.css : getBuiltinTheme("").css);
    } else {
      applyPreviewTheme(getBuiltinTheme(id).css);
    }
    engineRef.current?.refreshColors();
  }, [previewTheme, userThemes]);

  // The preview engine: real renderers, synthetic voice. Gated on page
  // visibility (SPEC14 FR-S3) — WKWebView pauses rAF for hidden windows but
  // NOT timers, so the 25 Hz feeder kept running if the window hid on this
  // section.
  useEffect(() => {
    if (!canvasRef.current || !pageVisible) return;
    const engine = new EffectEngine(canvasRef.current);
    engineRef.current = engine;
    engine.setEffect(previewEffect);
    engine.start();
    const synth = setInterval(() => {
      const t = performance.now() / 1000;
      const talk = Math.max(0, Math.sin(t * 2.1)) * 0.7 + Math.random() * 0.25;
      engine.feed(talk);
    }, 40);
    return () => {
      clearInterval(synth);
      engineRef.current = null;
      engine.dispose();
    };
  }, [previewEffect, pageVisible]);

  const selectedMode = modeFor(settings.overlay_effect, settings.overlay_theme);

  const scrollTo = (ref: { current: HTMLDivElement | null }) =>
    ref.current?.scrollIntoView({ behavior: "smooth", block: "start" });

  return (
    <>
      {/* SPEC16 FR-U + owner follow-up: the preview and the segmented
          control stay pinned while the galleries scroll beneath them. */}
      <div className="appearance-pinned">
        <div className="section-title">Preview</div>
        <div className="preview-stage" data-testid="appearance-preview">
          <div className="pill nh-preview nh-theme visible preview-pill">
            <span className="rec-dot" />
            <canvas className="fx-canvas" ref={canvasRef} data-testid="preview-canvas" />
            <span className="pill-timer">0:07</span>
          </div>
        </div>

        <div className="appearance-nav" data-testid="appearance-nav">
        <button data-testid="appearance-nav-modes" onClick={() => scrollTo(modesRef)}>
          Modes
        </button>
        <button data-testid="appearance-nav-effects" onClick={() => scrollTo(effectsRef)}>
          Effects
        </button>
        <button data-testid="appearance-nav-themes" onClick={() => scrollTo(themesRef)}>
          Themes
        </button>
        </div>
      </div>

      <div className="section-title appearance-anchor" ref={modesRef}>
        Modes
      </div>
      <div className="card">
        <div className="mode-gallery" data-testid="mode-gallery">
          {MODES.map((m) => {
            const pairTheme = THEMES.find((t) => t.id === m.theme);
            return (
              <button
                key={m.id}
                className={`mode-card ${selectedMode?.id === m.id ? "selected" : ""}`}
                data-testid={`mode-${m.id}`}
                onPointerEnter={() => setPreviewPair({ effect: m.effect, theme: m.theme })}
                onPointerLeave={() => setPreviewPair(null)}
                onClick={() => {
                  setPreviewPair(null);
                  void save({ ...settings, overlay_effect: m.effect, overlay_theme: m.theme });
                }}
              >
                {pairTheme && (
                  <span className="mode-swatch" style={{ background: pairTheme.swatch.bg }}>
                    <i style={{ background: pairTheme.swatch.primary }} />
                    <i style={{ background: pairTheme.swatch.accent }} />
                    <i style={{ background: pairTheme.swatch.text }} />
                  </span>
                )}
                <span className="mode-name">{m.name}</span>
                <span className="mode-tagline">{m.tagline}</span>
              </button>
            );
          })}
        </div>
      </div>

      <div className="section-title appearance-anchor" ref={effectsRef}>
        Effect
      </div>
      <div className="card">
        <div className="picker-grid" data-testid="effect-picker">
          {EFFECTS.map((e) => (
            <button
              key={e.id}
              className={`picker-item ${settings.overlay_effect === e.id ? "selected" : ""}`}
              data-testid={`effect-${e.id}`}
              onClick={() => save({ ...settings, overlay_effect: e.id })}
            >
              {e.name}
            </button>
          ))}
        </div>
      </div>

      <div className="section-title appearance-anchor" ref={themesRef}>
        Theme
      </div>
      <div className="card">
        <div className="picker-grid" data-testid="theme-picker">
          {THEMES.map((t) => (
            <button
              key={t.id}
              className={`picker-item swatch-item ${
                settings.overlay_theme === t.id ? "selected" : ""
              }`}
              data-testid={`theme-${t.id}`}
              onClick={() => save({ ...settings, overlay_theme: t.id })}
            >
              <span className="swatch" style={{ background: t.swatch.bg }}>
                <i style={{ background: t.swatch.primary }} />
                <i style={{ background: t.swatch.accent }} />
                <i style={{ background: t.swatch.text }} />
              </span>
              {t.name}
            </button>
          ))}
        </div>
      </div>

      <div className="section-title">Your themes</div>
      <div className="card">
        {userThemes.length === 0 ? (
          <div className="empty-note">
            Drop a .css file into the themes folder — see THEMES.md in that folder for the
            format.
          </div>
        ) : (
          <div className="picker-grid" data-testid="user-theme-picker">
            {userThemes.map((t) => (
              <button
                key={t.id}
                className={`picker-item ${settings.overlay_theme === t.id ? "selected" : ""}`}
                data-testid={`theme-${t.id}`}
                disabled={t.reason != null}
                title={t.reason ?? ""}
                onClick={() => save({ ...settings, overlay_theme: t.id })}
              >
                {t.name}
                {t.reason && <span className="row-sub"> — {t.reason}</span>}
              </button>
            ))}
          </div>
        )}
        <div className="row">
          <div className="row-sub">Themes folder: Application Support → com.yatyat.app → themes</div>
          <button className="btn" data-testid="reload-themes" onClick={loadUserThemes}>
            Reload themes
          </button>
        </div>
      </div>
    </>
  );
}
