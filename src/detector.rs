//! Finding installed harnesses: binary, version, credentials, desktop apps.
//!
//! Nothing here consumes quota: a version probe is `--version`, and a desktop
//! application is looked up by name only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{Config, Harness, Mode, Tier, Verified};
use crate::invoke::{capture, invocation, path_with};
use crate::paths::home_dir;
use crate::util::{strip_ansi, truncate};

/// Platform families the catalog speaks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Windows.
    Windows,
    /// macOS.
    Macos,
    /// Linux, including inside WSL.
    Linux,
}

impl Platform {
    /// The platform this binary was built for.
    pub fn current() -> Self {
        if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::Macos
        } else {
            Platform::Linux
        }
    }

    /// The name used in the catalog's `platforms` field.
    pub fn family(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Macos => "macos",
            Platform::Linux => "linux",
        }
    }
}

/// What NextHarness knows about one harness on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Installed, has what it needs, can be started.
    Ready,
    /// Installed but signed out; a run will confirm it.
    NeedsLogin,
    /// Declared credentials are absent.
    MissingKey,
    /// Not found here.
    NotInstalled,
    /// A terminal agent a human drives.
    ManualOnly,
    /// Only reachable inside WSL, which v1 does not drive.
    WslOnly,
}

impl Status {
    /// Translation key for the label shown by `doctor` and `init`.
    pub fn label_key(self) -> &'static str {
        match self {
            Status::Ready => "harness.status.ready",
            Status::NeedsLogin => "harness.status.needs_login",
            Status::MissingKey => "harness.status.missing_key",
            Status::NotInstalled => "harness.status.not_installed",
            Status::ManualOnly => "harness.status.manual_only",
            Status::WslOnly => "harness.status.wsl",
        }
    }

    /// Whether the harness can take part in a run.
    pub fn is_usable(self) -> bool {
        matches!(self, Status::Ready | Status::ManualOnly)
    }
}

/// The detection result for one catalog entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    /// Catalog id.
    pub id: String,
    /// Human name.
    pub name: String,
    /// How it may be used.
    pub mode: Mode,
    /// Price tier.
    pub tier: Tier,
    /// Trust level.
    pub verified: Verified,
    /// Outcome.
    pub status: Status,
    /// Resolved executable or application.
    pub binary: Option<PathBuf>,
    /// Version string, when one could be read without consuming quota.
    pub version: Option<String>,
    /// Human-readable explanation, for `doctor`.
    pub detail: String,
}

impl Detection {
    /// Whether this harness can be started for a task.
    pub fn is_ready(&self) -> bool {
        self.status == Status::Ready
    }
}

/// Looks harnesses up on this machine.
pub struct Detector {
    platform: Platform,
    search_paths: Vec<PathBuf>,
    gui_roots: Option<Vec<PathBuf>>,
    version_timeout: Duration,
    gui_cache: OnceLock<Vec<(String, PathBuf)>>,
}

impl Detector {
    /// Build a detector from the current environment and the catalog's extra
    /// search directories.
    pub fn new(config: &Config) -> Self {
        let mut search_paths: Vec<PathBuf> =
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
        for raw in config.search_dirs() {
            if let Some(path) = expand_path(raw) {
                search_paths.push(path);
            }
        }
        search_paths.extend(extra_search_paths());
        dedupe(&mut search_paths);

        Self {
            platform: Platform::current(),
            search_paths,
            gui_roots: None,
            version_timeout: Duration::from_secs(5),
            gui_cache: OnceLock::new(),
        }
    }

    /// A detector with explicit roots, for tests and unusual installs.
    pub fn with_roots(search_paths: Vec<PathBuf>, gui_roots: Vec<PathBuf>) -> Self {
        Self {
            platform: Platform::current(),
            search_paths,
            gui_roots: Some(gui_roots),
            version_timeout: Duration::from_secs(5),
            gui_cache: OnceLock::new(),
        }
    }

    /// The platform this detector was built for.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Detect every entry, in catalog order.
    pub fn detect_all(&self, config: &Config) -> Vec<Detection> {
        let mut claimed: HashMap<&str, &str> = HashMap::new();
        let mut results = Vec::with_capacity(config.catalog.harnesses.len());

        for harness in &config.catalog.harnesses {
            let detection = self.detect(harness);

            // Two entries sharing an install_hint are one product under two
            // binary names: the first installed one wins.
            if detection.status.is_usable() {
                if let Some(hint) = harness.install_hint.as_deref() {
                    if let Some(owner) = claimed.get(hint).copied() {
                        results.push(Detection {
                            status: Status::NotInstalled,
                            detail: format!("superseded by `{owner}`"),
                            ..detection
                        });
                        continue;
                    }
                    claimed.insert(hint, harness.id.as_str());
                }
            }
            results.push(detection);
        }

        results
    }

    /// Detect one entry.
    pub fn detect(&self, harness: &Harness) -> Detection {
        let mut detection = Detection {
            id: harness.id.clone(),
            name: harness.name.clone(),
            mode: harness.mode,
            tier: harness.default_tier,
            verified: harness.verified,
            status: Status::NotInstalled,
            binary: None,
            version: None,
            detail: String::new(),
        };

        if harness.mode == Mode::Gui {
            return match self.detect_gui(&harness.gui_keywords) {
                Some(path) => Detection {
                    status: Status::Ready,
                    detail: path.display().to_string(),
                    binary: Some(path),
                    ..detection
                },
                None => Detection {
                    detail: "application not found".to_string(),
                    ..detection
                },
            };
        }

        if !self.platform_supported(harness) {
            let wsl = harness.platforms.iter().any(|entry| entry == "wsl");
            detection.status = if wsl && self.has_wsl() {
                Status::WslOnly
            } else {
                Status::NotInstalled
            };
            detection.detail = format!("needs {}", harness.platforms.join("/"));
            return detection;
        }

        let Some(binary) = self.find_binary(&harness.binaries) else {
            detection.detail = format!("not found ({})", harness.binaries.join(", "));
            return detection;
        };
        detection.binary = Some(binary.clone());

        let missing: Vec<&str> = harness
            .companion_binaries
            .iter()
            .filter(|name| self.find_binary(std::slice::from_ref(*name)).is_none())
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            detection.detail = format!("missing companion binary ({})", missing.join(", "));
            return detection;
        }

        if !harness.version_args.is_empty() {
            match self.probe_version(&binary, &harness.version_args) {
                Some(version) => detection.version = Some(version),
                None => {
                    // A binary that cannot answer --version is probably a name
                    // collision, not the harness the catalog describes.
                    detection.detail = "version probe failed (name collision?)".to_string();
                    return detection;
                }
            }
        }

        if !harness.requires_env.is_empty() {
            let missing: Vec<&str> = harness
                .requires_env
                .iter()
                .map(String::as_str)
                .filter(|name| std::env::var_os(name).is_none())
                .collect();
            let has_auth_file = harness
                .auth_files
                .iter()
                .filter_map(|file| expand_path(file))
                .any(|path| path.exists());
            if missing.len() == harness.requires_env.len() && !has_auth_file {
                detection.status = Status::MissingKey;
                detection.detail = format!("missing env {}", missing.join(", "));
                return detection;
            }
        }

        detection.status = if harness.mode == Mode::Interactive {
            Status::ManualOnly
        } else {
            Status::Ready
        };
        detection.detail = binary.display().to_string();
        detection
    }

    fn platform_supported(&self, harness: &Harness) -> bool {
        if harness.platforms.is_empty() {
            return true;
        }
        harness
            .platforms
            .iter()
            .any(|entry| match (self.platform, entry.as_str()) {
                (Platform::Windows, "windows") => true,
                (Platform::Windows, "wsl") => self.has_wsl(),
                (Platform::Macos, "macos") => true,
                (Platform::Linux, "linux" | "wsl") => true,
                _ => false,
            })
    }

    fn has_wsl(&self) -> bool {
        cfg!(windows) && which::which("wsl.exe").is_ok()
    }

    fn find_binary(&self, names: &[String]) -> Option<PathBuf> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        for name in names {
            if name.contains(['/', '\\']) {
                let path = PathBuf::from(name);
                if path.is_file() {
                    return Some(path);
                }
                continue;
            }
            if let Ok(joined) = std::env::join_paths(&self.search_paths) {
                if let Ok(found) = which::which_in(name, Some(joined), &cwd) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn probe_version(&self, binary: &Path, args: &[String]) -> Option<String> {
        let output = capture(
            &invocation(binary),
            args,
            self.version_timeout,
            path_with(binary),
        )?;
        if !output.status.success() {
            return None;
        }
        let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
        let line = stdout
            .lines()
            .chain(stderr.lines())
            .map(str::trim)
            .find(|line| !line.is_empty())?;
        Some(truncate(line, 40))
    }

    fn detect_gui(&self, keywords: &[String]) -> Option<PathBuf> {
        if keywords.is_empty() {
            return None;
        }
        let candidates = self.gui_cache.get_or_init(|| self.scan_gui());
        for keyword in keywords {
            let needle = keyword.to_lowercase();
            if let Some((_, path)) = candidates
                .iter()
                .find(|(name, _)| name.to_lowercase().contains(&needle))
            {
                return Some(path.clone());
            }
        }
        None
    }

    fn scan_gui(&self) -> Vec<(String, PathBuf)> {
        let roots = match &self.gui_roots {
            Some(roots) => roots.clone(),
            None => default_gui_roots(self.platform),
        };
        let mut found = Vec::new();
        for root in &roots {
            collect_names(root, 2, &mut found);
        }
        #[cfg(windows)]
        if self.gui_roots.is_none() {
            found.extend(registry_apps());
        }
        found
    }
}

/// `~`, `%VAR%` and `$VAR` expansion for catalog paths.
pub fn expand_path(raw: &str) -> Option<PathBuf> {
    let mut expanded = raw.to_string();
    if let Some(rest) = expanded.strip_prefix("~/") {
        expanded = format!("{}/{rest}", home_dir()?.display());
    } else if expanded == "~" {
        expanded = home_dir()?.display().to_string();
    }

    #[cfg(windows)]
    {
        expanded = expand_windows_vars(&expanded);
    }
    #[cfg(not(windows))]
    {
        let mut resolved = String::new();
        let mut rest = expanded.as_str();
        while let Some(start) = rest.find("${") {
            resolved.push_str(&rest[..start]);
            let Some(end) = rest[start..].find('}') else {
                break;
            };
            let name = &rest[start + 2..start + end];
            resolved.push_str(&std::env::var(name).unwrap_or_default());
            rest = &rest[start + end + 1..];
        }
        resolved.push_str(rest);
        expanded = resolved;
    }

    if expanded.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(expanded))
    }
}

#[cfg(windows)]
fn expand_windows_vars(input: &str) -> String {
    let mut resolved = String::new();
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        resolved.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(value) => resolved.push_str(&value),
                    Err(_) => {
                        resolved.push('%');
                        resolved.push_str(name);
                        resolved.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                resolved.push_str(rest);
                return resolved;
            }
        }
    }
    resolved.push_str(rest);
    resolved
}

fn dedupe(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::BTreeSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

fn default_gui_roots(platform: Platform) -> Vec<PathBuf> {
    match platform {
        Platform::Windows => {
            let mut roots = Vec::new();
            for var in ["LOCALAPPDATA", "APPDATA", "PROGRAMDATA"] {
                if let Some(base) = std::env::var_os(var) {
                    let base = PathBuf::from(base);
                    if var == "LOCALAPPDATA" {
                        roots.push(base.join("Programs"));
                    } else {
                        roots.push(
                            base.join("Microsoft")
                                .join("Windows")
                                .join("Start Menu")
                                .join("Programs"),
                        );
                    }
                }
            }
            roots
        }
        Platform::Macos => {
            let mut roots = vec![PathBuf::from("/Applications")];
            if let Some(home) = home_dir() {
                roots.push(home.join("Applications"));
            }
            roots
        }
        Platform::Linux => {
            let mut roots = vec![
                PathBuf::from("/usr/share/applications"),
                PathBuf::from("/var/lib/flatpak/exports/share/applications"),
                PathBuf::from("/var/lib/snapd/desktop/applications"),
            ];
            if let Some(home) = home_dir() {
                roots.push(home.join(".local/share/applications"));
                roots.push(home.join(".local/share/flatpak/exports/share/applications"));
            }
            roots
        }
    }
}

fn collect_names(dir: &Path, depth: usize, found: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            found.push((name.to_string(), path.clone()));
        }
        if depth > 0 && path.is_dir() {
            collect_names(&path, depth - 1, found);
        }
    }
}

#[cfg(windows)]
fn registry_apps() -> Vec<(String, PathBuf)> {
    let mut apps = Vec::new();
    for key in [
        r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    ] {
        let output = std::process::Command::new("reg")
            .args(["query", key, "/s", "/v", "DisplayName"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        let Ok(output) = output else {
            continue;
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some((_, value)) = line.split_once("REG_SZ") else {
                continue;
            };
            let value = value.trim();
            if !value.is_empty() {
                apps.push((
                    value.to_string(),
                    PathBuf::from(format!("{value} (uninstall registry)")),
                ));
            }
        }
    }
    apps
}

/// `npm prefix -g` plus the usual nvm/volta/pnpm locations.
fn extra_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(prefix) = npm_global_prefix() {
        #[cfg(windows)]
        paths.push(prefix);
        #[cfg(not(windows))]
        paths.push(prefix.join("bin"));
    }

    if let Some(home) = home_dir() {
        paths.push(home.join(".volta").join("bin"));
        paths.push(home.join(".local").join("share").join("pnpm"));
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm").join("versions").join("node")) {
            for entry in entries.flatten() {
                paths.push(entry.path().join("bin"));
            }
        }
    }

    paths
}

fn npm_global_prefix() -> Option<PathBuf> {
    let npm = which::which("npm").ok()?;
    let output = capture(
        &invocation(&npm),
        &["prefix".to_string(), "-g".to_string()],
        Duration::from_secs(3),
        path_with(&npm),
    )?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    }
}
