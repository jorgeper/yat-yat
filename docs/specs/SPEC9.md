# SPEC9: Yat Yat — Check for Updates (GitHub-releases updater)

An increment over docs/specs/SPEC.md–SPEC8.md (authoritative elsewhere). A
direct port of marky-mark's SPEC19 — same mechanisms, same UI — adapted to
Yat Yat's realities: Apple-Silicon-only builds, native menus, and the
settings window as the only chrome.

**What ships:** **Check for Updates…** in the app menu and the tray menu,
backed by the official `tauri-plugin-updater` with **GitHub Releases as the
update server**: a manual, user-initiated check against a rolling `updater`
release manifest, then one-click download → signature verification →
install → relaunch. The local-only guarantee is amended precisely, not
weakened: all update network is Rust-side, user-initiated, to two fixed
GitHub endpoints, verified against a baked-in public key.

Out of scope: automatic/scheduled checks, delta updates, downgrade UI,
changelog rendering beyond the release-note text, updating the *currently
installed* pre-SPEC9 build (the first hop is always a manual download).

---

## 1. Trust & endpoint model (FR-T)

1. **Signing:** a minisign keypair generated once (`tauri signer
   generate`). The **public key** lives in `tauri.conf.json`'s updater
   config (committed); the **private key + password live only in GitHub
   Actions secrets** (`TAURI_SIGNING_PRIVATE_KEY`,
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) and in a local untracked backup
   whose path is reported to the human. **No private-key material may ever
   be committed** (§8 greps for it).
2. **Endpoint:**
   `https://github.com/jorgeper/yat-yat/releases/download/updater/latest.json`
   — a **rolling release tagged `updater`** (GitHub's `releases/latest`
   skips pre-releases, so alphas need the indirection). Drafts stay
   invisible: the rolling manifest only advances on *publish* (§3).
3. Version comparison, download, signature verification, install, and
   relaunch are all the plugin's stock behavior — no hand-rolled crypto or
   installer logic.

## 2. App UX (FR-U)

1. **"Check for Updates…"** menu items: app menu directly after About, and
   the tray menu directly after Settings… (the tray is Yat Yat's everyday
   surface). Both show the settings window and emit a `check-updates`
   event to it — the dialog lives in the settings webview.
2. The dialog (`update-dialog`, marky-mark's exact component shape and
   testids) walks: **checking** (`update-checking`) → **up to date**
   (`update-none`, shows the current version) or **available**
   (`update-available`: new version + release-note text, buttons
   `update-install` / `update-later`) → **downloading** (`update-progress`,
   live percent) → **ready** (`update-restart` relaunches). Errors
   (offline, malformed manifest, bad signature) land in a dismissable
   `update-error` state with the reason — never a crash, never a partial
   install. Esc/click-outside close it.
3. **Seam:** app code never imports plugin APIs directly. `src/ipc/
   updates.ts` exposes `{ check(): Promise<{version, notes} | null>,
   downloadAndInstall(onProgress), restart() }`; the Tauri branch
   implements it with `@tauri-apps/plugin-updater` + `plugin-process`
   (the Update object survives from check to install); the browser branch
   is a mock driven by a `window.__yyUpdate` test hook (next-check result:
   null / a version / an error; records progress, installs, restarts) —
   the e2e seam, mirroring marky-mark's `__mmUpdate`.

## 3. Release pipeline (FR-C)

1. `tauri.conf.json`: `bundle.createUpdaterArtifacts: true` and the
   updater plugin config (pubkey + the §1.2 endpoint). Release builds emit
   `Yat Yat.app.tar.gz` + `.sig` alongside the dmg. CSP untouched.
2. `release.yml`: signing env vars from secrets on the build job; the
   updater artifacts are versioned
   (`Yat Yat_<ver>_aarch64.app.tar.gz[.sig]`), checksummed, and a
   generated **`latest.json`** (platform key `darwin-aarch64`, URL into
   the versioned release's assets, embedded signature, version, notes,
   pub_date) joins the draft release. The manifest is composed by
   **`scripts/updater-manifest.mjs`** — pure exported core (U14-tested);
   malformed inputs throw rather than emit a broken manifest.
3. **New workflow `updater-manifest.yml`** — on release published/
   prereleased/released (idempotent; skips the `updater` tag itself) plus
   a `workflow_dispatch` recovery lever: copies the published release's
   `latest.json` onto the rolling `updater` release, creating it (plain,
   non-pre-release, clearly described as machine-consumed) if absent.
   Publishing stays the human act it is today.
4. Rust: `tauri_plugin_updater` + `tauri_plugin_process` registered in
   `lib.rs`; the default capability gains `updater:default` +
   `process:default`.

## 4. Dependencies (deliberately granted)

Exactly four, the ecosystem-standard updater pair: cargo
`tauri-plugin-updater` + `tauri-plugin-process`; npm
`@tauri-apps/plugin-updater` + `@tauri-apps/plugin-process`.
`npm run licenses` regenerated; the allowlist gate must stay green.

## 5. Privacy story (FR-P)

README's "Fully local" bullet is amended precisely: dictation still never
touches the network; the ONLY network operations are (a) user-initiated
model downloads, (b) the localhost-locked enhancement pass, and now (c) the
**user-initiated** update check/download, Rust-side, exclusively to the two
GitHub endpoints, responses verified against the baked-in public key. No
telemetry, no auto-checks. The enhancement localhost guard (R8) and every
existing network property stay byte-for-byte enforced.

## 6. Tests (added: U14, E15a–E15b)

1. **U14** — the manifest composer: given version/notes/pub-date/signed
   asset → valid updater schema (platform keyed `darwin-aarch64`, https
   URL, embedded signature, ISO pub_date); missing/malformed inputs
   (bad platform, non-https URL, empty signature, unparseable date) throw.
2. **E15a** — the dialog flow (mock): `check-updates` event opens the
   dialog → checking → available (version + notes shown) → install
   (progress reaches 100%) → restart recorded on `__yyUpdate`; and the
   up-to-date path shows `update-none` with the current version.
3. **E15b** — failure honesty: mock error ⇒ `update-error` with the
   message, dialog dismissable, settings fully functional after; a second
   check succeeds (state resets).
4. No existing test may be modified, weakened, or deleted.

## 7. Docs

README: a short "Updates" note (manual check, GitHub-hosted, signed) + the
amended privacy bullet (§5). docs/RELEASING.md: the new artifacts, the
rolling `updater` release, secrets, and the unchanged human publish act.
docs/ARCHITECTURE.md: an updater paragraph in the release-pipeline note.

## 8. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R13, U1–U14,
   E1–E15b, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` with the signing key in env exits 0 and the
   bundle dir contains `Yat Yat.app.tar.gz` + `.sig`; running
   `scripts/updater-manifest.mjs` against them emits a schema-valid
   `latest.json` (shown).
3. `gh secret list` shows `TAURI_SIGNING_PRIVATE_KEY` and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the private key exists in an
   untracked local file whose path is reported to the human.
4. No private-key material in the repo: `git grep -l "minisign encrypted
   secret key"` prints nothing.
5. `npm run licenses` green with the new packages; no `.skip/.only/.todo`
   in tests/; both workflows parse.
6. README, docs/RELEASING.md, docs/ARCHITECTURE.md updated per §5/§7.
7. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green: on an installed build, app menu and tray both
show Check for Updates…; the dialog reports "up to date" against the live
endpoint once a manifest exists; after the next published release, the
full check → install → relaunch hop works.
