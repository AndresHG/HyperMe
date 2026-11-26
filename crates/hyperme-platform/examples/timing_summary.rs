//! AC-10: summarises the timing log into median and 90th percentile, as Markdown for
//! `docs/results/`. Numbers only.
//!
//! ```sh
//! cargo run -p hyperme-platform --example timing_summary -- [timing.tsv] [--network wired] [--out FILE]
//! ```
//!
//! Turn on Privacy > Timing log in Settings, dictate 50 times on a wired network, then run it.
//! The default log is `~/Library/Logs/HyperMe/timing.tsv`.

use std::path::PathBuf;

use hyperme_platform::timing::{Quantiles, summarise};

/// AC-10's gate on key release to text pasted.
const GATE_MEDIAN_MS: u64 = 800;
const GATE_P90_MS: u64 = 1200;
const NEEDED: usize = 50;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut network = "not stated".to_owned();
    let mut out = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--network" => network = args.next().unwrap_or_default(),
            "--out" => out = args.next().map(PathBuf::from),
            other => path = Some(PathBuf::from(other)),
        }
    }
    let path = path.unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Library/Logs/HyperMe/timing.tsv")
    });
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!(
                "{}: {e}. Turn on the timing log in Settings > Privacy and dictate first.",
                path.display()
            );
            std::process::exit(1);
        }
    };
    let s = summarise(&text);
    let row = |name: &str, q: Option<Quantiles>| match q {
        Some(q) => format!("| {name} | {} ms | {} ms |\n", q.median, q.p90),
        None => format!("| {name} | - | - |\n"),
    };
    let verdict = match s.paste {
        Some(_) if s.dictations < NEEDED => format!(
            "Not enough for the gate: {} dictations, AC-10 asks for {NEEDED}.",
            s.dictations
        ),
        Some(q) if q.median <= GATE_MEDIAN_MS && q.p90 <= GATE_P90_MS => format!(
            "AC-10 met on this run: median {} ms (at or under {GATE_MEDIAN_MS}), 90th percentile {} ms (at or under {GATE_P90_MS}).",
            q.median, q.p90
        ),
        Some(q) => format!(
            "AC-10 not met on this run: median {} ms (gate {GATE_MEDIAN_MS}), 90th percentile {} ms (gate {GATE_P90_MS}).",
            q.median, q.p90
        ),
        None => "No dictations in the log.".to_owned(),
    };
    let report = format!(
        "# Key release to text pasted (AC-10)\n\n\
         - Dictations: {}, of which {} fell back to the raw transcript\n\
         - Network: {network}\n\
         - Source: the timing log, summarised by `examples/timing_summary.rs` (median as \
         flowbench, nearest-rank 90th percentile)\n\n\
         | From key release to | Median | 90th percentile |\n| --- | --- | --- |\n{}{}{}\n{verdict}\n",
        s.dictations,
        s.fallbacks,
        row("final transcript (`stt_ms`)", s.stt),
        row("text ready (`total_ms`)", s.total),
        row("paste sent (AC-10)", s.paste),
    );
    print!("{report}");
    if let Some(out) = out
        && let Err(e) = std::fs::write(&out, &report)
    {
        eprintln!("{}: {e}", out.display());
        std::process::exit(1);
    }
}
