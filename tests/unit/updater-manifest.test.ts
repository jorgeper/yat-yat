// U14: the updater-manifest composer (SPEC9 §6.1) — valid tauri-updater
// schema out of signed artifacts; malformed inputs throw rather than emit a
// broken manifest.

import { describe, expect, it } from "vitest";
// @ts-expect-error — plain .mjs module without type declarations
import { composeManifest } from "../../scripts/updater-manifest.mjs";

const good = {
  version: "0.1.0-alpha.2",
  notes: "Yat Yat 0.1.0-alpha.2 — see the release page.",
  pubDate: "2026-07-12T00:00:00Z",
  assets: [
    {
      platform: "darwin-aarch64",
      url: "https://github.com/jorgeper/yat-yat/releases/download/v0.1.0-alpha.2/Yat.Yat_0.1.0-alpha.2_aarch64.app.tar.gz",
      signature: "dW50cnVzdGVkIGNvbW1lbnQ6...sig...\n",
    },
  ],
};

describe("U14: updater manifest composer", () => {
  it("emits a schema-valid manifest with the darwin-aarch64 platform", () => {
    const m = composeManifest(good);
    expect(m.version).toBe("0.1.0-alpha.2");
    expect(m.notes).toContain("0.1.0-alpha.2");
    expect(m.pub_date).toBe("2026-07-12T00:00:00.000Z");
    expect(Object.keys(m.platforms)).toEqual(["darwin-aarch64"]);
    expect(m.platforms["darwin-aarch64"].url).toMatch(/^https:\/\//);
    // Signatures are embedded content, trimmed.
    expect(m.platforms["darwin-aarch64"].signature).toBe("dW50cnVzdGVkIGNvbW1lbnQ6...sig...");
  });

  it("throws on malformed inputs instead of emitting a broken manifest", () => {
    expect(() => composeManifest({ ...good, version: "" })).toThrow(/version/);
    expect(() => composeManifest({ ...good, pubDate: "not a date" })).toThrow(/pubDate/);
    expect(() => composeManifest({ ...good, assets: [] })).toThrow(/asset/);
    expect(() =>
      composeManifest({
        ...good,
        assets: [{ ...good.assets[0], platform: "windows-x86_64" }],
      }),
    ).toThrow(/unknown platform/);
    expect(() =>
      composeManifest({ ...good, assets: [{ ...good.assets[0], url: "http://insecure" }] }),
    ).toThrow(/bad url/);
    expect(() =>
      composeManifest({ ...good, assets: [{ ...good.assets[0], signature: "  " }] }),
    ).toThrow(/signature/);
  });
});
