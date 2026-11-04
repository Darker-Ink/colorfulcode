//! Documentation, typesetting and template languages.

use super::markup::PHP_SPEC;
use super::{lang, Definition, Engine};
use crate::lexer::generic::{features::*, Interp, Lexer, Spec, Str};
use crate::lexer::{is_ident, is_ident_start, Ctx};
use crate::{Kind, Language};

pub static LANGUAGES: &[&Definition] = &[
    &LATEX, &BIBTEX, &RST, &ASCIIDOC, &ORG, &TYPST, &GHERKIN, &HANDLEBARS, &JINJA, &TWIG, &LIQUID, &ERB, &EJS, &PUG,
    &HAML, &SLIM, &BLADE, &RAZOR, &GOTEMPLATE, &ROFF, &TEXINFO, &POD,
];

pub static LATEX: Definition = Definition::new("latex", "LaTeX", Engine::Custom(latex))
    .aliases(&["tex"])
    .extensions(&["tex", "ltx", "sty", "cls", "dtx"]);

pub static BIBTEX: Definition =
    Definition::new("bibtex", "BibTeX", Engine::Custom(bibtex)).aliases(&["bib"]).extensions(&["bib", "bibtex"]);

pub static RST: Definition = Definition::new("rst", "reStructuredText", Engine::Custom(rst))
    .aliases(&["restructuredtext", "rest"])
    .extensions(&["rst"]);

pub static ASCIIDOC: Definition = Definition::new("asciidoc", "AsciiDoc", Engine::Custom(asciidoc))
    .aliases(&["adoc"])
    .extensions(&["adoc", "asciidoc"]);

pub static ORG: Definition =
    Definition::new("org", "Org", Engine::Custom(org)).aliases(&["orgmode", "org-mode"]).extensions(&["org"]);

pub static TYPST: Definition =
    Definition::new("typst", "Typst", Engine::Custom(typst)).aliases(&["typ"]).extensions(&["typ"]);

pub static GHERKIN: Definition = Definition::new("gherkin", "Gherkin", Engine::Custom(gherkin))
    .aliases(&["cucumber", "feature"])
    .extensions(&["feature"]);

pub static HANDLEBARS: Definition =
    Definition::new("handlebars", "Handlebars", Engine::Custom(|ctx| template(ctx, &HANDLEBARS_TEMPLATE)))
        .aliases(&["hbs", "mustache", "htmlbars"])
        .extensions(&["hbs", "handlebars", "mustache"]);

pub static JINJA: Definition = Definition::new("jinja", "Jinja", Engine::Custom(|ctx| template(ctx, &JINJA_TEMPLATE)))
    .aliases(&["jinja2", "j2", "django", "htmldjango", "nunjucks", "njk"])
    .extensions(&["jinja", "jinja2", "j2", "njk", "nunjucks"]);

pub static TWIG: Definition =
    Definition::new("twig", "Twig", Engine::Custom(|ctx| template(ctx, &TWIG_TEMPLATE))).extensions(&["twig"]);

pub static LIQUID: Definition =
    Definition::new("liquid", "Liquid", Engine::Custom(|ctx| template(ctx, &LIQUID_TEMPLATE))).extensions(&["liquid"]);

pub static ERB: Definition = Definition::new("erb", "ERB", Engine::Custom(|ctx| template(ctx, &ERB_TEMPLATE)))
    .aliases(&["eruby", "rhtml"])
    .extensions(&["erb", "rhtml"]);

pub static EJS: Definition =
    Definition::new("ejs", "EJS", Engine::Custom(|ctx| template(ctx, &EJS_TEMPLATE))).extensions(&["ejs"]);

pub static PUG: Definition =
    Definition::new("pug", "Pug", Engine::Custom(pug)).aliases(&["jade"]).extensions(&["pug", "jade"]);

pub static HAML: Definition = Definition::new("haml", "Haml", Engine::Custom(haml)).extensions(&["haml"]);

pub static SLIM: Definition = Definition::new("slim", "Slim", Engine::Custom(slim)).extensions(&["slim"]);

pub static BLADE: Definition = Definition::new("blade", "Blade", Engine::Custom(|ctx| template(ctx, &BLADE_TEMPLATE)))
    .aliases(&["laravel-blade"])
    .extensions(&["blade.php", "blade"]);

pub static RAZOR: Definition = Definition::new("razor", "Razor", Engine::Custom(|ctx| template(ctx, &RAZOR_TEMPLATE)))
    .aliases(&["cshtml"])
    .extensions(&["cshtml", "razor"]);

pub static GOTEMPLATE: Definition =
    Definition::new("gotemplate", "Go Template", Engine::Custom(|ctx| template(ctx, &GO_TEMPLATE)))
        .aliases(&["gohtml", "go-template", "go-html-template", "gotmpl"])
        .extensions(&["gohtml", "tmpl", "gotmpl"]);

pub static ROFF: Definition = Definition::new("roff", "Roff", Engine::Custom(roff))
    .aliases(&["man", "groff", "nroff", "troff", "manpage"])
    .extensions(&[
        "man", "roff", "groff", "nroff", "troff", "mdoc", "tmac", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    ]);

pub static TEXINFO: Definition = Definition::new("texinfo", "Texinfo", Engine::Custom(texinfo))
    .aliases(&["texi"])
    .extensions(&["texi", "texinfo", "txi"]);

pub static POD: Definition =
    Definition::new("pod", "Pod", Engine::Custom(pod)).aliases(&["perlpod"]).extensions(&["pod", "pod6"]);

// Shared helpers

/// End of a name made of identifier bytes and any of `extra`.
fn name_end(ctx: &Ctx, from: usize, extra: &[u8]) -> usize {
    let mut i = from;
    loop {
        let b = ctx.at(i);
        if b == 0 || !(is_ident(b) || extra.contains(&b)) {
            return i;
        }
        i += 1;
    }
}

/// How far a closing bracket is searched for when a missing one would make
/// the lexer rescan the same text again and again.
const SCAN_LIMIT: usize = 1024;

/// `p + SCAN_LIMIT`, capped at `len`.
fn near(p: usize, len: usize) -> usize {
    (p + SCAN_LIMIT).min(len)
}

/// Offset of the bracket closing the one at `open`, before `limit`. Backslash
/// escapes are skipped, and so are quoted strings when `quotes` is set.
fn matching(src: &[u8], open: usize, limit: usize, quotes: bool) -> Option<usize> {
    let o = src[open];
    let c = match o {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b'<' => b'>',
        _ => return None,
    };
    let limit = limit.min(src.len());
    let mut depth = 0usize;
    let mut i = open;
    while i < limit {
        let b = src[i];
        if b == c {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        } else if b == o {
            depth += 1;
        } else if b == b'\\' {
            i += 1;
        } else if quotes && matches!(b, b'"' | b'\'' | b'`') {
            i += 1;
            while i < limit && src[i] != b {
                if src[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
        }
        i += 1;
    }
    None
}

/// Number of spaces and tabs at the start of the line beginning at `start`.
fn indent(ctx: &Ctx, start: usize) -> usize {
    ctx.skip_spaces_from(start) - start
}

/// The line `[start, end)` without a trailing carriage return.
fn line_text<'s>(ctx: &Ctx<'s, '_>, start: usize, end: usize) -> &'s [u8] {
    let line = &ctx.src[start..end];
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// End of the lines after the line ending at `line_end` that are blank or
/// indented deeper than `base`. Trailing blank lines are not included.
fn indented_block(ctx: &Ctx, line_end: usize, base: usize) -> usize {
    let mut end = line_end;
    let mut pos = line_end;
    while pos < ctx.len() {
        let s = pos + 1;
        let e = ctx.line_end(s);
        if !line_text(ctx, s, e).trim_ascii().is_empty() {
            if indent(ctx, s) <= base {
                break;
            }
            end = e;
        }
        pos = e;
    }
    end
}

/// Offset of `needle` at or after `from` when it ends before `end`.
fn find_in(ctx: &Ctx, from: usize, end: usize, needle: &[u8]) -> Option<usize> {
    let end = end.min(ctx.len());
    let mut i = from;
    while i + needle.len() <= end {
        let at = i + ctx.src[i..end].iter().position(|&b| b == needle[0])?;
        if ctx.src[at..end].starts_with(needle) {
            return Some(at);
        }
        i = at + 1;
    }
    None
}

/// Runs `body` over every line, keeping the cursor moving forward.
fn each_line(ctx: &mut Ctx, mut body: impl FnMut(&mut Ctx, usize, usize)) {
    while !ctx.done() {
        let start = ctx.pos;
        let end = ctx.line_end(start);
        body(ctx, start, end);
        if ctx.pos < end {
            ctx.pos = end;
        }
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

/// Runs `language` (by name) over a block, or marks it as a string if unknown.
fn embed_named(ctx: &mut Ctx, name: &[u8], start: usize, end: usize) {
    let name = std::str::from_utf8(name).unwrap_or("").trim();
    match Language::find(name).filter(|_| !name.is_empty()) {
        Some(language) => ctx.embed(language, start, end),
        None => ctx.emit(start, end, Kind::String),
    }
    ctx.pos = ctx.pos.max(end);
}

/// Emits `open`, the inside and `close` of a bracketed range.
fn emit_wrapped(ctx: &mut Ctx, open: usize, close: usize, kind: Kind) {
    ctx.emit(open, open + 1, Kind::Punctuation);
    ctx.emit(open + 1, close, kind);
    ctx.emit(close, close + 1, Kind::Punctuation);
    ctx.pos = close + 1;
}

/// Bare `http://` and `https://` links. Returns the end of the link.
fn url_end(src: &[u8], at: usize, end: usize) -> Option<usize> {
    let rest = &src[at..end];
    if !(rest.starts_with(b"https://") || rest.starts_with(b"http://")) {
        return None;
    }
    let mut e = rest
        .iter()
        .position(|&b| matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'"' | b'[' | b']'))
        .unwrap_or(rest.len());
    while e > 0 && matches!(rest[e - 1], b'.' | b',' | b')' | b';' | b':') {
        e -= 1;
    }
    Some(at + e)
}

// LaTeX

fn latex(ctx: &mut Ctx) {
    let end = ctx.len();
    tex(ctx, end, false);
}

/// Lexes TeX in `[ctx.pos, end)`. In math mode plain text is a string.
fn tex(ctx: &mut Ctx, end: usize, math: bool) {
    while ctx.pos < end {
        let start = ctx.pos;
        match ctx.at(start) {
            b'%' => {
                ctx.pos = ctx.line_end(start).min(end);
                ctx.emit(start, ctx.pos, Kind::Comment);
            }
            b'\\' => tex_command(ctx, end, math),
            b'$' if !math => tex_dollar(ctx, end),
            b'{' | b'}' | b'[' | b']' => ctx.token(1, Kind::Punctuation),
            b'&' => ctx.token(1, Kind::Operator),
            b'^' | b'_' if math => ctx.token(1, Kind::Operator),
            b'0'..=b'9' if math => {
                let mut e = start;
                while ctx.at(e).is_ascii_digit() || (ctx.at(e) == b'.' && ctx.at(e + 1).is_ascii_digit()) {
                    e += 1;
                }
                ctx.pos = e.min(end);
                ctx.emit(start, ctx.pos, Kind::Number);
            }
            _ => {
                let rest = &ctx.src[start..end];
                let n = rest
                    .iter()
                    .position(|&b| {
                        matches!(b, b'%' | b'\\' | b'$' | b'{' | b'}' | b'[' | b']' | b'&')
                            || (math && (b.is_ascii_digit() || b == b'^' || b == b'_'))
                    })
                    .unwrap_or(rest.len())
                    .max(1);
                ctx.pos = start + n;
                if math {
                    ctx.emit(start, ctx.pos, Kind::String);
                }
            }
        }
    }
    ctx.pos = end;
}

/// Inline `$...$` and display `$$...$$` math.
fn tex_dollar(ctx: &mut Ctx, end: usize) {
    let start = ctx.pos;
    let display = ctx.at(start + 1) == b'$';
    let open = 1 + display as usize;
    let body = start + open;
    let src = ctx.src;
    let mut i = body;
    let mut close = None;
    while i < end {
        match src[i] {
            b'\\' => i += 1,
            b'$' if !display || src.get(i + 1) == Some(&b'$') => {
                close = Some(i);
                break;
            }
            // Math never spans a paragraph break.
            b'\n' if ctx.at(ctx.skip_spaces_from(i + 1)) == b'\n' => break,
            _ => {}
        }
        i += 1;
    }
    match close {
        Some(c) if c + open <= end => {
            ctx.emit(start, body, Kind::String);
            ctx.pos = body;
            tex(ctx, c, true);
            ctx.pos = c + open;
            ctx.emit(c, ctx.pos, Kind::String);
        }
        _ => ctx.pos = body.min(end),
    }
}

const TEX_KEYWORDS: &[&[u8]] = &[
    b"begin", b"end", b"documentclass", b"usepackage", b"RequirePackage", b"newcommand", b"renewcommand",
    b"providecommand", b"newenvironment", b"renewenvironment", b"def", b"edef", b"gdef", b"xdef", b"let", b"input",
    b"include", b"includeonly", b"item", b"if", b"ifx", b"ifnum", b"else", b"fi", b"makeatletter", b"makeatother",
    b"maketitle", b"tableofcontents", b"appendix", b"bibliography", b"bibliographystyle", b"NeedsTeXFormat",
    b"ProvidesPackage", b"ProvidesClass", b"DeclareOption", b"ProcessOptions", b"newif", b"newcounter", b"setcounter",
    b"setlength", b"newlength", b"DeclareMathOperator", b"part", b"chapter", b"section", b"subsection",
    b"subsubsection", b"paragraph", b"subparagraph", b"title",
];

/// The kind of the main argument of a command, if it has a special one.
fn tex_argument(name: &[u8]) -> Option<Kind> {
    Some(match name {
        b"part" | b"chapter" | b"section" | b"subsection" | b"subsubsection" | b"paragraph" | b"subparagraph"
        | b"title" => Kind::Heading,
        b"emph" | b"textit" | b"textsl" => Kind::Emphasis,
        b"textbf" => Kind::Strong,
        b"texttt" => Kind::String,
        b"url" | b"href" | b"includegraphics" => Kind::Link,
        b"label" | b"ref" | b"eqref" | b"pageref" | b"cite" | b"citep" | b"citet" | b"autoref" | b"cref" | b"Cref"
        | b"nameref" => Kind::Constant,
        b"documentclass" | b"usepackage" | b"RequirePackage" | b"begin" | b"end" => Kind::Type,
        _ => return None,
    })
}

fn is_math_env(env: &[u8]) -> bool {
    let env = env.strip_suffix(b"*").unwrap_or(env);
    matches!(
        env,
        b"equation"
            | b"align"
            | b"alignat"
            | b"gather"
            | b"multline"
            | b"flalign"
            | b"eqnarray"
            | b"math"
            | b"displaymath"
    )
}

/// End of the paragraph containing `from`: the next blank line, within reach.
fn tex_paragraph_end(src: &[u8], from: usize, end: usize) -> usize {
    let end = near(from, end);
    let mut i = from;
    while let Some(k) = src[i..end].iter().position(|&b| b == b'\n') {
        let line = i + k + 1;
        let next = src[line..end].iter().position(|&b| !matches!(b, b' ' | b'\t' | b'\r')).map(|n| line + n);
        match next {
            Some(n) if src[n] == b'\n' => return line,
            Some(_) => i = line,
            None => return end,
        }
    }
    end
}

/// Offset of `\end{env}` at or after `from`.
fn tex_find_end(ctx: &Ctx, from: usize, end: usize, env: &[u8]) -> Option<usize> {
    let mut i = from;
    while let Some(p) = find_in(ctx, i, end, b"\\end{") {
        let name = p + 5;
        if ctx.src[name..end].starts_with(env) && ctx.at(name + env.len()) == b'}' {
            return Some(p);
        }
        i = p + 1;
    }
    None
}

fn tex_command(ctx: &mut Ctx, end: usize, math: bool) {
    let start = ctx.pos;
    let first = ctx.at(start + 1);
    if !(first.is_ascii_alphabetic() || first == b'@') {
        // `\(...\)` and `\[...\]` math, or a control symbol such as `\%`.
        if !math && (first == b'(' || first == b'[') {
            let closer: &[u8] = if first == b'(' { b"\\)" } else { b"\\]" };
            if let Some(c) = find_in(ctx, start + 2, tex_paragraph_end(ctx.src, start, end), closer) {
                ctx.emit(start, start + 2, Kind::String);
                ctx.pos = start + 2;
                tex(ctx, c, true);
                ctx.pos = c + 2;
                ctx.emit(c, c + 2, Kind::String);
                return;
            }
        }
        ctx.pos = start + 1;
        if ctx.pos < end {
            ctx.bump_char();
        }
        ctx.pos = ctx.pos.min(end);
        ctx.emit(start, ctx.pos, Kind::Escape);
        return;
    }
    let mut e = start + 1;
    while e < end && (ctx.at(e).is_ascii_alphabetic() || ctx.at(e) == b'@') {
        e += 1;
    }
    let name = &ctx.src[start + 1..e];
    if ctx.at(e) == b'*' && e < end {
        e += 1;
    }
    let keyword = TEX_KEYWORDS.contains(&name);
    ctx.pos = e;
    ctx.emit(start, e, if keyword { Kind::Keyword } else { Kind::Function });
    if name == b"verb" {
        let delim = ctx.at(e);
        if delim != 0 && !delim.is_ascii_alphabetic() && delim < 0x80 {
            let line_end = ctx.line_end(e).min(end);
            if let Some(i) = ctx.src[e + 1..line_end].iter().position(|&b| b == delim) {
                ctx.pos = e + 1 + i + 1;
                ctx.emit(e, ctx.pos, Kind::String);
            }
        }
        return;
    }
    let Some(kind) = tex_argument(name) else {
        return;
    };
    let mut p = ctx.skip_spaces_from(e);
    if ctx.at(p) == b'[' && name != b"begin" {
        if let Some(c) = matching(ctx.src, p, tex_paragraph_end(ctx.src, p, end), false) {
            emit_wrapped(ctx, p, c, Kind::Text);
            p = ctx.skip_spaces_from(c + 1);
        }
    }
    if ctx.at(p) != b'{' {
        return;
    }
    let Some(c) = matching(ctx.src, p, tex_paragraph_end(ctx.src, p, end), false) else {
        return;
    };
    emit_wrapped(ctx, p, c, kind);
    if name == b"begin" {
        let env = &ctx.src[p + 1..c];
        tex_environment(ctx, env, end, math);
    }
}

/// The body of `\begin{env}` for environments that are not plain TeX.
fn tex_environment(ctx: &mut Ctx, env: &[u8], end: usize, math: bool) {
    let body = ctx.pos;
    let verbatim = matches!(env, b"verbatim" | b"verbatim*" | b"Verbatim" | b"lstlisting" | b"minted" | b"comment");
    if !verbatim && (math || !is_math_env(env)) {
        return;
    }
    let close = tex_find_end(ctx, body, end, env).unwrap_or(end);
    if !verbatim {
        tex(ctx, close, true);
        return;
    }
    let mut code = body;
    let mut language: &[u8] = b"";
    // Options and the language: `[language=Python]` or `{python}`.
    if ctx.at(code) == b'[' {
        if let Some(c) = matching(ctx.src, code, close, false) {
            let opts = &ctx.src[code + 1..c];
            if let Some(i) = opts.windows(9).position(|w| w == b"language=") {
                let v = &opts[i + 9..];
                language = &v[..v.iter().position(|&b| b == b',' || b == b']').unwrap_or(v.len())];
            }
            emit_wrapped(ctx, code, c, Kind::Text);
            code = c + 1;
        }
    }
    if env == b"minted" && ctx.at(code) == b'{' {
        if let Some(c) = matching(ctx.src, code, close, false) {
            language = &ctx.src[code + 1..c];
            emit_wrapped(ctx, code, c, Kind::Attribute);
            code = c + 1;
        }
    }
    if env == b"comment" {
        ctx.emit(code, close, Kind::Comment);
    } else if language.is_empty() {
        ctx.emit(code, close, Kind::String);
    } else {
        embed_named(ctx, language, code, close);
    }
    ctx.pos = close;
}

// BibTeX

fn bibtex(ctx: &mut Ctx) {
    while !ctx.done() {
        let rest = &ctx.src[ctx.pos..];
        let skip = rest.iter().position(|&b| b == b'@' || b == b'%').unwrap_or(rest.len());
        ctx.pos += skip;
        match ctx.peek() {
            b'%' => {
                let start = ctx.pos;
                ctx.skip_line();
                ctx.emit_from(start, Kind::Comment);
            }
            b'@' => bib_entry(ctx),
            _ => {}
        }
    }
}

fn skip_whitespace_from(ctx: &Ctx, mut i: usize) -> usize {
    while matches!(ctx.at(i), b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

fn bib_entry(ctx: &mut Ctx) {
    let start = ctx.pos;
    let kind_end = name_end(ctx, start + 1, b"");
    if kind_end == start + 1 {
        ctx.pos += 1;
        return;
    }
    ctx.emit(start, kind_end, Kind::Keyword);
    ctx.pos = kind_end;
    let open = skip_whitespace_from(ctx, kind_end);
    if !matches!(ctx.at(open), b'{' | b'(') {
        return;
    }
    let kind = &ctx.src[start + 1..kind_end];
    if kind.eq_ignore_ascii_case(b"comment") {
        let close = matching(ctx.src, open, ctx.len(), false).map_or(ctx.len(), |c| c + 1);
        ctx.pos = close;
        ctx.emit(open, close, Kind::Comment);
        return;
    }
    let macros = kind.eq_ignore_ascii_case(b"string");
    let close_byte = if ctx.at(open) == b'{' { b'}' } else { b')' };
    ctx.emit(open, open + 1, Kind::Punctuation);
    ctx.pos = open + 1;
    if !macros && !kind.eq_ignore_ascii_case(b"preamble") {
        // The citation key.
        let key = skip_whitespace_from(ctx, ctx.pos);
        let mut e = key;
        while !matches!(ctx.at(e), b',' | b' ' | b'\t' | b'\n' | b'\r' | b'}' | b')' | 0) {
            e += 1;
        }
        ctx.emit(key, e, Kind::Constant);
        ctx.pos = e;
    }
    loop {
        ctx.pos = skip_whitespace_from(ctx, ctx.pos);
        let p = ctx.pos;
        match ctx.peek() {
            0 | b'@' => return,
            b if b == close_byte => {
                ctx.token(1, Kind::Punctuation);
                return;
            }
            b',' => ctx.token(1, Kind::Punctuation),
            b'=' | b'#' => ctx.token(1, Kind::Operator),
            b'{' => {
                ctx.pos = matching(ctx.src, p, ctx.len(), false).map_or(ctx.len(), |c| c + 1);
                ctx.emit(p, ctx.pos, Kind::String);
            }
            b'"' => {
                let mut i = p + 1;
                let mut depth = 0;
                while i < ctx.len() && !(ctx.src[i] == b'"' && depth == 0) {
                    match ctx.src[i] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        b'\\' => i += 1,
                        _ => {}
                    }
                    i += 1;
                }
                ctx.pos = (i + 1).min(ctx.len());
                ctx.emit(p, ctx.pos, Kind::String);
            }
            b'0'..=b'9' => {
                let e = name_end(ctx, p, b"");
                ctx.pos = e;
                ctx.emit(p, e, Kind::Number);
            }
            b if is_ident_start(b) => {
                let e = name_end(ctx, p, b"-:.+");
                let field = ctx.next_non_space(e) == b'=';
                ctx.pos = e;
                ctx.emit(p, e, if field && !macros { Kind::Property } else { Kind::Variable });
            }
            _ => ctx.bump_char(),
        }
    }
}

// Templates
//
// Template languages are HTML with embedded tags. The driver finds the tag
// delimiters, runs HTML over the text between them and lexes each tag with a
// small spec or an embedded language.

/// Where the host HTML stopped at the end of a text segment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Html {
    Text,
    /// Inside a tag. The value says whether its content is raw text.
    Tag(u8),
    /// Inside a quoted attribute value: the quote and the raw flag.
    Quote(u8, u8),
    Comment,
    /// Inside a `<script>` or `<style>` element.
    Raw(u8),
}

const SCRIPT: u8 = 1;
const STYLE: u8 = 2;

fn raw_close(raw: u8) -> &'static [u8] {
    if raw == STYLE {
        b"</style"
    } else {
        b"</script"
    }
}

/// Case insensitive search for an ASCII `needle` in `[from, end)`.
fn find_ignore_case(src: &[u8], from: usize, end: usize, needle: &[u8]) -> Option<usize> {
    let last = end.checked_sub(needle.len())?;
    (from..=last).find(|&i| src[i..i + needle.len()].eq_ignore_ascii_case(needle))
}

/// Lexes host HTML in `[start, end)`, continuing from `state`.
fn html_text(ctx: &mut Ctx, start: usize, end: usize, state: &mut Html) {
    let src = ctx.src;
    let mut pos = start;
    while pos < end {
        match *state {
            Html::Text => {
                ctx.embed(lang("html"), pos, end);
                *state = html_scan(src, pos, end);
                pos = end;
            }
            Html::Comment => {
                let close = find_in(ctx, pos, end, b"-->");
                let stop = close.map_or(end, |p| p + 3);
                ctx.emit(pos, stop, Kind::Comment);
                if close.is_some() {
                    *state = Html::Text;
                }
                pos = stop;
            }
            Html::Quote(q, raw) => match src[pos..end].iter().position(|&b| b == q) {
                Some(i) => {
                    ctx.emit(pos, pos + i + 1, Kind::String);
                    pos += i + 1;
                    *state = Html::Tag(raw);
                }
                None => {
                    ctx.emit(pos, end, Kind::String);
                    pos = end;
                }
            },
            Html::Tag(raw) => match src[pos] {
                b'>' => {
                    ctx.emit(pos, pos + 1, Kind::Punctuation);
                    pos += 1;
                    *state = if raw != 0 { Html::Raw(raw) } else { Html::Text };
                }
                b'/' if src.get(pos + 1) == Some(&b'>') && pos + 2 <= end => {
                    ctx.emit(pos, pos + 2, Kind::Punctuation);
                    pos += 2;
                    *state = Html::Text;
                }
                q @ (b'"' | b'\'') => {
                    ctx.emit(pos, pos + 1, Kind::String);
                    pos += 1;
                    *state = Html::Quote(q, raw);
                }
                b'=' => {
                    ctx.emit(pos, pos + 1, Kind::Operator);
                    pos += 1;
                }
                b'<' => *state = Html::Text,
                b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
                _ => {
                    let mut e = pos;
                    while e < end
                        && !matches!(src[e], b' ' | b'\t' | b'\n' | b'\r' | b'=' | b'>' | b'"' | b'\'' | b'<' | b'/')
                    {
                        e += 1;
                    }
                    let e = if e == pos { pos + 1 } else { e };
                    ctx.emit(pos, e, Kind::Attribute);
                    pos = e;
                }
            },
            Html::Raw(raw) => {
                let close = find_ignore_case(src, pos, end, raw_close(raw));
                let stop = close.unwrap_or(end);
                ctx.embed(if raw == STYLE { lang("css") } else { lang("javascript") }, pos, stop);
                if close.is_some() {
                    *state = Html::Text;
                }
                pos = stop;
            }
        }
    }
    ctx.pos = ctx.pos.max(end);
}

/// Works out where HTML lexing of `[start, end)` stopped.
fn html_scan(src: &[u8], start: usize, end: usize) -> Html {
    let mut state = Html::Text;
    let mut i = start;
    while i < end {
        let b = src[i];
        match state {
            Html::Text if b == b'<' => {
                if src[i..end].starts_with(b"<!--") {
                    state = Html::Comment;
                    i += 4;
                    continue;
                }
                let next = src.get(i + 1).copied().unwrap_or(0);
                if next.is_ascii_alphabetic() || next == b'/' || next == b'!' {
                    let mut j = i + 1;
                    while j < end && (is_ident(src[j]) || src[j] == b'-') {
                        j += 1;
                    }
                    let name = &src[i + 1..j];
                    let raw = if name.eq_ignore_ascii_case(b"script") {
                        SCRIPT
                    } else if name.eq_ignore_ascii_case(b"style") {
                        STYLE
                    } else {
                        0
                    };
                    state = Html::Tag(raw);
                    i = j;
                    continue;
                }
            }
            Html::Tag(raw) => match b {
                b'"' | b'\'' => state = Html::Quote(b, raw),
                b'>' => state = if raw != 0 && src[i - 1] != b'/' { Html::Raw(raw) } else { Html::Text },
                _ => {}
            },
            Html::Quote(q, raw) if b == q => state = Html::Tag(raw),
            Html::Comment if src[i..end].starts_with(b"-->") => {
                state = Html::Text;
                i += 3;
                continue;
            }
            Html::Raw(raw)
                if b == b'<' && {
                    let close = raw_close(raw);
                    src[i..end].len() >= close.len() && src[i..i + close.len()].eq_ignore_ascii_case(close)
                } =>
            {
                state = Html::Text;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    state
}

/// How the inside of a template tag is lexed.
#[derive(Clone, Copy)]
enum Body {
    Comment,
    /// An escaped delimiter such as `<%%`, left as text.
    Literal,
    Expr(&'static Spec),
    /// Like `Expr`, but a leading word followed by arguments is a helper call.
    Helper(&'static Spec),
    /// The first word names the tag, as in `{% if x %}`.
    Stmt(&'static Spec),
    /// Code in another language, by id.
    Code(&'static str),
}

struct Delim {
    open: &'static [u8],
    close: &'static [u8],
    body: Body,
}

const fn delim(open: &'static str, close: &'static str, body: Body) -> Delim {
    Delim { open: open.as_bytes(), close: close.as_bytes(), body }
}

type TextHook = fn(&mut Tpl<'_, '_, '_>, usize) -> Option<usize>;

struct Template {
    /// Tag delimiters, longest first.
    delims: &'static [Delim],
    /// First bytes of the delimiters and hook triggers.
    starts: &'static [u8],
    /// Whitespace control markers allowed just inside the delimiters.
    trim: &'static [u8],
    /// Kind of the delimiters themselves.
    mark: Kind,
    /// Text level constructs such as Blade directives. Returns where to resume scanning.
    hook: Option<TextHook>,
}

struct Tpl<'c, 's, 'o> {
    ctx: &'c mut Ctx<'s, 'o>,
    html: Html,
    /// Start of the host text not lexed yet.
    text: usize,
    /// Open code blocks whose `}` is still to come (Razor).
    depth: u32,
}

impl Tpl<'_, '_, '_> {
    /// Lexes the pending host text up to `upto`.
    fn flush(&mut self, upto: usize) {
        if upto > self.text {
            html_text(self.ctx, self.text, upto, &mut self.html);
        }
        self.text = self.text.max(upto);
    }

    /// Lexes the tag at `at` and returns its end.
    fn tag(&mut self, at: usize, d: &Delim, t: &Template) -> usize {
        let len = self.ctx.len();
        let src = self.ctx.src;
        let mut inner = (at + d.open.len()).min(len);
        if t.trim.contains(&self.ctx.at(inner)) {
            inner += 1;
        }
        let close = match d.body {
            Body::Comment => find_in(self.ctx, inner, len, d.close),
            _ => tag_close(src, inner, d.close),
        }
        .unwrap_or(len);
        let mut inner_end = close;
        if inner_end > inner && t.trim.contains(&src[inner_end - 1]) {
            inner_end -= 1;
        }
        let end = (close + d.close.len()).min(len);
        let ctx = &mut *self.ctx;
        let comment = matches!(d.body, Body::Comment) || (matches!(d.body, Body::Helper(_)) && ctx.at(inner) == b'!');
        if comment {
            ctx.emit(at, end, Kind::Comment);
            ctx.pos = end;
            return end;
        }
        ctx.emit(at, inner, t.mark);
        let mut word: &[u8] = b"";
        match d.body {
            Body::Expr(spec) => ctx.embed_spec(spec, inner, inner_end),
            Body::Helper(spec) => helper(ctx, spec, inner, inner_end),
            Body::Stmt(spec) => {
                let w = skip_whitespace_from(ctx, inner).min(inner_end);
                let e = name_end(ctx, w, b"").min(inner_end);
                word = &src[w..e];
                ctx.emit(w, e, Kind::Keyword);
                // Re-lexing the tag name lets the spec see it, as in `block name`.
                ctx.embed_spec(spec, w, inner_end);
            }
            Body::Code(id) => ctx.embed(lang(id), inner, inner_end),
            Body::Comment | Body::Literal => {}
        }
        ctx.emit(inner_end, end, t.mark);
        ctx.pos = end;
        // Raw and comment blocks run up to their end tag.
        let end_word: &[u8] = match word {
            b"raw" => b"endraw",
            b"verbatim" => b"endverbatim",
            b"comment" => b"endcomment",
            _ => return end,
        };
        let open = find_in(ctx, end, len, end_word)
            .and_then(|p| src[end..p].windows(d.open.len()).rposition(|w| w == d.open))
            .map_or(len, |i| end + i);
        if word == b"comment" {
            ctx.emit(end, open, Kind::Comment);
        } else {
            html_text(ctx, end, open, &mut self.html);
        }
        ctx.pos = open;
        open
    }
}

/// Offset of the closing delimiter `close` at or after `from`, skipping quoted
/// strings so that `{{ "}}" }}` stays one tag.
fn tag_close(src: &[u8], from: usize, close: &[u8]) -> Option<usize> {
    let mut i = from;
    while i < src.len() {
        let b = src[i];
        if src[i..].starts_with(close) {
            return Some(i);
        }
        if matches!(b, b'"' | b'\'' | b'`') {
            let limit = near(i, src.len());
            let mut j = i + 1;
            while j < limit && src[j] != b && (b == b'`' || src[j] != b'\n') {
                j += 1 + (src[j] == b'\\' && b != b'`') as usize;
            }
            if j < limit && src[j] == b {
                i = j;
            }
        }
        i += 1;
    }
    None
}

/// Runs a template language over the whole input.
fn template(ctx: &mut Ctx, t: &'static Template) {
    let start = ctx.pos;
    let len = ctx.len();
    let mut tp = Tpl { ctx, html: Html::Text, text: start, depth: 0 };
    let mut i = start;
    while i < len {
        let src = tp.ctx.src;
        match src[i..].iter().position(|b| t.starts.contains(b)) {
            Some(off) => i += off,
            None => break,
        }
        if let Some(d) = t.delims.iter().find(|d| src[i..].starts_with(d.open)) {
            if let Body::Literal = d.body {
                i += d.open.len();
                continue;
            }
            tp.flush(i);
            i = tp.tag(i, d, t).max(i + 1);
            tp.text = i;
            continue;
        }
        match t.hook.and_then(|hook| hook(&mut tp, i)) {
            Some(next) => i = next.max(i + 1),
            None => i += 1,
        }
    }
    tp.flush(len);
    tp.ctx.pos = len;
}

/// Handlebars and Go template tags: block markers and helper calls.
fn helper(ctx: &mut Ctx, spec: &'static Spec, inner: usize, end: usize) {
    let src = ctx.src;
    let mut p = skip_whitespace_from(ctx, inner).min(end);
    let mut call = true;
    let b = ctx.at(p);
    if p < end && matches!(b, b'#' | b'/' | b'^' | b'>' | b'&') && ctx.at(p + 1) != b'*' {
        let e = if matches!(b, b'>' | b'&') { p + 1 } else { name_end(ctx, p + 1, b"-") }.min(end);
        ctx.emit(p, e, if b == b'&' { Kind::Operator } else { Kind::Keyword });
        call = b == b'>';
        p = ctx.skip_spaces_from(e).min(end);
    }
    if is_ident_start(ctx.at(p)) && p < end {
        let e = name_end(ctx, p, b"-").min(end);
        let word = &src[p..e];
        let args = ctx.skip_spaces_from(e);
        let known = [spec.keywords, spec.builtins, spec.constants, spec.variables]
            .iter()
            .any(|list| list.iter().any(|w| w.as_bytes() == word));
        if !known && (b == b'>' || (call && args > e && args < end)) {
            ctx.emit(p, e, Kind::Function);
            p = e;
        }
    }
    ctx.embed_spec(spec, p, end);
}

/// `| name`: the word after a pipe is a filter.
fn filter_hook(lx: &mut Lexer) -> bool {
    if lx.ctx.peek_at(1) == b'|' {
        return false;
    }
    lx.ctx.token(1, Kind::Operator);
    let p = lx.ctx.skip_spaces_from(lx.ctx.pos);
    if is_ident_start(lx.ctx.at(p)) {
        let e = lx.scan_ident(p);
        lx.ctx.pos = e;
        lx.ctx.emit(p, e, Kind::Function);
        lx.mark_value();
    } else {
        lx.mark_operator();
    }
    true
}

// Jinja, Twig and Liquid

static JINJA_SPEC: Spec = Spec {
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "if", "elif", "else", "endif", "for", "endfor", "in", "not", "and", "or", "is", "block", "endblock", "extends",
        "include", "import", "from", "as", "with", "without", "context", "endwith", "set", "endset", "macro",
        "endmacro", "call", "endcall", "filter", "endfilter", "raw", "endraw", "autoescape", "endautoescape",
        "recursive", "ignore", "missing", "do", "break", "continue", "trans", "endtrans", "pluralize", "scoped",
        "required", "load", "url", "csrf_token", "empty", "spaceless", "endspaceless", "comment", "endcomment", "now",
    ],
    constants: &["true", "false", "none", "True", "False", "None"],
    variables: &["loop", "self", "caller", "varargs", "kwargs"],
    builtins: &["range", "dict", "lipsum", "cycler", "joiner", "namespace", "super"],
    function_keywords: &["macro", "block"],
    operators: "+-*/%=<>!~?:",
    features: CALLS | MEMBERS,
    hook: Some(filter_hook),
    hook_chars: "|",
    ..Spec::DEFAULT
};

static TWIG_SPEC: Spec = Spec {
    strings: &[Str::quoted("\"").interp(Interp::HashBrace), Str::quoted("'")],
    keywords: &[
        "if", "elseif", "else", "endif", "for", "endfor", "in", "not", "and", "or", "is", "block", "endblock",
        "extends", "include", "import", "from", "as", "with", "only", "embed", "endembed", "use", "set", "endset",
        "macro", "endmacro", "apply", "endapply", "autoescape", "endautoescape", "verbatim", "endverbatim", "sandbox",
        "endsandbox", "do", "flush", "deprecated", "cache", "endcache", "ignore", "missing", "matches", "starts",
        "ends", "defined", "same", "divisible", "by", "guard", "endguard",
    ],
    constants: &["true", "false", "null", "none"],
    variables: &["loop", "_self", "_context", "_charset"],
    builtins: &["range", "constant", "cycle", "date", "dump", "parent", "attribute", "source", "max", "min", "random"],
    function_keywords: &["macro", "block"],
    operators: "+-*/%=<>!~?:",
    features: CALLS | MEMBERS | IDENT_KEYS,
    hook: Some(filter_hook),
    hook_chars: "|",
    ..Spec::DEFAULT
};

static LIQUID_SPEC: Spec = Spec {
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "if", "elsif", "else", "endif", "unless", "endunless", "case", "when", "endcase", "for", "endfor", "in",
        "break", "continue", "cycle", "tablerow", "endtablerow", "assign", "capture", "endcapture", "increment",
        "decrement", "include", "render", "layout", "section", "sections", "schema", "endschema", "form", "endform",
        "paginate", "endpaginate", "with", "as", "and", "or", "contains", "liquid", "echo", "comment", "endcomment",
        "raw", "endraw", "limit", "offset", "reversed", "by", "javascript", "endjavascript", "style", "endstyle",
    ],
    constants: &["true", "false", "nil", "null", "empty", "blank"],
    variables: &["forloop", "tablerowloop"],
    operators: "=<>!",
    features: MEMBERS | IDENT_KEYS,
    hook: Some(filter_hook),
    hook_chars: "|",
    ..Spec::DEFAULT
};

static JINJA_TEMPLATE: Template = Template {
    delims: &[
        delim("{#", "#}", Body::Comment),
        delim("{%", "%}", Body::Stmt(&JINJA_SPEC)),
        delim("{{", "}}", Body::Expr(&JINJA_SPEC)),
    ],
    starts: b"{",
    trim: b"-+",
    mark: Kind::Punctuation,
    hook: None,
};

static TWIG_TEMPLATE: Template = Template {
    delims: &[
        delim("{#", "#}", Body::Comment),
        delim("{%", "%}", Body::Stmt(&TWIG_SPEC)),
        delim("{{", "}}", Body::Expr(&TWIG_SPEC)),
    ],
    starts: b"{",
    trim: b"-~",
    mark: Kind::Punctuation,
    hook: None,
};

static LIQUID_TEMPLATE: Template = Template {
    delims: &[delim("{%", "%}", Body::Stmt(&LIQUID_SPEC)), delim("{{", "}}", Body::Expr(&LIQUID_SPEC))],
    starts: b"{",
    trim: b"-",
    mark: Kind::Punctuation,
    hook: None,
};

// Handlebars and Go templates

/// `key=value` hash arguments.
fn handlebars_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let at = ctx.pos;
    if ctx.peek_at(1) == b'=' {
        return false;
    }
    let mut s = at;
    while s > 0 && (is_ident(ctx.src[s - 1]) || ctx.src[s - 1] == b'-') {
        s -= 1;
    }
    if s < at {
        ctx.truncate_to(s);
        ctx.emit(s, at, Kind::Property);
    }
    ctx.token(1, Kind::Operator);
    lx.mark_operator();
    true
}

static HANDLEBARS_SPEC: Spec = Spec {
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &["if", "unless", "each", "with", "else", "as", "lookup", "log", "let", "in", "yield"],
    constants: &["true", "false", "null", "undefined"],
    variables: &["this"],
    sigils: &[(b'@', Kind::Variable)],
    ident_chars: "-",
    features: MEMBERS | LISP_CALLS,
    hook: Some(handlebars_hook),
    hook_chars: "=",
    ..Spec::DEFAULT
};

static HANDLEBARS_TEMPLATE: Template = Template {
    delims: &[
        delim("\\{{", "", Body::Literal),
        delim("{{!--", "--}}", Body::Comment),
        delim("{{!", "}}", Body::Comment),
        delim("{{{", "}}}", Body::Helper(&HANDLEBARS_SPEC)),
        delim("{{", "}}", Body::Helper(&HANDLEBARS_SPEC)),
    ],
    starts: b"{\\",
    trim: b"~",
    mark: Kind::Punctuation,
    hook: None,
};

/// Pipes and the bare `$` (the root context).
fn go_template_hook(lx: &mut Lexer) -> bool {
    match lx.ctx.peek() {
        b'$' if !is_ident(lx.ctx.peek_at(1)) => {
            lx.ctx.token(1, Kind::Variable);
            lx.mark_value();
            true
        }
        b'|' => filter_hook(lx),
        _ => false,
    }
}

static GO_TEMPLATE_SPEC: Spec = Spec {
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("`").raw().multiline(), Str::quoted("'")],
    keywords: &["if", "else", "end", "range", "with", "define", "template", "block", "break", "continue"],
    builtins: &[
        "and", "or", "not", "len", "index", "slice", "print", "printf", "println", "eq", "ne", "lt", "le", "gt", "ge",
        "call", "html", "js", "urlquery", "include", "default", "quote", "toYaml", "toJson", "nindent", "indent",
        "required", "tpl", "trim", "upper", "lower",
    ],
    constants: &["true", "false", "nil"],
    sigils: &[(b'$', Kind::Variable)],
    operators: "=:!",
    features: MEMBERS | LISP_CALLS,
    hook: Some(go_template_hook),
    hook_chars: "|$",
    ..Spec::DEFAULT
};

static GO_TEMPLATE: Template = Template {
    delims: &[delim("{{", "}}", Body::Helper(&GO_TEMPLATE_SPEC))],
    starts: b"{",
    trim: b"-",
    mark: Kind::Punctuation,
    hook: None,
};

// ERB and EJS

static ERB_TEMPLATE: Template = Template {
    delims: &[
        delim("<%#", "%>", Body::Comment),
        delim("<%%", "", Body::Literal),
        delim("<%=", "%>", Body::Code("ruby")),
        delim("<%-", "%>", Body::Code("ruby")),
        delim("<%", "%>", Body::Code("ruby")),
    ],
    starts: b"<",
    trim: b"-=",
    mark: Kind::Meta,
    hook: None,
};

static EJS_TEMPLATE: Template = Template {
    delims: &[
        delim("<%#", "%>", Body::Comment),
        delim("<%%", "", Body::Literal),
        delim("<%=", "%>", Body::Code("javascript")),
        delim("<%-", "%>", Body::Code("javascript")),
        delim("<%_", "%>", Body::Code("javascript")),
        delim("<%", "%>", Body::Code("javascript")),
    ],
    starts: b"<",
    trim: b"-_",
    mark: Kind::Meta,
    hook: None,
};

// Blade

static BLADE_TEMPLATE: Template = Template {
    delims: &[
        delim("{{--", "--}}", Body::Comment),
        delim("{!!", "!!}", Body::Expr(&PHP_SPEC)),
        delim("{{", "}}", Body::Expr(&PHP_SPEC)),
    ],
    starts: b"{@",
    trim: b"",
    mark: Kind::Punctuation,
    hook: Some(blade_hook),
};

/// `@directive`, `@directive(args)`, `@php ... @endphp` and `@verbatim`.
fn blade_hook(tp: &mut Tpl, at: usize) -> Option<usize> {
    let src = tp.ctx.src;
    if at > 0 && (is_ident(src[at - 1]) || src[at - 1] == b'.') {
        return None;
    }
    let next = tp.ctx.at(at + 1);
    if next == b'@' || (next == b'{' && tp.ctx.at(at + 2) == b'{') {
        // Escaped: `@@if` and `@{{ shown as is }}`.
        return Some(at + 3);
    }
    if !next.is_ascii_alphabetic() {
        return None;
    }
    let e = name_end(tp.ctx, at + 1, b"");
    let name = &src[at + 1..e];
    tp.flush(at);
    let ctx = &mut *tp.ctx;
    ctx.emit(at, e, Kind::Keyword);
    let mut end = e;
    let p = ctx.skip_spaces_from(e);
    if ctx.at(p) == b'(' {
        if let Some(c) = matching(src, p, near(p, ctx.len()), true) {
            ctx.emit(p, p + 1, Kind::Punctuation);
            ctx.embed_spec(&PHP_SPEC, p + 1, c);
            ctx.emit(c, c + 1, Kind::Punctuation);
            end = c + 1;
        }
    } else if name == b"php" || name == b"verbatim" {
        let closer: &[u8] = if name == b"php" { b"@endphp" } else { b"@endverbatim" };
        let c = ctx.find(e, closer).unwrap_or(ctx.len());
        if name == b"php" {
            ctx.embed_spec(&PHP_SPEC, e, c);
        } else {
            html_text(ctx, e, c, &mut tp.html);
        }
        end = c;
    }
    ctx.pos = end;
    tp.text = end;
    Some(end)
}

// Razor

static RAZOR_TEMPLATE: Template =
    Template { delims: &[], starts: b"@}", trim: b"", mark: Kind::Punctuation, hook: Some(razor_hook) };

const RAZOR_BLOCKS: &[&[u8]] = &[b"if", b"for", b"foreach", b"while", b"switch", b"lock", b"try", b"do", b"section"];

const RAZOR_DIRECTIVES: &[&[u8]] = &[
    b"model", b"using", b"inject", b"page", b"inherits", b"namespace", b"layout", b"implements", b"addTagHelper",
    b"removeTagHelper", b"attribute", b"rendermode", b"typeparam", b"preservewhitespace",
];

fn razor_hook(tp: &mut Tpl, at: usize) -> Option<usize> {
    let src = tp.ctx.src;
    let len = src.len();
    if src[at] == b'}' {
        // The end of a code block opened by `@if (...) {` and friends.
        if tp.depth == 0 || !tp.ctx.at_line_start(at) {
            return None;
        }
        tp.flush(at);
        tp.depth -= 1;
        tp.ctx.emit(at, at + 1, Kind::Punctuation);
        let p = skip_whitespace_from(tp.ctx, at + 1);
        let w = name_end(tp.ctx, p, b"");
        let end = if matches!(&src[p..w], b"else" | b"catch" | b"finally" | b"while") {
            razor_block(tp, p, w)
        } else {
            at + 1
        };
        tp.ctx.pos = end;
        tp.text = end;
        return Some(end);
    }
    if at > 0 && is_ident(src[at - 1]) {
        return None;
    }
    let next = tp.ctx.at(at + 1);
    let end = match next {
        b'@' => return Some(at + 2),
        b'*' => {
            tp.flush(at);
            let end = tp.ctx.find(at + 2, b"*@").map_or(len, |p| p + 2);
            tp.ctx.emit(at, end, Kind::Comment);
            end
        }
        b'{' | b'(' => {
            tp.flush(at);
            let close = matching(src, at + 1, len, true).unwrap_or(len);
            let ctx = &mut *tp.ctx;
            ctx.emit(at, at + 2, Kind::Punctuation);
            ctx.embed(lang("csharp"), at + 2, close);
            ctx.emit(close, close + 1, Kind::Punctuation);
            (close + 1).min(len)
        }
        b':' => {
            tp.flush(at);
            tp.ctx.emit(at, at + 2, Kind::Punctuation);
            at + 2
        }
        _ if is_ident_start(next) => {
            let w = name_end(tp.ctx, at + 1, b"");
            let word = &src[at + 1..w];
            tp.flush(at);
            if RAZOR_BLOCKS.contains(&word) || (word == b"using" && tp.ctx.next_non_space(w) == b'(') {
                razor_block(tp, at, w)
            } else if word == b"code" || word == b"functions" {
                let ctx = &mut *tp.ctx;
                ctx.emit(at, w, Kind::Keyword);
                let p = skip_whitespace_from(ctx, w);
                if ctx.at(p) == b'{' {
                    let c = matching(src, p, len, true).unwrap_or(len);
                    ctx.emit(p, p + 1, Kind::Punctuation);
                    ctx.embed(lang("csharp"), p + 1, c);
                    ctx.emit(c, c + 1, Kind::Punctuation);
                    (c + 1).min(len)
                } else {
                    w
                }
            } else if RAZOR_DIRECTIVES.contains(&word) && tp.ctx.at_line_start(at) {
                let ctx = &mut *tp.ctx;
                ctx.emit(at, w, Kind::Keyword);
                let e = ctx.line_end(w);
                ctx.embed(lang("csharp"), w, e);
                e
            } else {
                // Implicit expression: `@Model.Items[0].Name`, `@Html.Raw(x)`, `@await F()`.
                let mut e = if word == b"await" { name_end(tp.ctx, tp.ctx.skip_spaces_from(w), b"") } else { w };
                loop {
                    match tp.ctx.at(e) {
                        b'.' if is_ident_start(tp.ctx.at(e + 1)) => e = name_end(tp.ctx, e + 1, b""),
                        b'(' | b'[' => match matching(src, e, near(e, len), true) {
                            Some(c) => e = c + 1,
                            None => break,
                        },
                        _ => break,
                    }
                }
                let ctx = &mut *tp.ctx;
                ctx.emit(at, at + 1, Kind::Punctuation);
                ctx.embed(lang("csharp"), at + 1, e);
                e
            }
        }
        _ => return None,
    };
    tp.ctx.pos = end;
    tp.text = end;
    Some(end)
}

/// A keyword with an optional `(condition)` and a `{` that opens a block of markup.
fn razor_block(tp: &mut Tpl, start: usize, word_end: usize) -> usize {
    let ctx = &mut *tp.ctx;
    let src = ctx.src;
    ctx.emit(start, word_end, Kind::Keyword);
    let mut end = word_end;
    let mut p = skip_whitespace_from(ctx, end);
    if src[p..].starts_with(b"if") && !is_ident(ctx.at(p + 2)) {
        ctx.emit(p, p + 2, Kind::Keyword);
        end = p + 2;
        p = skip_whitespace_from(ctx, end);
    }
    if ctx.at(p) == b'(' {
        if let Some(c) = matching(src, p, near(p, src.len()), true) {
            ctx.emit(p, p + 1, Kind::Punctuation);
            ctx.embed(lang("csharp"), p + 1, c);
            ctx.emit(c, c + 1, Kind::Punctuation);
            end = c + 1;
            p = skip_whitespace_from(ctx, end);
        }
    } else if is_ident_start(ctx.at(p)) {
        // `@section Scripts {`
        let e = name_end(ctx, p, b"");
        ctx.emit(p, e, Kind::Function);
        end = e;
        p = skip_whitespace_from(ctx, e);
    }
    if ctx.at(p) == b'{' {
        ctx.emit(p, p + 1, Kind::Punctuation);
        tp.depth += 1;
        end = p + 1;
    }
    end
}

// reStructuredText

fn rst(ctx: &mut Ctx) {
    // Indentation of the paragraph that ended with `::`: a literal block follows.
    let mut literal: Option<usize> = None;
    each_line(ctx, |ctx, start, end| {
        let line = line_text(ctx, start, end);
        if line.trim_ascii().is_empty() {
            return;
        }
        let ind = indent(ctx, start);
        if let Some(base) = literal.take() {
            if ind > base {
                let block = indented_block(ctx, end, base);
                ctx.emit(start + ind, block, Kind::String);
                ctx.pos = block;
                return;
            }
        }
        if rst_line(ctx, start, end, ind) && line.trim_ascii_end().ends_with(b"::") {
            literal = Some(ind);
        }
    });
}

/// A line made of one repeated punctuation character, as used for section titles.
fn is_adornment(line: &[u8]) -> bool {
    line.len() >= 2 && b"=-`:'\"~^_*+#<>.".contains(&line[0]) && line.iter().all(|&b| b == line[0])
}

/// The line after the one ending at `end`.
fn next_line(ctx: &Ctx, end: usize) -> Option<(usize, usize)> {
    (end < ctx.len()).then(|| (end + 1, ctx.line_end(end + 1)))
}

/// Lexes one line. Returns true for an ordinary paragraph line.
fn rst_line(ctx: &mut Ctx, start: usize, end: usize, ind: usize) -> bool {
    let line = line_text(ctx, start, end);
    let content = line[ind..].trim_ascii_end();
    let at = start + ind;
    let content_end = at + content.len();
    if content.starts_with(b"..") && matches!(content.get(2), None | Some(b' ')) {
        rst_explicit(ctx, start, end, ind);
        return false;
    }
    if ind == 0 && is_adornment(content) {
        // Overline, title and underline.
        if let Some((t, t_end)) = next_line(ctx, end) {
            if let Some((u, u_end)) = next_line(ctx, t_end) {
                let under = line_text(ctx, u, u_end).trim_ascii_end();
                if under == content && !line_text(ctx, t, t_end).trim_ascii().is_empty() {
                    ctx.emit(start, content_end, Kind::Heading);
                    let title = line_text(ctx, t, t_end).trim_ascii_end().len();
                    ctx.emit(t + indent(ctx, t), t + title, Kind::Heading);
                    ctx.emit(u, u + under.len(), Kind::Heading);
                    ctx.pos = u_end;
                    return false;
                }
            }
        }
        if content.len() >= 4 {
            ctx.emit(at, content_end, Kind::Punctuation);
            return false;
        }
    }
    if ind == 0 {
        if let Some((u, u_end)) = next_line(ctx, end) {
            let under = line_text(ctx, u, u_end).trim_ascii_end();
            if is_adornment(under) && under.len() >= 3 && !is_adornment(content) {
                ctx.emit(start, content_end, Kind::Heading);
                ctx.emit(u, u + under.len(), Kind::Heading);
                ctx.pos = u_end;
                return false;
            }
        }
    }
    // Table borders.
    let border = (content[0] == b'+' && content.len() > 2 && content.iter().all(|b| b"+-=: ".contains(b)))
        || (content[0] == b'=' && content.contains(&b' ') && content.iter().all(|b| b"= ".contains(b)));
    if border {
        ctx.emit(at, content_end, Kind::Punctuation);
        return false;
    }
    // Doctest blocks.
    if content.starts_with(b">>>") {
        ctx.emit(at, at + 3, Kind::Punctuation);
        ctx.embed(lang("python"), at + 3, content_end);
        return false;
    }
    let mut p = at;
    let next = |i: usize| content.get(i).copied().unwrap_or(b' ');
    match content[0] {
        // Field lists: `:name: value`.
        b':' => {
            if let Some(i) = content[1..].iter().position(|&b| b == b':') {
                if i > 0 && matches!(next(i + 2), b' ' | b'\t') {
                    ctx.emit(at, at + i + 2, Kind::Property);
                    p = at + i + 2;
                }
            }
        }
        b'-' | b'*' | b'+' if next(1) == b' ' => {
            ctx.emit(at, at + 1, Kind::Keyword);
            p = at + 1;
        }
        b'|' if content.len() > 1 && content.ends_with(b"|") => {
            // Grid table rows.
            let mut p = at;
            for (i, _) in content.iter().enumerate().filter(|(_, &b)| b == b'|') {
                rst_inline(ctx, p, at + i);
                ctx.emit(at + i, at + i + 1, Kind::Punctuation);
                p = at + i + 1;
            }
            return false;
        }
        b'|' if next(1) == b' ' => {
            ctx.emit(at, at + 1, Kind::Punctuation);
            p = at + 1;
        }
        b'#' if next(1) == b'.' && next(2) == b' ' => {
            ctx.emit(at, at + 2, Kind::Keyword);
            p = at + 2;
        }
        b'0'..=b'9' | b'(' => {
            let digits = content.iter().skip(1).take_while(|b| b.is_ascii_digit()).count() + 1;
            let paren = content[0] == b'(';
            let marker = if paren { next(digits) == b')' } else { matches!(next(digits), b'.' | b')') };
            if digits > paren as usize && marker && next(digits + 1) == b' ' {
                ctx.emit(at, at + digits + 1, Kind::Keyword);
                p = at + digits + 1;
            }
        }
        _ => {}
    }
    if content == b"::" {
        ctx.emit(at, content_end, Kind::Punctuation);
        return true;
    }
    rst_inline(ctx, p, content_end);
    true
}

const RST_PROSE_DIRECTIVES: &[&[u8]] = &[
    b"note", b"warning", b"tip", b"important", b"caution", b"danger", b"attention", b"hint", b"error", b"admonition",
    b"seealso", b"topic", b"sidebar", b"rubric", b"deprecated", b"versionadded", b"versionchanged",
];

/// Explicit markup: directives, targets, footnotes, substitutions and comments.
fn rst_explicit(ctx: &mut Ctx, start: usize, end: usize, ind: usize) {
    let src = ctx.src;
    let at = start + ind;
    let line_end = start + line_text(ctx, start, end).len();
    let p = ctx.skip_spaces_from(at + 2).min(line_end);
    let c = ctx.at(p);
    if c == b'_' && p < line_end {
        // Targets: `.. _name: https://example.com`.
        ctx.emit(at, at + 2, Kind::Punctuation);
        let colon = src[p..line_end].iter().position(|&b| b == b':').map(|i| p + i);
        let name_end = colon.map_or(line_end, |c| c + 1);
        ctx.emit(p, name_end, Kind::Link);
        ctx.emit(ctx.skip_spaces_from(name_end).min(line_end), line_end, Kind::String);
        ctx.pos = line_end;
        return;
    }
    if c == b'[' {
        // Footnotes and citations: `.. [1] Text`.
        if let Some(close) = src[p..line_end].iter().position(|&b| b == b']').map(|i| p + i) {
            ctx.emit(at, at + 2, Kind::Punctuation);
            ctx.emit(p, close + 1, Kind::Link);
            rst_inline(ctx, close + 1, line_end);
            ctx.pos = line_end;
            return;
        }
    }
    let mut substitution = None;
    if c == b'|' {
        if let Some(close) = src[p + 1..line_end].iter().position(|&b| b == b'|').map(|i| p + 1 + i) {
            substitution = Some(close + 1);
        }
    }
    let name_start = substitution.map_or(p, |s| ctx.skip_spaces_from(s));
    let mut name_stop = name_end(ctx, name_start, b"-_.+");
    while ctx.at(name_stop) == b':' && is_ident(ctx.at(name_stop + 1)) {
        name_stop = name_end(ctx, name_stop + 1, b"-_.+");
    }
    let directive = name_stop > name_start && src[name_stop..].starts_with(b"::");
    if !directive && substitution.is_none() {
        // A comment, together with its indented continuation lines.
        let block = indented_block(ctx, end, ind);
        ctx.emit(at, block, Kind::Comment);
        ctx.pos = block;
        return;
    }
    ctx.emit(at, at + 2, Kind::Punctuation);
    if let Some(s) = substitution {
        ctx.emit(p, s, Kind::Variable);
    }
    if !directive {
        rst_inline(ctx, name_start, line_end);
        ctx.pos = line_end;
        return;
    }
    let name = &src[name_start..name_stop];
    ctx.emit(name_start, name_stop, Kind::Keyword);
    ctx.emit(name_stop, name_stop + 2, Kind::Punctuation);
    let args = ctx.skip_spaces_from(name_stop + 2).min(line_end);
    if RST_PROSE_DIRECTIVES.contains(&name) {
        rst_inline(ctx, args, line_end);
    } else {
        ctx.emit(args, line_end, Kind::String);
    }
    // Options: `:linenos:` and `:caption: text`.
    let mut pos = end;
    while let Some((s, e)) = next_line(ctx, pos) {
        let i = indent(ctx, s);
        if i <= ind || ctx.at(s + i) != b':' {
            break;
        }
        let Some(close) = src[s + i + 1..e].iter().position(|&b| b == b':').map(|k| s + i + 1 + k) else {
            break;
        };
        ctx.emit(s + i, close + 1, Kind::Property);
        pos = e;
    }
    ctx.pos = pos;
    let language: &[u8] = match name {
        b"code" | b"code-block" | b"sourcecode" | b"ipython" | b"raw" => {
            let a = &src[args..line_end];
            &a[..a.iter().position(|&b| b == b' ').unwrap_or(a.len())]
        }
        b"doctest" | b"testcode" => b"python",
        b"math" | b"testoutput" | b"parsed-literal" => b"text",
        _ => return,
    };
    let body_end = indented_block(ctx, pos, ind);
    if body_end > pos {
        if language == b"text" {
            ctx.emit(pos + 1, body_end, Kind::String);
        } else {
            embed_named(ctx, language, pos + 1, body_end);
        }
        ctx.pos = body_end;
    }
}

/// Characters that may come before inline markup.
fn rst_opens(b: u8) -> bool {
    !b.is_ascii_alphanumeric() && b != b'\\' && b < 0x80
}

fn rst_inline(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let mut i = start;
    while i < end {
        let b = src[i];
        let opens = i == start || rst_opens(src[i - 1]);
        let next = if i + 1 < end { src[i + 1] } else { b' ' };
        match b {
            b'\\' if next.is_ascii_punctuation() || next.is_ascii_alphanumeric() => {
                ctx.emit(i, i + 2, Kind::Escape);
                i += 2;
            }
            b'`' if opens && next == b'`' => match find_in(ctx, i + 2, end, b"``") {
                Some(c) => {
                    ctx.emit(i, c + 2, Kind::String);
                    i = c + 2;
                }
                None => i += 2,
            },
            b'`' if opens => match src[i + 1..end].iter().position(|&c| c == b'`').map(|k| i + 1 + k) {
                Some(c) => {
                    let mut e = c + 1;
                    let mut kind = Kind::String;
                    if ctx.at(e) == b'_' {
                        kind = Kind::Link;
                        e += 1 + (ctx.at(e + 1) == b'_') as usize;
                    }
                    let e = e.min(end);
                    ctx.emit(i, e, kind);
                    i = e;
                }
                None => i += 1,
            },
            b':' if opens && is_ident_start(next) => {
                // Roles: :role:`text` and :py:func:`name`.
                let mut e = i + 1;
                while e < end
                    && (is_ident(src[e]) || b"-_.+".contains(&src[e]) || (src[e] == b':' && ctx.at(e + 1) != b'`'))
                {
                    e += 1;
                }
                if ctx.at(e) == b':' && ctx.at(e + 1) == b'`' {
                    ctx.emit(i, e + 1, Kind::Function);
                    i = e + 1;
                } else {
                    i += 1;
                }
            }
            b'*' if opens => {
                let strong = next == b'*';
                let delim: &[u8] = if strong { b"**" } else { b"*" };
                let from = i + delim.len();
                if matches!(ctx.at(from), b' ' | b'\t' | b'\r' | b'\n' | 0) || from >= end {
                    i = from;
                    continue;
                }
                match find_in(ctx, from + 1, end, delim).filter(|&c| !src[c - 1].is_ascii_whitespace()) {
                    Some(c) => {
                        ctx.emit(i, c + delim.len(), if strong { Kind::Strong } else { Kind::Emphasis });
                        i = c + delim.len();
                    }
                    None => i = from,
                }
            }
            b'|' if opens && !next.is_ascii_whitespace() && next != b'|' => {
                match src[i + 1..end].iter().position(|&c| c == b'|').map(|k| i + 1 + k) {
                    Some(c) if !src[c - 1].is_ascii_whitespace() => {
                        let mut e = c + 1;
                        while e < end && src[e] == b'_' && e < c + 3 {
                            e += 1;
                        }
                        ctx.emit(i, e, Kind::Variable);
                        i = e;
                    }
                    _ => i += 1,
                }
            }
            b'[' if opens => match src[i..end].iter().position(|&c| c == b']').map(|k| i + k) {
                Some(c) if ctx.at(c + 1) == b'_' && c + 2 <= end => {
                    ctx.emit(i, c + 2, Kind::Link);
                    i = c + 2;
                }
                _ => i += 1,
            },
            _ if opens && b.is_ascii_alphanumeric() => {
                if let Some(u) = url_end(src, i, end) {
                    ctx.emit(i, u, Kind::Link);
                    i = u;
                    continue;
                }
                // Simple references: `word_`.
                let mut e = i;
                while e < end && (src[e].is_ascii_alphanumeric() || src[e] == b'_' || src[e] == b'-') {
                    e += 1;
                }
                if src[e - 1] == b'_' && e - 1 > i && !(e < end && is_ident(src[e])) {
                    ctx.emit(i, e, Kind::Link);
                }
                i = e;
            }
            _ => i += 1,
        }
    }
    ctx.pos = ctx.pos.max(end);
}

// AsciiDoc

fn asciidoc(ctx: &mut Ctx) {
    // Language named by the last `[source,lang]` line.
    let mut source: Option<(usize, usize)> = None;
    while !ctx.done() {
        let start = ctx.pos;
        let end = ctx.line_end(start);
        let line = line_text(ctx, start, end);
        let line_end = start + line.len();
        let pending = source.take();
        if line.trim_ascii().is_empty() {
            source = pending;
        } else if is_delimiter(line) || line.starts_with(b"```") {
            source = adoc_block(ctx, start, end, pending);
        } else if line.starts_with(b"//") {
            ctx.emit(start, line_end, Kind::Comment);
        } else if line[0] == b'[' && line.ends_with(b"]") {
            ctx.emit(start, line_end, Kind::Meta);
            let inner = &line[1..line.len() - 1];
            if inner.starts_with(b"source") {
                let mut parts = inner.split(|&b| b == b',');
                if let Some(lang) = parts.nth(1) {
                    let offset = lang.as_ptr() as usize - line.as_ptr() as usize;
                    source = Some((start + offset, start + offset + lang.len()));
                }
            } else {
                source = pending;
            }
        } else if line[0] == b'.' && line.len() > 1 && !matches!(line[1], b' ' | b'.') {
            // Block title.
            ctx.emit(start, line_end, Kind::Heading);
            source = pending;
        } else {
            adoc_line(ctx, start, line_end);
        }
        ctx.pos = ctx.pos.max(end);
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

/// A block delimiter: four or more of the same character, or `|===`.
fn is_delimiter(line: &[u8]) -> bool {
    let line = line.trim_ascii_end();
    (line.len() >= 4 && b"-.+*=_/".contains(&line[0]) && line.iter().all(|&b| b == line[0])) || line == b"|==="
}

/// A delimited block starting at `start`. Returns the pending source language.
fn adoc_block(ctx: &mut Ctx, start: usize, end: usize, source: Option<(usize, usize)>) -> Option<(usize, usize)> {
    let src = ctx.src;
    let line = line_text(ctx, start, end).trim_ascii_end();
    let fence = line.starts_with(b"```");
    let delim = if fence { &line[..3] } else { line };
    let kind = line[0];
    if kind != b'/' {
        ctx.emit(start, start + delim.len(), Kind::Punctuation);
    }
    let mut language = source.map(|(s, e)| &src[s..e]);
    if fence {
        ctx.emit(start + 3, start + line.len(), Kind::Attribute);
        let info = &line[3..];
        if !info.is_empty() {
            language = Some(info);
        }
    }
    if !matches!(kind, b'-' | b'.' | b'+' | b'/' | b'`') {
        return source;
    }
    // Find the closing delimiter.
    let body = (end + 1).min(ctx.len());
    let mut pos = body;
    let mut close = (ctx.len(), ctx.len());
    while pos < ctx.len() {
        let e = ctx.line_end(pos);
        if line_text(ctx, pos, e).trim_ascii_end() == delim {
            close = (pos, e);
            break;
        }
        pos = e + 1;
    }
    match kind {
        b'/' => ctx.emit(start, close.1, Kind::Comment),
        b'+' => ctx.embed(lang("html"), body, close.0),
        _ => match language {
            Some(name) => embed_named(ctx, name, body, close.0),
            None => ctx.emit(body, close.0, Kind::String),
        },
    }
    ctx.emit(close.0, close.1, Kind::Punctuation);
    ctx.pos = close.1;
    None
}

fn adoc_line(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let line = &src[start..end];
    // Section titles: `== Title`.
    let level = line.iter().take_while(|&&b| b == b'=' || b == b'#').count();
    if (1..=6).contains(&level) && line.get(level) == Some(&b' ') {
        ctx.emit(start, end, Kind::Heading);
        return;
    }
    // Attribute entries: `:name: value`.
    if line[0] == b':' {
        if let Some(i) = line[1..].iter().position(|&b| b == b':') {
            if i > 0 && !line[1..=i].contains(&b' ') {
                ctx.emit(start, start + i + 2, Kind::Property);
                adoc_inline(ctx, start + i + 2, end);
                return;
            }
        }
    }
    if matches!(line, b"'''" | b"<<<" | b"---" | b"***") {
        ctx.emit(start, end, Kind::Punctuation);
        return;
    }
    // Block macros: `image::file.png[Alt]`.
    let word = name_end(ctx, start, b"-");
    if word > start && src[word..end].starts_with(b"::") && !matches!(ctx.at(word + 2), b' ' | b'\t' | 0 | b'\n') {
        ctx.emit(start, word, Kind::Keyword);
        ctx.emit(word, word + 2, Kind::Punctuation);
        let attrs = src[word..end].iter().position(|&b| b == b'[').map_or(end, |i| word + i);
        ctx.emit(word + 2, attrs, Kind::String);
        if attrs < end {
            emit_wrapped(ctx, attrs, end - 1, Kind::Text);
        }
        return;
    }
    // Admonitions.
    for label in [&b"NOTE: "[..], b"TIP: ", b"IMPORTANT: ", b"WARNING: ", b"CAUTION: "] {
        if line.starts_with(label) {
            ctx.emit(start, start + label.len() - 1, Kind::Keyword);
            adoc_inline(ctx, start + label.len() - 1, end);
            return;
        }
    }
    let mut p = start;
    // List markers and checklists.
    let marker = line.iter().take_while(|&&b| matches!(b, b'*' | b'-' | b'.')).count();
    if marker > 0 && line.get(marker) == Some(&b' ') {
        ctx.emit(start, start + marker, Kind::Keyword);
        p = start + marker + 1;
        if matches!(&src[p..end.min(p + 3)], b"[ ]" | b"[x]" | b"[*]") {
            ctx.emit(p, p + 3, Kind::Constant);
            p += 3;
        }
    } else if line[0].is_ascii_digit() {
        let d = line.iter().take_while(|b| b.is_ascii_digit()).count();
        if line.get(d) == Some(&b'.') && line.get(d + 1) == Some(&b' ') {
            ctx.emit(start, start + d + 1, Kind::Keyword);
            p = start + d + 1;
        }
    } else if line[0] == b'|' {
        ctx.emit(start, start + 1, Kind::Punctuation);
        p = start + 1;
    } else if let Some(i) = find_in(ctx, start, end, b"::").filter(|&i| i > start) {
        // Description lists: `term:: definition`.
        if matches!(ctx.at(i + 2), b' ' | b'\t' | b'\r' | b'\n' | 0) || i + 2 == end {
            ctx.emit(start, i, Kind::Property);
            ctx.emit(i, i + 2, Kind::Punctuation);
            p = i + 2;
        }
    }
    adoc_inline(ctx, p, end);
}

/// Bytes that may come before constrained inline markup.
fn adoc_opens(b: u8) -> bool {
    !b.is_ascii_alphanumeric() && b != b'\\' && b < 0x80
}

fn adoc_inline(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let mut i = start;
    while i < end {
        let b = src[i];
        let opens = i == start || adoc_opens(src[i - 1]);
        let next = if i + 1 < end { src[i + 1] } else { b' ' };
        match b {
            b'\\' if matches!(next, b'*' | b'_' | b'`' | b'{' | b'+' | b'#' | b'<') => {
                ctx.emit(i, i + 2, Kind::Escape);
                i += 2;
            }
            b'*' | b'_' | b'`' | b'+' | b'#' => {
                let double = next == b;
                if !double && !opens {
                    i += 1;
                    continue;
                }
                let delim = &src[i..i + 1 + double as usize];
                let from = i + delim.len();
                if matches!(ctx.at(from), b' ' | b'\t' | b'\r' | b'\n') || from >= end {
                    i = from;
                    continue;
                }
                let close = find_in(ctx, from + 1, end, delim).filter(|&c| {
                    !src[c - 1].is_ascii_whitespace() && (double || c + 1 >= end || !src[c + 1].is_ascii_alphanumeric())
                });
                match close {
                    Some(c) => {
                        let kind = match b {
                            b'*' => Kind::Strong,
                            b'_' | b'#' => Kind::Emphasis,
                            _ => Kind::String,
                        };
                        ctx.emit(i, c + delim.len(), kind);
                        i = c + delim.len();
                    }
                    None => i = from,
                }
            }
            b'{' if is_ident_start(next) => {
                let e = name_end(ctx, i + 1, b"-");
                if ctx.at(e) == b'}' && e < end {
                    ctx.emit(i, e + 1, Kind::Variable);
                    i = e + 1;
                } else {
                    i += 1;
                }
            }
            b'<' if next == b'<' => match find_in(ctx, i + 2, end, b">>") {
                Some(c) => {
                    ctx.emit(i, c + 2, Kind::Link);
                    i = c + 2;
                }
                None => i += 2,
            },
            b' ' if next == b'+' && i + 2 == end => {
                ctx.emit(i + 1, end, Kind::Punctuation);
                i = end;
            }
            _ if opens && b.is_ascii_alphabetic() => {
                if let Some(u) = url_end(src, i, end) {
                    let e = adoc_macro_text(src, u, end);
                    ctx.emit(i, e, Kind::Link);
                    i = e;
                    continue;
                }
                // Inline macros: `link:url[text]`, `kbd:[Ctrl+C]`, `footnote:[note]`.
                let w = name_end(ctx, i, b"-");
                if ctx.at(w) == b':' && w + 1 < end && !matches!(ctx.at(w + 1), b' ' | b':') {
                    let target_end = src[w..end].iter().position(|&c| c == b'[' || c == b' ').map_or(end, |k| w + k);
                    if ctx.at(target_end) == b'[' {
                        let e = adoc_macro_text(src, target_end, end);
                        let name = &src[i..w];
                        if matches!(name, b"link" | b"xref" | b"mailto") {
                            ctx.emit(i, e, Kind::Link);
                        } else {
                            ctx.emit(i, w + 1, Kind::Function);
                            ctx.emit(w + 1, e, Kind::String);
                        }
                        i = e;
                        continue;
                    }
                }
                i = w;
            }
            _ => i += 1,
        }
    }
    ctx.pos = ctx.pos.max(end);
}

/// End of an optional `[text]` after a link or macro target at `at`.
fn adoc_macro_text(src: &[u8], at: usize, end: usize) -> usize {
    if src.get(at) == Some(&b'[') {
        if let Some(c) = src[at..end].iter().position(|&b| b == b']') {
            return at + c + 1;
        }
    }
    at
}

// Org

fn org(ctx: &mut Ctx) {
    each_line(ctx, org_line);
}

const ORG_TODO: &[&[u8]] = &[b"TODO", b"DONE", b"NEXT", b"WAITING", b"HOLD", b"CANCELLED", b"CANCELED", b"STARTED"];

fn org_line(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let line = line_text(ctx, start, end);
    let line_end = start + line.len();
    let ind = indent(ctx, start);
    let at = start + ind;
    let content = &line[ind..];
    if content.is_empty() {
        return;
    }
    // Headlines.
    let stars = line.iter().take_while(|&&b| b == b'*').count();
    if stars > 0 && line.get(stars) == Some(&b' ') {
        let mut p = start + stars + 1;
        let word = name_end(ctx, p, b"-");
        let mut title_end = line_end;
        // Tags at the end: `:work:urgent:`.
        if line.ends_with(b":") {
            if let Some(space) = line.iter().rposition(|&b| b == b' ' || b == b'\t') {
                let tags = &line[space + 1..];
                if tags.len() > 2 && tags[0] == b':' {
                    title_end = start + space;
                }
            }
        }
        ctx.emit(start, p, Kind::Heading);
        if ORG_TODO.contains(&&src[p..word]) {
            ctx.emit(p, word, Kind::Keyword);
            p = word;
        }
        let q = ctx.skip_spaces_from(p);
        if src[q..line_end].starts_with(b"[#") && ctx.at(q + 3) == b']' {
            ctx.emit(p, q, Kind::Heading);
            ctx.emit(q, q + 4, Kind::Constant);
            p = q + 4;
        }
        ctx.emit(p, title_end, Kind::Heading);
        ctx.emit(ctx.skip_spaces_from(title_end).min(line_end), line_end, Kind::Attribute);
        return;
    }
    if content.starts_with(b"#+") {
        org_keyword(ctx, at, line_end);
        return;
    }
    if content[0] == b'#' && matches!(content.get(1), None | Some(b' ')) {
        ctx.emit(at, line_end, Kind::Comment);
        return;
    }
    // Drawers and properties: `:PROPERTIES:`, `:ID: abc`, `:END:`.
    if content[0] == b':' && content.len() > 1 {
        if content[1] == b' ' {
            ctx.emit(at, line_end, Kind::String);
            return;
        }
        if let Some(i) = content[1..].iter().position(|&b| b == b':') {
            if i > 0 && !content[1..=i].contains(&b' ') {
                let close = at + i + 2;
                let drawer = close == line_end;
                ctx.emit(at, close, if drawer { Kind::Meta } else { Kind::Property });
                ctx.emit(ctx.skip_spaces_from(close).min(line_end), line_end, Kind::String);
                return;
            }
        }
    }
    if content.len() >= 5 && content.iter().all(|&b| b == b'-') {
        ctx.emit(at, line_end, Kind::Punctuation);
        return;
    }
    // Tables.
    if content[0] == b'|' {
        if content.get(1) == Some(&b'-') {
            ctx.emit(at, line_end, Kind::Punctuation);
            return;
        }
        let mut p = at;
        while let Some(i) = src[p..line_end].iter().position(|&b| b == b'|') {
            org_inline(ctx, p, p + i);
            ctx.emit(p + i, p + i + 1, Kind::Punctuation);
            p += i + 1;
        }
        org_inline(ctx, p, line_end);
        return;
    }
    let mut p = at;
    // List items, checkboxes and description terms.
    let b = content[0];
    let next = content.get(1).copied().unwrap_or(b'\n');
    if (matches!(b, b'-' | b'+') || (b == b'*' && ind > 0)) && next == b' ' {
        ctx.emit(at, at + 1, Kind::Keyword);
        p = at + 2;
    } else if b.is_ascii_digit() {
        let d = content.iter().take_while(|b| b.is_ascii_digit()).count();
        if matches!(content.get(d), Some(b'.' | b')')) && content.get(d + 1) == Some(&b' ') {
            ctx.emit(at, at + d + 1, Kind::Keyword);
            p = at + d + 2;
        }
    }
    if p > at {
        if matches!(&src[p..line_end.min(p + 3)], b"[ ]" | b"[X]" | b"[x]" | b"[-]") {
            ctx.emit(p, p + 3, Kind::Constant);
            p += 3;
        }
        if let Some(i) = find_in(ctx, p, line_end, b" :: ") {
            ctx.emit(p, i, Kind::Property);
            ctx.emit(i + 1, i + 3, Kind::Punctuation);
            p = i + 3;
        }
    }
    org_inline(ctx, p, line_end);
}

/// `#+KEY: value` lines and `#+BEGIN_...` blocks.
fn org_keyword(ctx: &mut Ctx, at: usize, line_end: usize) {
    let src = ctx.src;
    let key_end = name_end(ctx, at + 2, b"-_");
    let key = &src[at + 2..key_end];
    if key.len() > 6 && key[..6].eq_ignore_ascii_case(b"begin_") {
        let kind = &key[6..];
        ctx.emit(at, key_end, Kind::Meta);
        let args = ctx.skip_spaces_from(key_end).min(line_end);
        let lang_end = name_end(ctx, args, b"-+#").min(line_end);
        ctx.emit(args, lang_end, Kind::Attribute);
        // The block runs to `#+END_<kind>`.
        let body = (line_end + 1).min(ctx.len());
        let mut pos = body;
        let mut close = (ctx.len(), ctx.len());
        while pos < ctx.len() {
            let e = ctx.line_end(pos);
            let t = line_text(ctx, pos, e).trim_ascii();
            if t.len() >= 6 + kind.len() && t[..6].eq_ignore_ascii_case(b"#+end_") && t[6..].eq_ignore_ascii_case(kind)
            {
                close = (pos + indent(ctx, pos), pos + line_text(ctx, pos, e).len());
                break;
            }
            pos = e + 1;
        }
        let body_end = close.0.min(ctx.len());
        let kind = kind.to_ascii_lowercase();
        match kind.as_slice() {
            b"src" | b"export" => embed_named(ctx, &src[args..lang_end], body, body_end),
            b"example" | b"verse" => ctx.emit(body, body_end, Kind::String),
            b"comment" => ctx.emit(body, body_end, Kind::Comment),
            _ => {
                ctx.pos = line_end;
                return;
            }
        }
        ctx.emit(close.0, close.1, Kind::Meta);
        ctx.pos = close.1;
        return;
    }
    let colon = if ctx.at(key_end) == b':' { key_end + 1 } else { key_end };
    ctx.emit(at, colon, Kind::Meta);
    let value = ctx.skip_spaces_from(colon).min(line_end);
    let kind = if key.eq_ignore_ascii_case(b"title") { Kind::Heading } else { Kind::String };
    ctx.emit(value, line_end, kind);
}

/// Whether `b` may follow the closing marker of inline emphasis.
fn org_closes(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t'
            | b'\r'
            | b'\n'
            | b'-'
            | b'.'
            | b','
            | b';'
            | b':'
            | b'!'
            | b'?'
            | b'\''
            | b')'
            | b'}'
            | b'"'
            | b']'
            | 0
    )
}

fn org_inline(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let mut i = start;
    while i < end {
        let b = src[i];
        let opens = i == start || matches!(src[i - 1], b' ' | b'\t' | b'-' | b'(' | b'{' | b'\'' | b'"');
        match b {
            b'*' | b'/' | b'_' | b'=' | b'~' | b'+' if opens => {
                let next = ctx.at(i + 1);
                if i + 1 >= end || matches!(next, b' ' | b'\t' | b'\r') || next == b {
                    i += 1;
                    continue;
                }
                let close = src[i + 2..end]
                    .iter()
                    .enumerate()
                    .position(|(k, &c)| {
                        let at = i + 2 + k;
                        c == b && !src[at - 1].is_ascii_whitespace() && org_closes(ctx.at(at + 1))
                    })
                    .map(|k| i + 2 + k);
                match close {
                    Some(c) => {
                        let kind = match b {
                            b'*' => Kind::Strong,
                            b'/' | b'_' => Kind::Emphasis,
                            b'+' => Kind::Deleted,
                            _ => Kind::String,
                        };
                        ctx.emit(i, c + 1, kind);
                        i = c + 1;
                    }
                    None => i += 1,
                }
            }
            b'[' if ctx.at(i + 1) == b'[' => match find_in(ctx, i + 2, end, b"]]") {
                Some(c) => {
                    ctx.emit(i, c + 2, Kind::Link);
                    i = c + 2;
                }
                None => i += 2,
            },
            b'[' if src[i..end].starts_with(b"[fn:") => match src[i..end].iter().position(|&c| c == b']') {
                Some(c) => {
                    ctx.emit(i, i + c + 1, Kind::Link);
                    i += c + 1;
                }
                None => i += 1,
            },
            b'<' | b'[' if ctx.at(i + 1).is_ascii_digit() && ctx.at(i + 5) == b'-' => {
                // Timestamps: <2024-03-01 Fri 10:00> and [2024-03-01].
                let close = if b == b'<' { b'>' } else { b']' };
                match src[i..end].iter().position(|&c| c == close) {
                    Some(c) => {
                        ctx.emit(i, i + c + 1, Kind::Constant);
                        i += c + 1;
                    }
                    None => i += 1,
                }
            }
            b'<' if ctx.at(i + 1) == b'<' => match find_in(ctx, i + 2, end, b">>") {
                Some(c) => {
                    ctx.emit(i, c + 2, Kind::Link);
                    i = c + 2;
                }
                None => i += 2,
            },
            _ if opens && b.is_ascii_alphabetic() => {
                if let Some(u) = url_end(src, i, end) {
                    ctx.emit(i, u, Kind::Link);
                    i = u;
                    continue;
                }
                let w = name_end(ctx, i, b"");
                let word = &src[i..w];
                if ctx.at(w) == b':' && matches!(word, b"SCHEDULED" | b"DEADLINE" | b"CLOSED") {
                    ctx.emit(i, w + 1, Kind::Keyword);
                    i = w + 1;
                } else if word.starts_with(b"src_") && ctx.at(w) == b'{' {
                    let c = src[w..end].iter().position(|&c| c == b'}').map_or(end, |k| w + k + 1);
                    ctx.emit(i, c, Kind::String);
                    i = c;
                } else {
                    i = w;
                }
            }
            _ => i += 1,
        }
    }
    ctx.pos = ctx.pos.max(end);
}

// Typst

fn typst(ctx: &mut Ctx) {
    typst_markup(ctx, false);
}

const TYPST_STATEMENTS: &[&[u8]] = &[
    b"let", b"set", b"show", b"import", b"include", b"if", b"for", b"while", b"return", b"context", b"break",
    b"continue",
];

/// Markup up to the end, or up to an unbalanced `]` when `nested`.
fn typst_markup(ctx: &mut Ctx, nested: bool) {
    let src = ctx.src;
    let mut depth = 0usize;
    while !ctx.done() {
        let i = ctx.pos;
        let b = src[i];
        let line_start = ctx.at_line_start(i);
        let next = ctx.at(i + 1);
        match b {
            b']' if nested => {
                if depth == 0 {
                    return;
                }
                depth -= 1;
                ctx.pos += 1;
            }
            b'[' => {
                depth += 1;
                ctx.pos += 1;
            }
            b'=' if line_start => {
                let n = src[i..].iter().take_while(|&&c| c == b'=').count();
                if matches!(ctx.at(i + n), b' ' | b'\t') {
                    ctx.pos = ctx.line_end(i);
                    ctx.emit(i, ctx.pos, Kind::Heading);
                } else {
                    ctx.pos += n;
                }
            }
            b'-' | b'+' if line_start && matches!(next, b' ' | b'\t') => ctx.token(1, Kind::Keyword),
            b'/' if next == b'/' => {
                ctx.skip_line();
                ctx.emit(i, ctx.pos, Kind::Comment);
            }
            b'/' if next == b'*' => crate::lexer::generic::block_comment(ctx, "/*", "*/", true),
            b'/' if line_start && next == b' ' => {
                // Term lists: `/ Term: description`.
                ctx.token(1, Kind::Keyword);
                let line_end = ctx.line_end(i);
                if let Some(c) = src[i..line_end].iter().position(|&c| c == b':') {
                    ctx.emit(i + 2, i + c, Kind::Strong);
                    ctx.pos = i + c;
                }
            }
            b'\\' => {
                if next.is_ascii_punctuation() || next.is_ascii_alphanumeric() {
                    ctx.token(2, Kind::Escape);
                } else {
                    ctx.token(1, Kind::Punctuation);
                }
            }
            b'*' | b'_' if i == 0 || !src[i - 1].is_ascii_alphanumeric() => match typst_span_end(src, i + 1, b) {
                Some(c) => {
                    ctx.pos = c + 1;
                    ctx.emit(i, ctx.pos, if b == b'*' { Kind::Strong } else { Kind::Emphasis });
                }
                None => ctx.pos += 1,
            },
            b'`' => typst_raw(ctx),
            b'$' => {
                let mut e = i + 1;
                while e < src.len() && src[e] != b'$' {
                    e += 1 + (src[e] == b'\\') as usize;
                }
                ctx.pos = (e + 1).min(src.len());
                ctx.emit(i, ctx.pos, Kind::String);
            }
            b'#' => typst_hash(ctx),
            b'<' if is_ident_start(next) => {
                let e = name_end(ctx, i + 1, b"-.:");
                if ctx.at(e) == b'>' {
                    ctx.pos = e + 1;
                    ctx.emit(i, ctx.pos, Kind::Constant);
                } else {
                    ctx.pos += 1;
                }
            }
            b'@' if is_ident_start(next) => {
                let mut e = name_end(ctx, i + 1, b"-.:");
                while e > i + 1 && matches!(src[e - 1], b'.' | b':') {
                    e -= 1;
                }
                ctx.pos = e;
                ctx.emit(i, e, Kind::Constant);
            }
            b'h' if i == 0 || !is_ident(src[i - 1]) => match url_end(src, i, src.len()) {
                Some(e) => {
                    ctx.pos = e;
                    ctx.emit(i, e, Kind::Link);
                }
                None => ctx.pos += 1,
            },
            _ => {
                let rest = &src[i + 1..];
                let skip = rest.iter().position(|b| b"[]=-+/\\*_`$#<@h".contains(b)).unwrap_or(rest.len());
                ctx.pos = i + 1 + skip;
            }
        }
    }
}

/// Closing `*` or `_` of strong or emphasized text, within the paragraph.
fn typst_span_end(src: &[u8], from: usize, marker: u8) -> Option<usize> {
    let mut i = from;
    while i < near(from, src.len()) {
        match src[i] {
            b'\\' => i += 1,
            b'\n' if src[i + 1..].iter().find(|&&b| b != b' ' && b != b'\t') == Some(&b'\n') => return None,
            b']' => return None,
            b if b == marker => return (i > from).then_some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Raw text: `` `code` `` or a fenced block with a language tag.
fn typst_raw(ctx: &mut Ctx) {
    let src = ctx.src;
    let start = ctx.pos;
    let ticks = src[start..].iter().take_while(|&&b| b == b'`').count();
    if ticks < 3 {
        let close = if ticks == 2 {
            Some(start)
        } else {
            src[start + 1..].iter().position(|&b| b == b'`').map(|i| start + 1 + i)
        };
        ctx.pos = close.map_or(start + ticks, |c| if ticks == 2 { start + 2 } else { c + 1 });
        ctx.emit(start, ctx.pos, Kind::String);
        return;
    }
    let fence = &src[start..start + ticks];
    let lang_end = name_end(ctx, start + ticks, b"-+#");
    let close = ctx.find(lang_end, fence).unwrap_or(src.len());
    ctx.emit(start, start + ticks, Kind::Punctuation);
    ctx.emit(start + ticks, lang_end, Kind::Attribute);
    embed_named(ctx, &src[start + ticks..lang_end], lang_end, close);
    ctx.pos = (close + ticks).min(src.len());
    ctx.emit(close, ctx.pos, Kind::Punctuation);
}

/// Code after `#` in markup: a statement or an expression.
fn typst_hash(ctx: &mut Ctx) {
    let src = ctx.src;
    let start = ctx.pos;
    let p = start + 1;
    let b = ctx.at(p);
    if b.is_ascii_alphabetic() || b == b'_' {
        let e = name_end(ctx, p, b"-");
        let word = &src[p..e];
        if TYPST_STATEMENTS.contains(&word) {
            ctx.emit(start, e, Kind::Keyword);
            let stop = typst_close(src, e, true).unwrap_or(src.len());
            ctx.embed_spec(&TYPST_SPEC, e, stop);
            ctx.pos = stop;
            return;
        }
        let mut stop = e;
        loop {
            match ctx.at(stop) {
                b'.' if ctx.at(stop + 1).is_ascii_alphabetic() => stop = name_end(ctx, stop + 1, b"-"),
                b'(' | b'[' => match typst_close(src, stop, false) {
                    Some(c) => stop = c + 1,
                    None => break,
                },
                _ => break,
            }
        }
        let kind = if matches!(word, b"none" | b"auto" | b"true" | b"false") {
            Kind::Constant
        } else if matches!(ctx.at(e), b'(' | b'[') {
            Kind::Function
        } else {
            Kind::Variable
        };
        ctx.emit(start, e, kind);
        ctx.embed_spec(&TYPST_SPEC, e, stop);
        ctx.pos = stop;
        return;
    }
    if b == b'{' || b == b'(' {
        if let Some(c) = typst_close(src, p, false) {
            ctx.emit(start, p, Kind::Punctuation);
            ctx.embed_spec(&TYPST_SPEC, p, c + 1);
            ctx.pos = c + 1;
            return;
        }
    }
    ctx.pos += 1;
}

/// Offset of the bracket closing the one at `from`, tracking code and content
/// nesting. With `statement`, scans a statement instead and returns where it
/// ends: a newline or `;` at the top level, or an unbalanced closing bracket.
fn typst_close(src: &[u8], from: usize, statement: bool) -> Option<usize> {
    let mut stack = [0u8; 32];
    let mut top = 0usize;
    let mut i = from;
    while i < src.len() {
        let b = src[i];
        let content = top > 0 && stack[top - 1] == b']';
        match b {
            b'\n' | b';' if statement && top == 0 => return Some(i),
            b'(' | b'{' if !content || (i > 0 && (is_ident(src[i - 1]) || src[i - 1] == b'#')) => {
                if top == stack.len() {
                    return None;
                }
                stack[top] = if b == b'(' { b')' } else { b'}' };
                top += 1;
            }
            b'[' => {
                if top == stack.len() {
                    return None;
                }
                stack[top] = b']';
                top += 1;
            }
            b')' | b'}' | b']' => {
                if top == 0 {
                    return if statement { Some(i) } else { None };
                }
                if stack[top - 1] == b {
                    top -= 1;
                    if top == 0 && !statement {
                        return Some(i);
                    }
                }
            }
            b'"' if !content => {
                i += 1;
                while i < src.len() && src[i] != b'"' && src[i] != b'\n' {
                    i += 1 + (src[i] == b'\\') as usize;
                }
            }
            b'\\' => i += 1,
            b'/' if src.get(i + 1) == Some(&b'/') => {
                while i + 1 < src.len() && src[i + 1] != b'\n' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    statement.then_some(src.len())
}

/// Content blocks inside code switch back to markup.
fn typst_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    match ctx.peek() {
        b'[' if ctx.can_nest() => {
            ctx.token(1, Kind::Punctuation);
            ctx.enter();
            typst_markup(ctx, true);
            ctx.leave();
            if ctx.peek() == b']' {
                ctx.token(1, Kind::Punctuation);
            }
            lx.mark_value();
            true
        }
        b'$' => {
            let start = ctx.pos;
            let close = ctx.src[start + 1..].iter().position(|&b| b == b'$').map_or(ctx.len(), |i| start + i + 2);
            ctx.pos = close;
            ctx.emit(start, close, Kind::String);
            lx.mark_value();
            true
        }
        _ => false,
    }
}

static TYPST_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[Str::quoted("\"")],
    keywords: &[
        "let", "set", "show", "if", "else", "for", "in", "while", "break", "continue", "return", "import", "include",
        "as", "and", "or", "not", "context",
    ],
    constants: &["true", "false", "none", "auto"],
    types: &["int", "float", "str", "bool", "array", "dictionary", "content", "length", "ratio", "angle", "color"],
    ident_chars: "-",
    features: CALLS | MEMBERS | IDENT_KEYS | UNITS,
    hook: Some(typst_hook),
    hook_chars: "[$",
    ..Spec::DEFAULT
};

// Gherkin

const GHERKIN_SECTIONS: &[&[u8]] = &[
    b"Feature", b"Background", b"Scenario Outline", b"Scenario Template", b"Scenario", b"Examples", b"Example",
    b"Scenarios", b"Rule", b"Ability", b"Business Need",
];

const GHERKIN_STEPS: &[&[u8]] = &[b"Given ", b"When ", b"Then ", b"And ", b"But ", b"* "];

fn gherkin(ctx: &mut Ctx) {
    each_line(ctx, gherkin_line);
}

fn gherkin_line(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let line = line_text(ctx, start, end);
    let line_end = start + line.len();
    let at = start + indent(ctx, start);
    let content = &src[at..line_end];
    match content.first() {
        None => {}
        Some(b'#') => ctx.emit(at, line_end, Kind::Comment),
        Some(b'@') => {
            let mut p = at;
            while p < line_end {
                if ctx.at(p) == b'#' {
                    ctx.emit(p, line_end, Kind::Comment);
                    break;
                }
                let e = src[p..line_end].iter().position(|&b| b == b' ' || b == b'\t').map_or(line_end, |i| p + i);
                if ctx.at(p) == b'@' {
                    ctx.emit(p, e, Kind::Meta);
                }
                p = ctx.skip_spaces_from(e);
            }
        }
        Some(b'|') => {
            let mut p = at;
            while p < line_end {
                match ctx.at(p) {
                    b'|' => {
                        ctx.emit(p, p + 1, Kind::Punctuation);
                        p += 1;
                    }
                    b'\\' => {
                        ctx.emit(p, p + 2, Kind::Escape);
                        p += 2;
                    }
                    _ => {
                        let e =
                            src[p..line_end].iter().position(|&b| b == b'|' || b == b'\\').map_or(line_end, |i| p + i);
                        gherkin_step(ctx, p, e);
                        p = e;
                    }
                }
            }
        }
        _ if content.starts_with(b"\"\"\"") || content.starts_with(b"```") => {
            let fence = &content[..3];
            ctx.emit(at, at + 3, Kind::Punctuation);
            ctx.emit(at + 3, line_end, Kind::Attribute);
            let media = content[3..].trim_ascii();
            let body = (end + 1).min(ctx.len());
            let mut pos = body;
            let mut close = (ctx.len(), ctx.len());
            while pos < ctx.len() {
                let e = ctx.line_end(pos);
                if line_text(ctx, pos, e).trim_ascii() == fence {
                    let i = pos + indent(ctx, pos);
                    close = (i, i + 3);
                    break;
                }
                pos = e + 1;
            }
            if media.is_empty() {
                ctx.emit(body, close.0, Kind::String);
            } else {
                embed_named(ctx, media, body, close.0);
            }
            ctx.emit(close.0, close.1, Kind::Punctuation);
            ctx.pos = close.1;
        }
        _ => {
            for section in GHERKIN_SECTIONS {
                if content.starts_with(section) && content.get(section.len()) == Some(&b':') {
                    let colon = at + section.len() + 1;
                    ctx.emit(at, colon, Kind::Keyword);
                    ctx.emit(ctx.skip_spaces_from(colon).min(line_end), line_end, Kind::Heading);
                    return;
                }
            }
            for step in GHERKIN_STEPS {
                if content.starts_with(step) {
                    let e = at + step.len() - 1;
                    ctx.emit(at, e, Kind::Keyword);
                    gherkin_step(ctx, e, line_end);
                    return;
                }
            }
        }
    }
}

/// Step text: strings, `<placeholders>` and numbers.
fn gherkin_step(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let mut i = start;
    while i < end {
        let b = src[i];
        match b {
            b'"' => {
                let e = src[i + 1..end].iter().position(|&c| c == b'"').map_or(end, |k| i + k + 2);
                ctx.emit(i, e, Kind::String);
                i = e;
            }
            b'<' => match src[i + 1..end].iter().position(|&c| c == b'>' || c == b' ') {
                Some(k) if src[i + 1 + k] == b'>' && k > 0 => {
                    ctx.emit(i, i + k + 2, Kind::Variable);
                    i += k + 2;
                }
                _ => i += 1,
            },
            b'0'..=b'9' if i == start || !is_ident(src[i - 1]) => {
                let mut e = i;
                while e < end && (src[e].is_ascii_digit() || (src[e] == b'.' && ctx.at(e + 1).is_ascii_digit())) {
                    e += 1;
                }
                if e < end && is_ident(src[e]) {
                    i = e;
                    continue;
                }
                ctx.emit(i, e, Kind::Number);
                i = e;
            }
            _ => i += 1,
        }
    }
    ctx.pos = ctx.pos.max(end);
}

// Indentation based templates: Pug, Haml and Slim

/// Text with `#{expr}` (and `!{expr}`) interpolation in `language`.
fn interpolated_text(ctx: &mut Ctx, start: usize, end: usize, language: &str) {
    let src = ctx.src;
    let mut i = start;
    while let Some(p) = find_in(ctx, i, end, b"{") {
        if p == start || !matches!(src[p - 1], b'#' | b'!') || (p >= start + 2 && src[p - 2] == b'\\') {
            i = p + 1;
            continue;
        }
        match matching(src, p, end, true) {
            Some(c) => {
                ctx.emit(p - 1, p + 1, Kind::Punctuation);
                ctx.embed(lang(language), p + 1, c);
                ctx.emit(c, c + 1, Kind::Punctuation);
                i = c + 1;
            }
            None => break,
        }
    }
    ctx.pos = ctx.pos.max(end);
}

/// An indented block of lines after `end` as `language`, a filter body.
fn filter_block(ctx: &mut Ctx, name: &[u8], end: usize, base: usize) {
    let block = indented_block(ctx, end, base);
    if block > end {
        let name: &[u8] = match name {
            b"plain" | b"escaped" | b"preserve" | b"cdata" => b"text",
            b"coffee" => b"coffeescript",
            other => other,
        };
        embed_named(ctx, name, end + 1, block);
    }
    ctx.pos = block;
}

/// A whole indented block (the line and its children) as a comment.
fn comment_block(ctx: &mut Ctx, at: usize, end: usize, base: usize) {
    let block = indented_block(ctx, end, base);
    ctx.emit(at, block, Kind::Comment);
    ctx.pos = block;
}

/// End of an unquoted attribute value: whitespace, `,` or a closer at depth zero.
fn value_end(src: &[u8], from: usize, end: usize, comma: bool) -> usize {
    const OPS: &[u8] = b"+-*/%=<>!?:&|";
    let mut depth = 0usize;
    let mut i = from;
    while i < end {
        match src[i] {
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < end && src[i] != q {
                    i += 1 + (src[i] == b'\\') as usize;
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    return i;
                }
                depth -= 1;
            }
            b' ' | b'\t' | b'\n' | b'\r' if depth == 0 => {
                // With commas as separators, spaces around operators stay in the value.
                let next = src[i..end].iter().find(|b| !b.is_ascii_whitespace()).copied().unwrap_or(b',');
                if !comma || !(OPS.contains(&next) || (i > from && OPS.contains(&src[i - 1]))) {
                    return i;
                }
            }
            b',' if depth == 0 && comma => return i,
            _ => {}
        }
        i += 1;
    }
    end.min(i)
}

/// `name=value` attribute lists inside `(...)`, with values in `language`.
fn attribute_list(ctx: &mut Ctx, open: usize, close: usize, language: &str) {
    let src = ctx.src;
    ctx.emit(open, open + 1, Kind::Punctuation);
    let mut i = open + 1;
    while i < close {
        match src[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b',' => {
                ctx.emit(i, i + 1, Kind::Punctuation);
                i += 1;
            }
            _ => {
                let mut e = i;
                while e < close && !matches!(src[e], b'=' | b'!' | b',' | b' ' | b'\t' | b'\n' | b'\r') {
                    e += 1;
                }
                let e = if e == i { i + 1 } else { e };
                ctx.emit(i, e, Kind::Attribute);
                i = ctx.skip_spaces_from(e).min(close);
                let op = if src[i..close].starts_with(b"!=") { 2 } else { (src.get(i) == Some(&b'=')) as usize };
                if op == 0 || i >= close {
                    continue;
                }
                ctx.emit(i, i + op, Kind::Operator);
                let v = ctx.skip_spaces_from(i + op).min(close);
                i = value_end(src, v, close, true);
                if matches!(ctx.at(v), b'"' | b'\'') && i == value_end(src, v, close, true) && src[i - 1] == src[v] {
                    ctx.emit(v, i, Kind::String);
                } else {
                    ctx.embed(lang(language), v, i);
                }
            }
        }
    }
    if ctx.at(close) == b')' {
        ctx.emit(close, close + 1, Kind::Punctuation);
    }
    ctx.pos = ctx.pos.max(close);
}

// Pug

const PUG_KEYWORDS: &[&[u8]] = &[
    b"if", b"else", b"unless", b"each", b"for", b"while", b"case", b"when", b"default", b"include", b"extends",
    b"block", b"append", b"prepend", b"mixin", b"doctype", b"yield",
];

fn pug(ctx: &mut Ctx) {
    each_line(ctx, pug_line);
}

fn pug_line(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let line_end = start + line_text(ctx, start, end).len();
    let ind = indent(ctx, start);
    let at = start + ind;
    if at >= line_end {
        return;
    }
    let rest = &src[at..line_end];
    let js = lang("javascript");
    if rest.starts_with(b"//") {
        comment_block(ctx, at, end, ind);
        return;
    }
    match rest[0] {
        b'|' => {
            ctx.emit(at, at + 1, Kind::Punctuation);
            interpolated_text(ctx, at + 1, line_end, "javascript");
        }
        b'<' => ctx.embed(lang("html"), at, line_end),
        b'-' if rest.len() == 1 => {
            ctx.emit(at, at + 1, Kind::Punctuation);
            let block = indented_block(ctx, end, ind);
            ctx.embed(js, end, block);
        }
        b'-' | b'=' => {
            ctx.emit(at, at + 1, Kind::Punctuation);
            ctx.embed(js, at + 1, line_end);
        }
        b'!' if rest.get(1) == Some(&b'=') => {
            ctx.emit(at, at + 2, Kind::Punctuation);
            ctx.embed(js, at + 2, line_end);
        }
        b'+' => {
            let e = name_end(ctx, at + 1, b"-");
            ctx.emit(at, e, Kind::Function);
            let mut p = e;
            if ctx.at(e) == b'(' {
                if let Some(c) = matching(src, e, line_end, true) {
                    ctx.embed(js, e, c + 1);
                    p = c + 1;
                }
            }
            pug_tag(ctx, p, line_end, end, ind);
        }
        b':' => {
            let e = name_end(ctx, at + 1, b"-");
            ctx.emit(at, e, Kind::Meta);
            filter_block(ctx, &src[at + 1..e], end, ind);
        }
        _ => {
            let w = name_end(ctx, at, b"-");
            let word = &src[at..w];
            if PUG_KEYWORDS.contains(&word) && matches!(ctx.at(w), b' ' | b'\t' | b'\r' | b'\n' | 0) {
                ctx.emit(at, w, Kind::Keyword);
                let p = ctx.skip_spaces_from(w).min(line_end);
                match word {
                    b"mixin" | b"block" | b"append" | b"prepend" => {
                        let e = name_end(ctx, p, b"-").min(line_end);
                        ctx.emit(p, e, Kind::Function);
                        if ctx.at(e) == b'(' {
                            ctx.embed(js, e, line_end);
                        }
                    }
                    b"include" | b"extends" | b"doctype" => ctx.emit(p, line_end, Kind::String),
                    b"else" if src[p..line_end].starts_with(b"if") => {
                        ctx.emit(p, p + 2, Kind::Keyword);
                        ctx.embed(js, p + 2, line_end);
                    }
                    _ => ctx.embed(js, p, line_end),
                }
                return;
            }
            pug_tag(ctx, at, line_end, end, ind);
        }
    }
}

/// A tag line: `a.btn#main(href=url, title="x")= expr` or `p: em text`.
fn pug_tag(ctx: &mut Ctx, mut p: usize, mut line_end: usize, mut end: usize, ind: usize) {
    let src = ctx.src;
    let mut tag: &[u8] = b"";
    let mut selector = true;
    while p < line_end {
        let b = src[p];
        match b {
            _ if selector && b.is_ascii_alphabetic() => {
                let e = tag_name_end(ctx, p, line_end);
                tag = &src[p..e];
                ctx.emit(p, e, Kind::Tag);
                p = e;
                selector = false;
            }
            b'.' | b'#' if is_ident_start(ctx.at(p + 1)) || ctx.at(p + 1) == b'-' => {
                let e = name_end(ctx, p + 1, b"-").min(line_end);
                ctx.emit(p, e, Kind::Attribute);
                p = e;
                selector = false;
            }
            b'(' => match matching(src, p, near(p, ctx.len()), true) {
                Some(close) => {
                    attribute_list(ctx, p, close, "javascript");
                    p = close + 1;
                    if p > line_end {
                        end = ctx.line_end(p);
                        line_end = start_of_cr(ctx, end);
                    }
                }
                None => {
                    attribute_list(ctx, p, line_end, "javascript");
                    p = line_end;
                }
            },
            b'&' if src[p..line_end].starts_with(b"&attributes(") => {
                ctx.emit(p, p + 11, Kind::Keyword);
                let close = matching(src, p + 11, line_end, true).map_or(line_end, |c| c + 1);
                ctx.embed(lang("javascript"), p + 11, close);
                p = close;
            }
            b':' if ctx.at(p + 1) == b' ' => {
                ctx.emit(p, p + 1, Kind::Punctuation);
                p = ctx.skip_spaces_from(p + 1);
                selector = true;
            }
            b'/' => {
                ctx.emit(p, p + 1, Kind::Punctuation);
                p += 1;
            }
            _ => break,
        }
    }
    ctx.pos = ctx.pos.max(p);
    if p >= line_end {
        return;
    }
    match src[p] {
        b'.' if p + 1 == line_end => {
            ctx.emit(p, p + 1, Kind::Punctuation);
            let block = indented_block(ctx, end, ind);
            match tag {
                b"script" => ctx.embed(lang("javascript"), end, block),
                b"style" => ctx.embed(lang("css"), end, block),
                _ => interpolated_text(ctx, end, block, "javascript"),
            }
            ctx.pos = block;
        }
        b'=' => {
            ctx.emit(p, p + 1, Kind::Punctuation);
            ctx.embed(lang("javascript"), p + 1, line_end);
        }
        b'!' if ctx.at(p + 1) == b'=' => {
            ctx.emit(p, p + 2, Kind::Punctuation);
            ctx.embed(lang("javascript"), p + 2, line_end);
        }
        _ => interpolated_text(ctx, p, line_end, "javascript"),
    }
}

/// End of a tag name such as `svg:rect`, without a trailing `:`.
fn tag_name_end(ctx: &Ctx, p: usize, end: usize) -> usize {
    let mut e = name_end(ctx, p, b"-:").min(end);
    while e > p + 1 && ctx.src[e - 1] == b':' {
        e -= 1;
    }
    e
}

/// `end` without a preceding carriage return.
fn start_of_cr(ctx: &Ctx, end: usize) -> usize {
    if end > 0 && ctx.at(end - 1) == b'\r' {
        end - 1
    } else {
        end
    }
}

// Haml

fn haml(ctx: &mut Ctx) {
    each_line(ctx, haml_line);
}

fn haml_line(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let line_end = start + line_text(ctx, start, end).len();
    let ind = indent(ctx, start);
    let at = start + ind;
    if at >= line_end {
        return;
    }
    let rest = &src[at..line_end];
    let ruby = lang("ruby");
    match rest[0] {
        b'-' if rest.get(1) == Some(&b'#') => comment_block(ctx, at, end, ind),
        b'/' => comment_block(ctx, at, end, ind),
        b'!' if rest.starts_with(b"!!!") => ctx.emit(at, line_end, Kind::Meta),
        b'\\' => {
            ctx.emit(at, at + 1, Kind::Escape);
            interpolated_text(ctx, at + 1, line_end, "ruby");
        }
        b'=' | b'~' | b'-' => {
            ctx.emit(at, at + 1, Kind::Punctuation);
            ctx.embed(ruby, at + 1, line_end);
        }
        b'!' | b'&' if rest.get(1) == Some(&b'=') => {
            ctx.emit(at, at + 2, Kind::Punctuation);
            ctx.embed(ruby, at + 2, line_end);
        }
        b':' if is_ident_start(ctx.at(at + 1)) => {
            let e = name_end(ctx, at + 1, b"-");
            ctx.emit(at, e, Kind::Meta);
            filter_block(ctx, &src[at + 1..e], end, ind);
        }
        b'%' | b'.' | b'#' if is_ident_start(ctx.at(at + 1)) || ctx.at(at + 1) == b'-' => haml_tag(ctx, at, line_end),
        _ => interpolated_text(ctx, at, line_end, "ruby"),
    }
}

/// `%tag.class#id{ruby: hash}(html="attrs")[obj]/<>= expr` and inline text.
fn haml_tag(ctx: &mut Ctx, mut p: usize, mut line_end: usize) {
    let src = ctx.src;
    let ruby = lang("ruby");
    while p < line_end {
        match src[p] {
            b'%' => {
                let e = name_end(ctx, p + 1, b"-:").min(line_end);
                ctx.emit(p, e, Kind::Tag);
                p = e;
            }
            b'.' | b'#' if is_ident_start(ctx.at(p + 1)) || ctx.at(p + 1) == b'-' => {
                let e = name_end(ctx, p + 1, b"-").min(line_end);
                ctx.emit(p, e, Kind::Attribute);
                p = e;
            }
            b'{' | b'[' | b'(' => {
                let close = matching(src, p, near(p, ctx.len()), true);
                let stop = close.unwrap_or(line_end);
                if src[p] == b'(' {
                    attribute_list(ctx, p, stop, "ruby");
                } else {
                    ctx.emit(p, p + 1, Kind::Punctuation);
                    ctx.embed(ruby, p + 1, stop);
                    if close.is_some() {
                        ctx.emit(stop, stop + 1, Kind::Punctuation);
                    }
                }
                p = if close.is_some() { stop + 1 } else { line_end };
            }
            b'<' | b'>' | b'/' => {
                ctx.emit(p, p + 1, Kind::Punctuation);
                p += 1;
            }
            _ => break,
        }
        if p > line_end {
            line_end = start_of_cr(ctx, ctx.line_end(p));
        }
    }
    ctx.pos = ctx.pos.max(p);
    if p >= line_end {
        return;
    }
    let op = match src[p] {
        b'=' | b'~' => 1,
        b'!' | b'&' if ctx.at(p + 1) == b'=' => 2,
        _ => 0,
    };
    if op > 0 {
        ctx.emit(p, p + op, Kind::Punctuation);
        ctx.embed(ruby, p + op, line_end);
    } else {
        interpolated_text(ctx, p, line_end, "ruby");
    }
}

// Slim

fn slim(ctx: &mut Ctx) {
    each_line(ctx, slim_line);
}

fn slim_line(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let line_end = start + line_text(ctx, start, end).len();
    let ind = indent(ctx, start);
    let at = start + ind;
    if at >= line_end {
        return;
    }
    let rest = &src[at..line_end];
    let ruby = lang("ruby");
    match rest[0] {
        b'/' => comment_block(ctx, at, end, ind),
        b'|' | b'\'' => {
            ctx.emit(at, at + 1, Kind::Punctuation);
            let block = indented_block(ctx, end, ind);
            interpolated_text(ctx, at + 1, block, "ruby");
            ctx.pos = block;
        }
        b'-' | b'=' => {
            let op = 1 + rest[1..].iter().take(2).take_while(|b| matches!(b, b'=' | b'>' | b'<' | b'\'')).count();
            ctx.emit(at, at + op, Kind::Punctuation);
            ctx.embed(ruby, at + op, line_end);
        }
        b'<' => ctx.embed(lang("html"), at, line_end),
        _ => {
            let w = name_end(ctx, at, b"-");
            let word = &src[at..w];
            if word == b"doctype" {
                ctx.emit(at, w, Kind::Keyword);
                ctx.emit(ctx.skip_spaces_from(w).min(line_end), line_end, Kind::String);
            } else if w > at && ctx.at(w) == b':' && w + 1 == line_end {
                ctx.emit(at, w + 1, Kind::Meta);
                filter_block(ctx, word, end, ind);
            } else if w > at || matches!(rest[0], b'.' | b'#') {
                slim_tag(ctx, at, line_end);
            } else {
                interpolated_text(ctx, at, line_end, "ruby");
            }
        }
    }
}

/// `a.btn#main href="x" title=title.upcase = expr` and inline text.
fn slim_tag(ctx: &mut Ctx, mut p: usize, line_end: usize) {
    let src = ctx.src;
    let ruby = lang("ruby");
    let mut selector = true;
    while p < line_end {
        let b = src[p];
        if selector && b.is_ascii_alphabetic() {
            let e = tag_name_end(ctx, p, line_end);
            ctx.emit(p, e, Kind::Tag);
            p = e;
            selector = false;
        } else if matches!(b, b'.' | b'#') && (is_ident_start(ctx.at(p + 1)) || ctx.at(p + 1) == b'-') {
            let e = name_end(ctx, p + 1, b"-").min(line_end);
            ctx.emit(p, e, Kind::Attribute);
            p = e;
            selector = false;
        } else if b == b':' && ctx.at(p + 1) == b' ' {
            ctx.emit(p, p + 1, Kind::Punctuation);
            p = ctx.skip_spaces_from(p + 1);
            selector = true;
        } else {
            break;
        }
    }
    // Wrapped attributes: `a(href="x")`.
    if matches!(ctx.at(p), b'(' | b'[' | b'{') && p < line_end {
        if let Some(close) = matching(src, p, line_end, true) {
            ctx.emit(p, p + 1, Kind::Punctuation);
            slim_attributes(ctx, p + 1, close);
            ctx.emit(close, close + 1, Kind::Punctuation);
            p = close + 1;
        }
    }
    // Bare attributes: `href="x" title=title`.
    p = slim_attributes(ctx, p, line_end);
    ctx.pos = ctx.pos.max(p);
    let p = ctx.skip_spaces_from(p).min(line_end);
    match ctx.at(p) {
        _ if p >= line_end => {}
        b'=' => {
            let op = 1 + (ctx.at(p + 1) == b'=') as usize;
            ctx.emit(p, p + op, Kind::Punctuation);
            ctx.embed(ruby, p + op, line_end);
        }
        b'/' if p + 1 == line_end => ctx.emit(p, p + 1, Kind::Punctuation),
        _ => interpolated_text(ctx, p, line_end, "ruby"),
    }
}

/// Lexes `name=value` pairs from `p` and returns where they stop.
fn slim_attributes(ctx: &mut Ctx, mut p: usize, end: usize) -> usize {
    let src = ctx.src;
    loop {
        let s = skip_whitespace_from(ctx, p).min(end);
        let e = name_end(ctx, s, b"-:@").min(end);
        if e == s || ctx.at(e) != b'=' || ctx.at(e + 1) == b'=' {
            return p;
        }
        ctx.emit(s, e, Kind::Attribute);
        ctx.emit(e, e + 1, Kind::Operator);
        let v = e + 1;
        let stop = value_end(src, v, end, false);
        if matches!(ctx.at(v), b'"' | b'\'') {
            ctx.emit(v, stop, Kind::String);
        } else {
            ctx.embed(lang("ruby"), v, stop);
        }
        p = stop;
    }
}

// Roff

fn roff(ctx: &mut Ctx) {
    // The font selected by `\f` escapes carries over to later lines.
    let mut font = Kind::Text;
    while !ctx.done() {
        let start = ctx.pos;
        let end = ctx.line_end(start);
        let line_end = start + line_text(ctx, start, end).len();
        if matches!(ctx.at(start), b'.' | b'\'') {
            roff_request(ctx, start, line_end);
        } else {
            roff_text(ctx, start, line_end, Kind::Text, &mut font);
        }
        ctx.pos = end;
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

/// A control line: `.SH NAME`, `.B bold`, `.BR ls (1)` or a `.\"` comment.
fn roff_request(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let p = ctx.skip_spaces_from(start + 1).min(end);
    if src[p..end].starts_with(b"\\\"") || src[p..end].starts_with(b"\\#") {
        ctx.emit(start, end, Kind::Comment);
        return;
    }
    let mut e = p;
    while e < end && !matches!(src[e], b' ' | b'\t' | b'\\') {
        e += 1;
    }
    ctx.emit(start, e, Kind::Keyword);
    let name = &src[p..e];
    let fixed = match name {
        b"SH" | b"SS" | b"Sh" | b"Ss" => Some(Kind::Heading),
        b"B" | b"SB" => Some(Kind::Strong),
        b"I" => Some(Kind::Emphasis),
        _ => None,
    };
    let alternating = matches!(name, b"BI" | b"BR" | b"IB" | b"IR" | b"RB" | b"RI");
    let mut i = e;
    let mut n = 0;
    while i < end {
        let s = ctx.skip_spaces_from(i).min(end);
        if s >= end {
            break;
        }
        if src[s..end].starts_with(b"\\\"") {
            ctx.emit(s, end, Kind::Comment);
            break;
        }
        // One argument, possibly quoted.
        let a = if src[s] == b'"' {
            src[s + 1..end].iter().position(|&b| b == b'"').map_or(end, |k| s + k + 2)
        } else {
            let mut a = s;
            while a < end && !matches!(src[a], b' ' | b'\t') {
                a += 1 + (src[a] == b'\\' && a + 1 < end && src[a + 1] != b'"') as usize;
            }
            a.min(end)
        };
        let kind = if alternating {
            match name[n % 2] {
                b'B' => Kind::Strong,
                b'I' => Kind::Emphasis,
                _ => Kind::Text,
            }
        } else if let Some(k) = fixed {
            k
        } else if src[s] == b'"' {
            Kind::String
        } else if src[s..a]
            .iter()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+' | b'n' | b'i' | b'p' | b'v' | b'm' | b'u'))
            && src[s].is_ascii_digit()
        {
            Kind::Number
        } else {
            Kind::Text
        };
        if kind == Kind::String || kind == Kind::Number {
            ctx.emit(s, a, kind);
        } else {
            let mut plain = Kind::Text;
            roff_text(ctx, s, a, kind, &mut plain);
        }
        n += 1;
        i = a;
    }
}

/// Text with escapes. Plain runs get `base`, or the current font when one is set.
fn roff_text(ctx: &mut Ctx, start: usize, end: usize, base: Kind, font: &mut Kind) {
    let src = ctx.src;
    let mut i = start;
    let mut seg = start;
    while i < end {
        if src[i] != b'\\' {
            i += 1;
            continue;
        }
        let kind = if *font != Kind::Text { *font } else { base };
        ctx.emit(seg, i, kind);
        let next = ctx.at(i + 1);
        let mut e = i + 2;
        let mut esc = Kind::Escape;
        match next {
            b'"' | b'#' => {
                ctx.emit(i, end, Kind::Comment);
                ctx.pos = end;
                return;
            }
            b'f' => {
                let (name_start, name_stop, stop) = roff_name(src, i + 2, end);
                e = stop;
                *font = match &src[name_start..name_stop] {
                    b"B" | b"BI" | b"CB" | b"3" | b"4" => Kind::Strong,
                    b"I" | b"CI" | b"2" => Kind::Emphasis,
                    _ => Kind::Text,
                };
            }
            b'(' => e = i + 4,
            b'[' => e = src[i..end].iter().position(|&b| b == b']').map_or(end, |k| i + k + 1),
            b'*' | b'n' | b'$' => {
                e = roff_name(src, i + 2, end).2;
                esc = Kind::Variable;
            }
            b's' => {
                e = i + 2;
                if matches!(ctx.at(e), b'+' | b'-') {
                    e += 1;
                }
                if ctx.at(e) == b'(' {
                    e += 3;
                } else {
                    while ctx.at(e).is_ascii_digit() && e < i + 5 {
                        e += 1;
                    }
                }
            }
            b'X' | b'h' | b'v' | b'w' | b'l' | b'L' | b'D' | b'o' | b'Z' | b'b' | b'N' if ctx.at(i + 2) == b'\'' => {
                e = src[i + 3..end].iter().position(|&b| b == b'\'').map_or(end, |k| i + 3 + k + 1);
            }
            _ if next >= 0x80 => {
                ctx.pos = i + 1;
                ctx.bump_char();
                e = ctx.pos;
            }
            _ => {}
        }
        let mut e = e.min(end);
        while e < end && (src[e] & 0xC0) == 0x80 {
            e += 1;
        }
        ctx.emit(i, e, esc);
        i = e;
        seg = e;
    }
    let kind = if *font != Kind::Text { *font } else { base };
    ctx.emit(seg, end, kind);
    ctx.pos = ctx.pos.max(end);
}

/// The name after `\f`, `\*` or `\n`: one character, `(xx` or `[name]`.
/// Returns the name's range and the end of the escape.
fn roff_name(src: &[u8], at: usize, end: usize) -> (usize, usize, usize) {
    match src.get(at) {
        Some(b'(') => ((at + 1).min(end), (at + 3).min(end), (at + 3).min(end)),
        Some(b'[') => {
            let stop = src[at..end].iter().position(|&b| b == b']').map_or(end, |k| at + k);
            ((at + 1).min(stop), stop, (stop + 1).min(end))
        }
        Some(b) if *b < 0x80 => (at.min(end), (at + 1).min(end), (at + 1).min(end)),
        _ => (at.min(end), at.min(end), at.min(end)),
    }
}

// Texinfo

const TEXINFO_SECTIONS: &[&[u8]] = &[
    b"top", b"chapter", b"section", b"subsection", b"subsubsection", b"unnumbered", b"unnumberedsec",
    b"unnumberedsubsec", b"appendix", b"appendixsec", b"appendixsubsec", b"majorheading", b"chapheading", b"heading",
    b"subheading", b"subsubheading", b"settitle", b"title", b"subtitle", b"node",
];

fn texinfo(ctx: &mut Ctx) {
    let src = ctx.src;
    if ctx.starts_with(b"\\input") {
        ctx.skip_line();
        ctx.emit(0, ctx.pos, Kind::Meta);
    }
    while !ctx.done() {
        let rest = &src[ctx.pos..];
        let skip = rest.iter().position(|&b| matches!(b, b'@' | b'{' | b'}')).unwrap_or(rest.len());
        ctx.pos += skip;
        match ctx.peek() {
            b'@' => texinfo_command(ctx),
            b'{' | b'}' => ctx.token(1, Kind::Punctuation),
            _ => {}
        }
    }
}

fn texinfo_command(ctx: &mut Ctx) {
    let src = ctx.src;
    let start = ctx.pos;
    let next = ctx.at(start + 1);
    if !next.is_ascii_alphabetic() {
        // `@@`, `@{`, `@}`, `@.` and other symbols.
        ctx.pos = start + 1;
        if !ctx.done() {
            ctx.bump_char();
        }
        ctx.emit(start, ctx.pos, Kind::Escape);
        return;
    }
    let e = name_end(ctx, start + 1, b"-");
    let name = &src[start + 1..e];
    let line_end = start_of_cr(ctx, ctx.line_end(e));
    if (name == b"c" || name == b"comment") && matches!(ctx.at(e), b' ' | b'\t' | b'\r' | b'\n' | 0) {
        ctx.pos = line_end;
        ctx.emit(start, line_end, Kind::Comment);
        return;
    }
    ctx.pos = e;
    if ctx.at(e) == b'{' {
        let close = texinfo_brace(src, e);
        let kind = match name {
            b"code" | b"samp" | b"kbd" | b"key" | b"command" | b"env" | b"file" | b"option" | b"verb" | b"t" => {
                Kind::String
            }
            b"emph" | b"i" | b"dfn" | b"cite" | b"slanted" => Kind::Emphasis,
            b"strong" | b"b" => Kind::Strong,
            b"var" => Kind::Variable,
            b"ref" | b"xref" | b"pxref" | b"uref" | b"url" | b"email" | b"anchor" => Kind::Link,
            _ => {
                ctx.emit(start, e, if close == Some(e + 1) { Kind::Escape } else { Kind::Function });
                if close == Some(e + 1) {
                    ctx.pos = e + 2;
                    ctx.emit(e, e + 2, Kind::Escape);
                }
                return;
            }
        };
        ctx.emit(start, e, Kind::Function);
        if let Some(c) = close {
            emit_wrapped(ctx, e, c, kind);
        }
        return;
    }
    ctx.emit(start, e, Kind::Keyword);
    let args = ctx.skip_spaces_from(e).min(line_end);
    if TEXINFO_SECTIONS.contains(&name) {
        ctx.pos = line_end;
        ctx.emit(args, line_end, if name == b"node" { Kind::Link } else { Kind::Heading });
        return;
    }
    if name == b"end" {
        let w = name_end(ctx, args, b"").min(line_end);
        ctx.emit(args, w, Kind::Keyword);
        ctx.pos = w;
        return;
    }
    let body: Option<Kind> = match name {
        b"example" | b"smallexample" | b"lisp" | b"smalllisp" | b"verbatim" | b"display" | b"format" => {
            Some(Kind::String)
        }
        b"ignore" => Some(Kind::Comment),
        b"tex" | b"html" => Some(Kind::Text),
        _ => None,
    };
    let Some(kind) = body else {
        return;
    };
    // Find `@end name` on its own line.
    let mut close = ctx.len();
    let mut pos = line_end;
    while let Some(p) = ctx.find(pos, b"@end ") {
        let w = name_end(ctx, ctx.skip_spaces_from(p + 5), b"");
        if &src[ctx.skip_spaces_from(p + 5)..w] == name && ctx.at_line_start(p) {
            close = p;
            break;
        }
        pos = p + 1;
    }
    let body_start = (line_end + 1).min(close);
    match (kind, name) {
        (Kind::Comment, _) => ctx.emit(start, close, Kind::Comment),
        (Kind::Text, b"tex") => ctx.embed(lang("latex"), body_start, close),
        (Kind::Text, _) => ctx.embed(lang("html"), body_start, close),
        _ => ctx.emit(body_start, close, kind),
    }
    ctx.pos = close;
}

/// Offset of the `}` closing the brace at `open`, skipping `@{` and `@}`.
fn texinfo_brace(src: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < near(open, src.len()) {
        match src[i] {
            b'@' => i += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

// Pod

fn pod(ctx: &mut Ctx) {
    let mut code = false;
    while !ctx.done() {
        let start = ctx.pos;
        let end = ctx.line_end(start);
        let command = ctx.at(start) == b'=' && ctx.at(start + 1).is_ascii_alphabetic();
        if code && !command {
            // Perl code after `=cut`, up to the next command paragraph.
            let mut stop = ctx.len();
            let mut p = start;
            while let Some(i) = ctx.find(p, b"\n=") {
                if ctx.at(i + 2).is_ascii_alphabetic() {
                    stop = i + 1;
                    break;
                }
                p = i + 1;
            }
            ctx.embed(lang("perl"), start, stop);
            ctx.pos = stop;
            continue;
        }
        code = false;
        if line_text(ctx, start, end).trim_ascii().is_empty() {
            ctx.pos = end + (end < ctx.len()) as usize;
            continue;
        }
        // A paragraph runs to the next blank line.
        let mut para_end = end;
        while para_end < ctx.len() {
            let s = para_end + 1;
            let e = ctx.line_end(s);
            if line_text(ctx, s, e).trim_ascii().is_empty() {
                break;
            }
            para_end = e;
        }
        if command {
            code = pod_command(ctx, start, end, para_end);
        } else if matches!(ctx.at(start), b' ' | b'\t') {
            ctx.emit(start, para_end, Kind::String);
        } else {
            pod_inline(ctx, start, para_end);
        }
        ctx.pos = ctx.pos.max(para_end).max(end);
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

/// A command paragraph. Returns true after `=cut`.
fn pod_command(ctx: &mut Ctx, start: usize, end: usize, para_end: usize) -> bool {
    let src = ctx.src;
    let e = name_end(ctx, start + 1, b"");
    let name = &src[start + 1..e];
    let line_end = start_of_cr(ctx, end);
    ctx.emit(start, e, Kind::Keyword);
    let args = ctx.skip_spaces_from(e).min(line_end);
    match name {
        b"cut" => return true,
        b"head1" | b"head2" | b"head3" | b"head4" | b"head5" | b"head6" => ctx.emit(args, para_end, Kind::Heading),
        b"over" => ctx.emit(args, line_end, Kind::Number),
        b"encoding" => ctx.emit(args, line_end, Kind::String),
        b"item" => {
            // Bullets and numbers: `=item *` and `=item 2.`
            let marker = src[args..line_end].iter().take_while(|b| b.is_ascii_digit()).count();
            let marker = match ctx.at(args + marker) {
                b'*' if marker == 0 && matches!(ctx.at(args + 1), b' ' | b'\t' | b'\r' | b'\n' | 0) => 1,
                b'.' if marker > 0 => marker + 1,
                _ => 0,
            };
            ctx.emit(args, args + marker, Kind::Keyword);
            pod_inline(ctx, args + marker, para_end);
        }
        b"for" | b"begin" | b"end" => {
            let f = name_end(ctx, args, b"-:").min(line_end);
            let format = &src[args..f];
            ctx.emit(args, f, Kind::Attribute);
            if name == b"for" {
                let kind = if format == b"comment" { Kind::Comment } else { Kind::String };
                ctx.emit(ctx.skip_spaces_from(f).min(para_end), para_end, kind);
            } else if name == b"begin" {
                // The body runs to `=end format`.
                let mut close = ctx.len();
                let mut p = line_end;
                while let Some(i) = ctx.find(p, b"\n=end") {
                    let s = ctx.skip_spaces_from(i + 5);
                    if src[s..].starts_with(format) {
                        close = i + 1;
                        break;
                    }
                    p = i + 1;
                }
                let body = (line_end + 1).min(close);
                match format {
                    b"comment" => ctx.emit(body, close, Kind::Comment),
                    b"html" => ctx.embed(lang("html"), body, close),
                    _ => ctx.emit(body, close, Kind::String),
                }
                ctx.pos = close;
            }
        }
        _ => pod_inline(ctx, args, para_end),
    }
    false
}

/// Formatting codes: `B<bold>`, `I<italic>`, `C<< code >>`, `L<link>` and `E<gt>`.
fn pod_inline(ctx: &mut Ctx, start: usize, end: usize) {
    let src = ctx.src;
    let mut i = start;
    while i < end {
        let b = src[i];
        if !(b"BICLEFSXZ".contains(&b) && ctx.at(i + 1) == b'<' && (i == start || !is_ident(src[i - 1]))) {
            i += 1;
            continue;
        }
        let angles = src[i + 1..end].iter().take_while(|&&c| c == b'<').count();
        let end = near(i, end);
        let close = if angles > 1 {
            // `C<< a <=> b >>`: the closer is the same number of `>` after whitespace.
            let closer = &b">>>>>>>>"[..angles.min(8)];
            let mut p = i + 1 + angles;
            let mut found = None;
            while let Some(c) = find_in(ctx, p, end, closer) {
                if src[c - 1].is_ascii_whitespace() {
                    found = Some(c + angles);
                    break;
                }
                p = c + 1;
            }
            found
        } else {
            let mut depth = 0usize;
            let mut found = None;
            let mut p = i + 1;
            while p < end {
                match src[p] {
                    b'<' => depth += 1,
                    b'>' => {
                        depth -= 1;
                        if depth == 0 {
                            found = Some(p + 1);
                            break;
                        }
                    }
                    _ => {}
                }
                p += 1;
            }
            found
        };
        let Some(close) = close else {
            i += 1;
            continue;
        };
        let kind = match b {
            b'B' => Kind::Strong,
            b'I' => Kind::Emphasis,
            b'C' | b'F' => Kind::String,
            b'L' => Kind::Link,
            b'E' => Kind::Escape,
            b'X' => Kind::Meta,
            _ => Kind::Text,
        };
        ctx.emit(i, close, kind);
        i = close;
    }
    ctx.pos = ctx.pos.max(end);
}
