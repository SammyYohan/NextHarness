//! Language selection and the locale-parity check.
//!
//! Translations live in `locales/*.yml` and are compiled into the binary by
//! `rust-i18n`. A test keeps the two files in lockstep: a key that exists in one
//! and not the other is a bug, not a fallback.

/// Locales shipped with this build, most specific first is not needed because
/// [`normalize`] reduces any tag to one of these.
pub const LOCALES: [&str; 8] = ["en", "fr", "es", "de", "it", "pt", "zh", "ja"];

/// Fallback locale used when nothing else matches.
pub const DEFAULT_LOCALE: &str = "en";

/// Map an arbitrary locale tag (`fr-FR`, `fr_CA`, `EN`) onto a shipped locale.
pub fn normalize(tag: &str) -> Option<&'static str> {
    let lowered = tag.to_ascii_lowercase();
    let base = lowered.split(['-', '_']).next().unwrap_or_default();
    LOCALES.iter().copied().find(|locale| *locale == base)
}

/// Select the interface language: explicit flag, then the system locale, then
/// English. Returns the locale that was selected.
pub fn init(requested: Option<&str>) -> &'static str {
    let chosen = requested
        .and_then(normalize)
        .or_else(|| sys_locale::get_locale().and_then(|tag| normalize(&tag)))
        .unwrap_or(DEFAULT_LOCALE);
    rust_i18n::set_locale(chosen);
    chosen
}

/// Translate one key for the current locale.
pub fn tr(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}

/// Translate one key and fill its `%{name}` placeholders.
///
/// The templates are filled here rather than through the macro so that the set
/// of values can be built at the call site.
pub fn tr_args(key: &str, args: &[(&str, String)]) -> String {
    fill(&tr(key), args)
}

/// Translate one key for an explicit locale and fill its placeholders.
pub fn translate_args(locale: &str, key: &str, args: &[(&str, String)]) -> String {
    fill(&translate(locale, key), args)
}

fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut text = template.to_string();
    for (name, value) in args {
        text = text.replace(&format!("%{{{name}}}"), value);
    }
    text
}

/// Translate one key for an explicit locale. Used by tests and by `doctor`
/// reports, which must not depend on the caller's language.
pub fn translate(locale: &str, key: &str) -> String {
    rust_i18n::t!(key, locale = locale).to_string()
}

/// The raw locale files, used by the parity test and by `doctor`.
pub fn sources() -> [(&'static str, &'static str); 8] {
    [
        ("en", include_str!("../locales/en.yml")),
        ("fr", include_str!("../locales/fr.yml")),
        ("es", include_str!("../locales/es.yml")),
        ("de", include_str!("../locales/de.yml")),
        ("it", include_str!("../locales/it.yml")),
        ("pt", include_str!("../locales/pt.yml")),
        ("zh", include_str!("../locales/zh.yml")),
        ("ja", include_str!("../locales/ja.yml")),
    ]
}

/// Extract the keys of a flat `_version: 1` locale file.
///
/// This is deliberately a line scanner, not a YAML parser: the format is ours,
/// flat and one key per line, and the only consumer is the parity test.
pub fn parse_keys(source: &str) -> Vec<String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("_version"))
        .filter_map(|line| line.split_once(':').map(|(key, _)| key.trim().to_string()))
        .filter(|key| !key.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_tags_normalise() {
        assert_eq!(normalize("fr"), Some("fr"));
        assert_eq!(normalize("fr-FR"), Some("fr"));
        assert_eq!(normalize("FR_ca"), Some("fr"));
        assert_eq!(normalize("en_GB"), Some("en"));
        assert_eq!(normalize("es"), Some("es"));
        assert_eq!(normalize("es-ES"), Some("es"));
        assert_eq!(normalize("de"), Some("de"));
        assert_eq!(normalize("it"), Some("it"));
        assert_eq!(normalize("pt"), Some("pt"));
        assert_eq!(normalize("pt-BR"), Some("pt"));
        assert_eq!(normalize("zh"), Some("zh"));
        assert_eq!(normalize("zh-CN"), Some("zh"));
        assert_eq!(normalize("ja"), Some("ja"));
        assert_eq!(normalize("ja-JP"), Some("ja"));
        assert_eq!(normalize(""), None);
    }

    #[test]
    fn placeholders_are_filled() {
        let text = tr_args(
            "cli.run.starting",
            &[
                ("name", "x".to_string()),
                ("tier", "free".to_string()),
                ("attempt", "1".to_string()),
            ],
        );
        assert!(text.contains('x'), "{text}");
        assert!(
            !text.contains("%{"),
            "every placeholder must be filled: {text}"
        );

        let french = translate_args(
            "fr",
            "cli.run.starting",
            &[
                ("name", "y".to_string()),
                ("tier", "free".to_string()),
                ("attempt", "2".to_string()),
            ],
        );
        assert!(!french.contains("%{"));
    }

    #[test]
    fn key_scanner_ignores_comments_and_version() {
        let keys = parse_keys("_version: 1\n# comment\napp.name: \"x: y\"\n\ngate.body: \"z\"\n");
        assert_eq!(keys, vec!["app.name".to_string(), "gate.body".to_string()]);
    }
}
