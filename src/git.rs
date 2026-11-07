//! The few git commands Orion needs: is this a repository, did the harness
//! change anything, and what changed.
//!
//! Every call is read-only and silent. A missing `git` binary is not an error:
//! the answer is simply "unknown", and a change is assumed when it matters.

use std::path::Path;
use std::process::{Command, Stdio};

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
    )
}

/// Whether `root` is inside a git working tree.
pub fn is_repo(root: &Path) -> bool {
    git(root, &["rev-parse", "--is-inside-work-tree"]).as_deref() == Some("true")
}

/// `git status --porcelain`, or `None` when git cannot answer.
pub fn status(root: &Path) -> Option<String> {
    git(root, &["status", "--porcelain"])
}

/// Whether the working tree differs from `HEAD`. Unknown counts as changed:
/// assuming "no change" would let a real edit look like a failed attempt.
pub fn changed(root: &Path) -> bool {
    match status(root) {
        Some(text) => !text.trim().is_empty(),
        None => true,
    }
}

/// `git diff --stat` for tracked changes, plus the untracked files git leaves
/// out of it.
pub fn diff_stat(root: &Path) -> Option<String> {
    let tracked = git(root, &["diff", "--stat"])?;
    let mut summary = tracked.trim().to_string();
    if let Some(listing) = status(root) {
        let untracked: Vec<&str> = listing
            .lines()
            .filter_map(|line| line.strip_prefix("?? "))
            .take(20)
            .collect();
        if !untracked.is_empty() {
            if !summary.is_empty() {
                summary.push_str("\n\n");
            }
            summary.push_str("untracked:\n");
            for file in untracked {
                summary.push_str("  ");
                summary.push_str(file);
                summary.push('\n');
            }
            summary = summary.trim_end().to_string();
        }
    }
    Some(summary)
}
