# Launching the open-source-alpha build with /goal

Run from this directory (`yat-yat/`). SPEC.md through SPEC7.md (at repo root
until this spec's §4 moves them) remain authoritative except where
docs/specs/SPEC8.md adds the release machinery. `~/src/marky-mark` is the
reference implementation — read its release.yml, scripts/release-prepare.mjs,
scripts/licenses.mjs, and docs/RELEASING.md before writing Yat Yat's.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC8.md in full (read SPEC.md through SPEC7.md first — they are at repo root until §4 moves them into docs/specs/ — and study ~/src/marky-mark's release.yml, release-prepare.mjs, licenses.mjs, and RELEASING.md as the reference implementation; do NOT implement code signing, notarization, auto-updater, Windows/Linux packaging, or universal macOS builds). Done when: 'npm run validate' exits 0 with its complete output — Rust tests R1–R13, frontend tests U1–U13, e2e tests E1–E14c, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND grepping "version" in package.json and src-tauri/tauri.conf.json plus the version line of src-tauri/Cargo.toml all print 0.2.0-alpha.1, AND 'npm run release:prepare -- 0.2.0-alpha.2 --no-commit' prints a diffstat touching exactly the three version files plus lockfiles with the tree restored afterward and a same-version rerun reporting a no-op, AND 'npm run licenses' exits 0 with a second consecutive run producing zero diff in THIRD-PARTY-NOTICES.md, AND 'npm run tauri build' exits 0 with the produced .app path and size (< 80 MB) printed and the dmg filename carrying 0.2.0-alpha.1, AND .github/workflows/release.yml parses cleanly (actionlint if available, else a YAML parse check printed), AND LICENSE (MIT, Copyright (c) 2026 Jorge Pereira), THIRD-PARTY-NOTICES.md, docs/RELEASING.md, and docs/license.md all exist per SPEC8 §§4–6, AND README.md carries the MIT and release badges, the alpha banner, a Download section with /releases/latest and /releases links, and a License section, AND 'ls docs/specs' lists SPEC.md through SPEC8.md and 'ls docs/goals' lists GOAL.md through GOAL8.md with no SPEC*, GOAL*, or ARCHITECTURE.md left at repo root, AND 'git log --follow --oneline docs/specs/SPEC.md | tail -1' prints the file's original first commit, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified — moved files must stay byte-identical (git mv renames, verified via git status showing pure renames); the About panel must carry AboutMetadata with authors Jorge Pereira, license MIT License, copyright © 2026 Jorge Pereira, and the repo website per SPEC8 §5; the version pre-release identifier must never be stripped; the release-prepare version transforms must be exported pure functions covered by U12 and the license allowlist core covered by U13, including the tauri-nspanel MIT-OR-Apache override; existing tests may not be weakened, stubbed, or deleted; the only sanctioned network in the harness remains I1's cached model download; links in README/THEMES.md to moved docs must be updated; if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 60 turns or 6 hours even if incomplete, and summarize remaining work.
```

## Unattended one-liner

Same text via `claude -p "/goal …"` — escape the inner double quotes as in
GOAL6.md/GOAL7.md.

## After it goes green (your part — pushes and the first release)

```bash
cd ~/src/yat-yat
! git push
git tag -a v0.2.0-alpha.1 -m "Yat Yat 0.2.0-alpha.1"
! git push origin v0.2.0-alpha.1          # ← starts the pipeline
gh run watch                              # test gate → macOS build → draft release
gh release view v0.2.0-alpha.1           # exactly 2 assets: dmg + SHA256SUMS.txt
# smoke-test the draft (download, shasum -c, install, dictate), then:
gh release edit v0.2.0-alpha.1 --draft=false --prerelease
```

## Notes

- CI's validate gate downloads Whisper tiny (~75 MB) once per cache miss —
  wire `~/.cache/yatyat-validate` into actions/cache so reruns are quick.
- The Accessibility grant re-keys on every unsigned build — the release
  notes header must tell updaters to re-grant (the app's own wizard walks
  them through it; README Troubleshooting already covers it).
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments (use "bypass"/"solely"/"pending" phrasing).
- Manual checks after green: Yat Yat menu → About shows Yat Yat,
  v0.2.0-alpha.1, Jorge Pereira, MIT License; every README link still
  resolves (docs moved!); `git log --follow` walks a moved spec's history.
