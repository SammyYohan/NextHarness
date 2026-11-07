mod common;

use common::{write_script, TestDir};
use orion_relay::config::Config;
use orion_relay::detector::{Detector, Platform, Status};

fn catalog(body: &str) -> Config {
    Config::parse(body, None, "test").expect("test catalog loads")
}

const HEADLESS: &str = r#"
schema = 1

[[harness]]
id = "fake"
name = "Fake"
mode = "headless"
binaries = ["fake-harness"]
version_args = ["--version"]
run = ["run", "{prompt}"]
default_tier = "free"
enabled_by_default = true
verified = "unverified"
"#;

#[test]
fn a_binary_on_the_search_path_is_ready_and_versioned() {
    let dir = TestDir::new("detect-ready");
    let bin = dir.join("bin");
    write_script(&bin, "fake-harness", &["1.2.3"], 0);

    let config = catalog(HEADLESS);
    let detector = Detector::with_roots(vec![bin], Vec::new());
    let detection = detector.detect(config.harness("fake").expect("harness"));

    assert_eq!(detection.status, Status::Ready);
    assert_eq!(detection.version.as_deref(), Some("1.2.3"));
    assert!(detection.binary.is_some());
}

#[test]
fn a_missing_binary_is_not_installed() {
    let dir = TestDir::new("detect-missing");
    let config = catalog(HEADLESS);
    let detector = Detector::with_roots(vec![dir.join("empty")], Vec::new());
    let detection = detector.detect(config.harness("fake").expect("harness"));

    assert_eq!(detection.status, Status::NotInstalled);
    assert!(detection.detail.contains("not found"));
    assert!(detection.binary.is_none());
}

#[test]
fn a_binary_that_fails_its_version_probe_is_rejected() {
    let dir = TestDir::new("detect-bad-version");
    let bin = dir.join("bin");
    write_script(&bin, "fake-harness", &["not a version"], 3);

    let config = catalog(HEADLESS);
    let detector = Detector::with_roots(vec![bin], Vec::new());
    let detection = detector.detect(config.harness("fake").expect("harness"));

    assert_eq!(
        detection.status,
        Status::NotInstalled,
        "a non-zero --version must not be trusted"
    );
    assert!(detection.detail.contains("version probe"));
}

#[test]
fn declared_credentials_are_required() {
    let dir = TestDir::new("detect-env");
    let bin = dir.join("bin");
    write_script(&bin, "fake-harness", &["1.2.3"], 0);

    let body = format!("{HEADLESS}requires_env = [\"ORION_TEST_ABSENT_KEY\"]\n");
    let config = catalog(&body);
    let detector = Detector::with_roots(vec![bin], Vec::new());
    let detection = detector.detect(config.harness("fake").expect("harness"));

    assert_eq!(detection.status, Status::MissingKey);
    assert!(detection.detail.contains("ORION_TEST_ABSENT_KEY"));
}

#[test]
fn companion_binaries_must_exist_too() {
    let dir = TestDir::new("detect-companion");
    let bin = dir.join("bin");
    write_script(&bin, "fake-harness", &["1.2.3"], 0);

    let body = format!("{HEADLESS}companion_binaries = [\"missing-companion\"]\n");
    let config = catalog(&body);
    let detector = Detector::with_roots(vec![bin], Vec::new());
    let detection = detector.detect(config.harness("fake").expect("harness"));

    assert_eq!(detection.status, Status::NotInstalled);
    assert!(detection.detail.contains("companion"));
}

#[test]
fn platform_restricted_harnesses_are_not_reported_as_installed() {
    if Platform::current() != Platform::Windows {
        return;
    }
    let dir = TestDir::new("detect-platform");
    let bin = dir.join("bin");
    write_script(&bin, "fake-harness", &["1.2.3"], 0);

    let body = format!("{HEADLESS}platforms = [\"linux\", \"macos\"]\n");
    let config = catalog(&body);
    let detector = Detector::with_roots(vec![bin], Vec::new());
    let detection = detector.detect(config.harness("fake").expect("harness"));

    assert_eq!(detection.status, Status::NotInstalled);
    assert!(detection.detail.starts_with("needs "));
}

#[test]
fn an_interactive_harness_is_manual_only() {
    let dir = TestDir::new("detect-interactive");
    let bin = dir.join("bin");
    write_script(&bin, "fake-tui", &["1.0.0"], 0);

    let body = r#"
schema = 1

[[harness]]
id = "tui"
name = "TUI"
mode = "interactive"
binaries = ["fake-tui"]
version_args = ["--version"]
manual_prompt = "Read .orion/HANDOFF.md and continue the task."
default_tier = "free"
enabled_by_default = true
verified = "official"
"#;
    let config = catalog(body);
    let detector = Detector::with_roots(vec![bin], Vec::new());
    let detection = detector.detect(config.harness("tui").expect("harness"));

    assert_eq!(detection.status, Status::ManualOnly);
    assert!(detection.status.is_usable());
}

#[test]
fn gui_applications_are_found_by_keyword() {
    let dir = TestDir::new("detect-gui");
    let apps = dir.join("Programs");
    std::fs::create_dir_all(apps.join("Antigravity").join("bin")).expect("app tree");
    std::fs::write(apps.join("Antigravity").join("Antigravity.exe"), b"x").expect("app file");

    let body = r#"
schema = 1

[[harness]]
id = "app"
name = "App"
mode = "gui"
gui_keywords = ["antigravity"]
manual_prompt = "Read .orion/HANDOFF.md and continue the task."
default_tier = "free"
enabled_by_default = true
verified = "unverified"
"#;
    let config = catalog(body);
    let detector = Detector::with_roots(Vec::new(), vec![apps]);
    let detection = detector.detect(config.harness("app").expect("harness"));

    assert_eq!(detection.status, Status::Ready);
    assert!(detection.detail.to_lowercase().contains("antigravity"));

    let absent = Detector::with_roots(Vec::new(), vec![dir.join("nowhere")]);
    assert_eq!(
        absent
            .detect(config.harness("app").expect("harness"))
            .status,
        Status::NotInstalled
    );
}

#[test]
fn two_entries_sharing_an_install_hint_are_one_product() {
    let dir = TestDir::new("detect-hint");
    let bin = dir.join("bin");
    write_script(&bin, "fake-harness", &["1.2.3"], 0);

    let body = r#"
schema = 1

[[harness]]
id = "kilo"
name = "Kilo"
mode = "headless"
binaries = ["fake-harness"]
version_args = ["--version"]
run = ["run", "{prompt}"]
default_tier = "free"
enabled_by_default = true
verified = "official"
install_hint = "npm install -g kilo"

[[harness]]
id = "kilo-legacy"
name = "Kilo Legacy"
mode = "headless"
binaries = ["fake-harness"]
version_args = ["--version"]
run = ["{prompt}"]
default_tier = "free"
enabled_by_default = true
verified = "official"
install_hint = "npm install -g kilo"
"#;
    let config = catalog(body);
    let detector = Detector::with_roots(vec![bin], Vec::new());
    let detections = detector.detect_all(&config);

    assert_eq!(detections[0].status, Status::Ready);
    assert_eq!(detections[1].status, Status::NotInstalled);
    assert!(detections[1].detail.contains("superseded by `kilo`"));
}
