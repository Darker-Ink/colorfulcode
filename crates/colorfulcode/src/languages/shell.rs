//! Shells and build tool languages.

use super::{Definition, Engine};
use crate::lexer::generic::{features::*, Interp, Lexer, Spec, Str};
use crate::lexer::is_ident;
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[&BASH];

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

static BASH_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[
        Str::quoted("\"").multiline().interp(Interp::DollarParen),
        Str::quoted("'").raw().multiline(),
        Str::quoted("`").multiline().interp(Interp::DollarParen),
    ],
    keywords: &[
        "if", "then", "else", "elif", "fi", "case", "esac", "for", "select", "while", "until", "do", "done", "in",
        "function", "time", "coproc",
    ],
    builtins: &[
        "alias", "bg", "bind", "break", "builtin", "caller", "cd", "command", "compgen", "complete", "continue",
        "declare", "dirs", "disown", "echo", "enable", "eval", "exec", "exit", "export", "false", "fc", "fg",
        "getopts", "hash", "help", "history", "jobs", "kill", "let", "local", "logout", "mapfile", "popd", "printf",
        "pushd", "pwd", "read", "readarray", "readonly", "return", "set", "shift", "shopt", "source", "suspend",
        "test", "times", "trap", "true", "type", "typeset", "ulimit", "umask", "unalias", "unset", "wait",
    ],
    ident_chars: "-.",
    operators: "+-*/%=&|^!<>~?:",
    features: COMMANDS | FLAGS,
    hook: Some(bash_hook),
    hook_chars: "#$<",
    ..Spec::DEFAULT
};

pub static BASH: Definition = Definition::new("bash", "Bash", Engine::Generic(&BASH_SPEC))
    .aliases(&["sh", "shell", "zsh", "ksh", "shellscript", "console"])
    .extensions(&["sh", "bash", "zsh", "ksh", "command", "ebuild", "eclass"])
    .filenames(&[
        ".bashrc", ".bash_profile", ".bash_aliases", ".profile", ".zshrc", ".zprofile", ".zshenv", "PKGBUILD",
        "APKBUILD",
    ]);
