//! HTML, XML, component formats, Markdown and PHP.

use super::{lang, Definition, Engine};
use crate::lexer::generic::{features::*, Interp, Lexer, Spec, Str};
use crate::lexer::{is_ident, Ctx};
use crate::{Kind, Language};

pub static LANGUAGES: &[&Definition] = &[&HTML, &XML, &SVG, &VUE, &SVELTE, &ASTRO, &MARKDOWN, &MDX, &PHP];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavor {
    Html,
    Xml,
    Vue,
    Svelte,
    Astro,
    Php,
}

pub static HTML: Definition =
    Definition::new("html", "HTML", Engine::Custom(|ctx| Markup::new(ctx, Flavor::Html).run()))
        .aliases(&["htm", "xhtml"])
        .extensions(&["html", "htm", "xhtml", "shtml"]);

pub static XML: Definition = Definition::new("xml", "XML", Engine::Custom(|ctx| Markup::new(ctx, Flavor::Xml).run()))
    .aliases(&["rss", "atom", "plist", "xsd", "xsl"])
    .extensions(&[
        "xml", "xsd", "xsl", "xslt", "rss", "atom", "plist", "csproj", "fsproj", "vbproj", "props", "targets", "resx",
        "xaml", "wsdl", "pom", "storyboard", "xib", "nuspec", "kml", "gpx",
    ]);

pub static SVG: Definition =
    Definition::new("svg", "SVG", Engine::Custom(|ctx| Markup::new(ctx, Flavor::Xml).run())).extensions(&["svg"]);

pub static VUE: Definition =
    Definition::new("vue", "Vue", Engine::Custom(|ctx| Markup::new(ctx, Flavor::Vue).run())).extensions(&["vue"]);

pub static SVELTE: Definition =
    Definition::new("svelte", "Svelte", Engine::Custom(|ctx| Markup::new(ctx, Flavor::Svelte).run()))
        .extensions(&["svelte"]);

pub static ASTRO: Definition = Definition::new("astro", "Astro", Engine::Custom(astro)).extensions(&["astro"]);

/// Astro: TypeScript frontmatter between `---` fences followed by a template.
fn astro(ctx: &mut Ctx) {
    if ctx.starts_with(b"---") {
        let body = ctx.line_end(0);
        ctx.emit(0, body, Kind::Punctuation);
        if let Some(close) = find_line(ctx, body, b"---") {
            ctx.embed(lang("typescript"), body, close);
            ctx.pos = ctx.line_end(close);
            ctx.emit(close, ctx.pos, Kind::Punctuation);
        } else {
            ctx.embed(lang("typescript"), body, ctx.len());
        }
    }
    Markup::new(ctx, Flavor::Astro).run();
}

/// Offset of the first line at or after `from` that is exactly `marker`.
fn find_line(ctx: &Ctx, from: usize, marker: &[u8]) -> Option<usize> {
    let mut pos = from;
    while pos < ctx.len() {
        if ctx.src[pos] == b'\n' {
            pos += 1;
            continue;
        }
        let end = ctx.line_end(pos);
        let line = &ctx.src[pos..end];
        if line.strip_suffix(b"\r").unwrap_or(line).trim_ascii_end() == marker {
            return Some(pos);
        }
        pos = end;
    }
    None
}

struct Markup<'c, 's, 'o> {
    ctx: &'c mut Ctx<'s, 'o>,
    flavor: Flavor,
}

impl<'c, 's, 'o> Markup<'c, 's, 'o> {
    fn new(ctx: &'c mut Ctx<'s, 'o>, flavor: Flavor) -> Self {
        Markup { ctx, flavor }
    }

    fn script_lang(&self) -> Language {
        match self.flavor {
            Flavor::Vue | Flavor::Svelte | Flavor::Astro => lang("typescript"),
            _ => lang("javascript"),
        }
    }

    fn run(&mut self) {
        while !self.ctx.done() {
            let rest = &self.ctx.src[self.ctx.pos..];
            let skip = rest.iter().position(|&b| matches!(b, b'<' | b'&' | b'{')).unwrap_or(rest.len());
            self.ctx.pos += skip;
            match self.ctx.peek() {
                0 => break,
                b'<' => self.tag(),
                b'&' => self.entity(),
                b'{' => self.expression(),
                _ => self.ctx.pos += 1,
            }
        }
    }

    fn entity(&mut self) {
        let start = self.ctx.pos;
        let mut end = start + 1;
        if self.ctx.at(end) == b'#' {
            end += 1;
        }
        while self.ctx.at(end).is_ascii_alphanumeric() && end - start < 32 {
            end += 1;
        }
        if self.ctx.at(end) == b';' && end > start + 1 {
            self.ctx.pos = end + 1;
            self.ctx.emit(start, end + 1, Kind::Escape);
        } else {
            self.ctx.pos += 1;
        }
    }

    /// Template expressions: `{{ x }}` in Vue, `{x}` in Svelte and Astro.
    fn expression(&mut self) {
        let start = self.ctx.pos;
        match self.flavor {
            Flavor::Vue if self.ctx.starts_with(b"{{") => {
                let close = self.ctx.find(start + 2, b"}}").unwrap_or(self.ctx.len());
                self.ctx.emit(start, start + 2, Kind::Punctuation);
                self.ctx.embed(lang("javascript"), start + 2, close);
                self.ctx.pos = (close + 2).min(self.ctx.len());
                self.ctx.emit(close, self.ctx.pos, Kind::Punctuation);
            }
            Flavor::Svelte | Flavor::Astro => {
                let close = matching_brace(self.ctx.src, start).unwrap_or(self.ctx.len());
                self.ctx.emit(start, start + 1, Kind::Punctuation);
                let mut inner = start + 1;
                // Svelte blocks: {#if x}, {:else}, {/each}, {@html x}.
                if matches!(self.ctx.at(inner), b'#' | b':' | b'/' | b'@') {
                    let mut e = inner + 1;
                    while is_ident(self.ctx.at(e)) {
                        e += 1;
                    }
                    self.ctx.emit(inner, e, Kind::Keyword);
                    inner = e;
                }
                // Astro expressions may contain JSX; Svelte ones never do.
                let code = if self.flavor == Flavor::Astro { lang("tsx") } else { lang("typescript") };
                self.ctx.embed(code, inner, close);
                self.ctx.pos = (close + 1).min(self.ctx.len());
                self.ctx.emit(close, self.ctx.pos, Kind::Punctuation);
            }
            _ => self.ctx.pos += 1,
        }
    }

    fn tag(&mut self) {
        let start = self.ctx.pos;
        let ctx = &mut *self.ctx;
        if ctx.starts_with(b"<!--") {
            let end = ctx.find(start + 4, b"-->").map_or(ctx.len(), |p| p + 3);
            ctx.pos = end;
            ctx.emit(start, end, Kind::Comment);
            return;
        }
        if ctx.starts_with(b"<![CDATA[") {
            let end = ctx.find(start, b"]]>").map_or(ctx.len(), |p| p + 3);
            ctx.emit(start, start + 9, Kind::Punctuation);
            ctx.emit(start + 9, end.saturating_sub(3).max(start + 9), Kind::String);
            ctx.pos = end;
            ctx.emit(end.saturating_sub(3).max(start + 9), end, Kind::Punctuation);
            return;
        }
        if ctx.starts_with(b"<?") {
            if self.flavor == Flavor::Php {
                self.php_block();
                return;
            }
            let end = ctx.find(start, b"?>").map_or(ctx.len(), |p| p + 2);
            ctx.pos = end;
            ctx.emit(start, end, Kind::Meta);
            return;
        }
        if ctx.starts_with(b"<!") {
            let end = ctx.src[start..].iter().position(|&b| b == b'>').map_or(ctx.len(), |p| start + p + 1);
            ctx.pos = end;
            ctx.emit(start, end, Kind::Meta);
            return;
        }
        let closing = ctx.peek_at(1) == b'/';
        let name_start = start + 1 + closing as usize;
        let first = ctx.at(name_start);
        if !(first.is_ascii_alphabetic() || first == b'_' || first >= 0x80 || (!closing && first == b'>')) {
            ctx.pos += 1;
            return;
        }
        ctx.emit(start, name_start, Kind::Punctuation);
        let mut end = name_start;
        while is_ident(ctx.at(end)) || matches!(ctx.at(end), b'-' | b':' | b'.') {
            end += 1;
        }
        let component = self.flavor != Flavor::Xml && first.is_ascii_uppercase();
        ctx.emit(name_start, end, if component { Kind::Type } else { Kind::Tag });
        ctx.pos = end;
        let name = &ctx.src[name_start..end];
        let raw_text: Option<&[u8]> = if closing || self.flavor == Flavor::Xml {
            None
        } else if name.eq_ignore_ascii_case(b"script") {
            Some(b"script")
        } else if name.eq_ignore_ascii_case(b"style") {
            Some(b"style")
        } else {
            None
        };
        let mut embedded = if raw_text == Some(b"style") { lang("css") } else { self.script_lang() };
        let mut is_json_or_other = false;
        let self_closing = self.attributes(&mut embedded, &mut is_json_or_other);
        if let (Some(tag), false) = (raw_text, self_closing) {
            let body = self.ctx.pos;
            let close = find_closing_tag(self.ctx.src, body, tag).unwrap_or(self.ctx.len());
            if is_json_or_other {
                self.ctx.emit(body, close, Kind::String);
            } else {
                self.ctx.embed(embedded, body, close);
            }
            self.ctx.pos = close;
        }
    }

    /// Lexes attributes up to and including `>`. Returns true for `/>`.
    fn attributes(&mut self, embedded: &mut Language, opaque: &mut bool) -> bool {
        loop {
            self.ctx.skip_whitespace();
            let b = self.ctx.peek();
            match b {
                0 => return false,
                b'>' => {
                    self.ctx.token(1, Kind::Punctuation);
                    return false;
                }
                b'/' if self.ctx.peek_at(1) == b'>' => {
                    self.ctx.token(2, Kind::Punctuation);
                    return true;
                }
                b'?' if self.ctx.peek_at(1) == b'>' => {
                    self.ctx.token(2, Kind::Punctuation);
                    return true;
                }
                b'<' => return false,
                b'{' if matches!(self.flavor, Flavor::Svelte | Flavor::Astro) => self.expression(),
                b'"' | b'\'' => self.quoted(None),
                _ => {
                    let start = self.ctx.pos;
                    let mut end = start;
                    while !matches!(self.ctx.at(end), b' ' | b'\t' | b'\n' | b'\r' | b'=' | b'>' | b'"' | b'\'' | 0)
                        && !(self.ctx.at(end) == b'/' && self.ctx.at(end + 1) == b'>')
                    {
                        end += 1;
                    }
                    if end == start {
                        self.ctx.bump_char();
                        continue;
                    }
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Attribute);
                    let name = &self.ctx.src[start..end];
                    let directive = self.flavor == Flavor::Vue
                        && (name.starts_with(b":")
                            || name.starts_with(b"@")
                            || name.starts_with(b"v-")
                            || name.starts_with(b"#"));
                    let is_lang = name.eq_ignore_ascii_case(b"lang") || name.eq_ignore_ascii_case(b"type");
                    self.ctx.skip_whitespace();
                    if self.ctx.peek() != b'=' {
                        continue;
                    }
                    self.ctx.token(1, Kind::Operator);
                    self.ctx.skip_whitespace();
                    match self.ctx.peek() {
                        b'"' | b'\'' => {
                            let value_start = self.ctx.pos + 1;
                            if directive {
                                self.quoted(Some(lang("javascript")));
                            } else {
                                self.quoted(None);
                            }
                            if is_lang {
                                let value_end = self.ctx.pos.saturating_sub(1).max(value_start);
                                let value = &self.ctx.src[value_start..value_end];
                                set_embedded(value, embedded, opaque);
                            }
                        }
                        b'{' if matches!(self.flavor, Flavor::Svelte | Flavor::Astro) => self.expression(),
                        _ => {
                            let s = self.ctx.pos;
                            while !matches!(self.ctx.peek(), b' ' | b'\t' | b'\n' | b'\r' | b'>' | 0) {
                                self.ctx.pos += 1;
                            }
                            self.ctx.emit_from(s, Kind::String);
                            if is_lang {
                                let value = &self.ctx.src[s..self.ctx.pos];
                                set_embedded(value, embedded, opaque);
                            }
                        }
                    }
                }
            }
        }
    }

    /// A quoted attribute value, optionally containing code.
    fn quoted(&mut self, code: Option<Language>) {
        let q = self.ctx.peek();
        let start = self.ctx.pos;
        let end = self.ctx.src[start + 1..].iter().position(|&b| b == q).map_or(self.ctx.len(), |i| start + 1 + i + 1);
        match code {
            Some(lang) => {
                self.ctx.emit(start, start + 1, Kind::String);
                let inner_end = if end > start + 1 && self.ctx.at(end - 1) == q { end - 1 } else { end };
                self.ctx.embed(lang, start + 1, inner_end);
                self.ctx.emit(inner_end, end, Kind::String);
            }
            None => self.ctx.emit(start, end, Kind::String),
        }
        self.ctx.pos = end;
    }

    /// `<?php ... ?>` and `<?= ... ?>` blocks.
    fn php_block(&mut self) {
        let start = self.ctx.pos;
        let mut open = start + 2;
        if self.ctx.starts_with_at(open, b"php") {
            open += 3;
        } else if self.ctx.at(open) == b'=' {
            open += 1;
        }
        self.ctx.emit(start, open, Kind::Meta);
        let close = self.ctx.find(open, b"?>").unwrap_or(self.ctx.len());
        self.ctx.embed_spec(&PHP_SPEC, open, close);
        self.ctx.pos = (close + 2).min(self.ctx.len());
        self.ctx.emit(close, self.ctx.pos, Kind::Meta);
    }
}

fn set_embedded(value: &[u8], embedded: &mut Language, opaque: &mut bool) {
    let v = std::str::from_utf8(value).unwrap_or("").trim().to_ascii_lowercase();
    let v = v.as_str();
    let name = match v {
        "" | "module" | "text/javascript" | "application/javascript" | "text/babel" | "js" | "javascript" => {
            "javascript"
        }
        "ts" | "typescript" | "text/typescript" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "application/json" | "application/ld+json" | "importmap" | "json" | "speculationrules" => "json",
        "text/css" | "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "less" => "less",
        "stylus" | "styl" => "stylus",
        "postcss" => "css",
        "text/x-template" | "text/html" | "text/ng-template" => "html",
        _ if v.contains('/') => {
            *opaque = true;
            return;
        }
        _ => return,
    };
    *embedded = lang(name);
}

/// Offset of `</tag` (case insensitive) at or after `from`.
fn find_closing_tag(src: &[u8], from: usize, tag: &[u8]) -> Option<usize> {
    let mut i = from;
    while let Some(off) = src[i..].iter().position(|&b| b == b'<') {
        let at = i + off;
        if src.get(at + 1) == Some(&b'/')
            && src.len() >= at + 2 + tag.len()
            && src[at + 2..at + 2 + tag.len()].eq_ignore_ascii_case(tag)
        {
            return Some(at);
        }
        i = at + 1;
    }
    None
}

/// Offset of the `}` matching the `{` at `open`, skipping strings and template literals.
pub(crate) fn matching_brace(src: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < src.len() {
        match src[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < src.len() && src[i] != q {
                    if src[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

// PHP

/// PHP files are templates when they contain `<?php`, plain code otherwise
/// (as in most snippets).
fn php(ctx: &mut Ctx) {
    if ctx.find(0, b"<?").is_some() {
        Markup::new(ctx, Flavor::Php).run();
    } else {
        Lexer::new(ctx, &PHP_SPEC).run();
    }
}

/// Heredoc and nowdoc strings: `<<<EOT` and `<<<'EOT'`.
fn php_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    if !ctx.starts_with(b"<<<") {
        return false;
    }
    let start = ctx.pos;
    let mut i = start + 3;
    while ctx.at(i) == b' ' {
        i += 1;
    }
    let quoted = matches!(ctx.at(i), b'\'' | b'"');
    let word_start = i + quoted as usize;
    let mut word_end = word_start;
    while is_ident(ctx.at(word_end)) {
        word_end += 1;
    }
    if word_end == word_start {
        return false;
    }
    let word = &ctx.src[word_start..word_end];
    let mut pos = ctx.line_end(word_end);
    let mut end = ctx.len();
    while pos < ctx.len() {
        let line_start = pos + 1;
        let line_end = ctx.line_end(line_start);
        let line = ctx.src[line_start..line_end].trim_ascii_start();
        if line.starts_with(word) && !line.get(word.len()).is_some_and(|&b| is_ident(b)) {
            end = line_end - (line.len() - word.len());
            break;
        }
        pos = line_end;
    }
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    lx.mark_value();
    true
}

pub(crate) static PHP_SPEC: Spec = Spec {
    line_comments: &["//", "#"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").multiline().interp(Interp::Dollar), Str::quoted("'").multiline()],
    keywords: &[
        "abstract", "and", "as", "break", "callable", "case", "catch", "clone", "const", "continue", "declare",
        "default", "do", "echo", "else", "elseif", "empty", "enddeclare", "endfor", "endforeach", "endif", "endswitch",
        "endwhile", "eval", "exit", "die", "extends", "final", "finally", "fn", "for", "foreach", "global", "goto",
        "if", "implements", "include", "include_once", "instanceof", "insteadof", "isset", "list", "match",
        "namespace", "new", "or", "print", "private", "protected", "public", "readonly", "require", "require_once",
        "return", "static", "switch", "throw", "try", "unset", "use", "var", "while", "xor", "yield",
    ],
    types: &[
        "array", "bool", "float", "int", "iterable", "mixed", "never", "object", "string", "void", "self", "parent",
    ],
    constants: &["true", "false", "null"],
    variables: &["$this"],
    function_keywords: &["function"],
    type_keywords: &["class", "interface", "trait", "enum"],
    sigils: &[(b'$', Kind::Variable)],
    case_insensitive: true,
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | PRINTF_FORMAT,
    hook: Some(php_hook),
    hook_chars: "<",
    ..Spec::DEFAULT
};

pub static PHP: Definition = Definition::new("php", "PHP", Engine::Custom(php))
    .extensions(&["php", "phtml", "php3", "php4", "php5", "php8", "phps"]);

// Markdown

pub static MARKDOWN: Definition = Definition::new("markdown", "Markdown", Engine::Custom(|ctx| markdown(ctx, false)))
    .aliases(&["md"])
    .extensions(&["md", "markdown", "mdown", "mkd", "mkdn", "mdwn", "ronn", "workbook"])
    .filenames(&["README", "CHANGELOG", "LICENSE.md"]);

pub static MDX: Definition =
    Definition::new("mdx", "MDX", Engine::Custom(|ctx| markdown(ctx, true))).extensions(&["mdx"]);

fn markdown(ctx: &mut Ctx, mdx: bool) {
    // Front matter.
    for (fence, language) in [(b"---".as_slice(), "yaml"), (b"+++".as_slice(), "toml")] {
        if ctx.pos == 0 && ctx.starts_with(fence) && matches!(ctx.at(3), b'\n' | b'\r') {
            let body = ctx.line_end(0);
            ctx.emit(0, body, Kind::Punctuation);
            if let Some(close) = find_line(ctx, body, fence) {
                ctx.embed(lang(language), body, close);
                ctx.pos = ctx.line_end(close);
                ctx.emit(close, ctx.pos, Kind::Punctuation);
            }
        }
    }
    while !ctx.done() {
        let line_start = ctx.pos;
        let line_end = ctx.line_end(line_start);
        block(ctx, line_start, line_end, mdx);
        if ctx.pos <= line_start {
            ctx.pos = line_end;
        }
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

/// Handles one block level line (and any following lines it owns).
fn block(ctx: &mut Ctx, start: usize, end: usize, mdx: bool) {
    let line = &ctx.src[start..end];
    let indent = line.iter().take_while(|&&b| b == b' ').count();
    let content = &line[indent.min(line.len())..];
    let at = start + indent;
    if indent >= 4 || content.is_empty() {
        ctx.pos = end;
        return;
    }
    // Fenced code block.
    if let Some(fence_char @ (b'`' | b'~')) = content.first().copied() {
        let fence_len = content.iter().take_while(|&&b| b == fence_char).count();
        if fence_len >= 3 {
            fenced_code(ctx, at, end, fence_char, fence_len);
            return;
        }
    }
    // Headings.
    if content[0] == b'#' {
        let hashes = content.iter().take_while(|&&b| b == b'#').count();
        if hashes <= 6 && matches!(content.get(hashes), None | Some(b' ' | b'\t' | b'\r')) {
            ctx.emit(at, end, Kind::Heading);
            ctx.pos = end;
            return;
        }
    }
    // Setext heading: a paragraph line followed by === or ---.
    if end < ctx.len() {
        let next_end = ctx.line_end(end + 1);
        let next = ctx.src[end + 1..next_end].trim_ascii();
        let marker = |c: u8| !next.is_empty() && next.iter().all(|&b| b == c);
        let paragraph = !matches!(content[0], b'-' | b'*' | b'+' | b'>' | b'|' | b'<');
        if paragraph && (marker(b'=') || (marker(b'-') && next.len() >= 2)) {
            ctx.emit(at, end, Kind::Heading);
            ctx.emit(end + 1, next_end, Kind::Heading);
            ctx.pos = next_end;
            return;
        }
    }
    // Thematic break.
    let compact: Vec<u8> = content.iter().copied().filter(|&b| b != b' ' && b != b'\r').collect();
    if compact.len() >= 3 && matches!(compact[0], b'-' | b'*' | b'_') && compact.iter().all(|&b| b == compact[0]) {
        ctx.emit(at, end, Kind::Punctuation);
        ctx.pos = end;
        return;
    }
    // MDX import/export lines and JSX blocks.
    if mdx && (content.starts_with(b"import ") || content.starts_with(b"export ")) {
        ctx.embed(lang("jsx"), at, end);
        return;
    }
    if content[0] == b'<' && content.get(1).is_some_and(|b| b.is_ascii_alphabetic() || *b == b'/' || *b == b'!') {
        let language = if mdx { lang("jsx") } else { lang("html") };
        ctx.embed(language, at, end);
        return;
    }
    let mut pos = at;
    // Blockquote markers.
    while ctx.at(pos) == b'>' {
        ctx.emit(pos, pos + 1, Kind::Punctuation);
        pos += 1;
        while ctx.at(pos) == b' ' {
            pos += 1;
        }
    }
    // List markers and task boxes.
    let b = ctx.at(pos);
    if matches!(b, b'-' | b'*' | b'+') && matches!(ctx.at(pos + 1), b' ' | b'\t') {
        ctx.emit(pos, pos + 1, Kind::Keyword);
        pos += 2;
    } else if b.is_ascii_digit() {
        let mut e = pos;
        while ctx.at(e).is_ascii_digit() && e - pos < 9 {
            e += 1;
        }
        if matches!(ctx.at(e), b'.' | b')') && matches!(ctx.at(e + 1), b' ' | b'\t') {
            ctx.emit(pos, e + 1, Kind::Keyword);
            pos = e + 2;
        }
    }
    if ctx.at(pos) == b'[' && matches!(ctx.at(pos + 1), b' ' | b'x' | b'X') && ctx.at(pos + 2) == b']' {
        ctx.emit(pos, pos + 3, Kind::Constant);
        pos += 3;
    }
    // Link reference definitions: [label]: url
    if ctx.at(pos) == b'[' {
        if let Some(close) = ctx.src[pos..end].iter().position(|&b| b == b']').map(|i| pos + i) {
            if ctx.at(close + 1) == b':' {
                ctx.emit(pos, close + 1, Kind::Link);
                ctx.emit(close + 1, close + 2, Kind::Punctuation);
                ctx.emit(close + 2, end, Kind::String);
                ctx.pos = end;
                return;
            }
        }
    }
    inline(ctx, pos, end);
    ctx.pos = end;
}

fn fenced_code(ctx: &mut Ctx, at: usize, end: usize, fence_char: u8, fence_len: usize) {
    let info_start = at + fence_len;
    ctx.emit(at, info_start, Kind::Punctuation);
    let info = ctx.src[info_start..end].trim_ascii();
    let name_len = info.iter().take_while(|&&b| !matches!(b, b' ' | b'{' | b',' | b'\t')).count();
    let name = std::str::from_utf8(&info[..name_len]).unwrap_or("");
    ctx.emit(info_start, end, Kind::Attribute);
    // Find the closing fence.
    let body = (end + 1).min(ctx.len());
    let mut pos = body;
    let mut close = ctx.len();
    let mut close_end = ctx.len();
    while pos < ctx.len() {
        let e = ctx.line_end(pos);
        let line = ctx.src[pos..e].trim_ascii();
        if line.len() >= fence_len && line.iter().all(|&b| b == fence_char) {
            close = pos;
            close_end = e;
            break;
        }
        pos = e + 1;
    }
    match Language::find(name).filter(|_| !name.is_empty()) {
        Some(language) => ctx.embed(language, body, close),
        None => ctx.emit(body, close, Kind::String),
    }
    ctx.emit(close, close_end, Kind::Punctuation);
    ctx.pos = close_end;
}

/// Inline formatting within `[start, end)`.
fn inline(ctx: &mut Ctx, start: usize, end: usize) {
    let mut i = start;
    let src = ctx.src;
    // Inline constructs are short. Bounding every search for a closing
    // delimiter keeps long lines full of unclosed ones linear.
    let window = |from: usize| (from + 512).min(end).max(from.min(end));
    while i < end {
        let b = src[i];
        match b {
            b'\\' if i + 1 < end && src[i + 1].is_ascii_punctuation() => {
                ctx.emit(i, i + 2, Kind::Escape);
                i += 2;
            }
            b'`' => {
                let ticks = src[i..end].iter().take_while(|&&c| c == b'`').count();
                let fence = &src[i..i + ticks];
                match ctx.find_in(i + ticks, window(i), fence) {
                    Some(p) => {
                        ctx.emit(i, p + ticks, Kind::String);
                        i = p + ticks;
                    }
                    None => i += ticks,
                }
            }
            b'*' | b'_' | b'~' => {
                let run = src[i..end].iter().take_while(|&&c| c == b).count();
                let word_before = i > start && src[i - 1].is_ascii_alphanumeric();
                let next = src.get(i + run).copied().unwrap_or(b' ');
                if (b == b'_' && word_before) || matches!(next, b' ' | b'\t' | b'\r' | b'\n') || (b == b'~' && run != 2)
                {
                    i += run;
                    continue;
                }
                let delim = &src[i..i + run.min(3)];
                match ctx.find_in(i + delim.len(), window(i), delim).filter(|&p| !matches!(src[p - 1], b' ' | b'\t')) {
                    Some(p) => {
                        let kind = match (b, delim.len()) {
                            (b'~', _) => Kind::Deleted,
                            (_, 1) => Kind::Emphasis,
                            _ => Kind::Strong,
                        };
                        ctx.emit(i, p + delim.len(), kind);
                        i = p + delim.len();
                    }
                    None => i += run,
                }
            }
            b'[' | b'!' => {
                let open = if b == b'!' { i + 1 } else { i };
                if src.get(open) != Some(&b'[') {
                    i += 1;
                    continue;
                }
                let Some(close) = bracket_end(src, open, window(open)) else {
                    i += 1;
                    continue;
                };
                match src.get(close + 1) {
                    Some(b'(') => {
                        let url_end =
                            src[close + 1..window(close + 1)].iter().position(|&c| c == b')').map(|p| close + 1 + p);
                        let Some(url_end) = url_end else {
                            i += 1;
                            continue;
                        };
                        ctx.emit(i, open + 1, Kind::Punctuation);
                        inline_link_text(ctx, open + 1, close);
                        ctx.emit(close, close + 2, Kind::Punctuation);
                        ctx.emit(close + 2, url_end, Kind::String);
                        ctx.emit(url_end, url_end + 1, Kind::Punctuation);
                        i = url_end + 1;
                    }
                    Some(b'[') => {
                        let ref_end =
                            src[close + 1..window(close + 1)].iter().position(|&c| c == b']').map(|p| close + 1 + p);
                        let Some(ref_end) = ref_end else {
                            i += 1;
                            continue;
                        };
                        ctx.emit(i, open + 1, Kind::Punctuation);
                        inline_link_text(ctx, open + 1, close);
                        ctx.emit(close, close + 2, Kind::Punctuation);
                        ctx.emit(close + 2, ref_end, Kind::String);
                        ctx.emit(ref_end, ref_end + 1, Kind::Punctuation);
                        i = ref_end + 1;
                    }
                    _ => i = open + 1,
                }
            }
            b'<' => {
                // Autolinks and inline HTML.
                let close = src[i..window(i)].iter().position(|&c| c == b'>').map(|p| i + p);
                match close {
                    Some(c) if src[i + 1..c].starts_with(b"http") || src[i + 1..c].contains(&b'@') => {
                        ctx.emit(i, i + 1, Kind::Punctuation);
                        ctx.emit(i + 1, c, Kind::Link);
                        ctx.emit(c, c + 1, Kind::Punctuation);
                        i = c + 1;
                    }
                    Some(c) if src.get(i + 1).is_some_and(|b| b.is_ascii_alphabetic() || *b == b'/' || *b == b'!') => {
                        ctx.embed(lang("html"), i, c + 1);
                        i = c + 1;
                    }
                    _ => i += 1,
                }
            }
            b'h' if src[i..end].starts_with(b"https://") || src[i..end].starts_with(b"http://") => {
                if i > start && is_ident(src[i - 1]) {
                    i += 1;
                    continue;
                }
                let e = src[i..end]
                    .iter()
                    .position(|&c| matches!(c, b' ' | b'\t' | b'\r' | b')' | b'>' | b'"' | b'\''))
                    .map_or(end, |p| i + p);
                ctx.emit(i, e, Kind::Link);
                i = e;
            }
            b'|' => {
                ctx.emit(i, i + 1, Kind::Punctuation);
                i += 1;
            }
            _ => i += 1,
        }
    }
}

fn inline_link_text(ctx: &mut Ctx, start: usize, end: usize) {
    ctx.emit(start, end, Kind::Link);
}

/// Offset of the `]` closing the `[` at `open`, allowing one level of nesting.
fn bracket_end(src: &[u8], open: usize, end: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, &b) in src[open..end].iter().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}
