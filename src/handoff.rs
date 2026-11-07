//! The two documents that carry a task across harnesses.
//!
//! `TASK.md` is the original request, written once per run. `HANDOFF.md` is
//! rewritten before every switch: the next harness learns what was done, what
//! the repository looks like now, and that it must not start over.
//!
//! Everything that comes from an agent's output is masked here, so a secret
//! echoed in a log never lands in a file that will be read by another program.

use std::fs;
use std::path::Path;

use crate::config::Harness;
use crate::error::{self, Result};
use crate::git;
use crate::i18n;
use crate::paths::{home_dir, Paths};
use crate::util::{mask_home, mask_secrets, tail_lines};

/// How much of the previous output the handoff keeps.
pub const OUTPUT_TAIL: usize = 40;

/// Write the task document, creating `.NextHarness` if needed.
pub fn write_task(paths: &Paths, task: &str) -> Result<()> {
    fs::create_dir_all(&paths.next_harness_dir).map_err(|e| error::io(&paths.next_harness_dir, e))?;
    let body = format!("# {}\n\n{}\n", i18n::tr("handoff.task"), task.trim());
    fs::write(&paths.task_file, body).map_err(|e| error::io(&paths.task_file, e))
}

/// Read the task document back, for `NextHarness resume`.
pub fn read_task(paths: &Paths) -> Result<String> {
    let text = fs::read_to_string(&paths.task_file).map_err(|e| error::io(&paths.task_file, e))?;
    let body = text
        .lines()
        .skip_while(|line| line.starts_with('#') || line.trim().is_empty())
        .collect::<Vec<&str>>()
        .join("\n");
    Ok(body.trim().to_string())
}

/// What the next harness needs to know about the previous one.
pub struct Handoff<'a> {
    /// The original task.
    pub task: &'a str,
    /// The harness that just stopped.
    pub harness: &'a Harness,
    /// Why it stopped, already translated.
    pub reason: &'a str,
    /// Its output, raw: only the tail is kept and it is masked here.
    pub output: &'a str,
}

/// Rewrite `HANDOFF.md` for the next harness.
pub fn write(paths: &Paths, root: &Path, doc: &Handoff) -> Result<()> {
    fs::create_dir_all(&paths.next_harness_dir).map_err(|e| error::io(&paths.next_harness_dir, e))?;

    let mut body = String::new();
    body.push_str(&format!("# {}\n\n", i18n::tr("handoff.title")));
    body.push_str(&format!(
        "## {}\n\n{}\n\n",
        i18n::tr("handoff.task"),
        doc.task.trim()
    ));

    body.push_str(&format!("## {}\n\n", i18n::tr("handoff.previous")));
    body.push_str(&format!("- `{}` ({})\n", doc.harness.id, doc.harness.name));
    body.push_str(&format!("- {}\n\n", doc.reason));

    body.push_str(&format!("## {}\n\n", i18n::tr("handoff.repo")));
    if git::is_repo(root) {
        let status = git::status(root).unwrap_or_default();
        let diff = git::diff_stat(root).unwrap_or_default();
        let summary = status.trim().to_string();
        body.push_str("```\n");
        if summary.is_empty() {
            body.push_str("(clean)\n");
        } else {
            body.push_str(&summary);
            body.push('\n');
        }
        body.push_str("```\n\n");
        if !diff.trim().is_empty() {
            body.push_str("```\n");
            body.push_str(diff.trim_end());
            body.push_str("\n```\n\n");
        }
    } else {
        body.push_str(&format!("{}\n\n", i18n::tr("handoff.not_repo")));
    }

    body.push_str(&format!("## {}\n\n", i18n::tr("handoff.output")));
    body.push_str("```\n");
    let tail = tail_lines(&mask_secrets(doc.output), OUTPUT_TAIL);
    body.push_str(tail.trim_end());
    body.push_str("\n```\n\n");

    body.push_str(&format!("## {}\n\n", i18n::tr("handoff.next")));
    body.push_str(&format!("{}\n", i18n::tr("handoff.continue")));

    let body = match home_dir() {
        Some(home) => mask_home(&body, &home),
        None => body,
    };
    fs::write(&paths.handoff_file, body).map_err(|e| error::io(&paths.handoff_file, e))
}
