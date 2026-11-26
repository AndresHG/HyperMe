//! The one built-in scheduler (specs/92-daily-tasks, design.md, "The scheduler"; R-30 to R-36,
//! R-23, R-33a): one async task, the job table in memory, one timer to the nearest next run,
//! presence events in, job runs out.
//!
//! The platform supplies the [`Clock`] (wall time, the local zone and a timer) and the
//! [`Presence`] events; the app supplies the [`Runner`] that does each job's work and the
//! [`JobStore`] that keeps the job table. Tests use a paused Tokio clock and fakes.

use std::future::Future;

use time::{Date, SignedDuration, UtcDateTime};
use tokio::sync::mpsc;

use crate::config;
use crate::schedule::{self, CatchUp, Schedule, Zone};
use crate::tasks::{TaskError, WeekdaySet};

/// Input in the last this many seconds, with the screen unlocked and the Mac awake, is
/// "present" (design.md, "Presence on macOS").
pub const PRESENT_INPUT_SECS: u64 = 10 * 60;

/// The waits before a failed job is retried at a presence event: after the first failure, the
/// second, and the third and later (R-36).
pub const BACKOFF: [SignedDuration; 3] = [
    SignedDuration::minutes(1),
    SignedDuration::minutes(5),
    SignedDuration::minutes(15),
];

/// Wall time, the local zone and a timer. On macOS the timer follows a monotonic clock that
/// stops while the Mac sleeps, which is why the scheduler recomputes on `Woke`.
pub trait Clock: Zone + Send {
    fn now(&self) -> UtcDateTime;
    fn sleep_until(&self, at: UtcDateTime) -> impl Future<Output = ()> + Send;
}

/// What the platform reports about the Mac and the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceEvent {
    Woke,
    Asleep,
    Unlocked,
    Locked,
    /// HyperMe started.
    Launched,
    ClockChanged,
    ZoneChanged,
    /// Low Power Mode or the battery under 20% (R-33a).
    PowerChanged {
        low: bool,
    },
}

/// The platform's presence events, and the time since the last input, read only when a job
/// asks.
pub trait Presence: Send {
    /// The next event; `None` when the platform stops sending them.
    fn next_event(&mut self) -> impl Future<Output = Option<PresenceEvent>> + Send;
    fn seconds_since_input(&self) -> u64;
}

/// What a job does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobKind {
    /// Roll-over at day start (R-13).
    RollOver,
    /// The morning card (R-15).
    Morning,
    /// The evening card (deferred, R-24).
    Evening,
    /// A nudge before a meeting or a due time (R-22).
    Nudge,
    /// A sync job for one connector, with read tools only.
    Sync(String),
    /// A prompt job from Settings → Automations.
    Prompt,
}

impl JobKind {
    /// Cards are held while HyperMe is paused (R-23).
    pub fn is_card(&self) -> bool {
        matches!(self, JobKind::Morning | JobKind::Evening | JobKind::Nudge)
    }
}

/// How a job's last run went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Ok,
    Failed(String),
}

/// One row of the job table (R-31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub name: String,
    pub kind: JobKind,
    pub schedule: Schedule,
    pub enabled: bool,
    /// How long after a missed time it still runs; zero skips missed runs (R-35).
    pub catch_up: SignedDuration,
    /// The tools the job may call; sync jobs get read tools only (SPEC section 4).
    pub allowlist: Vec<String>,
    /// Prompt jobs only.
    pub prompt: Option<String>,
    pub last_run: Option<UtcDateTime>,
    pub last_status: Option<JobStatus>,
    /// Failures on `fails_day`, the local day of the last failure.
    pub fails_today: u32,
    pub fails_day: Option<Date>,
}

impl Job {
    /// An enabled job that has never run, with no catch-up and no tools.
    pub fn new(name: &str, kind: JobKind, schedule: Schedule) -> Job {
        Job {
            name: name.to_owned(),
            kind,
            schedule,
            enabled: true,
            catch_up: SignedDuration::ZERO,
            allowlist: Vec::new(),
            prompt: None,
            last_run: None,
            last_status: None,
            fails_today: 0,
            fails_day: None,
        }
    }
}

/// Does a job's work: roll-over, a card, a sync. An error is recorded and retried (R-36).
pub trait Runner: Send {
    fn run(&mut self, job: &Job) -> impl Future<Output = Result<(), String>> + Send;
}

/// The job table on disk.
pub trait JobStore: Send {
    fn jobs(&self) -> impl Future<Output = Result<Vec<Job>, TaskError>> + Send;
    fn save_job(&self, job: &Job) -> impl Future<Output = Result<(), TaskError>> + Send;
}

/// What the app asks of the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// "Pause Hyper for 1 hour": hold cards and nudges until then (R-23).
    Pause {
        until: UtcDateTime,
    },
    Resume,
}

/// What the scheduler tells the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// A job failed for the second time today: show "Job failed" with ↵ Fix, once (R-36).
    JobFailed { job: String },
}

/// The built-in jobs from the config: roll-over at day start every day, catching up for a
/// day, and the morning card on presence. The evening card is deferred.
pub fn builtin_jobs(tasks: &config::Tasks) -> Vec<Job> {
    let rules = tasks.day_rules();
    let mut rollover = Job::new(
        "rollover",
        JobKind::RollOver,
        Schedule::At {
            time: rules.day_start,
            weekdays: WeekdaySet::ALL,
        },
    );
    rollover.catch_up = SignedDuration::hours(24);
    let morning = Job::new("morning", JobKind::Morning, tasks.morning());
    vec![rollover, morning]
}

/// The job table at launch: each built-in job takes its kind, schedule and catch-up from the
/// config and keeps its stored state; jobs the user added are kept as stored.
pub fn seed(stored: Vec<Job>, builtin: Vec<Job>) -> Vec<Job> {
    let mut jobs: Vec<Job> = builtin
        .into_iter()
        .map(|b| match stored.iter().find(|s| s.name == b.name) {
            Some(s) => Job {
                kind: b.kind,
                schedule: b.schedule,
                catch_up: b.catch_up,
                ..s.clone()
            },
            None => b,
        })
        .collect();
    for s in stored {
        if !jobs.iter().any(|j| j.name == s.name) {
            jobs.push(s);
        }
    }
    jobs
}

/// What [`run`] needs.
#[derive(Debug)]
pub struct Parts<C, P, R, S> {
    pub clock: C,
    pub presence: P,
    pub runner: R,
    pub store: S,
    pub jobs: Vec<Job>,
    pub commands: mpsc::UnboundedReceiver<Command>,
    pub signals: mpsc::UnboundedSender<Signal>,
}

struct Entry {
    job: Job,
    next: Option<UtcDateTime>,
    retry_after: Option<UtcDateTime>,
}

struct Scheduler<C, P, R, S> {
    clock: C,
    presence: P,
    runner: R,
    store: S,
    jobs: Vec<Entry>,
    commands: mpsc::UnboundedReceiver<Command>,
    signals: mpsc::UnboundedSender<Signal>,
    presence_open: bool,
    unlocked: bool,
    awake: bool,
    low_power: bool,
    /// An "every n minutes" job was skipped on low power (R-33a).
    missed_on_low_power: bool,
    paused_until: Option<UtcDateTime>,
    /// Cards held during a pause, in the order they came.
    held: Vec<usize>,
}

/// Runs the scheduler until the command channel closes. At start it catches up missed runs;
/// then it waits on one timer for the nearest next run, and on presence events and commands.
pub async fn run<C: Clock, P: Presence, R: Runner, S: JobStore>(parts: Parts<C, P, R, S>) {
    let mut s = Scheduler {
        clock: parts.clock,
        presence: parts.presence,
        runner: parts.runner,
        store: parts.store,
        jobs: parts
            .jobs
            .into_iter()
            .map(|job| Entry {
                job,
                next: None,
                retry_after: None,
            })
            .collect(),
        commands: parts.commands,
        signals: parts.signals,
        presence_open: true,
        unlocked: false,
        awake: true,
        low_power: false,
        missed_on_low_power: false,
        paused_until: None,
        held: Vec::new(),
    };
    let now = s.clock.now();
    s.recompute(now);
    s.catch_up_all(now).await;

    loop {
        let wake_at = s.wake_at();
        tokio::select! {
            biased;
            command = s.commands.recv() => match command {
                Some(command) => s.command(command).await,
                None => return,
            },
            event = s.presence.next_event(), if s.presence_open => match event {
                Some(event) => s.event(event).await,
                None => s.presence_open = false,
            },
            () = sleep_or_never(&s.clock, wake_at) => s.due().await,
        }
    }
}

async fn sleep_or_never<C: Clock>(clock: &C, at: Option<UtcDateTime>) {
    match at {
        Some(at) => clock.sleep_until(at).await,
        None => std::future::pending().await,
    }
}

impl<C: Clock, P: Presence, R: Runner, S: JobStore> Scheduler<C, P, R, S> {
    fn local_date(&self, at: UtcDateTime) -> Date {
        at.to_offset(self.clock.offset_at(at)).date()
    }

    fn present(&self) -> bool {
        self.unlocked && self.awake && self.presence.seconds_since_input() < PRESENT_INPUT_SECS
    }

    fn ran_today(&self, i: usize, now: UtcDateTime) -> bool {
        self.jobs[i]
            .job
            .last_run
            .is_some_and(|ran| self.local_date(ran) == self.local_date(now))
    }

    /// The next run of job `i` after `now`. "Every n minutes" counts from its last run.
    fn next_for(&self, i: usize, now: UtcDateTime) -> Option<UtcDateTime> {
        let job = &self.jobs[i].job;
        if !job.enabled {
            return None;
        }
        match job.schedule {
            Schedule::EveryWhilePresent { minutes: 0 } => None,
            Schedule::EveryWhilePresent { minutes } => {
                let every = SignedDuration::minutes(i64::from(minutes));
                Some(job.last_run.map_or(now, |ran| ran + every).max(now))
            }
            _ => schedule::next_run(&job.schedule, now, &self.clock),
        }
    }

    /// Computes every next run again, from the wall clock (R-34).
    fn recompute(&mut self, now: UtcDateTime) {
        for i in 0..self.jobs.len() {
            self.jobs[i].next = self.next_for(i, now);
        }
    }

    fn wake_at(&self) -> Option<UtcDateTime> {
        self.jobs
            .iter()
            .filter_map(|e| e.next)
            .chain(self.paused_until)
            .min()
    }

    /// Runs, once each, the timed jobs whose last time was missed inside their window (R-35).
    async fn catch_up_all(&mut self, now: UtcDateTime) {
        for i in 0..self.jobs.len() {
            let job = &self.jobs[i].job;
            if !job.enabled {
                continue;
            }
            let decision =
                schedule::catch_up(&job.schedule, job.last_run, now, job.catch_up, &self.clock);
            if let CatchUp::Run { .. } = decision {
                self.run_or_hold(i, now).await;
                self.jobs[i].next = self.next_for(i, now);
            }
        }
    }

    /// The timer fired: run what is due, and end a pause that is over.
    async fn due(&mut self) {
        let now = self.clock.now();
        if self.paused_until.is_some_and(|until| until <= now) {
            self.resume(now).await;
        }
        for i in 0..self.jobs.len() {
            if !self.jobs[i].next.is_some_and(|next| next <= now) {
                continue;
            }
            match self.jobs[i].job.schedule {
                Schedule::At { .. } => self.run_or_hold(i, now).await,
                Schedule::OnPresenceAfter { .. } => {
                    if self.present() && !self.ran_today(i, now) {
                        self.run_or_hold(i, now).await;
                    }
                }
                Schedule::EveryWhilePresent { minutes } => {
                    if self.low_power {
                        self.missed_on_low_power = true;
                    } else if self.present() {
                        self.run(i, now).await;
                    }
                    let every = SignedDuration::minutes(i64::from(minutes));
                    self.jobs[i].next = Some(now + every);
                    continue;
                }
            }
            self.jobs[i].next = self.next_for(i, now);
        }
    }

    async fn event(&mut self, event: PresenceEvent) {
        let now = self.clock.now();
        match event {
            PresenceEvent::Woke => {
                self.awake = true;
                self.recompute(now);
                self.catch_up_all(now).await;
                if self.missed_on_low_power {
                    self.run_every(now).await;
                }
            }
            PresenceEvent::Asleep => self.awake = false,
            PresenceEvent::Unlocked | PresenceEvent::Launched => {
                self.unlocked = true;
                self.awake = true;
                self.on_presence(now).await;
            }
            PresenceEvent::Locked => self.unlocked = false,
            PresenceEvent::ClockChanged | PresenceEvent::ZoneChanged => {
                self.recompute(now);
                self.catch_up_all(now).await;
            }
            PresenceEvent::PowerChanged { low } => {
                let was_low = self.low_power;
                self.low_power = low;
                if was_low && !low && self.missed_on_low_power {
                    self.run_every(now).await;
                }
            }
        }
    }

    /// The user sat down: the morning card if it is time and not yet shown today (R-15), and
    /// failed jobs whose backoff is over (R-36).
    async fn on_presence(&mut self, now: UtcDateTime) {
        for i in 0..self.jobs.len() {
            let job = &self.jobs[i].job;
            if job.enabled
                && schedule::on_presence(&job.schedule, now, &self.clock)
                && !self.ran_today(i, now)
            {
                self.run_or_hold(i, now).await;
            }
        }
        for i in 0..self.jobs.len() {
            if self.jobs[i].retry_after.is_some_and(|at| at <= now) {
                self.run(i, now).await;
            }
        }
    }

    /// Runs each "every n minutes" job once, after low power (R-33a).
    async fn run_every(&mut self, now: UtcDateTime) {
        self.missed_on_low_power = false;
        for i in 0..self.jobs.len() {
            let job = &self.jobs[i].job;
            if let (true, Schedule::EveryWhilePresent { minutes }) = (job.enabled, job.schedule)
                && minutes > 0
            {
                self.run(i, now).await;
                self.jobs[i].next = Some(now + SignedDuration::minutes(i64::from(minutes)));
            }
        }
    }

    /// Runs job `i`, or holds it if it is a card and HyperMe is paused. A held card counts as
    /// run, so it is not shown again when the pause ends unless it is the one kept (R-23).
    async fn run_or_hold(&mut self, i: usize, now: UtcDateTime) {
        let paused = self.paused_until.is_some_and(|until| until > now);
        if paused && self.jobs[i].job.kind.is_card() {
            self.jobs[i].job.last_run = Some(now);
            self.held.push(i);
            self.save(i).await;
        } else {
            self.run(i, now).await;
        }
    }

    async fn run(&mut self, i: usize, now: UtcDateTime) {
        let result = self.runner.run(&self.jobs[i].job).await;
        let today = self.local_date(now);
        let entry = &mut self.jobs[i];
        entry.job.last_run = Some(now);
        if entry.job.fails_day != Some(today) {
            entry.job.fails_today = 0;
            entry.job.fails_day = Some(today);
        }
        match result {
            Ok(()) => {
                entry.job.last_status = Some(JobStatus::Ok);
                entry.retry_after = None;
            }
            Err(message) => {
                entry.job.last_status = Some(JobStatus::Failed(message));
                entry.job.fails_today += 1;
                let wait = BACKOFF[(entry.job.fails_today as usize - 1).min(BACKOFF.len() - 1)];
                entry.retry_after = Some(now + wait);
                if entry.job.fails_today == 2 {
                    // The app may have stopped listening; the run itself still counts.
                    let _ = self.signals.send(Signal::JobFailed {
                        job: entry.job.name.clone(),
                    });
                }
            }
        }
        self.save(i).await;
    }

    async fn save(&mut self, i: usize) {
        // A failed save loses only the run's bookkeeping; the next run saves again.
        let _ = self.store.save_job(&self.jobs[i].job).await;
    }

    async fn command(&mut self, command: Command) {
        let now = self.clock.now();
        match command {
            Command::Pause { until } if until > now => self.paused_until = Some(until),
            Command::Pause { .. } | Command::Resume => self.resume(now).await,
        }
    }

    /// Ends a pause: the held cards collapse into one, the latest morning or evening card.
    /// Held nudges are dropped, since their moment has passed (R-23).
    async fn resume(&mut self, now: UtcDateTime) {
        self.paused_until = None;
        let held = std::mem::take(&mut self.held);
        let latest = held
            .into_iter()
            .rev()
            .find(|&i| matches!(self.jobs[i].job.kind, JobKind::Morning | JobKind::Evening));
        if let Some(i) = latest {
            self.run(i, now).await;
        }
    }
}
