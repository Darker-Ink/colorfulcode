//! A table driven lexer that covers the large family of "code like" languages.
//!
//! A language is described by a static [`Spec`]: its comment markers, string
//! rules, word lists and a few feature flags. Anything the tables cannot
//! express is handled by an optional [`Hook`] that runs before the generic
//! rules. No regular expressions are involved; everything is a single forward
//! pass over the bytes.

// This module is a toolkit for the language files; not every helper is used by every build.
#![allow(dead_code)]

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{OnceLock, RwLock};

use super::{is_ident, Ctx};
use crate::Kind;

/// Feature flags for [`Spec::features`].
pub mod features {
    /// `name(` is highlighted as a function.
    pub const CALLS: u32 = 1 << 0;
    /// `Capitalized` identifiers are highlighted as types.
    pub const CAPITALIZED_TYPES: u32 = 1 << 1;
    /// `ALL_CAPS` identifiers are highlighted as constants.
    pub const UPPER_CONSTANTS: u32 = 1 << 2;
    /// Identifiers after `.` are highlighted as properties (or functions when called).
    pub const MEMBERS: u32 = 1 << 3;
    /// JavaScript style `/regex/flags` literals.
    pub const REGEX: u32 = 1 << 4;
    /// JSX elements in expression position.
    pub const JSX_ELEMENTS: u32 = 1 << 5;
    /// `#directive` at the start of a line is a preprocessor directive.
    pub const PREPROCESSOR: u32 = 1 << 6;
    /// A string directly followed by `:` is a property name (JSON keys).
    pub const STRING_KEYS: u32 = 1 << 7;
    /// An identifier directly followed by a single `:` is a property name.
    pub const IDENT_KEYS: u32 = 1 << 8;
    /// The first word after `(` is a function call (Lisp family).
    pub const LISP_CALLS: u32 = 1 << 9;
    /// `@name` is an annotation or decorator.
    pub const ANNOTATIONS: u32 = 1 << 10;
    /// `name!` is a macro call.
    pub const BANG_MACROS: u32 = 1 << 11;
    /// The first word of a line is a command (shell like languages).
    pub const COMMANDS: u32 = 1 << 12;
    /// Identifiers followed by `=` (not `==`) at the start of a line are properties.
    pub const LINE_KEYS: u32 = 1 << 13;
    /// `-flag` and `--flag` words are highlighted as attributes.
    pub const FLAGS: u32 = 1 << 14;
    /// A number directly followed by letters (`10px`, `2em`) keeps the unit in the number.
    pub const UNITS: u32 = 1 << 15;
    /// `name = (args) =>` and `name = function` mark `name` as a function.
    pub const ARROW_FUNCTIONS: u32 = 1 << 16;
    /// `Capitalized(` is a constructor or tuple struct and highlighted as a type.
    pub const CONSTRUCTORS: u32 = 1 << 17;
    /// Like CALLS, but the `(` must follow the name directly (`f(x)`, not `f (x)`).
    pub const TIGHT_CALLS: u32 = 1 << 18;
    /// `name = value` anywhere marks `name` as a property (not `==` or `=>`).
    pub const ASSIGN_KEYS: u32 = 1 << 19;
    /// `-` joins identifiers when a letter follows (`read-file`).
    pub const KEBAB: u32 = 1 << 20;
}

/// String interpolation styles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interp {
    None,
    /// `${expr}`
    DollarBrace,
    /// `#{expr}`
    HashBrace,
    /// `{expr}` with `{{` as an escaped brace.
    Brace,
    /// `$name` and `${expr}`
    Dollar,
    /// `$name`, `${expr}` and `$(expr)`, as in shells and Julia.
    DollarParen,
    /// `\(expr)`
    BackslashParen,
}

/// A string (or string like) literal rule.
#[derive(Clone, Copy, Debug)]
pub struct Str {
    pub open: &'static str,
    pub close: &'static str,
    /// Backslash escapes are recognised.
    pub escape: bool,
    /// A doubled closing delimiter is an escaped delimiter (`'it''s'`).
    pub doubled: bool,
    /// The literal may span lines.
    pub multiline: bool,
    pub interp: Interp,
    pub kind: Kind,
}

impl Str {
    pub const fn new(open: &'static str, close: &'static str) -> Str {
        Str { open, close, escape: true, doubled: false, multiline: false, interp: Interp::None, kind: Kind::String }
    }

    /// `"..."` with backslash escapes.
    pub const fn quoted(q: &'static str) -> Str {
        Str::new(q, q)
    }

    pub const fn raw(self) -> Str {
        Str { escape: false, ..self }
    }

    pub const fn doubled(self) -> Str {
        Str { doubled: true, ..self }
    }

    pub const fn multiline(self) -> Str {
        Str { multiline: true, ..self }
    }

    pub const fn interp(self, interp: Interp) -> Str {
        Str { interp, ..self }
    }

    pub const fn kind(self, kind: Kind) -> Str {
        Str { kind, ..self }
    }
}

/// A string prefix letter such as Python's `r` or `f`.
#[derive(Clone, Copy, Debug)]
pub struct Prefix {
    pub ch: u8,
    pub raw: bool,
    pub interp: Interp,
}

impl Prefix {
    pub const fn plain(ch: u8) -> Prefix {
        Prefix { ch, raw: false, interp: Interp::None }
    }
    pub const fn raw(ch: u8) -> Prefix {
        Prefix { ch, raw: true, interp: Interp::None }
    }
    pub const fn interp(ch: u8, interp: Interp) -> Prefix {
        Prefix { ch, raw: false, interp }
    }
}

/// Custom rule run before the generic ones. Returns `true` when it consumed input.
pub type Hook = fn(&mut Lexer) -> bool;

/// Description of a table driven language.
#[derive(Clone, Copy)]
pub struct Spec {
    pub line_comments: &'static [&'static str],
    pub block_comments: &'static [(&'static str, &'static str)],
    pub nested_comments: bool,
    pub strings: &'static [Str],
    pub string_prefixes: &'static [Prefix],
    pub keywords: &'static [&'static str],
    pub types: &'static [&'static str],
    pub builtins: &'static [&'static str],
    pub constants: &'static [&'static str],
    /// Special variables such as `self`.
    pub variables: &'static [&'static str],
    /// Keywords whose following identifier names a function (`fn`, `def`).
    pub function_keywords: &'static [&'static str],
    /// Keywords whose following identifier names a type (`class`, `struct`).
    pub type_keywords: &'static [&'static str],
    /// Extra bytes allowed inside identifiers (for example `-` or `.`).
    pub ident_chars: &'static str,
    /// Extra bytes that may also start an identifier (for example `$`).
    pub ident_start: &'static str,
    /// Extra bytes allowed only as the last character of an identifier (`?` and `!` in Ruby).
    pub ident_suffix: &'static str,
    /// Prefix bytes that turn the following identifier into a token of the given kind.
    pub sigils: &'static [(u8, Kind)],
    pub operators: &'static str,
    pub case_insensitive: bool,
    pub features: u32,
    pub hook: Option<Hook>,
    /// Bytes that trigger the hook. Empty means every byte.
    pub hook_chars: &'static str,
}

impl Spec {
    pub const DEFAULT: Spec = Spec {
        line_comments: &[],
        block_comments: &[],
        nested_comments: false,
        strings: &[],
        string_prefixes: &[],
        keywords: &[],
        types: &[],
        builtins: &[],
        constants: &[],
        function_keywords: &[],
        type_keywords: &[],
        ident_chars: "",
        ident_start: "",
        ident_suffix: "",
        variables: &[],
        sigils: &[],
        operators: "+-*/%=&|^!<>~?:",
        case_insensitive: false,
        features: 0,
        hook: None,
        hook_chars: "",
    };

    #[inline(always)]
    pub fn has(&self, feature: u32) -> bool {
        self.features & feature != 0
    }

    /// Lookup tables for this spec, built once and cached by address.
    pub(crate) fn compiled(&'static self) -> &'static Compiled {
        type Cache = RwLock<HashMap<usize, &'static Compiled, BuildHasherDefault<FxHasher>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let key = self as *const Spec as usize;
        if let Some(c) = cache.read().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return c;
        }
        let compiled: &'static Compiled = Box::leak(Box::new(Compiled::new(self)));
        cache.write().unwrap_or_else(|e| e.into_inner()).entry(key).or_insert(compiled)
    }
}

/// What follows a word.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Next {
    None,
    Function,
    Type,
}

#[derive(Clone, Copy)]
struct Word {
    kind: Kind,
    next: Next,
}

/// FxHash, a tiny and fast hash for short keys.
#[derive(Default)]
struct FxHasher(u64);

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        for chunk in bytes.chunks(8) {
            let mut v = 0u64;
            for (i, &b) in chunk.iter().enumerate() {
                v |= (b as u64) << (i * 8);
            }
            self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(K);
        }
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

type Words = HashMap<&'static [u8], Word, BuildHasherDefault<FxHasher>>;

const SET_IDENT: u16 = 1;
const SET_SUFFIX: u16 = 2;
const SET_HOOK: u16 = 4;
const SET_COMMENT: u16 = 8;
const SET_STRING: u16 = 16;
const SET_OPERATOR: u16 = 32;
const SET_SIGIL: u16 = 64;
const SET_PREFIX: u16 = 128;
const SET_START: u16 = 256;

/// Lookup tables derived from a [`Spec`] on first use.
pub(crate) struct Compiled {
    words: Words,
    max_word: usize,
    /// Bit `n` is set when some word has length `n`; cheap rejection before hashing.
    word_lengths: u64,
    /// Bytes that start some word (both cases for case insensitive specs).
    word_starts: [bool; 256],
    sets: [u16; 256],
    sigils: [Option<Kind>; 128],
}

impl Compiled {
    fn new(spec: &Spec) -> Compiled {
        let mut words = Words::default();
        let mut max_word = 0;
        let mut word_lengths = 0u64;
        let mut word_starts = [false; 256];
        let mut add = |list: &'static [&'static str], kind: Kind, next: Next| {
            for w in list {
                max_word = max_word.max(w.len());
                word_lengths |= 1u64.checked_shl(w.len() as u32).unwrap_or(0);
                if let Some(&b) = w.as_bytes().first() {
                    word_starts[b as usize] = true;
                    word_starts[b.to_ascii_lowercase() as usize] = true;
                    word_starts[b.to_ascii_uppercase() as usize] = true;
                }
                words.insert(w.as_bytes(), Word { kind, next });
            }
        };
        add(spec.constants, Kind::Constant, Next::None);
        add(spec.variables, Kind::Variable, Next::None);
        add(spec.builtins, Kind::Builtin, Next::None);
        add(spec.types, Kind::Type, Next::None);
        add(spec.keywords, Kind::Keyword, Next::None);
        add(spec.function_keywords, Kind::Keyword, Next::Function);
        add(spec.type_keywords, Kind::Keyword, Next::Type);

        let mut sets = [0u16; 256];
        for (b, set) in sets.iter_mut().enumerate() {
            if is_ident(b as u8) {
                *set |= SET_IDENT;
            }
            if is_ident(b as u8) && !(b as u8).is_ascii_digit() {
                *set |= SET_START;
            }
        }
        for b in spec.ident_chars.bytes() {
            sets[b as usize] |= SET_IDENT;
        }
        for b in spec.ident_start.bytes() {
            sets[b as usize] |= SET_IDENT | SET_START;
        }
        for b in spec.ident_suffix.bytes() {
            sets[b as usize] |= SET_SUFFIX;
        }
        if spec.hook.is_some() {
            if spec.hook_chars.is_empty() {
                sets.iter_mut().for_each(|s| *s |= SET_HOOK);
            } else {
                for b in spec.hook_chars.bytes() {
                    sets[b as usize] |= SET_HOOK;
                }
            }
        }
        for c in spec.line_comments.iter().chain(spec.block_comments.iter().map(|(o, _)| o)) {
            sets[c.as_bytes()[0] as usize] |= SET_COMMENT;
        }
        for s in spec.strings {
            sets[s.open.as_bytes()[0] as usize] |= SET_STRING;
        }
        for b in spec.operators.bytes() {
            sets[b as usize] |= SET_OPERATOR;
        }
        for p in spec.string_prefixes {
            sets[p.ch as usize] |= SET_PREFIX;
        }
        let mut sigils = [None; 128];
        for &(b, kind) in spec.sigils {
            sets[b as usize] |= SET_SIGIL;
            sigils[b as usize & 127] = Some(kind);
        }
        if max_word >= 64 {
            word_lengths = u64::MAX;
        }
        Compiled { words, max_word, word_lengths, word_starts, sets, sigils }
    }
}

/// State of the previous significant token, used for context sensitive rules.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prev {
    /// Start of input or after an operator: an expression can start here.
    Start,
    /// After a value (identifier, literal, closing bracket).
    Value,
    /// After a keyword.
    Keyword,
}

pub struct Lexer<'c, 's, 'o> {
    pub ctx: &'c mut Ctx<'s, 'o>,
    pub spec: &'static Spec,
    c: &'static Compiled,
    prev: Prev,
    pending: Option<Kind>,
    /// Byte that begins a nested group in `run_until` mode.
    stop_open: u8,
    stop_close: u8,
}

impl<'c, 's, 'o> Lexer<'c, 's, 'o> {
    pub fn new(ctx: &'c mut Ctx<'s, 'o>, spec: &'static Spec) -> Self {
        Lexer { ctx, spec, c: spec.compiled(), prev: Prev::Start, pending: None, stop_open: 0, stop_close: 0 }
    }

    /// Lexes until the end of input.
    pub fn run(&mut self) {
        self.lex();
    }

    /// Lexes until the first unbalanced `close` byte (which is not consumed).
    pub fn run_until(&mut self, close: u8) {
        self.stop_close = close;
        self.stop_open = match close {
            b'}' => b'{',
            b')' => b'(',
            b']' => b'[',
            _ => 0,
        };
        self.lex();
    }

    /// Lexes a nested region (an interpolation or embedded expression) until
    /// the first unbalanced `close`, which is left unconsumed.
    pub fn nested(&mut self, close: u8) {
        if !self.ctx.can_nest() {
            return;
        }
        let saved = (self.prev, self.pending, self.stop_open, self.stop_close);
        self.prev = Prev::Start;
        self.pending = None;
        self.ctx.enter();
        self.run_until(close);
        self.ctx.leave();
        (self.prev, self.pending, self.stop_open, self.stop_close) = saved;
    }

    /// The kind a word has in this language's word lists, without side effects.
    pub fn word_kind(&self, word: &[u8]) -> Option<Kind> {
        self.lookup(word).map(|w| w.kind)
    }

    /// The kind the next identifier will get because of a preceding keyword (`fn`, `class`).
    pub fn pending(&self) -> Option<Kind> {
        self.pending
    }

    /// Makes the next identifier a `kind`, as `function_keywords` and `type_keywords` do.
    pub fn set_pending(&mut self, kind: Option<Kind>) {
        self.pending = kind;
    }

    /// The previous token allows an expression to start here.
    pub fn expression_start(&self) -> bool {
        self.prev != Prev::Value
    }

    pub fn mark_value(&mut self) {
        self.prev = Prev::Value;
    }

    pub fn mark_operator(&mut self) {
        self.prev = Prev::Start;
    }

    #[inline(always)]
    fn set(&self, b: u8) -> u16 {
        self.c.sets[b as usize]
    }

    #[inline(always)]
    pub fn is_ident_byte(&self, b: u8) -> bool {
        self.set(b) & SET_IDENT != 0
    }

    fn lex(&mut self) {
        let mut depth = 0u32;
        while !self.ctx.done() {
            let b = self.ctx.peek();
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                let src = self.ctx.src;
                let mut pos = self.ctx.pos + 1;
                while pos < src.len() && matches!(src[pos], b' ' | b'\t' | b'\n' | b'\r') {
                    pos += 1;
                }
                self.ctx.pos = pos;
                continue;
            }
            let set = self.set(b);
            if set & SET_HOOK != 0 {
                if let Some(hook) = self.spec.hook {
                    if hook(self) {
                        continue;
                    }
                }
            }
            if self.stop_close != 0 {
                if b == self.stop_close {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                } else if b == self.stop_open {
                    depth += 1;
                }
            }
            if set & SET_COMMENT != 0 && self.comment() {
                continue;
            }
            if set & SET_STRING != 0 && self.string_at_cursor() {
                continue;
            }
            if b.is_ascii_digit()
                || (b == b'.'
                    && self.ctx.peek_at(1).is_ascii_digit()
                    && !self.prev_is_ident()
                    && (self.ctx.pos == 0 || self.ctx.src[self.ctx.pos - 1] != b'.'))
            {
                self.number();
                continue;
            }
            if set & SET_START != 0 {
                self.ident();
                continue;
            }
            if set & SET_SIGIL != 0 && self.sigil(b) {
                continue;
            }
            self.symbol(b);
        }
    }

    fn prev_is_ident(&self) -> bool {
        let p = self.ctx.pos;
        p > 0 && self.is_ident_byte(self.ctx.src[p - 1])
    }

    fn comment(&mut self) -> bool {
        // Block markers first: they often extend a line marker (`#=` and `#`, `--[[` and `--`).
        for &(open, close) in self.spec.block_comments {
            if self.ctx.starts_with(open.as_bytes()) {
                block_comment(self.ctx, open, close, self.spec.nested_comments);
                return true;
            }
        }
        for c in self.spec.line_comments {
            if self.ctx.starts_with(c.as_bytes()) {
                let start = self.ctx.pos;
                self.ctx.skip_line();
                self.ctx.emit_from(start, Kind::Comment);
                return true;
            }
        }
        false
    }

    fn string_at_cursor(&mut self) -> bool {
        for rule in self.spec.strings {
            if self.ctx.starts_with(rule.open.as_bytes()) {
                let start = self.ctx.pos;
                self.string(start, rule, false, rule.interp);
                return true;
            }
        }
        false
    }

    /// Lexes a string literal whose opening delimiter is at the cursor.
    /// `start` may be earlier than the cursor when a prefix was consumed.
    pub fn string(&mut self, start: usize, rule: &Str, raw: bool, interp: Interp) {
        let escape = rule.escape && !raw;
        let close = rule.close.as_bytes();
        let kind = rule.kind;
        self.ctx.pos += rule.open.len();
        let mut seg = start;
        let interp_byte = match interp {
            Interp::None => 0,
            Interp::DollarBrace | Interp::Dollar | Interp::DollarParen => b'$',
            Interp::HashBrace => b'#',
            Interp::Brace => b'{',
            Interp::BackslashParen => b'\\',
        };
        loop {
            // Fast scan to the next interesting byte.
            let rest = &self.ctx.src[self.ctx.pos..];
            let skip = rest
                .iter()
                .position(|&b| b == close[0] || b == b'\\' || b == b'\n' || (b == interp_byte && b != 0))
                .unwrap_or(rest.len());
            self.ctx.pos += skip;
            if self.ctx.done() {
                self.ctx.emit_from(seg, kind);
                break;
            }
            let b = self.ctx.peek();
            if interp != Interp::None && b == interp_byte && self.interpolation(&mut seg, kind, interp) {
                continue;
            }
            if b == b'\\' && escape {
                self.ctx.emit_from(seg, kind);
                let esc_start = self.ctx.pos;
                escape_len(self.ctx);
                self.ctx.emit_from(esc_start, Kind::Escape);
                seg = self.ctx.pos;
                continue;
            }
            if self.ctx.starts_with(close) {
                if rule.doubled && self.ctx.starts_with_at(self.ctx.pos + close.len(), close) {
                    self.ctx.pos += close.len() * 2;
                    continue;
                }
                self.ctx.pos += close.len();
                self.ctx.emit_from(seg, kind);
                break;
            }
            if b == b'\n' && !rule.multiline {
                self.ctx.emit_from(seg, kind);
                break;
            }
            self.ctx.pos += 1;
        }
        self.prev = Prev::Value;
        if self.spec.has(features::STRING_KEYS) && kind == Kind::String {
            let after = self.ctx.next_non_space(self.ctx.pos);
            if after == b':' && self.ctx.at(self.ctx.pos + 1) != b':' {
                self.retag(start, Kind::Property);
            }
        }
    }

    /// Changes the kind of everything from `start` to the cursor.
    pub fn retag(&mut self, start: usize, kind: Kind) {
        let end = self.ctx.pos;
        self.ctx.truncate_to(start);
        self.ctx.emit(start, end, kind);
    }

    /// Handles an interpolation at the cursor. Returns `false` if the byte is
    /// not actually the start of an interpolation.
    fn interpolation(&mut self, seg: &mut usize, kind: Kind, interp: Interp) -> bool {
        let pos = self.ctx.pos;
        let next = self.ctx.at(pos + 1);
        let (open_len, close) = match interp {
            Interp::DollarBrace if next == b'{' => (2, b'}'),
            Interp::HashBrace if next == b'{' => (2, b'}'),
            Interp::Dollar | Interp::DollarParen if next == b'{' => (2, b'}'),
            Interp::DollarParen if next == b'(' => (2, b')'),
            Interp::Dollar | Interp::DollarParen if next.is_ascii_alphabetic() || next == b'_' => {
                self.ctx.emit_from(*seg, kind);
                let mut end = pos + 1;
                while is_ident(self.ctx.at(end)) {
                    end += 1;
                }
                self.ctx.emit(pos, end, Kind::Variable);
                self.ctx.pos = end;
                *seg = end;
                return true;
            }
            Interp::Brace if next == b'{' => {
                self.ctx.pos += 2;
                return true;
            }
            Interp::Brace => (1, b'}'),
            Interp::BackslashParen if next == b'(' => (2, b')'),
            _ => return false,
        };
        if !self.ctx.can_nest() {
            return false;
        }
        self.ctx.emit_from(*seg, kind);
        self.ctx.token(open_len, Kind::Punctuation);
        self.nested(close);
        if self.ctx.peek() == close {
            self.ctx.token(1, Kind::Punctuation);
        }
        *seg = self.ctx.pos;
        true
    }

    fn number(&mut self) {
        let start = self.ctx.pos;
        scan_number(self.ctx, self.spec.has(features::UNITS));
        self.ctx.emit_from(start, Kind::Number);
        self.prev = Prev::Value;
    }

    /// Reads an identifier at the cursor and returns its end.
    pub fn scan_ident(&self, from: usize) -> usize {
        let src = self.ctx.src;
        let mut end = from;
        loop {
            while end < src.len() && self.set(src[end]) & SET_IDENT != 0 {
                end += 1;
            }
            let kebab = self.spec.has(features::KEBAB)
                && end > from
                && src.get(end) == Some(&b'-')
                && src.get(end + 1).is_some_and(|b| b.is_ascii_alphabetic());
            if !kebab {
                break;
            }
            end += 1;
        }
        if end < src.len() && self.set(src[end]) & SET_SUFFIX != 0 {
            // `a != b` should not swallow the `!`.
            if src.get(end + 1) != Some(&b'=') {
                end += 1;
            }
        }
        end
    }

    fn lookup(&self, word: &[u8]) -> Option<Word> {
        if word.len() > self.c.max_word
            || 1u64.checked_shl(word.len() as u32).is_some_and(|bit| self.c.word_lengths & bit == 0)
            || !self.c.word_starts[word[0] as usize]
        {
            return None;
        }
        if self.spec.case_insensitive {
            let mut buf = [0u8; 64];
            let lower = &mut buf[..word.len()];
            lower.copy_from_slice(word);
            lower.make_ascii_lowercase();
            self.c.words.get(&*lower).copied()
        } else {
            self.c.words.get(word).copied()
        }
    }

    /// Classifies a bare word, returning its kind.
    pub fn classify(&mut self, start: usize, end: usize) -> Kind {
        let word = &self.ctx.src[start..end];
        let spec = self.spec;
        let after_pos = self.ctx.skip_spaces_from(end);
        let after = self.ctx.at(after_pos);
        let before = if start > 0 { self.ctx.src[start - 1] } else { 0 };
        let called = if spec.has(features::TIGHT_CALLS) {
            self.ctx.at(end) == b'('
        } else {
            spec.has(features::CALLS) && after == b'('
        };
        // Members win over keywords: `promise.catch()`, `map.get(key)`.
        let arrow = before == b'>' && start >= 2 && self.ctx.src[start - 2] == b'-';
        if spec.has(features::MEMBERS) && (arrow || (before == b'.' && (start < 2 || self.ctx.src[start - 2] != b'.')))
        {
            self.prev = Prev::Value;
            self.pending = None;
            return if called { Kind::Function } else { Kind::Property };
        }
        // `key:` directly followed by a colon is a key, even when the word is a keyword.
        if spec.has(features::IDENT_KEYS) && self.ctx.at(end) == b':' && !matches!(self.ctx.at(end + 1), b':' | b'=') {
            self.prev = Prev::Value;
            self.pending = None;
            return Kind::Property;
        }
        if let Some(w) = self.lookup(word) {
            if w.kind == Kind::Builtin {
                // A name being defined wins over builtins: `def print`, `proc echo`.
                if let Some(kind) = self.pending.take() {
                    self.prev = Prev::Value;
                    return kind;
                }
            }
            self.pending = match w.next {
                Next::None => None,
                Next::Function => Some(Kind::Function),
                Next::Type => Some(Kind::Type),
            };
            self.prev = match w.kind {
                Kind::Keyword => Prev::Keyword,
                _ => Prev::Value,
            };
            return w.kind;
        }
        self.prev = Prev::Value;
        if spec.has(features::LISP_CALLS) && before == b'(' {
            self.pending = None;
            return Kind::Function;
        }
        if let Some(kind) = self.pending.take() {
            return kind;
        }
        if after == b'='
            && !matches!(self.ctx.at(after_pos + 1), b'=' | b'>' | b'~')
            && (spec.has(features::ASSIGN_KEYS) || (spec.has(features::LINE_KEYS) && self.ctx.at_line_start(start)))
        {
            return Kind::Property;
        }
        let first = word[0];
        // `Point(1, 2)` is a type; all caps calls (`COUNT(x)`, `ASSERT(x)`) stay functions.
        if called
            && spec.has(features::CONSTRUCTORS)
            && first.is_ascii_uppercase()
            && word.iter().any(|b| b.is_ascii_lowercase())
        {
            return Kind::Type;
        }
        if called || (spec.has(features::ARROW_FUNCTIONS) && after == b'=' && self.function_assigned(end)) {
            return Kind::Function;
        }
        if spec.has(features::UPPER_CONSTANTS)
            && word.len() > 1
            && first.is_ascii_uppercase()
            && word.iter().all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            return Kind::Constant;
        }
        if spec.has(features::CAPITALIZED_TYPES) && first.is_ascii_uppercase() {
            return Kind::Type;
        }
        Kind::Text
    }

    /// Whether `= (args) =>`, `= async x =>` or `= function` follows `end`.
    fn function_assigned(&self, end: usize) -> bool {
        let src = self.ctx.src;
        let mut i = self.ctx.skip_spaces_from(end);
        if self.ctx.at(i + 1) == b'=' || self.ctx.at(i + 1) == b'>' {
            return false;
        }
        i = self.ctx.skip_spaces_from(i + 1);
        if src[i..].starts_with(b"async") && !is_ident(self.ctx.at(i + 5)) {
            i = self.ctx.skip_spaces_from(i + 5);
        }
        if src[i..].starts_with(b"function") && !is_ident(self.ctx.at(i + 8)) {
            return true;
        }
        if self.ctx.at(i) == b'<' {
            match angle_close(src, i) {
                Some(close) => i = self.ctx.skip_spaces_from(close + 1),
                None => return false,
            }
        }
        if self.ctx.at(i) == b'(' {
            let Some(close) = paren_close(src, i) else {
                return false;
            };
            i = self.ctx.skip_spaces_from(close + 1);
            if self.ctx.at(i) == b':' {
                // Return type annotation: look for `=>` on the same line.
                let line_end = self.ctx.line_end(i);
                return self.ctx.find(i, b"=>").is_some_and(|p| p < line_end);
            }
        } else if is_ident(self.ctx.at(i)) {
            i = self.ctx.skip_spaces_from(self.scan_ident(i));
        } else {
            return false;
        }
        src[i..].starts_with(b"=>")
    }

    fn ident(&mut self) {
        let start = self.ctx.pos;
        let end = self.scan_ident(start);
        // String prefixes such as r"..." or f'...'.
        if self.set(self.ctx.src[start]) & SET_PREFIX != 0 && end - start <= 3 && self.try_prefixed_string(start, end) {
            return;
        }
        if self.spec.has(features::BANG_MACROS) && self.ctx.at(end) == b'!' && self.ctx.at(end + 1) != b'=' {
            self.ctx.pos = end + 1;
            self.ctx.emit(start, end + 1, Kind::Function);
            self.prev = Prev::Start;
            return;
        }
        if self.spec.has(features::COMMANDS) && self.command_position(start) {
            self.ctx.pos = end;
            let kind = match self.lookup(&self.ctx.src[start..end]) {
                _ if self.ctx.at(end) == b'=' => Kind::Variable,
                Some(w) => w.kind,
                None => Kind::Function,
            };
            self.ctx.emit(start, end, kind);
            self.prev = Prev::Value;
            return;
        }
        self.ctx.pos = end;
        let kind = if self.spec.has(features::COMMANDS) && self.ctx.at(end) == b'=' && self.ctx.at(end + 1) != b'=' {
            // Shell assignments in argument position: `export PATH=...`, `make CC=gcc`.
            Kind::Variable
        } else {
            self.classify(start, end)
        };
        self.ctx.emit(start, end, kind);
    }

    /// For shell like languages: whether a word at `start` is in command position.
    fn command_position(&self, start: usize) -> bool {
        let src = self.ctx.src;
        let mut i = start;
        while i > 0 && matches!(src[i - 1], b' ' | b'\t') {
            i -= 1;
        }
        if i == 0 || matches!(src[i - 1], b'\n' | b';' | b'|' | b'&' | b'(' | b'`' | b'{' | b'!') {
            return true;
        }
        // After a keyword that introduces a command: `then echo`, `do make`.
        let word_end = i;
        while i > 0 && src[i - 1].is_ascii_alphabetic() {
            i -= 1;
        }
        matches!(
            &src[i..word_end],
            b"then"
                | b"do"
                | b"else"
                | b"elif"
                | b"if"
                | b"while"
                | b"until"
                | b"time"
                | b"sudo"
                | b"exec"
                | b"command"
                | b"xargs"
                | b"env"
        ) && self.command_position(i)
    }

    fn try_prefixed_string(&mut self, start: usize, end: usize) -> bool {
        let mut raw = false;
        let mut interp = Interp::None;
        for &b in &self.ctx.src[start..end] {
            match self.spec.string_prefixes.iter().find(|p| p.ch == b) {
                Some(p) => {
                    raw |= p.raw;
                    if p.interp != Interp::None {
                        interp = p.interp;
                    }
                }
                None => return false,
            }
        }
        for rule in self.spec.strings {
            if self.ctx.starts_with_at(end, rule.open.as_bytes()) {
                self.ctx.pos = end;
                // A raw prefix (`r'...'`) also turns off the rule's interpolation unless
                // another prefix asks for it (`rf"..."`).
                let interp = match interp {
                    Interp::None if raw => Interp::None,
                    Interp::None => rule.interp,
                    other => other,
                };
                self.string(start, rule, raw, interp);
                return true;
            }
        }
        false
    }

    fn sigil(&mut self, b: u8) -> bool {
        let Some(kind) = self.c.sigils[b as usize & 127] else {
            return false;
        };
        let start = self.ctx.pos;
        let mut i = start + 1;
        // Allow doubled sigils such as `@@class_var` or `$$`.
        if self.ctx.at(i) == b {
            i += 1;
        }
        let next = self.ctx.at(i);
        if !(next.is_ascii_alphabetic() || next == b'_' || next >= 0x80) {
            return false;
        }
        // `a ? b :c` and `foo::bar` must not become symbols.
        if b == b':' && start > 0 && (is_ident(self.ctx.src[start - 1]) || self.ctx.src[start - 1] == b':') {
            return false;
        }
        let mut end = self.scan_ident(i);
        if kind == Kind::Meta {
            // Annotations may be dotted: `@org.junit.Test`.
            while self.ctx.at(end) == b'.' && is_ident(self.ctx.at(end + 1)) {
                end = self.scan_ident(end + 1);
            }
        }
        self.ctx.pos = end;
        self.ctx.emit(start, end, kind);
        self.prev = Prev::Value;
        true
    }

    fn symbol(&mut self, b: u8) {
        let spec = self.spec;
        let start = self.ctx.pos;
        match b {
            b'/' if spec.has(features::REGEX) && self.prev != Prev::Value && self.regex() => {}
            b'<' if spec.has(features::JSX_ELEMENTS) && self.prev != Prev::Value && self.jsx() => {}
            b'#' if spec.has(features::PREPROCESSOR) && self.ctx.at_line_start(start) => self.preprocessor(),
            b'@' if spec.has(features::ANNOTATIONS) && is_ident(self.ctx.peek_at(1)) => {
                let mut end = self.scan_ident(start + 1);
                while self.ctx.at(end) == b'.' && is_ident(self.ctx.at(end + 1)) {
                    end = self.scan_ident(end + 1);
                }
                self.ctx.pos = end;
                self.ctx.emit(start, end, Kind::Meta);
                self.prev = Prev::Start;
            }
            b'-' if spec.has(features::FLAGS)
                && start > 0
                && matches!(self.ctx.src[start - 1], b' ' | b'\t')
                && (self.ctx.peek_at(1).is_ascii_alphabetic()
                    || (self.ctx.peek_at(1) == b'-' && self.ctx.peek_at(2).is_ascii_alphabetic())) =>
            {
                let mut end = start + 1;
                while matches!(self.ctx.at(end), b'-') {
                    end += 1;
                }
                while is_ident(self.ctx.at(end)) || self.ctx.at(end) == b'-' {
                    end += 1;
                }
                self.ctx.pos = end;
                self.ctx.emit(start, end, Kind::Attribute);
                self.prev = Prev::Value;
            }
            _ => {
                if self.set(b) & SET_OPERATOR != 0 {
                    let mut end = start + 1;
                    while end < self.ctx.len() && self.set(self.ctx.src[end]) & SET_OPERATOR != 0 {
                        // Never swallow the start of a comment.
                        if self.set(self.ctx.src[end]) & SET_COMMENT != 0 {
                            break;
                        }
                        end += 1;
                    }
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Operator);
                    self.prev = Prev::Start;
                } else if b.is_ascii_punctuation() {
                    self.ctx.token(1, Kind::Punctuation);
                    match b {
                        b')' | b']' | b'}' => self.prev = Prev::Value,
                        b'.' => {}
                        _ => {
                            self.prev = Prev::Start;
                            if matches!(b, b'(' | b'[' | b'{' | b';' | b',' | b'=') {
                                self.pending = None;
                            }
                        }
                    }
                } else {
                    self.ctx.bump_char();
                }
            }
        }
    }

    fn preprocessor(&mut self) {
        let start = self.ctx.pos;
        let mut end = start + 1;
        while matches!(self.ctx.at(end), b' ' | b'\t') {
            end += 1;
        }
        let word_start = end;
        while is_ident(self.ctx.at(end)) {
            end += 1;
        }
        let directive = &self.ctx.src[word_start..end];
        self.ctx.pos = end;
        self.ctx.emit(start, end, Kind::Meta);
        if directive == b"include" || directive == b"import" {
            self.ctx.skip_spaces();
            if self.ctx.peek() == b'<' {
                let s = self.ctx.pos;
                let line_end = self.ctx.line_end(s);
                let close = self.ctx.src[s..line_end].iter().position(|&b| b == b'>').map_or(line_end, |i| s + i + 1);
                self.ctx.pos = close;
                self.ctx.emit(s, close, Kind::String);
            }
        }
        self.prev = Prev::Start;
    }

    /// JavaScript style regex literal at the cursor.
    fn regex(&mut self) -> bool {
        let start = self.ctx.pos;
        let src = self.ctx.src;
        let next = self.ctx.at(start + 1);
        if next == b'/' || next == b'*' || next == b' ' {
            return false;
        }
        let mut i = start + 1;
        let mut class = false;
        loop {
            match src.get(i) {
                None | Some(b'\n') => return false,
                Some(b'\\') => i += 2,
                Some(b'[') => {
                    class = true;
                    i += 1;
                }
                Some(b']') => {
                    class = false;
                    i += 1;
                }
                Some(b'/') if !class => break,
                Some(_) => i += 1,
            }
        }
        i += 1;
        while self.ctx.at(i).is_ascii_alphabetic() {
            i += 1;
        }
        self.ctx.pos = i.min(self.ctx.len());
        self.ctx.emit(start, self.ctx.pos, Kind::Regex);
        self.prev = Prev::Value;
        true
    }

    /// JSX element at the cursor (`<` in expression position).
    fn jsx(&mut self) -> bool {
        let next = self.ctx.peek_at(1);
        if !(next.is_ascii_alphabetic() || next == b'>') || !self.ctx.can_nest() {
            return false;
        }
        if next != b'>' && self.type_parameters_ahead(self.ctx.pos) {
            return false;
        }
        let cp = self.ctx.checkpoint();
        self.ctx.enter();
        let ok = self.jsx_element();
        self.ctx.leave();
        if !ok {
            self.ctx.rollback(cp);
            return false;
        }
        self.prev = Prev::Value;
        true
    }

    /// Whether the `<` at `lt` opens the type parameters of a generic arrow
    /// function rather than a JSX element. Recognises all the TSX spellings:
    /// `<T,>(x) => x`, `<T extends U>(x) => x`, `<T = X,>`, `<const T,>` and
    /// plain `<T>(x: T): R => x`.
    fn type_parameters_ahead(&self, lt: usize) -> bool {
        let src = self.ctx.src;
        let mut i = lt + 1;
        let word_end = |from: usize| {
            let mut e = from;
            while e < src.len() && (is_ident(src[e]) || src[e] == b'$') {
                e += 1;
            }
            e
        };
        let mut end = word_end(i);
        if &src[i..end] == b"const" && matches!(self.ctx.at(end), b' ' | b'\t' | b'\n') {
            return true;
        }
        // Skip a dotted name; JSX names can contain `.`, `-` and `:`.
        while matches!(self.ctx.at(end), b'.' | b'-' | b':') && is_ident(self.ctx.at(end + 1)) {
            end = word_end(end + 1);
        }
        i = end;
        while matches!(self.ctx.at(i), b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
        }
        match self.ctx.at(i) {
            // `<T,` can never start a JSX element.
            b',' => return true,
            // `<T = Default>`: an attribute would have a name before `=`.
            b'=' if self.ctx.at(i + 1) != b'>' && i > end => return true,
            b'e' if src[i..].starts_with(b"extends") && !is_ident(self.ctx.at(i + 7)) && i > end => {
                // `<T extends U>` vs `<div extends="x">`.
                let after = self.ctx.next_non_space(i + 7);
                return after != b'=';
            }
            b'>' => {}
            _ => return false,
        }
        // `<T>` followed by a parameter list and `=>` (or a return type).
        let mut j = i + 1;
        while matches!(self.ctx.at(j), b' ' | b'\t' | b'\n' | b'\r') {
            j += 1;
        }
        if self.ctx.at(j) != b'(' {
            return false;
        }
        let Some(close) = paren_close(src, j) else {
            return false;
        };
        let mut k = close + 1;
        while matches!(self.ctx.at(k), b' ' | b'\t' | b'\n' | b'\r') {
            k += 1;
        }
        src[k..].starts_with(b"=>") || (self.ctx.at(k) == b':' && self.ctx.at(k + 1) != b':')
    }

    /// Lexes type arguments up to `end` without JSX or regex detection.
    fn type_arguments(&mut self, end: usize) {
        while self.ctx.pos < end {
            let b = self.ctx.peek();
            if is_ident(b) && !b.is_ascii_digit() {
                let start = self.ctx.pos;
                let e = self.scan_ident(start).min(end);
                self.ctx.pos = e;
                let kind = match self.classify(start, e) {
                    Kind::Text => Kind::Type,
                    k => k,
                };
                self.ctx.emit(start, e, kind);
            } else if b.is_ascii_punctuation() {
                let kind = if matches!(b, b'|' | b'&' | b'=' | b'?') { Kind::Operator } else { Kind::Punctuation };
                self.ctx.token(1, kind);
            } else {
                self.ctx.bump_char();
            }
        }
        self.pending = None;
    }

    fn jsx_name(&mut self) -> Option<bool> {
        let start = self.ctx.pos;
        let mut end = start;
        while is_ident(self.ctx.at(end)) || matches!(self.ctx.at(end), b'.' | b'-' | b':' | b'$') {
            end += 1;
        }
        if end == start {
            return None;
        }
        let component = self.ctx.src[start].is_ascii_uppercase() || self.ctx.src[start..end].contains(&b'.');
        self.ctx.pos = end;
        self.ctx.emit(start, end, if component { Kind::Type } else { Kind::Tag });
        Some(component)
    }

    /// Parses `<tag attrs>children</tag>`. Returns false if this does not look like JSX.
    fn jsx_element(&mut self) -> bool {
        self.ctx.token(1, Kind::Punctuation); // <
        let fragment = self.ctx.peek() == b'>';
        if !fragment {
            if self.jsx_name().is_none() {
                return false;
            }
            // Type arguments on a component: `<Select<Option> value={v} />`.
            if self.ctx.peek() == b'<' {
                if let Some(close) = angle_close(self.ctx.src, self.ctx.pos) {
                    self.type_arguments(close + 1);
                }
            }
            loop {
                self.ctx.skip_whitespace();
                let b = self.ctx.peek();
                match b {
                    0 => return true,
                    b'/' if self.ctx.peek_at(1) == b'>' => {
                        self.ctx.token(2, Kind::Punctuation);
                        return true;
                    }
                    b'>' => {
                        self.ctx.token(1, Kind::Punctuation);
                        break;
                    }
                    b'{' => self.jsx_expression(),
                    b'/' if self.ctx.peek_at(1) == b'/' || self.ctx.peek_at(1) == b'*' => {
                        if !self.comment() {
                            self.ctx.pos += 1;
                        }
                    }
                    _ if is_ident(b) => {
                        let s = self.ctx.pos;
                        let mut e = s;
                        while is_ident(self.ctx.at(e)) || matches!(self.ctx.at(e), b'-' | b':' | b'$') {
                            e += 1;
                        }
                        self.ctx.pos = e;
                        self.ctx.emit(s, e, Kind::Attribute);
                        self.ctx.skip_whitespace();
                        if self.ctx.peek() == b'=' {
                            self.ctx.token(1, Kind::Operator);
                            self.ctx.skip_whitespace();
                            match self.ctx.peek() {
                                q @ (b'"' | b'\'') => {
                                    let s = self.ctx.pos;
                                    let close = self.ctx.src[s + 1..]
                                        .iter()
                                        .position(|&c| c == q)
                                        .map_or(self.ctx.len(), |i| s + i + 2);
                                    self.ctx.pos = close;
                                    self.ctx.emit(s, close, Kind::String);
                                }
                                b'{' => self.jsx_expression(),
                                _ => {}
                            }
                        }
                    }
                    _ => return false,
                }
            }
        } else {
            self.ctx.token(1, Kind::Punctuation);
        }
        // Children.
        loop {
            let rest = &self.ctx.src[self.ctx.pos..];
            let skip = rest.iter().position(|&b| b == b'<' || b == b'{').unwrap_or(rest.len());
            self.ctx.pos += skip;
            match self.ctx.peek() {
                0 => return true,
                b'{' => self.jsx_expression(),
                _ => {
                    if self.ctx.peek_at(1) == b'/' {
                        self.ctx.token(2, Kind::Punctuation);
                        self.ctx.skip_whitespace();
                        let _ = self.jsx_name();
                        self.ctx.skip_whitespace();
                        if self.ctx.peek() == b'>' {
                            self.ctx.token(1, Kind::Punctuation);
                        }
                        return true;
                    }
                    let next = self.ctx.peek_at(1);
                    if (next.is_ascii_alphabetic() || next == b'>') && self.ctx.can_nest() {
                        let cp = self.ctx.checkpoint();
                        self.ctx.enter();
                        let ok = self.jsx_element();
                        self.ctx.leave();
                        if !ok {
                            self.ctx.rollback(cp);
                            self.ctx.pos += 1;
                        }
                    } else {
                        self.ctx.pos += 1;
                    }
                }
            }
        }
    }

    fn jsx_expression(&mut self) {
        self.ctx.token(1, Kind::Punctuation);
        self.nested(b'}');
        if self.ctx.peek() == b'}' {
            self.ctx.token(1, Kind::Punctuation);
        }
    }
}

/// Offset of the `>` closing the `<` at `lt`, ignoring `=>` inside. Gives up
/// after a reasonable distance or at characters that cannot appear in types.
fn angle_close(src: &[u8], lt: usize) -> Option<usize> {
    let mut depth = 0;
    let limit = (lt + 1024).min(src.len());
    let mut i = lt;
    while i < limit {
        match src[i] {
            b'<' => depth += 1,
            b'>' if i > 0 && src[i - 1] == b'=' => {}
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b';' | b'}' if depth == 1 && !src[lt..i].contains(&b'{') => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Offset of the `)` matching the `(` at `open`, skipping strings.
fn paren_close(src: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0;
    let limit = (open + 4096).min(src.len());
    let mut i = open;
    while i < limit {
        match src[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < limit && src[i] != q {
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

/// Consumes a block comment at the cursor.
pub fn block_comment(ctx: &mut Ctx, open: &str, close: &str, nested: bool) {
    let start = ctx.pos;
    ctx.pos += open.len();
    let mut depth = 1;
    let (open, close) = (open.as_bytes(), close.as_bytes());
    while !ctx.done() {
        if ctx.starts_with(close) {
            ctx.pos += close.len();
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if nested && ctx.starts_with(open) {
            ctx.pos += open.len();
            depth += 1;
        } else {
            ctx.pos += 1;
        }
    }
    ctx.emit_from(start, Kind::Comment);
}

/// Consumes an escape sequence starting at a backslash at the cursor.
pub fn escape_len(ctx: &mut Ctx) {
    ctx.pos += 1;
    let b = ctx.peek();
    match b {
        b'x' => {
            ctx.pos += 1;
            take_while_max(ctx, 2, |b| b.is_ascii_hexdigit());
        }
        b'u' | b'U' => {
            ctx.pos += 1;
            if ctx.peek() == b'{' {
                let end = ctx.src[ctx.pos..].iter().take(10).position(|&b| b == b'}');
                if let Some(i) = end {
                    ctx.pos += i + 1;
                }
            } else {
                take_while_max(ctx, if b == b'u' { 4 } else { 8 }, |b| b.is_ascii_hexdigit());
            }
        }
        b'0'..=b'7' => take_while_max(ctx, 3, |b| (b'0'..=b'7').contains(&b)),
        0 => {}
        _ => ctx.bump_char(),
    }
}

fn take_while_max(ctx: &mut Ctx, max: usize, f: impl Fn(u8) -> bool) {
    let mut n = 0;
    while n < max && f(ctx.peek()) && !ctx.done() {
        ctx.pos += 1;
        n += 1;
    }
}

/// Consumes a numeric literal at the cursor.
pub fn scan_number(ctx: &mut Ctx, units: bool) {
    let b = ctx.peek();
    let radix = if b == b'0' { ctx.peek_at(1) | 0x20 } else { 0 };
    if matches!(radix, b'x' | b'b' | b'o') && ctx.peek_at(2).is_ascii_alphanumeric() {
        ctx.pos += 2;
        while ctx.peek().is_ascii_hexdigit() || ctx.peek() == b'_' {
            ctx.pos += 1;
        }
    } else {
        digits(ctx);
        if ctx.peek() == b'.' && ctx.peek_at(1).is_ascii_digit() {
            ctx.pos += 1;
            digits(ctx);
        }
        if ctx.peek() | 0x20 == b'e' {
            let sign = matches!(ctx.peek_at(1), b'+' | b'-') as usize;
            if ctx.peek_at(1 + sign).is_ascii_digit() {
                ctx.pos += 1 + sign;
                digits(ctx);
            }
        }
    }
    // Suffixes such as `u32`, `f`, `L`, `n` or units like `px` and `%`.
    while ctx.peek().is_ascii_alphanumeric() || ctx.peek() == b'_' {
        ctx.pos += 1;
    }
    if units && ctx.peek() == b'%' {
        ctx.pos += 1;
    }
}

fn digits(ctx: &mut Ctx) {
    while ctx.peek().is_ascii_digit() || (ctx.peek() == b'_' && ctx.peek_at(1).is_ascii_digit()) {
        ctx.pos += 1;
    }
}
