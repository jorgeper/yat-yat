// U10: personal-dictionary table in Settings → Cleanup (SPEC7 FR-D3) —
// renders entries, add/edit/delete round-trip through save (set_settings),
// and a row with an empty "Heard" cell is never persisted.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import CleanupSection from "../../src/settings/CleanupSection";
import { defaultSettings } from "../../src/ipc/types";
import type { Settings } from "../../src/ipc/types";

afterEach(cleanup);

function renderCleanup(overrides: Partial<Settings> = {}) {
  const settings: Settings = { ...defaultSettings(), ...overrides };
  const save = vi.fn().mockResolvedValue(undefined);
  render(<CleanupSection settings={settings} save={save} />);
  return { settings, save };
}

const seeded = [
  { from: "acme corp", to: "AcmeCorp" },
  { from: "jorge pereira", to: "Jorge Pereira" },
];

describe("U10: personal dictionary table", () => {
  it("renders the existing entries", () => {
    renderCleanup({ dictionary: [...seeded] });
    expect(screen.getAllByTestId("dict-row")).toHaveLength(2);
    expect(screen.getByTestId("dict-from-0")).toHaveValue("acme corp");
    expect(screen.getByTestId("dict-to-0")).toHaveValue("AcmeCorp");
    expect(screen.getByTestId("dict-from-1")).toHaveValue("jorge pereira");
  });

  it("adds a row through save", () => {
    const { save } = renderCleanup();
    fireEvent.change(screen.getByTestId("dict-new-from"), { target: { value: "  yat yat " } });
    fireEvent.change(screen.getByTestId("dict-new-to"), { target: { value: "Yat Yat" } });
    fireEvent.click(screen.getByTestId("dict-add"));
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][0].dictionary).toEqual([{ from: "yat yat", to: "Yat Yat" }]);
  });

  it("never persists a row with an empty Heard cell", () => {
    const { save } = renderCleanup();
    fireEvent.change(screen.getByTestId("dict-new-to"), { target: { value: "orphan" } });
    fireEvent.click(screen.getByTestId("dict-add"));
    fireEvent.change(screen.getByTestId("dict-new-from"), { target: { value: "   " } });
    fireEvent.click(screen.getByTestId("dict-add"));
    expect(save).not.toHaveBeenCalled();
  });

  it("edits commit on blur", () => {
    const { save } = renderCleanup({ dictionary: [...seeded] });
    const from = screen.getByTestId("dict-from-0");
    fireEvent.change(from, { target: { value: "acme corporation" } });
    fireEvent.blur(from);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][0].dictionary[0]).toEqual({
      from: "acme corporation",
      to: "AcmeCorp",
    });
    expect(save.mock.calls[0][0].dictionary[1]).toEqual(seeded[1]);
  });

  it("an edit that empties Heard is dropped, not saved", () => {
    const { save } = renderCleanup({ dictionary: [...seeded] });
    const from = screen.getByTestId("dict-from-0");
    fireEvent.change(from, { target: { value: "  " } });
    fireEvent.blur(from);
    expect(save).not.toHaveBeenCalled();
  });

  it("deletes a row through save", () => {
    const { save } = renderCleanup({ dictionary: [...seeded] });
    fireEvent.click(screen.getByTestId("dict-delete-0"));
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][0].dictionary).toEqual([seeded[1]]);
  });
});
