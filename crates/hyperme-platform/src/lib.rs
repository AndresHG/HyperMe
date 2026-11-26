//! HyperMe's platform code: the real implementations of `hyperme_core::traits`. Code that only
//! works on one OS sits in a `cfg(target_os)` module.

pub mod history;
pub mod hotkey;
#[cfg(target_os = "macos")]
pub mod keychain;
pub mod microphone;
pub mod paste;
#[cfg(target_os = "macos")]
pub mod permissions;
#[cfg(target_os = "macos")]
pub mod presence;
#[cfg(target_os = "macos")]
pub mod sounds;
pub mod task_store;
pub mod timing;
