//! End-to-end relay tests. They use the repository's own fake harness, so no
//! network, account or real quota is involved, and no dialog can ever appear.

mod common;

use std::path::PathBuf;

use common::TestDir;
use orion_relay::config::Config;
use orion_relay::detector::Detector;
use orion_relay::gatekeeper::{SilentUi, Ui};
use orion_relay::orchestrator::{self, Conclusion, Report, Session, Settings};
use orion_relay::paths::Paths;
use orion_relay::state::State;

fn catalog(entries: &str) -> Config {
    let text = format!(
        "schema = 1\n\n[detection_generic]\n\
         quota = [\"(?i)quota exceeded\", \"(?i)rate limit reached\", \"(?i)usage limit exceeded\"]\n\
         auth = [\"(?i)invalid api key\"]\n\n{entries}\n"
    );
    Config::parse(&text, None, "<test>").expect("test catalog")
}

fn bin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-harness"))
        .parent()
        .expect("parent")
        .to_path_buf()
}

struct House {
    dir: TestDir,
}

impl House {
    fn new(name: &str) -> Self {
        let dir = TestDir::new(name);
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .arg("init")
            .output();
        Self { dir }
    }

    fn paths(&self) -> Paths {
        Paths::under(self.dir.path(), self.dir.path().join("config"))
    }

    fn relay(&self, config: &Config, task: &str, state: &mut State, settings: Settings) -> Report {
        self.relay_with(config, task, state, settings, &SilentUi)
    }

    fn relay_with(
        &self,
        config: &Config,
        task: &str,
        state: &mut State,
        settings: Settings,
        ui: &dyn Ui,
    ) -> Report {
        let paths = self.paths();
        let detections = Detector::with_roots(vec![bin_dir()], vec![]).detect_all(config);
        let session = Session {
            config,
            paths: &paths,
            detections: &detections,
            root: self.dir.path(),
            task,
            ui,
            settings,
        };
        orchestrator::run(&session, state).expect("relay runs")
    }

    fn handoff(&self) -> String {
        std::fs::read_to_string(self.paths().handoff_file).unwrap_or_default()
    }

    fn task_file(&self) -> String {
        std::fs::read_to_string(self.paths().task_file).unwrap_or_default()
    }
}

fn entry(id: &str, name: &str, mode: &str, run: &str, tier: &str) -> String {
    format!(
        "[[harness]]\nid = \"{id}\"\nname = \"{name}\"\nmode = \"{mode}\"\n\
         binaries = [\"fake-harness\"]\nrun = [{run}]\ndefault_tier = \"{tier}\"\n"
    )
}

#[test]
fn a_quota_message_makes_orion_relay_to_the_next_harness() {
    let house = House::new("relay-quota");
    let config = catalog(&format!(
        "{}{}",
        entry(
            "first",
            "First agent",
            "headless",
            "\"quota-exit\", \"{prompt}\"",
            "free"
        ),
        entry(
            "second",
            "Second agent",
            "headless",
            "\"success\", \"{prompt}\"",
            "free"
        ),
    ));

    let mut state = State::default();
    let report = house.relay(
        &config,
        "rename the widget module",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Finished);
    assert_eq!(report.winner(), Some("second"));
    assert_eq!(report.attempts.len(), 2);
    assert!(state.is_paused("first", orion_relay::timefmt::now_epoch()));
    assert!(!state.is_paused("second", orion_relay::timefmt::now_epoch()));

    // The task and the handoff exist, and the handoff tells the next harness not
    // to start over.
    assert!(house.task_file().contains("rename the widget module"));
    let handoff = house.handoff();
    assert!(handoff.contains("First agent"), "{handoff}");
    assert!(handoff.contains("rate limit reached"), "{handoff}");
    assert!(
        handoff.to_lowercase().contains("do not redo"),
        "the handoff must say what not to redo: {handoff}"
    );
}

#[test]
fn a_quota_word_inside_a_long_successful_log_pauses_nothing() {
    let house = House::new("relay-wordy");
    let config = catalog(&entry(
        "wordy",
        "Wordy agent",
        "headless",
        "\"word-in-success\", \"{prompt}\"",
        "free",
    ));

    let mut state = State::default();
    let report = house.relay(
        &config,
        "explain the quota code",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Finished);
    assert!(
        !state.is_paused("wordy", orion_relay::timefmt::now_epoch()),
        "one mention inside a long successful run must not pause a harness"
    );
    assert!(
        house.handoff().is_empty(),
        "no handoff is needed on success"
    );
}

#[test]
fn the_relay_limit_stops_the_walk() {
    let house = House::new("relay-limit");
    let config = catalog(&format!(
        "{}{}{}",
        entry("a", "A", "headless", "\"quota-exit\", \"{prompt}\"", "free"),
        entry("b", "B", "headless", "\"quota-exit\", \"{prompt}\"", "free"),
        entry("c", "C", "headless", "\"success\", \"{prompt}\"", "free"),
    ));

    let mut state = State::default();
    let settings = Settings {
        max_relays: 2,
        manual: false,
    };
    let report = house.relay(&config, "do something", &mut state, settings);

    assert_eq!(report.conclusion, Conclusion::Exhausted);
    assert_eq!(report.attempts.len(), 2);
    assert!(state.is_paused("a", orion_relay::timefmt::now_epoch()));
    assert!(state.is_paused("b", orion_relay::timefmt::now_epoch()));
    assert!(
        !state.is_paused("c", orion_relay::timefmt::now_epoch()),
        "the third harness was never tried"
    );
}

#[test]
fn a_paid_harness_is_never_started_without_consent() {
    let house = House::new("relay-paid");
    let config = catalog(&entry(
        "expensive",
        "Expensive agent",
        "headless",
        "\"success\", \"{prompt}\"",
        "paid",
    ));

    let mut state = State::default();
    let report = house.relay(&config, "do something", &mut state, Settings::default());

    assert_eq!(report.conclusion, Conclusion::Stopped);
    assert!(
        report.attempts.is_empty(),
        "a paid harness must not run without a click"
    );
    assert!(house.task_file().contains("do something"));
}

#[test]
fn pauses_survive_a_restart() {
    let house = House::new("relay-restart");
    let config = catalog(&format!(
        "{}{}",
        entry(
            "first",
            "First",
            "headless",
            "\"quota-exit\", \"{prompt}\"",
            "free"
        ),
        entry(
            "second",
            "Second",
            "headless",
            "\"success\", \"{prompt}\"",
            "free"
        ),
    ));

    let paths = house.paths();
    let mut state = State::default();
    let first = house.relay(&config, "write the docs", &mut state, Settings::default());
    assert_eq!(first.conclusion, Conclusion::Finished);

    // A new process would only have the file.
    let reloaded = State::load(&paths.state_file);
    assert!(reloaded.is_paused("first", orion_relay::timefmt::now_epoch()));

    let mut state = reloaded;
    let second = house.relay(&config, "write the docs", &mut state, Settings::default());
    assert_eq!(second.conclusion, Conclusion::Finished);
    assert_eq!(
        second.attempts.first().map(|attempt| attempt.id.as_str()),
        Some("second"),
        "the paused harness must be skipped after a restart"
    );
}

#[test]
fn the_handoff_masks_what_the_agent_printed() {
    let house = House::new("relay-mask");
    let config = catalog(&format!(
        "{}{}",
        entry(
            "leaky",
            "Leaky",
            "headless",
            "\"quota-leak\", \"{prompt}\"",
            "free"
        ),
        entry(
            "next",
            "Next",
            "headless",
            "\"success\", \"{prompt}\"",
            "free"
        ),
    ));

    let mut state = State::default();
    let report = house.relay(
        &config,
        "fix the key handling",
        &mut state,
        Settings::default(),
    );
    assert_eq!(report.conclusion, Conclusion::Finished);

    let handoff = house.handoff();
    assert!(handoff.contains("rate limit reached"), "{handoff}");
    assert!(
        !handoff.contains("sk-abcdefghijklmnop"),
        "a secret must never be copied into the handoff: {handoff}"
    );

    // Every attempt leaves a transcript, and the transcript is masked too.
    let mut transcripts = 0;
    for entry in std::fs::read_dir(&house.paths().runs_dir).expect("runs directory") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("log") {
            continue;
        }
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            !body.contains("sk-abcdefghijklmnop"),
            "the run log {} leaks a secret",
            path.display()
        );
        transcripts += 1;
    }
    assert!(transcripts >= 2, "each attempt must leave a transcript");
}

#[test]
fn an_interactive_harness_is_only_prepared() {
    let house = House::new("relay-manual");
    let config = catalog(&entry(
        "human",
        "Human-driven",
        "interactive",
        "\"{prompt}\"",
        "free",
    ));

    let mut state = State::default();
    let settings = Settings {
        max_relays: 3,
        manual: true,
    };
    let report = house.relay(&config, "review the pull request", &mut state, settings);

    assert_eq!(report.conclusion, Conclusion::Manual);
    assert_eq!(report.attempts.len(), 1);
    assert_eq!(report.attempts[0].id, "human");
}

#[test]
fn an_interactive_harness_is_ignored_without_manual_mode() {
    let house = House::new("relay-manual-off");
    let config = catalog(&entry(
        "human",
        "Human-driven",
        "interactive",
        "\"{prompt}\"",
        "free",
    ));

    let mut state = State::default();
    let report = house.relay(
        &config,
        "review the pull request",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Exhausted);
    assert!(report.attempts.is_empty());
}
