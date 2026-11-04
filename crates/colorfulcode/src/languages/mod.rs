// The trimmed build leaves the languages it does not register unused on purpose.
#![cfg_attr(not(feature = "all-languages"), allow(dead_code))]

//! Language definitions and lookup.

use crate::lexer::generic::{Lexer, Spec};
use crate::lexer::Ctx;

mod c_family;
mod data;
mod docs;
mod functional;
mod markup;
mod misc;
mod scripting;
mod shell;
mod web;

/// How a language is lexed.
pub enum Engine {
    /// Table driven lexer.
    Generic(&'static Spec),
    /// Hand written lexer.
    Custom(fn(&mut Ctx)),
}

/// Static description of a language.
pub struct Definition {
    /// Canonical identifier, for example `"rust"`.
    pub id: &'static str,
    /// Human readable name, for example `"Rust"`.
    pub name: &'static str,
    /// Other names accepted by [`Language::find`].
    pub aliases: &'static [&'static str],
    /// File extensions without the dot.
    pub extensions: &'static [&'static str],
    /// Exact file names such as `Dockerfile`.
    pub filenames: &'static [&'static str],
    pub engine: Engine,
}

impl Definition {
    pub(crate) const fn new(id: &'static str, name: &'static str, engine: Engine) -> Definition {
        Definition { id, name, aliases: &[], extensions: &[], filenames: &[], engine }
    }

    pub(crate) const fn aliases(self, aliases: &'static [&'static str]) -> Definition {
        Definition { aliases, ..self }
    }

    pub(crate) const fn extensions(self, extensions: &'static [&'static str]) -> Definition {
        Definition { extensions, ..self }
    }

    pub(crate) const fn filenames(self, filenames: &'static [&'static str]) -> Definition {
        Definition { filenames, ..self }
    }
}

/// A supported language. Cheap to copy and compare.
#[derive(Clone, Copy)]
pub struct Language(&'static Definition);

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Language {}

impl std::fmt::Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Language").field(&self.0.id).finish()
    }
}

impl Language {
    /// Canonical identifier, for example `"typescript"`.
    pub fn id(self) -> &'static str {
        self.0.id
    }

    /// Display name, for example `"TypeScript"`.
    pub fn name(self) -> &'static str {
        self.0.name
    }

    pub fn aliases(self) -> &'static [&'static str] {
        self.0.aliases
    }

    pub fn extensions(self) -> &'static [&'static str] {
        self.0.extensions
    }

    pub fn filenames(self) -> &'static [&'static str] {
        self.0.filenames
    }

    /// Every supported language, sorted by id.
    pub fn all() -> &'static [Language] {
        registry::all()
    }

    /// Plain text: no highlighting at all.
    pub fn plain() -> Language {
        Language(&misc::PLAIN)
    }

    /// Finds a language by id, alias, name or file extension (case insensitive).
    ///
    /// ```
    /// use colorfulcode::Language;
    /// assert_eq!(Language::find("ts").unwrap().id(), "typescript");
    /// assert_eq!(Language::find("C++").unwrap().id(), "cpp");
    /// ```
    pub fn find(name: &str) -> Option<Language> {
        let name = name.trim();
        let name = name.strip_prefix('.').unwrap_or(name);
        if name.len() > 64 {
            return None;
        }
        let mut buf = [0u8; 64];
        let lower = &mut buf[..name.len()];
        lower.copy_from_slice(name.as_bytes());
        lower.make_ascii_lowercase();
        registry::names().get(&*lower).copied()
    }

    /// Guesses the language from a file path using its name and extension.
    ///
    /// ```
    /// use colorfulcode::Language;
    /// assert_eq!(Language::from_path("src/main.rs").unwrap().id(), "rust");
    /// assert_eq!(Language::from_path("types/index.d.ts").unwrap().id(), "typescript");
    /// ```
    pub fn from_path(path: &str) -> Option<Language> {
        let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let all = Language::all();
        if let Some(l) = all.iter().find(|l| l.filenames().iter().any(|f| f.eq_ignore_ascii_case(file))) {
            return Some(*l);
        }
        // Try the longest extension first so `foo.d.ts` style names work.
        let mut rest = file;
        while let Some(i) = rest.find('.') {
            rest = &rest[i + 1..];
            if let Some(l) = all.iter().find(|l| l.extensions().iter().any(|e| e.eq_ignore_ascii_case(rest))) {
                return Some(*l);
            }
        }
        None
    }

    /// Guesses the language from the content, using a shebang or a few
    /// telltale first lines. Returns `None` when unsure.
    pub fn from_content(code: &str) -> Option<Language> {
        let first = code.lines().next().unwrap_or("").trim();
        if let Some(shebang) = first.strip_prefix("#!") {
            let mut parts = shebang.split_whitespace();
            let mut program = parts.next().unwrap_or("").rsplit('/').next().unwrap_or("");
            if program == "env" {
                program = parts.find(|p| !p.starts_with('-')).unwrap_or("");
            }
            let program = program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
            return Language::find(match program {
                "sh" | "bash" | "zsh" | "ksh" | "dash" | "ash" => "bash",
                "node" | "deno" | "bun" => "javascript",
                "python" => "python",
                other => other,
            });
        }
        if first.starts_with("<?xml") {
            return Language::find("xml");
        }
        if first.starts_with("<?php") {
            return Language::find("php");
        }
        if first.to_ascii_lowercase().starts_with("<!doctype html") || first.starts_with("<html") {
            return Language::find("html");
        }
        None
    }

    /// Guesses the language of `code` from its content alone. Opt-in:
    /// highlighting never calls it. Tries [`Language::from_content`] first,
    /// then scores the first 4 KB against every language, which takes well
    /// under a millisecond (the word tables are built on the first call).
    /// Returns `None` when no language is a convincing match.
    ///
    /// ```
    /// use colorfulcode::Language;
    /// let code = "def greet(name):\n    if not name:\n        return None\n    print(f\"hi {name}\")\n";
    /// assert_eq!(Language::detect(code).unwrap().id(), "python");
    /// assert_eq!(Language::detect("just a sentence"), None);
    /// ```
    pub fn detect(code: &str) -> Option<Language> {
        crate::detect::detect(code, Language::all())
    }

    /// Like [`Language::detect`], but only picks from `candidates`, for
    /// callers that know the likely set.
    ///
    /// ```
    /// use colorfulcode::Language;
    /// let web = ["javascript", "typescript"].map(|id| Language::find(id).unwrap());
    /// let code = "const total: number = items.length;";
    /// assert_eq!(Language::detect_among(code, &web).unwrap().id(), "typescript");
    /// ```
    pub fn detect_among(code: &str, candidates: &[Language]) -> Option<Language> {
        crate::detect::detect(code, candidates)
    }

    /// The table driven spec behind this language, if any.
    pub(crate) fn spec(self) -> Option<&'static Spec> {
        match self.0.engine {
            Engine::Generic(spec) => Some(spec),
            Engine::Custom(_) => None,
        }
    }

    pub(crate) fn lex(self, ctx: &mut Ctx) {
        match self.0.engine {
            Engine::Generic(spec) => Lexer::new(ctx, spec).run(),
            Engine::Custom(f) => f(ctx),
        }
    }
}

/// Looks up a language that is known to exist. Used by embedding lexers.
pub(crate) fn lang(id: &str) -> Language {
    Language::find(id).unwrap_or_else(Language::plain)
}

mod registry {
    use super::*;
    use std::collections::HashMap;
    use std::sync::OnceLock;

    /// Every language.
    #[cfg(feature = "all-languages")]
    static GROUPS: &[&[&Definition]] = &[
        c_family::LANGUAGES,
        data::LANGUAGES,
        docs::LANGUAGES,
        functional::LANGUAGES,
        markup::LANGUAGES,
        misc::LANGUAGES,
        scripting::LANGUAGES,
        shell::LANGUAGES,
        web::LANGUAGES,
    ];

    /// The most used languages, for smaller builds. Everything else is removed
    /// by the linker because nothing refers to it.
    #[cfg(not(feature = "all-languages"))]
    static GROUPS: &[&[&Definition]] = &[&[
        &c_family::C,
        &c_family::CPP,
        &c_family::CSHARP,
        &c_family::DART,
        &c_family::GO,
        &c_family::JAVA,
        &c_family::KOTLIN,
        &c_family::OBJECTIVE_C,
        &c_family::RUST,
        &c_family::SCALA,
        &c_family::SWIFT,
        &c_family::ZIG,
        &data::DIFF,
        &data::DOTENV,
        &data::GRAPHQL,
        &data::HCL,
        &data::INI,
        &data::PROTOBUF,
        &data::SQL,
        &data::TOML,
        &data::YAML,
        &functional::HASKELL,
        &markup::ASTRO,
        &markup::HTML,
        &markup::MARKDOWN,
        &markup::MDX,
        &markup::PHP,
        &markup::SVELTE,
        &markup::SVG,
        &markup::VUE,
        &markup::XML,
        &misc::PLAIN,
        &scripting::ELIXIR,
        &scripting::LUA,
        &scripting::PERL,
        &scripting::PYTHON,
        &scripting::R,
        &scripting::RUBY,
        &shell::BASH,
        &shell::BATCH,
        &shell::DOCKERFILE,
        &shell::MAKEFILE,
        &shell::NGINX,
        &shell::POWERSHELL,
        &web::CSS,
        &web::JAVASCRIPT,
        &web::JSON,
        &web::JSON5,
        &web::JSONC,
        &web::JSX,
        &web::LESS,
        &web::SASS,
        &web::SCSS,
        &web::TSX,
        &web::TYPESCRIPT,
    ]];

    pub fn all() -> &'static [Language] {
        static ALL: OnceLock<Vec<Language>> = OnceLock::new();
        ALL.get_or_init(|| {
            let mut all: Vec<Language> = GROUPS.iter().flat_map(|g| g.iter().map(|d| Language(d))).collect();
            all.sort_by_key(|l| l.id());
            all
        })
    }

    /// Lowercase ids, aliases, names and extensions mapped to their language,
    /// so lookups by name do not scan every language. When names collide the
    /// stronger kind wins: id, then alias, then name, then extension.
    pub fn names() -> &'static HashMap<Vec<u8>, Language> {
        static NAMES: OnceLock<HashMap<Vec<u8>, Language>> = OnceLock::new();
        NAMES.get_or_init(|| {
            let mut map = HashMap::new();
            let all = all();
            let mut add = |key: &str, lang: Language| {
                map.insert(key.to_ascii_lowercase().into_bytes(), lang);
            };
            for l in all {
                l.extensions().iter().for_each(|e| add(e, *l));
            }
            for l in all {
                add(l.name(), *l);
            }
            for l in all {
                l.aliases().iter().for_each(|a| add(a, *l));
            }
            for l in all {
                add(l.id(), *l);
            }
            map
        })
    }
}
