//! HTML rendering.
//!
//! Two styles are supported: class names (`<span class="cc-keyword">`) that
//! are colored by a stylesheet such as [`Theme::to_css`], and inline styles
//! (`<span style="color:#ff79c6">`) that need no stylesheet at all.

use crate::{Kind, Style, Theme, Token};
use std::fmt::Write;

/// Options for [`render`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// Use inline styles from this theme instead of class names.
    pub theme: Option<&'a Theme>,
    /// Wrap the output in `<pre class="cc"><code>`.
    pub wrap: bool,
    /// Wrap every line in `<span class="cc-line">`. Needed for line numbers
    /// and line highlights.
    pub lines: bool,
    /// Prefix every line with its number in a `cc-ln` span. Implies `lines`.
    pub line_numbers: bool,
    /// One based line numbers that get the extra `cc-hl` class. Implies `lines`.
    pub highlight_lines: &'a [u32],
}

/// Renders tokens produced by [`crate::highlight`] as HTML.
pub fn render(code: &str, tokens: &[Token], options: &Options) -> String {
    let mut out = String::with_capacity(code.len() * 2 + 64);
    let lines = options.lines || options.line_numbers || !options.highlight_lines.is_empty();
    if options.wrap {
        match options.theme {
            Some(t) => {
                let _ = write!(
                    out,
                    "<pre class=\"cc\" style=\"background:{};color:{}\"><code>",
                    t.background, t.foreground
                );
            }
            None => out.push_str("<pre class=\"cc\"><code>"),
        }
    }
    let gutter = if options.line_numbers { digits(code.bytes().filter(|&b| b == b'\n').count() + 1) } else { 0 };
    let mut writer = Writer { out, theme: options.theme, highlight: options.highlight_lines, line: 1, lines, gutter };
    if lines {
        writer.open_line();
    }
    // Merge neighbors that render identically to keep the markup small.
    let mut i = 0;
    while i < tokens.len() {
        let kind = tokens[i].kind;
        let start = tokens[i].start as usize;
        let mut end = tokens[i].end as usize;
        let key = writer.key(kind);
        i += 1;
        while i < tokens.len() && writer.key(tokens[i].kind) == key {
            end = tokens[i].end as usize;
            i += 1;
        }
        writer.token(&code[start..end], kind);
    }
    if lines {
        writer.out.push_str("</span>");
    }
    let mut out = writer.out;
    if options.wrap {
        out.push_str("</code></pre>");
    }
    out
}

struct Writer<'a> {
    out: String,
    theme: Option<&'a Theme>,
    highlight: &'a [u32],
    line: u32,
    lines: bool,
    /// Width of the line number gutter in digits, or 0 for none.
    gutter: usize,
}

/// What a kind renders as; equal keys can be merged.
#[derive(PartialEq, Eq)]
enum Key {
    Plain,
    Class(Kind),
    Inline(Style),
}

impl Writer<'_> {
    fn key(&self, kind: Kind) -> Key {
        match self.theme {
            Some(theme) => {
                let style = theme.style(kind);
                if style == theme.style(Kind::Text) {
                    Key::Plain
                } else {
                    Key::Inline(style)
                }
            }
            None if kind == Kind::Text => Key::Plain,
            None => Key::Class(kind),
        }
    }

    fn open_line(&mut self) {
        let highlighted = self.highlight.contains(&self.line);
        self.out.push_str(if highlighted { "<span class=\"cc-line cc-hl\"" } else { "<span class=\"cc-line\"" });
        if let (Some(theme), true) = (self.theme, highlighted) {
            let c = theme.foreground;
            let _ = write!(
                self.out,
                " style=\"display:inline-block;width:100%;background:rgba({},{},{},0.1)\"",
                c.r, c.g, c.b
            );
        }
        self.out.push('>');
        if self.gutter > 0 {
            self.out.push_str("<span class=\"cc-ln\"");
            if self.theme.is_some() {
                let _ = write!(
                    self.out,
                    " style=\"display:inline-block;min-width:{}ch;margin-right:2ch;text-align:right;opacity:0.45;user-select:none\"",
                    self.gutter
                );
            }
            let _ = write!(self.out, ">{}</span>", self.line);
        }
    }

    fn token(&mut self, text: &str, kind: Kind) {
        if !self.lines {
            self.span(text, kind);
            return;
        }
        let mut rest = text;
        while let Some(nl) = rest.find('\n') {
            self.span(&rest[..nl], kind);
            self.out.push_str("</span>\n");
            self.line += 1;
            self.open_line();
            rest = &rest[nl + 1..];
        }
        self.span(rest, kind);
    }

    fn span(&mut self, text: &str, kind: Kind) {
        if text.is_empty() {
            return;
        }
        match self.key(kind) {
            Key::Plain => escape_into(&mut self.out, text),
            Key::Class(kind) => {
                self.out.push_str("<span class=\"");
                self.out.push_str(kind.class());
                self.out.push_str("\">");
                escape_into(&mut self.out, text);
                self.out.push_str("</span>");
            }
            Key::Inline(style) => {
                let _ = write!(self.out, "<span style=\"color:{}", style.color);
                if style.bold {
                    self.out.push_str(";font-weight:bold");
                }
                if style.italic {
                    self.out.push_str(";font-style:italic");
                }
                if style.underline {
                    self.out.push_str(";text-decoration:underline");
                }
                self.out.push_str("\">");
                escape_into(&mut self.out, text);
                self.out.push_str("</span>");
            }
        }
    }
}

fn digits(mut n: usize) -> usize {
    let mut d = 1;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}

/// Appends `text` with `&`, `<`, `>`, `"` and `'` escaped.
pub fn escape_into(out: &mut String, text: &str) {
    let bytes = text.as_bytes();
    let mut last = 0;
    for (i, &b) in bytes.iter().enumerate() {
        let rep = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };
        out.push_str(&text[last..i]);
        out.push_str(rep);
        last = i + 1;
    }
    out.push_str(&text[last..]);
}

/// Returns `text` with HTML special characters escaped.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_into(&mut out, text);
    out
}
