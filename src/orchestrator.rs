//! The per-task loop: walk the catalog in file order, start one harness, decide
//! what its exit means, and hand the task to the next one when the free quota
//! is gone.
//!
//! Nothing here decides on a single signal. A harness is only paused when
//! [`crate::quota::classify`] confirms it, and a paid harness is only started
//! after [`crate::gatekeeper::decide`] recorded a human answer.

use std::path::Path;

use crate::config::{Config, Harness, Mode};
use crate::detector::{Detection, Status};
use crate::error::Result;
use crate::gatekeeper::{self, Answer, Question, Ui};
use crate::git;
use crate::handoff::{self, Handoff};
use crate::i18n;
use crate::paths::Paths;
use crate::quota::{self, Effect};
use crate::runner::{self, Params};
use crate::state::State;
use crate::timefmt::{format_duration, format_utc, now_epoch};

/// How many harnesses one run may try when nothing says otherwise.
pub const DEFAULT_MAX_ATTEMPTS: usize = 3;

/// What this run is allowed to do.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Maximum number of harnesses tried in one run.
    pub max_attempts: usize,
    /// Whether human-driven harnesses may be prepared.
    pub manual: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            manual: false,
        }
    }
}

/// Everything constant for one run.
pub struct Session<'a> {
    /// The merged catalog.
    pub config: &'a Config,
    /// NextHarness's files.
    pub paths: &'a Paths,
    /// Detection results, in catalog order.
    pub detections: &'a [Detection],
    /// Repository root.
    pub root: &'a Path,
    /// The task, as the human wrote it.
    pub task: &'a str,
    /// The surface allowed to ask questions.
    pub ui: &'a dyn Ui,
    /// Limits for this run.
    pub settings: Settings,
}

/// How the run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conclusion {
    /// A harness finished the task.
    Finished,
    /// A human-driven harness was prepared; NextHarness waits for `next-harness resume`.
    Manual,
    /// Every candidate was paused, unusable, or declined.
    Exhausted,
    /// The human cancelled.
    Stopped,
}

/// One attempt, for the run log and for tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    /// Catalog id.
    pub id: String,
    /// Human name.
    pub name: String,
    /// One translated line describing what happened.
    pub note: String,
}

/// The result of one run.
#[derive(Debug, Clone)]
pub struct Report {
    /// How it ended.
    pub conclusion: Conclusion,
    /// Attempts in order.
    pub attempts: Vec<Attempt>,
    /// Earliest pause among the harnesses involved, if any.
    pub paused_until: Option<u64>,
}

impl Report {
    /// The harness that finished the task, if one did.
    pub fn winner(&self) -> Option<&str> {
        match self.conclusion {
            Conclusion::Finished => self.attempts.last().map(|attempt| attempt.id.as_str()),
            _ => None,
        }
    }
}

/// Run the handoff loop once.
pub fn run(session: &Session, state: &mut State) -> Result<Report> {
    handoff::write_task(session.paths, session.task)?;
    state.prune(now_epoch());

    let resuming = session.paths.handoff_file.is_file();
    let limit = session.settings.max_attempts.max(1);
    let mut attempts: Vec<Attempt> = Vec::new();

    for id in session.config.ids() {
        if attempts.len() >= limit {
            break;
        }
        let Some(harness) = session.config.harness(id) else {
            continue;
        };
        let Some(detection) = session.detections.iter().find(|entry| entry.id == id) else {
            continue;
        };

        if !candidate(harness, detection, state, session.settings.manual) {
            continue;
        }

        if harness.mode == Mode::Interactive {
            return Ok(prepare_manual(session, harness, attempts));
        }

        let now = now_epoch();
        let question = Question {
            id: harness.id.clone(),
            name: harness.name.clone(),
            tier: harness.default_tier,
            reset_at: state
                .next_reset(now)
                .or_else(|| state.pause_until(&harness.id, now).map(|pause| pause.until)),
        };
        match gatekeeper::decide(&question, session.ui, now) {
            Answer::Allow => {}
            Answer::Postpone => continue,
            Answer::Abort => {
                return Ok(Report {
                    conclusion: Conclusion::Stopped,
                    attempts,
                    paused_until: state.next_reset(now_epoch()),
                })
            }
        }

        let prompt = if resuming || !attempts.is_empty() {
            i18n::tr("prompt.handoff")
        } else {
            i18n::tr("prompt.task")
        };
        let args = build_args(harness, resuming, &prompt);
        let Some(binary) = detection.binary.clone() else {
            continue;
        };
        let params = Params::default().with_startup_grace(harness.startup_grace_s);

        session.ui.say(&i18n::tr_args(
            "cli.run.starting",
            &[
                ("name", harness.name.clone()),
                ("tier", harness.default_tier.label().to_string()),
                ("attempt", (attempts.len() + 1).to_string()),
            ],
        ));

        let outcome = match runner::run(&binary, &args, session.root, &params) {
            Ok(outcome) => outcome,
            Err(error) => {
                attempts.push(Attempt {
                    id: harness.id.clone(),
                    name: harness.name.clone(),
                    note: error.to_string(),
                });
                continue;
            }
        };

        record(session.paths, harness, &outcome);

        let now = now_epoch();
        let verdict = quota::classify(
            harness,
            &session.config.catalog.detection_generic,
            &outcome,
            git::changed(session.root),
            now,
        );

        if let Some(verdict) = verdict {
            let mut confirmed = verdict.confirmed;
            if !confirmed {
                // One signal only: ask instead of assuming. Without an
                // interface the answer is "no", and the run simply continues
                // without pausing anything.
                session.ui.say(&i18n::tr_args(
                    "cli.run.quota_weak",
                    &[("name", harness.name.clone())],
                ));
                confirmed = session
                    .ui
                    .confirm(&i18n::tr("gate.title"), &verdict.evidence)
                    == Some(true);
            }

            if confirmed && verdict.effect == Effect::Pause {
                let until = now + verdict.pause_seconds(now);
                state.pause(
                    &harness.id,
                    until,
                    verdict.kind.label(),
                    Some(&verdict.evidence),
                );
                state.save(&session.paths.state_file)?;

                let reason = pause_sentence(harness, verdict.kind, until, now);
                session.ui.say(&reason);
                handoff::write(
                    session.paths,
                    session.root,
                    &Handoff {
                        task: session.task,
                        harness,
                        reason: &reason,
                        output: &outcome.output,
                    },
                )?;
                session.ui.say(&i18n::tr_args(
                    "cli.run.handoff",
                    &[("path", session.paths.handoff_file.display().to_string())],
                ));
                attempts.push(Attempt {
                    id: harness.id.clone(),
                    name: harness.name.clone(),
                    note: reason,
                });
                continue;
            }
        }

        if outcome.succeeded() {
            let note = i18n::tr_args("cli.run.finished", &[("name", harness.name.clone())]);
            session.ui.say(&note);
            attempts.push(Attempt {
                id: harness.id.clone(),
                name: harness.name.clone(),
                note,
            });
            return Ok(Report {
                conclusion: Conclusion::Finished,
                attempts,
                paused_until: state.next_reset(now_epoch()),
            });
        }

        let reason = stop_sentence(harness, &outcome);
        session.ui.say(&reason);
        handoff::write(
            session.paths,
            session.root,
            &Handoff {
                task: session.task,
                harness,
                reason: &reason,
                output: &outcome.output,
            },
        )?;
        attempts.push(Attempt {
            id: harness.id.clone(),
            name: harness.name.clone(),
            note: reason,
        });
    }

    let paused_until = state.next_reset(now_epoch());
    if let Some(until) = paused_until {
        session.ui.say(&i18n::tr_args(
            "cli.run.paused_until",
            &[
                ("when", format_utc(until)),
                (
                    "relative",
                    format_duration(until.saturating_sub(now_epoch())),
                ),
            ],
        ));
    }
    session.ui.say(&i18n::tr("cli.run.none_left"));

    Ok(Report {
        conclusion: Conclusion::Exhausted,
        attempts,
        paused_until,
    })
}

/// Keep the transcript of one attempt in `.next-harness/runs`, masked and bounded.
///
/// This is what makes a "blocked" or "out of quota" decision reviewable after
/// the fact; it never contains a credential.
fn record(paths: &Paths, harness: &Harness, outcome: &runner::Outcome) {
    if std::fs::create_dir_all(&paths.runs_dir).is_err() {
        return;
    }
    let file = paths
        .runs_dir
        .join(format!("{}-{}.log", now_epoch(), harness.id));
    let header = format!(
        "# {} ({})\n# exit {:?}, {}s, timed out {}, stalled {}\n\n",
        harness.id,
        harness.name,
        outcome.exit_code,
        outcome.duration.as_secs(),
        outcome.timed_out,
        outcome.stalled
    );
    let body = crate::util::mask_secrets(&outcome.output);
    let tail = crate::util::tail_lines(&body, 400);
    let _ = std::fs::write(file, format!("{header}{tail}\n"));
}

/// Whether a harness may be considered at all.
fn candidate(harness: &Harness, detection: &Detection, state: &State, manual: bool) -> bool {
    if harness.mode == Mode::Gui {
        return false;
    }
    if !harness.enabled_by_default && !manual {
        return false;
    }
    if !matches!(detection.status, Status::Ready | Status::ManualOnly) {
        return false;
    }
    if harness.mode == Mode::Interactive && !manual {
        return false;
    }
    !state.is_paused(&harness.id, now_epoch())
}

/// Expand a run template: `{prompt}` becomes the fixed sentence, `{unattended}`
/// expands only for harnesses that declare unattended arguments.
fn build_args(harness: &Harness, resuming: bool, prompt: &str) -> Vec<String> {
    let template = if resuming && !harness.run_resume.is_empty() {
        &harness.run_resume
    } else {
        &harness.run
    };

    let mut args = Vec::with_capacity(template.len());
    for token in template {
        match token.as_str() {
            "{prompt}" => args.push(prompt.to_string()),
            "{unattended}" => args.extend(harness.unattended_args.iter().cloned()),
            other => args.push(other.to_string()),
        }
    }
    args
}

fn pause_sentence(harness: &Harness, kind: quota::Kind, until: u64, now: u64) -> String {
    let key = match kind {
        quota::Kind::Quota => "cli.run.quota",
        quota::Kind::Auth => "cli.run.auth",
    };
    let head = i18n::tr_args(key, &[("name", harness.name.clone())]);
    let tail = i18n::tr_args(
        "cli.run.paused_until",
        &[
            ("when", format_utc(until)),
            ("relative", format_duration(until.saturating_sub(now))),
        ],
    );
    format!("{head} — {tail}")
}

fn stop_sentence(harness: &Harness, outcome: &runner::Outcome) -> String {
    let seconds = outcome.duration.as_secs().to_string();
    if outcome.stalled {
        i18n::tr_args(
            "cli.run.blocked",
            &[("name", harness.name.clone()), ("seconds", seconds)],
        )
    } else if outcome.timed_out {
        i18n::tr_args(
            "cli.run.timed_out",
            &[("name", harness.name.clone()), ("seconds", seconds)],
        )
    } else {
        i18n::tr_args(
            "cli.run.crashed",
            &[
                ("name", harness.name.clone()),
                (
                    "code",
                    outcome
                        .exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "?".to_string()),
                ),
            ],
        )
    }
}

/// Human-driven harness: put the prompt on the clipboard, say what to do, and
/// never touch its interface.
fn prepare_manual(session: &Session, harness: &Harness, mut attempts: Vec<Attempt>) -> Report {
    let resuming = session.paths.handoff_file.is_file();
    let prompt = harness.manual_prompt.clone().unwrap_or_else(|| {
        if resuming {
            i18n::tr("prompt.handoff")
        } else {
            i18n::tr("prompt.task")
        }
    });

    let copied = copy_to_clipboard(&prompt);
    session.ui.say(&i18n::tr_args(
        "cli.run.manual",
        &[("name", harness.name.clone())],
    ));
    session.ui.say(&i18n::tr_args(
        "manual.open",
        &[("name", harness.name.clone())],
    ));
    if copied {
        session.ui.say(&i18n::tr("cli.run.manual_prompt_copied"));
    }
    session.ui.say(&i18n::tr_args(
        "manual.press_enter",
        &[("name", harness.name.clone())],
    ));
    session.ui.say(&i18n::tr("manual.resume_hint"));

    if session.ui.interactive() {
        let _ = gatekeeper::wait_for_key();
    }

    attempts.push(Attempt {
        id: harness.id.clone(),
        name: harness.name.clone(),
        note: i18n::tr_args("cli.run.manual", &[("name", harness.name.clone())]),
    });

    Report {
        conclusion: Conclusion::Manual,
        attempts,
        paused_until: None,
    }
}

fn copy_to_clipboard(text: &str) -> bool {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
        .is_ok()
}
