# Launching the Windows-port build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC9.md remain
authoritative except where docs/specs/SPEC10.md adds the Windows port.
`~/src/marky-mark` is again the reference: its release.yml build-windows
job, cargo-xwin fallback, and updater-manifest windows-x86_64 handling.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC10.md in full (read docs/specs/SPEC.md through SPEC9.md first; study ~/src/marky-mark's build-windows job and updater manifest as the reference; do NOT implement Windows code signing, ARM64 Windows, MSI packaging, or Microsoft Store distribution). Done when: 'npm run validate' exits 0 on macOS with its complete output — Rust tests R1–R14, frontend tests U1–U14, e2e tests E1–E15b, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'cargo check --target x86_64-pc-windows-msvc' exits 0 (via cargo-xwin locally or shown green in CI), AND a pushed run shows the new test-windows job (cargo test + vitest + frontend build on windows-latest) and build-windows job (NSIS bundle with signing env) BOTH green with 'gh run view' output printed and the setup.exe plus .sig among the artifacts, AND scripts/updater-manifest.mjs invoked with both a mac and a windows asset prints a schema-valid latest.json containing darwin-aarch64 and windows-x86_64 entries, AND the release job's asset guard and gh release create lines cover exactly dmg + setup.exe + both updater artifacts + latest.json + SHA256SUMS.txt, AND README.md carries the Windows download row and SmartScreen first-launch note with docs/RELEASING.md and docs/ARCHITECTURE.md updated per SPEC10 §4, AND 'npm run licenses' exits 0, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; the Windows frontmost_app must compare lowercased executable paths with every failure path returning None (guard fails open) and its key normalization must be a pure exported function covered by new R14; sound-cue playback must remain fire-and-forget off the dictation timing path; the ONLY sanctioned existing-test change is U14 gaining windows-x86_64 as valid with the unknown-platform case moving to a genuinely unknown id (SPEC10 §5) — nothing else may be modified, weakened, or deleted; the windows/windows-sys crate dependency must be Windows-target-only; macOS behavior must be byte-for-byte unaffected (no shared-code regressions); if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 60 turns or 6 hours even if incomplete, and summarize remaining work.
```

## After it goes green (your part)

The next release you cut ships both platforms; the updater manifest then
serves both keys and Windows installs self-update exactly like macOS.
Before publishing that release, walk SPEC10 §7's manual QA checklist on a
real Windows machine — hotkey, overlay focus semantics, paste targets,
focus guard, mic consent, Vulkan/CPU inference. Anything broken there is
the next spec, not a reason to game this one.

## Notes

- No Windows machine is required to reach this DoD (CI + cargo-xwin cover
  it), but one IS required before shipping the first Windows build to
  anyone — budget a few hours of hands-on QA.
- The signing keypair already covers Windows (minisign is
  platform-agnostic); no new secrets.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments.
