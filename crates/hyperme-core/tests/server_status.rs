//! Slice 2 of specs/166-gmail-approvals: the server states between pulls, their precedence, the
//! once-a-day alert and pull, and the held-back count. Tests cite `specs/166-gmail-approvals`.

use hyperme_core::health::{
    MemoryServerStatus, PullEnd, ServerState, ServerStatusStore, after_pull, after_review,
    pull_runs,
};
use hyperme_core::sync::held_back;
use serde_json::json;
use time::{Date, Month};

const ACCOUNT: &str = "google:personal";
const SERVER: &str = "gmail";

fn day(n: u8) -> Date {
    Date::from_calendar_date(2026, Month::November, n).unwrap()
}

fn state(store: &MemoryServerStatus) -> Option<ServerState> {
    store.status(ACCOUNT, SERVER).unwrap().state
}

/// AC-3: "approval needed" marks the server "Needs your OK" with one alert; a second failing
/// pull the same day raises none, and the next pulls that day do not run; the next day one runs
/// and may alert again (R-8).
#[test]
fn needs_ok_alerts_once_a_day_and_pulls_once_a_day() {
    let store = MemoryServerStatus::default();
    let needs = PullEnd::NeedsUser(ServerState::NeedsOk);
    assert!(pull_runs(&store.status(ACCOUNT, SERVER).unwrap(), day(27)));
    assert!(after_pull(&store, ACCOUNT, SERVER, needs, day(27)).unwrap());
    assert_eq!(state(&store), Some(ServerState::NeedsOk));
    assert!(!after_pull(&store, ACCOUNT, SERVER, needs, day(27)).unwrap());
    assert!(!pull_runs(&store.status(ACCOUNT, SERVER).unwrap(), day(27)));
    assert!(pull_runs(&store.status(ACCOUNT, SERVER).unwrap(), day(28)));
    assert!(after_pull(&store, ACCOUNT, SERVER, needs, day(28)).unwrap());
}

/// AC-4 (state part): a pull that works clears the state (R-16a).
#[test]
fn a_pull_that_works_clears_the_state() {
    let store = MemoryServerStatus::default();
    after_pull(
        &store,
        ACCOUNT,
        SERVER,
        PullEnd::NeedsUser(ServerState::NeedsOk),
        day(27),
    )
    .unwrap();
    after_pull(
        &store,
        ACCOUNT,
        SERVER,
        PullEnd::Done { held_back: 0 },
        day(27),
    )
    .unwrap();
    assert_eq!(state(&store), None);
    assert!(pull_runs(&store.status(ACCOUNT, SERVER).unwrap(), day(27)));
}

/// AC-5, AC-9 (state part): "Not allowed" and "Blocked by your admin" skip every unattended
/// pull and raise no alert (R-15, R-16).
#[test]
fn not_allowed_and_blocked_skip_pulls_without_alerts() {
    for blocked in [ServerState::NotAllowed, ServerState::Blocked] {
        let store = MemoryServerStatus::default();
        assert!(
            !after_pull(
                &store,
                ACCOUNT,
                SERVER,
                PullEnd::NeedsUser(blocked),
                day(27)
            )
            .unwrap()
        );
        assert_eq!(state(&store), Some(blocked));
        for n in [27, 28, 29] {
            assert!(!pull_runs(&store.status(ACCOUNT, SERVER).unwrap(), day(n)));
        }
    }
}

/// R-16a: a higher state replaces a lower one, never the reverse; other failures leave it.
#[test]
fn higher_states_win() {
    let store = MemoryServerStatus::default();
    after_pull(
        &store,
        ACCOUNT,
        SERVER,
        PullEnd::NeedsUser(ServerState::NotAllowed),
        day(27),
    )
    .unwrap();
    assert!(
        !after_pull(
            &store,
            ACCOUNT,
            SERVER,
            PullEnd::NeedsUser(ServerState::NeedsOk),
            day(28)
        )
        .unwrap()
    );
    assert_eq!(state(&store), Some(ServerState::NotAllowed));
    after_pull(
        &store,
        ACCOUNT,
        SERVER,
        PullEnd::NeedsUser(ServerState::Blocked),
        day(28),
    )
    .unwrap();
    assert_eq!(state(&store), Some(ServerState::Blocked));
    after_pull(&store, ACCOUNT, SERVER, PullEnd::Failed, day(28)).unwrap();
    assert_eq!(state(&store), Some(ServerState::Blocked));
}

/// R-16a: a new sign-in or a disconnect forgets every server of that account only.
#[test]
fn clearing_an_account_forgets_its_servers_only() {
    let store = MemoryServerStatus::default();
    let needs = PullEnd::NeedsUser(ServerState::NeedsOk);
    after_pull(&store, ACCOUNT, SERVER, needs, day(27)).unwrap();
    after_pull(&store, ACCOUNT, "calendar", needs, day(27)).unwrap();
    after_pull(&store, "google:work", SERVER, needs, day(27)).unwrap();
    store.clear_account(ACCOUNT).unwrap();
    assert_eq!(state(&store), None);
    assert_eq!(store.status(ACCOUNT, "calendar").unwrap().state, None);
    assert_eq!(
        store.status("google:work", SERVER).unwrap().state,
        Some(ServerState::NeedsOk)
    );
}

/// AC-14: the pull's held-back count is kept for the account page (R-17).
#[test]
fn held_back_items_are_counted_and_kept() {
    let twelve: Vec<_> = (0..12).map(|i| json!({"id": format!("t{i}")})).collect();
    assert_eq!(
        held_back(&json!({"threads": [], "omittedContent": twelve})),
        12
    );
    assert_eq!(
        held_back(&json!({"omittedContent": {"omittedCount": 12, "reason": "x"}})),
        12
    );
    assert_eq!(
        held_back(&json!({"omittedContent": {"items": [1, 2, 3]}})),
        3
    );
    assert_eq!(held_back(&json!({"threads": []})), 0);
    assert_eq!(held_back(&json!({"omittedContent": []})), 0);
    assert_eq!(held_back(&json!({"omittedContent": 12})), 12);

    let store = MemoryServerStatus::default();
    after_pull(
        &store,
        ACCOUNT,
        SERVER,
        PullEnd::Done { held_back: 12 },
        day(27),
    )
    .unwrap();
    assert_eq!(store.status(ACCOUNT, SERVER).unwrap().held_back, 12);
}

/// R-16a: several pulls of one server combine: the strongest waiting state wins, waiting wins
/// over a failure, a failure over success, and held-back counts add up.
#[test]
fn pull_ends_combine() {
    let done = |n| PullEnd::Done { held_back: n };
    let needs = PullEnd::NeedsUser(ServerState::NeedsOk);
    let blocked = PullEnd::NeedsUser(ServerState::Blocked);
    assert_eq!(done(2).and(done(3)), done(5));
    assert_eq!(done(2).and(PullEnd::Failed), PullEnd::Failed);
    assert_eq!(needs.and(PullEnd::Failed), needs);
    assert_eq!(PullEnd::Failed.and(needs), needs);
    assert_eq!(blocked.and(needs), blocked);
    assert_eq!(needs.and(blocked), blocked);
    assert_eq!(done(1).and(needs), needs);
}

/// R-8: Later (Esc) in Review or the setup check records "Needs your OK" without using up the
/// day's background try or alert; a Review that works clears the state.
#[test]
fn a_review_left_unanswered_keeps_the_days_alert() {
    let store = MemoryServerStatus::default();
    let needs = PullEnd::NeedsUser(ServerState::NeedsOk);
    after_review(&store, ACCOUNT, SERVER, needs, day(27)).unwrap();
    let status = store.status(ACCOUNT, SERVER).unwrap();
    assert_eq!(status.state, Some(ServerState::NeedsOk));
    assert!(pull_runs(&status, day(27)));
    assert!(
        after_pull(&store, ACCOUNT, SERVER, needs, day(27)).unwrap(),
        "alert still due"
    );
    after_review(
        &store,
        ACCOUNT,
        SERVER,
        PullEnd::Done { held_back: 0 },
        day(27),
    )
    .unwrap();
    assert_eq!(state(&store), None);
}
