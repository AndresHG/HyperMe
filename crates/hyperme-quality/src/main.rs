//! `hyperme-quality`: replays a set of saved dictations through the app's own state machine
//! and providers, and scores the pasted text against the owner's corrected texts (issue 99).
//!
//! ```sh
//! cargo run --release -p hyperme-quality -- run --set spikes/flowbench/samples
//! cargo run --release -p hyperme-quality -- import --set ~/hyperme-quality --days 7
//! ```
//!
//! See `crates/hyperme-quality/README.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hyperme_core::config::{CleanupLevel, Config};
use hyperme_core::dictation::Settings;
use hyperme_core::dictionary::Dictionary;
use hyperme_providers::cleanup::{ChatCleanup, prompt_version};
use hyperme_providers::speechmatics::Speechmatics;
use hyperme_quality::replay::{Replayed, replay, replay_raw};
use hyperme_quality::report::{gold_draft, markdown, summarise, table, texts};
use hyperme_quality::set::{self, Lang};

const USAGE: &str = "\
usage:
  hyperme-quality run --set DIR [--names FILE] [--dictionary FILE] [--only es-01,en-02]
                      [--level light] [--prompt v4] [--deadline-ms N] [--config FILE] [--network TEXT]
                      [--reuse-raw FILE [--pace-ms N]] [--out FILE | --no-results]
  hyperme-quality import --set DIR [--history DIR] [--days N]

Keys come from SPEECHMATICS_API_KEY / GROQ_API_KEY (the provider name in capitals, then
_API_KEY), or else from the keychain entries the app uses.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("run") => Options::parse(&args[1..]).and_then(|o| run(&o)),
        Some("import") => Options::parse(&args[1..]).and_then(|o| import(&o)),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Default)]
struct Options {
    set: Option<PathBuf>,
    names: Option<PathBuf>,
    dictionary: Option<PathBuf>,
    reuse_raw: Option<PathBuf>,
    pace_ms: Option<u64>,
    only: Vec<String>,
    prompt: Option<String>,
    level: Option<CleanupLevel>,
    deadline_ms: Option<u64>,
    config: Option<PathBuf>,
    network: Option<String>,
    out: Option<PathBuf>,
    no_results: bool,
    history: Option<PathBuf>,
    days: Option<u64>,
}

impl Options {
    fn parse(args: &[String]) -> Result<Options, String> {
        let mut o = Options::default();
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value\n\n{USAGE}"))
            };
            let number = |v: String| {
                v.parse::<u64>()
                    .map_err(|_| format!("{flag} takes a number, not {v:?}"))
            };
            match flag.as_str() {
                "--set" => o.set = Some(value()?.into()),
                "--names" => o.names = Some(value()?.into()),
                "--dictionary" => o.dictionary = Some(value()?.into()),
                "--reuse-raw" => o.reuse_raw = Some(value()?.into()),
                "--pace-ms" => o.pace_ms = Some(number(value()?)?),
                "--only" => {
                    o.only = value()?.split(',').map(|s| s.trim().to_owned()).collect();
                }
                "--prompt" => o.prompt = Some(value()?),
                "--level" => {
                    o.level = Some(match value()?.as_str() {
                        "none" => CleanupLevel::None,
                        "light" => CleanupLevel::Light,
                        "medium" => CleanupLevel::Medium,
                        other => {
                            return Err(format!(
                                "--level takes none, light or medium, not {other:?}"
                            ));
                        }
                    });
                }
                "--deadline-ms" => o.deadline_ms = Some(number(value()?)?),
                "--config" => o.config = Some(value()?.into()),
                "--network" => o.network = Some(value()?),
                "--out" => o.out = Some(value()?.into()),
                "--no-results" => o.no_results = true,
                "--history" => o.history = Some(value()?.into()),
                "--days" => o.days = Some(number(value()?)?),
                other => return Err(format!("unknown option {other}\n\n{USAGE}")),
            }
        }
        Ok(o)
    }

    fn set(&self) -> Result<&Path, String> {
        self.set
            .as_deref()
            .ok_or_else(|| format!("--set is required\n\n{USAGE}"))
    }
}

/// The app's data folder: `~/Library/Application Support/HyperMe` on macOS.
fn data_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/HyperMe")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map_or_else(|| home.join(".local/share"), PathBuf::from)
            .join("HyperMe")
    }
}

/// The app's config, read but never created, or the defaults when there is none.
fn config(path: Option<&Path>) -> Result<(Config, String), String> {
    let path = path.map_or_else(|| data_dir().join("config.toml"), Path::to_owned);
    match std::fs::read_to_string(&path) {
        Ok(text) => Config::parse(&text, &path)
            .map(|c| (c, path.display().to_string()))
            .map_err(|e| e.to_string()),
        Err(_) => Ok((Config::default(), "the built-in defaults".to_owned())),
    }
}

/// A provider's key from the environment, or from the app's keychain entry. Never printed.
fn key(provider: &str) -> Result<String, String> {
    let var = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));
    if let Ok(key) = std::env::var(&var)
        && !key.trim().is_empty()
    {
        return Ok(key.trim().to_owned());
    }
    keychain_key(provider)?.ok_or_else(|| {
        format!("no {provider} key: set {var}, or add the key in HyperMe's Settings first")
    })
}

#[cfg(target_os = "macos")]
fn keychain_key(provider: &str) -> Result<Option<String>, String> {
    use hyperme_core::secrets::SecretStore;
    hyperme_platform::keychain::KeychainStore::new()
        .get(provider)
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn keychain_key(_provider: &str) -> Result<Option<String>, String> {
    Ok(None)
}

fn command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !text.is_empty()).then_some(text)
}

/// Model, chip, memory and system version. No host or user name.
fn machine() -> String {
    let memory = command("sysctl", &["-n", "hw.memsize"])
        .and_then(|bytes| bytes.parse::<u64>().ok())
        .map(|bytes| format!("{} GB", bytes >> 30));
    let parts: Vec<String> = [
        command("sysctl", &["-n", "hw.model"]),
        command("sysctl", &["-n", "machdep.cpu.brand_string"]),
        memory,
        command("sw_vers", &["-productVersion"]).map(|v| format!("macOS {v}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    } else {
        parts.join(", ")
    }
}

/// `path`, or `path` with `-2`, `-3` ... before the extension: a file is never overwritten.
fn unused(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut path = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    path
}

/// The repository's `docs/results/`, found from the current folder.
fn results_dir() -> Option<PathBuf> {
    let here = std::env::current_dir().ok()?;
    here.ancestors()
        .find(|a| a.join(".git").exists())
        .map(|root| root.join("docs/results"))
        .filter(|dir| dir.is_dir())
}

fn run(o: &Options) -> Result<(), String> {
    let dir = o.set()?;
    set::check_location(dir)?;
    let set = set::load(dir, &o.only, o.names.as_deref())?;
    let (mut config, config_from) = config(o.config.as_deref())?;
    if let Some(level) = o.level {
        config.cleanup.level = level;
        // A level on the command line means its own prompt, not the config's override.
        config.cleanup.prompt = None;
    }
    if let Some(prompt) = &o.prompt {
        config.cleanup.prompt = Some(prompt.clone());
    }
    if let Some(ms) = o.deadline_ms {
        config.cleanup.deadline_ms = ms;
    }
    let dictionary = match &o.dictionary {
        Some(path) => Dictionary::parse(
            &std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?,
        ),
        None => Dictionary::default(),
    };
    let cleanup = ChatCleanup::new(&config.cleanup, &key(&config.cleanup.provider)?)
        .map_err(|e| e.message)?
        .with_dictionary(&dictionary.prompt_line());
    let settings = Settings {
        dictionary: dictionary.clone(),
        ..Settings::from_config(&config)
    };
    let saved = match &o.reuse_raw {
        Some(path) => Some(set::read_raw(path)?),
        None => None,
    };

    let audio_s: f64 = set
        .samples
        .iter()
        .map(|s| s.pcm.len() as f64 / f64::from(set::SAMPLE_RATE))
        .sum();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("starting the runtime: {e}"))?;
    let results = match &saved {
        Some(raw) => {
            eprintln!(
                "Cleaning {} saved transcripts (cleanup only) from {}",
                set.samples.len(),
                set.dir.display()
            );
            let pace = Duration::from_millis(o.pace_ms.unwrap_or(0));
            runtime.block_on(replay_raw(
                &cleanup,
                settings,
                &set.samples,
                raw,
                pace,
                |r| eprintln!("{}", line(r)),
            ))
        }
        None => {
            let speech = Speechmatics::new(&config.speech, key(&config.speech.provider)?);
            eprintln!(
                "Replaying {} samples ({:.0} s of audio, in real time) from {}",
                set.samples.len(),
                audio_s,
                set.dir.display()
            );
            runtime.block_on(replay(&speech, &cleanup, settings, &set.samples, |r| {
                eprintln!("{}", line(r));
            }))
        }
    };

    let rows = summarise(&results, &set.gold, &set.names);
    println!("\n{}", table(&rows));

    let date = command("date", &["+%F"]).unwrap_or_else(|| {
        let days = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs()
            / 86_400;
        format!("day-{days}")
    });
    let clock = command("date", &["+%H:%M %Z"]).unwrap_or_default();
    let count = |lang| results.iter().filter(|r| r.lang == Some(lang)).count();
    let with_gold = results
        .iter()
        .filter(|r| set.gold.contains_key(&r.sample))
        .count();
    let c = &config.cleanup;
    let setup = [
        ("Machine", machine()),
        (
            "Network",
            o.network.clone().unwrap_or_else(|| "not stated".to_owned()),
        ),
        (
            "Speech",
            match &o.reuse_raw {
                Some(path) => format!(
                    "not called: raw transcripts reused from `{}` (an earlier live run), \
                     cleanup only{}",
                    path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned()
                    ),
                    o.pace_ms
                        .map_or(String::new(), |ms| format!(", {ms} ms between samples"))
                ),
                None => format!("{} `{}`", config.speech.provider, config.speech.model),
            },
        ),
        (
            "Cleanup",
            format!(
                "{} `{}`, reasoning {}, level {}, prompt `{}`, deadline {} ms",
                c.provider,
                c.model,
                c.reasoning_effort,
                level_name(c.level),
                if c.level == CleanupLevel::None && c.prompt.is_none() {
                    "none: no model call"
                } else {
                    prompt_version(c)
                },
                c.deadline_ms
            ),
        ),
        (
            "Short dictations",
            format!(
                "under {} words skip the cleanup",
                config.dictation.short_dictation_words
            ),
        ),
        (
            "Samples",
            format!(
                "{} ({} es, {} en), {:.0} s of audio, {} with a corrected text",
                results.len(),
                count(Lang::Es),
                count(Lang::En),
                audio_s,
                with_gold
            ),
        ),
        ("Names list", format!("{} names", set.names.len())),
        (
            "Dictionary",
            if dictionary.is_empty() {
                "none".to_owned()
            } else {
                format!(
                    "{} terms, substituted and in the prompt",
                    dictionary.terms().len()
                )
            },
        ),
        ("Config", config_source(&config_from)),
        (
            "Tool",
            format!(
                "`hyperme-quality` at `{}`",
                command("git", &["rev-parse", "--short", "HEAD"]).unwrap_or_default()
            ),
        ),
    ];

    // Private outputs go to the set folder only (AC-5).
    let texts_path = unused(&set.dir, &format!("quality-texts-{date}"), "md");
    std::fs::write(
        &texts_path,
        texts(
            &results,
            &set.gold,
            &format!("Quality replay texts, {date}"),
        ),
    )
    .map_err(|e| format!("writing {}: {e}", texts_path.display()))?;
    eprintln!("Texts (private): {}", texts_path.display());
    // A live run's raw transcripts, for cleanup-only replays with --reuse-raw.
    if saved.is_none() {
        let raw: BTreeMap<String, String> = results
            .iter()
            .filter_map(|r| r.raw.clone().map(|raw| (r.sample.clone(), raw)))
            .collect();
        let raw_path = unused(&set.dir, &format!("raw-{date}"), "json");
        set::write_raw(&raw_path, &raw)?;
        eprintln!(
            "Raw transcripts (private), for --reuse-raw: {}",
            raw_path.display()
        );
    }
    let missing: BTreeMap<String, String> =
        serde_json::from_str::<BTreeMap<String, String>>(&gold_draft(&results))
            .unwrap_or_default()
            .into_iter()
            .filter(|(name, _)| !set.gold.contains_key(name))
            .collect();
    let drafted = set::add_to_draft(&set.dir, &missing)?;
    if drafted > 0 {
        eprintln!(
            "{drafted} samples have no corrected text yet: added to {}. Correct them and move \
             them into gold.json.",
            set.dir.join("gold-draft.json").display()
        );
    }

    if o.no_results {
        return Ok(());
    }
    let out = match &o.out {
        Some(path) => Some(path.clone()),
        None => results_dir().map(|dir| unused(&dir, &format!("quality-{date}"), "md")),
    };
    match out {
        Some(path) => {
            let md = markdown(
                (&date, &clock),
                &setup,
                &rows,
                with_gold > 0,
                !set.names.is_empty(),
            );
            std::fs::write(&path, md).map_err(|e| format!("writing {}: {e}", path.display()))?;
            eprintln!("Results (numbers only): {}", path.display());
        }
        None => eprintln!("No docs/results/ here: pass --out to write the results file."),
    }
    Ok(())
}

fn level_name(level: CleanupLevel) -> &'static str {
    match level {
        CleanupLevel::None => "none",
        CleanupLevel::Light => "light",
        CleanupLevel::Medium => "medium",
    }
}

/// Where the config came from, without the user's home folder.
fn config_source(from: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match from.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => from.to_owned(),
    }
}

fn line(r: &Replayed) -> String {
    match &r.outcome {
        Ok(p) => format!(
            "{:<8} stt {:>5} ms  first {:>5}  total {:>5}{}",
            r.sample,
            p.stt_ms,
            p.first_ms.map_or("-".to_owned(), |ms| ms.to_string()),
            p.total_ms,
            match (p.fallback, &p.why) {
                (true, Some(why)) => format!("  FALLBACK: {why}"),
                (true, None) => "  FALLBACK".to_owned(),
                _ => String::new(),
            }
        ),
        Err(error) => format!("{:<8} FAILED: {error}", r.sample),
    }
}

fn import(o: &Options) -> Result<(), String> {
    let dir = o.set()?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    set::check_location(dir)?;
    let history = o
        .history
        .clone()
        .unwrap_or_else(|| data_dir().join("history"));
    let since = o.days.map(|days| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as i64;
        now - days as i64 * 86_400_000
    });
    let items = set::read_history(&history, since)?;
    let done = set::import(dir, &items)?;
    println!(
        "{} dictations added to {}, {} already there. Correct their texts in gold-draft.json and \
         move them into gold.json.",
        done.added,
        dir.display(),
        done.skipped
    );
    Ok(())
}
