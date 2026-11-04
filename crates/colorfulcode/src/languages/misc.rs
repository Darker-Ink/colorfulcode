//! Plain text and languages that do not fit elsewhere: assembly and IRs,
//! hardware description, legacy business, scientific and niche languages.

use super::{Definition, Engine};
use crate::lexer::generic::{features::*, scan_number, Interp, Lexer, Prefix, Spec, Str};
use crate::lexer::{is_ident, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[
    &PLAIN, &ASM, &GAS, &ARM, &RISCV, &LLVM, &WAT, &FORTRAN, &PASCAL, &ADA, &VHDL, &VERILOG, &SYSTEMVERILOG, &COBOL,
    &VBNET, &VBA, &BASIC, &FORTH, &MATLAB, &WOLFRAM, &SAS, &STATA, &ABAP, &APL, &SMALLTALK, &TLA, &ALLOY, &OPENSCAD,
    &GCODE, &QML, &KUSTO, &DAX, &MQL,
];

pub static PLAIN: Definition = Definition::new("text", "Plain Text", Engine::Custom(|_: &mut Ctx| {}))
    .aliases(&["plain", "plaintext", "txt", "none"])
    .extensions(&["txt", "text"]);

macro_rules! letters {
    () => {
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
    };
}

// Shared helpers

/// Case insensitive membership test for short word lists.
fn any_eq(word: &[u8], list: &[&str]) -> bool {
    list.iter().any(|w| word.eq_ignore_ascii_case(w.as_bytes()))
}

/// End of the run of bytes matching `f` starting at `from`.
fn scan(ctx: &Ctx, from: usize, f: impl Fn(u8) -> bool) -> usize {
    let mut end = from;
    while end < ctx.len() && f(ctx.src[end]) {
        end += 1;
    }
    end
}

/// Emits `[start, end)` as `kind` and moves the cursor to `end`.
fn put(ctx: &mut Ctx, start: usize, end: usize, kind: Kind) {
    let end = end.min(ctx.len());
    ctx.pos = end.max(ctx.pos);
    ctx.emit(start, end, kind);
}

/// The word (letters only) that precedes `pos`, skipping spaces.
fn word_before<'s>(ctx: &Ctx<'s, '_>, pos: usize) -> &'s [u8] {
    let mut end = pos;
    while end > 0 && matches!(ctx.src[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && ctx.src[start - 1].is_ascii_alphabetic() {
        start -= 1;
    }
    let src: &'s [u8] = ctx.src;
    &src[start..end]
}

/// The byte before `pos` on its line, skipping spaces and tabs (`0` at the start of input).
fn byte_before(ctx: &Ctx, pos: usize) -> u8 {
    let mut i = pos;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    if i == 0 {
        0
    } else {
        ctx.src[i - 1]
    }
}

/// Whether only spaces remain on the line after `pos`.
fn rest_blank(ctx: &Ctx, pos: usize) -> bool {
    matches!(ctx.at(ctx.skip_spaces_from(pos)), b'\n' | b'\r' | 0)
}

/// A single line string from `start` (with an opening delimiter `open_len` bytes
/// long) to `close`, in which `refs` finds macro references (it returns their end).
/// A doubled `close` is an escaped delimiter.
fn string_with_refs(ctx: &mut Ctx, start: usize, open_len: usize, close: &[u8], refs: fn(&Ctx, usize) -> usize) {
    let mut i = (start + open_len).min(ctx.len());
    let mut seg = start;
    while i < ctx.len() && ctx.src[i] != b'\n' {
        if ctx.src[i..].starts_with(close) {
            if close.len() == 1 && ctx.at(i + 1) == close[0] {
                i += 2;
                continue;
            }
            i += close.len();
            ctx.emit(seg, i, Kind::String);
            ctx.pos = i;
            return;
        }
        let end = refs(ctx, i);
        if end > i {
            ctx.emit(seg, i, Kind::String);
            ctx.emit(i, end, Kind::Variable);
            seg = end;
            i = end;
        } else {
            i += 1;
        }
    }
    ctx.emit(seg, i, Kind::String);
    ctx.pos = i;
}

/// A double quoted string with `\XX` hex, `\u{...}` and single character escapes.
fn hex_string(ctx: &mut Ctx, start: usize) {
    ctx.pos += 1;
    let mut seg = start;
    while !ctx.done() {
        match ctx.peek() {
            b'"' => {
                ctx.pos += 1;
                break;
            }
            b'\n' => break,
            b'\\' => {
                ctx.emit_from(seg, Kind::String);
                let esc = ctx.pos;
                let len = if ctx.peek_at(1).is_ascii_hexdigit() && ctx.peek_at(2).is_ascii_hexdigit() {
                    3
                } else if ctx.peek_at(1) == b'u' && ctx.peek_at(2) == b'{' {
                    ctx.src[esc..].iter().take(12).position(|&b| b == b'}').map_or(2, |i| i + 1)
                } else {
                    2
                };
                ctx.pos = (esc + len).min(ctx.len());
                if ctx.pos < ctx.len() {
                    // Never split a multi byte character.
                    while ctx.pos < ctx.len() && (ctx.src[ctx.pos] & 0xC0) == 0x80 {
                        ctx.pos += 1;
                    }
                }
                ctx.emit_from(esc, Kind::Escape);
                seg = ctx.pos;
            }
            _ => ctx.pos += 1,
        }
    }
    ctx.emit_from(seg, Kind::String);
}

// Assembly

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arch {
    Nasm,
    Gas,
    Arm,
    RiscV,
}

const CPP_DIRECTIVES: &[&str] =
    &["include", "define", "undef", "if", "ifdef", "ifndef", "elif", "else", "endif", "error", "pragma", "line"];

const ASM_DATA: &[&str] = &[
    "db", "dw", "dd", "dq", "dt", "do", "dy", "dz", "resb", "resw", "resd", "resq", "rest", "reso", "resy", "resz",
    "equ",
];

const X86_PREFIXES: &[&str] = &["rep", "repe", "repz", "repne", "repnz", "lock"];

/// `prefix` followed by a decimal number no larger than `max`.
fn indexed(word: &[u8], prefix: &str, max: u32) -> bool {
    let p = prefix.len();
    word.len() > p
        && word.len() <= p + 2
        && word[..p].eq_ignore_ascii_case(prefix.as_bytes())
        && word[p..].iter().all(u8::is_ascii_digit)
        && word[p..].iter().fold(0, |n, d| n * 10 + u32::from(d - b'0')) <= max
}

/// Named x86 registers, looked up through the spec's `variables` list.
const X86_REGISTERS: &[&str] = &[
    "al", "ah", "ax", "eax", "rax", "bl", "bh", "bx", "ebx", "rbx", "cl", "ch", "cx", "ecx", "rcx", "dl", "dh", "dx",
    "edx", "rdx", "si", "esi", "rsi", "sil", "di", "edi", "rdi", "dil", "bp", "ebp", "rbp", "bpl", "sp", "esp", "rsp",
    "spl", "ip", "eip", "rip", "cs", "ds", "es", "fs", "gs", "ss", "eflags", "rflags",
];

const ARM_REGISTERS: &[&str] =
    &["sp", "lr", "pc", "fp", "ip", "xzr", "wzr", "wsp", "apsr", "cpsr", "spsr", "fpscr", "fpsr", "fpcr", "nzcv"];

const RISCV_REGISTERS: &[&str] = &["zero", "ra", "sp", "gp", "tp", "fp", "pc"];

/// Numbered registers such as `r8d`, `xmm3`, `x29`, `v0` or `a7`.
fn indexed_register(w: &[u8], arch: Arch) -> bool {
    if w.len() > 6 || (arch != Arch::Nasm && !w.last().is_some_and(u8::is_ascii_digit)) {
        return false;
    }
    match arch {
        Arch::Nasm => {
            let n = w.len();
            let sized = n > 2 && matches!(w[n - 1] | 0x20, b'd' | b'w' | b'b') && w[n - 2].is_ascii_digit();
            let base = if sized { &w[..n - 1] } else { w };
            indexed(base, "r", 15)
                || ["xmm", "ymm", "zmm"].iter().any(|p| indexed(w, p, 31))
                || ["cr", "dr", "mm", "st", "k"].iter().any(|p| indexed(w, p, 15))
        }
        Arch::Gas => false,
        Arch::Arm => {
            indexed(w, "r", 15)
                || indexed(w, "x", 30)
                || indexed(w, "w", 30)
                || ["v", "q", "d", "s", "h", "b", "z"].iter().any(|p| indexed(w, p, 31))
        }
        Arch::RiscV => {
            [("x", 31), ("f", 31), ("v", 31), ("t", 6), ("s", 11), ("a", 7), ("ft", 11), ("fs", 11), ("fa", 7)]
                .iter()
                .any(|&(p, max)| indexed(w, p, max))
        }
    }
}

/// Whether the word at `start` is where an instruction or directive goes:
/// the start of a line, after a label, after `;` (GNU) or after a prefix (x86).
fn asm_statement_start(ctx: &Ctx, start: usize, arch: Arch) -> bool {
    let mut i = start;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    if i == 0 {
        return true;
    }
    match ctx.src[i - 1] {
        b'\n' => true,
        b':' => {
            let mut j = i - 1;
            while j > 0 && (is_ident(ctx.src[j - 1]) || ctx.src[j - 1] == b'.') {
                j -= 1;
            }
            j < i - 1 && ctx.at_line_start(j)
        }
        b';' => arch != Arch::Nasm,
        _ if arch == Arch::Nasm => {
            let mut j = i;
            while j > 0 && ctx.src[j - 1].is_ascii_alphabetic() {
                j -= 1;
            }
            any_eq(&ctx.src[j..i], X86_PREFIXES) && ctx.at_line_start(j)
        }
        _ => false,
    }
}

/// Labels, mnemonics, directives, registers, immediates and macros.
fn asm_hook(lx: &mut Lexer, arch: Arch) -> bool {
    let start = lx.ctx.pos;
    let b = lx.ctx.peek();
    let next = lx.ctx.peek_at(1);
    match b {
        b'#' => {
            let ctx = &mut *lx.ctx;
            if ctx.at_line_start(start) && next.is_ascii_alphabetic() {
                let end = scan(ctx, start + 1, |c| c.is_ascii_alphanumeric());
                if any_eq(&ctx.src[start + 1..end], CPP_DIRECTIVES) {
                    put(ctx, start, end, Kind::Meta);
                    let path = ctx.skip_spaces_from(end);
                    if ctx.at(path) == b'<' {
                        let line_end = ctx.line_end(path);
                        let close =
                            ctx.src[path..line_end].iter().position(|&c| c == b'>').map_or(line_end, |i| path + i + 1);
                        put(ctx, path, close, Kind::String);
                    }
                    return true;
                }
            }
            if arch == Arch::Arm {
                let i = if next == b'-' { start + 2 } else { start + 1 };
                if ctx.at(i).is_ascii_digit() {
                    ctx.pos = i;
                    scan_number(ctx, false);
                    ctx.emit_from(start, Kind::Number);
                    return true;
                }
            }
            false
        }
        b'%' => {
            let ctx = &mut *lx.ctx;
            if arch == Arch::Nasm {
                if next == b'%' {
                    let end = scan(ctx, start + 2, |c| is_ident(c) || c == b'.');
                    let kind = if ctx.at(end) == b':' { Kind::Function } else { Kind::Variable };
                    put(ctx, start, end, kind);
                    return true;
                }
                if next.is_ascii_digit() {
                    let end = scan(ctx, start + 1, |c| c.is_ascii_digit());
                    put(ctx, start, end, Kind::Variable);
                    return true;
                }
                if next.is_ascii_alphabetic() && ctx.at_line_start(start) {
                    let end = scan(ctx, start + 1, is_ident);
                    put(ctx, start, end, Kind::Meta);
                    return true;
                }
                return false;
            }
            if next.is_ascii_alphabetic() || next == b'_' {
                let end = scan(ctx, start + 1, is_ident);
                put(ctx, start, end, if arch == Arch::Gas { Kind::Variable } else { Kind::Builtin });
                return true;
            }
            false
        }
        b'$' => {
            let ctx = &mut *lx.ctx;
            match arch {
                Arch::Nasm => {
                    let len = if next == b'$' { 2 } else { 1 };
                    if is_ident(ctx.at(start + len)) {
                        return false;
                    }
                    ctx.token(len, Kind::Variable);
                    true
                }
                Arch::Gas => {
                    let i = if next == b'-' { start + 2 } else { start + 1 };
                    if ctx.at(i).is_ascii_digit() {
                        ctx.pos = i;
                        scan_number(ctx, false);
                        ctx.emit_from(start, Kind::Number);
                    } else if next.is_ascii_alphabetic() || matches!(next, b'_' | b'.') {
                        // `$symbol` is the address of a symbol as an immediate.
                        let end = scan(ctx, start + 1, |c| is_ident(c) || c == b'.');
                        put(ctx, start, end, Kind::Constant);
                    } else {
                        ctx.token(1, Kind::Operator);
                    }
                    true
                }
                _ => false,
            }
        }
        b'0'..=b'9' => {
            // GNU numeric local labels: `1:` defines one, `1b` and `1f` refer back and forward.
            let ctx = &mut *lx.ctx;
            if start > 0 && is_ident(ctx.src[start - 1]) {
                return false;
            }
            let end = scan(ctx, start, |c| c.is_ascii_digit());
            let kind = if ctx.at(end) == b':' && ctx.at(end + 1) != b':' && asm_statement_start(ctx, start, arch) {
                Kind::Function
            } else if matches!(ctx.at(end), b'b' | b'f') && !is_ident(ctx.at(end + 1)) {
                put(ctx, start, end + 1, Kind::Text);
                return true;
            } else {
                return false;
            };
            put(ctx, start, end, kind);
            true
        }
        _ => {
            if b == b'.' && !(next.is_ascii_alphabetic() || next == b'_') {
                return false;
            }
            let end = lx.scan_ident(start);
            let ctx = &mut *lx.ctx;
            let statement = asm_statement_start(ctx, start, arch);
            let kind = if statement && ctx.at(end) == b':' && ctx.at(end + 1) != b':' {
                Kind::Function
            } else if statement {
                let i = ctx.skip_spaces_from(end);
                let j = scan(ctx, i, |c| c.is_ascii_alphanumeric());
                if b == b'.' {
                    Kind::Meta
                } else if (ctx.at(i) == b'=' && ctx.at(i + 1) != b'=')
                    || (arch == Arch::Nasm && (2..=4).contains(&(j - i)) && any_eq(&ctx.src[i..j], ASM_DATA))
                {
                    Kind::Function
                } else {
                    Kind::Keyword
                }
            } else if b == b'.' {
                Kind::Text
            } else {
                // Named registers come from the `variables` list through the generic rules.
                let word = &ctx.src[start..end];
                let base = &word[..word.iter().position(|&c| c == b'.').unwrap_or(word.len())];
                if !indexed_register(base, arch) {
                    return false;
                }
                Kind::Variable
            };
            put(ctx, start, end, kind);
            true
        }
    }
}

fn nasm_hook(lx: &mut Lexer) -> bool {
    asm_hook(lx, Arch::Nasm)
}

fn gas_hook(lx: &mut Lexer) -> bool {
    asm_hook(lx, Arch::Gas)
}

fn arm_hook(lx: &mut Lexer) -> bool {
    asm_hook(lx, Arch::Arm)
}

fn riscv_hook(lx: &mut Lexer) -> bool {
    asm_hook(lx, Arch::RiscV)
}

const ASM_HOOK_CHARS: &str = concat!(letters!(), "_.%$#");
/// GNU syntax also has numeric local labels.
const GNU_HOOK_CHARS: &str = concat!(letters!(), "_.%$#0123456789");
const ASM_OPERATORS: &str = "+-*/%=&|^!<>~";

const ASM_BASE: Spec = Spec {
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    ident_chars: ".",
    operators: ASM_OPERATORS,
    case_insensitive: true,
    hook_chars: ASM_HOOK_CHARS,
    ..Spec::DEFAULT
};

static NASM_SPEC: Spec = Spec {
    line_comments: &[";"],
    block_comments: &[],
    strings: &[Str::quoted("\"").raw(), Str::quoted("'").raw(), Str::quoted("`")],
    keywords: &[
        "ptr", "rel", "abs", "near", "far", "short", "offset", "seg", "wrt", "strict", "nosplit", "db", "dw", "dd",
        "dq", "dt", "do", "dy", "dz", "resb", "resw", "resd", "resq", "rest", "reso", "resy", "resz", "equ", "times",
        "incbin",
    ],
    types: &["byte", "word", "dword", "qword", "tword", "oword", "yword", "zword"],
    variables: X86_REGISTERS,
    hook: Some(nasm_hook),
    ..ASM_BASE
};

static GAS_SPEC: Spec =
    Spec { line_comments: &["#", "//"], hook: Some(gas_hook), hook_chars: GNU_HOOK_CHARS, ..ASM_BASE };

static ARM_SPEC: Spec = Spec {
    line_comments: &["@", "//", ";"],
    keywords: &[
        "lsl", "lsr", "asr", "ror", "rrx", "uxtb", "uxth", "uxtw", "uxtx", "sxtb", "sxth", "sxtw", "sxtx", "eq", "ne",
        "cs", "hs", "cc", "lo", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "al",
    ],
    variables: ARM_REGISTERS,
    hook: Some(arm_hook),
    hook_chars: GNU_HOOK_CHARS,
    ..ASM_BASE
};

static RISCV_SPEC: Spec = Spec {
    line_comments: &["#"],
    variables: RISCV_REGISTERS,
    hook: Some(riscv_hook),
    hook_chars: GNU_HOOK_CHARS,
    ..ASM_BASE
};

pub static ASM: Definition = Definition::new("asm", "Assembly", Engine::Generic(&NASM_SPEC))
    .aliases(&["nasm", "x86asm", "assembly", "x86", "yasm", "masm"])
    .extensions(&["asm", "nasm"]);

pub static GAS: Definition = Definition::new("gas", "GNU Assembler", Engine::Generic(&GAS_SPEC))
    .aliases(&["gnu-asm", "gnuasm", "att"])
    .extensions(&["s", "S"]);

pub static ARM: Definition = Definition::new("arm", "ARM Assembly", Engine::Generic(&ARM_SPEC))
    .aliases(&["armasm", "arm64", "aarch64", "arm-asm"]);

pub static RISCV: Definition = Definition::new("riscv", "RISC-V Assembly", Engine::Generic(&RISCV_SPEC))
    .aliases(&["risc-v", "riscv64", "riscv32", "rv64"]);

// LLVM IR

/// `%local`, `@global`, `!metadata`, `#attributes`, labels, `iN` types and hex escapes.
fn llvm_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let b = lx.ctx.peek();
    let next = lx.ctx.peek_at(1);
    match b {
        b'%' | b'@' | b'!' | b'#' => {
            let ctx = &mut *lx.ctx;
            let end = if b == b'#' {
                scan(ctx, start + 1, |c| c.is_ascii_digit())
            } else if next == b'"' && b != b'!' {
                let close = ctx.src[start + 2..ctx.line_end(start)].iter().position(|&c| c == b'"');
                close.map_or(start + 1, |i| start + 2 + i + 1)
            } else {
                scan(ctx, start + 1, |c| is_ident(c) || matches!(c, b'.' | b'$' | b'-'))
            };
            if end == start + 1 {
                if b == b'!' && matches!(next, b'{' | b'"') {
                    ctx.token(1, Kind::Meta);
                    return true;
                }
                return false;
            }
            let kind = match b {
                b'%' => Kind::Variable,
                b'@' if ctx.next_non_space(end) == b'(' => Kind::Function,
                b'@' => Kind::Variable,
                _ => Kind::Meta,
            };
            put(ctx, start, end, kind);
            lx.mark_value();
            true
        }
        b'"' => {
            hex_string(lx.ctx, start);
            true
        }
        b'c' if next == b'"' => {
            lx.ctx.pos += 1;
            hex_string(lx.ctx, start);
            true
        }
        _ => {
            let end = lx.scan_ident(start);
            let ctx = &mut *lx.ctx;
            let word = &ctx.src[start..end];
            let kind = if ctx.at(end) == b':' && ctx.at_line_start(start) {
                Kind::Function
            } else if word.len() > 1 && word[0] == b'i' && word[1..].iter().all(u8::is_ascii_digit) {
                Kind::Type
            } else {
                return false;
            };
            put(ctx, start, end, kind);
            true
        }
    }
}

static LLVM_SPEC: Spec = Spec {
    line_comments: &[";"],
    keywords: &[
        "define", "declare", "global", "constant", "private", "internal", "external", "linkonce", "linkonce_odr",
        "weak", "weak_odr", "common", "appending", "extern_weak", "available_externally", "dso_local", "unnamed_addr",
        "local_unnamed_addr", "align", "section", "comdat", "personality", "nounwind", "readonly", "readnone",
        "writeonly", "noinline", "alwaysinline", "optnone", "uwtable", "nonnull", "noalias", "nocapture", "sret",
        "byval", "noundef", "returned", "target", "datalayout", "triple", "source_filename", "attributes", "type",
        "opaque", "to", "x", "volatile", "atomic", "inbounds", "nuw", "nsw", "exact", "fast", "tail", "musttail",
        "ret", "br", "switch", "indirectbr", "invoke", "resume", "unreachable", "fneg", "add", "fadd", "sub", "fsub",
        "mul", "fmul", "udiv", "sdiv", "fdiv", "urem", "srem", "frem", "shl", "lshr", "ashr", "and", "or", "xor",
        "extractelement", "insertelement", "shufflevector", "extractvalue", "insertvalue", "alloca", "load", "store",
        "fence", "cmpxchg", "atomicrmw", "getelementptr", "trunc", "zext", "sext", "fptrunc", "fpext", "fptoui",
        "fptosi", "uitofp", "sitofp", "ptrtoint", "inttoptr", "bitcast", "addrspacecast", "icmp", "fcmp", "phi",
        "select", "call", "va_arg", "landingpad", "freeze", "eq", "ne", "ugt", "uge", "ult", "ule", "sgt", "sge",
        "slt", "sle", "oeq", "ogt", "oge", "olt", "ole", "one", "ord", "ueq", "une", "uno",
    ],
    types: &[
        "void", "half", "bfloat", "float", "double", "fp128", "x86_fp80", "ppc_fp128", "label", "metadata", "ptr",
        "token",
    ],
    constants: &["true", "false", "null", "undef", "poison", "zeroinitializer", "none"],
    ident_chars: ".",
    features: IDENT_KEYS,
    hook: Some(llvm_hook),
    hook_chars: concat!(letters!(), "%@!#\""),
    ..Spec::DEFAULT
};

pub static LLVM: Definition =
    Definition::new("llvm", "LLVM IR", Engine::Generic(&LLVM_SPEC)).aliases(&["llvm-ir", "ll"]).extensions(&["ll"]);

// WebAssembly text

/// `$names` (functions after `func` and `call`), `i32.add` style instructions and `offset=`.
fn wat_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    match lx.ctx.peek() {
        b'$' => {
            let ctx = &mut *lx.ctx;
            let end = scan(ctx, start + 1, |c| is_ident(c) || b"._-/:<>!?'*+@#$%&^|~".contains(&c));
            if end == start + 1 {
                return false;
            }
            let before = word_before(ctx, start);
            let kind = if any_eq(before, &["func", "call", "return_call", "ref.func"]) {
                Kind::Function
            } else {
                Kind::Variable
            };
            put(ctx, start, end, kind);
            lx.mark_value();
            true
        }
        b'"' => {
            hex_string(lx.ctx, start);
            true
        }
        _ => {
            let end = lx.scan_ident(start);
            let ctx = &mut *lx.ctx;
            let kind = if ctx.at(end) == b'=' {
                Kind::Property
            } else if ctx.src[start..end].contains(&b'.') {
                Kind::Keyword
            } else {
                return false;
            };
            put(ctx, start, end, kind);
            true
        }
    }
}

static WAT_SPEC: Spec = Spec {
    line_comments: &[";;"],
    block_comments: &[("(;", ";)")],
    nested_comments: true,
    keywords: &[
        "module", "func", "param", "result", "local", "global", "memory", "table", "type", "import", "export", "start",
        "elem", "data", "offset", "mut", "block", "loop", "if", "then", "else", "end", "br", "br_if", "br_table",
        "return", "call", "call_indirect", "return_call", "drop", "select", "unreachable", "nop", "item", "declare",
        "rec", "sub", "field", "struct", "array", "try", "catch", "throw", "tag",
    ],
    types: &["i32", "i64", "f32", "f64", "v128", "funcref", "externref", "anyref", "i8", "i16"],
    constants: &["nan", "inf"],
    ident_chars: ".",
    hook: Some(wat_hook),
    hook_chars: concat!(letters!(), "$\""),
    ..Spec::DEFAULT
};

pub static WAT: Definition = Definition::new("wat", "WebAssembly Text", Engine::Generic(&WAT_SPEC))
    .aliases(&["wast", "webassembly", "wasm"])
    .extensions(&["wat", "wast"]);

// Fortran

/// `.and.` style operators, `.true.`, `%component` access and fixed form comments.
fn fortran_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'c' | b'C' | b'*' => {
            // Fixed form: `C`, `c` or `*` in column 1. Free form code such as
            // `c = 1`, `c(i) = 2` or `call` at column 1 is left alone.
            if start > 0 && ctx.src[start - 1] != b'\n' {
                return false;
            }
            let next = ctx.peek_at(1);
            if ctx.peek() != b'*' {
                let after = ctx.next_non_space(start + 1);
                if is_ident(next) || matches!(after, b'=' | b'(' | b'%') {
                    return false;
                }
            }
            let end = ctx.line_end(start);
            put(ctx, start, end, Kind::Comment);
            true
        }
        b'.' => {
            let end = scan(ctx, start + 1, |c| c.is_ascii_alphabetic());
            if end == start + 1 || ctx.at(end) != b'.' {
                return false;
            }
            let word = &ctx.src[start + 1..end];
            let kind = if any_eq(word, &["true", "false"]) {
                Kind::Constant
            } else if any_eq(word, &["and", "or", "not", "eqv", "neqv", "eq", "ne", "lt", "le", "gt", "ge"]) {
                Kind::Keyword
            } else {
                return false;
            };
            put(ctx, start, end + 1, kind);
            lx.mark_operator();
            true
        }
        _ => {
            if !(ctx.peek_at(1).is_ascii_alphabetic() || ctx.peek_at(1) == b'_') {
                return false;
            }
            ctx.token(1, Kind::Operator);
            let end = lx.scan_ident(start + 1);
            put(lx.ctx, start + 1, end, Kind::Property);
            lx.mark_value();
            true
        }
    }
}

static FORTRAN_SPEC: Spec = Spec {
    line_comments: &["!"],
    strings: &[Str::quoted("'").raw().doubled(), Str::quoted("\"").raw().doubled()],
    keywords: &[
        "allocatable", "allocate", "assign", "associate", "block", "call", "case", "class", "close", "common",
        "contains", "continue", "cycle", "data", "deallocate", "default", "dimension", "do", "else", "elseif",
        "elsewhere", "end", "enddo", "endif", "entry", "enum", "enumerator", "equivalence", "exit", "extends",
        "external", "final", "forall", "format", "go", "goto", "if", "implicit", "import", "in", "include", "inout",
        "intent", "interface", "intrinsic", "kind", "len", "module", "namelist", "none", "nullify", "only", "open",
        "operator", "optional", "out", "parameter", "pass", "pause", "pointer", "print", "private", "procedure",
        "program", "protected", "public", "pure", "read", "recursive", "elemental", "result", "return", "rewind",
        "save", "select", "sequence", "stop", "target", "then", "to", "use", "value", "volatile", "where", "while",
        "write", "concurrent", "submodule", "abstract", "deferred", "non_overridable", "nopass", "bind", "impure",
    ],
    types: &["integer", "real", "complex", "character", "logical", "double", "precision"],
    builtins: &[
        "abs", "sqrt", "sin", "cos", "tan", "exp", "log", "log10", "mod", "max", "min", "sum", "product", "size",
        "shape", "reshape", "allocated", "associated", "present", "trim", "adjustl", "adjustr", "len_trim", "index",
        "int", "nint", "dble", "huge", "tiny", "epsilon", "matmul", "transpose", "dot_product", "maxval", "minval",
        "random_number", "selected_real_kind", "selected_int_kind", "floor", "ceiling", "achar", "iachar",
    ],
    function_keywords: &["subroutine", "function", "call", "program"],
    type_keywords: &["type"],
    case_insensitive: true,
    features: PREPROCESSOR,
    hook: Some(fortran_hook),
    hook_chars: ".%cC*",
    ..Spec::DEFAULT
};

pub static FORTRAN: Definition = Definition::new("fortran", "Fortran", Engine::Generic(&FORTRAN_SPEC))
    .aliases(&["f90", "f95", "f03", "f08", "f77", "fortran90"])
    .extensions(&["f90", "f95", "f03", "f08", "f", "for", "f77", "fpp"]);

// Pascal and Delphi

/// Compiler directives, `#13` characters, `$FF` and `%101` numbers and `TName` types.
fn pascal_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'{' if next == b'$' => {
            let end = ctx.find(start, b"}").map_or(ctx.len(), |i| i + 1);
            put(ctx, start, end, Kind::Meta);
            true
        }
        b'(' if next == b'*' && ctx.peek_at(2) == b'$' => {
            let end = ctx.find(start, b"*)").map_or(ctx.len(), |i| i + 2);
            put(ctx, start, end, Kind::Meta);
            true
        }
        b'#' => {
            let end = if next == b'$' {
                scan(ctx, start + 2, |c| c.is_ascii_hexdigit())
            } else {
                scan(ctx, start + 1, |c| c.is_ascii_digit())
            };
            if end <= start + 1 || ctx.at(end - 1) == b'$' {
                return false;
            }
            put(ctx, start, end, Kind::String);
            lx.mark_value();
            true
        }
        b @ (b'$' | b'%' | b'&') => {
            let end = match b {
                b'$' => scan(ctx, start + 1, |c| c.is_ascii_hexdigit()),
                b'%' => scan(ctx, start + 1, |c| c == b'0' || c == b'1'),
                _ => scan(ctx, start + 1, |c| (b'0'..=b'7').contains(&c)),
            };
            if end == start + 1 || is_ident(ctx.at(end)) {
                return false;
            }
            put(ctx, start, end, Kind::Number);
            lx.mark_value();
            true
        }
        _ => {
            // Delphi naming convention: TStringList, EConvertError, IInterface.
            let end = lx.scan_ident(start);
            let word = &lx.ctx.src[start..end];
            if word.len() > 2 && word[1].is_ascii_uppercase() && word[2..].iter().any(u8::is_ascii_lowercase) {
                put(lx.ctx, start, end, Kind::Type);
                lx.mark_value();
                // `procedure TFoo.Bar`: the method name.
                let ctx = &mut *lx.ctx;
                if ctx.at(end) == b'.'
                    && any_eq(word_before(ctx, start), &["function", "procedure", "constructor", "destructor"])
                {
                    let name_end = scan(ctx, end + 1, is_ident);
                    ctx.token(1, Kind::Punctuation);
                    put(ctx, end + 1, name_end, Kind::Function);
                }
                return true;
            }
            false
        }
    }
}

static PASCAL_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("{", "}"), ("(*", "*)")],
    strings: &[Str::quoted("'").raw().doubled()],
    keywords: &[
        "and", "array", "as", "asm", "begin", "case", "class", "const", "div", "do", "downto", "else", "end", "except",
        "exports", "file", "finalization", "finally", "for", "goto", "if", "implementation", "in", "inherited",
        "initialization", "inline", "interface", "is", "label", "library", "mod", "not", "object", "of", "or", "out",
        "packed", "program", "property", "raise", "record", "repeat", "resourcestring", "set", "shl", "shr", "then",
        "threadvar", "to", "try", "type", "unit", "until", "uses", "var", "while", "with", "xor", "private",
        "protected", "public", "published", "strict", "override", "virtual", "abstract", "overload", "reintroduce",
        "static", "dynamic", "forward", "external", "read", "write", "default", "specialize", "generic", "on", "exit",
        "break", "continue", "operator", "helper", "absolute", "cdecl", "stdcall",
    ],
    types: &[
        "integer", "cardinal", "shortint", "smallint", "longint", "int64", "uint64", "byte", "word", "longword",
        "boolean", "char", "widechar", "ansichar", "real", "single", "double", "extended", "currency", "string",
        "ansistring", "widestring", "unicodestring", "shortstring", "pointer", "variant", "pchar", "text", "nativeint",
    ],
    builtins: &[
        "writeln", "write", "readln", "inc", "dec", "length", "setlength", "high", "low", "ord", "chr", "assigned",
        "freeandnil", "inttostr", "strtoint", "format", "copy", "pos", "trim", "uppercase", "lowercase", "sizeof",
    ],
    constants: &["true", "false", "nil"],
    variables: &["self", "result"],
    function_keywords: &["function", "procedure", "constructor", "destructor"],
    case_insensitive: true,
    features: CALLS | MEMBERS,
    hook: Some(pascal_hook),
    hook_chars: "{(#$%&TEI",
    ..Spec::DEFAULT
};

pub static PASCAL: Definition = Definition::new("pascal", "Pascal", Engine::Generic(&PASCAL_SPEC))
    .aliases(&["delphi", "objectpascal", "freepascal", "pas", "dpr"])
    .extensions(&["pas", "dpr", "dpk", "lpr", "lfm"]);

// Ada and VHDL

/// Character literals, `Name'Attribute` ticks and based numbers like `16#FF#`.
fn tick_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    if ctx.peek() == b'\'' {
        let prev = if start > 0 { ctx.src[start - 1] } else { 0 };
        if is_ident(prev) || prev == b')' {
            if next.is_ascii_alphabetic() {
                let end = lx.scan_ident(start + 1);
                put(lx.ctx, start, end, Kind::Meta);
                lx.mark_value();
                return true;
            }
            return false;
        }
        if ctx.at(start + 2) == b'\'' && (b' '..0x80).contains(&next) {
            put(ctx, start, start + 3, Kind::String);
            lx.mark_value();
            return true;
        }
        return false;
    }
    let end = scan(ctx, start, |c| c.is_ascii_digit() || c == b'_');
    if ctx.at(end) != b'#' || !ctx.at(end + 1).is_ascii_hexdigit() {
        return false;
    }
    let close = scan(ctx, end + 1, |c| c.is_ascii_hexdigit() || c == b'_' || c == b'.');
    if ctx.at(close) != b'#' {
        return false;
    }
    ctx.pos = close + 1;
    if ctx.peek() | 0x20 == b'e' {
        let sign = matches!(ctx.peek_at(1), b'+' | b'-') as usize;
        if ctx.peek_at(1 + sign).is_ascii_digit() {
            ctx.pos = scan(ctx, ctx.pos + 1 + sign, |c| c.is_ascii_digit());
        }
    }
    ctx.emit_from(start, Kind::Number);
    lx.mark_value();
    true
}

static ADA_SPEC: Spec = Spec {
    line_comments: &["--"],
    strings: &[Str::quoted("\"").raw().doubled()],
    keywords: &[
        "abort", "abs", "abstract", "accept", "access", "aliased", "all", "and", "array", "at", "begin", "body",
        "case", "constant", "declare", "delay", "delta", "digits", "do", "else", "elsif", "end", "entry", "exception",
        "exit", "for", "function", "generic", "goto", "if", "in", "interface", "is", "limited", "loop", "mod", "new",
        "not", "of", "or", "others", "out", "overriding", "package", "pragma", "private", "procedure", "protected",
        "raise", "range", "record", "rem", "renames", "requeue", "return", "reverse", "select", "separate", "some",
        "subtype", "synchronized", "tagged", "task", "terminate", "then", "type", "until", "use", "when", "while",
        "with", "xor",
    ],
    types: &[
        "integer", "natural", "positive", "float", "long_float", "long_integer", "boolean", "character", "string",
        "wide_string", "unbounded_string", "duration", "wide_character",
    ],
    constants: &["true", "false", "null"],
    function_keywords: &["function", "procedure"],
    type_keywords: &["type", "subtype", "package", "body", "task"],
    case_insensitive: true,
    operators: "+-*/=&|<>:",
    hook: Some(tick_hook),
    hook_chars: "'0123456789",
    ..Spec::DEFAULT
};

/// The subtype in VHDL declarations: `name : [in|out] type`. The declared
/// names themselves come from `IDENT_KEYS`.
fn vhdl_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let b = lx.ctx.peek();
    if b == b'\'' || b.is_ascii_digit() {
        return tick_hook(lx);
    }
    let ctx = &*lx.ctx;
    let prev = ctx.prev_non_space(start);
    let typed = prev == b':'
        || (prev.is_ascii_alphabetic() && any_eq(word_before(ctx, start), &["in", "out", "inout", "buffer"]));
    if !typed {
        return false;
    }
    let end = lx.scan_ident(start);
    lx.ctx.pos = end;
    let kind = match lx.classify(start, end) {
        Kind::Text | Kind::Constant => Kind::Type,
        k => k,
    };
    lx.ctx.emit(start, end, kind);
    true
}

pub static ADA: Definition = Definition::new("ada", "Ada", Engine::Generic(&ADA_SPEC))
    .aliases(&["ada95", "ada2012", "adb", "ads"])
    .extensions(&["adb", "ads", "ada"]);

static VHDL_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").raw().doubled()],
    string_prefixes: &[
        Prefix::plain(b'x'),
        Prefix::plain(b'X'),
        Prefix::plain(b'b'),
        Prefix::plain(b'B'),
        Prefix::plain(b'o'),
        Prefix::plain(b'O'),
    ],
    keywords: &[
        "abs", "access", "after", "alias", "all", "and", "array", "assert", "attribute", "begin", "block", "buffer",
        "bus", "case", "configuration", "constant", "disconnect", "downto", "else", "elsif", "end", "exit", "file",
        "for", "function", "generate", "generic", "group", "guarded", "if", "impure", "in", "inertial", "inout", "is",
        "label", "library", "linkage", "literal", "loop", "map", "mod", "nand", "new", "next", "nor", "not", "null",
        "of", "on", "open", "or", "others", "out", "port", "postponed", "procedure", "process", "pure", "range",
        "record", "register", "reject", "rem", "report", "return", "rol", "ror", "select", "severity", "shared",
        "signal", "sla", "sll", "sra", "srl", "subtype", "then", "to", "transport", "type", "unaffected", "units",
        "until", "use", "variable", "wait", "when", "while", "with", "xnor", "xor", "entity", "architecture",
        "component", "package", "body", "context", "force", "release",
    ],
    types: &[
        "std_logic", "std_logic_vector", "std_ulogic", "std_ulogic_vector", "signed", "unsigned", "integer", "natural",
        "positive", "boolean", "bit", "bit_vector", "string", "character", "time", "real",
    ],
    builtins: &[
        "rising_edge", "falling_edge", "to_integer", "to_unsigned", "to_signed", "resize", "to_stdlogicvector",
        "conv_integer", "conv_std_logic_vector", "shift_left", "shift_right", "now",
    ],
    constants: &["true", "false", "note", "warning", "error", "failure"],
    function_keywords: &["function", "procedure"],
    type_keywords: &[
        "entity", "architecture", "component", "configuration", "package", "body", "type", "subtype", "of",
    ],
    case_insensitive: true,
    operators: "+-*/=&|<>:",
    features: UPPER_CONSTANTS | IDENT_KEYS,
    hook: Some(vhdl_hook),
    hook_chars: concat!(letters!(), "'0123456789"),
    ..Spec::DEFAULT
};

pub static VHDL: Definition =
    Definition::new("vhdl", "VHDL", Engine::Generic(&VHDL_SPEC)).aliases(&["vhd"]).extensions(&["vhd", "vhdl"]);

// Verilog and SystemVerilog

/// Sized literals like `8'hFF`, `` `define `` directives and `.port(sig)` connections.
fn verilog_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'`' if next.is_ascii_alphabetic() || next == b'_' => {
            let end = scan(ctx, start + 1, is_ident);
            put(ctx, start, end, Kind::Meta);
            true
        }
        b'.' if next.is_ascii_alphabetic() || next == b'_' => {
            let end = scan(ctx, start + 1, is_ident);
            if ctx.next_non_space(end) != b'(' {
                return false;
            }
            // `.port(signal)` connections versus `obj.method()` calls.
            let prev = if start > 0 { ctx.src[start - 1] } else { 0 };
            let kind = if is_ident(prev) || matches!(prev, b')' | b']') { Kind::Function } else { Kind::Property };
            ctx.token(1, Kind::Punctuation);
            put(ctx, start + 1, end, kind);
            true
        }
        b if b.is_ascii_alphabetic() || b == b'_' => verilog_word(lx),
        b => {
            let tick = if b == b'\'' { start } else { scan(ctx, start, |c| c.is_ascii_digit() || c == b'_') };
            if ctx.at(tick) != b'\'' {
                return false;
            }
            let mut i = tick + 1;
            if ctx.at(i) | 0x20 == b's' {
                i += 1;
            }
            let based = matches!(ctx.at(i) | 0x20, b'b' | b'o' | b'd' | b'h');
            let end = if based {
                let digits = ctx.skip_spaces_from(i + 1);
                scan(ctx, digits, |c| c.is_ascii_hexdigit() || matches!(c | 0x20, b'x' | b'z' | b'_' | b'?'))
            } else if b == b'\''
                && matches!(ctx.at(i), b'0' | b'1' | b'x' | b'z' | b'X' | b'Z')
                && !is_ident(ctx.at(i + 1))
            {
                i + 1
            } else {
                return false;
            };
            put(ctx, start, end, Kind::Number);
            lx.mark_value();
            true
        }
    }
}

/// Module instantiations: `counter #(...) u0 (...)` and `counter u0 (...)`.
fn verilog_word(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let end = lx.scan_ident(start);
    let after = lx.ctx.skip_spaces_from(end);
    lx.ctx.pos = end;
    let kind = lx.classify(start, end);
    let ctx = &*lx.ctx;
    let kind = match kind {
        Kind::Text | Kind::Function if ctx.at(after) == b'#' && ctx.at(after + 1) == b'(' => Kind::Type,
        Kind::Text if ctx.at(after).is_ascii_alphabetic() || ctx.at(after) == b'_' => {
            let name_end = scan(ctx, after, is_ident);
            let name = &ctx.src[after..name_end];
            if ctx.next_non_space(name_end) == b'(' && lx.word_kind(name) != Some(Kind::Keyword) {
                Kind::Type
            } else {
                Kind::Text
            }
        }
        Kind::Function if verilog_instance(lx, start) => Kind::Text,
        k => k,
    };
    lx.ctx.emit(start, end, kind);
    true
}

/// Whether the identifier at `start` follows a module name or a `#(...)` parameter list.
fn verilog_instance(lx: &Lexer, start: usize) -> bool {
    let ctx = &*lx.ctx;
    let mut i = start;
    while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t' | b'\n' | b'\r') {
        i -= 1;
    }
    if i > 0 && ctx.src[i - 1] == b')' {
        let mut depth = 0;
        let limit = i.saturating_sub(4096);
        while i > limit {
            i -= 1;
            match ctx.src[i] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        while i > 0 && matches!(ctx.src[i - 1], b' ' | b'\t') {
                            i -= 1;
                        }
                        return i > 0 && ctx.src[i - 1] == b'#';
                    }
                }
                _ => {}
            }
        }
        return false;
    }
    let word_end = i;
    while i > 0 && is_ident(ctx.src[i - 1]) {
        i -= 1;
    }
    let word = &ctx.src[i..word_end];
    !word.is_empty() && !word[0].is_ascii_digit() && !matches!(lx.word_kind(word), Some(Kind::Keyword | Kind::Type))
}

const VERILOG_KEYWORDS: &[&str] = &[
    "always", "and", "assign", "automatic", "begin", "buf", "case", "casex", "casez", "deassign", "default",
    "defparam", "disable", "edge", "else", "end", "endcase", "endfunction", "endgenerate", "endmodule", "endprimitive",
    "endspecify", "endtask", "event", "for", "force", "forever", "fork", "function", "generate", "genvar", "if",
    "initial", "inout", "input", "join", "localparam", "macromodule", "module", "nand", "negedge", "nor", "not", "or",
    "output", "parameter", "posedge", "primitive", "release", "repeat", "signed", "specify", "specparam", "task",
    "unsigned", "wait", "while", "xnor", "xor",
];

const VERILOG_TYPES: &[&str] =
    &["wire", "reg", "integer", "real", "realtime", "time", "tri", "tri0", "tri1", "wand", "wor", "supply0", "supply1"];

static VERILOG_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    keywords: VERILOG_KEYWORDS,
    types: VERILOG_TYPES,
    function_keywords: &["function", "task"],
    type_keywords: &["module"],
    sigils: &[(b'$', Kind::Builtin)],
    features: CALLS | UPPER_CONSTANTS,
    hook: Some(verilog_hook),
    hook_chars: concat!(letters!(), "_`.'0123456789"),
    ..Spec::DEFAULT
};

static SYSTEMVERILOG_SPEC: Spec = Spec {
    keywords: &[
        "always", "always_comb", "always_ff", "always_latch", "and", "assert", "assign", "assume", "automatic",
        "begin", "break", "case", "casex", "casez", "class", "clocking", "const", "constraint", "continue", "cover",
        "covergroup", "default", "disable", "do", "else", "end", "endcase", "endclass", "endclocking", "endfunction",
        "endgenerate", "endgroup", "endinterface", "endmodule", "endpackage", "endprogram", "endproperty", "endtask",
        "enum", "export", "extends", "extern", "final", "for", "foreach", "forever", "fork", "function", "generate",
        "genvar", "if", "iff", "implements", "import", "initial", "inout", "input", "inside", "interface", "join",
        "join_any", "join_none", "local", "localparam", "modport", "module", "negedge", "new", "not", "or", "output",
        "package", "packed", "parameter", "posedge", "priority", "program", "property", "protected", "pure", "rand",
        "randc", "ref", "repeat", "return", "sequence", "signed", "static", "struct", "super", "task", "this",
        "typedef", "union", "unique", "unsigned", "virtual", "wait", "while", "with",
    ],
    types: &[
        "wire", "reg", "logic", "bit", "byte", "shortint", "int", "longint", "integer", "real", "shortreal",
        "realtime", "time", "string", "chandle", "event", "void", "tri", "supply0", "supply1",
    ],
    constants: &["null"],
    type_keywords: &["module", "class", "interface", "package", "program", "extends"],
    ..VERILOG_SPEC
};

pub static VERILOG: Definition =
    Definition::new("verilog", "Verilog", Engine::Generic(&VERILOG_SPEC)).extensions(&["v", "vh"]);

pub static SYSTEMVERILOG: Definition =
    Definition::new("systemverilog", "SystemVerilog", Engine::Generic(&SYSTEMVERILOG_SPEC))
        .aliases(&["sv"])
        .extensions(&["sv", "svh"]);

// COBOL

/// Fixed format comment lines, `PIC` clauses and paragraph names.
fn cobol_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let b = lx.ctx.peek();
    if b.is_ascii_digit() {
        // Sequence numbers in columns 1 to 6 of fixed format source.
        let ctx = &mut *lx.ctx;
        let end = scan(ctx, start, |c| c.is_ascii_digit());
        if end - start != 6 || (start > 0 && ctx.src[start - 1] != b'\n') || is_ident(ctx.at(end)) {
            return false;
        }
        put(ctx, start, end, Kind::Comment);
        return true;
    }
    if b == b'*' || b == b'/' {
        let ctx = &mut *lx.ctx;
        let line = ctx.src[..start].iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1);
        if start - line == 6 && ctx.src[line..start].iter().all(|c| c.is_ascii_digit() || *c == b' ') {
            let end = ctx.line_end(start);
            put(ctx, start, end, Kind::Comment);
            return true;
        }
        return false;
    }
    let end = lx.scan_ident(start);
    let ctx = &mut *lx.ctx;
    let word = &ctx.src[start..end];
    if any_eq(word, &["pic", "picture"]) {
        put(ctx, start, end, Kind::Keyword);
        let mut i = ctx.skip_spaces_from(end);
        let is = scan(ctx, i, |c| c.is_ascii_alphabetic());
        if any_eq(&ctx.src[i..is], &["is"]) {
            ctx.emit(i, is, Kind::Keyword);
            i = ctx.skip_spaces_from(is);
        }
        let mut pic_end = scan(ctx, i, |c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r'));
        if pic_end > i && ctx.at(pic_end - 1) == b'.' {
            pic_end -= 1;
        }
        put(ctx, i, pic_end, Kind::Type);
        return true;
    }
    let line = ctx.src[..start].iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1);
    let prefix = &ctx.src[line..start];
    let area_a = prefix.iter().all(|&c| c == b' ')
        || (prefix.len() >= 7 && prefix[..6].iter().all(u8::is_ascii_digit) && prefix[6..].iter().all(|&c| c == b' '));
    if area_a && ctx.at(end) == b'.' && rest_blank(ctx, end + 1) {
        let kind = match lx.classify(start, end) {
            Kind::Text => Kind::Function,
            k => k,
        };
        put(lx.ctx, start, end, kind);
        return true;
    }
    false
}

static COBOL_SPEC: Spec = Spec {
    line_comments: &["*>"],
    strings: &[Str::quoted("\"").raw().doubled(), Str::quoted("'").raw().doubled()],
    keywords: &[
        "accept", "access", "add", "advancing", "after", "all", "alphabetic", "and", "are", "area", "ascending",
        "assign", "at", "before", "binary", "blank", "by", "call", "cancel", "character", "close", "comp", "comp-3",
        "comp-5", "compute", "configuration", "continue", "copy", "corresponding", "data", "delete", "descending",
        "display", "divide", "division", "else", "end", "end-call", "end-compute", "end-evaluate", "end-if",
        "end-perform", "end-read", "end-search", "end-string", "end-write", "environment", "equal", "evaluate", "exit",
        "fd", "file", "file-control", "filler", "from", "giving", "go", "goback", "greater", "id", "identification",
        "if", "in", "indexed", "initialize", "input", "input-output", "inspect", "into", "is", "key", "less",
        "linkage", "local-storage", "mode", "move", "multiply", "next", "not", "occurs", "of", "open", "or",
        "organization", "other", "output", "packed-decimal", "perform", "procedure", "program", "program-id", "read",
        "record", "redefines", "remainder", "replacing", "return", "rewrite", "rounded", "run", "search", "section",
        "select", "sequential", "set", "sign", "size", "sort", "source-computer", "stop", "string", "subtract",
        "tallying", "than", "then", "thru", "through", "times", "to", "unstring", "until", "upon", "usage", "using",
        "value", "values", "varying", "when", "with", "working-storage", "write", "author", "function", "returning",
        "reference", "content", "line", "lines", "page", "screen",
    ],
    constants: &[
        "zero", "zeros", "zeroes", "space", "spaces", "high-value", "high-values", "low-value", "low-values", "quote",
        "quotes", "null", "nulls", "true", "false",
    ],
    function_keywords: &["perform", "function"],
    ident_chars: "-",
    case_insensitive: true,
    operators: "+-*/=<>&",
    hook: Some(cobol_hook),
    hook_chars: concat!(letters!(), "*/0123456789"),
    ..Spec::DEFAULT
};

pub static COBOL: Definition = Definition::new("cobol", "COBOL", Engine::Generic(&COBOL_SPEC))
    .aliases(&["cob", "cbl"])
    .extensions(&["cob", "cbl", "cpy", "cobol"]);

// Visual Basic family

/// `$"..."` interpolation, `#date#` literals, `&HFF` numbers, `REM` and `End Sub` pairs.
fn vb_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let next = lx.ctx.peek_at(1);
    match lx.ctx.peek() {
        b'$' if next == b'"' => {
            lx.ctx.pos += 1;
            lx.string(start, &VB_INTERP, false, Interp::Brace);
            true
        }
        b'#' => {
            let ctx = &mut *lx.ctx;
            if ctx.at_line_start(start) {
                return false;
            }
            let body = scan(ctx, start + 1, |c| c.is_ascii_digit() || b"/-: .APMapm".contains(&c));
            if body == start + 1 || ctx.at(body) != b'#' || !ctx.at(start + 1).is_ascii_digit() {
                return false;
            }
            put(ctx, start, body + 1, Kind::String);
            lx.mark_value();
            true
        }
        b'&' => {
            let ctx = &mut *lx.ctx;
            let end = match next | 0x20 {
                b'h' => scan(ctx, start + 2, |c| c.is_ascii_hexdigit()),
                b'o' => scan(ctx, start + 2, |c| (b'0'..=b'7').contains(&c)),
                b'b' => scan(ctx, start + 2, |c| c == b'0' || c == b'1'),
                _ => return false,
            };
            if end == start + 2 {
                return false;
            }
            let end = scan(ctx, end, |c| c.is_ascii_alphabetic());
            put(ctx, start, end, Kind::Number);
            lx.mark_value();
            true
        }
        _ => {
            let end = lx.scan_ident(start);
            let ctx = &mut *lx.ctx;
            let word = &ctx.src[start..end];
            if any_eq(word, &["rem"]) {
                let line_end = ctx.line_end(start);
                put(ctx, start, line_end, Kind::Comment);
                return true;
            }
            if !any_eq(word, &["end", "exit"]) {
                return false;
            }
            put(ctx, start, end, Kind::Keyword);
            let i = ctx.skip_spaces_from(end);
            let j = scan(ctx, i, |c| c.is_ascii_alphabetic());
            if j > i && !is_ident(ctx.at(j)) {
                put(ctx, i, j, Kind::Keyword);
            }
            lx.mark_operator();
            true
        }
    }
}

static VB_INTERP: Str = Str::quoted("\"").raw().doubled();

/// Keywords VB.NET and VBA share; each dialect adds its own below.
macro_rules! vb_keywords {
    ($($extra:literal),* $(,)?) => {
        &[
            "alias", "and", "as", "byref", "byval", "call", "case", "const", "declare", "dim", "do", "each", "else",
            "elseif", "end", "enum", "erase", "error", "event", "exit", "for", "friend", "function", "get", "goto",
            "if", "implements", "in", "is", "lib", "like", "loop", "mod", "new", "next", "not", "on", "option",
            "optional", "or", "paramarray", "private", "property", "public", "raiseevent", "redim", "resume",
            "return", "select", "set", "static", "step", "stop", "sub", "then", "to", "typeof", "until", "while",
            "with", "withevents", "xor", "explicit", "compare", $($extra),*
        ]
    };
}

static VBNET_SPEC: Spec = Spec {
    line_comments: &["'"],
    strings: &[Str::quoted("\"").raw().doubled()],
    keywords: vb_keywords![
        "addhandler", "addressof", "andalso", "catch", "class", "continue", "default", "delegate", "finally", "global",
        "handles", "imports", "inherits", "interface", "isnot", "module", "mustinherit", "mustoverride", "namespace",
        "narrowing", "of", "operator", "orelse", "overloads", "overridable", "overrides", "partial", "protected",
        "readonly", "removehandler", "shadows", "shared", "structure", "synclock", "throw", "try", "trycast", "using",
        "when", "widening", "writeonly", "async", "await", "yield", "iterator", "strict", "ctype", "directcast",
        "gettype", "nameof",
    ],
    types: &[
        "boolean", "byte", "date", "decimal", "double", "integer", "long", "object", "single", "string", "char",
        "sbyte", "short", "uinteger", "ulong", "ushort",
    ],
    builtins: &[
        "msgbox", "inputbox", "console", "debug", "cstr", "cint", "clng", "cdbl", "cbool", "cdate", "len", "left",
        "right", "mid", "trim", "ucase", "lcase", "instr", "replace", "split", "join", "format", "now", "isnothing",
        "isnumeric",
    ],
    constants: &["true", "false", "nothing", "vbcrlf", "vbtab", "vbnewline"],
    variables: &["me", "mybase", "myclass"],
    type_keywords: &["as", "of", "class", "structure", "module", "interface", "enum", "new", "inherits", "implements"],
    case_insensitive: true,
    features: CALLS | MEMBERS | PREPROCESSOR,
    hook: Some(vb_hook),
    hook_chars: "$#&rReE",
    ..Spec::DEFAULT
};

static VBA_SPEC: Spec = Spec {
    keywords: vb_keywords!["let", "wend", "gosub", "attribute", "type", "open", "close", "print"],
    types: &[
        "boolean", "byte", "date", "decimal", "double", "integer", "long", "object", "single", "string", "variant",
        "currency", "collection", "longlong", "longptr",
    ],
    builtins: &[
        "msgbox", "inputbox", "debug", "cstr", "cint", "clng", "cdbl", "cbool", "cdate", "len", "left", "right", "mid",
        "trim", "ucase", "lcase", "instr", "replace", "split", "join", "format", "now", "isnumeric", "isempty",
        "isnull", "isobject", "createobject", "application", "activesheet", "activeworkbook", "worksheets", "range",
        "cells", "thisworkbook",
    ],
    constants: &["true", "false", "nothing", "null", "empty", "vbcrlf", "vbtab", "vbnewline", "vbnullstring"],
    variables: &["me"],
    type_keywords: &["as", "new", "enum", "type", "implements"],
    hook_chars: "#&rReE",
    ..VBNET_SPEC
};

pub static VBNET: Definition = Definition::new("vbnet", "Visual Basic .NET", Engine::Generic(&VBNET_SPEC))
    .aliases(&["vb", "vb.net", "visualbasic", "visual-basic"])
    .extensions(&["vb"]);

pub static VBA: Definition = Definition::new("vba", "VBA", Engine::Generic(&VBA_SPEC))
    .aliases(&["vbscript", "vbs", "bas", "visual-basic-for-applications"])
    .extensions(&["vba", "vbs", "bas"]);

static BASIC_SPEC: Spec = Spec {
    line_comments: &["'"],
    strings: &[Str::quoted("\"").raw()],
    keywords: &[
        "and", "as", "call", "case", "cls", "color", "const", "data", "declare", "def", "dim", "do", "else", "elseif",
        "end", "exit", "for", "function", "gosub", "goto", "if", "input", "is", "let", "line", "locate", "loop", "mod",
        "next", "not", "on", "open", "close", "or", "print", "randomize", "read", "redim", "restore", "return",
        "screen", "select", "shared", "sleep", "step", "sub", "system", "then", "to", "type", "until", "using", "wend",
        "while", "xor", "beep", "erase", "static", "byval", "byref", "namespace", "scope",
    ],
    types: &["integer", "long", "single", "double", "string", "byte", "boolean", "short", "ulong", "any", "ptr"],
    builtins: &[
        "abs", "asc", "chr$", "cint", "cos", "exp", "inkey$", "instr", "int", "lcase$", "left$", "len", "log", "mid$",
        "right$", "rnd", "sgn", "sin", "space$", "sqr", "str$", "string$", "tan", "timer", "ucase$", "val", "ltrim$",
        "rtrim$",
    ],
    constants: &["true", "false"],
    ident_suffix: "$%!#&",
    case_insensitive: true,
    features: CALLS | PREPROCESSOR,
    hook: Some(vb_hook),
    hook_chars: "&rReE",
    ..Spec::DEFAULT
};

pub static BASIC: Definition = Definition::new("basic", "BASIC", Engine::Generic(&BASIC_SPEC))
    .aliases(&["qbasic", "freebasic", "qb64", "quickbasic"])
    .extensions(&["bi", "qb64"]);

// Forth

/// Whether `word` is a Forth number such as `-42`, `$FF`, `%101`, `3.`, `1e3` or `'c'`.
fn forth_number(word: &[u8]) -> bool {
    if word.len() == 3 && word[0] == b'\'' && word[2] == b'\'' {
        return true;
    }
    let (hex, rest) = match word.first() {
        Some(b'$') => (true, &word[1..]),
        Some(b'#' | b'%' | b'&') => (false, &word[1..]),
        _ => (false, word),
    };
    let rest = rest.strip_prefix(b"-").unwrap_or(rest);
    match rest.first() {
        Some(c) if c.is_ascii_digit() || (hex && c.is_ascii_hexdigit()) => {}
        _ => return false,
    }
    rest.iter()
        .all(|&c| c.is_ascii_digit() || (hex && c.is_ascii_hexdigit()) || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
}

/// Forth is whitespace delimited: every word goes through this hook.
fn forth_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let src = lx.ctx.src;
    let end = scan(lx.ctx, start, |c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r'));
    let word = &src[start..end];
    let ctx = &mut *lx.ctx;
    match word {
        b"\\" => {
            let end = ctx.line_end(start);
            put(ctx, start, end, Kind::Comment);
        }
        b"(" | b".(" => {
            let close = ctx.src[end..].iter().position(|&c| c == b')').map_or(ctx.len(), |i| end + i + 1);
            put(ctx, start, close, if word == b"(" { Kind::Comment } else { Kind::String });
        }
        _ if word.len() > 1
            && word.ends_with(b"\"")
            && any_eq(word, &[".\"", "s\"", "c\"", "abort\"", "s\\\"", ",\""]) =>
        {
            put(ctx, start, end, Kind::Builtin);
            let body = (end + 1).min(ctx.len());
            let line_end = ctx.line_end(body);
            let close = ctx.src[body..line_end].iter().position(|&c| c == b'"').map_or(line_end, |i| body + i + 1);
            put(ctx, end, close, Kind::String);
        }
        _ => {
            ctx.pos = end;
            let kind = match lx.classify(start, end) {
                Kind::Text if forth_number(word) => Kind::Number,
                Kind::Keyword
                    if any_eq(
                        word,
                        &[
                            "variable", "2variable", "fvariable", "constant", "2constant", "fconstant", "value",
                            "create", "to",
                        ],
                    ) =>
                {
                    // The next word is the name being defined.
                    lx.set_pending(Some(Kind::Variable));
                    Kind::Keyword
                }
                k => k,
            };
            lx.ctx.emit(start, end, kind);
        }
    }
    true
}

static FORTH_SPEC: Spec = Spec {
    keywords: &[
        ":", ";", "if", "else", "then", "begin", "until", "again", "while", "repeat", "do", "?do", "loop", "+loop",
        "leave", "unloop", "exit", "recurse", "case", "of", "endof", "endcase", "immediate", "does>", "postpone",
        "literal", "[", "]", "'", "[']", "[char]", "char", "variable", "2variable", "fvariable", "constant",
        "2constant", "fconstant", "value", "to", "create", "allot", "defer", "is", "include", "require", "marker",
        ":noname", "execute", "abort", "quit", "bye",
    ],
    builtins: &[
        "dup", "drop", "swap", "over", "rot", "-rot", "nip", "tuck", "pick", "roll", "2dup", "2drop", "2swap", "2over",
        "?dup", ">r", "r>", "r@", "@", "!", "c@", "c!", "+!", ".", ".s", "emit", "cr", "type", "key", "space",
        "spaces", "+", "-", "*", "/", "mod", "/mod", "*/", "negate", "abs", "min", "max", "and", "or", "xor", "invert",
        "lshift", "rshift", "=", "<>", "<", ">", "0=", "0<", "0>", "1+", "1-", "2*", "2/", "u.", "bl", "count", "here",
        ",", "c,", "cells", "cell+", "chars", "fill", "move", "depth", "true", "false",
    ],
    function_keywords: &[":", "defer"],
    case_insensitive: true,
    hook: Some(forth_hook),
    ..Spec::DEFAULT
};

pub static FORTH: Definition =
    Definition::new("forth", "Forth", Engine::Generic(&FORTH_SPEC)).extensions(&["fth", "4th", "forth", "frt"]);

// MATLAB and Octave

static MATLAB_CHARS: Str = Str::quoted("'").raw().doubled();

/// Transpose versus strings, `%{ %}` block comments and `function` signatures.
fn matlab_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    match lx.ctx.peek() {
        b'\'' => {
            let prev = if start > 0 { lx.ctx.src[start - 1] } else { 0 };
            if is_ident(prev) || matches!(prev, b')' | b']' | b'}' | b'\'' | b'.') {
                lx.ctx.token(1, Kind::Operator);
                lx.mark_value();
            } else {
                lx.string(start, &MATLAB_CHARS, false, Interp::None);
            }
            true
        }
        b'%' | b'#' => {
            let ctx = &mut *lx.ctx;
            if ctx.peek_at(1) != b'{' || !ctx.at_line_start(start) || !rest_blank(ctx, start + 2) {
                return false;
            }
            let close = if ctx.peek() == b'%' { b"%}" } else { b"#}" };
            let end = ctx.find(start + 2, close).map_or(ctx.len(), |i| i + 2);
            put(ctx, start, end, Kind::Comment);
            true
        }
        _ => {
            let end = lx.scan_ident(start);
            let word = &lx.ctx.src[start..end];
            if word == b"function" {
                put(lx.ctx, start, end, Kind::Keyword);
                // `function [a, b] = name(...)`: lex the outputs, then name the function.
                let ctx = &mut *lx.ctx;
                let line_end = ctx.line_end_within(end, 512);
                let paren = ctx.find_in(end, line_end, b"(").unwrap_or(line_end);
                let eq = ctx.find_in(end, paren, b"=");
                if let Some(eq) = eq {
                    ctx.embed_spec(&MATLAB_SPEC, end, eq);
                    put(ctx, eq, eq + 1, Kind::Operator);
                }
                // Property accessors `get.Name` are named as a whole; plain names are left pending.
                let name = ctx.skip_spaces_from(eq.map_or(end, |e| e + 1));
                let name_end = scan(ctx, name, |c| is_ident(c) || c == b'.');
                if ctx.src[name..name_end].contains(&b'.') {
                    put(ctx, name, name_end, Kind::Function);
                } else {
                    lx.set_pending(Some(Kind::Function));
                }
                return true;
            }
            matlab_command(lx, start, end)
        }
    }
}

/// Command syntax at the start of a statement: `hold on`, `format long`, `close all`, `clc`.
fn matlab_command(lx: &mut Lexer, start: usize, end: usize) -> bool {
    const COMMANDS: &[&str] = &[
        "hold", "format", "close", "clear", "clc", "clf", "clearvars", "warning", "more", "pkg", "load", "save", "cd",
        "axis", "grid", "box", "figure", "echo", "diary", "syms", "rng", "drawnow", "shg",
    ];
    let ctx = &*lx.ctx;
    let word = &ctx.src[start..end];
    if word.len() < 2 || word.len() > 9 || !matches!(byte_before(ctx, start), 0 | b'\n' | b';' | b',') {
        return false;
    }
    // A plain call or assignment is not command syntax: `hold(ax, 'on')`, `figure = 1`, `close - 1`.
    let arg = ctx.skip_spaces_from(end);
    let after = ctx.at(arg);
    let has_args = arg > end
        && (after.is_ascii_alphanumeric()
            || matches!(after, b'.' | b'/')
            || (after == b'-' && ctx.at(arg + 1) != b' '));
    if !(has_args || matches!(after, b'\n' | b'\r' | b';' | b',' | b'%' | 0)) || !any_eq(word, COMMANDS) {
        return false;
    }
    put(lx.ctx, start, end, Kind::Builtin);
    if has_args {
        let ctx = &mut *lx.ctx;
        let stop = scan(ctx, arg, |c| !matches!(c, b';' | b',' | b'%' | b'\n'));
        let mut text_end = stop;
        while text_end > arg && matches!(ctx.src[text_end - 1], b' ' | b'\t' | b'\r') {
            text_end -= 1;
        }
        put(ctx, arg, text_end, Kind::String);
    }
    lx.mark_value();
    true
}

static MATLAB_SPEC: Spec = Spec {
    line_comments: &["%", "#", "..."],
    strings: &[Str::quoted("\"").raw().doubled()],
    keywords: &[
        "break", "case", "catch", "classdef", "continue", "else", "elseif", "end", "for", "function", "global", "if",
        "otherwise", "parfor", "persistent", "return", "spmd", "switch", "try", "while", "properties", "methods",
        "events", "enumeration", "arguments", "endfunction", "endif", "endfor", "endwhile", "unwind_protect",
    ],
    builtins: &[
        "disp", "fprintf", "sprintf", "printf", "zeros", "ones", "eye", "rand", "randn", "size", "length", "numel",
        "plot", "figure", "hold", "xlabel", "ylabel", "title", "legend", "grid", "subplot", "error", "warning",
        "isempty", "strcmp", "num2str", "str2num", "str2double", "max", "min", "sum", "mean", "abs", "sqrt", "floor",
        "ceil", "round", "mod", "linspace", "cell", "struct", "fieldnames", "isfield", "cellfun", "arrayfun", "find",
        "any", "all", "sort", "unique", "reshape", "repmat", "inv", "det", "exp", "log", "sin", "cos", "nargin",
        "nargout", "varargin", "varargout", "numel", "class", "isa", "tic", "toc",
    ],
    constants: &["true", "false", "pi", "eps", "Inf", "inf", "NaN", "nan"],
    features: MEMBERS,
    hook: Some(matlab_hook),
    hook_chars: "'%#abcdefghlmprs",
    ..Spec::DEFAULT
};

pub static MATLAB: Definition =
    Definition::new("matlab", "MATLAB", Engine::Generic(&MATLAB_SPEC)).aliases(&["octave", "matlab-octave"]);

// Wolfram Language

/// End of a named character such as `\[Alpha]` at `pos`, or `pos` if there is none.
fn wolfram_named_char(ctx: &Ctx, pos: usize) -> usize {
    if ctx.at(pos) != b'\\' || ctx.at(pos + 1) != b'[' {
        return pos;
    }
    let end = scan(ctx, pos + 2, |c| c.is_ascii_alphanumeric());
    if end > pos + 2 && ctx.at(end) == b']' {
        end + 1
    } else {
        pos
    }
}

/// A string where `\[Name]`, `\:XXXX` and `\.XX` are escapes as well as `\n` style ones.
fn wolfram_string(ctx: &mut Ctx, start: usize) {
    let mut i = start + 1;
    let mut seg = start;
    while i < ctx.len() {
        match ctx.src[i] {
            b'"' => {
                i += 1;
                break;
            }
            b'\\' => {
                let mut end = wolfram_named_char(ctx, i);
                if end == i {
                    let hex = match ctx.at(i + 1) {
                        b':' => 4,
                        b'.' => 2,
                        _ => 0,
                    };
                    end = if hex > 0 { scan(ctx, i + 2, |c| c.is_ascii_hexdigit()).min(i + 2 + hex) } else { i + 2 };
                    // Never split a multi byte character.
                    while end < ctx.len() && (ctx.src[end] & 0xC0) == 0x80 {
                        end += 1;
                    }
                }
                ctx.emit(seg, i, Kind::String);
                ctx.emit(i, end, Kind::Escape);
                i = end.min(ctx.len());
                seg = i;
            }
            _ => i += 1,
        }
    }
    ctx.emit(seg, i, Kind::String);
    ctx.pos = i;
}

/// Symbols, `f[x]` calls, `x_Integer` patterns, `#` slots, strings and named characters.
fn wolfram_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let b = lx.ctx.peek();
    let symbol = |c: u8| c.is_ascii_alphanumeric() || c == b'$' || c == b'`';
    if b == b'"' {
        wolfram_string(lx.ctx, start);
        lx.mark_value();
        return true;
    }
    if b == b'\\' {
        let ctx = &mut *lx.ctx;
        let end = wolfram_named_char(ctx, start);
        if end == start {
            return false;
        }
        let name = &ctx.src[start + 2..end - 1];
        let kind = if any_eq(name, &["Pi", "Infinity", "ExponentialE", "ImaginaryI", "ImaginaryJ", "Degree"]) {
            Kind::Constant
        } else if any_eq(
            name,
            &[
                "Rule", "RuleDelayed", "Element", "NotElement", "Equal", "NotEqual", "LessEqual", "GreaterEqual",
                "And", "Or", "Not", "Implies", "Function", "Times", "Cross", "Divide", "Union", "Intersection",
                "Subset", "SubsetEqual", "Rightarrow", "LeftArrow", "RightArrow", "Map", "Distributed",
            ],
        ) {
            Kind::Operator
        } else if ctx.at(end) == b'_' {
            // A pattern such as `\[Alpha]_`.
            Kind::Variable
        } else {
            Kind::Text
        };
        put(ctx, start, end, kind);
        if kind == Kind::Operator {
            lx.mark_operator();
        } else {
            lx.mark_value();
        }
        return true;
    }
    if b == b'#' {
        let ctx = &mut *lx.ctx;
        let mut end = start + 1;
        if ctx.at(end) == b'#' {
            end += 1;
        }
        end = scan(ctx, end, |c| c.is_ascii_alphanumeric());
        put(ctx, start, end, Kind::Variable);
        lx.mark_value();
        return true;
    }
    let end = if b == b'_' { start } else { scan(lx.ctx, start, symbol) };
    if lx.ctx.at(end) == b'_' {
        let ctx = &mut *lx.ctx;
        let blanks = scan(ctx, end, |c| c == b'_').min(end + 3);
        let head = scan(ctx, blanks, symbol);
        let blanks = if ctx.at(blanks) == b'.' { blanks + 1 } else { blanks };
        put(ctx, start, blanks, Kind::Variable);
        put(ctx, blanks, head.max(blanks), Kind::Type);
        lx.mark_value();
        return true;
    }
    lx.ctx.pos = end;
    let kind = match lx.classify(start, end) {
        Kind::Text if lx.ctx.at(end) == b'[' && lx.ctx.at(end + 1) != b'[' => Kind::Function,
        k => k,
    };
    lx.ctx.emit(start, end, kind);
    true
}

static WOLFRAM_SPEC: Spec = Spec {
    block_comments: &[("(*", "*)")],
    nested_comments: true,
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "If", "Which", "Switch", "Do", "For", "While", "Module", "Block", "With", "Function", "Return", "Break",
        "Continue", "Throw", "Catch", "Check", "Begin", "End", "BeginPackage", "EndPackage", "Needs", "Get",
        "Condition", "Pattern", "Set", "SetDelayed", "Clear", "ClearAll", "Protect", "Unprotect", "SetAttributes",
    ],
    builtins: &[
        "Print", "Plot", "ListPlot", "Plot3D", "Table", "Map", "Apply", "Select", "Length", "Range", "Total", "Sort",
        "Join", "Append", "Prepend", "Part", "First", "Last", "Rest", "Take", "Drop", "Sum", "Product", "N", "Solve",
        "NSolve", "Integrate", "D", "Simplify", "FullSimplify", "Expand", "Factor", "Sqrt", "Sin", "Cos", "Exp", "Log",
        "Abs", "Max", "Min", "Mod", "StringJoin", "ToString", "Graphics", "Show", "Manipulate", "Association", "Keys",
        "Values", "Lookup", "Normal", "Flatten", "Partition", "Transpose", "Dot", "Det", "Inverse", "Fold", "Nest",
        "NestList", "FixedPoint", "Cases", "Count", "Position", "Head", "Echo", "Dataset", "Import", "Export",
        "Reverse", "Accumulate", "Differences", "Tally", "GroupBy", "AssociationThread", "StringSplit",
    ],
    constants: &[
        "True", "False", "Null", "None", "All", "Automatic", "Infinity", "Pi", "E", "I", "Indeterminate",
        "ComplexInfinity", "Missing", "Degree", "GoldenRatio",
    ],
    operators: "+-*/=&|^!<>~?:@.",
    hook: Some(wolfram_hook),
    hook_chars: concat!(letters!(), "$#_\\\""),
    ..Spec::DEFAULT
};

pub static WOLFRAM: Definition = Definition::new("wolfram", "Wolfram Language", Engine::Generic(&WOLFRAM_SPEC))
    .aliases(&["mathematica", "wl", "nb", "wolfram-language", "mma"])
    .extensions(&["wl", "nb", "wls", "wlt"]);

// SAS

const SAS_MACRO: &[&str] = &[
    "macro", "mend", "let", "put", "if", "then", "else", "do", "end", "to", "by", "while", "until", "global", "local",
    "include", "eval", "sysevalf", "sysfunc", "str", "nrstr", "quote", "nrquote", "bquote", "superq", "return", "goto",
    "symdel", "abort",
];

/// `&name`, `&&name` and `&name.` macro references in a double quoted SAS string.
fn sas_ref(ctx: &Ctx, i: usize) -> usize {
    if ctx.at(i) != b'&' {
        return i;
    }
    let mut j = i + 1;
    while ctx.at(j) == b'&' {
        j += 1;
    }
    let c = ctx.at(j);
    if !(c.is_ascii_alphabetic() || c == b'_') {
        return i;
    }
    let end = scan(ctx, j, is_ident);
    if ctx.at(end) == b'.' {
        end + 1
    } else {
        end
    }
}

/// `%macro` statements and calls, `* comment;` statements and macro references in strings.
fn sas_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    if ctx.peek() == b'"' {
        string_with_refs(ctx, start, 1, b"\"", sas_ref);
        lx.mark_value();
        return true;
    }
    if ctx.peek() == b'*' {
        if !matches!(ctx.prev_non_space(start), 0 | b';') {
            return false;
        }
        let end = ctx.find(start, b";").map_or(ctx.len(), |i| i + 1);
        put(ctx, start, end, Kind::Comment);
        return true;
    }
    if !(next.is_ascii_alphabetic() || next == b'_') {
        return false;
    }
    let end = scan(ctx, start + 1, is_ident);
    let word = &ctx.src[start + 1..end];
    if any_eq(word, SAS_MACRO) {
        let macro_def = any_eq(word, &["macro"]);
        let variable = any_eq(word, &["let", "global", "local"]);
        put(ctx, start, end, Kind::Keyword);
        if macro_def {
            lx.set_pending(Some(Kind::Function));
        } else if variable {
            // Macro variables are often named like keywords (`%let data = ...`), which
            // a pending kind would not override, so the name is taken here.
            let i = ctx.skip_spaces_from(end);
            let j = scan(ctx, i, is_ident);
            put(ctx, i, j, Kind::Variable);
        }
    } else {
        put(ctx, start, end, Kind::Function);
    }
    true
}

static SAS_SPEC: Spec = Spec {
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("'").raw().doubled(), Str::quoted("\"").raw().doubled()],
    keywords: &[
        "data", "set", "run", "quit", "if", "then", "else", "do", "end", "to", "by", "while", "until", "output",
        "input", "infile", "file", "put", "length", "format", "informat", "label", "keep", "drop", "retain", "where",
        "merge", "array", "select", "when", "otherwise", "libname", "filename", "options", "title", "footnote", "ods",
        "class", "var", "model", "tables", "out", "and", "or", "not", "in", "eq", "ne", "lt", "le", "gt", "ge",
        "delete", "return", "leave", "continue", "stop", "abort", "call", "create", "table", "as", "from", "group",
        "order", "having", "on", "join", "left", "right", "inner", "distinct", "nodupkey", "noprint", "cards",
        "datalines", "first", "last", "update", "modify", "rename", "id", "weight", "freq", "means", "descending",
    ],
    builtins: &[
        "sum", "mean", "min", "max", "n", "nmiss", "substr", "scan", "trim", "strip", "upcase", "lowcase", "cats",
        "catx", "compress", "index", "intck", "intnx", "today", "date", "mdy", "year", "month", "day", "round", "int",
        "abs", "log", "exp", "sqrt", "lag", "missing", "coalesce", "propcase", "tranwrd", "count",
    ],
    constants: &["_null_", "_all_", "_n_", "_error_", "_numeric_", "_character_"],
    function_keywords: &["proc"],
    sigils: &[(b'&', Kind::Variable)],
    operators: "+-*/=<>|^~!:",
    case_insensitive: true,
    features: CALLS,
    hook: Some(sas_hook),
    hook_chars: "%*\"",
    ..Spec::DEFAULT
};

pub static SAS: Definition = Definition::new("sas", "SAS", Engine::Generic(&SAS_SPEC)).extensions(&["sas"]);

// Stata

/// `` `local' ``, `$global` and `${global}` macro references in a Stata string.
fn stata_ref(ctx: &Ctx, i: usize) -> usize {
    match ctx.at(i) {
        b'`' if ctx.at(i + 1) != b'"' => {
            let limit = ctx.line_end_within(i, 128);
            ctx.find_in(i + 1, limit, b"'").map_or(i, |close| close + 1)
        }
        b'$' if ctx.at(i + 1) == b'{' => {
            let limit = ctx.line_end_within(i, 128);
            ctx.find_in(i + 2, limit, b"}").map_or(i, |close| close + 1)
        }
        b'$' if ctx.at(i + 1).is_ascii_alphabetic() || ctx.at(i + 1) == b'_' => scan(ctx, i + 1, is_ident),
        _ => i,
    }
}

/// Local macros `` `name' ``, strings with macro references and `*` comment lines.
fn stata_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek() {
        b'"' => string_with_refs(ctx, start, 1, b"\"", stata_ref),
        b'`' if ctx.peek_at(1) == b'"' => string_with_refs(ctx, start, 2, b"\"'", stata_ref),
        b'`' => {
            let end = stata_ref(ctx, start);
            if end == start {
                return false;
            }
            put(ctx, start, end, Kind::Variable);
        }
        _ => {
            if !ctx.at_line_start(start) {
                return false;
            }
            let end = ctx.line_end(start);
            put(ctx, start, end, Kind::Comment);
            return true;
        }
    }
    lx.mark_value();
    true
}

static STATA_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").raw()],
    keywords: &[
        "program", "define", "end", "if", "else", "in", "foreach", "forvalues", "while", "continue", "local", "global",
        "tempvar", "tempname", "tempfile", "args", "syntax", "gen", "generate", "egen", "replace", "summarize", "sum",
        "tabulate", "tab", "regress", "reg", "logit", "probit", "list", "use", "save", "clear", "set", "display", "di",
        "by", "bysort", "sort", "merge", "append", "keep", "drop", "label", "quietly", "qui", "noisily", "capture",
        "cap", "preserve", "restore", "return", "ereturn", "scalar", "matrix", "import", "export", "insheet",
        "outsheet", "collapse", "reshape", "rename", "describe", "count", "assert", "of", "varlist", "numlist",
        "newlist", "exit", "version", "which", "using", "graph", "twoway", "scatter", "line", "histogram", "estimates",
        "predict", "test", "xtset", "tsset", "duplicates", "encode", "decode", "destring", "tostring", "format",
        "mata", "do", "run", "include",
    ],
    builtins: &[
        "strlen", "substr", "strpos", "lower", "upper", "trim", "real", "string", "round", "floor", "ceil", "abs",
        "log", "exp", "sqrt", "missing", "inlist", "inrange", "cond", "mod", "max", "min", "mean", "sd", "_n", "_N",
        "_b", "_se", "_rc",
    ],
    types: &["byte", "int", "long", "float", "double", "str", "strL"],
    sigils: &[(b'$', Kind::Variable)],
    features: CALLS,
    hook: Some(stata_hook),
    hook_chars: "`*\"",
    ..Spec::DEFAULT
};

pub static STATA: Definition =
    Definition::new("stata", "Stata", Engine::Generic(&STATA_SPEC)).aliases(&["do", "ado"]).extensions(&["do", "ado"]);

// ABAP

/// `*` comment lines, `<field_symbols>` and `##PRAGMAS`.
fn abap_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'*' if start == 0 || ctx.src[start - 1] == b'\n' => {
            let end = ctx.line_end(start);
            put(ctx, start, end, Kind::Comment);
            true
        }
        b'<' if next.is_ascii_alphabetic() || next == b'_' => {
            let end = lx.scan_ident(start + 1);
            if lx.ctx.at(end) != b'>' {
                return false;
            }
            put(lx.ctx, start, end + 1, Kind::Variable);
            lx.mark_value();
            true
        }
        b'#' if next == b'#' => {
            let end = scan(ctx, start + 2, is_ident);
            put(ctx, start, end, Kind::Meta);
            true
        }
        b'-' => {
            // Structure component: `ls_row-field`, `<fs>-field`.
            if start == 0 || matches!(ctx.src[start - 1], b' ' | b'\t' | b'\n') || !next.is_ascii_alphabetic() {
                return false;
            }
            ctx.token(1, Kind::Operator);
            let end = lx.scan_ident(start + 1);
            put(lx.ctx, start + 1, end, Kind::Property);
            lx.mark_value();
            true
        }
        _ => {
            let mut end = lx.scan_ident(start);
            let src = lx.ctx.src;
            let Some(dash) = src[start..end].iter().position(|&c| c == b'-').map(|i| start + i) else {
                return false;
            };
            if src[end - 1] == b'-' {
                // `obj->method`
                end -= 1;
            }
            lx.ctx.pos = end;
            // Dashed keywords (`field-symbols`) stay whole; other words are `structure-component`.
            match lx.word_kind(&src[start..end]) {
                Some(Kind::Keyword) => {
                    let kind = lx.classify(start, end);
                    lx.ctx.emit(start, end, kind);
                }
                _ if dash >= end => {
                    let kind = lx.classify(start, end);
                    lx.ctx.emit(start, end, kind);
                }
                _ => {
                    let kind = lx.classify(start, dash);
                    lx.ctx.emit(start, dash, kind);
                    lx.ctx.emit(dash, dash + 1, Kind::Operator);
                    lx.ctx.emit(dash + 1, end, Kind::Property);
                }
            }
            true
        }
    }
}

static ABAP_SPEC: Spec = Spec {
    line_comments: &["\""],
    strings: &[
        Str::quoted("'").raw().doubled(),
        Str::quoted("`").raw().doubled(),
        Str::quoted("|").interp(Interp::Brace),
    ],
    keywords: &[
        "abstract", "and", "append", "assign", "at", "begin", "between", "by", "call", "case", "catch", "changing",
        "check", "class", "clear", "collect", "concatenate", "condense", "constants", "continue", "create", "data",
        "default", "definition", "delete", "describe", "do", "else", "elseif", "end", "endcase", "endclass", "enddo",
        "endform", "endfunction", "endif", "endloop", "endmethod", "endselect", "endtry", "endwhile", "eq", "event",
        "exceptions", "exit", "exporting", "field-symbols", "final", "for", "form", "from", "function", "ge", "gt",
        "if", "implementation", "importing", "in", "include", "initial", "inner", "insert", "interface", "into", "is",
        "join", "key", "le", "left", "like", "loop", "lt", "message", "method", "methods", "modify", "move", "ne",
        "new", "not", "of", "or", "order", "parameters", "perform", "private", "protected", "public", "raise",
        "raising", "read", "ref", "report", "return", "returning", "section", "select", "select-options", "single",
        "sort", "split", "start-of-selection", "end-of-selection", "table", "tables", "field-symbol", "length",
        "decimals", "to", "try", "type", "types", "up", "update", "using", "value", "when", "where", "while", "with",
        "write", "cast", "conv", "corresponding", "line", "assigning", "binary", "search", "standard", "sorted",
        "hashed", "unique", "inheriting", "initialization", "redefinition", "static", "class-methods", "class-data",
        "occurs", "range", "reduce", "let", "next",
    ],
    types: &[
        "string", "xstring", "abap_bool", "int8", "decfloat16", "decfloat34", "timestampl", "timestamp", "sy", "syst",
        "any", "data", "object",
    ],
    builtins: &[
        "lines", "strlen", "condense", "to_upper", "to_lower", "boolc", "xsdbool", "line_exists",
        "cl_abap_char_utilities",
    ],
    constants: &["abap_true", "abap_false", "space", "abap_undefined"],
    variables: &["me", "super"],
    function_keywords: &["form", "perform", "method", "methods", "class-methods"],
    type_keywords: &["type", "of", "class", "interface", "new", "ref"],
    sigils: &[(b'@', Kind::Variable)],
    ident_chars: "-",
    case_insensitive: true,
    features: CALLS | MEMBERS,
    hook: Some(abap_hook),
    hook_chars: concat!(letters!(), "_*<#-"),
    ..Spec::DEFAULT
};

pub static ABAP: Definition = Definition::new("abap", "ABAP", Engine::Generic(&ABAP_SPEC)).extensions(&["abap"]);

// APL

const APL_FUNCTIONS: &[char] = &[
    '+', '-', '×', '÷', '⌈', '⌊', '*', '⍟', '|', '!', '○', '~', '?', '∊', '⍷', '⍴', ',', '⍪', '⌽', '⊖', '⍉', '↑', '↓',
    '⊂', '⊃', '⊆', '⌷', '⍋', '⍒', '⍳', '⍸', '∪', '∩', '⊣', '⊢', '≡', '≢', '<', '≤', '=', '≥', '>', '≠', '∨', '∧', '⍲',
    '⍱', '⊥', '⊤', '⌹', '⍕', '⍎', '^',
];
const APL_OPERATORS: &[char] = &['/', '\\', '⌿', '⍀', '¨', '⍨', '⍣', '.', '∘', '⍤', '⍥', '@', '⌸', '⌺', '⍠', '&'];

/// Decodes the character at `pos` and returns it with its length in bytes.
fn char_at(ctx: &Ctx, pos: usize) -> (char, usize) {
    let b = ctx.at(pos);
    if b < 0x80 {
        return (b as char, 1);
    }
    let len: usize = match b {
        0xF0.. => 4,
        0xE0.. => 3,
        0xC0.. => 2,
        _ => 1,
    };
    let len = len.min(ctx.len() - pos).max(1);
    let mut c = u32::from(b & (0x7F >> len));
    for k in 1..len {
        c = (c << 6) | u32::from(ctx.at(pos + k) & 0x3F);
    }
    (char::from_u32(c).unwrap_or('\u{FFFD}'), len)
}

fn apl(ctx: &mut Ctx) {
    while !ctx.done() {
        let start = ctx.pos;
        let (c, len) = char_at(ctx, start);
        let digit_after = |ctx: &Ctx, i: usize| ctx.at(i).is_ascii_digit();
        match c {
            '⍝' => {
                ctx.skip_line();
                ctx.emit_from(start, Kind::Comment);
            }
            '\'' => {
                let mut i = start + 1;
                loop {
                    match ctx.at(i) {
                        b'\'' if ctx.at(i + 1) == b'\'' => i += 2,
                        b'\'' => {
                            i += 1;
                            break;
                        }
                        b'\n' => break,
                        0 if i >= ctx.len() => break,
                        _ => i += 1,
                    }
                }
                put(ctx, start, i, Kind::String);
            }
            '0'..='9' | '¯' | '.' if c != '.' || digit_after(ctx, start + 1) => {
                let mut i = start;
                loop {
                    let (d, l) = char_at(ctx, i);
                    if i < ctx.len() && (d.is_ascii_alphanumeric() || d == '.' || d == '¯' || d == '_') {
                        i += l;
                    } else {
                        break;
                    }
                }
                put(ctx, start, i, Kind::Number);
            }
            '⍺' | '⍵' => {
                let end = if char_at(ctx, start + len).0 == c { start + 2 * len } else { start + len };
                put(ctx, start, end, Kind::Variable);
            }
            '∇' | '→' => put(ctx, start, start + len, Kind::Keyword),
            '⍬' => put(ctx, start, start + len, Kind::Constant),
            '⎕' | '⍞' | ':' | '#' => {
                let end = scan(ctx, start + len, |b| b.is_ascii_alphanumeric() || b == b'_');
                let kind = match c {
                    ':' if end > start + 1 => Kind::Keyword,
                    ':' => Kind::Punctuation,
                    '#' => Kind::Variable,
                    _ => Kind::Builtin,
                };
                put(ctx, start, end, kind);
            }
            '←' => put(ctx, start, start + len, Kind::Operator),
            '⋄' | '(' | ')' | '[' | ']' | '{' | '}' | ';' => put(ctx, start, start + len, Kind::Punctuation),
            _ if c.is_alphabetic() || c == '_' || c == '∆' || c == '⍙' => {
                let mut i = start;
                loop {
                    let (d, l) = char_at(ctx, i);
                    if i < ctx.len() && (d.is_alphanumeric() || d == '_' || d == '∆' || d == '⍙') {
                        i += l;
                    } else {
                        break;
                    }
                }
                ctx.pos = i;
            }
            _ if APL_FUNCTIONS.contains(&c) => put(ctx, start, start + len, Kind::Operator),
            _ if APL_OPERATORS.contains(&c) => put(ctx, start, start + len, Kind::Keyword),
            _ => ctx.pos = start + len,
        }
    }
}

pub static APL: Definition = Definition::new("apl", "APL", Engine::Custom(apl))
    .aliases(&["dyalog"])
    .extensions(&["apl", "dyalog", "aplf", "apln", "aplc"]);

// Smalltalk

static SMALLTALK_SYMBOL: Str = Str::quoted("'").raw().doubled().kind(Kind::Constant);

/// `$c` characters, `#symbols`, `:blockArgs` and `keyword:` message parts.
fn smalltalk_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let next = lx.ctx.peek_at(1);
    match lx.ctx.peek() {
        b'$' => {
            if start + 1 >= lx.ctx.len() {
                return false;
            }
            lx.ctx.pos += 1;
            lx.ctx.bump_char();
            lx.ctx.emit_from(start, Kind::String);
        }
        b'#' => {
            if next.is_ascii_alphabetic() || next == b'_' {
                let end = scan(lx.ctx, start + 1, |c| is_ident(c) || c == b':');
                put(lx.ctx, start, end, Kind::Constant);
            } else if next == b'\'' {
                lx.ctx.pos += 1;
                lx.string(start, &SMALLTALK_SYMBOL, false, Interp::None);
            } else if b"+-*/<>=~@%|&?,\\".contains(&next) {
                let end = scan(lx.ctx, start + 1, |c| b"+-*/<>=~@%|&?,\\".contains(&c));
                put(lx.ctx, start, end, Kind::Constant);
            } else {
                return false;
            }
        }
        b':' => {
            if !(next.is_ascii_alphabetic() || next == b'_') {
                return false;
            }
            let end = lx.scan_ident(start + 1);
            put(lx.ctx, start, end, Kind::Variable);
        }
        _ => {
            let end = lx.scan_ident(start);
            if lx.ctx.at(end) != b':' || lx.ctx.at(end + 1) == b'=' {
                return false;
            }
            put(lx.ctx, start, end + 1, Kind::Function);
            lx.mark_operator();
            return true;
        }
    }
    lx.mark_value();
    true
}

static SMALLTALK_SPEC: Spec = Spec {
    block_comments: &[("\"", "\"")],
    strings: &[Str::quoted("'").raw().doubled()],
    keywords: &["subclass", "instanceVariableNames", "classVariableNames", "package", "category"],
    constants: &["true", "false", "nil"],
    variables: &["self", "super", "thisContext"],
    features: CAPITALIZED_TYPES,
    hook: Some(smalltalk_hook),
    hook_chars: concat!(letters!(), "$#:"),
    ..Spec::DEFAULT
};

pub static SMALLTALK: Definition = Definition::new("smalltalk", "Smalltalk", Engine::Generic(&SMALLTALK_SPEC))
    .aliases(&["st", "squeak", "pharo"])
    .extensions(&["st"]);

// TLA+

/// `\in` style operators, `Name ==` definitions and PlusCal blocks inside comments.
fn tla_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    match ctx.peek() {
        b'\\' => {
            if !next.is_ascii_alphabetic() {
                return false;
            }
            let end = scan(ctx, start + 1, |c| c.is_ascii_alphabetic());
            put(ctx, start, end, Kind::Operator);
            lx.mark_operator();
            true
        }
        b'(' => {
            if next != b'*' {
                return false;
            }
            let mut i = start + 2;
            while matches!(ctx.at(i), b' ' | b'\t' | b'\n' | b'\r') {
                i += 1;
            }
            if !(ctx.starts_with_at(i, b"--algorithm") || ctx.starts_with_at(i, b"--fair")) {
                return false;
            }
            ctx.token(2, Kind::Comment);
            true
        }
        b'*' => {
            if next != b')' {
                return false;
            }
            ctx.token(2, Kind::Comment);
            true
        }
        _ => {
            if !ctx.at_line_start(start) {
                return false;
            }
            let end = lx.scan_ident(start);
            let ctx = &mut *lx.ctx;
            let mut i = ctx.skip_spaces_from(end);
            if ctx.at(i) == b'(' {
                let line_end = ctx.line_end(i);
                match ctx.src[i..line_end].iter().position(|&c| c == b')') {
                    Some(p) => i = ctx.skip_spaces_from(i + p + 1),
                    None => return false,
                }
            }
            if !ctx.starts_with_at(i, b"==") || ctx.at(i + 2) == b'=' {
                return false;
            }
            put(ctx, start, end, Kind::Function);
            true
        }
    }
}

static TLA_SPEC: Spec = Spec {
    line_comments: &["\\*"],
    block_comments: &[("(*", "*)")],
    nested_comments: true,
    strings: &[Str::quoted("\"")],
    keywords: &[
        "MODULE", "EXTENDS", "CONSTANT", "CONSTANTS", "VARIABLE", "VARIABLES", "ASSUME", "ASSUMPTION", "AXIOM",
        "THEOREM", "LEMMA", "PROPOSITION", "COROLLARY", "PROOF", "BY", "DEF", "DEFS", "QED", "OBVIOUS", "OMITTED",
        "HAVE", "TAKE", "WITNESS", "PICK", "SUFFICES", "USE", "HIDE", "LET", "IN", "IF", "THEN", "ELSE", "CASE",
        "OTHER", "CHOOSE", "EXCEPT", "DOMAIN", "SUBSET", "UNION", "UNCHANGED", "ENABLED", "INSTANCE", "WITH", "LOCAL",
        "RECURSIVE", "LAMBDA", "SF_", "WF_", "algorithm", "fair", "variables", "variable", "define", "macro",
        "procedure", "process", "begin", "end", "while", "do", "if", "then", "else", "elsif", "either", "or", "with",
        "await", "when", "skip", "goto", "call", "return", "print", "assert",
    ],
    types: &["BOOLEAN", "STRING", "Nat", "Int", "Real", "Seq"],
    builtins: &[
        "Len", "Append", "Head", "Tail", "SubSeq", "SelectSeq", "Cardinality", "IsFiniteSet", "Print", "Assert",
    ],
    constants: &["TRUE", "FALSE", "self"],
    type_keywords: &["MODULE"],
    operators: "+-*/%=&|^!<>~?:#\\'",
    features: UPPER_CONSTANTS,
    hook: Some(tla_hook),
    hook_chars: concat!(letters!(), "\\(*"),
    ..Spec::DEFAULT
};

pub static TLA: Definition = Definition::new("tla", "TLA+", Engine::Generic(&TLA_SPEC))
    .aliases(&["tla+", "tlaplus", "pluscal"])
    .extensions(&["tla"]);

// Alloy

static ALLOY_SPEC: Spec = Spec {
    line_comments: &["//", "--"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    keywords: &[
        "abstract", "all", "and", "as", "assert", "but", "check", "disj", "else", "exactly", "expect", "extends",
        "fact", "for", "fun", "iff", "implies", "in", "let", "lone", "module", "no", "not", "one", "open", "or",
        "pred", "run", "set", "sig", "some", "sum", "var", "enum", "private", "always", "eventually", "after",
        "before", "historically", "once", "until", "since", "steps",
    ],
    types: &["Int", "String", "seq"],
    constants: &["none", "univ", "iden"],
    variables: &["this"],
    function_keywords: &["fun", "pred", "assert", "fact"],
    type_keywords: &["sig", "extends", "enum"],
    features: CAPITALIZED_TYPES | MEMBERS,
    ..Spec::DEFAULT
};

pub static ALLOY: Definition = Definition::new("alloy", "Alloy", Engine::Generic(&ALLOY_SPEC)).extensions(&["als"]);

// OpenSCAD

/// `include <file.scad>` and `use <file.scad>` paths.
fn openscad_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    if !matches!(word_before(ctx, start), b"include" | b"use") {
        return false;
    }
    let line_end = ctx.line_end(start);
    let end = ctx.src[start..line_end].iter().position(|&c| c == b'>').map_or(line_end, |i| start + i + 1);
    put(ctx, start, end, Kind::String);
    true
}

static OPENSCAD_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    keywords: &[
        "module", "function", "include", "use", "if", "else", "for", "let", "each", "intersection_for", "assert",
        "echo",
    ],
    builtins: &[
        "cube", "sphere", "cylinder", "polyhedron", "square", "circle", "polygon", "text", "translate", "rotate",
        "scale", "resize", "mirror", "multmatrix", "color", "offset", "hull", "minkowski", "union", "difference",
        "intersection", "linear_extrude", "rotate_extrude", "import", "surface", "projection", "render", "children",
        "len", "concat", "str", "abs", "sin", "cos", "tan", "sqrt", "pow", "min", "max", "floor", "ceil", "round",
        "norm", "cross", "search", "lookup", "rands", "is_undef", "is_num", "is_list",
    ],
    constants: &["true", "false", "undef", "PI"],
    function_keywords: &["module", "function"],
    sigils: &[(b'$', Kind::Variable)],
    features: CALLS | UPPER_CONSTANTS,
    hook: Some(openscad_hook),
    hook_chars: "<",
    ..Spec::DEFAULT
};

pub static OPENSCAD: Definition =
    Definition::new("openscad", "OpenSCAD", Engine::Generic(&OPENSCAD_SPEC)).aliases(&["scad"]).extensions(&["scad"]);

// G-code

fn gcode_number(ctx: &Ctx, from: usize) -> usize {
    let mut i = from;
    if matches!(ctx.at(i), b'+' | b'-') {
        i += 1;
    }
    scan(ctx, i, |c| c.is_ascii_digit() || c == b'.')
}

fn gcode(ctx: &mut Ctx) {
    while !ctx.done() {
        let start = ctx.pos;
        let b = ctx.peek();
        match b {
            b';' => {
                ctx.skip_line();
                ctx.emit_from(start, Kind::Comment);
            }
            b'(' => {
                // One scan to `)` or the line end keeps long lines linear.
                let end = match ctx.src[start..].iter().position(|&c| c == b')' || c == b'\n') {
                    Some(i) if ctx.src[start + i] == b')' => start + i + 1,
                    Some(i) => start + i,
                    None => ctx.len(),
                };
                put(ctx, start, end, Kind::Comment);
            }
            b'%' => ctx.token(1, Kind::Meta),
            b'#' => {
                let end = scan(ctx, start + 1, |c| is_ident(c) || c == b'<' || c == b'>');
                put(ctx, start, end.max(start + 1), Kind::Variable);
            }
            b'"' => {
                let end = match ctx.src[start + 1..].iter().position(|&c| c == b'"' || c == b'\n') {
                    Some(i) if ctx.src[start + 1 + i] == b'"' => start + i + 2,
                    Some(i) => start + 1 + i,
                    None => ctx.len(),
                };
                put(ctx, start, end, Kind::String);
            }
            b'0'..=b'9' | b'.' => {
                let end = gcode_number(ctx, start).max(start + 1);
                put(ctx, start, end, Kind::Number);
            }
            _ if b.is_ascii_alphabetic() => {
                let end = scan(ctx, start, |c| c.is_ascii_alphabetic() || c == b'_');
                if end - start > 1 {
                    let kind = if start > 0 && ctx.src[start - 1] == b'=' {
                        Kind::Text
                    } else if ctx.at(end) == b'=' {
                        Kind::Property
                    } else if ctx.at_line_start(start) {
                        Kind::Function
                    } else {
                        Kind::Keyword
                    };
                    let end = if kind == Kind::Function { scan(ctx, end, is_ident) } else { end };
                    put(ctx, start, end, kind);
                    continue;
                }
                let num_end = gcode_number(ctx, end);
                match b.to_ascii_uppercase() {
                    b'G' | b'M' | b'T' => {
                        put(ctx, start, num_end, Kind::Keyword);
                        if b.eq_ignore_ascii_case(&b'M') && matches!(&ctx.src[end..num_end], b"117" | b"118") {
                            let line_end = ctx.line_end(num_end);
                            let msg_end = ctx.src[num_end..line_end]
                                .iter()
                                .position(|&c| c == b';')
                                .map_or(line_end, |i| num_end + i);
                            let msg = ctx.skip_spaces_from(num_end);
                            put(ctx, msg.min(msg_end), msg_end, Kind::String);
                        }
                    }
                    b'N' => put(ctx, start, num_end, Kind::Meta),
                    b'O' => put(ctx, start, num_end, Kind::Function),
                    _ => {
                        put(ctx, start, end, Kind::Property);
                        put(ctx, end, num_end, Kind::Number);
                    }
                }
            }
            b'+' | b'-' | b'*' | b'/' | b'=' | b'<' | b'>' => ctx.token(1, Kind::Operator),
            b'[' | b']' => ctx.token(1, Kind::Punctuation),
            _ => ctx.bump_char(),
        }
    }
}

pub static GCODE: Definition = Definition::new("gcode", "G-code", Engine::Custom(gcode))
    .aliases(&["g-code", "nc"])
    .extensions(&["gcode", "nc", "ngc", "gco"]);

// QML

static QML_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'"), Str::quoted("`").multiline().interp(Interp::DollarBrace)],
    keywords: &[
        "import", "as", "property", "readonly", "required", "default", "alias", "signal", "pragma", "component",
        "enum", "inline", "on", "if", "else", "for", "while", "do", "return", "break", "continue", "switch", "case",
        "var", "let", "const", "new", "delete", "typeof", "instanceof", "in", "of", "try", "catch", "finally", "throw",
    ],
    types: &["int", "real", "double", "bool", "string", "url", "list", "variant", "vector2d", "vector3d"],
    builtins: &["console", "Qt", "Math", "JSON", "parent", "Component"],
    constants: &["true", "false", "null", "undefined"],
    variables: &["this"],
    function_keywords: &["function", "signal"],
    features: CALLS | CAPITALIZED_TYPES | MEMBERS | IDENT_KEYS,
    ..Spec::DEFAULT
};

pub static QML: Definition = Definition::new("qml", "QML", Engine::Generic(&QML_SPEC)).extensions(&["qml"]);

// Kusto

static KQL_VERBATIM: [Str; 2] = [Str::quoted("\"").raw(), Str::quoted("'").raw()];

/// `@"verbatim"` strings and dashed operators such as `project-away` and `mv-expand`.
fn kusto_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let next = lx.ctx.peek_at(1);
    if lx.ctx.peek() == b'@' {
        let rule = match next {
            b'"' => &KQL_VERBATIM[0],
            b'\'' => &KQL_VERBATIM[1],
            _ => return false,
        };
        lx.ctx.pos += 1;
        lx.string(start, rule, true, Interp::None);
        return true;
    }
    let ctx = &mut *lx.ctx;
    let end = scan(ctx, start, |c| c.is_ascii_alphabetic() || c == b'-');
    let word = &ctx.src[start..end];
    if !word.contains(&b'-')
        || !any_eq(
            word,
            &[
                "project-away", "project-rename", "project-reorder", "project-keep", "mv-expand", "mv-apply",
                "make-series", "top-nested", "top-hitters", "parse-where", "parse-kv",
            ],
        )
    {
        return false;
    }
    put(ctx, start, end, Kind::Keyword);
    lx.mark_operator();
    true
}

static KUSTO_SPEC: Spec = Spec {
    line_comments: &["//"],
    strings: &[Str::quoted("\""), Str::quoted("'"), Str::quoted("```").multiline().raw()],
    keywords: &[
        "where", "project", "extend", "summarize", "by", "take", "limit", "top", "sort", "order", "asc", "desc",
        "join", "kind", "on", "union", "let", "render", "distinct", "evaluate", "as", "datatable", "print", "range",
        "from", "to", "step", "in", "has", "contains", "startswith", "endswith", "and", "or", "not", "between",
        "materialize", "invoke", "lookup", "getschema", "search", "find", "parse", "with", "inner", "leftouter",
        "rightouter", "fullouter", "leftanti", "rightanti", "leftsemi", "rightsemi", "innerunique", "matches", "regex",
        "nulls", "first", "last", "sample", "serialize", "facet", "fork", "as", "set", "alias", "declare", "pattern",
        "restrict", "access", "hasprefix", "hassuffix", "has_any", "has_all", "in~",
    ],
    types: &["string", "int", "long", "real", "bool", "datetime", "timespan", "dynamic", "guid", "decimal"],
    builtins: &[
        "ago", "now", "bin", "count", "countif", "dcount", "sum", "sumif", "avg", "min", "max", "make_list",
        "make_set", "arg_max", "arg_min", "percentile", "strcat", "tostring", "toint", "tolong", "todatetime",
        "isempty", "isnotempty", "iff", "case", "extract", "split", "parse_json", "format_datetime", "startofday",
        "datetime_diff", "round", "strlen", "tolower", "toupper", "array_length", "pack", "bag_pack",
    ],
    constants: &["true", "false", "null"],
    features: CALLS,
    hook: Some(kusto_hook),
    hook_chars: "@pmt",
    ..Spec::DEFAULT
};

pub static KUSTO: Definition =
    Definition::new("kusto", "Kusto", Engine::Generic(&KUSTO_SPEC)).aliases(&["kql"]).extensions(&["kql", "csl"]);

// DAX

/// `[Column]` references, `'Quoted Table'` names and `Table[Column]` prefixes.
fn dax_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    // Scans to the closing quote or the end of the line, whichever comes first.
    let close = |ctx: &Ctx, q: u8| {
        let mut i = start + 1;
        while i < ctx.len() && ctx.src[i] != b'\n' {
            if ctx.src[i] == q {
                if ctx.at(i + 1) == q {
                    i += 2;
                    continue;
                }
                return i + 1;
            }
            i += 1;
        }
        i
    };
    match ctx.peek() {
        b'[' => {
            let end = close(ctx, b']');
            put(ctx, start, end, Kind::Property);
        }
        b'\'' => {
            let end = close(ctx, b'\'');
            put(ctx, start, end, Kind::Type);
        }
        _ => {
            let end = lx.scan_ident(start);
            if lx.ctx.at(end) != b'[' {
                return false;
            }
            put(lx.ctx, start, end, Kind::Type);
        }
    }
    lx.mark_value();
    true
}

static DAX_SPEC: Spec = Spec {
    line_comments: &["//", "--"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").raw().doubled()],
    keywords: &[
        "var", "return", "define", "evaluate", "order", "by", "asc", "desc", "measure", "column", "table", "in", "not",
        "and", "or", "start", "at", "function", "calculationgroup",
    ],
    builtins: &[
        "calculate", "calculatetable", "filter", "all", "allexcept", "allselected", "sum", "sumx", "average",
        "averagex", "count", "countrows", "countx", "distinctcount", "min", "max", "minx", "maxx", "divide", "if",
        "switch", "related", "relatedtable", "values", "distinct", "selectedvalue", "hasonevalue", "isblank", "blank",
        "true", "false", "format", "datesytd", "totalytd", "sameperiodlastyear", "dateadd", "keepfilters",
        "removefilters", "userelationship", "summarize", "summarizecolumns", "addcolumns", "selectcolumns", "topn",
        "rankx", "earlier", "lookupvalue", "concatenatex", "year", "month", "today", "now", "coalesce",
    ],
    case_insensitive: true,
    features: CALLS,
    hook: Some(dax_hook),
    hook_chars: concat!(letters!(), "_['"),
    ..Spec::DEFAULT
};

pub static DAX: Definition =
    Definition::new("dax", "DAX", Engine::Generic(&DAX_SPEC)).aliases(&["msdax"]).extensions(&["dax", "msdax"]);

// MQL4 and MQL5

static MQL_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "break", "case", "const", "continue", "default", "delete", "do", "else", "enum", "extern", "for", "if",
        "input", "sinput", "new", "operator", "private", "protected", "public", "return", "sizeof", "static", "switch",
        "template", "typename", "virtual", "while", "this", "override", "final", "export", "import",
    ],
    types: &[
        "bool", "char", "uchar", "short", "ushort", "int", "uint", "long", "ulong", "float", "double", "string",
        "color", "datetime", "void", "MqlTick", "MqlRates", "MqlTradeRequest", "MqlTradeResult",
    ],
    builtins: &[
        "Print", "Alert", "Comment", "Symbol", "Period", "Bid", "Ask", "Point", "Digits", "_Symbol", "_Point",
        "_Digits", "_Period", "OrderSend", "OrderSelect", "OrderClose", "PositionSelect", "TimeCurrent",
        "NormalizeDouble", "ArraySize", "ArrayResize", "iMA", "iRSI", "iClose", "SymbolInfoDouble",
        "AccountInfoDouble",
    ],
    constants: &["true", "false", "NULL", "EMPTY_VALUE", "clrNONE"],
    type_keywords: &["class", "struct", "enum", "interface"],
    features: CALLS | UPPER_CONSTANTS | MEMBERS | PREPROCESSOR,
    ..Spec::DEFAULT
};

pub static MQL: Definition = Definition::new("mql", "MQL", Engine::Generic(&MQL_SPEC))
    .aliases(&["mql4", "mql5", "mq4", "mq5"])
    .extensions(&["mq4", "mq5", "mqh"]);
