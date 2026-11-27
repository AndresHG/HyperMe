//! Slice 3: the dictation state machine with every trait faked. Tests cite the acceptance
//! criteria in `specs/25-dictation-core/spec.md`. The clock is tokio's paused clock, so delays
//! are exact and the tests take no real time, except AC-11, which measures real overhead.

mod common;

use common::*;
use hyperme_core::dictation::{Event, Status, format_short};
use hyperme_core::traits::ErrorKind;

/// AC-1: key-down starts capture, and opens the speech session and the cleanup warm-up in
/// parallel; the machine reaches `Recording` before either connection completes.
#[tokio::test(start_paused = true)]
async fn key_down_records_before_connections_complete() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "hello there my friend, how are you");
    speech.open_delay = ms(300);
    let mut cleanup = FakeCleanup::new(&log, "Hello there, my friend. How are you?");
    cleanup.warm_delay = ms(200);
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    // Both connections started, neither finished, and no time has passed.
    assert!(log.has("mic start"));
    assert!(log.has("speech open start"));
    assert!(log.has("warm start"));
    assert!(!log.has("speech opened"));
    assert!(!log.has("warmed"));
    assert_eq!(log.at("speech open start"), Some(0));
    assert_eq!(log.at("warm start"), Some(0));

    // Audio captured before the session opened is buffered and sent, in order, when it opens.
    h.frames(0, 5).await;
    tokio::time::sleep(ms(400)).await;
    assert_eq!(log.at("speech opened"), Some(300));
    assert_eq!(log.at("warmed"), Some(200));
    h.frames(5, 2).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;

    let audio: Vec<String> = log
        .entries()
        .into_iter()
        .filter(|e| e.starts_with("audio"))
        .collect();
    assert_eq!(
        audio,
        ["audio 2400 from 0", "audio 480 from 5", "audio 480 from 6"]
    );
    assert_eq!(log.pasted(), ["Hello there, my friend. How are you?"]);
}

/// AC-2, R-2: capture starts at key-down, before the history entry is created, so the SQLite
/// insert and the audio file do not add to key-down latency.
#[tokio::test(start_paused = true)]
async fn mic_starts_before_the_history_entry() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "hello there my friend, how are you");
    let cleanup = FakeCleanup::new(&log, "Hello there, my friend. How are you?");
    let mut h = Harness::start(&log, speech, cleanup, settings());
    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    let begin = log
        .entries()
        .iter()
        .position(|e| e.starts_with("history begin"))
        .unwrap_or_else(|| panic!("no history begin in {:?}", log.entries()));
    assert!(log.index("mic start") < begin, "{:?}", log.entries());
}

/// AC-4: a hold shorter than the minimum ends in `Idle` with nothing pasted and the session
/// closed.
#[tokio::test(start_paused = true)]
async fn short_hold_cancels() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "should never be pasted");
    let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings());

    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    tokio::time::sleep(ms(249)).await;
    h.send(Event::KeyUp).await;
    let seen = h.until(&Status::Idle).await;
    assert!(seen.contains(&Status::Cancelled), "{seen:?}");
    assert!(log.has("session closed"), "{:?}", log.entries());
    assert!(!log.has("finalize"));
    assert!(log.has("mic stop"));
    assert!(log.pasted().is_empty());
}

/// AC-4: a short hold while the session is still opening drops the open attempt.
#[tokio::test(start_paused = true)]
async fn short_hold_while_opening_cancels() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "should never be pasted");
    // Past the tap at 100 ms and the double-tap window after it (#167 AC-2).
    speech.open_delay = ms(1000);
    let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings());

    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    tokio::time::sleep(ms(100)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    tokio::time::sleep(ms(2000)).await;
    assert!(
        !log.has("speech opened"),
        "the open attempt went on: {:?}",
        log.entries()
    );
    assert!(log.pasted().is_empty());
}

/// AC-4: Esc at any point before paste ends in `Idle` with nothing pasted and the session
/// closed: while recording, while transcribing, and while cleaning.
#[tokio::test(start_paused = true)]
async fn esc_cancels_at_every_stage() {
    for stage in [Status::Recording, Status::Transcribing, Status::Cleaning] {
        let log = Log::new();
        let mut speech = FakeSpeech::new(&log, "this is long enough to clean up");
        speech.finish_delay = ms(300);
        let mut cleanup = FakeCleanup::new(&log, "This is long enough to clean up.");
        cleanup.delay = ms(300);
        let mut h = Harness::start(&log, speech, cleanup, settings());

        h.send(Event::KeyDown).await;
        h.until(&Status::Recording).await;
        if stage != Status::Recording {
            tokio::time::sleep(ms(500)).await;
            h.send(Event::KeyUp).await;
            h.until(&stage).await;
        }
        h.send(Event::Esc).await;
        let seen = h.until(&Status::Idle).await;
        assert!(seen.contains(&Status::Cancelled), "{stage:?}: {seen:?}");
        tokio::time::sleep(ms(1000)).await;
        h.settle().await;
        assert!(
            log.pasted().is_empty(),
            "{stage:?}: pasted {:?}",
            log.pasted()
        );
        assert!(log.has("session closed"), "{stage:?}: {:?}", log.entries());
        assert!(log.has("mic stop"), "{stage:?}");
    }
}

/// AC-5: at the maximum length, recording stops and the dictation is processed as if the key
/// had been released. The late key-up is ignored.
#[tokio::test(start_paused = true)]
async fn maximum_length_processes_the_dictation() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "a very long dictation that goes on");
    let cleanup = FakeCleanup::new(&log, "A very long dictation that goes on.");
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    h.until(&Status::Transcribing).await;
    assert_eq!(log.at("mic stop"), Some(120_000));
    assert_eq!(log.at("finalize"), Some(120_000));
    h.until(&Status::Done { raw: false }).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    h.settle().await;
    assert_eq!(log.pasted(), ["A very long dictation that goes on."]);
}

/// AC-6: on release the finalize message is sent, and the text is pasted only after the final
/// transcript.
#[tokio::test(start_paused = true)]
async fn release_finalizes_then_pastes() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "send the report to the team today");
    speech.finish_delay = ms(250);
    let cleanup = FakeCleanup::new(&log, "Send the report to the team today.");
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    h.frames(0, 3).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;

    assert_eq!(log.at("finalize"), Some(1000));
    assert_eq!(log.at("final transcript"), Some(1250));
    let paste = "paste: Send the report to the team today.";
    assert!(log.index("finalize") < log.index("final transcript"));
    assert!(log.index("final transcript") < log.index(paste));
    // R-11: the cleanup gets the transcript and the frontmost app's name.
    assert!(
        log.has("clean: send the report to the team today [TextEdit]"),
        "{:?}",
        log.entries()
    );
}

/// AC-9: transcripts under the short-dictation threshold skip cleanup and get only a capital
/// letter and a full stop.
#[tokio::test(start_paused = true)]
async fn short_transcripts_skip_cleanup() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "sounds good");
    let mut h = Harness::start(
        &log,
        speech,
        FakeCleanup::new(&log, "never used"),
        settings(),
    );

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    let seen = h.until(&Status::Done { raw: false }).await;
    assert!(!seen.contains(&Status::Cleaning), "{seen:?}");
    assert!(!log.entries().iter().any(|e| e.starts_with("clean:")));
    assert_eq!(log.pasted(), ["Sounds good."]);
}

/// AC-9 (R-13): the short-dictation format.
#[test]
fn short_format() {
    assert_eq!(format_short("sounds good"), "Sounds good.");
    assert_eq!(format_short("  vale  "), "Vale.");
    assert_eq!(format_short("is it done?"), "Is it done?");
    assert_eq!(format_short("¡qué bien!"), "¡Qué bien!");
    assert_eq!(format_short("ok."), "Ok.");
    assert_eq!(format_short("Already Capital"), "Already Capital.");
}

/// AC-9 (R-14): an empty transcript pastes nothing and says "Nothing heard".
#[tokio::test(start_paused = true)]
async fn empty_transcript_pastes_nothing() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "   ");
    let mut h = Harness::start(
        &log,
        speech,
        FakeCleanup::new(&log, "never used"),
        settings(),
    );

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    let seen = h.until(&Status::Idle).await;
    assert!(seen.contains(&Status::NothingHeard), "{seen:?}");
    assert!(log.pasted().is_empty());
}

/// R-12: past the cleanup deadline the raw transcript is pasted, at the deadline.
#[tokio::test(start_paused = true)]
async fn slow_cleanup_pastes_raw_at_the_deadline() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "this one takes the cleanup too long");
    let mut cleanup = FakeCleanup::new(&log, "never in time");
    cleanup.delay = ms(5000);
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: true }).await;
    assert_eq!(log.pasted(), ["this one takes the cleanup too long"]);
    let pasted_at = log
        .at("paste: this one takes the cleanup too long")
        .unwrap();
    assert_eq!(pasted_at, 1000 + 1500);
}

/// R-20: a cleanup error pastes the raw transcript, marked as raw.
#[tokio::test(start_paused = true)]
async fn failed_cleanup_pastes_raw() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "the cleanup host is down today");
    let mut cleanup = FakeCleanup::new(&log, "");
    cleanup.reply = Err(err(ErrorKind::Network, "connection refused"));
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: true }).await;
    assert_eq!(log.pasted(), ["the cleanup host is down today"]);
}

/// R-19: when the speech session cannot open, the error is shown and nothing is pasted.
#[tokio::test(start_paused = true)]
async fn speech_failure_pastes_nothing() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "never heard");
    speech.open_delay = ms(100);
    speech.open_error = Some(err(ErrorKind::Network, "speech host unreachable"));
    let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings());

    h.send(Event::KeyDown).await;
    let seen = h.until(&Status::Idle).await;
    assert!(
        seen.iter()
            .any(|s| matches!(s, Status::Failed { message, retry: true } if message.contains("speech host unreachable"))),
        "{seen:?}"
    );
    assert!(log.has("mic stop"));
    assert!(log.pasted().is_empty());
}

/// Design, "The state machine": a key-down while working is ignored, with "Still working".
#[tokio::test(start_paused = true)]
async fn key_down_while_working_is_ignored() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "the first dictation is still running");
    speech.finish_delay = ms(500);
    let cleanup = FakeCleanup::new(&log, "The first dictation is still running.");
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Transcribing).await;
    h.send(Event::KeyDown).await;
    h.until(&Status::StillWorking).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    h.settle().await;
    assert_eq!(log.pasted(), ["The first dictation is still running."]);
    assert_eq!(
        log.entries().iter().filter(|e| *e == "mic start").count(),
        1
    );

    // And the next dictation works.
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    h.settle().await;
    assert_eq!(log.pasted().len(), 2);
}

/// R-17 (the core half of AC-14): with secure input on (a password field has focus), holding
/// the hotkey records nothing, opens no connection and says so in the pill.
#[tokio::test(start_paused = true)]
async fn secure_input_records_nothing() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "my password is hunter2");
    let cleanup = FakeCleanup::new(&log, "never used");
    let mut h = Harness::start_with(
        &log,
        speech,
        cleanup,
        settings(),
        FakeFocus {
            secure: true,
            ..FakeFocus::textedit()
        },
    );

    h.send(Event::KeyDown).await;
    let seen = h.until(&Status::Idle).await;
    assert_eq!(seen, [Status::SecureInput, Status::Idle]);
    h.send(Event::KeyUp).await;
    h.settle().await;
    assert!(log.entries().is_empty(), "{:?}", log.entries());
}

/// AC-11: with zero-latency fakes, the pipeline's own overhead from release to paste sent is
/// under 20 ms at the 90th percentile. Real clock, 100 dictations.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipeline_overhead_is_under_20_ms() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "a normal sentence with enough words to clean");
    let cleanup = FakeCleanup::new(&log, "A normal sentence with enough words to clean.");
    let mut settings = settings();
    settings.min_hold = std::time::Duration::ZERO;
    let mut h = Harness::start(&log, speech, cleanup, settings);

    let mut overheads = Vec::new();
    for _ in 0..100 {
        h.send(Event::KeyDown).await;
        h.until(&Status::Recording).await;
        h.frames(0, 5).await;
        let released = std::time::Instant::now();
        h.send(Event::KeyUp).await;
        h.until(&Status::Done { raw: false }).await;
        overheads.push(released.elapsed());
        h.until(&Status::Idle).await;
    }
    overheads.sort();
    let p90 = overheads[89];
    println!("AC-11 overhead: p50 {:?}, p90 {p90:?}", overheads[49]);
    assert!(p90 < ms(20), "p90 overhead {p90:?}");
    assert_eq!(log.pasted().len(), 100);
}

/// AC-15 (core half): the dictation's audio goes to the history as it is captured, with the
/// app's name, and its text when it is pasted; a cancelled dictation and one where nothing was
/// heard are discarded.
#[tokio::test(start_paused = true)]
async fn history_gets_the_audio_and_the_text() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "remember to buy milk on the way home");
    let cleanup = FakeCleanup::new(&log, "Remember to buy milk on the way home.");
    let history = FakeHistory::new(&log);
    let mut h = Harness::start_full(
        &log,
        speech,
        cleanup,
        settings(),
        FakeFocus::textedit(),
        history.clone(),
    );

    h.send(Event::KeyDown).await;
    h.frames(0, 10).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert!(log.has("history begin 1 [TextEdit]"), "{:?}", log.entries());
    assert_eq!(history.samples(1), 4800);
    assert!(log.has("history done 1: Remember to buy milk on the way home. (en)"));

    // A tap is discarded.
    h.send(Event::KeyDown).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Cancelled).await;
    h.until(&Status::Idle).await;
    assert!(log.has("history discard 2"));
}

#[tokio::test(start_paused = true)]
async fn nothing_heard_is_discarded() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "");
    let history = FakeHistory::new(&log);
    let mut h = Harness::start_full(
        &log,
        speech,
        FakeCleanup::new(&log, "x"),
        settings(),
        FakeFocus::textedit(),
        history,
    );
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::NothingHeard).await;
    h.until(&Status::Idle).await;
    assert!(log.has("history discard 1"));
}

/// AC-23 (core half, R-31): when the speech connection drops, the audio is kept and Retry is
/// offered; a retry against a working provider sends the saved audio again and pastes the
/// transcript.
#[tokio::test(start_paused = true)]
async fn a_dropped_dictation_can_be_retried() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "the connection dropped but the words were kept");
    *speech.finish_error.lock().unwrap() = Some(err(ErrorKind::Network, "connection reset"));
    let finish_error = speech.finish_error.clone();
    let cleanup = FakeCleanup::new(&log, "The connection dropped, but the words were kept.");
    let history = FakeHistory::new(&log);
    let mut h = Harness::start_full(
        &log,
        speech,
        cleanup,
        settings(),
        FakeFocus::textedit(),
        history.clone(),
    );

    h.send(Event::KeyDown).await;
    h.frames(0, 25).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    let seen = h.until(&Status::Idle).await;
    assert!(
        seen.iter().any(|s| matches!(s, Status::Failed { message, retry: true } if message.contains("connection reset"))),
        "{seen:?}"
    );
    assert!(log.has("history failed 1"));
    assert!(log.pasted().is_empty());
    assert_eq!(history.samples(1), 25 * 480);

    // The provider works again; Retry sends the saved audio and pastes.
    *finish_error.lock().unwrap() = None;
    let before = log.entries().len();
    h.send(Event::Retry(None)).await;
    h.until(&Status::Done { raw: false }).await;
    let retried: Vec<String> = log.entries()[before..]
        .iter()
        .filter(|e| e.starts_with("audio"))
        .cloned()
        .collect();
    assert_eq!(retried, [format!("audio {} from 0", 25 * 480)]);
    assert_eq!(
        log.pasted(),
        ["The connection dropped, but the words were kept."]
    );
    assert!(log.has("history done 1: The connection dropped, but the words were kept. (en)"));

    // Nothing is left to retry.
    h.send(Event::Retry(None)).await;
    h.until(&Status::Idle).await;
    h.settle().await;
    assert_eq!(log.pasted().len(), 1);
}

/// R-31: a retry after the user moved to another app copies the text instead of pasting it.
#[tokio::test(start_paused = true)]
async fn a_retry_in_another_app_copies() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "this goes to the clipboard after the retry");
    *speech.finish_error.lock().unwrap() = Some(err(ErrorKind::Network, "connection reset"));
    let finish_error = speech.finish_error.clone();
    let cleanup = FakeCleanup::new(&log, "This goes to the clipboard after the retry.");
    let focus = FakeFocus::textedit();
    let frontmost = focus.app.clone();
    let mut h = Harness::start_full(
        &log,
        speech,
        cleanup,
        settings(),
        focus,
        FakeHistory::new(&log),
    );

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    *finish_error.lock().unwrap() = None;
    *frontmost.lock().unwrap() = "Safari".to_owned();
    h.send(Event::Retry(None)).await;
    h.until(&Status::Copied { raw: false }).await;
    assert!(log.pasted().is_empty());
    assert!(log.has("copy: This goes to the clipboard after the retry."));
}

/// R-31: a retry that fails again keeps the audio and offers Retry again.
#[tokio::test(start_paused = true)]
async fn a_failed_retry_can_be_retried() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "still failing");
    *speech.finish_error.lock().unwrap() = Some(err(ErrorKind::Network, "connection reset"));
    let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings());
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    h.send(Event::Retry(None)).await;
    let seen = h.until(&Status::Idle).await;
    assert!(
        seen.iter()
            .any(|s| matches!(s, Status::Failed { retry: true, .. })),
        "{seen:?}"
    );
    h.send(Event::Retry(None)).await;
    let seen = h.until(&Status::Idle).await;
    assert!(
        seen.iter()
            .any(|s| matches!(s, Status::Failed { retry: true, .. })),
        "{seen:?}"
    );
}

/// AC-24 (R-32): the start and stop sounds play when the setting is on, and not when it is off.
#[tokio::test(start_paused = true)]
async fn sounds_follow_the_setting() {
    for on in [true, false] {
        let log = Log::new();
        let mut settings = settings();
        settings.sounds = on;
        let speech = FakeSpeech::new(&log, "ok then");
        let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings);
        h.send(Event::KeyDown).await;
        h.until(&Status::Recording).await;
        let started = log.has("sound Start");
        tokio::time::sleep(ms(600)).await;
        h.send(Event::KeyUp).await;
        h.until(&Status::Idle).await;
        assert_eq!(started, on, "start sound with sounds = {on}");
        assert_eq!(log.has("sound Stop"), on, "stop sound with sounds = {on}");
        if on {
            assert!(
                log.index("sound Stop") < log.index("finalize"),
                "the stop sound plays at release"
            );
        }
    }
}

/// AC-20 (R-30): with timing on, each pasted dictation writes one line of stage timings, as ms
/// from key-down and from key release (flowbench's stt_ms, first_ms, total_ms), and no text.
#[tokio::test(start_paused = true)]
async fn timing_on_writes_one_line_of_numbers_per_dictation() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "please send the quarterly numbers to the team");
    speech.finish_delay = ms(300);
    let mut cleanup = FakeCleanup::new(&log, "Please send the quarterly numbers to the team.");
    cleanup.delay = ms(120);
    let settings = settings();
    let timing = FakeTiming::default();
    let mut h = Harness::start_timed(&log, speech, cleanup, settings, timing.clone(), true);

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(40)).await;
    h.frames(0, 5).await;
    tokio::time::sleep(ms(960)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;

    let lines = timing.lines.lock().unwrap().clone();
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line.first_audio_ms, Some(40));
    assert_eq!(line.release_ms, 1000);
    assert_eq!(line.stt_ms, 300);
    assert_eq!(line.first_ms, Some(420));
    assert_eq!(line.total_ms, 420);
    assert_eq!(line.paste_ms, 420);
    assert_eq!(line.language.as_deref(), Some("en"));
    assert_eq!(line.words, 8);
    assert!(!line.fallback);
    let shown = format!("{line:?}");
    assert!(
        !shown.contains("quarterly"),
        "the timing line holds text: {shown}"
    );
}

/// AC-20: a fallback is marked, and a short dictation has no first cleaned token.
#[tokio::test(start_paused = true)]
async fn timing_marks_fallbacks_and_short_dictations() {
    let log = Log::new();
    let settings = settings();
    let timing = FakeTiming::default();
    let mut cleanup = FakeCleanup::new(&log, "never in time");
    cleanup.delay = ms(5000);
    let speech = FakeSpeech::new(&log, "this one is long enough to clean");
    let mut h = Harness::start_timed(
        &log,
        speech,
        cleanup,
        settings.clone(),
        timing.clone(),
        true,
    );
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: true }).await;
    let line = timing.lines.lock().unwrap()[0].clone();
    assert!(line.fallback);
    assert_eq!(line.first_ms, None);
    assert_eq!(line.total_ms, 1500);

    let log = Log::new();
    let timing = FakeTiming::default();
    let mut h = Harness::start_timed(
        &log,
        FakeSpeech::new(&log, "sounds good"),
        FakeCleanup::new(&log, "x"),
        settings,
        timing.clone(),
        true,
    );
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    let line = timing.lines.lock().unwrap()[0].clone();
    assert_eq!(
        (line.first_ms, line.words, line.first_audio_ms),
        (None, 2, None)
    );
}

/// AC-20: with timing off nothing is recorded, and a cancelled dictation records nothing either.
#[tokio::test(start_paused = true)]
async fn timing_off_records_nothing() {
    let log = Log::new();
    let timing = FakeTiming::default();
    let speech = FakeSpeech::new(&log, "send the report today please team");
    let cleanup = FakeCleanup::new(&log, "Send the report today, please, team.");
    let mut h = Harness::start_timed(&log, speech, cleanup, settings(), timing.clone(), false);
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert!(timing.lines.lock().unwrap().is_empty());

    let on = settings();
    let log = Log::new();
    let mut h = Harness::start_timed(
        &log,
        FakeSpeech::new(&log, "x"),
        FakeCleanup::new(&log, "x"),
        on,
        timing.clone(),
        true,
    );
    h.send(Event::KeyDown).await;
    h.send(Event::Esc).await;
    h.until(&Status::Idle).await;
    assert!(timing.lines.lock().unwrap().is_empty());
}

// Home window and History (#98), slice 2: retry any dictation by id, from Home.

/// A harness whose speech can be made to fail, the switch, and the frontmost app; one
/// dictation already done and pasted into TextEdit as dictation 1.
async fn with_done_dictation(log: &Log, history: &FakeHistory) -> (Harness, FailSwitch, Front) {
    with_done_dictation_slow(log, history, ms(0)).await
}

/// As [`with_done_dictation`], with speech that takes `finish_delay` to finalize.
async fn with_done_dictation_slow(
    log: &Log,
    history: &FakeHistory,
    finish_delay: std::time::Duration,
) -> (Harness, FailSwitch, Front) {
    let mut speech = FakeSpeech::new(log, "retry this dictation from home please");
    speech.finish_delay = finish_delay;
    let finish_error = speech.finish_error.clone();
    let cleanup = FakeCleanup::new(log, "Retry this dictation from Home, please.");
    let focus = FakeFocus::textedit();
    let front = focus.app.clone();
    let mut h = Harness::start_full(log, speech, cleanup, settings(), focus, history.clone());
    h.send(Event::KeyDown).await;
    h.frames(0, 10).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    h.until(&Status::Idle).await;
    (h, finish_error, front)
}

type FailSwitch = std::sync::Arc<std::sync::Mutex<Option<hyperme_core::traits::Error>>>;
type Front = std::sync::Arc<std::sync::Mutex<String>>;

/// #98 AC-10 (core half, R-16): Retry of a done dictation by id sends its saved audio again,
/// cleans it for the app it was dictated into, copies the new text instead of pasting (Home
/// has focus), and replaces the item's text instead of finishing it again.
#[tokio::test(start_paused = true)]
async fn retry_by_id_copies_and_replaces_the_text() {
    let log = Log::new();
    let history = FakeHistory::new(&log);
    let (mut h, _, front) = with_done_dictation(&log, &history).await;
    *front.lock().unwrap() = "HyperMe".to_owned();
    let before = log.entries().len();

    h.send(Event::Retry(Some(1))).await;
    h.until(&Status::Copied { raw: false }).await;
    h.until(&Status::Idle).await;
    let after: Vec<String> = log.entries()[before..].to_vec();
    assert!(
        after.contains(&format!("audio {} from 0", 10 * 480)),
        "{after:?}"
    );
    assert!(
        after.contains(&"clean: retry this dictation from home please [TextEdit]".to_owned()),
        "{after:?}"
    );
    assert!(after.contains(&"copy: Retry this dictation from Home, please.".to_owned()));
    assert!(
        after.contains(
            &"history replace 1: Retry this dictation from Home, please. (en)".to_owned()
        )
    );
    assert!(
        !after
            .iter()
            .any(|e| e.starts_with("paste: ") || e.starts_with("history done"))
    );
}

/// #98 AC-10 (core half): a failing retry by id keeps the old text, says why, and does not
/// offer the pill's Retry when nothing else failed.
#[tokio::test(start_paused = true)]
async fn failed_retry_by_id_keeps_the_old_text() {
    let log = Log::new();
    let history = FakeHistory::new(&log);
    let (mut h, fail, _) = with_done_dictation(&log, &history).await;
    *fail.lock().unwrap() = Some(err(ErrorKind::Network, "connection reset"));
    let before = log.entries().len();

    h.send(Event::Retry(Some(1))).await;
    let seen = h.until(&Status::Idle).await;
    assert!(
        seen.iter().any(|s| matches!(s, Status::Failed { message, retry: false } if message.contains("connection reset"))),
        "{seen:?}"
    );
    let after: Vec<String> = log.entries()[before..].to_vec();
    assert!(
        !after
            .iter()
            .any(|e| e.starts_with("history ") || e.starts_with("copy: ")),
        "{after:?}"
    );
    assert_eq!(history.samples(1), 10 * 480, "the audio stays");
}

/// #98 AC-10 (core half): a dictation with no audio left cannot be retried.
#[tokio::test(start_paused = true)]
async fn retry_by_id_without_audio_fails() {
    let log = Log::new();
    let history = FakeHistory::new(&log);
    let (mut h, _, _) = with_done_dictation(&log, &history).await;
    history.audio.lock().unwrap().remove(&1);
    h.send(Event::Retry(Some(1))).await;
    let seen = h.until(&Status::Idle).await;
    assert!(
        seen.iter().any(|s| matches!(s, Status::Failed { message, retry: false } if message.starts_with("Retry:"))),
        "{seen:?}"
    );
    let opens = log
        .entries()
        .iter()
        .filter(|e| *e == "speech open start")
        .count();
    assert_eq!(opens, 1, "no speech session for a retry without audio");
}

/// #98 AC-10 (core half): a retry by id leaves the last failed dictation in place for the
/// pill's Retry, and a retry by id of that same dictation clears it.
#[tokio::test(start_paused = true)]
async fn retry_by_id_leaves_the_last_failed_dictation() {
    let log = Log::new();
    let history = FakeHistory::new(&log);
    let speech = FakeSpeech::new(&log, "the words were kept on disk for later");
    let fail = speech.finish_error.clone();
    let cleanup = FakeCleanup::new(&log, "The words were kept on disk for later.");
    let mut h = Harness::start_full(
        &log,
        speech,
        cleanup,
        settings(),
        FakeFocus::textedit(),
        history.clone(),
    );

    // Dictation 1 is done; dictation 2 fails and waits for the pill's Retry.
    for failing in [false, true] {
        *fail.lock().unwrap() = failing.then(|| err(ErrorKind::Network, "connection reset"));
        h.send(Event::KeyDown).await;
        h.frames(0, 5).await;
        tokio::time::sleep(ms(600)).await;
        h.send(Event::KeyUp).await;
        h.until(&Status::Idle).await;
    }
    assert!(log.has("history failed 2"));

    // Retrying dictation 1 fails too: the pill still offers Retry for dictation 2.
    h.send(Event::Retry(Some(1))).await;
    let seen = h.until(&Status::Idle).await;
    assert!(
        seen.iter()
            .any(|s| matches!(s, Status::Failed { retry: true, .. })),
        "{seen:?}"
    );

    // The provider works again: the pill's Retry still retries dictation 2 and pastes it.
    *fail.lock().unwrap() = None;
    h.send(Event::Retry(None)).await;
    h.until(&Status::Done { raw: false }).await;
    h.until(&Status::Idle).await;
    assert!(log.has("history done 2: The words were kept on disk for later. (en)"));

    // Dictation 3 fails; Home retries it by id, which clears the pill's Retry.
    *fail.lock().unwrap() = Some(err(ErrorKind::Network, "connection reset"));
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    *fail.lock().unwrap() = None;
    h.send(Event::Retry(Some(3))).await;
    h.until(&Status::Copied { raw: false }).await;
    assert!(log.has("history replace 3: The words were kept on disk for later. (en)"));
    let before = log.entries().len();
    h.send(Event::Retry(None)).await;
    h.until(&Status::Idle).await;
    h.settle().await;
    assert!(
        !log.entries()[before..]
            .iter()
            .any(|e| e.starts_with("speech open")),
        "nothing left for the pill's Retry"
    );
}

/// #98 AC-10 (core half), Phase 1 R-31: a retry by id while a dictation is in progress is
/// ignored, as the pill's Retry is.
#[tokio::test(start_paused = true)]
async fn retry_by_id_during_a_dictation_is_ignored() {
    let log = Log::new();
    let history = FakeHistory::new(&log);
    let (mut h, _, _) = with_done_dictation_slow(&log, &history, ms(300)).await;
    let before = log.entries().len();
    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    h.send(Event::Retry(Some(1))).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Transcribing).await;
    h.send(Event::Retry(Some(1))).await;
    h.until(&Status::Done { raw: false }).await;
    h.until(&Status::Idle).await;
    h.settle().await;
    let after: Vec<String> = log.entries()[before..].to_vec();
    assert_eq!(
        after.iter().filter(|e| *e == "speech open start").count(),
        1,
        "{after:?}"
    );
    assert!(
        !after
            .iter()
            .any(|e| e.starts_with("copy: ") || e.starts_with("history replace"))
    );
}

/// Settings with a small dictionary (spec 93).
fn with_dictionary() -> hyperme_core::dictation::Settings {
    hyperme_core::dictation::Settings {
        dictionary: hyperme_core::dictionary::Dictionary::parse("Zentrix: zen trix, sun trix"),
        ..settings()
    }
}

/// Spec 93, AC-2: the cleanup gets the transcript with the dictionary applied.
#[tokio::test(start_paused = true)]
async fn the_cleanup_gets_the_dictionary_words() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "send the sun trix report today");
    let cleanup = FakeCleanup::new(&log, "Send the Zentrix report today.");
    let mut h = Harness::start(&log, speech, cleanup, with_dictionary());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert!(
        log.has("clean: send the Zentrix report today [TextEdit]"),
        "{:?}",
        log.entries()
    );
}

/// Spec 93, AC-2: a short dictation, which skips the cleanup, still gets the dictionary.
#[tokio::test(start_paused = true)]
async fn a_short_dictation_gets_the_dictionary_words() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "zen trix");
    let cleanup = FakeCleanup::new(&log, "never used");
    let mut h = Harness::start(&log, speech, cleanup, with_dictionary());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert_eq!(log.pasted(), ["Zentrix."]);
}

/// Spec 93, AC-2: the raw fallback after a cleanup failure still gets the dictionary.
#[tokio::test(start_paused = true)]
async fn the_raw_fallback_gets_the_dictionary_words() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "the sun trix host is down");
    let mut cleanup = FakeCleanup::new(&log, "");
    cleanup.reply = Err(err(ErrorKind::Network, "connection refused"));
    let mut h = Harness::start(&log, speech, cleanup, with_dictionary());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: true }).await;
    assert_eq!(log.pasted(), ["the Zentrix host is down"]);
}

/// Spec 93, AC-8 (R-10a): a dictionary saved while the app runs is used from the next
/// dictation, in the substitution and in the cleanup prompt, with no restart.
#[tokio::test(start_paused = true)]
async fn a_saved_dictionary_applies_from_the_next_dictation() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "send the sun trix report today");
    let cleanup = FakeCleanup::new(&log, "Send the report today.");
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert!(log.has("clean: send the sun trix report today [TextEdit]"));

    h.dictionary
        .send(hyperme_core::dictionary::Dictionary::parse(
            "Zentrix: sun trix",
        ));
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert!(
        log.has("clean: send the Zentrix report today [TextEdit]"),
        "{:?}",
        log.entries()
    );
    assert!(log.has("dictionary:\n\nThe speaker's dictionary: Zentrix."));
}

// ---- Hands-free (#167): double-tap to lock, tap again to stop ----

/// Taps the hotkey, then presses it again `gap` after the release.
async fn double_tap(h: &mut Harness, gap: u64) {
    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    tokio::time::sleep(ms(100)).await;
    h.send(Event::KeyUp).await;
    tokio::time::sleep(ms(gap)).await;
    h.send(Event::KeyDown).await;
}

/// #167 AC-1, AC-3: a tap and a second key-down within the window lock recording; the same
/// capture and session go on, the key-ups are ignored, and the next key-down stops and pastes.
#[tokio::test(start_paused = true)]
async fn double_tap_locks_and_a_tap_stops() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "hands free works like this");
    let cleanup = FakeCleanup::new(&log, "Hands-free works like this.");
    let mut h = Harness::start(&log, speech, cleanup, settings());

    double_tap(&mut h, 300).await;
    h.until(&Status::Locked).await;
    h.send(Event::KeyUp).await;
    h.frames(0, 3).await;
    tokio::time::sleep(ms(5000)).await;
    assert!(
        !log.has("finalize"),
        "a key-up stopped it: {:?}",
        log.entries()
    );
    assert!(!log.has("sound Stop"), "the stop sound played between taps");
    h.send(Event::KeyDown).await;
    h.until(&Status::Transcribing).await;
    assert_eq!(log.at("finalize"), Some(5400));
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    h.until(&Status::Idle).await;
    h.settle().await;
    assert_eq!(log.pasted(), ["Hands-free works like this."]);
    assert_eq!(count(&log, "mic start"), 1);
    assert_eq!(count(&log, "speech open start"), 1);
    assert!(log.has("audio 480 from 2"), "{:?}", log.entries());
}

/// #167 AC-2: a tap with no second key-down is cancelled when the window ends; a key-down
/// after it starts a new dictation.
#[tokio::test(start_paused = true)]
async fn a_lone_tap_is_cancelled_after_the_window() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "should never be pasted");
    let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings());

    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    tokio::time::sleep(ms(100)).await;
    h.send(Event::KeyUp).await;
    let seen = h.until(&Status::Idle).await;
    assert!(seen.contains(&Status::Cancelled), "{seen:?}");
    assert!(!seen.contains(&Status::Locked), "{seen:?}");
    assert_eq!(log.at("mic stop"), Some(500));
    assert!(log.has("session closed"));
    assert!(log.pasted().is_empty());

    // Too late for a double tap: an ordinary dictation.
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    let seen = h.until(&Status::Idle).await;
    assert!(!seen.contains(&Status::Locked), "{seen:?}");
    assert_eq!(log.pasted().len(), 1);
}

/// #167 AC-4: the pill's stop button stops and pastes a locked dictation; Esc (and the pill's
/// ✕, which sends it) cancels one.
#[tokio::test(start_paused = true)]
async fn the_pill_stops_or_cancels_a_locked_dictation() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "stopped from the pill");
    let cleanup = FakeCleanup::new(&log, "Stopped from the pill.");
    let mut h = Harness::start(&log, speech, cleanup, settings());
    double_tap(&mut h, 200).await;
    h.until(&Status::Locked).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::Stop).await;
    h.until(&Status::Done { raw: false }).await;
    h.until(&Status::Idle).await;
    assert_eq!(log.pasted(), ["Stopped from the pill."]);

    let log = Log::new();
    let speech = FakeSpeech::new(&log, "never pasted");
    let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings());
    double_tap(&mut h, 200).await;
    h.until(&Status::Locked).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::Esc).await;
    let seen = h.until(&Status::Idle).await;
    assert!(seen.contains(&Status::Cancelled), "{seen:?}");
    tokio::time::sleep(ms(1000)).await;
    h.settle().await;
    assert!(log.pasted().is_empty());
    assert!(log.has("session closed"));
}

/// #167 AC-5: the maximum length ends a locked dictation and processes it.
#[tokio::test(start_paused = true)]
async fn the_maximum_length_ends_a_locked_dictation() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "a long hands free dictation");
    let cleanup = FakeCleanup::new(&log, "A long hands-free dictation.");
    let mut h = Harness::start(&log, speech, cleanup, settings());
    double_tap(&mut h, 200).await;
    h.until(&Status::Locked).await;
    h.until(&Status::Transcribing).await;
    assert_eq!(log.at("finalize"), Some(120_000));
    h.until(&Status::Idle).await;
    h.settle().await;
    assert_eq!(log.pasted(), ["A long hands-free dictation."]);
}

/// #167 AC-7: a hold behaves as before, and a stop from the pill outside hands-free is
/// ignored, while recording and while idle.
#[tokio::test(start_paused = true)]
async fn stop_outside_hands_free_is_ignored() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "held the usual way");
    let cleanup = FakeCleanup::new(&log, "Held the usual way.");
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::Stop).await;
    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    tokio::time::sleep(ms(400)).await;
    h.send(Event::Stop).await;
    tokio::time::sleep(ms(400)).await;
    assert!(!log.has("finalize"), "Stop ended a held dictation");
    h.send(Event::KeyUp).await;
    let seen = h.until(&Status::Idle).await;
    assert!(!seen.contains(&Status::Locked), "{seen:?}");
    assert_eq!(log.at("finalize"), Some(800));
    h.settle().await;
    assert_eq!(log.pasted(), ["Held the usual way."]);
}

fn count(log: &Log, entry: &str) -> usize {
    log.entries().iter().filter(|e| *e == entry).count()
}

/// Spec 93, AC-5 (R-5): above the prompt cap the warm-up prefix carries no dictionary, and the
/// cleanup gets only the entries relevant to this dictation, set before the request.
#[tokio::test(start_paused = true)]
async fn above_the_cap_the_cleanup_gets_the_relevant_entries() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "send the sun trix report to the corbit team");
    let cleanup = FakeCleanup::new(&log, "Send the Zentrix report to the Quorbit team.");
    let filler: String = (0..20).map(|i| format!("Kfz{i:03}\n")).collect();
    let settings = hyperme_core::dictation::Settings {
        prompt_cap: 10,
        ..settings()
    };
    let mut h = Harness::start(&log, speech, cleanup, settings);
    h.dictionary
        .send(hyperme_core::dictionary::Dictionary::parse(&format!(
            "Zentrix: sun trix\nQuorbit\nAna Ruiz\n{filler}"
        )));

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    let entries = log.entries();
    let at = |line: &str| entries.iter().position(|e| e.contains(line));
    let relevant = at("dictionary:\n\nThe speaker's dictionary: Quorbit, Zentrix.");
    assert!(at("dictionary:") < relevant, "{entries:?}");
    assert!(
        relevant < at("clean: send the Zentrix report"),
        "{entries:?}"
    );
    assert_eq!(entries[at("dictionary:").unwrap()], "dictionary:");
}

/// Spec 93, D6 (R-16): each pasted dictation is told to the app with the pasted text, the
/// transcript after the dictionary, the app and the entries used, for learning and use counts.
#[tokio::test(start_paused = true)]
async fn a_pasted_dictation_is_told_for_learning() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "send the sun trix report today");
    let cleanup = FakeCleanup::new(&log, "Send the Zentrix report today.");
    let mut h = Harness::start(&log, speech, cleanup, with_dictionary());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(1000)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    let told = h.pasted.try_recv().expect("nothing told");
    assert_eq!(
        told,
        hyperme_core::dictation::Pasted {
            text: "Send the Zentrix report today.".to_owned(),
            transcript: "send the Zentrix report today".to_owned(),
            app: Some("TextEdit".to_owned()),
            used: vec!["Zentrix".to_owned()],
        }
    );
}
