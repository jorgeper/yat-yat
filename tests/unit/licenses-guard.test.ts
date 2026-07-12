// U13: the license allowlist guard core (SPEC8 §7) — permissive expressions
// pass, copyleft fails loudly, OR elects a permissive branch, AND requires
// every branch, and the tauri-nspanel metadata gap is explicitly overridden.

import { describe, expect, it } from "vitest";
// @ts-expect-error — plain .mjs module without type declarations
import { ALLOWED, licenseAllowed, OVERRIDES } from "../../scripts/licenses.mjs";

describe("U13: license allowlist guard", () => {
  it("passes the permissive expressions present in this repo's graphs", () => {
    for (const expr of [
      "MIT",
      "Apache-2.0",
      "MIT OR Apache-2.0",
      "(Apache-2.0 OR MIT) AND BSD-3-Clause",
      "Apache-2.0 WITH LLVM-exception",
      "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
      "Unlicense",
      "CDLA-Permissive-2.0",
      "CC0-1.0 OR MIT-0 OR Apache-2.0",
      "Zlib OR Apache-2.0 OR MIT",
      "Unicode-3.0",
      "MPL-2.0",
    ]) {
      expect(licenseAllowed(expr), expr).toBe(true);
    }
  });

  it("fails copyleft and missing licenses", () => {
    for (const expr of ["GPL-3.0-only", "AGPL-3.0", "LGPL-2.1-or-later", "", null, undefined]) {
      expect(licenseAllowed(expr as string), String(expr)).toBe(false);
    }
  });

  it("OR elects the permissive branch; AND requires every branch", () => {
    expect(licenseAllowed("MIT OR GPL-3.0")).toBe(true);
    expect(licenseAllowed("GPL-3.0 OR MIT")).toBe(true);
    expect(licenseAllowed("Apache-2.0 AND GPL-3.0")).toBe(false);
    expect(licenseAllowed("(MIT OR GPL-3.0) AND ISC")).toBe(true);
  });

  it("legacy slash-form duals normalize the way the generator applies them", () => {
    // The generator rewrites '/' to ' OR ' before evaluating.
    expect(licenseAllowed("Apache-2.0/MIT".replace(/\//g, " OR "))).toBe(true);
  });

  it("tauri-nspanel's missing metadata is covered by an explicit MIT-or-Apache override", () => {
    const override = OVERRIDES.get("tauri-nspanel");
    expect(override).toBe("MIT OR Apache-2.0");
    expect(licenseAllowed(override)).toBe(true);
  });

  it("the allowlist itself contains no copyleft entries", () => {
    for (const id of ALLOWED) {
      expect(/(^|-)GPL/.test(id), id).toBe(false);
    }
  });
});
