//! Speech-to-text engine abstraction over transcribe-rs (SPEC §3).
//! Two families: Parakeet (ONNX, directory models) and Whisper (GGUF files
//! via whisper.cpp — Metal on macOS).

use crate::registry::{EngineFamily, ModelEntry, Precision};
use anyhow::{Context, Result};
use std::path::Path;
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity};
use transcribe_rs::onnx::Quantization;
use transcribe_rs::whisper_cpp::{WhisperEngine, WhisperInferenceParams};

/// A clean, punctuated prompt nudges Whisper toward non-verbatim, well-formed
/// output (research: fillers in the prompt make Whisper verbatim; clean prose
/// keeps it clean).
const WHISPER_INITIAL_PROMPT: &str =
    "The following is a clear, well-punctuated transcription of the speaker's words.";

pub enum SttEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
}

/// One transcribed word with its position in the audio, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

pub struct LoadedModel {
    pub model_id: String,
    /// Language handed to Whisper; None = detect it from the audio, which
    /// costs a second pass over the audio (measured: it doubles the time of
    /// a short dictation on large-v3-turbo). Parakeet ignores it.
    pub whisper_language: Option<String>,
    engine: SttEngine,
}

impl LoadedModel {
    pub fn load(entry: &ModelEntry, data_dir: &Path) -> Result<Self> {
        let mut loaded =
            Self::load_path_with(entry.engine, &entry.engine_path(data_dir), entry.precision)
            .with_context(|| format!("loading model {}", entry.id))?;
        loaded.model_id = entry.id.clone();
        Ok(loaded)
    }

    /// Load from an explicit path (CLI / validation harness).
    pub fn load_path(family: EngineFamily, path: &Path) -> Result<Self> {
        Self::load_path_with(family, path, Precision::Int8)
    }

    /// [`Self::load_path`] for a Parakeet folder holding `precision` weights.
    pub fn load_path_with(family: EngineFamily, path: &Path, precision: Precision) -> Result<Self> {
        let quantization = match precision {
            Precision::Int8 => Quantization::Int8,
            Precision::Fp32 => Quantization::FP32,
        };
        let engine = match family {
            EngineFamily::Whisper => SttEngine::Whisper(
                WhisperEngine::load(path)
                    .map_err(|e| anyhow::anyhow!("loading whisper model: {e}"))?,
            ),
            EngineFamily::Parakeet => SttEngine::Parakeet(
                ParakeetModel::load(path, &quantization)
                    .map_err(|e| anyhow::anyhow!("loading parakeet model: {e}"))?,
            ),
        };
        Ok(Self {
            model_id: path.display().to_string(),
            whisper_language: None,
            engine,
        })
    }

    /// Transcribe 16 kHz mono f32 samples to raw text.
    pub fn transcribe(&mut self, samples: &[f32]) -> Result<String> {
        let text = match &mut self.engine {
            SttEngine::Whisper(engine) => engine
                .transcribe_with(
                    samples,
                    &WhisperInferenceParams {
                        initial_prompt: Some(WHISPER_INITIAL_PROMPT.to_string()),
                        language: self.whisper_language.clone(),
                        ..Default::default()
                    },
                )
                .map_err(|e| anyhow::anyhow!("whisper inference: {e}"))?
                .text,
            SttEngine::Parakeet(engine) => engine
                .transcribe_with(
                    &with_noise_floor(samples),
                    &ParakeetParams {
                        timestamp_granularity: Some(TimestampGranularity::Segment),
                        ..Default::default()
                    },
                )
                .map_err(|e| anyhow::anyhow!("parakeet inference: {e}"))?
                .text,
        };
        // Engine artifact, filtered at the engine boundary so EVERY consumer
        // (live passes, the stop path, Retry, the CLI) is protected.
        Ok(strip_prompt_echo(text.trim()))
    }
}

impl LoadedModel {
    /// Transcribe to words with timestamps — what chunked transcription
    /// needs to keep only the words inside a chunk when the engine was also
    /// given context audio on either side (chunking.rs). Parakeet only;
    /// None for engines without word timing here.
    pub fn transcribe_words(&mut self, samples: &[f32]) -> Result<Option<Vec<Word>>> {
        let SttEngine::Parakeet(engine) = &mut self.engine else {
            return Ok(None);
        };
        let result = engine
            .transcribe_with(
                &with_noise_floor(samples),
                &ParakeetParams {
                    timestamp_granularity: Some(TimestampGranularity::Word),
                    ..Default::default()
                },
            )
            .map_err(|e| anyhow::anyhow!("parakeet inference: {e}"))?;
        Ok(Some(
            result
                .segments
                .unwrap_or_default()
                .into_iter()
                .map(|s| Word {
                    start: s.start,
                    end: s.end,
                    text: s.text,
                })
                .collect(),
        ))
    }
}

/// Peak of the noise [`with_noise_floor`] adds, as a fraction of full scale
/// (about -52 dB; its RMS is about -60 dB, well under a quiet room).
const NOISE_FLOOR_PEAK: f32 = 0.0025;

/// The audio with a faint, fixed noise floor under it, for Parakeet.
///
/// The quantized (int8) Parakeet encoder skips whole sentences on some clean
/// recordings, and which ones it skips changes with the length of audio it is
/// given. Measured on a 20 s dictation: 45 of 56 words for the whole
/// recording, 0 for its first 10 s, 15 for its first 18 s. With this noise
/// added the same passes return 56, 30 and 56 — the same as the
/// full-precision weights. Noise a third as strong did not help; noise ten
/// times stronger did no harm.
///
/// The noise is generated from a fixed seed, so a recording always
/// transcribes the same way.
pub fn with_noise_floor(samples: &[f32]) -> Vec<f32> {
    let mut state: u32 = 0x9E37_79B9;
    let mut unit = move || {
        // xorshift32, mapped to [0, 1).
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 8) as f32 / (1u32 << 24) as f32
    };
    samples
        .iter()
        // The difference of two uniform values is triangular around zero.
        .map(|&s| s + (unit() - unit()) * NOISE_FLOOR_PEAK)
        .collect()
}

/// During silence, Whisper hallucinates its own initial prompt back —
/// usually mutated ("This is the clear, well-pu-ctuated transcription of
/// the speaker's words."). Sentence-level filter (R16): a sentence is an
/// echo when it shares a run of 4+ consecutive normalized words with the
/// prompt. Real dictation virtually never reproduces a 4-word run of
/// prompt-engineering prose; mutated echoes always keep one intact.
pub fn strip_prompt_echo(text: &str) -> String {
    fn words(s: &str) -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty())
            .map(|w| w.to_lowercase())
            .collect()
    }

    let prompt_words = words(WHISPER_INITIAL_PROMPT);
    let is_echo = |sentence: &str| -> bool {
        let sw = words(sentence);
        if sw.len() < 4 {
            return false;
        }
        sw.windows(4)
            .any(|run| prompt_words.windows(4).any(|p| p == run))
    };

    // Split into sentences, keeping each terminator with its sentence.
    let mut out = String::new();
    let mut sentence = String::new();
    for c in text.chars() {
        sentence.push(c);
        if matches!(c, '.' | '!' | '?') {
            if !is_echo(&sentence) {
                out.push_str(&sentence);
            }
            sentence.clear();
        }
    }
    if !sentence.trim().is_empty() && !is_echo(&sentence) {
        out.push_str(&sentence);
    }
    out.trim().to_string()
}

#[cfg(test)]
mod echo_tests {
    use super::*;

    // R45: the Parakeet noise floor.
    #[test]
    fn r45_noise_floor_is_faint_fixed_and_fills_silence() {
        let silence = vec![0.0f32; 16_000];
        let a = with_noise_floor(&silence);
        assert_eq!(a.len(), silence.len());
        // Faint: never above the stated peak, and about -60 dB RMS.
        assert!(a.iter().all(|s| s.abs() <= NOISE_FLOOR_PEAK));
        let rms = (a.iter().map(|s| s * s).sum::<f32>() / a.len() as f32).sqrt();
        assert!((0.0008..0.0013).contains(&rms), "rms {rms}");
        // Fills digital silence, without a DC offset.
        assert!(a.iter().filter(|s| **s != 0.0).count() > 15_900);
        assert!((a.iter().sum::<f32>() / a.len() as f32).abs() < 1e-4);
        // Fixed: the same recording always gets the same noise.
        assert_eq!(a, with_noise_floor(&silence));
        // Speech passes through with only the noise added.
        let speech: Vec<f32> = (0..1_600).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let b = with_noise_floor(&speech);
        assert!(speech.iter().zip(&b).all(|(x, y)| (x - y).abs() <= NOISE_FLOOR_PEAK));
    }

    // R16: the prompt-echo filter (silence hallucination guard).
    #[test]
    fn r16_drops_verbatim_and_mutated_prompt_echoes() {
        assert_eq!(strip_prompt_echo(WHISPER_INITIAL_PROMPT), "");
        // The mutation observed in the field: doubled word + mangled token.
        assert_eq!(
            strip_prompt_echo(
                "This is the clear, clear, well-pu-ctuated transcription of the speaker's words."
            ),
            ""
        );
    }

    #[test]
    fn r16_keeps_real_speech_and_mixed_output() {
        let real = "Ship the release notes on Wednesday.";
        assert_eq!(strip_prompt_echo(real), real);
        // Echo sentence next to real speech: the speech survives.
        let mixed = format!("{WHISPER_INITIAL_PROMPT} Call the dentist tomorrow.");
        assert_eq!(strip_prompt_echo(&mixed), "Call the dentist tomorrow.");
    }

    #[test]
    fn r16_short_or_coincidental_overlap_is_not_an_echo() {
        // Shares words but never a 4-word run of the prompt.
        let s = "The transcription is clear and the words are the speaker's own.";
        assert_eq!(strip_prompt_echo(s), s);
        assert_eq!(strip_prompt_echo(""), "");
    }
}

/// Read a WAV file into 16 kHz mono f32 samples (for Retry and the CLI).
pub fn read_wav_16k_mono(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).context("opening wav")?;
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<_, _>>()?
        }
    };
    // Downmix to mono.
    let mono: Vec<f32> = if spec.channels > 1 {
        raw.chunks_exact(spec.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect()
    } else {
        raw
    };
    if spec.sample_rate == 16_000 {
        Ok(mono)
    } else {
        crate::audio::resample_to_16k(&mono, spec.sample_rate)
    }
}
