//! Functional languages and the Lisp family.

use super::{Definition, Engine};
use crate::lexer::generic::{features::*, scan_number, Interp, Lexer, Prefix, Spec, Str};
use crate::lexer::{is_ident, is_ident_start, Ctx};
use crate::Kind;

pub static LANGUAGES: &[&Definition] = &[
    &HASKELL, &ELM, &PURESCRIPT, &IDRIS, &AGDA, &UNISON, &LEAN, &COQ, &OCAML, &FSHARP, &SML, &REASON, &CLOJURE,
    &SCHEME, &RACKET, &COMMONLISP, &EMACSLISP, &FENNEL, &JANET, &HY, &ERLANG, &GLEAM, &PROLOG, &ROC, &GRAIN, &KOKA,
];

// Shared helpers

const LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_";

/// Offset just past the UTF-8 character at `i`.
fn after_char(ctx: &Ctx, i: usize) -> usize {
    if i >= ctx.len() {
        return i;
    }
    let mut j = i + 1;
    while j < ctx.len() && (ctx.src[j] & 0xC0) == 0x80 {
        j += 1;
    }
    j
}

/// The rest of the line from `from`, capped so unterminated constructs stay cheap.
fn line_window<'s>(ctx: &Ctx<'s, '_>, from: usize) -> &'s [u8] {
    let window = &ctx.src[from..(from + 256).min(ctx.len())];
    let end = window.iter().position(|&b| b == b'\n').unwrap_or(window.len());
    &window[..end]
}

fn col0(ctx: &Ctx, pos: usize) -> bool {
    pos == 0 || ctx.src[pos - 1] == b'\n'
}

/// Haskell style operator symbols.
fn is_symbol(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'*'
            | b'+'
            | b'.'
            | b'/'
            | b'<'
            | b'='
            | b'>'
            | b'?'
            | b'@'
            | b'\\'
            | b'^'
            | b'|'
            | b'~'
            | b':'
            | b'-'
    )
}

/// A character literal such as `'a'`, `'\n'` or `'\x41'` at the cursor.
fn char_literal(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let escaped = ctx.peek_at(1) == b'\\';
    let close = if escaped {
        let from = start + 3;
        match ctx.src.get(from..).and_then(|r| r.iter().take(10).position(|&b| b == b'\'' || b == b'\n')) {
            Some(i) if ctx.at(from + i) == b'\'' => from + i,
            _ => return false,
        }
    } else {
        if matches!(ctx.peek_at(1), 0 | b'\'' | b'\n') {
            return false;
        }
        let i = after_char(ctx, start + 1);
        if ctx.at(i) != b'\'' {
            return false;
        }
        i
    };
    ctx.emit(start, start + 1, Kind::String);
    ctx.emit(start + 1, close, if escaped { Kind::Escape } else { Kind::String });
    ctx.emit(close, close + 1, Kind::String);
    ctx.pos = close + 1;
    lx.mark_value();
    true
}

// Haskell family

/// `--` starts a comment unless the dashes belong to an operator (`-->`, `|--`).
fn dash(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    if ctx.peek_at(1) != b'-' {
        return false;
    }
    let start = ctx.pos;
    let mut i = start + 2;
    while ctx.at(i) == b'-' {
        i += 1;
    }
    if is_symbol(ctx.at(i)) || (start > 0 && is_symbol(ctx.src[start - 1])) {
        while is_symbol(ctx.at(i)) {
            i += 1;
        }
        ctx.pos = i;
        ctx.emit(start, i, Kind::Operator);
        lx.mark_operator();
    } else {
        ctx.skip_line();
        ctx.emit_from(start, Kind::Comment);
    }
    true
}

/// `{-# LANGUAGE ... #-}` pragmas.
fn pragma(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    if !ctx.starts_with(b"{-#") {
        return false;
    }
    let start = ctx.pos;
    ctx.pos = ctx.find(start + 3, b"#-}").map_or(ctx.len(), |p| p + 3);
    ctx.emit_from(start, Kind::Meta);
    true
}

/// `%default total` style directives at the start of a line.
fn directive(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    if !lx.ctx.at_line_start(start) || !lx.ctx.peek_at(1).is_ascii_alphabetic() {
        return false;
    }
    let end = lx.scan_ident(start + 1);
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Meta);
    true
}

/// Unison character literals: `?a`, `?\n`.
fn question_char(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    if next == 0 || next.is_ascii_whitespace() {
        return false;
    }
    let end = if next == b'\\' { after_char(ctx, start + 2) } else { after_char(ctx, start + 1) };
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    lx.mark_value();
    true
}

/// Qualified names such as `Data.Map.lookup`: the module path is a type and
/// the final lowercase name stays plain (or a function when called).
fn qualified(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let mut end = lx.scan_ident(start);
    let mut dotted = false;
    while lx.ctx.at(end) == b'.' && lx.ctx.at(end + 1).is_ascii_uppercase() {
        end = lx.scan_ident(end + 1);
        dotted = true;
    }
    let next = lx.ctx.at(end + 1);
    let member = lx.ctx.at(end) == b'.' && (next.is_ascii_lowercase() || next == b'_');
    if !dotted && !member {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Type);
    lx.mark_value();
    if member {
        lx.ctx.token(1, Kind::Punctuation);
        let e = lx.scan_ident(end + 1);
        lx.ctx.pos = e;
        let called = lx.spec.has(CALLS) && lx.ctx.next_non_space(e) == b'(';
        lx.ctx.emit(end + 1, e, if called { Kind::Function } else { Kind::Text });
    }
    true
}

/// Names declared at the start of a line: `name :: Type` (or `name : Type`),
/// record fields, and with `top` set any definition in column 0. With
/// `lambda` set, `name = \x ->` and `name = |x|` are functions too.
fn declaration(lx: &mut Lexer, sig: &[u8], top: bool, lambda: bool) -> bool {
    let start = lx.ctx.pos;
    let before = if start > 0 { lx.ctx.src[start - 1] } else { b'\n' };
    if !matches!(before, b' ' | b'\t' | b'\n' | b'{' | b',') {
        return false;
    }
    let line_start = lx.ctx.at_line_start(start);
    let field = matches!(lx.ctx.prev_non_space(start), b'{' | b',');
    if !line_start && !field {
        return false;
    }
    let end = lx.scan_ident(start);
    let after = lx.ctx.skip_spaces_from(end);
    let has_sig = lx.ctx.starts_with_at(after, sig) && !is_symbol(lx.ctx.at(after + sig.len()));
    let next = lx.ctx.at(after);
    // `a <+> b = ...` and ``a `op` b = ...`` define operators, not `a`.
    let infix = next == b'`' || (is_symbol(next) && !(next == b'=' && !is_symbol(lx.ctx.at(after + 1))));
    let assigned_lambda = lambda
        && line_start
        && next == b'='
        && lx.ctx.at(after + 1) != b'='
        && matches!(lx.ctx.next_non_space(after + 1), b'\\' | b'|');
    let kind = if has_sig && field {
        Kind::Property
    } else if has_sig || assigned_lambda || (top && !infix && col0(lx.ctx, start)) {
        Kind::Function
    } else {
        return false;
    };
    lx.ctx.pos = end;
    let kind = match lx.classify(start, end) {
        Kind::Keyword => Kind::Keyword,
        _ => kind,
    };
    lx.ctx.emit(start, end, kind);
    true
}

fn fp_hook(lx: &mut Lexer, sig: &[u8]) -> bool {
    match lx.ctx.peek() {
        b'-' => dash(lx),
        b'{' => pragma(lx),
        b'\'' => {
            if char_literal(lx) {
                return true;
            }
            // Promotion ticks and Template Haskell quotes: 'Just, ''Maybe.
            let n = if lx.ctx.peek_at(1) == b'\'' { 2 } else { 1 };
            lx.ctx.token(n, Kind::Operator);
            lx.mark_operator();
            true
        }
        b'%' => directive(lx),
        b'?' => question_char(lx),
        b'A'..=b'Z' => qualified(lx),
        _ => declaration(lx, sig, true, false),
    }
}

fn haskell_hook(lx: &mut Lexer) -> bool {
    fp_hook(lx, b"::")
}

fn colon_hook(lx: &mut Lexer) -> bool {
    fp_hook(lx, b":")
}

const HS_OPERATORS: &str = "+-*/%=&|^!<>~?:.$\\@#";

const HASKELL_KEYWORDS: &[&str] = &[
    "case", "class", "data", "default", "deriving", "do", "else", "family", "forall", "foreign", "hiding", "if",
    "import", "in", "infix", "infixl", "infixr", "instance", "let", "mdo", "module", "newtype", "of", "qualified",
    "then", "type", "where", "∀",
];

const HASKELL_BUILTINS: &[&str] = &[
    "map", "filter", "foldr", "foldl", "foldl'", "fmap", "pure", "return", "print", "putStrLn", "putStr", "getLine",
    "show", "read", "error", "undefined", "fst", "snd", "head", "tail", "length", "null", "concat", "concatMap", "zip",
    "zipWith", "sum", "product", "maximum", "minimum", "reverse", "lookup", "elem", "not", "id", "const", "flip",
    "mapM", "mapM_", "forM_", "traverse", "sequence", "when", "unless", "seq", "maybe", "either", "otherwise",
];

static HASKELL_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("{-", "-}")],
    nested_comments: true,
    strings: &[Str::quoted("\"")],
    keywords: HASKELL_KEYWORDS,
    builtins: HASKELL_BUILTINS,
    constants: &["True", "False"],
    ident_chars: "'",
    operators: HS_OPERATORS,
    features: CAPITALIZED_TYPES,
    hook: Some(haskell_hook),
    hook_chars: "-{'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static HASKELL: Definition = Definition::new("haskell", "Haskell", Engine::Generic(&HASKELL_SPEC))
    .aliases(&["hs"])
    .extensions(&["hs", "hs-boot", "hsig"]);

static ELM_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("{-", "-}")],
    nested_comments: true,
    strings: &[Str::quoted("\"\"\"").multiline(), Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "if", "then", "else", "case", "of", "let", "in", "type", "alias", "module", "exposing", "import", "as", "port",
        "where", "effect",
    ],
    builtins: &[
        "identity", "always", "not", "negate", "modBy", "remainderBy", "round", "floor", "ceiling", "truncate",
        "toFloat", "min", "max", "compare", "abs", "sqrt", "xor",
    ],
    constants: &["True", "False"],
    operators: "+-*/%=&|^!<>~?:\\",
    features: CAPITALIZED_TYPES | MEMBERS,
    hook: Some(colon_hook),
    hook_chars: "-ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static ELM: Definition = Definition::new("elm", "Elm", Engine::Generic(&ELM_SPEC)).extensions(&["elm"]);

static PURESCRIPT_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("{-", "-}")],
    nested_comments: true,
    strings: &[Str::quoted("\"\"\"").raw().multiline(), Str::quoted("\"")],
    keywords: &[
        "ado", "as", "case", "class", "data", "derive", "do", "else", "forall", "foreign", "hiding", "if", "import",
        "in", "infix", "infixl", "infixr", "instance", "let", "module", "newtype", "of", "then", "type", "where", "∀",
    ],
    builtins: &[
        "map", "filter", "foldl", "foldr", "pure", "bind", "discard", "show", "log", "logShow", "identity", "const",
        "flip", "not", "otherwise", "unit", "void", "when", "unless", "traverse_", "for_",
    ],
    constants: &["true", "false"],
    ident_chars: "'",
    operators: "+-*/%=&|^!<>~?:$\\@#",
    features: CAPITALIZED_TYPES | MEMBERS,
    hook: Some(haskell_hook),
    hook_chars: "-'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static PURESCRIPT: Definition = Definition::new("purescript", "PureScript", Engine::Generic(&PURESCRIPT_SPEC))
    .aliases(&["purs"])
    .extensions(&["purs"]);

static IDRIS_SPEC: Spec = Spec {
    line_comments: &["|||", "--"],
    block_comments: &[("{-", "-}")],
    nested_comments: true,
    strings: &[Str::quoted("\"\"\"").multiline(), Str::quoted("\"")],
    keywords: &[
        "module", "import", "namespace", "public", "export", "private", "data", "record", "interface",
        "implementation", "where", "let", "in", "do", "case", "of", "if", "then", "else", "total", "partial",
        "covering", "mutual", "parameters", "using", "auto", "implicit", "default", "with", "rewrite", "impossible",
        "forall", "constructor", "infix", "infixl", "infixr",
    ],
    builtins: &[
        "map", "filter", "foldr", "foldl", "putStrLn", "printLn", "show", "pure", "length", "reverse", "the", "cast",
        "not", "id", "const",
    ],
    constants: &["True", "False"],
    ident_chars: "'",
    operators: HS_OPERATORS,
    features: CAPITALIZED_TYPES,
    hook: Some(colon_hook),
    hook_chars: "-{'%ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static IDRIS: Definition =
    Definition::new("idris", "Idris", Engine::Generic(&IDRIS_SPEC)).aliases(&["idr", "idris2"]).extensions(&["idr"]);

/// Agda names may contain most symbols: `_+_`, `+-comm`, `≤-refl`. A name
/// made only of symbols is an operator.
fn agda_hook(lx: &mut Lexer) -> bool {
    let b = lx.ctx.peek();
    if b == b'-' && lx.ctx.peek_at(1) == b'-' {
        return dash(lx);
    }
    if b == b'{' {
        return pragma(lx);
    }
    if !b.is_ascii_punctuation() || b == b'_' || b == b'\'' {
        return colon_hook(lx);
    }
    let start = lx.ctx.pos;
    let end = lx.scan_ident(start);
    if lx.ctx.src[start..end].iter().all(|b| b.is_ascii_punctuation()) {
        lx.ctx.pos = end;
        lx.ctx.emit(start, end, Kind::Operator);
        lx.mark_operator();
        return true;
    }
    if declaration(lx, b":", true, false) {
        return true;
    }
    lx.ctx.pos = end;
    let kind = lx.classify(start, end);
    lx.ctx.emit(start, end, kind);
    true
}

static AGDA_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("{-", "-}")],
    nested_comments: true,
    strings: &[Str::quoted("\"")],
    keywords: &[
        "abstract", "coinductive", "constructor", "data", "do", "eta-equality", "field", "forall", "hiding", "import",
        "in", "inductive", "infix", "infixl", "infixr", "instance", "let", "macro", "module", "mutual", "open",
        "overlap", "pattern", "postulate", "primitive", "private", "public", "record", "renaming", "rewrite", "syntax",
        "to", "using", "variable", "where", "with", "λ", "∀", "→",
    ],
    types: &["Set", "Prop", "Setω"],
    ident_chars: "'-+*=<>!?/\\|^~:&$%,",
    ident_start: "+*=<>!?/\\|^~&$%-",
    operators: HS_OPERATORS,
    features: CAPITALIZED_TYPES,
    hook: Some(agda_hook),
    hook_chars: "-{'+*=<>!?/\\|^~&$%ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static AGDA: Definition = Definition::new("agda", "Agda", Engine::Generic(&AGDA_SPEC)).extensions(&["agda"]);

static UNISON_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("{-", "-}")],
    nested_comments: true,
    strings: &[
        Str::quoted("\"\"\"").raw().multiline(),
        Str::quoted("\""),
        Str::new("{{", "}}").raw().multiline().kind(Kind::Comment),
    ],
    keywords: &[
        "ability", "cases", "do", "else", "forall", "handle", "if", "let", "match", "namespace", "structural", "then",
        "type", "unique", "use", "where", "with", "and", "or", "∀",
    ],
    builtins: &["printLine", "toText", "bug", "todo", "abort", "watch"],
    constants: &["true", "false"],
    ident_chars: "'",
    operators: "+-*/%=&|^!<>~:.$\\@#",
    features: CAPITALIZED_TYPES,
    hook: Some(colon_hook),
    hook_chars: "-'?ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static UNISON: Definition = Definition::new("unison", "Unison", Engine::Generic(&UNISON_SPEC)).extensions(&["u"]);

// Lean and Coq

/// Character literals and `@[attribute]` lists.
fn lean_hook(lx: &mut Lexer) -> bool {
    match lx.ctx.peek() {
        b'\'' => char_literal(lx),
        b'@' if lx.ctx.peek_at(1) == b'[' => {
            let ctx = &mut *lx.ctx;
            let start = ctx.pos;
            let end = line_window(ctx, start).iter().position(|&b| b == b']').map_or(start + 2, |i| start + i + 1);
            ctx.pos = end;
            ctx.emit(start, end, Kind::Meta);
            true
        }
        _ => false,
    }
}

static LEAN_SPEC: Spec = Spec {
    line_comments: &["--"],
    block_comments: &[("/-", "-/")],
    nested_comments: true,
    strings: &[Str::quoted("\"")],
    string_prefixes: &[
        Prefix::interp(b's', Interp::Brace),
        Prefix::interp(b'm', Interp::Brace),
        Prefix::interp(b'f', Interp::Brace),
        Prefix::plain(b'!'),
    ],
    keywords: &[
        "abbrev", "at", "by", "calc", "deriving", "do", "else", "end", "example", "export", "extends", "for", "from",
        "fun", "have", "if", "import", "in", "instance", "let", "match", "mut", "mutual", "namespace", "noncomputable",
        "open", "partial", "private", "protected", "return", "section", "set_option", "show", "suffices", "then",
        "universe", "unsafe", "variable", "where", "with", "λ", "∀", "∃",
    ],
    types: &["Prop", "Type", "Sort"],
    builtins: &[
        "intro", "intros", "apply", "exact", "rfl", "simp", "rw", "induction", "cases", "constructor", "assumption",
        "contradiction", "omega", "decide", "norm_num", "linarith", "ring", "sorry", "trivial", "exists", "use",
        "refine", "obtain", "rcases", "unfold", "ext", "funext", "aesop", "specialize", "exfalso", "left", "right",
    ],
    constants: &["true", "false", "True", "False"],
    function_keywords: &["def", "theorem", "lemma", "axiom", "opaque"],
    type_keywords: &["structure", "class", "inductive"],
    ident_chars: "'",
    ident_suffix: "!?",
    features: CAPITALIZED_TYPES | PREPROCESSOR,
    hook: Some(lean_hook),
    hook_chars: "'@",
    ..Spec::DEFAULT
};

pub static LEAN: Definition =
    Definition::new("lean", "Lean", Engine::Generic(&LEAN_SPEC)).aliases(&["lean4"]).extensions(&["lean"]);

static COQ_SPEC: Spec = Spec {
    block_comments: &[("(*", "*)")],
    nested_comments: true,
    strings: &[Str::quoted("\"").raw().doubled().multiline()],
    keywords: &[
        "Require", "Import", "Export", "From", "Module", "End", "Section", "Variable", "Variables", "Hypothesis",
        "Context", "Proof", "Qed", "Defined", "Admitted", "Notation", "Infix", "Check", "Compute", "Eval", "Print",
        "Search", "Arguments", "Implicit", "Set", "Unset", "Open", "Scope", "Local", "Global", "Program", "Axiom",
        "Parameter", "Hint", "Resolve", "Opaque", "Transparent", "Existing", "match", "with", "end", "fun", "forall",
        "exists", "let", "in", "if", "then", "else", "return", "as", "fix", "cofix", "struct",
    ],
    types: &["nat", "bool", "list", "Prop", "Type", "option", "unit", "prod", "sum", "Z", "string"],
    builtins: &[
        "intros", "intro", "simpl", "reflexivity", "rewrite", "apply", "exact", "induction", "destruct", "auto",
        "eauto", "lia", "omega", "unfold", "split", "left", "right", "assumption", "discriminate", "inversion",
        "subst", "constructor", "trivial", "congruence", "f_equal", "specialize", "generalize", "assert", "now",
        "repeat", "try", "exfalso", "contradiction", "symmetry", "transitivity", "tauto", "easy", "exists",
    ],
    constants: &["true", "false"],
    function_keywords: &[
        "Definition", "Fixpoint", "CoFixpoint", "Theorem", "Lemma", "Example", "Corollary", "Remark", "Fact",
        "Proposition", "Ltac", "Let", "Instance",
    ],
    type_keywords: &["Inductive", "CoInductive", "Record", "Structure", "Class", "Variant"],
    ident_chars: "'",
    operators: "+-*/%=&|^!<>~?:\\@",
    ..Spec::DEFAULT
};

pub static COQ: Definition =
    Definition::new("coq", "Coq", Engine::Generic(&COQ_SPEC)).aliases(&["rocq", "gallina"]).extensions(&["coq"]);

// ML family

const BINDING_MODIFIERS: &[&[u8]] = &[b"rec", b"inline", b"mutable", b"private", b"internal", b"public", b"nonrec"];

/// `let f x = ...` names a function, `let x = ...` a value. F# `member
/// this.Name` names a method or property.
fn binding(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let end = lx.scan_ident(start);
    let word = &lx.ctx.src[start..end];
    let member = matches!(word, b"member" | b"override" | b"default");
    if !member && word != b"let" && word != b"and" {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Keyword);
    lx.mark_operator();
    loop {
        let mut s = lx.ctx.skip_spaces_from(lx.ctx.pos);
        let mut e = lx.scan_ident(s);
        if lx.ctx.starts_with_at(s, b"``") {
            // F# ``name with spaces``.
            if let Some(i) = line_window(lx.ctx, s + 2).windows(2).position(|w| w == b"``") {
                e = s + 2 + i + 2;
                lx.ctx.pos = e;
                lx.ctx.emit(s, e, Kind::Function);
            }
            return true;
        }
        if e == s {
            return true;
        }
        let w = &lx.ctx.src[s..e];
        if BINDING_MODIFIERS.contains(&w) || (member && w == b"val") {
            lx.ctx.pos = e;
            lx.ctx.emit(s, e, Kind::Keyword);
            continue;
        }
        let first = lx.ctx.src[s];
        if member && lx.ctx.at(e) == b'.' && is_ident_start(lx.ctx.at(e + 1)) {
            // The self identifier, then the member name.
            lx.ctx.emit(s, e, Kind::Variable);
            lx.ctx.emit(e, e + 1, Kind::Punctuation);
            s = e + 1;
            e = lx.scan_ident(s);
        } else if !member && !(first.is_ascii_lowercase() || first == b'_') {
            return true;
        }
        lx.ctx.pos = e;
        let kind = match lx.classify(s, e) {
            Kind::Keyword => Kind::Keyword,
            _ => match lx.ctx.next_non_space(e) {
                b'=' | b':' | b',' | b')' | b'|' | b';' | b'\n' | b'\r' | 0 if member => Kind::Property,
                b'=' | b':' | b',' | b')' | b'|' | b';' | b'\n' | b'\r' | 0 => Kind::Text,
                _ => Kind::Function,
            },
        };
        lx.ctx.emit(s, e, kind);
        return true;
    }
}

/// OCaml `[@attr]` and F# `[<Attr>]` attributes.
fn ml_attribute(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    match ctx.peek_at(1) {
        b'@' => {
            let mut i = start + 2;
            while ctx.at(i) == b'@' {
                i += 1;
            }
            while is_ident(ctx.at(i)) || ctx.at(i) == b'.' {
                i += 1;
            }
            ctx.pos = i;
            ctx.emit(start, i, Kind::Meta);
            lx.nested(b']');
            if lx.ctx.peek() == b']' {
                lx.ctx.token(1, Kind::Meta);
            }
            true
        }
        b'<' => match line_window(ctx, start + 2).windows(2).position(|w| w == b">]") {
            Some(i) => {
                ctx.pos = start + 2 + i + 2;
                ctx.emit_from(start, Kind::Meta);
                true
            }
            None => false,
        },
        _ => false,
    }
}

/// OCaml quoted strings: `{|raw|}` and `{id|raw|id}`.
fn quoted_string(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let src = ctx.src;
    let start = ctx.pos;
    let mut i = start + 1;
    while ctx.at(i).is_ascii_lowercase() || ctx.at(i) == b'_' {
        i += 1;
    }
    if ctx.at(i) != b'|' {
        return false;
    }
    let id = &src[start + 1..i];
    let mut from = i + 1;
    let end = loop {
        match ctx.find(from, b"|") {
            None => break ctx.len(),
            Some(p) if src[p + 1..].starts_with(id) && ctx.at(p + 1 + id.len()) == b'}' => break p + 2 + id.len(),
            Some(p) => from = p + 1,
        }
    };
    ctx.pos = end;
    ctx.emit(start, end, Kind::String);
    lx.mark_value();
    true
}

static SML_CHAR: Str = Str::quoted("\"");

fn ml_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let next = lx.ctx.peek_at(1);
    match lx.ctx.peek() {
        b'\'' => {
            if char_literal(lx) {
                return true;
            }
            if !is_ident_start(next) {
                return false;
            }
            // Type variable: 'a, 'T.
            let end = lx.scan_ident(start + 1);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Type);
            lx.mark_value();
            true
        }
        b'`' => {
            if next == b'`' {
                // F# ``identifier with spaces``.
                let Some(i) = line_window(lx.ctx, start + 2).windows(2).position(|w| w == b"``") else {
                    return false;
                };
                lx.ctx.pos = start + 2 + i + 2;
                lx.ctx.emit_from(start, Kind::Text);
                lx.mark_value();
                return true;
            }
            if !is_ident_start(next) {
                return false;
            }
            // OCaml polymorphic variant.
            let end = lx.scan_ident(start + 1);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Constant);
            lx.mark_value();
            true
        }
        b'{' => quoted_string(lx),
        b'[' => ml_attribute(lx),
        b'~' | b'?' if next.is_ascii_lowercase() || next == b'_' => {
            // Labelled and optional arguments.
            let end = lx.scan_ident(start + 1);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Property);
            true
        }
        b'(' if lx.ctx.starts_with(b"(*)") => {
            lx.ctx.token(1, Kind::Punctuation);
            lx.ctx.token(1, Kind::Operator);
            lx.ctx.token(1, Kind::Punctuation);
            lx.mark_value();
            true
        }
        b'#' if next == b'"' => {
            // SML character literal: #"a".
            lx.ctx.pos = start + 1;
            lx.string(start, &SML_CHAR, false, Interp::None);
            true
        }
        b'#' if is_ident_start(next) => {
            // SML record selector: #name.
            let end = lx.scan_ident(start + 1);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Property);
            true
        }
        b'.' if start > 0 && matches!(lx.ctx.src[start - 1], b'+' | b'-' | b'*' | b'/') => {
            // Float operators: +. -. *. /.
            lx.ctx.token(1, Kind::Operator);
            lx.mark_operator();
            true
        }
        b'l' | b'a' | b'm' | b'o' | b'd' => binding(lx),
        _ => false,
    }
}

const ML_TYPES: &[&str] = &[
    "int", "float", "string", "bool", "char", "unit", "list", "array", "option", "ref", "exn", "bytes", "int32",
    "int64", "nativeint", "format", "seq", "lazy_t",
];

static OCAML_SPEC: Spec = Spec {
    block_comments: &[("(*", "*)")],
    nested_comments: true,
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "and", "as", "assert", "asr", "begin", "class", "constraint", "do", "done", "downto", "else", "end",
        "exception", "for", "fun", "function", "functor", "if", "in", "include", "inherit", "initializer", "land",
        "lazy", "let", "lor", "lsl", "lsr", "lxor", "match", "mod", "module", "mutable", "new", "nonrec", "object",
        "of", "open", "or", "private", "rec", "sig", "struct", "then", "to", "try", "val", "virtual", "when", "while",
        "with",
    ],
    types: ML_TYPES,
    builtins: &[
        "print_endline", "print_string", "print_int", "print_newline", "prerr_endline", "failwith", "invalid_arg",
        "raise", "ignore", "fst", "snd", "not", "incr", "decr", "string_of_int", "int_of_string", "float_of_int",
        "compare", "min", "max",
    ],
    constants: &["true", "false"],
    function_keywords: &["method", "external"],
    type_keywords: &["type"],
    ident_chars: "'",
    operators: "+-*/%=&|^!<>~?:@$",
    features: CAPITALIZED_TYPES,
    hook: Some(ml_hook),
    hook_chars: "'`{[~?la.",
    ..Spec::DEFAULT
};

pub static OCAML: Definition =
    Definition::new("ocaml", "OCaml", Engine::Generic(&OCAML_SPEC)).aliases(&["ml"]).extensions(&["ml", "mli", "mll"]);

static FSHARP_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("(*", "*)")],
    nested_comments: true,
    strings: &[
        Str::new("$\"\"\"", "\"\"\"").raw().multiline().interp(Interp::Brace),
        Str::new("$\"", "\"").multiline().interp(Interp::Brace),
        Str::new("@\"", "\"").raw().doubled().multiline(),
        Str::quoted("\"\"\"").raw().multiline(),
        Str::quoted("\"").multiline(),
    ],
    keywords: &[
        "abstract", "and", "as", "assert", "base", "begin", "class", "default", "delegate", "do", "done", "downcast",
        "downto", "elif", "else", "end", "exception", "extern", "for", "fun", "function", "global", "if", "in",
        "inherit", "inline", "interface", "internal", "lazy", "let", "match", "member", "module", "mutable",
        "namespace", "new", "not", "of", "open", "or", "override", "private", "public", "rec", "return", "select",
        "sig", "static", "struct", "then", "to", "try", "upcast", "use", "val", "void", "when", "while", "with",
        "yield", "let!", "use!", "do!", "return!", "yield!", "match!",
    ],
    types: &[
        "int", "string", "float", "bool", "unit", "char", "byte", "sbyte", "int16", "uint16", "int32", "uint32",
        "int64", "uint64", "decimal", "float32", "double", "obj", "exn", "list", "option", "voption", "seq", "array",
        "nativeint", "bigint",
    ],
    builtins: &[
        "printfn", "printf", "sprintf", "eprintfn", "failwith", "failwithf", "invalidArg", "ignore", "id", "fst",
        "snd", "raise", "box", "unbox", "nameof", "typeof", "async", "task",
    ],
    constants: &["true", "false", "null"],
    type_keywords: &["type"],
    ident_chars: "'",
    ident_suffix: "!",
    operators: "+-*/%=&|^!<>~?:@$",
    features: CAPITALIZED_TYPES | MEMBERS | PREPROCESSOR,
    hook: Some(ml_hook),
    hook_chars: "'`[(lamod",
    ..Spec::DEFAULT
};

pub static FSHARP: Definition = Definition::new("fsharp", "F#", Engine::Generic(&FSHARP_SPEC))
    .aliases(&["fs", "f#", "fsx"])
    .extensions(&["fs", "fsi", "fsx", "fsscript"]);

static SML_SPEC: Spec = Spec {
    block_comments: &[("(*", "*)")],
    nested_comments: true,
    strings: &[Str::quoted("\"")],
    keywords: &[
        "abstype", "and", "andalso", "as", "case", "do", "else", "end", "fn", "handle", "if", "in", "include", "infix",
        "infixr", "let", "local", "nonfix", "of", "op", "open", "orelse", "raise", "rec", "sharing", "sig", "struct",
        "then", "val", "where", "while", "with", "withtype",
    ],
    types: &[
        "int", "real", "string", "char", "bool", "unit", "list", "option", "exn", "word", "ref", "array", "vector",
        "order",
    ],
    builtins: &[
        "print", "map", "foldl", "foldr", "rev", "length", "hd", "tl", "null", "app", "concat", "implode", "explode",
        "size", "ord", "chr", "not", "ignore", "real", "floor", "round", "trunc", "abs", "valOf", "isSome",
    ],
    constants: &["true", "false", "nil", "NONE", "SOME"],
    function_keywords: &["fun"],
    type_keywords: &["datatype", "type", "eqtype", "structure", "signature", "functor", "exception"],
    ident_chars: "'",
    operators: "+-*/%=&|^!<>~?:@$",
    features: CAPITALIZED_TYPES,
    hook: Some(ml_hook),
    hook_chars: "'#",
    ..Spec::DEFAULT
};

pub static SML: Definition = Definition::new("sml", "Standard ML", Engine::Generic(&SML_SPEC))
    .aliases(&["standardml"])
    .extensions(&["sml", "sig"]);

static REASON_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "and", "as", "assert", "constraint", "downto", "else", "exception", "external", "for", "fun", "if", "in",
        "include", "lazy", "let", "module", "mutable", "nonrec", "of", "open", "private", "rec", "sig", "struct",
        "switch", "to", "try", "val", "when", "while", "with",
    ],
    types: ML_TYPES,
    builtins: &["print_endline", "print_string", "failwith", "raise", "ignore", "fst", "snd", "not"],
    constants: &["true", "false"],
    type_keywords: &["type"],
    ident_chars: "'",
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS | ARROW_FUNCTIONS | JSX_ELEMENTS,
    hook: Some(ml_hook),
    hook_chars: "'`{[~?",
    ..Spec::DEFAULT
};

pub static REASON: Definition = Definition::new("reason", "Reason", Engine::Generic(&REASON_SPEC))
    .aliases(&["re", "reasonml"])
    .extensions(&["re", "rei"]);

// Lisp family

/// Identifier bytes shared by the Lisp dialects.
const LISP_IDENT: &str = "-+*/<>=!?.%&$";

/// The datum after `#_` or `#;`, commented out as a whole.
fn datum_comment(ctx: &mut Ctx) {
    let start = ctx.pos;
    let mut i = ctx.skip_spaces_from(start + 2);
    if matches!(ctx.at(i), b'(' | b'[' | b'{') {
        let mut depth = 0usize;
        while i < ctx.len() {
            match ctx.src[i] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                b'"' => {
                    i += 1;
                    while i < ctx.len() && ctx.src[i] != b'"' {
                        i += if ctx.src[i] == b'\\' { 2 } else { 1 };
                    }
                }
                b'\\' => i += 1,
                _ => {}
            }
            i += 1;
        }
    } else {
        while i < ctx.len() && !matches!(ctx.src[i], b' ' | b'\t' | b'\n' | b'\r' | b')' | b']' | b'}') {
            i += 1;
        }
    }
    ctx.pos = i.min(ctx.len());
    ctx.emit_from(start, Kind::Comment);
}

/// A character literal body starting at `i`: one character, or a name such
/// as `newline` or `u00e9`. Returns its end.
fn char_name(ctx: &Ctx, i: usize) -> usize {
    let first = ctx.at(i);
    let mut end = after_char(ctx, i);
    if first.is_ascii_alphabetic() {
        while ctx.at(end).is_ascii_alphanumeric() || ctx.at(end) == b'-' {
            end += 1;
        }
    }
    end
}

/// Reader syntax that starts with `#`.
fn lisp_hash(lx: &mut Lexer) -> bool {
    let ctx = &mut *lx.ctx;
    let start = ctx.pos;
    let next = ctx.peek_at(1);
    let word_end = |ctx: &Ctx, from: usize| {
        let mut e = from;
        while is_ident(ctx.at(e)) || matches!(ctx.at(e), b'-' | b'/' | b'.') {
            e += 1;
        }
        e
    };
    match next {
        // Block comments, strings, regexes and bracket strings have their own rules.
        b'|' | b'"' | b'[' | 0 => return false,
        b'_' | b';' => datum_comment(ctx),
        b'\\' if start + 2 < ctx.len() => {
            let end = char_name(ctx, start + 2);
            ctx.pos = end;
            ctx.emit(start, end, Kind::String);
        }
        b':' => {
            let end = lx.scan_ident(start + 2);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Constant);
        }
        b'\'' if is_ident_start(ctx.at(start + 2)) => {
            // Function reference: #'car.
            ctx.token(2, Kind::Operator);
            let end = lx.scan_ident(start + 2);
            lx.ctx.pos = end;
            lx.ctx.emit(start + 2, end, Kind::Function);
        }
        b'\'' | b'`' | b',' | b'*' | b'@' => {
            let mut end = start + 2;
            while matches!(ctx.at(end), b'@' | b'*') {
                end += 1;
            }
            ctx.pos = end;
            ctx.emit(start, end, Kind::Operator);
            lx.mark_operator();
            return true;
        }
        b'(' | b'{' => {
            ctx.token(1, Kind::Punctuation);
            lx.mark_operator();
            return true;
        }
        b'#' => {
            // ##Inf, ##NaN
            let end = word_end(ctx, start + 2);
            ctx.pos = end;
            ctx.emit(start, end, Kind::Constant);
        }
        b'+' | b'-' | b'?' => {
            // Feature expressions and reader conditionals: #+sbcl, #?(:clj ...).
            let mut end = start + 2;
            if ctx.at(end) == b'@' {
                end += 1;
            }
            end = word_end(ctx, end);
            ctx.pos = end;
            ctx.emit(start, end, Kind::Meta);
        }
        b'r' | b'p' if ctx.at(start + 2) == b'x' && ctx.at(start + 3) == b'"' => return false,
        _ if next.is_ascii_alphabetic() => {
            let end = word_end(ctx, start + 1);
            let word = &ctx.src[start + 1..end];
            let radix = matches!(next | 0x20, b'x' | b'b' | b'o' | b'e' | b'i' | b'd');
            let kind = if matches!(word, b"t" | b"f" | b"true" | b"false") {
                Kind::Constant
            } else if radix && word[1..].iter().all(|b| b.is_ascii_hexdigit() || *b == b'.') && word.len() > 1 {
                Kind::Number
            } else if word == b"lang" && ctx.at_line_start(start) {
                ctx.pos = ctx.line_end(start);
                ctx.emit_from(start, Kind::Meta);
                return true;
            } else {
                // Tagged literals and reader macros: #inst, #js, #hash, #p.
                Kind::Meta
            };
            ctx.pos = end;
            ctx.emit(start, end, kind);
        }
        _ => return false,
    }
    lx.mark_value();
    true
}

/// Numbers (with signs and ratios), `*earmuffs*` and `+constants+`.
fn lisp_word(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let b = lx.ctx.peek();
    let digit_at = if matches!(b, b'+' | b'-') { 1 } else { 0 };
    if lx.ctx.peek_at(digit_at).is_ascii_digit() {
        lx.ctx.pos = start + digit_at;
        scan_number(lx.ctx, false);
        if lx.ctx.peek() == b'/' && lx.ctx.peek_at(1).is_ascii_digit() {
            lx.ctx.pos += 1;
            scan_number(lx.ctx, false);
        }
        if !lx.is_ident_byte(lx.ctx.peek()) {
            lx.ctx.emit_from(start, Kind::Number);
            lx.mark_value();
            return true;
        }
        // A symbol that starts with digits, such as `1+`.
        let end = lx.scan_ident(start);
        lx.ctx.pos = end;
        let kind = lx.classify(start, end);
        lx.ctx.emit(start, end, kind);
        return true;
    }
    if !matches!(b, b'*' | b'+') || !lx.ctx.peek_at(1).is_ascii_alphabetic() {
        return false;
    }
    let end = lx.scan_ident(start);
    if end - start < 3 || lx.ctx.at(end - 1) != b {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, if b == b'*' { Kind::Variable } else { Kind::Constant });
    lx.mark_value();
    true
}

/// `(fn name [args])` names a function, `(fn [args])` does not.
fn lisp_fn(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    let end = lx.scan_ident(start);
    let word = &lx.ctx.src[start..end];
    if word != b"fn" && word != b"lambda" && word != "λ".as_bytes() {
        return false;
    }
    lx.ctx.pos = end;
    lx.ctx.emit(start, end, Kind::Keyword);
    let s = lx.ctx.skip_spaces_from(end);
    if is_ident_start(lx.ctx.at(s)) {
        let e = lx.scan_ident(s);
        lx.ctx.pos = e;
        lx.ctx.emit(s, e, Kind::Function);
    }
    true
}

fn lisp_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    match lx.ctx.peek() {
        b'f' | b'l' | 0xCE => lisp_fn(lx),
        b'#' => lisp_hash(lx),
        b'\\' if start + 1 < lx.ctx.len() => {
            // Clojure character literal: \a, \newline.
            let end = char_name(lx.ctx, start + 1);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::String);
            lx.mark_value();
            true
        }
        b'?' if start + 1 < lx.ctx.len() => {
            // Emacs Lisp character literal: ?a, ?\n, ?\C-x.
            let ctx = &mut *lx.ctx;
            let mut i = start + 1;
            if ctx.at(i) == b'\\' {
                i = after_char(ctx, i + 1);
                while ctx.at(i) == b'-' && ctx.at(i - 1).is_ascii_uppercase() && i + 1 < ctx.len() {
                    i += 1;
                    if ctx.at(i) == b'\\' {
                        i += 1;
                    }
                    i = after_char(ctx, i);
                }
            } else {
                i = after_char(ctx, i);
            }
            ctx.pos = i.min(ctx.len());
            ctx.emit_from(start, Kind::String);
            lx.mark_value();
            true
        }
        b'^' => {
            // Clojure metadata: ^String, ^:private, ^{...}.
            let mut end = start + 1;
            if lx.ctx.at(end) == b':' {
                end += 1;
            }
            end = lx.scan_ident(end);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Meta);
            true
        }
        _ => lisp_word(lx),
    }
}

const LISP_OPERATORS: &str = "'`~,@^";

static CLOJURE_SPEC: Spec = Spec {
    line_comments: &[";"],
    strings: &[Str::quoted("\"").multiline(), Str::new("#\"", "\"").multiline().kind(Kind::Regex)],
    keywords: &[
        "def", "defonce", "fn", "fn*", "let", "letfn", "loop", "recur", "if", "if-not", "if-let", "if-some", "when",
        "when-not", "when-let", "when-some", "when-first", "cond", "condp", "case", "do", "quote", "var", "throw",
        "try", "catch", "finally", "new", "set!", "and", "or", "not", "doseq", "dotimes", "for", "while", "binding",
        "->", "->>", "as->", "some->", "some->>", "cond->", "cond->>", "doto", "..", "delay", "lazy-seq", "reify",
        "proxy", "extend-type", "extend-protocol", "import", "require", "use", "refer", "in-ns", "&",
    ],
    builtins: &[
        "println", "print", "prn", "str", "map", "filter", "reduce", "first", "rest", "next", "cons", "conj", "assoc",
        "dissoc", "get", "get-in", "assoc-in", "update", "update-in", "count", "vec", "vector", "list", "hash-map",
        "keys", "vals", "seq", "into", "range", "apply", "partial", "comp", "identity", "inc", "dec", "nil?", "empty?",
        "some", "every?", "swap!", "reset!", "atom", "deref", "merge", "concat", "take", "drop", "sort", "sort-by",
        "group-by", "frequencies", "format", "max", "min", "mapv", "filterv", "keep", "remove", "distinct", "mapcat",
        "zipmap", "partition", "re-find", "re-matches", "re-seq", "slurp", "spit", "nth", "last", "contains?", "subs",
    ],
    constants: &["true", "false", "nil"],
    function_keywords: &["defn", "defn-", "defmacro", "defmulti", "defmethod"],
    type_keywords: &["defprotocol", "defrecord", "deftype", "definterface", "ns"],
    ident_chars: "-+*/<>=!?.%&$'#",
    ident_start: LISP_IDENT,
    sigils: &[(b':', Kind::Constant)],
    operators: "'`~@^",
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "#\\^*-+0123456789",
    ..Spec::DEFAULT
};

pub static CLOJURE: Definition = Definition::new("clojure", "Clojure", Engine::Generic(&CLOJURE_SPEC))
    .aliases(&["clj", "cljs", "cljc", "edn", "clojurescript"])
    .extensions(&["clj", "cljs", "cljc", "edn"]);

const SCHEME_KEYWORDS: &[&str] = &[
    "define", "define-values", "lambda", "let", "let*", "letrec", "letrec*", "let-values", "let*-values", "if", "cond",
    "case", "else", "when", "unless", "and", "or", "not", "begin", "do", "set!", "quote", "quasiquote", "unquote",
    "syntax-rules", "let-syntax", "letrec-syntax", "delay", "force", "import", "export", "library", "guard",
    "parameterize", "call/cc", "call-with-current-continuation", "values", "call-with-values", "dynamic-wind",
    "include", "=>", "λ",
];

static SCHEME_SPEC: Spec = Spec {
    line_comments: &[";"],
    block_comments: &[("#|", "|#")],
    nested_comments: true,
    strings: &[Str::quoted("\"").multiline()],
    keywords: SCHEME_KEYWORDS,
    builtins: &[
        "display", "newline", "write", "car", "cdr", "cons", "list", "append", "map", "for-each", "filter", "reduce",
        "fold-left", "fold-right", "length", "reverse", "apply", "null?", "pair?", "list?", "eq?", "eqv?", "equal?",
        "zero?", "number?", "string?", "symbol?", "procedure?", "vector", "vector-ref", "vector-set!", "make-vector",
        "string-append", "string-length", "substring", "number->string", "string->number", "symbol->string", "assoc",
        "assq", "member", "memq", "error", "exit", "abs", "min", "max", "quotient", "remainder", "modulo", "expt",
        "sqrt", "list-ref", "cadr", "caddr",
    ],
    function_keywords: &["define-syntax"],
    type_keywords: &["define-record-type"],
    ident_chars: "-+*/<>=!?.%&$:^~",
    ident_start: LISP_IDENT,
    operators: "'`,@",
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "#-+0123456789",
    ..Spec::DEFAULT
};

pub static SCHEME: Definition = Definition::new("scheme", "Scheme", Engine::Generic(&SCHEME_SPEC))
    .aliases(&["scm"])
    .extensions(&["scm", "ss", "sld", "sls", "sps"]);

static RACKET_SPEC: Spec = Spec {
    line_comments: &[";"],
    block_comments: &[("#|", "|#")],
    nested_comments: true,
    strings: &[
        Str::quoted("\"").multiline(),
        Str::new("#\"", "\"").multiline(),
        Str::new("#rx\"", "\"").kind(Kind::Regex),
        Str::new("#px\"", "\"").kind(Kind::Regex),
    ],
    keywords: &[
        "define", "define-syntax-rule", "define-values", "define/contract", "lambda", "λ", "case-lambda", "let",
        "let*", "letrec", "let-values", "if", "cond", "case", "else", "when", "unless", "and", "or", "not", "begin",
        "do", "set!", "quote", "quasiquote", "unquote", "syntax-rules", "syntax-case", "match", "match-define", "for",
        "for/list", "for/vector", "for/hash", "for/fold", "for/sum", "for*", "for*/list", "module", "module+",
        "provide", "require", "all-defined-out", "only-in", "prefix-in", "rename-in", "except-in", "contract-out",
        "with-handlers", "parameterize", "values", "=>",
    ],
    builtins: &[
        "display", "displayln", "printf", "newline", "write", "car", "cdr", "cons", "first", "rest", "list", "append",
        "map", "for-each", "filter", "foldl", "foldr", "length", "reverse", "apply", "null?", "empty?", "empty",
        "equal?", "eq?", "zero?", "add1", "sub1", "range", "build-list", "format", "string-append", "string-join",
        "number->string", "hash", "hash-ref", "hash-set!", "make-hash", "vector", "vector-ref", "error", "raise",
        "void", "sort", "member", "assoc",
    ],
    function_keywords: &["define-syntax"],
    type_keywords: &["struct", "define-struct"],
    ident_chars: "-+*/<>=!?.%&$:^~",
    ident_start: LISP_IDENT,
    operators: "'`,@",
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "#-+0123456789",
    ..Spec::DEFAULT
};

pub static RACKET: Definition = Definition::new("racket", "Racket", Engine::Generic(&RACKET_SPEC))
    .aliases(&["rkt"])
    .extensions(&["rkt", "rktl", "rktd", "scrbl"]);

static COMMONLISP_SPEC: Spec = Spec {
    line_comments: &[";"],
    block_comments: &[("#|", "|#")],
    nested_comments: true,
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "defvar", "defparameter", "defconstant", "defpackage", "in-package", "lambda", "let", "let*", "flet", "labels",
        "macrolet", "if", "when", "unless", "cond", "case", "ecase", "typecase", "etypecase", "progn", "prog1",
        "block", "return", "return-from", "loop", "do", "dolist", "dotimes", "tagbody", "go", "setf", "setq", "psetf",
        "incf", "decf", "push", "pop", "and", "or", "not", "quote", "function", "declare", "declaim", "the",
        "handler-case", "handler-bind", "unwind-protect", "ignore-errors", "multiple-value-bind", "destructuring-bind",
        "with-open-file", "with-slots", "eval-when", "&optional", "&rest", "&key", "&body", "&aux",
        "&allow-other-keys",
    ],
    builtins: &[
        "format", "print", "princ", "prin1", "terpri", "car", "cdr", "cons", "list", "append", "mapcar", "mapc",
        "reduce", "remove-if", "remove-if-not", "funcall", "apply", "length", "reverse", "nth", "first", "second",
        "third", "rest", "last", "member", "assoc", "gethash", "make-hash-table", "make-instance", "slot-value",
        "error", "signal", "values", "vector", "aref", "concatenate", "string=", "equal", "eq", "eql", "null", "listp",
        "numberp", "zerop", "1+", "1-",
    ],
    constants: &["t", "nil"],
    function_keywords: &["defun", "defmacro", "defgeneric", "defmethod"],
    type_keywords: &["defclass", "defstruct", "define-condition", "deftype"],
    ident_chars: "-+*/<>=!?.%&$:^~",
    ident_start: LISP_IDENT,
    sigils: &[(b':', Kind::Constant)],
    operators: "'`,@",
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "#*-+0123456789",
    ..Spec::DEFAULT
};

pub static COMMONLISP: Definition = Definition::new("commonlisp", "Common Lisp", Engine::Generic(&COMMONLISP_SPEC))
    .aliases(&["lisp", "cl", "common-lisp"])
    .extensions(&["lisp", "cl", "lsp", "asd"]);

static EMACSLISP_SPEC: Spec = Spec {
    line_comments: &[";"],
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "defvar", "defvar-local", "defcustom", "defconst", "defgroup", "defface", "defalias", "lambda", "let", "let*",
        "if", "when", "unless", "cond", "and", "or", "not", "progn", "prog1", "while", "dolist", "dotimes", "catch",
        "throw", "condition-case", "unwind-protect", "save-excursion", "save-restriction", "with-current-buffer",
        "setq", "setq-local", "setf", "interactive", "require", "provide", "quote", "function", "pcase", "cl-loop",
        "ignore-errors", "&optional", "&rest",
    ],
    builtins: &[
        "message", "format", "concat", "insert", "car", "cdr", "cons", "list", "append", "mapcar", "mapc", "funcall",
        "apply", "length", "nth", "buffer-string", "point", "goto-char", "current-buffer", "string-match",
        "replace-regexp-in-string", "add-hook", "global-set-key", "define-key", "kbd", "error", "user-error",
    ],
    constants: &["t", "nil"],
    function_keywords: &[
        "defun", "defmacro", "defsubst", "cl-defun", "cl-defmacro", "cl-defmethod", "define-minor-mode",
        "define-derived-mode",
    ],
    type_keywords: &["cl-defstruct"],
    ident_chars: "-+*/<>=!?.%&$:^~",
    ident_start: LISP_IDENT,
    sigils: &[(b':', Kind::Constant)],
    operators: "'`,@",
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "#*?-+0123456789",
    ..Spec::DEFAULT
};

pub static EMACSLISP: Definition = Definition::new("emacslisp", "Emacs Lisp", Engine::Generic(&EMACSLISP_SPEC))
    .aliases(&["elisp", "emacs-lisp", "el"])
    .extensions(&["el"])
    .filenames(&[".emacs", "_emacs", ".gnus"]);

static FENNEL_SPEC: Spec = Spec {
    line_comments: &[";"],
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "local", "var", "set", "global", "let", "if", "when", "do", "while", "for", "each", "match", "case", "->",
        "->>", "-?>", "-?>>", "doto", "macros", "import-macros", "require-macros", "include", "values", "and", "or",
        "not", "not=", "tset", "hashfn", "partial", "accumulate", "collect", "icollect", "fcollect", "faccumulate",
        "eval-compiler", "lua", "quote", "with-open", "where",
    ],
    builtins: &[
        "print", "require", "ipairs", "pairs", "type", "tostring", "tonumber", "length", "select", "error", "assert",
        "pcall", "unpack", "setmetatable", "string.format", "table.insert", "table.concat", "table.remove",
    ],
    constants: &["true", "false", "nil"],
    function_keywords: &["macro"],
    ident_chars: "-+*/<>=!?.%&$:^~",
    ident_start: LISP_IDENT,
    sigils: &[(b':', Kind::Constant)],
    operators: LISP_OPERATORS,
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "#-+0123456789flλ",
    ..Spec::DEFAULT
};

pub static FENNEL: Definition =
    Definition::new("fennel", "Fennel", Engine::Generic(&FENNEL_SPEC)).aliases(&["fnl"]).extensions(&["fnl"]);

static JANET_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[
        Str::new("``", "``").raw().multiline(),
        Str::new("`", "`").raw().multiline(),
        Str::quoted("\"").multiline(),
    ],
    keywords: &[
        "def", "var", "let", "if", "do", "when", "unless", "cond", "case", "match", "while", "loop", "for", "each",
        "seq", "generate", "set", "break", "quote", "quasiquote", "unquote", "splice", "try", "defer", "edefer",
        "with", "import", "use", "and", "or", "not", "->", "->>", "-?>", "-?>>", "as->", "if-let", "when-let",
        "upscope",
    ],
    builtins: &[
        "print", "printf", "pp", "string", "length", "map", "filter", "reduce", "array", "table", "struct", "get",
        "put", "keys", "values", "pairs", "in", "type", "describe", "error", "eprint", "slurp", "spit", "range", "sum",
        "string/format", "array/push",
    ],
    constants: &["true", "false", "nil"],
    function_keywords: &["defn", "defn-", "defmacro", "defmacro-"],
    ident_chars: "-+*/<>=!?.%&$:^~",
    ident_start: LISP_IDENT,
    sigils: &[(b':', Kind::Constant)],
    operators: "'~,;@|",
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "-+0123456789f",
    ..Spec::DEFAULT
};

pub static JANET: Definition =
    Definition::new("janet", "Janet", Engine::Generic(&JANET_SPEC)).extensions(&["janet", "jdn"]);

static HY_SPEC: Spec = Spec {
    line_comments: &[";"],
    strings: &[Str::new("#[[", "]]").raw().multiline(), Str::quoted("\"").multiline()],
    string_prefixes: &[Prefix::interp(b'f', Interp::Brace), Prefix::raw(b'r'), Prefix::plain(b'b')],
    keywords: &[
        "setv", "setx", "let", "if", "when", "unless", "cond", "do", "for", "while", "break", "continue", "return",
        "yield", "import", "require", "with", "try", "except", "finally", "else", "raise", "and", "or", "not", "in",
        "is", "assert", "global", "nonlocal", "del", "quote", "quasiquote", "unquote", "lambda", "match", "await",
        "lfor", "dfor", "sfor", "gfor", "fn",
    ],
    builtins: &[
        "print", "len", "range", "str", "int", "float", "list", "dict", "set", "tuple", "map", "filter", "sorted",
        "enumerate", "zip", "isinstance", "open", "format", "get", "cut",
    ],
    constants: &["True", "False", "None"],
    variables: &["self"],
    function_keywords: &["defn", "defmacro"],
    type_keywords: &["defclass"],
    ident_chars: "-+*/<>=!?.%&$:^~",
    ident_start: LISP_IDENT,
    sigils: &[(b':', Kind::Constant)],
    operators: LISP_OPERATORS,
    features: LISP_CALLS,
    hook: Some(lisp_hook),
    hook_chars: "#-+0123456789",
    ..Spec::DEFAULT
};

pub static HY: Definition =
    Definition::new("hy", "Hy", Engine::Generic(&HY_SPEC)).aliases(&["hylang"]).extensions(&["hy"]);

// Erlang and Prolog

/// Variables, atoms, module attributes, `$c` characters and `16#FF` numbers.
fn erlang_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    match lx.ctx.peek() {
        b'-' => {
            if !col0(lx.ctx, start) || !lx.ctx.peek_at(1).is_ascii_lowercase() {
                return false;
            }
            let end = lx.scan_ident(start + 1);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Meta);
            true
        }
        b'$' => {
            let ctx = &mut *lx.ctx;
            if start + 1 >= ctx.len() {
                return false;
            }
            ctx.emit(start, start + 1, Kind::String);
            if ctx.at(start + 1) == b'\\' {
                ctx.pos = after_char(ctx, start + 2);
                ctx.emit_from(start + 1, Kind::Escape);
            } else {
                ctx.pos = after_char(ctx, start + 1);
                ctx.emit_from(start + 1, Kind::String);
            }
            lx.mark_value();
            true
        }
        b'0'..=b'9' => {
            let ctx = &mut *lx.ctx;
            scan_number(ctx, false);
            if ctx.peek() == b'#' && ctx.peek_at(1).is_ascii_alphanumeric() {
                ctx.pos += 1;
                while ctx.peek().is_ascii_alphanumeric() {
                    ctx.pos += 1;
                }
            }
            ctx.emit_from(start, Kind::Number);
            lx.mark_value();
            true
        }
        b'A'..=b'Z' | b'_' => {
            let end = lx.scan_ident(start);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Variable);
            lx.mark_value();
            true
        }
        _ => {
            let end = lx.scan_ident(start);
            lx.ctx.pos = end;
            let kind = match lx.classify(start, end) {
                Kind::Text => {
                    let ctx = &*lx.ctx;
                    let after = ctx.skip_spaces_from(end);
                    let next = ctx.at(after);
                    // Record fields: `#rec.field` and `#rec{field = 1}`.
                    let field = (start > 0 && ctx.src[start - 1] == b'.')
                        || (next == b'=' && !matches!(ctx.at(after + 1), b'=' | b':' | b'<' | b'/' | b'>'));
                    if next == b':' && is_ident_start(ctx.at(after + 1)) {
                        Kind::Type
                    } else if next == b'/' && ctx.at(after + 1).is_ascii_digit() {
                        Kind::Function
                    } else if field {
                        Kind::Property
                    } else {
                        Kind::Constant
                    }
                }
                kind => kind,
            };
            lx.ctx.emit(start, end, kind);
            true
        }
    }
}

static ERLANG_SPEC: Spec = Spec {
    line_comments: &["%"],
    strings: &[
        Str::quoted("\"\"\"").raw().multiline(),
        Str::quoted("\"").multiline(),
        Str::quoted("'").kind(Kind::Constant),
    ],
    keywords: &[
        "after", "and", "andalso", "band", "begin", "bnot", "bor", "bsl", "bsr", "bxor", "case", "catch", "cond",
        "div", "end", "fun", "if", "let", "maybe", "not", "of", "or", "orelse", "receive", "rem", "try", "when", "xor",
        "else",
    ],
    builtins: &[
        "self", "spawn", "spawn_link", "length", "hd", "tl", "element", "setelement", "is_atom", "is_binary",
        "is_list", "is_integer", "is_float", "is_number", "is_tuple", "is_map", "is_function", "is_pid",
        "atom_to_list", "list_to_atom", "integer_to_list", "list_to_integer", "binary_to_list", "list_to_binary",
        "tuple_size", "map_size", "make_ref", "node", "abs", "round", "trunc",
    ],
    constants: &["true", "false", "undefined"],
    sigils: &[(b'?', Kind::Meta), (b'#', Kind::Type)],
    features: CALLS,
    hook: Some(erlang_hook),
    hook_chars: "-$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static ERLANG: Definition = Definition::new("erlang", "Erlang", Engine::Generic(&ERLANG_SPEC))
    .aliases(&["erl"])
    .extensions(&["erl", "hrl", "escript", "app.src"])
    .filenames(&["rebar.config"]);

/// Variables, clause heads, directives and `0'c` character codes.
fn prolog_hook(lx: &mut Lexer) -> bool {
    let start = lx.ctx.pos;
    match lx.ctx.peek() {
        b'0' if lx.ctx.peek_at(1) == b'\'' && start + 2 < lx.ctx.len() => {
            let ctx = &mut *lx.ctx;
            let end = match ctx.at(start + 2) {
                b'\\' => after_char(ctx, start + 3),
                b'\'' if ctx.at(start + 3) == b'\'' => start + 4,
                _ => after_char(ctx, start + 2),
            };
            ctx.pos = end.min(ctx.len());
            ctx.emit_from(start, Kind::Number);
            lx.mark_value();
            true
        }
        b'A'..=b'Z' | b'_' => {
            let end = lx.scan_ident(start);
            lx.ctx.pos = end;
            lx.ctx.emit(start, end, Kind::Variable);
            lx.mark_value();
            true
        }
        b':' if col0(lx.ctx, start) && lx.ctx.peek_at(1) == b'-' => {
            lx.ctx.token(2, Kind::Meta);
            let s = lx.ctx.skip_spaces_from(lx.ctx.pos);
            let e = lx.scan_ident(s);
            lx.ctx.pos = e;
            lx.ctx.emit(s, e, Kind::Meta);
            true
        }
        b'a'..=b'z' if col0(lx.ctx, start) => {
            let end = lx.scan_ident(start);
            lx.ctx.pos = end;
            let kind = match lx.classify(start, end) {
                Kind::Keyword => Kind::Keyword,
                _ => Kind::Function,
            };
            lx.ctx.emit(start, end, kind);
            true
        }
        _ => false,
    }
}

static PROLOG_SPEC: Spec = Spec {
    line_comments: &["%"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\"").doubled(), Str::quoted("'").doubled(), Str::quoted("`")],
    keywords: &[
        "is", "mod", "rem", "div", "xor", "rdiv", "dynamic", "discontiguous", "multifile", "module", "use_module",
        "ensure_loaded", "initialization", "meta_predicate", "table",
    ],
    builtins: &[
        "write", "writeln", "print", "format", "nl", "read", "atom", "number", "integer", "var", "nonvar", "is_list",
        "length", "append", "member", "memberchk", "reverse", "msort", "sort", "predsort", "nth0", "nth1", "last",
        "sum_list", "max_list", "min_list", "findall", "bagof", "setof", "forall", "between", "succ", "plus",
        "atom_codes", "atom_chars", "atom_length", "atom_concat", "sub_atom", "number_codes", "char_code",
        "atom_string", "string_concat", "split_string", "assert", "asserta", "assertz", "retract", "call", "catch",
        "throw", "halt", "functor", "arg", "copy_term", "maplist", "foldl", "include", "exclude", "tab",
    ],
    constants: &["true", "fail", "false", "otherwise"],
    operators: "+-*/\\=<>@#&$?^~:;|!",
    features: CALLS,
    hook: Some(prolog_hook),
    hook_chars: "0:ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
    ..Spec::DEFAULT
};

pub static PROLOG: Definition =
    Definition::new("prolog", "Prolog", Engine::Generic(&PROLOG_SPEC)).extensions(&["pro", "prolog"]);

// Newer ML style languages

static GLEAM_SPEC: Spec = Spec {
    line_comments: &["//"],
    strings: &[Str::quoted("\"").multiline()],
    keywords: &[
        "as", "assert", "auto", "case", "const", "delegate", "derive", "echo", "else", "if", "implement", "import",
        "let", "macro", "opaque", "panic", "pub", "test", "todo", "use",
    ],
    constants: &["True", "False", "Nil"],
    function_keywords: &["fn"],
    type_keywords: &["type"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static GLEAM: Definition = Definition::new("gleam", "Gleam", Engine::Generic(&GLEAM_SPEC)).extensions(&["gleam"]);

/// Qualified names, signatures and `name = \x ->` style function definitions.
fn roc_hook(lx: &mut Lexer) -> bool {
    match lx.ctx.peek() {
        b'A'..=b'Z' => qualified(lx),
        _ => declaration(lx, b":", false, true),
    }
}

static ROC_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[
        Str::quoted("\"\"\"").multiline().interp(Interp::Dollar),
        Str::quoted("\"").interp(Interp::Dollar),
        Str::quoted("'"),
    ],
    keywords: &[
        "app", "as", "crash", "dbg", "else", "expect", "exposing", "if", "import", "imports", "implements", "is",
        "module", "package", "packages", "platform", "provides", "requires", "return", "then", "var", "when", "where",
        "with", "for", "in",
    ],
    ident_suffix: "!",
    operators: "+-*/%=&|^!<>~?:\\",
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS,
    hook: Some(roc_hook),
    hook_chars: LETTERS,
    ..Spec::DEFAULT
};

pub static ROC: Definition = Definition::new("roc", "Roc", Engine::Generic(&ROC_SPEC)).extensions(&["roc"]);

static GRAIN_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    strings: &[Str::quoted("\""), Str::quoted("'")],
    keywords: &[
        "and", "as", "assert", "break", "continue", "else", "except", "exception", "fail", "for", "foreign", "from",
        "if", "include", "let", "macro", "match", "module", "mut", "primitive", "provide", "abstract", "rec", "return",
        "throw", "unsafe", "use", "when", "while", "wasm",
    ],
    builtins: &["print", "toString", "ignore", "identity"],
    constants: &["true", "false", "void"],
    type_keywords: &["type", "enum", "record"],
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS | ARROW_FUNCTIONS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static GRAIN: Definition = Definition::new("grain", "Grain", Engine::Generic(&GRAIN_SPEC)).extensions(&["gr"]);

static KOKA_SPEC: Spec = Spec {
    line_comments: &["//"],
    block_comments: &[("/*", "*/")],
    nested_comments: true,
    strings: &[Str::quoted("\""), Str::quoted("'")],
    string_prefixes: &[Prefix::raw(b'r')],
    keywords: &[
        "abstract", "as", "break", "co", "con", "continue", "elif", "else", "extend", "final", "fn", "forall",
        "handle", "handler", "if", "import", "in", "infix", "infixl", "infixr", "inline", "interface", "linear",
        "mask", "match", "module", "named", "noinline", "open", "override", "pub", "raw", "rec", "reference", "return",
        "scoped", "some", "then", "val", "var", "with", "exists",
    ],
    types: &[
        "int", "bool", "char", "string", "float64", "float32", "int8", "int16", "int32", "int64", "list", "vector",
        "maybe", "either", "unit", "exn", "ndet", "io", "console", "ref",
    ],
    builtins: &[
        "println", "print", "show", "map", "foldl", "foldr", "filter", "length", "head", "tail", "reverse", "zip",
        "sum", "max", "min", "abs",
    ],
    constants: &["True", "False"],
    function_keywords: &["fun", "ctl"],
    type_keywords: &["type", "struct", "effect", "alias"],
    ident_chars: "-'",
    features: CALLS | CONSTRUCTORS | CAPITALIZED_TYPES | MEMBERS,
    ..Spec::DEFAULT
};

pub static KOKA: Definition = Definition::new("koka", "Koka", Engine::Generic(&KOKA_SPEC)).extensions(&["kk"]);
