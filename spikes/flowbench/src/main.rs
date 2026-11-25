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
mod net;
mod pipeline;
mod quality;
mod replay;
mod report;
mod stats;
mod stt;
#[cfg(test)]
mod testserver;
mod vocab;

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use tokio::time::Instant;

use crate::audio::Clip;
use crate::cleanup::{ChatCleanup, CleanupProvider};
use crate::fake::{FakeCleanup, FakeSpeech};
use crate::lang::Lang;
use crate::report::RunInfo;
use crate::stt::SpeechProvider;
use crate::stt::deepgram::{self, DeepgramSpeech};
use crate::stt::openai::{self, OpenAiSpeech};
use crate::stt::speechmatics::{self, SpeechmaticsSpeech};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SttProvider {
    Openai,
    /// Streaming only (pipeline B); EU host by default
    Speechmatics,
    /// Streaming only (pipeline B); EU host by default
    Deepgram,
    /// Offline: canned text and simulated timings. Needs no key and no samples.
    Fake,
}

impl SttProvider {
    /// The streaming model when `--stt-live-model` is not given.
    fn default_live_model(self) -> &'static str {
        match self {
            SttProvider::Openai | SttProvider::Fake => "gpt-live-transcribe",
            SttProvider::Speechmatics => speechmatics::DEFAULT_MODEL,
            SttProvider::Deepgram => deepgram::DEFAULT_MODEL,
        }
    }

    /// The streaming URL when `--realtime-url` is not given.
    fn default_url(self) -> &'static str {
        match self {
            SttProvider::Openai | SttProvider::Fake => {
                "wss://api.openai.com/v1/realtime?intent=transcription"
            }
            SttProvider::Speechmatics => speechmatics::DEFAULT_URL,
            SttProvider::Deepgram => deepgram::DEFAULT_URL,
        }
    }
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

    /// Speech model for streaming (pipeline B). Default: gpt-live-transcribe (OpenAI),
    /// melia-1 (Speechmatics), nova-3 (Deepgram)
    #[arg(long)]
    stt_live_model: Option<String>,

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

    /// Cleanup prompt: v1 (the runs of 25 November), v2 (fillers named, examples), v3 (strict:
    /// change as little as possible), or a file
    #[arg(long, default_value = "v1")]
    prompt: String,

    /// The user's dictionary: one term per line, optionally `Term: sounds, like`. Sent to
    /// Speechmatics as additional_vocab and added to the cleanup prompt
    #[arg(long)]
    vocab: Option<PathBuf>,

    /// WebSocket URL for streaming transcription. Default: the provider's (EU for Speechmatics
    /// and Deepgram)
    #[arg(long)]
    realtime_url: Option<String>,

    /// How many times to run the whole sample set
    #[arg(long, default_value_t = 1)]
    runs: u32,

    /// Live microphone instead of samples: Enter to start, Enter to stop
    #[arg(long)]
    mic: bool,

    /// Language hints for the speech models, the same for every dictation; `off` sends none
    #[arg(long, default_value = "es,en")]
    languages: String,

    /// OpenAI: the live model's delay (minimal, low, medium, high, xhigh). Speechmatics:
    /// max_delay in seconds (0.7 to 4). Default: not sent
    #[arg(long)]
    stt_delay: Option<String>,

    /// Deepgram: end the dictation on the Finalize reply instead of waiting for CloseStream to
    /// process all audio. Faster, but lost final words in the first real run
    #[arg(long)]
    deepgram_finalize_only: bool,

    /// Print the TCP round trip to each speech and cleanup host, then stop. No key needed
    #[arg(long)]
    net: bool,

    /// Hosts for --net, instead of the built-in list; repeat the flag for several
    #[arg(long, value_name = "HOST")]
    net_host: Vec<String>,

    /// Replay saved raw transcripts through the cleanup only: no speaking, no speech provider
    #[arg(long)]
    replay: bool,

    /// Transcripts for --replay: a JSON file, or a results file that still has its texts
    #[arg(long, value_name = "PATH")]
    transcripts: Option<PathBuf>,

    /// The owner's corrected texts, for zero-edit scoring in --replay
    #[arg(long, value_name = "PATH")]
    gold: Option<PathBuf>,

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
    if args.net {
        let hosts: Vec<String> = if args.net_host.is_empty() {
            net::DEFAULT_HOSTS.iter().map(|h| (*h).to_owned()).collect()
        } else {
            args.net_host.clone()
        };
        print!("{}", net::report(&hosts).await);
        return Ok(());
    }
    if args.replay {
        let saved = replay::load_transcripts(
            &args
                .transcripts
                .clone()
                .unwrap_or_else(|| samples_dir().join("transcripts.json")),
        )?;
        let saved = only_named(saved, &args.samples)?;
        if args.stt_provider == SttProvider::Fake {
            return replay_run(&args, &saved, &FakeCleanup).await;
        }
        let llm = chat_cleanup(&args, http_client()?)?;
        return replay_run(&args, &saved, &llm).await;
    }
    // Samples are read before any key is read and before anything touches the network.
    let clips = match (args.mic, args.stt_provider) {
        (true, _) => Vec::new(),
        (false, SttProvider::Fake) => fake::clips(),
        (false, _) => audio::load_clips(&samples_dir(), &args.samples)?,
    };
    if args.stt_provider == SttProvider::Fake {
        return run(&args, clips, &FakeSpeech, &FakeCleanup).await;
    }
    // One client for the upload and the cleanup, so both use the same warm connection.
    let http = http_client()?;
    let llm = chat_cleanup(&args, http.clone())?;
    let languages = match args.languages.as_str() {
        "off" => Vec::new(),
        list => list.split(',').map(|code| code.trim().to_owned()).collect(),
    };
    let trace = std::env::var_os("FLOWBENCH_TRACE").is_some();
    let live_model = live_model(&args);
    let url = realtime_url(&args);
    match args.stt_provider {
        SttProvider::Openai => {
            let stt = OpenAiSpeech {
                http,
                key: http::key_from_env(openai::KEY_ENV)?,
                file_url: openai::FILE_URL.to_owned(),
                realtime_url: url,
                file_model: args.stt_file_model.clone(),
                live_model,
                languages,
                delay: args.stt_delay.clone(),
                trace,
            };
            run(&args, clips, &stt, &llm).await
        }
        SttProvider::Speechmatics => {
            let max_delay = args
                .stt_delay
                .as_deref()
                .map(|d| d.parse::<f64>())
                .transpose()
                .context("--stt-delay for Speechmatics is max_delay in seconds, such as 0.7")?;
            let vocab = vocab_terms(&args)?;
            if !vocab.is_empty() && !speechmatics::takes_vocab(&live_model) {
                eprintln!(
                    "Speechmatics {live_model} refuses additional_vocab; the dictionary goes only \
                     to the cleanup prompt"
                );
            }
            let stt = SpeechmaticsSpeech {
                key: http::key_from_env(speechmatics::KEY_ENV)?,
                url,
                model: live_model,
                languages,
                max_delay,
                vocab,
                trace,
            };
            run(&args, clips, &stt, &llm).await
        }
        SttProvider::Deepgram => {
            if args.stt_delay.is_some() {
                bail!("--stt-delay has no meaning for Deepgram");
            }
            let stt = DeepgramSpeech {
                key: http::key_from_env(deepgram::KEY_ENV)?,
                url,
                model: live_model,
                languages,
                wait_for_close: !args.deepgram_finalize_only,
                trace,
            };
            run(&args, clips, &stt, &llm).await
        }
        SttProvider::Fake => unreachable!("handled above"),
    }
}

fn live_model(args: &Args) -> String {
    args.stt_live_model
        .clone()
        .unwrap_or_else(|| args.stt_provider.default_live_model().to_owned())
}

fn realtime_url(args: &Args) -> String {
    args.realtime_url
        .clone()
        .unwrap_or_else(|| args.stt_provider.default_url().to_owned())
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("creating the HTTP client")
}

fn chat_cleanup(args: &Args, http: reqwest::Client) -> Result<ChatCleanup> {
    Ok(ChatCleanup {
        http,
        base_url: args.llm_base_url.trim_end_matches('/').to_owned(),
        key: http::key_from_env(&args.llm_key_env)?,
        key_env: args.llm_key_env.clone(),
        model: args.llm_model.clone(),
        reasoning_effort: (args.reasoning_effort != "off").then(|| args.reasoning_effort.clone()),
        prompt: cleanup::prompt_named(&args.prompt)? + &vocab::prompt_line(&vocab_terms(args)?),
    })
}

fn vocab_terms(args: &Args) -> Result<Vec<vocab::Term>> {
    args.vocab
        .as_deref()
        .map(vocab::load)
        .transpose()
        .map(Option::unwrap_or_default)
}

/// Keeps the named samples, in the order given; all when no name is given.
fn only_named(saved: Vec<replay::Saved>, names: &[String]) -> Result<Vec<replay::Saved>> {
    if names.is_empty() {
        return Ok(saved);
    }
    names
        .iter()
        .map(|name| {
            saved
                .iter()
                .find(|s| &s.sample == name)
                .cloned()
                .with_context(|| format!("no saved transcript for {name}"))
        })
        .collect()
}

/// `--replay`: the transcripts through the cleanup, then the table and two files: numbers in
/// docs/results/, texts in samples/.
async fn replay_run<C: CleanupProvider>(
    args: &Args,
    saved: &[replay::Saved],
    llm: &C,
) -> Result<()> {
    let fake = args.stt_provider == SttProvider::Fake;
    if fake {
        println!("{FAKE_BANNER}\n");
    }
    llm.warm()
        .await
        .context("the cleanup connection could not be opened")?;
    let gold_path = args
        .gold
        .clone()
        .unwrap_or_else(|| samples_dir().join("gold.json"));
    let gold = replay::load_gold(&gold_path)?;
    println!(
        "Replaying {} transcript(s), {} run(s), through {} at {} with prompt {}.\n\
         Times in ms from the moment the request is sent.\n",
        saved.len(),
        args.runs,
        args.llm_model,
        args.llm_base_url,
        args.prompt
    );
    let deadline = Duration::from_millis(args.llm_deadline_ms);
    let results = replay::run_all(llm, saved, args.runs, deadline).await;
    let rows = replay::summarise(&results, &gold);
    println!("\n{}", replay::table(&rows));

    let (date, clock) = report::today()?;
    let mut settings = vec![
        ("Machine", report::machine()),
        ("Transcripts", saved.len().to_string()),
        ("Runs", args.runs.to_string()),
        ("Cleanup deadline", format!("{} ms", args.llm_deadline_ms)),
        ("Prompt", format!("`{}`", args.prompt)),
        (
            "Tool",
            format!("`spikes/flowbench` at {}", report::commit()),
        ),
    ];
    if !fake {
        let host = net::host_of(&args.llm_base_url).unwrap_or("the cleanup host");
        settings.extend([
            (
                "Cleanup",
                format!("`{}` at `{}`", args.llm_model, args.llm_base_url),
            ),
            ("Reasoning effort", args.reasoning_effort.clone()),
            (
                "Network",
                format!(
                    "{}; TCP round trip {}",
                    report::network_interface(),
                    net::summary(host).await
                ),
            ),
        ]);
    }
    let markdown = replay::markdown((&date, &clock), &settings, &rows, !gold.is_empty());
    let slug: String = args
        .llm_model
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let title = format!(
        "Replay {date}, {} with prompt {}",
        args.llm_model, args.prompt
    );
    let texts = replay::texts(&results, &gold, &title);
    if fake {
        let dir = crate_dir().join("target");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("replay-fake.md"), &markdown)?;
        println!(
            "\nResults written to {}",
            dir.join("replay-fake.md").display()
        );
        return Ok(());
    }
    let numbers = report::write_new(
        &crate_dir().join("../../docs/results"),
        &format!("replay-{date}"),
        &markdown,
    )?;
    let private = report::write_new(&samples_dir(), &format!("replay-{date}-{slug}"), &texts)?;
    println!("\nNumbers written to {}", numbers.display());
    println!(
        "Texts written to {} (not in the repository)",
        private.display()
    );
    if gold.is_empty() {
        let draft = samples_dir().join("gold-draft.json");
        std::fs::write(&draft, replay::gold_draft(&results))?;
        println!(
            "No gold texts yet. Correct {} to what you would send, save it as {}, and the \
             next replay scores zero-edit rate.",
            draft.display(),
            gold_path.display()
        );
    }
    Ok(())
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
    let mut network = format!(
        "{}; cold HTTPS request to the cleanup host {cold_ms} ms, warm round trip {warm_ms} ms",
        report::network_interface()
    );
    if !fake {
        let speech_url = realtime_url(args);
        let mut hosts = vec![net::host_of(&speech_url), net::host_of(&args.llm_base_url)];
        hosts.dedup();
        let trips: Vec<String> =
            futures_util::future::join_all(hosts.into_iter().flatten().map(net::summary)).await;
        network.push_str(&format!("; TCP round trip {}", trips.join(", ")));
    }
    let mut settings = vec![
        ("Samples", samples),
        ("Runs", args.runs.to_string()),
        ("Cleanup deadline", format!("{} ms", args.llm_deadline_ms)),
    ];
    if !fake {
        settings.extend([
            (
                "Speech, streaming (B)",
                format!("`{}` at `{}`", live_model(args), realtime_url(args)),
            ),
            (
                "Speech, upload (A)",
                if args.stt_provider == SttProvider::Openai {
                    format!("`{}`", args.stt_file_model)
                } else {
                    "not run: this provider only streams".to_owned()
                },
            ),
            ("Language hints", args.languages.clone()),
            (
                "Live model delay",
                args.stt_delay.clone().unwrap_or("not sent".to_owned()),
            ),
            ("Prompt", format!("`{}`", args.prompt)),
            (
                "Dictionary",
                match vocab_terms(args) {
                    Ok(terms) if !terms.is_empty() => format!("{} terms", terms.len()),
                    _ => "none".to_owned(),
                },
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
    if !fake {
        let saved = samples_dir().join("transcripts.json");
        let count = replay::save_transcripts(&saved, &results)?;
        println!(
            "Raw transcripts of {count} sample(s) saved to {} for --replay (not in the repository)",
            saved.display()
        );
    }
    Ok(())
}
