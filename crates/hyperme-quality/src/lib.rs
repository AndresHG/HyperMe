//! HyperMe's quality harness (issue 99): replays saved dictations through the app's own state
//! machine and providers, and scores the text against the owner's corrected texts. The measure
//! for the phase 2 gate and for every prompt change.

pub mod replay;
pub mod report;
pub mod score;
pub mod set;
