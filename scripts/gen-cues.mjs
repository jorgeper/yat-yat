// Generates the two sound-cue WAVs (SPEC7 FR-C1) with no audio dependencies:
// quiet, short, synthesized sines. Run once; outputs are committed.
//   cue-start.wav      soft tick when recording arms (~70 ms, 880 Hz)
//   cue-delivered.wav  soft two-note click when text lands (~110 ms, 660→990 Hz)
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RATE = 24000;

function wav(samples) {
  const data = Buffer.alloc(samples.length * 2);
  samples.forEach((s, i) => data.writeInt16LE(Math.round(Math.max(-1, Math.min(1, s)) * 32767), i * 2));
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + data.length, 4);
  header.write("WAVE", 8);
  header.write("fmt ", 12);
  header.writeUInt32LE(16, 16); // PCM chunk size
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(1, 22); // mono
  header.writeUInt32LE(RATE, 24);
  header.writeUInt32LE(RATE * 2, 28); // byte rate
  header.writeUInt16LE(2, 32); // block align
  header.writeUInt16LE(16, 34); // bits
  header.write("data", 36);
  header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]);
}

// A single soft note: sine + a whisper of octave, fast attack, smooth decay.
function note(freq, ms, gain) {
  const n = Math.round((RATE * ms) / 1000);
  const out = new Array(n);
  for (let i = 0; i < n; i++) {
    const t = i / RATE;
    const attack = Math.min(1, i / (RATE * 0.004));
    const decay = Math.exp((-5 * i) / n);
    out[i] =
      gain * attack * decay * (Math.sin(2 * Math.PI * freq * t) + 0.25 * Math.sin(4 * Math.PI * freq * t));
  }
  return out;
}

const silence = (ms) => new Array(Math.round((RATE * ms) / 1000)).fill(0);

writeFileSync(join(root, "src-tauri/resources/cue-start.wav"), wav(note(880, 70, 0.18)));
console.log("wrote src-tauri/resources/cue-start.wav");

writeFileSync(
  join(root, "src-tauri/resources/cue-delivered.wav"),
  wav([...note(660, 45, 0.16), ...silence(12), ...note(990, 55, 0.14)]),
);
console.log("wrote src-tauri/resources/cue-delivered.wav");
