# SPEC8: Yat Yat — open-source alpha: semver, CI/CD releases, docs layout, About, license

An increment over SPEC.md–SPEC7.md (authoritative elsewhere; at repo root
until §4 of this spec moves them under `docs/specs/`). Yat Yat adopts the
release machinery, repo layout, and licensing scaffolding proven in
`~/src/marky-mark` (SPEC10 there is the reference implementation — copy its
patterns, adapt its details to this repo). Out of scope, seams only: code
signing/notarization, auto-updater, Windows/Linux packaging, universal
(Intel) macOS builds.

---

## 1. Semantic versioning, alpha channel (FR-V)

1. Strict semver.org versioning with a pre-release channel. Yat Yat is in
   **alpha**: versions read `MAJOR.MINOR.PATCH-alpha.N`. This spec sets the
   version to **`0.2.0-alpha.1`** (0.1.0 was the internal era; never
   published). The pre-release identifier is **never stripped**.
2. The version lives in exactly three files, always in lock-step:
   `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
   (lockfiles refresh alongside). Tags mirror the files (`v` + version) —
   the files are the source of truth, never `git describe`.
3. **`scripts/release-prepare.mjs`** (`npm run release:prepare -- <version>
   [--no-commit]`), ported from marky-mark: validates full semver
   (pre-release ids allowed, leading zeros rejected), rewrites all three
   files, refreshes `package-lock.json` and `src-tauri/Cargo.lock`, prints a
   scoped diffstat, and commits `chore: release v<version>` unless
   `--no-commit`. A rerun with the version already in place is a no-op. The
   version-apply helpers (semver check, JSON rewrite, TOML rewrite) are
   exported pure string transforms so U12 exercises exactly the code that
   rewrites the release files.

## 2. CI/CD release pipeline (FR-CI)

1. New **`.github/workflows/release.yml`**. Triggers: tag push `v*` and
   `workflow_dispatch` with a `version` input (dry-run → draft prerelease,
   no tag needed). Concurrency group keyed on the ref; `contents: write` on
   the release job only.
2. Jobs:
   - **test** (macos-latest): resolve the release version (tag name or
     input); **version check** — fail naming any of the three files whose
     version differs; `npm ci`, `npx playwright install chromium`,
     `npm run validate` (the full gate: R, U, E, I suites — I1's one-time
     Whisper-tiny download into `~/.cache/yatyat-validate/` is the harness's
     only sanctioned network and is fine in CI; cache it with
     `actions/cache`). Cache node + cargo (`Swatinem/rust-cache`,
     workspaces: src-tauri).
   - **build-macos** (macos-latest, needs test): `npm run tauri build` →
     the Apple Silicon `.dmg`. (Universal/Intel is a documented seam: the
     ONNX Runtime + whisper.cpp Metal x86_64 cross-build is unproven — do
     not attempt it in this pass.)
   - **release** (needs both): download artifacts, normalize asset names
     (GitHub rewrites spaces to dots — rename first so checksums match what
     users download), write `SHA256SUMS.txt`, create one **draft** GitHub
     Release with generated notes plus a fixed header explaining each asset
     and the unsigned-alpha caveats — Gatekeeper (System Settings → Privacy
     & Security → Open Anyway, or `xattr -dr com.apple.quarantine`), the
     mic/Accessibility permissions story, and that each unsigned build
     re-keys the Accessibility grant (re-grant on update). Publishing is
     always a human action.
3. Asset guard: exactly **two assets** — one `.dmg` + `SHA256SUMS.txt`;
   fail if the dmg exceeds **80 MB** (SPEC §9's app-size budget).
4. Releases live on **GitHub Releases**:
   `https://github.com/jorgeper/yat-yat/releases/latest` is the canonical
   "get it" link; `/releases` is the full index. No binaries in git.

## 3. README rewrite (FR-R)

Reshape README.md to the marky-mark standard OSS shape, keeping Yat Yat's
existing content (the local-only guarantee stays in the opening pitch — it
is the product's identity):

1. Name + one-liner; **badges** (License: MIT → LICENSE; Release →
   `/releases/latest`); an **alpha banner** (pre-release, unsigned builds,
   expect rough edges).
2. A **Download** section: table with the macOS `.dmg` (Apple Silicon,
   unsigned), a "verify against SHA256SUMS.txt" line, the all-versions
   `/releases` link, and a **First launch on macOS** subsection (Gatekeeper
   Open Anyway / `xattr` alternative; then onboarding asks for mic +
   Accessibility — link the existing permissions section).
3. Existing sections follow, updated: how it works, looks/themes, live
   transcription, models, enhancement, permissions, troubleshooting.
4. **Building from source** (the current Build section) and **Installing
   dev builds** stay; docs links point into the new layout (§4):
   `docs/ARCHITECTURE.md`, `docs/RELEASING.md`, `docs/specs/`, THEMES.md.
5. A **License** section: MIT, Jorge Pereira, THIRD-PARTY-NOTICES.md link.

## 4. Docs layout (FR-D)

1. `git mv` (history must survive `git log --follow`; contents
   byte-identical — these are historical records):
   - `SPEC.md`–`SPEC7.md` → `docs/specs/`
   - `GOAL.md`–`GOAL7.md` → `docs/goals/`
   - `ARCHITECTURE.md` → `docs/ARCHITECTURE.md`
2. New docs:
   - **`docs/RELEASING.md`** — the operator manual, marky-mark shape, both
     flows: *from Claude Code* ("release 0.2.0-alpha.2" → release:prepare,
     validate, licenses, tag; pushes handed back to the human:
     `! git push` then `! git push origin v<ver>`; Claude watches
     `gh run watch` and reports the draft) and *manual* (the same steps as
     copy-paste commands: prepare → validate → push → tag → watch →
     smoke-test the draft with `gh release download` + `shasum -c` →
     publish `gh release edit v<ver> --draft=false --prerelease`, stable
     later with `--latest`; plus dry-runs via workflow_dispatch, retracting
     a bad tag, and the semver/alpha policy: alpha.N = fixes/increments,
     MINOR = a new SPEC milestone, beta = feature-frozen, 1.0.0 = signed +
     stable).
   - **`docs/license.md`** — the license decision record (§6).
3. `SECURITY.md`, `BLOCKERS.md`, `THEMES.md`, `README.md`, `LICENSE`,
   `THIRD-PARTY-NOTICES.md` stay at root. Update every link to moved files
   in living docs (README, THEMES.md, CLAUDE.md if present); historical
   SPEC/GOAL contents are not edited.

## 5. About panel (FR-A)

The native app menu (added post-SPEC7) currently uses `.about(None)`.
Populate `AboutMetadata` to match marky-mark's About content — name,
developer, license: short version omitted, name **"Yat Yat"**, authors
**["Jorge Pereira"]** (this is the "Developer"), license **"MIT License"**,
copyright **"© 2026 Jorge Pereira"**, website
`https://github.com/jorgeper/yat-yat`. The version comes from the bundle
(tauri fills it) — no hardcoded version strings. Native panels aren't
headless-testable; this is a manual check in §8.

## 6. License scaffolding (FR-L)

Decision (audit run 2026-07-11, full graphs): **MIT**, copyright Jorge
Pereira. The resolved cargo graph (~650 crates) and npm production tree
(8 packages) contain only permissive licenses — MIT/Apache-2.0 duals
dominate; notable cases: `whisper-rs`/`whisper-rs-sys` are Unlicense
(wrapping MIT whisper.cpp), `cpal`/`hound` Apache-2.0, MPL-2.0 appears only
in transitive crates (file-level copyleft, no obligation on our license),
LGPL only ever as one branch of an OR (elect MIT/Apache), and
`tauri-nspanel` ships LICENSE_MIT + LICENSE_APACHE-2.0 files but no
`license` field in Cargo metadata (treat as MIT OR Apache-2.0 via an
explicit override in the checker, with a comment naming the files). STT
models are downloaded by the user, never bundled — Parakeet's CC-BY-4.0 and
Whisper's MIT do not constrain the app license; note their attribution in
license.md.

1. **`LICENSE`** at root: MIT, `Copyright (c) 2026 Jorge Pereira`.
2. `"license": "MIT"` added to package.json (Cargo.toml already has it).
3. **`scripts/licenses.mjs`** (`npm run licenses`), ported from marky-mark:
   walks the npm production tree (package-lock.json) and `cargo metadata`
   (resolved graph), regenerates **`THIRD-PARTY-NOTICES.md`** (name,
   version, license; sorted; no timestamps; byte-identical on rerun), and
   **exits non-zero on any license outside the permissive allowlist**
   (marky-mark's list plus the expressions present here, e.g.
   `CDLA-Permissive-2.0`, `MIT-0`; OR = any branch passes, AND = all
   branches must pass). The tauri-nspanel override lives here.
4. **`docs/license.md`**: the decision record — recommendation, the audit
   tables (cargo + npm), the special cases above, and the model-attribution
   note.

## 7. Tests

- **U12 (release-prepare):** the exported pure transforms — semver
  validation (accepts `1.2.3`, `0.2.0-alpha.1`; rejects `1.2`, `01.2.3`,
  empty pre-release ids), JSON rewrite touches only the top-level version
  field, TOML rewrite touches only the `[package]` version line,
  pre-release identifiers preserved verbatim.
- **U13 (license guard):** the checker core — a permissive set passes; a
  fake `GPL-3.0-only` entry fails naming the offender; `MIT OR GPL-3.0`
  passes (OR elects the permissive branch); `Apache-2.0 AND GPL-3.0` fails;
  a missing license fails unless explicitly overridden.
- validate.mjs step labels update to U1–U13. No other test changes; nothing
  existing may be weakened, stubbed, or deleted.

## 8. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R13, U1–U13,
   E1–E14c, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `grep '"version"' package.json src-tauri/tauri.conf.json` and
   `grep '^version' src-tauri/Cargo.toml` all print **0.2.0-alpha.1**.
3. `npm run release:prepare -- 0.2.0-alpha.2 --no-commit` prints a diffstat
   touching exactly the three version files + lockfiles (tree restored
   afterward); a same-version rerun reports a no-op.
4. `npm run licenses` exits 0 and a second consecutive run produces zero
   diff in THIRD-PARTY-NOTICES.md.
5. `npm run tauri build` exits 0; `.app` path + size (< 80 MB) printed; the
   dmg filename carries `0.2.0-alpha.1`.
6. `.github/workflows/release.yml` parses cleanly (`actionlint` if
   available, else a YAML parse check printed in the transcript).
7. LICENSE, THIRD-PARTY-NOTICES.md, docs/RELEASING.md, docs/license.md all
   exist per §§4–6; README has the badges, alpha banner, Download section,
   `/releases/latest` + `/releases` links, and License section.
8. `git log --follow --oneline docs/specs/SPEC.md | tail -1` shows the
   original commit (history survived the moves); `ls docs/specs` lists
   SPEC.md–SPEC8.md and `ls docs/goals` lists GOAL.md–GOAL8.md; no SPEC*/
   GOAL*/ARCHITECTURE.md remain at root.
9. `grep -rn ".skip\|.only\|.todo" tests/` prints nothing; moved SPEC/GOAL
   contents byte-identical (`git status` shows pure renames).
10. docs/ARCHITECTURE.md gains a short "Release pipeline" note: version
    lock-step + release-prepare, the workflow topology (test → build →
    draft release), and the license allowlist guard.
11. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green: Yat Yat menu → About shows name, version
0.2.0-alpha.1, Jorge Pereira, and MIT License; after the first real tag
push, `gh run watch` ends in a draft release with exactly the dmg +
SHA256SUMS.txt, and the checksums verify.
