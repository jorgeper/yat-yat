# Blockers / honest caveats

0. **macOS 26 has no public API for "is my menu-bar item visible?"** Control
   Center owns the per-app allowance and hosts visible third-party items, so
   even window-server enumeration misattributes them. The onboarding gate
   uses the best available signal (the app's own NSStatusBarWindow frame +
   occlusion, in-process) and additionally offers an explicit user
   attestation ("I can see the icon — continue") in case the heuristic and
   reality disagree on some future macOS build.

1. **Parakeet V3 performance numbers are unmeasured.** SPEC §7 sets a
   stop-to-paste target for Parakeet v3, but the goal constraint permits only
   the cached Whisper-tiny download in `scripts/validate-stt` — the ~460 MB
   Parakeet archive cannot be fetched during this run. What was tried: the
   Whisper-tiny path measures the identical pipeline (same audio, cleanup,
   delivery code; different engine call), and its numbers are recorded in
   ARCHITECTURE.md. Measuring Parakeet is a one-command manual follow-up after
   downloading it through the app: dictate once, read the
   "stop -> delivered in …" log line.

2. **Hotkey/paste end-to-end latency and permission flows need a human.**
   macOS Accessibility cannot be granted headlessly, so bare Right-⌘ capture,
   real paste-into-Terminal, clipboard restore, and overlay focus behavior are
   manual checks (listed in GOAL.md). The perf probe measures the overlay
   show path (0.29 ms), which is the app-controlled portion of the < 150 ms
   budget.

3. **SPEC10: Windows whisper-vulkan does not build in CI (release profile).**
   Attempted per SPEC10 §1: `whisper-vulkan` in the Windows target table +
   Vulkan SDK 1.3.296 on windows-latest. Two failure layers were fixed or
   isolated with evidence:
   - CMake's MSBuild generator refuses ggml-vulkan's `vulkan-shaders-gen`
     ExternalProject rule chain ("items cannot be built in parallel",
     case-insensitive path dedup) — fixed by switching native builds to
     Ninja + the MSVC dev env (this fix is kept; MSBuild breaks regardless
     of Vulkan).
   - Under the **release** profile only, the nested shader-gen try-compile
     then dies with MSVC `C1083: Cannot open compiler generated file: '':
     Invalid argument` (debug builds of the same target succeed — proven by
     the green test-windows job on run 29200844825; release failed on the
     same commit).
   Verdict: upstream ggml/whisper-rs-sys toolchain bug, not fixable from
   this repo without patching the vendored build. Windows ships **CPU
   whisper** for the alpha (fully functional, slower); re-enabling is the
   one-line feature swap in `src-tauri/Cargo.toml` once upstream fixes land.
