//! NextHarness: one coding task, relayed across the AI coding harnesses
//! installed on this machine.
//!
//! The module map follows the relay itself: [`config`] reads the catalog,
//! [`detector`] finds what is installed, [`runner`] starts one harness,
//! [`quota`] decides whether it ran out, [`orchestrator`] moves to the next one.

rust_i18n::i18n!("locales", fallback = "en");

pub mod cache;
pub mod cli;
pub mod config;
pub mod detector;
pub mod doctor;
pub mod error;
pub mod gatekeeper;
pub mod git;
pub mod handoff;
pub mod i18n;
pub mod invoke;
pub mod orchestrator;
pub mod paths;
pub mod prefs;
pub mod quota;
pub mod runner;
pub mod state;
pub mod timefmt;
pub mod util;

pub use error::{Error, Result};

/// Version string used by `--version` and the doctor report.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
