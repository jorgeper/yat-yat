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
        /// Replay live transcription over the file and report what each
        /// on-screen update costs: the old way (re-read the whole recording
        /// so far) against the chunked way (read only since the last cut).
        #[arg(long)]
        live: bool,
        /// Parakeet only: the folder holds the full-precision (fp32) files.
        #[arg(long)]
        full_precision: bool,
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
            live,
            full_precision,
            wav,
        }) => {
            let family = match engine.as_str() {
                "whisper" => EngineFamily::Whisper,
                "parakeet" => EngineFamily::Parakeet,
                other => anyhow::bail!("unknown engine '{other}' (whisper|parakeet)"),
            };
            let samples = yatyat_lib::stt::read_wav_16k_mono(&wav)?;
            let t0 = std::time::Instant::now();
            let mut model = yatyat_lib::stt::LoadedModel::load_path_with(
                family,
                &model_path,
                if full_precision {
                    yatyat_lib::registry::Precision::Fp32
                } else {
                    yatyat_lib::registry::Precision::Int8
                },
            )?;
            let load_time = t0.elapsed();
            if live {
                // Replay what the app's chunker does in live mode, in audio
                // time: poll every 250 ms, commit a chunk when a cut is
                // found, otherwise preview the audio since the last cut (at
                // most every 500 ms). Prints every text the overlay would be
                // sent, and what each preview cost against re-reading the
                // whole recording so far.
                use yatyat_lib::chunking::{
                    context_start, next_chunk, preview_text, stitch, to_absolute,
                };
                let no_words = || anyhow::anyhow!("--live needs word timestamps (parakeet)");
                // YATYAT_LIVE_SKIP_WHOLE=1 skips timing the old whole-recording
                // pass (slow on long files) when only the new path matters.
                let skip_whole = std::env::var_os("YATYAT_LIVE_SKIP_WHOLE").is_some();
                // Audio time advances by the poll interval, plus whatever each
                // pass took: the app cannot look again while one is running.
                let poll = (yatyat_lib::live::PREVIEW_POLL.as_secs_f64() * 16_000.0) as usize;
                let mut committed = 0usize;
                let mut words = Vec::new();
                let mut next_preview = 0usize;
                let mut filter = yatyat_lib::chunking::PreviewFilter::default();
                let (mut whole_total, mut tail_total, mut whole_max, mut tail_max) =
                    (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                let mut previews = 0;
                let mut lags = Vec::new();
                let mut seen = poll;
                while seen <= samples.len() {
                    let start = context_start(committed);
                    let buf = &samples[start..seen];
                    if let Some(chunk) = next_chunk(buf, committed - start) {
                        let t = std::time::Instant::now();
                        let pass = model.transcribe_words(&buf[..chunk.end])?.ok_or_else(no_words)?;
                        stitch(&mut words, to_absolute(pass, start), committed as f32 / 16_000.0);
                        committed = start + chunk.keep_to;
                        eprintln!("{:>6.2}s  cut at {:.2}s", seen as f64 / 16_000.0, committed as f64 / 16_000.0);
                        seen += (t.elapsed().as_secs_f64() * 16_000.0) as usize;
                        next_preview = seen;
                    } else if seen >= next_preview && buf.len() >= yatyat_lib::chunking::PREVIEW_MIN_SAMPLES {
                        let t = std::time::Instant::now();
                        let pass = model.transcribe_words(buf)?.ok_or_else(no_words)?;
                        let pass_time = t.elapsed();
                        let tail_time = pass_time.as_secs_f64();
                        let seam = committed as f32 / 16_000.0;
                        let tail = to_absolute(pass, start);
                        let mut all = words.clone();
                        if all.is_empty() {
                            all = tail.clone();
                        } else {
                            stitch(&mut all, tail.clone(), seam);
                        }
                        let text = preview_text(&words, tail, seam);
                        let final_words = yatyat_lib::chunking::final_word_count(&words, committed);
                        let text = filter.confirmed(&text, final_words);
                        if !skip_whole {
                            let t = std::time::Instant::now();
                            model.transcribe(&samples[..seen])?;
                            let whole_time = t.elapsed().as_secs_f64();
                            whole_total += whole_time;
                            whole_max = whole_max.max(whole_time);
                        }
                        // The text reaches the screen when the pass is done.
                        seen += (tail_time * 16_000.0) as usize;
                        let shown = text.split_whitespace().count();
                        if let Some(word) = shown.checked_sub(1).and_then(|i| all.get(i)) {
                            lags.push(seen as f64 / 16_000.0 - word.start as f64);
                        }
                        println!("{:.2}\t{text}", seen as f64 / 16_000.0);
                        tail_total += tail_time;
                        tail_max = tail_max.max(tail_time);
                        previews += 1;
                        next_preview = seen
                            + (yatyat_lib::live::preview_gap(pass_time).as_secs_f64() * 16_000.0) as usize;
                    }
                    seen += poll;
                }
                lags.sort_by(|a, b| a.total_cmp(b));
                if !lags.is_empty() {
                    eprintln!(
                        "newest word on screen was spoken: median {:.2}s ago, 90th percentile {:.2}s ago",
                        lags[lags.len() / 2],
                        lags[lags.len() * 9 / 10]
                    );
                }
                eprintln!(
                    "{previews} previews over {:.1}s: whole-so-far {whole_total:.2}s total, worst {:.0} ms; \
                     since-last-cut {tail_total:.2}s total, worst {:.0} ms",
                    samples.len() as f64 / 16_000.0,
                    whole_max * 1e3,
                    tail_max * 1e3
                );
                return Ok(());
            }
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
