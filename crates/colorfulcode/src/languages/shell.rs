//! Shells, build tools and the configuration formats that live next to them.

use super::{Definition, Engine, Language};
use crate::lexer::generic::{features::*, Interp, Lexer, Prefix, Spec, Str};
use crate::lexer::{is_ident, is_ident_start, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[
    &BASH, &POWERSHELL, &FISH, &BATCH, &DOCKERFILE, &MAKEFILE, &CMAKE, &NGINX, &APACHECONF, &AWK, &NIX, &JUST, &MESON,
    &NINJA, &VIM, &GITIGNORE, &GITCONFIG, &GITATTRIBUTES, &SSHCONFIG, &CADDYFILE, &SYSTEMD, &CRONTAB, &PROCFILE,
    &XONSH, &NUSHELL,
];

// Shared helpers

/// Whether only spaces, or one of `seps`, precede `pos` on its line.
fn statement_start(ctx: &Ctx, pos: usize, seps: &[u8]) -> bool {
    let mut i = pos;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    i == 0 || matches!(ctx.src[i - 1], b'\n' | b'\r') || seps.contains(&ctx.src[i - 1])
}

/// End of the run of bytes from `from` that satisfy `f`.
fn scan(ctx: &Ctx, from: usize, f: impl Fn(u8) -> bool) -> usize {
    let mut i = from;
    while i < ctx.len() && f(ctx.src[i]) {
        i += 1;
    }
    i
}

/// End of the character starting at `i`.
fn char_end(ctx: &Ctx, i: usize) -> usize {
    scan(ctx, i + 1, |b| b & 0xC0 == 0x80)
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// Offset of the bracket closing the one at `open`, before `limit`.
fn closing(ctx: &Ctx, open: usize, limit: usize) -> Option<usize> {
    let o = ctx.at(open);
    let c = match o {
        b'(' => b')',
        b'{' => b'}',
        b'[' => b']',
        _ => return None,
    };
    let mut depth = 0;
    for (i, &b) in ctx.src[open..limit.min(ctx.len())].iter().enumerate() {
        if b == o {
            depth += 1;
        } else if b == c {
            depth -= 1;
            if depth == 0 {
                return Some(open + i);
            }
        }
    }
    None
}

/// End of a line, following trailing backslash continuations.
fn logical_line_end(ctx: &Ctx, from: usize) -> usize {
    let mut end = ctx.line_end(from);
    loop {
        let mut i = end;
        if i > from && ctx.at(i - 1) == b'\r' {
            i -= 1;
        }
        if i > from && ctx.at(i - 1) == b'\\' && end < ctx.len() {
            end = ctx.line_end(end + 1);
        } else {
            return end;
        }
    }
}

/// Emits a comment from `start` to the end of its line.
fn line_comment(ctx: &mut Ctx, start: usize) {
    let end = ctx.line_end(start);
    ctx.emit(start, end, Kind::Comment);
    ctx.pos = end;
}

/// `kw name`: emits the keyword as `kinds.0` and the following name as
/// `kinds.1`. Flags between them (`def --env name`, `set -gx name`) are
/// attributes. With `seps`, `kw` must start a statement.
fn keyword_then(lx: &mut Lexer, kw: &[u8], kinds: (Kind, Kind), seps: Option<&[u8]>) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let end = start + kw.len();
    if !ctx.starts_with_ignore_case(kw) || !matches!(ctx.at(end), b' ' | b'\t') {
        return false;
    }
    if let Some(seps) = seps {
        if !statement_start(ctx, start, seps) {
            return false;
        }
    }
    ctx.emit(start, end, kinds.0);
    let mut s = ctx.skip_spaces_from(end);
    while ctx.at(s) == b'-' && (ctx.at(s + 1) == b'-' || ctx.at(s + 1).is_ascii_alphabetic()) {
        let e = scan(ctx, s, |b| !is_space(b));
        ctx.emit(s, e, Kind::Attribute);
        s = ctx.skip_spaces_from(e);
    }
    let name_end = match ctx.at(s) {
        q @ (b'"' | b'\'') => {
            let le = ctx.line_end(s);
            ctx.src[s + 1..le].iter().position(|&b| b == q).map_or(le, |i| s + i + 2)
        }
        _ => scan(ctx, s, |b| !is_space(b) && !matches!(b, b';' | b'(' | b')' | b'[' | b'{' | b'=' | b'|' | b'&')),
    };
    ctx.emit(s, name_end, kinds.1);
    ctx.pos = name_end.max(end);
    lx.mark_value();
    true
}

/// Shell code whose first word is a command, in places where the shell lexer
/// cannot tell (after `RUN`, a recipe prefix or a cron schedule).
fn shell_command(ctx: &mut Ctx, start: usize, end: usize, spec: &'static Spec) {
    let w = ctx.skip_spaces_from(start);
    let we = scan(ctx, w, |b| is_ident(b) || matches!(b, b'-' | b'.' | b'/')).min(end);
    let mut from = start;
    if we > w && ctx.at(w) != b'-' && ctx.at(we) != b'=' {
        let word = &ctx.src[w..we];
        let kind = if BASH_KEYWORDS.iter().any(|k| k.as_bytes() == word) {
            Kind::Keyword
        } else if BASH_BUILTINS.iter().any(|k| k.as_bytes() == word) {
            Kind::Builtin
        } else {
            Kind::Function
        };
        ctx.emit(w, we, kind);
        from = we;
    }
    ctx.embed_spec(spec, from, end);
    ctx.pos = ctx.pos.max(end);
}

/// A recipe line (Makefile, justfile): optional `@`, `-` and `+` prefixes,
/// then shell code lexed with `spec`.
fn recipe_line(ctx: &mut Ctx, start: usize, end: usize, spec: &'static Spec) {
    let s = ctx.skip_spaces_from(start);
    let p = scan(ctx, s, |b| matches!(b, b'@' | b'-' | b'+')).min(end);
    ctx.emit(s, p, Kind::Operator);
    shell_command(ctx, p.max(start), end, spec);
}

/// A quoted string at the cursor with backslash escapes, in which `var`
/// returns the end of a variable reference starting at a `$`.
fn var_string(ctx: &mut Ctx, var: fn(&Ctx, usize) -> Option<usize>) {
    let start = ctx.pos;
    let quote = ctx.peek();
    let mut seg = start;
    ctx.pos += 1;
    loop {
        let rest = &ctx.src[ctx.pos..];
        ctx.pos += rest.iter().position(|&b| b == quote || b == b'\\' || b == b'$').unwrap_or(rest.len());
        if ctx.done() {
            ctx.emit_from(seg, Kind::String);
            return;
        }
        let at = ctx.pos;
        match ctx.peek() {
            b'\\' => {
                ctx.emit_from(seg, Kind::String);
                ctx.pos += 1;
                if !ctx.done() {
                    ctx.bump_char();
                }
                ctx.emit_from(at, Kind::Escape);
                seg = ctx.pos;
            }
            b'$' => match var(ctx, at) {
                Some(end) => {
                    ctx.emit_from(seg, Kind::String);
                    ctx.emit(at, end, Kind::Variable);
                    ctx.pos = end;
                    seg = end;
                }
                None => ctx.pos += 1,
            },
            _ => {
                ctx.pos += 1;
                ctx.emit_from(seg, Kind::String);
                return;
            }
        }
    }
}

/// End of a `${name}` reference at `start`, stopping at the line end.
fn braced_end(ctx: &Ctx, start: usize) -> usize {
    let le = ctx.line_end(start);
    ctx.src[start..le].iter().position(|&b| b == b'}').map_or(le, |i| start + i + 1)
}

/// Values in configuration files: strings, numbers, booleans, `$VAR`,
/// `${VAR}` and `%h` style specifiers.
fn conf_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let (end, kind) = match (ctx.peek(), ctx.peek_at(1)) {
        (b'%', n) if n.is_ascii_alphabetic() || n == b'%' => (start + 2, Kind::Variable),
        (b'$', b'{') => (braced_end(ctx, start), Kind::Variable),
        // Numbers with units and addresses: `10min`, `512M`, `203.0.113.10`.
        (b'0'..=b'9', _) => (scan(ctx, start, |b| b.is_ascii_alphanumeric() || b == b'.'), Kind::Number),
        _ => return false,
    };
    ctx.emit(start, end, kind);
    ctx.pos = end;
    lx.mark_value();
    true
}

const CONF_CONSTANTS: &[&str] = &["true", "false", "yes", "no", "on", "off", "none", "any", "auto"];

static CONF_VALUE_SPEC: Spec = Spec {
    strings: &[Str::quoted("\""), Str::quoted("'")],
    constants: CONF_CONSTANTS,
    sigils: &[(b'$', Kind::Variable)],
    ident_chars: "-.",
    operators: "=!*?<>|&~",
    case_insensitive: true,
    features: FLAGS,
    hook: Some(conf_hook),
    hook_chars: "%$0123456789",
    ..Spec::DEFAULT
};

// Bash and POSIX shells

/// Variables, substitutions, heredocs and `#` inside words.
fn bash_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'#' => {
            // `#` only starts a comment at the beginning of a word.
            if start > 0 && !matches!(ctx.src[start - 1], b' ' | b'\t' | b'\n' | b';' | b'(' | b'|' | b'&') {
                ctx.pos += 1;
                return true;
            }
            false
        }
        b'$' => {
            let next = ctx.peek_at(1);
            match next {
                b'(' => {
                    ctx.token(2, Kind::Punctuation);
                    lx.nested(b')');
                    if lx.ctx.peek() == b')' {
                        lx.ctx.token(1, Kind::Punctuation);
                    }
                }
                b'{' => {
                    let end = ctx.src[start..]
                        .iter()
                        .position(|&b| b == b'}' || b == b'\n')
                        .map_or(ctx.len(), |i| start + i + 1);
                    ctx.pos = end;
                    ctx.emit(start, end, Kind::Variable);
                }
                b'\'' => {
                    // ANSI C quoting: $'...' with backslash escapes.
                    ctx.pos += 1;
                    lx.string(start, &ANSI_C_STRING, false, Interp::None);
                    return true;
                }
                b'@' | b'*' | b'#' | b'?' | b'$' | b'!' | b'-' | b'0'..=b'9' => ctx.token(2, Kind::Variable),
                _ if is_ident(next) => {
                    let mut end = start + 1;
                    while is_ident(ctx.at(end)) {
                        end += 1;
                    }
                    ctx.pos = end;
                    ctx.emit(start, end, Kind::Variable);
                }
                _ => return false,
            }
            lx.mark_value();
            true
        }
        b'<' if ctx.starts_with(b"<<") && !ctx.starts_with(b"<<<") => heredoc(lx),
        _ => false,
    }
}

fn heredoc(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let mut i = start + 2;
    let strip_tabs = matches!(ctx.at(i), b'-' | b'~');
    if strip_tabs {
        i += 1;
    }
    while ctx.at(i) == b' ' {
        i += 1;
    }
    let quote = ctx.at(i);
    let quoted = matches!(quote, b'\'' | b'"');
    let word_start = if quoted { i + 1 } else { i };
    let mut word_end = word_start;
    while is_ident(ctx.at(word_end)) || ctx.at(word_end) == b'-' {
        word_end += 1;
    }
    if word_end == word_start {
        return false;
    }
    let delimiter = &ctx.src[word_start..word_end];
    let marker_end = if quoted && ctx.at(word_end) == quote { word_end + 1 } else { word_end };
    ctx.emit(start, word_start - quoted as usize, Kind::Operator);
    ctx.emit(word_start - quoted as usize, marker_end, Kind::String);
    // The rest of the current line is ordinary shell code.
    let line_end = ctx.line_end(marker_end);
    let spec = lx.spec;
    ctx.embed_spec(spec, marker_end, line_end);
    if line_end >= ctx.len() {
        return true;
    }
    // Find the terminating line.
    let body_start = line_end + 1;
    let mut pos = body_start;
    let mut body_end = ctx.len();
    let mut term_end = ctx.len();
    while pos < ctx.len() {
        let end = ctx.line_end(pos);
        let mut line = &ctx.src[pos..end];
        if strip_tabs {
            while let [b'\t' | b' ', rest @ ..] = line {
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
    ctx.emit(body_start, body_end, Kind::String);
    ctx.emit(body_end, term_end, Kind::String);
    ctx.pos = term_end;
    true
}

const ANSI_C_STRING: Str = Str::quoted("'").multiline();

const BASH_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "case", "esac", "for", "select", "while", "until", "do", "done", "in",
    "function", "time", "coproc",
];

const BASH_BUILTINS: &[&str] = &[
    "alias", "bg", "bind", "break", "builtin", "caller", "cd", "command", "compgen", "complete", "continue", "declare",
    "dirs", "disown", "echo", "enable", "eval", "exec", "exit", "export", "false", "fc", "fg", "getopts", "hash",
    "help", "history", "jobs", "kill", "let", "local", "logout", "mapfile", "popd", "printf", "pushd", "pwd", "read",
    "readarray", "readonly", "return", "set", "shift", "shopt", "source", "suspend", "test", "times", "trap", "true",
    "type", "typeset", "ulimit", "umask", "unalias", "unset", "wait",
];

const BASH_RULES: Spec = Spec {
    line_comments: &["#"],
    strings: &[
        Str::quoted("\"").multiline().interp(Interp::DollarParen),
        Str::quoted("'").raw().multiline(),
        Str::quoted("`").multiline().interp(Interp::DollarParen),
    ],
    keywords: BASH_KEYWORDS,
    builtins: BASH_BUILTINS,
    ident_chars: "-.",
    operators: "+-*/%=&|^!<>~?:",
    features: COMMANDS | FLAGS,
    hook: Some(bash_hook),
    hook_chars: "#$<",
    ..Spec::DEFAULT
};

static BASH_SPEC: Spec = BASH_RULES;

pub static BASH: Definition = Definition::new("bash", "Bash", Engine::Generic(&BASH_SPEC))
    .aliases(&["sh", "shell", "zsh", "ksh", "shellscript", "console"])
    .extensions(&["sh", "bash", "zsh", "ksh", "command", "ebuild", "eclass"])
    .filenames(&[
        ".bashrc", ".bash_profile", ".bash_aliases", ".profile", ".zshrc", ".zprofile", ".zshenv", "PKGBUILD",
        "APKBUILD",
    ]);

// PowerShell

const PS_OPERATORS: &[&str] = &[
    "eq", "ne", "gt", "ge", "lt", "le", "like", "notlike", "match", "notmatch", "contains", "notcontains", "in",
    "notin", "replace", "is", "isnot", "as", "and", "or", "not", "xor", "band", "bor", "bxor", "bnot", "shl", "shr",
    "split", "join", "f",
];

/// `$var`, `$env:PATH`, `${name}` and `$true`. Returns the end and kind.
fn ps_variable(ctx: &Ctx, start: usize) -> Option<(usize, Kind)> {
    let next = ctx.at(start + 1);
    if next == b'{' {
        return Some((braced_end(ctx, start), Kind::Variable));
    }
    if matches!(next, b'_' | b'$' | b'?' | b'^') && !is_ident(ctx.at(start + 2)) {
        return Some((start + 2, Kind::Variable));
    }
    if !is_ident(next) {
        return None;
    }
    let mut end = scan(ctx, start + 1, is_ident);
    // Scope or drive qualifier: `$env:PATH`, `$script:count`.
    if ctx.at(end) == b':' && is_ident_start(ctx.at(end + 1)) {
        end = scan(ctx, end + 1, is_ident);
    }
    let name = &ctx.src[start + 1..end];
    let constant = [&b"true"[..], b"false", b"null"].iter().any(|c| c.eq_ignore_ascii_case(name));
    Some((end, if constant { Kind::Constant } else { Kind::Variable }))
}

/// Double quoted and here strings with backtick escapes and `$(...)`.
fn ps_string(lx: &mut Lexer, start: usize, here: bool) {
    let mut seg = start;
    lx.ctx.pos = start + if here { 2 } else { 1 };
    loop {
        let ctx = &mut *lx.ctx;
        let rest = &ctx.src[ctx.pos..];
        ctx.pos += rest.iter().position(|&b| matches!(b, b'"' | b'`' | b'$')).unwrap_or(rest.len());
        if ctx.done() {
            ctx.emit_from(seg, Kind::String);
            break;
        }
        let at = ctx.pos;
        match ctx.peek() {
            b'`' => {
                ctx.emit_from(seg, Kind::String);
                ctx.pos += 1;
                if !ctx.done() {
                    ctx.bump_char();
                }
                ctx.emit_from(at, Kind::Escape);
                seg = ctx.pos;
            }
            b'"' => {
                if here {
                    ctx.pos += 1;
                    if ctx.peek() == b'@' && ctx.at_line_start(at) {
                        ctx.pos += 1;
                        ctx.emit_from(seg, Kind::String);
                        break;
                    }
                } else if ctx.peek_at(1) == b'"' {
                    ctx.pos += 2;
                } else {
                    ctx.pos += 1;
                    ctx.emit_from(seg, Kind::String);
                    break;
                }
            }
            _ => {
                if ctx.peek_at(1) == b'(' && ctx.can_nest() {
                    ctx.emit_from(seg, Kind::String);
                    ctx.token(2, Kind::Punctuation);
                    lx.nested(b')');
                    if lx.ctx.peek() == b')' {
                        lx.ctx.token(1, Kind::Punctuation);
                    }
                    seg = lx.ctx.pos;
                } else if let Some((end, kind)) = ps_variable(ctx, at) {
                    ctx.emit_from(seg, Kind::String);
                    ctx.emit(at, end, kind);
                    ctx.pos = end;
                    seg = end;
                } else {
                    ctx.pos += 1;
                }
            }
        }
    }
    lx.mark_value();
}

fn ps_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'$' => {
            if next == b'(' {
                ctx.token(2, Kind::Punctuation);
                lx.nested(b')');
                if lx.ctx.peek() == b')' {
                    lx.ctx.token(1, Kind::Punctuation);
                }
                lx.mark_value();
                return true;
            }
            let Some((end, kind)) = ps_variable(ctx, start) else {
                return false;
            };
            ctx.emit(start, end, kind);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'"' => {
            ps_string(lx, start, false);
            true
        }
        b'@' => match next {
            b'"' | b'\'' if matches!(ctx.at(start + 2), b'\n' | b'\r') => {
                if next == b'"' {
                    ps_string(lx, start, true);
                } else {
                    let end = ctx.find(start + 2, b"\n'@").map_or(ctx.len(), |i| i + 3);
                    ctx.emit(start, end, Kind::String);
                    ctx.pos = end;
                    lx.mark_value();
                }
                true
            }
            _ if is_ident_start(next) => {
                // Splatting: `@params`.
                let end = scan(ctx, start + 1, is_ident);
                ctx.emit(start, end, Kind::Variable);
                ctx.pos = end;
                lx.mark_value();
                true
            }
            _ => false,
        },
        b'-' => {
            let before = if start > 0 { ctx.src[start - 1] } else { b'\n' };
            if !next.is_ascii_alphabetic() || !matches!(before, b' ' | b'\t' | b'\n' | b'(' | b'{' | b',') {
                return false;
            }
            let end = scan(ctx, start + 1, is_ident);
            let word = &ctx.src[start + 1..end];
            let is_op = |w: &[u8]| PS_OPERATORS.iter().any(|op| op.as_bytes().eq_ignore_ascii_case(w));
            let op = is_op(word) || (word.len() > 2 && matches!(word[0] | 0x20, b'c' | b'i') && is_op(&word[1..]));
            ctx.emit(start, end, if op { Kind::Operator } else { Kind::Attribute });
            ctx.pos = end;
            if op {
                lx.mark_operator();
            } else {
                lx.mark_value();
            }
            true
        }
        b'[' => {
            // Type literals `[int]`, `[System.IO.Path]` and attributes `[Parameter(...)]`.
            let before = if start > 0 { ctx.src[start - 1] } else { 0 };
            if !next.is_ascii_alphabetic() || is_ident(before) || matches!(before, b')' | b']' | b'$') {
                return false;
            }
            let name_end = scan(ctx, start + 1, |b| is_ident(b) || b == b'.');
            let mut end = name_end;
            while ctx.at(end) == b'[' && ctx.at(end + 1) == b']' {
                end += 2;
            }
            let kind = match ctx.at(end) {
                b']' => Kind::Type,
                b'(' => Kind::Meta,
                b'[' if end == name_end => Kind::Type,
                _ => return false,
            };
            ctx.token(1, Kind::Punctuation);
            ctx.emit(start + 1, end, kind);
            ctx.pos = end;
            if ctx.peek() == b']' {
                ctx.token(1, Kind::Punctuation);
                lx.mark_operator();
            }
            true
        }
        _ => {
            // Cmdlets: `Get-ChildItem`, `ForEach-Object`.
            let word_end = scan(ctx, start, is_ident);
            if ctx.at(word_end) != b'-' || !ctx.at(word_end + 1).is_ascii_alphabetic() {
                return false;
            }
            let end = scan(ctx, word_end, |b| is_ident(b) || b == b'-');
            ctx.emit(start, end, Kind::Function);
            ctx.pos = end;
            lx.mark_value();
            true
        }
    }
}

static POWERSHELL_SPEC: Spec = Spec {
    line_comments: &["#"],
    block_comments: &[("<#", "#>")],
    strings: &[Str::quoted("'").raw().doubled().multiline()],
    keywords: &[
        "begin", "break", "catch", "continue", "data", "default", "do", "dynamicparam", "else", "elseif", "end",
        "exit", "finally", "for", "foreach", "from", "hidden", "if", "in", "param", "process", "return", "static",
        "switch", "throw", "trap", "try", "until", "using", "while",
    ],
    function_keywords: &["function", "filter", "workflow", "configuration"],
    type_keywords: &["class", "enum"],
    operators: "+-*/%=!<>|&",
    case_insensitive: true,
    features: CALLS | MEMBERS | LINE_KEYS,
    hook: Some(ps_hook),
    hook_chars: "$\"@-[abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static POWERSHELL: Definition = Definition::new("powershell", "PowerShell", Engine::Generic(&POWERSHELL_SPEC))
    .aliases(&["ps1", "pwsh", "posh", "ps"])
    .extensions(&["ps1", "psm1", "psd1"]);

// fish

fn fish_hook(lx: &mut Lexer) -> bool {
    if lx.ctx.peek() == b'f' {
        return keyword_then(lx, b"function", (Kind::Keyword, Kind::Function), Some(&b";"[..]));
    }
    if lx.ctx.peek() == b's' {
        return keyword_then(lx, b"set", (Kind::Builtin, Kind::Variable), None);
    }
    bash_hook(lx)
}

static FISH_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"").multiline().interp(Interp::DollarParen), Str::quoted("'").multiline()],
    keywords: &[
        "if", "else", "end", "switch", "case", "for", "in", "while", "begin", "and", "or", "not", "return", "break",
        "continue", "function",
    ],
    builtins: &[
        "abbr", "alias", "argparse", "bind", "builtin", "cd", "command", "commandline", "complete", "contains",
        "count", "echo", "emit", "eval", "exec", "exit", "false", "functions", "history", "jobs", "math", "printf",
        "pwd", "random", "read", "realpath", "set", "set_color", "source", "status", "string", "test", "true", "type",
        "ulimit", "wait",
    ],
    ident_chars: "-.",
    features: COMMANDS | FLAGS,
    hook: Some(fish_hook),
    hook_chars: "#$fs",
    ..Spec::DEFAULT
};

pub static FISH: Definition = Definition::new("fish", "fish", Engine::Generic(&FISH_SPEC)).extensions(&["fish"]);

// Batch files

/// End of a variable reference at `start`: `%NAME%`, `%~dp0`, `%%i`,
/// `%1`, `%NAME:~0,5%` or the delayed `!NAME!`.
fn batch_variable(ctx: &Ctx, start: usize) -> Option<usize> {
    let next = ctx.at(start + 1);
    let le = ctx.line_end(start);
    match (ctx.at(start), next) {
        (b'%', b'%') if ctx.at(start + 2) == b'~' || ctx.at(start + 2).is_ascii_alphabetic() => {
            Some(scan(ctx, start + 2, |b| b == b'~' || b.is_ascii_alphanumeric()))
        }
        (b'%', b'~') => {
            // Modifier letters followed by the argument number: `%~dp0`, `%~nx1`.
            let e = scan(ctx, start + 2, |b| b.is_ascii_alphabetic());
            Some(if ctx.at(e).is_ascii_digit() || ctx.at(e) == b'*' { e + 1 } else { e }.min(le))
        }
        (b'%', b'0'..=b'9' | b'*') => Some(start + 2),
        (b'%' | b'!', _) if is_ident_start(next) => {
            let close = ctx.at(start);
            let i = ctx.src[start + 1..le].iter().position(|&b| b == close || (close == b'!' && b == b' '))?;
            (ctx.at(start + 1 + i) == close).then_some(start + i + 2)
        }
        _ => None,
    }
}

fn batch_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'%' | b'!' => {
            let Some(end) = batch_variable(ctx, start) else {
                return false;
            };
            ctx.emit(start, end, Kind::Variable);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'"' => {
            // Variables expand inside quotes.
            let le = ctx.line_end(start);
            let end = ctx.src[start + 1..le].iter().position(|&b| b == b'"').map_or(le, |i| start + i + 2);
            let mut seg = start;
            let mut i = start + 1;
            while i < end {
                match batch_variable(ctx, i).filter(|&e| e <= end) {
                    Some(e) => {
                        ctx.emit(seg, i, Kind::String);
                        ctx.emit(i, e, Kind::Variable);
                        seg = e;
                        i = e;
                    }
                    None => i += 1,
                }
            }
            ctx.emit(seg, end, Kind::String);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'^' => {
            // A caret escapes the next character.
            let end = char_end(ctx, start + 1).min(ctx.len());
            ctx.emit(start, end, Kind::Escape);
            ctx.pos = end;
            true
        }
        b':' => {
            if next == b':' && statement_start(ctx, start, b"") {
                line_comment(ctx, start);
                return true;
            }
            if !is_ident_start(next) || !(statement_start(ctx, start, b"") || ctx.at(start - 1) == b' ') {
                return false;
            }
            // Labels, and their uses in `goto :eof` and `call :sub`.
            let end = scan(ctx, start + 1, |b| is_ident(b) || b == b'-' || b == b'.');
            ctx.emit(start, end, Kind::Function);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'/' if start > 0 && ctx.src[start - 1] == b' ' && (next.is_ascii_alphabetic() || next == b'?') => {
            let end = scan(ctx, start + 1, |b| b.is_ascii_alphanumeric() || b == b'?');
            ctx.emit(start, end, Kind::Attribute);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'@' => {
            ctx.token(1, Kind::Operator);
            true
        }
        b'r' | b'R'
            if ctx.starts_with_ignore_case(b"rem")
                && matches!(ctx.at(start + 3), b' ' | b'\t' | b'\n' | b'\r' | b':' | 0)
                && statement_start(ctx, start, b"@&(|") =>
        {
            line_comment(ctx, start);
            true
        }
        b'g' | b'G' if keyword_then(lx, b"goto", (Kind::Keyword, Kind::Function), None) => true,
        b'c' | b'C' if keyword_then(lx, b"call", (Kind::Keyword, Kind::Function), None) => true,
        _ => {
            // A word after `!VAR!` or an escaped `^|` is not a command, although
            // the generic rules treat `!` and `|` as command separators.
            let ctx = &*lx.ctx;
            let mut p = start;
            while p > 0 && matches!(ctx.src[p - 1], b' ' | b'\t') {
                p -= 1;
            }
            let after_var = p > 0 && ctx.src[p - 1] == b'!';
            let after_escape = p > 1 && matches!(ctx.src[p - 1], b'|' | b'&') && ctx.src[p - 2] == b'^';
            if !is_ident_start(ctx.peek()) || !(after_var || after_escape) {
                return false;
            }
            let end = lx.scan_ident(start);
            let kind = lx.classify(start, end);
            lx.ctx.emit(start, end, kind);
            lx.ctx.pos = end;
            true
        }
    }
}

static BATCH_SPEC: Spec = Spec {
    keywords: &[
        "if", "else", "for", "in", "do", "goto", "call", "exit", "not", "exist", "defined", "errorlevel", "equ", "neq",
        "lss", "leq", "gtr", "geq", "setlocal", "endlocal", "shift", "enabledelayedexpansion", "enableextensions",
    ],
    builtins: &[
        "echo", "set", "cd", "chdir", "cls", "copy", "del", "dir", "erase", "md", "mkdir", "move", "pause", "popd",
        "pushd", "rd", "ren", "rename", "rmdir", "start", "title", "type", "choice", "timeout", "where", "xcopy",
        "robocopy", "findstr", "color", "ver", "path", "prompt",
    ],
    constants: &["on", "off", "nul", "con"],
    ident_chars: "-.",
    operators: "=<>|&+-*",
    case_insensitive: true,
    features: COMMANDS,
    hook: Some(batch_hook),
    hook_chars: "%!\":/@^abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static BATCH: Definition = Definition::new("batch", "Batch", Engine::Generic(&BATCH_SPEC))
    .aliases(&["bat", "cmd", "batchfile", "dosbatch"])
    .extensions(&["bat", "cmd"]);

// Dockerfile

static DOCKER_SPEC: Spec = Spec {
    strings: &[Str::quoted("\"").interp(Interp::Dollar), Str::quoted("'").raw()],
    keywords: &["as"],
    ident_chars: "-.",
    operators: "=",
    case_insensitive: true,
    features: COMMANDS | FLAGS,
    hook: Some(bash_hook),
    hook_chars: "$",
    ..Spec::DEFAULT
};

fn dockerfile(ctx: &mut Ctx) {
    while !ctx.done() {
        ctx.skip_whitespace();
        if ctx.done() {
            break;
        }
        let start = ctx.pos;
        if ctx.peek() == b'#' {
            line_comment(ctx, start);
            continue;
        }
        let end = logical_line_end(ctx, start);
        docker_instruction(ctx, start, end);
        ctx.pos = ctx.pos.max(end);
    }
}

fn docker_instruction(ctx: &mut Ctx, mut start: usize, end: usize) {
    loop {
        let word_end = scan(ctx, start, |b| b.is_ascii_alphabetic()).min(end);
        if word_end == start {
            ctx.embed_spec(&DOCKER_SPEC, start, end);
            return;
        }
        ctx.emit(start, word_end, Kind::Keyword);
        let args = ctx.skip_spaces_from(word_end);
        let word = &ctx.src[start..word_end];
        let is = |w: &[u8]| word.eq_ignore_ascii_case(w);
        if is(b"onbuild") && args < end && ctx.at(args).is_ascii_alphabetic() {
            start = args;
            continue;
        }
        if is(b"run") || is(b"cmd") || is(b"entrypoint") {
            docker_shell(ctx, args, end);
        } else if is(b"healthcheck") {
            // `HEALTHCHECK --interval=5s CMD curl ...`
            let upper = ctx.find(args, b" CMD ").filter(|&i| i < end);
            let lower = ctx.find(args, b" cmd ").filter(|&i| i < end);
            match upper.or(lower) {
                Some(i) => {
                    ctx.embed_spec(&DOCKER_SPEC, args, i);
                    ctx.emit(i + 1, i + 4, Kind::Keyword);
                    docker_shell(ctx, i + 5, end);
                }
                None => ctx.embed_spec(&DOCKER_SPEC, args, end),
            }
        } else {
            ctx.embed_spec(&DOCKER_SPEC, args, end);
        }
        return;
    }
}

/// The command of `RUN`, `CMD` and `ENTRYPOINT`: JSON exec form or shell form.
fn docker_shell(ctx: &mut Ctx, mut start: usize, end: usize) {
    // Options such as `--mount=type=cache,target=/root/.cache`.
    while start < end && ctx.at(start) == b'-' && ctx.at(start + 1) == b'-' {
        let e = scan(ctx, start, |b| !is_space(b)).min(end);
        let eq = ctx.src[start..e].iter().position(|&b| b == b'=').map_or(e, |i| start + i);
        ctx.emit(start, eq, Kind::Attribute);
        ctx.embed_spec(&DOCKER_SPEC, eq, e);
        start = ctx.skip_spaces_from(e);
    }
    if ctx.at(start) == b'[' {
        ctx.embed_spec(&DOCKER_SPEC, start, end);
        return;
    }
    let end = docker_heredoc_end(ctx, start, end);
    shell_command(ctx, start, end, &BASH_SPEC);
}

/// `RUN <<EOF` takes the following lines up to the delimiter.
fn docker_heredoc_end(ctx: &Ctx, start: usize, end: usize) -> usize {
    let Some(at) = ctx.find(start, b"<<").filter(|&i| i < end && ctx.at(i + 2) != b'<') else {
        return end;
    };
    let mut i = at + 2;
    let strip = ctx.at(i) == b'-';
    i += strip as usize;
    if matches!(ctx.at(i), b'"' | b'\'') {
        i += 1;
    }
    let word_end = scan(ctx, i, is_ident);
    if word_end == i || end >= ctx.len() {
        return end;
    }
    let word = &ctx.src[i..word_end];
    let mut pos = end + 1;
    while pos < ctx.len() {
        let le = ctx.line_end(pos);
        let line = &ctx.src[if strip { ctx.skip_spaces_from(pos) } else { pos }..le];
        if line.strip_suffix(b"\r").unwrap_or(line) == word {
            return le;
        }
        pos = le + 1;
    }
    ctx.len()
}

pub static DOCKERFILE: Definition = Definition::new("dockerfile", "Dockerfile", Engine::Custom(dockerfile))
    .aliases(&["docker", "containerfile"])
    .extensions(&["dockerfile", "containerfile"])
    .filenames(&["Dockerfile", "Containerfile"]);

// Makefile

const MAKE_FUNCTIONS: &[&str] = &[
    "subst", "patsubst", "strip", "findstring", "filter", "filter-out", "sort", "word", "wordlist", "words",
    "firstword", "lastword", "dir", "notdir", "suffix", "basename", "addsuffix", "addprefix", "join", "wildcard",
    "realpath", "abspath", "error", "warning", "info", "shell", "origin", "flavor", "foreach", "if", "or", "and",
    "call", "eval", "file", "value", "let", "intcmp",
];

const MAKE_DIRECTIVES: &[&str] = &[
    "include", "-include", "sinclude", "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif", "define", "endef",
    "export", "unexport", "override", "private", "undefine", "vpath",
];

/// Make variable references: `$(VAR)`, `${VAR}`, `$@`, `$(SRC:.c=.o)` and
/// function calls such as `$(patsubst ...)` and `$(shell ...)`.
fn make_dollar(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match next {
        b'(' | b'{' => {
            let close = if next == b'(' { b')' } else { b'}' };
            let name_start = start + 2;
            let name_end = scan(ctx, name_start, |b| {
                is_ident(b) || matches!(b, b'.' | b'-' | b'@' | b'<' | b'^' | b'*' | b'?' | b'%' | b'/' | b'+')
            });
            let after = ctx.at(name_end);
            let simple_end = if after == close && name_end > name_start {
                Some(name_end + 1)
            } else if after == b':' && name_end > name_start {
                // Substitution reference: `$(SRC:.c=.o)`.
                let le = ctx.line_end(name_end);
                ctx.src[name_end..le]
                    .iter()
                    .position(|&b| b == close || matches!(b, b'$' | b'(' | b' '))
                    .filter(|&i| ctx.src[name_end + i] == close)
                    .map(|i| name_end + i + 1)
            } else {
                None
            };
            if let Some(end) = simple_end {
                ctx.emit(start, end, Kind::Variable);
                ctx.pos = end;
                lx.mark_value();
                return true;
            }
            let name = &ctx.src[name_start..name_end];
            let function = matches!(after, b' ' | b'\t') && MAKE_FUNCTIONS.iter().any(|f| f.as_bytes() == name);
            ctx.token(2, Kind::Punctuation);
            if function {
                ctx.emit(name_start, name_end, Kind::Function);
                ctx.pos = name_end;
                if name == b"shell" {
                    let end = closing(ctx, start + 1, ctx.len()).unwrap_or_else(|| ctx.line_end(name_end));
                    shell_command(ctx, name_end, end, &MAKE_RECIPE_SPEC);
                } else {
                    lx.nested(close);
                }
            } else {
                lx.nested(close);
            }
            if lx.ctx.peek() == close {
                lx.ctx.token(1, Kind::Punctuation);
            }
            lx.mark_value();
            true
        }
        b'$' if ctx.at(start + 2) == b'(' => {
            // `$$(cmd)` is a shell command substitution.
            ctx.token(3, Kind::Punctuation);
            lx.nested(b')');
            if lx.ctx.peek() == b')' {
                lx.ctx.token(1, Kind::Punctuation);
            }
            lx.mark_value();
            true
        }
        b'$' => {
            // `$$` passes a literal `$` to the shell: `$$HOME`, `$${name}`.
            let end = if ctx.at(start + 2) == b'{' { braced_end(ctx, start) } else { scan(ctx, start + 2, is_ident) };
            ctx.emit(start, end, Kind::Variable);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        0 | b' ' | b'\t' | b'\n' | b'\r' => false,
        _ => {
            ctx.pos += 1;
            ctx.bump_char();
            ctx.emit_from(start, Kind::Variable);
            lx.mark_value();
            true
        }
    }
}

fn make_recipe_hook(lx: &mut Lexer) -> bool {
    match lx.ctx.peek() {
        b'$' => make_dollar(lx),
        b'"' => {
            recipe_string(lx, b"$", make_dollar);
            true
        }
        _ => bash_hook(lx),
    }
}

/// A double quoted shell string in a recipe. `special` runs at every byte
/// in `stops` (make's `$`, just's `{{`) and returns false to keep it as text.
fn recipe_string(lx: &mut Lexer, stops: &[u8], special: fn(&mut Lexer) -> bool) {
    let start = lx.ctx.pos;
    let mut seg = start;
    lx.ctx.pos += 1;
    loop {
        let ctx = &mut *lx.ctx;
        let rest = &ctx.src[ctx.pos..];
        ctx.pos += rest.iter().position(|&b| b == b'"' || b == b'\\' || stops.contains(&b)).unwrap_or(rest.len());
        if ctx.done() {
            ctx.emit_from(seg, Kind::String);
            break;
        }
        let at = ctx.pos;
        match ctx.peek() {
            b'"' => {
                ctx.pos += 1;
                ctx.emit_from(seg, Kind::String);
                break;
            }
            b'\\' => {
                ctx.emit_from(seg, Kind::String);
                ctx.pos += 1;
                if !ctx.done() {
                    ctx.bump_char();
                }
                ctx.emit_from(at, Kind::Escape);
                seg = ctx.pos;
            }
            _ => {
                ctx.emit_from(seg, Kind::String);
                if !special(lx) || lx.ctx.pos == at {
                    lx.ctx.pos = at + 1;
                    lx.ctx.emit(at, at + 1, Kind::String);
                }
                seg = lx.ctx.pos;
            }
        }
    }
    lx.mark_value();
}

static MAKE_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"")],
    keywords: &["ifeq", "ifneq", "ifdef", "ifndef"],
    ident_chars: "-.",
    operators: "=:+?!|%*<>&",
    features: FLAGS,
    hook: Some(make_dollar),
    hook_chars: "$",
    ..Spec::DEFAULT
};

static MAKE_RECIPE_SPEC: Spec = Spec { hook: Some(make_recipe_hook), hook_chars: "#$<\"", ..BASH_RULES };

fn makefile(ctx: &mut Ctx) {
    let mut in_rule = false;
    while !ctx.done() {
        let start = ctx.pos;
        match ctx.peek() {
            b'\n' | b'\r' => {
                ctx.pos += 1;
                continue;
            }
            b'\t' if in_rule => {
                let end = logical_line_end(ctx, start);
                recipe_line(ctx, start, end, &MAKE_RECIPE_SPEC);
                continue;
            }
            _ => {}
        }
        let end = logical_line_end(ctx, start);
        let s = ctx.skip_spaces_from(start);
        if s >= end || matches!(ctx.at(s), b'\n' | b'\r') {
            ctx.pos = end;
            continue;
        }
        if ctx.at(s) == b'#' {
            ctx.emit(s, end, Kind::Comment);
            ctx.pos = end;
            continue;
        }
        make_line(ctx, s, end, &mut in_rule);
        ctx.pos = ctx.pos.max(end);
    }
}

fn make_line(ctx: &mut Ctx, mut s: usize, end: usize, in_rule: &mut bool) {
    let mut names_are_vars = false;
    // Directives, possibly chained: `else ifeq`, `export override`.
    loop {
        let we = scan(ctx, s, |b| b.is_ascii_alphanumeric() || b == b'-').min(end);
        let word = &ctx.src[s..we];
        if we == s
            || !matches!(ctx.at(we), b' ' | b'\t' | b'\n' | b'\r' | b'(' | 0)
            || !MAKE_DIRECTIVES.iter().any(|d| d.as_bytes() == word)
        {
            break;
        }
        ctx.emit(s, we, Kind::Keyword);
        match word {
            b"define" => return make_define(ctx, we, end),
            b"export" | b"unexport" | b"override" | b"private" | b"undefine" => names_are_vars = true,
            b"else" => {}
            _ => {
                ctx.embed_spec(&MAKE_SPEC, we, end);
                return;
            }
        }
        s = ctx.skip_spaces_from(we);
        if s >= end {
            return;
        }
    }
    // Find the first top level `=` or `:`.
    let mut depth = 0;
    let mut op = None;
    let mut i = s;
    while i < end {
        match ctx.src[i] {
            b'(' | b'{' => depth += 1,
            b')' | b'}' if depth > 0 => depth -= 1,
            b'#' if depth == 0 => break,
            b'=' | b':' if depth == 0 => {
                op = Some(i);
                break;
            }
            _ => {}
        }
        i += 1;
    }
    let Some(i) = op else {
        if names_are_vars {
            make_names(ctx, s, end, Kind::Variable);
        } else {
            ctx.embed_spec(&MAKE_SPEC, s, end);
        }
        return;
    };
    let colons = scan(ctx, i, |b| b == b':');
    let (op_start, op_end, assign) = if ctx.src[i] == b'=' {
        let os = if i > s && matches!(ctx.src[i - 1], b'+' | b'?' | b'!') { i - 1 } else { i };
        (os, i + 1, true)
    } else if ctx.at(colons) == b'=' {
        (i, colons + 1, true)
    } else {
        (i, colons, false)
    };
    if assign {
        make_names(ctx, s, op_start, Kind::Variable);
        ctx.emit(op_start, op_end, Kind::Operator);
        ctx.embed_spec(&MAKE_SPEC, op_end, end);
        *in_rule = false;
        return;
    }
    let phony = ctx.src[s..op_start].trim_ascii_end() == b".PHONY";
    make_names(ctx, s, op_start, Kind::Function);
    ctx.emit(op_start, op_end, Kind::Operator);
    if phony {
        make_names(ctx, op_end, end, Kind::Function);
        *in_rule = true;
        return;
    }
    // An inline recipe follows `;`.
    match ctx.src[op_end..end].iter().position(|&b| b == b';').map(|p| op_end + p) {
        Some(semi) => {
            ctx.embed_spec(&MAKE_SPEC, op_end, semi);
            ctx.emit(semi, semi + 1, Kind::Punctuation);
            recipe_line(ctx, semi + 1, end, &MAKE_RECIPE_SPEC);
        }
        None => ctx.embed_spec(&MAKE_SPEC, op_end, end),
    }
    *in_rule = true;
}

/// Space separated targets or variable names. Special targets such as
/// `.PHONY` are keywords and references are lexed as make code.
fn make_names(ctx: &mut Ctx, start: usize, end: usize, kind: Kind) {
    let mut i = start;
    while i < end {
        i = scan(ctx, i, |b| matches!(b, b' ' | b'\t' | b'\\' | b'\n' | b'\r')).min(end);
        let e = scan(ctx, i, |b| !is_space(b)).min(end);
        if e == i {
            break;
        }
        let word = &ctx.src[i..e];
        if word.contains(&b'$') {
            ctx.embed_spec(&MAKE_SPEC, i, e);
        } else if word.len() > 1 && word[0] == b'.' && word[1].is_ascii_uppercase() {
            ctx.emit(i, e, Kind::Keyword);
        } else {
            ctx.emit(i, e, kind);
        }
        i = e;
    }
}

/// `define NAME` up to the matching `endef`.
fn make_define(ctx: &mut Ctx, from: usize, end: usize) {
    make_names(ctx, from, end, Kind::Variable);
    let body = (end + 1).min(ctx.len());
    let mut pos = body;
    while pos < ctx.len() {
        let s = ctx.skip_spaces_from(pos);
        let le = ctx.line_end(pos);
        if ctx.starts_with_at(s, b"endef") && !is_ident(ctx.at(s + 5)) {
            ctx.embed_spec(&MAKE_SPEC, body, pos);
            ctx.emit(s, s + 5, Kind::Keyword);
            ctx.pos = le;
            return;
        }
        pos = le + 1;
    }
    ctx.embed_spec(&MAKE_SPEC, body, ctx.len());
}

pub static MAKEFILE: Definition = Definition::new("makefile", "Makefile", Engine::Custom(makefile))
    .aliases(&["make", "mk", "mak", "bsdmake"])
    .extensions(&["mk", "mak", "make"])
    .filenames(&["Makefile", "GNUmakefile", "makefile", "BSDmakefile"]);

// CMake

/// Bracket arguments and comments, `${VAR}`, `$ENV{VAR}`, generator
/// expressions, quoted arguments and the names declared by `function()`.
fn cmake_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'#' | b'[' => {
            let open = start + (ctx.peek() == b'#') as usize;
            if ctx.at(open) != b'[' {
                return false;
            }
            let eq = scan(ctx, open + 1, |b| b == b'=') - open - 1;
            if ctx.at(open + 1 + eq) != b'[' {
                return false;
            }
            let mut close = vec![b']'];
            close.extend(std::iter::repeat(b'=').take(eq));
            close.push(b']');
            let end = ctx.find(open + eq + 2, &close).map_or(ctx.len(), |i| i + close.len());
            ctx.emit(start, end, if open > start { Kind::Comment } else { Kind::String });
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'$' => {
            let brace = match ctx.peek_at(1) {
                b'{' => start + 1,
                b'<' => {
                    // Generator expression: `$<CONFIG:Debug>`.
                    ctx.token(2, Kind::Meta);
                    lx.nested(b'>');
                    if lx.ctx.peek() == b'>' {
                        lx.ctx.token(1, Kind::Meta);
                    }
                    return true;
                }
                b'E' | b'C' => {
                    let e = scan(ctx, start + 1, |b| b.is_ascii_uppercase());
                    if ctx.at(e) != b'{' {
                        return false;
                    }
                    e
                }
                _ => return false,
            };
            let le = ctx.line_end(start);
            let end = closing(ctx, brace, le).map_or(le, |i| i + 1);
            ctx.emit(start, end, Kind::Variable);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'"' => {
            var_string(ctx, cmake_variable);
            lx.mark_value();
            true
        }
        b'(' => {
            // `function(name ...)`, `set(VAR ...)`: the first argument is declared.
            let we = {
                let mut i = start;
                while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
                    i -= 1;
                }
                i
            };
            let mut ws = we;
            while ws > 0 && is_ident(ctx.src[ws - 1]) {
                ws -= 1;
            }
            let word = &ctx.src[ws..we];
            let is = |list: &[&str]| list.iter().any(|w| w.as_bytes().eq_ignore_ascii_case(word));
            let kind = if is(&["function", "macro"]) {
                Kind::Function
            } else if is(&["set", "unset", "option", "foreach"]) {
                Kind::Variable
            } else {
                return false;
            };
            ctx.token(1, Kind::Punctuation);
            let s = ctx.skip_spaces_from(ctx.pos);
            let e = scan(ctx, s, is_ident);
            ctx.emit(s, e, kind);
            ctx.pos = e;
            true
        }
        _ => false,
    }
}

/// `${VAR}`, `$ENV{VAR}` and `$CACHE{VAR}` inside quoted arguments.
fn cmake_variable(ctx: &Ctx, at: usize) -> Option<usize> {
    let brace = scan(ctx, at + 1, |b| b.is_ascii_uppercase());
    if ctx.at(brace) != b'{' || !(brace == at + 1 || matches!(&ctx.src[at + 1..brace], b"ENV" | b"CACHE")) {
        return None;
    }
    let le = ctx.line_end(at);
    Some(closing(ctx, brace, le).map_or(le, |i| i + 1))
}

static CMAKE_SPEC: Spec = Spec {
    line_comments: &["#"],
    keywords: &[
        "if", "elseif", "else", "endif", "foreach", "endforeach", "while", "endwhile", "function", "endfunction",
        "macro", "endmacro", "return", "break", "continue", "block", "endblock", "and", "or", "not", "defined",
        "matches", "less", "greater", "equal", "strless", "strgreater", "strequal", "version_less", "version_greater",
        "version_equal", "version_greater_equal", "version_less_equal", "exists", "is_directory", "command", "in",
        "lists", "items", "range", "public", "private", "interface", "required", "components", "status", "warning",
        "fatal_error", "send_error", "author_warning", "cache", "bool", "filepath", "internal", "force",
        "parent_scope", "append", "glob", "glob_recurse", "targets", "destination", "files", "directory", "static",
        "shared", "module", "object", "imported", "alias", "version", "languages", "description",
    ],
    constants: &["on", "off", "true", "false", "yes", "no", "notfound", "ignore"],
    case_insensitive: true,
    features: CALLS | FLAGS,
    hook: Some(cmake_hook),
    hook_chars: "#[$\"(",
    ..Spec::DEFAULT
};

pub static CMAKE: Definition = Definition::new("cmake", "CMake", Engine::Generic(&CMAKE_SPEC))
    .extensions(&["cmake"])
    .filenames(&["CMakeLists.txt"]);

// nginx

/// `$var` and `${var}`.
fn nginx_variable(ctx: &Ctx, at: usize) -> Option<usize> {
    match ctx.at(at + 1) {
        b'{' => Some(braced_end(ctx, at)),
        b'0'..=b'9' => Some(at + 2),
        n if is_ident_start(n) => Some(scan(ctx, at + 1, is_ident)),
        _ => None,
    }
}

/// Directives (the first word of a statement), variables, strings with
/// variables and IP addresses.
fn nginx_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'$' => {
            let Some(end) = nginx_variable(ctx, start) else {
                return false;
            };
            ctx.emit(start, end, Kind::Variable);
            ctx.pos = end;
            true
        }
        b'"' | b'\'' => {
            var_string(ctx, nginx_variable);
            true
        }
        b'0'..=b'9' => {
            // `127.0.0.1`, `10m`, `1.5`.
            let end = scan(ctx, start, |b| b.is_ascii_alphanumeric() || b == b'.');
            ctx.emit(start, end, Kind::Number);
            ctx.pos = end;
            true
        }
        _ => {
            if !statement_start(ctx, start, b";{}") {
                return false;
            }
            let end = lx.scan_ident(start);
            lx.ctx.emit(start, end, Kind::Keyword);
            lx.ctx.pos = end;
            lx.mark_operator();
            true
        }
    }
}

static NGINX_SPEC: Spec = Spec {
    line_comments: &["#"],
    constants: &["on", "off"],
    ident_chars: "-.",
    operators: "=~!^*",
    hook: Some(nginx_hook),
    hook_chars: "$\"'0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_",
    ..Spec::DEFAULT
};

pub static NGINX: Definition = Definition::new("nginx", "nginx", Engine::Generic(&NGINX_SPEC))
    .aliases(&["nginxconf"])
    .extensions(&["nginx", "nginxconf"])
    .filenames(&["nginx.conf"]);

// Apache

fn apache_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'#' => {
            // Comments only start a line; elsewhere `#` is part of a value.
            if statement_start(ctx, start, b"") {
                line_comment(ctx, start);
            } else {
                ctx.pos += 1;
            }
            true
        }
        b'<' => {
            let name = start + 1 + (next == b'/') as usize;
            if !ctx.at(name).is_ascii_alphabetic() {
                return false;
            }
            let end = scan(ctx, name, |b| is_ident(b) || b == b'-');
            ctx.emit(start, name, Kind::Punctuation);
            ctx.emit(name, end, Kind::Tag);
            ctx.pos = end;
            true
        }
        b'%' | b'$' => {
            let end = match next {
                b'{' => braced_end(ctx, start),
                b'0'..=b'9' => start + 2,
                _ => return false,
            };
            ctx.emit(start, end, Kind::Variable);
            ctx.pos = end;
            true
        }
        b'[' => {
            // RewriteRule flags: `[L,R=301,NC]`.
            let le = ctx.line_end(start);
            let Some(close) = ctx.src[start..le].iter().position(|&b| b == b']').map(|i| start + i) else {
                return false;
            };
            let inner = &ctx.src[start + 1..close];
            if inner.is_empty() || !inner.iter().all(|&b| is_ident(b) || matches!(b, b',' | b'=' | b'-' | b':')) {
                return false;
            }
            ctx.token(1, Kind::Punctuation);
            let mut i = start + 1;
            while i < close {
                let e = scan(ctx, i, |b| b != b',' && b != b'=').min(close);
                ctx.emit(i, e, Kind::Attribute);
                if e < close {
                    ctx.emit(e, e + 1, Kind::Punctuation);
                    if ctx.src[e] == b'=' {
                        let v = scan(ctx, e + 1, |b| b != b',').min(close);
                        let number = ctx.src[e + 1..v].iter().all(u8::is_ascii_digit);
                        ctx.emit(e + 1, v, if number { Kind::Number } else { Kind::Text });
                        i = v;
                        continue;
                    }
                }
                i = e + 1;
            }
            ctx.emit(close, close + 1, Kind::Punctuation);
            ctx.pos = close + 1;
            true
        }
        _ => {
            if !statement_start(ctx, start, b"") {
                return false;
            }
            let end = lx.scan_ident(start);
            lx.ctx.emit(start, end, Kind::Keyword);
            lx.ctx.pos = end;
            true
        }
    }
}

static APACHE_SPEC: Spec = Spec {
    strings: &[Str::quoted("\"")],
    constants: &["on", "off"],
    ident_chars: "-.",
    operators: "=!",
    case_insensitive: true,
    hook: Some(apache_hook),
    hook_chars: "#<%$[abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static APACHECONF: Definition = Definition::new("apacheconf", "Apache", Engine::Generic(&APACHE_SPEC))
    .aliases(&["apache", "htaccess", "aconf"])
    .extensions(&["apacheconf", "vhost"])
    .filenames(&[".htaccess", "httpd.conf", "apache2.conf"]);

// AWK

fn awk_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    if ctx.peek() == b'/' {
        // A pattern at the start of a line is a regex even after `}`.
        if !statement_start(ctx, start, b"") || matches!(next, b' ' | b'/' | b'\n' | 0) {
            return false;
        }
        let le = ctx.line_end(start);
        let mut i = start + 1;
        while i < le && ctx.src[i] != b'/' {
            i += if ctx.src[i] == b'\\' { 2 } else { 1 };
        }
        if i >= le {
            return false;
        }
        ctx.emit(start, i + 1, Kind::Regex);
        ctx.pos = i + 1;
        lx.mark_value();
        return true;
    }
    let end = if next.is_ascii_digit() {
        scan(ctx, start + 1, |b| b.is_ascii_digit())
    } else if is_ident_start(next) {
        scan(ctx, start + 1, is_ident)
    } else {
        return false;
    };
    ctx.emit(start, end, Kind::Variable);
    ctx.pos = end;
    lx.mark_value();
    true
}

static AWK_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"")],
    keywords: &[
        "BEGIN", "END", "BEGINFILE", "ENDFILE", "if", "else", "while", "for", "do", "break", "continue", "next",
        "nextfile", "exit", "return", "delete", "in", "getline", "print", "printf",
    ],
    builtins: &[
        "length", "substr", "index", "split", "sub", "gsub", "match", "sprintf", "sin", "cos", "atan2", "exp", "log",
        "sqrt", "int", "rand", "srand", "tolower", "toupper", "system", "close", "fflush", "gensub", "strftime",
        "systime", "asort", "asorti",
    ],
    variables: &[
        "NR", "NF", "FNR", "FS", "OFS", "RS", "ORS", "FILENAME", "SUBSEP", "RSTART", "RLENGTH", "ENVIRON", "ARGC",
        "ARGV", "CONVFMT", "OFMT", "IGNORECASE", "PROCINFO",
    ],
    function_keywords: &["function", "func"],
    features: CALLS | REGEX,
    hook: Some(awk_hook),
    hook_chars: "$/",
    ..Spec::DEFAULT
};

pub static AWK: Definition = Definition::new("awk", "AWK", Engine::Generic(&AWK_SPEC))
    .aliases(&["gawk", "mawk", "nawk"])
    .extensions(&["awk", "gawk"]);

// Nix

/// Paths: `./default.nix`, `../lib`, `~/x`, `/etc/nixos` and `<nixpkgs>`.
fn nix_hook(lx: &mut Lexer) -> bool {
    let expression = lx.expression_start();
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let path_char = |b: u8| is_ident(b) || matches!(b, b'.' | b'/' | b'-' | b'+');
    let end = match ctx.peek() {
        b'<' => {
            if !is_ident_start(ctx.peek_at(1)) {
                return false;
            }
            let e = scan(ctx, start + 1, path_char);
            if ctx.at(e) != b'>' {
                return false;
            }
            e + 1
        }
        b'.' => {
            let prefix = if ctx.starts_with(b"./") { 2 } else { 3 };
            if (prefix == 3 && !ctx.starts_with(b"../")) || !path_char(ctx.at(start + prefix)) {
                return false;
            }
            scan(ctx, start, path_char)
        }
        b'~' | b'/' => {
            let first = if ctx.peek() == b'~' { 2 } else { 1 };
            if (first == 2 && ctx.peek_at(1) != b'/') || !is_ident(ctx.at(start + first)) || !expression {
                return false;
            }
            scan(ctx, start + first, path_char)
        }
        _ => return false,
    };
    ctx.emit(start, end, Kind::String);
    ctx.pos = end;
    lx.mark_value();
    true
}

static NIX_SPEC: Spec = Spec {
    line_comments: &["#"],
    block_comments: &[("/*", "*/")],
    strings: &[
        Str::quoted("\"").multiline().interp(Interp::DollarBrace),
        Str::new("''", "''").raw().multiline().interp(Interp::DollarBrace),
    ],
    keywords: &["assert", "else", "if", "in", "inherit", "let", "or", "rec", "then", "with"],
    builtins: &[
        "builtins", "import", "map", "toString", "throw", "abort", "derivation", "fetchTarball", "fetchGit",
        "baseNameOf", "dirOf", "isNull", "removeAttrs", "placeholder",
    ],
    constants: &["true", "false", "null"],
    ident_chars: "-'",
    operators: "+-*/=!<>&|?@:",
    features: MEMBERS | LINE_KEYS,
    hook: Some(nix_hook),
    hook_chars: "<./~",
    ..Spec::DEFAULT
};

pub static NIX: Definition =
    Definition::new("nix", "Nix", Engine::Generic(&NIX_SPEC)).aliases(&["nixos"]).extensions(&["nix"]);

// just

static JUST_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[
        Str::quoted("\"\"\"").multiline(),
        Str::quoted("'''").raw().multiline(),
        Str::quoted("```").raw().multiline(),
        Str::quoted("\""),
        Str::quoted("'").raw(),
        Str::quoted("`").raw(),
    ],
    keywords: &["if", "else", "set", "alias", "export", "import", "mod"],
    builtins: &["env", "env_var", "env_var_or_default", "justfile_directory", "invocation_directory", "os", "arch"],
    constants: &["true", "false"],
    ident_chars: "-",
    operators: "=+/!&|:?~",
    features: CALLS,
    ..Spec::DEFAULT
};

/// `{{ expression }}` inside recipe bodies.
fn just_recipe_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    if ctx.peek() == b'"' {
        recipe_string(lx, b"${", just_recipe_hook);
        return true;
    }
    if ctx.peek() != b'{' {
        return bash_hook(lx);
    }
    if ctx.peek_at(1) != b'{' {
        return false;
    }
    if ctx.starts_with(b"{{{{") {
        ctx.pos += 4;
        return true;
    }
    let le = ctx.line_end(start);
    let Some(close) = ctx.find(start + 2, b"}}").filter(|&i| i < le) else {
        return false;
    };
    ctx.emit(start, start + 2, Kind::Punctuation);
    ctx.embed_spec(&JUST_SPEC, start + 2, close);
    ctx.emit(close, close + 2, Kind::Punctuation);
    ctx.pos = close + 2;
    lx.mark_value();
    true
}

static JUST_RECIPE_SPEC: Spec = Spec { hook: Some(just_recipe_hook), hook_chars: "#$<{\"", ..BASH_RULES };

fn just(ctx: &mut Ctx) {
    let mut in_recipe = false;
    while !ctx.done() {
        let start = ctx.pos;
        let b = ctx.peek();
        if b == b'\n' || b == b'\r' {
            ctx.pos += 1;
            continue;
        }
        if in_recipe && matches!(b, b' ' | b'\t') {
            just_body(ctx, start);
            in_recipe = false;
            continue;
        }
        in_recipe = false;
        let end = logical_line_end(ctx, start);
        let s = ctx.skip_spaces_from(start);
        match ctx.at(s) {
            b'#' => line_comment(ctx, s),
            b'[' => {
                // Attributes: `[private]`, `[group('ci')]`.
                ctx.emit(s, s + 1, Kind::Meta);
                let close = closing(ctx, s, end).unwrap_or(end);
                let name = scan(ctx, s + 1, |b| is_ident(b) || b == b'-').min(close);
                ctx.emit(s + 1, name, Kind::Meta);
                ctx.embed_spec(&JUST_SPEC, name, close);
                if close < end {
                    ctx.emit(close, close + 1, Kind::Meta);
                    ctx.embed_spec(&JUST_SPEC, close + 1, end);
                }
            }
            _ => in_recipe = just_statement(ctx, s, end),
        }
        ctx.pos = ctx.pos.max(end);
    }
}

/// One top level line. Returns true for a recipe header.
fn just_statement(ctx: &mut Ctx, mut s: usize, end: usize) -> bool {
    let word_char = |b: u8| is_ident(b) || b == b'-';
    loop {
        let we = scan(ctx, s, word_char).min(end);
        let word = &ctx.src[s..we];
        let after = ctx.skip_spaces_from(we);
        let assign = ctx.at(after) == b':' && ctx.at(after + 1) == b'=';
        match word {
            b"export" | b"set" | b"alias" | b"import" | b"mod" if !assign && ctx.at(after) != b':' && after > we => {
                ctx.emit(s, we, Kind::Keyword);
                match word {
                    b"export" => {
                        s = after;
                        continue;
                    }
                    b"set" => {
                        let ne = scan(ctx, after, word_char).min(end);
                        ctx.emit(after, ne, Kind::Property);
                        ctx.embed_spec(&JUST_SPEC, ne, end);
                    }
                    b"alias" => {
                        // `alias b := build`: both names are recipes.
                        let ne = scan(ctx, after, word_char).min(end);
                        ctx.emit(after, ne, Kind::Function);
                        let op = ctx.skip_spaces_from(ne);
                        if ctx.starts_with_at(op, b":=") {
                            ctx.emit(op, op + 2, Kind::Operator);
                            let t = ctx.skip_spaces_from(op + 2);
                            let te = scan(ctx, t, word_char).min(end);
                            ctx.emit(t, te, Kind::Function);
                            ctx.embed_spec(&JUST_SPEC, te, end);
                        }
                    }
                    _ => ctx.embed_spec(&JUST_SPEC, after, end),
                }
                return false;
            }
            _ if assign && we > s => {
                ctx.emit(s, we, Kind::Variable);
                ctx.emit(after, after + 2, Kind::Operator);
                ctx.embed_spec(&JUST_SPEC, after + 2, end);
                return false;
            }
            _ => break,
        }
    }
    // Recipe header: `[@]name param='default' +rest: dep (dep2 arg)`.
    let name_start = s + (ctx.at(s) == b'@') as usize;
    let name_end = scan(ctx, name_start, word_char).min(end);
    let Some(colon) = just_header_colon(ctx, name_end, end).filter(|_| name_end > name_start) else {
        ctx.embed_spec(&JUST_SPEC, s, end);
        return false;
    };
    ctx.emit(s, name_start, Kind::Operator);
    ctx.emit(name_start, name_end, Kind::Function);
    let mut i = name_end;
    let mut value = false;
    while i < colon {
        let b = ctx.src[i];
        if word_char(b) {
            let e = scan(ctx, i, word_char).min(colon);
            if !value {
                ctx.emit(i, e, Kind::Variable);
            }
            value = false;
            i = e;
        } else if matches!(b, b'"' | b'\'' | b'(' | b'`') {
            let e = if b == b'(' {
                closing(ctx, i, colon).map_or(colon, |c| c + 1)
            } else {
                ctx.src[i + 1..colon].iter().position(|&c| c == b).map_or(colon, |p| i + p + 2)
            };
            ctx.embed_spec(&JUST_SPEC, i, e);
            value = false;
            i = e;
        } else {
            if matches!(b, b'=' | b'+' | b'*' | b'$') {
                ctx.emit(i, i + 1, Kind::Operator);
                value = b == b'=';
            }
            i += 1;
        }
    }
    ctx.emit(colon, colon + 1, Kind::Operator);
    // Dependencies.
    let mut i = colon + 1;
    while i < end {
        let b = ctx.src[i];
        if word_char(b) {
            let e = scan(ctx, i, word_char).min(end);
            ctx.emit(i, e, Kind::Function);
            i = e;
        } else if b == b'(' {
            let e = closing(ctx, i, end).map_or(end, |c| c + 1);
            ctx.emit(i, i + 1, Kind::Punctuation);
            let e2 = scan(ctx, ctx.skip_spaces_from(i + 1), word_char).min(e);
            ctx.emit(ctx.skip_spaces_from(i + 1), e2, Kind::Function);
            ctx.embed_spec(&JUST_SPEC, e2, e);
            i = e;
        } else if b == b'#' {
            ctx.emit(i, end, Kind::Comment);
            break;
        } else {
            if b == b'&' {
                ctx.emit(i, i + 1, Kind::Operator);
            }
            i += 1;
        }
    }
    ctx.pos = end;
    true
}

/// The `:` ending a recipe header, outside strings and parentheses.
fn just_header_colon(ctx: &Ctx, from: usize, end: usize) -> Option<usize> {
    let mut i = from;
    let mut depth = 0;
    while i < end {
        match ctx.src[i] {
            q @ (b'"' | b'\'' | b'`') => {
                i = ctx.src[i + 1..end].iter().position(|&b| b == q).map_or(end, |p| i + p + 1);
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'#' if depth == 0 => return None,
            b':' if depth == 0 => return (ctx.at(i + 1) != b'=').then_some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Indented recipe body. A shebang line selects the language of the body.
fn just_body(ctx: &mut Ctx, start: usize) {
    let mut end = ctx.line_end(start);
    let mut pos = start;
    while pos < ctx.len() {
        let le = ctx.line_end(pos);
        let s = ctx.skip_spaces_from(pos);
        if s < le && !matches!(ctx.at(s), b'\r') {
            if s == pos {
                break;
            }
            end = le;
        }
        pos = le + 1;
    }
    let first = ctx.skip_spaces_from(start);
    if ctx.starts_with_at(first, b"#!") {
        let le = ctx.line_end(first);
        ctx.emit(first, le, Kind::Comment);
        let shebang = std::str::from_utf8(&ctx.src[first..le]).ok().and_then(Language::from_content);
        if let Some(language) = shebang.filter(|l| l.id() != "bash") {
            ctx.embed(language, le, end);
            ctx.pos = ctx.pos.max(end);
            return;
        }
        ctx.pos = le;
    }
    while ctx.pos < end {
        let ls = ctx.pos;
        let le = ctx.line_end(ls);
        recipe_line(ctx, ls, le, &JUST_RECIPE_SPEC);
        ctx.pos = (le + 1).min(ctx.len()).max(ctx.pos);
    }
    ctx.pos = ctx.pos.max(end);
}

pub static JUST: Definition = Definition::new("just", "Just", Engine::Custom(just))
    .aliases(&["justfile"])
    .extensions(&["just"])
    .filenames(&["justfile", "Justfile", ".justfile"]);

// Meson

static MESON_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("'''").raw().multiline(), Str::quoted("'")],
    string_prefixes: &[Prefix::plain(b'f')],
    keywords: &["if", "elif", "else", "endif", "foreach", "endforeach", "break", "continue", "and", "or", "not", "in"],
    builtins: &["meson", "host_machine", "build_machine", "target_machine"],
    constants: &["true", "false"],
    features: CALLS | MEMBERS | IDENT_KEYS,
    hook: Some(|lx| keyword_then(lx, b"foreach", (Kind::Keyword, Kind::Variable), None)),
    hook_chars: "f",
    ..Spec::DEFAULT
};

pub static MESON: Definition = Definition::new("meson", "Meson", Engine::Generic(&MESON_SPEC))
    .filenames(&["meson.build", "meson_options.txt", "meson.options"]);

// Ninja

/// `$var`, `${var}`, `$` escapes and the rule name after `build out:`.
fn ninja_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    if ctx.peek().is_ascii_alphabetic() {
        // Declarations start in the first column; indented names are variables.
        if start > 0 && ctx.src[start - 1] != b'\n' {
            return false;
        }
        let end = scan(ctx, start, is_ident);
        let word = &ctx.src[start..end];
        if word == b"rule" || word == b"pool" {
            return keyword_then(lx, word, (Kind::Keyword, Kind::Function), None);
        }
        if !matches!(word, b"build" | b"default" | b"include" | b"subninja") {
            return false;
        }
        ctx.emit(start, end, Kind::Keyword);
        ctx.pos = end;
        return true;
    }
    if ctx.peek() == b':' {
        let mut ls = start;
        while ls > 0 && ctx.src[ls - 1] != b'\n' {
            ls -= 1;
        }
        if !ctx.starts_with_at(ls, b"build ") {
            return false;
        }
        ctx.token(1, Kind::Operator);
        let s = ctx.skip_spaces_from(ctx.pos);
        let e = scan(ctx, s, |b| is_ident(b) || b == b'-');
        ctx.emit(s, e, Kind::Function);
        ctx.pos = e;
        return true;
    }
    let next = ctx.peek_at(1);
    let (end, kind) = match next {
        b'{' => (braced_end(ctx, start), Kind::Variable),
        b'$' | b' ' | b':' | b'\n' => (start + 2, Kind::Escape),
        b'\r' => (start + 2 + (ctx.at(start + 2) == b'\n') as usize, Kind::Escape),
        _ if is_ident(next) => (scan(ctx, start + 1, |b| is_ident(b) || b == b'-'), Kind::Variable),
        _ => return false,
    };
    ctx.emit(start, end, kind);
    ctx.pos = end;
    true
}

static NINJA_SPEC: Spec = Spec {
    line_comments: &["#"],
    ident_chars: "-.",
    operators: "=|@",
    features: LINE_KEYS | FLAGS,
    hook: Some(ninja_hook),
    hook_chars: "$:bdiprs",
    ..Spec::DEFAULT
};

pub static NINJA: Definition = Definition::new("ninja", "Ninja", Engine::Generic(&NINJA_SPEC)).extensions(&["ninja"]);

// Vim script

fn vim_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'"' => {
            // A comment starts a line, or has no closing quote on its line.
            let le = ctx.line_end(start);
            if statement_start(ctx, start, b"|") || !ctx.src[start + 1..le].contains(&b'"') {
                line_comment(ctx, start);
                return true;
            }
            false
        }
        b'#' => {
            // Vim9 script comments.
            if statement_start(ctx, start, b"") && matches!(next, b' ' | b'\t' | b'\n' | b'\r' | 0) {
                line_comment(ctx, start);
                return true;
            }
            false
        }
        b'&' => {
            // Options: `&tabstop`, `&l:shiftwidth`.
            if !next.is_ascii_alphabetic() {
                return false;
            }
            let mut end = start + 1;
            if ctx.at(end + 1) == b':' && matches!(next, b'l' | b'g') {
                end += 2;
            }
            let end = scan(ctx, end, is_ident);
            ctx.emit(start, end, Kind::Variable);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        b'<' => {
            // Key notation: `<leader>`, `<C-w>`, `<CR>`, `<silent>`.
            if !next.is_ascii_alphabetic() {
                return false;
            }
            let e = scan(ctx, start + 1, |b| b.is_ascii_alphanumeric() || b == b'-');
            if ctx.at(e) != b'>' || e - start > 24 {
                return false;
            }
            ctx.emit(start, e + 1, Kind::Constant);
            ctx.pos = e + 1;
            lx.mark_value();
            true
        }
        _ if next == b':' && is_ident(ctx.at(start + 2)) && !(start > 0 && is_ident(ctx.src[start - 1])) => {
            // Scoped names: `g:loaded`, `s:Helper()`, `a:000`, `v:true`.
            let end = scan(ctx, start + 2, |b| is_ident(b) || b == b'#');
            let kind = if ctx.next_non_space(end) == b'(' {
                Kind::Function
            } else if matches!(&ctx.src[start..end], b"v:true" | b"v:false" | b"v:null") {
                Kind::Constant
            } else {
                Kind::Variable
            };
            ctx.emit(start, end, kind);
            ctx.pos = end;
            lx.mark_value();
            true
        }
        _ => {
            // The first word of a statement is a command.
            if !statement_start(ctx, start, b"|") || !ctx.peek().is_ascii_alphabetic() {
                return false;
            }
            let end = lx.scan_ident(start);
            match &lx.ctx.src[start..end] {
                b"let" | b"const" | b"var" | b"unlet" => {
                    return keyword_then(lx, &lx.ctx.src[start..end], (Kind::Keyword, Kind::Variable), None)
                }
                b"set" | b"setlocal" | b"setglobal" | b"se" | b"setl" => {
                    vim_set(lx.ctx, start, end);
                    return true;
                }
                _ => {}
            }
            let kind = match lx.classify(start, end) {
                Kind::Text => Kind::Function,
                kind => kind,
            };
            lx.ctx.emit(start, end, kind);
            lx.ctx.pos = end;
            true
        }
    }
}

/// `set number tabstop=4 nowrap`: option names are properties.
fn vim_set(ctx: &mut Ctx, start: usize, end: usize) {
    ctx.emit(start, end, Kind::Keyword);
    let le = ctx.line_end(end);
    let mut i = end;
    while i < le {
        i = scan(ctx, i, |b| b == b' ' || b == b'\t');
        let b = ctx.at(i);
        if i >= le || b == b'|' || b == b'"' {
            break;
        }
        let name = scan(ctx, i, |b| b.is_ascii_alphanumeric() || b == b'_').min(le);
        ctx.emit(i, name, Kind::Property);
        let op = scan(ctx, name, |b| matches!(b, b'=' | b'+' | b'-' | b'^' | b'!' | b'&' | b'?' | b':')).min(le);
        ctx.emit(name, op, Kind::Operator);
        let mut value = op;
        while value < le && !matches!(ctx.src[value], b' ' | b'\t' | b'|' | b'"') {
            value += if ctx.src[value] == b'\\' { 2 } else { 1 };
        }
        let value = value.min(le);
        let number = value > op && ctx.src[op..value].iter().all(u8::is_ascii_digit);
        ctx.emit(op, value, if number { Kind::Number } else { Kind::Text });
        i = value.max(i + 1);
    }
    ctx.pos = i.min(le).max(end);
}

static VIM_SPEC: Spec = Spec {
    strings: &[Str::quoted("'").raw().doubled(), Str::quoted("\"")],
    keywords: &[
        "if", "elseif", "else", "endif", "while", "endwhile", "for", "endfor", "in", "try", "catch", "finally",
        "endtry", "endfunction", "endfunc", "endf", "enddef", "return", "let", "unlet", "const", "var", "call",
        "execute", "exe", "echo", "echom", "echomsg", "echoerr", "augroup", "autocmd", "au", "finish", "break",
        "continue", "throw", "set", "setlocal", "setglobal", "syntax", "highlight", "hi", "map", "noremap", "nnoremap",
        "inoremap", "vnoremap", "xnoremap", "onoremap", "tnoremap", "cnoremap", "nmap", "imap", "vmap", "xmap",
        "command", "filetype", "source", "runtime", "normal", "silent", "vim9script", "colorscheme", "abort", "range",
        "echohl", "redraw", "dict", "closure", "is", "isnot", "import", "export", "class", "endclass",
    ],
    builtins: &[
        "exists", "has", "expand", "fnamemodify", "get", "len", "empty", "join", "split", "map", "filter", "printf",
        "substitute", "matchstr", "strftime", "system", "tolower", "toupper", "type", "line", "col", "getline",
        "setline", "bufnr", "add", "extend", "keys", "values", "string",
    ],
    constants: &["true", "false", "null"],
    function_keywords: &["function", "fu", "func", "def"],
    ident_chars: "#",
    operators: "+-*/%=!<>&|?:.~",
    sigils: &[(b'$', Kind::Variable)],
    features: CALLS | FLAGS | STRING_KEYS,
    hook: Some(vim_hook),
    hook_chars: "\"#&<abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static VIM: Definition = Definition::new("vim", "Vim Script", Engine::Generic(&VIM_SPEC))
    .aliases(&["viml", "vimscript", "vim9"])
    .extensions(&["vim", "vimrc"])
    .filenames(&[".vimrc", "_vimrc", ".gvimrc", "vimrc", ".exrc"]);

// Ignore files

/// A gitignore style glob: wildcards are operators, `\x` is an escape and
/// everything else is `base`.
fn glob(ctx: &mut Ctx, start: usize, end: usize, base: Kind) {
    let mut i = start;
    let mut seg = start;
    while i < end {
        let (e, kind) = match ctx.src[i] {
            b'\\' => (char_end(ctx, i + 1).min(end), Kind::Escape),
            b'*' | b'?' => (i + 1, Kind::Operator),
            b'[' => (ctx.src[i..end].iter().position(|&b| b == b']').map_or(i + 1, |p| i + p + 1), Kind::Regex),
            b'/' => (i + 1, Kind::Punctuation),
            _ => {
                i += 1;
                continue;
            }
        };
        ctx.emit(seg, i, base);
        ctx.emit(i, e, kind);
        i = e;
        seg = e;
    }
    ctx.emit(seg, end, base);
}

fn gitignore(ctx: &mut Ctx) {
    while !ctx.done() {
        let start = ctx.pos;
        let end = ctx.line_end(start);
        let s = ctx.skip_spaces_from(start);
        match ctx.at(s) {
            b'#' => ctx.emit(s, end, Kind::Comment),
            b'!' => {
                ctx.emit(s, s + 1, Kind::Operator);
                glob(ctx, s + 1, end, Kind::Text);
            }
            _ => glob(ctx, s, end, Kind::Text),
        }
        ctx.pos = (end + 1).min(ctx.len());
    }
}

pub static GITIGNORE: Definition = Definition::new("gitignore", "Ignore List", Engine::Custom(gitignore))
    .aliases(&["ignore", "dockerignore", "npmignore"])
    .extensions(&["gitignore"])
    .filenames(&[".gitignore", ".dockerignore", ".npmignore", ".prettierignore", ".eslintignore", ".vscodeignore"]);

// INI style configuration (git config, systemd units)

/// Sections (`[core]`, `[remote "origin"]`) and `key = value` lines. The
/// `value` callback lexes each value given the key.
fn ini_like(ctx: &mut Ctx, value: fn(&mut Ctx, &[u8], usize, usize)) {
    while !ctx.done() {
        ctx.skip_whitespace();
        if ctx.done() {
            break;
        }
        let s = ctx.pos;
        let end = logical_line_end(ctx, s);
        match ctx.peek() {
            b'#' | b';' => line_comment(ctx, s),
            b'[' => {
                ctx.token(1, Kind::Punctuation);
                let close = ctx.src[s..end].iter().position(|&b| b == b']').map(|i| s + i);
                let inner_end = close.unwrap_or(end);
                let name_end = scan(ctx, s + 1, |b| !is_space(b) && b != b'"' && b != b']').min(inner_end);
                ctx.emit(s + 1, name_end, Kind::Type);
                ctx.embed_spec(&GITCONFIG_VALUE_SPEC, name_end, inner_end);
                if let Some(c) = close {
                    ctx.emit(c, c + 1, Kind::Punctuation);
                    ctx.embed_spec(&GITCONFIG_VALUE_SPEC, c + 1, end);
                }
            }
            _ => {
                let eq = ctx.src[s..end].iter().position(|&b| b == b'=').map(|i| s + i);
                let mut key_end = eq.unwrap_or(end);
                while key_end > s && is_space(ctx.src[key_end - 1]) {
                    key_end -= 1;
                }
                ctx.emit(s, key_end, Kind::Property);
                if let Some(eq) = eq {
                    ctx.emit(eq, eq + 1, Kind::Operator);
                    let v = ctx.skip_spaces_from(eq + 1);
                    value(ctx, &ctx.src[s..key_end], v, end);
                }
            }
        }
        ctx.pos = ctx.pos.max(end);
    }
}

static GITCONFIG_VALUE_SPEC: Spec = Spec {
    line_comments: &["#", ";"],
    strings: &[Str::quoted("\"")],
    constants: CONF_CONSTANTS,
    ident_chars: "-.",
    operators: "=",
    case_insensitive: true,
    features: FLAGS,
    ..Spec::DEFAULT
};

fn gitconfig_value(ctx: &mut Ctx, _key: &[u8], start: usize, end: usize) {
    // Shell aliases: `lg = !git log --oneline`.
    if ctx.at(start) == b'!' {
        ctx.emit(start, start + 1, Kind::Operator);
        shell_command(ctx, start + 1, end, &BASH_SPEC);
    } else {
        ctx.embed_spec(&GITCONFIG_VALUE_SPEC, start, end);
    }
}

pub static GITCONFIG: Definition =
    Definition::new("gitconfig", "Git Config", Engine::Custom(|ctx: &mut Ctx| ini_like(ctx, gitconfig_value)))
        .aliases(&["git-config", "gitmodules"])
        .extensions(&["gitconfig"])
        .filenames(&[".gitconfig", ".gitmodules"]);

static SYSTEMD_VALUE_SPEC: Spec = Spec { line_comments: &[], ..CONF_VALUE_SPEC };

fn systemd_value(ctx: &mut Ctx, key: &[u8], mut start: usize, end: usize) {
    if key.starts_with(b"Exec") {
        // `ExecStart=-/usr/bin/app --flag`: prefixes, then the program.
        let p = scan(ctx, start, |b| matches!(b, b'-' | b'@' | b':' | b'+' | b'!')).min(end);
        ctx.emit(start, p, Kind::Operator);
        let e = scan(ctx, p, |b| !is_space(b)).min(end);
        ctx.emit(p, e, Kind::Function);
        start = e;
    }
    ctx.embed_spec(&SYSTEMD_VALUE_SPEC, start, end);
}

pub static SYSTEMD: Definition =
    Definition::new("systemd", "systemd Unit", Engine::Custom(|ctx: &mut Ctx| ini_like(ctx, systemd_value)))
        .aliases(&["systemd-unit", "unit"])
        .extensions(&["service", "socket", "timer", "mount", "automount", "target", "slice", "path", "netdev"]);

// Git attributes

fn gitattributes(ctx: &mut Ctx) {
    while !ctx.done() {
        let start = ctx.pos;
        let end = ctx.line_end(start);
        let mut i = ctx.skip_spaces_from(start);
        if ctx.at(i) == b'#' {
            ctx.emit(i, end, Kind::Comment);
        } else {
            if ctx.starts_with_at(i, b"[attr]") {
                // Macro definition: `[attr]binary -diff -merge -text`.
                ctx.emit(i, i + 6, Kind::Keyword);
                let e = scan(ctx, i + 6, |b| !is_space(b)).min(end);
                ctx.emit(i + 6, e, Kind::Function);
                i = e;
            } else {
                let e = scan(ctx, i, |b| !is_space(b)).min(end);
                glob(ctx, i, e, Kind::Text);
                i = e;
            }
            while i < end {
                i = scan(ctx, i, is_space);
                if i >= end {
                    break;
                }
                let e = scan(ctx, i, |b| !is_space(b)).min(end);
                let p = scan(ctx, i, |b| b == b'-' || b == b'!').min(e);
                ctx.emit(i, p, Kind::Operator);
                let eq = ctx.src[p..e].iter().position(|&b| b == b'=').map_or(e, |x| p + x);
                ctx.emit(p, eq, Kind::Attribute);
                if eq < e {
                    ctx.emit(eq, eq + 1, Kind::Operator);
                    ctx.emit(eq + 1, e, Kind::String);
                }
                i = e;
            }
        }
        ctx.pos = (end + 1).min(ctx.len());
    }
}

pub static GITATTRIBUTES: Definition =
    Definition::new("gitattributes", "Git Attributes", Engine::Custom(gitattributes))
        .aliases(&["git-attributes"])
        .filenames(&[".gitattributes"]);

// SSH config

fn sshconfig(ctx: &mut Ctx) {
    while !ctx.done() {
        ctx.skip_whitespace();
        if ctx.done() {
            break;
        }
        let s = ctx.pos;
        let end = ctx.line_end(s);
        if ctx.peek() == b'#' {
            line_comment(ctx, s);
            continue;
        }
        let ke = scan(ctx, s, |b| b.is_ascii_alphanumeric()).min(end);
        let key = &ctx.src[s..ke];
        let keyword = key.eq_ignore_ascii_case(b"host") || key.eq_ignore_ascii_case(b"match");
        ctx.emit(s, ke, if keyword { Kind::Keyword } else { Kind::Property });
        let mut v = ctx.skip_spaces_from(ke);
        if ctx.at(v) == b'=' {
            ctx.emit(v, v + 1, Kind::Operator);
            v = ctx.skip_spaces_from(v + 1);
        }
        if key.eq_ignore_ascii_case(b"host") {
            // Host patterns: `*.example.com !bastion`.
            while v < end {
                let e = scan(ctx, v, |b| !is_space(b)).min(end);
                let p = v + (ctx.at(v) == b'!') as usize;
                ctx.emit(v, p, Kind::Operator);
                glob(ctx, p, e, Kind::Type);
                v = scan(ctx, e, is_space);
            }
        } else if key.len() > 7 && key[key.len() - 7..].eq_ignore_ascii_case(b"command") {
            // `ProxyCommand`, `LocalCommand` and friends run a shell command.
            shell_command(ctx, v, end, &BASH_SPEC);
        } else {
            ctx.embed_spec(&CONF_VALUE_SPEC, v, end);
        }
        ctx.pos = ctx.pos.max(end);
    }
}

pub static SSHCONFIG: Definition = Definition::new("sshconfig", "SSH Config", Engine::Custom(sshconfig))
    .aliases(&["ssh-config", "ssh_config", "sshdconfig"])
    .filenames(&["ssh_config", "sshd_config"]);

// Caddyfile

fn caddyfile(ctx: &mut Ctx) {
    let mut depth = 0u32;
    let mut first = true;
    while !ctx.done() {
        ctx.skip_whitespace();
        if ctx.done() {
            break;
        }
        let s = ctx.pos;
        let end = ctx.line_end(s);
        match ctx.peek() {
            b'#' => {
                line_comment(ctx, s);
                continue;
            }
            b'}' | b'{' => {}
            b'@' => {
                // Named matcher definition.
                let e = scan(ctx, s, |b| !is_space(b) && b != b'{').min(end);
                ctx.emit(s, e, Kind::Meta);
                ctx.pos = e;
            }
            b'(' if depth == 0 => {
                // Snippet: `(common) {`.
                let e = ctx.src[s..end].iter().position(|&b| b == b')').map_or(end, |i| s + i);
                ctx.emit(s, s + 1, Kind::Punctuation);
                ctx.emit(s + 1, e, Kind::Function);
                ctx.emit(e, e + 1, Kind::Punctuation);
                ctx.pos = (e + 1).min(end);
            }
            _ => {
                let mut trimmed = end;
                while trimmed > s && is_space(ctx.src[trimmed - 1]) {
                    trimmed -= 1;
                }
                if depth == 0 && (first || ctx.at(trimmed.saturating_sub(1)) == b'{') {
                    // Site addresses: `example.com, www.example.com {`.
                    while ctx.pos < end && ctx.peek() != b'{' {
                        let w = ctx.pos;
                        let e = scan(ctx, w, |b| !is_space(b) && b != b',' && b != b'{').min(end);
                        ctx.emit(w, e, Kind::Type);
                        let n = scan(ctx, e, |b| matches!(b, b' ' | b'\t' | b',' | b'\r'));
                        ctx.pos = if n > w { n } else { w + 1 }.min(end);
                    }
                } else {
                    let e = scan(ctx, s, |b| !is_space(b) && b != b'{').min(end);
                    ctx.emit(s, e, Kind::Keyword);
                    ctx.pos = e;
                }
            }
        }
        first = false;
        caddy_args(ctx, end, &mut depth);
        ctx.pos = ctx.pos.max(end);
    }
}

/// The arguments of a Caddyfile line up to `end`.
fn caddy_args(ctx: &mut Ctx, end: usize, depth: &mut u32) {
    while ctx.pos < end {
        let s = ctx.pos;
        let b = ctx.peek();
        match b {
            b' ' | b'\t' | b'\r' => ctx.pos += 1,
            b'#' if s > 0 && is_space(ctx.src[s - 1]) => {
                line_comment(ctx, s);
                return;
            }
            b'{' if matches!(ctx.peek_at(1), b' ' | b'\t' | b'\n' | b'\r' | 0) => {
                ctx.token(1, Kind::Punctuation);
                *depth += 1;
            }
            b'}' => {
                ctx.token(1, Kind::Punctuation);
                *depth = depth.saturating_sub(1);
            }
            b'"' => {
                let mut seg = s;
                ctx.pos += 1;
                while ctx.pos < end {
                    match ctx.peek() {
                        b'\\' => {
                            ctx.emit_from(seg, Kind::String);
                            let e = char_end(ctx, ctx.pos + 1).min(end);
                            ctx.emit(ctx.pos, e, Kind::Escape);
                            ctx.pos = e;
                            seg = e;
                        }
                        b'{' => {
                            ctx.emit_from(seg, Kind::String);
                            seg = caddy_placeholder(ctx, end);
                        }
                        b'"' => {
                            ctx.pos += 1;
                            break;
                        }
                        _ => ctx.pos += 1,
                    }
                }
                ctx.emit_from(seg, Kind::String);
            }
            b'`' => {
                // Backtick tokens are raw and may span lines.
                let e = ctx.src[s + 1..].iter().position(|&c| c == b'`').map_or(ctx.len(), |i| s + i + 2);
                ctx.emit(s, e, Kind::String);
                ctx.pos = e;
                return;
            }
            _ => {
                let e = scan(ctx, s, |c| !is_space(c) && c != b'"').min(end).max(s + 1);
                let word = &ctx.src[s..e];
                if b == b'@' {
                    ctx.emit(s, e, Kind::Meta);
                } else if b.is_ascii_digit() && word.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'.') {
                    ctx.emit(s, e, Kind::Number);
                } else if word == b"*" {
                    ctx.emit(s, e, Kind::Operator);
                } else {
                    // Placeholders may appear inside a word: `/srv{path}`.
                    while ctx.pos < e {
                        if ctx.peek() == b'{' {
                            caddy_placeholder(ctx, e);
                        } else {
                            ctx.pos += 1;
                        }
                    }
                }
                ctx.pos = ctx.pos.max(e);
            }
        }
    }
}

/// A `{placeholder}` at the cursor. Returns the position after it.
fn caddy_placeholder(ctx: &mut Ctx, end: usize) -> usize {
    let s = ctx.pos;
    // Placeholders are short; bounding the search keeps long lines linear.
    match ctx.src[s..end.min(s + 128)].iter().position(|&b| b == b'}' || b == b' ') {
        Some(i) if ctx.src[s + i] == b'}' && i > 1 => {
            ctx.emit(s, s + i + 1, Kind::Variable);
            ctx.pos = s + i + 1;
        }
        _ => ctx.pos = s + 1,
    }
    ctx.pos
}

pub static CADDYFILE: Definition =
    Definition::new("caddyfile", "Caddyfile", Engine::Custom(caddyfile)).aliases(&["caddy"]).filenames(&["Caddyfile"]);

// crontab

fn crontab(ctx: &mut Ctx) {
    while !ctx.done() {
        ctx.skip_whitespace();
        if ctx.done() {
            break;
        }
        let s = ctx.pos;
        let end = ctx.line_end(s);
        let b = ctx.peek();
        if b == b'#' {
            line_comment(ctx, s);
            continue;
        }
        let first = scan(ctx, s, |b| !is_space(b)).min(end);
        let name = scan(ctx, s, is_ident);
        if b == b'@' {
            // `@reboot`, `@daily` and friends.
            ctx.emit(s, first, Kind::Keyword);
            shell_command(ctx, first, end, &BASH_SPEC);
        } else if name > s && ctx.at(name) == b'=' {
            ctx.emit(s, name, Kind::Variable);
            ctx.emit(name, name + 1, Kind::Operator);
            ctx.embed_spec(&CONF_VALUE_SPEC, name + 1, end);
        } else {
            // Five schedule fields, then the command.
            let mut i = s;
            for _ in 0..5 {
                i = ctx.skip_spaces_from(i);
                let e = scan(ctx, i, |b| !is_space(b)).min(end);
                let mut j = i;
                while j < e {
                    let c = ctx.src[j];
                    let k = scan(ctx, j, |x| x.is_ascii_alphanumeric()).min(e).max(char_end(ctx, j));
                    let kind = if c.is_ascii_digit() {
                        Kind::Number
                    } else if c.is_ascii_alphabetic() {
                        Kind::Constant
                    } else {
                        Kind::Operator
                    };
                    ctx.emit(j, k, kind);
                    j = k;
                }
                i = e;
            }
            shell_command(ctx, i, end, &BASH_SPEC);
        }
        ctx.pos = ctx.pos.max(end);
    }
}

pub static CRONTAB: Definition = Definition::new("crontab", "crontab", Engine::Custom(crontab))
    .aliases(&["cron"])
    .extensions(&["cron", "crontab"])
    .filenames(&["crontab"]);

// Procfile

fn procfile(ctx: &mut Ctx) {
    while !ctx.done() {
        ctx.skip_whitespace();
        if ctx.done() {
            break;
        }
        let s = ctx.pos;
        let end = ctx.line_end(s);
        if ctx.peek() == b'#' {
            line_comment(ctx, s);
            continue;
        }
        let name = scan(ctx, s, |b| is_ident(b) || b == b'-').min(end);
        if name > s && ctx.at(name) == b':' {
            ctx.emit(s, name, Kind::Property);
            ctx.emit(name, name + 1, Kind::Punctuation);
            shell_command(ctx, name + 1, end, &BASH_SPEC);
        } else {
            shell_command(ctx, s, end, &BASH_SPEC);
        }
        ctx.pos = ctx.pos.max(end);
    }
}

pub static PROCFILE: Definition =
    Definition::new("procfile", "Procfile", Engine::Custom(procfile)).filenames(&["Procfile"]);

// xonsh

/// Subprocess operators `$(...)`, `!(...)`, `$[...]`, `![...]`, Python
/// injection `@(...)` and environment variables `$VAR`.
fn xonsh_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    let (open_len, shell) = match (ctx.peek(), next) {
        (b'$' | b'!', b'(' | b'[') => (2, true),
        (b'@', b'(') => (2, false),
        (b'@', b'$') if ctx.peek_at(2) == b'(' => (3, true),
        (b'$', b'{') => (2, false),
        (b'$', _) if is_ident_start(next) => {
            let end = scan(ctx, start + 1, is_ident);
            ctx.emit(start, end, Kind::Variable);
            ctx.pos = end;
            lx.mark_value();
            return true;
        }
        _ => return false,
    };
    let open = start + open_len - 1;
    let close = if ctx.at(open) == b'[' {
        b']'
    } else if ctx.at(open) == b'{' {
        b'}'
    } else {
        b')'
    };
    ctx.token(open_len, Kind::Punctuation);
    if shell {
        let end = closing(ctx, open, ctx.len()).unwrap_or(ctx.len());
        shell_command(ctx, open + 1, end, &XONSH_SUBPROC_SPEC);
    } else {
        lx.nested(close);
    }
    if lx.ctx.peek() == close {
        lx.ctx.token(1, Kind::Punctuation);
    }
    lx.mark_value();
    true
}

/// Python injection `@(expr)` inside subprocess mode.
fn xonsh_subproc_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    if ctx.peek() != b'@' {
        return bash_hook(lx);
    }
    if ctx.peek_at(1) != b'(' {
        return false;
    }
    let end = closing(ctx, start + 1, ctx.len()).unwrap_or(ctx.len());
    ctx.emit(start, start + 2, Kind::Punctuation);
    ctx.embed_spec(&XONSH_SPEC, start + 2, end);
    ctx.emit(end, end + 1, Kind::Punctuation);
    ctx.pos = (end + 1).min(ctx.len());
    lx.mark_value();
    true
}

static XONSH_SUBPROC_SPEC: Spec = Spec { hook: Some(xonsh_subproc_hook), hook_chars: "#$<@", ..BASH_RULES };

static XONSH_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"\"\"").multiline(), Str::quoted("'''").multiline(), Str::quoted("\""), Str::quoted("'")],
    string_prefixes: &[
        Prefix::raw(b'r'),
        Prefix::raw(b'R'),
        Prefix::interp(b'f', Interp::Brace),
        Prefix::interp(b'F', Interp::Brace),
        Prefix::plain(b'b'),
        Prefix::plain(b'p'),
    ],
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "case", "continue", "del", "elif", "else", "except",
        "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "match", "nonlocal", "not", "or",
        "pass", "raise", "return", "try", "while", "with", "yield", "xontrib",
    ],
    variables: &["self", "cls"],
    types: &["int", "float", "str", "bool", "bytes", "list", "dict", "set", "tuple", "object"],
    builtins: &[
        "print", "len", "range", "open", "isinstance", "enumerate", "zip", "map", "filter", "sorted", "any", "all",
        "aliases", "source", "cd", "echo",
    ],
    constants: &["True", "False", "None"],
    function_keywords: &["def"],
    type_keywords: &["class"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    hook: Some(xonsh_hook),
    hook_chars: "$!@",
    ..Spec::DEFAULT
};

pub static XONSH: Definition = Definition::new("xonsh", "Xonsh", Engine::Generic(&XONSH_SPEC))
    .aliases(&["xsh"])
    .extensions(&["xsh", "xonshrc"])
    .filenames(&[".xonshrc"]);

// Nushell

/// `$"..."` interpolated strings with `(expr)` parts.
fn nu_interpolated(lx: &mut Lexer, start: usize) {
    let quote = lx.ctx.at(start + 1);
    lx.ctx.pos = start + 2;
    let mut seg = start;
    loop {
        let ctx = &mut *lx.ctx;
        let rest = &ctx.src[ctx.pos..];
        ctx.pos +=
            rest.iter().position(|&b| b == quote || b == b'(' || (b == b'\\' && quote == b'"')).unwrap_or(rest.len());
        if ctx.done() {
            ctx.emit_from(seg, Kind::String);
            break;
        }
        let at = ctx.pos;
        match ctx.peek() {
            b'\\' => {
                ctx.emit_from(seg, Kind::String);
                ctx.pos += 1;
                if !ctx.done() {
                    ctx.bump_char();
                }
                ctx.emit_from(at, Kind::Escape);
                seg = ctx.pos;
            }
            b'(' if ctx.can_nest() => {
                ctx.emit_from(seg, Kind::String);
                ctx.token(1, Kind::Punctuation);
                lx.nested(b')');
                if lx.ctx.peek() == b')' {
                    lx.ctx.token(1, Kind::Punctuation);
                }
                seg = lx.ctx.pos;
            }
            b'(' => ctx.pos += 1,
            _ => {
                ctx.pos += 1;
                ctx.emit_from(seg, Kind::String);
                break;
            }
        }
    }
    lx.mark_value();
}

fn nu_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let b = ctx.peek();
    if b == b'$' {
        if !matches!(ctx.peek_at(1), b'"' | b'\'') {
            return false;
        }
        nu_interpolated(lx, start);
        return true;
    }
    if b == b'r' && ctx.peek_at(1) == b'#' {
        // Raw strings: `r#'...'#`.
        let hashes = scan(ctx, start + 1, |b| b == b'#') - start - 1;
        let open = start + 1 + hashes;
        if ctx.at(open) != b'\'' {
            return false;
        }
        let mut close = vec![b'\''];
        close.extend(std::iter::repeat(b'#').take(hashes));
        let end = ctx.find(open + 1, &close).map_or(ctx.len(), |i| i + close.len());
        ctx.emit(start, end, Kind::String);
        ctx.pos = end;
        lx.mark_value();
        return true;
    }
    let end = lx.scan_ident(start);
    let ctx = &mut *lx.ctx;
    match &ctx.src[start..end] {
        b"def" => return keyword_then(lx, b"def", (Kind::Keyword, Kind::Function), None),
        b"let" => return keyword_then(lx, b"let", (Kind::Keyword, Kind::Variable), None),
        b"mut" => return keyword_then(lx, b"mut", (Kind::Keyword, Kind::Variable), None),
        b"const" => return keyword_then(lx, b"const", (Kind::Keyword, Kind::Variable), None),
        b"export" | b"extern" => {
            ctx.emit(start, end, Kind::Keyword);
            ctx.pos = end;
            return true;
        }
        _ => {}
    }
    // Record keys and parameters: `{name: "x"}`, `[path: string]`.
    if ctx.at(end) == b':' && !matches!(ctx.at(end + 1), b':' | b'/') {
        ctx.emit(start, end, Kind::Property);
        ctx.pos = end;
        return true;
    }
    // Closure parameters `{|x| ...}` and the `_` pattern are not commands.
    let before = ctx.prev_non_space(start);
    let closure = before == b'|' && {
        let bar = ctx.src[..start].iter().rposition(|&b| b == b'|').unwrap_or(0);
        ctx.prev_non_space(bar) == b'{'
    };
    if closure || &ctx.src[start..end] == b"_" {
        ctx.emit(start, end, if closure { Kind::Variable } else { Kind::Text });
        ctx.pos = end;
        lx.mark_value();
        return true;
    }
    false
}

static NUSHELL_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\""), Str::quoted("'").raw(), Str::quoted("`").raw()],
    keywords: &[
        "def", "export", "extern", "let", "mut", "const", "if", "else", "match", "for", "in", "loop", "while", "break",
        "continue", "return", "try", "catch", "module", "use", "alias", "source", "source-env", "overlay", "hide",
        "where", "and", "or", "not", "xor", "mod", "do", "not-in", "starts-with", "ends-with", "like", "not-like",
    ],
    types: &[
        "int", "string", "float", "bool", "list", "record", "table", "any", "nothing", "path", "duration", "filesize",
        "datetime", "closure", "binary", "glob", "number", "range", "cell-path",
    ],
    builtins: &[
        "echo", "print", "ls", "cd", "open", "save", "get", "select", "each", "length", "sort-by", "str", "into",
        "from", "to", "http", "error", "help", "describe", "first", "last", "reduce", "append",
    ],
    constants: &["true", "false", "null"],
    sigils: &[(b'$', Kind::Variable)],
    ident_chars: "-",
    operators: "+-*/%=!<>|&~?.",
    features: COMMANDS | FLAGS | MEMBERS,
    hook: Some(nu_hook),
    hook_chars: "$abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_",
    ..Spec::DEFAULT
};

pub static NUSHELL: Definition =
    Definition::new("nushell", "Nushell", Engine::Generic(&NUSHELL_SPEC)).aliases(&["nu"]).extensions(&["nu"]);
