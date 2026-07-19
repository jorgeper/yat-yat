// SPEC13 FR-P1: pure progress estimation for the transcribing overlay.
// No Tauri types here (house pattern: cleanup.rs) — the pipeline owns the
// ticker thread and event emission; this module owns the math.
//
// The fill is an ESTIMATE (transcribe-rs exposes no progress callback):
// expected transcription time = audio seconds × a per-model real-time
// factor learned as an EMA. The curve eases toward CAP and only real
// completion shows full — the estimate never masquerades as a measurement.

/// Seed real-time factor before any observation. Deliberately slow-side:
/// an over-estimate makes the bar finish early (pleasant), an
/// under-estimate parks it at the cap (annoying).
pub const DEFAULT_RTF: f32 = 0.5;

/// EMA smoothing for observed real-time factors.
pub const ALPHA: f32 = 0.3;

/// The estimate never shows more than this — completion alone shows full.
const CAP: f32 = 0.95;

/// `expected` below this is clamped up so `fraction` is total.
const MIN_EXPECTED_SECS: f32 = 0.1;

/// Estimated progress in [0, CAP]: 0 at 0, ≈0.9 when `elapsed == expected`,
/// approaching CAP as elapsed grows.
pub fn fraction(elapsed_secs: f32, expected_secs: f32) -> f32 {
    let elapsed = elapsed_secs.max(0.0);
    let expected = expected_secs.max(MIN_EXPECTED_SECS);
    // CAP·(1 − e^(−k·t/T)) with k = ln 19 lands exactly on 0.9 at t == T
    // and approaches CAP asymptotically.
    const K: f32 = 2.944_438_9; // ln 19
    CAP * (1.0 - (-K * elapsed / expected).exp())
}

/// Exponential moving average of the measured real-time factor
/// (wall-clock STT seconds ÷ audio seconds) for one model.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Rtf {
    value: f32,
    /// False until the first real measurement. The seed is a guess, not
    /// data — the first observation replaces it outright instead of
    /// blending (a 0.5 seed on a 0.01-RTF machine took ~9 dictations to
    /// converge, which read as "crawls to 10% then snaps").
    #[serde(default)]
    observed: bool,
}

impl Default for Rtf {
    fn default() -> Self {
        Self { value: DEFAULT_RTF, observed: false }
    }
}

impl Rtf {
    pub fn estimate(&self) -> f32 {
        self.value
    }

    /// Fold one measured transcription into the EMA. Non-finite or
    /// non-positive inputs are ignored.
    pub fn observe(&mut self, audio_secs: f32, wall_secs: f32) {
        if !(audio_secs > 0.0 && audio_secs.is_finite() && wall_secs > 0.0 && wall_secs.is_finite())
        {
            return;
        }
        let measured = wall_secs / audio_secs;
        if self.observed {
            self.value += ALPHA * (measured - self.value);
        } else {
            self.value = measured;
            self.observed = true;
        }
    }
}

/// The per-model RTF map with debounced persistence (SPEC14 FR-D5, R23):
/// observations only mark the store dirty — the injected writer runs at
/// flush time (return-to-idle / app exit), never per observation. With live
/// transcription on, the old write-per-observation hit the disk every 1–4 s
/// for a whole recording.
pub struct RtfStore {
    map: std::collections::HashMap<String, Rtf>,
    dirty: bool,
}

impl RtfStore {
    pub fn new(map: std::collections::HashMap<String, Rtf>) -> Self {
        Self { map, dirty: false }
    }

    /// Current estimate for one model (the seed when never measured).
    pub fn estimate(&self, model: &str) -> f32 {
        self.map.get(model).copied().unwrap_or_default().estimate()
    }

    /// Fold one measurement in and mark the store dirty. Never writes.
    pub fn observe(&mut self, model: &str, audio_secs: f32, wall_secs: f32) {
        self.map
            .entry(model.to_string())
            .or_default()
            .observe(audio_secs, wall_secs);
        self.dirty = true;
    }

    /// Invoke `write` with the map — only when dirty — then clear the flag.
    /// Returns whether a write happened.
    pub fn flush(
        &mut self,
        write: impl FnOnce(&std::collections::HashMap<String, Rtf>),
    ) -> bool {
        if !self.dirty {
            return false;
        }
        self.dirty = false;
        write(&self.map);
        true
    }
}

/// Load the per-model RTF map from disk. Missing or corrupt files read as
/// empty (models fall back to the seed).
pub fn load_rtf_map(path: &std::path::Path) -> std::collections::HashMap<String, Rtf> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Best-effort persistence — an unwritable disk must never break dictation.
pub fn save_rtf_map(path: &std::path::Path, map: &std::collections::HashMap<String, Rtf>) {
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // R18: the estimator's properties (SPEC13 §4) — the exact curve is
    // unspecified; these assertions are the contract.

    #[test]
    fn r18_fraction_zero_at_start() {
        assert_eq!(fraction(0.0, 10.0), 0.0);
        assert_eq!(fraction(0.0, 0.0), 0.0);
    }

    #[test]
    fn r18_fraction_strictly_increasing() {
        let expected = 8.0;
        let mut prev = -1.0f32;
        for i in 0..=200 {
            let elapsed = i as f32 * 0.08; // 0..=2×expected
            let f = fraction(elapsed, expected);
            assert!(
                f > prev,
                "fraction must strictly increase: f({elapsed}) = {f} !> {prev}"
            );
            prev = f;
        }
    }

    #[test]
    fn r18_fraction_near_09_at_expected() {
        for expected in [0.5f32, 3.0, 30.0, 300.0] {
            let f = fraction(expected, expected);
            assert!(
                (f - 0.9).abs() < 0.05,
                "fraction(expected, expected) should be ≈0.9, got {f}"
            );
        }
    }

    #[test]
    fn r18_fraction_capped_below_completion() {
        for (elapsed, expected) in [(100.0f32, 1.0f32), (1000.0, 10.0), (5.0, 0.0), (1.0, -3.0)] {
            let f = fraction(elapsed, expected);
            assert!(f <= 0.95, "fraction({elapsed}, {expected}) = {f} exceeds cap");
            assert!(f >= 0.0, "fraction({elapsed}, {expected}) = {f} negative");
            assert!(f.is_finite(), "fraction({elapsed}, {expected}) not finite");
        }
    }

    #[test]
    fn r18_rtf_seed_before_observation() {
        assert_eq!(Rtf::default().estimate(), DEFAULT_RTF);
    }

    #[test]
    fn r18_rtf_first_observation_replaces_seed_then_ema() {
        // The seed is a guess, not data — the first real measurement must
        // replace it outright (a 0.5 seed on a 0.01-RTF machine took ~9
        // dictations to converge and made the fill crawl then snap).
        let mut rtf = Rtf::default();
        // 10 s of audio transcribed in 2 s wall → observed RTF 0.2.
        rtf.observe(10.0, 2.0);
        assert!(
            (rtf.estimate() - 0.2).abs() < 1e-6,
            "first observation replaces the seed: got {}",
            rtf.estimate()
        );
        // Later observations blend by ALPHA.
        rtf.observe(10.0, 4.0); // observed 0.4
        let want = 0.2 + ALPHA * (0.4 - 0.2);
        assert!(
            (rtf.estimate() - want).abs() < 1e-6,
            "EMA step: got {}, want {want}",
            rtf.estimate()
        );
    }

    #[test]
    fn r18_rtf_ignores_junk_observations() {
        let mut rtf = Rtf::default();
        rtf.observe(0.0, 1.0);
        rtf.observe(-5.0, 1.0);
        rtf.observe(10.0, 0.0);
        rtf.observe(10.0, -1.0);
        rtf.observe(f32::NAN, 1.0);
        rtf.observe(10.0, f32::INFINITY);
        assert_eq!(rtf.estimate(), DEFAULT_RTF, "junk must not move the EMA");
        // Junk must not count as "observed": the next real measurement
        // still replaces the seed.
        rtf.observe(10.0, 2.0);
        assert!((rtf.estimate() - 0.2).abs() < 1e-6);
    }

    // R23: RTF persistence is debounced (SPEC14 FR-D5) — observations mark
    // dirty without invoking the writer; a flush writes exactly once and
    // clears the flag; flushing a clean store writes nothing.
    #[test]
    fn r23_observe_marks_dirty_without_writing() {
        let mut store = RtfStore::new(Default::default());
        let mut writes = 0;
        store.observe("m", 10.0, 2.0);
        store.observe("m", 10.0, 3.0);
        store.observe("other", 5.0, 1.0);
        // No flush yet — the writer must never have run.
        assert_eq!(writes, 0);
        assert!((store.estimate("m") - (0.2 + ALPHA * (0.3 - 0.2))).abs() < 1e-6);

        assert!(store.flush(|map| {
            writes += 1;
            assert_eq!(map.len(), 2, "flush sees every observed model");
        }));
        assert_eq!(writes, 1, "one flush, one write — not one per observation");
    }

    #[test]
    fn r23_flush_without_observation_writes_nothing() {
        let mut store = RtfStore::new(Default::default());
        let mut writes = 0;
        assert!(!store.flush(|_| writes += 1));
        assert_eq!(writes, 0, "a clean store never touches the disk");

        // Dirty → flush → clean again: the second flush is also a no-write.
        store.observe("m", 10.0, 2.0);
        assert!(store.flush(|_| writes += 1));
        assert!(!store.flush(|_| writes += 1));
        assert_eq!(writes, 1);
    }

    #[test]
    fn r23_estimate_reads_never_dirty_the_store() {
        let mut store = RtfStore::new(Default::default());
        assert_eq!(store.estimate("never-measured"), DEFAULT_RTF);
        let mut writes = 0;
        assert!(!store.flush(|_| writes += 1));
        assert_eq!(writes, 0);
    }

    // R21: per-model RTFs survive relaunch via <data dir>/rtf.json.
    #[test]
    fn r21_rtf_map_roundtrips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rtf.json");
        let mut map = std::collections::HashMap::new();
        let mut fast = Rtf::default();
        fast.observe(10.0, 0.2); // 0.02
        map.insert("parakeet".to_string(), fast);
        map.insert("untouched".to_string(), Rtf::default());
        save_rtf_map(&path, &map);

        let loaded = load_rtf_map(&path);
        assert!((loaded["parakeet"].estimate() - 0.02).abs() < 1e-6);
        // Observed state survives too: the next observation blends
        // instead of replacing.
        let mut back = loaded["parakeet"];
        back.observe(10.0, 0.4); // observed 0.04
        let want = 0.02 + ALPHA * (0.04 - 0.02);
        assert!((back.estimate() - want).abs() < 1e-6);
        // An unobserved entry stays seed-replaceable after the round-trip.
        let mut seed = loaded["untouched"];
        seed.observe(10.0, 2.0);
        assert!((seed.estimate() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn r21_rtf_map_missing_or_corrupt_yields_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_rtf_map(&dir.path().join("nope.json")).is_empty());
        let bad = dir.path().join("rtf.json");
        std::fs::write(&bad, "{ not json").unwrap();
        assert!(load_rtf_map(&bad).is_empty());
    }
}
