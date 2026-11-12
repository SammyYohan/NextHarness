//! NextHarness: one coding task, handed to the next AI coding harness whenever
//! the current one runs out of free quota.
//!
//! The module map follows one task through its life: [`config`] reads the
//! catalog, [`detector`] finds what is installed, [`runner`] starts one harness,
//! [`quota`] decides whether it ran out, [`orchestrator`] hands the task over.

rust_i18n::i18n!("locales", fallback = "en");

pub mod cache;
pub mod cli;
pub mod config;
pub mod detector;
pub mod diagnostic;
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
