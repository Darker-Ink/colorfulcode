//! Targeted checks for the data, configuration and query languages.

mod common;

use colorfulcode::Kind::*;
use colorfulcode::Language;
use common::{assert_not, assert_tokens};

#[test]
fn yaml_keys_scalars_and_comments() {
    assert_tokens(
        "yaml",
        "name: web # comment\nport: 8080\nratio: -1.5e3\non: yes\nnone: ~\nwhen: 2024-01-15\nurl: http://x.io/a#b",
        &[
            ("name", Property),
            (":", Punctuation),
            ("web", String),
            ("# comment", Comment),
            ("8080", Number),
            ("-1.5e3", Number),
            ("on", Property),
            ("yes", Constant),
            ("~", Constant),
            ("2024-01-15", Number),
            ("http://x.io/a#b", String),
        ],
    );
    // `#` only starts a comment after whitespace, and `:` needs a space to be a key.
    assert_not("yaml", "key: a#b", "#b", Comment);
    assert_tokens("yaml", "time: 12:30", &[("time", Property), ("12:30", Number)]);
    assert_tokens("yaml", "version: 1.2.3", &[("1.2.3", String)]);
}

#[test]
fn yaml_quoted_strings() {
    assert_tokens(
        "yaml",
        "\"a key\": \"tab\\there\"\n'b': 'it''s'",
        &[("\"a key\"", Property), ("\"tab", String), ("\\t", Escape), ("'b'", Property), ("''", Escape)],
    );
    assert_tokens("yaml", "msg: \"first\n  second\"\nnext: 1", &[("\"first\n  second\"", String), ("next", Property)]);
}

#[test]
fn yaml_sequences_anchors_and_tags() {
    assert_tokens(
        "yaml",
        "base: &base\n  a: 1\nitems:\n  - <<: *base\n  - !!str 42\n  - - nested",
        &[
            ("&base", Variable),
            ("a", Property),
            ("-", Punctuation),
            ("<<", Property),
            ("*base", Variable),
            ("!!str", Type),
            ("nested", String),
        ],
    );
}

#[test]
fn yaml_block_scalars() {
    let code = "script: |\n  echo \"# not a comment\"\n\n  exit: 1\nafter: true\nfold: >-\n    a\n    b\n";
    assert_tokens(
        "yaml",
        code,
        &[
            ("script", Property),
            ("|", Operator),
            ("echo \"# not a comment\"", String),
            ("exit: 1", String),
            ("after", Property),
            ("true", Constant),
            (">-", Operator),
            ("a", String),
        ],
    );
    assert_not("yaml", code, "exit", Property);
    // The body ends at the first line indented no deeper than the key.
    assert_tokens(
        "yaml",
        "- run: |\n    make\n  name: build",
        &[("run", Property), ("make", String), ("name", Property), ("build", String)],
    );
}

#[test]
fn yaml_flow_collections_and_documents() {
    assert_tokens(
        "yaml",
        "%YAML 1.2\n---\nenv: {DEBUG: true, \"x\": [1, two]}\n...\n",
        &[
            ("%YAML 1.2", Meta),
            ("---", Punctuation),
            ("{", Punctuation),
            ("DEBUG", Property),
            ("true", Constant),
            ("\"x\"", Property),
            ("1", Number),
            ("two", String),
            ("]}", Punctuation),
            ("...", Punctuation),
        ],
    );
    // Flow collections may span lines.
    assert_tokens("yaml", "list: [\n  a,\n  b # c\n]\nk: v", &[("a", String), ("# c", Comment), ("k", Property)]);
}

#[test]
fn markdown_front_matter_uses_yaml_and_toml() {
    assert_tokens(
        "markdown",
        "---\ntitle: Hello\ndraft: false\n---\n# Heading\n",
        &[("title", Property), ("Hello", String), ("false", Constant), ("# Heading", Heading)],
    );
    assert_tokens(
        "markdown",
        "+++\ntitle = \"Hello\"\n+++\ntext\n",
        &[("title", Property), ("=", Operator), ("\"Hello\"", String)],
    );
}

#[test]
fn toml_tables_keys_and_values() {
    assert_tokens(
        "toml",
        "[server.\"eu-1\"]\nhost.name = 'raw\\n'\nport = 8_080\nwhen = 1979-05-27 07:32:00Z\n[[items]]\ninline = { a = 1, b = [true, -inf] }",
        &[
            ("[", Punctuation),
            ("server", Type),
            ("\"eu-1\"", Type),
            ("host", Property),
            ("name", Property),
            ("'raw\\n'", String),
            ("8_080", Number),
            ("1979-05-27 07:32:00Z", Number),
            ("[[", Punctuation),
            ("items", Type),
            ("a", Property),
            ("b", Property),
            ("true", Constant),
            ("-inf", Number),
        ],
    );
    // Arrays in value position are not table headers, even across lines.
    assert_tokens("toml", "a = [\n  \"x\",\n]\nb = 1", &[("\"x\"", String), ("b", Property)]);
    assert_tokens("toml", "s = \"\"\"\nline \\\"q\\\"\"\"\"", &[("\\\"", Escape)]);
}

#[test]
fn ini_sections_and_values() {
    assert_tokens(
        "ini",
        "; comment\n[remote \"origin\"]\nurl = https://x.io ; trailing\nport: 8080\nhome = /home/${USER}\nbare-flag",
        &[
            ("; comment", Comment),
            ("remote", Type),
            ("\"origin\"", String),
            ("url", Property),
            ("https://x.io", String),
            ("; trailing", Comment),
            ("port", Property),
            ("8080", Number),
            ("${USER}", Variable),
            ("bare-flag", Property),
        ],
    );
    assert_eq!(Language::from_path(".editorconfig").unwrap().id(), "ini");
    assert_eq!(Language::from_path("setup.cfg").unwrap().id(), "ini");
}

#[test]
fn properties_escapes_and_continuations() {
    assert_tokens(
        "properties",
        "! note\nkey\\ one = a\\tb \\\n    continued\nother:value",
        &[
            ("! note", Comment),
            ("key\\ one", Property),
            ("\\t", Escape),
            ("continued", String),
            ("other", Property),
            ("value", String),
        ],
    );
}

#[test]
fn dotenv_values() {
    assert_tokens(
        "dotenv",
        "export A=\"x ${B} $C\\n\"\nD='$raw'\nE=10 # note",
        &[
            ("export", Keyword),
            ("A", Property),
            ("${B}", Variable),
            ("$C", Variable),
            ("\\n", Escape),
            ("'$raw'", String),
            ("10", Number),
            ("# note", Comment),
        ],
    );
    assert_eq!(Language::from_path("app/.env.local").unwrap().id(), "dotenv");
    assert_eq!(Language::find("env").unwrap().id(), "dotenv");
}

#[test]
fn csv_columns_cycle_kinds() {
    assert_tokens(
        "csv",
        "a,\"b, \"\"c\"\"\",3\nx,y,z",
        &[("a", Property), (",", Punctuation), ("\"b, \"\"c\"\"\"", String), ("3", Number), ("x", Property)],
    );
    // Semicolon separated files are detected from the first line.
    assert_tokens("csv", "a;b\n1;2", &[("a", Property), ("b", String), ("1", Property)]);
    assert_tokens("tsv", "a b\tc", &[("a b", Property), ("c", String)]);
}

#[test]
fn sql_features() {
    assert_tokens(
        "postgres",
        "SELECT u.\"Name\", count(*)::int FROM users u WHERE id = $1 AND note = 'it''s' -- c\n",
        &[
            ("SELECT", Keyword),
            ("\"Name\"", Property),
            ("count", Builtin),
            ("::", Operator),
            ("int", Type),
            ("users", Type),
            ("$1", Variable),
            ("'it''s'", String),
            ("-- c", Comment),
        ],
    );
    assert_tokens(
        "sql",
        "create function f() returns void as $body$ select 1; $body$ language sql;",
        &[("create", Keyword), ("$body$ select 1; $body$", String), ("language", Keyword)],
    );
    assert_tokens("tsql", "select [Order Id] from t where x = @p", &[("[Order Id]", Property), ("@p", Variable)]);
    assert_tokens("mysql", "select * from `t` limit :n", &[("`t`", Property), (":n", Variable)]);
    assert_not("sql", "select arr[1] from t", "[1]", Property);
}

#[test]
fn graphql_fields_and_types() {
    assert_tokens(
        "graphql",
        "query Q($id: ID!) { user(id: $id) { type name ...F @include(if: true) } }",
        &[
            ("query", Keyword),
            ("Q", Function),
            ("$id", Variable),
            ("ID", Type),
            ("user", Function),
            ("id", Property),
            ("type", Property),
            ("name", Property),
            ("F", Type),
            ("@include", Meta),
            ("true", Constant),
        ],
    );
    assert_tokens("gql", "type User { role: Role! }", &[("type", Keyword), ("User", Type), ("role", Property)]);
}

#[test]
fn diff_lines() {
    assert_tokens(
        "diff",
        "diff --git a/x b/x\nindex 1..2\n--- a/x\n+++ b/x\n@@ -1 +1 @@ fn main\n-old\n+new\n ctx\n--- removed dashes\n",
        &[
            ("diff --git a/x b/x", Heading),
            ("index 1..2", Meta),
            ("--- a/x", Heading),
            ("+++ b/x", Heading),
            ("@@ -1 +1 @@", Meta),
            ("fn main", Text),
            ("-old", Deleted),
            ("+new", Inserted),
            ("--- removed dashes", Deleted),
        ],
    );
}

#[test]
fn protobuf_and_thrift() {
    assert_tokens(
        "proto",
        "syntax = \"proto3\";\nmessage User { repeated string tags = 1; Status s = 2; }\nservice S { rpc Get(Req) returns (User); }",
        &[
            ("syntax", Keyword),
            ("message", Keyword),
            ("User", Type),
            ("repeated", Keyword),
            ("string", Type),
            ("tags", Property),
            ("Status", Type),
            ("rpc", Keyword),
            ("Get", Function),
        ],
    );
    assert_tokens(
        "thrift",
        "struct W { 1: required i32 n = 0 }\nservice C { void ping() }",
        &[("struct", Keyword), ("W", Type), ("i32", Type), ("ping", Function)],
    );
}

#[test]
fn hcl_blocks_and_interpolation() {
    assert_tokens(
        "terraform",
        "resource \"aws_instance\" \"web\" {\n  count = var.n\n  name = \"${var.p}-web\"\n  lifecycle {\n    x = { Owner = \"me\" }\n  }\n}",
        &[
            ("resource", Keyword),
            ("\"aws_instance\"", String),
            ("count", Property),
            ("var", Variable),
            ("n", Property),
            ("${", Punctuation),
            ("p", Property),
            ("-web\"", String),
            ("lifecycle", Keyword),
            ("Owner", Property),
        ],
    );
    assert_tokens(
        "hcl",
        "data = <<-EOT\n  body ${x}\n  EOT\nnext = 1",
        &[("<<-EOT", Operator), ("next", Property), ("1", Number)],
    );
}

#[test]
fn prisma_models() {
    assert_tokens(
        "prisma",
        "model User {\n  id Int @id @default(autoincrement())\n  posts Post[]\n  @@map(\"users\")\n}",
        &[
            ("model", Keyword),
            ("User", Type),
            ("id", Property),
            ("Int", Type),
            ("@id", Meta),
            ("autoincrement", Builtin),
            ("posts", Property),
            ("@@map", Meta),
        ],
    );
}

#[test]
fn http_requests_and_responses() {
    assert_tokens(
        "http",
        "POST {{base}}/users HTTP/1.1\nContent-Type: application/json\n\n{\"a\": 1}\n\n### next\nHTTP/1.1 404 Not Found\nX: y\n",
        &[
            ("POST", Keyword),
            ("{{base}}", Variable),
            ("/users", String),
            ("HTTP/1.1", Type),
            ("Content-Type", Property),
            ("application/json", String),
            ("\"a\"", Property),
            ("1", Number),
            ("### next", Comment),
            ("404", Number),
            ("Not Found", String),
            ("X", Property),
        ],
    );
}

#[test]
fn cypher_patterns() {
    assert_tokens(
        "cypher",
        "MATCH (p:Person {name: $n})-[:KNOWS]->(f) RETURN f.name, count(*)",
        &[
            ("MATCH", Keyword),
            ("Person", Type),
            ("name", Property),
            ("$n", Variable),
            ("KNOWS", Type),
            ("->", Operator),
            ("RETURN", Keyword),
            ("name", Property),
            ("count", Builtin),
        ],
    );
}

#[test]
fn sparql_names_and_iris() {
    assert_tokens(
        "sparql",
        "PREFIX foaf: <http://xmlns.com/foaf/0.1/>\nSELECT ?n WHERE { ?p a foaf:Person ; foaf:name ?n . FILTER(?x < 3) }",
        &[
            ("PREFIX", Keyword),
            ("foaf:", Type),
            ("<http://xmlns.com/foaf/0.1/>", String),
            ("?n", Variable),
            ("a", Keyword),
            ("Person", Property),
            ("<", Operator),
        ],
    );
}

#[test]
fn kdl_nodes() {
    assert_tokens(
        "kdl",
        "node \"arg\" key=1 #true {\n  child r#\"raw\"# (u8)5\n}\n/- gone 1\nlast",
        &[
            ("node", Tag),
            ("\"arg\"", String),
            ("key", Attribute),
            ("1", Number),
            ("#true", Constant),
            ("child", Tag),
            ("r#\"raw\"#", String),
            ("u8", Type),
            ("5", Number),
            ("/- gone 1", Comment),
            ("last", Tag),
        ],
    );
}

#[test]
fn cue_definitions_and_fields() {
    assert_tokens(
        "cue",
        "#Def: {\n  name?: string | *\"x\"\n  msg: \"hi \\(name)\"\n}",
        &[
            ("#Def", Type),
            ("name?", Property),
            ("string", Type),
            ("\"hi ", String),
            ("\\(", Punctuation),
            ("name", Text),
        ],
    );
}

#[test]
fn jsonnet_fields() {
    assert_tokens(
        "jsonnet",
        "local x = { a: 1, b:: 2, c+: { d: $.a }, t: |||\n  text\n|||, v: @'x''y' };",
        &[
            ("local", Keyword),
            ("a", Property),
            ("b", Property),
            ("::", Punctuation),
            ("c", Property),
            ("+:", Punctuation),
            ("$", Variable),
            ("|||\n  text\n|||", String),
            ("@'x''y'", String),
        ],
    );
}

#[test]
fn dot_graphs() {
    assert_tokens(
        "graphviz",
        "digraph G {\n  a -> b [label=\"x\", color=red];\n  c [label=<<b>hi</b>>];\n}",
        &[
            ("digraph", Keyword),
            ("G", Type),
            ("->", Operator),
            ("label", Attribute),
            ("\"x\"", String),
            ("color", Attribute),
            ("red", String),
            ("<<b>hi</b>>", String),
        ],
    );
}

#[test]
fn mermaid_diagrams() {
    assert_tokens(
        "mermaid",
        "flowchart LR\n  A[Start] -->|go| B((End))\n  %% note\n  C{x}\nsequenceDiagram\n  A->>B: Hello there",
        &[
            ("flowchart", Keyword),
            ("Start", String),
            ("-->", Operator),
            ("go", String),
            ("((", Punctuation),
            ("End", String),
            ("%% note", Comment),
            ("x", String),
            ("Hello there", String),
        ],
    );
}

#[test]
fn plantuml_diagrams() {
    assert_tokens(
        "plantuml",
        "@startuml\n' comment\nactor User as U #Red\nU -> S : request\n:activity;\n@enduml",
        &[
            ("@startuml", Meta),
            ("' comment", Comment),
            ("actor", Keyword),
            ("#Red", Constant),
            ("->", Operator),
            ("request", String),
            ("activity", String),
            ("@enduml", Meta),
        ],
    );
}

#[test]
fn log_levels_and_timestamps() {
    assert_tokens(
        "log",
        "2024-01-15 10:30:00,123 ERROR [main] failed id=7 \"q\" see https://x.io/a?b=1 WARN info DEBUG app.INFO: ok",
        &[
            ("2024-01-15 10:30:00,123", Number),
            ("ERROR", Deleted),
            ("[", Punctuation),
            ("id", Property),
            ("\"q\"", String),
            ("https://x.io/a?b=1", Link),
            ("WARN", Constant),
            ("info", Inserted),
            ("DEBUG", Comment),
            ("INFO", Inserted),
        ],
    );
    assert_not("log", "it's fine, it failed", "fine", Comment);
    assert_not("log", "it's fine, it failed", "failed", Deleted);
}

#[test]
fn regex_patterns() {
    assert_tokens(
        "regex",
        "^(?<y>\\d{4})[^a-z\\]]+?\\k<y>\\.|(?:x)*$",
        &[
            ("^", Keyword),
            ("(", Punctuation),
            ("?<", Keyword),
            ("y", Variable),
            ("\\d", Builtin),
            ("{4}", Operator),
            ("^", Operator),
            ("a", String),
            ("-", Operator),
            ("\\]", Escape),
            ("+?", Operator),
            ("\\k<y>", Variable),
            ("\\.", Escape),
            ("|", Operator),
            ("?:", Keyword),
            ("$", Keyword),
        ],
    );
    assert_tokens("regex", "/a.b/gi", &[("/", Punctuation), (".", Builtin), ("gi", Keyword)]);
    assert_tokens("regex", "(?x) a # note\n b", &[("# note", Comment)]);
}

#[test]
fn lookups() {
    for (name, id) in [
        ("yml", "yaml"),
        ("tf", "hcl"),
        ("terraform", "hcl"),
        ("patch", "diff"),
        ("proto", "protobuf"),
        ("gv", "dot"),
        ("mmd", "mermaid"),
        ("puml", "plantuml"),
        ("libsonnet", "jsonnet"),
        ("plsql", "sql"),
        ("gql", "graphql"),
    ] {
        assert_eq!(Language::find(name).unwrap().id(), id, "{name}");
    }
    for (path, id) in [
        ("Cargo.lock", "toml"),
        ("infra/main.tf", "hcl"),
        ("vars.tfvars", "hcl"),
        ("schema.prisma", "prisma"),
        ("api.http", "http"),
        (".env.example", "dotenv"),
        ("server.log", "log"),
        ("app.properties", "properties"),
    ] {
        assert_eq!(Language::from_path(path).unwrap().id(), id, "{path}");
    }
}
