//! Dynamic scripting languages.

use super::{Definition, Engine};
use crate::lexer::generic::{escape_len, features::*, Interp, Lexer, Prefix, Spec, Str};
use crate::lexer::{is_ident, is_ident_start, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[
    &PYTHON, &RUBY, &PERL, &LUA, &R, &JULIA, &ELIXIR, &CRYSTAL, &NIM, &COFFEESCRIPT, &TCL, &GDSCRIPT, &MOJO, &RAKU,
    &AUTOHOTKEY, &APPLESCRIPT, &WREN, &SQUIRREL, &PONY, &MOONSCRIPT, &LIVESCRIPT,
];

// Shared helpers

/// The byte that closes `open`: brackets pair up, anything else closes itself.
fn closing(open: u8) -> u8 {
    match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b'<' => b'>',
        b => b,
    }
}

/// Offset of the bracket closing the one at `open` on the same line, for short
/// subscripts such as `$h{key}` inside strings. Never looks past `stop` (the
/// literal's own closing byte) or more than a hundred bytes ahead.
fn subscript_close(src: &[u8], open: usize, limit: usize, stop: u8) -> Option<usize> {
    let o = *src.get(open)?;
    let c = closing(o);
    let limit = limit.min(open + 128).min(src.len());
    let mut depth = 0u32;
    for (i, &b) in src.iter().enumerate().take(limit).skip(open) {
        if b == o {
            depth += 1;
        } else if b == c {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        } else if b == b'\n' || (b == stop && i > open) {
            return None;
        }
    }
    None
}

/// How the body of a quote like literal is scanned and highlighted.
#[derive(Clone, Copy)]
struct Quote {
    kind: Kind,
    /// Escape byte (`\` or AutoHotkey's backtick), or 0 for none.
    escape: u8,
    /// Interpolation opener such as `#{`, ending in an opening bracket.
    interp: &'static [u8],
    /// The language hook lexes the whole interpolation, brackets included (Tcl's `[cmd]`).
    hook_interp: bool,
    /// Sigils that start a variable inside the literal (`$name`, `@list`).
    vars: &'static [u8],
    /// Perl style `$h{key}`, `$a[0]`, `$r->{x}` and `@{[ expr ]}` after a sigil.
    subscripts: bool,
    multiline: bool,
}

impl Quote {
    const fn new(kind: Kind) -> Quote {
        Quote { kind, escape: b'\\', interp: b"", hook_interp: false, vars: b"", subscripts: false, multiline: true }
    }

    const fn interp(self, interp: &'static [u8]) -> Quote {
        Quote { interp, ..self }
    }

    const fn vars(self, vars: &'static [u8]) -> Quote {
        Quote { vars, ..self }
    }

    const fn escape(self, escape: u8) -> Quote {
        Quote { escape, ..self }
    }

    const fn single_line(self) -> Quote {
        Quote { multiline: false, ..self }
    }

    const fn with_kind(self, kind: Kind) -> Quote {
        Quote { kind, ..self }
    }

    /// The same literal without escapes or interpolation (`'...'` heredocs).
    const fn raw(self) -> Quote {
        Quote { escape: 0, interp: b"", vars: b"", subscripts: false, ..self }
    }
}

/// End of a variable name starting at `from`, just after its sigil: `name`,
/// `Pkg::name`, `::name`, digits or `{name}`. Returns `from` when there is none.
fn var_end(ctx: &Ctx, from: usize, limit: usize) -> usize {
    let first = ctx.at(from);
    if first == b'{' {
        let mut e = from + 1;
        while e < limit && (is_ident(ctx.src[e]) || ctx.src[e] == b':') {
            e += 1;
        }
        return if e > from + 1 && e < limit && ctx.src[e] == b'}' { e + 1 } else { from };
    }
    let mut e = from;
    if first == b':' && ctx.at(from + 1) == b':' && is_ident_start(ctx.at(from + 2)) {
        e += 2;
    } else if !is_ident(first) {
        return from;
    }
    while e < limit {
        let b = ctx.src[e];
        if is_ident(b) {
            e += 1;
        } else if b == b':' && ctx.at(e + 1) == b':' && e + 2 < limit && is_ident_start(ctx.src[e + 2]) {
            e += 2;
        } else {
            break;
        }
    }
    e.min(limit)
}

/// Offset just after the last non blank byte before `pos` on its line.
fn skip_back_spaces(ctx: &Ctx, pos: usize) -> usize {
    let mut i = pos;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    i
}

/// Whether a `%` or `&` at the cursor is a sigil rather than an operator: in
/// expression position, or after a space and glued to a name (`keys %h`).
fn sigil_position(lx: &Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let next = ctx.at(pos + 1);
    lx.expression_start()
        || (pos > 0
            && matches!(ctx.src[pos - 1], b' ' | b'\t')
            && (is_ident_start(next) || matches!(next, b'$' | b'{' | b'*' | b'!' | b'?')))
}

/// Lexes a literal body from `from` up to the unbalanced `close` byte (or up
/// to `limit` when `close` is 0) and returns its end. Tokens start at `start`
/// so a prefix and the opening delimiter join the literal.
fn scan_quoted(lx: &mut Lexer, start: usize, from: usize, limit: usize, open: u8, close: u8, q: Quote) -> usize {
    let limit = limit.min(lx.ctx.len());
    let mut seg = start;
    let mut depth = 0u32;
    let mut i = from;
    while i < limit {
        let b = lx.ctx.src[i];
        if b == q.escape && b != 0 {
            if matches!(q.kind, Kind::Regex | Kind::Comment) {
                i += 2;
                while i < limit && lx.ctx.src[i] & 0xC0 == 0x80 {
                    i += 1;
                }
                continue;
            }
            let ctx = &mut *lx.ctx;
            ctx.emit(seg, i, q.kind);
            ctx.pos = i;
            escape_len(ctx);
            let end = ctx.pos.min(limit);
            ctx.emit(i, end, Kind::Escape);
            i = end;
            seg = end;
            continue;
        }
        if !q.interp.is_empty() && lx.ctx.starts_with_at(i, q.interp) && lx.ctx.can_nest() {
            lx.ctx.emit(seg, i, q.kind);
            i = interpolation(lx, i, i + q.interp.len(), limit, q.hook_interp);
            seg = i;
            continue;
        }
        if q.vars.contains(&b) {
            let end = var_end(lx.ctx, i + 1, limit);
            if end > i + 1 {
                lx.ctx.emit(seg, i, q.kind);
                lx.ctx.emit(i, end, Kind::Variable);
                i = end;
                if q.subscripts {
                    i = subscripts(lx, i, limit, if open == close { close } else { 0 });
                }
                seg = i;
                continue;
            }
            // `@{[ expr ]}` and `${\ expr}` run code inside the string.
            if q.subscripts && lx.ctx.at(i + 1) == b'{' && lx.ctx.can_nest() {
                lx.ctx.emit(seg, i, q.kind);
                i = interpolation(lx, i, i + 2, limit, false);
                seg = i;
                continue;
            }
        }
        if b == b'\n' && !q.multiline {
            break;
        }
        if close != 0 {
            if b == open && open != close {
                depth += 1;
            } else if b == close {
                if depth == 0 {
                    i += 1;
                    break;
                }
                depth -= 1;
            }
        }
        i += 1;
    }
    let end = i.min(limit);
    lx.ctx.emit(seg, end, q.kind);
    lx.ctx.pos = end;
    lx.mark_value();
    end
}

/// Lexes the interpolation at `at` as code up to the bracket that closes the
/// one just before `body`, never past `limit`, and returns its end. As in the
/// engine, an unclosed interpolation runs on instead of looking ahead for its
/// close, so no input can make this quadratic.
fn interpolation(lx: &mut Lexer, at: usize, body: usize, limit: usize, hook: bool) -> usize {
    let full = lx.ctx.src;
    lx.ctx.src = &full[..limit];
    lx.ctx.pos = at;
    match lx.spec.hook {
        Some(h) if hook => {
            if !h(lx) {
                lx.ctx.pos = at + 1;
            }
        }
        _ => {
            let close = closing(full[body - 1]);
            lx.ctx.emit(at, body, Kind::Punctuation);
            lx.ctx.pos = body;
            lx.nested(close);
            if lx.ctx.peek() == close {
                lx.ctx.token(1, Kind::Punctuation);
            }
        }
    }
    lx.ctx.src = full;
    lx.ctx.pos
}

/// Perl subscripts after a variable in a string (`{key}`, `[0]`, `->` before
/// either), lexed as code. Returns their end.
fn subscripts(lx: &mut Lexer, from: usize, limit: usize, stop: u8) -> usize {
    let spec = lx.spec;
    let mut i = from;
    loop {
        let arrow = if lx.ctx.starts_with_at(i, b"->") { 2 } else { 0 };
        if !matches!(lx.ctx.at(i + arrow), b'[' | b'{') {
            return i;
        }
        let Some(c) = subscript_close(lx.ctx.src, i + arrow, limit, stop) else {
            return i;
        };
        lx.ctx.emit(i, i + arrow, Kind::Operator);
        lx.ctx.embed_spec(spec, i + arrow, c + 1);
        i = c + 1;
    }
}

/// Lexes a literal whose opening delimiter is at `open_at`. Brackets nest;
/// any other delimiter closes itself.
fn quote(lx: &mut Lexer, start: usize, open_at: usize, q: Quote) -> usize {
    let open = lx.ctx.at(open_at);
    let limit = lx.ctx.len();
    scan_quoted(lx, start, open_at + 1, limit, open, closing(open), q)
}

/// Regex flags (`/x/gi`) after a literal that ends at `end`.
fn flags(lx: &mut Lexer, end: usize, kind: Kind) {
    let ctx = &mut *lx.ctx;
    let mut e = end;
    while ctx.at(e).is_ascii_alphabetic() {
        e += 1;
    }
    ctx.emit(end, e, kind);
    ctx.pos = ctx.pos.max(e);
}

/// `/regex/flags` at the cursor, in expression position. `interp` is the
/// interpolation opener (`#{`) or empty. With `classes`, a `/` inside `[...]`
/// does not end the literal (JavaScript rules) and a class may span at most 64
/// bytes, so a run of unclosed ones stays linear; otherwise (Ruby, Perl) the
/// first unescaped `/` ends it. A `/` with no closing one on its line is division.
fn slash_regex(lx: &mut Lexer, classes: bool, interp: &'static [u8]) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    if matches!(ctx.at(pos + 1), b'/' | b'*' | b'\n') {
        return false;
    }
    // Start of the open character class, or 0.
    let (mut i, mut class, mut interpolated) = (pos + 1, 0, false);
    let close = loop {
        if i >= ctx.len() || (class > 0 && i - class > 64) {
            break None;
        }
        match ctx.src[i] {
            b'\n' => break None,
            b'\\' => i += 1,
            b'[' if classes && class == 0 => class = i,
            b']' => class = 0,
            b'/' if class == 0 => break Some(i),
            b if !interp.is_empty() && b == interp[0] && ctx.starts_with_at(i, interp) => interpolated = true,
            _ => {}
        }
        i += 1;
    };
    let Some(close) = close else {
        lx.ctx.token(1, Kind::Operator);
        lx.mark_operator();
        return true;
    };
    if interpolated {
        quote(lx, pos, pos, Quote::new(Kind::Regex).interp(interp).single_line());
    } else {
        emit_to(lx, close + 1, Kind::Regex);
    }
    flags(lx, lx.ctx.pos, Kind::Regex);
    lx.mark_value();
    true
}

/// Start of the identifier that ends at `end`, if there is one. Words joined
/// by `-` between letters (`read-file`) count as one.
fn word_before(lx: &Lexer, end: usize) -> Option<usize> {
    let src = lx.ctx.src;
    let mut s = end;
    loop {
        while s > 0 && lx.is_ident_byte(src[s - 1]) {
            s -= 1;
        }
        if s >= 2 && s < end && src[s - 1] == b'-' && src[s].is_ascii_alphabetic() && src[s - 2].is_ascii_alphabetic() {
            s -= 1;
        } else {
            break;
        }
    }
    (s < end && !src[s].is_ascii_digit()).then_some(s)
}

/// Retags the word that ends just before `pos` (ignoring spaces) as `kind`.
fn retag_prev_word(lx: &mut Lexer, pos: usize, kind: Kind) -> bool {
    let src = lx.ctx.src;
    let mut end = pos;
    while end > 0 && matches!(src[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    let Some(ws) = word_before(lx, end) else {
        return false;
    };
    if ws > 0 && matches!(src[ws - 1], b'@' | b'$' | b'%') {
        return false;
    }
    lx.ctx.truncate_to(ws);
    lx.ctx.emit(ws, end, kind);
    true
}

/// `name: value` hash keys and keyword arguments: retags the word right
/// before the `:` at the cursor as a property. Words in `skip` keep their kind.
fn key_colon(lx: &mut Lexer, skip: &[&str]) -> bool {
    let pos = lx.ctx.pos;
    let next = lx.ctx.at(pos + 1);
    if pos + 1 < lx.ctx.len() && !matches!(next, b' ' | b'\t' | b'\r' | b'\n' | b',' | b')' | b'}' | b']') {
        return false;
    }
    let Some(ws) = word_before(lx, pos) else {
        return false;
    };
    let src = lx.ctx.src;
    if ws > 0 && matches!(src[ws - 1], b'.' | b':' | b'@' | b'$' | b'?') {
        return false;
    }
    // `a ? b: c` is a ternary; `foo? b: 1` passes a keyword argument to `foo?`.
    let before = skip_back_spaces(lx.ctx, ws);
    if before < ws && before > 0 && src[before - 1] == b'?' && (before < 2 || !is_ident(src[before - 2])) {
        return false;
    }
    if skip.iter().any(|w| w.as_bytes() == &src[ws..pos]) {
        return false;
    }
    let ctx = &mut *lx.ctx;
    ctx.truncate_to(ws);
    ctx.emit(ws, pos, Kind::Property);
    ctx.token(1, Kind::Punctuation);
    lx.mark_operator();
    // `class:` and `def:` as keys must not name the next word.
    lx.set_pending(None);
    true
}

/// Whether an optional parameter list and then one of `arrows` follow `from`.
fn arrow_ahead(ctx: &Ctx, from: usize, arrows: &[&[u8]]) -> bool {
    let mut i = ctx.skip_spaces_from(from);
    if ctx.at(i) == b'(' {
        let limit = (i + 512).min(ctx.len());
        let mut depth = 0;
        while i < limit {
            match ctx.src[i] {
                b'\n' => return false,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if i >= limit {
            return false;
        }
        i = ctx.skip_spaces_from(i + 1);
    }
    arrows.iter().any(|a| ctx.starts_with_at(i, a))
}

/// `name = (args) ->` and `name: ->` at an assignment operator: marks `name`
/// as a function. Never consumes input.
fn arrow_assignment(lx: &mut Lexer, arrows: &[&[u8]]) -> bool {
    let pos = lx.ctx.pos;
    if lx.ctx.peek() == b'=' && matches!(lx.ctx.peek_at(1), b'=' | b'>') {
        return false;
    }
    if arrow_ahead(lx.ctx, pos + 1, arrows) {
        retag_prev_word(lx, pos, Kind::Function);
    }
    false
}

/// A heredoc at `<<`: `<<~EOS`, `<<-EOS`, `<<'EOS'` or `<<"EOS"`. `indent`
/// lists the marks that allow an indented terminator. The rest of the opening
/// line is code; the body runs to the terminator line.
fn heredoc(lx: &mut Lexer, indent: &[u8], q: Quote) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    let mut i = start + 2;
    let indented = indent.contains(&ctx.at(i));
    if indented {
        i += 1;
    }
    let quote = ctx.at(i);
    let quoted = matches!(quote, b'\'' | b'"' | b'`');
    let word_start = if quoted { i + 1 } else { i };
    let first = ctx.at(word_start);
    if !(first.is_ascii_uppercase() || ((indented || quoted) && is_ident_start(first) && first < 0x80)) {
        return false;
    }
    let mut word_end = word_start;
    while is_ident(ctx.at(word_end)) {
        word_end += 1;
    }
    if quoted && ctx.at(word_end) != quote {
        return false;
    }
    let marker_end = word_end + quoted as usize;
    let mark = word_start - quoted as usize;
    lx.ctx.emit(start, mark, Kind::Operator);
    lx.ctx.emit(mark, marker_end, Kind::String);
    let body = if quote == b'\'' { q.raw() } else { q };
    heredoc_body(lx, marker_end, word_start, word_end, indented, body);
    true
}

/// The rest of a heredoc whose opening marker ends at `marker_end`: the rest of
/// that line is code, then the body runs to the line holding only the word at
/// `word_start..word_end` (after indentation when `indented`).
fn heredoc_body(lx: &mut Lexer, marker_end: usize, word_start: usize, word_end: usize, indented: bool, q: Quote) {
    let ctx = &*lx.ctx;
    let line_end = ctx.line_end(marker_end);
    let body_start = line_end + 1;
    let delimiter = &ctx.src[word_start..word_end];
    let (mut body_end, mut term_end) = (ctx.len(), ctx.len());
    let mut pos = body_start;
    while pos < ctx.len() {
        let end = ctx.line_end(pos);
        let mut line = &ctx.src[pos..end];
        if indented {
            while let [b' ' | b'\t', rest @ ..] = line {
                line = rest;
            }
        }
        if line.strip_suffix(b"\r").unwrap_or(line) == delimiter {
            body_end = pos;
            term_end = end;
            break;
        }
        pos = end + 1;
    }
    let spec = lx.spec;
    let ctx = &mut *lx.ctx;
    ctx.embed_spec(spec, marker_end, line_end);
    if body_start >= ctx.len() {
        ctx.pos = ctx.len();
        return;
    }
    scan_quoted(lx, body_start, body_start, body_end, 0, 0, q);
    lx.ctx.emit(body_end, term_end, Kind::String);
    lx.ctx.pos = term_end;
}

/// Emits from the cursor to `end` as `kind` and moves past it.
fn emit_to(lx: &mut Lexer, end: usize, kind: Kind) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let end = end.min(ctx.len());
    ctx.pos = end;
    ctx.emit(start, end, kind);
    end > start
}

/// Lua's `..` concatenation (and `...` varargs), an operator rather than
/// punctuation.
fn lua_concat(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    if ctx.peek() != b'.' || ctx.peek_at(1) != b'.' {
        return false;
    }
    let len = if ctx.peek_at(2) == b'.' { 3 } else { 2 };
    ctx.token(len, Kind::Operator);
    lx.mark_operator();
    true
}

/// Documentation block from a `=directive` line up to the line starting
/// with `close` (or the end of input).
fn pod_block(lx: &mut Lexer, close: &[u8]) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let line = ctx.line_end(pos);
    let mut i = line;
    let end = loop {
        if i >= ctx.len() {
            break ctx.len();
        }
        if ctx.starts_with_at(i + 1, close) {
            break ctx.line_end(i + 1);
        }
        i = ctx.line_end(i + 1);
    };
    emit_to(lx, end, Kind::Comment)
}

// Python and Mojo

/// What a backslash means inside a format string.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Backslash {
    /// Starts an escape sequence.
    Escape,
    /// Python raw strings: kept as is, but `\"` does not end the string.
    Skip,
    /// An ordinary character (Nim raw strings).
    Literal,
}

/// A format string with `{expr!r:spec}` fields (Python f-strings, Nim `fmt`).
#[derive(Clone, Copy)]
struct Fmt {
    delim: &'static [u8],
    backslash: Backslash,
    multiline: bool,
    /// Fields may hold strings using the same quote (Python 3.12).
    nested_quotes: bool,
}

/// Lexes a format string body from `from` (just after the opening `delim`)
/// with tokens starting at `start`, so a prefix joins the literal.
fn format_string(lx: &mut Lexer, start: usize, from: usize, f: Fmt) {
    let len = lx.ctx.len();
    let mut seg = start;
    let mut i = from;
    let end = loop {
        let src = lx.ctx.src;
        let skip = src[i..]
            .iter()
            .position(|&b| matches!(b, b'{' | b'}' | b'\\' | b'\n') || b == f.delim[0])
            .unwrap_or(len - i);
        i += skip;
        if i >= len {
            break len;
        }
        let next = lx.ctx.at(i + 1);
        match src[i] {
            b'\\' => match f.backslash {
                Backslash::Escape => {
                    let ctx = &mut *lx.ctx;
                    ctx.emit(seg, i, Kind::String);
                    ctx.pos = i;
                    escape_len(ctx);
                    ctx.emit(i, ctx.pos, Kind::Escape);
                    i = ctx.pos;
                    seg = i;
                }
                Backslash::Skip => i = (i + 2).min(len),
                Backslash::Literal => i += 1,
            },
            // `{{` and `}}` stand for literal braces.
            b @ (b'{' | b'}') if next == b => i += 2,
            b'{' => {
                lx.ctx.emit(seg, i, Kind::String);
                match format_field(lx, i, f) {
                    Some(end) => {
                        i = end;
                        seg = end;
                    }
                    None => i += 1,
                }
            }
            b'\n' if !f.multiline => break i,
            _ if src[i..].starts_with(f.delim) => break i + f.delim.len(),
            _ => i += 1,
        }
    };
    lx.ctx.emit(seg, end, Kind::String);
    lx.ctx.pos = end;
    lx.mark_value();
}

/// A `{expr!r:spec}` replacement field at `at`. Returns its end, or `None`
/// when no `}`, `!` or `:` ends the expression within a short distance, in
/// which case the brace is plain text.
fn format_field(lx: &mut Lexer, at: usize, f: Fmt) -> Option<usize> {
    if !lx.ctx.can_nest() {
        return None;
    }
    let src = lx.ctx.src;
    let limit = (at + 256).min(src.len());
    let mut depth = 0u32;
    let mut j = at + 1;
    let stop = loop {
        match *src[..limit].get(j)? {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b'}' if depth == 0 => break j,
            b'}' => depth -= 1,
            b'!' if depth == 0 && src.get(j + 1) != Some(&b'=') => break j,
            b':' if depth == 0 => break j,
            b'\n' if !f.multiline => return None,
            q @ (b'"' | b'\'') => {
                if !f.nested_quotes && src[j..].starts_with(f.delim) {
                    return None;
                }
                let k = src[j + 1..limit].iter().position(|&b| b == q || b == b'\n')?;
                if src[j + 1 + k] != q {
                    return None;
                }
                j += k + 1;
            }
            _ => {}
        }
        j += 1;
    };
    lx.ctx.emit(at, at + 1, Kind::Punctuation);
    lx.ctx.src = &src[..stop];
    lx.ctx.pos = at + 1;
    lx.nested(b'}');
    lx.ctx.src = src;
    let mut k = stop;
    if src[k] != b'}' {
        // `!r` conversion and `:spec`, which may hold nested `{fields}`.
        let mut seg = k;
        if src[k] == b'!' {
            k += 1;
            while src.get(k).is_some_and(|b| b.is_ascii_alphabetic()) {
                k += 1;
            }
            lx.ctx.emit(seg, k, Kind::Punctuation);
            seg = k;
        }
        if src.get(k) == Some(&b':') {
            k += 1;
            lx.ctx.emit(seg, k, Kind::Punctuation);
            seg = k;
        }
        loop {
            match src.get(k) {
                None => break,
                Some(b'}') => break,
                Some(b'{') => {
                    lx.ctx.emit(seg, k, Kind::String);
                    lx.ctx.enter();
                    let end = format_field(lx, k, f);
                    lx.ctx.leave();
                    k = end.unwrap_or(k + 1);
                    seg = k;
                }
                Some(b'\n') if !f.multiline => break,
                _ if src[k..].starts_with(f.delim) => break,
                _ => k += 1,
            }
        }
        lx.ctx.emit(seg, k, Kind::String);
    }
    lx.ctx.pos = k;
    if lx.ctx.peek() == b'}' {
        lx.ctx.token(1, Kind::Punctuation);
    }
    Some(lx.ctx.pos)
}

/// `f"..."` and `t"..."` strings with any `r` prefix.
fn python_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let (mut i, mut fmt, mut raw) = (pos, false, false);
    while i < pos + 2 {
        match ctx.at(i) | 0x20 {
            b'f' | b't' => fmt = true,
            b'r' => raw = true,
            _ => break,
        }
        i += 1;
    }
    let q = ctx.at(i);
    if !fmt || !matches!(q, b'"' | b'\'') || (pos > 0 && is_ident(ctx.src[pos - 1])) {
        return false;
    }
    let triple = ctx.at(i + 1) == q && ctx.at(i + 2) == q;
    let delim: &'static [u8] = match (q, triple) {
        (b'"', true) => b"\"\"\"",
        (b'"', false) => b"\"",
        (_, true) => b"'''",
        _ => b"'",
    };
    let backslash = if raw { Backslash::Skip } else { Backslash::Escape };
    format_string(lx, pos, i + delim.len(), Fmt { delim, backslash, multiline: triple, nested_quotes: true });
    true
}

const PY_STRINGS: &[Str] =
    &[Str::quoted("\"\"\"").multiline(), Str::quoted("'''").multiline(), Str::quoted("\""), Str::quoted("'")];

const PY_PREFIXES: &[Prefix] = &[
    Prefix::raw(b'r'),
    Prefix::raw(b'R'),
    Prefix::plain(b'b'),
    Prefix::plain(b'B'),
    Prefix::plain(b'u'),
    Prefix::plain(b'U'),
];

const PY_TYPES: &[&str] = &[
    "int", "float", "str", "bool", "bytes", "bytearray", "list", "dict", "set", "frozenset", "tuple", "object",
    "complex", "memoryview", "type",
];

const PY_BUILTINS: &[&str] = &[
    "abs", "aiter", "all", "anext", "any", "ascii", "bin", "breakpoint", "callable", "chr", "classmethod", "compile",
    "delattr", "dir", "divmod", "enumerate", "eval", "exec", "filter", "format", "getattr", "globals", "hasattr",
    "hash", "help", "hex", "id", "input", "isinstance", "issubclass", "iter", "len", "locals", "map", "max", "min",
    "next", "oct", "open", "ord", "pow", "print", "property", "range", "repr", "reversed", "round", "setattr", "slice",
    "sorted", "staticmethod", "sum", "super", "vars", "zip", "__import__", "__name__", "__file__", "__doc__",
];

const PY_CONSTANTS: &[&str] = &["True", "False", "None", "NotImplemented", "Ellipsis", "__debug__"];

const PY_FEATURES: u32 = CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS;

static PYTHON_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: PY_STRINGS,
    string_prefixes: PY_PREFIXES,
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "case", "continue", "del", "elif", "else", "except",
        "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "match", "nonlocal", "not", "or",
        "pass", "raise", "return", "try", "while", "with", "yield",
    ],
    variables: &["self", "cls"],
    types: PY_TYPES,
    builtins: PY_BUILTINS,
    constants: PY_CONSTANTS,
    function_keywords: &["def"],
    type_keywords: &["class"],
    features: PY_FEATURES,
    hook: Some(python_hook),
    hook_chars: "fFtTrR",
    ..Spec::DEFAULT
};

pub static PYTHON: Definition = Definition::new("python", "Python", Engine::Generic(&PYTHON_SPEC))
    .aliases(&["py", "python3", "py3"])
    .extensions(&["py", "pyw", "pyi", "pyx", "pxd", "gyp", "bzl", "star"])
    .filenames(&["SConstruct", "SConscript", "BUILD", "WORKSPACE", "Tiltfile"]);

static MOJO_SPEC: Spec = Spec {
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "case", "continue", "del", "elif", "else", "except",
        "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "match", "nonlocal", "not", "or",
        "pass", "raise", "return", "try", "while", "with", "yield", "var", "let", "alias", "inout", "owned",
        "borrowed", "mut", "out", "ref", "read", "raises", "capturing", "comptime",
    ],
    types: &[
        "Int", "UInt", "Int8", "Int16", "Int32", "Int64", "UInt8", "UInt16", "UInt32", "UInt64", "Float16", "Float32",
        "Float64", "Bool", "String", "StringLiteral", "SIMD", "DType", "List", "Dict", "Optional", "object", "int",
        "float", "str", "bool", "list", "dict", "tuple", "Self",
    ],
    function_keywords: &["def", "fn"],
    type_keywords: &["class", "struct", "trait"],
    ..PYTHON_SPEC
};

pub static MOJO: Definition =
    Definition::new("mojo", "Mojo", Engine::Generic(&MOJO_SPEC)).extensions(&["mojo", "\u{1F525}"]);

// Ruby and Crystal

const RUBY_STRING: Quote = Quote::new(Kind::String).interp(b"#{");

/// `%w[]` style literals at the cursor.
fn ruby_percent(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let letter = ctx.at(pos + 1);
    let mut open_at = pos + 1;
    if letter.is_ascii_alphabetic() {
        if !matches!(letter, b'w' | b'W' | b'i' | b'I' | b'q' | b'Q' | b'r' | b's' | b'x') {
            return false;
        }
        if pos > 0 && (is_ident(ctx.src[pos - 1]) || matches!(ctx.src[pos - 1], b')' | b']')) {
            return false;
        }
        open_at += 1;
    } else if !lx.expression_start() {
        return false;
    }
    if !matches!(ctx.at(open_at), b'(' | b'[' | b'{' | b'<' | b'|' | b'!' | b'/' | b'^') {
        return false;
    }
    let kind = match letter {
        b'r' => Kind::Regex,
        b'i' | b'I' | b's' => Kind::Constant,
        _ => Kind::String,
    };
    let q = match letter {
        b'w' | b'i' | b'q' | b's' => Quote::new(kind),
        _ => Quote::new(kind).interp(b"#{"),
    };
    let end = quote(lx, pos, open_at, q);
    if letter == b'r' {
        flags(lx, end, Kind::Regex);
    }
    true
}

/// Symbols, hash keys, `%` literals, heredocs, `=begin` comments, Crystal
/// annotations, `def self.name` and regex literals.
fn ruby_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b':' => {
            if ctx.at(pos + 1) == b'"' && (pos == 0 || !is_ident(ctx.src[pos - 1])) {
                quote(lx, pos, pos + 1, RUBY_STRING.with_kind(Kind::Constant));
                return true;
            }
            key_colon(lx, &[])
        }
        b'%' => ruby_percent(lx),
        b'<' if ctx.at(pos + 1) == b'<' => heredoc(lx, b"~-", RUBY_STRING),
        b'=' if ctx.starts_with(b"=begin") && ctx.at_line_start(pos) => pod_block(lx, b"=end"),
        b'_' if ctx.starts_with(b"__END__")
            && matches!(ctx.at(pos + 7), b'\n' | b'\r' | 0)
            && ctx.at_line_start(pos) =>
        {
            let end = ctx.len();
            emit_to(lx, end, Kind::Comment)
        }
        b'@' if ctx.at(pos + 1) == b'[' => {
            lx.ctx.token(2, Kind::Meta);
            lx.nested(b']');
            if lx.ctx.peek() == b']' {
                lx.ctx.token(1, Kind::Meta);
            }
            true
        }
        b'.' if pos >= 4 && &ctx.src[pos - 4..pos] == b"self" && is_ident_start(ctx.at(pos + 1)) => {
            let before = skip_back_spaces(ctx, pos - 4);
            if before < 3 || &ctx.src[before - 3..before] != b"def" || (before > 3 && is_ident(ctx.src[before - 4])) {
                return false;
            }
            let end = lx.scan_ident(pos + 1);
            lx.ctx.token(1, Kind::Punctuation);
            emit_to(lx, end, Kind::Function)
        }
        b'/' if lx.expression_start() => slash_regex(lx, false, b"#{"),
        _ => false,
    }
}

const RUBY_KEYWORDS: &[&str] = &[
    "alias", "and", "begin", "break", "case", "defined?", "do", "else", "elsif", "end", "ensure", "for", "if", "in",
    "next", "not", "or", "redo", "rescue", "retry", "return", "super", "then", "undef", "unless", "until", "when",
    "while", "yield", "BEGIN", "END",
];

const RUBY_SIGILS: &[(u8, Kind)] = &[(b'@', Kind::Variable), (b'$', Kind::Variable), (b':', Kind::Constant)];

const RUBY_STRINGS: &[Str] = &[
    Str::quoted("\"").multiline().interp(Interp::HashBrace),
    Str::quoted("'").multiline(),
    Str::quoted("`").multiline().interp(Interp::HashBrace),
];

static RUBY_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: RUBY_STRINGS,
    keywords: RUBY_KEYWORDS,
    variables: &["self", "__method__"],
    builtins: &[
        "puts", "print", "p", "pp", "require", "require_relative", "include", "extend", "prepend", "attr_reader",
        "attr_writer", "attr_accessor", "private", "protected", "public", "module_function", "raise", "lambda", "proc",
        "loop", "format", "sprintf", "gets", "catch", "throw", "block_given?", "define_method",
    ],
    constants: &["true", "false", "nil", "__FILE__", "__LINE__", "__dir__"],
    function_keywords: &["def"],
    type_keywords: &["class", "module"],
    ident_suffix: "?!",
    sigils: RUBY_SIGILS,
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS,
    hook: Some(ruby_hook),
    hook_chars: ":%<=_@./",
    ..Spec::DEFAULT
};

pub static RUBY: Definition = Definition::new("ruby", "Ruby", Engine::Generic(&RUBY_SPEC))
    .aliases(&["rb", "jruby"])
    .extensions(&["rb", "rbw", "rake", "gemspec", "ru", "podspec", "thor"])
    .filenames(&[
        "Gemfile", "Rakefile", "Guardfile", "Podfile", "Vagrantfile", "Brewfile", "Fastfile", "Capfile", "Berksfile",
        ".irbrc", ".pryrc",
    ]);

static CRYSTAL_SPEC: Spec = Spec {
    keywords: &[
        "alias", "and", "as", "asm", "begin", "break", "case", "do", "else", "elsif", "end", "ensure", "extend", "for",
        "forall", "if", "in", "include", "next", "not", "of", "or", "out", "pointerof", "private", "protected", "redo",
        "require", "rescue", "retry", "return", "select", "sizeof", "super", "then", "typeof", "uninitialized",
        "union", "unless", "until", "when", "while", "with", "yield", "abstract", "previous_def",
    ],
    types: &["Int32", "Int64", "UInt8", "Float64", "String", "Char", "Bool", "Nil", "Symbol", "Array", "Hash"],
    builtins: &[
        "puts", "print", "p", "pp", "raise", "property", "getter", "setter", "spawn", "sleep", "loop", "record",
        "gets", "sprintf",
    ],
    constants: &["true", "false", "nil", "__FILE__", "__LINE__", "__DIR__"],
    function_keywords: &["def", "fun", "macro"],
    type_keywords: &["class", "module", "struct", "enum", "lib", "annotation", "type"],
    strings: &[
        Str::quoted("\"").multiline().interp(Interp::HashBrace),
        Str::quoted("'"),
        Str::quoted("`").multiline().interp(Interp::HashBrace),
    ],
    ..RUBY_SPEC
};

pub static CRYSTAL: Definition =
    Definition::new("crystal", "Crystal", Engine::Generic(&CRYSTAL_SPEC)).aliases(&["cr"]).extensions(&["cr"]);

// Perl

const PERL_STRING: Quote = Quote { subscripts: true, ..Quote::new(Kind::String).vars(b"$@") };

/// `$scalar`, `$#array`, `${name}`, `$1` and punctuation variables.
fn perl_scalar(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let len = ctx.len();
    let next = ctx.at(pos + 1);
    let end = match next {
        b'#' => match ctx.at(pos + 2) {
            b'{' | b'$' => pos + 2,
            _ => var_end(ctx, pos + 2, len).max(pos + 2),
        },
        b'$' => var_end(ctx, pos + 2, len).max(pos + 2),
        b'{' => match var_end(ctx, pos + 1, len) {
            e if e > pos + 1 => e,
            _ => pos + 1,
        },
        b'^' if ctx.at(pos + 2).is_ascii_uppercase() => pos + 3,
        b'0'..=b'9' => {
            let mut e = pos + 1;
            while ctx.at(e).is_ascii_digit() {
                e += 1;
            }
            e
        }
        b'&' | b'`' | b'\'' | b'+' | b'!' | b'@' | b'/' | b'\\' | b',' | b';' | b'.' | b'<' | b'>' => pos + 2,
        _ => match var_end(ctx, pos + 1, len) {
            e if e > pos + 1 => e,
            _ => return false,
        },
    };
    emit_to(lx, end, Kind::Variable);
    lx.mark_value();
    true
}

/// `m//`, `s///`, `tr///`, `y///`, `q()`, `qq{}`, `qw[]`, `qr//` and `qx``.
fn perl_quote_op(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let mut end = pos;
    while ctx.at(end).is_ascii_lowercase() && end - pos < 3 {
        end += 1;
    }
    let word = &ctx.src[pos..end];
    let (kind, parts, q) = match word {
        b"m" | b"qr" => (Kind::Regex, 1, Quote::new(Kind::Regex)),
        b"s" | b"tr" | b"y" => (Kind::Regex, 2, Quote::new(Kind::Regex)),
        b"q" | b"qw" => (Kind::String, 1, Quote::new(Kind::String)),
        b"qq" | b"qx" => (Kind::String, 1, PERL_STRING),
        _ => return false,
    };
    let delim = ctx.at(end);
    if is_ident(delim)
        || delim.is_ascii_whitespace()
        || matches!(delim, 0 | b',' | b';' | b')' | b'}' | b']' | b'>' | b'=' | b'-' | b'.' | b':' | b'$')
    {
        return false;
    }
    if pos > 0 {
        let prev = ctx.src[pos - 1];
        if is_ident(prev)
            || matches!(prev, b'$' | b'@' | b'%' | b'&')
            || (prev == b'>' && pos >= 2 && ctx.src[pos - 2] == b'-')
        {
            return false;
        }
    }
    let mut stop = quote(lx, pos, end, q);
    if parts == 2 && stop > end + 1 && lx.ctx.at(stop - 1) == closing(delim) {
        stop = if closing(delim) == delim {
            quote(lx, stop - 1, stop - 1, q)
        } else {
            let next = lx.ctx.skip_spaces_from(stop);
            match lx.ctx.at(next) {
                b'{' | b'(' | b'[' | b'<' | b'/' | b'|' | b'#' | b'!' => quote(lx, next, next, q),
                _ => stop,
            }
        };
    }
    if kind == Kind::Regex {
        flags(lx, stop, Kind::Regex);
    }
    true
}

/// Sigils, quote like operators, heredocs, POD, fat comma keys and hash subscripts.
fn perl_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let len = ctx.len();
    match ctx.peek() {
        b'$' => perl_scalar(lx),
        b'@' | b'%' => {
            if ctx.peek() == b'%' && !sigil_position(lx) {
                return false;
            }
            let end = match ctx.at(pos + 1) {
                b'$' | b'{' => pos + 1,
                b'_' | b'+' | b'-' if ctx.peek() == b'@' || ctx.at(pos + 1) != b'_' => pos + 2,
                _ => var_end(ctx, pos + 1, len),
            };
            if end == pos + 1 && !matches!(ctx.at(pos + 1), b'$' | b'{') {
                return false;
            }
            emit_to(lx, end, Kind::Variable);
            lx.mark_value();
            true
        }
        b'&' if lx.expression_start() && is_ident_start(ctx.at(pos + 1)) && ctx.at(pos + 1) != b'&' => {
            let end = var_end(ctx, pos + 1, len);
            emit_to(lx, end, Kind::Function);
            lx.mark_value();
            true
        }
        b'"' => {
            quote(lx, pos, pos, PERL_STRING);
            true
        }
        b'/' => {
            // A regex in expression position, or after `split` and `grep`.
            let mut e = skip_back_spaces(ctx, pos);
            let word_end = e;
            while e > 0 && ctx.src[e - 1].is_ascii_lowercase() {
                e -= 1;
            }
            if !lx.expression_start() && !matches!(&ctx.src[e..word_end], b"split" | b"grep") {
                return false;
            }
            slash_regex(lx, false, b"")
        }
        b'<' if ctx.at(pos + 1) == b'<' => heredoc(lx, b"~", PERL_STRING),
        b'=' if ctx.at(pos + 1) == b'>' => {
            retag_prev_word(lx, pos, Kind::Property);
            false
        }
        b'=' if ctx.at(pos + 1).is_ascii_alphabetic() && ctx.at_line_start(pos) => pod_block(lx, b"=cut"),
        b'_' if (ctx.starts_with(b"__END__") || ctx.starts_with(b"__DATA__")) && ctx.at_line_start(pos) => {
            emit_to(lx, len, Kind::Comment)
        }
        b'{' => perl_subscript(lx),
        b'm' | b'q' | b's' | b't' | b'y' => perl_quote_op(lx),
        _ => false,
    }
}

/// A bareword hash subscript such as `$h{key}` or `$r->{key}`.
fn perl_subscript(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    if pos == 0 {
        return false;
    }
    let prev = ctx.src[pos - 1];
    let subscript = match prev {
        b'}' | b']' => true,
        b'>' => pos >= 2 && ctx.src[pos - 2] == b'-',
        _ if is_ident(prev) => {
            let mut s = pos - 1;
            while s > 0 && is_ident(ctx.src[s - 1]) {
                s -= 1;
            }
            s > 0 && ctx.src[s - 1] == b'$'
        }
        _ => false,
    };
    let mut end = pos + 1;
    if ctx.at(end) == b'-' {
        end += 1;
    }
    let word_start = end;
    while is_ident(ctx.at(end)) {
        end += 1;
    }
    if !subscript || end == word_start || !is_ident_start(ctx.at(word_start)) || ctx.at(end) != b'}' {
        return false;
    }
    let ctx = &mut *lx.ctx;
    ctx.token(1, Kind::Punctuation);
    ctx.token(end - pos - 1, Kind::Property);
    ctx.token(1, Kind::Punctuation);
    lx.mark_value();
    true
}

static PERL_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("'").multiline(), Str::quoted("`").multiline()],
    keywords: &[
        "my", "our", "local", "state", "if", "elsif", "else", "unless", "while", "until", "for", "foreach", "do",
        "last", "next", "redo", "return", "use", "no", "require", "and", "or", "not", "xor", "eq", "ne", "lt", "gt",
        "le", "ge", "cmp", "BEGIN", "END", "given", "when", "default", "goto",
    ],
    builtins: &[
        "print", "printf", "say", "die", "warn", "open", "close", "push", "pop", "shift", "unshift", "splice", "keys",
        "values", "each", "exists", "delete", "defined", "ref", "bless", "scalar", "join", "split", "map", "grep",
        "sort", "reverse", "length", "substr", "index", "sprintf", "lc", "uc", "chomp", "chop", "wantarray", "eval",
        "exit", "binmode", "local", "sprintf", "time", "localtime",
    ],
    constants: &["undef", "__FILE__", "__LINE__", "__PACKAGE__", "__SUB__"],
    function_keywords: &["sub"],
    type_keywords: &["package"],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS,
    hook: Some(perl_hook),
    hook_chars: "$@%&\"/<=_{mqsty",
    ..Spec::DEFAULT
};

pub static PERL: Definition = Definition::new("perl", "Perl", Engine::Generic(&PERL_SPEC))
    .aliases(&["pl", "perl5"])
    .extensions(&["pl", "pm", "t", "psgi"])
    .filenames(&["cpanfile", ".perltidyrc"]);

// Raku

const RAKU_STRING: Quote = Quote::new(Kind::String).vars(b"$@").interp(b"{");

/// End of a Raku identifier: `-` and `'` may join words (`is-done`, `isn't`).
fn raku_ident_end(ctx: &Ctx, from: usize) -> usize {
    let mut end = from;
    loop {
        while is_ident(ctx.at(end)) {
            end += 1;
        }
        if matches!(ctx.at(end), b'-' | b'\'') && ctx.at(end + 1).is_ascii_alphabetic() && end > from {
            end += 1;
        } else {
            return end;
        }
    }
}

/// `q:to/END/`, `qq:to/END/` and `Q:heredoc<END>` with the word `q` at
/// `pos..end`. The terminator may be indented.
fn raku_heredoc(lx: &mut Lexer, pos: usize, end: usize) -> bool {
    let ctx = &*lx.ctx;
    let body = match &ctx.src[pos..end] {
        b"qq" => RAKU_STRING,
        b"q" | b"Q" => Quote::new(Kind::String).raw(),
        _ => return false,
    };
    let adverb = if ctx.starts_with_at(end, b":to") {
        3
    } else if ctx.starts_with_at(end, b":heredoc") {
        8
    } else {
        return false;
    };
    let open = ctx.at(end + adverb);
    if !matches!(open, b'/' | b'<' | b'{' | b'(' | b'[' | b'\'' | b'"') {
        return false;
    }
    let word_start = end + adverb + 1;
    let mut word_end = word_start;
    while is_ident(ctx.at(word_end)) || ctx.at(word_end) == b'-' {
        word_end += 1;
    }
    if word_end == word_start || ctx.at(word_end) != closing(open) {
        return false;
    }
    lx.ctx.emit(pos, word_end + 1, Kind::String);
    heredoc_body(lx, word_end + 1, word_start, word_end, true, body);
    true
}

/// `<a b c>` word lists in expression position and `%h<key>` subscripts.
fn raku_angle(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let prev = if pos > 0 { ctx.src[pos - 1] } else { 0 };
    let subscript = is_ident(prev) || matches!(prev, b')' | b']' | b'}' | b'>');
    let next = ctx.at(pos + 1);
    if !(subscript || lx.expression_start())
        || matches!(next, 0 | b'=' | b'<' | b'>' | b'-' | b'+' | b'~' | b'\n' | b'\r')
        || (subscript && next.is_ascii_whitespace())
    {
        return false;
    }
    let end = ctx.line_end_within(pos + 1, 256);
    let close = match ctx.src[pos + 1..end]
        .iter()
        .position(|&b| matches!(b, b'>' | b'<' | b';' | b'{' | b'}' | b'(' | b')' | b'='))
    {
        Some(i) if ctx.src[pos + 1 + i] == b'>' => pos + 1 + i,
        _ => return false,
    };
    if subscript {
        let ctx = &mut *lx.ctx;
        ctx.token(1, Kind::Punctuation);
        ctx.emit(pos + 1, close, Kind::Property);
        ctx.pos = close;
        ctx.token(1, Kind::Punctuation);
    } else {
        emit_to(lx, close + 1, Kind::String);
    }
    lx.mark_value();
    true
}

fn raku_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let b = ctx.peek();
    match b {
        b'#' if ctx.at(pos + 1) == b'`' && matches!(ctx.at(pos + 2), b'(' | b'[' | b'{' | b'<') => {
            quote(lx, pos, pos + 2, Quote::new(Kind::Comment).escape(0));
            true
        }
        b'=' if ctx.at(pos + 1).is_ascii_alphabetic() && ctx.at_line_start(pos) => {
            if ctx.starts_with(b"=begin") {
                return pod_block(lx, b"=end");
            }
            if ctx.starts_with(b"=finish") {
                return emit_to(lx, ctx.len(), Kind::Comment);
            }
            let end = ctx.find(pos, b"\n\n").unwrap_or(ctx.len());
            emit_to(lx, end, Kind::Comment)
        }
        b'$' | b'@' | b'%' | b'&' => {
            if matches!(b, b'%' | b'&') && !sigil_position(lx) {
                return false;
            }
            let mut i = pos + 1;
            if matches!(ctx.at(i), b'.' | b'!' | b'*' | b'?' | b'^' | b':' | b'=' | b'~')
                && is_ident_start(ctx.at(i + 1))
            {
                i += 1;
            }
            let end = if is_ident_start(ctx.at(i)) {
                raku_ident_end(ctx, i)
            } else if b == b'$' && matches!(ctx.at(i), b'/' | b'!' | b'0'..=b'9') {
                i + 1
            } else {
                return false;
            };
            emit_to(lx, end, if b == b'&' { Kind::Function } else { Kind::Variable });
            lx.mark_value();
            true
        }
        b'"' => {
            quote(lx, pos, pos, RAKU_STRING);
            true
        }
        b'=' if ctx.at(pos + 1) == b'>' => {
            retag_prev_word(lx, pos, Kind::Property);
            false
        }
        b'/' if lx.expression_start() && ctx.at(pos + 1) != b'/' => {
            quote(lx, pos, pos, Quote::new(Kind::Regex).single_line());
            true
        }
        b'<' => raku_angle(lx),
        b'\'' if pos > 0 && is_ident(ctx.src[pos - 1]) && ctx.at(pos + 1).is_ascii_alphabetic() => {
            // `isn't`: an apostrophe between letters joins the word.
            let Some(ws) = word_before(lx, pos) else {
                return false;
            };
            let end = raku_ident_end(ctx, ws);
            lx.ctx.truncate_to(ws);
            lx.ctx.pos = end;
            let kind = lx.classify(ws, end);
            lx.ctx.emit(ws, end, kind);
            true
        }
        _ if is_ident_start(b) => {
            let end = raku_ident_end(ctx, pos);
            let delim = ctx.at(end);
            if delim == b':' && raku_heredoc(lx, pos, end) {
                return true;
            }
            let ctx = &*lx.ctx;
            let quoted = !is_ident(delim)
                && !delim.is_ascii_whitespace()
                && !matches!(delim, 0 | b'(' | b',' | b';' | b')' | b'}' | b']' | b'>' | b'=' | b'.' | b':' | b'-');
            let q = match &ctx.src[pos..end] {
                b"m" | b"rx" | b"s" | b"tr" if quoted => Quote::new(Kind::Regex),
                b"q" | b"Q" | b"qw" if quoted => Quote::new(Kind::String),
                b"qq" | b"qqx" | b"qx" if quoted => RAKU_STRING,
                _ => {
                    lx.ctx.pos = end;
                    let kind = lx.classify(pos, end);
                    lx.ctx.emit(pos, end, kind);
                    return true;
                }
            };
            let stop = quote(lx, pos, end, q);
            if &lx.ctx.src[pos..end] == b"s" && closing(delim) == delim && stop > end + 1 {
                quote(lx, stop - 1, stop - 1, q);
            }
            true
        }
        _ => false,
    }
}

static RAKU_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("'").multiline(), Str::new("\u{300C}", "\u{300D}").raw().multiline()],
    keywords: &[
        "my", "our", "has", "state", "constant", "is", "does", "but", "if", "elsif", "else", "unless", "with",
        "orwith", "without", "for", "loop", "while", "until", "repeat", "given", "when", "default", "return", "take",
        "gather", "do", "once", "die", "try", "CATCH", "CONTROL", "BEGIN", "END", "use", "need", "import", "unit",
        "last", "next", "redo", "fail", "and", "or", "not", "so", "xor", "where", "lazy", "start", "await", "react",
        "whenever", "supply", "emit",
    ],
    builtins: &[
        "say", "print", "put", "note", "dd", "elems", "chars", "keys", "values", "push", "pop", "map", "grep", "sort",
        "join", "split", "prompt", "slurp", "spurt", "lines", "words", "exit",
    ],
    constants: &["True", "False", "Nil", "Inf", "NaN", "pi", "tau"],
    variables: &["self"],
    function_keywords: &["sub", "method", "submethod", "multi", "proto", "token", "rule", "regex"],
    type_keywords: &["class", "role", "grammar", "module", "package", "subset", "enum"],
    sigils: &[(b':', Kind::Constant)],
    features: CALLS | CAPITALIZED_TYPES | MEMBERS | KEBAB,
    hook: Some(raku_hook),
    // Letters that start quote operators (`m//`, `rx//`, `s///`, `tr///`, `q//`, `Q//`).
    hook_chars: "#=$@%&\"/<'mrstqQ",
    ..Spec::DEFAULT
};

pub static RAKU: Definition = Definition::new("raku", "Raku", Engine::Generic(&RAKU_SPEC))
    .aliases(&["perl6"])
    .extensions(&["raku", "rakumod", "rakutest", "rakudoc", "p6", "pl6", "pm6"]);

// Lua and MoonScript

/// A Lua long bracket at `at`: `[[ ... ]]` or `[==[ ... ]==]`. Returns its end.
fn long_bracket(ctx: &Ctx, at: usize) -> Option<usize> {
    if ctx.at(at) != b'[' {
        return None;
    }
    let mut i = at + 1;
    let mut level = 0;
    while ctx.at(i) == b'=' {
        level += 1;
        i += 1;
    }
    if ctx.at(i) != b'[' {
        return None;
    }
    let mut from = i + 1;
    while let Some(p) = ctx.find(from, b"]") {
        let mut k = p + 1;
        let mut n = 0;
        while n < level && ctx.at(k) == b'=' {
            n += 1;
            k += 1;
        }
        if n == level && ctx.at(k) == b']' {
            return Some(k + 1);
        }
        from = p + 1;
    }
    Some(ctx.len())
}

/// In `function M.name` the word after `function` is a table, not the function.
fn unmark_function_owner(lx: &mut Lexer, pos: usize) {
    let Some(ws) = word_before(lx, pos) else {
        return;
    };
    let ctx = &*lx.ctx;
    let before = skip_back_spaces(ctx, ws);
    if before < ws
        && before >= 8
        && &ctx.src[before - 8..before] == b"function"
        && !is_ident(ctx.at(before.wrapping_sub(9)))
    {
        lx.ctx.truncate_to(ws);
        lx.ctx.emit(ws, pos, Kind::Text);
    }
}

/// Long strings and comments, `obj:method()` calls and `function M.name`.
fn lua_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'-' if ctx.starts_with(b"--[") => match long_bracket(ctx, pos + 2) {
            Some(end) => emit_to(lx, end, Kind::Comment),
            None => false,
        },
        b'[' => match long_bracket(ctx, pos) {
            Some(end) => {
                emit_to(lx, end, Kind::String);
                lx.mark_value();
                true
            }
            None => false,
        },
        b'.' if ctx.at(pos + 1) == b'.' => lua_concat(lx),
        b'.' => {
            unmark_function_owner(lx, pos);
            false
        }
        b':' if is_ident_start(ctx.at(pos + 1)) && pos > 0 && ctx.src[pos - 1] != b':' => {
            let end = lx.scan_ident(pos + 1);
            if !matches!(ctx.next_non_space(end), b'(' | b'"' | b'\'' | b'{') {
                return false;
            }
            unmark_function_owner(lx, pos);
            lx.ctx.token(1, Kind::Punctuation);
            emit_to(lx, end, Kind::Function);
            lx.mark_value();
            true
        }
        _ => false,
    }
}

const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "for", "goto", "if", "in", "local", "not", "or", "repeat", "return",
    "then", "until", "while",
];

const LUA_BUILTINS: &[&str] = &[
    "print", "pairs", "ipairs", "type", "tostring", "tonumber", "require", "error", "assert", "pcall", "xpcall",
    "select", "setmetatable", "getmetatable", "rawget", "rawset", "rawequal", "next", "unpack", "table", "string",
    "math", "io", "os", "coroutine", "utf8", "debug", "_G", "_ENV", "_VERSION",
];

static LUA_SPEC: Spec = Spec {
    line_comments: &["--"],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: LUA_KEYWORDS,
    builtins: LUA_BUILTINS,
    constants: &["nil", "true", "false"],
    variables: &["self"],
    function_keywords: &["function"],
    operators: "+-*/%=&|^<>~#",
    features: CALLS | UPPER_CONSTANTS | MEMBERS,
    hook: Some(lua_hook),
    hook_chars: "-[.:",
    ..Spec::DEFAULT
};

pub static LUA: Definition = Definition::new("lua", "Lua", Engine::Generic(&LUA_SPEC))
    .extensions(&["lua", "rockspec", "wlua"])
    .filenames(&[".luacheckrc"]);

const ARROWS: &[&[u8]] = &[b"->", b"=>"];

/// Long strings, `obj\method` calls and arrow function names.
fn moon_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'[' => match long_bracket(ctx, pos) {
            Some(end) => {
                emit_to(lx, end, Kind::String);
                lx.mark_value();
                true
            }
            None => false,
        },
        b'\\' if is_ident_start(ctx.at(pos + 1)) => {
            let end = lx.scan_ident(pos + 1);
            lx.ctx.token(1, Kind::Punctuation);
            emit_to(lx, end, Kind::Function);
            lx.mark_value();
            true
        }
        b'/' if lx.expression_start() => slash_regex(lx, true, b"#{"),
        b'=' | b':' => arrow_assignment(lx, ARROWS),
        b'.' => lua_concat(lx),
        _ => false,
    }
}

static MOONSCRIPT_SPEC: Spec = Spec {
    line_comments: &["--"],
    strings: &[Str::quoted("\"").multiline().interp(Interp::HashBrace), Str::quoted("'").multiline()],
    keywords: &[
        "and", "break", "class", "continue", "do", "else", "elseif", "export", "extends", "for", "from", "if",
        "import", "in", "local", "not", "or", "return", "switch", "then", "unless", "using", "when", "while", "with",
    ],
    builtins: LUA_BUILTINS,
    constants: &["nil", "true", "false"],
    variables: &["self", "super"],
    // `{ :name }` is shorthand for `{ name: name }`.
    sigils: &[(b'@', Kind::Variable), (b':', Kind::Property)],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | IDENT_KEYS,
    hook: Some(moon_hook),
    hook_chars: "[\\=:.",
    ..Spec::DEFAULT
};

pub static MOONSCRIPT: Definition = Definition::new("moonscript", "MoonScript", Engine::Generic(&MOONSCRIPT_SPEC))
    .aliases(&["moon"])
    .extensions(&["moon"]);

// R

/// `df$col`, `obj@slot`, `%in%` operators, `\(x)` lambdas and `name <- function`.
fn r_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'$' | b'@' => {
            let prev = if pos > 0 { ctx.src[pos - 1] } else { 0 };
            if !(lx.is_ident_byte(prev) || matches!(prev, b')' | b']')) || !is_ident_start(ctx.at(pos + 1)) {
                return false;
            }
            let end = lx.scan_ident(pos + 1);
            lx.ctx.token(1, Kind::Operator);
            emit_to(lx, end, Kind::Property);
            lx.mark_value();
            true
        }
        b'%' => match ctx.src[pos + 1..].iter().take(16).position(|&b| matches!(b, b'%' | b' ' | b'\n')) {
            Some(i) if ctx.src[pos + 1 + i] == b'%' => {
                emit_to(lx, pos + i + 2, Kind::Operator);
                lx.mark_operator();
                true
            }
            _ => false,
        },
        b'\\' if ctx.at(pos + 1) == b'(' => {
            lx.ctx.token(1, Kind::Keyword);
            true
        }
        b'<' | b'=' => {
            let op_end = if ctx.starts_with(b"<<-") {
                pos + 3
            } else if ctx.starts_with(b"<-") {
                pos + 2
            } else if ctx.peek() == b'=' && ctx.at(pos + 1) != b'=' {
                pos + 1
            } else {
                return false;
            };
            let i = ctx.skip_spaces_from(op_end);
            if (ctx.starts_with_at(i, b"function") && !is_ident(ctx.at(i + 8))) || ctx.starts_with_at(i, b"\\(") {
                retag_prev_word(lx, pos, Kind::Function);
            }
            false
        }
        _ => false,
    }
}

static R_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"").multiline(), Str::quoted("'").multiline(), Str::quoted("`").kind(Kind::Text)],
    keywords: &["if", "else", "repeat", "while", "function", "for", "in", "next", "break", "return"],
    builtins: &[
        "c", "list", "library", "require", "print", "paste", "paste0", "cat", "length", "sum", "mean", "max", "min",
        "seq", "seq_len", "seq_along", "rep", "data.frame", "matrix", "vector", "names", "nrow", "ncol", "apply",
        "lapply", "sapply", "vapply", "is.na", "is.null", "stop", "warning", "message", "invisible", "tryCatch",
        "sprintf", "format", "round", "unlist", "setNames", "stopifnot",
    ],
    constants: &["TRUE", "FALSE", "NULL", "NA", "NaN", "Inf", "NA_integer_", "NA_real_", "NA_character_", "T", "F"],
    ident_chars: ".",
    features: CALLS | UPPER_CONSTANTS,
    hook: Some(r_hook),
    hook_chars: "$@%\\<=",
    ..Spec::DEFAULT
};

pub static R: Definition = Definition::new("r", "R", Engine::Generic(&R_SPEC))
    .aliases(&["rscript", "splus"])
    .extensions(&["r", "rprofile", "rhistory"])
    .filenames(&[".Rprofile", ".Renviron"]);

// Julia

/// Char literals versus the transpose operator, and
/// `r"regex"` and `raw"..."` strings.
fn julia_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let pos = ctx.pos;
    let prev = if pos > 0 { ctx.src[pos - 1] } else { 0 };
    match ctx.peek() {
        b'\'' => {
            if is_ident(prev) || matches!(prev, b')' | b']' | b'}' | b'\'' | b'.') {
                ctx.token(1, Kind::Operator);
                return true;
            }
            let end = if ctx.at(pos + 1) == b'\\' {
                ctx.src[pos + 2..].iter().take(12).position(|&b| b == b'\'').map(|i| pos + 3 + i)
            } else {
                let mut i = pos + 2;
                while ctx.at(i) & 0xC0 == 0x80 {
                    i += 1;
                }
                (ctx.at(i) == b'\'' && ctx.at(pos + 1) != b'\'' && ctx.at(pos + 1) != b'\n').then_some(i + 1)
            };
            match end {
                Some(end) => {
                    emit_to(lx, end, Kind::String);
                    lx.mark_value();
                    true
                }
                None => false,
            }
        }
        b'r' if !is_ident(prev) => {
            if ctx.starts_with(b"r\"") && !ctx.starts_with(b"r\"\"\"") {
                ctx.pos += 1;
                lx.string(pos, &JULIA_REGEX, false, Interp::None);
                flags(lx, lx.ctx.pos, Kind::Regex);
                return true;
            }
            if ctx.starts_with(b"raw\"") {
                let end = ctx.find(pos + 4, b"\"").map_or(ctx.len(), |p| p + 1);
                emit_to(lx, end, Kind::String);
                lx.mark_value();
                return true;
            }
            false
        }
        _ => false,
    }
}

const JULIA_REGEX: Str = Str::quoted("\"").kind(Kind::Regex);

static JULIA_SPEC: Spec = Spec {
    line_comments: &["#"],
    block_comments: &[("#=", "=#")],
    nested_comments: true,
    strings: &[
        Str::quoted("\"\"\"").multiline().interp(Interp::DollarParen),
        Str::quoted("\"").multiline().interp(Interp::DollarParen),
        Str::quoted("`").interp(Interp::DollarParen),
    ],
    string_prefixes: &[Prefix::plain(b'b'), Prefix::raw(b'v')],
    keywords: &[
        "baremodule", "begin", "break", "catch", "const", "continue", "do", "else", "elseif", "end", "export",
        "finally", "for", "global", "if", "import", "in", "isa", "let", "local", "mutable", "abstract", "primitive",
        "quote", "return", "try", "using", "where", "while", "outer", "public",
    ],
    types: &["Int", "Int64", "Float64", "String", "Bool", "Any", "Nothing", "Vector", "Matrix", "Array", "Dict"],
    builtins: &[
        "println", "print", "push!", "pop!", "append!", "length", "size", "zeros", "ones", "collect", "map", "filter",
        "reduce", "sum", "typeof", "eltype", "isempty", "haskey", "get", "error", "throw", "string", "parse", "round",
        "abs", "sqrt", "keys", "values", "enumerate", "zip", "eachindex",
    ],
    constants: &["true", "false", "nothing", "missing", "Inf", "NaN", "pi", "im", "undef"],
    function_keywords: &["function", "macro"],
    type_keywords: &["struct", "type", "module"],
    ident_suffix: "!",
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    hook: Some(julia_hook),
    hook_chars: "'r",
    ..Spec::DEFAULT
};

pub static JULIA: Definition =
    Definition::new("julia", "Julia", Engine::Generic(&JULIA_SPEC)).aliases(&["jl"]).extensions(&["jl"]);

// Elixir

const ELIXIR_STRING: Quote = Quote::new(Kind::String).interp(b"#{");

/// `~r/regex/`, `~s(string)`, `~w[words]a` and `~S"""heredoc"""` sigils.
fn elixir_sigil(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let letter = ctx.at(pos + 1);
    if !letter.is_ascii_alphabetic() {
        return false;
    }
    let mut i = pos + 2;
    if letter.is_ascii_uppercase() {
        while ctx.at(i).is_ascii_uppercase() {
            i += 1;
        }
    }
    let kind = if matches!(letter, b'r' | b'R') { Kind::Regex } else { Kind::String };
    let q = if letter.is_ascii_lowercase() { Quote::new(kind).interp(b"#{") } else { Quote::new(kind).escape(0) };
    let end = if ctx.starts_with_at(i, b"\"\"\"") || ctx.starts_with_at(i, b"'''") {
        let close = ctx.find(i + 3, &ctx.src[i..i + 3]).unwrap_or(ctx.len());
        scan_quoted(lx, pos, i + 3, close, 0, 0, q);
        let end = (close + 3).min(lx.ctx.len());
        lx.ctx.emit(close, end, kind);
        end
    } else if matches!(ctx.at(i), b'/' | b'|' | b'"' | b'\'' | b'(' | b'[' | b'{' | b'<') {
        quote(lx, pos, i, q)
    } else {
        return false;
    };
    flags(lx, end, kind);
    true
}

/// Sigils, atoms, keyword list keys, module aliases, `&1` captures and `?a` characters.
fn elixir_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'~' => elixir_sigil(lx),
        b':' => {
            if ctx.at(pos + 1) == b'"' && (pos == 0 || !is_ident(ctx.src[pos - 1])) {
                quote(lx, pos, pos + 1, ELIXIR_STRING.with_kind(Kind::Constant));
                return true;
            }
            key_colon(lx, &["do", "else", "after", "rescue", "catch"])
        }
        b'.' if ctx.at(pos + 1).is_ascii_uppercase() => {
            let end = lx.scan_ident(pos + 1);
            lx.ctx.token(1, Kind::Punctuation);
            emit_to(lx, end, Kind::Type);
            lx.mark_value();
            true
        }
        b'&' if ctx.at(pos + 1).is_ascii_digit() => {
            emit_to(lx, pos + 2, Kind::Variable);
            lx.mark_value();
            true
        }
        // `?a` and `?\n` character literals.
        b'?' if !matches!(ctx.at(pos + 1), 0 | b' ' | b'\t' | b'\r' | b'\n') => {
            let ctx = &mut *lx.ctx;
            ctx.token(1, Kind::String);
            if ctx.peek() == b'\\' {
                let at = ctx.pos;
                escape_len(ctx);
                ctx.emit_from(at, Kind::Escape);
            } else {
                let at = ctx.pos;
                ctx.bump_char();
                ctx.emit_from(at, Kind::String);
            }
            lx.mark_value();
            true
        }
        _ => false,
    }
}

static ELIXIR_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[
        Str::quoted("\"\"\"").multiline().interp(Interp::HashBrace),
        Str::quoted("'''").multiline().interp(Interp::HashBrace),
        Str::quoted("\"").multiline().interp(Interp::HashBrace),
        Str::quoted("'").interp(Interp::HashBrace),
    ],
    keywords: &[
        "do", "end", "fn", "when", "case", "cond", "if", "unless", "else", "with", "for", "receive", "try", "catch",
        "rescue", "after", "import", "alias", "require", "use", "quote", "unquote", "and", "or", "not", "in",
        "defstruct", "defexception", "defoverridable",
    ],
    builtins: &[
        "raise", "reraise", "throw", "spawn", "send", "self", "is_atom", "is_binary", "is_list", "is_map",
        "is_integer", "is_nil", "is_function", "length", "hd", "tl", "elem", "put_elem", "inspect", "to_string",
        "__MODULE__", "__ENV__", "__CALLER__", "__DIR__",
    ],
    constants: &["true", "false", "nil"],
    function_keywords: &["def", "defp", "defmacro", "defmacrop", "defguard", "defguardp", "defdelegate"],
    type_keywords: &["defmodule", "defprotocol", "defimpl"],
    ident_suffix: "?!",
    sigils: &[(b':', Kind::Constant)],
    features: CALLS | CAPITALIZED_TYPES | MEMBERS | ANNOTATIONS,
    hook: Some(elixir_hook),
    hook_chars: "~:.&?",
    ..Spec::DEFAULT
};

pub static ELIXIR: Definition = Definition::new("elixir", "Elixir", Engine::Generic(&ELIXIR_SPEC))
    .aliases(&["ex", "exs"])
    .extensions(&["ex", "exs"])
    .filenames(&["mix.lock"]);

// Nim

/// `{.pragmas.}`, `1'u8` literal suffixes and `fmt"{x}"` / `&"{x}"` format strings.
fn nim_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let pos = ctx.pos;
    let prev = if pos > 0 { ctx.src[pos - 1] } else { 0 };
    match ctx.peek() {
        b'f' if ctx.starts_with(b"fmt\"") && !is_ident(prev) => {
            ctx.token(3, Kind::Function);
            nim_format(lx, Backslash::Literal);
            true
        }
        b'&' if ctx.at(pos + 1) == b'"' => {
            ctx.token(1, Kind::Operator);
            nim_format(lx, Backslash::Escape);
            true
        }
        b'{' if ctx.at(pos + 1) == b'.' => {
            let end = ctx.find(pos + 2, b".}").map_or(ctx.len(), |p| p + 2);
            emit_to(lx, end, Kind::Meta)
        }
        b'\'' if pos > 0 && ctx.src[pos - 1].is_ascii_digit() && is_ident_start(ctx.at(pos + 1)) => {
            let mut end = pos + 1;
            while is_ident(ctx.at(end)) {
                end += 1;
            }
            emit_to(lx, end, Kind::Number)
        }
        _ => false,
    }
}

/// A format string whose opening quote is at the cursor. Triple quoted ones are raw.
fn nim_format(lx: &mut Lexer, backslash: Backslash) {
    let pos = lx.ctx.pos;
    let (delim, backslash): (&'static [u8], _) =
        if lx.ctx.starts_with(b"\"\"\"") { (b"\"\"\"", Backslash::Literal) } else { (b"\"", backslash) };
    format_string(
        lx,
        pos,
        pos + delim.len(),
        Fmt { delim, backslash, multiline: delim.len() == 3, nested_quotes: false },
    );
}

static NIM_SPEC: Spec = Spec {
    line_comments: &["#"],
    block_comments: &[("##[", "]##"), ("#[", "]#")],
    nested_comments: true,
    strings: &[Str::quoted("\"\"\"").raw().multiline(), Str::quoted("\""), Str::quoted("'")],
    string_prefixes: &[Prefix::raw(b'r'), Prefix::raw(b'R')],
    keywords: &[
        "addr", "and", "as", "asm", "bind", "block", "break", "case", "cast", "concept", "const", "continue",
        "converter", "defer", "discard", "distinct", "div", "do", "elif", "else", "end", "enum", "except", "export",
        "finally", "for", "from", "if", "import", "in", "include", "interface", "is", "isnot", "let", "mixin", "mod",
        "not", "notin", "object", "of", "or", "out", "ptr", "raise", "ref", "return", "shl", "shr", "static", "try",
        "tuple", "using", "var", "when", "while", "xor", "yield",
    ],
    types: &[
        "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32", "uint64", "float", "float32",
        "float64", "bool", "char", "string", "cstring", "pointer", "seq", "array", "set", "openArray", "varargs",
        "void", "auto", "untyped", "typed", "typedesc", "range",
    ],
    builtins: &[
        "echo", "len", "high", "low", "inc", "dec", "ord", "chr", "new", "newSeq", "assert", "doAssert", "quit",
        "sizeof", "repr",
    ],
    constants: &["true", "false", "nil"],
    variables: &["result"],
    function_keywords: &["proc", "func", "method", "iterator", "macro", "template"],
    type_keywords: &["type"],
    features: CALLS | CAPITALIZED_TYPES | MEMBERS,
    hook: Some(nim_hook),
    hook_chars: "{'f&",
    ..Spec::DEFAULT
};

pub static NIM: Definition = Definition::new("nim", "Nim", Engine::Generic(&NIM_SPEC))
    .aliases(&["nimrod"])
    .extensions(&["nim", "nims", "nimble"]);

// CoffeeScript and LiveScript

/// `###` block comments, `///` heregexes and arrow function names.
fn coffee_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'#' if ctx.starts_with(b"###") && ctx.at(pos + 3) != b'#' => {
            let end = ctx.find(pos + 3, b"###").map_or(ctx.len(), |p| p + 3);
            emit_to(lx, end, Kind::Comment)
        }
        b'/' if ctx.starts_with(b"///") && lx.expression_start() => {
            let end = ctx.find(pos + 3, b"///").map_or(ctx.len(), |p| p + 3);
            emit_to(lx, end, Kind::Regex);
            flags(lx, end, Kind::Regex);
            lx.mark_value();
            true
        }
        b'/' if lx.expression_start() => slash_regex(lx, true, b"#{"),
        b'=' | b':' => arrow_assignment(lx, ARROWS),
        _ => false,
    }
}

const COFFEE_STRINGS: &[Str] = &[
    Str::quoted("\"\"\"").multiline().interp(Interp::HashBrace),
    Str::quoted("'''").multiline(),
    Str::quoted("\"").multiline().interp(Interp::HashBrace),
    Str::quoted("'").multiline(),
    Str::quoted("```").raw().multiline(),
    Str::quoted("`").raw().multiline(),
];

const COFFEE_CONSTANTS: &[&str] = &["true", "false", "null", "undefined", "yes", "no", "on", "off", "NaN", "Infinity"];

const COFFEE_BUILTINS: &[&str] =
    &["console", "window", "document", "require", "module", "exports", "process", "Math", "JSON"];

const COFFEE_FEATURES: u32 = CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | IDENT_KEYS;

static COFFEESCRIPT_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: COFFEE_STRINGS,
    keywords: &[
        "if", "else", "unless", "then", "for", "in", "of", "own", "by", "when", "while", "until", "loop", "switch",
        "return", "break", "continue", "new", "try", "catch", "finally", "throw", "and", "or", "not", "is", "isnt",
        "instanceof", "typeof", "delete", "do", "extends", "super", "import", "export", "from", "default", "as",
        "await", "yield",
    ],
    builtins: COFFEE_BUILTINS,
    constants: COFFEE_CONSTANTS,
    variables: &["this"],
    type_keywords: &["class"],
    sigils: &[(b'@', Kind::Variable)],
    features: COFFEE_FEATURES,
    hook: Some(coffee_hook),
    hook_chars: "#/=:",
    ..Spec::DEFAULT
};

pub static COFFEESCRIPT: Definition =
    Definition::new("coffeescript", "CoffeeScript", Engine::Generic(&COFFEESCRIPT_SPEC))
        .aliases(&["coffee", "coffee-script"])
        .extensions(&["coffee", "cson", "iced"])
        .filenames(&["Cakefile"]);

const LIVESCRIPT_ARROWS: &[&[u8]] = &[b"->", b"~>", b"-->", b"~~>", b"!->", b"!~>", b"=>"];

/// `\word` strings and arrow function names.
fn livescript_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'\\' if is_ident_start(ctx.at(pos + 1)) => {
            let end = lx.scan_ident(pos + 1);
            emit_to(lx, end, Kind::String);
            lx.mark_value();
            true
        }
        b'/' if lx.expression_start() => slash_regex(lx, true, b"#{"),
        b'=' | b':' => arrow_assignment(lx, LIVESCRIPT_ARROWS),
        _ => false,
    }
}

static LIVESCRIPT_SPEC: Spec = Spec {
    line_comments: &["#"],
    block_comments: &[("/*", "*/")],
    strings: COFFEE_STRINGS,
    keywords: &[
        "if", "else", "unless", "then", "for", "in", "of", "own", "by", "from", "to", "til", "when", "while", "until",
        "loop", "switch", "case", "default", "fallthrough", "return", "break", "continue", "new", "try", "catch",
        "finally", "throw", "and", "or", "not", "is", "isnt", "instanceof", "typeof", "delete", "do", "extends",
        "implements", "super", "import", "export", "require", "let", "const", "var", "function", "it", "that", "match",
        "otherwise", "with", "async", "await",
    ],
    builtins: COFFEE_BUILTINS,
    constants: COFFEE_CONSTANTS,
    variables: &["this"],
    type_keywords: &["class"],
    sigils: &[(b'@', Kind::Variable)],
    features: COFFEE_FEATURES | KEBAB,
    hook: Some(livescript_hook),
    hook_chars: "\\=:/",
    ..Spec::DEFAULT
};

pub static LIVESCRIPT: Definition = Definition::new("livescript", "LiveScript", Engine::Generic(&LIVESCRIPT_SPEC))
    .aliases(&["live-script", "ls"])
    .extensions(&["ls"])
    .filenames(&["Slakefile"]);

// Tcl

/// Whether `pos` starts a Tcl command: only spaces since a newline, `;`, `{` or `[`.
fn tcl_command_start(ctx: &Ctx, pos: usize) -> bool {
    let i = skip_back_spaces(ctx, pos);
    i == 0 || matches!(ctx.src[i - 1], b'\n' | b';' | b'{' | b'[')
}

/// Comments only at command start, `[command]` substitution, `$vars` and `proc` names.
fn tcl_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'#' => {
            if tcl_command_start(ctx, pos) {
                return false;
            }
            let mut end = pos + 1;
            while is_ident(ctx.at(end)) {
                end += 1;
            }
            lx.ctx.pos = end;
            true
        }
        b'[' => {
            lx.ctx.token(1, Kind::Punctuation);
            lx.mark_operator();
            let start = lx.ctx.skip_spaces_from(pos + 1);
            if is_ident_start(lx.ctx.at(start)) || lx.ctx.at(start) == b':' {
                let end = lx.scan_ident(start);
                lx.ctx.pos = end;
                let kind = match lx.classify(start, end) {
                    Kind::Text => Kind::Function,
                    k => k,
                };
                lx.ctx.emit(start, end, kind);
            }
            // Lex the substitution as a unit so it also works inside strings.
            lx.nested(b']');
            if lx.ctx.peek() == b']' {
                lx.ctx.token(1, Kind::Punctuation);
            }
            lx.mark_value();
            true
        }
        b'$' => {
            let end = var_end(ctx, pos + 1, ctx.len());
            if end == pos + 1 {
                return false;
            }
            emit_to(lx, end, Kind::Variable);
            lx.mark_value();
            true
        }
        b'"' => {
            quote(lx, pos, pos, TCL_STRING);
            true
        }
        b'{' if is_ident_start(ctx.at(pos + 1)) => {
            // `{word list}` is data, but `{puts $x}` is a body whose first word
            // is a known command; the engine marks that one.
            let end = lx.scan_ident(pos + 1);
            if lx.word_kind(&ctx.src[pos + 1..end]).is_some() {
                return false;
            }
            lx.ctx.token(1, Kind::Punctuation);
            lx.ctx.pos = end;
            let kind = lx.classify(pos + 1, end);
            lx.ctx.emit(pos + 1, end, kind);
            true
        }
        b'p' if ctx.starts_with(b"proc") && matches!(ctx.at(pos + 4), b' ' | b'\t') && tcl_command_start(ctx, pos) => {
            lx.ctx.token(4, Kind::Keyword);
            let start = lx.ctx.skip_spaces_from(pos + 4);
            let end = lx.scan_ident(start);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Function);
            true
        }
        _ => false,
    }
}

const TCL_STRING: Quote = Quote { hook_interp: true, ..Quote::new(Kind::String).interp(b"[").vars(b"$") };

static TCL_SPEC: Spec = Spec {
    line_comments: &["#"],
    keywords: &[
        "if", "then", "else", "elseif", "for", "foreach", "while", "switch", "return", "break", "continue",
        "namespace", "package", "variable", "global", "upvar", "uplevel", "try", "catch", "finally", "on", "trap",
        "error", "default", "proc", "apply", "coroutine", "yield", "oo::class", "method", "constructor",
    ],
    builtins: &[
        "set", "unset", "puts", "expr", "incr", "append", "lappend", "lindex", "llength", "list", "lsort", "lrange",
        "lsearch", "lset", "lmap", "string", "dict", "array", "format", "scan", "regexp", "regsub", "split", "join",
        "open", "close", "gets", "read", "file", "info", "source", "exec", "after", "eval", "subst", "clock", "exit",
        "rename", "concat", "vwait", "fconfigure", "flush",
    ],
    constants: &["true", "false", "yes", "no"],
    ident_chars: ":",
    features: COMMANDS | FLAGS,
    hook: Some(tcl_hook),
    hook_chars: "#[$\"p{",

    ..Spec::DEFAULT
};

pub static TCL: Definition = Definition::new("tcl", "Tcl", Engine::Generic(&TCL_SPEC))
    .aliases(&["tclsh", "wish"])
    .extensions(&["tcl", "tk", "tm", "itcl"]);

// GDScript

/// `$Node/Path`, `%UniqueNode`, `&"StringName"` and `^"NodePath"`.
fn gdscript_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let b = ctx.peek();
    let next = ctx.at(pos + 1);
    if next == b'"' && (b != b'%' || lx.expression_start()) {
        quote(lx, pos, pos + 1, Quote::new(Kind::String).single_line());
        return true;
    }
    if matches!(b, b'$' | b'%') && is_ident_start(next) && (b == b'$' || lx.expression_start()) {
        let mut end = pos + 1;
        while is_ident(ctx.at(end)) || (b == b'$' && ctx.at(end) == b'/' && is_ident_start(ctx.at(end + 1))) {
            end += 1;
        }
        emit_to(lx, end, Kind::Variable);
        lx.mark_value();
        return true;
    }
    false
}

static GDSCRIPT_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: PY_STRINGS,
    string_prefixes: &[Prefix::raw(b'r')],
    keywords: &[
        "if", "elif", "else", "for", "while", "match", "when", "break", "continue", "pass", "return", "class_name",
        "extends", "is", "in", "as", "signal", "static", "const", "enum", "var", "await", "breakpoint", "preload",
        "super", "not", "and", "or", "void", "set", "get",
    ],
    types: &[
        "int", "float", "bool", "String", "StringName", "NodePath", "Vector2", "Vector2i", "Vector3", "Vector3i",
        "Color", "Array", "Dictionary", "Variant", "Callable", "Signal",
    ],
    builtins: &[
        "print", "prints", "printerr", "push_error", "push_warning", "load", "range", "len", "str", "typeof",
        "is_instance_valid", "get_node", "randi", "randf", "clamp", "lerp", "abs", "min", "max",
    ],
    constants: &["true", "false", "null", "PI", "TAU", "INF", "NAN"],
    variables: &["self"],
    function_keywords: &["func"],
    type_keywords: &["class"],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    hook: Some(gdscript_hook),
    hook_chars: "$%&^",
    ..Spec::DEFAULT
};

pub static GDSCRIPT: Definition = Definition::new("gdscript", "GDScript", Engine::Generic(&GDSCRIPT_SPEC))
    .aliases(&["gd", "godot"])
    .extensions(&["gd"]);

// AutoHotkey

const AHK_STRING: Quote = Quote::new(Kind::String).escape(b'`').single_line();

/// Offset of the first `::` in `line`.
fn double_colon(line: &[u8]) -> Option<usize> {
    line.windows(2).position(|w| w == b"::")
}

/// Hotkeys (`^j::`), hotstrings (`::btw::`) and labels on a line starting at the cursor.
fn ahk_line_label(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    let line_end = ctx.line_end(pos);
    let line = &ctx.src[pos..line_end];
    if line.first() == Some(&b':') {
        // Hotstring: `:options:abbreviation::replacement`.
        let Some(opts) = line[1..].iter().position(|&b| b == b':') else {
            return false;
        };
        let Some(sep) = double_colon(&line[opts + 2..]).map(|i| pos + opts + 2 + i) else {
            return false;
        };
        emit_to(lx, sep + 2, Kind::Meta);
        let rest = lx.ctx.skip_spaces_from(sep + 2);
        if rest < line_end && lx.ctx.at(rest) != b'{' {
            lx.ctx.pos = rest;
            emit_to(lx, line_end, Kind::String);
        }
        return true;
    }
    if let Some(sep) = double_colon(line).filter(|&i| i > 0 && i <= 40).map(|i| pos + i) {
        if ctx.src[pos..sep].iter().any(|b| matches!(b, b'"' | b'\'' | b'(' | b'=' | b',' | b'%' | b'.')) {
            return false;
        }
        return emit_to(lx, sep + 2, Kind::Meta);
    }
    // `Label:` alone on its line.
    let mut end = pos;
    while is_ident(ctx.at(end)) {
        end += 1;
    }
    if end > pos
        && ctx.at(end) == b':'
        && ctx.at(end + 1) != b'='
        && ctx.src[end + 1..line_end].iter().all(|b| b.is_ascii_whitespace())
        && !ctx.src[pos..end].eq_ignore_ascii_case(b"default")
    {
        return emit_to(lx, end + 1, Kind::Meta);
    }
    if end == pos || !ctx.src[pos].is_ascii_alphabetic() {
        return false;
    }
    // Commands: `MsgBox, text` (v1) takes literal arguments, `var = text` is a
    // v1 literal assignment and `WinMinimize title` calls without parentheses.
    let kind = lx.word_kind(&ctx.src[pos..end]);
    let next_at = ctx.skip_spaces_from(end);
    let next = ctx.at(next_at);
    let spaced = next_at > end;
    match next {
        b',' if kind != Some(Kind::Constant) => {
            lx.ctx.emit(pos, end, kind.unwrap_or(Kind::Function));
            lx.ctx.emit(next_at, next_at + 1, Kind::Punctuation);
            ahk_args(lx, next_at + 1, line_end, true);
        }
        b'=' if kind.is_none() && !matches!(ctx.at(next_at + 1), b'=' | b'>') => {
            lx.ctx.emit(next_at, next_at + 1, Kind::Operator);
            ahk_args(lx, next_at + 1, line_end, false);
        }
        b'"' | b'\'' | b'%' | b'0'..=b'9' | b'\r' | b'\n' | 0 | b'_' | b'a'..=b'z' | b'A'..=b'Z'
            if (spaced || matches!(next, b'\r' | b'\n' | 0)) && matches!(kind, None | Some(Kind::Builtin)) =>
        {
            lx.ctx.pos = end;
            lx.ctx.emit(pos, end, kind.unwrap_or(Kind::Function));
        }
        _ => return false,
    }
    lx.mark_operator();
    true
}

/// Literal command arguments (AutoHotkey v1) from `from` to the end of the
/// line or a comment: `%var%` references, backtick escapes, plain numbers and
/// `% expr` expressions. `commas` splits them into separate arguments.
fn ahk_args(lx: &mut Lexer, from: usize, line_end: usize, commas: bool) {
    let ctx = &*lx.ctx;
    let mut end = from;
    while end < line_end && !(ctx.src[end] == b';' && matches!(ctx.src[end - 1], b' ' | b'\t')) {
        end += 1;
    }
    let mut i = from;
    while i < end {
        let ctx = &mut *lx.ctx;
        let arg = ctx.skip_spaces_from(i);
        if ctx.at(arg) == b'%' && matches!(ctx.at(arg + 1), b' ' | b'\t') {
            // A forced expression runs to the end of the line.
            ctx.emit(arg, arg + 1, Kind::Punctuation);
            let spec = lx.spec;
            lx.ctx.embed_spec(spec, arg + 1, end);
            break;
        }
        let mut j = arg;
        while j < end && !(commas && ctx.src[j] == b',') {
            match ctx.src[j] {
                b'`' => {
                    let mut e = (j + 2).min(end);
                    while e < end && ctx.src[e] & 0xC0 == 0x80 {
                        e += 1;
                    }
                    ctx.emit(j, e, Kind::Escape);
                    j = e;
                }
                b'%' => {
                    let mut e = j + 1;
                    while e < end && is_ident(ctx.src[e]) {
                        e += 1;
                    }
                    if e > j + 1 && ctx.at(e) == b'%' {
                        ctx.emit(j, e + 1, Kind::Variable);
                        j = e + 1;
                    } else {
                        j += 1;
                    }
                }
                _ => j += 1,
            }
        }
        let word = ctx.src[arg..j].trim_ascii_end();
        if !word.is_empty() && (word.iter().all(u8::is_ascii_digit) || (word.starts_with(b"0x") && word.len() > 2)) {
            ctx.emit(arg, arg + word.len(), Kind::Number);
        }
        if j < end {
            ctx.emit(j, j + 1, Kind::Punctuation);
        }
        i = j + 1;
    }
    lx.ctx.pos = end;
}

/// `;` comments need a space before them, backtick escapes, `%var%`, hotkeys and labels.
fn ahk_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b';' => {
            if pos == 0 || ctx.src[pos - 1].is_ascii_whitespace() {
                return false;
            }
            lx.ctx.pos += 1;
            true
        }
        b'"' | b'\'' => {
            quote(lx, pos, pos, AHK_STRING);
            true
        }
        b'%' if !matches!(ctx.at(pos + 1), b' ' | b'\t' | b'%') => {
            let line_end = ctx.line_end_within(pos + 1, 128);
            let Some(close) = ctx.src[pos + 1..line_end].iter().position(|&b| b == b'%').map(|i| pos + 1 + i) else {
                return false;
            };
            if ctx.src[pos + 1..close].iter().all(|&b| is_ident(b)) {
                emit_to(lx, close + 1, Kind::Variable);
            } else {
                // `%expr%` dereferences the result of an expression.
                let spec = lx.spec;
                lx.ctx.token(1, Kind::Punctuation);
                lx.ctx.embed_spec(spec, pos + 1, close);
                lx.ctx.token(1, Kind::Punctuation);
            }
            lx.mark_value();
            true
        }
        _ if ctx.at_line_start(pos) => ahk_line_label(lx),
        _ => false,
    }
}

static AUTOHOTKEY_SPEC: Spec = Spec {
    line_comments: &[";"],
    block_comments: &[("/*", "*/")],
    keywords: &[
        "if", "else", "loop", "while", "for", "in", "return", "break", "continue", "goto", "gosub", "global", "local",
        "static", "try", "catch", "finally", "throw", "switch", "case", "default", "until", "and", "or", "not", "new",
        "is", "isset", "super", "unset", "contains",
    ],
    type_keywords: &["class", "extends"],
    builtins: &[
        "msgbox", "send", "sendinput", "sendtext", "run", "runwait", "sleep", "winactivate", "winwait",
        "winwaitactive", "winexist", "winactive", "winclose", "settimer", "hotkey", "inputbox", "tooltip", "exitapp",
        "reload", "fileappend", "fileread", "regexmatch", "regexreplace", "strlen", "substr", "instr", "strsplit",
        "format", "strreplace", "click", "mousemove", "gui", "map", "array",
    ],
    constants: &["true", "false"],
    variables: &["this"],
    case_insensitive: true,
    features: CALLS | MEMBERS | PREPROCESSOR,
    hook: Some(ahk_hook),
    hook_chars: ";\"'%#^!+<>*~$:abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static AUTOHOTKEY: Definition = Definition::new("autohotkey", "AutoHotkey", Engine::Generic(&AUTOHOTKEY_SPEC))
    .aliases(&["ahk"])
    .extensions(&["ahk", "ahkl"]);

// AppleScript

static APPLESCRIPT_SPEC: Spec = Spec {
    line_comments: &["--", "#"],
    block_comments: &[("(*", "*)")],
    nested_comments: true,
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "about", "above", "after", "against", "and", "around", "as", "at", "before", "behind", "below", "beneath",
        "beside", "between", "by", "considering", "contains", "continue", "copy", "div", "does", "else", "equal",
        "equals", "error", "every", "exit", "from", "get", "given", "global", "if", "ignoring", "in", "into", "is",
        "local", "mod", "not", "of", "onto", "or", "out", "over", "property", "put", "repeat", "return", "returning",
        "script", "set", "some", "tell", "than", "that", "the", "then", "through", "thru", "timeout", "times", "to",
        "transaction", "try", "until", "where", "while", "whose", "with", "without", "first", "last", "front", "back",
        "middle", "contain", "greater", "less",
    ],
    types: &[
        "application", "alias", "file", "folder", "list", "record", "text", "string", "integer", "real", "number",
        "boolean", "date", "window", "document", "item", "class", "reference",
    ],
    builtins: &["display", "dialog", "notification", "activate", "quit", "count", "log", "run", "launch", "open"],
    constants: &["true", "false", "missing", "value", "pi", "tab", "linefeed", "space", "quote"],
    variables: &["me", "my", "it", "its", "result"],
    // `end name` closes a handler, so the name after `end` is one too (`end tell` stays a keyword).
    function_keywords: &["on", "end"],
    case_insensitive: true,
    features: CALLS,
    ..Spec::DEFAULT
};

pub static APPLESCRIPT: Definition = Definition::new("applescript", "AppleScript", Engine::Generic(&APPLESCRIPT_SPEC))
    .aliases(&["osascript"])
    .extensions(&["applescript"]);

// Wren

/// Strings with `%(expr)` interpolation, `"""` raw strings and `_fields`.
fn wren_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let pos = ctx.pos;
    match ctx.peek() {
        b'"' if ctx.starts_with(b"\"\"\"") => {
            let end = ctx.find(pos + 3, b"\"\"\"").map_or(ctx.len(), |p| p + 3);
            emit_to(lx, end, Kind::String);
            lx.mark_value();
            true
        }
        b'"' => {
            quote(lx, pos, pos, Quote::new(Kind::String).interp(b"%("));
            true
        }
        b'_' if is_ident(ctx.at(pos + 1)) && (pos == 0 || !is_ident(ctx.src[pos - 1])) => {
            let end = lx.scan_ident(pos);
            emit_to(lx, end, Kind::Variable);
            lx.mark_value();
            true
        }
        _ => false,
    }
}

static WREN_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    keywords: &[
        "as", "break", "class", "construct", "continue", "else", "for", "foreign", "if", "import", "in", "is",
        "return", "static", "super", "var", "while",
    ],
    builtins: &["System", "Fiber", "Fn", "List", "Map", "Num", "String", "Object", "Range", "Sequence", "Bool"],
    constants: &["true", "false", "null"],
    variables: &["this"],
    features: CALLS | CAPITALIZED_TYPES | MEMBERS,
    hook: Some(wren_hook),
    hook_chars: "\"_",
    ..Spec::DEFAULT
};

pub static WREN: Definition = Definition::new("wren", "Wren", Engine::Generic(&WREN_SPEC)).extensions(&["wren"]);

// Squirrel

static SQUIRREL_SPEC: Spec = Spec {
    line_comments: &["//", "#"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::new("@\"", "\"").raw().doubled().multiline(), Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "base", "break", "case", "catch", "clone", "const", "continue", "default", "delete", "else", "extends", "for",
        "foreach", "if", "in", "instanceof", "local", "resume", "return", "static", "switch", "throw", "try", "typeof",
        "while", "yield", "constructor",
    ],
    builtins: &["print", "error", "array", "assert", "type", "getroottable", "setroottable", "len", "newthread"],
    constants: &["true", "false", "null"],
    variables: &["this"],
    function_keywords: &["function"],
    type_keywords: &["class", "enum"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS,
    ..Spec::DEFAULT
};

pub static SQUIRREL: Definition =
    Definition::new("squirrel", "Squirrel", Engine::Generic(&SQUIRREL_SPEC)).aliases(&["nut"]).extensions(&["nut"]);

// Pony

static PONY_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[Str::quoted("\"\"\"").raw().multiline(), Str::quoted("\"").multiline(), Str::quoted("'")],
    keywords: &[
        "use", "var", "let", "embed", "if", "then", "elseif", "else", "end", "while", "do", "repeat", "until", "for",
        "in", "match", "try", "with", "recover", "consume", "return", "break", "continue", "error", "object", "is",
        "isnt", "and", "or", "not", "xor", "as", "where", "iftype", "ifdef", "compile_error", "iso", "trn", "ref",
        "val", "box", "tag",
    ],
    types: &["Bool", "String", "Array", "U8", "U32", "U64", "I32", "I64", "F32", "F64", "USize", "ISize", "None"],
    constants: &["true", "false"],
    variables: &["this"],
    function_keywords: &["fun", "be", "new"],
    type_keywords: &["actor", "class", "primitive", "struct", "trait", "interface", "type"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS,
    ..Spec::DEFAULT
};

pub static PONY: Definition = Definition::new("pony", "Pony", Engine::Generic(&PONY_SPEC)).extensions(&["pony"]);
