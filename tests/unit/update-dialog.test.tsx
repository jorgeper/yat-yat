// U23 (SPEC15 §3.3): UpdateDialog honesty — the progress phase ignores
// Escape and backdrop-click (no silent background install), the ready phase
// warns macOS users about the post-restart Accessibility re-key (and no other
// platform sees it), and a rejecting restart() lands in the dismissable
// error phase instead of a dead button.

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import UpdateDialog from "../../src/settings/UpdateDialog";
import type { UpdatesApi } from "../../src/ipc/updates";

afterEach(cleanup);

function fakeUpdates(overrides: Partial<UpdatesApi> = {}): UpdatesApi {
  return {
    check: async () => ({ version: "9.9.9", notes: "Big fixes." }),
    downloadAndInstall: async () => {},
    restart: async () => {},
    ...overrides,
  };
}

function backdrop(container: HTMLElement): Element {
  const el = container.querySelector(".modal-overlay");
  expect(el).not.toBeNull();
  return el!;
}

describe("U23: update-dialog honesty", () => {
  it("u23 progress phase ignores Escape and backdrop-click; other phases close", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const onClose = vi.fn();
    const updates = fakeUpdates({
      downloadAndInstall: async (onProgress) => {
        onProgress(42);
        await gate;
        onProgress(100);
      },
    });
    const { container } = render(
      <UpdateDialog currentVersion="0.1.0" platform="macos" updates={updates} onClose={onClose} />,
    );
    await waitFor(() => expect(screen.getByTestId("update-available")).toBeInTheDocument());

    fireEvent.click(screen.getByTestId("update-install"));
    await waitFor(() => expect(screen.getByTestId("update-progress")).toBeInTheDocument());

    // Mid-download: the dialog is locked open.
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.mouseDown(backdrop(container));
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByTestId("update-dialog")).toBeInTheDocument();

    // Released: the ready phase closes as before (SPEC9 FR-U2).
    release();
    await waitFor(() => expect(screen.getByTestId("update-restart")).toBeInTheDocument());
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("u23 ready phase on macos renders the Accessibility warning", async () => {
    render(
      <UpdateDialog
        currentVersion="0.1.0"
        platform="macos"
        updates={fakeUpdates()}
        onClose={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getByTestId("update-available")).toBeInTheDocument());
    fireEvent.click(screen.getByTestId("update-install"));
    await waitFor(() => expect(screen.getByTestId("update-restart")).toBeInTheDocument());
    expect(screen.getByTestId("update-ax-warning")).toBeInTheDocument();
    expect(screen.getByTestId("update-ax-warning").textContent).toMatch(/Accessibility/);
  });

  it("u23 ready phase on other platforms renders no warning", async () => {
    render(
      <UpdateDialog
        currentVersion="0.1.0"
        platform="windows"
        updates={fakeUpdates()}
        onClose={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getByTestId("update-available")).toBeInTheDocument());
    fireEvent.click(screen.getByTestId("update-install"));
    await waitFor(() => expect(screen.getByTestId("update-restart")).toBeInTheDocument());
    expect(screen.queryByTestId("update-ax-warning")).toBeNull();
  });

  it("u23 rejecting restart() transitions to the error phase with the message", async () => {
    const updates = fakeUpdates({
      restart: async () => {
        throw new Error("relaunch was denied by the OS");
      },
    });
    render(
      <UpdateDialog currentVersion="0.1.0" platform="macos" updates={updates} onClose={() => {}} />,
    );
    await waitFor(() => expect(screen.getByTestId("update-available")).toBeInTheDocument());
    fireEvent.click(screen.getByTestId("update-install"));
    await waitFor(() => expect(screen.getByTestId("update-restart")).toBeInTheDocument());

    fireEvent.click(screen.getByTestId("update-restart"));
    await waitFor(() => expect(screen.getByTestId("update-error")).toBeInTheDocument());
    expect(screen.getByTestId("update-error").textContent).toContain(
      "relaunch was denied by the OS",
    );
  });
});
