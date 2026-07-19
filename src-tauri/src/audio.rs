//! Microphone capture (SPEC §2): cpal at the device's native config, mono
//! downmix, software resample to 16 kHz. Levels for the overlay waveform are
//! emitted ~30×/s while recording.
//!
//! The cpal Stream is !Send, so it lives on a dedicated worker thread that
//! receives commands over a channel (pattern adapted from Handy, MIT).

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{FftFixedIn, Resampler};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

pub const TARGET_RATE: u32 = 16_000;
/// Recordings shorter than 1 s are zero-padded to 1.25 s — tiny buffers make
/// STT engines misbehave.
const MIN_SAMPLES: usize = (TARGET_RATE as usize) * 5 / 4;
/// SPEC FR-1.4: recording auto-stops at 5 minutes.
pub const MAX_RECORD_SECS: u64 = 300;
/// SPEC14 FR-D1: after the stream stops, the stop-drain considers the channel
/// flushed once it has been quiet this long…
const STOP_QUIET_MS: u64 = 15;
/// …and never waits longer than this in total (the old fixed sleep).
const STOP_DRAIN_CAP_MS: u64 = 60;

pub type LevelCallback = Arc<dyn Fn(f32) + Send + Sync + 'static>;

enum Cmd {
    Start {
        device_name: Option<String>,
        reply: mpsc::Sender<Result<()>>,
    },
    Stop {
        reply: mpsc::Sender<Result<Vec<f32>>>,
    },
    /// Non-destructive copy of everything captured so far, at the device rate
    /// (SPEC3 live passes). Capture continues untouched.
    Snapshot {
        reply: mpsc::Sender<(Vec<f32>, u32)>,
    },
    Cancel,
    Shutdown,
}

pub struct AudioRecorder {
    tx: mpsc::Sender<Cmd>,
    recording: Arc<AtomicBool>,
}

impl AudioRecorder {
    pub fn new(level_cb: LevelCallback) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let recording = Arc::new(AtomicBool::new(false));
        let rec_flag = recording.clone();
        std::thread::Builder::new()
            .name("audio-recorder".into())
            .spawn(move || worker(rx, rec_flag, level_cb))
            .expect("spawn audio worker");
        Self { tx, recording }
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    pub fn start(&self, device_name: Option<String>) -> Result<()> {
        let (reply, rx) = mpsc::channel();
        self.tx.send(Cmd::Start { device_name, reply })?;
        rx.recv().context("audio worker gone")?
    }

    /// Stop and return all captured samples resampled to 16 kHz mono.
    pub fn stop(&self) -> Result<Vec<f32>> {
        let (reply, rx) = mpsc::channel();
        self.tx.send(Cmd::Stop { reply })?;
        rx.recv().context("audio worker gone")?
    }

    /// Copy of the audio captured so far, 16 kHz mono. Recording continues;
    /// returns None when not recording. The buffer is kept at 16 kHz by the
    /// worker (SPEC14 FR-A1), so this is a plain copy — no per-pass resample.
    pub fn snapshot_16k(&self) -> Option<Vec<f32>> {
        if !self.is_recording() {
            return None;
        }
        let (reply, rx) = mpsc::channel();
        self.tx.send(Cmd::Snapshot { reply }).ok()?;
        let (samples, _rate) = rx.recv().ok()?;
        Some(samples)
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(Cmd::Cancel);
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
    }
}

struct ActiveStream {
    _stream: cpal::Stream,
    samples_rx: mpsc::Receiver<Vec<f32>>,
    started: std::time::Instant,
}

/// Collect whatever the capture callback still has queued once the stream is
/// stopping: keep receiving until the channel has been quiet for `quiet` (or
/// disconnects — the dropped stream drops the last sender), hard-capped at
/// `cap` total. Replaces the old fixed 60 ms sleep (SPEC14 FR-D1, R22).
fn drain_pending(
    rx: &mpsc::Receiver<Vec<f32>>,
    quiet: std::time::Duration,
    cap: std::time::Duration,
) -> Vec<Vec<f32>> {
    let started = std::time::Instant::now();
    let mut chunks = Vec::new();
    loop {
        let elapsed = started.elapsed();
        if elapsed >= cap {
            break;
        }
        match rx.recv_timeout(quiet.min(cap - elapsed)) {
            Ok(chunk) => chunks.push(chunk),
            Err(_) => break, // quiet window elapsed, or sender gone: flushed
        }
    }
    chunks
}

fn worker(rx: mpsc::Receiver<Cmd>, recording: Arc<AtomicBool>, level_cb: LevelCallback) {
    let mut active: Option<ActiveStream> = None;
    // The capture buffer holds 16 kHz mono (SPEC14 FR-A1): chunks are
    // resampled incrementally as they arrive, so snapshots are plain copies
    // and the stop path never resamples a whole recording.
    let mut captured = crate::live::CaptureBuffer::default();
    let mut resampler: Option<StreamResampler> = None;

    loop {
        // Drain any captured audio while recording; poll commands at 10 ms.
        if let Some(stream) = &active {
            while let Ok(chunk) = stream.samples_rx.try_recv() {
                if let Some(rs) = resampler.as_mut() {
                    captured.push_chunk(rs.process(&chunk));
                }
            }
            // Safety cap (SPEC FR-1.4).
            if stream.started.elapsed().as_secs() >= MAX_RECORD_SECS {
                log::warn!("recording hit {MAX_RECORD_SECS}s cap; auto-stopping capture");
                // Keep captured audio; stop the stream (dropping it stops
                // capture) but stay in "recording" state until the pipeline
                // calls stop() and collects it.
                let stream = active.take().unwrap();
                drop(stream._stream);
                for chunk in drain_pending(
                    &stream.samples_rx,
                    std::time::Duration::from_millis(STOP_QUIET_MS),
                    std::time::Duration::from_millis(STOP_DRAIN_CAP_MS),
                ) {
                    if let Some(rs) = resampler.as_mut() {
                        captured.push_chunk(rs.process(&chunk));
                    }
                }
            }
        }

        // Only poll while a stream needs draining; idle waits block (SPEC14
        // FR-A4 — no 100 wakeups/s forever for a background utility).
        let cmd = if active.is_some() {
            match rx.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(cmd) => cmd,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => return,
            }
        };

        match cmd {
            Cmd::Start { device_name, reply } => {
                captured.clear();
                let result = open_stream(device_name, level_cb.clone());
                match result {
                    Ok((stream, samples_rx, device_rate)) => {
                        match StreamResampler::new(device_rate) {
                            Ok(rs) => {
                                resampler = Some(rs);
                                active = Some(ActiveStream {
                                    _stream: stream,
                                    samples_rx,
                                    started: std::time::Instant::now(),
                                });
                                recording.store(true, Ordering::SeqCst);
                                let _ = reply.send(Ok(()));
                            }
                            Err(e) => {
                                let _ = reply.send(Err(e));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            Cmd::Stop { reply } => {
                if let Some(stream) = active.take() {
                    drop(stream._stream);
                    for chunk in drain_pending(
                        &stream.samples_rx,
                        std::time::Duration::from_millis(STOP_QUIET_MS),
                        std::time::Duration::from_millis(STOP_DRAIN_CAP_MS),
                    ) {
                        if let Some(rs) = resampler.as_mut() {
                            captured.push_chunk(rs.process(&chunk));
                        }
                    }
                }
                // Flush the resampler tail — without this the last partial
                // chunk (word endings!) is lost.
                if let Some(rs) = resampler.as_mut() {
                    captured.push_chunk(rs.flush());
                }
                resampler = None;
                recording.store(false, Ordering::SeqCst);
                let mut samples = captured.take();
                if !samples.is_empty() && samples.len() < MIN_SAMPLES {
                    samples.resize(MIN_SAMPLES, 0.0);
                }
                let _ = reply.send(Ok(samples));
            }
            Cmd::Snapshot { reply } => {
                if let Some(stream) = &active {
                    while let Ok(chunk) = stream.samples_rx.try_recv() {
                        if let Some(rs) = resampler.as_mut() {
                            captured.push_chunk(rs.process(&chunk));
                        }
                    }
                }
                // Already 16 kHz — a snapshot is just a copy now.
                let _ = reply.send((captured.snapshot(), TARGET_RATE));
            }
            Cmd::Cancel => {
                active = None;
                resampler = None;
                captured.clear();
                recording.store(false, Ordering::SeqCst);
            }
            Cmd::Shutdown => return,
        }
    }
}

fn open_stream(
    device_name: Option<String>,
    level_cb: LevelCallback,
) -> Result<(cpal::Stream, mpsc::Receiver<Vec<f32>>, u32)> {
    let host = cpal::default_host();
    let device = match &device_name {
        Some(name) => host
            .input_devices()?
            .find(|d| d.name().map(|n| n == *name).unwrap_or(false))
            .ok_or_else(|| anyhow!("input device '{name}' not found"))?,
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?,
    };
    let config = device
        .default_input_config()
        .context("querying input config")?;
    let device_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    // ~33 ms level window at the device rate (≈30 updates/s).
    let level_window = (device_rate / 30) as usize;
    let mut level_acc: Vec<f32> = Vec::with_capacity(level_window);

    macro_rules! build {
        ($t:ty) => {{
            let tx = tx.clone();
            let level_cb = level_cb.clone();
            let mut level_acc = std::mem::take(&mut level_acc);
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[$t], _| {
                    let mono: Vec<f32> = data
                        .chunks_exact(channels)
                        .map(|frame| {
                            frame
                                .iter()
                                .map(|s| cpal::Sample::to_sample::<f32>(*s))
                                .sum::<f32>()
                                / channels as f32
                        })
                        .collect();
                    // Level emission (~30 Hz): RMS -> dB -> 0..1.
                    for &s in &mono {
                        level_acc.push(s);
                        if level_acc.len() >= level_window {
                            let rms = (level_acc.iter().map(|x| x * x).sum::<f32>()
                                / level_acc.len() as f32)
                                .sqrt();
                            let db = 20.0 * (rms.max(1e-9)).log10();
                            // Map -55 dB..-8 dB onto 0..1 (Handy's vocal range).
                            let norm = ((db + 55.0) / 47.0).clamp(0.0, 1.0);
                            level_cb(norm.powf(0.7));
                            level_acc.clear();
                        }
                    }
                    let _ = tx.send(mono);
                },
                |err| log::error!("audio stream error: {err}"),
                None,
            )?
        }};
    }

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::U16 => build!(u16),
        cpal::SampleFormat::I32 => build!(i32),
        other => return Err(anyhow!("unsupported sample format {other:?}")),
    };
    stream.play().context("starting input stream")?;
    Ok((stream, rx, device_rate))
}

/// Incremental device-rate → 16 kHz resampler (SPEC14 FR-A1): chunks feed a
/// persistent `FftFixedIn` in the same 1024-frame batches `resample_to_16k`
/// uses, so the incremental path and the whole-buffer path produce identical
/// output for the same input (R24). `flush` zero-pads the final partial batch
/// exactly like the whole-buffer loop's tail.
pub struct StreamResampler {
    /// None at 16 kHz devices — chunks pass through untouched.
    inner: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
    out: Vec<f32>,
    passthrough: Vec<f32>,
}

const RESAMPLE_CHUNK: usize = 1024;

impl StreamResampler {
    pub fn new(from_rate: u32) -> Result<Self> {
        let inner = if from_rate == TARGET_RATE {
            None
        } else {
            Some(
                FftFixedIn::<f32>::new(
                    from_rate as usize,
                    TARGET_RATE as usize,
                    RESAMPLE_CHUNK,
                    1,
                    1,
                )
                .context("creating stream resampler")?,
            )
        };
        Ok(Self {
            inner,
            pending: Vec::new(),
            out: Vec::new(),
            passthrough: Vec::new(),
        })
    }

    /// Feed one captured chunk; returns the 16 kHz samples produced so far by
    /// whole 1024-frame batches (a borrowed scratch buffer, valid until the
    /// next call — no per-chunk allocation).
    pub fn process(&mut self, chunk: &[f32]) -> &[f32] {
        let Some(resampler) = self.inner.as_mut() else {
            self.passthrough.clear();
            self.passthrough.extend_from_slice(chunk);
            return &self.passthrough;
        };
        self.pending.extend_from_slice(chunk);
        self.out.clear();
        let mut pos = 0;
        while self.pending.len() - pos >= RESAMPLE_CHUNK {
            if let Ok(result) = resampler.process(&[&self.pending[pos..pos + RESAMPLE_CHUNK]], None)
            {
                self.out.extend_from_slice(&result[0]);
            }
            pos += RESAMPLE_CHUNK;
        }
        self.pending.drain(..pos);
        &self.out
    }

    /// Zero-pad and process the final partial batch (the whole-buffer loop's
    /// tail behavior). Without this the last <64 ms of speech is lost.
    pub fn flush(&mut self) -> &[f32] {
        self.out.clear();
        let Some(resampler) = self.inner.as_mut() else {
            return &self.out;
        };
        if self.pending.is_empty() {
            return &self.out;
        }
        self.pending.resize(RESAMPLE_CHUNK, 0.0);
        if let Ok(result) = resampler.process(&[&self.pending[..]], None) {
            self.out.extend_from_slice(&result[0]);
        }
        self.pending.clear();
        &self.out
    }
}

/// Resample mono f32 samples to 16 kHz (rubato FFT resampler, 1024-frame chunks).
pub fn resample_to_16k(samples: &[f32], from_rate: u32) -> Result<Vec<f32>> {
    if from_rate == TARGET_RATE || samples.is_empty() {
        return Ok(samples.to_vec());
    }
    const CHUNK: usize = 1024;
    let mut resampler = FftFixedIn::<f32>::new(from_rate as usize, TARGET_RATE as usize, CHUNK, 1, 1)
        .context("creating resampler")?;
    let mut out: Vec<f32> = Vec::with_capacity(
        (samples.len() as u64 * TARGET_RATE as u64 / from_rate as u64) as usize + CHUNK,
    );
    let mut pos = 0;
    let mut buf = vec![0.0f32; CHUNK];
    while pos < samples.len() {
        let take = CHUNK.min(samples.len() - pos);
        buf[..take].copy_from_slice(&samples[pos..pos + take]);
        buf[take..].fill(0.0); // zero-pad the final partial chunk
        let result = resampler
            .process(&[&buf[..]], None)
            .context("resampling chunk")?;
        out.extend_from_slice(&result[0]);
        pos += take;
    }
    Ok(out)
}

/// Enumerate input device names for the settings picker.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// Write 16 kHz mono samples as a WAV file (retained last recording, FR-5).
pub fn write_wav_16k_mono(path: &std::path::Path, samples: &[f32]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

/// 16-bit PCM variant for the retained recording (SPEC14 FR-D3 — half the
/// file, no STT-relevant quality loss; `read_wav_16k_mono` handles Int).
pub fn write_wav_16k_mono_pcm16(path: &std::path::Path, samples: &[f32]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_halves_sample_count_from_32k() {
        let samples: Vec<f32> = (0..32_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let out = resample_to_16k(&samples, 32_000).unwrap();
        // 1 second of audio in, ~1 second out (chunk padding adds a tail).
        let expected = 16_000f32;
        assert!(
            (out.len() as f32 - expected).abs() / expected < 0.1,
            "got {} samples",
            out.len()
        );
    }

    #[test]
    fn resample_passthrough_at_16k() {
        let samples = vec![0.5f32; 1000];
        assert_eq!(resample_to_16k(&samples, 16_000).unwrap().len(), 1000);
    }

    // R22: the adaptive stop-drain (SPEC14 FR-D1) — collects everything
    // queued, returns as soon as the channel goes quiet, never waits past
    // the cap.
    #[test]
    fn r22_drain_collects_all_queued_chunks_fast() {
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        tx.send(vec![0.1; 100]).unwrap();
        tx.send(vec![0.2; 100]).unwrap();
        tx.send(vec![0.3; 100]).unwrap();
        drop(tx); // stream gone — like the worker dropping the cpal stream
        let t0 = std::time::Instant::now();
        let chunks = drain_pending(
            &rx,
            std::time::Duration::from_millis(15),
            std::time::Duration::from_millis(60),
        );
        let elapsed = t0.elapsed();
        assert_eq!(chunks.len(), 3, "every queued chunk collected");
        assert_eq!(chunks[2][0], 0.3);
        assert!(
            elapsed < std::time::Duration::from_millis(40),
            "disconnected channel drains immediately, took {elapsed:?}"
        );
    }

    #[test]
    fn r22_drain_quiet_channel_returns_within_quiet_window() {
        // Sender still alive but silent: the quiet window, not the old fixed
        // 60 ms sleep, decides.
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let t0 = std::time::Instant::now();
        let chunks = drain_pending(
            &rx,
            std::time::Duration::from_millis(15),
            std::time::Duration::from_millis(60),
        );
        let elapsed = t0.elapsed();
        drop(tx);
        assert!(chunks.is_empty());
        assert!(
            elapsed < std::time::Duration::from_millis(45),
            "quiet channel must return well under the 60 ms cap, took {elapsed:?}"
        );
    }

    #[test]
    fn r22_drain_never_exceeds_the_cap() {
        // A pathological sender that never goes quiet: the cap bounds us.
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let feeder = std::thread::spawn(move || {
            for _ in 0..100 {
                if tx.send(vec![0.0; 10]).is_err() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });
        let t0 = std::time::Instant::now();
        let _ = drain_pending(
            &rx,
            std::time::Duration::from_millis(15),
            std::time::Duration::from_millis(60),
        );
        let elapsed = t0.elapsed();
        drop(rx);
        let _ = feeder.join();
        assert!(
            elapsed < std::time::Duration::from_millis(120),
            "drain must stay near the 60 ms cap, took {elapsed:?}"
        );
    }

    // R24: the incremental 16 kHz capture path (SPEC14 FR-A1) is equivalent
    // to the whole-buffer resample_to_16k — including the flushed tail.
    fn incremental_equals_full(from_rate: u32, chunk_len: usize, total: usize) {
        let samples: Vec<f32> = (0..total)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / from_rate as f32).sin() * 0.5)
            .collect();
        let full = resample_to_16k(&samples, from_rate).unwrap();

        let mut rs = StreamResampler::new(from_rate).unwrap();
        let mut incremental: Vec<f32> = Vec::new();
        for chunk in samples.chunks(chunk_len) {
            incremental.extend_from_slice(rs.process(chunk));
        }
        incremental.extend_from_slice(rs.flush());

        assert_eq!(
            incremental.len(),
            full.len(),
            "incremental ({from_rate} Hz, {chunk_len}-sample chunks) length differs from full"
        );
        for (i, (a, b)) in incremental.iter().zip(full.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "sample {i} differs at {from_rate} Hz: {a} vs {b}"
            );
        }
    }

    #[test]
    fn r24_incremental_capture_matches_full_resample_48k() {
        // 480-sample chunks: CoreAudio's typical ~10 ms callback at 48 kHz —
        // deliberately not aligned to the resampler's 1024-frame batches.
        incremental_equals_full(48_000, 480, 48_000 * 5 / 2);
    }

    #[test]
    fn r24_incremental_capture_matches_full_resample_44100() {
        // Non-integral ratio + a chunk size that never divides 1024.
        incremental_equals_full(44_100, 441, 44_100 * 2);
    }

    #[test]
    fn r24_incremental_passthrough_at_16k() {
        let samples: Vec<f32> = (0..5000).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut rs = StreamResampler::new(16_000).unwrap();
        let mut out: Vec<f32> = Vec::new();
        for chunk in samples.chunks(160) {
            out.extend_from_slice(rs.process(chunk));
        }
        out.extend_from_slice(rs.flush());
        assert_eq!(out, samples, "16 kHz devices pass through untouched");
    }

    #[test]
    fn pcm16_wav_roundtrip_within_quantization() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t16.wav");
        let samples: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.02).sin() * 0.3).collect();
        write_wav_16k_mono_pcm16(&path, &samples).unwrap();
        let back = crate::stt::read_wav_16k_mono(&path).unwrap();
        assert_eq!(back.len(), samples.len());
        // 16-bit quantization step is ~3e-5 — well below anything audible.
        assert!((back[100] - samples[100]).abs() < 1e-4);
    }

    #[test]
    fn wav_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.wav");
        let samples: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.02).sin() * 0.3).collect();
        write_wav_16k_mono(&path, &samples).unwrap();
        let back = crate::stt::read_wav_16k_mono(&path).unwrap();
        assert_eq!(back.len(), samples.len());
        assert!((back[100] - samples[100]).abs() < 1e-6);
    }
}
