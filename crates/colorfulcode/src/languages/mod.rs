//! Language definitions and lookup.

use crate::lexer::generic::{Lexer, Spec};
use crate::lexer::Ctx;

mod c_family;
mod data;
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
        let eq = |s: &str| s.eq_ignore_ascii_case(name);
        let all = Language::all();
        all.iter()
            .find(|l| eq(l.id()))
            .or_else(|| all.iter().find(|l| l.aliases().iter().any(|a| eq(a))))
            .or_else(|| all.iter().find(|l| eq(l.name())))
            .or_else(|| all.iter().find(|l| l.extensions().iter().any(|e| eq(e))))
            .copied()
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
    use std::sync::OnceLock;

    static GROUPS: &[&[&Definition]] = &[
        c_family::LANGUAGES,
        data::LANGUAGES,
        functional::LANGUAGES,
        markup::LANGUAGES,
        misc::LANGUAGES,
        scripting::LANGUAGES,
        shell::LANGUAGES,
        web::LANGUAGES,
    ];

    pub fn all() -> &'static [Language] {
        static ALL: OnceLock<Vec<Language>> = OnceLock::new();
        ALL.get_or_init(|| {
            let mut all: Vec<Language> = GROUPS.iter().flat_map(|g| g.iter().map(|d| Language(d))).collect();
            all.sort_by_key(|l| l.id());
            all
        })
    }
}
