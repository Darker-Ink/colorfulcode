//! Targeted checks for the most used languages.

mod common;

use colorfulcode::Kind::*;
use common::{assert_not, assert_tokens};

#[test]
fn tsx_generic_arrow_functions() {
    let cases = [
        "const id = <T,>(x: T): T => x;",
        "const f = async <T extends object>(a: T) => a;",
        "const g = <T = string,>(v: T) => v;",
        "const h = <T>(x: T): T => x;",
        "const k = <const T,>(x: T) => x;",
        "const m = <T, U extends T[]>(a: T, b: U) => [a, ...b];",
        "items.map(<T,>(x: T) => x)",
    ];
    for code in cases {
        assert_tokens("tsx", code, &[("T", Type), ("=>", Operator)]);
        assert_not("tsx", code, "T", Tag);
        assert_not("tsx", code, "T", Text);
    }
}

#[test]
fn tsx_elements_and_components() {
    let code = r#"const a = <List<Item> items={xs} render={(i) => <b>{i.name}</b>} />;"#;
    assert_tokens(
        "tsx",
        code,
        &[("List", Type), ("Item", Type), ("items", Attribute), ("xs", Text), ("b", Tag), ("name", Property)],
    );
    assert_tokens(
        "tsx",
        "return <div className=\"x\">hi {name}</div>",
        &[("div", Tag), ("className", Attribute), ("\"x\"", String)],
    );
    assert_tokens("tsx", "return <><Foo.Bar /></>", &[("Foo.Bar", Type)]);
}

#[test]
fn comparisons_are_not_jsx() {
    assert_not("tsx", "if (a < b && c > d) {}", "b", Tag);
    assert_tokens("tsx", "for (let i = 0; i < n; i++) {}", &[("<", Operator)]);
    assert_tokens(
        "typescript",
        "const xs = new Array<number>(); f<T>(x);",
        &[("Array", Type), ("number", Type), ("T", Type)],
    );
}

#[test]
fn javascript_regex_and_division() {
    assert_tokens("javascript", "const re = /a\\/b[/]c/gi;", &[("/a\\/b[/]c/gi", Regex)]);
    assert_tokens("javascript", "const half = total / 2 / count;", &[("/", Operator)]);
    assert_tokens("javascript", "return /x/.test(s)", &[("/x/", Regex), ("test", Function)]);
}

#[test]
fn javascript_template_literals_nest() {
    assert_tokens(
        "javascript",
        "`a ${b ? `c ${d}` : 'e'} f`",
        &[
            ("`a ", String),
            ("${", Punctuation),
            ("b", Text),
            ("`c ", String),
            ("d", Text),
            ("'e'", String),
            (" f`", String),
        ],
    );
}

#[test]
fn javascript_functions_and_members() {
    assert_tokens(
        "javascript",
        "const add = (a, b) => a + b; const run = async () => {}; obj.catch(fn); this.#count;",
        &[("add", Function), ("run", Function), ("catch", Function), ("#count", Property)],
    );
}

#[test]
fn rust_features() {
    assert_tokens(
        "rust",
        "#[derive(Debug)] fn f<'a>(x: &'a str) -> Option<u8> { let c = 'x'; println!(\"{c}\"); x.len().await; r#\"raw\"# }",
        &[
            ("#[derive", Meta),
            ("f", Function),
            ("'a", Meta),
            ("str", Type),
            ("Option", Type),
            ("'x'", String),
            ("println!", Function),
            ("len", Function),
            ("await", Keyword),
            ("r#\"raw\"#", String),
        ],
    );
    assert_tokens("rust", "/* outer /* inner */ still */ x", &[("/* outer /* inner */ still */", Comment)]);
}

#[test]
fn python_strings() {
    assert_tokens(
        "python",
        "x = f\"{a!r} {{b}}\"",
        &[("f\"", String), ("{", Punctuation), ("a", Text), (" {{b}}\"", String)],
    );
    assert_tokens("python", "y = rb'\\d'", &[("rb'\\d'", String)]);
    assert_tokens("python", "@app.route('/')\ndef index(): pass", &[("@app.route", Meta), ("index", Function)]);
}

#[test]
fn bash_features() {
    assert_tokens(
        "bash",
        "echo \"hi $USER $(whoami)\" # done\nexport PATH=\"$HOME/bin\"",
        &[("echo", Builtin), ("$USER", Variable), ("whoami", Function), ("# done", Comment), ("PATH", Variable)],
    );
    assert_tokens("bash", "cat <<EOF\nbody $x\nEOF\nls", &[("<<", Operator), ("EOF", String), ("ls", Function)]);
    assert_tokens("bash", "echo a#b", &[("echo", Builtin)]);
    assert_not("bash", "echo a#b", "#b", Comment);
}

#[test]
fn css_selectors_and_values() {
    assert_tokens(
        "css",
        "a.btn:hover > #id { color: #fff !important; width: calc(100% - 2px); }",
        &[
            ("a", Tag),
            (".btn", Type),
            (":hover", Keyword),
            ("#id", Constant),
            ("color", Property),
            ("#fff", Number),
            ("!important", Keyword),
            ("calc", Function),
            ("100%", Number),
        ],
    );
}

#[test]
fn html_embeds_script_and_style() {
    assert_tokens(
        "html",
        "<style>p { color: red }</style><script>let x = 1;</script>",
        &[("style", Tag), ("p", Tag), ("color", Property), ("script", Tag), ("let", Keyword), ("1", Number)],
    );
}

#[test]
fn markdown_fenced_code_uses_the_language() {
    assert_tokens(
        "markdown",
        "# Title\n\n```rust\nfn main() {}\n```\n",
        &[("# Title", Heading), ("fn", Keyword), ("main", Function)],
    );
}

#[test]
fn json_keys_are_properties() {
    assert_tokens(
        "json",
        r#"{"a": "b", "c": [true, null, 1.5e3]}"#,
        &[("\"a\"", Property), ("\"b\"", String), ("true", Constant), ("1.5e3", Number)],
    );
}

#[test]
fn html_lines() {
    use colorfulcode::{highlight, html, Language};
    let code = "a\nb\n";
    let out = highlight(code, Language::plain()).to_html(&html::Options { line_numbers: true, ..Default::default() });
    assert_eq!(
        out,
        "<span class=\"cc-line\"><span class=\"cc-ln\">1</span>a</span>\n<span class=\"cc-line\"><span class=\"cc-ln\">2</span>b</span>\n"
    );
}
