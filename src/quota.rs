//! Deciding whether an attempt ran out of quota — the two-signal rule.
//!
//! One regex hit never decides anything: an agent may write the words "rate
//! limit" while describing code. A verdict needs (A) a pattern in the last
//! [`TAIL_LINES`] lines **and** (B) an independent signal:
//!
//! - the process exited non-zero, or
//! - it exited 0, printed less than [`SHORT_OUTPUT`] characters **and** changed
//!   no file according to git.
//!
//! Harnesses that document `quota_ignores_exit_code` replace (B) with "the output
//! is short": their usage-limit message arrives with exit code 0.

use std::time::Duration;

use regex::Regex;

use crate::config::{Harness, Patterns};
use crate::runner::Outcome;
use crate::util::{mask_secrets, tail_lines, truncate};

/// Only this many trailing lines are searched.
pub const TAIL_LINES: usize = 40;

/// "Short output" for the second signal.
pub const SHORT_OUTPUT: usize = 600;

/// Pause applied when the harness gives no reset time.
pub const DEFAULT_PAUSE: Duration = Duration::from_secs(3_600);

/// What was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The harness is out of quota.
    Quota,
    /// The harness wants a login.
    Auth,
}

impl Kind {
    /// Value stored in the state file.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Quota => "quota",
            Kind::Auth => "auth",
        }
    }
}

/// What NextHarness should do with the verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Confirmed: pause the harness and hand off.
    Pause,
    /// Possible: ask the human instead of guessing.
    OpenWindow,
}

/// A detection result.
#[derive(Debug, Clone)]
pub struct Verdict {
    /// Which rule fired.
    pub kind: Kind,
    /// True when both signals agreed.
    pub confirmed: bool,
    /// A masked excerpt of the matching line, for logs and the handoff.
    pub evidence: String,
    /// Reset time announced by the harness, if it gave one.
    pub reset_at: Option<u64>,
    /// What to do next.
    pub effect: Effect,
}

impl Verdict {
    /// Seconds of pause this verdict implies, from `now`.
    pub fn pause_seconds(&self, now: u64) -> u64 {
        match self.reset_at {
            Some(at) if at > now => at - now,
            _ => DEFAULT_PAUSE.as_secs(),
        }
    }
}

fn compile(patterns: &Patterns, kind: Kind, harness: &Harness) -> Vec<Regex> {
    let mut raw: Vec<&String> = match kind {
        Kind::Quota => patterns.quota.iter().collect(),
        Kind::Auth => patterns.auth.iter().collect(),
    };
    if let Some(specific) = &harness.detection {
        let list = match kind {
            Kind::Quota => &specific.quota,
            Kind::Auth => &specific.auth,
        };
        raw.extend(list.iter());
    }
    raw.iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect()
}

fn first_match(patterns: &[Regex], text: &str) -> Option<String> {
    for pattern in patterns {
        if let Some(found) = pattern.find(text) {
            let line_start = text[..found.start()]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            let line_end = text[found.end()..]
                .find('\n')
                .map_or(text.len(), |index| found.end() + index);
            return Some(truncate(&mask_secrets(&text[line_start..line_end]), 160));
        }
    }
    None
}

/// Seconds in one unit of a reset message.
fn unit_seconds(unit: &str) -> Option<u64> {
    let unit = unit.to_ascii_lowercase();
    match unit.chars().next()? {
        'h' => Some(3_600),
        'm' if unit.starts_with("ms") => Some(0),
        'm' => Some(60),
        's' => Some(1),
        'd' => Some(86_400),
        _ => None,
    }
}

/// Parse a reset time out of the transcript: `resets in 2 hours`,
/// `try again in 30 seconds`, `resets at 14:30`.
///
/// Times of day are interpreted in UTC, which is what NextHarness displays.
pub fn parse_reset(patterns: &[String], text: &str, now: u64) -> Option<u64> {
    for pattern in patterns {
        let Ok(regex) = Regex::new(pattern) else {
            continue;
        };
        let Some(captures) = regex.captures(text) else {
            continue;
        };
        if let (Some(amount), Some(unit)) = (captures.get(2), captures.get(3)) {
            let value = amount.as_str().parse::<u64>().ok()?;
            let seconds = unit_seconds(unit.as_str())?;
            return Some(now + value * seconds);
        }
        if let Some(clock) = captures.get(2) {
            let mut parts = clock.as_str().split(':');
            let hours = parts.next()?.parse::<u64>().ok()?;
            let minutes = parts.next()?.parse::<u64>().ok()?;
            let seconds_today = now % 86_400;
            let target = hours * 3_600 + minutes * 60;
            let mut at = now - seconds_today + target;
            if at <= now {
                at += 86_400;
            }
            return Some(at);
        }
    }
    None
}

/// Classify one finished attempt.
///
/// `files_changed` comes from git: a harness that edited files did real work
/// even if the word "quota" appears in its transcript.
pub fn classify(
    harness: &Harness,
    patterns: &Patterns,
    outcome: &Outcome,
    files_changed: bool,
    now: u64,
) -> Option<Verdict> {
    let tail = tail_lines(&outcome.output, TAIL_LINES);
    if tail.trim().is_empty() {
        return None;
    }

    let short = outcome.length() < SHORT_OUTPUT;
    let exit_failed = outcome.exit_code != Some(0);

    for kind in [Kind::Auth, Kind::Quota] {
        let Some(evidence) = first_match(&compile(patterns, kind, harness), &tail) else {
            continue;
        };

        let confirmed = if harness.quota_ignores_exit_code {
            short
        } else {
            exit_failed || (short && !files_changed)
        };

        let effect = if confirmed {
            Effect::Pause
        } else if short && !exit_failed {
            Effect::OpenWindow
        } else {
            // Long output, files changed, exit 0: an agent describing quotas,
            // not an exhausted quota.
            continue;
        };

        let mut resets: Vec<String> = patterns.reset.clone();
        if let Some(specific) = &harness.detection {
            resets.extend(specific.reset.iter().cloned());
        }
        let reset_at = parse_reset(&resets, &tail, now);

        return Some(Verdict {
            kind,
            confirmed,
            evidence,
            reset_at,
            effect,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(text: &str, exit_code: Option<u32>) -> Outcome {
        Outcome {
            exit_code,
            output: text.to_string(),
            timed_out: false,
            stalled: false,
            duration: Duration::from_secs(1),
        }
    }

    fn builtin() -> crate::config::Config {
        crate::config::Config::builtin().expect("builtin catalog")
    }

    #[test]
    fn short_output_parsing_is_exact() {
        assert_eq!(unit_seconds("h"), Some(3_600));
        assert_eq!(unit_seconds("hours"), Some(3_600));
        assert_eq!(unit_seconds("min"), Some(60));
        assert_eq!(unit_seconds("seconds"), Some(1));
        assert_eq!(unit_seconds("d"), Some(86_400));
        assert_eq!(unit_seconds("x"), None);
    }

    #[test]
    fn reset_durations_and_clocks_are_understood() {
        let config = builtin();
        let reset = &config.catalog.detection_generic.reset;
        let now = 1_000_000;
        assert_eq!(
            parse_reset(reset, "resets in 2 hours", now),
            Some(now + 7_200)
        );
        assert_eq!(
            parse_reset(reset, "try again in 30 seconds", now),
            Some(now + 30)
        );
        let utc_midnight = (now / 86_400) * 86_400;
        assert_eq!(
            parse_reset(reset, "resets at 14:30", utc_midnight),
            Some(utc_midnight + 14 * 3_600 + 30 * 60)
        );
        assert_eq!(parse_reset(reset, "nothing to parse", now), None);
    }

    #[test]
    fn a_quota_message_with_a_failing_exit_is_confirmed() {
        let config = builtin();
        let harness = config.harness("opencode").expect("opencode");
        let verdict = classify(
            harness,
            &config.catalog.detection_generic,
            &outcome("rate limit reached", Some(1)),
            false,
            0,
        )
        .expect("verdict");
        assert_eq!(verdict.kind, Kind::Quota);
        assert!(verdict.confirmed);
        assert_eq!(verdict.effect, Effect::Pause);
    }

    #[test]
    fn a_quota_message_with_exit_zero_needs_short_output_and_no_edits() {
        let config = builtin();
        let harness = config.harness("opencode").expect("opencode");
        let short = outcome("usage limit reached\n", Some(0));

        let confirmed = classify(harness, &config.catalog.detection_generic, &short, false, 0)
            .expect("verdict");
        assert!(confirmed.confirmed);

        let uncertain = classify(harness, &config.catalog.detection_generic, &short, true, 0)
            .expect("a short quota message after real edits is still suspicious");
        assert!(
            !uncertain.confirmed,
            "files changed, so the rule is not satisfied"
        );
        assert_eq!(
            uncertain.effect,
            Effect::OpenWindow,
            "an uncertain quota must ask the human, never pause on its own"
        );
    }

    #[test]
    fn the_word_quota_in_a_long_successful_log_is_not_a_verdict() {
        let config = builtin();
        let harness = config.harness("opencode").expect("opencode");
        let mut log = String::from("I renamed the quota struct and the rate limit helper.\n");
        for line in 0..60 {
            log.push_str(&format!("line {line} of a long, successful build log\n"));
        }
        assert!(
            classify(
                harness,
                &config.catalog.detection_generic,
                &outcome(&log, Some(0)),
                false,
                0
            )
            .is_none(),
            "a single mention in a long successful run must be ignored"
        );
    }

    #[test]
    fn quota_ignores_exit_code_accepts_short_output_on_success() {
        let config = builtin();
        let harness = config.harness("claude").expect("claude");
        let verdict = classify(
            harness,
            &config.catalog.detection_generic,
            &outcome("You have hit your limit.\n", Some(0)),
            true,
            0,
        )
        .expect("verdict");
        assert!(verdict.confirmed, "short output is enough for this harness");
    }

    #[test]
    fn auth_patterns_win_and_are_labelled() {
        let config = builtin();
        let harness = config.harness("opencode").expect("opencode");
        let verdict = classify(
            harness,
            &config.catalog.detection_generic,
            &outcome("invalid api key", Some(2)),
            false,
            0,
        )
        .expect("verdict");
        assert_eq!(verdict.kind, Kind::Auth);
        assert!(verdict.confirmed);
    }

    #[test]
    fn a_mention_inside_a_long_successful_log_is_not_a_verdict() {
        let config = builtin();
        let harness = config.harness("opencode").expect("opencode");
        let mut log = String::from("rate limit reached\n");
        for line in 0..60 {
            log.push_str(&format!("line {line}\n"));
        }
        // long output + exit 0 + files changed is not a verdict at all
        assert!(classify(
            harness,
            &config.catalog.detection_generic,
            &outcome(&log, Some(0)),
            true,
            0
        )
        .is_none());
    }

    #[test]
    fn the_default_pause_is_one_hour() {
        let verdict = Verdict {
            kind: Kind::Quota,
            confirmed: true,
            evidence: String::new(),
            reset_at: None,
            effect: Effect::Pause,
        };
        assert_eq!(verdict.pause_seconds(1_000), 3_600);
        let with_reset = Verdict {
            reset_at: Some(4_600),
            ..verdict
        };
        assert_eq!(with_reset.pause_seconds(1_000), 3_600);
    }
}
