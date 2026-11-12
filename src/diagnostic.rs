//! Local zero-token sanity checks to ensure an interrupted agent didn't leave
//! broken syntax, merge conflict markers, or corrupted files for the next agent.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::git;

/// An issue detected on a file in the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// File path relative to repository root.
    pub file: String,
    /// Category of issue.
    pub kind: String,
    /// Human-readable explanation.
    pub message: String,
}

/// The result of running local diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticReport {
    pub issues: Vec<Issue>,
}

impl DiagnosticReport {
    /// True when no issue was detected.
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    /// Short human summary of the diagnostic result.
    pub fn summary(&self) -> String {
        if self.issues.is_empty() {
            "clean".to_string()
        } else {
            format!("{} issue(s) detected", self.issues.len())
        }
    }
}

/// Run fast local sanity checks on the repository root.
pub fn check(root: &Path) -> DiagnosticReport {
    let mut issues = Vec::new();

    // 1. Check git conflict markers
    if let Some(diff_issues) = git::diff_check(root) {
        for line in diff_issues.lines() {
            let line = line.trim();
            if line.contains("conflict marker") || line.contains("leftover") {
                let parts: Vec<&str> = line.split(':').collect();
                let file = parts.first().unwrap_or(&"unknown").trim().to_string();
                issues.push(Issue {
                    file,
                    kind: "conflict-marker".to_string(),
                    message: line.to_string(),
                });
            }
        }
    }

    // 2. Inspect changed files for syntax / unclosed delimiters
    let changed = git::changed_files(root);
    for rel_path in &changed {
        let full_path = root.join(rel_path);
        if !full_path.is_file() {
            continue;
        }

        // Avoid checking .next-harness internal files
        if rel_path.starts_with(".next-harness") {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&full_path) {
            if rel_path.ends_with(".json") {
                if let Err(err) = serde_json::from_str::<serde_json::Value>(&content) {
                    issues.push(Issue {
                        file: rel_path.clone(),
                        kind: "json-syntax".to_string(),
                        message: format!("invalid JSON: {err}"),
                    });
                }
            } else if rel_path.ends_with(".toml") {
                if let Err(err) = toml::from_str::<toml::Value>(&content) {
                    issues.push(Issue {
                        file: rel_path.clone(),
                        kind: "toml-syntax".to_string(),
                        message: format!("invalid TOML: {err}"),
                    });
                }
            } else if is_code_file(rel_path) && has_unclosed_delimiters(&content) {
                issues.push(Issue {
                    file: rel_path.clone(),
                    kind: "unclosed-delimiter".to_string(),
                    message: "unclosed brackets or braces: file appears truncated or incomplete"
                        .to_string(),
                });
            }
        }
    }

    // 3. Fast Cargo check if Rust project and .rs files changed
    let has_rs_changes = changed.iter().any(|f| f.ends_with(".rs"));
    if has_rs_changes && root.join("Cargo.toml").is_file() {
        if let Some(err) = quick_cargo_check(root) {
            issues.push(Issue {
                file: "Cargo.toml".to_string(),
                kind: "cargo-check".to_string(),
                message: err,
            });
        }
    }

    DiagnosticReport { issues }
}

fn is_code_file(path: &str) -> bool {
    let extensions = [
        ".rs", ".js", ".ts", ".jsx", ".tsx", ".c", ".cpp", ".h", ".java", ".go",
    ];
    extensions.iter().any(|ext| path.ends_with(ext))
}

/// Delimiter and string matching to detect incomplete or truncated code files.
pub fn has_unclosed_delimiters(code: &str) -> bool {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let mut escaped = false;
    let mut in_line_comment = false;

    let chars: Vec<char> = code.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];

        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }

        if in_string {
            if ch == string_char {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Check for line comment //
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            in_line_comment = true;
            i += 2;
            continue;
        }

        match ch {
            '"' | '\'' | '`' => {
                in_string = true;
                string_char = ch;
            }
            '{' | '(' | '[' => stack.push(ch),
            '}' if stack.pop() != Some('{') => return true,
            ')' if stack.pop() != Some('(') => return true,
            ']' if stack.pop() != Some('[') => return true,
            '}' | ')' | ']' => {}  // matched correctly, continue
            _ => {}
        }
        i += 1;
    }

    !stack.is_empty() || in_string
}

fn quick_cargo_check(root: &Path) -> Option<String> {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--message-format=short")
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error_lines: Vec<&str> = stderr
            .lines()
            .filter(|line| line.contains("error:") || line.contains("error["))
            .take(3)
            .collect();
        if !error_lines.is_empty() {
            return Some(error_lines.join(" | "));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_code_has_no_delimiter_issues() {
        let code = r#"
            // Some comment
            fn test() {
                let x = vec![1, 2, 3];
                println!("hello world ({})", x.len());
            }
        "#;
        assert!(!has_unclosed_delimiters(code));
    }

    #[test]
    fn truncated_code_is_detected() {
        let code = r#"
            fn test() {
                let x = "unterminated string;
        "#;
        assert!(has_unclosed_delimiters(code));

        let code_bracket = r#"
            fn test() {
                let x = [1, 2, 3;
            }
        "#;
        assert!(has_unclosed_delimiters(code_bracket));
    }
}
