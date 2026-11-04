//! Checks for documentation and template languages.

mod common;

use colorfulcode::Kind::*;
use colorfulcode::Language;
use common::{assert_not, assert_tokens};

#[test]
fn file_names_pick_the_template_language() {
    let id = |path: &str| Language::from_path(path).map(|l| l.id());
    assert_eq!(id("views/welcome.blade.php"), Some("blade"));
    assert_eq!(id("app/views/index.html.erb"), Some("erb"));
    assert_eq!(id("Pages/Index.cshtml"), Some("razor"));
    assert_eq!(id("man/colorful.1"), Some("roff"));
    assert_eq!(id("templates/base.html.j2"), Some("jinja"));
    assert_eq!(Language::find("mustache").unwrap().id(), "handlebars");
    assert_eq!(Language::find("tex").unwrap().id(), "latex");
}

#[test]
fn latex_commands_math_and_verbatim() {
    let code = "\\section{Intro}\\label{sec:a} % note\nSee $x^2 + 1$ and \\emph{this}.";
    assert_tokens(
        "latex",
        code,
        &[
            ("\\section", Keyword),
            ("Intro", Heading),
            ("\\label", Function),
            ("sec:a", Constant),
            ("% note", Comment),
            ("$x", String),
            ("^", Operator),
            ("2", Number),
            ("this", Emphasis),
        ],
    );
    let code = "\\begin{verbatim}\n$ not math % not a comment\n\\end{verbatim}";
    assert_tokens("latex", code, &[("verbatim", Type), ("\n$ not math % not a comment\n", String), ("\\end", Keyword)]);
    assert_not("latex", code, "% not a comment", Comment);
    // Code listings embed their language.
    let code = "\\begin{lstlisting}[language=Python]\ndef f(): pass\n\\end{lstlisting}";
    assert_tokens("latex", code, &[("def", Keyword), ("f", Function)]);
    // A lone dollar does not swallow the next paragraph.
    assert_not("latex", "costs $5\n\nNext \\textbf{bold} $x$", "\\textbf", String);
    assert_tokens("latex", "50\\% off", &[("\\%", Escape)]);
}

#[test]
fn bibtex_entries() {
    let code = "@article{knuth84,\n  title = {The {\\TeX}book},\n  year = 1984,\n  month = jun # \"x\",\n}";
    assert_tokens(
        "bibtex",
        code,
        &[
            ("@article", Keyword),
            ("knuth84", Constant),
            ("title", Property),
            ("{The {\\TeX}book}", String),
            ("1984", Number),
            ("jun", Variable),
            ("#", Operator),
        ],
    );
    assert_tokens("bibtex", "@comment{ignored @book{x}}", &[("{ignored @book{x}}", Comment)]);
}

#[test]
fn rst_sections_directives_and_roles() {
    let code = "Title\n=====\n\nUse ``code`` and :ref:`target` or `link <https://x.org>`_.\n";
    assert_tokens(
        "rst",
        code,
        &[
            ("Title", Heading),
            ("=====", Heading),
            ("``code``", String),
            (":ref:", Function),
            ("`target`", String),
            ("`link <https://x.org>`_", Link),
        ],
    );
    let code = ".. code-block:: python\n   :linenos:\n\n   def f():\n       return 1\n\nAfter.\n";
    assert_tokens(
        "rst",
        code,
        &[("code-block", Keyword), ("python", String), (":linenos:", Property), ("def", Keyword), ("1", Number)],
    );
    let code = ".. a comment\n   continued\n\nText::\n\n   literal *text*\n";
    assert_tokens("rst", code, &[(".. a comment\n   continued", Comment), ("literal *text*", String)]);
    assert_tokens(
        "rst",
        "Some *em* and **strong** text_",
        &[("*em*", Emphasis), ("**strong**", Strong), ("text_", Link)],
    );
}

#[test]
fn asciidoc_blocks() {
    let code = "== Section\n:attr: value\n\n[source,python]\n----\nx = 1\n----\n\n*bold* and {attr}\n";
    assert_tokens(
        "asciidoc",
        code,
        &[
            ("== Section", Heading),
            (":attr:", Property),
            ("[source,python]", Meta),
            ("----", Punctuation),
            ("1", Number),
            ("----", Punctuation),
            ("*bold*", Strong),
            ("{attr}", Variable),
        ],
    );
    assert_tokens("asciidoc", "////\nhidden\n////\n// note", &[("////\nhidden\n////", Comment), ("// note", Comment)]);
}

#[test]
fn org_headlines_and_blocks() {
    let code = "* TODO [#A] Task :tag:\n#+BEGIN_SRC python\nx = 1\n#+END_SRC\nText *bold* and [[https://x.org][link]].";
    assert_tokens(
        "org",
        code,
        &[
            ("TODO", Keyword),
            ("[#A]", Constant),
            (":tag:", Attribute),
            ("#+BEGIN_SRC", Meta),
            ("1", Number),
            ("#+END_SRC", Meta),
            ("*bold*", Strong),
            ("[[https://x.org][link]]", Link),
        ],
    );
    assert_not("org", "2 * 3 * 4", "* 3 *", Strong);
}

#[test]
fn typst_markup_and_code() {
    let code = "= Title\n#let x = [Hi *there*]\n#rect(width: 2cm)[Box] $a^2$ @ref";
    assert_tokens(
        "typst",
        code,
        &[
            ("= Title", Heading),
            ("#let", Keyword),
            ("*there*", Strong),
            ("#rect", Function),
            ("width", Property),
            ("2cm", Number),
            ("Box", Text),
            ("$a", String),
            ("^", Operator),
            ("2", Number),
            ("$", String),
            ("@ref", Constant),
        ],
    );
    assert_not("typst", "a snake_case_name here", "_case_", Emphasis);
    let code = r#"$ sqrt(x) + alpha.alt dot 2.5 \$ "text $" #f(1) $ after"#;
    assert_tokens(
        "typst",
        code,
        &[
            ("sqrt", Function),
            ("+", Operator),
            ("alpha.alt", Constant),
            ("2.5", Number),
            ("\\$", Escape),
            ("\"text $\"", String),
            ("#f", Function),
            ("$", String),
            ("after", Text),
        ],
    );
    // Code mode math.
    assert_tokens("typst", "#let e = $m c^2$", &[("#let", Keyword), ("$m", String), ("^", Operator)]);
}

#[test]
fn gherkin_steps() {
    let code = "@smoke\nFeature: Cart\n  Scenario Outline: Pay\n    Given I have <count> \"apples\"\n    | a | 1 |";
    assert_tokens(
        "gherkin",
        code,
        &[
            ("@smoke", Meta),
            ("Feature:", Keyword),
            ("Cart", Heading),
            ("Scenario Outline:", Keyword),
            ("Given", Keyword),
            ("<count>", Variable),
            ("\"apples\"", String),
            ("|", Punctuation),
            ("1", Number),
        ],
    );
}

#[test]
fn handlebars_tags() {
    let code = "<p class=\"{{#if on}}on{{/if}}\">{{! note }}{{link \"Home\" href=url}}{{{raw}}}</p>";
    assert_tokens(
        "handlebars",
        code,
        &[
            ("p", Tag),
            ("class", Attribute),
            ("#if", Keyword),
            ("/if", Keyword),
            ("\"", String),
            ("{{! note }}", Comment),
            ("link", Function),
            ("\"Home\"", String),
            ("href", Property),
            ("raw", Text),
            ("p", Tag),
        ],
    );
    assert_tokens("handlebars", "{{#each items as |item|}}{{@index}}{{/each}}", &[("@index", Variable)]);
}

#[test]
fn jinja_tags_filters_and_raw() {
    let code = "{# c #}{% for x in xs %}<li>{{ x.name | upper }}</li>{% endfor %}";
    assert_tokens(
        "jinja",
        code,
        &[
            ("{# c #}", Comment),
            ("for", Keyword),
            ("in", Keyword),
            ("li", Tag),
            ("name", Property),
            ("|", Operator),
            ("upper", Function),
            ("endfor", Keyword),
        ],
    );
    assert_tokens("jinja", "{% block content %}{% endblock %}", &[("block", Keyword), ("content", Function)]);
    let code = "{% raw %}{{ not_code }}{% endraw %}";
    assert_not("jinja", code, "not_code", Text);
    assert_tokens("jinja", code, &[("{{ not_code }}", Text), ("endraw", Keyword)]);
    // Expressions inside attributes and scripts keep the HTML state.
    let code = "<a href=\"/u/{{ id }}\" title=\"x\">go</a><script>var n = {{ n }}; f(1)</script>";
    assert_tokens(
        "jinja",
        code,
        &[
            ("\"/u/", String),
            ("id", Text),
            ("\"", String),
            ("title", Attribute),
            ("var", Keyword),
            ("f", Function),
            ("1", Number),
            ("script", Tag),
        ],
    );
}

#[test]
fn twig_and_liquid() {
    assert_tokens("twig", "{{ name|default('x') ~ '!' }}", &[("default", Function), ("~", Operator)]);
    let code = "{% comment %}hidden {{ x }}{% endcomment %}{{ p.price | money: 'USD' }}";
    assert_tokens(
        "liquid",
        code,
        &[("comment", Keyword), ("hidden {{ x }}", Comment), ("endcomment", Keyword), ("money", Function)],
    );
}

#[test]
fn erb_and_ejs() {
    let code = "<%# note %><li><%= link %></li><%% literal %>";
    assert_tokens("erb", code, &[("<%# note %>", Comment), ("li", Tag), ("<%=", Meta), ("%>", Meta)]);
    assert_not("erb", code, "<%%", Meta);
    let code = "<% if (user) { %><b><%= user.name %></b><% } %>";
    assert_tokens(
        "ejs",
        code,
        &[("<%", Meta), ("if", Keyword), ("b", Tag), ("<%=", Meta), ("name", Property), ("%>", Meta)],
    );
}

#[test]
fn blade_directives() {
    let code = "@if ($user->admin)\n<b>{{ $user->name }}</b> mail@example.com\n@endif {{-- c --}}";
    assert_tokens(
        "blade",
        code,
        &[
            ("@if", Keyword),
            ("$user", Variable),
            ("admin", Property),
            ("b", Tag),
            ("name", Property),
            ("@endif", Keyword),
            ("{{-- c --}}", Comment),
        ],
    );
    assert_not("blade", code, "@example", Keyword);
    assert_tokens("blade", "@php\n$x = 1;\n@endphp", &[("@php", Keyword), ("$x", Variable), ("@endphp", Keyword)]);
}

#[test]
fn razor_code() {
    let code = "@* c *@\n@if (ok)\n{\n  <p>@Model.Name a@b.com</p>\n}\nelse\n{\n  <p>no</p>\n}";
    assert_tokens(
        "razor",
        code,
        &[
            ("@* c *@", Comment),
            ("@if", Keyword),
            ("p", Tag),
            (">@", Punctuation),
            ("}", Punctuation),
            ("else", Keyword),
            ("{", Punctuation),
            ("}", Punctuation),
        ],
    );
    assert_not("razor", code, "@b", Punctuation);
}

#[test]
fn go_templates() {
    let code = "{{/* c */}}{{ range $i, $x := .Items }}<li>{{ $x.Name | printf \"%q\" }}</li>{{ end }}";
    assert_tokens(
        "gotemplate",
        code,
        &[
            ("/* c */", Comment),
            ("range", Keyword),
            ("$i", Variable),
            ("Items", Property),
            ("li", Tag),
            ("Name", Property),
            ("printf", Function),
            ("end", Keyword),
        ],
    );
    assert_tokens("gotemplate", "{{ myFunc .A \"b\" }}", &[("myFunc", Function), ("A", Property)]);
}

#[test]
fn pug_haml_and_slim() {
    let code = "//- note\na.btn#go(href=url, title=\"x\") Go #{name}\n- const y = 1";
    assert_tokens(
        "pug",
        code,
        &[
            ("//- note", Comment),
            ("a", Tag),
            ("href", Attribute),
            ("\"x\"", String),
            ("#{", Punctuation),
            ("const", Keyword),
            ("1", Number),
        ],
    );
    let code = "-# note\n%p.lead{class: x}(title=\"t\") Hi\n:javascript\n  f(1)";
    assert_tokens(
        "haml",
        code,
        &[("-# note", Comment), ("%p", Tag), (".lead", Attribute), ("title", Attribute), ("f", Function)],
    );
    let code = "/ note\np.lead title=\"t\" Hello\n| text #{x}\njavascript:\n  f(1)";
    assert_tokens(
        "slim",
        code,
        &[
            ("/ note", Comment),
            ("p", Tag),
            ("title", Attribute),
            ("\"t\"", String),
            ("|", Punctuation),
            ("f", Function),
        ],
    );
}

#[test]
fn roff_texinfo_and_pod() {
    let code = ".\\\" comment\n.SH NAME\nls \\- list \\fBfiles\\fR now\n.B bold";
    assert_tokens(
        "roff",
        code,
        &[
            (".\\\" comment", Comment),
            (".SH", Keyword),
            ("NAME", Heading),
            ("\\-", Escape),
            ("\\fB", Escape),
            ("files", Strong),
            (".B", Keyword),
            ("bold", Strong),
        ],
    );
    let code = "@c note\n@chapter Intro\nUse @code{ls} and @@.\n@example\n@not a command\n@end example";
    assert_tokens(
        "texinfo",
        code,
        &[
            ("@c note", Comment),
            ("@chapter", Keyword),
            ("Intro", Heading),
            ("ls", String),
            ("@@", Escape),
            ("@not a command\n", String),
        ],
    );
    let code = "=head1 NAME\n\nB<bold> C<< $a <=> $b >> L<perlpod>\n\n    verbatim code\n\n=cut\n";
    assert_tokens(
        "pod",
        code,
        &[
            ("=head1", Keyword),
            ("NAME", Heading),
            ("B<bold>", Strong),
            ("C<< $a <=> $b >>", String),
            ("L<perlpod>", Link),
            ("    verbatim code", String),
            ("=cut", Keyword),
        ],
    );
}

#[test]
fn liquid_tag_and_comment_tag() {
    let code = "{% # note %}{% liquid\n  # pick\n  assign n = list | size\n  echo n\n%}";
    assert_tokens(
        "liquid",
        code,
        &[
            ("{% # note %}", Comment),
            ("liquid", Keyword),
            ("# pick", Comment),
            ("assign", Keyword),
            ("size", Function),
            ("echo", Keyword),
        ],
    );
    // Named parameters read as keys.
    let code = "{% for p in list limit: 2 offset: 1 %}";
    assert_tokens("liquid", code, &[("limit", Property), ("offset", Property)]);
}

#[test]
fn mustache_delimiter_changes() {
    let code = "{{a}} {{=<% %>=}} <% b %> {{c}} <%! note %><%={{ }}=%>{{#d}}";
    assert_tokens(
        "mustache",
        code,
        &[
            ("{{=", Punctuation),
            ("<%", String),
            ("%>", String),
            ("=}}", Punctuation),
            ("b", Text),
            ("{{c}}", Text),
            ("<%! note %>", Comment),
            ("#d", Keyword),
        ],
    );
    // An unclosed change leaves the rest as text.
    assert_not("mustache", "{{=<% %> <% x %>", "<%", Punctuation);
}

#[test]
fn pug_inline_tags() {
    let code = "p Text #[em.big(title=t) word #{x}] and #[b= y] \\#[no]";
    assert_tokens(
        "pug",
        code,
        &[
            ("#[", Punctuation),
            ("em", Tag),
            (".big", Attribute),
            ("title", Attribute),
            ("word", Text),
            ("#{", Punctuation),
            ("x", Text),
            ("}]", Punctuation),
            ("b", Tag),
            ("=", Punctuation),
            ("y", Text),
        ],
    );
    assert_not("pug", "p a \\#[no] b", "no", Tag);
}

#[test]
fn inline_markup_wraps_within_a_paragraph() {
    let code = "Some *wrapped\nemphasis* and ``a\nliteral`` here *x\n\nnot* closed.";
    assert_tokens("rst", code, &[("*wrapped\nemphasis*", Emphasis), ("``a\nliteral``", String)]);
    assert_not("rst", code, "*x\n\nnot*", Emphasis);
    let code = "Some *wrapped\nstrong* then _more_.\n\n- item *one\n- item two*";
    assert_tokens("asciidoc", code, &[("*wrapped\nstrong*", Strong), ("_more_", Emphasis)]);
    assert_not("asciidoc", code, "*one\n- item two*", Strong);
    // Org emphasis spans at most two lines.
    let code = "Some /wrapped\ntext/ and =a\nb\nc= end.";
    assert_tokens("org", code, &[("/wrapped\ntext/", Emphasis)]);
    assert_not("org", code, "=a\nb\nc=", String);
}

#[test]
fn unclosed_inline_markup_on_long_lines_stays_linear() {
    let line = "*a `b ``c |d [e <<f ".repeat(20_000);
    for id in ["rst", "asciidoc", "org"] {
        let start = std::time::Instant::now();
        let _ = colorfulcode::highlight(&line, Language::find(id).unwrap());
        assert!(start.elapsed().as_secs() < 2, "{id} is too slow on a long line");
    }
}
