// U15: human-readable byte sizes for the uninstall dialog (SPEC11 §3).

import { describe, expect, it } from "vitest";
import { formatBytes } from "../../src/lib/format";

describe("U15: formatBytes", () => {
  it("handles zero and sub-kilobyte values as plain bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("switches units at each 1024 boundary", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
  });

  it("rounds to one decimal", () => {
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(2040109466)).toBe("1.9 GB"); // the Parakeet-sized case
  });

  it("never crashes on junk", () => {
    expect(formatBytes(-5)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });
});
