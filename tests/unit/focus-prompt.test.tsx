// U11: the focus-guard overlay state (SPEC7 FR-G3) — renders both app names
// and both buttons; the paste and copy-fallback buttons each fire the right
// IPC command through the browser mock.

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import OverlayApp from "../../src/overlay/OverlayApp";

afterEach(cleanup);

type MockHandle = {
  emit: (event: string, payload: unknown) => void;
  calls: () => { command: string; args: Record<string, unknown> | undefined }[];
};

async function showFocusPrompt(): Promise<MockHandle> {
  render(<OverlayApp />);
  // The mock IPC shim attaches window.__mock when OverlayApp's listeners load.
  await waitFor(() => expect((window as unknown as { __mock?: MockHandle }).__mock).toBeDefined());
  const mock = (window as unknown as { __mock: MockHandle }).__mock;
  // Listener registration is async; emit until the pill leaves "hidden".
  await waitFor(() => {
    mock.emit("show-overlay", {
      state: "focus-changed",
      from_app: "Terminal",
      to_app: "Slack",
    });
    expect(screen.getByTestId("overlay-pill")).toHaveAttribute("data-state", "focus-changed");
  });
  return mock;
}

const resolveCalls = (mock: MockHandle) =>
  mock.calls().filter((c) => c.command === "resolve_focus_prompt");

describe("U11: focus-guard overlay state", () => {
  it("renders both app names and both buttons", async () => {
    await showFocusPrompt();
    expect(screen.getByTestId("focus-from")).toHaveTextContent("Terminal");
    expect(screen.getByTestId("focus-to")).toHaveTextContent("Slack");
    expect(screen.getByTestId("focus-paste-btn")).toBeVisible();
    expect(screen.getByTestId("focus-copy-btn")).toBeVisible();
  });

  it("Paste resolves the prompt with action=paste", async () => {
    const mock = await showFocusPrompt();
    const before = resolveCalls(mock).length;
    screen.getByTestId("focus-paste-btn").click();
    await waitFor(() => {
      const calls = resolveCalls(mock);
      expect(calls.length).toBe(before + 1);
      expect(calls[calls.length - 1]?.args?.action).toBe("paste");
    });
  });

  it("the copy-fallback button resolves the prompt with action=copy", async () => {
    const mock = await showFocusPrompt();
    const before = resolveCalls(mock).length;
    screen.getByTestId("focus-copy-btn").click();
    await waitFor(() => {
      const calls = resolveCalls(mock);
      expect(calls.length).toBe(before + 1);
      expect(calls[calls.length - 1]?.args?.action).toBe("copy");
    });
  });
});
