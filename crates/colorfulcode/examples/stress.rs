//! Looks for inputs that make a lexer slower than linear. Every language gets
//! a long single line (and many short lines) of each pattern at two sizes; a
//! pattern is reported when doubling the input more than triples the time.
//!
//! cargo run --release --example stress [language]

use colorfulcode::{highlight_into, Language};
use std::time::Instant;

const PATTERNS: &[&str] = &[
    "\"", "'", "`", "/", "*", "#", "<", ">", "{", "}", "(", ")", "[", "]", "\\", "$", "@", "-", "=", ":", ";", "%",
    "&", "|", "!", "?", "~", "^", "+", ".", ",", "<!--", "/*", "\"\"\"", "${", "#{", "{{", "{%", "<?", "```", "<a ",
    "</", "--", "(*", "{-", "#[", "[[", "@{", "$(", "<<", "x:", "x =", "a.", "f(", "(/[", "\\begin{", "#!", "%{",
    "<%", "@if", "x\"", "r#\"", "sudo ", "q{", "<T,",
];

fn time(code: &str, lang: Language, tokens: &mut Vec<colorfulcode::Token>) -> f64 {
    let start = Instant::now();
    highlight_into(code, lang, tokens);
    start.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let filter = std::env::args().nth(1);
    let mut tokens = Vec::new();
    let mut found = 0;
    for lang in Language::all() {
        if filter.as_deref().is_some_and(|f| f != lang.id()) {
            continue;
        }
        for pattern in PATTERNS {
            for sep in ["", "\n"] {
                let unit = format!("{pattern}a{sep}");
                let n = 60_000 / unit.len();
                let small = unit.repeat(n);
                let large = unit.repeat(n * 2);
                let a = time(&small, *lang, &mut tokens).max(0.05);
                let b = time(&large, *lang, &mut tokens);
                if b > 15.0 && b / a > 3.0 {
                    found += 1;
                    let shape = if sep.is_empty() { "one line" } else { "lines" };
                    println!("{:<14} {:?} ({shape}): {a:.1} ms -> {b:.1} ms", lang.id(), pattern);
                }
            }
        }
    }
    println!("{found} slow cases");
}
