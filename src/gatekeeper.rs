//! The gatekeeper: the only place where a run may spend money, and only after a
//! human said yes.
//!
//! Order of preference is deliberate. A terminal gets a three-way prompt (pay,
//! wait, cancel) preceded by a bell, because the whole point is to interrupt
//! whoever walked away. A desktop session gets a native yes/no dialog. With
//! neither, Orion waits for the announced reset or stops: it never spends
//! because nobody was there to be asked.

use std::io::IsTerminal;

use native_dialog::{MessageDialogBuilder, MessageLevel};

use crate::config::Tier;
use crate::i18n;
use crate::timefmt::format_utc;

/// What the human decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Spend, explicitly allowed.
    Allow,
    /// Do not spend now; try again after the reset.
    Postpone,
    /// Do not spend, and do not keep going.
    Abort,
}

/// The question being asked.
#[derive(Debug, Clone)]
pub struct Question {
    /// Harness id.
    pub id: String,
    /// Human name.
    pub name: String,
    /// Its price tier: anything but `Free` needs consent.
    pub tier: Tier,
    /// Reset time, when a pause is in effect.
    pub reset_at: Option<u64>,
}

/// The surface that asks. Injected so the tests can prove what happens when
/// nothing can answer.
pub trait Ui {
    /// Whether a human is reachable at a terminal.
    fn interactive(&self) -> bool;
    /// Native yes/no. `None` means no dialog could be shown.
    fn confirm(&self, title: &str, text: &str) -> Option<bool>;
    /// Numbered choices at the terminal. `None` means no answer.
    fn choose(&self, text: &str, options: &[String]) -> Option<usize>;
    /// Tell the human something without asking anything.
    fn say(&self, text: &str);
}

/// Decide whether `question`'s harness may run.
pub fn decide(question: &Question, ui: &dyn Ui, now: u64) -> Answer {
    if !question.tier.requires_consent() {
        return Answer::Allow;
    }

    let title = i18n::tr("gate.title");
    let when = question
        .reset_at
        .map(format_utc)
        .unwrap_or_else(|| i18n::tr("harness.status.unknown"));
    let body = format!(
        "{}\n\n{}: {}",
        i18n::tr("gate.body"),
        question.name,
        question.tier.label()
    );

    if ui.interactive() {
        let options = vec![
            i18n::tr_args("gate.pay", &[("name", question.name.clone())]),
            i18n::tr_args("gate.wait", &[("when", when.clone())]),
            i18n::tr("gate.cancel"),
        ];
        // The bell is the point: this question is asked while nobody is looking.
        print!("\u{7}");
        return match ui.choose(&format!("{body}\n"), &options) {
            Some(0) => Answer::Allow,
            Some(1) => Answer::Postpone,
            _ => Answer::Abort,
        };
    }

    if let Some(agreed) = ui.confirm(&title, &body) {
        if agreed {
            return Answer::Allow;
        }
        return if question.reset_at.is_some() {
            Answer::Postpone
        } else {
            Answer::Abort
        };
    }

    ui.say(&i18n::tr("gate.no_ui"));
    let _ = now;
    if question.reset_at.is_some() {
        Answer::Postpone
    } else {
        Answer::Abort
    }
}

/// The real surfaces of this machine.
#[derive(Debug, Clone)]
pub struct SystemUi {
    main_thread: std::thread::ThreadId,
    gui: bool,
}

impl Default for SystemUi {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemUi {
    /// Capture the calling thread as the main thread. macOS only allows a dialog
    /// there, so this must be built early, from `main`.
    pub fn new() -> Self {
        Self {
            main_thread: std::thread::current().id(),
            gui: has_desktop(),
        }
    }

    /// Whether a graphical session exists at all.
    pub fn desktop(&self) -> bool {
        self.gui
    }
}

/// True when a window could plausibly be shown.
fn has_desktop() -> bool {
    if cfg!(windows) || cfg!(target_os = "macos") {
        true
    } else {
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
    }
}

impl Ui for SystemUi {
    fn interactive(&self) -> bool {
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
    }

    fn confirm(&self, title: &str, text: &str) -> Option<bool> {
        // A dialog off the main thread is not allowed on macOS, and asking from a
        // worker thread elsewhere would block the wrong thing.
        if !self.gui || std::thread::current().id() != self.main_thread {
            return None;
        }
        MessageDialogBuilder::default()
            .set_title(title)
            .set_text(text)
            .set_level(MessageLevel::Warning)
            .confirm()
            .show()
            .ok()
    }

    fn choose(&self, text: &str, options: &[String]) -> Option<usize> {
        println!("{text}");
        for (index, option) in options.iter().enumerate() {
            println!("  {}) {option}", index + 1);
        }
        let answer = crate::gatekeeper::read_key()?;
        let digit = answer.to_digit(10)? as usize;
        if digit >= 1 && digit <= options.len() {
            Some(digit - 1)
        } else {
            None
        }
    }

    fn say(&self, text: &str) {
        println!("{text}");
    }
}

/// A terminal keystroke, without waiting for Enter and without leaving the
/// terminal in raw mode.
fn read_key() -> Option<char> {
    use crossterm::event::{Event, KeyCode, KeyEventKind};

    crossterm::terminal::enable_raw_mode().ok()?;
    let mut answer = None;
    while let Ok(event) = crossterm::event::read() {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            answer = match key.code {
                KeyCode::Char(character) => Some(character),
                KeyCode::Esc => None,
                KeyCode::Enter => Some(' '),
                _ => continue,
            };
            break;
        }
    }
    let _ = crossterm::terminal::disable_raw_mode();
    answer
}

/// Waits for any keystroke, used by `orion resume` and manual mode.
pub fn wait_for_key() -> bool {
    use crossterm::event::{Event, KeyCode, KeyEventKind};

    if crossterm::terminal::enable_raw_mode().is_err() {
        return false;
    }
    let mut pressed = false;
    while let Ok(event) = crossterm::event::read() {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if matches!(key.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ')) {
                pressed = true;
            }
            break;
        }
    }
    let _ = crossterm::terminal::disable_raw_mode();
    pressed
}

/// A surface that can never answer: the fallback used by unattended runs and by
/// the test suite, where a dialog must never appear.
#[derive(Debug, Clone, Copy, Default)]
pub struct SilentUi;

impl Ui for SilentUi {
    fn interactive(&self) -> bool {
        false
    }

    fn confirm(&self, _title: &str, _text: &str) -> Option<bool> {
        None
    }

    fn choose(&self, _text: &str, _options: &[String]) -> Option<usize> {
        None
    }

    fn say(&self, _text: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(tier: Tier, reset_at: Option<u64>) -> Question {
        Question {
            id: "h".to_string(),
            name: "Harness".to_string(),
            tier,
            reset_at,
        }
    }

    #[test]
    fn a_free_harness_needs_no_permission() {
        assert_eq!(
            decide(&question(Tier::Free, None), &SilentUi, 0),
            Answer::Allow
        );
    }

    #[test]
    fn without_any_interface_a_paid_harness_is_never_started() {
        for tier in [Tier::Paid, Tier::Unknown] {
            assert_ne!(
                decide(&question(tier, None), &SilentUi, 0),
                Answer::Allow,
                "{tier:?} must never be spent without consent"
            );
            assert_eq!(
                decide(&question(tier, Some(3_600)), &SilentUi, 0),
                Answer::Postpone,
                "a known reset means waiting, not stopping"
            );
        }
    }

    struct Yes;
    impl Ui for Yes {
        fn interactive(&self) -> bool {
            false
        }
        fn confirm(&self, _title: &str, _text: &str) -> Option<bool> {
            Some(true)
        }
        fn choose(&self, _text: &str, _options: &[String]) -> Option<usize> {
            None
        }
        fn say(&self, _text: &str) {}
    }

    #[test]
    fn an_explicit_yes_is_the_only_way_to_spend() {
        assert_eq!(
            decide(&question(Tier::Unknown, None), &Yes, 0),
            Answer::Allow
        );
    }

    #[test]
    fn terminal_choices_map_to_pay_wait_and_cancel() {
        struct Fixed(Option<usize>);
        impl Ui for Fixed {
            fn interactive(&self) -> bool {
                true
            }
            fn confirm(&self, _title: &str, _text: &str) -> Option<bool> {
                None
            }
            fn choose(&self, _text: &str, _options: &[String]) -> Option<usize> {
                self.0
            }
            fn say(&self, _text: &str) {}
        }
        assert_eq!(
            decide(&question(Tier::Paid, None), &Fixed(Some(0)), 0),
            Answer::Allow
        );
        assert_eq!(
            decide(&question(Tier::Paid, None), &Fixed(Some(1)), 0),
            Answer::Postpone
        );
        assert_eq!(
            decide(&question(Tier::Paid, None), &Fixed(Some(2)), 0),
            Answer::Abort
        );
        assert_eq!(
            decide(&question(Tier::Paid, None), &Fixed(None), 0),
            Answer::Abort
        );
    }
}
