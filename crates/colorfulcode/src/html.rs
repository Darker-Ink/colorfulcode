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
    let mut out = String::new();
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
    // A final line break ends the last line rather than starting an empty one.
    let body = if lines { code.strip_suffix('\n').unwrap_or(code) } else { code };
    let gutter = if options.line_numbers { digits(body.bytes().filter(|&b| b == b'\n').count() + 1) } else { 0 };
    let mut writer = Writer::new(out, options, lines, gutter);
    // Reserve roughly the final size up front: inline styles make the output
    // several times larger than the input, and regrowing means copying it all.
    let tags: usize = tokens
        .iter()
        .map(|t| match writer.group[t.kind as usize] {
            0 => 0,
            g => writer.open[g as usize].len() + "</span>".len(),
        })
        .sum();
    writer.out.reserve(code.len() + code.len() / 16 + tags + 64);
    if lines {
        writer.open_line();
    }
    // Merge neighbors that render identically to keep the markup small.
    let mut i = 0;
    while i < tokens.len() {
        let group = writer.group[tokens[i].kind as usize];
        let start = tokens[i].start as usize;
        let mut end = tokens[i].end as usize;
        i += 1;
        while i < tokens.len() && writer.group[tokens[i].kind as usize] == group {
            end = tokens[i].end as usize;
            i += 1;
        }
        let end = end.min(body.len());
        if start < end {
            writer.token(&body[start..end], group);
        }
    }
    if lines {
        writer.out.push_str("</span>");
        if body.len() < code.len() {
            writer.out.push('\n');
        }
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
    /// Kinds that render identically share a group; group 0 is plain text.
    group: [u8; Kind::COUNT],
    /// Opening tag for each group, built once per render.
    open: Vec<String>,
}

impl<'a> Writer<'a> {
    fn new(out: String, options: &Options<'a>, lines: bool, gutter: usize) -> Self {
        let mut group = [0u8; Kind::COUNT];
        let mut open = vec![String::new()];
        let mut styles: Vec<Style> = Vec::new();
        for kind in Kind::ALL.iter().copied().skip(1) {
            match options.theme {
                None => {
                    group[kind as usize] = open.len() as u8;
                    open.push(format!("<span class=\"{}\">", kind.class()));
                }
                Some(theme) => {
                    let style = theme.style(kind);
                    if style == theme.style(Kind::Text) {
                        continue;
                    }
                    if let Some(i) = styles.iter().position(|s| *s == style) {
                        group[kind as usize] = i as u8 + 1;
                        continue;
                    }
                    styles.push(style);
                    group[kind as usize] = open.len() as u8;
                    let mut tag = format!("<span style=\"color:{}", style.color);
                    if style.bold {
                        tag.push_str(";font-weight:bold");
                    }
                    if style.italic {
                        tag.push_str(";font-style:italic");
                    }
                    if style.underline {
                        tag.push_str(";text-decoration:underline");
                    }
                    tag.push_str("\">");
                    open.push(tag);
                }
            }
        }
        Writer { out, theme: options.theme, highlight: options.highlight_lines, line: 1, lines, gutter, group, open }
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

    fn token(&mut self, text: &str, group: u8) {
        if !self.lines {
            self.span(text, group);
            return;
        }
        let mut rest = text;
        while let Some(nl) = rest.find('\n') {
            self.span(&rest[..nl], group);
            self.out.push_str("</span>\n");
            self.line += 1;
            self.open_line();
            rest = &rest[nl + 1..];
        }
        self.span(rest, group);
    }

    fn span(&mut self, text: &str, group: u8) {
        if text.is_empty() {
            return;
        }
        if group == 0 {
            escape_into(&mut self.out, text);
        } else {
            self.out.push_str(&self.open[group as usize]);
            escape_into(&mut self.out, text);
            self.out.push_str("</span>");
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
        // Most bytes are not special; one table lookup rejects them.
        if !SPECIAL[b as usize] {
            continue;
        }
        let rep = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            _ => "&#39;",
        };
        out.push_str(&text[last..i]);
        out.push_str(rep);
        last = i + 1;
    }
    out.push_str(&text[last..]);
}

const SPECIAL: [bool; 256] = {
    let mut t = [false; 256];
    t[b'&' as usize] = true;
    t[b'<' as usize] = true;
    t[b'>' as usize] = true;
    t[b'"' as usize] = true;
    t[b'\'' as usize] = true;
    t
};

/// Returns `text` with HTML special characters escaped.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_into(&mut out, text);
    out
}
