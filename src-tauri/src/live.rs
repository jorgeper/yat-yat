//! Live-transcription helpers (SPEC3): the pass-interval controller and the
//! capture buffer. Both are pure so R10 can prove the guarantees — snapshots
//! never disturb capture, and slow hardware stretches the cadence instead of
//! breaking recordings.

use std::time::Duration;

pub const BASE_INTERVAL_MS: u64 = 1000;
pub const MAX_INTERVAL_MS: u64 = 4000;
/// A pass costing more than this fraction of the interval stretches it.
const STRETCH_THRESHOLD: f64 = 0.6;
/// A pass costing less than this fraction lets the interval recover.
const RECOVER_THRESHOLD: f64 = 0.3;

/// Adaptive cadence for live passes (SPEC3 FR-L2 perf guard).
#[derive(Debug)]
pub struct LiveInterval {
    current_ms: u64,
    stretched_logged: bool,
}

impl Default for LiveInterval {
    fn default() -> Self {
        Self {
            current_ms: BASE_INTERVAL_MS,
            stretched_logged: false,
        }
    }
}

impl LiveInterval {
    pub fn current(&self) -> Duration {
        Duration::from_millis(self.current_ms)
    }

    /// Feed the duration of a completed pass; adjusts the next interval.
    pub fn on_pass(&mut self, pass: Duration) {
        let pass_ms = pass.as_millis() as f64;
        let current = self.current_ms as f64;
        if pass_ms > current * STRETCH_THRESHOLD {
            let stretched = ((self.current_ms * 3) / 2).min(MAX_INTERVAL_MS);
            if stretched > self.current_ms && !self.stretched_logged {
                log::info!(
                    "live pass took {pass_ms:.0}ms; stretching live interval to {stretched}ms"
                );
                self.stretched_logged = true;
            }
            self.current_ms = stretched;
        } else if pass_ms < current * RECOVER_THRESHOLD {
            self.current_ms = ((self.current_ms * 2) / 3).max(BASE_INTERVAL_MS);
        }
    }
}

/// Shortest rest between two live preview passes on the chunked path
/// (pipeline::chunk_loop).
pub const PREVIEW_MIN_GAP: Duration = Duration::from_millis(400);

/// How long the chunked live path rests after a preview pass that took
/// `pass`: at least [`PREVIEW_MIN_GAP`], and never less than the pass
/// itself — so previews use at most half of one core however slow the
/// machine, the same promise [`LiveInterval`] makes for whole-buffer passes.
pub fn preview_gap(pass: Duration) -> Duration {
    pass.max(PREVIEW_MIN_GAP)
}

/// Accumulates mono samples during a recording. Snapshots are non-destructive
/// (SPEC3 FR-L2: capture must not hiccup); `take` drains for the final pass.
#[derive(Debug, Default)]
pub struct CaptureBuffer {
    samples: Vec<f32>,
}

impl CaptureBuffer {
    pub fn push_chunk(&mut self, chunk: &[f32]) {
        self.samples.extend_from_slice(chunk);
    }

    /// Copy of everything captured so far; capture continues untouched.
    pub fn snapshot(&self) -> Vec<f32> {
        self.samples.clone()
    }

    /// Copy of the audio captured from sample `from` onward (the chunker's
    /// view: everything since its last cut). Empty when `from` is past the
    /// end. Capture continues untouched.
    pub fn snapshot_from(&self, from: usize) -> Vec<f32> {
        self.samples.get(from..).map(<[f32]>::to_vec).unwrap_or_default()
    }

    pub fn take(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }

    /// Reset AND release the allocation (SPEC14 FR-A3, R25): a cancelled
    /// 5-minute recording must not leave tens of MB resident in the worker.
    pub fn clear(&mut self) {
        self.samples = Vec::new();
    }

    /// Retained allocation, for the R25 release guarantee.
    pub fn capacity(&self) -> usize {
        self.samples.capacity()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // R10: interval controller — stretch on slow passes, cap, recovery.
    #[test]
    fn r10_interval_stretches_on_slow_passes() {
        let mut interval = LiveInterval::default();
        assert_eq!(interval.current(), Duration::from_millis(1000));
        interval.on_pass(Duration::from_millis(700)); // > 60% of 1000
        assert_eq!(interval.current(), Duration::from_millis(1500));
        interval.on_pass(Duration::from_millis(1200));
        assert_eq!(interval.current(), Duration::from_millis(2250));
    }

    #[test]
    fn r10_interval_caps_at_four_seconds() {
        let mut interval = LiveInterval::default();
        for _ in 0..10 {
            interval.on_pass(Duration::from_millis(10_000));
        }
        assert_eq!(interval.current(), Duration::from_millis(MAX_INTERVAL_MS));
    }

    #[test]
    fn r10_interval_recovers_when_passes_get_fast() {
        let mut interval = LiveInterval::default();
        for _ in 0..10 {
            interval.on_pass(Duration::from_millis(10_000));
        }
        for _ in 0..20 {
            interval.on_pass(Duration::from_millis(100)); // < 30% of anything ≥ 1000
        }
        assert_eq!(interval.current(), Duration::from_millis(BASE_INTERVAL_MS));
    }

    #[test]
    fn r10_interval_stable_in_the_comfortable_band() {
        let mut interval = LiveInterval::default();
        interval.on_pass(Duration::from_millis(450)); // between 30% and 60%
        assert_eq!(interval.current(), Duration::from_millis(1000));
    }

    // R41: the chunked live path's pacing — a floor between previews, and
    // at most half the time spent transcribing on a slow machine.
    #[test]
    fn r41_preview_gap_has_a_floor_and_caps_the_duty_cycle() {
        assert_eq!(preview_gap(Duration::from_millis(0)), PREVIEW_MIN_GAP);
        assert_eq!(preview_gap(Duration::from_millis(150)), PREVIEW_MIN_GAP);
        assert_eq!(preview_gap(PREVIEW_MIN_GAP), PREVIEW_MIN_GAP);
        for slow_ms in [401u64, 900, 5_000] {
            let pass = Duration::from_millis(slow_ms);
            assert_eq!(preview_gap(pass), pass, "rest as long as the pass took");
        }
        // Faster than the whole-buffer cadence it replaces.
        assert!(PREVIEW_MIN_GAP < Duration::from_millis(BASE_INTERVAL_MS));
    }

    // R25: clear (the Cancel path) releases the buffer's allocation — a
    // cancelled long recording must not pin memory until the next stop.
    #[test]
    fn r25_clear_releases_capacity_take_still_drains() {
        let mut buffer = CaptureBuffer::default();
        buffer.push_chunk(&vec![0.1f32; 1_000_000]);
        assert!(buffer.capacity() >= 1_000_000);
        buffer.clear();
        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), 0, "clear must release the allocation");

        // take() (the stop path) frees by moving the Vec out — unchanged.
        buffer.push_chunk(&vec![0.2f32; 500_000]);
        let taken = buffer.take();
        assert_eq!(taken.len(), 500_000);
        assert_eq!(buffer.capacity(), 0, "take leaves no retained allocation");
    }

    // R10: snapshots never disturb capture — snapshot twice, second is longer.
    #[test]
    fn r10_snapshot_is_non_destructive_and_capture_continues() {
        let mut buffer = CaptureBuffer::default();
        buffer.push_chunk(&[0.1, 0.2, 0.3]);
        let first = buffer.snapshot();
        assert_eq!(first.len(), 3);

        buffer.push_chunk(&[0.4, 0.5]);
        let second = buffer.snapshot();
        assert_eq!(second.len(), 5, "capture continued after snapshot");
        assert_eq!(second[..3], first[..], "earlier audio unchanged");

        let taken = buffer.take();
        assert_eq!(taken.len(), 5);
        assert!(buffer.is_empty(), "take drains for the final pass");
    }

    // R31: the chunker's partial snapshot — only the audio after its last
    // cut, never disturbing capture, and safe past the end.
    #[test]
    fn r31_snapshot_from_copies_only_the_pending_audio() {
        let mut buffer = CaptureBuffer::default();
        buffer.push_chunk(&[0.1, 0.2, 0.3, 0.4]);
        assert_eq!(buffer.snapshot_from(0), vec![0.1, 0.2, 0.3, 0.4]);
        assert_eq!(buffer.snapshot_from(3), vec![0.4]);
        assert!(buffer.snapshot_from(4).is_empty());
        assert!(buffer.snapshot_from(99).is_empty(), "past the end is empty, not a panic");
        buffer.push_chunk(&[0.5]);
        assert_eq!(buffer.snapshot_from(3), vec![0.4, 0.5], "capture continued");
        assert_eq!(buffer.len(), 5, "snapshots are non-destructive");
    }
}
