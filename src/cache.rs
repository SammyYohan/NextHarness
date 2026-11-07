//! A short-lived cache of the last detection, so `status`, `init` and `run` do
//! not re-probe every binary each time.
//!
//! It expires on its own and `orion doctor --refresh` ignores it entirely: a
//! cache that outlives an installation would hide the harness the user just
//! installed.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::detector::Detection;
use crate::error::{self, Result};
use crate::timefmt::now_epoch;

/// How long a detection result stays usable.
pub const MAX_AGE: u64 = 600;

#[derive(Debug, Serialize, Deserialize)]
struct Cache {
    version: u32,
    detected_at: u64,
    entries: Vec<Detection>,
}

fn write(path: &Path, entries: &[Detection]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| error::io(parent, e))?;
    }
    let cache = Cache {
        version: 1,
        detected_at: now_epoch(),
        entries: entries.to_vec(),
    };
    let body = serde_json::to_vec(&cache).map_err(|e| error::Error::msg(e.to_string()))?;
    std::fs::write(path, body).map_err(|e| error::io(path, e))
}

/// Store the result, ignoring failures: a cache is an optimisation, never a
/// requirement.
pub fn store(path: &Path, entries: &[Detection]) {
    let _ = write(path, entries);
}

/// Read the cached detection, when it exists, parses, and is fresh.
pub fn load(path: &Path, expected: usize) -> Option<Vec<Detection>> {
    let text = std::fs::read_to_string(path).ok()?;
    let cache: Cache = serde_json::from_str(&text).ok()?;
    if cache.version != 1 || cache.entries.len() != expected {
        return None;
    }
    if now_epoch().saturating_sub(cache.detected_at) > MAX_AGE {
        return None;
    }
    Some(cache.entries)
}
