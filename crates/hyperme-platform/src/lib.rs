//! HyperMe's platform code: the real implementations of `hyperme_core::traits`. Code that only
//! works on one OS sits in a `cfg(target_os)` module.

pub mod hotkey;
pub mod microphone;
pub mod paste;
