//! Properties every language must satisfy on any input: no panics, no hangs,
//! and tokens that cover the input exactly on character boundaries.

use colorfulcode::{highlight, Kind, Language};
use std::path::Path;

fn check(code: &str, lang: Language) {
    let hl = highlight(code, lang);
    let tokens = hl.tokens();
    let mut pos = 0;
    let mut prev: Option<Kind> = None;
    for t in tokens {
        assert_eq!(t.start as usize, pos, "{}: gap or overlap at {pos}", lang.id());
        assert!(t.end > t.start, "{}: empty token at {pos}", lang.id());
        assert!(code.is_char_boundary(t.start as usize), "{}: token starts inside a character", lang.id());
        assert!(code.is_char_boundary(t.end as usize), "{}: token ends inside a character", lang.id());
        assert_ne!(prev, Some(t.kind), "{}: adjacent tokens share a kind at {pos}", lang.id());
        prev = Some(t.kind);
        pos = t.end as usize;
    }
    assert_eq!(pos, code.len(), "{}: tokens do not reach the end", lang.id());
}

fn samples() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/samples");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        out.push((name, std::fs::read_to_string(&path).unwrap()));
    }
    out.sort();
    out
}

#[test]
fn every_language_has_a_sample() {
    let names: Vec<String> = samples().into_iter().map(|(n, _)| n).collect();
    let missing: Vec<&str> = Language::all()
        .iter()
        .map(|l| l.id())
        .filter(|id| !names.iter().any(|n| n.split('.').next() == Some(id)))
        .collect();
    assert!(missing.is_empty(), "missing samples for: {missing:?}");
}

#[test]
fn samples_in_every_language() {
    // Every sample through every lexer: a cheap way to feed each lexer lots of
    // input it was not written for.
    let samples = samples();
    for lang in Language::all() {
        for (_, code) in &samples {
            check(code, *lang);
        }
    }
}

#[test]
fn truncated_samples() {
    // Unterminated strings, comments and tags at every possible position.
    for (name, code) in samples() {
        let lang = Language::find(name.split('.').next().unwrap()).unwrap();
        let mut cut = 0;
        while cut < code.len() {
            if code.is_char_boundary(cut) {
                check(&code[..cut], lang);
            }
            cut += 7;
        }
    }
}

#[test]
fn random_input() {
    const ALPHABET: &[&str] = &[
        "\"", "'", "`", "/", "*", "#", "<", ">", "{", "}", "(", ")", "[", "]", "\\", "$", "@", "-", "=", ":", ";",
        "\n", " ", "\t", "a", "Z", "_", "0", "9", ".", "!", "?", "%", "&", "|", "~", "^", "+", ",", "é", "日", "🎨",
        "\r", "<!--", "-->", "/*", "*/", "\"\"\"", "${", "#{", "<?php", "?>", "```", "<script>", "</script>",
        "<style>", "<<EOF\n", "EOF", "r#\"", "{-", "-}", "(*", "*)", "--", "%%", "fn", "class", "x",
    ];
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for lang in Language::all() {
        for _ in 0..60 {
            let len = (next() % 80) as usize;
            let code: String = (0..len).map(|_| ALPHABET[(next() % ALPHABET.len() as u64) as usize]).collect();
            check(&code, *lang);
        }
    }
}

#[test]
fn deep_nesting() {
    let inputs = [
        "`${".repeat(5000),
        "<a>{".repeat(5000),
        "(".repeat(10000),
        "{".repeat(10000),
        "f\"{".repeat(5000),
        "\"$(".repeat(5000),
        "#{\"".repeat(5000),
        "<div ".repeat(5000),
    ];
    for lang in Language::all() {
        for code in &inputs {
            check(code, *lang);
        }
    }
}

#[test]
fn empty_input() {
    for lang in Language::all() {
        assert!(highlight("", *lang).tokens().is_empty());
    }
}
