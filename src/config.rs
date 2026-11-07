//! The catalog: the built-in `profiles/builtin.toml`, merged with every
//! `<config_dir>/profiles.d/*.toml` override, then validated.
//!
//! Overrides are patches: a file only has to restate the fields it changes, and
//! an unknown `id` adds a new harness. Merging happens on the TOML tree before
//! deserialisation, so a partial override never has to repeat unrelated fields.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::error::{self, Error, Result};

/// The compiled-in catalog. It is the source of truth when nothing overrides it.
pub const BUILTIN: &str = include_str!("../profiles/builtin.toml");

fn default_true() -> bool {
    true
}

/// How Orion is allowed to use a harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Orion starts it and reads its output.
    Headless,
    /// A terminal agent a human drives; Orion only prepares the handoff.
    Interactive,
    /// A desktop application; detected by name, never driven.
    Gui,
}

/// Price tier. `Unknown` is treated as paid everywhere: never spend by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Expected to have a usable free allowance.
    Free,
    /// Costs money or needs a paid subscription.
    Paid,
    /// Nobody knows; treated as paid.
    #[default]
    Unknown,
}

impl Tier {
    /// Small label used by `doctor`, `init` and the run log.
    pub fn label(self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Paid => "paid",
            Tier::Unknown => "unknown",
        }
    }

    /// True when Orion must ask before starting this harness.
    pub fn requires_consent(self) -> bool {
        !matches!(self, Tier::Free)
    }
}

/// How much the catalog trusts an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Verified {
    /// Documented by the vendor.
    Official,
    /// Documented by a third party that wraps the harness.
    ThirdParty,
    /// Must be tested on a real machine before it is trusted.
    #[default]
    Unverified,
}

impl Verified {
    /// Stable label for the doctor report.
    pub fn label(self) -> &'static str {
        match self {
            Verified::Official => "official",
            Verified::ThirdParty => "third_party",
            Verified::Unverified => "unverified",
        }
    }
}

/// Detection patterns, generic or per harness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Patterns {
    /// "Out of quota" signals.
    #[serde(default)]
    pub quota: Vec<String>,
    /// "Sign in again" signals.
    #[serde(default)]
    pub auth: Vec<String>,
    /// "Resets in 3 hours" signals.
    #[serde(default)]
    pub reset: Vec<String>,
}

/// Extra directories to search for binaries, per platform family.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchDirs {
    /// Unix paths, `~` expanded at runtime.
    #[serde(default)]
    pub unix: Vec<String>,
    /// Windows paths, `%VAR%` expanded at runtime.
    #[serde(default)]
    pub windows: Vec<String>,
}

/// One catalog entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Harness {
    /// Stable identifier, used by overrides, state and the CLI.
    pub id: String,
    /// Human name.
    pub name: String,
    /// How Orion may use it.
    pub mode: Mode,
    /// Executable names looked up on `PATH`.
    #[serde(default)]
    pub binaries: Vec<String>,
    /// Extra executables that must also exist.
    #[serde(default)]
    pub companion_binaries: Vec<String>,
    /// Arguments that print a version, without consuming quota.
    #[serde(default)]
    pub version_args: Vec<String>,
    /// Template started for a task; must contain `{prompt}`.
    #[serde(default)]
    pub run: Vec<String>,
    /// Template used by `orion resume`.
    #[serde(default)]
    pub run_resume: Vec<String>,
    /// Arguments that disable permission prompts.
    #[serde(default)]
    pub unattended_args: Vec<String>,
    /// The harness cannot run headless without those arguments.
    #[serde(default)]
    pub unattended_required: bool,
    /// Arguments that enable ACP, kept verbatim for a future version.
    #[serde(default)]
    pub acp_args: Vec<String>,
    /// Arguments that request JSON output, kept verbatim for a future version.
    #[serde(default)]
    pub json_args: Vec<String>,
    /// The harness only produces output on a real pseudo-terminal.
    #[serde(default)]
    pub needs_pty: bool,
    /// Platforms this harness runs on (`linux`, `macos`, `windows`, `wsl`).
    #[serde(default)]
    pub platforms: Vec<String>,
    /// Environment variables that must be present, any one of them.
    #[serde(default)]
    pub requires_env: Vec<String>,
    /// Files that may hold credentials.
    #[serde(default)]
    pub auth_files: Vec<String>,
    /// Price tier.
    #[serde(default)]
    pub default_tier: Tier,
    /// Whether `orion run` may consider it without being asked.
    #[serde(default = "default_true")]
    pub enabled_by_default: bool,
    /// Trust level of the entry.
    #[serde(default)]
    pub verified: Verified,
    /// Where the entry comes from.
    #[serde(default)]
    pub source: Option<String>,
    /// How to install the binary.
    #[serde(default)]
    pub install_hint: Option<String>,
    /// Free-form notes shown by `doctor`.
    #[serde(default)]
    pub notes: Option<String>,
    /// Case-insensitive keywords used to find a desktop application.
    #[serde(default)]
    pub gui_keywords: Vec<String>,
    /// Sentence a human copies into an interactive harness.
    #[serde(default)]
    pub manual_prompt: Option<String>,
    /// A usage-limit message can arrive with exit code 0.
    #[serde(default)]
    pub quota_ignores_exit_code: bool,
    /// Seconds a harness may take before printing anything.
    #[serde(default)]
    pub startup_grace_s: Option<u64>,
    /// Harness-specific patterns, added to the generic ones.
    #[serde(default)]
    pub detection: Option<Patterns>,
}

impl Harness {
    /// True when Orion starts this harness itself.
    pub fn is_headless(&self) -> bool {
        self.mode == Mode::Headless
    }

    /// Whether this harness runs on the given platform family.
    pub fn supports_platform(&self, platform: &str) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|entry| entry == platform)
    }
}

/// The catalog document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    /// Schema version understood by this build.
    pub schema: u32,
    /// Date the entries were compiled.
    #[serde(default)]
    pub catalog_date: Option<String>,
    /// Patterns shared by every harness.
    #[serde(default)]
    pub detection_generic: Patterns,
    /// Extra binary search directories.
    #[serde(default)]
    pub detect_search_dirs: SearchDirs,
    /// Every entry, in relay order.
    #[serde(default, rename = "harness")]
    pub harnesses: Vec<Harness>,
}

/// A loaded and validated catalog.
#[derive(Debug, Clone)]
pub struct Config {
    /// The merged document.
    pub catalog: Catalog,
    /// Where it came from: `<builtin>` or a path.
    pub source: String,
    /// Override files that were applied, in order.
    pub override_files: Vec<PathBuf>,
    /// Non-fatal problems worth showing in `doctor`.
    pub warnings: Vec<String>,
}

impl Config {
    /// Load the compiled-in catalog with no overrides.
    pub fn builtin() -> Result<Self> {
        Self::parse(BUILTIN, None, "<builtin>")
    }

    /// Load the catalog for this user: built-in (or `ORION_CATALOG`), then the
    /// overrides in `<config_dir>/profiles.d`.
    pub fn load(config_dir: &Path) -> Result<Self> {
        let override_dir = config_dir.join("profiles.d");
        match std::env::var_os("ORION_CATALOG") {
            Some(path) => {
                let path = PathBuf::from(path);
                let text = std::fs::read_to_string(&path).map_err(|e| error::io(&path, e))?;
                Self::parse(&text, Some(&override_dir), &path.display().to_string())
            }
            None => Self::parse(BUILTIN, Some(&override_dir), "<builtin>"),
        }
    }

    /// Parse a catalog body, apply every `*.toml` in `override_dir`, validate.
    pub fn parse(text: &str, override_dir: Option<&Path>, source: &str) -> Result<Self> {
        let mut base: Value = toml::from_str(text).map_err(|e| Error::Catalog(e.to_string()))?;

        let mut override_files = Vec::new();
        if let Some(dir) = override_dir {
            if dir.is_dir() {
                let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
                    .map_err(|e| error::io(dir, e))?
                    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
                    })
                    .collect();
                files.sort();
                for file in files {
                    let body = std::fs::read_to_string(&file).map_err(|e| error::io(&file, e))?;
                    let patch: Value = toml::from_str(&body)
                        .map_err(|e| Error::Catalog(format!("{}: {e}", file.display())))?;
                    merge_catalog(&mut base, patch);
                    override_files.push(file);
                }
            }
        }

        let catalog: Catalog =
            Catalog::deserialize(base).map_err(|e| Error::Catalog(e.to_string()))?;
        let warnings = validate(&catalog)?;

        Ok(Self {
            catalog,
            source: source.to_string(),
            override_files,
            warnings,
        })
    }

    /// Look one entry up by identifier.
    pub fn harness(&self, id: &str) -> Option<&Harness> {
        self.catalog
            .harnesses
            .iter()
            .find(|harness| harness.id == id)
    }

    /// Extra directories to search, for the current platform.
    pub fn search_dirs(&self) -> &[String] {
        if cfg!(windows) {
            &self.catalog.detect_search_dirs.windows
        } else {
            &self.catalog.detect_search_dirs.unix
        }
    }

    /// Identifiers in relay order.
    pub fn ids(&self) -> Vec<&str> {
        self.catalog
            .harnesses
            .iter()
            .map(|harness| harness.id.as_str())
            .collect()
    }
}

/// Deep-merge `patch` into `base`; arrays are replaced, tables merged.
fn merge_value(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Table(base_table), Value::Table(patch_table)) => {
            for (key, value) in patch_table {
                if let Some(existing) = base_table.get_mut(&key) {
                    merge_value(existing, value);
                } else {
                    base_table.insert(key, value);
                }
            }
        }
        (slot, patch_value) => *slot = patch_value,
    }
}

fn merge_catalog(base: &mut Value, patch: Value) {
    let Value::Table(patch_table) = patch else {
        return;
    };
    let Some(base_table) = base.as_table_mut() else {
        return;
    };
    for (key, value) in patch_table {
        if key == "harness" {
            merge_harnesses(base_table, value);
        } else if let Some(existing) = base_table.get_mut(&key) {
            merge_value(existing, value);
        } else {
            base_table.insert(key, value);
        }
    }
}

/// Harness arrays merge entry by entry, matched on `id`.
fn merge_harnesses(base_table: &mut Table, patch: Value) {
    let Value::Array(patch_entries) = patch else {
        return;
    };
    let mut entries = match base_table.remove("harness") {
        Some(Value::Array(entries)) => entries,
        _ => Vec::new(),
    };
    for patch_entry in patch_entries {
        let id = patch_entry
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let position = id.as_deref().and_then(|wanted| {
            entries
                .iter()
                .position(|entry| entry.get("id").and_then(Value::as_str) == Some(wanted))
        });
        match position {
            Some(index) => {
                if let Some(entry) = entries.get_mut(index) {
                    merge_value(entry, patch_entry);
                }
            }
            None => entries.push(patch_entry),
        }
    }
    base_table.insert("harness".to_string(), Value::Array(entries));
}

fn check_patterns(owner: &str, patterns: &Patterns, problems: &mut Vec<String>) {
    let lists = [
        ("quota", &patterns.quota),
        ("auth", &patterns.auth),
        ("reset", &patterns.reset),
    ];
    for (kind, list) in lists {
        for pattern in list {
            if let Err(err) = Regex::new(pattern) {
                problems.push(format!(
                    "`{owner}`: {kind} pattern {pattern:?} does not compile: {err}"
                ));
            }
        }
    }
}

/// Hard errors stop the load; warnings are shown by `doctor`.
fn validate(catalog: &Catalog) -> Result<Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if catalog.schema != 1 {
        problems.push(format!(
            "schema {} is not supported by this build (expected 1)",
            catalog.schema
        ));
    }

    check_patterns(
        "detection_generic",
        &catalog.detection_generic,
        &mut problems,
    );

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for harness in &catalog.harnesses {
        if harness.id.is_empty() {
            problems.push("a harness has an empty id".to_string());
            continue;
        }
        if !seen.insert(harness.id.as_str()) {
            problems.push(format!("duplicate harness id `{}`", harness.id));
        }
        match harness.mode {
            Mode::Headless => {
                if harness.binaries.is_empty() {
                    problems.push(format!(
                        "`{}`: headless harness without binaries",
                        harness.id
                    ));
                }
                if harness.run.is_empty() {
                    problems.push(format!(
                        "`{}`: headless harness without a run template",
                        harness.id
                    ));
                } else if !harness.run.iter().any(|arg| arg == "{prompt}") {
                    problems.push(format!(
                        "`{}`: run must contain the {{prompt}} placeholder",
                        harness.id
                    ));
                }
                if harness.unattended_required && harness.unattended_args.is_empty() {
                    warnings.push(format!(
                        "`{}`: unattended_required without unattended_args will always be skipped",
                        harness.id
                    ));
                }
            }
            Mode::Gui => {
                if harness.gui_keywords.is_empty() {
                    problems.push(format!(
                        "`{}`: GUI harness without gui_keywords",
                        harness.id
                    ));
                }
            }
            Mode::Interactive => {}
        }
        if let Some(patterns) = &harness.detection {
            check_patterns(&harness.id, patterns, &mut problems);
        }
    }

    if catalog.detection_generic.quota.is_empty() && catalog.detection_generic.auth.is_empty() {
        warnings.push(
            "catalog has no quota or auth patterns: no attempt could be classified".to_string(),
        );
    }

    if problems.is_empty() {
        Ok(warnings)
    } else {
        Err(Error::Catalog(problems.join("; ")))
    }
}
