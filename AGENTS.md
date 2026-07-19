# AGENTS.md

Operating rules for coding agents in this repo. CLAUDE.md is the primary
project guide (workflow, commands, architecture, traps); this file holds
rules learned in operation that CLAUDE.md doesn't cover.

## Writing /goal launcher commands (GOAL<N>.md)

- **The /goal condition is capped at 4000 characters** — the launcher
  rejects anything longer. Verify before saving the GOAL file:
  `wc -m` on the command text (everything after `/goal `).
- Never restate the spec's FRs inside the condition; that's what blows
  the cap. The pattern that fits:
  1. "Implement docs/specs/SPEC<N>.md in full" + "the SPEC is
     authoritative / where this condition is silent, the SPEC decides"
     (the condition already forbids editing the SPEC, so this is
     binding).
  2. The transcript-provable Done-when checks only: full
     `npm run validate` output with the exact test-ID ranges, signed
     `tauri build` path + size < 80 MB, any spec-specific provable
     number, docs updated, the `.skip|.only|.todo` grep.
  3. The standing constraints: SPEC/GOAL/condition unmodified; NO
     existing test amended, weakened, or deleted; no new network
     access; BLOCKERS.md instead of gaming a check.
  4. A turn/time stop with a summarize-remaining-work instruction.
- One or two spec-critical safety callouts (e.g. a data-loss rule) may
  be repeated in the condition when getting them wrong is
  unrecoverable; everything else stays in the SPEC.
