//! Fast syntax highlighting for a lot of languages.
//!
//! ```
//! use colorfulcode::{highlight, Kind, Language};
//!
//! let rust = Language::find("rust").unwrap();
//! let code = "fn main() { println!(\"hi\"); }";
//! let highlighted = highlight(code, rust);
//!
//! for (text, kind) in highlighted.iter() {
//!     if kind == Kind::Keyword {
//!         assert_eq!(text, "fn");
//!     }
//! }
//! ```
//!
//! The output is a flat list of [`Token`]s that covers the input exactly, so
//! rendering is a matter of mapping each [`Kind`] to a color. Ready made
//! renderers exist for [HTML](html) and [ANSI terminals](ansi), and
//! [`Theme`] provides colors for native renderers.

pub mod ansi;
pub mod html;
mod kind;
mod languages;
mod lexer;
mod theme;
mod utf16;

pub use kind::Kind;
pub use languages::Language;
pub use theme::{Color, Style, Theme};

use std::ops::Range;

/// A highlighted span of the input. Offsets are UTF-8 byte offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Token {
    pub start: u32,
    pub end: u32,
    pub kind: Kind,
}

impl Token {
    /// Byte range of the token in the source.
    pub fn range(&self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        (self.end - self.start) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Highlights `code` and returns the result.
///
/// The tokens cover the entire input without gaps or overlaps. Adjacent tokens
/// always have different kinds.
pub fn highlight(code: &str, language: Language) -> Highlighted<'_> {
    let mut tokens = Vec::new();
    highlight_into(code, language, &mut tokens);
    Highlighted { code, tokens }
}

/// Like [`highlight`] but writes into an existing buffer, so repeated calls
/// can reuse the allocation. The buffer is cleared first.
///
/// # Panics
///
/// Panics if `code` is 4 GiB or larger, since offsets are stored as `u32`.
pub fn highlight_into(code: &str, language: Language, tokens: &mut Vec<Token>) {
    assert!(code.len() <= u32::MAX as usize, "input is too large to highlight");
    tokens.clear();
    tokens.reserve(code.len() / 5 + 1);
    let mut ctx = lexer::Ctx::new(code.as_bytes(), tokens);
    language.lex(&mut ctx);
    // Cover any trailing text.
    let end = tokens.last().map_or(0, |t| t.end) as usize;
    if end < code.len() {
        tokens.push(Token { start: end as u32, end: code.len() as u32, kind: Kind::Text });
    }
}

/// Highlighted source code: the input together with its tokens.
#[derive(Clone, Debug)]
pub struct Highlighted<'a> {
    code: &'a str,
    tokens: Vec<Token>,
}

impl<'a> Highlighted<'a> {
    /// The source that was highlighted.
    pub fn code(&self) -> &'a str {
        self.code
    }

    /// The tokens, covering the whole source.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn into_tokens(self) -> Vec<Token> {
        self.tokens
    }

    /// Iterates over `(text, kind)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&'a str, Kind)> + '_ {
        let code = self.code;
        self.tokens.iter().map(move |t| (&code[t.range()], t.kind))
    }

    /// Iterates over lines. Each line yields `(text, kind)` pairs and never
    /// contains the line break. Tokens spanning several lines are split.
    pub fn lines(&self) -> Lines<'a, '_> {
        Lines { code: self.code, tokens: &self.tokens, index: 0, pos: 0, done: false }
    }

    /// Converts the tokens to UTF-16 offsets, as used by JavaScript, Java,
    /// Kotlin, C# and Apple's `NSString`.
    pub fn utf16_tokens(&self) -> Vec<Token> {
        utf16::convert(self.code, &self.tokens)
    }

    /// Renders HTML. See [`html::render`].
    pub fn to_html(&self, options: &html::Options) -> String {
        html::render(self.code, &self.tokens, options)
    }

    /// Renders ANSI escaped text for terminals. See [`ansi::render`].
    pub fn to_ansi(&self, theme: &Theme, mode: ansi::ColorMode) -> String {
        ansi::render(self.code, &self.tokens, theme, mode)
    }
}

/// Iterator over the lines of a [`Highlighted`].
pub struct Lines<'a, 't> {
    code: &'a str,
    tokens: &'t [Token],
    index: usize,
    pos: usize,
    done: bool,
}

impl<'a, 't> Iterator for Lines<'a, 't> {
    type Item = Vec<(&'a str, Kind)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let mut line = Vec::new();
        while let Some(token) = self.tokens.get(self.index) {
            let end = token.end as usize;
            let text = &self.code[self.pos..end];
            if let Some(nl) = text.find('\n') {
                let piece = text[..nl].strip_suffix('\r').unwrap_or(&text[..nl]);
                if !piece.is_empty() {
                    line.push((piece, token.kind));
                }
                self.pos += nl + 1;
                if self.pos >= end {
                    self.index += 1;
                }
                return Some(line);
            }
            if !text.is_empty() {
                line.push((text, token.kind));
            }
            self.pos = end;
            self.index += 1;
        }
        self.done = true;
        Some(line)
    }
}
