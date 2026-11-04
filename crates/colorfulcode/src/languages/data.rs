//! Data, configuration and query languages.

use super::{lang, Definition, Engine};
use crate::lexer::generic::{escape_len, features::*, Interp, Lexer, Prefix, Spec, Str};
use crate::lexer::{is_ident, is_ident_start, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[
    &YAML, &TOML, &INI, &PROPERTIES, &DOTENV, &CSV, &TSV, &SQL, &GRAPHQL, &DIFF, &PROTOBUF, &THRIFT, &HCL, &PRISMA,
    &HTTP, &CYPHER, &SPARQL, &KDL, &CUE, &JSONNET, &DOT, &MERMAID, &PLANTUML, &LOG, &REGEX,
];

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
                    // Keys end at an unescaped separator or whitespace.
                    while ctx.pos < line_end && !matches!(ctx.peek(), b'\r' | b'=' | b':' | b' ' | b'\t') {
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

// SQL

/// Dollar quoted bodies (`$$ ... $$`, `$fn$ ... $fn$`), parameters (`$1`,
/// `:name`), `::` casts and bracketed identifiers (`[Order Details]`).
fn sql_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'$' => {
            let next = ctx.peek_at(1);
            if next.is_ascii_digit() {
                let mut end = start + 1;
                while ctx.at(end).is_ascii_digit() {
                    end += 1;
                }
                ctx.pos = end;
                ctx.emit(start, end, Kind::Variable);
                lx.mark_value();
                return true;
            }
            let mut tag_end = start + 1;
            while is_ident(ctx.at(tag_end)) && tag_end - start < 64 {
                tag_end += 1;
            }
            if ctx.at(tag_end) != b'$' {
                return false;
            }
            let tag = &ctx.src[start..=tag_end];
            let end = ctx.find(tag_end + 1, tag).map_or(ctx.len(), |p| p + tag.len());
            ctx.pos = end;
            ctx.emit(start, end, Kind::String);
            lx.mark_value();
            true
        }
        b'[' => {
            if start > 0 && (is_ident(ctx.src[start - 1]) || matches!(ctx.src[start - 1], b')' | b']')) {
                return false;
            }
            let line_end = ctx.line_end(start);
            let Some(close) = ctx.src[start..line_end].iter().position(|&b| b == b']') else {
                return false;
            };
            let inner = &ctx.src[start + 1..start + close];
            if inner.is_empty() || !inner.iter().all(|&b| is_ident(b) || matches!(b, b' ' | b'-' | b'$' | b'#')) {
                return false;
            }
            ctx.pos = start + close + 1;
            ctx.emit(start, ctx.pos, Kind::Property);
            lx.mark_value();
            true
        }
        b':' if ctx.peek_at(1) == b':' => {
            ctx.token(2, Kind::Operator);
            lx.mark_operator();
            true
        }
        b':' if is_ident_start(ctx.peek_at(1)) && (start == 0 || !is_ident(ctx.src[start - 1])) => {
            let end = lx.scan_ident(start + 1);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Variable);
            lx.mark_value();
            true
        }
        _ => false,
    }
}

static SQL_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("/*", "*/")],
    strings: &[
        Str::quoted("'").doubled().multiline(),
        Str::quoted("\"").raw().doubled().kind(Kind::Property),
        Str::quoted("`").raw().doubled().kind(Kind::Property),
    ],
    string_prefixes: &[
        Prefix::plain(b'E'),
        Prefix::plain(b'e'),
        Prefix::plain(b'N'),
        Prefix::plain(b'n'),
        Prefix::plain(b'X'),
        Prefix::plain(b'x'),
        Prefix::plain(b'B'),
        Prefix::plain(b'b'),
    ],
    keywords: &[
        "add", "after", "all", "alter", "analyze", "and", "any", "as", "asc", "begin", "between", "by", "call",
        "cascade", "case", "check", "collate", "column", "commit", "conflict", "constraint", "create", "cross",
        "current", "database", "declare", "default", "delete", "desc", "distinct", "do", "drop", "each", "else",
        "elsif", "end", "escape", "except", "exception", "execute", "exists", "explain", "extension", "fetch",
        "filter", "first", "following", "for", "foreign", "from", "full", "function", "grant", "group", "having", "if",
        "ilike", "in", "index", "inner", "insert", "intersect", "into", "is", "join", "key", "language", "last",
        "lateral", "left", "like", "limit", "loop", "materialized", "merge", "natural", "next", "not", "nothing",
        "nulls", "of", "offset", "on", "only", "or", "order", "outer", "over", "partition", "preceding", "primary",
        "procedure", "raise", "range", "recursive", "references", "rename", "replace", "return", "returning",
        "returns", "revoke", "right", "rollback", "row", "rows", "schema", "select", "sequence", "set", "show",
        "table", "temp", "temporary", "then", "to", "top", "transaction", "trigger", "truncate", "unbounded", "union",
        "unique", "update", "use", "using", "vacuum", "values", "view", "when", "where", "while", "window", "with",
        "within",
    ],
    types: &[
        "array", "bigint", "bigserial", "binary", "bit", "blob", "bool", "boolean", "bytea", "char", "character",
        "clob", "date", "datetime", "decimal", "double", "float", "int", "integer", "interval", "json", "jsonb",
        "money", "nchar", "numeric", "nvarchar", "precision", "real", "serial", "smallint", "text", "time",
        "timestamp", "timestamptz", "tinyint", "uuid", "varbinary", "varchar", "varying", "void", "xml",
    ],
    builtins: &[
        "avg", "cast", "coalesce", "concat", "count", "current_date", "current_timestamp", "date_trunc", "extract",
        "greatest", "least", "length", "lower", "max", "min", "now", "nullif", "rank", "round", "row_number",
        "string_agg", "array_agg", "substring", "sum", "trim", "upper",
    ],
    constants: &["null", "true", "false", "unknown"],
    // Table names follow these words.
    type_keywords: &["table", "into", "from", "join", "update", "exists"],
    sigils: &[(b'@', Kind::Variable)],
    operators: "+-*/%=&|^!<>~?:",
    case_insensitive: true,
    features: CALLS | MEMBERS,
    hook: Some(sql_hook),
    hook_chars: "$[:",
    ..Spec::DEFAULT
};

pub static SQL: Definition = Definition::new("sql", "SQL", Engine::Generic(&SQL_SPEC))
    .aliases(&["mysql", "postgres", "postgresql", "pgsql", "plpgsql", "sqlite", "tsql", "plsql", "mssql", "mariadb"])
    .extensions(&["sql", "ddl", "dml", "pgsql", "psql", "tsql"]);

// GraphQL

const GRAPHQL_KEYWORDS: &[&str] = &[
    "query", "mutation", "subscription", "fragment", "on", "extend", "schema", "directive", "implements", "repeatable",
];

const GRAPHQL_TYPE_KEYWORDS: &[&str] = &["type", "interface", "union", "enum", "input", "scalar"];

/// Lowercase names are fields (or arguments) unless they are keywords or
/// calls. A name followed by `:` is always a field, even `type:`, and so is
/// `type` in a selection set, where no type name follows it.
fn graphql_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let end = lx.scan_ident(start);
    let word = &lx.ctx.src[start..end];
    let next = lx.ctx.next_non_space(end);
    if next != b':' {
        let reserved = |list: &[&str]| list.iter().any(|w| w.as_bytes() == word);
        if next == b'('
            || reserved(GRAPHQL_KEYWORDS)
            || (reserved(GRAPHQL_TYPE_KEYWORDS) && next.is_ascii_uppercase())
            || reserved(&["true", "false", "null"])
        {
            return false;
        }
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Property);
    lx.mark_value();
    true
}

static GRAPHQL_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"\"\"").multiline(), Str::quoted("\"")],
    keywords: GRAPHQL_KEYWORDS,
    types: &["Int", "Float", "String", "Boolean", "ID"],
    constants: &["true", "false", "null"],
    type_keywords: GRAPHQL_TYPE_KEYWORDS,
    sigils: &[(b'$', Kind::Variable)],
    operators: "=!|&",
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | ANNOTATIONS,
    hook: Some(graphql_hook),
    hook_chars: "abcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static GRAPHQL: Definition = Definition::new("graphql", "GraphQL", Engine::Generic(&GRAPHQL_SPEC))
    .aliases(&["gql"])
    .extensions(&["graphql", "gql", "graphqls"]);

// Diff

pub static DIFF: Definition = Definition::new("diff", "Diff", Engine::Custom(diff))
    .aliases(&["patch", "udiff"])
    .extensions(&["diff", "patch", "rej"]);

/// Extended header lines written by `git diff` and `diff -r`.
const DIFF_META: &[&[u8]] = &[
    b"index ", b"new file mode", b"deleted file mode", b"old mode", b"new mode", b"similarity index", b"rename from",
    b"rename to", b"copy from", b"copy to", b"Binary files", b"Only in",
];

fn diff(ctx: &mut Ctx) {
    while !ctx.done() {
        let start = ctx.pos;
        let end = ctx.line_end(start);
        let line = &ctx.src[start..end];
        let next = if end < ctx.len() { &ctx.src[end + 1..ctx.line_end(end + 1)] } else { &[][..] };
        let kind = match line.first().copied().unwrap_or(0) {
            // File headers come in pairs: `--- a/file` then `+++ b/file`.
            b'-' if line.starts_with(b"--- ") && next.starts_with(b"+++ ") => {
                ctx.emit(start, end, Kind::Heading);
                ctx.emit(end + 1, end + 1 + next.len(), Kind::Heading);
                ctx.pos = end + 1 + next.len();
                if ctx.peek() == b'\n' {
                    ctx.pos += 1;
                }
                continue;
            }
            b'*' if line.starts_with(b"*** ") && next.starts_with(b"--- ") => Kind::Heading,
            b'+' | b'>' => Kind::Inserted,
            b'-' | b'<' => Kind::Deleted,
            b'!' => Kind::Constant,
            b'@' if line.starts_with(b"@@") => {
                // `@@ -1,3 +1,4 @@ fn context`: the trailing context stays text.
                let close = ctx.find(start + 2, b"@@").filter(|&p| p < end).map_or(end, |p| p + 2);
                ctx.emit(start, close, Kind::Meta);
                ctx.pos = end;
                if ctx.peek() == b'\n' {
                    ctx.pos += 1;
                }
                continue;
            }
            b'\\' => Kind::Comment,
            b'*' if line.iter().all(|&b| b == b'*' || b == b'\r') && line.len() > 3 => Kind::Meta,
            b'=' if line.iter().all(|&b| b == b'=' || b == b'\r') => Kind::Meta,
            _ if line.starts_with(b"diff ") || line.starts_with(b"Index: ") => Kind::Heading,
            _ if DIFF_META.iter().any(|p| line.starts_with(p)) => Kind::Meta,
            _ => Kind::Text,
        };
        ctx.emit(start, end, kind);
        ctx.pos = end;
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

// Protocol Buffers

/// `name =` is a field, option or enum value name.
fn assignment_key_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let end = lx.scan_ident(start);
    let eq = lx.ctx.skip_spaces_from(end);
    if lx.ctx.at(eq) != b'='
        || matches!(lx.ctx.at(eq + 1), b'=' | b'>' | b'~')
        || matches!(&lx.ctx.src[start..end], b"syntax" | b"edition")
    {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Property);
    true
}

static PROTOBUF_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "syntax", "edition", "package", "import", "option", "returns", "stream", "oneof", "map", "repeated",
        "optional", "required", "reserved", "extensions", "to", "max", "public", "weak", "group",
    ],
    types: &[
        "double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32",
        "sfixed64", "bool", "string", "bytes",
    ],
    constants: &["true", "false", "inf", "nan"],
    function_keywords: &["rpc"],
    type_keywords: &["message", "enum", "service", "extend"],
    operators: "=-+<>",
    features: CAPITALIZED_TYPES | UPPER_CONSTANTS,
    hook: Some(assignment_key_hook),
    hook_chars: "abcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static PROTOBUF: Definition = Definition::new("protobuf", "Protocol Buffers", Engine::Generic(&PROTOBUF_SPEC))
    .aliases(&["proto", "proto3"])
    .extensions(&["proto"]);

// Thrift

static THRIFT_SPEC: Spec = Spec {
    line_comments: &["//", "#"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "namespace", "include", "cpp_include", "const", "required", "optional", "oneway", "extends", "throws", "async",
    ],
    types: &[
        "bool", "byte", "i8", "i16", "i32", "i64", "double", "string", "binary", "list", "set", "map", "void", "uuid",
    ],
    constants: &["true", "false"],
    type_keywords: &["struct", "union", "exception", "enum", "service", "typedef", "senum"],
    operators: "=-+<>*",
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS,
    ..Spec::DEFAULT
};

pub static THRIFT: Definition =
    Definition::new("thrift", "Thrift", Engine::Generic(&THRIFT_SPEC)).extensions(&["thrift"]);

// HCL and Terraform

/// Attribute and object keys (`name =`), nested block types at the start of
/// a line (`lifecycle {`), and heredocs (`<<EOF`, `<<-EOF`).
fn hcl_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    if lx.ctx.peek() != b'<' {
        if assignment_key_hook(lx) {
            return true;
        }
        let end = lx.scan_ident(start);
        if !lx.ctx.at_line_start(start) || !matches!(lx.ctx.next_non_space(end), b'{' | b'"') {
            return false;
        }
        lx.ctx.pos = end;
        lx.ctx.emit(start, end, Kind::Keyword);
        return true;
    }
    let ctx = &mut *lx.ctx;
    if !ctx.starts_with(b"<<") {
        return false;
    }
    let mut i = start + 2;
    let indented = ctx.at(i) == b'-';
    i += indented as usize;
    let word_start = i;
    while is_ident(ctx.at(i)) {
        i += 1;
    }
    if i == word_start || !matches!(ctx.at(ctx.skip_spaces_from(i)), b'\n' | b'\r') {
        return false;
    }
    let delimiter = &ctx.src[word_start..i];
    ctx.emit(start, i, Kind::Operator);
    let body = ctx.line_end(i);
    let mut pos = body + 1;
    let mut body_end = ctx.len();
    let mut term_end = ctx.len();
    while pos < ctx.len() {
        let end = ctx.line_end(pos);
        let line = ctx.src[pos..end].trim_ascii();
        if line == delimiter {
            body_end = pos;
            term_end = end;
            break;
        }
        pos = end + 1;
    }
    ctx.emit(body, body_end, Kind::String);
    ctx.emit(body_end, term_end, Kind::Operator);
    ctx.pos = term_end;
    lx.mark_value();
    true
}

static HCL_SPEC: Spec = Spec {
    line_comments: &["#", "//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").interp(Interp::DollarBrace)],
    keywords: &[
        "resource", "data", "variable", "output", "locals", "module", "provider", "terraform", "backend",
        "provisioner", "connection", "lifecycle", "dynamic", "content", "moved", "import", "check", "removed", "for",
        "in", "if", "else", "endif", "endfor",
    ],
    types: &["string", "number", "bool", "list", "map", "set", "object", "tuple", "any"],
    variables: &["var", "local", "each", "count", "self", "path"],
    constants: &["true", "false", "null"],
    ident_chars: "-",
    operators: "+-*/%=&|!<>?:",
    features: CALLS | MEMBERS,
    hook: Some(hcl_hook),
    hook_chars: "<abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_",
    ..Spec::DEFAULT
};

pub static HCL: Definition = Definition::new("hcl", "HCL", Engine::Generic(&HCL_SPEC))
    .aliases(&["terraform", "tf", "opentofu"])
    .extensions(&["tf", "tfvars", "hcl", "nomad"])
    .filenames(&[".terraformrc", "terraform.rc"]);

// Prisma

/// Field names: a lowercase word that starts a line and is followed by a type.
fn prisma_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    if !lx.ctx.at_line_start(start) {
        return false;
    }
    let end = lx.scan_ident(start);
    let next = lx.ctx.next_non_space(end);
    if end == lx.ctx.skip_spaces_from(end) || !is_ident_start(next) {
        return false;
    }
    if matches!(&lx.ctx.src[start..end], b"model" | b"enum" | b"type" | b"view" | b"datasource" | b"generator") {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Property);
    true
}

static PRISMA_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    keywords: &["datasource", "generator"],
    types: &["String", "Int", "BigInt", "Float", "Decimal", "Boolean", "DateTime", "Json", "Bytes", "Unsupported"],
    builtins: &["autoincrement", "now", "uuid", "cuid", "env", "dbgenerated", "auto", "sequence"],
    constants: &["true", "false", "null"],
    type_keywords: &["model", "enum", "type", "view"],
    sigils: &[(b'@', Kind::Meta)],
    operators: "=?!",
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | LINE_KEYS | IDENT_KEYS,
    hook: Some(prisma_hook),
    hook_chars: "abcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static PRISMA: Definition =
    Definition::new("prisma", "Prisma", Engine::Generic(&PRISMA_SPEC)).extensions(&["prisma"]);

// HTTP

pub static HTTP: Definition = Definition::new("http", "HTTP", Engine::Custom(http))
    .aliases(&["https", "rest", "restclient"])
    .extensions(&["http", "rest"]);

const HTTP_METHODS: &[&[u8]] =
    &[b"GET", b"POST", b"PUT", b"PATCH", b"DELETE", b"HEAD", b"OPTIONS", b"CONNECT", b"TRACE", b"QUERY"];

/// HTTP messages, also in the `.http` file format with `###` separators,
/// comments and `{{variables}}`.
fn http(ctx: &mut Ctx) {
    while !ctx.done() {
        // Comments, separators, blank lines and `@name = value` definitions.
        let line_start = ctx.pos;
        let line_end = ctx.line_end(line_start);
        let at = ctx.skip_spaces_from(line_start);
        ctx.pos = at;
        if ctx.peek() == b'#' || ctx.starts_with(b"//") {
            line_comment(ctx);
        } else if ctx.peek() == b'@' {
            let mut e = at + 1;
            while is_ident(ctx.at(e)) || ctx.at(e) == b'-' {
                e += 1;
            }
            ctx.emit(at, e, Kind::Variable);
            ctx.pos = ctx.skip_spaces_from(e);
            if ctx.peek() == b'=' {
                ctx.token(1, Kind::Operator);
                ctx.skip_spaces();
            }
            http_text(ctx, line_end, Kind::String);
        } else if at < line_end && !matches!(ctx.peek(), b'\r') {
            http_message(ctx, line_end);
            continue;
        }
        ctx.pos = line_end;
        if ctx.peek() == b'\n' {
            ctx.pos += 1;
        }
    }
}

fn http_message(ctx: &mut Ctx, line_end: usize) {
    // Start line: `METHOD target HTTP/1.1` or `HTTP/1.1 200 OK`.
    let at = ctx.pos;
    let word_end = (at..line_end).find(|&i| matches!(ctx.src[i], b' ' | b'\t' | b'\r')).unwrap_or(line_end);
    let word = &ctx.src[at..word_end];
    if word.starts_with(b"HTTP/") {
        ctx.emit(at, word_end, Kind::Type);
        ctx.pos = ctx.skip_spaces_from(word_end);
        let s = ctx.pos;
        while ctx.peek().is_ascii_digit() {
            ctx.pos += 1;
        }
        ctx.emit_from(s, Kind::Number);
        ctx.pos = ctx.skip_spaces_from(ctx.pos);
        let e = trim_end(ctx.src, ctx.pos, line_end);
        ctx.emit(ctx.pos, e, Kind::String);
    } else {
        let method = HTTP_METHODS.contains(&word);
        let target = if method {
            ctx.emit(at, word_end, Kind::Keyword);
            ctx.skip_spaces_from(word_end)
        } else {
            at
        };
        ctx.pos = target;
        let mut target_end = trim_end(ctx.src, target, line_end);
        if let Some(v) = ctx.find(target, b" HTTP/").filter(|&v| v < line_end) {
            target_end = v;
        }
        http_text(ctx, target_end, Kind::String);
        if target_end < line_end {
            let s = ctx.skip_spaces_from(target_end);
            ctx.emit(s, trim_end(ctx.src, s, line_end), Kind::Type);
        }
    }
    ctx.pos = line_end;
    // Headers until a blank line.
    let mut content_type: &[u8] = b"";
    while ctx.peek() == b'\n' {
        let s = ctx.pos + 1;
        let e = ctx.line_end(s);
        if ctx.src[s..e].trim_ascii().is_empty() {
            ctx.pos = e;
            break;
        }
        if ctx.at(s) == b'#' || ctx.starts_with_at(s, b"//") {
            ctx.pos = s;
            line_comment(ctx);
            continue;
        }
        // Query continuation lines in `.http` files: `    &page=2`.
        let Some(colon) = ctx.src[s..e].iter().position(|&b| b == b':').map(|i| s + i) else {
            ctx.pos = ctx.skip_spaces_from(s);
            http_text(ctx, e, Kind::String);
            ctx.pos = e;
            continue;
        };
        let name = &ctx.src[s..colon];
        ctx.emit(s, colon, Kind::Property);
        ctx.emit(colon, colon + 1, Kind::Punctuation);
        ctx.pos = ctx.skip_spaces_from(colon + 1);
        if name.eq_ignore_ascii_case(b"content-type") {
            content_type = &ctx.src[ctx.pos..e];
        }
        http_text(ctx, e, Kind::String);
        ctx.pos = e;
    }
    // Body until the next `###` separator.
    if ctx.peek() != b'\n' {
        return;
    }
    let body = ctx.pos + 1;
    let mut end = body;
    while end < ctx.len() && !ctx.starts_with_at(end, b"###") {
        end = ctx.line_end(end);
        if end < ctx.len() {
            end += 1;
        }
    }
    let first = ctx.src[body..end].iter().find(|b| !b.is_ascii_whitespace()).copied().unwrap_or(0);
    let has = |s: &[u8]| content_type.windows(s.len()).any(|w| w.eq_ignore_ascii_case(s));
    let language = if has(b"json") || (content_type.is_empty() && matches!(first, b'{' | b'[')) {
        Some("json")
    } else if has(b"html") {
        Some("html")
    } else if has(b"xml") || (content_type.is_empty() && first == b'<') {
        Some("xml")
    } else if has(b"graphql") {
        Some("graphql")
    } else if has(b"javascript") {
        Some("javascript")
    } else if has(b"css") {
        Some("css")
    } else if has(b"yaml") {
        Some("yaml")
    } else {
        None
    };
    match language {
        Some(id) => ctx.embed(lang(id), body, end),
        None => ctx.pos = end,
    }
    ctx.pos = end;
}

/// Text up to `end` with `{{variable}}` references.
fn http_text(ctx: &mut Ctx, end: usize, kind: Kind) {
    let mut seg = ctx.pos;
    while ctx.pos < end {
        if ctx.starts_with(b"{{") {
            if let Some(close) = ctx.find(ctx.pos + 2, b"}}").filter(|&c| c < end) {
                ctx.emit_from(seg, kind);
                ctx.emit(ctx.pos, close + 2, Kind::Variable);
                ctx.pos = close + 2;
                seg = ctx.pos;
                continue;
            }
        }
        ctx.pos += 1;
    }
    let e = trim_end(ctx.src, seg, end);
    ctx.emit(seg, e, kind);
    ctx.pos = end;
}

/// Retags the identifier that ends at `end` as `kind`. Returns false if there
/// is no identifier there.
fn retag_word(lx: &mut Lexer, end: usize, kind: Kind) -> bool {
    let mut start = end;
    while start > 0 && lx.is_ident_byte(lx.ctx.src[start - 1]) {
        start -= 1;
    }
    if start == end || lx.ctx.src[start].is_ascii_digit() {
        return false;
    }
    lx.ctx.truncate_to(start);
    lx.ctx.emit(start, end, kind);
    true
}

// Cypher

/// Labels and relationship types (`:Person`, `[:KNOWS]`) and map keys (`{name: ...}`).
fn cypher_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let next = lx.ctx.peek_at(1);
    if is_ident_start(next) || next == b'`' {
        let end = if next == b'`' {
            let line_end = lx.ctx.line_end(start);
            lx.ctx.src[start + 2..line_end].iter().position(|&b| b == b'`').map_or(line_end, |i| start + 2 + i + 1)
        } else {
            lx.scan_ident(start + 1)
        };
        lx.ctx.emit(start, start + 1, Kind::Punctuation);
        lx.ctx.emit(start + 1, end, Kind::Type);
        lx.ctx.pos = end;
        lx.mark_value();
        return true;
    }
    if matches!(next, b' ' | b'\t') && retag_word(lx, start, Kind::Property) {
        lx.ctx.token(1, Kind::Punctuation);
        lx.mark_operator();
        return true;
    }
    false
}

static CYPHER_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("'"), Str::quoted("\""), Str::quoted("`").raw().kind(Kind::Property)],
    keywords: &[
        "match", "optional", "where", "return", "with", "create", "merge", "delete", "detach", "set", "remove",
        "unwind", "order", "by", "skip", "limit", "as", "and", "or", "not", "xor", "in", "is", "distinct", "case",
        "when", "then", "else", "end", "on", "call", "yield", "union", "all", "asc", "desc", "ascending", "descending",
        "starts", "ends", "contains", "exists", "foreach", "load", "csv", "from", "headers", "constraint", "index",
        "unique", "drop", "using", "periodic", "commit", "explain", "profile", "assert", "require", "for", "if", "key",
        "node", "transactions", "any", "none", "single",
    ],
    builtins: &[
        "count", "collect", "size", "length", "coalesce", "id", "labels", "type", "keys", "nodes", "relationships",
        "properties", "tolower", "toupper", "tostring", "tointeger", "tofloat", "range", "head", "last", "tail", "sum",
        "avg", "min", "max", "timestamp", "date", "datetime", "duration", "point", "split", "replace", "substring",
        "trim", "elementid", "startnode", "endnode", "shortestpath", "allshortestpaths",
    ],
    constants: &["true", "false", "null"],
    sigils: &[(b'$', Kind::Variable)],
    operators: "+-*/%=<>|^!~:",
    case_insensitive: true,
    features: CALLS | MEMBERS,
    hook: Some(cypher_hook),
    hook_chars: ":",
    ..Spec::DEFAULT
};

pub static CYPHER: Definition = Definition::new("cypher", "Cypher", Engine::Generic(&CYPHER_SPEC))
    .aliases(&["neo4j", "opencypher"])
    .extensions(&["cypher", "cyp"]);

// SPARQL

/// IRIs (`<http://...>`), prefixed names (`foaf:name`) and blank nodes (`_:b0`).
fn sparql_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    if ctx.peek() == b'<' {
        let line_end = ctx.line_end(start);
        let mut i = start + 1;
        while i < line_end && !matches!(ctx.src[i], b'>' | b' ' | b'\t' | b'<' | b'"' | b'{' | b'}' | b'\r') {
            i += 1;
        }
        if ctx.at(i) != b'>' {
            return false;
        }
        ctx.pos = i + 1;
        ctx.emit(start, i + 1, Kind::String);
        lx.mark_value();
        return true;
    }
    let name = |ctx: &Ctx, mut i: usize| {
        while is_ident(ctx.at(i)) || ctx.at(i) == b'-' || (ctx.at(i) == b'.' && is_ident(ctx.at(i + 1))) {
            i += 1;
        }
        i
    };
    let colon = name(ctx, start);
    if ctx.at(colon) != b':' {
        return false;
    }
    let end = name(ctx, colon + 1);
    if &ctx.src[start..colon] == b"_" {
        ctx.emit(start, end, Kind::Variable);
    } else {
        ctx.emit(start, colon + 1, Kind::Type);
        ctx.emit(colon + 1, end, Kind::Property);
    }
    ctx.pos = end;
    lx.mark_value();
    true
}

static SPARQL_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"\"\"").multiline(), Str::quoted("'''").multiline(), Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "select", "construct", "describe", "ask", "where", "from", "named", "prefix", "base", "optional", "filter",
        "union", "minus", "graph", "service", "bind", "values", "order", "by", "group", "having", "limit", "offset",
        "distinct", "reduced", "as", "asc", "desc", "not", "exists", "in", "insert", "delete", "data", "with", "using",
        "load", "clear", "drop", "create", "add", "move", "copy", "silent", "default", "all", "into", "a", "undef",
    ],
    builtins: &[
        "str", "lang", "langmatches", "datatype", "bound", "iri", "uri", "bnode", "regex", "contains", "strstarts",
        "strends", "strlen", "substr", "count", "sum", "min", "max", "avg", "sample", "group_concat", "concat",
        "ucase", "lcase", "now", "year", "month", "day", "coalesce", "if", "isiri", "isuri", "isblank", "isliteral",
        "isnumeric", "replace", "abs", "round",
    ],
    constants: &["true", "false"],
    sigils: &[(b'?', Kind::Variable), (b'$', Kind::Variable), (b'@', Kind::Meta)],
    operators: "=!<>&|+-*/^",
    case_insensitive: true,
    features: CALLS,
    hook: Some(sparql_hook),
    hook_chars: "<:_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static SPARQL: Definition =
    Definition::new("sparql", "SPARQL", Engine::Generic(&SPARQL_SPEC)).aliases(&["rq"]).extensions(&["sparql", "rq"]);

// KDL

pub static KDL: Definition = Definition::new("kdl", "KDL", Engine::Custom(kdl)).extensions(&["kdl"]);

/// Bytes that end a bare KDL identifier or value.
fn kdl_delimiter(b: u8) -> bool {
    is_blank(b) || matches!(b, b'(' | b')' | b'{' | b'}' | b'[' | b']' | b'/' | b'\\' | b'"' | b';' | b'=' | b'#')
}

fn kdl(ctx: &mut Ctx) {
    // Whether the next word is a node name.
    let mut node = true;
    while !ctx.done() {
        let start = ctx.pos;
        match ctx.peek() {
            b'\n' => {
                ctx.pos += 1;
                node = true;
            }
            b' ' | b'\t' | b'\r' => ctx.pos += 1,
            b';' | b'{' | b'}' => {
                ctx.token(1, Kind::Punctuation);
                node = true;
            }
            b'/' if ctx.peek_at(1) == b'/' => line_comment(ctx),
            b'/' if ctx.peek_at(1) == b'*' => crate::lexer::generic::block_comment(ctx, "/*", "*/", true),
            b'/' if ctx.peek_at(1) == b'-' => kdl_slashdash(ctx, node),
            b'(' => {
                // Type annotation: `(u8)`, `(date)"2024-01-01"`.
                let line_end = ctx.line_end(start);
                let close = ctx.src[start..line_end].iter().position(|&b| b == b')').map(|i| start + i);
                ctx.emit(start, start + 1, Kind::Punctuation);
                ctx.emit(start + 1, close.unwrap_or(line_end), Kind::Type);
                ctx.pos = line_end;
                if let Some(close) = close {
                    ctx.emit(close, close + 1, Kind::Punctuation);
                    ctx.pos = close + 1;
                }
            }
            b'=' => ctx.token(1, Kind::Operator),
            b'\\' => {
                // Line continuation: the node goes on after the newline.
                ctx.token(1, Kind::Operator);
                ctx.skip_spaces();
                if ctx.starts_with(b"//") {
                    line_comment(ctx);
                }
                ctx.skip_spaces();
                if ctx.peek() == b'\r' {
                    ctx.pos += 1;
                }
                if ctx.peek() == b'\n' {
                    ctx.pos += 1;
                }
            }
            b'"' => {
                if ctx.starts_with(b"\"\"\"") {
                    delimited(ctx, 3, b"\"\"\"", true, Kind::String);
                } else {
                    quoted(ctx, Kind::String, ESC | MULTI);
                }
                kdl_retag(ctx, start, node);
                node = false;
            }
            b'#' | b'r' if kdl_raw_string(ctx) => {
                kdl_retag(ctx, start, node);
                node = false;
            }
            b'#' => {
                // `#true`, `#false`, `#null`, `#inf`, `#-inf`, `#nan`.
                let mut end = start + 1;
                while !kdl_delimiter(ctx.at(end)) {
                    end += 1;
                }
                ctx.pos = end;
                ctx.emit(start, end, Kind::Constant);
                node = false;
            }
            _ => {
                let mut end = start;
                while end < ctx.len() && !kdl_delimiter(ctx.at(end)) {
                    end += 1;
                }
                if end == start {
                    ctx.bump_char();
                    continue;
                }
                let word = &ctx.src[start..end];
                let kind = if ctx.at(end) == b'=' {
                    Kind::Attribute
                } else if node {
                    Kind::Tag
                } else if matches!(word, b"true" | b"false" | b"null") {
                    Kind::Constant
                } else if is_number(word) {
                    Kind::Number
                } else {
                    Kind::String
                };
                ctx.pos = end;
                ctx.emit(start, end, kind);
                node = false;
            }
        }
    }
}

/// Raw strings: `r"..."`, `r#"..."#` (KDL 1) and `#"..."#` (KDL 2).
fn kdl_raw_string(ctx: &mut Ctx) -> bool {
    let start = ctx.pos;
    let mut i = start + (ctx.peek() == b'r') as usize;
    let hashes = ctx.src[i..].iter().take_while(|&&b| b == b'#').count();
    i += hashes;
    if ctx.at(i) != b'"' || (hashes == 0 && ctx.peek() != b'r') {
        return false;
    }
    let mut close = Vec::with_capacity(hashes + 1);
    close.push(b'"');
    close.resize(hashes + 1, b'#');
    let end = ctx.find(i + 1, &close).map_or(ctx.len(), |p| p + close.len());
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    true
}

/// A string used as a node name or property key.
fn kdl_retag(ctx: &mut Ctx, start: usize, node: bool) {
    let kind = if ctx.peek() == b'=' {
        Kind::Attribute
    } else if node {
        Kind::Tag
    } else {
        return;
    };
    let end = ctx.pos;
    ctx.truncate_to(start);
    ctx.emit(start, end, kind);
}

/// `/-` comments out the next node, argument, property or children block.
fn kdl_slashdash(ctx: &mut Ctx, node: bool) {
    let start = ctx.pos;
    let mut i = ctx.skip_spaces_from(start + 2);
    let mut depth = 0u32;
    while i < ctx.len() {
        match ctx.src[i] {
            b'{' => depth += 1,
            b'}' if depth == 0 => break,
            b'}' => {
                depth -= 1;
                if depth == 0 && !node {
                    i += 1;
                    break;
                }
            }
            b'\n' | b';' if depth == 0 && node => break,
            b' ' | b'\t' | b'\n' | b';' if depth == 0 && !node => break,
            _ => {}
        }
        i += 1;
    }
    ctx.pos = i.max(start + 2);
    ctx.emit(start, ctx.pos, Kind::Comment);
}

// CUE

/// Raw strings: `#"..."#`, `##"..."##` and their multi-line forms.
fn cue_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let hashes = ctx.src[start..].iter().take_while(|&&b| b == b'#').count();
    let open = start + hashes;
    if ctx.at(open) != b'"' {
        return false;
    }
    let quotes = if ctx.starts_with_at(open, b"\"\"\"") { 3 } else { 1 };
    let mut close = vec![b'"'; quotes];
    close.resize(quotes + hashes, b'#');
    let end = ctx.find(open + quotes, &close).map_or(ctx.len(), |p| p + close.len());
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    lx.mark_value();
    true
}

static CUE_SPEC: Spec = Spec {
    line_comments: &["//"],
    strings: &[
        Str::quoted("\"\"\"").multiline().interp(Interp::BackslashParen),
        Str::quoted("'''").multiline().interp(Interp::BackslashParen),
        Str::quoted("\"").interp(Interp::BackslashParen),
        Str::quoted("'").interp(Interp::BackslashParen),
    ],
    keywords: &["package", "import", "for", "in", "if", "let"],
    types: &[
        "string", "int", "float", "bool", "bytes", "number", "uint", "rune", "int8", "int16", "int32", "int64",
        "int128", "uint8", "uint16", "uint32", "uint64", "uint128", "float32", "float64",
    ],
    builtins: &["len", "close", "and", "or", "div", "mod", "quo", "rem"],
    constants: &["true", "false", "null", "_"],
    ident_suffix: "?!",
    sigils: &[(b'#', Kind::Type)],
    operators: "+-*/=!<>&|?~",
    features: CALLS | MEMBERS | IDENT_KEYS | STRING_KEYS | ANNOTATIONS,
    hook: Some(cue_hook),
    hook_chars: "#",
    ..Spec::DEFAULT
};

pub static CUE: Definition = Definition::new("cue", "CUE", Engine::Generic(&CUE_SPEC)).extensions(&["cue"]);

// Jsonnet

static JSONNET_VERBATIM: [Str; 2] =
    [Str::quoted("\"").raw().doubled().multiline(), Str::quoted("'").raw().doubled().multiline()];

/// The root object `$`, verbatim strings (`@"..."`) and field separators
/// (`name::`, `name+:`), which mark the field name as a property.
fn jsonnet_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    match lx.ctx.peek() {
        b'$' => {
            lx.ctx.token(1, Kind::Variable);
            lx.mark_value();
            true
        }
        b'@' => {
            let rule = match lx.ctx.peek_at(1) {
                b'"' => &JSONNET_VERBATIM[0],
                b'\'' => &JSONNET_VERBATIM[1],
                _ => return false,
            };
            lx.ctx.pos += 1;
            lx.string(start, rule, true, Interp::None);
            true
        }
        b => {
            let plus = (b == b'+') as usize;
            let mut end = start + plus;
            while lx.ctx.at(end) == b':' && end - start - plus < 3 {
                end += 1;
            }
            let colons = end - start - plus;
            if colons == 0 || (colons == 1 && plus == 0) || !retag_word(lx, start, Kind::Property) {
                return false;
            }
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Punctuation);
            lx.mark_operator();
            true
        }
    }
}

static JSONNET_SPEC: Spec = Spec {
    line_comments: &["//", "#"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::new("|||", "|||").raw().multiline(), Str::quoted("\"").multiline(), Str::quoted("'").multiline()],
    keywords: &[
        "local", "function", "if", "then", "else", "for", "in", "import", "importstr", "importbin", "assert", "error",
        "tailstrict",
    ],
    variables: &["self", "super"],
    builtins: &["std"],
    constants: &["true", "false", "null"],
    operators: "+-*/%=&|^!<>~",
    features: CALLS | MEMBERS | IDENT_KEYS | STRING_KEYS,
    hook: Some(jsonnet_hook),
    hook_chars: "$@+:",
    ..Spec::DEFAULT
};

pub static JSONNET: Definition = Definition::new("jsonnet", "Jsonnet", Engine::Generic(&JSONNET_SPEC))
    .aliases(&["libsonnet"])
    .extensions(&["jsonnet", "libsonnet"]);

// Graphviz DOT

/// Attributes (`shape=box`), HTML labels (`<<b>bold</b>>`) and `#` lines.
fn dot_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    match lx.ctx.peek() {
        b'#' if lx.ctx.at_line_start(start) => {
            line_comment(lx.ctx);
            true
        }
        b'#' => false,
        b'<' => {
            if lx.ctx.prev_non_space(start) != b'=' {
                return false;
            }
            let mut depth = 0u32;
            let mut i = start;
            while i < lx.ctx.len() {
                match lx.ctx.src[i] {
                    b'<' => depth += 1,
                    b'>' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            lx.ctx.pos = i;
            lx.ctx.emit(start, i, Kind::String);
            lx.mark_value();
            true
        }
        _ => {
            let end = lx.scan_ident(start);
            let eq = lx.ctx.skip_spaces_from(end);
            if lx.ctx.at(eq) != b'=' {
                return false;
            }
            lx.ctx.emit(start, end, Kind::Attribute);
            lx.ctx.emit(eq, eq + 1, Kind::Operator);
            let value = lx.ctx.skip_spaces_from(eq + 1);
            if is_ident_start(lx.ctx.at(value)) {
                let value_end = lx.scan_ident(value);
                lx.ctx.emit(value, value_end, Kind::String);
                lx.ctx.pos = value_end;
                lx.mark_value();
            } else {
                lx.ctx.pos = eq + 1;
                lx.mark_operator();
            }
            true
        }
    }
}

static DOT_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").multiline()],
    keywords: &["node", "edge", "strict"],
    type_keywords: &["graph", "digraph", "subgraph"],
    operators: "-=>",
    case_insensitive: true,
    hook: Some(dot_hook),
    hook_chars: "#<abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_",
    ..Spec::DEFAULT
};

pub static DOT: Definition =
    Definition::new("dot", "DOT", Engine::Generic(&DOT_SPEC)).aliases(&["graphviz", "gv"]).extensions(&["dot", "gv"]);

// Mermaid

/// Directives (`%%{init: ...}%%`), node shapes (`A[label]`, `B((circle))`),
/// edge labels (`-->|label|`) and message text after `: `.
fn mermaid_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let line_end = ctx.line_end(start);
    match ctx.peek() {
        b'%' => {
            if !ctx.starts_with(b"%%{") {
                return false;
            }
            let end = ctx.find(start, b"}%%").map_or(line_end, |p| p + 3);
            ctx.pos = end;
            ctx.emit(start, end, Kind::Meta);
        }
        b'[' | b'(' | b'{' | b'>' => {
            if start == 0 || !is_ident(ctx.src[start - 1]) {
                return false;
            }
            let mut open_end = start + 1;
            while open_end - start < 3 && matches!(ctx.at(open_end), b'[' | b'(' | b'{' | b'/' | b'\\') {
                open_end += 1;
            }
            let mut close = open_end;
            let mut in_quotes = false;
            while close < line_end {
                match ctx.src[close] {
                    b'"' => in_quotes = !in_quotes,
                    b']' | b')' | b'}' if !in_quotes => break,
                    _ => {}
                }
                close += 1;
            }
            if close >= line_end {
                return false;
            }
            let mut text_end = close;
            while text_end > open_end && matches!(ctx.src[text_end - 1], b'/' | b'\\') {
                text_end -= 1;
            }
            let mut end = close;
            while end < line_end && end - close < 3 && matches!(ctx.src[end], b']' | b')' | b'}') {
                end += 1;
            }
            ctx.emit(start, open_end, Kind::Punctuation);
            ctx.emit(open_end, text_end, Kind::String);
            ctx.emit(text_end, end, Kind::Punctuation);
            ctx.pos = end;
        }
        b'|' => {
            if start == 0 || !matches!(ctx.src[start - 1], b'>' | b'-' | b'=' | b'.') {
                return false;
            }
            let Some(close) = ctx.src[start + 1..line_end].iter().position(|&b| b == b'|').map(|i| start + 1 + i)
            else {
                return false;
            };
            ctx.emit(start, start + 1, Kind::Punctuation);
            ctx.emit(start + 1, close, Kind::String);
            ctx.emit(close, close + 1, Kind::Punctuation);
            ctx.pos = close + 1;
        }
        b':' => {
            if !matches!(ctx.peek_at(1), b' ' | b'\t') {
                return false;
            }
            let text = ctx.skip_spaces_from(start + 1);
            let text_end = trim_end(ctx.src, text, line_end);
            ctx.emit(start, start + 1, Kind::Punctuation);
            let kind = if is_number(&ctx.src[text..text_end]) { Kind::Number } else { Kind::String };
            ctx.emit(text, text_end, kind);
            ctx.pos = line_end;
        }
        _ => return false,
    }
    lx.mark_value();
    true
}

static MERMAID_SPEC: Spec = Spec {
    line_comments: &["%%"],
    strings: &[Str::quoted("\"").raw()],
    keywords: &[
        "graph", "flowchart", "subgraph", "end", "direction", "TB", "TD", "BT", "RL", "LR", "sequenceDiagram",
        "classDiagram", "stateDiagram", "erDiagram", "gantt", "pie", "journey", "gitGraph", "mindmap", "timeline",
        "quadrantChart", "requirementDiagram", "sankey", "participant", "actor", "as", "loop", "alt", "else", "opt",
        "par", "and", "critical", "break", "rect", "note", "over", "left", "right", "of", "activate", "deactivate",
        "autonumber", "class", "classDef", "style", "linkStyle", "click", "callback", "state", "section", "title",
        "dateFormat", "axisFormat", "excludes", "todayMarker", "commit", "branch", "checkout", "merge", "namespace",
        "accTitle", "accDescr", "showData", "box", "create", "destroy",
    ],
    constants: &["true", "false"],
    operators: "-=.<>~&*+",
    hook: Some(mermaid_hook),
    hook_chars: "%[({>|:",
    ..Spec::DEFAULT
};

pub static MERMAID: Definition = Definition::new("mermaid", "Mermaid", Engine::Generic(&MERMAID_SPEC))
    .aliases(&["mmd"])
    .extensions(&["mmd", "mermaid"]);

// PlantUML

/// Colors (`#Red`), stereotypes (`<<Entity>>`), activities (`:step;`) and
/// message text after `: `.
fn plantuml_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let line_end = ctx.line_end(start);
    match ctx.peek() {
        b'#' => {
            let mut end = start + 1;
            while is_ident(ctx.at(end)) {
                end += 1;
            }
            if end == start + 1 {
                return false;
            }
            ctx.pos = end;
            ctx.emit(start, end, Kind::Constant);
        }
        b'<' if !ctx.starts_with(b"<<") => {
            // Standard library includes: `!include <C4/C4_Container>`.
            let line_start = ctx.skip_spaces_from(start - column(ctx, start));
            let close = ctx.src[start..line_end].iter().position(|&b| b == b'>');
            match close {
                Some(c) if ctx.starts_with_at(line_start, b"!include") => {
                    ctx.pos = start + c + 1;
                    ctx.emit(start, ctx.pos, Kind::String);
                }
                _ => return false,
            }
        }
        b'<' => {
            let Some(close) = ctx.find(start + 2, b">>").filter(|&c| c < line_end) else {
                return false;
            };
            ctx.pos = close + 2;
            ctx.emit(start, ctx.pos, Kind::Meta);
        }
        b':' if ctx.at_line_start(start) => {
            let close =
                ctx.src[start..line_end].iter().position(|&b| matches!(b, b';' | b'|' | b'<')).map(|i| start + i);
            let text_end = close.unwrap_or(line_end);
            ctx.emit(start, start + 1, Kind::Punctuation);
            ctx.emit(start + 1, text_end, Kind::String);
            ctx.pos = text_end;
            if ctx.peek() == b';' {
                ctx.token(1, Kind::Punctuation);
            }
        }
        b':' if matches!(ctx.peek_at(1), b' ' | b'\t') => {
            let text = ctx.skip_spaces_from(start + 1);
            ctx.emit(start, start + 1, Kind::Punctuation);
            ctx.emit(text, trim_end(ctx.src, text, line_end), Kind::String);
            ctx.pos = line_end;
        }
        _ => return false,
    }
    lx.mark_value();
    true
}

static PLANTUML_SPEC: Spec = Spec {
    line_comments: &["'"],
    block_comments: &[("/'", "'/")],
    strings: &[Str::quoted("\"").raw()],
    keywords: &[
        "participant", "actor", "boundary", "control", "entity", "database", "collections", "queue", "class",
        "interface", "abstract", "enum", "annotation", "package", "namespace", "note", "as", "of", "over", "left",
        "right", "top", "bottom", "end", "if", "then", "else", "elseif", "endif", "while", "endwhile", "repeat",
        "fork", "again", "start", "stop", "kill", "detach", "title", "skinparam", "activate", "deactivate", "return",
        "alt", "opt", "loop", "group", "par", "break", "critical", "usecase", "component", "node", "rectangle",
        "state", "object", "hide", "show", "legend", "header", "footer", "newpage", "autonumber", "extends",
        "implements", "partition", "split", "box", "ref", "create", "destroy", "frame", "cloud", "folder", "artifact",
        "card", "storage", "agent", "map", "json", "is", "endlegend", "endnote", "direction", "to",
    ],
    sigils: &[(b'@', Kind::Meta), (b'!', Kind::Meta), (b'$', Kind::Variable)],
    operators: "-<>|*.+=\\/~",
    features: CAPITALIZED_TYPES,
    hook: Some(plantuml_hook),
    hook_chars: "#<:",
    ..Spec::DEFAULT
};

pub static PLANTUML: Definition = Definition::new("plantuml", "PlantUML", Engine::Generic(&PLANTUML_SPEC))
    .aliases(&["puml"])
    .extensions(&["puml", "plantuml", "pu", "iuml", "wsd"]);

// Log files

pub static LOG: Definition = Definition::new("log", "Log", Engine::Custom(log)).aliases(&["logs"]).extensions(&["log"]);

/// The kind used for a log level word, if `word` is one.
fn log_level(word: &[u8]) -> Option<Kind> {
    let mut buf = [0u8; 9];
    if word.len() > buf.len() {
        return None;
    }
    let upper = &mut buf[..word.len()];
    upper.copy_from_slice(word);
    upper.make_ascii_uppercase();
    let kind = match &*upper {
        b"ERROR" | b"FATAL" | b"CRITICAL" => Kind::Deleted,
        b"WARN" | b"WARNING" => Kind::Constant,
        b"INFO" | b"NOTICE" => Kind::Inserted,
        b"DEBUG" | b"TRACE" => Kind::Comment,
        // Abbreviations and rarer levels only count when written in capitals.
        _ if word != upper => return None,
        b"ERR" | b"CRIT" | b"SEVERE" | b"PANIC" | b"EMERG" | b"ALERT" | b"FAILED" | b"FAIL" => Kind::Deleted,
        b"WRN" => Kind::Constant,
        b"INF" => Kind::Inserted,
        b"DBG" | b"VERBOSE" | b"FINE" | b"FINER" | b"FINEST" => Kind::Comment,
        _ => return None,
    };
    // Accept `ERROR`, `Error` and `error`, but not `eRRor`.
    (word == upper || word[1..].iter().all(u8::is_ascii_lowercase)).then_some(kind)
}

fn log(ctx: &mut Ctx) {
    const MONTHS: &[&[u8]] =
        &[b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec"];
    while !ctx.done() {
        let start = ctx.pos;
        let b = ctx.peek();
        match b {
            b'"' => quoted(ctx, Kind::String, ESC),
            b'\'' if start == 0 || !ctx.src[start - 1].is_ascii_alphanumeric() => {
                let line_end = ctx.line_end(start);
                let close = ctx.src[start + 1..line_end].iter().position(|&b| b == b'\'').map(|i| start + 1 + i);
                match close {
                    Some(c) if !ctx.at(c + 1).is_ascii_alphanumeric() => {
                        ctx.pos = c + 1;
                        ctx.emit(start, ctx.pos, Kind::String);
                    }
                    _ => ctx.pos += 1,
                }
            }
            b'[' | b']' | b'(' | b')' | b'{' | b'}' | b'|' => ctx.token(1, Kind::Punctuation),
            _ if b.is_ascii_alphanumeric() || b == b'_' => {
                let word_char =
                    |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-' | b'/' | b'$');
                let mut end = start;
                while word_char(ctx.at(end)) {
                    end += 1;
                }
                // Keep trailing punctuation out of the word: `done.` or `at 10:30:00:`.
                while end > start + 1 && matches!(ctx.at(end - 1), b'.' | b':' | b'-' | b'/') {
                    end -= 1;
                }
                let word = &ctx.src[start..end];
                let kind = if word
                    .iter()
                    .position(|&b| b == b':')
                    .is_some_and(|i| ctx.starts_with_at(start + i, b"://"))
                {
                    // A URL runs to the next space or closing delimiter.
                    while !is_blank(ctx.at(end)) && !matches!(ctx.at(end), b'"' | b'\'' | b'<' | b'>' | b')' | b']') {
                        end += 1;
                    }
                    Kind::Link
                } else if b.is_ascii_digit() {
                    // Dates, times, IP addresses, durations and plain numbers.
                    if ctx.at(end) == b' '
                        && ctx.at(end + 1).is_ascii_digit()
                        && ctx.at(end + 3) == b':'
                        && is_date(word)
                    {
                        end += 1;
                        while word_char(ctx.at(end)) {
                            end += 1;
                        }
                    }
                    // Java style milliseconds: `10:30:00,123`.
                    if ctx.at(end) == b',' && ctx.at(end + 1).is_ascii_digit() && ctx.src[start..end].contains(&b':') {
                        end += 1;
                        while ctx.at(end).is_ascii_digit() {
                            end += 1;
                        }
                    }
                    Kind::Number
                } else if let Some(kind) = log_level(word) {
                    kind
                } else if let Some(kind) = word.iter().rposition(|&b| b == b'.').and_then(|dot| {
                    log_level(&word[dot + 1..])
                        .filter(|_| word[dot + 1..].iter().all(u8::is_ascii_uppercase))
                        .map(|k| (dot, k))
                }) {
                    // `production.ERROR`: only the level is colored.
                    let (dot, kind) = kind;
                    ctx.emit(start, start + dot + 1, Kind::Text);
                    ctx.pos = end;
                    ctx.emit(start + dot + 1, end, kind);
                    continue;
                } else if MONTHS.contains(&word) && ctx.at(end) == b' ' && ctx.next_non_space(end).is_ascii_digit() {
                    Kind::Number
                } else if ctx.at(end) == b'=' {
                    Kind::Property
                } else if matches!(word, b"true" | b"false" | b"null" | b"nil" | b"None" | b"undefined") {
                    Kind::Constant
                } else if (word.ends_with(b"Exception") || word.ends_with(b"Error"))
                    && word.rsplit(|&b| b == b'.').next().is_some_and(|w| w.first().is_some_and(u8::is_ascii_uppercase))
                {
                    Kind::Type
                } else {
                    Kind::Text
                };
                ctx.pos = end;
                ctx.emit(start, end, kind);
            }
            _ => ctx.bump_char(),
        }
    }
}

// Regular expressions

pub static REGEX: Definition = Definition::new("regex", "Regular Expression", Engine::Custom(regex))
    .aliases(&["regexp"])
    .extensions(&["regex", "regexp"]);

/// A pattern on its own, optionally written as `/pattern/flags`.
fn regex(ctx: &mut Ctx) {
    let mut end = ctx.len();
    let mut flags = None;
    if ctx.peek() == b'/' {
        let text_end = trim_end(ctx.src, ctx.pos, ctx.len());
        if let Some(last) = ctx.src[ctx.pos + 1..text_end].iter().rposition(|&b| b == b'/').map(|i| ctx.pos + 1 + i) {
            if ctx.src[last + 1..text_end].iter().all(u8::is_ascii_alphabetic) {
                ctx.token(1, Kind::Punctuation);
                end = last;
                flags = Some(text_end);
            }
        }
    }
    regex_body(ctx, end);
    if let Some(flags_end) = flags {
        ctx.pos = end;
        ctx.token(1, Kind::Punctuation);
        ctx.emit(end + 1, flags_end, Kind::Keyword);
        ctx.pos = flags_end;
    }
}

fn regex_body(ctx: &mut Ctx, end: usize) {
    let mut extended = false;
    // Start of the current character class contents, if inside one.
    let mut class: Option<usize> = None;
    while ctx.pos < end {
        let start = ctx.pos;
        let b = ctx.peek();
        if let Some(class_start) = class {
            match b {
                b'\\' => regex_escape(ctx, end),
                b']' if start > class_start => {
                    ctx.token(1, Kind::Punctuation);
                    class = None;
                }
                b'[' if ctx.peek_at(1) == b':' => {
                    let close = ctx.find(start + 2, b":]").filter(|&c| c < end).map_or(start + 1, |c| c + 2);
                    ctx.pos = close;
                    ctx.emit_from(start, Kind::Builtin);
                }
                b'-' if start > class_start && ctx.peek_at(1) != b']' => ctx.token(1, Kind::Operator),
                _ => {
                    ctx.bump_char();
                    ctx.emit_from(start, Kind::String);
                }
            }
            continue;
        }
        match b {
            b'\\' => regex_escape(ctx, end),
            b'[' => {
                ctx.token(1, Kind::Punctuation);
                if ctx.peek() == b'^' {
                    ctx.token(1, Kind::Operator);
                }
                class = Some(ctx.pos);
            }
            b'(' => {
                ctx.token(1, Kind::Punctuation);
                if ctx.peek() == b'?' {
                    extended |= regex_group(ctx, end);
                }
            }
            b')' => ctx.token(1, Kind::Punctuation),
            b'*' | b'+' | b'?' => {
                ctx.pos += 1;
                if matches!(ctx.peek(), b'?' | b'+') && ctx.pos < end {
                    ctx.pos += 1;
                }
                ctx.emit_from(start, Kind::Operator);
            }
            b'{' => {
                let mut i = start + 1;
                while i < end && (ctx.src[i].is_ascii_digit() || ctx.src[i] == b',') {
                    i += 1;
                }
                if i > start + 1 && ctx.at(i) == b'}' && i < end {
                    ctx.pos = i + 1;
                    if ctx.pos < end && matches!(ctx.peek(), b'?' | b'+') {
                        ctx.pos += 1;
                    }
                    ctx.emit_from(start, Kind::Operator);
                } else {
                    ctx.pos += 1;
                }
            }
            b'|' => ctx.token(1, Kind::Operator),
            b'^' | b'$' => ctx.token(1, Kind::Keyword),
            b'.' => ctx.token(1, Kind::Builtin),
            b'#' if extended => {
                ctx.pos = ctx.line_end(start).min(end);
                ctx.emit_from(start, Kind::Comment);
            }
            _ => ctx.bump_char(),
        }
    }
    ctx.pos = end;
}

/// Group syntax after `(`: `?:`, `?=`, `?<name>`, `?P<name>`, `?#comment`
/// and inline flags. Returns true when the `x` (extended) flag is turned on.
fn regex_group(ctx: &mut Ctx, end: usize) -> bool {
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match next {
        b'#' => {
            let close = ctx.src[start..end].iter().position(|&b| b == b')').map_or(end, |i| start + i);
            ctx.pos = close;
            ctx.emit_from(start, Kind::Comment);
            false
        }
        b':' | b'=' | b'!' | b'>' | b'|' => {
            ctx.token(2, Kind::Keyword);
            false
        }
        b'<' if matches!(ctx.peek_at(2), b'=' | b'!') => {
            ctx.token(3, Kind::Keyword);
            false
        }
        b'<' | b'\'' | b'P' | b'&' => {
            // Named groups and references: `(?<name>`, `(?'name'`, `(?P<name>`, `(?P=name)`, `(?&name)`.
            let mut i = start + 2;
            if next == b'P' {
                if !matches!(ctx.at(i), b'<' | b'=' | b'>') {
                    ctx.token(1, Kind::Keyword);
                    return false;
                }
                i += 1;
            }
            ctx.pos = i.min(end);
            ctx.emit_from(start, Kind::Keyword);
            let name = ctx.pos;
            while ctx.pos < end && is_ident(ctx.peek()) {
                ctx.pos += 1;
            }
            ctx.emit_from(name, Kind::Variable);
            if ctx.pos < end && matches!(ctx.peek(), b'>' | b'\'') {
                ctx.token(1, Kind::Keyword);
            }
            false
        }
        _ => {
            // Inline flags: `(?i)`, `(?x-s:`.
            let mut i = start + 1;
            while i < end && (ctx.src[i].is_ascii_alphabetic() || ctx.src[i] == b'-') {
                i += 1;
            }
            if ctx.at(i) == b':' && i < end {
                i += 1;
            }
            let on = &ctx.src[start + 1..i];
            let extended = on.split(|&b| b == b'-').next().is_some_and(|f| f.contains(&b'x'));
            ctx.pos = i;
            ctx.emit_from(start, Kind::Keyword);
            extended
        }
    }
}

/// An escape at the cursor: character classes such as `\d` are builtins,
/// backreferences are variables and everything else is an escape.
fn regex_escape(ctx: &mut Ctx, end: usize) {
    let start = ctx.pos;
    let c = ctx.peek_at(1);
    let until = |ctx: &Ctx, close: u8| {
        ctx.src[start..end].iter().position(|&b| b == close).map_or(start + 2, |i| start + i + 1)
    };
    let (len_end, kind) = match c {
        b'd' | b'D' | b'w' | b'W' | b's' | b'S' | b'b' | b'B' | b'A' | b'z' | b'Z' | b'G' | b'h' | b'H' | b'v'
        | b'V' | b'R' | b'X' | b'K' | b'N' => (start + 2, Kind::Builtin),
        b'p' | b'P' if ctx.at(start + 2) == b'{' => (until(ctx, b'}'), Kind::Builtin),
        b'p' | b'P' => (start + 3, Kind::Builtin),
        b'1'..=b'9' => {
            let mut e = start + 2;
            while ctx.at(e).is_ascii_digit() {
                e += 1;
            }
            (e, Kind::Variable)
        }
        b'k' | b'g' => match ctx.at(start + 2) {
            b'<' => (until(ctx, b'>'), Kind::Variable),
            b'{' => (until(ctx, b'}'), Kind::Variable),
            b'\'' => {
                let close =
                    ctx.src[start + 3..end].iter().position(|&b| b == b'\'').map_or(start + 3, |i| start + 4 + i);
                (close, Kind::Variable)
            }
            _ => (start + 2, Kind::Escape),
        },
        b'x' | b'u' if ctx.at(start + 2) == b'{' => (until(ctx, b'}'), Kind::Escape),
        b'x' | b'u' => {
            let max = if c == b'x' { 2 } else { 4 };
            let mut e = start + 2;
            while e < start + 2 + max && ctx.at(e).is_ascii_hexdigit() {
                e += 1;
            }
            (e, Kind::Escape)
        }
        b'c' => (start + 3, Kind::Escape),
        0 => (start + 1, Kind::Escape),
        _ => {
            ctx.pos = start + 1;
            ctx.bump_char();
            (ctx.pos, Kind::Escape)
        }
    };
    ctx.pos = len_end.min(end).max(start + 1);
    // Never stop inside a multi byte character.
    while ctx.pos < end && (ctx.src[ctx.pos] & 0xC0) == 0x80 {
        ctx.pos += 1;
    }
    ctx.emit_from(start, kind);
}
