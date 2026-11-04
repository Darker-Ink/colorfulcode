//! The shared lexing machinery.
//!
//! [`Ctx`] owns the output token list and a cursor into the source. Every
//! language lexer, whether table driven ([`generic`]) or hand written, only
//! ever emits the interesting tokens. Gaps between them are filled with
//! [`Kind::Text`] and adjacent tokens of the same kind are merged, so the
//! final output always covers the whole input exactly once.

#![allow(dead_code)]

pub mod generic;

use crate::{Kind, Language, Token};

/// Maximum nesting depth for embedded languages and interpolations. Protects
/// against stack overflows on adversarial input.
const MAX_DEPTH: u32 = 48;

pub struct Ctx<'s, 'o> {
    /// The source, truncated to the end of the region being lexed. Positions
    /// are always absolute offsets into the original input.
    pub src: &'s [u8],
    pub pos: usize,
    out: &'o mut Vec<Token>,
    depth: u32,
}

/// A saved lexer position that can be restored when a speculative parse fails.
#[derive(Clone, Copy)]
pub struct Checkpoint {
    pos: usize,
    len: usize,
    last_end: u32,
}

impl<'s, 'o> Ctx<'s, 'o> {
    pub fn new(src: &'s [u8], out: &'o mut Vec<Token>) -> Self {
        Ctx { src, pos: 0, out, depth: 0 }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.src.len()
    }

    #[inline(always)]
    pub fn done(&self) -> bool {
        self.pos >= self.src.len()
    }

    #[inline(always)]
    pub fn peek(&self) -> u8 {
        self.at(self.pos)
    }

    #[inline(always)]
    pub fn peek_at(&self, offset: usize) -> u8 {
        self.at(self.pos + offset)
    }

    /// Byte at `index`, or `0` past the end.
    #[inline(always)]
    pub fn at(&self, index: usize) -> u8 {
        match self.src.get(index) {
            Some(&b) => b,
            None => 0,
        }
    }

    #[inline(always)]
    pub fn starts_with(&self, s: &[u8]) -> bool {
        self.src[self.pos..].starts_with(s)
    }

    #[inline(always)]
    pub fn starts_with_at(&self, index: usize, s: &[u8]) -> bool {
        index <= self.src.len() && self.src[index..].starts_with(s)
    }

    /// Case insensitive (ASCII) prefix check at the cursor.
    pub fn starts_with_ignore_case(&self, s: &[u8]) -> bool {
        let rest = &self.src[self.pos..];
        rest.len() >= s.len() && rest[..s.len()].eq_ignore_ascii_case(s)
    }

    /// Records a token. Gaps are filled with text and equal neighbors merged.
    #[inline]
    pub fn emit(&mut self, start: usize, end: usize, kind: Kind) {
        let mut start = start as u32;
        let end = end.min(self.src.len()) as u32;
        let last_end = self.out.last().map_or(0, |t| t.end);
        if start < last_end {
            start = last_end;
        }
        if start >= end {
            return;
        }
        if start > last_end {
            self.push(last_end, start, Kind::Text);
        }
        self.push(start, end, kind);
    }

    #[inline(always)]
    fn push(&mut self, start: u32, end: u32, kind: Kind) {
        if let Some(last) = self.out.last_mut() {
            if last.kind == kind && last.end == start {
                last.end = end;
                return;
            }
        }
        self.out.push(Token { start, end, kind });
    }

    /// Emits `[pos, pos + len)` and advances the cursor past it.
    #[inline]
    pub fn token(&mut self, len: usize, kind: Kind) {
        let start = self.pos;
        self.pos = (start + len).min(self.src.len());
        self.emit(start, self.pos, kind);
    }

    /// Emits from `start` to the cursor.
    #[inline]
    pub fn emit_from(&mut self, start: usize, kind: Kind) {
        let end = self.pos;
        self.emit(start, end, kind);
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint { pos: self.pos, len: self.out.len(), last_end: self.out.last().map_or(0, |t| t.end) }
    }

    pub fn rollback(&mut self, cp: Checkpoint) {
        self.pos = cp.pos;
        self.out.truncate(cp.len);
        if let Some(last) = self.out.last_mut() {
            last.end = cp.last_end;
        }
    }

    /// Discards every token from `start` on, shortening one that crosses it.
    pub fn truncate_to(&mut self, start: usize) {
        let start = start as u32;
        while let Some(last) = self.out.last_mut() {
            if last.start >= start {
                self.out.pop();
            } else {
                if last.end > start {
                    last.end = start;
                }
                break;
            }
        }
    }

    /// Advances past one UTF-8 encoded character.
    #[inline]
    pub fn bump_char(&mut self) {
        self.pos += 1;
        while self.pos < self.src.len() && (self.src[self.pos] & 0xC0) == 0x80 {
            self.pos += 1;
        }
    }

    pub fn skip_spaces(&mut self) {
        while matches!(self.peek(), b' ' | b'\t') {
            self.pos += 1;
        }
    }

    pub fn skip_whitespace(&mut self) {
        while matches!(self.peek(), b' ' | b'\t' | b'\n' | b'\r' | 0x0C) {
            self.pos += 1;
        }
    }

    /// Moves the cursor to the next `\n` (or the end) without consuming it.
    pub fn skip_line(&mut self) {
        self.pos = self.line_end(self.pos);
    }

    /// Offset of the next `\n` at or after `from`, or the end of input.
    pub fn line_end(&self, from: usize) -> usize {
        match self.src[from.min(self.src.len())..].iter().position(|&b| b == b'\n') {
            Some(i) => from + i,
            None => self.src.len(),
        }
    }

    /// Like [`Ctx::line_end`] but looks at most `max` bytes ahead. Use it in
    /// hooks that run often, so a huge single line cannot make lexing quadratic.
    pub fn line_end_within(&self, from: usize, max: usize) -> usize {
        let from = from.min(self.src.len());
        let limit = (from + max).min(self.src.len());
        match self.src[from..limit].iter().position(|&b| b == b'\n') {
            Some(i) => from + i,
            None => limit,
        }
    }

    /// Offset of the first occurrence of `needle` at or after `from`.
    pub fn find(&self, from: usize, needle: &[u8]) -> Option<usize> {
        if needle.is_empty() || from > self.src.len() {
            return None;
        }
        let first = needle[0];
        let hay = &self.src[from..];
        let mut i = 0;
        while let Some(off) = hay[i..].iter().position(|&b| b == first) {
            let at = i + off;
            if hay[at..].starts_with(needle) {
                return Some(from + at);
            }
            i = at + 1;
        }
        None
    }

    /// Whether only spaces and tabs precede `pos` on its line.
    pub fn at_line_start(&self, pos: usize) -> bool {
        let mut i = pos;
        while i > 0 {
            match self.src[i - 1] {
                b' ' | b'\t' => i -= 1,
                b'\n' => return true,
                _ => return false,
            }
        }
        true
    }

    /// Previous non space byte before `pos` on any line, or `0`.
    pub fn prev_non_space(&self, pos: usize) -> u8 {
        let mut i = pos;
        while i > 0 {
            let b = self.src[i - 1];
            if !matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                return b;
            }
            i -= 1;
        }
        0
    }

    /// Next byte at or after `pos` that is not a space or tab.
    pub fn next_non_space(&self, pos: usize) -> u8 {
        self.at(self.skip_spaces_from(pos))
    }

    /// First offset at or after `pos` that is not a space or tab.
    pub fn skip_spaces_from(&self, pos: usize) -> usize {
        let mut i = pos;
        while matches!(self.at(i), b' ' | b'\t') {
            i += 1;
        }
        i
    }

    /// Runs `lang` over `[start, end)` and leaves the cursor at `end`.
    pub fn embed(&mut self, lang: Language, start: usize, end: usize) {
        let end = end.min(self.src.len());
        if start >= end {
            self.pos = self.pos.max(end);
            return;
        }
        if self.depth < MAX_DEPTH {
            let mut sub = Ctx { src: &self.src[..end], pos: start, out: &mut *self.out, depth: self.depth + 1 };
            lang.lex(&mut sub);
        }
        self.pos = end;
    }

    /// Runs a table driven spec over `[start, end)` and leaves the cursor at `end`.
    pub fn embed_spec(&mut self, spec: &'static generic::Spec, start: usize, end: usize) {
        let end = end.min(self.src.len());
        if start < end && self.depth < MAX_DEPTH {
            let mut sub = Ctx { src: &self.src[..end], pos: start, out: &mut *self.out, depth: self.depth + 1 };
            generic::Lexer::new(&mut sub, spec).run();
        }
        self.pos = self.pos.max(end);
    }

    /// Like [`Ctx::embed`] but stops at the first unbalanced `close` byte,
    /// which is left unconsumed. Only works for table driven languages.
    pub fn embed_until(&mut self, lang: Language, close: u8) {
        match lang.spec() {
            Some(spec) if self.depth < MAX_DEPTH => {
                self.depth += 1;
                generic::Lexer::new(self, spec).run_until(close);
                self.depth -= 1;
            }
            _ => {
                // Fall back to skipping to the matching close byte.
                let open = match close {
                    b'}' => b'{',
                    b')' => b'(',
                    b']' => b'[',
                    _ => 0,
                };
                let mut depth = 0;
                while !self.done() {
                    let b = self.peek();
                    if b == open {
                        depth += 1;
                    } else if b == close {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    self.pos += 1;
                }
            }
        }
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    pub fn can_nest(&self) -> bool {
        self.depth < MAX_DEPTH
    }

    pub(crate) fn enter(&mut self) {
        self.depth += 1;
    }

    pub(crate) fn leave(&mut self) {
        self.depth -= 1;
    }
}

/// True for bytes that can appear in a typical identifier. Non ASCII bytes
/// are treated as identifier characters so multi byte characters are never
/// split.
#[inline(always)]
pub fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

#[inline(always)]
pub fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}
