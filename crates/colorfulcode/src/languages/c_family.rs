//! C, C++, Rust and other languages with C like syntax.

use super::{Definition, Engine};
use crate::lexer::generic::{block_comment, features::*, scan_number, Interp, Lexer, Prefix, Spec, Str};
use crate::lexer::{is_ident, is_ident_start, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[
    &C, &CPP, &RUST, &GO, &JAVA, &CSHARP, &KOTLIN, &SWIFT, &SCALA, &DART, &OBJECTIVE_C, &CUDA, &ZIG, &D, &GROOVY,
    &SOLIDITY, &GLSL, &HLSL, &WGSL, &V, &ODIN, &HAXE, &VALA, &APEX, &ACTIONSCRIPT, &HARE, &CARBON,
];

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
            let end = hashed_close(ctx, i + 1, hashes);
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
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | BANG_MACROS,
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
    let mut close = [0u8; 18];
    close[0] = b')';
    close[1..=paren].copy_from_slice(&ctx.src[delim_start..delim_start + paren]);
    close[paren + 1] = b'"';
    let close = &close[..paren + 2];
    let end = ctx.find(delim_start + paren, close).map_or(ctx.len(), |p| p + close.len());
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    lx.mark_value();
    true
}

const CPP_KEYWORDS: &[&str] = &[
    "alignas", "alignof", "asm", "auto", "break", "case", "catch", "co_await", "co_return", "co_yield", "concept",
    "const", "const_cast", "consteval", "constexpr", "constinit", "continue", "decltype", "default", "delete", "do",
    "dynamic_cast", "else", "explicit", "export", "extern", "final", "for", "friend", "goto", "if", "import", "inline",
    "module", "mutable", "namespace", "new", "noexcept", "operator", "override", "private", "protected", "public",
    "register", "reinterpret_cast", "requires", "return", "sizeof", "static", "static_assert", "static_cast", "switch",
    "template", "this", "thread_local", "throw", "try", "typedef", "typeid", "typename", "using", "virtual",
    "volatile", "while", "and", "or", "not", "xor",
];

const CPP_TYPES: &[&str] = &[
    "bool", "char", "char8_t", "char16_t", "char32_t", "double", "float", "int", "long", "short", "signed", "unsigned",
    "void", "wchar_t", "size_t", "ssize_t", "ptrdiff_t", "intptr_t", "uintptr_t", "int8_t", "int16_t", "int32_t",
    "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "FILE",
];

static CPP_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: C_STRINGS,
    string_prefixes: C_PREFIXES,
    keywords: CPP_KEYWORDS,
    types: CPP_TYPES,
    builtins: &["std", "cout", "cin", "cerr", "endl", "stdin", "stdout", "stderr"],
    constants: &["NULL", "true", "false", "nullptr"],
    type_keywords: &["class", "struct", "enum", "union"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    hook: Some(cpp_hook),
    hook_chars: "RuUL",
    ..Spec::DEFAULT
};

pub static C: Definition = Definition::new("c", "C", Engine::Generic(&C_SPEC)).extensions(&["c", "h"]);

pub static CPP: Definition = Definition::new("cpp", "C++", Engine::Generic(&CPP_SPEC))
    .aliases(&["c++", "cxx", "cc", "hpp"])
    .extensions(&["cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "ipp", "tpp", "ino"]);

// Shared helpers

/// The word that ends at `pos`, ignoring spaces and tabs in between.
fn word_before<'s>(ctx: &Ctx<'s, '_>, pos: usize) -> &'s [u8] {
    let src = ctx.src;
    let mut end = pos.min(src.len());
    while end > 0 && matches!(src[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_ident(src[start - 1]) {
        start -= 1;
    }
    &src[start..end]
}

/// End of a raw string whose body starts at `from` and closes with `"` and `hashes` hashes.
fn hashed_close(ctx: &Ctx, from: usize, hashes: usize) -> usize {
    let src = ctx.src;
    let mut i = from.min(src.len());
    while let Some(q) = src[i..].iter().position(|&b| b == b'"') {
        let q = i + q;
        let n = src[q + 1..].iter().take(hashes).take_while(|&&b| b == b'#').count();
        if n == hashes {
            return q + 1 + hashes;
        }
        i = q + 1;
    }
    src.len()
}

/// `[Attribute(args)]` at the start of a line or before a parameter.
fn bracket_attribute(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    let first = ctx.peek_at(1);
    if !first.is_ascii_alphabetic() || !ctx.can_nest() {
        return false;
    }
    let param = first.is_ascii_uppercase() && matches!(ctx.prev_non_space(start), b'(' | b',');
    if !param && !ctx.at_line_start(start) {
        return false;
    }
    if !ctx.src[start..ctx.line_end_within(start, 1024)].contains(&b']') {
        return false;
    }
    let mut end = lx.scan_ident(start + 1);
    while ctx.at(end) == b'.' && is_ident(ctx.at(end + 1)) {
        end = lx.scan_ident(end + 1);
    }
    // Attribute targets: `[assembly: Name]`, `[return: Name]`.
    if ctx.at(end) == b':' && ctx.at(end + 1) != b':' {
        end = lx.scan_ident(ctx.skip_spaces_from(end + 1));
        while ctx.at(end) == b'.' && is_ident(ctx.at(end + 1)) {
            end = lx.scan_ident(end + 1);
        }
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Meta);
    lx.nested(b']');
    if lx.ctx.peek() == b']' {
        lx.ctx.token(1, Kind::Meta);
    }
    true
}

/// Char literals such as `'a'` and `'\n'`; otherwise `'name` is a symbol.
fn char_or_symbol(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let end = if ctx.peek_at(1) == b'\\' {
        ctx.src[start + 2..].iter().take(10).position(|&b| b == b'\'').map(|i| start + 2 + i + 1)
    } else {
        let mut i = start + 1;
        if i < ctx.len() && ctx.src[i] != b'\n' {
            i += 1;
            while i < ctx.len() && (ctx.src[i] & 0xC0) == 0x80 {
                i += 1;
            }
        }
        (ctx.at(i) == b'\'' && i > start + 1).then_some(i + 1)
    };
    if let Some(end) = end {
        ctx.pos = end;
        ctx.emit(start, end, Kind::String);
    } else if is_ident_start(ctx.peek_at(1)) {
        let end = lx.scan_ident(start + 1);
        lx.ctx.pos = end;
        lx.ctx.emit(start, end, Kind::Constant);
    } else {
        return false;
    }
    lx.mark_value();
    true
}

/// Whether the token before `pos` reads like a type (`int x`, `List<T> x`, `Foo? x`).
fn type_before(ctx: &Ctx, pos: usize, types: &[&str]) -> bool {
    let mut i = pos;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    if i == pos || i == 0 {
        return false;
    }
    match ctx.src[i - 1] {
        b'>' => !matches!(ctx.at(i.wrapping_sub(2)), b'=' | b'-'),
        b']' | b'?' => true,
        _ => {
            let w = word_before(ctx, i);
            !w.is_empty() && (w[0].is_ascii_uppercase() || types.iter().any(|t| t.as_bytes() == w))
        }
    }
}

/// Whether an operator, a spaced ternary or `return` comes right before `pos`, so a name
/// there is a value.
fn value_before(ctx: &Ctx, pos: usize) -> bool {
    let mut i = pos;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    let spaced = ctx.at(i.wrapping_sub(2)) == b' ';
    match ctx.at(i.wrapping_sub(1)) {
        b'=' | b'+' | b'-' | b'*' | b'/' | b'%' | b'!' | b'&' | b'|' => true,
        b'>' => ctx.at(i.wrapping_sub(2)) == b'=',
        // `c ? First : Last`, but not `Foo? x` or `class A : Base`.
        b'?' => spaced,
        b':' => spaced && ctx.find_in(line_start_within(ctx, i, 256), i, b" ? ").is_some(),
        b'n' => matches!(word_before(ctx, i), b"return" | b"in"),
        _ => false,
    }
}

/// Start of the line containing `pos`, looking back at most `max` bytes.
fn line_start_within(ctx: &Ctx, pos: usize, max: usize) -> usize {
    let stop = pos.saturating_sub(max);
    ctx.src[stop..pos].iter().rposition(|&b| b == b'\n').map_or(stop, |i| stop + i + 1)
}

/// The innermost bracket left open before `pos` (`(`, `[`, `{`, or `<` right after a name)
/// and its offset. Looks back at most 512 bytes, gives up at `;` or a closed block, and only
/// accepts a `{` on the same line.
fn enclosing_open(ctx: &Ctx, pos: usize) -> Option<(u8, usize)> {
    let src = ctx.src;
    let stop = pos.saturating_sub(512);
    let (mut depth, mut lines) = (0u32, false);
    let mut i = pos.min(src.len());
    while i > stop {
        i -= 1;
        match src[i] {
            b'\n' => lines = true,
            b')' | b']' => depth += 1,
            b'}' if depth > 0 => depth += 1,
            b'{' if depth == 0 && lines => return None,
            b'(' | b'[' | b'{' if depth == 0 => return Some((src[i], i)),
            b'(' | b'[' | b'{' => depth -= 1,
            b'<' if depth == 0 && i > 0 && is_ident(src[i - 1]) => return Some((b'<', i)),
            b';' | b'}' if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

/// Whether the `(` at `paren` holds call arguments or a condition (`f(Name)`, `if (Done)`,
/// `return (A, B)`) rather than types (`typeof(Foo)`, `catch (Exception)`, `(Foo, Bar) pair`).
fn argument_list(ctx: &Ctx, paren: usize) -> bool {
    let mut i = paren;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    let tight = i == paren;
    match ctx.at(i.wrapping_sub(1)) {
        b'>' => tight,
        b if is_ident(b) => {
            let word = word_before(ctx, i);
            if tight {
                !matches!(word, b"typeof" | b"sizeof" | b"default" | b"catch")
            } else {
                matches!(word, b"if" | b"while" | b"switch" | b"return" | b"in")
            }
        }
        _ => value_before(ctx, paren),
    }
}

/// For languages with PascalCase members (C#): `Name = x`, `string Name { get; }`, `{Name}`,
/// `f(Name)`, `x >= Limit` and `return Name;` make the name a property instead of a type.
fn pascal_member(lx: &mut Lexer, types: &[&str]) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    if lx.pending().is_some() || (start > 0 && ctx.src[start - 1] == b'.') {
        return false;
    }
    let end = lx.scan_ident(start);
    let mut i = end;
    while ctx.at(i).is_ascii_whitespace() {
        i += 1;
    }
    let gap = i > end;
    let prev = ctx.prev_non_space(start);
    // Format specifiers: `{Total:C2}`.
    if ctx.at(start.wrapping_sub(1)) == b':' && ctx.at(end) == b'}' {
        lx.ctx.pos = end;
        lx.ctx.emit(start, end, Kind::String);
        return true;
    }
    // `{Name}`, `f(A, B)`, `new[] { A, B }`, `xs[Index]`.
    let listed = || match prev {
        b'{' => true,
        b'(' | b',' | b'[' => match enclosing_open(ctx, start) {
            Some((b'(', p)) => argument_list(ctx, p),
            Some((b, _)) => b != b'<',
            None => false,
        },
        _ => false,
    };
    let member = match ctx.at(i) {
        b'=' if ctx.at(i + 1) == b'>' => type_before(ctx, start, types),
        b'=' => true,
        // `(Foo)x` is a cast.
        b')' if prev == b'(' && !gap && (is_ident_start(ctx.at(i + 1)) || ctx.at(i + 1) == b'(') => false,
        b'}' | b',' | b';' | b')' | b']' | b'{' | 0 => {
            listed() || type_before(ctx, start, types) || value_before(ctx, start)
        }
        // Named tuple elements and arguments, format specifiers in `{Total:C2}`.
        b':' if ctx.at(i + 1) != b':' => matches!(prev, b'(' | b',' | b'{') || value_before(ctx, start),
        b'?' => gap || matches!(ctx.at(i + 1), b'?' | b'.'),
        b'>' | b'<' if !gap => ctx.at(i + 1) == b'=',
        b'/' => gap && !matches!(ctx.at(i + 1), b'/' | b'*'),
        b'!' | b'&' | b'|' | b'+' | b'-' | b'*' | b'%' | b'^' | b'<' | b'>' => gap,
        _ => false,
    };
    if !member {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Property);
    lx.mark_value();
    true
}

/// `#!` on the first line.
fn shebang(ctx: &mut Ctx) -> bool {
    if ctx.pos == 0 && ctx.starts_with(b"#!") {
        ctx.skip_line();
        ctx.emit_from(0, Kind::Comment);
        return true;
    }
    false
}

// Go

const GO_EXPR_KEYWORDS: &[&[u8]] = &[
    b"return", b"case", b"go", b"defer", b"if", b"for", b"range", b"else", b"switch", b"select", b"const", b"var",
    b"package", b"import", b"goto", b"break", b"continue", b"func",
];

/// Standard packages whose exported members are constants, variables or functions.
const GO_PACKAGES: &[&[u8]] = &[
    b"atomic", b"base64", b"big", b"binary", b"bufio", b"bytes", b"context", b"errors", b"exec", b"filepath", b"flag",
    b"fmt", b"fs", b"hex", b"http", b"io", b"ioutil", b"json", b"log", b"math", b"net", b"os", b"path", b"reflect",
    b"regexp", b"runtime", b"signal", b"slices", b"slog", b"sort", b"sql", b"strconv", b"strings", b"sync", b"syscall",
    b"template", b"time", b"tls", b"unicode", b"url", b"utf8",
];

/// Whether a name at `pos` sits where Go expects a type: `*T`, `[]T`, `x T`, `) T`,
/// `func(T)`, `(T, error)`, `x.(T)` and type parameters `[T any]`.
fn go_type_position(ctx: &Ctx, pos: usize) -> bool {
    let src = ctx.src;
    let mut i = pos;
    while i > 0 && matches!(src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    if i == 0 {
        return false;
    }
    let gap = i < pos;
    match src[i - 1] {
        // `*T` and `[]T`, but not `5*time.Second`.
        b'*' => !gap && !(is_ident(ctx.at(i.wrapping_sub(2))) || ctx.at(i.wrapping_sub(2)) == b')'),
        b']' => !gap,
        b')' => gap,
        b if is_ident(b) => gap && !GO_EXPR_KEYWORDS.contains(&word_before(ctx, i)),
        b'(' | b',' | b'[' => match enclosing_open(ctx, pos) {
            Some((b'(', p)) => {
                let before = ctx.prev_non_space(p);
                before == b')' || before == b'.' || matches!(word_before(ctx, p), b"func" | b"new")
            }
            Some((b'[', p)) => {
                // `func Map[T any]`, `type List[T any]`.
                let mut name = p;
                while name > 0 && is_ident(src[name - 1]) {
                    name -= 1;
                }
                name < p && matches!(word_before(ctx, name), b"func" | b"type")
            }
            _ => false,
        },
        _ => false,
    }
}

/// For a name alone on its line: a constant in a `const (...)` or `var (...)` block, an
/// embedded type in a struct or interface.
fn go_block_member(ctx: &Ctx, pos: usize) -> Kind {
    let src = ctx.src;
    let stop = pos.saturating_sub(512);
    // Look at up to 16 earlier lines, nearest first, for one that decides.
    let mut end = pos;
    for _ in 0..16 {
        let Some(nl) = src[stop..end].iter().rposition(|&b| b == b'\n') else {
            break;
        };
        let line_end = stop + nl;
        let line_start = src[stop..line_end].iter().rposition(|&b| b == b'\n').map_or(stop, |i| stop + i + 1);
        end = line_end;
        let line = src[line_start..line_end].trim_ascii();
        match line.last() {
            None => continue,
            _ if line.starts_with(b"//") => continue,
            Some(b'{') => return Kind::Type,
            Some(b'(') => return Kind::Constant,
            _ if line.contains(&b'=') => return Kind::Constant,
            // A field such as `Name string`.
            _ if line.iter().any(|b| b.is_ascii_whitespace()) => return Kind::Type,
            _ => {}
        }
    }
    Kind::Constant
}

/// Exported names: types in type positions, constants elsewhere, and qualified names such as
/// `*http.Request` and `http.StatusOK`.
fn go_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    if lx.pending().is_some() {
        return false;
    }
    let qualified = ctx.peek() == b'.';
    // For `pkg.Name`, the start of `pkg`.
    let mut q = start;
    let name = if qualified {
        if !ctx.peek_at(1).is_ascii_uppercase() {
            return false;
        }
        while q > 0 && is_ident(ctx.src[q - 1]) {
            q -= 1;
        }
        if q == start || ctx.at(q.wrapping_sub(1)) == b'.' {
            return false;
        }
        start + 1
    } else {
        if start > 0 && ctx.src[start - 1] == b'.' {
            return false;
        }
        start
    };
    let end = lx.scan_ident(name);
    let next = ctx.skip_spaces_from(end);
    let after = ctx.at(next);
    if after == b'(' {
        return false;
    }
    let alone = matches!(after, b'\n' | b'\r' | b'/' | 0) && ctx.at_line_start(q);
    let kind = if qualified {
        if go_type_position(ctx, q) || ctx.at(end) == b'{' {
            Kind::Type
        } else if alone {
            // Embedded types such as `sync.Mutex`.
            go_block_member(ctx, q)
        } else if GO_PACKAGES.contains(&&ctx.src[q..start]) {
            Kind::Constant
        } else {
            return false;
        }
    } else if ctx.at(end) == b'{' {
        Kind::Type
    } else if ctx.at_line_start(start) && (is_ident_start(after) || matches!(after, b'*' | b'[')) {
        // Struct fields (`Name string`), type specs in a `type (...)` block,
        // and const specs (`Admin Role = iota`).
        let ty = ctx.skip_spaces_from(end);
        let ty_end = lx.scan_ident(ty);
        match &ctx.src[ty..ty_end] {
            b"struct" | b"interface" => Kind::Type,
            _ if ty_end > ty && ctx.next_non_space(ty_end) == b'=' => Kind::Constant,
            _ => Kind::Property,
        }
    } else if go_type_position(ctx, start) {
        Kind::Type
    } else if alone {
        go_block_member(ctx, start)
    } else if after == b':'
        && !matches!(ctx.at(next + 1), b'=' | b':')
        && (ctx.at_line_start(start)
            || match ctx.prev_non_space(start) {
                b'{' => true,
                b',' => matches!(enclosing_open(ctx, start), Some((b'{', _))),
                _ => false,
            })
    {
        // Keys in composite literals: `User{Name: "x"}`, but not `case A, B:`.
        Kind::Property
    } else {
        Kind::Constant
    };
    let ctx = &mut *lx.ctx;
    if qualified {
        ctx.token(1, Kind::Punctuation);
    }
    ctx.pos = end;
    ctx.emit(name, end, kind);
    lx.mark_value();
    true
}

static GO_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'"), Str::quoted("`").raw().multiline()],
    keywords: &[
        "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "go", "goto",
        "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "var",
    ],
    types: &[
        "any", "bool", "byte", "comparable", "complex64", "complex128", "error", "float32", "float64", "int", "int8",
        "int16", "int32", "int64", "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr",
    ],
    builtins: &[
        "append", "cap", "clear", "close", "complex", "copy", "delete", "imag", "len", "make", "max", "min", "new",
        "panic", "print", "println", "real", "recover",
    ],
    constants: &["true", "false", "nil", "iota"],
    function_keywords: &["func"],
    type_keywords: &["type"],
    features: CALLS | MEMBERS,
    hook: Some(go_hook),
    hook_chars: ".ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static GO: Definition =
    Definition::new("go", "Go", Engine::Generic(&GO_SPEC)).aliases(&["golang"]).extensions(&["go"]);

// Java

static JAVA_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"\"\"").multiline(), Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "abstract", "assert", "break", "case", "catch", "continue", "default", "do", "else", "exports", "extends",
        "final", "finally", "for", "if", "implements", "import", "instanceof", "module", "native", "new", "non",
        "open", "opens", "package", "permits", "private", "protected", "provides", "public", "requires", "return",
        "sealed", "static", "strictfp", "super", "switch", "synchronized", "throw", "throws", "transient", "try",
        "uses", "var", "volatile", "when", "while", "with", "yield",
    ],
    types: &["boolean", "byte", "char", "double", "float", "int", "long", "short", "void"],
    variables: &["this"],
    constants: &["true", "false", "null"],
    type_keywords: &["class", "interface", "enum", "record"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static JAVA: Definition = Definition::new("java", "Java", Engine::Generic(&JAVA_SPEC)).extensions(&["java", "jsh"]);

// C#

const CS_TYPES: &[&str] = &[
    "bool", "byte", "char", "decimal", "double", "dynamic", "float", "int", "long", "nint", "nuint", "object", "sbyte",
    "short", "string", "uint", "ulong", "ushort", "var", "void",
];

static CS_PLAIN: Str = Str::quoted("\"");
static CS_VERBATIM: Str = Str::quoted("\"").raw().doubled().multiline();

/// Interpolated `$"..."`, verbatim `@"..."` and raw `$"""..."""` strings, and `@name` identifiers.
fn csharp_string(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    let mut i = start;
    let (mut dollars, mut verbatim) = (0, false);
    loop {
        match ctx.at(i) {
            b'$' => dollars += 1,
            b'@' if !verbatim => verbatim = true,
            _ => break,
        }
        i += 1;
    }
    if ctx.at(i) != b'"' {
        if verbatim && i == start + 1 && is_ident_start(ctx.at(i)) {
            lx.ctx.pos = lx.scan_ident(i);
            lx.mark_value();
            return true;
        }
        return false;
    }
    let raw = ctx.starts_with_at(i, b"\"\"\"");
    lx.ctx.pos = i;
    if raw {
        csharp_raw_string(lx, start, dollars);
        return true;
    }
    let rule = if verbatim { &CS_VERBATIM } else { &CS_PLAIN };
    let interp = if dollars == 1 { Interp::Brace } else { Interp::None };
    lx.string(start, rule, false, interp);
    true
}

/// Raw strings `"""..."""`, with as many quotes as they open with. With `dollars` leading `$`,
/// a run of that many braces opens an interpolation and shorter runs are literal text.
fn csharp_raw_string(lx: &mut Lexer, start: usize, dollars: usize) {
    let ctx = &mut *lx.ctx;
    let quotes = ctx.src[ctx.pos..].iter().take_while(|&&b| b == b'"').count();
    ctx.pos += quotes;
    let mut seg = start;
    loop {
        let ctx = &mut *lx.ctx;
        let rest = &ctx.src[ctx.pos..];
        let Some(p) = rest.iter().position(|&b| b == b'"' || (b == b'{' && dollars > 0)) else {
            ctx.pos = ctx.len();
            break;
        };
        let p = ctx.pos + p;
        let run = ctx.src[p..].iter().take_while(|&&b| b == ctx.src[p]).count();
        ctx.pos = p + run;
        if ctx.src[p] == b'"' {
            if run >= quotes {
                break;
            }
            continue;
        }
        if run < dollars || !ctx.can_nest() {
            continue;
        }
        // Extra braces before the hole are literal.
        ctx.pos = p + run - dollars;
        ctx.emit_from(seg, Kind::String);
        ctx.token(dollars, Kind::Punctuation);
        lx.nested(b'}');
        let ctx = &mut *lx.ctx;
        let close = ctx.src[ctx.pos..].iter().take(dollars).take_while(|&&b| b == b'}').count();
        ctx.token(close, Kind::Punctuation);
        seg = ctx.pos;
    }
    lx.ctx.emit_from(seg, Kind::String);
    lx.mark_value();
}

/// Whether the `{` at `pos` opens an enum body: `enum Name {` or `enum Name : byte {`.
fn enum_body(ctx: &Ctx, pos: usize) -> bool {
    let src = ctx.src;
    let back_ws = |mut i: usize| {
        while i > 0 && src[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        i
    };
    let back_word = |mut i: usize| {
        while i > 0 && is_ident(src[i - 1]) {
            i -= 1;
        }
        i
    };
    let mut end = back_ws(pos);
    let mut start = back_word(end);
    let colon = back_ws(start);
    if colon > 0 && src[colon - 1] == b':' && start < end {
        end = back_ws(colon - 1);
        start = back_word(end);
    }
    if start == end {
        return false;
    }
    let kw = back_ws(start);
    &src[back_word(kw)..kw] == b"enum"
}

/// Enum bodies: member names are constants. Gives up at `;` or `{`, which never appear in one.
fn csharp_enum(lx: &mut Lexer) {
    lx.ctx.token(1, Kind::Punctuation);
    loop {
        lx.ctx.skip_whitespace();
        if lx.ctx.done() {
            break;
        }
        let ctx = &mut *lx.ctx;
        let start = ctx.pos;
        match ctx.peek() {
            b'}' => {
                ctx.token(1, Kind::Punctuation);
                break;
            }
            b';' | b'{' => break,
            b'/' if ctx.peek_at(1) == b'/' => {
                ctx.skip_line();
                ctx.emit_from(start, Kind::Comment);
            }
            b'/' if ctx.peek_at(1) == b'*' => block_comment(ctx, "/*", "*/", false),
            b'[' => {
                if !bracket_attribute(lx) {
                    lx.ctx.token(1, Kind::Punctuation);
                }
            }
            b'"' => lx.string(start, &CS_PLAIN, false, Interp::None),
            b'\'' => lx.string(start, &C_STRINGS[1], false, Interp::None),
            b if b.is_ascii_digit() => {
                scan_number(ctx, false);
                ctx.emit_from(start, Kind::Number);
            }
            b if is_ident_start(b) => {
                let end = lx.scan_ident(start);
                let kind = if start > 0 && lx.ctx.src[start - 1] == b'.' {
                    Kind::Property
                } else {
                    lx.word_kind(&lx.ctx.src[start..end]).unwrap_or(Kind::Constant)
                };
                lx.ctx.pos = end;
                lx.ctx.emit(start, end, kind);
            }
            b',' | b'(' | b')' | b'.' => ctx.token(1, Kind::Punctuation),
            b if b.is_ascii_punctuation() => ctx.token(1, Kind::Operator),
            _ => ctx.bump_char(),
        }
    }
    lx.mark_value();
}

fn csharp_hook(lx: &mut Lexer) -> bool {
    match lx.ctx.peek() {
        b'[' => bracket_attribute(lx),
        b'$' | b'@' => csharp_string(lx),
        b'"' if lx.ctx.starts_with(b"\"\"\"") => {
            csharp_raw_string(lx, lx.ctx.pos, 0);
            true
        }
        b'{' => {
            if !enum_body(lx.ctx, lx.ctx.pos) {
                return false;
            }
            csharp_enum(lx);
            true
        }
        _ => pascal_member(lx, CS_TYPES),
    }
}

static CSHARP_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[CS_PLAIN, Str::quoted("'")],
    keywords: &[
        "abstract", "and", "as", "async", "await", "base", "break", "case", "catch", "checked", "const", "continue",
        "default", "delegate", "do", "else", "event", "explicit", "extern", "file", "finally", "fixed", "for",
        "foreach", "get", "global", "goto", "if", "implicit", "in", "init", "internal", "is", "lock", "namespace",
        "not", "operator", "or", "out", "override", "params", "partial", "private", "protected", "public", "readonly",
        "ref", "required", "return", "sealed", "set", "sizeof", "stackalloc", "static", "switch", "throw", "try",
        "typeof", "unchecked", "unsafe", "using", "virtual", "volatile", "when", "where", "while", "with", "yield",
    ],
    types: CS_TYPES,
    builtins: &["nameof"],
    variables: &["this"],
    constants: &["true", "false", "null"],
    type_keywords: &["class", "struct", "interface", "enum", "record", "new"],
    features: CALLS | CAPITALIZED_TYPES | MEMBERS | PREPROCESSOR,
    hook: Some(csharp_hook),
    hook_chars: "\"$@[{ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static CSHARP: Definition =
    Definition::new("csharp", "C#", Engine::Generic(&CSHARP_SPEC)).aliases(&["cs", "c#"]).extensions(&["cs", "csx"]);

// Kotlin

static KOTLIN_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[
        Str::quoted("\"\"\"").raw().multiline().interp(Interp::Dollar),
        Str::quoted("\"").interp(Interp::Dollar),
        Str::quoted("'"),
    ],
    keywords: &[
        "abstract", "actual", "annotation", "as", "break", "by", "catch", "companion", "const", "constructor",
        "continue", "crossinline", "data", "do", "else", "enum", "expect", "external", "final", "finally", "for",
        "fun", "get", "if", "import", "in", "infix", "init", "inline", "inner", "internal", "is", "lateinit",
        "noinline", "open", "operator", "out", "override", "package", "private", "protected", "public", "reified",
        "return", "sealed", "set", "super", "suspend", "tailrec", "throw", "try", "val", "value", "var", "vararg",
        "when", "where", "while",
    ],
    builtins: &[
        "println", "print", "listOf", "mutableListOf", "mapOf", "mutableMapOf", "setOf", "arrayOf", "require", "check",
        "error", "lazy", "repeat", "TODO",
    ],
    variables: &["this", "it", "field"],
    constants: &["true", "false", "null"],
    type_keywords: &["class", "interface", "object", "typealias"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static KOTLIN: Definition = Definition::new("kotlin", "Kotlin", Engine::Generic(&KOTLIN_SPEC))
    .aliases(&["kt", "kts"])
    .extensions(&["kt", "kts"]);

// Swift

/// Raw strings `#"..."#` close with `"` and the same number of hashes; `\#(x)` interpolates
/// and `\#n` escapes.
fn swift_raw_string(lx: &mut Lexer, start: usize, hashes: usize) {
    let mut seg = start;
    loop {
        let ctx = &mut *lx.ctx;
        let Some(p) = ctx.src[ctx.pos..].iter().position(|&b| b == b'"' || b == b'\\') else {
            ctx.pos = ctx.len();
            break;
        };
        let p = ctx.pos + p;
        let marked = ctx.src[p + 1..].iter().take(hashes).take_while(|&&b| b == b'#').count() == hashes;
        ctx.pos = p + 1;
        if !marked {
            continue;
        }
        if ctx.src[p] == b'"' {
            ctx.pos = p + 1 + hashes;
            break;
        }
        // `\#(` interpolation or `\#n` escape.
        let after = p + 1 + hashes;
        ctx.emit(seg, p, Kind::String);
        if ctx.at(after) == b'(' && ctx.can_nest() {
            ctx.pos = p;
            ctx.token(hashes + 2, Kind::Punctuation);
            lx.nested(b')');
            if lx.ctx.peek() == b')' {
                lx.ctx.token(1, Kind::Punctuation);
            }
        } else {
            ctx.pos = after;
            if ctx.peek() == b'u' && ctx.peek_at(1) == b'{' {
                let close = ctx.src[after..].iter().take(12).position(|&b| b == b'}');
                ctx.pos = close.map_or(after + 1, |c| after + c + 1);
            } else if !ctx.done() {
                ctx.bump_char();
            }
            ctx.emit_from(p, Kind::Escape);
        }
        seg = lx.ctx.pos;
    }
    lx.ctx.emit_from(seg, Kind::String);
    lx.mark_value();
}

/// Raw strings `#"..."#`, directives such as `#if` and `#selector`, closure arguments `$0`,
/// and all caps initializers such as `UUID()`.
fn swift_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'$' => {
            let end = lx.scan_ident(start + 1);
            if end == start + 1 {
                return false;
            }
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Variable);
            lx.mark_value();
            true
        }
        b'#' => {
            let hashes = ctx.src[start..].iter().take_while(|&&b| b == b'#').count();
            let i = start + hashes;
            if ctx.at(i) == b'"' {
                let quotes = if ctx.starts_with_at(i, b"\"\"\"") { 3 } else { 1 };
                ctx.pos = i + quotes;
                swift_raw_string(lx, start, hashes);
                return true;
            }
            if hashes == 1 && is_ident_start(ctx.at(i)) {
                let end = lx.scan_ident(i);
                lx.ctx.pos = end;
                lx.ctx.emit(start, end, Kind::Meta);
                return true;
            }
            false
        }
        _ => {
            // Swift functions are lowerCamelCase, so `URL(string:)` builds a type.
            let end = lx.scan_ident(start);
            let ctx = &*lx.ctx;
            if ctx.at(end) != b'(' || end - start < 2 || lx.pending().is_some() || ctx.at(start.wrapping_sub(1)) == b'.'
            {
                return false;
            }
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Type);
            lx.mark_value();
            true
        }
    }
}

static SWIFT_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[
        Str::quoted("\"\"\"").multiline().interp(Interp::BackslashParen),
        Str::quoted("\"").interp(Interp::BackslashParen),
    ],
    keywords: &[
        "any", "as", "associatedtype", "async", "await", "break", "case", "catch", "continue", "convenience",
        "default", "defer", "deinit", "didSet", "do", "dynamic", "else", "fallthrough", "fileprivate", "final", "for",
        "get", "guard", "if", "import", "in", "indirect", "infix", "init", "inout", "internal", "is", "isolated",
        "lazy", "let", "macro", "mutating", "nonisolated", "nonmutating", "open", "operator", "optional", "override",
        "package", "postfix", "precedencegroup", "prefix", "private", "public", "repeat", "required", "rethrows",
        "return", "set", "some", "static", "subscript", "super", "switch", "throw", "throws", "try", "unowned", "var",
        "weak", "where", "while", "willSet",
    ],
    types: &["Self"],
    builtins: &["print", "debugPrint", "fatalError", "precondition", "assert", "min", "max", "abs", "zip", "stride"],
    variables: &["self"],
    constants: &["true", "false", "nil"],
    function_keywords: &["func"],
    type_keywords: &["class", "struct", "enum", "protocol", "extension", "actor", "typealias"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS | ANNOTATIONS,
    hook: Some(swift_hook),
    hook_chars: "#$ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static SWIFT: Definition = Definition::new("swift", "Swift", Engine::Generic(&SWIFT_SPEC)).extensions(&["swift"]);

// Scala

const SCALA_STRINGS: &[Str] = &[Str::quoted("\"\"\"").raw().multiline(), Str::quoted("\"")];

/// Interpolated strings `s"..."`, `f"..."`, `raw"..."` and custom ones such as `sql"..."`, then
/// char literals and symbols.
fn scala_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let quote = ctx.pos;
    if ctx.peek() == b'\'' {
        return char_or_symbol(lx);
    }
    let mut start = quote;
    while start > 0 && is_ident(ctx.src[start - 1]) {
        start -= 1;
    }
    if start == quote || !is_ident_start(ctx.src[start]) {
        return false;
    }
    let rule = &SCALA_STRINGS[if ctx.starts_with(b"\"\"\"") { 0 } else { 1 }];
    let raw = &ctx.src[start..quote] == b"raw";
    ctx.truncate_to(start);
    lx.string(start, rule, raw, Interp::Dollar);
    true
}

static SCALA_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: SCALA_STRINGS,
    keywords: &[
        "abstract", "case", "catch", "derives", "do", "else", "end", "export", "extends", "extension", "final",
        "finally", "for", "forSome", "given", "if", "implicit", "import", "inline", "lazy", "match", "new", "opaque",
        "open", "override", "package", "private", "protected", "return", "sealed", "super", "then", "throw",
        "transparent", "try", "using", "val", "var", "while", "with", "yield",
    ],
    builtins: &["println", "print", "require", "assert"],
    variables: &["this"],
    constants: &["true", "false", "null", "None"],
    function_keywords: &["def"],
    type_keywords: &["class", "trait", "object", "type", "enum"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    hook: Some(scala_hook),
    hook_chars: "'\"",
    ..Spec::DEFAULT
};

pub static SCALA: Definition =
    Definition::new("scala", "Scala", Engine::Generic(&SCALA_SPEC)).extensions(&["scala", "sc", "sbt"]);

// Dart

static DART_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[
        Str::quoted("'''").multiline().interp(Interp::Dollar),
        Str::quoted("\"\"\"").multiline().interp(Interp::Dollar),
        Str::quoted("'").interp(Interp::Dollar),
        Str::quoted("\"").interp(Interp::Dollar),
    ],
    // Raw strings `r'...'` have no escapes or interpolation.
    string_prefixes: &[Prefix::raw(b'r')],
    keywords: &[
        "abstract", "as", "assert", "async", "await", "base", "break", "case", "catch", "const", "continue",
        "covariant", "default", "deferred", "do", "dynamic", "else", "export", "extends", "external", "factory",
        "final", "finally", "for", "get", "hide", "if", "implements", "import", "in", "interface", "is", "late",
        "library", "new", "on", "operator", "part", "required", "rethrow", "return", "sealed", "set", "show", "static",
        "super", "switch", "sync", "throw", "try", "var", "when", "while", "with", "yield",
    ],
    types: &["int", "double", "num", "bool", "void"],
    builtins: &["print"],
    variables: &["this"],
    constants: &["true", "false", "null"],
    type_keywords: &["class", "mixin", "enum", "extension", "typedef"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static DART: Definition = Definition::new("dart", "Dart", Engine::Generic(&DART_SPEC)).extensions(&["dart"]);

// Objective-C

const OBJC_AT_KEYWORDS: &[&[u8]] = &[
    b"interface", b"implementation", b"end", b"property", b"synthesize", b"dynamic", b"protocol", b"class",
    b"selector", b"encode", b"autoreleasepool", b"try", b"catch", b"finally", b"throw", b"optional", b"required",
    b"import", b"synchronized", b"public", b"private", b"protected", b"package", b"available",
];

/// `@keywords`, `@"strings"` and selector parts in messages and method declarations. Selector
/// names are found at the `:`, `]` or `)` after them and re-kinded.
fn objc_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    let src = ctx.src;
    match ctx.peek() {
        b'@' => {
            if ctx.peek_at(1) == b'"' {
                lx.ctx.pos += 1;
                lx.string(start, &C_STRINGS[0], false, Interp::None);
                return true;
            }
            let end = lx.scan_ident(start + 1);
            if !OBJC_AT_KEYWORDS.contains(&&src[start + 1..end]) {
                return false;
            }
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Keyword);
            true
        }
        b':' => {
            // `setName:` in a message or declaration, but not `case X:`, `default:` or `a ? b : c`.
            let mut name = start;
            while name > 0 && is_ident(src[name - 1]) {
                name -= 1;
            }
            let word = &src[name..start];
            if name < start && ctx.peek_at(1) != b':' && word != b"default" && word_before(ctx, name) != b"case" {
                lx.ctx.truncate_to(name);
                lx.ctx.emit(name, start, Kind::Function);
            }
            false
        }
        b']' => {
            // `[receiver message]`
            let mut end = start;
            while end > 0 && matches!(src[end - 1], b' ' | b'\t') {
                end -= 1;
            }
            let name = end - word_before(ctx, end).len();
            let p = ctx.prev_non_space(name);
            if name < end
                && matches!(ctx.at(name.wrapping_sub(1)), b' ' | b'\t' | b'\n')
                && is_ident_start(src[name])
                && (is_ident(p) || p == b']' || p == b')')
            {
                lx.ctx.truncate_to(name);
                lx.ctx.emit(name, end, Kind::Function);
            }
            false
        }
        _ => {
            // `- (void)viewDidLoad`: the name right after the return type.
            let Some(open) = src[start.saturating_sub(64)..start].iter().rposition(|&b| b == b'(' || b == b')') else {
                return false;
            };
            let open = start.saturating_sub(64) + open;
            let mut sign = open;
            while sign > 0 && matches!(src[sign - 1], b' ' | b'\t') {
                sign -= 1;
            }
            if src[open] != b'(' || !matches!(ctx.at(sign.wrapping_sub(1)), b'-' | b'+') || !ctx.at_line_start(sign - 1)
            {
                return false;
            }
            let name = ctx.skip_spaces_from(start + 1);
            if !is_ident_start(ctx.at(name)) {
                return false;
            }
            let end = lx.scan_ident(name);
            lx.ctx.token(1, Kind::Punctuation);
            lx.ctx.pos = end;
            lx.ctx.emit(name, end, Kind::Function);
            lx.mark_value();
            true
        }
    }
}

static OBJC_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: C_STRINGS,
    string_prefixes: C_PREFIXES,
    keywords: &[
        "auto", "break", "case", "const", "continue", "default", "do", "else", "extern", "for", "goto", "if", "inline",
        "register", "restrict", "return", "sizeof", "static", "switch", "typedef", "volatile", "while", "in", "out",
        "inout", "bycopy", "byref", "oneway", "nonatomic", "atomic", "strong", "weak", "assign", "copy", "retain",
        "readonly", "readwrite", "nullable", "nonnull", "getter", "setter", "__block", "__weak", "__strong",
        "__bridge", "super",
    ],
    types: &[
        "char", "double", "float", "int", "long", "short", "signed", "unsigned", "void", "bool", "id", "instancetype",
        "BOOL", "SEL", "IMP", "size_t", "int32_t", "int64_t", "uint8_t", "uint32_t", "uint64_t",
    ],
    variables: &["self", "_cmd"],
    constants: &["nil", "Nil", "YES", "NO", "NULL", "true", "false"],
    type_keywords: &["struct", "enum", "union"],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    hook: Some(objc_hook),
    hook_chars: "@:])",
    ..Spec::DEFAULT
};

pub static OBJECTIVE_C: Definition = Definition::new("objective-c", "Objective-C", Engine::Generic(&OBJC_SPEC))
    .aliases(&["objc", "obj-c", "objectivec", "objective-c++", "objc++"])
    .extensions(&["m", "mm"]);

// CUDA

/// `__global__` style qualifiers, then C++ raw strings.
fn cuda_hook(lx: &mut Lexer) -> bool {
    if lx.ctx.peek() != b'_' {
        return cpp_hook(lx);
    }
    let start = lx.ctx.pos;
    let end = lx.scan_ident(start);
    let word = &lx.ctx.src[start..end];
    if word.len() > 4 && word.starts_with(b"__") && word.ends_with(b"__") {
        lx.ctx.pos = end;
        lx.ctx.emit(start, end, Kind::Keyword);
        return true;
    }
    false
}

static CUDA_SPEC: Spec = Spec {
    builtins: &[
        "threadIdx", "blockIdx", "blockDim", "gridDim", "warpSize", "__syncthreads", "cudaMalloc", "cudaFree",
        "cudaMemcpy", "cudaDeviceSynchronize", "atomicAdd", "std", "printf",
    ],
    types: &[
        "bool", "char", "double", "float", "int", "long", "short", "signed", "unsigned", "void", "size_t", "int8_t",
        "int32_t", "int64_t", "uint8_t", "uint32_t", "uint64_t", "dim3", "half", "float2", "float3", "float4", "int2",
        "int3", "int4", "uint2", "uint3", "uint4", "cudaError_t", "cudaStream_t",
    ],
    hook: Some(cuda_hook),
    hook_chars: "RuUL_",
    ..CPP_SPEC
};

pub static CUDA: Definition =
    Definition::new("cuda", "CUDA", Engine::Generic(&CUDA_SPEC)).aliases(&["cu"]).extensions(&["cu", "cuh"]);

// Zig

/// Multiline string lines: `\\ text`.
fn zig_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    if ctx.peek_at(1) != b'\\' {
        return false;
    }
    let start = ctx.pos;
    ctx.skip_line();
    ctx.emit_from(start, Kind::String);
    lx.mark_value();
    true
}

static ZIG_SPEC: Spec = Spec {
    line_comments: &["//"],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "addrspace", "align", "allowzero", "and", "anyframe", "asm", "async", "await", "break", "callconv", "catch",
        "comptime", "const", "continue", "defer", "else", "enum", "errdefer", "error", "export", "extern", "for", "if",
        "inline", "linksection", "noalias", "noinline", "nosuspend", "opaque", "or", "orelse", "packed", "pub",
        "resume", "return", "struct", "suspend", "switch", "test", "threadlocal", "try", "union", "unreachable",
        "usingnamespace", "var", "volatile", "while",
    ],
    types: &[
        "bool", "void", "noreturn", "type", "anyerror", "anyopaque", "anytype", "comptime_int", "comptime_float",
        "isize", "usize", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f16", "f32", "f64",
        "f80", "f128", "c_int", "c_uint", "c_long", "c_char",
    ],
    constants: &["true", "false", "null", "undefined"],
    function_keywords: &["fn"],
    sigils: &[(b'@', Kind::Builtin)],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS,
    hook: Some(zig_hook),
    hook_chars: "\\",
    ..Spec::DEFAULT
};

pub static ZIG: Definition = Definition::new("zig", "Zig", Engine::Generic(&ZIG_SPEC)).extensions(&["zig", "zon"]);

// D

/// Token strings `q{...}` and delimited strings `q"(...)"`, `q"/.../"`, `q"EOS ... EOS"`.
/// `enum NAME = value;` and `enum int NAME = value;` declare constants, while
/// `enum Name { ... }` declares a type (left to the generic rules).
fn d_manifest_constant(lx: &mut Lexer) -> bool {
    let src = lx.ctx.src;
    let start = lx.ctx.pos;
    if !src[start..].starts_with(b"enum") || is_ident(lx.ctx.at(start + 4)) || (start > 0 && is_ident(src[start - 1])) {
        return false;
    }
    // Collect the words up to the first `=`, `{`, `;` or `:` on this line.
    let limit = lx.ctx.line_end_within(start, 256);
    let mut words = [(0usize, 0usize); 4];
    let mut count = 0;
    let mut i = start + 4;
    while i < limit {
        let b = src[i];
        if is_ident(b) {
            let e = lx.scan_ident(i);
            if count == words.len() {
                return false;
            }
            words[count] = (i, e);
            count += 1;
            i = e;
        } else if matches!(b, b' ' | b'\t') {
            i += 1;
        } else {
            break;
        }
    }
    if count == 0 || lx.ctx.at(i) != b'=' || lx.ctx.at(i + 1) == b'=' {
        return false;
    }
    lx.ctx.emit(start, start + 4, Kind::Keyword);
    for (n, &(s, e)) in words[..count].iter().enumerate() {
        let kind = if n + 1 == count { Kind::Constant } else { lx.word_kind(&src[s..e]).unwrap_or(Kind::Type) };
        lx.ctx.emit(s, e, kind);
    }
    lx.ctx.pos = words[count - 1].1;
    true
}

fn d_hook(lx: &mut Lexer) -> bool {
    if lx.ctx.peek() == b'e' {
        return d_manifest_constant(lx);
    }
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    let src = ctx.src;
    let open = ctx.peek_at(2);
    let end = match ctx.peek_at(1) {
        b'{' => bracket_close(src, start + 1, b'{', b'}', true).map(|p| p + 1),
        b'"' => match open {
            b'(' | b'[' | b'{' | b'<' => {
                let close = match open {
                    b'(' => b')',
                    b'[' => b']',
                    b'{' => b'}',
                    _ => b'>',
                };
                bracket_close(src, start + 2, open, close, false).map(|p| p + 1)
            }
            b if is_ident_start(b) => {
                // Heredoc: the identifier ends the string at the start of a line.
                let id_end = lx.scan_ident(start + 2);
                let id = &src[start + 2..id_end];
                let mut from = id_end;
                loop {
                    match ctx.find(from, id) {
                        Some(p) if src[p - 1] == b'\n' && ctx.at(p + id.len()) == b'"' => break Some(p + id.len()),
                        Some(p) => from = p + 1,
                        None => break None,
                    }
                }
            }
            0 | b'\n' | b' ' => return false,
            _ => ctx.find(start + 3, &[open, b'"']).map(|p| p + 1),
        },
        _ => return false,
    };
    let ctx = &mut *lx.ctx;
    // Skip the closing quote and an optional `c`, `w` or `d` suffix.
    let mut end = end.map_or(ctx.len(), |e| e + usize::from(ctx.at(start + 1) == b'"' && ctx.at(e) == b'"'));
    if matches!(ctx.at(end), b'c' | b'w' | b'd') && !is_ident(ctx.at(end + 1)) {
        end += 1;
    }
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    lx.mark_value();
    true
}

/// Offset of the `close` that balances the `open` at `from`, skipping `"strings"` if `tokens`.
fn bracket_close(src: &[u8], from: usize, open: u8, close: u8, tokens: bool) -> Option<usize> {
    let (mut depth, mut quoted) = (0usize, false);
    let mut i = from;
    while i < src.len() {
        match src[i] {
            b'\\' if quoted => i += 1,
            b'"' if tokens => quoted = !quoted,
            b if quoted || (b != open && b != close) => {}
            b if b == open => depth += 1,
            _ => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

static D_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/"), ("/+", "+/")],
    nested_comments: true,
    strings: &[Str::quoted("\"").multiline(), Str::quoted("`").raw().multiline(), Str::quoted("'")],
    string_prefixes: &[Prefix::raw(b'r'), Prefix::plain(b'x')],
    keywords: &[
        "abstract", "alias", "align", "asm", "assert", "auto", "break", "case", "cast", "catch", "const", "continue",
        "debug", "default", "delegate", "delete", "deprecated", "do", "else", "export", "extern", "final", "finally",
        "for", "foreach", "foreach_reverse", "function", "goto", "if", "immutable", "import", "in", "inout",
        "invariant", "is", "lazy", "mixin", "module", "new", "nothrow", "out", "override", "package", "pragma",
        "private", "protected", "public", "pure", "ref", "return", "scope", "shared", "static", "super", "switch",
        "synchronized", "throw", "try", "typeid", "typeof", "unittest", "version", "while", "with", "__gshared",
    ],
    types: &[
        "bool", "byte", "ubyte", "short", "ushort", "int", "uint", "long", "ulong", "char", "wchar", "dchar", "float",
        "double", "real", "void", "string", "wstring", "dstring", "size_t", "ptrdiff_t",
    ],
    builtins: &["writeln", "writefln", "writef", "write"],
    variables: &["this"],
    constants: &["true", "false", "null", "__FILE__", "__LINE__"],
    type_keywords: &["class", "struct", "interface", "enum", "union", "template"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS | BANG_MACROS,
    hook: Some(d_hook),
    hook_chars: "qe",
    ..Spec::DEFAULT
};

pub static D: Definition =
    Definition::new("d", "D", Engine::Generic(&D_SPEC)).aliases(&["dlang"]).extensions(&["d", "di"]);

// Groovy

static GROOVY_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[
        Str::quoted("\"\"\"").multiline().interp(Interp::Dollar),
        Str::quoted("'''").multiline(),
        Str::quoted("\"").interp(Interp::Dollar),
        Str::quoted("'"),
    ],
    keywords: &[
        "abstract", "as", "assert", "break", "case", "catch", "continue", "def", "default", "do", "else", "extends",
        "final", "finally", "for", "if", "implements", "import", "in", "instanceof", "native", "new", "package",
        "private", "protected", "public", "return", "static", "super", "switch", "synchronized", "throw", "throws",
        "transient", "try", "var", "volatile", "while",
    ],
    types: &["boolean", "byte", "char", "double", "float", "int", "long", "short", "void"],
    builtins: &["println", "print", "printf"],
    variables: &["this", "it", "delegate", "owner"],
    constants: &["true", "false", "null"],
    type_keywords: &["class", "interface", "enum", "trait", "record"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS | REGEX,
    hook: Some(|lx| shebang(lx.ctx)),
    hook_chars: "#",
    ..Spec::DEFAULT
};

pub static GROOVY: Definition = Definition::new("groovy", "Groovy", Engine::Generic(&GROOVY_SPEC))
    .aliases(&["gradle"])
    .extensions(&["groovy", "gvy", "gy", "gsh", "gradle"])
    .filenames(&["Jenkinsfile"]);

// Solidity

/// All caps interface names such as `IERC20` are types when they declare or convert.
fn caps_type(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    let end = lx.scan_ident(start);
    if (start > 0 && ctx.src[start - 1] == b'.') || ctx.src[start..end].iter().any(u8::is_ascii_lowercase) {
        return false;
    }
    let next = ctx.next_non_space(end);
    if !(is_ident_start(next) || next == b'(') {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Type);
    lx.mark_value();
    true
}

static SOLIDITY_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "abstract", "anonymous", "assembly", "break", "calldata", "catch", "constant", "constructor", "continue",
        "delete", "do", "else", "emit", "error", "external", "fallback", "for", "if", "immutable", "import", "indexed",
        "internal", "is", "memory", "modifier", "new", "override", "payable", "pragma", "private", "public", "pure",
        "receive", "return", "returns", "revert", "storage", "try", "unchecked", "using", "view", "virtual", "while",
        "mapping",
    ],
    types: &[
        "address", "bool", "string", "bytes", "byte", "int", "int8", "int16", "int32", "int64", "int128", "int256",
        "uint", "uint8", "uint16", "uint32", "uint64", "uint128", "uint256", "bytes4", "bytes32", "fixed", "ufixed",
    ],
    builtins: &["msg", "block", "tx", "abi", "require", "assert", "keccak256", "sha256", "ecrecover", "type"],
    variables: &["this", "super"],
    constants: &["true", "false", "wei", "gwei", "ether", "seconds", "minutes", "hours", "days", "weeks"],
    function_keywords: &["function"],
    type_keywords: &["contract", "interface", "library", "struct", "enum", "event"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS,
    hook: Some(caps_type),
    hook_chars: "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static SOLIDITY: Definition =
    Definition::new("solidity", "Solidity", Engine::Generic(&SOLIDITY_SPEC)).aliases(&["sol"]).extensions(&["sol"]);

// Shading languages

const SHADER_BUILTINS: &[&str] = &[
    "abs", "acos", "all", "any", "asin", "atan", "ceil", "clamp", "cos", "cross", "degrees", "distance", "dot", "exp",
    "exp2", "floor", "fract", "inverse", "length", "log", "log2", "max", "min", "mix", "normalize", "pow", "radians",
    "reflect", "refract", "round", "sign", "sin", "smoothstep", "sqrt", "step", "tan", "transpose", "texture",
    "textureLod", "texelFetch", "dFdx", "dFdy", "fwidth", "mod", "lerp", "saturate", "mul", "frac", "rsqrt", "ddx",
    "ddy", "select", "textureSample", "arrayLength",
];

static GLSL_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    keywords: &[
        "attribute", "break", "buffer", "case", "centroid", "coherent", "const", "continue", "default", "discard",
        "do", "else", "flat", "for", "highp", "if", "in", "inout", "invariant", "layout", "lowp", "mediump",
        "noperspective", "out", "patch", "precision", "readonly", "restrict", "return", "sample", "shared", "smooth",
        "subroutine", "switch", "uniform", "varying", "volatile", "while", "writeonly",
    ],
    types: &[
        "void", "bool", "int", "uint", "float", "double", "vec2", "vec3", "vec4", "ivec2", "ivec3", "ivec4", "uvec2",
        "uvec3", "uvec4", "bvec2", "bvec3", "bvec4", "dvec2", "dvec3", "dvec4", "mat2", "mat3", "mat4", "mat2x3",
        "mat3x4", "mat4x3", "sampler2D", "sampler3D", "samplerCube", "sampler2DArray", "sampler2DShadow", "image2D",
    ],
    builtins: SHADER_BUILTINS,
    constants: &["true", "false"],
    type_keywords: &["struct"],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    ..Spec::DEFAULT
};

pub static GLSL: Definition = Definition::new("glsl", "GLSL", Engine::Generic(&GLSL_SPEC))
    .extensions(&["glsl", "vert", "frag", "geom", "comp", "tesc", "tese"]);

/// Attributes, semantics after `:` and PascalCase declarations (`SamplerState LinearClamp`).
fn hlsl_hook(lx: &mut Lexer) -> bool {
    if lx.ctx.peek() == b'[' {
        return bracket_attribute(lx);
    }
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    if start > 0 && ctx.src[start - 1] == b'.' {
        return false;
    }
    let mut i = start;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    let end = lx.scan_ident(start);
    let kind = if ctx.at(i.wrapping_sub(1)) == b':' && ctx.at(i.wrapping_sub(2)) != b':' {
        Kind::Meta
    } else if is_ident_start(ctx.next_non_space(end)) {
        Kind::Type
    } else {
        return false;
    };
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, kind);
    lx.mark_value();
    true
}

static HLSL_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    keywords: &[
        "break", "case", "centroid", "const", "continue", "default", "discard", "do", "else", "export", "extern",
        "for", "groupshared", "if", "in", "inline", "inout", "linear", "namespace", "nointerpolation", "out",
        "packoffset", "precise", "register", "return", "row_major", "column_major", "shared", "static", "switch",
        "tbuffer", "typedef", "uniform", "volatile", "while",
    ],
    types: &[
        "void", "bool", "int", "uint", "dword", "half", "float", "double", "min16float", "bool2", "int2", "int3",
        "int4", "uint2", "uint3", "uint4", "half2", "half3", "half4", "float2", "float3", "float4", "float2x2",
        "float3x3", "float4x4", "float3x4", "matrix", "vector", "sampler", "Texture1D", "Texture2D", "Texture3D",
        "TextureCube", "Texture2DArray", "RWTexture2D", "RWTexture3D", "Buffer", "RWBuffer", "StructuredBuffer",
        "RWStructuredBuffer", "ByteAddressBuffer", "RWByteAddressBuffer", "SamplerState", "SamplerComparisonState",
        "ConstantBuffer",
    ],
    builtins: SHADER_BUILTINS,
    constants: &["true", "false"],
    type_keywords: &["struct", "class", "interface", "cbuffer"],
    features: CALLS | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    hook: Some(hlsl_hook),
    hook_chars: "[ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    ..Spec::DEFAULT
};

pub static HLSL: Definition =
    Definition::new("hlsl", "HLSL", Engine::Generic(&HLSL_SPEC)).extensions(&["hlsl", "hlsli", "fx", "fxh"]);

static WGSL_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    keywords: &[
        "alias", "break", "case", "const", "const_assert", "continue", "continuing", "default", "diagnostic",
        "discard", "else", "enable", "for", "if", "let", "loop", "override", "requires", "return", "switch", "var",
        "while", "function", "private", "workgroup", "uniform", "storage", "read", "write", "read_write",
    ],
    types: &[
        "bool", "f16", "f32", "i32", "u32", "vec2", "vec3", "vec4", "vec2f", "vec3f", "vec4f", "vec2i", "vec3i",
        "vec4i", "vec2u", "vec3u", "vec4u", "mat2x2", "mat3x3", "mat4x4", "mat4x4f", "mat3x3f", "array", "atomic",
        "ptr", "sampler", "sampler_comparison", "texture_2d", "texture_3d", "texture_cube", "texture_storage_2d",
        "texture_depth_2d",
    ],
    builtins: SHADER_BUILTINS,
    constants: &["true", "false"],
    function_keywords: &["fn"],
    type_keywords: &["struct"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static WGSL: Definition = Definition::new("wgsl", "WGSL", Engine::Generic(&WGSL_SPEC)).extensions(&["wgsl"]);

// V

/// Attributes `@[inline]`.
fn v_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    if ctx.peek_at(1) != b'[' || !ctx.can_nest() {
        return false;
    }
    ctx.token(2, Kind::Meta);
    let start = ctx.pos;
    let end = lx.scan_ident(start);
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Meta);
    lx.nested(b']');
    if lx.ctx.peek() == b']' {
        lx.ctx.token(1, Kind::Meta);
    }
    true
}

static V_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[
        Str::quoted("'").interp(Interp::DollarBrace),
        Str::quoted("\"").interp(Interp::DollarBrace),
        Str::quoted("`"),
    ],
    string_prefixes: &[Prefix::raw(b'r'), Prefix::plain(b'c')],
    keywords: &[
        "as", "asm", "assert", "atomic", "break", "const", "continue", "defer", "else", "enum", "for", "go", "goto",
        "if", "import", "in", "interface", "is", "isreftype", "lock", "match", "module", "mut", "or", "pub", "return",
        "rlock", "select", "shared", "sizeof", "spawn", "static", "struct", "type", "typeof", "union", "unsafe",
        "volatile", "__global",
    ],
    types: &[
        "bool", "string", "rune", "byte", "int", "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64",
        "usize", "f32", "f64", "voidptr", "any", "map", "chan", "thread",
    ],
    builtins: &["println", "print", "eprintln", "panic", "exit", "dump", "error"],
    constants: &["true", "false", "none", "nil"],
    function_keywords: &["fn"],
    sigils: &[(b'$', Kind::Meta)],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS,
    hook: Some(v_hook),
    hook_chars: "@",
    ..Spec::DEFAULT
};

pub static V: Definition = Definition::new("v", "V", Engine::Generic(&V_SPEC)).aliases(&["vlang"]).extensions(&["vsh"]);

// Odin

/// `name :: proc` declarations and `@(attributes)`.
fn odin_hook(lx: &mut Lexer) -> bool {
    let ctx = &*lx.ctx;
    let start = ctx.pos;
    if ctx.peek() == b'@' {
        if ctx.peek_at(1) != b'(' || !ctx.can_nest() {
            return false;
        }
        lx.ctx.token(2, Kind::Meta);
        lx.nested(b')');
        if lx.ctx.peek() == b')' {
            lx.ctx.token(1, Kind::Meta);
        }
        return true;
    }
    // At `::`: re-kind the name before it.
    if ctx.peek_at(1) != b':' {
        return false;
    }
    let mut end = start;
    while end > 0 && matches!(ctx.src[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    let mut name = end;
    while name > 0 && is_ident(ctx.src[name - 1]) {
        name -= 1;
    }
    let j = ctx.skip_spaces_from(start + 2);
    let kind = match &ctx.src[j..lx.scan_ident(j)] {
        _ if name == end => return false,
        b"proc" => Kind::Function,
        b"struct" | b"enum" | b"union" | b"distinct" | b"bit_set" => Kind::Type,
        _ => return false,
    };
    lx.ctx.truncate_to(name);
    lx.ctx.emit(name, end, kind);
    false
}

static ODIN_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[Str::quoted("\""), Str::quoted("'"), Str::quoted("`").raw().multiline()],
    keywords: &[
        "auto_cast", "bit_set", "break", "case", "cast", "context", "continue", "defer", "distinct", "do", "dynamic",
        "else", "enum", "fallthrough", "for", "foreign", "if", "import", "in", "map", "matrix", "not_in", "or_break",
        "or_continue", "or_else", "or_return", "package", "proc", "return", "struct", "switch", "transmute", "union",
        "using", "when", "where",
    ],
    types: &[
        "bool", "b8", "b32", "int", "uint", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "uintptr", "f16",
        "f32", "f64", "rune", "string", "cstring", "rawptr", "typeid", "any", "byte",
    ],
    builtins: &["len", "cap", "make", "delete", "append", "new", "free", "size_of", "align_of", "min", "max", "panic"],
    constants: &["true", "false", "nil"],
    sigils: &[(b'#', Kind::Meta)],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS,
    hook: Some(odin_hook),
    hook_chars: "@:",
    ..Spec::DEFAULT
};

pub static ODIN: Definition = Definition::new("odin", "Odin", Engine::Generic(&ODIN_SPEC)).extensions(&["odin"]);

// Haxe

/// `@:metadata` and `~/regex/` literals.
fn haxe_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'@' if ctx.peek_at(1) == b':' => {
            let end = lx.scan_ident(start + 2);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Meta);
            true
        }
        b'~' if ctx.peek_at(1) == b'/' => {
            let mut i = start + 2;
            while !matches!(ctx.at(i), b'/' | b'\n' | 0) {
                i += if ctx.at(i) == b'\\' { 2 } else { 1 };
            }
            i += 1;
            while ctx.at(i).is_ascii_alphabetic() {
                i += 1;
            }
            ctx.pos = i.min(ctx.len());
            ctx.emit_from(start, Kind::Regex);
            lx.mark_value();
            true
        }
        _ => false,
    }
}

static HAXE_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").multiline(), Str::quoted("'").multiline().interp(Interp::Dollar)],
    keywords: &[
        "abstract", "break", "case", "cast", "catch", "continue", "default", "do", "dynamic", "else", "extends",
        "extern", "final", "for", "if", "implements", "import", "in", "inline", "macro", "new", "operator", "overload",
        "override", "package", "private", "public", "return", "static", "switch", "throw", "try", "untyped", "using",
        "var", "while",
    ],
    builtins: &["trace"],
    variables: &["this", "super"],
    constants: &["true", "false", "null"],
    function_keywords: &["function"],
    type_keywords: &["class", "interface", "enum", "typedef"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS | PREPROCESSOR,
    hook: Some(haxe_hook),
    hook_chars: "@~",
    ..Spec::DEFAULT
};

pub static HAXE: Definition =
    Definition::new("haxe", "Haxe", Engine::Generic(&HAXE_SPEC)).aliases(&["hx"]).extensions(&["hx"]);

// Vala

static VALA_TEMPLATE: Str = Str::quoted("\"").interp(Interp::DollarParen);

/// Template strings `@"$name"` and `[Attributes]`.
fn vala_hook(lx: &mut Lexer) -> bool {
    if lx.ctx.peek() == b'[' {
        return bracket_attribute(lx);
    }
    if lx.ctx.peek_at(1) != b'"' {
        return false;
    }
    let start = lx.ctx.pos;
    lx.ctx.pos += 1;
    lx.string(start, &VALA_TEMPLATE, false, Interp::DollarParen);
    true
}

static VALA_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"\"\"").raw().multiline(), Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "abstract", "as", "async", "base", "break", "case", "catch", "const", "construct", "continue", "default",
        "delegate", "delete", "do", "dynamic", "else", "ensures", "errordomain", "extern", "finally", "for", "foreach",
        "get", "if", "in", "inline", "internal", "is", "lock", "namespace", "new", "out", "override", "owned",
        "private", "protected", "public", "ref", "requires", "return", "set", "signal", "sizeof", "static", "switch",
        "throw", "throws", "try", "typeof", "unowned", "using", "var", "virtual", "weak", "while", "yield",
    ],
    types: &[
        "bool", "char", "uchar", "int", "uint", "long", "ulong", "short", "ushort", "int8", "int16", "int32", "int64",
        "uint8", "uint16", "uint32", "uint64", "float", "double", "size_t", "string", "unichar", "void",
    ],
    builtins: &["print", "stdout", "stderr"],
    variables: &["this"],
    constants: &["true", "false", "null"],
    type_keywords: &["class", "struct", "interface", "enum"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    hook: Some(vala_hook),
    hook_chars: "@[",
    ..Spec::DEFAULT
};

pub static VALA: Definition =
    Definition::new("vala", "Vala", Engine::Generic(&VALA_SPEC)).extensions(&["vala", "vapi"]);

// Apex

static APEX_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("'")],
    keywords: &[
        "abstract", "break", "catch", "continue", "delete", "do", "else", "extends", "final", "finally", "for",
        "global", "if", "implements", "insert", "instanceof", "merge", "new", "override", "private", "protected",
        "public", "return", "static", "super", "switch", "testmethod", "throw", "transient", "trigger", "try",
        "undelete", "update", "upsert", "virtual", "webservice", "when", "while", "with", "without", "sharing",
        "inherited", "on", "before", "after", "select", "from", "where", "and", "or", "not", "in", "like", "limit",
        "order", "by", "asc", "desc", "group", "having", "offset",
    ],
    types: &[
        "blob", "boolean", "date", "datetime", "decimal", "double", "id", "integer", "list", "long", "map", "object",
        "set", "string", "time", "void", "sobject",
    ],
    builtins: &["system", "database", "schema", "userinfo", "test"],
    variables: &["this"],
    constants: &["true", "false", "null"],
    type_keywords: &["class", "interface", "enum"],
    sigils: &[(b':', Kind::Variable)],
    case_insensitive: true,
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static APEX: Definition =
    Definition::new("apex", "Apex", Engine::Generic(&APEX_SPEC)).extensions(&["apex", "trigger"]);

// ActionScript

static ACTIONSCRIPT_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "as", "break", "case", "catch", "const", "continue", "default", "delete", "do", "dynamic", "each", "else",
        "extends", "final", "finally", "for", "get", "if", "implements", "import", "in", "include", "instanceof",
        "internal", "is", "namespace", "native", "new", "override", "package", "private", "protected", "public",
        "return", "set", "static", "switch", "throw", "try", "typeof", "use", "var", "while", "with",
    ],
    types: &["int", "uint", "void", "Number", "String", "Boolean", "Object", "Array"],
    builtins: &["trace"],
    variables: &["this", "super"],
    constants: &["true", "false", "null", "undefined", "NaN", "Infinity"],
    function_keywords: &["function"],
    type_keywords: &["class", "interface"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | REGEX,
    hook: Some(bracket_attribute),
    hook_chars: "[",
    ..Spec::DEFAULT
};

pub static ACTIONSCRIPT: Definition =
    Definition::new("actionscript", "ActionScript", Engine::Generic(&ACTIONSCRIPT_SPEC))
        .aliases(&["as3", "actionscript3"])
        .extensions(&["as"]);

// Hare

static HARE_SPEC: Spec = Spec {
    line_comments: &["//"],
    strings: &[Str::quoted("\""), Str::quoted("'"), Str::quoted("`").raw().multiline()],
    keywords: &[
        "abort", "align", "alloc", "append", "as", "assert", "break", "case", "const", "continue", "def", "defer",
        "delete", "else", "enum", "export", "for", "free", "if", "insert", "is", "len", "let", "match", "offset",
        "return", "size", "static", "struct", "switch", "union", "use", "yield", "nullable", "vastart", "vaarg",
        "vaend",
    ],
    types: &[
        "bool", "done", "f32", "f64", "i8", "i16", "i32", "i64", "int", "never", "opaque", "rune", "size", "str", "u8",
        "u16", "u32", "u64", "uint", "uintptr", "valist", "void",
    ],
    constants: &["true", "false", "null"],
    function_keywords: &["fn"],
    type_keywords: &["type"],
    features: CALLS | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static HARE: Definition = Definition::new("hare", "Hare", Engine::Generic(&HARE_SPEC)).extensions(&["ha"]);

// Carbon

static CARBON_SPEC: Spec = Spec {
    line_comments: &["//"],
    strings: &[Str::quoted("'''").multiline(), Str::quoted("\"")],
    keywords: &[
        "abstract", "adapt", "addr", "alias", "and", "api", "as", "auto", "base", "break", "case", "const", "continue",
        "default", "else", "export", "extend", "final", "for", "forall", "friend", "if", "impl", "import", "in", "let",
        "library", "like", "match", "namespace", "not", "observe", "or", "override", "package", "partial", "private",
        "protected", "return", "returned", "then", "template", "type", "var", "virtual", "where", "while",
    ],
    types: &["bool", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f16", "f32", "f64", "Self"],
    variables: &["self"],
    constants: &["true", "false"],
    function_keywords: &["fn"],
    type_keywords: &["class", "interface", "choice", "constraint"],
    features: CALLS | CAPITALIZED_TYPES | MEMBERS,
    ..Spec::DEFAULT
};

pub static CARBON: Definition =
    Definition::new("carbon", "Carbon", Engine::Generic(&CARBON_SPEC)).extensions(&["carbon"]);
