//! Preferences written by `orion init` and shown by `orion config`.
//!
//! They are deliberately tiny and separate from the catalog: the catalog
//! describes harnesses, this file describes how this user wants the relay to
//! behave. A missing or corrupt file means "defaults", never a failed start.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{self, Result};
use crate::orchestrator::DEFAULT_MAX_RELAYS;

fn default_version() -> u32 {
    1
}

/// Relay behaviour for this user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    /// Schema version of this file.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Maximum number of harnesses one run may try.
    #[serde(default = "default_relays")]
    pub max_relays: usize,
    /// Whether human-driven harnesses may be prepared.
    #[serde(default)]
    pub manual: bool,
}

fn default_relays() -> usize {
    DEFAULT_MAX_RELAYS
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: default_version(),
            max_relays: default_relays(),
            manual: false,
        }
    }
}

impl Preferences {
    /// Read the preferences, falling back to defaults on any problem.
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write the preferences, creating the configuration directory.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| error::io(parent, e))?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|e| error::Error::msg(e.to_string()))?;
        let mut file = fs::File::create(path).map_err(|e| error::io(path, e))?;
        file.write_all(&body).map_err(|e| error::io(path, e))?;
        file.sync_all().map_err(|e| error::io(path, e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_means_defaults() {
        let prefs = Preferences::load(Path::new("does/not/exist.json"));
        assert_eq!(prefs, Preferences::default());
        assert_eq!(prefs.max_relays, DEFAULT_MAX_RELAYS);
        assert!(!prefs.manual);
    }

    #[test]
    fn a_partial_file_keeps_the_defaults_for_missing_fields() {
        let prefs: Preferences = serde_json::from_str(r#"{ "manual": true }"#).expect("parse");
        assert_eq!(prefs.max_relays, DEFAULT_MAX_RELAYS);
        assert!(prefs.manual);
    }
}
