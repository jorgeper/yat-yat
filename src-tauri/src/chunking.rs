//! Chunked transcription while recording: pure pause-finding and transcript
//! assembly.
//!
//! Parakeet's cost grows faster than the audio it is given (measured on an
//! M5, release: 35 s → 0.95 s, 70 s → 2.1 s, 140 s → 5.1 s), and the stop
//! path used to hand it the whole recording at once — so the wait between
//! "stop" and the paste grew with every second spoken. Instead, while the
//! user is still talking, a background thread (pipeline.rs::chunk_loop)
//! transcribes finished stretches of speech, cut at pauses; the stop path
//! then only owes the tail after the last cut.
//!
//! A chunk transcribed on its own reads worse at its edges: the engine
//! capitalizes its first word and ends it with a period whether or not a
//! sentence really starts or stops there (measured on real dictation:
//! "…ask them to Review the draft. So that we…"). So each chunk is handed
//! to the engine with context — a little audio before it and after it — so
//! consecutive passes overlap around each cut. The passes are then stitched
//! on the words they agree on inside that overlap ([`stitch`]), not on
//! timestamps alone: word timings shift by a few hundred ms between passes,
//! and a purely time-based seam was measured to drop and duplicate words.
//!
//! Everything here is pure so the rules are table-testable (R28–R32) and
//! the CLI (`transcribe --chunked`) can replay them over a WAV headlessly.

use crate::stt::Word;

/// 16 kHz mono throughout (audio.rs::TARGET_RATE).
const RATE: usize = 16_000;
/// Energy frame: 20 ms.
const FRAME: usize = RATE / 50;

/// Never cut a chunk shorter than this — short chunks starve the engine of
/// context and cost accuracy for no latency win.
pub const MIN_CHUNK_SECS: f32 = 4.0;
/// A pause this long is very likely a sentence boundary: cut there.
const PAUSE_MS: usize = 500;
/// Once this much audio is pending, a shorter breath pause is acceptable…
const RELAXED_AFTER_SECS: f32 = 15.0;
const RELAXED_PAUSE_MS: usize = 250;
/// …and past this, cut at the quietest point rather than let the tail (and
/// the stop-path wait) grow without bound.
pub const FORCE_AFTER_SECS: f32 = 30.0;
const FORCE_WINDOW_MS: usize = 200;

/// Audio handed to the engine before a chunk, so it knows whether the
/// chunk's first word starts a sentence.
pub const CONTEXT_BEFORE_SECS: f32 = 2.0;
/// Audio handed to the engine after a chunk, so it knows whether the chunk's
/// last word ends one. A cut is only taken once this much audio exists
/// beyond it.
pub const CONTEXT_AFTER_SECS: f32 = 1.5;

/// Frames quieter than this fraction of the stretch's loud level are silence.
const SILENCE_RATIO: f32 = 0.15;
/// …but never treat anything above this absolute RMS floor as speech-free
/// just because the stretch is quiet overall, and vice versa.
const SILENCE_FLOOR: f32 = 0.002;

fn frame_rms(samples: &[f32]) -> Vec<f32> {
    samples
        .chunks_exact(FRAME)
        .map(|f| (f.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt())
        .collect()
}

fn frames(ms: usize) -> usize {
    (ms * RATE / 1000) / FRAME
}

/// Where to cut `pending` (the audio captured since the last cut), as a
/// sample index into it — or None to keep waiting.
///
/// Rules, in order: nothing under [`MIN_CHUNK_SECS`]; the LONGEST pause of
/// at least 500 ms (250 ms once 15 s are pending) that leaves a long-enough
/// chunk, cut in the middle of it — the later one on a tie; and past 30 s
/// with no such pause, the quietest 200 ms after the first 10 s. The longest
/// pause is the likeliest sentence boundary: a cut at a mere comma pause
/// makes the engine punctuate the chunk's last words as a sentence end.
pub fn find_cut(pending: &[f32]) -> Option<usize> {
    let min_chunk = (MIN_CHUNK_SECS * RATE as f32) as usize;
    if pending.len() < min_chunk {
        return None;
    }
    let rms = frame_rms(pending);
    if rms.is_empty() {
        return None;
    }
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let loud = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
    let threshold = (loud * SILENCE_RATIO).max(SILENCE_FLOOR);

    let pending_secs = pending.len() as f32 / RATE as f32;
    let need = if pending_secs >= RELAXED_AFTER_SECS {
        frames(RELAXED_PAUSE_MS)
    } else {
        frames(PAUSE_MS)
    };
    let min_frames = min_chunk / FRAME;

    // Scan silence runs; remember the longest qualifying one (later on a tie).
    let mut best: Option<(usize, usize)> = None; // (run length, cut frame)
    let mut run_start: Option<usize> = None;
    for i in 0..=rms.len() {
        let silent = i < rms.len() && rms[i] < threshold;
        match (silent, run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(start)) => {
                let mid = start + (i - start) / 2;
                let len = i - start;
                if len >= need && mid >= min_frames && best.map_or(true, |(l, _)| len >= l) {
                    best = Some((len, mid));
                }
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some((_, mid)) = best {
        return Some(mid * FRAME);
    }

    if pending_secs < FORCE_AFTER_SECS {
        return None;
    }
    // No usable pause in 30 s of continuous speech: cut at the quietest
    // 200 ms window after the first 10 s.
    let window = frames(FORCE_WINDOW_MS).max(1);
    let from = frames(10_000);
    if rms.len() < from + window {
        return None;
    }
    let mut quietest = (f32::MAX, from);
    for start in from..=rms.len() - window {
        let energy: f32 = rms[start..start + window].iter().sum();
        if energy < quietest.0 {
            quietest = (energy, start);
        }
    }
    Some((quietest.1 + window / 2) * FRAME)
}

/// Where the engine's view of the next chunk starts: the last cut, minus the
/// context before it.
pub fn context_start(committed: usize) -> usize {
    committed.saturating_sub((CONTEXT_BEFORE_SECS * RATE as f32) as usize)
}

/// A reading of the audio not yet committed, taken while the user was still
/// recording: a live preview pass, or a pass run in a pause. If nothing was
/// said after it, the stop path uses it as the tail instead of transcribing
/// that audio again ([`reading_covers`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TailReading {
    /// The cut in force when the reading was taken. A later cut voids it.
    pub committed: usize,
    /// Sample index (absolute) the reading's audio ran to.
    pub end: usize,
    /// What it heard, in absolute recording time — context before the cut
    /// included, exactly what a tail pass hands to [`stitch`].
    pub words: Vec<Word>,
}

/// A reading must have ended at least this far into a pause to be trusted
/// with the last word: the engine saw the word finish, with silence after it.
const READING_GUARD_MS: usize = 200;
/// Loud frames tolerated in audio that otherwise counts as quiet: the
/// click of the stop key, not a word (3 frames = 60 ms).
const BLIP_FRAMES: usize = 3;
/// With live previews off, a pause this long triggers a reading of the
/// uncommitted audio, in case the user is about to stop.
pub const SPECULATE_PAUSE_MS: usize = 300;

/// Whether `window` holds no speech from sample `from` on. `window` is the
/// audio from [`context_start`] of the last cut — it sets what "loud" means,
/// by the same rule as [`find_cut`]. A few loud frames are let through
/// ([`BLIP_FRAMES`]).
pub fn quiet_from(window: &[f32], from: usize) -> bool {
    let rms = frame_rms(window);
    if rms.is_empty() {
        return false;
    }
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let loud = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
    let threshold = (loud * SILENCE_RATIO).max(SILENCE_FLOOR);
    let first = (from / FRAME).min(rms.len());
    rms[first..].iter().filter(|&&r| r >= threshold).count() <= BLIP_FRAMES
}

/// Whether `reading` already says everything in the recording: nothing was
/// spoken from shortly before it ended ([`READING_GUARD_MS`]) to the end of
/// `samples`, and no cut was taken since.
pub fn reading_covers(samples: &[f32], committed: usize, reading: &TailReading) -> bool {
    if reading.committed != committed || reading.end > samples.len() {
        return false;
    }
    let start = context_start(committed).min(samples.len());
    let guard = READING_GUARD_MS * RATE / 1000;
    quiet_from(&samples[start..], reading.end.saturating_sub(guard).saturating_sub(start))
}

/// Whether the audio so far ends in a pause long enough to be worth a
/// speculative reading ([`SPECULATE_PAUSE_MS`]).
pub fn ends_in_pause(window: &[f32]) -> bool {
    let pause = SPECULATE_PAUSE_MS * RATE / 1000;
    window.len() > pause && quiet_from(window, window.len() - pause)
}

/// A dictation left running with nothing said for this long stops by itself
/// and delivers what it has: a forgotten recording must not hold the
/// microphone open until the length cap.
pub const AUTO_STOP_SILENCE_SECS: usize = 30;
/// Audio kept after the last word when a dictation stops on silence.
const KEEP_AFTER_SPEECH_SECS: usize = 1;
/// The loud level needs at least this much audio before the silent stretch.
const SILENCE_REFERENCE_SECS: usize = 1;

/// Watches a recording for a long silence at its end ([`AUTO_STOP_SILENCE_SECS`]).
/// Fed the audio as it arrives; keeps one energy value per 20 ms frame, so a
/// 15-minute recording costs 45,000 numbers.
#[derive(Debug, Default)]
pub struct SilenceWatch {
    rms: Vec<f32>,
    /// Samples short of a whole frame, waiting for the next push.
    carry: Vec<f32>,
}

impl SilenceWatch {
    pub fn push(&mut self, samples: &[f32]) {
        self.carry.extend_from_slice(samples);
        let whole = self.carry.len() / FRAME * FRAME;
        self.rms.extend(frame_rms(&self.carry[..whole]));
        self.carry.drain(..whole);
    }

    /// Samples seen so far.
    pub fn len(&self) -> usize {
        self.rms.len() * FRAME + self.carry.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// When the recording ends in [`AUTO_STOP_SILENCE_SECS`] of silence: the
    /// sample to trim it to, shortly after the last sound.
    ///
    /// "Loud" is set by the audio BEFORE the silent stretch, by the same
    /// rule as [`find_cut`] — so room noise under speech counts as silence,
    /// while a recording that has only ever held steady noise never
    /// qualifies (nothing says that noise is not the speaker). A few loud
    /// frames in the stretch are let through ([`BLIP_FRAMES`]).
    pub fn silent_tail(&self) -> Option<usize> {
        let window = AUTO_STOP_SILENCE_SECS * RATE / FRAME;
        let before = self.rms.len().checked_sub(window)?;
        if before < SILENCE_REFERENCE_SECS * RATE / FRAME {
            return None;
        }
        let mut sorted = self.rms[..before].to_vec();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let loud = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
        let threshold = (loud * SILENCE_RATIO).max(SILENCE_FLOOR);
        if self.rms[before..].iter().filter(|&&r| r >= threshold).count() > BLIP_FRAMES {
            return None;
        }
        let last_sound = self.rms[..before]
            .iter()
            .rposition(|&r| r >= threshold)
            .map_or(0, |i| i + 1);
        Some(last_sound * FRAME + KEEP_AFTER_SPEECH_SECS * RATE)
    }
}

/// One chunk ready to transcribe, as sample indices into the buffer given to
/// [`next_chunk`]: hand the engine `[..end]`, keep the words inside
/// `[keep_from..keep_to]`. `keep_to` is the new cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    pub keep_from: usize,
    pub keep_to: usize,
    pub end: usize,
}

/// The next chunk to transcribe, if the audio so far holds one. `buf` is
/// everything captured from [`context_start`] of the last cut onward, and
/// `lead` is how much of its start is that context (already transcribed).
/// A cut is only taken when [`CONTEXT_AFTER_SECS`] of audio exists past it.
pub fn next_chunk(buf: &[f32], lead: usize) -> Option<Chunk> {
    let after = (CONTEXT_AFTER_SECS * RATE as f32) as usize;
    let pending = buf.get(lead..)?;
    let searchable = pending.len().checked_sub(after)?;
    let cut = find_cut(&pending[..searchable])?;
    Some(Chunk {
        keep_from: lead,
        keep_to: lead + cut,
        end: lead + cut + after,
    })
}

/// Letters and digits only, lowercased — how two passes' words are compared.
fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Where a word sits in time: its start. Its end is no use — the engine
/// reports a word as lasting until the next one begins, so the last word
/// before a pause "ends" seconds later, on the far side of the cut.
fn at(word: &Word) -> f32 {
    word.start
}

/// Two passes heard the same word only if they also place it at about the
/// same time — otherwise repeated words ("one two three", "it will… it
/// will") would match across the wrong occurrences.
const SAME_WORD_SECS: f32 = 1.0;

/// Append the next pass's words to the transcript so far. Both are in
/// absolute recording time; `seam_secs` is the cut between them. `acc` runs
/// past the cut (its pass had context after it) and `next` starts before it
/// (context before), so around the seam both passes transcribed the same
/// audio.
///
/// The two passes are aligned on the words they agree on in that overlap
/// (longest common subsequence). A word both heard is kept once. A word
/// only one pass heard is kept if that pass is the authority for its side
/// of the cut: `acc` before the cut, `next` after it — each word comes from
/// the pass that saw the most audio around it. With nothing in common this
/// degrades to splitting at the cut by word start time.
pub fn stitch(acc: &mut Vec<Word>, next: Vec<Word>, seam_secs: f32) {
    let lo = seam_secs - CONTEXT_BEFORE_SECS;
    let hi = seam_secs + CONTEXT_AFTER_SECS;
    let a_from = acc.iter().position(|w| at(w) >= lo).unwrap_or(acc.len());
    let b_to = next.iter().position(|w| at(w) >= hi).unwrap_or(next.len());
    let tail = acc.split_off(a_from);
    let a: Vec<String> = tail.iter().map(|w| normalized(&w.text)).collect();
    let b: Vec<String> = next[..b_to].iter().map(|w| normalized(&w.text)).collect();
    let same = |i: usize, j: usize| {
        !a[i].is_empty() && a[i] == b[j] && (at(&tail[i]) - at(&next[j])).abs() < SAME_WORD_SECS
    };

    // Longest common subsequence, then walk it back into matched pairs.
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if same(i, j) {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if same(i, j) && lcs[i][j] == lcs[i + 1][j + 1] + 1 {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs.push((a.len(), b_to)); // sentinel: flush what follows the last match

    let before = |w: &Word| at(w) < seam_secs;
    let (mut ia, mut jb) = (0, 0);
    for (i, j) in pairs {
        // Words only one pass heard, between two agreed words.
        acc.extend(tail[ia..i].iter().filter(|w| before(w)).cloned());
        acc.extend(next[jb..j].iter().filter(|w| !before(w)).cloned());
        // The agreed word itself, from the pass that owns its side.
        if i < a.len() {
            let word = if before(&tail[i]) { &tail[i] } else { &next[j] };
            acc.push(word.clone());
        }
        ia = i + 1;
        jb = j + 1;
    }
    acc.extend(next.into_iter().skip(b_to));
}

/// The transcript text of stitched words.
pub fn words_text(words: &[Word]) -> String {
    let parts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    join_texts(&parts)
}

/// Shift a pass's word times from its own window to absolute recording
/// time (`window_start` in samples).
pub fn to_absolute(mut words: Vec<Word>, window_start: usize) -> Vec<Word> {
    let offset = window_start as f32 / RATE as f32;
    for word in &mut words {
        word.start += offset;
        word.end += offset;
    }
    words
}

/// Shortest stretch of audio worth a live preview pass: under a second the
/// engine produces noise (the same floor the whole-buffer live loop uses).
pub const PREVIEW_MIN_SAMPLES: usize = RATE;

/// The text to show while recording: what is already committed plus a
/// tentative reading of the audio after the last cut (`tail`, absolute
/// times, from a pass over the context window). `committed` is not
/// touched — the tail is re-read on every preview and only a real cut
/// commits words.
pub fn preview_text(committed: &[Word], tail: Vec<Word>, seam_secs: f32) -> String {
    if committed.is_empty() {
        return words_text(&tail);
    }
    let mut all = committed.to_vec();
    stitch(&mut all, tail, seam_secs);
    words_text(&all)
}

/// Filters live previews down to what can be trusted on screen.
///
/// A preview's window ends wherever the speaker happens to be, usually
/// mid-phrase, and the engine papers over the cut: it invents an ending
/// ("…we can't be able to do it.") that the next preview takes back. Shown
/// raw, the text grows, shrinks and regrows. So a preview is shown only up
/// to the last word the previous preview also had (in order) — invented
/// endings almost never repeat — and committed words always are. A word
/// the engine keeps re-reading mid-phrase does not hold back what follows.
#[derive(Debug, Default)]
pub struct PreviewFilter {
    /// The previous preview, as normalized words.
    last: Vec<String>,
}

impl PreviewFilter {
    /// Word identity for agreement: case and punctuation do not count.
    fn norm(word: &str) -> String {
        word.chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'')
            .flat_map(char::to_lowercase)
            .collect()
    }

    /// Length of the part of `cur` that `prev` confirms: up to and including
    /// the last word of `cur` on a longest common subsequence with `prev`.
    fn agreed_len(prev: &[String], cur: &[String]) -> usize {
        let prefix = prev.iter().zip(cur).take_while(|(a, b)| a == b).count();
        let (a, b) = (&prev[prefix..], &cur[prefix..]);
        // Previews differ near their ends; past the shared start the two
        // remainders are short. If they are not, the prefix is the answer.
        if a.is_empty() || b.is_empty() || a.len() * b.len() > 250_000 {
            return prefix;
        }
        // lcs[i][j] = LCS length of a[i..] and b[j..].
        let mut lcs = vec![vec![0u16; b.len() + 1]; a.len() + 1];
        for i in (0..a.len()).rev() {
            for j in (0..b.len()).rev() {
                lcs[i][j] = if a[i] == b[j] && !a[i].is_empty() {
                    lcs[i + 1][j + 1] + 1
                } else {
                    lcs[i + 1][j].max(lcs[i][j + 1])
                };
            }
        }
        // Walk one LCS, remembering the last word of `cur` on it.
        let (mut i, mut j, mut last) = (0, 0, None);
        while i < a.len() && j < b.len() {
            if a[i] == b[j] && !a[i].is_empty() {
                last = Some(j);
                i += 1;
                j += 1;
            } else if lcs[i + 1][j] >= lcs[i][j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
        prefix + last.map_or(0, |j| j + 1)
    }

    /// The part of `preview` (a full [`preview_text`]) to show, given that
    /// its first `committed_words` words are final.
    pub fn confirmed(&mut self, preview: &str, committed_words: usize) -> String {
        let words: Vec<&str> = preview.split_whitespace().collect();
        let normed: Vec<String> = words.iter().map(|w| Self::norm(w)).collect();
        let agreed = Self::agreed_len(&self.last, &normed);
        self.last = normed;
        let shown = agreed.max(committed_words).min(words.len());
        let mut text = words[..shown].join(" ");
        // The last word of a whole preview carries the invented ending's
        // punctuation; mid-preview punctuation was written in context.
        if shown == words.len() && shown > committed_words {
            let trimmed = text.trim_end_matches(['.', '?', '!', ',']).len();
            text.truncate(trimmed);
        }
        text
    }
}

/// How many of `words` are final for a cut at `committed` samples: those
/// that start before it. (The list also holds the words just after the cut,
/// from the last pass's context — the next pass re-reads those.)
pub fn final_word_count(words: &[Word], committed: usize) -> usize {
    let seam = committed as f32 / RATE as f32;
    words.iter().filter(|w| w.start < seam).count()
}

/// Replay the live cut rules over a finished recording: the cuts a chunker
/// polling every `poll_secs` would have made, assuming each chunk's
/// transcription finished before the next poll. Returns absolute sample
/// indices, ascending; the audio after the last one is the stop-path tail.
pub fn plan_cuts(samples: &[f32], poll_secs: f32) -> Vec<usize> {
    let step = ((poll_secs * RATE as f32) as usize).max(FRAME);
    let mut cuts = Vec::new();
    let mut committed = 0;
    let mut seen = step;
    while seen < samples.len() {
        let start = context_start(committed);
        if let Some(chunk) = next_chunk(&samples[start..seen], committed - start) {
            committed = start + chunk.keep_to;
            cuts.push(committed);
        }
        seen += step;
    }
    cuts
}

/// Join per-chunk raw transcripts (in order) into one raw transcript for the
/// cleanup pass. Empty chunks — a stretch of pure silence — vanish.
pub fn join_texts<S: AsRef<str>>(parts: &[S]) -> String {
    parts
        .iter()
        .map(|p| p.as_ref().trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic "speech": a 220 Hz tone at a speech-like level.
    fn speech(secs: f32) -> Vec<f32> {
        (0..(secs * RATE as f32) as usize)
            .map(|i| 0.2 * (i as f32 * 220.0 * std::f32::consts::TAU / RATE as f32).sin())
            .collect()
    }

    fn silence(secs: f32) -> Vec<f32> {
        // A faint noise floor, not digital zero — real rooms are never silent.
        (0..(secs * RATE as f32) as usize)
            .map(|i| 0.001 * ((i * 7919 % 13) as f32 / 13.0 - 0.5))
            .collect()
    }

    fn cat(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.concat()
    }

    fn secs(samples: usize) -> f32 {
        samples as f32 / RATE as f32
    }

    // R48: a dictation stops by itself after a long silence, and only then.
    #[test]
    fn r48_a_long_silence_ends_the_dictation() {
        let watch = |audio: &[f32]| {
            let mut w = SilenceWatch::default();
            w.push(audio);
            w
        };
        let window = AUTO_STOP_SILENCE_SECS as f32;

        // Speech, then silence: nothing until the full window has passed.
        let spoken = cat(&[speech(5.0), silence(window - 1.0)]);
        assert_eq!(watch(&spoken).silent_tail(), None);
        // …then it stops, trimmed to one second after the last word.
        let done = cat(&[speech(5.0), silence(window)]);
        assert_eq!(watch(&done).silent_tail(), Some(6 * RATE));

        // A word anywhere in the window keeps the dictation open.
        let resumed = cat(&[speech(5.0), silence(20.0), speech(0.5), silence(window - 20.0)]);
        assert_eq!(watch(&resumed).silent_tail(), None);
        // A click (40 ms) does not.
        let click = cat(&[speech(5.0), silence(20.0), speech(0.04), silence(window - 20.0)]);
        assert_eq!(watch(&click).silent_tail(), Some(6 * RATE));

        // Steady noise from the start is never called silence: with nothing
        // louder to compare against it may be the speaker.
        let hum: Vec<f32> = silence(window + 10.0).iter().map(|s| s * 20.0).collect();
        assert_eq!(watch(&hum).silent_tail(), None);
        // Too little audio before the window to know what loud is.
        assert_eq!(watch(&silence(window + 0.5)).silent_tail(), None);

        // Fed in uneven pieces, as the recorder delivers it: same answer.
        let mut pieces = SilenceWatch::default();
        for piece in done.chunks(1_237) {
            pieces.push(piece);
        }
        assert_eq!(pieces.len(), done.len());
        assert_eq!(pieces.silent_tail(), Some(6 * RATE));
    }

    // R46: when a reading taken while recording may stand in for the tail.
    #[test]
    fn r46_reading_covers_only_when_nothing_was_said_after_it() {
        let at = |secs: f32| (secs * RATE as f32) as usize;
        let reading = |committed: usize, end: f32| TailReading {
            committed,
            end: at(end),
            words: Vec::new(),
        };
        // 3 s of speech, then the user falls silent and stops 1 s later.
        let audio = cat(&[speech(3.0), silence(1.0)]);
        // Read 0.5 s into the pause: it heard everything.
        assert!(reading_covers(&audio, 0, &reading(0, 3.5)));
        // Read only 0.1 s into the pause: the last word may be cut short.
        assert!(!reading_covers(&audio, 0, &reading(0, 3.1)));
        // Read while still speaking.
        assert!(!reading_covers(&audio, 0, &reading(0, 2.5)));

        // More speech after the reading: it is stale.
        let more = cat(&[speech(3.0), silence(1.0), speech(0.5), silence(0.3)]);
        assert!(!reading_covers(&more, 0, &reading(0, 3.5)));

        // The click of the stop key (40 ms) is not speech…
        let click = cat(&[speech(3.0), silence(0.8), speech(0.04), silence(0.1)]);
        assert!(reading_covers(&click, 0, &reading(0, 3.5)));
        // …but a short word (200 ms) is.
        let word = cat(&[speech(3.0), silence(0.8), speech(0.2), silence(0.1)]);
        assert!(!reading_covers(&word, 0, &reading(0, 3.5)));

        // A cut taken after the reading voids it, as does a reading that
        // claims more audio than was recorded.
        assert!(!reading_covers(&audio, at(1.0), &reading(0, 3.5)));
        assert!(!reading_covers(&audio, 0, &reading(0, 9.0)));

        // After a cut, "loud" is judged on the audio since the cut's context.
        let long = cat(&[speech(8.0), silence(0.6), speech(3.0), silence(1.0)]);
        assert!(reading_covers(&long, at(8.3), &reading(at(8.3), 12.1)));
        assert!(!reading_covers(&long, at(8.3), &reading(at(8.3), 11.0)));
    }

    // R47: when a pause is worth a speculative reading (live previews off).
    #[test]
    fn r47_a_pause_at_the_end_triggers_a_reading() {
        assert!(!ends_in_pause(&speech(3.0)), "still speaking");
        assert!(!ends_in_pause(&cat(&[speech(3.0), silence(0.2)])), "a breath, not a pause");
        assert!(ends_in_pause(&cat(&[speech(3.0), silence(0.4)])));
        assert!(!ends_in_pause(&silence(0.2)), "too little audio to judge");
        // Speech resumed: wait for the next pause.
        assert!(!ends_in_pause(&cat(&[speech(3.0), silence(0.4), speech(0.5)])));
    }

    // R28: the pause rules — where a chunk is cut, and when it is not.
    #[test]
    fn r28_cuts_in_the_middle_of_a_sentence_pause() {
        let audio = cat(&[speech(6.0), silence(0.6), speech(2.0)]);
        let cut = find_cut(&audio).expect("a 600 ms pause after 6 s must cut");
        assert!(
            (secs(cut) - 6.3).abs() < 0.05,
            "cut lands mid-pause, got {:.2}s",
            secs(cut)
        );
    }

    #[test]
    fn r28_waits_when_there_is_too_little_audio_or_no_pause() {
        assert_eq!(find_cut(&speech(3.0)), None, "under the minimum chunk");
        assert_eq!(find_cut(&speech(12.0)), None, "continuous speech: keep waiting");
        // A pause that would leave a chunk shorter than the minimum is skipped.
        let early = cat(&[speech(2.0), silence(0.6), speech(4.0)]);
        assert_eq!(find_cut(&early), None);
        // Word gaps (well under 500 ms) are not sentence pauses.
        let gaps = cat(&[speech(5.0), silence(0.2), speech(5.0)]);
        assert_eq!(find_cut(&gaps), None);
    }

    #[test]
    fn r28_prefers_the_longest_pause_and_the_later_one_on_a_tie() {
        // A sentence pause (900 ms) then a comma pause (550 ms): cut at the
        // sentence, even though the comma is later.
        let audio = cat(&[
            speech(5.0),
            silence(0.9),
            speech(5.0),
            silence(0.55),
            speech(1.0),
        ]);
        let cut = find_cut(&audio).unwrap();
        assert!(
            (secs(cut) - 5.45).abs() < 0.05,
            "the longer pause is the sentence boundary, got {:.2}s",
            secs(cut)
        );
        // Equal pauses: the later one keeps the stop-path tail short.
        let tie = cat(&[
            speech(5.0),
            silence(0.6),
            speech(5.0),
            silence(0.6),
            speech(1.0),
        ]);
        let cut = find_cut(&tie).unwrap();
        assert!((secs(cut) - 10.9).abs() < 0.05, "got {:.2}s", secs(cut));
    }

    #[test]
    fn r28_a_pause_still_in_progress_counts() {
        // The user is pausing right now: the run reaches the buffer's end.
        let audio = cat(&[speech(6.0), silence(0.7)]);
        let cut = find_cut(&audio).unwrap();
        assert!(secs(cut) > 6.0 && secs(cut) < 6.7);
    }

    // R29: long unbroken speech degrades gracefully instead of growing the
    // stop-path tail forever.
    #[test]
    fn r29_relaxes_to_breath_pauses_after_fifteen_seconds() {
        let audio = cat(&[speech(16.0), silence(0.3), speech(1.0)]);
        let cut = find_cut(&audio).expect("300 ms is enough once 15 s are pending");
        assert!((secs(cut) - 16.15).abs() < 0.05, "got {:.2}s", secs(cut));
        // The same breath pause under 15 s does not cut.
        let short = cat(&[speech(8.0), silence(0.3), speech(1.0)]);
        assert_eq!(find_cut(&short), None);
    }

    #[test]
    fn r29_forces_a_cut_at_the_quietest_point_after_thirty_seconds() {
        // 31 s with no pause at all, but a quieter 300 ms dip at 20 s — too
        // loud to count as silence (30% of the level, threshold is 15%).
        let mut audio = speech(31.0);
        let dip = (20.0 * RATE as f32) as usize..(20.3 * RATE as f32) as usize;
        for s in &mut audio[dip] {
            *s *= 0.3;
        }
        let cut = find_cut(&audio).expect("past 30 s a cut is forced");
        assert!(
            (secs(cut) - 20.15).abs() < 0.2,
            "forced cut lands in the dip, got {:.2}s",
            secs(cut)
        );
        assert_eq!(find_cut(&speech(29.0)), None, "not forced before 30 s");
    }

    // R30: replaying the rules over a whole recording, and assembling text.
    #[test]
    fn r30_plan_cuts_leaves_only_a_short_tail() {
        // Four 6 s sentences separated by 600 ms pauses, then a 2 s tail.
        let mut parts = Vec::new();
        for _ in 0..4 {
            parts.push(speech(6.0));
            parts.push(silence(0.6));
        }
        parts.push(speech(2.0));
        let audio = cat(&parts);
        let cuts = plan_cuts(&audio, 0.25);
        assert_eq!(cuts.len(), 4, "one cut per sentence pause: {cuts:?}");
        assert!(cuts.windows(2).all(|w| w[0] < w[1]), "ascending");
        let tail = secs(audio.len() - cuts[cuts.len() - 1]);
        assert!(tail < 2.5, "stop path owes only the last sentence, got {tail:.2}s");
        // Every chunk respects the minimum length.
        let mut prev = 0;
        for &cut in &cuts {
            assert!(secs(cut - prev) >= MIN_CHUNK_SECS);
            prev = cut;
        }
    }

    #[test]
    fn r30_plan_cuts_on_short_or_pauseless_audio_is_empty() {
        assert!(plan_cuts(&speech(3.0), 0.25).is_empty());
        assert!(plan_cuts(&speech(20.0), 0.25).is_empty());
        assert!(plan_cuts(&[], 0.25).is_empty());
    }

    // R32: context around a chunk — when a cut is allowed, and which of the
    // engine's words belong to the chunk.
    #[test]
    fn r32_next_chunk_waits_for_audio_after_the_cut() {
        let lead = (CONTEXT_BEFORE_SECS * RATE as f32) as usize;
        // 2 s of context, a 6 s sentence, a 600 ms pause, then only 0.5 s
        // more: the pause is there, but not enough audio after it yet.
        let early = cat(&[speech(2.0), speech(6.0), silence(0.6), speech(0.5)]);
        assert_eq!(next_chunk(&early, lead), None);
        // Two more seconds of speech: now the cut is taken.
        let later = cat(&[early.clone(), speech(2.0)]);
        let chunk = next_chunk(&later, lead).expect("enough audio after the pause");
        assert_eq!(chunk.keep_from, lead);
        assert!(
            (secs(chunk.keep_to) - 8.3).abs() < 0.05,
            "cut mid-pause, got {:.2}s",
            secs(chunk.keep_to)
        );
        assert!(
            (secs(chunk.end - chunk.keep_to) - CONTEXT_AFTER_SECS).abs() < 0.01,
            "the engine gets the context after the cut"
        );
        assert!(chunk.end <= later.len());
        // A lead past the buffer is None, not a panic.
        assert_eq!(next_chunk(&later, later.len() + 1), None);
    }

    #[test]
    fn r32_context_start_never_underflows() {
        assert_eq!(context_start(0), 0);
        assert_eq!(context_start(RATE), 0);
        assert_eq!(context_start(10 * RATE), 8 * RATE);
    }

    // R33: stitching two overlapping passes into one transcript.
    fn words(spec: &[(f32, &str)]) -> Vec<Word> {
        spec.iter()
            .map(|&(start, text)| Word {
                start,
                end: start + 0.3,
                text: text.into(),
            })
            .collect()
    }

    fn text(words: &[Word]) -> String {
        words_text(words)
    }

    #[test]
    fn r33_stitch_joins_on_the_words_both_passes_agree_on() {
        // Cut at 10 s. The first pass ran 1.5 s past it; the second started
        // 2 s before it. Standing alone, the first pass would end its last
        // word with a period and the second would capitalize its first.
        let mut acc = words(&[
            (7.0, "I"),
            (7.4, "want"),
            (8.2, "you"),
            (8.6, "to"),
            (9.0, "review"),
            (10.4, "drafts"),
            (10.9, "so."),
        ]);
        let next = words(&[
            (8.3, "You"),
            (8.7, "to"),
            (9.1, "review"),
            (10.5, "drafts"),
            (11.0, "so"),
            (11.4, "that"),
            (11.8, "it"),
            (12.2, "works."),
        ]);
        stitch(&mut acc, next, 10.0);
        assert_eq!(
            text(&acc),
            "I want you to review drafts so that it works.",
            "no duplicated or dropped words, and neither pass's edge artifact survives"
        );
    }

    // R42: the live preview — committed words plus a tentative reading of
    // the audio after the last cut, without committing anything.
    #[test]
    fn r42_preview_adds_the_tail_without_touching_committed_words() {
        // Nothing committed yet: the preview is just the pass.
        assert_eq!(
            preview_text(&[], words(&[(0.2, "Hello"), (0.6, "there.")]), 0.0),
            "Hello there."
        );

        let committed = words(&[
            (7.0, "I"),
            (7.4, "want"),
            (8.2, "you"),
            (8.6, "to"),
            (9.0, "review"),
        ]);
        let before = committed.clone();
        // The tail pass starts 2 s before the cut at 10 s, as every chunk does.
        let tail = words(&[
            (8.3, "You"),
            (8.7, "to"),
            (9.1, "review"),
            (10.5, "drafts"),
            (11.0, "so"),
        ]);
        assert_eq!(
            preview_text(&committed, tail, 10.0),
            "I want you to review drafts so"
        );
        assert_eq!(committed, before, "a preview commits nothing");
        // A later preview of a longer tail replaces the earlier one cleanly.
        let longer = words(&[
            (8.3, "You"),
            (8.7, "to"),
            (9.1, "review"),
            (10.5, "drafts"),
            (11.0, "so"),
            (11.4, "that"),
            (11.8, "it"),
            (12.2, "works."),
        ]);
        assert_eq!(
            preview_text(&committed, longer, 10.0),
            "I want you to review drafts so that it works."
        );
        // Silence after the cut: the committed text stands.
        assert_eq!(preview_text(&committed, Vec::new(), 10.0), "I want you to review");
    }

    // R43: only words two consecutive previews agree on reach the screen;
    // an invented ending never does, and committed words always do.
    #[test]
    fn r43_preview_filter_shows_only_what_two_previews_agree_on() {
        let mut filter = PreviewFilter::default();
        // First preview: nothing to agree with yet.
        assert_eq!(filter.confirmed("We should rewrite the report.", 0), "");
        // The engine takes "report." back; the agreed part shows.
        assert_eq!(
            filter.confirmed("We should rewrite the onboarding copy.", 0),
            "We should rewrite the"
        );
        // An invented ending appears once and is never shown.
        assert_eq!(
            filter.confirmed("We should rewrite the onboarding copy we can't be able to.", 0),
            "We should rewrite the onboarding copy"
        );
        assert_eq!(
            filter.confirmed("We should rewrite the onboarding copy rather than add.", 0),
            "We should rewrite the onboarding copy"
        );
        // Agreement ignores case and punctuation, and uses the newest
        // rendering; a fully agreed preview drops its closing punctuation.
        assert_eq!(
            filter.confirmed("We should rewrite the onboarding copy, rather than add.", 0),
            "We should rewrite the onboarding copy, rather than add"
        );

        // Committed words show even when the previous preview disagrees
        // (a cut re-reads the words around it), punctuation intact.
        let mut filter = PreviewFilter::default();
        filter.confirmed("Hello there general", 0);
        assert_eq!(
            filter.confirmed("Hello their. General Kenobi", 2),
            "Hello their. General"
        );
        assert_eq!(filter.confirmed("Hello you. Kenobi", 2), "Hello you. Kenobi");
        let mut filter = PreviewFilter::default();
        filter.confirmed("Hello their. General Kenobi", 2);

        // A word the engine keeps re-reading does not hide what follows it.
        let mut wobble = PreviewFilter::default();
        wobble.confirmed("its own memory, how will it load its memory.", 0);
        assert_eq!(
            wobble.confirmed("its own memory, how we it will load its memory and so", 0),
            "its own memory, how we it will load its memory"
        );
        // Silence after a cut: the committed text stands as written.
        assert_eq!(filter.confirmed("Hello their.", 2), "Hello their.");
        assert_eq!(filter.confirmed("", 0), "");
    }

    #[test]
    fn r33_stitch_survives_timestamp_drift_between_passes() {
        // The same words, but the second pass times them 300 ms later — a
        // purely time-based seam at 10.0 s would keep "to" from both.
        let mut acc = words(&[(9.0, "want"), (9.6, "to"), (10.2, "go"), (10.8, "home.")]);
        let next = words(&[(9.3, "Want"), (9.9, "to"), (10.5, "go"), (11.1, "home"), (11.6, "now.")]);
        stitch(&mut acc, next, 10.0);
        assert_eq!(text(&acc), "want to go home now.");
    }

    #[test]
    fn r33_stitch_matches_repeated_words_by_time_not_just_text() {
        // Counting: every word repeats nowhere, but "and" does — the seam
        // must pair the occurrences that are at the same time.
        let mut acc = words(&[
            (2.0, "one"),
            (4.0, "and"),
            (6.0, "two"),
            (9.0, "and"),
            (9.5, "three"),
            (10.5, "and"),
        ]);
        let next = words(&[(9.1, "And"), (9.6, "three"), (10.6, "and"), (11.2, "four.")]);
        stitch(&mut acc, next, 10.0);
        assert_eq!(text(&acc), "one and two and three and four.");
    }

    #[test]
    fn r33_stitch_ignores_word_end_times() {
        // Measured: the engine stretches the last word before a pause to the
        // next word's start ("to" 4.99 s → 9.15 s across a 4 s pause), so its
        // midpoint lands past the cut. Judged by midpoint, the second pass's
        // copy was kept too and the transcript read "you to to review".
        let w = |start: f32, end: f32, text: &str| Word {
            start,
            end,
            text: text.into(),
        };
        let mut acc = vec![w(4.2, 4.5, "want"), w(4.5, 4.9, "you"), w(4.9, 4.95, "to")];
        let next = vec![
            w(4.3, 4.7, "want"),
            w(4.7, 5.0, "you"),
            w(5.0, 9.15, "to"),
            w(9.15, 10.0, "review"),
        ];
        stitch(&mut acc, next, 5.9);
        assert_eq!(text(&acc), "want you to review");
    }

    #[test]
    fn r33_stitch_falls_back_to_the_cut_when_passes_share_no_word() {
        let mut acc = words(&[(8.0, "hello"), (9.2, "there."), (10.6, "extra")]);
        let next = words(&[(8.9, "Their"), (10.5, "general"), (11.0, "kenobi.")]);
        stitch(&mut acc, next, 10.0);
        assert_eq!(text(&acc), "hello there. general kenobi.");
        // First chunk: nothing accumulated yet.
        let mut empty = Vec::new();
        stitch(&mut empty, words(&[(0.5, "Hi.")]), 0.0);
        assert_eq!(text(&empty), "Hi.");
        // A silent pass adds nothing and removes nothing before the cut.
        let mut acc = words(&[(8.0, "done.")]);
        stitch(&mut acc, Vec::new(), 10.0);
        assert_eq!(text(&acc), "done.");
    }

    #[test]
    fn r33_stitch_keeps_a_word_only_the_owning_pass_heard() {
        // Measured on real dictation: the second pass, starting cold 2 s
        // before the cut, missed "works," — which sits before the cut, where
        // the first pass is the authority. And a word only the first pass
        // heard AFTER the cut (in its thin trailing context) is not trusted.
        let mut acc = words(&[
            (8.5, "the"),
            (9.0, "parser"),
            (9.5, "works,"),
            (10.3, "uh"),
            (10.7, "how"),
            (11.1, "the."),
        ]);
        let next = words(&[
            (8.6, "the"),
            (9.1, "parser"),
            (10.3, "Uh"),
            (10.7, "how"),
            (11.1, "this"),
            (11.5, "agent"),
        ]);
        stitch(&mut acc, next, 10.0);
        assert_eq!(text(&acc), "the parser works, Uh how this agent");
    }

    #[test]
    fn r33_to_absolute_shifts_a_window_into_recording_time() {
        let shifted = to_absolute(words(&[(0.5, "a")]), 8 * RATE);
        assert_eq!(shifted[0].start, 8.5);
        assert!((shifted[0].end - 8.8).abs() < 1e-5);
    }

    #[test]
    fn r30_join_texts_skips_empty_chunks_and_single_spaces() {
        assert_eq!(
            join_texts(&["First sentence. ", "", "  Second one.", " "]),
            "First sentence. Second one."
        );
        assert_eq!(join_texts::<&str>(&[]), "");
        assert_eq!(join_texts(&["", " "]), "");
    }
}
