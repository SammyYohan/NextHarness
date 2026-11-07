//! Persistent relay state: which harness is paused, until when, and why.
//!
//! The file is written atomically (temporary file in the same directory,
//! `fsync`, rename) so a crash during a save cannot leave a half-written state
//! that silently drops a pause and re-spends a quota.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{self, Result};
use crate::timefmt::now_epoch;

fn default_version() -> u32 {
    1
}

/// One pause: a harness NextHarness must not start before `until`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pause {
    /// Epoch seconds at which the harness may be tried again.
    pub until: u64,
    /// What was detected: `quota`, `auth`, `blocked` or `unknown`.
    pub kind: String,
    /// The matched output, already masked by the caller.
    #[serde(default)]
    pub detail: Option<String>,
    /// When the pause was recorded.
    #[serde(default)]
    pub set_at: u64,
}

/// Everything NextHarness remembers between runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// Schema version of the state file.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Pauses keyed by harness id.
    #[serde(default)]
    pub pauses: BTreeMap<String, Pause>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: default_version(),
            pauses: BTreeMap::new(),
        }
    }
}

impl State {
    /// Read the state file, treating a missing or corrupt file as "nothing
    /// remembered": a lost pause costs one wasted attempt, not a broken start.
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write the state file atomically.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| error::io(parent, e))?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|e| error::Error::msg(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(|e| error::io(&tmp, e))?;
            file.write_all(&body).map_err(|e| error::io(&tmp, e))?;
            file.sync_all().map_err(|e| error::io(&tmp, e))?;
        }
        fs::rename(&tmp, path).map_err(|e| error::io(path, e))?;
        Ok(())
    }

    /// Record a pause for `id`.
    pub fn pause(&mut self, id: &str, until: u64, kind: &str, detail: Option<&str>) {
        self.pauses.insert(
            id.to_string(),
            Pause {
                until,
                kind: kind.to_string(),
                detail: detail.map(str::to_string),
                set_at: now_epoch(),
            },
        );
    }

    /// The pause for `id`, if it is still active at `now`.
    pub fn pause_until(&self, id: &str, now: u64) -> Option<&Pause> {
        self.pauses.get(id).filter(|pause| pause.until > now)
    }

    /// Whether `id` must be skipped at `now`.
    pub fn is_paused(&self, id: &str, now: u64) -> bool {
        self.pause_until(id, now).is_some()
    }

    /// Drop one pause, or every pause when `id` is `None`.
    pub fn clear(&mut self, id: Option<&str>) {
        match id {
            Some(id) => {
                self.pauses.remove(id);
            }
            None => self.pauses.clear(),
        }
    }

    /// Forget pauses that have expired.
    pub fn prune(&mut self, now: u64) {
        self.pauses.retain(|_, pause| pause.until > now);
    }

    /// The earliest moment at which any paused harness becomes usable again.
    pub fn next_reset(&self, now: u64) -> Option<u64> {
        self.pauses
            .values()
            .filter(|pause| pause.until > now)
            .map(|pause| pause.until)
            .min()
    }
}
