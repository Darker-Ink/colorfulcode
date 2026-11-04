//! Opt-in language detection from content.
//!
//! Nothing here runs while highlighting. [`detect`] scores the first few KB of
//! the input against every candidate language and returns the best one:
//!
//! 1. Words: identifiers that appear in a language's keyword, type, builtin or
//!    constant lists, weighted by how rare they are across languages, so
//!    `elif` or `defp` count for a lot and `if` for almost nothing. Common
//!    keywords a language lacks count against it.
//! 2. Comments: lines starting with a comment marker the language has are a
//!    bonus, lines starting with one it lacks are a penalty.
//! 3. Clues: telltale snippets such as `:= `, `elif ` or `{{#`, a few points
//!    each (see [`CLUES`]).
//! 4. Checks: small functions for formats that need more than a pattern
//!    (JSON, YAML, CSV, diffs) and for languages that share most of their
//!    words (JavaScript and TypeScript, C and Java). See [`CHECKS`].
//!
//! The best score must clear a threshold, so prose and very short inputs
//! come out as `None`.

use crate::lexer::generic::{FxHasher, Spec};
use crate::Language;
use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasherDefault;
use std::sync::OnceLock;

/// A hash map with a fast hash for short keys.
type Map<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// Only this many bytes from the start of the input are looked at.
const SAMPLE_BYTES: usize = 4096;
/// The best score must reach this to be returned at all.
const MIN_SCORE: f32 = 3.0;

/// The best candidate for `code`, or `None` when nothing scores high enough.
pub(crate) fn detect(code: &str, candidates: &[Language]) -> Option<Language> {
    let mut end = code.len().min(SAMPLE_BYTES);
    while !code.is_char_boundary(end) {
        end -= 1;
    }
    // Windows line breaks would hide the patterns that span a line break.
    let unix;
    let code = match &code[..end] {
        text if text.contains('\r') => {
            unix = text.replace("\r\n", "\n");
            &unix
        }
        text => text,
    };
    let sample = Sample::new(code);
    if let Some(lang) = Language::from_content(sample.text).filter(|l| candidates.contains(l)) {
        return Some(lang);
    }
    let all = Language::all();
    let everything = std::ptr::eq(candidates, all);
    let mut allowed = vec![everything; all.len()];
    for lang in candidates.iter().filter(|_| !everything) {
        if let Some(i) = all.iter().position(|l| l == lang) {
            allowed[i] = true;
        }
    }
    let scores = scores(&sample);
    let mut best: Option<(Language, f32)> = None;
    for ((lang, &score), allowed) in all.iter().zip(&scores).zip(allowed) {
        if allowed && score >= MIN_SCORE && best.map_or(true, |(_, s)| score > s) {
            best = Some((*lang, score));
        }
    }
    best.map(|(lang, _)| lang)
}

/// A score for every language, in the order of [`Language::all`].
fn scores(sample: &Sample) -> Vec<f32> {
    let model = model();
    let all = Language::all();
    let mut scores = vec![0f32; all.len()];
    score_words(sample, model, &mut scores);
    // Prose has words but hardly any brackets or operators. Spare it from
    // the languages whose keywords are English words.
    let symbols = sample.text.bytes().filter(is_symbol).count();
    let prose = if symbols * 100 < sample.text.len() { 5.0 } else { 0.0 };
    let comments = comment_lines(sample);
    for (i, lang) in all.iter().enumerate() {
        if let Some(spec) = lang.spec() {
            scores[i] += score_comments(&comments, spec) - model.prior[i] - prose;
        }
    }
    for (i, at, each, patterns) in &model.clues {
        let matches = match at {
            Text => sample.found(patterns),
            Lines => sample.starting_any(patterns),
        };
        // A single phrase from a format's clue can be a coincidence in prose.
        if *each <= 4.0 || matches > 1 || *at == Lines {
            scores[*i] += points(matches, *each, 4.0 * each);
        }
    }
    for &(i, check) in &model.checks {
        scores[i] += check(sample);
    }
    scores
}

/// The text being scored, with its lines split up front.
struct Sample<'a> {
    text: &'a str,
    /// The text in ASCII lowercase.
    lower: String,
    /// Non-empty lines with leading whitespace removed.
    lines: Vec<&'a str>,
    /// Where each pair of bytes occurs: the positions of a pair with key `k`
    /// (see [`pair`]) are `positions[starts[k]..starts[k + 1]]`. Hundreds of
    /// patterns are looked for, and this beats searching the whole text for
    /// each one, especially in WebAssembly, which has no vectorized search.
    starts: Vec<u16>,
    positions: Vec<u16>,
    shapes: Shapes,
    /// [`css`], which several style sheet languages ask for.
    css: OnceCell<f32>,
    /// [`typescript`] and [`jsx`], for the four JavaScript dialects.
    script: OnceCell<(f32, bool)>,
}

/// How many lines have shapes that several checks look for.
#[derive(Default)]
struct Shapes {
    typed_declarations: usize,
    type_signatures: usize,
    python_blocks: usize,
    cmdlets: usize,
    commands: usize,
    /// A line that is just `end`.
    bare_end: bool,
}

impl Shapes {
    fn new(lines: &[&str]) -> Shapes {
        let mut shapes = Shapes::default();
        for line in lines {
            shapes.typed_declarations += is_typed_declaration(line) as usize;
            shapes.type_signatures += is_type_signature(line) as usize;
            shapes.python_blocks += is_python_block(line) as usize;
            shapes.cmdlets += is_cmdlet(line) as usize;
            shapes.commands += is_command(line) as usize;
            shapes.bare_end |= line.trim_end() == "end";
        }
        shapes
    }
}

/// Number of distinct [`pair`] keys.
const PAIRS: usize = 1 << 12;

/// A key for two adjacent bytes. Pairs can share a key: matches are checked.
fn pair(a: u8, b: u8) -> usize {
    ((a as usize) << 5 ^ b as usize) & (PAIRS - 1)
}

impl<'a> Sample<'a> {
    /// `text` is at most [`SAMPLE_BYTES`] long, so positions fit in `u16`.
    fn new(text: &'a str) -> Sample<'a> {
        debug_assert!(text.len() <= SAMPLE_BYTES);
        let lines: Vec<&str> = text.lines().map(str::trim_start).filter(|l| !l.is_empty()).collect();
        // A counting sort of the positions by the pair of bytes they start.
        let bytes = text.as_bytes();
        let key = |i: usize| pair(bytes[i], bytes.get(i + 1).copied().unwrap_or(0));
        let mut starts = vec![0u16; PAIRS + 1];
        for i in 0..bytes.len() {
            starts[key(i) + 1] += 1;
        }
        for k in 1..starts.len() {
            starts[k] += starts[k - 1];
        }
        let mut next = starts.clone();
        let mut positions = vec![0u16; bytes.len()];
        for i in 0..bytes.len() {
            positions[next[key(i)] as usize] = i as u16;
            next[key(i)] += 1;
        }
        let shapes = Shapes::new(&lines);
        let lower = text.to_ascii_lowercase();
        let (css, script) = (OnceCell::new(), OnceCell::new());
        Sample { text, lower, lines, starts, positions, shapes, css, script }
    }

    /// Whether `pattern` occurs anywhere.
    fn has(&self, pattern: &str) -> bool {
        let (text, pattern) = (self.text.as_bytes(), pattern.as_bytes());
        let &[a, b, ..] = pattern else { return pattern.iter().all(|b| text.contains(b)) };
        let k = pair(a, b);
        let candidates = &self.positions[self.starts[k] as usize..self.starts[k + 1] as usize];
        candidates.iter().any(|&i| text[i as usize..].starts_with(pattern))
    }

    /// Case insensitive [`Sample::has`] for an ASCII lowercase `pattern`.
    fn has_ci(&self, pattern: &str) -> bool {
        self.lower.contains(pattern)
    }

    /// How many of `patterns` occur somewhere.
    fn found(&self, patterns: &[&str]) -> usize {
        patterns.iter().filter(|p| self.has(p)).count()
    }

    /// Number of lines starting with `prefix` (after indentation).
    fn starting(&self, prefix: &str) -> usize {
        self.starting_any(&[prefix])
    }

    /// Number of lines starting with any of `prefixes`.
    fn starting_any(&self, prefixes: &[&str]) -> usize {
        // Comparing the first byte first skips most calls to `memcmp`.
        let starts = |l: &str, p: &str| l.as_bytes().first() == p.as_bytes().first() && l.starts_with(p);
        self.lines.iter().filter(|l| prefixes.iter().any(|p| starts(l, p))).count()
    }

    /// Number of lines for which `f` holds.
    fn count(&self, f: impl Fn(&str) -> bool) -> usize {
        self.lines.iter().filter(|l| f(l)).count()
    }

    /// Fraction of lines for which `f` holds.
    fn share(&self, f: impl Fn(&str) -> bool) -> f32 {
        if self.lines.is_empty() {
            return 0.0;
        }
        self.count(f) as f32 / self.lines.len() as f32
    }
}

/// Word weights, clues and checks by language index, built on first use.
struct Model {
    /// Words of case sensitive languages: `(language index, weight)`.
    exact: Map<String, Vec<(u16, f32)>>,
    /// Lowercased words of case insensitive languages.
    folded: Map<String, Vec<(u16, f32)>>,
    /// Words most languages have, lowercased. When one shows up, languages
    /// that lack it lose this much.
    common: Map<String, f32>,
    /// A small penalty that grows with the vocabulary, so that a language
    /// whose words are a superset of another's loses ties.
    prior: Vec<f32>,
    /// [`CLUES`] by language index, merged per place and weight.
    clues: Vec<(usize, At, f32, Vec<&'static str>)>,
    /// [`CHECKS`] by language index.
    checks: Vec<(usize, Check)>,
}

fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(build_model)
}

/// English words that some languages use as keywords. They turn up in prose
/// and comments everywhere, so they count for little.
const ENGLISH: &[&str] = &[
    "about", "above", "after", "all", "an", "and", "any", "as", "at", "before", "below", "between", "but", "by",
    "each", "every", "first", "for", "from", "has", "have", "if", "in", "into", "is", "it", "its", "last", "more",
    "my", "no", "not", "of", "on", "one", "only", "or", "out", "over", "some", "than", "that", "the", "then", "these",
    "this", "to", "under", "up", "use", "used", "what", "when", "where", "which", "while", "with", "you", "your",
];

fn build_model() -> Model {
    let all = Language::all();
    // Every word of every language with its weight. Long lists are diluted:
    // one of 500 builtins says less than one of 30 keywords.
    let mut vocab: Vec<HashMap<String, f32>> = vec![HashMap::new(); all.len()];
    for (lang, words) in all.iter().zip(&mut vocab) {
        let Some(spec) = lang.spec() else { continue };
        let lists: [(&[&str], f32); 7] = [
            (spec.keywords, 1.0),
            (spec.function_keywords, 1.0),
            (spec.type_keywords, 1.0),
            (spec.variables, 0.8),
            (spec.types, 0.7),
            (spec.constants, 0.7),
            (spec.builtins, 0.5),
        ];
        for (list, weight) in lists {
            let weight = weight * (40.0 / list.len().max(40) as f32).sqrt();
            for word in list.iter().filter(|w| w.len() > 1 && w.bytes().all(is_word)) {
                let key = if spec.case_insensitive { word.to_ascii_lowercase() } else { word.to_string() };
                let w = words.entry(key).or_insert(0.0);
                *w = w.max(weight);
            }
        }
    }
    let mut df: HashMap<String, f32> = HashMap::new();
    for word in vocab.iter().flat_map(|v| v.keys()) {
        *df.entry(word.to_ascii_lowercase()).or_default() += 1.0;
    }
    let n = vocab.iter().filter(|v| !v.is_empty()).count() as f32 + 1.0;
    let common = df.iter().filter(|(_, &d)| d / n >= 0.25).map(|(w, &d)| (w.clone(), d / n)).collect();
    let mut model = Model {
        exact: Map::default(),
        folded: Map::default(),
        common,
        prior: vocab.iter().map(|v| 0.1 * (v.len() as f32 + 1.0).ln()).collect(),
        clues: Vec::new(),
        checks: Vec::new(),
    };
    for (i, words) in vocab.into_iter().enumerate() {
        let insensitive = all[i].spec().is_some_and(|s| s.case_insensitive);
        for (word, weight) in words {
            let lower = word.to_ascii_lowercase();
            let english = if ENGLISH.contains(&lower.as_str()) { 0.25 } else { 1.0 };
            let idf = (n / df[&lower]).ln().min(4.0);
            // Having a common word pays back the penalty for seeing it.
            let entry = (i as u16, weight * idf * english + model.common.get(&lower).copied().unwrap_or(0.0));
            let map = if insensitive { &mut model.folded } else { &mut model.exact };
            map.entry(word).or_default().push(entry);
        }
    }
    // Ids missing from a trimmed build are skipped.
    let index = |id| Language::find(id).and_then(|l| all.iter().position(|x| *x == l));
    for &(id, at, each, patterns) in CLUES {
        let Some(i) = index(id) else { continue };
        match model.clues.iter_mut().find(|c| c.0 == i && c.1 == at && c.2 == each) {
            Some(clue) => clue.3.extend_from_slice(patterns),
            None => model.clues.push((i, at, each, patterns.to_vec())),
        }
    }
    for &(id, check) in CHECKS {
        model.checks.extend(index(id).map(|i| (i, check)));
    }
    model
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Brackets and operators, which code has plenty of and prose does not.
fn is_symbol(b: &u8) -> bool {
    matches!(b, b'{' | b'}' | b'[' | b']' | b'(' | b')' | b'<' | b'>' | b'=' | b';' | b'$' | b'&' | b'|')
}

/// Lines that are probably comments in some language. Their words are prose
/// and say little about the language, so they are skipped.
fn is_comment_line(line: &str) -> bool {
    ["//", "/*", "* ", "# ", "-- ", ";", "% ", "<!--", "\"\"\""].iter().any(|p| line.starts_with(p))
}

fn score_words(sample: &Sample, model: &Model, scores: &mut [f32]) {
    let mut seen: HashSet<&str, BuildHasherDefault<FxHasher>> =
        HashSet::with_capacity_and_hasher(256, Default::default());
    let mut lower = String::new();
    let mut missing = 0.0;
    for line in sample.lines.iter().filter(|l| !is_comment_line(l)) {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if !is_word(bytes[i]) {
                i += 1;
                continue;
            }
            let start = i;
            while i < bytes.len() && is_word(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            if word.len() < 2 || bytes[start].is_ascii_digit() || !seen.insert(word) {
                continue;
            }
            for &(lang, weight) in model.exact.get(word).into_iter().flatten() {
                scores[lang as usize] += weight;
            }
            lower.clear();
            lower.push_str(word);
            lower.make_ascii_lowercase();
            // `Select` says less about a case insensitive language than `select` or `SELECT`.
            let mixed = word != lower && word.bytes().any(|b| b.is_ascii_lowercase());
            let factor = if mixed { 0.4 } else { 1.0 };
            for &(lang, weight) in model.folded.get(&lower).into_iter().flatten() {
                scores[lang as usize] += weight * factor;
            }
            missing += model.common.get(&lower).copied().unwrap_or(0.0);
        }
    }
    for (score, lang) in scores.iter_mut().zip(Language::all()) {
        if lang.spec().is_some() {
            *score -= missing;
        }
    }
}

/// Comment markers that, at the start of a line, almost surely open a comment.
const MARKERS: [&str; 6] = ["//", "#", "--", ";", "%", "/*"];

/// How many lines (up to 5) start with each of [`MARKERS`].
fn comment_lines(sample: &Sample) -> [f32; MARKERS.len()] {
    MARKERS.map(|marker| {
        let comment = |l: &&&str| {
            l.strip_prefix(marker).is_some_and(|rest| match marker {
                // `#include`, `#[attr]` and `#!` are not comments.
                "#" => rest.is_empty() || rest.starts_with([' ', '#']),
                "--" => rest.is_empty() || rest.starts_with(' '),
                "%" => rest.is_empty() || rest.starts_with([' ', '%']),
                _ => true,
            })
        };
        sample.lines.iter().filter(comment).count().min(5) as f32
    })
}

/// Bonus for lines that start with the language's comment markers, penalty
/// for lines that start with a marker it does not have.
fn score_comments(counts: &[f32; MARKERS.len()], spec: &Spec) -> f32 {
    let mut score = 0.0;
    for (marker, &count) in MARKERS.iter().zip(counts).filter(|(_, &c)| c > 0.0) {
        let has = spec.line_comments.iter().any(|c| c.starts_with(marker))
            || spec.block_comments.iter().any(|(open, _)| open.starts_with(marker));
        score += if has { count * 0.6 } else { count * -1.5 };
    }
    score
}

type Check = fn(&Sample) -> f32;

/// `count` pieces of evidence worth `each`, up to `cap`.
fn points(count: usize, each: f32, cap: f32) -> f32 {
    (count as f32 * each).min(cap)
}

/// `score` if `condition` holds.
fn when(condition: bool, score: f32) -> f32 {
    if condition {
        score
    } else {
        0.0
    }
}

/// Where the patterns of a clue are looked for.
#[derive(Clone, Copy, PartialEq)]
enum At {
    /// Anywhere: each distinct pattern found counts once.
    Text,
    /// At the start of lines (after indentation): each such line counts.
    Lines,
}

use At::{Lines, Text};

/// Telltale snippets: `(language id, where, points per match, patterns)`.
/// A clue is worth at most four matches. The small ones settle close calls
/// between languages that share words, the big ones identify formats.
const CLUES: &[(&str, At, f32, &[&str])] = &[
    ("javascript", Text, 4.0, JS),
    ("jsx", Text, 4.0, JS),
    ("typescript", Text, 4.0, JS),
    ("tsx", Text, 4.0, JS),
    ("python", Text, 4.0, &["elif ", "self.", "__init__", "):\n", "print("]),
    ("python", Text, 4.0, &["None", "True", "False", " is not ", " import "]),
    ("ruby", Text, 4.0, &["do |", "puts ", "require '", "attr_accessor"]),
    ("ruby", Text, 4.0, &["elsif ", ".each", "#{", "end\n", "{ |", "(&:"]),
    ("lua", Text, 4.0, &["local ", "~=", "elseif ", "ipairs(", "pairs(", " .. ", " then\n", "end\n", "nil", " or {}"]),
    ("r", Text, 4.0, &[" <- ", "library(", "<- function(", "%>%", "data.frame(", "TRUE", "NULL", "%%", "%in%"]),
    ("go", Text, 4.0, &["package main", ":= ", "err != nil", "func (", "fmt.", "import ("]),
    ("go", Text, 4.0, &["go func", "`json:", "defer ", "[]string", "\n\treturn "]),
    ("rust", Text, 4.0, &["fn ", "let mut ", "impl ", "&self", "println!", "use std::", "#[derive", "Vec<", "&str"]),
    ("rust", Text, 4.0, &["i32", "::new(", "pub fn", "Some(", "Ok(", "&mut ", ".iter()", ".unwrap()", "usize", "..="]),
    ("java", Text, 4.0, &["System.out", "import java.", "@Override", "String[] args", " throws "]),
    ("java", Text, 4.0, &["public static ", "<>()", "new ArrayList", "private final "]),
    ("java", Text, 4.0, &["public class ", "Optional<", ".printStackTrace()", "List<"]),
    ("c", Text, 4.0, &["#include <stdio.h>", "#include <stdlib.h>", "printf(", "malloc(", "sizeof"]),
    ("c", Text, 4.0, &["size_t", "NULL", "memcpy(", "strlen(", "char *", "int *", "typedef ", "#define "]),
    ("cpp", Text, 4.0, &["std::", "template", "namespace ", "public:", "private:", "nullptr", "#include <iostream>"]),
    ("csharp", Text, 4.0, &["using System", "{ get;", "Console.", "async Task", "namespace "]),
    ("csharp", Text, 4.0, &["public string ", ".Where(", ".ToList()", "await _"]),
    ("kotlin", Text, 4.0, &["fun ", "val ", "data class", "println(", "listOf(", "?.", "!!", "when ("]),
    ("swift", Text, 4.0, &["import SwiftUI", "import Foundation", "guard ", "some View", "@State"]),
    ("swift", Text, 4.0, &["\\(", "func ", "$0", "let ", "print(", "Codable", "?? ", "-> Void", " in\n"]),
    ("dart", Text, 4.0, &["void main()", "final ", "@override", "Widget", "import 'package:", "late "]),
    ("dart", Text, 4.0, &["required ", "setState(", "super.key", "BuildContext", "=> print("]),
    ("scala", Text, 4.0, &["case class", "object ", "def ", "val ", "extends App"]),
    ("scala", Text, 4.0, &["import scala", "implicit ", "(_", "trait ", ".mkString"]),
    ("haskell", Text, 4.0, &[" :: ", "module ", " where\n", "import qualified", " <- ", "do\n", "$ ", "deriving"]),
    ("elixir", Text, 4.0, &["|>", "defmodule", "defp ", "do:", "%{", ":ok", " do\n", "IO.", "Enum.", " end"]),
    ("perl", Text, 4.0, &["my $", "my @", "my %", "use strict", "=~", "foreach my", "@_", "$_"]),
    ("bash", Text, 4.0, &["$(", "${", "[[ ", "; then", "; do", "\nfi", "esac", "echo ", "[ -", "alias ", "$HOME"]),
    ("powershell", Text, 4.0, &["-eq ", "-ne ", "-gt ", "param(", "[string]", "Write-Host", "Get-", "$_."]),
    ("fish", Text, 4.0, &["set -l ", "set -g", "set -x ", "$argv", "; and ", "; or "]),
    ("objective-c", Text, 4.0, &["@interface", "@implementation", "@end", "#import", "NSString", "@property"]),
    ("cuda", Text, 4.0, &["__global__", "<<<", "cudaMalloc", "__device__", "threadIdx"]),
    ("groovy", Text, 4.0, &["println ", "import groovy", "@CompileStatic"]),
    ("gas", Text, 4.0, &[".globl", ".section", "%r", "%e", ".type"]),
    ("riscv", Text, 4.0, &["ecall", " a0", " t0", " ra", "li ", "la "]),
    ("reason", Text, 4.0, &["switch (", "let rec ", " ++ ", "(~", "[@react", "=> {", "Js.", "{j|"]),
    ("purescript", Text, 4.0, &["import Prelude", "forall ", "Effect", "import Effect", " <> ", "case _ of", "Array "]),
    ("elm", Text, 4.0, &[" exposing (", "type alias ", "import Html", " of\n", "Msg"]),
    ("actionscript", Text, 4.0, &["import flash.", ":void", ":String"]),
    ("actionscript", Text, 4.0, &["public function", "extends Sprite", "package "]),
    ("haxe", Text, 4.0, &["package;", "static function", "trace(", ":Int", "using ", "@:"]),
    ("qml", Text, 4.0, &["import QtQuick", "anchors.", "property ", "Rectangle {", "Item {", "id: "]),
    ("nushell", Text, 4.0, &["| where ", "| each ", "$in", "| get ", "| sort-by"]),
    ("gleam", Text, 4.0, &["import gleam", "pub fn ", "let assert", "io.println", ": String"]),
    ("coffeescript", Text, 4.0, &["->", "unless ", "@", "#{", " is ", " isnt "]),
    ("livescript", Text, 4.0, &["require! ", "~>", "-->", "<|", "!\n", "!."]),
    ("moonscript", Text, 4.0, &["\\", "with ", ": =>", "-> @", "=> @"]),
    ("v", Text, 4.0, &["fn main()", "mut ", "module main", "println(", " or {", "pub fn "]),
    ("cue", Text, 4.0, &["package ", "#", "string &", "int &", "=~"]),
    ("nix", Text, 4.0, &["{ pkgs", "pkgs.", "mkDerivation", "inherit ", "with pkgs", "let\n"]),
    ("odin", Text, 4.0, &[":: proc", ":: struct", "fmt.println", "context."]),
    ("jsonnet", Text, 4.0, &["local ", "std.", "$.", "+:"]),
    ("mojo", Text, 4.0, &["fn ", "struct ", "var ", "inout ", "owned "]),
    ("xonsh", Text, 4.0, &["$(", "![", "$[", "@("]),
    // Formats
    ("handlebars", Text, 12.0, &["{{#", "{{/", "{{!", "{{else", "{{>"]),
    ("gotemplate", Text, 15.0, &["{{ .", "{{.", "{{ end }}", "{{ range", "{{ define", "{{- "]),
    ("jinja", Text, 6.0, &["{% macro", "{% from", "url_for(", "|safe", "{% endmacro"]),
    ("twig", Text, 6.0, &[".twig", "|trans", "|raw", "{% set", "{{ dump("]),
    ("liquid", Text, 6.0, &["{% assign", "{% capture", "{% endcomment", "{%- liquid", "{% render"]),
    ("blade", Text, 12.0, &["@extends(", "@section(", "@yield(", "@endforeach", "@endif", "{{--", "@csrf"]),
    ("razor", Text, 12.0, &["@model ", "@using ", "@inject ", "@page", "@{", "@*", "@foreach ("]),
    ("haml", Lines, 9.0, &["%html", "%head", "%body", "%div", "%p", "%a", "!!!", "-#"]),
    ("pug", Lines, 9.0, &["//-", "mixin ", "doctype", "include ", "block ", "extends "]),
    ("slim", Lines, 8.0, &["doctype", "/ ", "html lang=", "css:", "javascript:", "== "]),
    ("latex", Text, 12.0, &["\\documentclass", "\\begin{", "\\usepackage", "\\section", "\\end{"]),
    ("bibtex", Lines, 12.0, &["@article{", "@book{", "@inproceedings{", "@misc{", "@string{", "@online{"]),
    ("rst", Lines, 8.0, &[".. ", "===="]),
    ("asciidoc", Lines, 6.0, &["= ", "== ", "=== ", "[source", "----", ":toc:"]),
    ("org", Lines, 10.0, &["#+"]),
    ("typst", Lines, 10.0, &["#set ", "#let ", "#show ", "#import "]),
    ("roff", Lines, 10.0, &[".TH ", ".SH ", ".B ", ".I ", ".PP", ".TP", ".\\\""]),
    ("texinfo", Lines, 10.0, &["\\input texinfo", "@node ", "@chapter ", "@section ", "@setfilename", "@c "]),
    ("pod", Lines, 10.0, &["=head1", "=head2", "=cut", "=over", "=item", "=pod", "=encoding"]),
    ("gherkin", Lines, 12.0, &["Feature:", "Scenario:", "Background:", "Given ", "When ", "Then ", "Examples:"]),
    ("gitconfig", Lines, 10.0, &["[user]", "[core]", "[alias]", "[remote ", "[branch ", "[push]", "[pull]", "[init]"]),
    ("systemd", Lines, 12.0, &["[Unit]", "[Service]", "[Install]", "[Timer]", "[Socket]", "ExecStart="]),
    ("sshconfig", Lines, 10.0, &["Host ", "HostName ", "IdentityFile ", "User ", "Port ", "ServerAlive", "ProxyJump "]),
    ("caddyfile", Text, 10.0, &["reverse_proxy", "file_server", "encode ", "respond ", "root * ", "handle ", "tls "]),
    ("nginx", Text, 8.0, &["server {", "location ", "listen ", "server_name ", "proxy_pass ", "worker_processes"]),
    ("apacheconf", Text, 10.0, &["<VirtualHost", "<Directory", "DocumentRoot ", "ServerName ", "RewriteRule "]),
    ("dockerfile", Lines, 6.0, &["FROM ", "RUN ", "COPY ", "ADD ", "WORKDIR ", "ENV ", "ARG ", "CMD "]),
    ("dockerfile", Lines, 6.0, &["ENTRYPOINT ", "EXPOSE ", "LABEL ", "USER ", "VOLUME ", "HEALTHCHECK ", "SHELL "]),
    ("ninja", Lines, 10.0, &["rule ", "build ", "command = ", "ninja_required_version"]),
    ("just", Lines, 12.0, &["set shell", "set dotenv", "set export"]),
    ("just", Text, 6.0, &[" := ", "{{", "@just "]),
];

/// Checks that need more than a list of patterns, added to the other scores.
/// Languages with hand written lexers rely on clues and checks alone.
const CHECKS: &[(&str, Check)] = &[
    // Markup
    ("html", |s| points(s.found(HTML_TAGS), 3.0, 24.0) + when(s.has_ci("<!doctype html"), 30.0)),
    ("xml", |s| if s.has("<?xml") { 40.0 } else { xml(s) }),
    ("svg", |s| when(s.found(&["<svg ", "<svg>", "<svg\n"]) > 0, 45.0)),
    ("vue", |s| when(s.starting("<template") > 0 && s.found(&["<script", "<style"]) > 0, 45.0)),
    ("svelte", svelte),
    ("astro", |s| when(front_matter(s).is_some_and(|f| f.contains("import ") || f.contains("Astro.")), 45.0)),
    ("php", php),
    ("markdown", markdown),
    ("mdx", mdx),
    ("jinja", template),
    ("twig", template),
    ("liquid", template),
    ("erb", |s| when(s.has("<%"), 20.0 + points(s.found(&[" do |", " end ", "@", ".each"]), 6.0, 18.0))),
    ("ejs", |s| when(s.has("<%"), 20.0 + points(s.found(&["function", "=>", "}) %>", "include("]), 6.0, 18.0))),
    // Data and config
    ("json", |s| when(json(s) && !s.has("//") && !s.has("/*"), 40.0)),
    ("jsonc", |s| when(json(s) && s.starting_any(&["//", "/*"]) > 0, 45.0)),
    ("json5", |s| when(json5(s), 45.0)),
    ("yaml", yaml),
    ("toml", |s| when(toml(s), 30.0 + points(s.starting("["), 3.0, 9.0))),
    ("ini", |s| when((0.0..=0.8).contains(&assignments(s)) && s.starting("[") > 0 && s.lines.len() > 1, 30.0)),
    ("properties", |s| when(s.starting("[") == 0 && s.lines.len() > 1 && !sentences(s), properties(s))),
    ("dotenv", |s| when(s.share(is_env) > 0.0 && s.share(|l| is_env(l) || l.starts_with("# ")) > 0.9, 30.0)),
    ("gitignore", |s| when(s.count(is_glob) > 1, 25.0 * s.share(|l| l.starts_with("# ") || is_glob(l)))),
    ("gitattributes", |s| {
        when(s.count(is_attributes) > 0, 40.0 * s.share(|l| l.starts_with("# ") || is_attributes(l)))
    }),
    ("crontab", |s| points(s.count(is_cron), 15.0, 45.0)),
    ("procfile", |s| when(s.share(is_process) > 0.5 && s.share(|l| l.starts_with('#') || is_process(l)) > 0.8, 40.0)),
    ("diff", diff),
    ("http", |s| when(s.count(is_request) > 0, 40.0)),
    ("log", |s| when(s.share(is_log) > 0.2, 40.0 * s.share(is_log) + points(s.found(LOG_LEVELS), 3.0, 12.0))),
    ("csv", |s| separated(s, ',')),
    ("tsv", |s| separated(s, '\t')),
    ("gcode", |s| when(s.share(is_gcode) > 0.3, 35.0 * s.share(|l| l.starts_with([';', '(', '%']) || is_gcode(l)))),
    ("apl", |s| {
        when(!s.text.is_ascii(), points(s.text.matches(['⍝', '←', '⍵', '⍺', '⌿', '⍳', '⍴']).count(), 4.0, 40.0))
    }),
    ("kdl", |s| {
        when(s.has(" {") && !s.has("(define"), points(s.found(&["#true", "#false", "#null", "/-"]), 10.0, 30.0))
    }),
    ("regex", |s| {
        when(s.found(REGEX_GROUPS) > 0, 30.0 * s.share(|l| l.starts_with(['(', ')', '^', '[', '\\', '|', '$'])))
    }),
    // Style sheets
    ("css", |s| sheet(s, braces(s) && sass(s) + less(s) + s.starting("//") == 0, 0)),
    ("scss", |s| sheet(s, braces(s), sass(s) + s.starting("//"))),
    ("less", |s| sheet(s, braces(s) && less(s) > 0, less(s))),
    ("sass", |s| sheet(s, !braces(s) && sass(s) > 0, sass(s))),
    ("stylus", |s| sheet(s, !braces(s) && sass(s) == 0, 0)),
    // Build files
    ("dockerfile", |s| when(s.starting("FROM ") > 0, 12.0)),
    ("makefile", makefile),
    ("meson", |s| when(s.found(&["project(", "meson."]) > 0, points(s.found(MESON), 8.0, 40.0))),
    // Close relatives that share most of their words
    ("javascript", |s| javascript(s, false, false)),
    ("jsx", |s| javascript(s, false, true)),
    ("typescript", |s| javascript(s, true, false)),
    ("tsx", |s| javascript(s, true, true)),
    ("c", |s| points(s.shapes.typed_declarations, 2.0, 6.0)),
    ("cpp", |s| points(s.shapes.typed_declarations, 2.0, 6.0)),
    ("java", |s| points(s.shapes.typed_declarations, 2.0, 6.0)),
    ("csharp", |s| points(s.shapes.typed_declarations, 2.0, 6.0)),
    ("groovy", |s| -10.0 * semicolons(s)),
    ("haskell", |s| points(s.shapes.type_signatures, 4.0, 12.0)),
    ("purescript", |s| points(s.shapes.type_signatures, 4.0, 12.0)),
    ("python", |s| points(s.shapes.python_blocks, 4.0, 12.0)),
    ("bash", |s| points(s.shapes.commands, 3.0, 12.0)),
    ("powershell", |s| points(s.shapes.cmdlets, 5.0, 15.0)),
    ("sql", |s| points(SQL.iter().filter(|p| s.has_ci(p)).count(), 4.0, 16.0)),
    ("reason", |s| points(s.count(|l| l.starts_with("| ") && l.contains("=>")), 3.0, 12.0)),
    ("asm", |s| -points(s.found(&[".globl", ".section", "%r", "%e", ".type"]), 5.0, 20.0)),
    // Supersets of popular languages lose ties.
    ("mojo", |_| -5.0),
    ("xonsh", |_| -5.0),
];

const LOG_LEVELS: &[&str] = &[" INFO ", " WARN ", " ERROR ", " DEBUG "];

const REGEX_GROUPS: &[&str] = &["(?:", "(?<", "(?=", "(?!", "(?P", "(?x", "(?i"];

const MESON: &[&str] = &["project(", "executable(", "dependency(", "library(", "meson."];

const JS: &[&str] =
    &["console.", "=> ", "const ", "let ", "function ", "===", "require(", "document.", "export ", "module.exports"];

const SQL: &[&str] = &[
    "select ", " from ", " where ", "insert into", "create table", "order by", "group by", " join ", "delete from",
    "primary key", "not null",
];

const HTML_TAGS: &[&str] = &[
    "<div", "<span", "<p>", "<p ", "<a href", "<ul", "<li", "<head", "<body", "<table", "<img ", "<button", "<input",
    "<form", "<h1", "<h2", "<h3", "<section", "<nav", "<meta ", "<link ", "<title", "<br", "<label", "<header",
    "<footer", "<main", "<tr", "<td", "<th>", "<strong", "<em>", "<code>",
];

/// Generic XML: mostly lines of tags, and no HTML tags.
fn xml(s: &Sample) -> f32 {
    let tagged = s.share(|l| l.starts_with('<'));
    if tagged < 0.5 || !s.has("</") || s.found(HTML_TAGS) > 1 {
        return 0.0;
    }
    20.0 * tagged + if s.has("xmlns") { 10.0 } else { 0.0 }
}

/// The text between a leading `---` line and the next one.
fn front_matter<'a>(s: &Sample<'a>) -> Option<&'a str> {
    let rest = s.text.strip_prefix("---")?.strip_prefix(['\n', '\r'])?;
    rest.find("\n---").map(|end| &rest[..end])
}

/// `<?php`, or statements like `$name = ...;` and `$user->name`.
fn php(s: &Sample) -> f32 {
    if s.has("<?php") {
        return 50.0;
    }
    let statement = |l: &&&str| l.starts_with('$') && l.ends_with(';') && (l.contains(" = ") || l.contains("->"));
    let statements = s.lines.iter().filter(statement).count();
    if statements == 0 && !s.has("$this->") && !(s.has("function ") && s.has("($")) {
        return 0.0;
    }
    points(statements, 4.0, 12.0) + points(s.found(&["$this->", "<?=", "->", "echo ", "$_", "($", " . $"]), 4.0, 16.0)
}

/// Markdown with `import` or `export` lines and components.
fn mdx(s: &Sample) -> f32 {
    if s.starting_any(&["import ", "export "]) == 0 || s.starting("<") == 0 {
        return 0.0;
    }
    markdown(s) + 15.0
}

/// Headings, fences, links, lists and emphasis.
fn markdown(s: &Sample) -> f32 {
    let headings = s.lines.iter().filter(|l| l.starts_with('#') && l.trim_start_matches('#').starts_with(' ')).count();
    let deep = s.starting("## ") + s.starting("### ");
    points(headings, 2.0, 6.0)
        + points(deep, 3.0, 9.0)
        + points(s.starting("```"), 4.0, 12.0)
        + points(s.text.matches("](").count(), 3.0, 9.0)
        + points(s.starting_any(&["- ", "* ", "1. ", "> "]), 1.0, 5.0)
        + points(s.found(&["**", "__", "`"]), 1.0, 3.0)
}

/// Svelte blocks and directives, in a component rather than a Handlebars template.
fn svelte(s: &Sample) -> f32 {
    let markers = ["{#if", "{#each", "{#await", "{:else", "{@html", " on:", " bind:", "$: ", "{/if}", "{/each}"];
    when(s.has("<") && !s.has("{{"), points(s.found(&markers), 15.0, 60.0))
}

/// Tags inside `{% %}`, shared by Jinja, Twig and Liquid.
fn template(s: &Sample) -> f32 {
    points(s.found(&["{% if", "{% for", "{% end", "{% block", "{% extends", "{%- ", "{# ", "{{ "]), 5.0, 30.0)
}

/// Lines without leading comments, for the data formats below.
fn data_lines<'a>(s: &'a Sample) -> impl Iterator<Item = &'a str> + 'a {
    s.lines.iter().copied().filter(|l| !l.starts_with("//") && !l.starts_with("/*") && !l.starts_with('#'))
}

/// A JSON document: braces or brackets around quoted keys and plain values.
fn json(s: &Sample) -> bool {
    let first = data_lines(s).next().unwrap_or("");
    first.starts_with(['{', '[']) && data_lines(s).all(|l| l.starts_with(['"', '{', '}', '[', ']']) || is_value(l))
}

/// A number, date, boolean or null at the start of `line`.
fn is_value(line: &str) -> bool {
    let number = line.split([',', ' ', '}', ']']).next().unwrap_or("");
    let number = number.strip_prefix(['-', '+']).unwrap_or(number);
    (number.starts_with(|c: char| c.is_ascii_digit())
        && number.bytes().all(|b| b.is_ascii_hexdigit() || b"._:-+xoTZ".contains(&b)))
        || ["true", "false", "null"].iter().any(|v| line.starts_with(v))
}

/// JSON with unquoted keys or single quotes.
fn json5(s: &Sample) -> bool {
    let first = data_lines(s).next().unwrap_or("");
    let bare_key = |l: &str| l.split_once(':').is_some_and(|(k, _)| !k.is_empty() && k.bytes().all(is_word));
    first.starts_with(['{', '[']) && data_lines(s).any(bare_key) && !s.has(";") && !s.has("function")
}

/// `key: value` lines, list items and comments, without tabs or semicolons.
fn yaml(s: &Sample) -> f32 {
    let tag = |l: &str| l.starts_with('<') && l.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic);
    if s.has("\n\t") || s.lines.iter().any(|l| l.ends_with([';', '{']) || tag(l)) || sentences(s) {
        return 0.0;
    }
    let is_key = |l: &str| {
        let key = l.split_once(": ").map_or(l.strip_suffix(':').unwrap_or(""), |(k, _)| k);
        !key.is_empty() && key.bytes().all(|b| is_word(b) || b"-. \"'".contains(&b))
    };
    let keys = s.share(is_key);
    let share = s.share(|l| l.starts_with(['#', '-']) || is_key(l));
    if keys > 0.3 && share > 0.6 && s.lines.len() > 1 {
        30.0 * share
    } else {
        0.0
    }
}

/// Mostly lines that end like sentences: prose rather than data.
fn sentences(s: &Sample) -> bool {
    s.share(|l| l.ends_with(['.', '?', '!'])) > 0.4
}

/// `key = "value"` lines with TOML values, and no INI style comments or indentation.
fn toml(s: &Sample) -> bool {
    assignments(s) > 0.8 && s.lines.len() > 1 && s.starting(";") == 0 && !s.has("\n\t")
}

/// Java style `.properties`: `key=value` and `key: value` lines.
fn properties(s: &Sample) -> f32 {
    let share = s.share(is_property);
    when(share > 0.7, 25.0 * share)
}

/// For a file made of `[section]` and `key = value` lines: the share of
/// values typed like TOML (a quoted string, number, boolean, array or table).
/// Negative when the lines are something else.
fn assignments(s: &Sample) -> f32 {
    let (mut typed, mut total, mut other) = (0, 0, 0);
    for line in s.lines.iter().filter(|l| !l.starts_with(['#', ';', '['])) {
        let Some((key, value)) = line.split_once('=') else {
            other += 1;
            continue;
        };
        if key.trim().is_empty() || key.trim().contains([' ', '(', '{', '<', '$']) {
            other += 1;
            continue;
        }
        total += 1;
        let value = value.trim();
        if value.starts_with(['"', '\'', '[', '{']) || is_value(value) {
            typed += 1;
        }
    }
    if total == 0 || other * 3 > total {
        return -1.0;
    }
    typed as f32 / total as f32
}

/// Known CSS properties used as declarations, 4 points for each distinct one.
fn css(s: &Sample) -> f32 {
    const PROPERTIES: &[&str] = &[
        "color", "background", "background-color", "margin", "padding", "display", "font", "font-size", "font-family",
        "font-weight", "border", "border-radius", "width", "height", "position", "top", "left", "flex", "grid", "gap",
        "transition", "transform", "opacity", "cursor", "content", "z-index", "overflow", "text-align", "line-height",
        "box-shadow", "list-style", "max-width", "min-height", "text-decoration", "align-items", "justify-content",
    ];
    *s.css.get_or_init(|| {
        let mut seen = HashSet::new();
        // Splitting bytes is much faster than splitting chars, and stays valid UTF-8.
        for decl in s.text.as_bytes().split(|b| matches!(b, b'{' | b'}' | b';' | b'\n')) {
            let decl = std::str::from_utf8(decl).unwrap_or("");
            let key = decl.trim().split([':', ' ']).next().unwrap_or("");
            if PROPERTIES.contains(&key) || (key.starts_with("--") && key.len() > 2 && decl.contains(':')) {
                seen.insert(key);
            }
        }
        points(seen.len(), 4.0, 32.0)
    })
}

/// Style sheet evidence when the text has the dialect's `shape`: known
/// properties, plus `extra` points for syntax only the dialect has.
fn sheet(s: &Sample, shape: bool, extra: usize) -> f32 {
    if !shape {
        return 0.0;
    }
    let css = css(s);
    when(css > 0.0, css + points(extra, 4.0, 16.0))
}

/// Style sheet with braces and semicolons rather than indentation.
fn braces(s: &Sample) -> bool {
    s.has("{") && s.share(|l| l.ends_with([';', '{', '}', ','])) > 0.5
}

/// Sass and SCSS only syntax.
fn sass(s: &Sample) -> usize {
    s.starting_any(&["$", "@mixin", "@include", "@extend", "=", "+"]) + s.found(&["#{"])
}

/// `@name: value;` variables in Less (not `@media` and other at-rules).
fn less(s: &Sample) -> usize {
    let variable = |l: &&&str| {
        l.strip_prefix('@')
            .and_then(|rest| rest.split_once(':'))
            .is_some_and(|(name, _)| !name.is_empty() && name.bytes().all(|b| is_word(b) || b == b'-'))
    };
    s.lines.iter().filter(variable).count()
}

/// Rules (`target: prerequisites` and a tab indented recipe) and variables.
fn makefile(s: &Sample) -> f32 {
    // Code in brace languages has `label:` lines too, so plain rules only
    // count alongside make variables or in text without braces.
    let variables = s.found(&["$(", "$@", "$<", "$^", ".PHONY"]);
    if variables == 0 && s.has("{") {
        return 0.0;
    }
    let rule = |l: &str| {
        !l.starts_with([' ', '\t', '#'])
            && !l.ends_with('{')
            && l.split_once(':').is_some_and(|(target, rest)| !target.is_empty() && !rest.starts_with('='))
    };
    let rules = s.text.lines().zip(s.text.lines().skip(1)).filter(|(a, b)| rule(a) && b.starts_with('\t')).count();
    points(rules, 10.0, 20.0) + points(variables, 5.0, 20.0) + if s.has(".PHONY") { 10.0 } else { 0.0 }
}

/// `G1 X0 Y0`, `M104 S200`, `N10 ...`.
fn is_gcode(line: &str) -> bool {
    line.starts_with(['G', 'M', 'N', 'O', 'T']) && line.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
}

/// `*.png binary` or `*.sh text eol=lf`.
fn is_attributes(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let pattern = parts.next().unwrap_or("");
    let attributes = ["text", "binary", "eol=", "filter=", "diff", "merge", "linguist-", "-text", "-diff"];
    is_glob(pattern) && parts.next().is_some_and(|a| attributes.iter().any(|p| a.starts_with(p)))
}

/// `key=value`, `key = value` or `key: value` with a dotted or plain key.
fn is_property(line: &str) -> bool {
    if line.starts_with(['#', '!']) {
        return true;
    }
    if line.ends_with([';', ',', '{', '(']) {
        return false;
    }
    line.split_once(['=', ':']).is_some_and(|(key, value)| {
        let key = key.trim_end();
        !key.is_empty() && !value.trim().is_empty() && key.bytes().all(|b| is_word(b) || b == b'.' || b == b'-')
    })
}

/// `NAME=value` or `export NAME=value` with an upper case name.
fn is_env(line: &str) -> bool {
    line.strip_prefix("export ").unwrap_or(line).split_once('=').is_some_and(|(k, _)| {
        !k.is_empty() && k.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// A path pattern such as `*.log`, `/target/` or `!keep.txt`.
fn is_glob(line: &str) -> bool {
    !line.contains([' ', '=', ':', '(', '{', ';', '<', '>', '"', '\''])
        && (line.contains(['*', '/', '.']) || line.starts_with('!'))
}

/// `*/5 * * * * command`.
fn is_cron(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    let schedule = |f: &str| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit() || b"*/,-".contains(&b));
    ["@reboot ", "@daily ", "@hourly ", "@weekly ", "@monthly ", "@yearly "].iter().any(|p| line.starts_with(p))
        || line.contains('*') && (0..5).all(|_| fields.next().is_some_and(schedule)) && fields.next().is_some()
}

/// `name: command --flag`.
fn is_process(line: &str) -> bool {
    line.split_once(": ").is_some_and(|(name, cmd)| {
        name.bytes().all(is_word) && ["$", " -", "exec ", "&&"].iter().any(|p| cmd.contains(p))
    })
}

/// `GET /path HTTP/1.1` and friends.
fn is_request(line: &str) -> bool {
    let method = line.split(' ').next().unwrap_or("");
    ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].contains(&method) && line.contains(" HTTP/")
        || line.starts_with("HTTP/1.1 ")
}

/// CSV or TSV: nearly every line has the same number of separators, at least two.
fn separated(s: &Sample, sep: char) -> f32 {
    let counts: Vec<usize> = s.lines.iter().map(|l| l.matches(sep).count()).collect();
    let n = counts.first().copied().unwrap_or(0);
    let share = counts.iter().filter(|&&c| c == n).count() as f32 / counts.len().max(1) as f32;
    if n < 2 || counts.len() < 3 || share < 0.7 {
        return 0.0;
    }
    40.0 * share
}

/// Share of code lines that end with a semicolon.
fn semicolons(s: &Sample) -> f32 {
    let code = || s.lines.iter().filter(|l| !l.ends_with(['{', '}', '(', ')', ',']) && !is_comment_line(l));
    let total = code().count().max(1);
    code().filter(|l| l.ends_with(';')).count() as f32 / total as f32
}

/// Type annotations and other syntax that only TypeScript has.
fn typescript(s: &Sample) -> f32 {
    let markers = [
        ": string", ": number", ": boolean", ": void", ": any", ": unknown", "}: ", "?: ", "as const", "import type",
        "export type", "export interface", ": Promise<", " | undefined", " | null", "): ", "Record<",
    ];
    let annotated = s.lines.iter().filter(|l| is_annotated_binding(l)).count();
    points(s.found(&markers) + annotated, 4.0, 16.0)
}

/// `const name: Type = ...`. Rust has `const` too, but with upper case names.
fn is_annotated_binding(line: &str) -> bool {
    let line = line.strip_prefix("export ").unwrap_or(line);
    line.strip_prefix("const ")
        .and_then(|rest| rest.split_once(": "))
        .is_some_and(|(name, _)| name.bytes().all(is_word) && name.bytes().any(|b| b.is_ascii_lowercase()))
}

/// Evidence for one of JavaScript, JSX, TypeScript and TSX, which share
/// their words: type annotations and elements tell them apart.
fn javascript(s: &Sample, typed: bool, elements: bool) -> f32 {
    let (ts, jsx) = *s.script.get_or_init(|| (typescript(s), jsx(s)));
    let score = match (typed, elements, jsx) {
        (false, false, false) if ts == 0.0 => 2.0,
        (false, true, true) if ts == 0.0 => 8.0,
        // Without elements TypeScript wins the tie with TSX.
        (true, false, false) => ts + 0.5,
        (true, true, true) => ts + 8.0,
        _ => 0.0,
    };
    // Lua and Ruby close blocks with a bare `end`, and statically typed
    // languages declare `Type name = ...`. These languages do neither.
    let end = if s.shapes.bare_end { 6.0 } else { 0.0 };
    // Vue and Svelte components wrap scripts in markup.
    let markup = if s.starting("<script") > 0 { 10.0 } else { 0.0 };
    score - end - markup - points(s.shapes.typed_declarations, 4.0, 12.0)
}

/// `String name = ...`, `int count;` or `void main(`: a declaration that
/// starts with a type, which JavaScript does not have.
fn is_typed_declaration(line: &str) -> bool {
    const JS: &[&str] = &[
        "async", "await", "case", "class", "const", "delete", "else", "export", "function", "get", "import", "let",
        "new", "return", "set", "static", "throw", "typeof", "var", "yield",
    ];
    if !line.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return false;
    }
    let mut words = line.splitn(3, ' ');
    let (Some(ty), Some(rest)) = (words.next(), words.next()) else { return false };
    let name = rest.split(['(', ';']).next().unwrap_or("");
    let ident = |w: &str| w.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && w.bytes().all(is_word);
    let ty_ok =
        ty.starts_with(|c: char| c.is_ascii_alphabetic()) && ty.bytes().all(|b| is_word(b) || b"<>[],".contains(&b));
    let after = &line[ty.len() + 1 + name.len()..];
    ty_ok && !JS.contains(&ty) && ident(name) && (after.starts_with(" = ") || after.starts_with(['(', ';']))
}

/// File headers and hunks, where every other line starts with `+`, `-` or a space.
fn diff(s: &Sample) -> f32 {
    let headers = points(s.starting_any(&["@@ -", "--- ", "+++ ", "diff --git", "index "]), 10.0, 50.0);
    if s.starting("@@ -") == 0 {
        return headers;
    }
    let lines = s.text.lines().filter(|l| !l.is_empty());
    let share = lines.clone().filter(|l| l.starts_with(['+', '-', ' ', '@'])).count() as f32 / lines.count() as f32;
    headers.max(50.0 * share)
}

/// `name :: Type` at the start of a line, as in Haskell.
fn is_type_signature(line: &str) -> bool {
    let name = line.split(' ').next().unwrap_or("");
    !name.is_empty() && line[name.len()..].starts_with(" :: ") && name.bytes().all(|b| is_word(b) || b == b'\'')
}

/// A line starting with a date such as `2024-01-15`.
fn is_log(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() > 10 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-'
}

/// `def f(x):`, `for x in xs:` and other Python block openers.
fn is_python_block(line: &str) -> bool {
    const KEYWORDS: &[&str] =
        &["def", "class", "if", "elif", "else", "for", "while", "with", "try", "except", "finally"];
    line.trim_end().ends_with(':') && KEYWORDS.contains(&line.split([' ', '(', ':']).next().unwrap_or(""))
}

/// `git log --oneline` and other common shell commands.
fn is_command(line: &str) -> bool {
    const COMMANDS: &[&str] = &[
        "git", "curl", "wget", "cd", "ls", "echo", "sudo", "apt", "apt-get", "brew", "npm", "npx", "yarn", "pip",
        "mkdir", "rm", "cp", "mv", "chmod", "chown", "source", "cat", "grep", "sed", "awk", "docker", "kubectl",
        "cargo", "tar", "ssh",
    ];
    let mut words = line.split(' ');
    COMMANDS.contains(&words.next().unwrap_or("")) && words.next().is_some_and(|w| !w.starts_with(['=', '(', ':', '.']))
}

/// `Get-ChildItem -Path .` and other `Verb-Noun` commands.
fn is_cmdlet(line: &str) -> bool {
    if !line.starts_with(|c: char| c.is_ascii_uppercase()) {
        return false;
    }
    let word = line.split([' ', '(']).next().unwrap_or("");
    word.split_once('-').is_some_and(|(verb, noun)| {
        let capitalized =
            |w: &str| w.starts_with(|c: char| c.is_ascii_uppercase()) && w.bytes().all(|b| b.is_ascii_alphabetic());
        capitalized(verb) && capitalized(noun)
    })
}

/// JSX elements: closing tags or self closing components.
fn jsx(s: &Sample) -> bool {
    // Vue and Svelte components wrap their code in a `<script>` element instead.
    s.starting("<script") == 0
        && (s.has("</") || s.has("/>"))
        && s.found(&["return (", "return <", "className=", "=> (", "<>", "React", "={"]) > 0
}
