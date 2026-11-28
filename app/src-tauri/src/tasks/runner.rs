//! The daily runner: rolls the day over, builds the morning card and runs the connectors' sync
//! jobs. Tested with a fake clock, a fake UI and a fake syncer on a temporary store.

use super::*;

/// The sync jobs that fill each part of the morning card, by job name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Feeds {
    /// Jobs that pull calendar events.
    pub events: Vec<String>,
    /// Jobs that pull mail.
    pub mail: Vec<String>,
}

/// What the runner shows.
pub trait Ui: Send + Sync {
    /// Today changed: Home reloads it if open.
    fn refresh(&self);
    /// The morning card, built now. `update` is true when a sync filled in a card already
    /// shown today (R-18); the view replaces it only if it is still on screen.
    fn morning(&self, card: &MorningCard, update: bool);
    /// A nudge before a meeting or a due time (R-22).
    fn nudge(&self, card: &crate::pill::AlertView);
}

/// Runs one connector's pulls for every account that has it.
pub trait Syncer: Send + Sync {
    /// Runs sync job `id` (`<connector>-<server>`) with only the tools in `allowlist`.
    fn sync(
        &self,
        id: &str,
        allowlist: &[String],
        now: UtcDateTime,
    ) -> impl Future<Output = Result<(), String>> + Send;
    /// Which sync jobs fill which part, from the connectors connected now.
    fn feeds(&self) -> Feeds;
}

/// Does each job's work (R-13, R-15 to R-18, R-21).
pub struct DailyRunner<C, Y, U> {
    pub store: Arc<SqliteTaskStore>,
    pub clock: C,
    pub rules: DayRules,
    pub syncer: Y,
    pub ui: U,
}

pub(super) fn ms(at: UtcDateTime) -> i64 {
    (at - UtcDateTime::UNIX_EPOCH).whole_milliseconds() as i64
}

/// Today's date by the day start, and when it began (Unix ms), at `at`.
pub(super) fn today_of(zone: &dyn Zone, rules: &DayRules, at: UtcDateTime) -> (Date, i64) {
    let offset = zone.offset_at(at);
    let local = at.to_offset(offset);
    let today = rules.day_of(PlainDateTime::new(local.date(), local.time()));
    let began = rules.starts_at(today).assume_offset(offset).to_utc();
    (today, ms(began))
}

/// Today as Home shows it, from the store at `now` (R-10, R-21).
pub(super) async fn load_today(
    store: &SqliteTaskStore,
    zone: &(dyn Zone + Sync),
    rules: &DayRules,
    now: UtcDateTime,
) -> Result<TodayView, String> {
    let (today, day_began_ms) = today_of(zone, rules, now);
    let snapshot = store
        .snapshot(today, day_began_ms)
        .await
        .map_err(|e| e.to_string())?;
    let at = Now {
        today,
        day_began_ms,
        rules: *rules,
        morning_shown_ms: store.morning_at(today).map_err(|e| e.to_string())?,
        today_seen_ms: store.seen_at(today).map_err(|e| e.to_string())?,
    };
    Ok(brief::today(&snapshot, &at))
}

impl<C: Clock + Sync, Y: Syncer, U: Ui> DailyRunner<C, Y, U> {
    fn today(&self, at: UtcDateTime) -> (Date, i64) {
        today_of(&self.clock, &self.rules, at)
    }

    /// The morning card from the store and the caches, never the network (R-18). A part
    /// whose sync has not yet gone through shows "Updating…"; `synced` is a sync job that just
    /// did, before the scheduler saved it.
    async fn morning_card(
        &self,
        now: UtcDateTime,
        shown_ms: i64,
        synced: Option<&str>,
    ) -> Result<MorningCard, String> {
        let (today, day_began_ms) = self.today(now);
        let snapshot = self
            .store
            .snapshot(today, day_began_ms)
            .await
            .map_err(|e| e.to_string())?;
        let jobs = self.store.jobs().await.map_err(|e| e.to_string())?;
        let feeds = self.syncer.feeds();
        let ready = |names: &[String]| {
            names.is_empty()
                || names.iter().any(|name| {
                    synced == Some(name.as_str())
                        || jobs
                            .iter()
                            .any(|j| &j.name == name && j.last_status == Some(JobStatus::Ok))
                })
        };
        let meetings = if ready(&feeds.events) {
            let zone = &self.clock;
            Part::Ready(
                sync::meetings_on(&*self.store, today, zone)?
                    .into_iter()
                    .map(|m| Meeting {
                        title: m.title,
                        start: m.start.to_offset(zone.offset_at(m.start)).time(),
                        end: m.end.to_offset(zone.offset_at(m.end)).time(),
                        account: Some(m.account),
                        link: m.url,
                    })
                    .collect(),
            )
        } else {
            Part::Pending
        };
        let mail_to_answer = if ready(&feeds.mail) {
            Part::Ready(sync::threads_to_answer(&*self.store, now)? as u32)
        } else {
            Part::Pending
        };
        let yesterday = today
            .checked_sub(Duration::days(1))
            .map(|day| self.store.day_log(day))
            .transpose()
            .map_err(|e| e.to_string())?
            .filter(|log| *log != Default::default())
            .map(|log| DayCounts {
                captured: log.captured,
                done: log.done,
                moved: log.moved,
                dropped: log.dropped,
            });
        let now_view = Now {
            today,
            day_began_ms,
            rules: self.rules,
            morning_shown_ms: Some(shown_ms),
            today_seen_ms: self.store.seen_at(today).map_err(|e| e.to_string())?,
        };
        let sources = Sources {
            meetings,
            mail_to_answer,
            proposals: 0,
            yesterday,
        };
        Ok(brief::morning(&snapshot, &now_view, &sources))
    }
}

impl<C: Clock + Sync, Y: Syncer, U: Ui> Runner for DailyRunner<C, Y, U> {
    async fn run(&mut self, job: &Job) -> Result<(), String> {
        let now = self.clock.now();
        match &job.kind {
            JobKind::RollOver => {
                let (today, _) = self.today(now);
                self.store
                    .roll_over(today)
                    .await
                    .map_err(|e| e.to_string())?;
                self.ui.refresh();
            }
            JobKind::Morning => {
                let (today, _) = self.today(now);
                self.store
                    .mark_morning(today, ms(now))
                    .map_err(|e| e.to_string())?;
                let card = self.morning_card(now, ms(now), None).await?;
                self.ui.morning(&card, false);
                self.ui.refresh();
            }
            JobKind::Sync(id) => {
                self.syncer.sync(id, &job.allowlist, now).await?;
                let (today, _) = self.today(now);
                if let Some(shown) = self.store.morning_at(today).map_err(|e| e.to_string())? {
                    let card = self.morning_card(now, shown, Some(&job.name)).await?;
                    self.ui.morning(&card, true);
                }
                self.ui.refresh();
            }
            JobKind::Nudge => {
                let (today, day_began_ms) = self.today(now);
                let meetings = sync::meetings_on(&*self.store, today, &self.clock)?;
                let snapshot = self
                    .store
                    .snapshot(today, day_began_ms)
                    .await
                    .map_err(|e| e.to_string())?;
                if let Some(card) = nudge_card(&job.name, &meetings, &snapshot, now, &self.clock) {
                    self.ui.nudge(&card);
                }
            }
            // The evening card and prompt jobs are later slices.
            JobKind::Evening | JobKind::Prompt => {}
        }
        Ok(())
    }
}

// --- Nudges (slice 10, R-22, AC-10) -------------------------------------------------------
