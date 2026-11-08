//! `next-harness doctor`: what this machine has, in a form a maintainer can act on.
//!
//! The `--report` output is Markdown meant to be pasted into an issue. It
//! contains the operating system, the version, and one line per harness — never
//! an account name, never a credential, never an environment value.

use crate::config::Config;
use crate::detector::Detection;
use crate::i18n;
use crate::orchestrator::DEFAULT_MAX_ATTEMPTS;
use crate::paths::{home_dir, Paths};
use crate::timefmt::{format_utc, now_epoch};
use crate::util::{mask_home, truncate};
use crate::VERSION;

fn system() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

fn scrubbed(path: impl std::fmt::Display) -> String {
    let text = path.to_string();
    match home_dir() {
        Some(home) => mask_home(&text, &home),
        None => text,
    }
}

/// Lines for a human at the terminal.
pub fn lines(config: &Config, detections: &[Detection], paths: &Paths) -> Vec<String> {
    let mut out = vec![i18n::tr_args(
        "cli.doctor.system",
        &[
            ("os", std::env::consts::OS.to_string()),
            ("arch", std::env::consts::ARCH.to_string()),
        ],
    )];
    out.push(i18n::tr_args(
        "cli.doctor.version",
        &[("version", VERSION.to_string())],
    ));
    out.push(i18n::tr_args(
        "cli.doctor.catalog",
        &[
            ("harnesses", detections.len().to_string()),
            ("schema", config.catalog.schema.to_string()),
        ],
    ));
    out.push(i18n::tr_args(
        "cli.config.path",
        &[("path", scrubbed(paths.config_dir.display()))],
    ));

    for detection in detections {
        out.push(i18n::tr_args(
            "cli.doctor.line",
            &[
                ("status", i18n::tr(detection.status.label_key())),
                ("id", detection.id.clone()),
                ("version", detection.version.clone().unwrap_or_default()),
                ("verified", detection.verified.label().to_string()),
            ],
        ));
    }

    for warning in &config.warnings {
        out.push(i18n::tr_args(
            "cli.doctor.warning",
            &[("text", warning.clone())],
        ));
    }
    out
}

/// A Markdown report, safe to paste into a public issue.
pub fn markdown(config: &Config, detections: &[Detection], paths: &Paths) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", i18n::tr("cli.doctor.title")));
    out.push_str(&format!("- next-harness: {VERSION}\n"));
    out.push_str(&format!("- system: {}\n", system()));
    out.push_str(&format!("- rust: {}\n", rustc_version()));
    out.push_str(&format!(
        "- catalog: {} harnesses, schema {} ({})\n",
        detections.len(),
        config.catalog.schema,
        scrubbed(&config.source)
    ));
    if !config.override_files.is_empty() {
        for file in &config.override_files {
            out.push_str(&format!("- override: {}\n", scrubbed(file.display())));
        }
    }
    out.push_str(&format!(
        "- config: {}\n",
        scrubbed(paths.config_dir.display())
    ));
    out.push_str(&format!(
        "- state: {}\n",
        scrubbed(paths.state_file.display())
    ));
    out.push_str(&format!(
        "- attempts per run: {DEFAULT_MAX_ATTEMPTS} (default)\n\n"
    ));

    out.push_str("| id | name | mode | status | version | tier | verified |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for detection in detections {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            cell(&detection.id),
            cell(&detection.name),
            mode_label(detection),
            cell(&i18n::tr(detection.status.label_key())),
            cell(detection.version.as_deref().unwrap_or("-")),
            detection.tier.label(),
            detection.verified.label()
        ));
    }

    if !config.warnings.is_empty() {
        out.push_str("\n## warnings\n\n");
        for warning in &config.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }

    out.push_str(&format!("\n_reported at {}_\n", format_utc(now_epoch())));
    out
}

fn mode_label(detection: &Detection) -> &'static str {
    match detection.mode {
        crate::config::Mode::Headless => "headless",
        crate::config::Mode::Interactive => "interactive",
        crate::config::Mode::Gui => "gui",
    }
}

/// Keep a table cell on one line and unable to break the table.
fn cell(text: &str) -> String {
    truncate(&text.replace('|', "\\|").replace('\n', " "), 60)
}

fn rustc_version() -> String {
    option_env!("CARGO_PKG_RUST_VERSION")
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string()
}
