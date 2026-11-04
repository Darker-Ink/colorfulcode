#![allow(dead_code)]

use colorfulcode::{highlight, Kind, Language};

/// Highlights `code` and returns the non whitespace tokens as `(text, kind)`.
pub fn tokens(language: &str, code: &str) -> Vec<(String, Kind)> {
    let lang = Language::find(language).unwrap_or_else(|| panic!("unknown language {language}"));
    highlight(code, lang)
        .iter()
        .filter(|(text, kind)| *kind != Kind::Text || !text.trim().is_empty())
        .map(|(text, kind)| (if kind == Kind::Text { text.trim() } else { text }.to_string(), kind))
        .collect()
}

/// Asserts that each `(text, kind)` pair appears as a whole token, in order.
#[track_caller]
pub fn assert_tokens(language: &str, code: &str, expected: &[(&str, Kind)]) {
    let actual = tokens(language, code);
    let mut i = 0;
    for (text, kind) in expected {
        match actual[i..].iter().position(|(t, k)| t == text && k == kind) {
            Some(p) => i += p + 1,
            None => panic!(
                "{language}: expected {text:?} as {kind} in order\ncode: {code}\ntokens: {:#?}",
                actual.iter().map(|(t, k)| format!("{k}: {t:?}")).collect::<Vec<_>>()
            ),
        }
    }
}

/// Asserts that `text` is never highlighted as `kind`.
#[track_caller]
pub fn assert_not(language: &str, code: &str, text: &str, kind: Kind) {
    let actual = tokens(language, code);
    assert!(
        !actual.iter().any(|(t, k)| t == text && *k == kind),
        "{language}: {text:?} should not be {kind}\ntokens: {actual:?}"
    );
}
