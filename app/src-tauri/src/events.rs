//! The events Rust sends to the pages, and the one a page sends back, typed (spec 257, R-8). Each
//! is a small wrapper around its payload, registered in `bindings.rs`, so `bindings.ts` gives the
//! pages the same names and payload types. The names are the ones the pages already listened to.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

/// A state of the pill.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "pill")]
pub struct Pill(pub crate::pill::View);

/// The pill has shrunk away.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "pill-hidden")]
pub struct PillHidden;

/// The morning card or an alert, shown in the pill's place.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "card")]
pub struct Card(pub crate::pill::Card);

/// The microphone level while recording, 0 to 1.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "level")]
pub struct Level(pub f32);

/// The walkthrough's microphone level, and whether a voice was heard.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "onboarding-level")]
pub struct OnboardingLevel(pub f32, pub bool);

/// A dictation was pasted, in this many milliseconds from key release.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "dictation-pasted")]
pub struct DictationPasted(pub u64);

/// A key or provider changed, for the walkthrough.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "providers-changed")]
pub struct ProvidersChanged;

/// Home's attention dot, on or off (R-22).
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "home://attention")]
pub struct HomeAttention(pub bool);

/// Home reloads a block or a section (R-22).
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "home://refresh")]
pub struct HomeRefresh(pub String);

/// The history changed: a dictation was added, retried or deleted.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "home://changed")]
pub struct HomeChanged;

/// A retry from Home moved on (R-16).
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "home://retry")]
pub struct HomeRetry(pub crate::home::RetryEvent);

/// A Home block (or `today`) was on screen, sent by the page (R-21, R-22).
#[derive(Clone, Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "home://block-viewed")]
pub struct HomeBlockViewed(pub String);

/// A connector server asks something (specs/166).
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "connector-approval")]
pub struct ConnectorApproval(pub crate::approvals::CardView);

/// The question with this id is over.
#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "connector-approval-closed")]
pub struct ConnectorApprovalClosed(pub u64);
