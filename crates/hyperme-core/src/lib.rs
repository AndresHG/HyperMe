//! HyperMe's core: config, the dictation state machine and the traits that keep platform and
//! network code out of it. Nothing here calls the OS or the network.

pub mod audio;
pub mod brief;
pub mod config;
pub mod connectors;
pub mod cost;
pub mod dictation;
pub mod registry;
pub mod schedule;
pub mod scheduler;
pub mod secrets;
pub mod tasks;
pub mod traits;
