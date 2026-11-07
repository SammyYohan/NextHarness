mod common;

use common::TestDir;
use orion_relay::state::State;

#[test]
fn pauses_survive_a_reload() {
    let dir = TestDir::new("state-roundtrip");
    let file = dir.join("config/state.json");

    let mut state = State::default();
    state.pause("kilo", 1_000, "quota", Some("rate limit reached"));
    state.save(&file).expect("save");

    let loaded = State::load(&file);
    assert!(loaded.is_paused("kilo", 999));
    assert!(
        !loaded.is_paused("kilo", 1_000),
        "the pause ends at `until`"
    );
    assert_eq!(loaded.next_reset(999), Some(1_000));
    assert_eq!(
        loaded.pause_until("kilo", 999).map(|p| p.kind.as_str()),
        Some("quota")
    );
    assert_eq!(
        loaded
            .pause_until("kilo", 999)
            .and_then(|p| p.detail.as_deref()),
        Some("rate limit reached")
    );
}

#[test]
fn the_save_is_atomic_and_leaves_no_temporary_file() {
    let dir = TestDir::new("state-atomic");
    let file = dir.join("state.json");

    let mut state = State::default();
    state.pause("claude", u64::MAX, "auth", None);
    state.save(&file).expect("save");

    assert!(file.exists());
    assert!(!dir.join("state.json.tmp").exists());

    let body = std::fs::read_to_string(&file).expect("state file");
    assert!(body.contains("\"claude\""));
    assert!(body.contains("\"version\": 1"));
}

#[test]
fn a_missing_or_corrupt_state_file_is_treated_as_empty() {
    let dir = TestDir::new("state-corrupt");

    let missing = State::load(&dir.join("nope.json"));
    assert!(missing.pauses.is_empty());

    let corrupt = dir.write("state.json", "{ not json");
    let recovered = State::load(&corrupt);
    assert!(recovered.pauses.is_empty());
    assert_eq!(recovered.version, 1);
}

#[test]
fn clearing_and_pruning_remove_the_right_pauses() {
    let mut state = State::default();
    state.pause("a", 100, "quota", None);
    state.pause("b", 900, "auth", None);

    state.prune(500);
    assert!(!state.is_paused("a", 500));
    assert!(state.is_paused("b", 500));

    state.clear(Some("b"));
    assert!(state.pauses.is_empty());

    state.pause("c", 900, "quota", None);
    state.clear(None);
    assert!(state.pauses.is_empty());
}
