//! Speech-to-text engine abstraction over transcribe-rs (SPEC §3).
//! Two families: Parakeet (ONNX, directory models) and Whisper (GGUF files
//! via whisper.cpp — Metal on macOS).

use crate::registry::{EngineFamily, ModelEntry};
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

/// Where Parakeet runs: the CPU, or (macOS, experimental) Core ML, which
/// can use the GPU and Neural Engine. With Core ML compiled in, the
/// library's own default ("auto") would pick it silently, so every load
/// goes through here and states its choice.
pub fn parakeet_accelerator(coreml: bool) -> transcribe_rs::accel::OrtAccelerator {
    use transcribe_rs::accel::OrtAccelerator;
    if coreml && cfg!(target_os = "macos") {
        OrtAccelerator::CoreMl
    } else {
        OrtAccelerator::CpuOnly
    }
}

pub enum SttEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
}

pub struct LoadedModel {
    pub model_id: String,
    /// Language handed to Whisper; None = detect it from the audio, which
    /// costs a second pass over the audio (measured: it doubles the time of
    /// a short dictation on large-v3-turbo). Parakeet ignores it.
    pub whisper_language: Option<String>,
    /// Whether this (Parakeet) model was loaded onto Core ML; always false
    /// for Whisper. The accelerator is fixed at load, so a changed setting
    /// means a reload.
    pub coreml: bool,
    engine: SttEngine,
}

impl LoadedModel {
    pub fn load(entry: &ModelEntry, data_dir: &Path, coreml: bool) -> Result<Self> {
        let mut loaded = Self::load_path(entry.engine, &entry.engine_path(data_dir), coreml)
            .with_context(|| format!("loading model {}", entry.id))?;
        loaded.model_id = entry.id.clone();
        Ok(loaded)
    }

    /// Load from an explicit path (CLI / validation harness).
    pub fn load_path(family: EngineFamily, path: &Path, coreml: bool) -> Result<Self> {
        let coreml = coreml && family == EngineFamily::Parakeet;
        transcribe_rs::accel::set_ort_accelerator(parakeet_accelerator(coreml));
        let engine = match family {
            EngineFamily::Whisper => SttEngine::Whisper(
                WhisperEngine::load(path)
                    .map_err(|e| anyhow::anyhow!("loading whisper model: {e}"))?,
            ),
            EngineFamily::Parakeet => SttEngine::Parakeet(
                ParakeetModel::load(path, &Quantization::Int8)
                    .map_err(|e| anyhow::anyhow!("loading parakeet model: {e}"))?,
            ),
        };
        Ok(Self {
            model_id: path.display().to_string(),
            whisper_language: None,
            coreml,
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
                    samples,
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

    // R40: Parakeet runs on the CPU unless Core ML was asked for — never
    // the library's "auto", which would pick Core ML by itself now that it
    // is compiled in (measured slower: see ARCHITECTURE.md).
    #[test]
    fn r40_parakeet_stays_on_cpu_unless_coreml_is_chosen() {
        use transcribe_rs::accel::OrtAccelerator;
        assert_eq!(parakeet_accelerator(false), OrtAccelerator::CpuOnly);
        let chosen = parakeet_accelerator(true);
        if cfg!(target_os = "macos") {
            assert_eq!(chosen, OrtAccelerator::CoreMl);
        } else {
            assert_eq!(chosen, OrtAccelerator::CpuOnly, "no Core ML off macOS");
        }
        assert!(!crate::settings::Settings::default().parakeet_coreml, "off by default");
        let legacy: crate::settings::Settings = serde_json::from_str(r#"{"hotkey":"F19"}"#).unwrap();
        assert!(!legacy.parakeet_coreml, "legacy settings stay on the CPU");
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
