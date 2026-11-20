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
