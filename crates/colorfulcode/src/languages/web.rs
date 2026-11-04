//! JavaScript, TypeScript, CSS and JSON.

use super::{Definition, Engine};
use crate::lexer::generic::{features::*, Interp, Lexer, Spec, Str};
use crate::lexer::{is_ident, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] =
    &[&JAVASCRIPT, &JSX, &TYPESCRIPT, &TSX, &CSS, &SCSS, &SASS, &LESS, &STYLUS, &JSON, &JSONC, &JSON5];

const JS_STRINGS: &[Str] =
    &[Str::quoted("\""), Str::quoted("'"), Str::quoted("`").multiline().interp(Interp::DollarBrace)];

const JS_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "case", "catch", "const", "continue", "debugger", "default", "delete", "do",
    "else", "export", "extends", "finally", "for", "from", "get", "if", "import", "in", "instanceof", "let", "new",
    "of", "return", "set", "static", "super", "switch", "this", "throw", "try", "typeof", "var", "void", "while",
    "with", "yield",
];

const JS_CONSTANTS: &[&str] = &["true", "false", "null", "undefined", "NaN", "Infinity"];

const JS_BUILTINS: &[&str] = &[
    "console", "window", "document", "globalThis", "process", "require", "module", "exports", "arguments", "JSON",
    "Math", "Reflect", "Intl", "Atomics",
];

const JS_FEATURES: u32 =
    CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | REGEX | ARROW_FUNCTIONS | ANNOTATIONS;

const TS_KEYWORDS: &[&str] = &[
    "abstract", "accessor", "as", "asserts", "async", "await", "break", "case", "catch", "const", "continue",
    "debugger", "declare", "default", "delete", "do", "else", "export", "extends", "finally", "for", "from", "get",
    "if", "implements", "import", "in", "infer", "instanceof", "is", "keyof", "let", "module", "namespace", "new",
    "of", "out", "override", "private", "protected", "public", "readonly", "return", "satisfies", "set", "static",
    "super", "switch", "this", "throw", "try", "typeof", "unique", "using", "var", "void", "while", "with", "yield",
];

const TS_TYPES: &[&str] = &["any", "bigint", "boolean", "never", "number", "object", "string", "symbol", "unknown"];

/// Shebang lines and `#private` class members.
fn js_hook(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    if ctx.pos == 0 && ctx.starts_with(b"#!") {
        ctx.skip_line();
        ctx.emit_from(0, Kind::Comment);
        return true;
    }
    if is_ident(ctx.peek_at(1)) && !ctx.peek_at(1).is_ascii_digit() {
        let start = ctx.pos;
        let end = lx.scan_ident(start + 1);
        lx.ctx.pos = end;
        let kind = if lx.ctx.next_non_space(end) == b'(' { Kind::Function } else { Kind::Property };
        lx.ctx.emit(start, end, kind);
        lx.mark_value();
        return true;
    }
    false
}

const JS_BASE: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: JS_STRINGS,
    keywords: JS_KEYWORDS,
    constants: JS_CONSTANTS,
    builtins: JS_BUILTINS,
    function_keywords: &["function"],
    type_keywords: &["class"],
    ident_start: "$",
    features: JS_FEATURES,
    hook: Some(js_hook),
    hook_chars: "#",
    ..Spec::DEFAULT
};

pub static JSX_SPEC: Spec = Spec { features: JS_FEATURES | JSX_ELEMENTS, ..JS_BASE };

const TS_BASE: Spec =
    Spec { keywords: TS_KEYWORDS, types: TS_TYPES, type_keywords: &["class", "interface", "type", "enum"], ..JS_BASE };

pub static TS_SPEC: Spec = TS_BASE;
pub static TSX_SPEC: Spec = Spec { features: JS_FEATURES | JSX_ELEMENTS, ..TS_BASE };

pub static JAVASCRIPT: Definition = Definition::new("javascript", "JavaScript", Engine::Generic(&JSX_SPEC))
    .aliases(&["js", "node", "mjs", "cjs"])
    .extensions(&["js", "mjs", "cjs"]);

pub static JSX: Definition = Definition::new("jsx", "JSX", Engine::Generic(&JSX_SPEC)).extensions(&["jsx"]);

pub static TYPESCRIPT: Definition = Definition::new("typescript", "TypeScript", Engine::Generic(&TS_SPEC))
    .aliases(&["ts", "mts", "cts"])
    .extensions(&["ts", "mts", "cts"]);

pub static TSX: Definition = Definition::new("tsx", "TSX", Engine::Generic(&TSX_SPEC)).extensions(&["tsx"]);

// JSON

const JSON_CONSTANTS: &[&str] = &["true", "false", "null"];

static JSON_SPEC: Spec = Spec {
    strings: &[Str::quoted("\"")],
    constants: JSON_CONSTANTS,
    features: STRING_KEYS,
    operators: "",
    ..Spec::DEFAULT
};

static JSONC_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"")],
    constants: JSON_CONSTANTS,
    features: STRING_KEYS,
    operators: "",
    ..Spec::DEFAULT
};

static JSON5_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").multiline(), Str::quoted("'").multiline()],
    constants: &["true", "false", "null", "Infinity", "NaN"],
    ident_start: "$",
    features: STRING_KEYS | IDENT_KEYS,
    operators: "+-",
    ..Spec::DEFAULT
};

pub static JSON: Definition = Definition::new("json", "JSON", Engine::Generic(&JSON_SPEC))
    .extensions(&["json", "geojson", "webmanifest", "har", "jsonl", "ndjson"])
    .filenames(&[".babelrc", ".prettierrc", ".eslintrc", "composer.lock", "flake.lock"]);

pub static JSONC: Definition = Definition::new("jsonc", "JSON with Comments", Engine::Generic(&JSONC_SPEC))
    .aliases(&["json-with-comments"])
    .extensions(&["jsonc", "code-workspace"])
    .filenames(&["tsconfig.json", "jsconfig.json", "settings.json", "devcontainer.json", ".eslintrc.json"]);

pub static JSON5: Definition = Definition::new("json5", "JSON5", Engine::Generic(&JSON5_SPEC)).extensions(&["json5"]);

// CSS and friends

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Css,
    Scss,
    Less,
    /// Indentation based syntax (Sass and Stylus).
    Indented,
}

pub static CSS: Definition = Definition::new("css", "CSS", Engine::Custom(|ctx| Css::new(ctx, Dialect::Css).run()))
    .extensions(&["css", "pcss", "postcss"]);

pub static SCSS: Definition =
    Definition::new("scss", "SCSS", Engine::Custom(|ctx| Css::new(ctx, Dialect::Scss).run())).extensions(&["scss"]);

pub static SASS: Definition =
    Definition::new("sass", "Sass", Engine::Custom(|ctx| Css::new(ctx, Dialect::Indented).run())).extensions(&["sass"]);

pub static LESS: Definition =
    Definition::new("less", "Less", Engine::Custom(|ctx| Css::new(ctx, Dialect::Less).run())).extensions(&["less"]);

pub static STYLUS: Definition =
    Definition::new("stylus", "Stylus", Engine::Custom(|ctx| Css::new(ctx, Dialect::Indented).run()))
        .aliases(&["styl"])
        .extensions(&["styl"]);

struct Css<'c, 's, 'o> {
    ctx: &'c mut Ctx<'s, 'o>,
    dialect: Dialect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Selector,
    Declaration,
}

impl<'c, 's, 'o> Css<'c, 's, 'o> {
    fn new(ctx: &'c mut Ctx<'s, 'o>, dialect: Dialect) -> Self {
        Css { ctx, dialect }
    }

    fn run(&mut self) {
        let mut depth = 0u32;
        while !self.ctx.done() {
            self.ctx.skip_whitespace();
            if self.ctx.done() {
                break;
            }
            if self.comment() {
                continue;
            }
            let b = self.ctx.peek();
            match b {
                b'{' => {
                    depth += 1;
                    self.ctx.token(1, Kind::Punctuation);
                }
                b'}' => {
                    depth = depth.saturating_sub(1);
                    self.ctx.token(1, Kind::Punctuation);
                }
                b';' => self.ctx.token(1, Kind::Punctuation),
                b'@' => self.at_rule(),
                _ => match self.classify(depth) {
                    Part::Selector => self.selector(),
                    Part::Declaration => self.declaration(),
                },
            }
        }
    }

    fn comment(&mut self) -> bool {
        if self.ctx.starts_with(b"/*") {
            crate::lexer::generic::block_comment(self.ctx, "/*", "*/", false);
            return true;
        }
        if self.dialect != Dialect::Css && self.ctx.starts_with(b"//") {
            let start = self.ctx.pos;
            self.ctx.skip_line();
            self.ctx.emit_from(start, Kind::Comment);
            return true;
        }
        false
    }

    /// Decides whether the statement at the cursor is a selector or a declaration.
    fn classify(&self, depth: u32) -> Part {
        let src = self.ctx.src;
        let b = self.ctx.peek();
        if matches!(b, b'$') || (b == b'-' && self.ctx.peek_at(1) == b'-') {
            return Part::Declaration;
        }
        if self.dialect == Dialect::Indented {
            let end = self.ctx.line_end(self.ctx.pos);
            let line = &src[self.ctx.pos..end];
            // `prop: value` or `prop value` (Stylus) vs `a:hover` or `.class`.
            let colon_space = line.windows(2).any(|w| w == b": ");
            let selectorish = matches!(b, b'.' | b'#' | b'&' | b'[' | b':' | b'*' | b'>' | b'+' | b'~');
            return if colon_space && !selectorish { Part::Declaration } else { Part::Selector };
        }
        if depth == 0 && self.dialect == Dialect::Css {
            return Part::Selector;
        }
        let mut i = self.ctx.pos;
        let mut parens = 0;
        while i < src.len() {
            match src[i] {
                b'(' => parens += 1,
                b')' => parens -= 1,
                b'#' if src.get(i + 1) == Some(&b'{') => {
                    // Interpolation: skip to its end.
                    while i < src.len() && src[i] != b'}' {
                        i += 1;
                    }
                }
                b'"' | b'\'' => {
                    let q = src[i];
                    i += 1;
                    while i < src.len() && src[i] != q && src[i] != b'\n' {
                        i += 1;
                    }
                }
                b'{' if parens == 0 => return Part::Selector,
                b';' | b'}' if parens == 0 => return Part::Declaration,
                _ => {}
            }
            i += 1;
        }
        if depth == 0 {
            Part::Selector
        } else {
            Part::Declaration
        }
    }

    fn ends_statement(&self, b: u8, part: Part) -> bool {
        match part {
            Part::Selector => b == b'{' || b == b'}' || (self.dialect == Dialect::Indented && b == b'\n'),
            Part::Declaration => {
                b == b';'
                    || b == b'}'
                    || (b == b'{' && self.ctx.at(self.ctx.pos.saturating_sub(1)) != b'#')
                    || (self.dialect == Dialect::Indented && b == b'\n')
            }
        }
    }

    fn selector(&mut self) {
        while !self.ctx.done() {
            let b = self.ctx.peek();
            if self.ends_statement(b, Part::Selector) {
                return;
            }
            if self.comment() || self.interpolation() {
                continue;
            }
            match b {
                b'.' | b'#' if self.name_at(self.ctx.pos + 1) => {
                    let start = self.ctx.pos;
                    let end = self.name_end(start + 1);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, if b == b'.' { Kind::Type } else { Kind::Constant });
                }
                b':' => {
                    let start = self.ctx.pos;
                    let mut end = start + 1;
                    if self.ctx.at(end) == b':' {
                        end += 1;
                    }
                    end = self.name_end(end);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Keyword);
                }
                b'[' => self.attribute_selector(),
                b'&' | b'*' | b'>' | b'+' | b'~' | b'|' => self.ctx.token(1, Kind::Operator),
                b'%' if self.name_at(self.ctx.pos + 1) => {
                    let end = self.name_end(self.ctx.pos + 1);
                    let start = self.ctx.pos;
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Type);
                }
                b'"' | b'\'' => self.string(),
                b'(' | b')' | b',' => self.ctx.token(1, Kind::Punctuation),
                b'0'..=b'9' => self.number(),
                _ if self.name_at(self.ctx.pos) => {
                    let start = self.ctx.pos;
                    let end = self.name_end(start);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Tag);
                }
                _ => self.ctx.bump_char(),
            }
        }
    }

    fn attribute_selector(&mut self) {
        self.ctx.token(1, Kind::Punctuation);
        while !self.ctx.done() {
            let b = self.ctx.peek();
            match b {
                b']' => {
                    self.ctx.token(1, Kind::Punctuation);
                    return;
                }
                b'\n' | b'{' => return,
                b'"' | b'\'' => self.string(),
                b'=' | b'~' | b'|' | b'^' | b'$' | b'*' => self.ctx.token(1, Kind::Operator),
                _ if self.name_at(self.ctx.pos) => {
                    let start = self.ctx.pos;
                    let end = self.name_end(start);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Attribute);
                }
                _ => self.ctx.bump_char(),
            }
        }
    }

    fn at_rule(&mut self) {
        let start = self.ctx.pos;
        let end = self.name_end(start + 1);
        self.ctx.pos = end;
        let name = &self.ctx.src[start + 1..end];
        // Less variables: `@color: red;`
        if self.dialect == Dialect::Less && self.ctx.next_non_space(end) == b':' {
            self.ctx.emit(start, end, Kind::Variable);
            self.value();
            return;
        }
        self.ctx.emit(start, end, Kind::Keyword);
        let prelude_is_selector = matches!(name, b"extend" | b"at-root");
        if prelude_is_selector {
            self.selector();
        } else {
            self.value();
        }
    }

    fn declaration(&mut self) {
        let start = self.ctx.pos;
        if self.ctx.peek() == b'$' {
            let end = self.name_end(start + 1);
            self.ctx.pos = end;
            self.ctx.emit(start, end, Kind::Variable);
        } else {
            while !self.ctx.done() {
                if self.interpolation() {
                    continue;
                }
                let b = self.ctx.peek();
                if !(is_ident(b) || b == b'-' || b == b'*') {
                    break;
                }
                let s = self.ctx.pos;
                let end = self.name_end(s.max(start));
                let end = if end == s { s + 1 } else { end };
                self.ctx.pos = end;
                let kind = if self.ctx.src[start..].starts_with(b"--") { Kind::Variable } else { Kind::Property };
                self.ctx.emit(s, end, kind);
            }
        }
        self.ctx.skip_spaces();
        if self.ctx.peek() == b':' {
            self.ctx.token(1, Kind::Punctuation);
        }
        self.value();
    }

    /// Lexes a property value or at-rule prelude up to the end of the statement.
    fn value(&mut self) {
        while !self.ctx.done() {
            let b = self.ctx.peek();
            if self.ends_statement(b, Part::Declaration) {
                return;
            }
            if self.comment() || self.interpolation() {
                continue;
            }
            match b {
                b' ' | b'\t' | b'\r' | b'\n' => self.ctx.pos += 1,
                b'"' | b'\'' => self.string(),
                b'0'..=b'9' => self.number(),
                b'.' if self.ctx.peek_at(1).is_ascii_digit() => self.number(),
                b'#' if self.ctx.peek_at(1).is_ascii_alphanumeric() => {
                    let start = self.ctx.pos;
                    let end = self.name_end(start + 1);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Number);
                }
                b'!' if is_ident(self.ctx.peek_at(1)) => {
                    let start = self.ctx.pos;
                    let end = self.name_end(start + 1);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Keyword);
                }
                b'$' | b'@' if self.name_at(self.ctx.pos + 1) => {
                    let start = self.ctx.pos;
                    let end = self.name_end(start + 1);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Variable);
                }
                b'-' if self.ctx.peek_at(1) == b'-' && self.name_at(self.ctx.pos + 2) => {
                    let start = self.ctx.pos;
                    let end = self.name_end(start);
                    self.ctx.pos = end;
                    self.ctx.emit(start, end, Kind::Variable);
                }
                b'-' if self.ctx.peek_at(1).is_ascii_digit() || self.ctx.peek_at(1) == b'.' => self.number(),
                b'(' | b')' | b',' | b':' | b'[' | b']' => self.ctx.token(1, Kind::Punctuation),
                b'+' | b'-' | b'*' | b'/' | b'=' | b'<' | b'>' | b'%' => self.ctx.token(1, Kind::Operator),
                _ if self.name_at(self.ctx.pos) => self.word(),
                _ => self.ctx.bump_char(),
            }
        }
    }

    fn word(&mut self) {
        let start = self.ctx.pos;
        let end = self.name_end(start);
        self.ctx.pos = end;
        let word = &self.ctx.src[start..end];
        if self.ctx.peek() == b'(' {
            self.ctx.emit(start, end, Kind::Function);
            if word.eq_ignore_ascii_case(b"url") {
                // Unquoted URLs may contain `//` and other special characters.
                self.ctx.token(1, Kind::Punctuation);
                self.ctx.skip_spaces();
                if !matches!(self.ctx.peek(), b'"' | b'\'') {
                    let s = self.ctx.pos;
                    while !matches!(self.ctx.peek(), b')' | b'\n' | 0) {
                        self.ctx.pos += 1;
                    }
                    self.ctx.emit_from(s, Kind::String);
                }
            }
            return;
        }
        let kind = match word {
            b"and" | b"or" | b"not" | b"only" | b"from" | b"to" | b"through" | b"in" | b"if" | b"else" | b"each"
            | b"return" => Kind::Keyword,
            b"true" | b"false" | b"null" => Kind::Constant,
            // A property inside an at-rule prelude: `(max-width: 600px)`.
            _ if self.ctx.peek() == b':' && self.ctx.peek_at(1) != b':' => Kind::Property,
            _ => Kind::Constant,
        };
        self.ctx.emit(start, end, kind);
    }

    fn interpolation(&mut self) -> bool {
        let opens = match self.dialect {
            Dialect::Scss | Dialect::Indented => self.ctx.starts_with(b"#{"),
            Dialect::Less => self.ctx.starts_with(b"@{"),
            Dialect::Css => false,
        };
        if !opens {
            return false;
        }
        self.ctx.token(2, Kind::Punctuation);
        let start = self.ctx.pos;
        while !matches!(self.ctx.peek(), b'}' | b'\n' | 0) {
            self.ctx.pos += 1;
        }
        self.ctx.emit_from(start, Kind::Variable);
        if self.ctx.peek() == b'}' {
            self.ctx.token(1, Kind::Punctuation);
        }
        true
    }

    fn string(&mut self) {
        let q = self.ctx.peek();
        let start = self.ctx.pos;
        self.ctx.pos += 1;
        while !self.ctx.done() {
            let b = self.ctx.peek();
            if b == b'\\' {
                self.ctx.pos += 2;
                continue;
            }
            self.ctx.pos += 1;
            if b == q || b == b'\n' {
                break;
            }
        }
        self.ctx.pos = self.ctx.pos.min(self.ctx.len());
        self.ctx.emit_from(start, Kind::String);
    }

    fn number(&mut self) {
        let start = self.ctx.pos;
        if self.ctx.peek() == b'-' {
            self.ctx.pos += 1;
        }
        while self.ctx.peek().is_ascii_digit() || self.ctx.peek() == b'.' {
            self.ctx.pos += 1;
        }
        if self.ctx.peek() | 0x20 == b'e' && self.ctx.peek_at(1).is_ascii_digit() {
            self.ctx.pos += 1;
            while self.ctx.peek().is_ascii_digit() {
                self.ctx.pos += 1;
            }
        }
        // Units such as `px`, `rem` or `%`.
        while self.ctx.peek().is_ascii_alphabetic() || self.ctx.peek() == b'%' {
            self.ctx.pos += 1;
        }
        self.ctx.emit_from(start, Kind::Number);
    }

    fn name_at(&self, i: usize) -> bool {
        let b = self.ctx.at(i);
        is_ident(b) || (b == b'-' && (is_ident(self.ctx.at(i + 1)) || self.ctx.at(i + 1) == b'-'))
    }

    fn name_end(&self, from: usize) -> usize {
        let mut i = from;
        while is_ident(self.ctx.at(i)) || self.ctx.at(i) == b'-' {
            i += 1;
        }
        i
    }
}
