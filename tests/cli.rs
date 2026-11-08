//! The binary as a user meets it: real arguments, real files, no network and no
//! window ever opened.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::TestDir;

fn invoke(dir: &TestDir, catalog: Option<&Path>, args: &[&str]) -> Output {
    let config_dir = dir.path().join("config");
    let bin = std::env::var("CARGO_BIN_EXE_NEXT_HARNESS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let bin_name = if cfg!(windows) {
                "next-harness.exe"
            } else {
                "next-harness"
            };
            Path::new(&manifest_dir).join("target/debug").join(bin_name)
        });
    let mut command = Command::new(bin);

    // English unless the test asks for something else: the assertions below
    // must not depend on the language of the machine running them.
    if !args.contains(&"--lang") {
        command.args(["--lang", "en"]);
    }
    command
        .args(args)
        .current_dir(dir.path())
        .env("NEXTHARNESS_CONFIG_DIR", &config_dir);
    match catalog {
        Some(path) => command.env("NEXTHARNESS_CATALOG", path),
        None => command.env_remove("NEXTHARNESS_CATALOG"),
    };

    if let Ok(path) = std::env::var("PATH") {
        let bins = PathBuf::from(env!("CARGO_BIN_EXE_fake-harness"));
        let bins = bins.parent().expect("parent");
        let joined = std::env::join_paths(
            std::iter::once(bins.to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .expect("join PATH");
        command.env("PATH", joined);
    }
    command.output().expect("run next-harness")
}

fn next_harness(dir: &TestDir, args: &[&str]) -> Output {
    invoke(dir, None, args)
}

fn next_harness_with_catalog(dir: &TestDir, catalog: &Path, args: &[&str]) -> Output {
    invoke(dir, Some(catalog), args)
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn catalog(dir: &TestDir, entries: &str) -> PathBuf {
    let body = format!(
        "schema = 1\n\n[detection_generic]\n\
         quota = [\"(?i)quota exceeded\", \"(?i)rate limit reached\"]\n\
         auth = [\"(?i)invalid api key\"]\n\n{entries}\n"
    );
    let path = dir.path().join("catalog.toml");
    std::fs::write(&path, body).expect("write catalog");
    path
}

fn entry(id: &str, name: &str, run: &str) -> String {
    format!(
        "[[harness]]\nid = \"{id}\"\nname = \"{name}\"\nmode = \"headless\"\n\
         binaries = [\"fake-harness\"]\nrun = [{run}]\ndefault_tier = \"free\"\n"
    )
}

#[test]
fn the_doctor_report_is_pasteable_and_carries_no_account_name() {
    let dir = TestDir::new("cli-doctor");
    let output = next_harness(&dir, &["doctor", "--report"]);
    assert!(output.status.success(), "{}", text(&output));

    let report = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(report.starts_with("# "), "{report}");
    assert!(report.contains("| id |"), "{report}");
    assert!(report.contains("verified"), "{report}");
    assert!(report.contains("schema 1"), "{report}");

    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = home.to_string_lossy().to_string();
        if home.len() > 3 {
            assert!(
                !report.contains(&home),
                "the report must not contain the home directory: {report}"
            );
        }
    }
    assert!(!report.to_lowercase().contains("api_key="), "{report}");
}

#[test]
fn doctor_without_a_report_prints_the_harness_inventory() {
    let dir = TestDir::new("cli-doctor-text");
    let output = next_harness(&dir, &["doctor"]);
    assert!(output.status.success(), "{}", text(&output));
    let body = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(body.contains("schema 1"), "{body}");
}

#[test]
fn config_shows_where_everything_lives() {
    let dir = TestDir::new("cli-config");
    let output = next_harness(&dir, &["config"]);
    assert!(output.status.success());
    let body = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(body.contains("config"), "{body}");
    assert!(body.contains("state"), "{body}");
}

#[test]
fn status_says_when_nothing_is_paused() {
    let dir = TestDir::new("cli-status");
    let output = next_harness(&dir, &["status"]);
    assert!(output.status.success());
    let body = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(!body.trim().is_empty());
}

#[test]
fn a_run_hands_off_and_leaves_a_handoff() {
    let dir = TestDir::new("cli-run");
    let path = catalog(
        &dir,
        &format!(
            "{}{}",
            entry("first", "First", "\"quota-exit\", \"{prompt}\""),
            entry("second", "Second", "\"success\", \"{prompt}\""),
        ),
    );

    let output = next_harness_with_catalog(&dir, &path, &["run", "rename the module"]);
    assert!(output.status.success(), "{}", text(&output));

    let handoff = dir.path().join(".next-harness").join("HANDOFF.md");
    assert!(
        handoff.is_file(),
        "a handoff must be written when a harness is out of quota"
    );
    let body = std::fs::read_to_string(&handoff).expect("handoff");
    assert!(body.contains("rename the module"), "{body}");

    let status = next_harness_with_catalog(&dir, &path, &["status"]);
    let text = String::from_utf8_lossy(&status.stdout).to_string();
    assert!(text.contains("First"), "status must show the pause: {text}");
    assert!(text.contains("quota"), "status must say why: {text}");
}

#[test]
fn resume_uses_the_saved_task_and_switches_language() {
    let dir = TestDir::new("cli-resume");
    let path = catalog(
        &dir,
        &format!(
            "{}{}",
            entry("first", "First", "\"quota-exit\", \"{prompt}\""),
            entry("second", "Second", "\"success\", \"{prompt}\""),
        ),
    );

    let first = next_harness_with_catalog(&dir, &path, &["run", "improve the parser"]);
    assert!(first.status.success(), "{}", text(&first));

    let second = next_harness_with_catalog(&dir, &path, &["resume", "--lang", "fr"]);
    assert!(second.status.success(), "{}", text(&second));

    // The handoff prompt was used, so the previous attempt is not repeated.
    let body = std::fs::read_to_string(dir.path().join(".next-harness").join("HANDOFF.md"))
        .expect("handoff");
    assert!(body.contains("improve the parser"), "{body}");
}

#[test]
fn resume_without_a_task_explains_what_to_do() {
    let dir = TestDir::new("cli-resume-missing");
    let output = next_harness(&dir, &["resume"]);
    assert!(!output.status.success());
    let body = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(body.contains("next-harness run"), "{body}");
}

#[test]
fn init_writes_the_preferences_file() {
    let dir = TestDir::new("cli-init");
    let path = catalog(&dir, &entry("only", "Only", "\"success\", \"{prompt}\""));
    let output = next_harness_with_catalog(&dir, &path, &["init"]);
    assert!(output.status.success(), "{}", text(&output));

    let prefs = dir.path().join("config").join("config.json");
    assert!(prefs.is_file(), "init must write its preferences");
    let body = std::fs::read_to_string(&prefs).expect("prefs");
    assert!(body.contains("max_attempts"), "{body}");
}

#[test]
fn the_attempt_limit_can_be_raised_from_the_command_line() {
    let dir = TestDir::new("cli-limit");
    let path = catalog(
        &dir,
        &format!(
            "{}{}{}",
            entry("a", "A", "\"quota-exit\", \"{prompt}\""),
            entry("b", "B", "\"quota-exit\", \"{prompt}\""),
            entry("c", "C", "\"success\", \"{prompt}\""),
        ),
    );

    let output = next_harness_with_catalog(&dir, &path, &["run", "do it", "--max-attempts", "3"]);
    assert!(output.status.success(), "{}", text(&output));
}
