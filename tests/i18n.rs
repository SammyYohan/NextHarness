use next_harness::i18n::{parse_keys, sources, translate};

#[test]
fn all_locales_expose_exactly_the_same_keys() {
    let mut seen: Vec<(&str, Vec<String>)> = Vec::new();

    for (locale, body) in sources() {
        let mut keys = parse_keys(body);
        keys.sort();
        keys.dedup();
        assert!(!keys.is_empty(), "{locale} defines keys");
        seen.push((locale, keys));
    }

    assert!(seen.len() >= 2, "at least two locales ship");

    let baseline = seen[0].1.clone();
    for (locale, keys) in &seen[1..] {
        assert_eq!(
            baseline, *keys,
            "en and {} must define the same keys",
            locale
        );
    }
}

#[test]
fn every_key_resolves_to_text_and_not_to_its_own_name() {
    for (locale, body) in sources() {
        for key in parse_keys(body) {
            let text = translate(locale, &key);
            assert!(!text.is_empty(), "{locale}/{key} is empty");
            assert_ne!(
                text, key,
                "{locale}/{key} is missing from the compiled locales"
            );
        }
    }
}

#[test]
fn the_two_locales_really_differ() {
    assert_eq!(translate("en", "gate.title"), "Free quota exhausted");
    assert_eq!(translate("fr", "gate.title"), "Quota gratuit épuisé");
}
