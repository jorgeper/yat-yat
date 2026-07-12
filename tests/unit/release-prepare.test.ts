// U12: release-prepare's exported pure transforms (SPEC8 §7) — the exact code
// that rewrites the three version files. Strict semver, surgical rewrites,
// pre-release identifiers preserved verbatim.

import { describe, expect, it } from "vitest";
// @ts-expect-error — plain .mjs module without type declarations
import { isValidSemver, setCargoVersion, setJsonVersion } from "../../scripts/release-prepare.mjs";

describe("U12: semver validation", () => {
  it("accepts full semver, with and without pre-release ids", () => {
    for (const v of ["1.2.3", "0.2.0-alpha.1", "10.0.0-beta.12", "1.0.0-rc.1+build.5"]) {
      expect(isValidSemver(v), v).toBe(true);
    }
  });

  it("rejects partial versions, leading zeros, and junk", () => {
    for (const v of ["1.2", "01.2.3", "1.2.3-", "1.2.3-alpha..1", "v1.2.3", "", undefined]) {
      expect(isValidSemver(v as string), String(v)).toBe(false);
    }
  });
});

describe("U12: version file rewrites", () => {
  it("rewrites exactly the top-level version field of package.json-style files", () => {
    const json = `{\n  "name": "yat-yat",\n  "version": "0.1.0",\n  "dependencies": { "x": { "version": "9.9.9" } }\n}`;
    const out = setJsonVersion(json, "0.2.0-alpha.1");
    expect(out).toContain(`"version": "0.2.0-alpha.1"`);
    // The nested (dependency) version is untouched — first match wins.
    expect(out).toContain(`"version": "9.9.9"`);
    expect(out.replace(`"version": "0.2.0-alpha.1"`, `"version": "0.1.0"`)).toBe(json);
  });

  it("rewrites exactly the [package] version line of Cargo.toml", () => {
    const toml = `[package]\nname = "yat-yat"\nversion = "0.1.0"\n\n[dependencies]\ntauri = { version = "2" }\n`;
    const out = setCargoVersion(toml, "0.2.0-alpha.1");
    expect(out).toContain(`version = "0.2.0-alpha.1"`);
    expect(out).toContain(`tauri = { version = "2" }`);
    expect(out.replace(`version = "0.2.0-alpha.1"`, `version = "0.1.0"`)).toBe(toml);
  });

  it("preserves pre-release identifiers verbatim — never strips them", () => {
    const json = `{ "version": "0.2.0-alpha.1" }`;
    expect(setJsonVersion(json, "0.2.0-alpha.2")).toBe(`{ "version": "0.2.0-alpha.2" }`);
    const toml = `version = "0.2.0-alpha.1"`;
    expect(setCargoVersion(toml, "1.0.0-rc.1")).toBe(`version = "1.0.0-rc.1"`);
  });
});
