//! C, C++, Rust and other languages with C like syntax.

use super::{Definition, Engine};
use crate::lexer::generic::{features::*, Lexer, Prefix, Spec, Str};
use crate::lexer::is_ident;
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[&C, &CPP, &RUST];

// Rust

/// Lifetimes, char literals, raw strings, attributes and `.await`.
fn rust_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'\'' => {
            // Char literal: 'a', '\n', '\u{1F600}', or a multi byte character.
            if ctx.peek_at(1) == b'\\' {
                let end = ctx.src[start + 2..].iter().take(12).position(|&b| b == b'\'');
                if let Some(i) = end {
                    ctx.pos = start + 2 + i + 1;
                    ctx.emit(start, ctx.pos, Kind::String);
                    lx.mark_value();
                    return true;
                }
                return false;
            }
            let mut i = start + 1;
            if i < ctx.len() {
                i += 1;
                while i < ctx.len() && (ctx.src[i] & 0xC0) == 0x80 {
                    i += 1;
                }
            }
            if ctx.at(i) == b'\'' && ctx.at(start + 1) != b'\'' {
                ctx.pos = i + 1;
                ctx.emit(start, ctx.pos, Kind::String);
                lx.mark_value();
                return true;
            }
            // Lifetime or loop label.
            if is_ident(ctx.peek_at(1)) {
                let end = lx.scan_ident(start + 1);
                lx.ctx.pos = end;
                lx.ctx.emit(start, end, Kind::Meta);
                return true;
            }
            false
        }
        b'#' => {
            let inner = ctx.peek_at(1) == b'!';
            let open = if inner { 2 } else { 1 };
            if ctx.peek_at(open) != b'[' {
                return false;
            }
            ctx.token(open + 1, Kind::Meta);
            ctx.skip_spaces();
            let s = ctx.pos;
            while is_ident(ctx.peek()) || (ctx.peek() == b':' && ctx.peek_at(1) == b':') {
                ctx.pos += if ctx.peek() == b':' { 2 } else { 1 };
            }
            ctx.emit_from(s, Kind::Meta);
            lx.nested(b']');
            if lx.ctx.peek() == b']' {
                lx.ctx.token(1, Kind::Meta);
            }
            true
        }
        b'r' | b'b' | b'c' => {
            // Raw strings: r"..", r#".."#, br"..", cr#".."#, and byte chars b'x'.
            let mut i = start;
            if matches!(ctx.at(i), b'b' | b'c') {
                if ctx.at(i) == b'b' && ctx.at(i + 1) == b'\'' {
                    let close = ctx.src[i + 2..].iter().take(12).position(|&b| b == b'\'');
                    if let Some(c) = close {
                        ctx.pos = i + 2 + c + 1;
                        ctx.emit(start, ctx.pos, Kind::String);
                        lx.mark_value();
                        return true;
                    }
                    return false;
                }
                i += 1;
            }
            if ctx.at(i) != b'r' {
                return false;
            }
            i += 1;
            let hashes = ctx.src[i..].iter().take_while(|&&b| b == b'#').count();
            i += hashes;
            if ctx.at(i) != b'"' {
                return false;
            }
            let mut close = vec![b'"'];
            close.extend(std::iter::repeat(b'#').take(hashes));
            let end = ctx.find(i + 1, &close).map_or(ctx.len(), |p| p + close.len());
            ctx.pos = end;
            ctx.emit(start, end, Kind::String);
            lx.mark_value();
            true
        }
        b'.' if ctx.starts_with(b".await") && !is_ident(ctx.peek_at(6)) => {
            ctx.token(1, Kind::Punctuation);
            ctx.token(5, Kind::Keyword);
            lx.mark_value();
            true
        }
        _ => false,
    }
}

static RUST_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[Str::quoted("\"").multiline()],
    string_prefixes: &[Prefix::plain(b'b'), Prefix::plain(b'c')],
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "extern", "for", "if", "impl",
        "in", "let", "loop", "macro_rules", "match", "mod", "move", "mut", "pub", "ref", "return", "static", "super",
        "unsafe", "use", "where", "while", "yield", "gen", "Self",
    ],
    variables: &["self"],
    types: &[
        "bool", "char", "str", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64",
    ],
    constants: &["true", "false"],
    function_keywords: &["fn"],
    type_keywords: &["struct", "enum", "trait", "type", "union"],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | BANG_MACROS,
    hook: Some(rust_hook),
    hook_chars: "'#rbc.",
    ..Spec::DEFAULT
};

pub static RUST: Definition =
    Definition::new("rust", "Rust", Engine::Generic(&RUST_SPEC)).aliases(&["rs"]).extensions(&["rs"]);

// C and C++

const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "const", "continue", "default", "do", "else", "extern", "for", "goto", "if", "inline",
    "register", "restrict", "return", "sizeof", "static", "switch", "typedef", "volatile", "while", "_Alignas",
    "_Alignof", "_Atomic", "_Generic", "_Noreturn", "_Static_assert", "_Thread_local", "alignas", "alignof",
    "static_assert", "thread_local", "typeof",
];

const C_TYPES: &[&str] = &[
    "char", "double", "float", "int", "long", "short", "signed", "unsigned", "void", "bool", "_Bool", "_Complex",
    "size_t", "ssize_t", "ptrdiff_t", "intptr_t", "uintptr_t", "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t",
    "uint16_t", "uint32_t", "uint64_t", "wchar_t", "FILE", "va_list",
];

const C_BUILTINS: &[&str] = &["stdin", "stdout", "stderr", "errno"];

const C_STRINGS: &[Str] = &[Str::quoted("\""), Str::quoted("'")];

const C_PREFIXES: &[Prefix] = &[Prefix::plain(b'L'), Prefix::plain(b'u'), Prefix::plain(b'U'), Prefix::plain(b'8')];

static C_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: C_STRINGS,
    string_prefixes: C_PREFIXES,
    keywords: C_KEYWORDS,
    types: C_TYPES,
    builtins: C_BUILTINS,
    constants: &["NULL", "true", "false", "nullptr", "EOF"],
    type_keywords: &["struct", "enum", "union"],
    features: CALLS | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    ..Spec::DEFAULT
};

/// C++ raw strings: `R"delim(...)delim"`.
fn cpp_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let mut i = start;
    // Optional encoding prefix: u8R, uR, UR, LR.
    if ctx.starts_with(b"u8") {
        i += 2;
    } else if matches!(ctx.peek(), b'u' | b'U' | b'L') {
        i += 1;
    }
    if ctx.at(i) != b'R' || ctx.at(i + 1) != b'"' {
        return false;
    }
    let delim_start = i + 2;
    let Some(paren) = ctx.src[delim_start..].iter().take(17).position(|&b| b == b'(') else {
        return false;
    };
    let mut close = vec![b')'];
    close.extend_from_slice(&ctx.src[delim_start..delim_start + paren]);
    close.push(b'"');
    let end = ctx.find(delim_start + paren, &close).map_or(ctx.len(), |p| p + close.len());
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    lx.mark_value();
    true
}

static CPP_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: C_STRINGS,
    string_prefixes: C_PREFIXES,
    keywords: &[
        "alignas", "alignof", "asm", "auto", "break", "case", "catch", "co_await", "co_return", "co_yield", "concept",
        "const", "const_cast", "consteval", "constexpr", "constinit", "continue", "decltype", "default", "delete",
        "do", "dynamic_cast", "else", "explicit", "export", "extern", "final", "for", "friend", "goto", "if", "import",
        "inline", "module", "mutable", "namespace", "new", "noexcept", "operator", "override", "private", "protected",
        "public", "register", "reinterpret_cast", "requires", "return", "sizeof", "static", "static_assert",
        "static_cast", "switch", "template", "this", "thread_local", "throw", "try", "typedef", "typeid", "typename",
        "using", "virtual", "volatile", "while", "and", "or", "not", "xor",
    ],
    types: &[
        "bool", "char", "char8_t", "char16_t", "char32_t", "double", "float", "int", "long", "short", "signed",
        "unsigned", "void", "wchar_t", "size_t", "ssize_t", "ptrdiff_t", "intptr_t", "uintptr_t", "int8_t", "int16_t",
        "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "FILE",
    ],
    builtins: &["std", "cout", "cin", "cerr", "endl", "stdin", "stdout", "stderr"],
    constants: &["NULL", "true", "false", "nullptr"],
    type_keywords: &["class", "struct", "enum", "union"],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    hook: Some(cpp_hook),
    hook_chars: "RuUL",
    ..Spec::DEFAULT
};

pub static C: Definition = Definition::new("c", "C", Engine::Generic(&C_SPEC)).extensions(&["c", "h"]);

pub static CPP: Definition = Definition::new("cpp", "C++", Engine::Generic(&CPP_SPEC))
    .aliases(&["c++", "cxx", "cc", "hpp"])
    .extensions(&["cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "ipp", "tpp", "ino"]);
