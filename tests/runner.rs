mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::TestDir;
use next_harness::runner::{run, Params};

fn fake_harness() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-harness"))
}

fn params() -> Params {
    Params {
        inactivity: Duration::from_secs(20),
        absolute: Duration::from_secs(20),
        startup_grace: Duration::ZERO,
        rows: 40,
        cols: 120,
    }
}

#[test]
fn a_successful_run_is_captured_through_the_pseudo_terminal() {
    let dir = TestDir::new("runner-success");
    let outcome = run(
        &fake_harness(),
        &["success".to_string()],
        dir.path(),
        &params(),
    )
    .expect("run");

    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.timed_out);
    assert!(!outcome.stalled);
    assert!(outcome.output.contains("working"), "{}", outcome.output);
    assert!(outcome.output.contains("done"));
    assert!(outcome.succeeded());
}

#[test]
fn a_failing_harness_reports_its_exit_code() {
    let dir = TestDir::new("runner-quota");
    let outcome = run(
        &fake_harness(),
        &["quota-exit".to_string()],
        dir.path(),
        &params(),
    )
    .expect("run");

    assert_eq!(outcome.exit_code, Some(1));
    assert!(outcome.output.contains("rate limit"));
    assert!(outcome.output.contains("quota"));
    assert!(!outcome.succeeded());
}

#[test]
fn a_silent_harness_is_stopped_by_the_startup_budget() {
    let dir = TestDir::new("runner-silent");
    let quiet = Params {
        inactivity: Duration::from_secs(20),
        absolute: Duration::from_secs(60),
        startup_grace: Duration::from_millis(400),
        rows: 40,
        cols: 120,
    };

    let outcome = run(&fake_harness(), &["silent".to_string()], dir.path(), &quiet).expect("run");

    assert!(outcome.stalled, "no output must be reported as a stall");
    assert!(
        outcome.duration < Duration::from_secs(20),
        "the run must not wait for the harness to finish sleeping"
    );
}

#[test]
fn the_absolute_ceiling_kills_a_long_run() {
    let dir = TestDir::new("runner-timeout");
    let short = Params {
        inactivity: Duration::from_secs(60),
        absolute: Duration::from_millis(500),
        startup_grace: Duration::ZERO,
        rows: 40,
        cols: 120,
    };

    let outcome = run(&fake_harness(), &["silent".to_string()], dir.path(), &short).expect("run");

    assert!(outcome.timed_out);
    assert!(outcome.duration < Duration::from_secs(20));
}
