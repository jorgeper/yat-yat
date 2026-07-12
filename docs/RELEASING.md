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
gh release view v0.2.0-alpha.2               # 7 assets: dmg, setup.exe, app.tar.gz + both .sig, latest.json, SHA256SUMS.txt
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

## Updater (SPEC9)

Release builds are minisign-signed: CI injects `TAURI_SIGNING_PRIVATE_KEY`
and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (GitHub Actions secrets; the key
also lives in an untracked local backup — never in the repo), and tauri
emits `Yat Yat_<ver>_aarch64.app.tar.gz` + `.sig` plus a `latest.json`
manifest alongside the dmg. **Publishing a release** (the same human
`--draft=false` flip as always) triggers `updater-manifest.yml`, which
copies that release's `latest.json` onto the **rolling `updater` release**
— the fixed endpoint in-app Check for Updates… polls. Drafts never reach
it. If the manifest ever needs re-advancing (or rolling back), run the
workflow manually: `gh workflow run updater-manifest.yml -f tag=v<ver>`.

Local signed builds (to verify updater artifacts):

```bash
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/yat-yat-updater.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/yat-yat-updater.password)" \
npm run tauri build
```

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

Since SPEC9 the updater ships (see “Updater” above); since SPEC10 the
pipeline also builds the **Windows x64 NSIS installer** (test-windows +
build-windows jobs; the setup.exe doubles as the signed updater artifact,
and `latest.json` carries both `darwin-aarch64` and `windows-x86_64`).
Before publishing a release to Windows users, walk SPEC10 §7's manual QA
checklist on real Windows hardware.

Out of scope for now (seams noted in SPEC8/SPEC10): code signing /
notarization on both platforms (macOS signing will also end the
Accessibility re-grant dance), Linux packages, ARM64 Windows, and universal
(Intel) macOS builds.
