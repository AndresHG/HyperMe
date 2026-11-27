//! HyperMe's core: config, the dictation state machine and the traits that keep platform and
//! network code out of it. Nothing here calls the OS or the network.

pub mod actions;
pub mod approval;
pub mod audio;
pub mod brief;
pub mod config;
pub mod connector_settings;
pub mod connector_setup;
pub mod connectors;
pub mod cost;
pub mod diagnostics;
pub mod dictation;
pub mod dictionary;
pub mod fallback;
pub mod field_watch;
pub mod health;
pub mod hints;
pub mod learning;
pub mod registry;
pub mod report;
pub mod schedule;
pub mod scheduler;
pub mod secrets;
pub mod sync;
pub mod tasks;
pub mod trace;
pub mod traits;
pub mod warnings;
