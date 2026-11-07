//! Where Orion keeps its files: the per-user configuration directory (catalog
//! overrides, persistent state) and the per-repository `.orion` directory
//! (task, handoff, per-attempt logs).

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// All paths a run touches.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Per-user configuration directory.
    pub config_dir: PathBuf,
    /// Directory scanned for `*.toml` overrides.
    pub profiles_dir: PathBuf,
    /// Persisted pauses.
    pub state_file: PathBuf,
    /// Relay preferences written by `orion init`.
    pub config_file: PathBuf,
    /// Last detection run, so `status` and `init` stay instant.
    pub cache_file: PathBuf,
    /// `<repo>/.orion`.
    pub orion_dir: PathBuf,
    /// The task handed to every harness.
    pub task_file: PathBuf,
    /// The relay document written before switching harnesses.
    pub handoff_file: PathBuf,
    /// Summary an agent is asked to write when it finishes.
    pub result_file: PathBuf,
    /// One directory per run.
    pub runs_dir: PathBuf,
}

impl Paths {
    /// Build every path from a repository root and a configuration directory.
    pub fn under(root: &Path, config_dir: PathBuf) -> Self {
        let orion_dir = root.join(".orion");
        Self {
            profiles_dir: config_dir.join("profiles.d"),
            state_file: config_dir.join("state.json"),
            config_file: config_dir.join("config.json"),
            cache_file: config_dir.join("detect-cache.json"),
            task_file: orion_dir.join("TASK.md"),
            handoff_file: orion_dir.join("HANDOFF.md"),
            result_file: orion_dir.join("RESULT.md"),
            runs_dir: orion_dir.join("runs"),
            config_dir,
            orion_dir,
        }
    }

    /// Resolve against the real configuration directory of this user.
    pub fn resolve(root: &Path) -> Result<Self> {
        Ok(Self::under(root, config_dir()?))
    }
}

/// The per-user configuration directory, or `ORION_CONFIG_DIR` when set.
///
/// The environment variable is what tests and portable installs use; it is not a
/// user-facing setting.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("ORION_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "next-harness")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or_else(|| Error::msg("cannot determine a configuration directory for this platform"))
}

/// The current user's home directory, used to scrub it out of reports.
pub fn home_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}
