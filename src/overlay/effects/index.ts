// The effect registry (owner-requested 2026-11-20, replacing SPEC6 FR-A2 and
// SPEC16 FR-E): 7 built-ins. Adding an effect = one module + one entry here.
// Every effect works with every color — there are no fixed pairings.

import type { EffectDef } from "./types";
import { createRibbon } from "./ribbon";
import { createFilaments } from "./filaments";
import { createTrace } from "./trace";
import { createHalftone } from "./halftone";
import { createStardust } from "./stardust";
import { createMesh } from "./mesh";
import { createConstellation } from "./constellation";

export const DEFAULT_EFFECT = "ribbon";

export const EFFECTS: EffectDef[] = [
  { id: "ribbon", name: "Ribbon", create: createRibbon },
  { id: "filaments", name: "Filaments", create: createFilaments },
  { id: "trace", name: "Trace", create: createTrace },
  { id: "halftone", name: "Halftone", create: createHalftone },
  { id: "stardust", name: "Stardust", create: createStardust },
  { id: "mesh", name: "Mesh", create: createMesh },
  { id: "constellation", name: "Constellation", create: createConstellation },
];

/** Unknown ids fall back to the default — settings resilience. */
export function getEffect(id: string): EffectDef {
  return EFFECTS.find((e) => e.id === id) ?? EFFECTS.find((e) => e.id === DEFAULT_EFFECT)!;
}
