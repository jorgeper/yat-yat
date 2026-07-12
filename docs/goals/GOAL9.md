# Launching the updater build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC8.md remain
authoritative except where docs/specs/SPEC9.md adds the updater.
`~/src/marky-mark` is the reference implementation (SPEC19 there):
UpdateDialog.tsx, updater-manifest.mjs, both workflows, and the platform
seam port almost verbatim.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC9.md in full (read docs/specs/SPEC.md through SPEC8.md first; study ~/src/marky-mark's SPEC19 implementation — UpdateDialog.tsx, scripts/updater-manifest.mjs, release.yml, updater-manifest.yml — as the reference; do NOT implement automatic checks, delta updates, downgrade UI, or Windows/Linux). Done when: 'npm run validate' exits 0 with its complete output — Rust tests R1–R13, frontend tests U1–U14, e2e tests E1–E15b, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with TAURI_SIGNING_PRIVATE_KEY in env exits 0 with the bundle dir containing 'Yat Yat.app.tar.gz' and its '.sig' (paths printed), AND scripts/updater-manifest.mjs run against those artifacts prints a schema-valid latest.json with a darwin-aarch64 platform entry, AND 'gh secret list' shows TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD with the private key in an untracked local file whose path is reported, AND 'git grep -l "minisign encrypted secret key"' prints nothing, AND 'npm run licenses' exits 0, AND .github/workflows/release.yml and .github/workflows/updater-manifest.yml both parse cleanly, AND README.md carries the Updates note and the amended privacy bullet with docs/RELEASING.md and docs/ARCHITECTURE.md updated per SPEC9 §7, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; app code must reach the updater only through the src/ipc/updates.ts seam (plugin APIs imported nowhere else) with the browser mock driven by window.__yyUpdate; the dialog must use marky-mark's exact state machine and testids (update-dialog/-checking/-none/-available/-install/-later/-progress/-restart/-error); the manifest composer must be a pure exported function covered by U14 that throws on malformed input; both menu surfaces (app menu after About, tray after Settings…) must open the settings window and emit check-updates; no private-key material may be committed; the dictation pipeline's network properties (localhost-only enhancement, sanctioned model downloads) must stay untouched; existing tests may not be weakened, stubbed, or deleted; if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 60 turns or 6 hours even if incomplete, and summarize remaining work.
```

## After it goes green (your part)

The next release you cut and publish per docs/RELEASING.md automatically
carries the updater artifacts + latest.json, and the updater-manifest
workflow advances the rolling `updater` release on publish. Installed
builds from THIS spec onward can then self-update; the hop from any older
alpha is a manual download.

## Notes

- The signing password and key live outside the repo — if the key is ever
  lost, regenerate + rotate the pubkey in tauri.conf.json; shipped builds
  then need one manual hop again.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments.
