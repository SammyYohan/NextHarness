mod common;

use common::TestDir;
use next_harness::config::{Config, Mode, Tier, Verified, BUILTIN};

#[test]
fn the_builtin_catalog_loads_without_warnings() {
    let config = Config::builtin().expect("the compiled-in catalog must load");

    assert_eq!(config.catalog.schema, 1);
    assert_eq!(config.catalog.catalog_date.as_deref(), Some("2026-11-06"));
    assert_eq!(config.source, "<builtin>");
    assert!(config.override_files.is_empty());
    assert!(config.warnings.is_empty(), "{:?}", config.warnings);

    let harnesses = &config.catalog.harnesses;
    assert_eq!(harnesses.len(), 26, "the catalog ships 26 entries");

    let mut ids: Vec<&str> = harnesses.iter().map(|h| h.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 26, "every id is unique");

    assert_eq!(config.catalog.detect_search_dirs.windows.len(), 7);
    assert_eq!(config.catalog.detect_search_dirs.unix.len(), 10);
    assert_eq!(config.catalog.detection_generic.quota.len(), 9);
    assert_eq!(config.catalog.detection_generic.auth.len(), 6);
    assert_eq!(config.catalog.detection_generic.reset.len(), 3);
}

#[test]
fn every_headless_entry_can_actually_be_started() {
    let config = Config::builtin().expect("builtin catalog");
    for harness in config.catalog.harnesses.iter().filter(|h| h.is_headless()) {
        assert!(
            harness.run.iter().any(|arg| arg == "{prompt}"),
            "{} has no {{prompt}}",
            harness.id
        );
        assert!(!harness.binaries.is_empty(), "{} has no binary", harness.id);
    }

    let gui: Vec<&str> = config
        .catalog
        .harnesses
        .iter()
        .filter(|h| h.mode == Mode::Gui)
        .map(|h| h.id.as_str())
        .collect();
    assert_eq!(
        gui,
        vec!["antigravity-app", "freebuff-desktop", "cursor-app"]
    );
}

#[test]
fn unknown_tier_is_paid_by_default() {
    assert_eq!(Tier::default(), Tier::Unknown);
    assert!(Tier::Unknown.requires_consent());
    assert!(Tier::Paid.requires_consent());
    assert!(!Tier::Free.requires_consent());
    assert_eq!(Verified::default(), Verified::Unverified);
}

#[test]
fn overrides_patch_by_id_and_append_new_harnesses() {
    let dir = TestDir::new("config-override");
    dir.write(
        "profiles.d/50-local.toml",
        r#"
[[harness]]
id = "opencode"
run = ["run", "--model", "free", "{prompt}"]
unattended_args = ["--dangerously-skip-permissions", "--no-telemetry"]

[[harness]]
id = "my-agent"
name = "My Agent"
mode = "headless"
binaries = ["my-agent"]
run = ["--prompt", "{prompt}"]
default_tier = "free"
enabled_by_default = true
verified = "unverified"
"#,
    );

    let config = Config::parse(BUILTIN, Some(&dir.join("profiles.d")), "test")
        .expect("merged catalog must load");

    assert_eq!(config.catalog.harnesses.len(), 27);
    assert_eq!(config.override_files.len(), 1);

    let opencode = config.harness("opencode").expect("opencode survives");
    assert_eq!(opencode.run, vec!["run", "--model", "free", "{prompt}"]);
    assert_eq!(opencode.unattended_args.len(), 2);
    assert_eq!(
        opencode.default_tier,
        Tier::Free,
        "untouched fields survive"
    );
    assert_eq!(opencode.verified, Verified::Official);
    assert_eq!(opencode.binaries, vec!["opencode"]);

    let custom = config.harness("my-agent").expect("new harness is appended");
    assert_eq!(custom.name, "My Agent");
    assert_eq!(config.ids().last().copied(), Some("my-agent"));
}

#[test]
fn overriding_a_nested_table_does_not_drop_siblings() {
    let dir = TestDir::new("config-nested");
    dir.write(
        "profiles.d/10-patterns.toml",
        r#"
[detection_generic]
quota = ["(?i)quota exhausted"]
"#,
    );

    let config =
        Config::parse(BUILTIN, Some(&dir.join("profiles.d")), "test").expect("merged catalog");

    assert_eq!(
        config.catalog.detection_generic.quota,
        vec!["(?i)quota exhausted".to_string()]
    );
    assert_eq!(
        config.catalog.detection_generic.auth.len(),
        6,
        "auth patterns are untouched by a quota-only override"
    );
}

#[test]
fn a_headless_harness_without_prompt_is_rejected() {
    let catalog = r#"
schema = 1

[[harness]]
id = "no-prompt"
name = "No Prompt"
mode = "headless"
binaries = ["nope"]
run = ["go"]
"#;

    let error = Config::parse(catalog, None, "test").expect_err("must be rejected");
    let message = error.to_string();
    assert!(message.contains("{prompt}"), "{message}");
}

#[test]
fn a_pattern_that_does_not_compile_is_rejected() {
    let catalog = r#"
schema = 1

[detection_generic]
quota = ["(?i)unclosed("]

[[harness]]
id = "x"
name = "X"
mode = "headless"
binaries = ["x"]
run = ["{prompt}"]
"#;

    let error = Config::parse(catalog, None, "test").expect_err("must be rejected");
    let message = error.to_string();
    assert!(message.contains("does not compile"), "{message}");
}

#[test]
fn a_catalog_without_patterns_warns_instead_of_failing() {
    let catalog = r#"
schema = 1

[[harness]]
id = "x"
name = "X"
mode = "headless"
binaries = ["x"]
run = ["{prompt}"]
"#;

    let config = Config::parse(catalog, None, "test").expect("loads");
    assert!(
        config
            .warnings
            .iter()
            .any(|warning| warning.contains("no quota or auth patterns")),
        "{:?}",
        config.warnings
    );
}
