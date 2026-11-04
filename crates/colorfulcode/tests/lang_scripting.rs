//! Targeted checks for the scripting languages.

mod common;

use colorfulcode::Kind::*;
use colorfulcode::{highlight, Language};
use common::{assert_not, assert_tokens};

const IDS: &[&str] = &[
    "python", "ruby", "perl", "lua", "r", "julia", "elixir", "crystal", "nim", "coffeescript", "tcl", "gdscript",
    "mojo", "raku", "autohotkey", "applescript", "wren", "squirrel", "pony", "moonscript", "livescript",
];

/// Every pair of fragments that trigger the hand written rules, at the very
/// start of input and after other text, must lex without panicking and cover
/// the whole input.
#[test]
fn hook_fragments_never_panic() {
    const FRAGMENTS: &[&str] = &[
        ">", "->", "q(", "s{a}", "{a}", "<<~", "<<'", "<<E", "%w", "%(", "~r", "~S\"\"\"", "#`(", "=begin", "=cut",
        "[==[", "--[[", ":\"", "$#", "$", "@[", "@", "%", "&", "r\"", "raw\"", "'", "\"", "\\", "#{", "%(", "{.", "#[",
        "#=", "..", "-", "a-b", "self.", "def ", "function M", ":", "::", "=>", "=", "\n", " ", "x", "1", "`", "é",
        "__END__", "proc ", "[", "]", "}", ")", "f\"{", "{x:", "!r", "/[", "?", "<a", "q:to/E/", "fmt\"", "&\"", "M,",
        "@{[", "$h{", "x = ",
    ];
    for id in IDS {
        let lang = Language::find(id).unwrap();
        for a in FRAGMENTS {
            for b in FRAGMENTS {
                for code in [format!("{a}{b}"), format!("{a}{b}{a}"), format!("x {a}{b}")] {
                    let hl = highlight(&code, lang);
                    let end = hl.tokens().last().map_or(0, |t| t.end as usize);
                    assert_eq!(end, code.len(), "{id}: {code:?}");
                }
            }
        }
    }
}

#[test]
fn lookups() {
    for (name, id) in [
        ("rb", "ruby"),
        ("pl", "perl"),
        ("jl", "julia"),
        ("exs", "elixir"),
        ("cr", "crystal"),
        ("coffee", "coffeescript"),
        ("gd", "gdscript"),
        ("perl6", "raku"),
        ("ahk", "autohotkey"),
        ("moon", "moonscript"),
    ] {
        assert_eq!(Language::find(name).map(|l| l.id()), Some(id), "{name}");
    }
    assert_eq!(Language::from_path("lib/app.pm").unwrap().id(), "perl");
    assert_eq!(Language::from_path("Rakefile").unwrap().id(), "ruby");
    assert_eq!(Language::from_path("app/Gemfile").unwrap().id(), "ruby");
    assert_eq!(Language::from_content("#!/usr/bin/env ruby\n").unwrap().id(), "ruby");
    assert_eq!(Language::from_content("#!/usr/bin/perl -w\n").unwrap().id(), "perl");
    assert!(Language::from_path("x.m").is_none_or(|l| !["perl", "raku", "mojo"].contains(&l.id())));
}

#[test]
fn ruby_symbols_and_keys() {
    assert_tokens(
        "ruby",
        "attr_reader :name\nfoo(key: :value, other: 1)",
        &[(":name", Constant), ("key", Property), (":value", Constant), ("other", Property)],
    );
    assert_tokens("ruby", "Foo::Bar.new", &[("Foo", Type), ("::", Operator), ("Bar", Type)]);
    assert_tokens("ruby", "x = ok ? a : b", &[("?", Operator), (":", Operator)]);
    assert_not("ruby", "x = ok ? a : b", "a", Property);
    assert_tokens("ruby", "sym = :\"with space\"", &[(":\"with space\"", Constant)]);
}

#[test]
fn ruby_methods_and_variables() {
    assert_tokens(
        "ruby",
        "def valid?(x)\n  @x = $y || @@z\nend\ndef self.build!; end",
        &[
            ("def", Keyword),
            ("valid?", Function),
            ("@x", Variable),
            ("$y", Variable),
            ("@@z", Variable),
            ("build!", Function),
        ],
    );
    assert_tokens("ruby", "a.empty? ? 1 : 2", &[("empty?", Property), ("?", Operator)]);
}

#[test]
fn ruby_strings_and_literals() {
    assert_tokens(
        "ruby",
        r#"puts "Hi #{user.name}!\n""#,
        &[("\"Hi ", String), ("#{", Punctuation), ("name", Property), ("}", Punctuation), ("\\n", Escape)],
    );
    assert_tokens("ruby", "x = 'no #{interp}'", &[("'no #{interp}'", String)]);
    assert_tokens("ruby", "w = %w[a b c]\ni = %i(x y)", &[("%w[a b c]", String), ("%i(x y)", Constant)]);
    assert_tokens("ruby", "re = %r{a/b#{x}}i", &[("%r{a/b", Regex), ("x", Text), ("}i", Regex)]);
    assert_tokens("ruby", "x = n % 3", &[("%", Operator), ("3", Number)]);
    assert_tokens("ruby", "line =~ /ab+c/i", &[("/ab+c/i", Regex)]);
    assert_tokens("ruby", "half = total / 2", &[("/", Operator)]);
    assert_tokens("ruby", "(1..10).each", &[("1", Number), ("..", Punctuation), ("10", Number)]);
}

#[test]
fn ruby_heredocs_and_comments() {
    let code = "sql = <<~SQL.strip\n  SELECT #{cols}\n  FROM t\n  SQL\nputs sql";
    assert_tokens(
        "ruby",
        code,
        &[
            ("<<~", Operator),
            ("SQL", String),
            ("strip", Property),
            ("cols", Text),
            ("\n  FROM t\n  SQL", String),
            ("puts", Builtin),
        ],
    );
    assert_tokens("ruby", "raw = <<-'EOS'\n#{x}\n  EOS\n", &[("#{x}\n  EOS", String)]);
    assert_tokens("ruby", "class << self\nend", &[("<<", Operator), ("self", Variable)]);
    assert_tokens("ruby", "=begin\nnot code\n=end\nx = 1", &[("=begin\nnot code\n=end", Comment), ("1", Number)]);
    assert_tokens("ruby", "x = 1\n__END__\ndata", &[("__END__\ndata", Comment)]);
}

#[test]
fn crystal() {
    assert_tokens(
        "crystal",
        "@[Link(\"c\")]\nlib LibC\n  fun getpid : Int32\nend",
        &[("@[", Meta), ("Link", Function), ("]", Meta), ("lib", Keyword), ("LibC", Type), ("getpid", Function)],
    );
    assert_tokens("crystal", "def greet(name : String?) : Nil", &[("greet", Function), ("String?", Type)]);
}

#[test]
fn perl_variables() {
    assert_tokens(
        "perl",
        "my ($x, @list, %h) = ($_, $#list, $1, $Foo::bar);",
        &[
            ("my", Keyword),
            ("$x", Variable),
            ("@list", Variable),
            ("%h", Variable),
            ("$_", Variable),
            ("$#list", Variable),
            ("$1", Variable),
            ("$Foo::bar", Variable),
        ],
    );
    assert_tokens("perl", "my $r = $a % $b;", &[("%", Operator)]);
    assert_tokens("perl", "$h{key} = $r->{other};", &[("key", Property), ("other", Property)]);
    assert_tokens("perl", "my %c = (name => 'x');", &[("name", Property), ("=>", Operator)]);
    assert_tokens("perl", "$obj->method(1);", &[("$obj", Variable), ("method", Function)]);
}

#[test]
fn perl_quote_like() {
    assert_tokens(
        "perl",
        "$s =~ s/foo/bar/gi; $t =~ tr{a-z}{A-Z}; m#x#;",
        &[("s/foo/bar/gi", Regex), ("tr{a-z}{A-Z}", Regex), ("m#x#", Regex)],
    );
    assert_tokens("perl", "use POSIX qw(floor ceil);", &[("qw(floor ceil)", String)]);
    assert_tokens("perl", "my $q = qq{a {nested} $x};", &[("qq{a {nested} ", String), ("$x", Variable), ("}", String)]);
    assert_tokens("perl", "my @p = split /,\\s*/, $line;", &[("/,\\s*/", Regex)]);
    assert_tokens("perl", "my $s = $y; my $q = $m;", &[("$y", Variable), ("$m", Variable)]);
    assert_not("perl", "my %h = (s => 1, y => 2);", "s => 1, y =>", Regex);
}

#[test]
fn perl_strings_heredocs_and_pod() {
    assert_tokens(
        "perl",
        "print \"Hello $name and @list\\n\";",
        &[("$name", Variable), ("@list", Variable), ("\\n", Escape)],
    );
    assert_tokens("perl", "print 'no $interp';", &[("'no $interp'", String)]);
    assert_tokens(
        "perl",
        "print <<\"END\";\nHi $who\nEND\nmy $x;",
        &[("<<", Operator), ("\"END\"", String), ("$who", Variable), ("\nEND", String), ("$x", Variable)],
    );
    assert_tokens(
        "perl",
        "=head1 NAME\n\nFoo\n\n=cut\nmy $x;",
        &[("=head1 NAME\n\nFoo\n\n=cut", Comment), ("my", Keyword)],
    );
    assert_tokens("perl", "1;\n__END__\nstuff", &[("__END__\nstuff", Comment)]);
}

#[test]
fn lua_long_brackets() {
    assert_tokens(
        "lua",
        "--[==[ comment ]] still ]==]\nlocal s = [[a\nb]] .. [=[c]=]",
        &[
            ("--[==[ comment ]] still ]==]", Comment),
            ("local", Keyword),
            ("[[a\nb]]", String),
            ("..", Operator),
            ("[=[c]=]", String),
        ],
    );
    assert_tokens("lua", "-- line\nx = t[1]", &[("-- line", Comment), ("1", Number)]);
}

#[test]
fn lua_functions() {
    assert_tokens(
        "lua",
        "function M.util.new(x) end\nfunction Obj:method() end\nobj:call(1)",
        &[("M", Text), ("new", Function), ("Obj", Text), ("method", Function), ("call", Function)],
    );
    assert_tokens("lua", "local function helper() end", &[("helper", Function)]);
    assert_tokens("lua", "if a ~= nil then print(#t) end", &[("~=", Operator), ("nil", Constant), ("#", Operator)]);
}

#[test]
fn r_language() {
    assert_tokens(
        "r",
        "f <- function(x) x\ng = function() 1\nh <- \\(y) y",
        &[("f", Function), ("<-", Operator), ("function", Keyword), ("g", Function), ("h", Function), ("\\", Keyword)],
    );
    assert_tokens(
        "r",
        "df$col %in% c(TRUE, NA)",
        &[("$", Operator), ("col", Property), ("%in%", Operator), ("c", Builtin), ("TRUE", Constant), ("NA", Constant)],
    );
    assert_tokens("r", "is.na(x); data.frame(a = 1L)", &[("is.na", Builtin), ("data.frame", Builtin), ("1L", Number)]);
    assert_tokens("r", "x <- 5 %% 2", &[("%%", Operator)]);
}

#[test]
fn julia() {
    assert_tokens(
        "julia",
        "#= a #= nested =# b =#\nx = A' * b'",
        &[("#= a #= nested =# b =#", Comment), ("'", Operator)],
    );
    assert_tokens("julia", "c = 'x'; d = '\\n'", &[("'x'", String), ("'\\n'", String)]);
    assert_tokens(
        "julia",
        "println(\"v = $v and $(f(x))\")",
        &[("println", Builtin), ("$v", Variable), ("$(", Punctuation), ("f", Function)],
    );
    assert_tokens("julia", "push!(xs, 1); @time f(x)", &[("push!", Builtin), ("@time", Meta)]);
    assert_tokens("julia", "r = r\"^\\d+$\"i", &[("r\"^", Regex), ("+$\"i", Regex)]);
    assert_tokens("julia", "x::Vector{Int64}", &[("::", Operator), ("Vector", Type), ("Int64", Type)]);
}

#[test]
fn elixir() {
    assert_tokens(
        "elixir",
        "def greet(name), do: \"Hi #{name}\"",
        &[("def", Keyword), ("greet", Function), ("do", Keyword), ("#{", Punctuation), ("name", Text)],
    );
    assert_tokens("elixir", "[name: :ok, age: 3]", &[("name", Property), (":ok", Constant), ("age", Property)]);
    assert_tokens(
        "elixir",
        "Regex.match?(~r/^\\d+$/i, s) and ~w(a b)a",
        &[("Regex", Type), ("match?", Function), ("~r/^\\d+$/i", Regex), ("~w(a b)a", String)],
    );
    assert_tokens("elixir", "~s(a #{b} c)", &[("~s(a ", String), ("b", Text), (" c)", String)]);
    assert_tokens("elixir", "~S\"\"\"\nraw #{x}\n\"\"\"", &[("~S\"\"\"\nraw #{x}\n\"\"\"", String)]);
    assert_tokens(
        "elixir",
        "@moduledoc \"\"\"\nDocs\n\"\"\"",
        &[("@moduledoc", Meta), ("\"\"\"\nDocs\n\"\"\"", String)],
    );
    assert_tokens("elixir", "alias MyApp.Repo.User", &[("MyApp", Type), ("Repo", Type), ("User", Type)]);
    assert_tokens("elixir", "Enum.map(xs, &(&1 * 2))", &[("&1", Variable)]);
    assert_tokens("elixir", "a = :\"quoted atom\"", &[(":\"quoted atom\"", Constant)]);
    assert_tokens("elixir", "for x <- 1..10, do: x", &[("1", Number), ("..", Punctuation), ("10", Number)]);
}

#[test]
fn nim() {
    assert_tokens(
        "nim",
        "#[ a #[ b ]# c ]#\nproc add*(a: int): int {.inline.} = a",
        &[("#[ a #[ b ]# c ]#", Comment), ("proc", Keyword), ("add", Function), ("int", Type), ("{.inline.}", Meta)],
    );
    assert_tokens("nim", "let x = 10'u8", &[("10'u8", Number)]);
    assert_tokens("nim", "let p = r\"C:\\new\"", &[("r\"C:\\new\"", String)]);
    assert_tokens("nim", "for i in 0..<n: discard", &[("0", Number), ("..", Punctuation), ("<", Operator)]);
}

#[test]
fn coffeescript() {
    assert_tokens(
        "coffeescript",
        "###\nblock\n###\nadd = (a, b) -> a + b\nobj =\n  run: =>\n    @tasks.push 1",
        &[("###\nblock\n###", Comment), ("add", Function), ("->", Operator), ("run", Function), ("@tasks", Variable)],
    );
    assert_tokens("coffeescript", "msg = \"Hi #{name}\"", &[("#{", Punctuation), ("name", Text)]);
    assert_tokens("coffeescript", "re = ///\n a+ # c\n///g", &[("///\n a+ # c\n///g", Regex)]);
    assert_tokens("coffeescript", "x = {key: 1}", &[("key", Property)]);
    assert_not("coffeescript", "x = a == b", "a", Function);
}

#[test]
fn livescript() {
    assert_tokens(
        "livescript",
        "read-file = (path) -> fs.read-file-sync path\nx = \\word",
        &[("read-file", Function), ("read-file-sync", Property), ("\\word", String)],
    );
    assert_tokens("livescript", "y = n-1", &[("n", Text), ("-", Operator), ("1", Number)]);
}

#[test]
fn moonscript() {
    assert_tokens(
        "moonscript",
        "class Foo\n  bar: (x) =>\n    @x = x\nobj\\bar 1\ns = [[long]]",
        &[
            ("class", Keyword),
            ("Foo", Type),
            ("bar", Function),
            ("@x", Variable),
            ("bar", Function),
            ("[[long]]", String),
        ],
    );
}

#[test]
fn tcl() {
    assert_tokens(
        "tcl",
        "proc greet {name} {\n    puts \"Hi $name [string toupper $name]\"\n}",
        &[
            ("proc", Keyword),
            ("greet", Function),
            ("puts", Builtin),
            ("$name", Variable),
            ("[", Punctuation),
            ("string", Builtin),
            ("$name", Variable),
        ],
    );
    assert_tokens(
        "tcl",
        "set x 1 ;# trailing\n# full",
        &[(";", Punctuation), ("# trailing", Comment), ("# full", Comment)],
    );
    assert_not("tcl", "puts #notacomment", "#notacomment", Comment);
    assert_tokens("tcl", "lsort -integer $l", &[("-integer", Attribute), ("$l", Variable)]);
    assert_tokens("tcl", "foreach {k v} $d { myproc $k }", &[("k v", Text), ("myproc", Function)]);
}

#[test]
fn gdscript() {
    assert_tokens(
        "gdscript",
        "@onready var s = $Path/To/Node\nfunc _ready() -> void:\n\tvar n = &\"name\"",
        &[
            ("@onready", Meta),
            ("var", Keyword),
            ("$Path/To/Node", Variable),
            ("func", Keyword),
            ("_ready", Function),
            ("&\"name\"", String),
        ],
    );
    assert_tokens("gdscript", "var x = 10 % 3\nvar h = %HUD", &[("%", Operator), ("%HUD", Variable)]);
}

#[test]
fn mojo() {
    assert_tokens(
        "mojo",
        "struct Point:\n    fn dist(self, out other: Int) raises -> Float64:\n        return 0",
        &[
            ("struct", Keyword),
            ("Point", Type),
            ("fn", Keyword),
            ("dist", Function),
            ("self", Variable),
            ("out", Keyword),
            ("raises", Keyword),
        ],
    );
    // Python keeps its own word lists.
    assert_not("python", "fn = 1", "fn", Keyword);
}

#[test]
fn raku() {
    assert_tokens(
        "raku",
        "my $is-done = True;\nsub to-str(Int $n --> Str) { \"n=$n\" }",
        &[
            ("my", Keyword),
            ("$is-done", Variable),
            ("True", Constant),
            ("to-str", Function),
            ("Int", Type),
            ("$n", Variable),
        ],
    );
    assert_tokens("raku", "has $.name; has @!items;", &[("$.name", Variable), ("@!items", Variable)]);
    assert_tokens("raku", "#`( embedded ) say 1", &[("#`( embedded )", Comment), ("say", Builtin)]);
    assert_tokens(
        "raku",
        "=begin pod\nDocs\n=end pod\nsay 1",
        &[("=begin pod\nDocs\n=end pod", Comment), ("say", Builtin)],
    );
    assert_tokens("raku", "if $s ~~ / \\d+ / { }", &[("/ \\d+ /", Regex)]);
    assert_tokens("raku", "my @w = qw<a b>; f(name => 1)", &[("qw<a b>", String), ("name", Property)]);
    assert_tokens("raku", "say \"Total {$x + 1}\"", &[("{", Punctuation), ("$x", Variable), ("}", Punctuation)]);
}

#[test]
fn autohotkey() {
    assert_tokens(
        "autohotkey",
        "#SingleInstance Force\n^!n::Run \"notepad.exe\"\n::btw::by the way\nMyLabel:\nmsgbox \"a`nb\" ; note",
        &[
            ("#SingleInstance", Meta),
            ("^!n::", Meta),
            ("Run", Builtin),
            ("::btw::", Meta),
            ("by the way", String),
            ("MyLabel:", Meta),
            ("msgbox", Builtin),
            ("`n", Escape),
            ("; note", Comment),
        ],
    );
    assert_tokens("autohotkey", "x := %var% . \"a;b\"", &[("%var%", Variable), ("\"a;b\"", String)]);
    assert_not("autohotkey", "url := \"a\";b", ";b", Comment);
    assert_tokens("autohotkey", "class Foo extends Bar {", &[("class", Keyword), ("Foo", Type), ("Bar", Type)]);
}

#[test]
fn applescript() {
    assert_tokens(
        "applescript",
        "(* a (* b *) c *)\non greet(who)\n\tdisplay dialog \"Hi \" & who\nend greet -- done",
        &[
            ("(* a (* b *) c *)", Comment),
            ("on", Keyword),
            ("greet", Function),
            ("display", Builtin),
            ("-- done", Comment),
        ],
    );
    assert_tokens(
        "applescript",
        "Tell Application \"Finder\" to activate",
        &[("Tell", Keyword), ("Application", Type)],
    );
}

#[test]
fn wren() {
    assert_tokens(
        "wren",
        "class A {\n  construct new(x) { _x = x }\n  show { \"x=%(_x + 1)\\n\" }\n}",
        &[
            ("construct", Keyword),
            ("new", Function),
            ("_x", Variable),
            ("%(", Punctuation),
            ("_x", Variable),
            ("\\n", Escape),
        ],
    );
    assert_tokens("wren", "for (i in 1..5) {}", &[("1", Number), ("..", Punctuation), ("5", Number)]);
}

#[test]
fn squirrel() {
    assert_tokens(
        "squirrel",
        "local p = @\"C:\\dir\"\"q\"\"\"; # c\nfunction f() { return Player(1); }",
        &[("@\"C:\\dir\"\"q\"\"\"", String), ("# c", Comment), ("f", Function), ("Player", Type)],
    );
}

#[test]
fn pony() {
    assert_tokens(
        "pony",
        "actor Main\n  new create(env: Env) =>\n    env.out.print(\"hi\")\n  fun ref go(): U8 => 'a'",
        &[
            ("actor", Keyword),
            ("Main", Type),
            ("new", Keyword),
            ("create", Function),
            ("Env", Type),
            ("ref", Keyword),
            ("go", Function),
            ("'a'", String),
        ],
    );
}

#[test]
fn python_format_strings() {
    assert_tokens(
        "python",
        "f\"{x:>10} {y!r} {z:{w}.2f} {d['k']} {a != b}\"",
        &[
            ("x", Text),
            (":", Punctuation),
            (">10", String),
            ("!r}", Punctuation),
            (":{", Punctuation),
            ("w", Text),
            (".2f", String),
            ("'k'", String),
            ("!=", Operator),
        ],
    );
    assert_tokens(
        "python",
        "rf\"\\d{n}\\\"\" + 1",
        &[("rf\"\\d", String), ("n", Text), ("\\\"\"", String), ("1", Number)],
    );
    // A broken field does not swallow the next line.
    assert_tokens("python", "a = f\"{oops\nb = 1", &[("f\"{oops", String), ("b", Text), ("1", Number)]);
    assert_tokens("python", "t\"{name}\"", &[("t\"", String), ("{", Punctuation), ("name", Text)]);
}

#[test]
fn interpolation_fixes() {
    assert_not("ruby", "x = a ? b: c", "b", Property);
    assert_tokens("ruby", "f? b: 1", &[("b", Property)]);
    assert_tokens("ruby", "f(class: i.even?)", &[("class", Property), ("i", Text)]);
    assert_tokens(
        "ruby",
        "link_to \"x\", class: \"btn\", def: y",
        &[("class", Property), ("def", Property), ("y", Text)],
    );
    assert_not("ruby", "f(class: i.even?)", "i", Type);
    assert_tokens("ruby", "r = /a#{x}b/i", &[("/a", Regex), ("#{", Punctuation), ("x", Text), ("b/i", Regex)]);
    assert_tokens("ruby", "s.split(/ /)", &[("/ /", Regex)]);
    assert_tokens(
        "perl",
        "print \"$h{key} $a[0] $o->{x} @{[ $n + 1 ]}\";",
        &[
            ("$h", Variable),
            ("key", Property),
            ("$a", Variable),
            ("0", Number),
            ("->", Operator),
            ("x", Property),
            ("@{[", Punctuation),
            ("$n", Variable),
            ("]}", Punctuation),
        ],
    );
    assert_tokens("perl", "for (keys %h) { $x = $a % $b }", &[("%h", Variable), ("%", Operator)]);
    assert_tokens("perl", "if ($s =~ /a\\/b/) {}", &[("/a\\/b/", Regex)]);
    assert_tokens(
        "tcl",
        "puts \"a [lindex [split $s ,] 0] b\"",
        &[("lindex", Builtin), ("split", Builtin), ("0", Number), (" b\"", String)],
    );
    assert_tokens("tcl", "if {$x} {puts hi} else {myproc 1}", &[("puts", Builtin)]);
    assert_not("tcl", "set l {red green}", "red", Function);
    assert_tokens("wren", "\"a %(b + 1) c\"", &[("%(", Punctuation), ("b", Text), ("1", Number), (" c\"", String)]);
}

#[test]
fn raku_words_and_heredocs() {
    assert_tokens(
        "raku",
        "my @a = <foo bar>; say %h<key>;",
        &[("<foo bar>", String), ("%h", Variable), ("key", Property)],
    );
    assert_tokens("raku", "if $a < $b { }", &[("<", Operator)]);
    assert_tokens(
        "raku",
        "my $t = qq:to/END/;\n  Hi $name\n  END\nsay 1;",
        &[("qq:to/END/", String), ("$name", Variable), ("\n  END", String), ("say", Builtin)],
    );
    assert_tokens("raku", "sub is-done { isn't-ok() }", &[("is-done", Function), ("isn't-ok", Function)]);
}

#[test]
fn chars_formats_and_comments() {
    assert_tokens("elixir", "x = ?a; y = ?\\n", &[("?a", String), ("?", String), ("\\n", Escape)]);
    assert_tokens("elixir", "empty?(l)", &[("empty?", Function)]);
    assert_tokens(
        "nim",
        "echo &\"x={x:>4}\\t\", fmt\"{y}\"",
        &[("&", Operator), ("x", Text), (">4", String), ("\\t", Escape), ("fmt", Function), ("y", Text)],
    );
    assert_tokens("nim", "#[ a #[ b ]# c ]# x", &[("#[ a #[ b ]# c ]#", Comment)]);
    assert_tokens("julia", "#= a #= b =# c =# x", &[("#= a #= b =# c =#", Comment)]);
    assert_tokens(
        "coffeescript",
        "### block ###\n#### banner\nx = /a[/]b/g",
        &[("### block ###", Comment), ("#### banner", Comment), ("/a[/]b/g", Regex)],
    );
    assert_tokens("livescript", "/* c */ read-file = -> 1", &[("/* c */", Comment), ("read-file", Function)]);
}

#[test]
fn autohotkey_commands() {
    assert_tokens(
        "autohotkey",
        "MsgBox, It's done %name%, 4 ; note\nvar = Hi %x%\nWinMinimize title",
        &[
            ("MsgBox", Builtin),
            ("%name%", Variable),
            ("4", Number),
            ("; note", Comment),
            ("%x%", Variable),
            ("WinMinimize", Function),
        ],
    );
    assert_not("autohotkey", "MsgBox, It's done", "'s done", String);
    assert_tokens("autohotkey", "x := %\"dyn\" i%", &[("%", Punctuation), ("\"dyn\"", String), ("%", Punctuation)]);
    assert_tokens(
        "applescript",
        "on run\nend run\nto go()\nend go\nend tell",
        &[("run", Function), ("run", Function), ("go", Function), ("go", Function), ("tell", Keyword)],
    );
}
