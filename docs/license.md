# License decision record for Yat Yat

**Decision: MIT License**, copyright "Jorge Pereira" — scaffolded by SPEC8
(LICENSE at root, `license` metadata in package.json and Cargo.toml, the
About panel, and the THIRD-PARTY-NOTICES.md generator).

## Why MIT

- Nothing in the dependency graphs constrains the choice (audit below):
  permissive dependencies constrain how we *redistribute them*, not how we
  license *our own code*.
- MIT is the de-facto standard of this exact ecosystem (Tauri, React, Vite,
  Handy — the project Yat Yat adapts patterns from — are all MIT or
  MIT-dual). Zero-friction for contributors and companies.
- It matches marky-mark, the sibling project whose release machinery this
  repo adopts, so the whole portfolio reads consistently.
- The only obligation on users is keeping the copyright notice.

## Dependency audit (2026-07-11, full resolved graphs)

### Rust / cargo (~661 crates, ships in the desktop binary)

| License bucket | Count | Notes |
|---|---|---|
| MIT OR Apache-2.0 (incl. `/` spellings) | ~410 | the Rust-ecosystem default; we elect MIT |
| MIT | 145 | includes **transcribe-rs**, **handy-keys**, **enigo**, rubato |
| Apache-2.0 | 8 | includes **cpal**, **hound** |
| Unicode-3.0 / Unicode-DFS | 19 | ICU data crates |
| BSD-2/3-Clause (plain or OR-dual) | ~12 | |
| Zlib / BSL-1.0 / ISC / 0BSD combos | ~35 | |
| **Unlicense** | 2 | **whisper-rs / whisper-rs-sys** — public-domain-equivalent wrapper; the vendored **whisper.cpp** inside is MIT (ggml-org) |
| **MPL-2.0** | 5 | transitive only (e.g. resvg stack); file-level copyleft — imposes nothing on our license, files ship unmodified |
| MIT OR Apache-2.0 OR **LGPL-2.1-or-later** | 2 | `r-efi`; LGPL is one *branch of an OR* — we elect MIT |
| CDLA-Permissive-2.0 | 1 | `webpki-root-certs` (Mozilla root-store data) |
| **(no metadata)** | 1 | **tauri-nspanel** — its checkout ships `LICENSE_MIT` + `LICENSE_APACHE-2.0`; the Cargo.toml simply omits the `license` field. Handled by an explicit `MIT OR Apache-2.0` override in `scripts/licenses.mjs`, unit-tested by U13. |

No GPL/AGPL anywhere in the resolved graph.

### npm (8 production packages)

| License | Packages |
|---|---|
| MIT | react, react-dom, scheduler, tauri-plugin-macos-permissions-api, loose-envify, js-tokens |
| MIT OR Apache-2.0 | @tauri-apps/api, @tauri-apps/plugin-autostart |

Dev-only tooling (Vite, Playwright, TypeScript, Vitest…) is not distributed
and is excluded from THIRD-PARTY-NOTICES.md.

## Models (not bundled — user-downloaded at runtime)

Yat Yat ships no models; the catalog downloads them on the user's explicit
click, so their licenses do not constrain the app's license. For attribution:

| Model | License | Source |
|---|---|---|
| Parakeet TDT 0.6B v3 | CC-BY-4.0 | NVIDIA (int8 ONNX via community conversion) |
| Whisper (tiny/small/large-v3-turbo GGUF) | MIT | OpenAI weights, ggml conversions |

CC-BY-4.0 requires attribution when *distributing* the model — we don't; the
in-app catalog and this file credit NVIDIA regardless.

## Code provenance notes

- Patterns adapted from **Handy** (MIT) with attribution in README and
  ARCHITECTURE — MIT-compatible.
- **VoiceInk** (GPL-3.0) was used as a *behavior* reference only; no code was
  copied (SPEC §2 forbade it precisely to keep this decision clean).

## Ongoing guard

`npm run licenses` regenerates THIRD-PARTY-NOTICES.md from the real graphs
and **fails on any license outside the permissive allowlist** — a future
`cargo add`/`npm install` can never pull copyleft into the bundle silently
(U13 covers the checker's core, including the tauri-nspanel override).
