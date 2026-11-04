//! Dynamic scripting languages.

use super::{Definition, Engine};
use crate::lexer::generic::{features::*, Interp, Prefix, Spec, Str};

pub static LANGUAGES: &[&Definition] = &[&PYTHON];

// Python

static PYTHON_SPEC: Spec = Spec {
    line_comments: &["#"],
    strings: &[Str::quoted("\"\"\"").multiline(), Str::quoted("'''").multiline(), Str::quoted("\""), Str::quoted("'")],
    string_prefixes: &[
        Prefix::raw(b'r'),
        Prefix::raw(b'R'),
        Prefix::interp(b'f', Interp::Brace),
        Prefix::interp(b'F', Interp::Brace),
        Prefix::interp(b't', Interp::Brace),
        Prefix::plain(b'b'),
        Prefix::plain(b'B'),
        Prefix::plain(b'u'),
        Prefix::plain(b'U'),
    ],
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "case", "continue", "del", "elif", "else", "except",
        "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "match", "nonlocal", "not", "or",
        "pass", "raise", "return", "try", "while", "with", "yield",
    ],
    variables: &["self", "cls"],
    types: &[
        "int", "float", "str", "bool", "bytes", "bytearray", "list", "dict", "set", "frozenset", "tuple", "object",
        "complex", "memoryview", "type",
    ],
    builtins: &[
        "abs", "aiter", "all", "anext", "any", "ascii", "bin", "breakpoint", "callable", "chr", "classmethod",
        "compile", "delattr", "dir", "divmod", "enumerate", "eval", "exec", "filter", "format", "getattr", "globals",
        "hasattr", "hash", "help", "hex", "id", "input", "isinstance", "issubclass", "iter", "len", "locals", "map",
        "max", "min", "next", "oct", "open", "ord", "pow", "print", "property", "range", "repr", "reversed", "round",
        "setattr", "slice", "sorted", "staticmethod", "sum", "super", "vars", "zip", "__import__", "__name__",
        "__file__", "__doc__",
    ],
    constants: &["True", "False", "None", "NotImplemented", "Ellipsis", "__debug__"],
    function_keywords: &["def"],
    type_keywords: &["class"],
    features: CALLS | CAPITALIZED_TYPES | UPPER_CONSTANTS | MEMBERS | ANNOTATIONS,
    ..Spec::DEFAULT
};

pub static PYTHON: Definition = Definition::new("python", "Python", Engine::Generic(&PYTHON_SPEC))
    .aliases(&["py", "python3", "py3"])
    .extensions(&["py", "pyw", "pyi", "pyx", "pxd", "gyp", "bzl", "star"])
    .filenames(&["SConstruct", "SConscript", "BUILD", "WORKSPACE", "Tiltfile"]);
