use std::sync::Mutex;

use hyperme_core::scheduler::{Job, JobKind, JobStatus, JobStore, Runner};
use hyperme_core::sync::{Event, EventStore};
use hyperme_core::tasks::{DayRules, NewTask, Source, TaskStore};
use hyperme_platform::task_store::DayCounter;
use time::{Date, Month, SignedDuration, Time, UtcDateTime, UtcOffset};

use super::*;

/// Madrid in November, UTC+1, stopped at one moment.
struct FixedClock(UtcDateTime);

impl hyperme_core::schedule::Zone for FixedClock {
    fn offset_at(&self, _at: UtcDateTime) -> UtcOffset {
        UtcOffset::from_hms(1, 0, 0).unwrap()
    }
}

impl Clock for FixedClock {
    fn now(&self) -> UtcDateTime {
        self.0
    }
    async fn sleep_until(&self, _at: UtcDateTime) {
        std::future::pending::<()>().await
    }
}

#[derive(Default)]
struct FakeUi {
    refreshes: Mutex<usize>,
    cards: Mutex<Vec<(MorningCard, bool)>>,
    nudges: Mutex<Vec<crate::pill::AlertView>>,
}

impl Ui for Arc<FakeUi> {
    fn refresh(&self) {
        *self.refreshes.lock().unwrap() += 1;
    }
    fn morning(&self, card: &MorningCard, update: bool) {
        self.cards.lock().unwrap().push((card.clone(), update));
    }
    fn nudge(&self, card: &crate::pill::AlertView) {
        self.nudges.lock().unwrap().push(card.clone());
    }
}

type Calls = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// A syncer that writes one meeting into the cache, or fails.
struct FakeSyncer {
    store: Arc<SqliteTaskStore>,
    feeds: Feeds,
    fail: bool,
    calls: Calls,
}

impl Syncer for FakeSyncer {
    async fn sync(&self, id: &str, allowlist: &[String], now: UtcDateTime) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push((id.to_owned(), allowlist.to_vec()));
        if self.fail {
            return Err("no network".into());
        }
        let start = local(FRI, 10, 0);
        let event = Event {
            account: "google:personal".into(),
            id: "e1".into(),
            calendar: None,
            title: "Standup".into(),
            start,
            end: start + SignedDuration::minutes(15),
            all_day: false,
            url: Some("https://meet.example/abc".into()),
            status: None,
        };
        self.store.replace(
            "google:personal",
            now - SignedDuration::days(1),
            now + SignedDuration::days(7),
            &[event],
        )
    }
    fn feeds(&self) -> Feeds {
        self.feeds.clone()
    }
}

const THU: u8 = 26;
const FRI: u8 = 27;

fn day(d: u8) -> Date {
    Date::from_calendar_date(2026, Month::November, d).unwrap()
}

fn local(d: u8, h: u8, m: u8) -> UtcDateTime {
    UtcDateTime::new(day(d), Time::from_hms(h, m, 0).unwrap()) - SignedDuration::hours(1)
}

fn store(name: &str) -> Arc<SqliteTaskStore> {
    let dir = std::env::temp_dir().join(format!("hyperme-app-tasks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.sqlite3"));
    let _ = std::fs::remove_file(&path);
    Arc::new(SqliteTaskStore::open(&path).unwrap())
}

fn calendar_feeds() -> Feeds {
    Feeds {
        events: vec!["sync:google-calendar".into()],
        mail: vec!["sync:google-gmail".into()],
    }
}

struct Setup {
    runner: DailyRunner<FixedClock, FakeSyncer, Arc<FakeUi>>,
    ui: Arc<FakeUi>,
    calls: Calls,
}

fn setup(store: Arc<SqliteTaskStore>, at: UtcDateTime, feeds: Feeds, fail: bool) -> Setup {
    let ui = Arc::new(FakeUi::default());
    let calls = Arc::default();
    Setup {
        runner: DailyRunner {
            store: store.clone(),
            clock: FixedClock(at),
            rules: DayRules::default(),
            syncer: FakeSyncer {
                store,
                feeds,
                fail,
                calls: Arc::clone(&calls),
            },
            ui: ui.clone(),
        },
        ui,
        calls,
    }
}

fn task(title: &str, planned: Date, arrived: Date) -> NewTask {
    NewTask {
        title: title.into(),
        notes: None,
        day: Some(planned),
        due: None,
        source: Source::HyperMe,
        blocked_by: Vec::new(),
        created_at: 0,
        arrived_day: arrived,
    }
}

fn job(name: &str, kind: JobKind) -> Job {
    Job::new(
        name,
        kind,
        hyperme_core::schedule::Schedule::EveryWhilePresent { minutes: 30 },
    )
}

fn sync_job() -> Job {
    let mut job = job(
        "sync:google-calendar",
        JobKind::Sync("google-calendar".into()),
    );
    job.allowlist = vec!["list_events".into()];
    job
}

fn block_on<F: Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

/// AC-7 (app half), R-13: the roll-over job rolls yesterday's open task into today with a
/// count of one, and Home reloads Today.
#[test]
fn roll_over_moves_yesterdays_tasks_and_refreshes_home() {
    let store = store("rollover");
    let id = block_on(store.add(task("Call the dentist", day(THU), day(THU)))).unwrap();
    let mut s = setup(store.clone(), local(FRI, 4, 0), Feeds::default(), false);
    block_on(s.runner.run(&job("rollover", JobKind::RollOver))).unwrap();

    let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
    let rolled = snapshot.iter().find(|t| t.id == id).unwrap();
    assert_eq!(rolled.rolled, 1);
    assert_eq!(rolled.day, Some(day(THU)), "original day kept");
    assert_eq!(*s.ui.refreshes.lock().unwrap(), 1);
}

/// AC-9 (app half), R-17, R-18: the morning card comes from the store before any sync:
/// tasks for today, yesterday's count, and "Updating…" for calendar and mail. The day log
/// records when it showed.
#[test]
fn the_morning_card_shows_before_the_syncs_with_updating_parts() {
    let store = store("morning_pending");
    block_on(store.add(task("Send the report", day(FRI), day(THU)))).unwrap();
    store.count(day(THU), DayCounter::Done, 3).unwrap();
    let at = local(FRI, 7, 40);
    let mut s = setup(store.clone(), at, calendar_feeds(), false);
    block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();

    let cards = s.ui.cards.lock().unwrap();
    let (card, update) = &cards[0];
    assert!(!update);
    assert_eq!(card.date, day(FRI));
    assert_eq!(card.meetings, Part::Pending);
    assert_eq!(card.mail_to_answer, Part::Pending);
    assert_eq!(card.top[0].title, "Send the report");
    assert_eq!(card.yesterday.map(|y| y.done), Some(3));
    assert_eq!(store.morning_at(day(FRI)).unwrap(), Some(ms(at)));
    assert_eq!(*s.ui.refreshes.lock().unwrap(), 1);
}

/// R-17: with no calendar or mail connected, those parts are simply empty, never
/// "Updating…"; a day with nothing in the log has no "Yesterday".
#[test]
fn without_connectors_the_card_has_no_updating_parts() {
    let store = store("morning_none");
    let mut s = setup(store, local(FRI, 7, 40), Feeds::default(), false);
    block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
    let cards = s.ui.cards.lock().unwrap();
    assert_eq!(cards[0].0.meetings, Part::Ready(Vec::new()));
    assert_eq!(cards[0].0.mail_to_answer, Part::Ready(0));
    assert_eq!(cards[0].0.yesterday, None);
}

/// AC-9, R-18: when the calendar sync returns after the card showed, the card is built
/// again with today's meetings in local time, as an update; the sync ran with its own
/// allowlist only.
#[test]
fn a_sync_after_the_card_fills_in_the_meetings() {
    let store = store("morning_fill");
    let at = local(FRI, 7, 40);
    let mut s = setup(store.clone(), at, calendar_feeds(), false);
    block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
    block_on(s.runner.run(&sync_job())).unwrap();

    assert_eq!(
        *s.calls.lock().unwrap(),
        [("google-calendar".to_owned(), vec!["list_events".to_owned()])]
    );
    let cards = s.ui.cards.lock().unwrap();
    let (card, update) = &cards[1];
    assert!(update);
    let Part::Ready(meetings) = &card.meetings else {
        panic!("meetings still pending");
    };
    assert_eq!(meetings[0].title, "Standup");
    assert_eq!(meetings[0].start, Time::from_hms(10, 0, 0).unwrap());
    assert_eq!(card.mail_to_answer, Part::Pending, "mail has not synced");
    assert_eq!(*s.ui.refreshes.lock().unwrap(), 2);
}

/// R-18: a sync the scheduler saved as OK, on an earlier day or before a restart, makes
/// its part ready on the next card.
#[test]
fn a_sync_saved_as_ok_makes_its_part_ready() {
    let store = store("saved_ok");
    let mut saved = sync_job();
    saved.last_status = Some(JobStatus::Ok);
    block_on(store.save_job(&saved)).unwrap();
    let mut s = setup(store, local(FRI, 7, 40), calendar_feeds(), false);
    block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
    let cards = s.ui.cards.lock().unwrap();
    assert_eq!(cards[0].0.meetings, Part::Ready(Vec::new()));
    assert_eq!(cards[0].0.mail_to_answer, Part::Pending);
}

/// R-15, R-21: a sync before any morning card that day updates Home only; no card.
#[test]
fn a_sync_before_the_card_only_refreshes_home() {
    let store = store("sync_first");
    let mut s = setup(store, local(FRI, 5, 0), calendar_feeds(), false);
    block_on(s.runner.run(&sync_job())).unwrap();
    assert!(s.ui.cards.lock().unwrap().is_empty());
    assert_eq!(*s.ui.refreshes.lock().unwrap(), 1);
}

/// R-36: a failed sync is returned to the scheduler, which records it and retries; nothing
/// is redrawn.
#[test]
fn a_failed_sync_is_returned_and_redraws_nothing() {
    let store = store("sync_fail");
    let mut s = setup(store, local(FRI, 9, 0), calendar_feeds(), true);
    let result = block_on(s.runner.run(&sync_job()));
    assert_eq!(result, Err("no network".to_owned()));
    assert_eq!(*s.ui.refreshes.lock().unwrap(), 0);
}

// --- Home's Today ---

fn meeting(title: &str, start: UtcDateTime, minutes: i64) -> sync::Meeting {
    sync::Meeting {
        title: title.into(),
        start,
        end: start + SignedDuration::minutes(minutes),
        account: "google:personal".into(),
        url: Some("https://meet.example/x".into()),
    }
}

/// Owner, 27 November 2026: Home leads with today's meetings in local time; the next one
/// says "in n min" (rounded up), one under way says now, and past ones are marked.
#[test]
fn the_today_block_lists_meetings_with_the_next_one_in_minutes() {
    let now = local(FRI, 9, 48);
    let view = block_on(load_today(
        &store("page_meetings"),
        &FixedClock(now),
        &DayRules::default(),
        now,
    ))
    .unwrap();
    let meetings = [
        meeting("Standup", local(FRI, 9, 0), 15),
        meeting("Review", local(FRI, 9, 40), 30),
        meeting("Planning", local(FRI, 10, 0), 60),
        meeting("1:1", local(FRI, 15, 0), 30),
    ];
    let page = today_page(&view, &meetings, now, &FixedClock(now));
    let rows: Vec<(&str, &str, Option<i64>, bool, bool)> = page
        .meetings
        .iter()
        .map(|m| {
            (
                m.title.as_str(),
                m.start.as_str(),
                m.starts_in_min,
                m.now,
                m.past,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Standup", "09:00", None, false, true),
            ("Review", "09:40", None, true, false),
            ("Planning", "10:00", Some(12), false, false),
            ("1:1", "15:00", None, false, false),
        ]
    );
    assert_eq!(page.meetings[2].end, "11:00");
    assert_eq!(page.today, "2026-11-27");
    assert_eq!(page.tomorrow, "2026-11-28");
}

/// AC-5, AC-10, R-10, R-11, R-21: lines carry "new" after the morning card until Today is
/// seen, "unblocked" or "blocker dropped", and due marks; a blocked task sits under
/// "Waiting on" with its blocker's title.
#[test]
fn the_today_block_marks_new_unblocked_and_waiting() {
    let store = store("page_marks");
    let ins = block_on(store.add(task("Call the insurance", day(FRI), day(FRI)))).unwrap();
    let mut send = task("Send Ana the numbers", day(FRI), day(FRI));
    send.blocked_by = vec![ins];
    let send = block_on(store.add(send)).unwrap();
    let morning = local(FRI, 7, 40);
    store.mark_morning(day(FRI), ms(morning)).unwrap();
    let mut later = task("Book the room", day(FRI), day(FRI));
    later.created_at = ms(local(FRI, 15, 0));
    block_on(store.add(later)).unwrap();

    let now = local(FRI, 15, 30);
    let clock = FixedClock(now);
    let view = block_on(load_today(&store, &clock, &DayRules::default(), now)).unwrap();
    let page = today_page(&view, &[], now, &clock);
    assert_eq!(page.waiting.len(), 1);
    assert_eq!(page.waiting[0].on, "Call the insurance");
    let book = page
        .lines
        .iter()
        .find(|l| l.title == "Book the room")
        .unwrap();
    assert_eq!(book.marks, ["new"]);
    assert!(page.has_new);

    block_on(act(&store, ins, Act::Drop, day(FRI), ms(now))).unwrap();
    store.mark_seen(day(FRI), ms(now)).unwrap();
    let view = block_on(load_today(&store, &clock, &DayRules::default(), now)).unwrap();
    let page = today_page(&view, &[], now, &clock);
    let unblocked = page.lines.iter().find(|l| l.id == send).unwrap();
    assert_eq!(unblocked.marks, ["blocker dropped"]);
    assert!(!page.has_new, "seen clears new");
    assert!(page.waiting.is_empty());
}

/// R-4, R-29: Home's field adds a task for today, counted as captured; done, drop and
/// tomorrow count in the day log; rename changes the title; an empty title or a past day
/// is refused.
#[test]
fn row_actions_change_the_task_and_count_in_the_day_log() {
    let store = store("acts");
    let now_ms = ms(local(FRI, 11, 0));
    assert!(block_on(add_today(&store, "  ", day(FRI), now_ms)).is_err());
    let a = block_on(add_today(&store, " Call the dentist ", day(FRI), now_ms)).unwrap();
    let b = block_on(add_today(&store, "Pay the bill", day(FRI), now_ms)).unwrap();
    let c = block_on(add_today(&store, "Write the notes", day(FRI), now_ms)).unwrap();
    let d = block_on(add_today(&store, "Old thing", day(FRI), now_ms)).unwrap();

    block_on(act(&store, a, Act::Done, day(FRI), now_ms)).unwrap();
    block_on(act(&store, b, Act::Drop, day(FRI), now_ms)).unwrap();
    block_on(act(&store, c, Act::Tomorrow, day(FRI), now_ms)).unwrap();
    block_on(act(
        &store,
        d,
        Act::Rename("Old thing, renamed".into()),
        day(FRI),
        now_ms,
    ))
    .unwrap();
    assert!(block_on(act(&store, d, Act::Rename(" ".into()), day(FRI), now_ms)).is_err());
    assert!(block_on(act(&store, d, Act::Move(day(THU)), day(FRI), now_ms)).is_err());

    let log = store.day_log(day(FRI)).unwrap();
    assert_eq!(
        (log.captured, log.done, log.dropped, log.moved),
        (4, 1, 1, 1)
    );
    let tasks = block_on(store.snapshot(day(FRI), 0)).unwrap();
    let title = |id| tasks.iter().find(|t| t.id == id).map(|t| t.title.clone());
    assert_eq!(title(a).as_deref(), Some("Call the dentist"));
    assert_eq!(title(d).as_deref(), Some("Old thing, renamed"));
    assert!(title(c).is_none(), "moved to tomorrow, out of today");
}

/// AC-7 (UI), R-14: keep on a stale task keeps it in today and stops asking.
#[test]
fn keep_stops_asking_about_a_stale_task() {
    let store = store("keep_stale");
    let id = block_on(store.add(task("Renew the passport", day(25), day(25)))).unwrap();
    for _ in 0..3 {
        block_on(store.roll_over(day(FRI))).unwrap();
    }
    let now = local(FRI, 9, 0);
    let clock = FixedClock(now);
    let stale = |store: &SqliteTaskStore| {
        let view = block_on(load_today(store, &clock, &DayRules::default(), now)).unwrap();
        today_page(&view, &[], now, &clock)
            .lines
            .iter()
            .find(|l| l.id == id)
            .unwrap()
            .stale
    };
    assert!(stale(&store));
    block_on(act(&store, id, Act::Keep, day(FRI), ms(now))).unwrap();
    assert!(!stale(&store));
}

#[test]
fn days_from_the_picker_parse() {
    assert_eq!(
        parse_day("2026-11-30"),
        Some(Date::from_calendar_date(2026, Month::November, 30).unwrap())
    );
    assert_eq!(parse_day("2026-02-30"), None);
    assert_eq!(parse_day("monday"), None);
}

/// AC-9, R-18 (the part before the screen): the Morning job, from a store of 10 000 tasks
/// on disk to the card's words, in under 150 ms. Release builds only; run with
/// `cargo test --release -p hyperme-app morning_from_10_000 -- --nocapture` on the Mac and
/// record the numbers in docs/results/.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn morning_from_10_000_tasks_on_disk_is_well_under_150_ms() {
    let store = store("morning_10k");
    for i in 0..10_000u32 {
        // A spread like a long-used list: most planned on other days, some for today.
        let planned = match i % 10 {
            0 => day(FRI),
            1 => day(THU),
            _ => Date::from_calendar_date(2026, Month::December, 1 + (i % 28) as u8).unwrap(),
        };
        block_on(store.add(task(&format!("Task {i}"), planned, day(THU)))).unwrap();
    }
    let mut runs = Vec::new();
    for _ in 0..21 {
        let mut s = setup(store.clone(), local(FRI, 7, 40), calendar_feeds(), false);
        let started = std::time::Instant::now();
        block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
        let card = s.ui.cards.lock().unwrap()[0].0.clone();
        let view = crate::pill::morning_view(&card);
        runs.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(view.tasks.len(), 5);
    }
    runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (median, max) = (runs[runs.len() / 2], runs[runs.len() - 1]);
    println!("Morning job, 10000 tasks on disk, 21 runs: median {median:.2} ms, max {max:.2} ms");
    assert!(max < 150.0, "the morning job took {max:.2} ms");
}

// --- Job failed ---

fn google_with(
    accounts: &[&str],
) -> (
    hyperme_core::registry::Registry,
    hyperme_core::config::Connectors,
) {
    let mut text = hyperme_core::config::DEFAULT_CONFIG.to_owned();
    for label in accounts {
        text.push_str(&format!(
            "\n[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"{label}\"\n"
        ));
    }
    let connectors =
        hyperme_core::config::Config::parse(&text, std::path::Path::new("config.toml"))
            .unwrap()
            .connectors;
    let registry = hyperme_core::connector_settings::registry(None, &connectors).unwrap();
    (registry, connectors)
}

/// A mail server that answers with three synthetic threads, or with a waiting state.
struct FakeMail(Option<hyperme_core::health::ServerState>);

impl hyperme_core::sync::ToolCaller for FakeMail {
    async fn call(
        &self,
        _server: &str,
        _tool: &str,
        _args: serde_json::Value,
    ) -> Result<serde_json::Value, hyperme_core::sync::SyncError> {
        if let Some(state) = self.0 {
            return Err(hyperme_core::sync::SyncError::NeedsUser(state));
        }
        let thread = |id: &str| {
            serde_json::json!({"id": id, "messages": [{"subject": "Synthetic", "sender": "a@example.test",
                "snippet": "s", "internalDate": "1795716000000", "labelIds": ["INBOX"]}]})
        };
        Ok(
            serde_json::json!({"threads": [thread("t1"), thread("t2"), thread("t3")],
            "omittedContent": [{"id": "t4"}]}),
        )
    }
}

/// specs/166 AC-4, AC-13 (app part): a Review or check that gets through stores the threads,
/// counts what was held back and reads "Mail · works, 3 threads"; one the user refused reads
/// "not allowed: mail pulls are skipped"; neither is a job failure (R-16b).
#[test]
fn a_server_pull_reports_its_check_line() {
    use hyperme_core::health::{PullEnd, ServerState};
    use hyperme_core::sync::MailStore;
    let (registry, _) = google_with(&["personal"]);
    let google = registry.get("google").unwrap();
    let gmail = google.server("gmail").unwrap();
    let store = store("server-pull");
    let now = local(27, 9, 0);

    let pulled = block_on(pull_server(
        &FakeMail(None),
        &store,
        google,
        gmail,
        "google:personal",
        now,
    ));
    assert_eq!(pulled.end, PullEnd::Done { held_back: 1 });
    assert!(pulled.failures.is_empty());
    assert_eq!(
        check_line(google, gmail, &pulled),
        "Mail · works, 3 threads"
    );
    assert_eq!(store.all(Some("google:personal")).unwrap().len(), 3);

    let refused = FakeMail(Some(ServerState::NotAllowed));
    let pulled = block_on(pull_server(
        &refused,
        &store,
        google,
        gmail,
        "google:personal",
        now,
    ));
    assert_eq!(pulled.end, PullEnd::NeedsUser(ServerState::NotAllowed));
    assert!(pulled.failures.is_empty(), "never a job failure");
    assert_eq!(
        check_line(google, gmail, &pulled),
        "Mail · not allowed: mail pulls are skipped"
    );
    let waiting = FakeMail(Some(ServerState::NeedsOk));
    let pulled = block_on(pull_server(
        &waiting,
        &store,
        google,
        gmail,
        "google:personal",
        now,
    ));
    assert_eq!(
        check_line(google, gmail, &pulled),
        "Mail · not approved yet: mail pulls wait for your OK"
    );
}

/// specs/166 R-14: the scheduler leaves a server alone while the user checks it.
#[test]
fn a_server_being_checked_is_left_to_the_user() {
    assert!(!Checking::running("google:personal", "gmail"));
    let checking = Checking::start("google:personal", "gmail");
    assert!(Checking::running("google:personal", "gmail"));
    assert!(!Checking::running("google:personal", "calendar"));
    drop(checking);
    assert!(!Checking::running("google:personal", "gmail"));
}

/// specs/166 AC-3, R-8: the "Needs your OK" alert names the service and account, and
/// Review opens the account's page.
#[test]
fn the_needs_ok_alert_names_the_service_and_links_the_account() {
    let (registry, _) = google_with(&["personal"]);
    let google = registry.get("google").unwrap();
    let gmail = google.server("gmail").unwrap();
    let alert = needs_ok_alert(google, gmail, "personal", "google:personal");
    assert_eq!(alert.title, "Needs your OK");
    assert_eq!(
        alert.detail,
        "Google mail wants your OK before HyperMe reads it for personal"
    );
    assert_eq!(alert.action, "Review");
    assert_eq!(
        alert.link.as_deref(),
        Some("hyperme://settings/connectors/google:personal")
    );
}

/// AC-16, R-36: a sync job that failed twice says what it could not reach, and Fix opens
/// the account's page; with two accounts, the connector's.
#[test]
fn a_failed_sync_names_what_it_could_not_reach_and_links_the_fix() {
    let (registry, connectors) = google_with(&["personal"]);
    let alert = failed_alert("sync:google-calendar", &registry, &connectors);
    assert_eq!(alert.title, "Job failed");
    assert_eq!(
        alert.detail,
        "Couldn't reach your Google calendar twice today"
    );
    assert_eq!(alert.action, "Fix");
    assert_eq!(
        alert.link.as_deref(),
        Some("hyperme://settings/connectors/google:personal")
    );

    let (registry, connectors) = google_with(&["personal", "work"]);
    let alert = failed_alert("sync:google-gmail", &registry, &connectors);
    assert_eq!(alert.detail, "Couldn't reach your Google mail twice today");
    assert_eq!(
        alert.link.as_deref(),
        Some("hyperme://settings/connectors/google")
    );
}

/// R-36: a job that is not a sync (roll-over, the morning card) still alerts, with Fix
/// opening Settings.
#[test]
fn another_failed_job_alerts_without_a_link() {
    let (registry, connectors) = google_with(&[]);
    let alert = failed_alert("rollover", &registry, &connectors);
    assert_eq!(alert.detail, "rollover failed twice today");
    assert_eq!(alert.link, None);
}

// --- Nudges ---

fn due_task(title: &str, h: u8, m: u8) -> NewTask {
    NewTask {
        due: Some(hyperme_core::tasks::Due {
            date: day(FRI),
            time: Some(Time::from_hms(h, m, 0).unwrap()),
        }),
        ..task(title, day(FRI), day(FRI))
    }
}

/// AC-10, R-22: a meeting at 10:30 is nudged at 10:28 and a task due at 16:00 at 15:50;
/// moments already past, tasks with no due time and finished tasks get none.
#[test]
fn nudges_come_two_minutes_before_meetings_and_ten_before_due_times() {
    let store = store("nudges_plan");
    let due = block_on(store.add(due_task("Send the numbers", 16, 0))).unwrap();
    block_on(store.add(task("No time", day(FRI), day(FRI)))).unwrap();
    let done = block_on(store.add(due_task("Already done", 17, 0))).unwrap();
    block_on(store.finish(done, Finish::Done, 0)).unwrap();
    let now = local(FRI, 9, 0);
    let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
    let meetings = [
        meeting("Standup", local(FRI, 8, 30), 15),
        meeting("Weekly planning", local(FRI, 10, 30), 30),
    ];
    let nudges = nudges_for(&meetings, &snapshot, now, &FixedClock(now));
    let got: Vec<(String, UtcDateTime, UtcDateTime)> =
        nudges.into_iter().map(|n| (n.id, n.at, n.until)).collect();
    assert_eq!(
        got,
        [
            (
                format!("meeting:{}", ms(local(FRI, 10, 30))),
                local(FRI, 10, 28),
                local(FRI, 10, 30)
            ),
            (format!("task:{due}"), local(FRI, 15, 50), local(FRI, 16, 0)),
        ]
    );
}

/// AC-10, R-22: the nudge card says what and when, with Join for a meeting's web link and
/// Open Today for a task; a task finished meanwhile shows nothing.
#[test]
fn a_nudge_card_says_what_and_when() {
    let store = store("nudge_card");
    let id = block_on(store.add(due_task("Send the numbers", 16, 0))).unwrap();
    let now = local(FRI, 10, 28);
    let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
    let meetings = [meeting("Weekly planning", local(FRI, 10, 30), 30)];
    let zone = FixedClock(now);
    let card = nudge_card(
        &format!("meeting:{}", ms(local(FRI, 10, 30))),
        &meetings,
        &snapshot,
        now,
        &zone,
    )
    .unwrap();
    assert_eq!(card.title, "Weekly planning in 2 min");
    assert_eq!(card.detail, "10:30–11:00");
    assert_eq!(
        (card.action.as_str(), card.link.as_deref()),
        ("Join", Some("https://meet.example/x"))
    );

    let at = local(FRI, 15, 50);
    let card = nudge_card(&format!("task:{id}"), &[], &snapshot, at, &FixedClock(at)).unwrap();
    assert_eq!(card.title, "Send the numbers");
    assert_eq!(card.detail, "Due at 16:00 · in 10 min");
    assert_eq!(card.link.as_deref(), Some(TODAY_LINK));

    block_on(store.finish(id, Finish::Done, 0)).unwrap();
    let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
    assert_eq!(
        nudge_card(&format!("task:{id}"), &[], &snapshot, at, &FixedClock(at)),
        None
    );
}

/// AC-10, R-22: the Nudge job shows the card through the UI, with no sound or refresh.
#[test]
fn the_nudge_job_shows_its_card() {
    let store = store("nudge_job");
    let id = block_on(store.add(due_task("Send the numbers", 16, 0))).unwrap();
    let mut s = setup(store, local(FRI, 15, 50), Feeds::default(), false);
    block_on(s.runner.run(&job(&format!("task:{id}"), JobKind::Nudge))).unwrap();
    assert_eq!(s.ui.nudges.lock().unwrap()[0].title, "Send the numbers");
    assert_eq!(*s.ui.refreshes.lock().unwrap(), 0);
    assert!(s.ui.cards.lock().unwrap().is_empty());
}
