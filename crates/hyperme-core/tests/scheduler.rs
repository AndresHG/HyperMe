//! Tasks slice 4: the scheduler loop. Tests cite `specs/92-daily-tasks/spec.md`: AC-14,
//! AC-16, R-23 and R-33a, with the morning card's rules (R-15, R-16) and roll-over (R-13).
//!
//! The scheduler runs on Tokio's paused clock. The fake wall clock is that clock plus a jump,
//! so a Mac asleep (monotonic time stopped, wall time moved on) is a jump and a `Woke`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use hyperme_core::config::Config;
use hyperme_core::schedule::{Schedule, Zone};
use hyperme_core::scheduler::{
    Clock, Command, Job, JobKind, JobStatus, JobStore, Parts, Presence, PresenceEvent, Runner,
    Signal, builtin_jobs, run, seed,
};
use hyperme_core::tasks::{TaskError, WeekdaySet};
use time::{Date, Month, SignedDuration, Time, UtcDateTime, UtcOffset, Weekday};
use tokio::sync::mpsc;
use tokio::time::Instant;

// --- Fakes ---------------------------------------------------------------------------------

#[derive(Clone)]
struct FakeClock {
    base: UtcDateTime,
    start: Instant,
    jump: Arc<Mutex<SignedDuration>>,
    offset_hours: Arc<Mutex<i8>>,
    wakeups: Arc<AtomicUsize>,
}

impl FakeClock {
    fn new(base: UtcDateTime) -> Self {
        FakeClock {
            base,
            start: Instant::now(),
            jump: Arc::default(),
            offset_hours: Arc::new(Mutex::new(1)),
            wakeups: Arc::default(),
        }
    }
}

impl Zone for FakeClock {
    fn offset_at(&self, _at: UtcDateTime) -> UtcOffset {
        UtcOffset::from_hms(*self.offset_hours.lock().unwrap(), 0, 0).unwrap()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> UtcDateTime {
        self.base + self.start.elapsed() + *self.jump.lock().unwrap()
    }

    async fn sleep_until(&self, at: UtcDateTime) {
        let wait = at - self.now();
        if wait > SignedDuration::ZERO {
            tokio::time::sleep(wait.unsigned_abs()).await;
        }
        self.wakeups.fetch_add(1, Ordering::SeqCst);
    }
}

struct FakePresence {
    events: mpsc::UnboundedReceiver<PresenceEvent>,
    idle_s: Arc<AtomicU64>,
}

impl Presence for FakePresence {
    async fn next_event(&mut self) -> Option<PresenceEvent> {
        self.events.recv().await
    }

    fn seconds_since_input(&self) -> u64 {
        self.idle_s.load(Ordering::SeqCst)
    }
}

type Runs = Arc<Mutex<Vec<(String, UtcDateTime)>>>;

struct FakeRunner {
    clock: FakeClock,
    runs: Runs,
    failing: Arc<Mutex<HashSet<String>>>,
}

impl Runner for FakeRunner {
    async fn run(&mut self, job: &Job) -> Result<(), String> {
        self.runs
            .lock()
            .unwrap()
            .push((job.name.clone(), self.clock.now()));
        if self.failing.lock().unwrap().contains(&job.name) {
            Err("the connector's sign-in expired".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Default)]
struct MemoryJobs(Arc<Mutex<Vec<Job>>>);

impl JobStore for MemoryJobs {
    async fn jobs(&self) -> Result<Vec<Job>, TaskError> {
        Ok(self.0.lock().unwrap().clone())
    }

    async fn save_job(&self, job: &Job) -> Result<(), TaskError> {
        let mut jobs = self.0.lock().unwrap();
        jobs.retain(|j| j.name != job.name);
        jobs.push(job.clone());
        Ok(())
    }
}

struct Harness {
    clock: FakeClock,
    events: mpsc::UnboundedSender<PresenceEvent>,
    commands: mpsc::UnboundedSender<Command>,
    signals: mpsc::UnboundedReceiver<Signal>,
    runs: Runs,
    failing: Arc<Mutex<HashSet<String>>>,
    saved: MemoryJobs,
}

impl Harness {
    fn start(at: UtcDateTime, jobs: Vec<Job>) -> Harness {
        let clock = FakeClock::new(at);
        let (events, events_rx) = mpsc::unbounded_channel();
        let (commands, commands_rx) = mpsc::unbounded_channel();
        let (signals_tx, signals) = mpsc::unbounded_channel();
        let runs = Runs::default();
        let failing = Arc::new(Mutex::new(HashSet::new()));
        let saved = MemoryJobs::default();
        tokio::spawn(run(Parts {
            clock: clock.clone(),
            presence: FakePresence {
                events: events_rx,
                idle_s: Arc::new(AtomicU64::new(0)),
            },
            runner: FakeRunner {
                clock: clock.clone(),
                runs: runs.clone(),
                failing: failing.clone(),
            },
            store: saved.clone(),
            jobs,
            commands: commands_rx,
            signals: signals_tx,
        }));
        Harness {
            clock,
            events,
            commands,
            signals,
            runs,
            failing,
            saved,
        }
    }

    /// Sends a presence event and lets the scheduler handle it.
    async fn send(&self, event: PresenceEvent) {
        self.events.send(event).unwrap();
        settle().await;
    }

    async fn command(&self, command: Command) {
        self.commands.send(command).unwrap();
        settle().await;
    }

    /// Lets monotonic time pass, as when the Mac is awake.
    async fn pass(&self, d: SignedDuration) {
        tokio::time::sleep(d.unsigned_abs()).await;
        settle().await;
    }

    /// Moves wall time on without monotonic time, as a sleeping Mac does.
    fn sleep_mac(&self, d: SignedDuration) {
        *self.jump.lock().unwrap() += d;
    }

    fn runs(&self) -> Vec<(String, UtcDateTime)> {
        self.runs.lock().unwrap().clone()
    }

    fn saved(&self, name: &str) -> Job {
        let jobs = self.saved.0.lock().unwrap();
        jobs.iter().find(|j| j.name == name).unwrap().clone()
    }
}

impl std::ops::Deref for Harness {
    type Target = FakeClock;
    fn deref(&self) -> &FakeClock {
        &self.clock
    }
}

async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

// --- Times: Madrid in November is UTC+1 ----------------------------------------------------

fn local(day: u8, h: u8, m: u8) -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2026, Month::November, day).unwrap(),
        Time::from_hms(h, m, 0).unwrap(),
    ) - SignedDuration::hours(1)
}
const THU: u8 = 26;
const FRI: u8 = 27;

fn hm(h: u8, m: u8) -> Time {
    Time::from_hms(h, m, 0).unwrap()
}

const EVERY_DAY: WeekdaySet = WeekdaySet::ALL;

fn at_job(name: &str, kind: JobKind, h: u8, m: u8) -> Job {
    Job::new(
        name,
        kind,
        Schedule::At {
            time: hm(h, m),
            weekdays: EVERY_DAY,
        },
    )
}

fn names(runs: &[(String, UtcDateTime)]) -> Vec<&str> {
    runs.iter().map(|(n, _)| n.as_str()).collect()
}

// --- Timer ---------------------------------------------------------------------------------

/// AC-14, R-33: three jobs at 08:00, 12:00 and 18:30 make exactly three wake-ups in a day and
/// none in between; each run is saved with its time.
#[tokio::test(start_paused = true)]
async fn three_jobs_make_three_wake_ups_in_a_day() {
    let h = Harness::start(
        local(THU, 0, 0),
        vec![
            at_job("a", JobKind::Prompt, 8, 0),
            at_job("b", JobKind::Prompt, 12, 0),
            at_job("c", JobKind::Prompt, 18, 30),
        ],
    );
    settle().await;
    h.pass(SignedDuration::hours(24) - SignedDuration::MINUTE)
        .await;

    assert_eq!(
        h.runs(),
        vec![
            ("a".into(), local(THU, 8, 0)),
            ("b".into(), local(THU, 12, 0)),
            ("c".into(), local(THU, 18, 30)),
        ]
    );
    assert_eq!(h.wakeups.load(Ordering::SeqCst), 3);
    assert_eq!(h.saved("b").last_run, Some(local(THU, 12, 0)));
    assert_eq!(h.saved("b").last_status, Some(JobStatus::Ok));
}

/// AC-14, R-34, R-35: a wake after a sleep across 12:00 runs the 12:00 job once when inside
/// its catch-up window; another wake does not run it again.
#[tokio::test(start_paused = true)]
async fn a_wake_after_a_sleep_across_a_run_catches_up_once() {
    let mut noon = at_job("noon", JobKind::Prompt, 12, 0);
    noon.catch_up = SignedDuration::hours(2);
    noon.last_run = Some(local(25, 12, 0));
    let h = Harness::start(local(THU, 9, 0), vec![noon]);
    settle().await;

    h.pass(SignedDuration::minutes(150)).await; // 11:30, then asleep until 13:00
    h.sleep_mac(SignedDuration::minutes(90));
    h.send(PresenceEvent::Woke).await;
    assert_eq!(h.runs(), vec![("noon".into(), local(THU, 13, 0))]);

    h.send(PresenceEvent::Woke).await;
    h.send(PresenceEvent::ClockChanged).await;
    h.pass(SignedDuration::hours(2)).await;
    assert_eq!(h.runs().len(), 1, "never run twice for one missed time");
}

/// AC-14, R-35: outside its window the missed run is skipped.
#[tokio::test(start_paused = true)]
async fn a_wake_outside_the_window_skips_the_missed_run() {
    let mut noon = at_job("noon", JobKind::Prompt, 12, 0);
    noon.catch_up = SignedDuration::minutes(30);
    noon.last_run = Some(local(25, 12, 0));
    let h = Harness::start(local(THU, 9, 0), vec![noon]);
    settle().await;

    h.pass(SignedDuration::minutes(150)).await;
    h.sleep_mac(SignedDuration::minutes(90));
    h.send(PresenceEvent::Woke).await;
    h.pass(SignedDuration::hours(2)).await;
    assert!(h.runs().is_empty(), "{:?}", h.runs());
}

/// R-32, R-34: a time zone change computes the next run again: 18:30 becomes 18:30 New York
/// time (23:30 UTC), and the Madrid 18:30 (17:30 UTC) never runs.
#[tokio::test(start_paused = true)]
async fn a_zone_change_moves_the_timer() {
    let h = Harness::start(
        local(THU, 12, 0),
        vec![at_job("evening", JobKind::Prompt, 18, 30)],
    );
    settle().await;
    h.pass(SignedDuration::hours(1)).await;
    *h.offset_hours.lock().unwrap() = -5;
    h.send(PresenceEvent::ZoneChanged).await;
    h.pass(SignedDuration::hours(12)).await; // midnight UTC: 23:30 UTC has passed

    let utc_2330 = local(THU, 23, 30) + SignedDuration::hours(1);
    assert_eq!(h.runs(), vec![("evening".into(), utc_2330)]);
}

// --- Roll-over and the morning card --------------------------------------------------------

/// R-13, AC-7: roll-over runs exactly once per day start. HyperMe started at 08:00 after
/// missing 04:00 catches up once; wakes and clock changes do not repeat it; the next day
/// start runs it again.
#[tokio::test(start_paused = true)]
async fn roll_over_runs_once_per_day_start() {
    let config = Config::default();
    let mut jobs = builtin_jobs(&config.tasks);
    let rollover = jobs
        .iter_mut()
        .find(|j| j.kind == JobKind::RollOver)
        .unwrap();
    rollover.last_run = Some(local(25, 4, 0));
    let name = rollover.name.clone();
    let jobs: Vec<Job> = jobs.into_iter().filter(|j| j.name == name).collect();

    let h = Harness::start(local(THU, 8, 0), jobs);
    h.send(PresenceEvent::Launched).await;
    h.send(PresenceEvent::Woke).await;
    h.send(PresenceEvent::ClockChanged).await;
    h.send(PresenceEvent::Unlocked).await;
    assert_eq!(h.runs(), vec![(name.clone(), local(THU, 8, 0))]);

    h.pass(SignedDuration::hours(24)).await;
    assert_eq!(
        h.runs(),
        vec![(name.clone(), local(THU, 8, 0)), (name, local(FRI, 4, 0))]
    );
}

fn morning_only() -> Vec<Job> {
    builtin_jobs(&Config::default().tasks)
        .into_iter()
        .filter(|j| j.kind == JobKind::Morning)
        .collect()
}

/// AC-8 (scheduler half), R-15: locked all night, unlocked Friday at 07:40: the morning card
/// runs at 07:40 and not again that day; on Saturday, no card.
#[tokio::test(start_paused = true)]
async fn the_morning_card_runs_on_the_first_unlock_after_six_once_a_day() {
    let h = Harness::start(local(FRI, 5, 0), morning_only());
    h.send(PresenceEvent::Launched).await; // 05:00: too early
    h.send(PresenceEvent::Locked).await;
    assert!(h.runs().is_empty());

    h.pass(SignedDuration::minutes(160)).await; // 07:40
    h.send(PresenceEvent::Unlocked).await;
    h.send(PresenceEvent::Locked).await;
    h.pass(SignedDuration::minutes(20)).await;
    h.send(PresenceEvent::Unlocked).await;
    h.pass(SignedDuration::hours(3)).await; // past 09:30
    assert_eq!(names(&h.runs()), vec!["morning"]);
    assert_eq!(h.runs()[0].1, local(FRI, 7, 40));

    h.send(PresenceEvent::Locked).await;
    h.pass(SignedDuration::hours(21)).await; // Saturday 08:00
    h.send(PresenceEvent::Unlocked).await;
    h.pass(SignedDuration::hours(3)).await;
    assert_eq!(h.runs().len(), 1, "no card on Saturday");
}

/// AC-8 (scheduler half), R-16: already present at 05:00 and still present at 09:30, the
/// card runs at 09:30.
#[tokio::test(start_paused = true)]
async fn the_morning_card_runs_at_the_latest_time_for_a_user_already_present() {
    let h = Harness::start(local(FRI, 5, 0), morning_only());
    h.send(PresenceEvent::Launched).await;
    h.pass(SignedDuration::hours(5)).await;
    assert_eq!(h.runs(), vec![("morning".into(), local(FRI, 9, 30))]);
}

// --- Failures ------------------------------------------------------------------------------

/// AC-16, R-36: a failed job retries at the next presence event, with backoff of 1, 5 and 15
/// minutes; the second failure in a day signals "Job failed" once.
#[tokio::test(start_paused = true)]
async fn a_job_failing_twice_in_a_day_signals_once() {
    let mut h = Harness::start(
        local(THU, 7, 59),
        vec![at_job("sync", JobKind::Sync("github".into()), 8, 0)],
    );
    h.failing.lock().unwrap().insert("sync".into());
    h.send(PresenceEvent::Launched).await;
    h.pass(SignedDuration::minutes(1)).await; // 08:00: fails once

    h.pass(SignedDuration::seconds(30)).await;
    h.send(PresenceEvent::Unlocked).await; // inside the 1-minute backoff: no retry
    assert_eq!(h.runs().len(), 1);
    assert!(h.signals.try_recv().is_err());

    h.pass(SignedDuration::seconds(90)).await; // 08:02
    h.send(PresenceEvent::Unlocked).await; // retry, fails a second time
    assert_eq!(h.runs().len(), 2);
    assert_eq!(
        h.signals.try_recv().unwrap(),
        Signal::JobFailed { job: "sync".into() }
    );

    h.pass(SignedDuration::minutes(1)).await;
    h.send(PresenceEvent::Unlocked).await; // inside the 5-minute backoff
    assert_eq!(h.runs().len(), 2);
    h.pass(SignedDuration::minutes(5)).await; // 08:08
    h.send(PresenceEvent::Unlocked).await; // a third failure: no second signal
    assert_eq!(h.runs().len(), 3);
    assert!(h.signals.try_recv().is_err());

    let saved = h.saved("sync");
    assert_eq!(saved.fails_today, 3);
    assert!(matches!(saved.last_status, Some(JobStatus::Failed(_))));

    // It works again: the next retry clears the backoff.
    h.failing.lock().unwrap().clear();
    h.pass(SignedDuration::minutes(15)).await;
    h.send(PresenceEvent::Unlocked).await;
    assert_eq!(h.runs().len(), 4);
    assert_eq!(h.saved("sync").last_status, Some(JobStatus::Ok));
    h.pass(SignedDuration::minutes(30)).await;
    h.send(PresenceEvent::Unlocked).await;
    assert_eq!(h.runs().len(), 4, "no retry after a success");
}

// --- Pause ---------------------------------------------------------------------------------

/// R-23: while paused, cards are held and roll-over and sync still run; when the pause ends,
/// the held cards collapse into one, the latest.
#[tokio::test(start_paused = true)]
async fn a_pause_holds_cards_and_shows_the_latest_when_it_ends() {
    let mut jobs = morning_only();
    jobs.push(at_job("sync", JobKind::Sync("github".into()), 9, 45));
    jobs.push(at_job("evening", JobKind::Evening, 9, 50));
    let h = Harness::start(local(FRI, 5, 58), jobs);
    h.send(PresenceEvent::Launched).await;
    h.command(Command::Pause {
        until: local(FRI, 10, 0),
    })
    .await;
    h.pass(SignedDuration::minutes(4)).await; // 06:02: unlock after six, card held
    h.send(PresenceEvent::Unlocked).await;
    h.pass(SignedDuration::hours(4)).await; // past the 10:00 end of the pause

    assert_eq!(
        h.runs(),
        vec![
            ("sync".into(), local(FRI, 9, 45)),
            ("evening".into(), local(FRI, 10, 0)),
        ]
    );
    h.pass(SignedDuration::hours(2)).await;
    assert_eq!(
        h.runs().len(),
        2,
        "the held morning card is not shown again"
    );
}

/// R-23: "Resume" ends a pause early.
#[tokio::test(start_paused = true)]
async fn resuming_shows_the_held_card() {
    let h = Harness::start(local(FRI, 6, 0), morning_only());
    h.command(Command::Pause {
        until: local(FRI, 12, 0),
    })
    .await;
    h.send(PresenceEvent::Launched).await;
    assert!(h.runs().is_empty());
    h.pass(SignedDuration::minutes(10)).await;
    h.command(Command::Resume).await;
    assert_eq!(h.runs(), vec![("morning".into(), local(FRI, 6, 10))]);
}

// --- Low power -----------------------------------------------------------------------------

/// R-33a: on low power "every n minutes" jobs are skipped, and each runs once when power is
/// back; then they keep their rhythm.
#[tokio::test(start_paused = true)]
async fn low_power_skips_every_n_minutes_jobs_and_runs_them_once_after() {
    let sync = Job::new(
        "sync",
        JobKind::Sync("github".into()),
        Schedule::EveryWhilePresent { minutes: 30 },
    );
    let h = Harness::start(local(THU, 9, 0), vec![sync]);
    h.send(PresenceEvent::Launched).await;
    h.pass(SignedDuration::minutes(40)).await; // 09:00 and 09:30
    h.send(PresenceEvent::PowerChanged { low: true }).await;
    h.pass(SignedDuration::hours(2)).await; // 11:40
    h.send(PresenceEvent::PowerChanged { low: false }).await;
    h.pass(SignedDuration::minutes(35)).await; // 12:15

    let times: Vec<UtcDateTime> = h.runs().into_iter().map(|(_, t)| t).collect();
    assert_eq!(
        times,
        vec![
            local(THU, 9, 0),
            local(THU, 9, 30),
            local(THU, 11, 40),
            local(THU, 12, 10)
        ]
    );
}

/// R-33a: a wake also runs the jobs skipped on low power, once.
#[tokio::test(start_paused = true)]
async fn a_wake_runs_jobs_skipped_on_low_power_once() {
    let sync = Job::new(
        "sync",
        JobKind::Sync("github".into()),
        Schedule::EveryWhilePresent { minutes: 30 },
    );
    let h = Harness::start(local(THU, 9, 0), vec![sync]);
    h.send(PresenceEvent::Launched).await;
    h.send(PresenceEvent::PowerChanged { low: true }).await;
    h.pass(SignedDuration::minutes(40)).await;
    let before = h.runs().len();
    h.send(PresenceEvent::Woke).await;
    h.send(PresenceEvent::Woke).await;
    assert_eq!(h.runs().len(), before + 1);
}

// --- Connector sync (specs/97-google-connectors) -----------------------------------------

fn google_sync_job() -> Job {
    let text = format!(
        "{}\n[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"personal\"\n",
        hyperme_core::config::DEFAULT_CONFIG
    );
    let connectors = Config::parse(&text, std::path::Path::new("config.toml"))
        .unwrap()
        .connectors;
    let registry = hyperme_core::connector_settings::registry(None, &connectors).unwrap();
    hyperme_core::sync::sync_jobs(&registry, &connectors)
        .into_iter()
        .find(|job| job.name == "sync:google-calendar")
        .unwrap()
}

/// specs/97-google-connectors AC-8, R-16, R-17: with the default sync job, a 12-hour present
/// day makes no more than 40 calendar pulls, none while the Mac is asleep, and one on wake.
#[tokio::test(start_paused = true)]
async fn default_syncing_stays_under_forty_pulls_a_day_and_sleeps() {
    let mut jobs = morning_only();
    jobs.push(google_sync_job());
    let h = Harness::start(local(THU, 8, 0), jobs);
    h.send(PresenceEvent::Launched).await;
    h.pass(SignedDuration::hours(12)).await; // to 20:00
    let pulls = |h: &Harness| {
        h.runs()
            .into_iter()
            .filter(|(name, _)| name == "sync:google-calendar")
            .map(|(_, at)| at)
            .collect::<Vec<_>>()
    };
    let day = pulls(&h);
    assert!(day.len() <= 40, "{} pulls", day.len());
    assert!(day.len() >= 24, "{} pulls", day.len());

    h.send(PresenceEvent::Locked).await;
    h.send(PresenceEvent::Asleep).await;
    h.sleep_mac(SignedDuration::hours(11)); // the night, asleep: to 07:00
    assert_eq!(pulls(&h).len(), day.len(), "no pull while asleep");
    h.send(PresenceEvent::Woke).await;
    h.send(PresenceEvent::Unlocked).await;
    let after = pulls(&h);
    assert_eq!(after.len(), day.len() + 1, "one pull on wake");
    assert!(*after.last().unwrap() >= local(FRI, 7, 0));
}

/// R-16 ("Hyper, what's next"): a sync job runs at once on request, by name; an unknown name
/// does nothing.
#[tokio::test(start_paused = true)]
async fn a_sync_runs_on_request() {
    let h = Harness::start(local(THU, 9, 0), vec![google_sync_job()]);
    h.command(Command::Run {
        job: "sync:google-calendar".into(),
    })
    .await;
    h.command(Command::Run {
        job: "sync:nothing".into(),
    })
    .await;
    assert_eq!(names(&h.runs()), ["sync:google-calendar"]);
}

// --- Built-in jobs -------------------------------------------------------------------------

/// R-31, design.md: the built-in jobs come from the config (roll-over at day start every day,
/// catching up for a day; the morning card on presence); the stored state of a job survives a
/// new config, and jobs the user added are kept.
#[test]
fn built_in_jobs_come_from_the_config_and_keep_their_state() {
    let config = Config::default();
    let builtin = builtin_jobs(&config.tasks);
    let rollover = builtin
        .iter()
        .find(|j| j.kind == JobKind::RollOver)
        .unwrap();
    assert_eq!(
        rollover.schedule,
        Schedule::At {
            time: hm(4, 0),
            weekdays: WeekdaySet::ALL
        }
    );
    assert_eq!(rollover.catch_up, SignedDuration::hours(24));
    let morning = builtin.iter().find(|j| j.kind == JobKind::Morning).unwrap();
    assert_eq!(morning.schedule, config.tasks.morning());
    assert!(builtin.iter().all(|j| j.allowlist.is_empty()));
    assert!(WeekdaySet::ALL.contains(Weekday::Sunday));

    let mut stored_rollover = rollover.clone();
    stored_rollover.last_run = Some(local(25, 4, 0));
    stored_rollover.schedule = Schedule::At {
        time: hm(3, 0),
        weekdays: WeekdaySet::ALL,
    };
    let mut prompt = Job::new(
        "weekly review",
        JobKind::Prompt,
        Schedule::At {
            time: hm(17, 0),
            weekdays: WeekdaySet::from_array([Weekday::Friday]),
        },
    );
    prompt.allowlist = vec!["github.search_issues".into()];
    prompt.prompt = Some("List what I closed this week".into());

    let jobs = seed(vec![stored_rollover, prompt.clone()], builtin.clone());
    let got = jobs.iter().find(|j| j.name == rollover.name).unwrap();
    assert_eq!(got.last_run, Some(local(25, 4, 0)), "state kept");
    assert_eq!(got.schedule, rollover.schedule, "schedule from the config");
    assert!(jobs.contains(&prompt));
    assert_eq!(jobs.len(), builtin.len() + 1);
}
