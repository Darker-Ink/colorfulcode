//! Data, configuration and query languages.

use super::{lang, Definition, Engine};
use crate::lexer::generic::{escape_len, features::*, Interp, Lexer, Prefix, Spec, Str};
use crate::lexer::{is_ident, is_ident_start, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[&YAML, &TOML, &INI, &PROPERTIES, &DOTENV, &CSV, &TSV];

// Shared helpers

/// Backslash escapes are recognised.
const ESC: u8 = 1;
/// A doubled quote stands for one quote.
const DOUBLED: u8 = 2;
/// The string may span lines.
const MULTI: u8 = 4;
/// `$name` and `${name}` are variables.
const VARS: u8 = 8;

#[inline]
fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0)
}

/// Emits a comment from the cursor to the end of the line.
fn line_comment(ctx: &mut Ctx) {
    let start = ctx.pos;
    ctx.skip_line();
    ctx.emit_from(start, Kind::Comment);
}

/// Lexes a string whose opening quote is at the cursor.
fn quoted(ctx: &mut Ctx, kind: Kind, flags: u8) {
    let q = ctx.peek();
    let mut seg = ctx.pos;
    ctx.pos += 1;
    while !ctx.done() {
        let b = ctx.peek();
        if b == q {
            if flags & DOUBLED != 0 && ctx.peek_at(1) == q {
                ctx.emit_from(seg, kind);
                ctx.token(2, Kind::Escape);
                seg = ctx.pos;
                continue;
            }
            ctx.pos += 1;
            break;
        }
        match b {
            b'\\' if flags & ESC != 0 => {
                ctx.emit_from(seg, kind);
                let s = ctx.pos;
                escape_len(ctx);
                ctx.emit_from(s, Kind::Escape);
                seg = ctx.pos;
            }
            b'$' if flags & VARS != 0 => match dollar_var_end(ctx, ctx.pos, ctx.len()) {
                Some(end) => {
                    ctx.emit_from(seg, kind);
                    ctx.emit(ctx.pos, end, Kind::Variable);
                    ctx.pos = end;
                    seg = end;
                }
                None => ctx.pos += 1,
            },
            b'\n' if flags & MULTI == 0 => break,
            _ => ctx.pos += 1,
        }
    }
    ctx.emit_from(seg, kind);
}

/// End of `$name` or `${name}` at `pos`, if one starts there and ends by `limit`.
fn dollar_var_end(ctx: &Ctx, pos: usize, limit: usize) -> Option<usize> {
    let next = ctx.at(pos + 1);
    let end = if next == b'{' {
        let close = ctx.src[pos..limit].iter().position(|&b| b == b'}' || b == b'\n')?;
        if ctx.src[pos + close] != b'}' {
            return None;
        }
        pos + close + 1
    } else if is_ident_start(next) {
        let mut e = pos + 1;
        while is_ident(ctx.at(e)) {
            e += 1;
        }
        e
    } else {
        return None;
    };
    (end <= limit).then_some(end)
}

/// Lexes text delimited by `open_len` bytes at the cursor and `close`, which
/// may span lines.
fn delimited(ctx: &mut Ctx, open_len: usize, close: &[u8], escapes: bool, kind: Kind) {
    let mut seg = ctx.pos;
    ctx.pos += open_len;
    while !ctx.done() {
        if ctx.starts_with(close) {
            ctx.pos += close.len();
            // `""""` closes with a quote inside the string (TOML).
            let mut extra = 0;
            while extra < 2 && ctx.peek() == close[0] {
                ctx.pos += 1;
                extra += 1;
            }
            break;
        }
        if escapes && ctx.peek() == b'\\' {
            ctx.emit_from(seg, kind);
            let s = ctx.pos;
            escape_len(ctx);
            ctx.emit_from(s, Kind::Escape);
            seg = ctx.pos;
            continue;
        }
        ctx.pos += 1;
    }
    ctx.emit_from(seg, kind);
}

/// Strips trailing spaces, tabs and carriage returns.
fn trim_end(src: &[u8], start: usize, mut end: usize) -> usize {
    while end > start && matches!(src[end - 1], b' ' | b'\t' | b'\r') {
        end -= 1;
    }
    end
}

/// Column of `pos` within its line.
fn column(ctx: &Ctx, pos: usize) -> usize {
    pos - ctx.src[..pos].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1)
}

/// Whether `s` is a number: decimal with `_` separators, fraction and
/// exponent, or `0x`, `0o` and `0b` prefixed. A leading sign is allowed.
fn is_number(s: &[u8]) -> bool {
    let s = match s {
        [b'+' | b'-', rest @ ..] => rest,
        _ => s,
    };
    if let [b'0', b'x' | b'X' | b'o' | b'O' | b'b' | b'B', rest @ ..] = s {
        return !rest.is_empty() && rest.iter().all(|b| b.is_ascii_hexdigit() || *b == b'_');
    }
    let mut i = 0;
    let mut digits = 0;
    let mut take = |i: &mut usize| {
        while *i < s.len() && (s[*i].is_ascii_digit() || (s[*i] == b'_' && *i > 0)) {
            digits += 1;
            *i += 1;
        }
    };
    take(&mut i);
    if i < s.len() && s[i] == b'.' {
        i += 1;
        take(&mut i);
    }
    if digits == 0 {
        return false;
    }
    if i < s.len() && s[i] | 0x20 == b'e' {
        i += 1;
        if i < s.len() && matches!(s[i], b'+' | b'-') {
            i += 1;
        }
        let before = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        if i == before {
            return false;
        }
    }
    i == s.len()
}

/// Whether `s` is a date or timestamp such as `2024-01-31` or
/// `2024-01-31T10:00:00Z`, or a time such as `07:32:00`.
fn is_date(s: &[u8]) -> bool {
    let date = s.len() >= 8 && s[..4].iter().all(u8::is_ascii_digit) && s[4] == b'-';
    let time = s.len() >= 5 && s[..2].iter().all(u8::is_ascii_digit) && s[2] == b':';
    (date || time)
        && s.iter()
            .all(|&b| b.is_ascii_digit() || matches!(b, b'-' | b':' | b'.' | b'T' | b't' | b'Z' | b'z' | b'+' | b' '))
}

fn is_bool_word(s: &[u8]) -> bool {
    ["true", "false", "yes", "no", "on", "off"].iter().any(|w| s.eq_ignore_ascii_case(w.as_bytes()))
}

/// Emits a configuration value: numbers, booleans and dates keep their kind,
/// anything else is a string in which `${name}`, `$name` and `%(name)s` are
/// variables and, with `escapes`, backslash sequences are escapes.
fn config_value(ctx: &mut Ctx, start: usize, end: usize, escapes: bool) {
    let end = trim_end(ctx.src, start, end);
    if start >= end {
        return;
    }
    let text = &ctx.src[start..end];
    if is_number(text) || is_date(text) {
        ctx.emit(start, end, Kind::Number);
        ctx.pos = end;
        return;
    }
    if is_bool_word(text) || text.eq_ignore_ascii_case(b"null") || text.eq_ignore_ascii_case(b"none") {
        ctx.emit(start, end, Kind::Constant);
        ctx.pos = end;
        return;
    }
    let mut seg = start;
    let mut i = start;
    while i < end {
        match ctx.src[i] {
            b'$' => {
                if let Some(e) = dollar_var_end(ctx, i, end) {
                    ctx.emit(seg, i, Kind::String);
                    ctx.emit(i, e, Kind::Variable);
                    seg = e;
                    i = e;
                    continue;
                }
            }
            b'%' if ctx.at(i + 1) == b'(' => {
                if let Some(c) = ctx.src[i..end].iter().position(|&b| b == b')') {
                    let mut e = i + c + 1;
                    if e < end && ctx.src[e].is_ascii_alphabetic() {
                        e += 1;
                    }
                    ctx.emit(seg, i, Kind::String);
                    ctx.emit(i, e, Kind::Variable);
                    seg = e;
                    i = e;
                    continue;
                }
            }
            b'\\' if escapes => {
                ctx.emit(seg, i, Kind::String);
                ctx.pos = i;
                escape_len(ctx);
                let e = ctx.pos.min(end);
                ctx.emit(i, e, Kind::Escape);
                seg = e;
                i = e;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    ctx.emit(seg, end, Kind::String);
    ctx.pos = end;
}

// YAML

pub static YAML: Definition = Definition::new("yaml", "YAML", Engine::Custom(yaml))
    .aliases(&["yml"])
    .extensions(&["yaml", "yml"])
    .filenames(&[".clang-format", ".clang-tidy", ".gemrc"]);

fn yaml(ctx: &mut Ctx) {
    while !ctx.done() {
        let start = ctx.pos;
        let at = ctx.skip_spaces_from(start);
        ctx.pos = at;
        if at == start && yaml_document_marker(ctx, at) {
            ctx.token(3, Kind::Punctuation);
            ctx.skip_spaces();
            yaml_block(ctx, -1);
        } else if at == start && ctx.peek() == b'%' {
            ctx.skip_line();
            ctx.emit_from(at, Kind::Meta);
        } else {
            yaml_block(ctx, column(ctx, at) as isize - 1);
        }
        yaml_rest(ctx);
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

fn yaml_document_marker(ctx: &Ctx, at: usize) -> bool {
    (ctx.starts_with_at(at, b"---") || ctx.starts_with_at(at, b"..."))
        && is_blank(ctx.at(at + 3))
        && (at == 0 || ctx.at(at - 1) == b'\n')
}

/// Finishes a line: an optional comment, then anything left is plain text.
fn yaml_rest(ctx: &mut Ctx) {
    ctx.skip_spaces();
    if ctx.peek() == b'#' {
        line_comment(ctx);
    }
    ctx.skip_line();
}

/// Block indicators (`- `, `? `, `: `) followed by a node. `parent` is the
/// column of the enclosing node, which a block scalar must be indented past.
fn yaml_block(ctx: &mut Ctx, mut parent: isize) {
    while matches!(ctx.peek(), b'-' | b'?' | b':') && is_blank(ctx.peek_at(1)) {
        let b = ctx.peek();
        parent = column(ctx, ctx.pos) as isize;
        ctx.token(1, Kind::Punctuation);
        ctx.skip_spaces();
        if b == b':' {
            yaml_value(ctx, parent);
            return;
        }
    }
    yaml_properties(ctx);
    let start = ctx.pos;
    if let Some((key_end, colon)) = yaml_key(ctx, start, false) {
        let key_col = column(ctx, start) as isize;
        ctx.emit(start, key_end, Kind::Property);
        ctx.pos = colon;
        ctx.token(1, Kind::Punctuation);
        ctx.skip_spaces();
        yaml_value(ctx, key_col);
    } else {
        yaml_value(ctx, parent);
    }
}

/// If a mapping key starts at `from`, returns the end of the key and the
/// offset of its `:`.
fn yaml_key(ctx: &Ctx, from: usize, flow: bool) -> Option<(usize, usize)> {
    let b = ctx.at(from);
    if b == b'"' || b == b'\'' {
        let line_end = ctx.line_end(from);
        let mut i = from + 1;
        while i < line_end {
            let c = ctx.src[i];
            if c == b'\\' && b == b'"' {
                i += 2;
                continue;
            }
            if c == b {
                if b == b'\'' && ctx.at(i + 1) == b'\'' {
                    i += 2;
                    continue;
                }
                break;
            }
            i += 1;
        }
        if i >= line_end {
            return None;
        }
        let end = i + 1;
        let colon = ctx.skip_spaces_from(end);
        let after = ctx.at(colon + 1);
        return (ctx.at(colon) == b':' && (is_blank(after) || flow)).then_some((end, colon));
    }
    if matches!(b, b'[' | b']' | b'{' | b'}' | b',' | b'#' | b'&' | b'*' | b'!' | b'|' | b'>' | b'%' | b'@' | b'`')
        || is_blank(b)
    {
        return None;
    }
    let mut i = from;
    loop {
        match ctx.at(i) {
            0 | b'\n' => return None,
            b':' if is_blank(ctx.at(i + 1)) || (flow && matches!(ctx.at(i + 1), b',' | b']' | b'}')) => {
                return Some((trim_end(ctx.src, from, i), i));
            }
            b'#' if matches!(ctx.at(i - 1), b' ' | b'\t') => return None,
            b',' | b'[' | b']' | b'{' | b'}' if flow => return None,
            _ => i += 1,
        }
    }
}

/// Anchors (`&name`) and tags (`!tag`, `!!str`, `!<uri>`) before a node.
fn yaml_properties(ctx: &mut Ctx) {
    loop {
        match ctx.peek() {
            b'&' => yaml_name(ctx, Kind::Variable),
            b'!' => yaml_name(ctx, Kind::Type),
            _ => return,
        }
        ctx.skip_spaces();
    }
}

/// An anchor, alias or tag at the cursor, up to whitespace or a flow indicator.
fn yaml_name(ctx: &mut Ctx, kind: Kind) {
    let start = ctx.pos;
    let mut end = start + 1;
    if ctx.at(end) == b'<' {
        let line_end = ctx.line_end(start);
        end = ctx.src[start..line_end].iter().position(|&b| b == b'>').map_or(line_end, |i| start + i + 1);
    } else {
        while !is_blank(ctx.at(end)) && !matches!(ctx.at(end), b',' | b'[' | b']' | b'{' | b'}') {
            end += 1;
        }
    }
    ctx.pos = end;
    ctx.emit(start, end, kind);
}

fn yaml_value(ctx: &mut Ctx, parent: isize) {
    yaml_properties(ctx);
    match ctx.peek() {
        b'|' | b'>' if yaml_block_scalar(ctx, parent) => {}
        b'"' => quoted(ctx, Kind::String, ESC | MULTI),
        b'\'' => quoted(ctx, Kind::String, DOUBLED | MULTI),
        b'[' | b'{' => yaml_flow(ctx),
        b'*' => yaml_name(ctx, Kind::Variable),
        0 | b'\n' | b'\r' | b'#' => {}
        _ => {
            let start = ctx.pos;
            let end = yaml_plain_end(ctx, start, false);
            ctx.emit(start, end, yaml_scalar_kind(&ctx.src[start..end]));
            ctx.pos = end;
        }
    }
}

/// End of a plain scalar starting at `from` (trailing spaces excluded).
fn yaml_plain_end(ctx: &Ctx, from: usize, flow: bool) -> usize {
    let mut i = from;
    loop {
        match ctx.at(i) {
            0 | b'\n' => break,
            b'#' if i > from && matches!(ctx.at(i - 1), b' ' | b'\t') => break,
            b',' | b'[' | b']' | b'{' | b'}' if flow => break,
            b':' if flow && (is_blank(ctx.at(i + 1)) || matches!(ctx.at(i + 1), b',' | b']' | b'}')) => break,
            _ => i += 1,
        }
    }
    trim_end(ctx.src, from, i)
}

fn yaml_scalar_kind(s: &[u8]) -> Kind {
    const CONSTANTS: &[&[u8]] = &[
        b"~", b"null", b"Null", b"NULL", b"true", b"True", b"TRUE", b"false", b"False", b"FALSE", b"yes", b"Yes",
        b"YES", b"no", b"No", b"NO", b"on", b"On", b"ON", b"off", b"Off", b"OFF",
    ];
    if CONSTANTS.contains(&s) {
        return Kind::Constant;
    }
    let unsigned = match s {
        [b'+' | b'-', rest @ ..] => rest,
        _ => s,
    };
    if matches!(unsigned, b".inf" | b".Inf" | b".INF") || matches!(s, b".nan" | b".NaN" | b".NAN") {
        return Kind::Number;
    }
    if is_number(s) || is_date(s) {
        Kind::Number
    } else {
        Kind::String
    }
}

/// A literal (`|`) or folded (`>`) block scalar: the header, then every
/// following line indented past `parent`.
fn yaml_block_scalar(ctx: &mut Ctx, parent: isize) -> bool {
    let start = ctx.pos;
    let mut i = start + 1;
    let mut explicit = None;
    while i < start + 3 && matches!(ctx.at(i), b'-' | b'+' | b'1'..=b'9') {
        if ctx.at(i).is_ascii_digit() {
            explicit = Some((ctx.at(i) - b'0') as usize);
        }
        i += 1;
    }
    let after = ctx.skip_spaces_from(i);
    if !(matches!(ctx.at(after), 0 | b'\n' | b'\r') || (ctx.at(after) == b'#' && after > i)) {
        return false;
    }
    ctx.token(i - start, Kind::Operator);
    ctx.skip_spaces();
    if ctx.peek() == b'#' {
        line_comment(ctx);
    }
    let mut last = ctx.line_end(ctx.pos);
    let mut indent = explicit.map(|n| parent.max(0) as usize + n);
    let mut line = last;
    while ctx.at(line) == b'\n' {
        let line_start = line + 1;
        let line_end = ctx.line_end(line_start);
        line = line_end;
        let content = ctx.skip_spaces_from(line_start);
        if trim_end(ctx.src, content, line_end) == content {
            continue;
        }
        if content == line_start && yaml_document_marker(ctx, line_start) {
            break;
        }
        let col = content - line_start;
        match indent {
            None if col as isize > parent => indent = Some(col),
            None => break,
            Some(n) if col < n => break,
            Some(_) => {}
        }
        ctx.emit(content, line_end, Kind::String);
        last = line_end;
    }
    ctx.pos = last;
    true
}

/// A flow collection (`[...]` or `{...}`), which may span lines.
fn yaml_flow(ctx: &mut Ctx) {
    let mut depth = 0u32;
    while !ctx.done() {
        let start = ctx.pos;
        match ctx.peek() {
            b' ' | b'\t' | b'\r' | b'\n' => ctx.pos += 1,
            b'[' | b'{' => {
                depth += 1;
                ctx.token(1, Kind::Punctuation);
            }
            b']' | b'}' => {
                ctx.token(1, Kind::Punctuation);
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return;
                }
            }
            b',' => ctx.token(1, Kind::Punctuation),
            b'#' if start > 0 && is_blank(ctx.at(start - 1)) => line_comment(ctx),
            q @ (b'"' | b'\'') => {
                quoted(ctx, Kind::String, if q == b'"' { ESC | MULTI } else { DOUBLED | MULTI });
                if ctx.next_non_space(ctx.pos) == b':' {
                    let end = ctx.pos;
                    ctx.truncate_to(start);
                    ctx.emit(start, end, Kind::Property);
                }
            }
            b'&' | b'!' => yaml_properties(ctx),
            b'*' => yaml_name(ctx, Kind::Variable),
            b':' | b'?' | b'-' if is_blank(ctx.peek_at(1)) || matches!(ctx.peek_at(1), b',' | b']' | b'}') => {
                ctx.token(1, Kind::Punctuation)
            }
            _ => {
                if let Some((key_end, _)) = yaml_key(ctx, start, true) {
                    ctx.emit(start, key_end, Kind::Property);
                    ctx.pos = key_end;
                } else {
                    let end = yaml_plain_end(ctx, start, true);
                    if end == start {
                        ctx.bump_char();
                    } else {
                        ctx.emit(start, end, yaml_scalar_kind(&ctx.src[start..end]));
                        ctx.pos = end;
                    }
                }
            }
        }
    }
}

// TOML

pub static TOML: Definition = Definition::new("toml", "TOML", Engine::Custom(toml))
    .extensions(&["toml"])
    .filenames(&["Cargo.lock", "Pipfile", "poetry.lock", "uv.lock", "Gopkg.lock"]);

fn toml(ctx: &mut Ctx) {
    // One bit per open bracket, innermost lowest: 1 for an inline table.
    let mut tables: u64 = 0;
    let mut depth = 0u32;
    let mut key = true;
    while !ctx.done() {
        let b = ctx.peek();
        match b {
            b'\n' => {
                ctx.pos += 1;
                key |= depth == 0;
            }
            b' ' | b'\t' | b'\r' => ctx.pos += 1,
            b'#' => line_comment(ctx),
            b'[' if key && depth == 0 => {
                toml_header(ctx);
                key = false;
            }
            b'[' | b'{' => {
                ctx.token(1, Kind::Punctuation);
                tables = (tables << 1) | (b == b'{') as u64;
                depth += 1;
                key = b == b'{';
            }
            b']' | b'}' => {
                ctx.token(1, Kind::Punctuation);
                tables >>= 1;
                depth = depth.saturating_sub(1);
                key = false;
            }
            b',' => {
                ctx.token(1, Kind::Punctuation);
                key = depth > 0 && tables & 1 == 1;
            }
            b'=' => {
                ctx.token(1, Kind::Operator);
                key = false;
            }
            b'.' if key => ctx.token(1, Kind::Punctuation),
            b'"' | b'\'' => toml_string(ctx, if key { Kind::Property } else { Kind::String }),
            _ if key && toml_bare(b) => {
                let start = ctx.pos;
                while toml_bare(ctx.peek()) {
                    ctx.pos += 1;
                }
                ctx.emit_from(start, Kind::Property);
            }
            _ if b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'_') => toml_scalar(ctx),
            _ => {
                if b.is_ascii_punctuation() {
                    ctx.token(1, Kind::Punctuation);
                } else {
                    ctx.bump_char();
                }
            }
        }
    }
}

fn toml_bare(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn toml_string(ctx: &mut Ctx, kind: Kind) {
    let q = ctx.peek();
    if ctx.peek_at(1) == q && ctx.peek_at(2) == q {
        let close: &[u8] = if q == b'"' { b"\"\"\"" } else { b"'''" };
        delimited(ctx, 3, close, q == b'"' && kind == Kind::String, kind);
    } else {
        quoted(ctx, kind, if q == b'"' && kind == Kind::String { ESC } else { 0 });
    }
}

/// A table header: `[name]` or `[[name]]` with dotted and quoted parts.
fn toml_header(ctx: &mut Ctx) {
    let open = if ctx.peek_at(1) == b'[' { 2 } else { 1 };
    ctx.token(open, Kind::Punctuation);
    while !ctx.done() {
        match ctx.peek() {
            b'\n' => return,
            b']' => {
                let close = if ctx.peek_at(1) == b']' { 2 } else { 1 };
                ctx.token(close, Kind::Punctuation);
                return;
            }
            b'"' | b'\'' => quoted(ctx, Kind::Type, 0),
            b'.' => ctx.token(1, Kind::Punctuation),
            b' ' | b'\t' | b'\r' => ctx.pos += 1,
            b'#' => return,
            _ => {
                let start = ctx.pos;
                while !matches!(ctx.peek(), 0 | b'\n' | b']' | b'.' | b' ' | b'\t' | b'#' | b'"' | b'\'') {
                    ctx.pos += 1;
                }
                if ctx.pos == start {
                    ctx.bump_char();
                }
                ctx.emit_from(start, Kind::Type);
            }
        }
    }
}

/// A bare value: number, date, boolean, `inf` or `nan`.
fn toml_scalar(ctx: &mut Ctx) {
    let start = ctx.pos;
    let word = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'_' | b'.' | b':');
    while word(ctx.peek()) {
        ctx.pos += 1;
    }
    // `1979-05-27 07:32:00`: a space may separate the date and time.
    if ctx.pos - start == 10
        && ctx.peek() == b' '
        && ctx.peek_at(1).is_ascii_digit()
        && ctx.peek_at(2).is_ascii_digit()
        && ctx.peek_at(3) == b':'
    {
        ctx.pos += 1;
        while word(ctx.peek()) {
            ctx.pos += 1;
        }
    }
    let text = &ctx.src[start..ctx.pos];
    let unsigned = match text {
        [b'+' | b'-', rest @ ..] => rest,
        _ => text,
    };
    let kind = match text {
        b"true" | b"false" => Kind::Constant,
        _ if unsigned == b"inf" || unsigned == b"nan" => Kind::Number,
        _ if unsigned.first().is_some_and(u8::is_ascii_digit) => Kind::Number,
        _ => Kind::Text,
    };
    ctx.emit_from(start, kind);
}

// INI

pub static INI: Definition = Definition::new("ini", "INI", Engine::Custom(ini))
    .aliases(&["dosini", "cfg", "editorconfig", "gitconfig"])
    .extensions(&["ini", "cfg", "conf", "cnf", "prefs", "inf", "desktop"])
    .filenames(&[
        ".editorconfig", ".gitconfig", ".gitmodules", ".npmrc", ".pylintrc", ".flake8", ".coveragerc", ".hgrc",
    ]);

fn ini(ctx: &mut Ctx) {
    while !ctx.done() {
        let line_start = ctx.pos;
        let line_end = ctx.line_end(line_start);
        let at = ctx.skip_spaces_from(line_start);
        ctx.pos = at;
        match ctx.peek() {
            b';' | b'#' => line_comment(ctx),
            b'[' => {
                ctx.token(1, Kind::Punctuation);
                while ctx.pos < line_end {
                    match ctx.peek() {
                        b']' => {
                            ctx.token(1, Kind::Punctuation);
                            break;
                        }
                        b'"' => quoted(ctx, Kind::String, ESC),
                        b' ' | b'\t' => ctx.pos += 1,
                        _ => {
                            let s = ctx.pos;
                            ctx.pos += 1;
                            while ctx.pos < line_end && !matches!(ctx.peek(), b']' | b'"') {
                                ctx.pos += 1;
                            }
                            let e = trim_end(ctx.src, s, ctx.pos);
                            ctx.emit(s, e, Kind::Type);
                        }
                    }
                }
                ini_trailing(ctx, line_end);
            }
            0 | b'\n' | b'\r' => {}
            _ => {
                let sep = ctx.src[at..line_end].iter().position(|&b| b == b'=' || b == b':').map(|i| at + i);
                match sep {
                    Some(sep) => {
                        ctx.emit(at, trim_end(ctx.src, at, sep), Kind::Property);
                        ctx.pos = sep;
                        ctx.token(1, Kind::Operator);
                        ini_value(ctx, line_end);
                    }
                    // An indented line continues the previous value.
                    None if at > line_start => ini_value(ctx, line_end),
                    None => ctx.emit(at, trim_end(ctx.src, at, line_end), Kind::Property),
                }
            }
        }
        ctx.pos = line_end;
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

fn ini_value(ctx: &mut Ctx, line_end: usize) {
    ctx.skip_spaces();
    if matches!(ctx.peek(), b'"' | b'\'') {
        quoted(ctx, Kind::String, ESC);
        ini_trailing(ctx, line_end);
        return;
    }
    let start = ctx.pos;
    let mut end = line_end;
    let mut i = start;
    while i < line_end {
        if matches!(ctx.src[i], b';' | b'#') && matches!(ctx.at(i.wrapping_sub(1)), b' ' | b'\t') {
            end = i;
            break;
        }
        i += 1;
    }
    config_value(ctx, start, end, false);
    ctx.pos = end;
    ini_trailing(ctx, line_end);
}

/// An inline comment after a value or section header.
fn ini_trailing(ctx: &mut Ctx, line_end: usize) {
    ctx.skip_spaces();
    if matches!(ctx.peek(), b';' | b'#') {
        line_comment(ctx);
    }
    ctx.pos = line_end;
}

// Java properties

pub static PROPERTIES: Definition = Definition::new("properties", "Properties", Engine::Custom(properties))
    .aliases(&["java-properties"])
    .extensions(&["properties"]);

fn properties(ctx: &mut Ctx) {
    let mut continued = false;
    while !ctx.done() {
        let line_start = ctx.pos;
        let line_end = ctx.line_end(line_start);
        ctx.pos = ctx.skip_spaces_from(line_start);
        if continued {
            config_value(ctx, ctx.pos, line_end, true);
        } else {
            match ctx.peek() {
                b'#' | b'!' => line_comment(ctx),
                0 | b'\n' | b'\r' => {}
                _ => {
                    let start = ctx.pos;
                    while !matches!(ctx.peek(), 0 | b'\n' | b'\r' | b'=' | b':' | b' ' | b'\t') {
                        ctx.pos += if ctx.peek() == b'\\' { 2 } else { 1 };
                    }
                    ctx.pos = ctx.pos.min(line_end);
                    ctx.emit_from(start, Kind::Property);
                    ctx.skip_spaces();
                    if matches!(ctx.peek(), b'=' | b':') {
                        ctx.token(1, Kind::Operator);
                        ctx.skip_spaces();
                    }
                    let value = ctx.pos;
                    config_value(ctx, value, line_end, true);
                }
            }
        }
        // A line ending in an odd number of backslashes continues.
        let end = trim_end(ctx.src, line_start, line_end);
        let slashes = ctx.src[line_start..end].iter().rev().take_while(|&&b| b == b'\\').count();
        continued = slashes % 2 == 1 && !matches!(ctx.at(ctx.skip_spaces_from(line_start)), b'#' | b'!');
        ctx.pos = line_end;
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

// dotenv

pub static DOTENV: Definition =
    Definition::new("dotenv", "Dotenv", Engine::Custom(dotenv)).aliases(&["env"]).extensions(&["env"]).filenames(&[
        ".env", ".env.local", ".env.example", ".env.sample", ".env.development", ".env.production", ".env.test",
        ".env.staging",
    ]);

fn dotenv(ctx: &mut Ctx) {
    while !ctx.done() {
        ctx.pos = ctx.skip_spaces_from(ctx.pos);
        match ctx.peek() {
            b'#' => line_comment(ctx),
            b'\n' | b'\r' => {}
            _ => {
                if ctx.starts_with(b"export") && matches!(ctx.peek_at(6), b' ' | b'\t') {
                    ctx.token(6, Kind::Keyword);
                    ctx.skip_spaces();
                }
                let key = ctx.pos;
                while is_ident(ctx.peek()) || matches!(ctx.peek(), b'.' | b'-') {
                    ctx.pos += 1;
                }
                ctx.emit_from(key, Kind::Property);
                ctx.skip_spaces();
                if matches!(ctx.peek(), b'=' | b':') {
                    ctx.token(1, Kind::Operator);
                    ctx.skip_spaces();
                    match ctx.peek() {
                        b'"' | b'`' => quoted(ctx, Kind::String, ESC | MULTI | VARS),
                        b'\'' => quoted(ctx, Kind::String, MULTI),
                        _ => {
                            let value = ctx.pos;
                            let line_end = ctx.line_end(value);
                            let mut end = line_end;
                            let mut i = value;
                            while i < line_end {
                                if ctx.src[i] == b'#' && i > value && matches!(ctx.src[i - 1], b' ' | b'\t') {
                                    end = i;
                                    break;
                                }
                                i += 1;
                            }
                            config_value(ctx, value, end, false);
                            ctx.pos = end;
                        }
                    }
                }
                ctx.skip_spaces();
                if ctx.peek() == b'#' {
                    line_comment(ctx);
                }
            }
        }
        ctx.skip_line();
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

// CSV and TSV

pub static CSV: Definition =
    Definition::new("csv", "CSV", Engine::Custom(|ctx| columns(ctx, b','))).extensions(&["csv"]);

pub static TSV: Definition =
    Definition::new("tsv", "TSV", Engine::Custom(|ctx| columns(ctx, b'\t'))).extensions(&["tsv", "tab"]);

/// Kinds cycled through for each column so neighbors are easy to tell apart.
const COLUMN_KINDS: &[Kind] = &[Kind::Property, Kind::String, Kind::Number, Kind::Function, Kind::Type, Kind::Constant];

fn columns(ctx: &mut Ctx, mut sep: u8) {
    if sep == b',' {
        // Semicolon separated files are common where `,` is the decimal mark.
        let first = &ctx.src[ctx.pos..ctx.line_end(ctx.pos)];
        if !first.contains(&b',') && first.contains(&b';') {
            sep = b';';
        }
    }
    let mut column = 0;
    while !ctx.done() {
        let b = ctx.peek();
        if b == sep {
            ctx.token(1, Kind::Punctuation);
            column += 1;
            continue;
        }
        match b {
            b'\n' => {
                ctx.pos += 1;
                column = 0;
            }
            b'\r' => ctx.pos += 1,
            _ => {
                let start = ctx.pos;
                if b == b'"' {
                    quoted(ctx, Kind::Text, DOUBLED | MULTI);
                }
                while !ctx.done() && !matches!(ctx.peek(), b'\n' | b'\r') && ctx.peek() != sep {
                    ctx.pos += 1;
                }
                ctx.truncate_to(start);
                ctx.emit_from(start, COLUMN_KINDS[column % COLUMN_KINDS.len()]);
            }
        }
    }
}
