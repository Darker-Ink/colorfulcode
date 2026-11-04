//! A tiny tokenizer, written to show off the highlighter.
use std::collections::HashMap;
use std::fmt;

/// A lexical token with a borrowed slice of the input.
#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Number(f64),
    Ident(&'a str),
    Op(char),
}

const MAX_DEPTH: usize = 64;

pub struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    seen: HashMap<&'a str, u32>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self { src, pos: 0, seen: HashMap::new() }
    }

    pub async fn next_token(&mut self) -> Option<Token<'a>> {
        let rest = &self.src[self.pos..];
        let c = rest.chars().next()?;
        match c {
            '0'..='9' => {
                let len = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
                self.pos += len;
                rest[..len].parse().ok().map(Token::Number)
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let word = &rest[..rest.find(char::is_whitespace).unwrap_or(rest.len())];
                *self.seen.entry(word).or_insert(0) += 1;
                self.pos += word.len();
                Some(Token::Ident(word))
            }
            '\n' | '\t' => None,
            _ => {
                self.pos += c.len_utf8();
                Some(Token::Op(c))
            }
        }
    }
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Number(n) => write!(f, "{n:.2}"),
            Token::Ident(s) => write!(f, "{s}"),
            Token::Op(c) => write!(f, "'{c}'"),
        }
    }
}

fn main() {
    let raw = r#"let x = "quoted";"#;
    let bytes = b"\x00\xff";
    'outer: for i in 0..MAX_DEPTH {
        if i % 2 == 0 && bytes.len() > 1 {
            continue 'outer;
        }
        println!("{raw} {i:>3} {:?}", Some(i as u8));
    }
    let total: u64 = (1..=10).map(|x| x * 2).sum();
    assert_eq!(total, 110, "math is broken");
}
