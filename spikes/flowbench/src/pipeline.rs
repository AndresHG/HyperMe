//! The two pipelines, and the timestamp of every stage of every sample.

use std::time::Duration;

use anyhow::{Result, bail};
use tokio::time::{Instant, sleep_until};

use crate::audio::{Clip, chunk_schedule};
use crate::cleanup::{CleanupProvider, clean_with_deadline};
use crate::lang::{Lang, detect};
use crate::report;
use crate::stt::{LiveSession, SpeechProvider, Transcript};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pipeline {
    /// Upload the recording after key release. The baseline.
    A,
    /// Stream while speaking, commit at key release.
    B,
}

impl Pipeline {
    pub fn label(self) -> &'static str {
        match self {
            Pipeline::A => "A upload",
            Pipeline::B => "B stream",
        }
    }
}

/// One sample through one pipeline.
pub struct SampleResult {
    pub sample: String,
    /// The language spoken, when the sample name says.
    pub lang: Option<Lang>,
    pub pipeline: Pipeline,
    pub run: u32,
    pub audio_ms: u64,
    /// The measurements, or why the sample failed.
    pub outcome: Result<Measured, String>,
}

/// Stage times in milliseconds from key release. A negative value is before the release.
pub struct Measured {
    /// Connections open and warm: the socket (B) and the cleanup client (both).
    pub open_ms: i64,
    /// Last audio chunk sent (B only; in A the audio travels inside the upload).
    pub last_audio_ms: Option<i64>,
    /// Final transcript received.
    pub stt_ms: u64,
    /// First cleaned token received. `None` on a fallback.
    pub first_ms: Option<u64>,
    /// First cleanup token of any kind, reasoning included. Earlier than `first_ms` when the
    /// model thinks before it writes.
    pub first_any_ms: Option<u64>,
    /// Characters of streamed reasoning; zero when the host streams none.
    pub thinking_chars: usize,
    /// Reasoning tokens, when the host reports usage.
    pub reasoning_tokens: Option<u64>,
    /// Full text ready: the cleaned text, or the raw transcript on a fallback.
    pub total_ms: u64,
    pub raw: String,
    /// What the user would get: the cleaned text, or the raw transcript on a fallback.
    pub text: String,
    /// The cleanup missed its deadline, so the raw transcript was used.
    pub fallback: bool,
    pub fallback_note: Option<String>,
    /// Language codes reported by the speech provider, if it reports any.
    pub provider_languages: Vec<String>,
    /// Raw and cleaned text are both in the language spoken. `None` when that is unknown.
    pub lang_ok: Option<bool>,
}

/// The moments a pipeline records before the transcript arrives.
pub struct Marks {
    pub open: Instant,
    pub release: Instant,
    pub last_audio: Option<Instant>,
}

/// Runs every clip through both pipelines, `runs` times, printing each stage log as it goes.
pub async fn run_all<S: SpeechProvider, C: CleanupProvider>(
    stt: &S,
    llm: &C,
    clips: &[Clip],
    runs: u32,
    deadline: Duration,
) -> Vec<SampleResult> {
    let mut results = Vec::new();
    for run in 1..=runs {
        for clip in clips {
            for pipeline in [Pipeline::A, Pipeline::B] {
                if pipeline == Pipeline::A && !stt.supports_upload() {
                    continue;
                }
                let outcome = match pipeline {
                    Pipeline::A => upload_after_release(stt, llm, clip, deadline).await,
                    Pipeline::B => streaming(stt, llm, clip, deadline).await,
                };
                let result = SampleResult {
                    sample: clip.name.clone(),
                    lang: clip.lang,
                    pipeline,
                    run,
                    audio_ms: clip.duration_ms(),
                    outcome: outcome.map_err(|err| format!("{err:#}")),
                };
                println!("{}", report::stage_line(&result));
                results.push(result);
            }
        }
    }
    results
}

/// Pipeline A: at key release, upload the whole recording, then clean the transcript.
pub async fn upload_after_release<S: SpeechProvider, C: CleanupProvider>(
    stt: &S,
    llm: &C,
    clip: &Clip,
    deadline: Duration,
) -> Result<Measured> {
    llm.warm().await?;
    let wav = clip.wav.clone();
    let open = Instant::now();
    let release = Instant::now();
    let transcript = stt.transcribe_file(&clip.name, wav).await?;
    let marks = Marks {
        open,
        release,
        last_audio: None,
    };
    clean_and_measure(llm, deadline, marks, transcript, clip.lang).await
}

/// Pipeline B: open and warm every connection first, stream the audio in real time, then
/// commit at key release.
pub async fn streaming<S: SpeechProvider, C: CleanupProvider>(
    stt: &S,
    llm: &C,
    clip: &Clip,
    deadline: Duration,
) -> Result<Measured> {
    let (session, warmed) = tokio::join!(stt.open_live(&clip.name), llm.warm());
    let mut session = session?;
    warmed?;
    let open = Instant::now();
    send_paced(&mut session, &clip.pcm).await?;
    let release = Instant::now();
    let marks = Marks {
        open,
        release,
        last_audio: Some(release),
    };
    finish_live(session, llm, deadline, marks, clip.lang).await
}

/// Sends the audio in 100 ms chunks at the pace a microphone would deliver them.
pub async fn send_paced<L: LiveSession>(session: &mut L, pcm: &[i16]) -> Result<()> {
    let start = Instant::now();
    for (range, due) in chunk_schedule(pcm.len()) {
        sleep_until(start + due).await;
        session.send_audio(&pcm[range]).await?;
    }
    Ok(())
}

/// Key release for a live session: commit, wait for the final transcript, clean it.
pub async fn finish_live<L: LiveSession, C: CleanupProvider>(
    session: L,
    llm: &C,
    deadline: Duration,
    marks: Marks,
    expected: Option<Lang>,
) -> Result<Measured> {
    let transcript = session.finish().await?;
    clean_and_measure(llm, deadline, marks, transcript, expected).await
}

async fn clean_and_measure<C: CleanupProvider>(
    llm: &C,
    deadline: Duration,
    marks: Marks,
    transcript: Transcript,
    expected: Option<Lang>,
) -> Result<Measured> {
    let stt_at = Instant::now();
    let raw = transcript.text.trim().to_owned();
    if raw.is_empty() {
        // Counted as a result, a dictation with no words would look like a very fast one.
        bail!("the transcript is empty: nothing was heard");
    }
    let cleaned = clean_with_deadline(llm, &raw, deadline).await?;
    let since = |at: Instant| at.duration_since(marks.release).as_millis() as u64;
    let signed = |at: Instant| {
        if at >= marks.release {
            since(at) as i64
        } else {
            -(marks.release.duration_since(at).as_millis() as i64)
        }
    };
    Ok(Measured {
        open_ms: signed(marks.open),
        last_audio_ms: marks.last_audio.map(signed),
        stt_ms: since(stt_at),
        first_ms: cleaned.first.map(since),
        first_any_ms: cleaned.first_any.map(since),
        thinking_chars: cleaned.thinking_chars,
        reasoning_tokens: cleaned.reasoning_tokens,
        total_ms: since(cleaned.done),
        lang_ok: expected
            .map(|lang| detect(&raw) == Some(lang) && detect(&cleaned.text) == Some(lang)),
        fallback: cleaned.fallback,
        fallback_note: cleaned.note,
        provider_languages: transcript.languages,
        raw,
        text: cleaned.text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::SAMPLE_RATE;
    use crate::fake::{self, FakeCleanup, FakeSpeech};

    const DEADLINE: Duration = Duration::from_millis(1500);

    /// A live session that notes when each chunk arrived and how long it was.
    struct Recorder {
        started: Instant,
        arrivals: Vec<(u64, usize)>,
    }

    impl LiveSession for Recorder {
        async fn send_audio(&mut self, pcm: &[i16]) -> Result<()> {
            let at = self.started.elapsed().as_millis() as u64;
            self.arrivals.push((at, pcm.len()));
            Ok(())
        }

        async fn finish(self) -> Result<Transcript> {
            unreachable!("the pacing test never finishes the session")
        }
    }

    #[tokio::test(start_paused = true)]
    async fn audio_is_sent_in_100_ms_chunks_in_real_time() {
        // 350 ms of audio: three full chunks and a 50 ms tail.
        let pcm = vec![0i16; SAMPLE_RATE as usize * 350 / 1000];
        let mut session = Recorder {
            started: Instant::now(),
            arrivals: Vec::new(),
        };
        send_paced(&mut session, &pcm).await.unwrap();
        assert_eq!(
            session.arrivals,
            vec![(100, 2_400), (200, 2_400), (300, 2_400), (350, 1_200)]
        );
        // The last chunk leaves when the audio ends: that moment is the key release.
        assert_eq!(session.started.elapsed(), Duration::from_millis(350));
    }

    fn measured<'a>(results: &'a [SampleResult], sample: &str, pipeline: Pipeline) -> &'a Measured {
        results
            .iter()
            .find(|r| r.sample == sample && r.pipeline == pipeline)
            .and_then(|r| r.outcome.as_ref().ok())
            .expect("a successful result")
    }

    #[tokio::test(start_paused = true)]
    async fn the_whole_run_works_end_to_end_with_fake_providers() {
        let clips = fake::clips();
        let results = run_all(&FakeSpeech, &FakeCleanup, &clips, 1, DEADLINE).await;
        assert_eq!(results.len(), 12);
        assert!(results.iter().all(|r| r.outcome.is_ok()));

        // Pipeline A: the clock starts at the upload.
        let a = measured(&results, "es-01", Pipeline::A);
        let stt = FakeSpeech::file_delay("es-01").as_millis() as u64;
        let first = stt + FakeCleanup::first_token_delay(&a.raw).as_millis() as u64;
        let words = a.text.split(' ').count() as u64;
        assert_eq!(a.stt_ms, stt);
        assert_eq!(a.first_ms, Some(first));
        assert_eq!(
            a.total_ms,
            first + (words - 1) * FakeCleanup::PER_WORD.as_millis() as u64
        );
        assert_eq!(a.last_audio_ms, None);
        assert_eq!(a.text, FakeCleanup::cleaned(&a.raw));
        assert_eq!(a.lang_ok, Some(true));
        assert!(!a.fallback);

        // Pipeline B: the clock starts at the commit, after the audio has been streamed.
        let b = measured(&results, "es-01", Pipeline::B);
        assert_eq!(
            b.stt_ms,
            FakeSpeech::final_delay("es-01").as_millis() as u64
        );
        assert_eq!(b.open_ms, -(clips[0].duration_ms() as i64));
        assert_eq!(b.last_audio_ms, Some(0));
        assert!(b.stt_ms < a.stt_ms);
        assert!(b.first_ms.unwrap() > b.stt_ms && b.total_ms >= b.first_ms.unwrap());

        // The stalled cleanup: raw transcript at the deadline, counted as a fallback.
        let slow = measured(&results, "en-03", Pipeline::B);
        assert!(slow.fallback);
        assert_eq!(slow.text, slow.raw);
        assert_eq!(slow.first_ms, None);
        assert_eq!(slow.total_ms, slow.stt_ms + DEADLINE.as_millis() as u64);
        assert_eq!(slow.lang_ok, Some(true));
        assert_eq!(results.iter().filter(|r| is_fallback(r)).count(), 2);
    }

    fn is_fallback(result: &SampleResult) -> bool {
        result.outcome.as_ref().is_ok_and(|m| m.fallback)
    }

    struct BrokenSpeech;

    impl SpeechProvider for BrokenSpeech {
        type Live = Recorder;

        async fn transcribe_file(&self, _label: &str, _wav: Vec<u8>) -> Result<Transcript> {
            anyhow::bail!("HTTP 500: upstream error")
        }

        async fn open_live(&self, _label: &str) -> Result<Recorder> {
            anyhow::bail!("opening the realtime socket: refused")
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_provider_failure_is_recorded_and_the_run_goes_on() {
        let clips = fake::clips();
        let results = run_all(&BrokenSpeech, &FakeCleanup, &clips[..2], 1, DEADLINE).await;
        assert_eq!(results.len(), 4);
        let errors: Vec<_> = results
            .iter()
            .map(|r| r.outcome.as_ref().err().expect("a failure").as_str())
            .collect();
        assert!(errors[0].contains("HTTP 500"));
        assert!(errors[1].contains("refused"));
    }

    /// Always hears the given text.
    struct Hears(&'static str);

    impl SpeechProvider for Hears {
        type Live = Recorder;

        async fn transcribe_file(&self, _label: &str, _wav: Vec<u8>) -> Result<Transcript> {
            Ok(Transcript {
                text: self.0.to_owned(),
                languages: vec![],
            })
        }

        async fn open_live(&self, _label: &str) -> Result<Recorder> {
            unreachable!("these tests only upload")
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_empty_transcript_fails_the_sample() {
        let clip = &fake::clips()[0];
        let outcome = upload_after_release(&Hears("  "), &FakeCleanup, clip, DEADLINE).await;
        let error = outcome.err().expect("a failure").to_string();
        assert!(error.contains("transcript is empty"), "{error}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_cleanup_fails_the_sample_instead_of_looking_fast() {
        struct Rejects;
        impl CleanupProvider for Rejects {
            async fn warm(&self) -> Result<()> {
                Ok(())
            }
            async fn clean(
                &self,
                _raw: &str,
                _on_piece: &mut dyn FnMut(crate::cleanup::Piece<'_>),
            ) -> Result<()> {
                anyhow::bail!("HTTP 404: the model does not exist")
            }
        }
        let clip = &fake::clips()[0];
        let outcome = upload_after_release(&Hears("hola equipo"), &Rejects, clip, DEADLINE).await;
        let error = format!("{:#}", outcome.err().expect("a failure"));
        assert_eq!(error, "cleanup failed: HTTP 404: the model does not exist");
    }
}
