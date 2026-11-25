//! flowbench: the Phase 0 speed test for HyperMe.
//!
//! Measures the time from key release to cleaned text for two pipelines, on recorded samples or
//! on the live microphone. The plan is `docs/SPEED_TEST.md`.

mod audio;
mod cleanup;
mod fake;
mod http;
mod lang;
mod mic;
mod pipeline;
mod report;
mod stats;
mod stt;
#[cfg(test)]
mod testserver;

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use tokio::time::Instant;

use crate::audio::Clip;
use crate::cleanup::{ChatCleanup, CleanupProvider};
use crate::fake::{FakeCleanup, FakeSpeech};
use crate::lang::Lang;
use crate::report::RunInfo;
use crate::stt::SpeechProvider;
use crate::stt::openai::{self, OpenAiSpeech};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SttProvider {
    Openai,
    /// Offline: canned text and simulated timings. Needs no key and no samples.
    Fake,
}

/// Phase 0 speed test: key release to cleaned text, upload against streaming.
///
/// With no arguments it runs every WAV in samples/ through both pipelines and writes
/// docs/results/speed-test-YYYY-MM-DD.md. Model names and URLs are configuration: the defaults
/// below are the ones in docs/SPEED_TEST.md.
#[derive(Parser)]
struct Args {
    /// Speech provider
    #[arg(long, value_enum, default_value = "openai")]
    stt_provider: SttProvider,

    /// Speech model for streaming (pipeline B)
    #[arg(long, default_value = "gpt-live-transcribe")]
    stt_live_model: String,

    /// Speech model for file upload (pipeline A)
    #[arg(long, default_value = "gpt-transcribe")]
    stt_file_model: String,

    /// Cleanup model
    #[arg(long, default_value = "gpt-6-luna")]
    llm_model: String,

    /// Base URL of an OpenAI-compatible chat API for the cleanup
    #[arg(long, default_value = "https://api.openai.com/v1")]
    llm_base_url: String,

    /// Environment variable that holds the cleanup key
    #[arg(long, default_value = openai::KEY_ENV)]
    llm_key_env: String,

    /// Cleanup deadline in ms; after it the raw transcript is used and counted as a fallback
    #[arg(long, default_value_t = 1500)]
    llm_deadline_ms: u64,

    /// Reasoning effort for the cleanup model; `off` leaves the field out
    #[arg(long, default_value = "none")]
    reasoning_effort: String,

    /// WebSocket URL for streaming transcription
    #[arg(
        long,
        default_value = "wss://api.openai.com/v1/realtime?intent=transcription"
    )]
    realtime_url: String,

    /// How many times to run the whole sample set
    #[arg(long, default_value_t = 1)]
    runs: u32,

    /// Live microphone instead of samples: Enter to start, Enter to stop
    #[arg(long)]
    mic: bool,

    /// Language hints for the speech models, the same for every dictation; `off` sends none
    #[arg(long, default_value = "es,en")]
    languages: String,

    /// The live model's delay setting (minimal, low, medium, high, xhigh); default: not sent
    #[arg(long)]
    stt_delay: Option<String>,

    /// Record one sample from the microphone to samples/NAME.wav, for example es-01
    #[arg(long, value_name = "NAME")]
    record: Option<String>,

    /// Sample names to run, for example es-01 en-01; default: every WAV in samples/
    samples: Vec<String>,
}

/// No single HTTP request may hang the run.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const FAKE_BANNER: &str =
    "FAKE PROVIDERS: every number and text here is simulated. This is not a measurement.";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn samples_dir() -> PathBuf {
    crate_dir().join("samples")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(name) = &args.record {
        return mic::record(name, &samples_dir()).await;
    }
    // Samples are read before any key is read and before anything touches the network.
    let clips = match (args.mic, args.stt_provider) {
        (true, _) => Vec::new(),
        (false, SttProvider::Fake) => fake::clips(),
        (false, SttProvider::Openai) => audio::load_clips(&samples_dir(), &args.samples)?,
    };
    match args.stt_provider {
        SttProvider::Fake => run(&args, clips, &FakeSpeech, &FakeCleanup).await,
        SttProvider::Openai => {
            // One client for the upload and the cleanup, so both use the same warm connection.
            let http = reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .context("creating the HTTP client")?;
            let languages = match args.languages.as_str() {
                "off" => Vec::new(),
                list => list.split(',').map(|code| code.trim().to_owned()).collect(),
            };
            let stt = OpenAiSpeech {
                http: http.clone(),
                key: http::key_from_env(openai::KEY_ENV)?,
                file_url: openai::FILE_URL.to_owned(),
                realtime_url: args.realtime_url.clone(),
                file_model: args.stt_file_model.clone(),
                live_model: args.stt_live_model.clone(),
                languages,
                delay: args.stt_delay.clone(),
                trace: std::env::var_os("FLOWBENCH_TRACE").is_some(),
            };
            let llm = ChatCleanup {
                http,
                base_url: args.llm_base_url.trim_end_matches('/').to_owned(),
                key: http::key_from_env(&args.llm_key_env)?,
                key_env: args.llm_key_env.clone(),
                model: args.llm_model.clone(),
                reasoning_effort: (args.reasoning_effort != "off")
                    .then(|| args.reasoning_effort.clone()),
            };
            run(&args, clips, &stt, &llm).await
        }
    }
}

async fn timed(work: impl Future<Output = Result<()>>) -> Result<u128> {
    let start = Instant::now();
    work.await?;
    Ok(start.elapsed().as_millis())
}

async fn run<S: SpeechProvider, C: CleanupProvider>(
    args: &Args,
    clips: Vec<Clip>,
    stt: &S,
    llm: &C,
) -> Result<()> {
    let fake = args.stt_provider == SttProvider::Fake;
    let deadline = Duration::from_millis(args.llm_deadline_ms);
    if fake {
        println!("{FAKE_BANNER}\n");
    }

    // Before any audio is sent: check the cleanup key and URL, and time the connection.
    let cold_ms = timed(llm.warm())
        .await
        .context("the cleanup connection could not be opened")?;
    let warm_ms = timed(llm.warm()).await?;

    if args.mic {
        return mic::dictate(stt, llm, deadline).await;
    }
    let count = |lang| clips.iter().filter(|c| c.lang == Some(lang)).count();
    let samples = format!("{} Spanish, {} English", count(Lang::Es), count(Lang::En));
    println!(
        "{samples}; {} run(s); both pipelines. Times in ms from key release.\n",
        args.runs
    );

    let results = pipeline::run_all(stt, llm, &clips, args.runs, deadline).await;
    let rows = stats::aggregate(&results);
    println!("\n{}", report::console_texts(&results));
    println!("{}", report::console_table(&rows));
    if fake {
        println!("{FAKE_BANNER}");
    }
    for line in report::gate_lines(&rows, &results) {
        println!("{line}");
    }

    let (date, clock) = report::today()?;
    let network = format!(
        "{}; cold HTTPS request to the cleanup host {cold_ms} ms, warm round trip {warm_ms} ms",
        report::network_interface()
    );
    let mut settings = vec![
        ("Samples", samples),
        ("Runs", args.runs.to_string()),
        ("Cleanup deadline", format!("{} ms", args.llm_deadline_ms)),
    ];
    if !fake {
        settings.extend([
            (
                "Speech, streaming (B)",
                format!("`{}` at `{}`", args.stt_live_model, args.realtime_url),
            ),
            ("Speech, upload (A)", format!("`{}`", args.stt_file_model)),
            ("Language hints", args.languages.clone()),
            (
                "Live model delay",
                args.stt_delay.clone().unwrap_or("not sent".to_owned()),
            ),
            (
                "Cleanup",
                format!("`{}` at `{}`", args.llm_model, args.llm_base_url),
            ),
            ("Reasoning effort", args.reasoning_effort.clone()),
        ]);
    }
    let info = RunInfo {
        date,
        clock,
        fake,
        machine: report::machine(),
        network: if fake { "not used".to_owned() } else { network },
        commit: report::commit(),
        settings,
    };
    let markdown = report::markdown(&info, &rows, &results);
    let path = if fake {
        // Simulated numbers never go into docs/results/.
        let path = crate_dir().join("target").join("speed-test-fake.md");
        std::fs::create_dir_all(crate_dir().join("target"))?;
        std::fs::write(&path, markdown).with_context(|| format!("writing {}", path.display()))?;
        path
    } else {
        let dir = crate_dir().join("../../docs/results");
        report::write_new(&dir, &format!("speed-test-{}", info.date), &markdown)?
    };
    println!("\nResults written to {}", path.display());
    Ok(())
}
