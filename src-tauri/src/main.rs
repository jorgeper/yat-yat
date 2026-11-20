//! Yat Yat entry point. With no arguments this launches the menu-bar app;
//! subcommands expose the pipeline headlessly for the validation harness
//! (SPEC §8: I1 transcribes a fixture through the REAL engine + cleanup, I2
//! exercises cleanup-only mode).

#![cfg_attr(not(test), windows_subsystem = "windows")]

use clap::{Parser, Subcommand};
use yatyat_lib::cleanup::{clean, CleanOptions};
use yatyat_lib::registry::EngineFamily;
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "yat-yat", about = "Fast, fully-local voice dictation")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Transcribe a WAV file through the real engine + cleanup pipeline.
    Transcribe {
        /// Path to the model file (whisper GGUF) or directory (parakeet).
        #[arg(long)]
        model_path: PathBuf,
        /// Engine family: whisper | parakeet.
        #[arg(long)]
        engine: String,
        /// Print the raw transcript without the cleanup pass.
        #[arg(long)]
        raw: bool,
        /// Replay chunked transcription (chunking.rs) over the file: cut at
        /// pauses as the app does while recording, transcribe each chunk, and
        /// report the tail — the only part the app's stop path waits for.
        #[arg(long)]
        chunked: bool,
        /// 16 kHz (or any rate) WAV file to transcribe.
        wav: PathBuf,
    },
    /// Read text on stdin, apply the deterministic cleanup pass, print it.
    Clean {
        /// Disable repeated-word collapsing.
        #[arg(long)]
        keep_repeats: bool,
    },
    /// Measure cleanup-filter latency on a 1,000-word transcript (SPEC §7).
    BenchClean,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            yatyat_lib::run();
            Ok(())
        }
        Some(Command::Transcribe {
            model_path,
            engine,
            raw,
            chunked,
            wav,
        }) => {
            let family = match engine.as_str() {
                "whisper" => EngineFamily::Whisper,
                "parakeet" => EngineFamily::Parakeet,
                other => anyhow::bail!("unknown engine '{other}' (whisper|parakeet)"),
            };
            let samples = yatyat_lib::stt::read_wav_16k_mono(&wav)?;
            let t0 = std::time::Instant::now();
            let mut model = yatyat_lib::stt::LoadedModel::load_path(family, &model_path)?;
            let load_time = t0.elapsed();
            let text = if chunked {
                use yatyat_lib::chunking::{
                    context_start, stitch, to_absolute, words_text, CONTEXT_AFTER_SECS,
                };
                let after = (CONTEXT_AFTER_SECS * 16_000.0) as usize;
                let no_words = || anyhow::anyhow!("--chunked needs word timestamps (parakeet)");
                let cuts = yatyat_lib::chunking::plan_cuts(&samples, 0.25);
                let mut transcript = Vec::new();
                let mut from = 0;
                let t1 = std::time::Instant::now();
                for &cut in &cuts {
                    let start = context_start(from);
                    let words = model
                        .transcribe_words(&samples[start..cut + after])?
                        .ok_or_else(no_words)?;
                    stitch(&mut transcript, to_absolute(words, start), from as f32 / 16_000.0);
                    from = cut;
                }
                let chunk_time = t1.elapsed();
                let t2 = std::time::Instant::now();
                let whole = if cuts.is_empty() {
                    Some(model.transcribe(&samples)?)
                } else {
                    if from < samples.len() {
                        let start = context_start(from);
                        let mut tail = samples[start..].to_vec();
                        if tail.len() < yatyat_lib::audio::MIN_SAMPLES {
                            tail.resize(yatyat_lib::audio::MIN_SAMPLES, 0.0);
                        }
                        let words = model.transcribe_words(&tail)?.ok_or_else(no_words)?;
                        stitch(&mut transcript, to_absolute(words, start), from as f32 / 16_000.0);
                    }
                    None
                };
                let tail_time = t2.elapsed();
                eprintln!(
                    "model load: {load_time:?}, chunks: {} in {chunk_time:?} (during recording), \
                     tail: {:.1}s in {tail_time:?} (after stop), audio: {:.1}s, cuts at {:?}s",
                    cuts.len(),
                    (samples.len() - from) as f64 / 16_000.0,
                    samples.len() as f64 / 16_000.0,
                    cuts.iter()
                        .map(|&c| (c as f64 / 1_600.0).round() / 10.0)
                        .collect::<Vec<_>>()
                );
                whole.unwrap_or_else(|| words_text(&transcript))
            } else {
                let t1 = std::time::Instant::now();
                let text = model.transcribe(&samples)?;
                let infer_time = t1.elapsed();
                eprintln!(
                    "model load: {load_time:?}, inference: {infer_time:?}, audio: {:.1}s",
                    samples.len() as f64 / 16_000.0
                );
                text
            };
            if raw {
                println!("{text}");
            } else {
                println!("{}", clean(&text, &CleanOptions::default()));
            }
            Ok(())
        }
        Some(Command::Clean { keep_repeats }) => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            let opts = CleanOptions {
                collapse_repeats: !keep_repeats,
                ..Default::default()
            };
            println!("{}", clean(&input, &opts));
            Ok(())
        }
        Some(Command::BenchClean) => {
            // 1,000-word transcript with fillers sprinkled in.
            let mut words = Vec::with_capacity(1000);
            for i in 0..1000 {
                words.push(match i % 13 {
                    0 => "um",
                    5 => "uh",
                    9 => "hmm",
                    _ => "transcription",
                });
            }
            let text = words.join(" ");
            let opts = CleanOptions::default();
            // Warm up, then measure.
            for _ in 0..10 {
                std::hint::black_box(clean(&text, &opts));
            }
            let runs = 100;
            let t0 = std::time::Instant::now();
            for _ in 0..runs {
                std::hint::black_box(clean(&text, &opts));
            }
            let per_run = t0.elapsed() / runs;
            println!("cleanup(1000 words): {per_run:?} per run");
            Ok(())
        }
    }
}
