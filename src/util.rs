//! Text helpers: ANSI stripping for analysis, secret masking for logs and
//! handoff documents, and small truncation utilities.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

static ANSI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1B\][^\x07\x1B]*(?:\x07|\x1B\\)|\x1B\[[0-9;?]*[ -/]*[@-~]")
        .expect("ANSI escape pattern is valid")
});

/// Patterns whose match is replaced, in order. Each entry is
/// `(pattern, replacement)`; `$1` keeps a prefix such as `Bearer `.
static SECRETS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (
            Regex::new(r"\b(sk-[A-Za-z0-9_-]{12,})").expect("static pattern"),
            "***",
        ),
        (
            Regex::new(r"\b(gh[pousr]_[A-Za-z0-9]{16,})").expect("static pattern"),
            "***",
        ),
        (
            Regex::new(r"\b(xai-[A-Za-z0-9_-]{8,})").expect("static pattern"),
            "***",
        ),
        (
            Regex::new(r"\bAIza[0-9A-Za-z_-]{20,}").expect("static pattern"),
            "***",
        ),
        (
            Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._-]{12,}").expect("static pattern"),
            "${1}***",
        ),
        (
            Regex::new(r"(?im)^([A-Za-z0-9_]*(?:KEY|TOKEN|SECRET)[A-Za-z0-9_]*)\s*[=:]\s*\S+")
                .expect("static pattern"),
            "${1}=***",
        ),
    ]
});

/// Remove ANSI escape sequences so quota patterns are never split by colour
/// codes a harness emitted mid-sentence.
///
/// Both families that show up in practice are handled: control sequences
/// (`ESC[?25h`) and operating-system commands, which a console uses to set the
/// window title (`ESC]0;…BEL`) and which would otherwise leak the title text into
/// the transcript.
pub fn strip_ansi(input: &str) -> String {
    ANSI.replace_all(input, "").into_owned()
}

/// Replace anything that looks like a credential. Logs and `HANDOFF.md` grow out
/// of raw agent output, which may echo an environment variable.
pub fn mask_secrets(input: &str) -> String {
    let mut output = input.to_string();
    for (pattern, replacement) in SECRETS.iter() {
        output = pattern.replace_all(&output, *replacement).into_owned();
    }
    output
}

/// The last `count` lines of `input`.
pub fn tail_lines(input: &str, count: usize) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].join("\n")
}

/// Cut to `max_chars` characters (never bytes) and add an ellipsis.
pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// Replace the user's home directory with `~` so a pasted report carries no
/// account name.
pub fn mask_home(text: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    if home.is_empty() {
        return text.to_string();
    }
    text.replace(home.as_ref(), "~")
}

/// Sanitise one line of agent output for a terminal: no escapes, no secrets,
/// no trailing whitespace.
pub fn clean_line(line: &str) -> String {
    mask_secrets(&strip_ansi(line)).trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_is_removed_without_touching_text() {
        assert_eq!(
            strip_ansi("\x1b[31mrate limit\x1b[0m reached"),
            "rate limit reached"
        );
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn secrets_are_masked() {
        let masked = mask_secrets("export OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz");
        assert!(!masked.contains("sk-abcdefghijklmnopqrstuvwxyz"));
        assert!(masked.contains("OPENAI_API_KEY=***"));

        let bearer = mask_secrets("Authorization: Bearer abcdefghijklmnopqrst");
        assert!(!bearer.contains("abcdefghijklmnopqrst"));

        let pat = mask_secrets("token ghp_abcdefghijklmnopqrstuvwx leaked");
        assert!(!pat.contains("ghp_abcdefghijklmnopqrstuvwx"));
    }

    #[test]
    fn ordinary_text_survives_masking() {
        let text = "rate limit reached; resets in 3 hours";
        assert_eq!(mask_secrets(text), text);
    }

    #[test]
    fn tails_and_truncation_are_character_safe() {
        assert_eq!(tail_lines("a\nb\nc\nd", 2), "c\nd");
        assert_eq!(tail_lines("a", 5), "a");
        assert_eq!(truncate("héllo wörld", 5), "héll…");
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn home_is_masked() {
        assert_eq!(
            mask_home("/home/sammy/project", Path::new("/home/sammy")),
            "~/project"
        );
    }
}
