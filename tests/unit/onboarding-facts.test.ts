// U21: per-step snapshot fact plans (SPEC14 FR-S4) — a polling tick
// re-verifies just what the displayed step gates on. The tray window-server
// probe belongs to the menubar step alone, and captureReady (the one
// getAppInfo-backed fact) to the accessibility step alone, so a tick never
// issues more than a single getAppInfo.

import { describe, expect, it } from "vitest";
import { ALL_FACTS, factsForStep, stepsFor, type StepId } from "../../src/lib/onboarding";

const EVERY_STEP: StepId[] = stepsFor("macos");

describe("U21: factsForStep plans", () => {
  it("the menubar step is the sole tray-probe reader", () => {
    for (const step of EVERY_STEP) {
      expect(factsForStep(step).trayVisible, step).toBe(step === "menubar");
    }
  });

  it("captureReady — the getAppInfo-backed fact — is read solely on the accessibility step", () => {
    for (const step of EVERY_STEP) {
      expect(factsForStep(step).captureReady, step).toBe(step === "accessibility");
    }
  });

  it("each step re-reads exactly the grants its gate depends on", () => {
    // welcome's gate treats any grant as proof of a returning user.
    expect(factsForStep("welcome")).toEqual({
      microphone: true,
      accessibility: true,
      captureReady: false,
      trayVisible: false,
    });
    expect(factsForStep("microphone").microphone).toBe(true);
    expect(factsForStep("microphone").accessibility).toBe(false);
    expect(factsForStep("accessibility").accessibility).toBe(true);
    // model + try verify nothing external per tick — their facts are local.
    for (const step of ["model", "try"] as StepId[]) {
      const facts = factsForStep(step);
      expect(facts.microphone || facts.accessibility || facts.captureReady || facts.trayVisible,
        step,
      ).toBe(false);
    }
  });

  it("the full plan reads every fact (mount, navigation, re-show)", () => {
    expect(ALL_FACTS).toEqual({
      microphone: true,
      accessibility: true,
      captureReady: true,
      trayVisible: true,
    });
  });
});
