//! HyperMe's platform code: the real implementations of `hyperme_core::traits`. Code that only
//! works on one OS sits in a `cfg(target_os)` module.

pub mod crash;
pub mod dictionary_store;
#[cfg(target_os = "macos")]
pub mod field_watcher;
pub mod history;
pub mod hotkey;
#[cfg(target_os = "macos")]
pub mod keychain;
pub mod log_file;
pub mod microphone;
#[cfg(target_os = "macos")]
pub mod output_mute;
pub mod paste;
#[cfg(target_os = "macos")]
pub mod permissions;
#[cfg(target_os = "macos")]
pub mod presence;
#[cfg(target_os = "macos")]
pub mod sounds;
pub mod task_store;
pub mod timing;
pub mod trace_store;
