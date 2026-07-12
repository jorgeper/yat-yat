# Releasing Yat Yat

Releases are cut from a tag, built by `.github/workflows/release.yml` (SPEC8
§2), and land as a **draft** GitHub Release — nothing goes public until a
human smoke-tests the draft and flips it. Versions are strict semver; the
pre-release identifier (`0.2.0-alpha.1`) is **never stripped**. The version
lives in `package.json`, `src-tauri/tauri.conf.json`, and
`src-tauri/Cargo.toml`, and moves only via `npm run release:prepare`. Tags
mirror the files (`v` + version) — the files are the source of truth, not
`git describe`.

## Flow 1 — from Claude Code

Tell Claude: *"release 0.2.0-alpha.2"*. Claude runs, in order:

```bash
cd ~/src/yat-yat
npm run release:prepare -- 0.2.0-alpha.2   # bumps the 3 version files + lockfiles, commits
npm run validate                            # full gate must print VALIDATION: ALL PASSED
npm run licenses                            # regenerate THIRD-PARTY-NOTICES.md (commit if changed)
git tag -a v0.2.0-alpha.2 -m "Yat Yat 0.2.0-alpha.2"
```

Then the pushes (run them yourself if you prefer them human-triggered):

```bash
git push
git push origin v0.2.0-alpha.2              # ← this starts the pipeline
```

Claude then watches CI (`gh run watch`) and reports when the draft is up;
smoke-test and publish as below.

## Flow 2 — manually

```bash
# ---- cut ------------------------------------------------------------------
cd ~/src/yat-yat
npm run release:prepare -- 0.2.0-alpha.2    # bump 3 version files + locks, commit
npm run validate                            # must end with VALIDATION: ALL PASSED
git push
git tag -a v0.2.0-alpha.2 -m "Yat Yat 0.2.0-alpha.2"
git push origin v0.2.0-alpha.2              # ← this starts the pipeline

# ---- watch ------------------------------------------------------------------
gh run list --workflow release.yml
gh run watch                                 # test gate → macOS build → draft release

# ---- smoke-test the draft ----------------------------------------------------
gh release view v0.2.0-alpha.2               # exactly 2 assets: dmg, SHA256SUMS.txt
gh release download v0.2.0-alpha.2 -D /tmp/yy-smoke
(cd /tmp/yy-smoke && shasum -c SHA256SUMS.txt)   # verify, then install & dictate

# ---- publish (the only irreversible step) -------------------------------------
gh release edit v0.2.0-alpha.2 --draft=false --prerelease   # alpha/beta/rc
gh release edit v1.0.0 --draft=false --latest               # stable releases

# ---- other moves ---------------------------------------------------------------
gh workflow run release.yml -f version=0.2.0-alpha.2   # dry-run: draft prerelease, no tag
gh release delete v0.2.0-alpha.2 --yes                 # discard a draft/dry-run
git push origin :refs/tags/v0.2.0-alpha.2              # retract a bad tag
gh release list
```

Rules of thumb: the tag push is the trigger, the draft is the safety net, and
`--draft=false` is the only step that makes anything public. A failed run is
re-cut by fixing, deleting + re-pushing the tag, or `workflow_dispatch`.

Smoke-test essentials for Yat Yat specifically: install the dmg to
/Applications, clear quarantine (unsigned alpha — Gatekeeper will block the
first open), walk onboarding (mic + Accessibility + a model download), and
dictate into a real app. Remember that each unsigned build re-keys the
Accessibility grant — updating from a previous alpha will ask you to
re-grant, and the app's wizard handles it.

## Semver / alpha policy

- **`alpha.N` bumps** (`0.2.0-alpha.1` → `0.2.0-alpha.2`): fixes and
  incremental features on the way to the same milestone.
- **MINOR bumps** (`0.2.0-…` → `0.3.0-alpha.1`): a new feature milestone
  (roughly: a new SPEC delta implemented).
- **Graduating**: `-beta.N` when features for the milestone are frozen and
  only stabilization remains; dropping the pre-release id entirely (`1.0.0`)
  means signed builds, stable formats, and update guarantees — publish those
  with `--latest` so `/releases/latest` points at them.
- Pre-releases are published with `--prerelease` (GitHub labels them and
  keeps them off `/releases/latest` once a stable release exists).

Out of scope for now (seams noted in SPEC8): code signing / notarization
(which will also end the Accessibility re-grant dance), the auto-updater,
Windows/Linux packages, and universal (Intel) macOS builds.
