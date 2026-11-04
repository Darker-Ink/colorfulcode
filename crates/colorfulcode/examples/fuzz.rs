//! Feeds a language lots of short random inputs and prints the first one that
//! produces tokens splitting a character, together with its tokens.
//!
//! cargo run --release --example fuzz -- <language>

use colorfulcode::{highlight, Language};

const ALPHABET: &[&str] = &[
    "\"", "'", "`", "/", "*", "#", "<", ">", "{", "}", "(", ")", "[", "]", "\\", "$", "@", "-", "=", ":", ";", "\n",
    " ", "\t", "a", "Z", "_", "0", "9", ".", "!", "?", "%", "&", "|", "~", "^", "+", ",", "é", "日", "🎨", "\r",
];

fn main() {
    let name = std::env::args().nth(1).expect("usage: fuzz <language>");
    let lang = Language::find(&name).expect("unknown language");
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..200_000 {
        let len = (next() % 12) as usize;
        let code: String = (0..len).map(|_| ALPHABET[(next() % ALPHABET.len() as u64) as usize]).collect();
        let hl = highlight(&code, lang);
        let broken = hl
            .tokens()
            .iter()
            .any(|t| !code.is_char_boundary(t.start as usize) || !code.is_char_boundary(t.end as usize));
        if broken {
            println!("{code:?}");
            for t in hl.tokens() {
                println!("  {:?} {}..{}", t.kind, t.start, t.end);
            }
            std::process::exit(1);
        }
    }
}
