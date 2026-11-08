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
fn every_locale_keeps_the_same_placeholders_in_each_message() {
    let mut baseline: Option<Vec<(String, Vec<String>)>> = None;

    for (locale, body) in sources() {
        let slots_per_key: Vec<(String, Vec<String>)> = parse_keys(body)
            .into_iter()
            .map(|key| {
                let text = translate(locale, &key);
                let mut slots: Vec<String> = text
                    .split("%{")
                    .skip(1)
                    .map(|rest| {
                        rest.split('}')
                            .next()
                            .unwrap_or_default()
                            .trim()
                            .to_string()
                    })
                    .filter(|name| !name.is_empty())
                    .collect();
                slots.sort();
                slots.dedup();
                (key, slots)
            })
            .collect();

        match &baseline {
            None => baseline = Some(slots_per_key),
            Some(previous) => assert_eq!(
                previous, &slots_per_key,
                "en and {locale} disagree on which placeholders each message uses"
            ),
        }
    }

    assert!(baseline.is_some(), "en is the baseline");
}

#[test]
fn locales_really_differ() {
    assert_eq!(translate("en", "gate.title"), "Free quota exhausted");
    assert_eq!(translate("fr", "gate.title"), "Quota gratuit épuisé");
}
