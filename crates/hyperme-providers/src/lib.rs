//! HyperMe's network providers, behind the traits in `hyperme_core::traits`. Endpoints and
//! model names come from the config (R-26); keys are passed in by the caller, read from the
//! keychain, and never appear in an error message (R-25).

pub mod actions;
pub mod cleanup;
pub mod mcp;
pub mod oauth;
pub mod setup;
pub mod speechmatics;
