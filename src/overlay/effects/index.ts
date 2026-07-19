// The effect registry (SPEC6 FR-A2, SPEC16 FR-E + owner swap): 29 built-ins. Adding an
// effect = one module + one entry here. `classic-bars` is the default; the
// last thirteen are the SPEC16 cinema-mode renderers.

import type { EffectDef } from "./types";
import { createClassicBars } from "./classicBars";
import { createMirrorWave } from "./mirrorWave";
import { createOscilloscope } from "./oscilloscope";
import { createSpectrumBlocks } from "./spectrumBlocks";
import { createParticleFountain } from "./particleFountain";
import { createPulseOrb } from "./pulseOrb";
import { createRadialRings } from "./radialRings";
import { createCometTrail } from "./cometTrail";
import { createRipplePond } from "./ripplePond";
import { createStarfield } from "./starfield";
import { createHeartbeat } from "./heartbeat";
import { createDnaHelix } from "./dnaHelix";
import { createFireflies } from "./fireflies";
import { createGlitchBars } from "./glitchBars";
import { createAurora } from "./aurora";
import { createLarsonBar } from "./larsonBar";
import { createPhosphorTerminal } from "./phosphorTerminal";
import { createLightGrid } from "./lightGrid";
import { createLedCircuits } from "./ledCircuits";
import { createTrackerSweep } from "./trackerSweep";
import { createChromeBlob } from "./chromeBlob";
import { createGlyphRain } from "./glyphRain";
import { createHoloArcs } from "./holoArcs";
import { createEmoteFace } from "./emoteFace";
import { createConsoleLamps } from "./consoleLamps";
import { createElbowPanel } from "./elbowPanel";
import { createBladeSlash } from "./bladeSlash";
import { createNeonDiner } from "./neonDiner";
import { createPixelValley } from "./pixelValley";

export const DEFAULT_EFFECT = "classic-bars";

export const EFFECTS: EffectDef[] = [
  { id: "classic-bars", name: "Classic Bars", create: createClassicBars },
  { id: "mirror-wave", name: "Mirror Wave", create: createMirrorWave },
  { id: "oscilloscope", name: "Oscilloscope", create: createOscilloscope },
  { id: "spectrum-blocks", name: "Spectrum Blocks", create: createSpectrumBlocks },
  { id: "particle-fountain", name: "Particle Fountain", create: createParticleFountain },
  { id: "pulse-orb", name: "Pulse Orb", create: createPulseOrb },
  { id: "radial-rings", name: "Radial Rings", create: createRadialRings },
  { id: "comet-trail", name: "Comet Trail", create: createCometTrail },
  { id: "ripple-pond", name: "Ripple Pond", create: createRipplePond },
  { id: "starfield", name: "Starfield", create: createStarfield },
  { id: "heartbeat", name: "Heartbeat", create: createHeartbeat },
  { id: "dna-helix", name: "DNA Helix", create: createDnaHelix },
  { id: "fireflies", name: "Fireflies", create: createFireflies },
  { id: "glitch-bars", name: "Glitch Bars", create: createGlitchBars },
  { id: "aurora", name: "Aurora", create: createAurora },
  { id: "larson-bar", name: "Larson Bar", create: createLarsonBar },
  { id: "phosphor-terminal", name: "Phosphor Terminal", create: createPhosphorTerminal },
  { id: "light-grid", name: "Light Grid", create: createLightGrid },
  { id: "led-circuits", name: "LED Circuits", create: createLedCircuits },
  { id: "tracker-sweep", name: "Tracker Sweep", create: createTrackerSweep },
  { id: "chrome-blob", name: "Chrome Blob", create: createChromeBlob },
  { id: "glyph-rain", name: "Glyph Rain", create: createGlyphRain },
  { id: "holo-arcs", name: "Holo Arcs", create: createHoloArcs },
  { id: "emote-face", name: "Emote Face", create: createEmoteFace },
  { id: "console-lamps", name: "Console Lamps", create: createConsoleLamps },
  { id: "elbow-panel", name: "Elbow Panel", create: createElbowPanel },
  { id: "blade-slash", name: "Blade Slash", create: createBladeSlash },
  { id: "neon-diner", name: "Neon Diner", create: createNeonDiner },
  { id: "pixel-valley", name: "Pixel Valley", create: createPixelValley },
];

/** Unknown ids fall back to the default — settings resilience. */
export function getEffect(id: string): EffectDef {
  return EFFECTS.find((e) => e.id === id) ?? EFFECTS.find((e) => e.id === DEFAULT_EFFECT)!;
}
