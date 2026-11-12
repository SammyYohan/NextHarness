//! End-to-end handoff tests. They use the repository's own fake harness, so no
//! network, account or real quota is involved, and no dialog can ever appear.

mod common;

use std::path::PathBuf;

use common::TestDir;
use next_harness::config::Config;
use next_harness::detector::Detector;
use next_harness::gatekeeper::{SilentUi, Ui};
use next_harness::orchestrator::{self, Conclusion, Report, Session, Settings};
use next_harness::paths::Paths;
use next_harness::state::State;

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

    fn run_task(
        &self,
        config: &Config,
        task: &str,
        state: &mut State,
        settings: Settings,
    ) -> Report {
        self.run_task_with(config, task, state, settings, &SilentUi)
    }

    fn run_task_with(
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
        orchestrator::run(&session, state).expect("the run finishes")
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
fn a_quota_message_hands_the_task_to_the_next_harness() {
    let house = House::new("handoff-quota");
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
    let report = house.run_task(
        &config,
        "rename the widget module",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Finished);
    assert_eq!(report.winner(), Some("second"));
    assert_eq!(report.attempts.len(), 2);
    assert!(state.is_paused("first", next_harness::timefmt::now_epoch()));
    assert!(!state.is_paused("second", next_harness::timefmt::now_epoch()));

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
    let house = House::new("handoff-wordy");
    let config = catalog(&entry(
        "wordy",
        "Wordy agent",
        "headless",
        "\"word-in-success\", \"{prompt}\"",
        "free",
    ));

    let mut state = State::default();
    let report = house.run_task(
        &config,
        "explain the quota code",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Finished);
    assert!(
        !state.is_paused("wordy", next_harness::timefmt::now_epoch()),
        "one mention inside a long successful run must not pause a harness"
    );
    assert!(
        house.handoff().is_empty(),
        "no handoff is needed on success"
    );
}

#[test]
fn the_attempt_limit_stops_the_walk() {
    let house = House::new("handoff-limit");
    let config = catalog(&format!(
        "{}{}{}",
        entry("a", "A", "headless", "\"quota-exit\", \"{prompt}\"", "free"),
        entry("b", "B", "headless", "\"quota-exit\", \"{prompt}\"", "free"),
        entry("c", "C", "headless", "\"success\", \"{prompt}\"", "free"),
    ));

    let mut state = State::default();
    let settings = Settings {
        max_attempts: 2,
        manual: false,
        order: None,
        priority: Vec::new(),
    };
    let report = house.run_task(&config, "do something", &mut state, settings);

    assert_eq!(report.conclusion, Conclusion::Exhausted);
    assert_eq!(report.attempts.len(), 2);
    assert!(state.is_paused("a", next_harness::timefmt::now_epoch()));
    assert!(state.is_paused("b", next_harness::timefmt::now_epoch()));
    assert!(
        !state.is_paused("c", next_harness::timefmt::now_epoch()),
        "the third harness was never tried"
    );
}

#[test]
fn a_paid_harness_is_never_started_without_consent() {
    let house = House::new("handoff-paid");
    let config = catalog(&entry(
        "expensive",
        "Expensive agent",
        "headless",
        "\"success\", \"{prompt}\"",
        "paid",
    ));

    let mut state = State::default();
    let report = house.run_task(&config, "do something", &mut state, Settings::default());

    assert_eq!(report.conclusion, Conclusion::Stopped);
    assert!(
        report.attempts.is_empty(),
        "a paid harness must not run without a click"
    );
    assert!(house.task_file().contains("do something"));
}

#[test]
fn pauses_survive_a_restart() {
    let house = House::new("handoff-restart");
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
    let first = house.run_task(&config, "write the docs", &mut state, Settings::default());
    assert_eq!(first.conclusion, Conclusion::Finished);

    // A new process would only have the file.
    let reloaded = State::load(&paths.state_file);
    assert!(reloaded.is_paused("first", next_harness::timefmt::now_epoch()));

    let mut state = reloaded;
    let second = house.run_task(&config, "write the docs", &mut state, Settings::default());
    assert_eq!(second.conclusion, Conclusion::Finished);
    assert_eq!(
        second.attempts.first().map(|attempt| attempt.id.as_str()),
        Some("second"),
        "the paused harness must be skipped after a restart"
    );
}

#[test]
fn the_handoff_masks_what_the_agent_printed() {
    let house = House::new("handoff-mask");
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
    let report = house.run_task(
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
    let house = House::new("handoff-manual");
    let config = catalog(&entry(
        "human",
        "Human-driven",
        "interactive",
        "\"{prompt}\"",
        "free",
    ));

    let mut state = State::default();
    let settings = Settings {
        max_attempts: 3,
        manual: true,
        order: None,
        priority: Vec::new(),
    };
    let report = house.run_task(&config, "review the pull request", &mut state, settings);

    assert_eq!(report.conclusion, Conclusion::Manual);
    assert_eq!(report.attempts.len(), 1);
    assert_eq!(report.attempts[0].id, "human");
}

#[test]
fn an_interactive_harness_is_ignored_without_manual_mode() {
    let house = House::new("handoff-manual-off");
    let config = catalog(&entry(
        "human",
        "Human-driven",
        "interactive",
        "\"{prompt}\"",
        "free",
    ));

    let mut state = State::default();
    let report = house.run_task(
        &config,
        "review the pull request",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Exhausted);
    assert!(report.attempts.is_empty());
}

#[test]
fn checklist_is_created_and_passed_in_handoff() {
    let house = House::new("handoff-checklist");
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

    let mut state = State::default();
    let report = house.run_task(
        &config,
        "implement user profile feature",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Finished);
    let handoff = house.handoff();
    assert!(handoff.contains("implement user profile feature"), "{handoff}");
    assert!(handoff.contains("- [ ]"), "{handoff}");
    assert!(house.paths().checklist_file.is_file());
}

#[test]
fn configured_order_overrides_catalog_order() {
    let house = House::new("handoff-order");
    let config = catalog(&format!(
        "{}{}",
        entry(
            "first",
            "First",
            "headless",
            "\"success\", \"{prompt}\"",
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

    let mut state = State::default();
    let settings = Settings {
        max_attempts: 1,
        manual: false,
        order: Some(vec!["second".to_string()]),
        priority: Vec::new(),
    };
    let report = house.run_task(
        &config,
        "build feature",
        &mut state,
        settings,
    );

    assert_eq!(report.conclusion, Conclusion::Finished);
    assert_eq!(
        report.winner(),
        Some("second"),
        "second must run first when requested by order"
    );
}

#[test]
fn diagnostic_detects_interrupted_syntax_issue_in_handoff() {
    let house = House::new("handoff-diagnostic");
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

    // Create a broken json file in repository as if an agent was interrupted mid-write
    let broken_file = house.dir.path().join("broken.json");
    std::fs::write(&broken_file, "{\n  \"incomplete\": true,\n").unwrap();

    let mut state = State::default();
    let report = house.run_task(
        &config,
        "fix database migration",
        &mut state,
        Settings::default(),
    );

    assert_eq!(report.conclusion, Conclusion::Finished);
    let handoff = house.handoff();
    assert!(handoff.contains("broken.json"), "{handoff}");
    assert!(handoff.contains("json-syntax"), "{handoff}");
}
