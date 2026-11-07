use orion_relay::i18n::{parse_keys, sources, translate};

#[test]
fn english_and_french_expose_exactly_the_same_keys() {
    let mut seen: Vec<(&str, Vec<String>)> = Vec::new();

    for (locale, body) in sources() {
        let mut keys = parse_keys(body);
        keys.sort();
        keys.dedup();
        assert!(!keys.is_empty(), "{locale} defines keys");
        seen.push((locale, keys));
    }

    assert_eq!(seen.len(), 2, "two locales ship");
    assert_eq!(
        seen[0].1, seen[1].1,
        "{} and {} must define the same keys",
        seen[0].0, seen[1].0
    );
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
