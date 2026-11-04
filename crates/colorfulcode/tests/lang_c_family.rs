//! Targeted checks for the C family group.

mod common;

use colorfulcode::Kind::*;
use colorfulcode::Language;
use common::{assert_not, assert_tokens};

#[test]
fn lookups() {
    for (name, id) in [
        ("golang", "go"),
        ("c#", "csharp"),
        ("kt", "kotlin"),
        ("objc", "objective-c"),
        ("gradle", "groovy"),
        ("vlang", "v"),
        ("cu", "cuda"),
        ("sol", "solidity"),
    ] {
        assert_eq!(Language::find(name).unwrap().id(), id, "{name}");
    }
    for (path, id) in [
        ("build.gradle", "groovy"),
        ("Jenkinsfile", "groovy"),
        ("kernel.cuh", "cuda"),
        ("App.m", "objective-c"),
        ("build.zig", "zig"),
        ("shader.frag", "glsl"),
        ("main.vsh", "v"),
    ] {
        assert_eq!(Language::from_path(path).unwrap().id(), id, "{path}");
    }
    assert_ne!(Language::from_path("top.v").map(|l| l.id()), Some("v"));
}

#[test]
fn go_exported_names() {
    let code = "func (s *Server) Start(ctx context.Context) error { return ErrClosed }";
    assert_tokens(
        "go",
        code,
        &[("Server", Type), ("Start", Function), ("Context", Type), ("error", Type), ("return", Keyword)],
    );
    assert_not("go", code, "ErrClosed", Type);
    assert_tokens("go", "type User struct {\n\tName string `json:\"name\"`\n}", &[("User", Type), ("Name", Property)]);
    assert_tokens("go", "u := &User{Name: \"x\"}; fmt.Println(u)", &[("User", Type), ("Println", Function)]);
    assert_tokens("go", "w.WriteHeader(http.StatusOK)", &[("WriteHeader", Function), ("StatusOK", Constant)]);
    assert_tokens("go", "func Map[T any](xs []T) []T", &[("Map", Function), ("T", Type), ("any", Type), ("T", Type)]);
    assert_tokens("go", "s := `raw \\n` + \"a\\tb\"", &[("`raw \\n`", String), ("\\t", Escape)]);
}

#[test]
fn go_constants_and_packages() {
    assert_tokens(
        "go",
        "const MaxUsers = 10\nvar ErrNotFound = errors.New(\"x\")\nreturn nil, ErrNotFound",
        &[("MaxUsers", Constant), ("ErrNotFound", Constant), ("New", Function), ("ErrNotFound", Constant)],
    );
    assert_tokens(
        "go",
        "const (\n\tAdmin Role = iota\n\tGuest\n)",
        &[("Admin", Constant), ("Role", Type), ("Guest", Constant)],
    );
    assert_tokens(
        "go",
        "type Base struct {\n\tsync.Mutex\n\tEmbedded\n\tName string\n}",
        &[("Mutex", Type), ("Embedded", Type), ("Name", Property)],
    );
    assert_tokens(
        "go",
        "u := &User{Name: \"x\", Role: Admin}\nswitch r {\ncase Admin, Guest:\n}",
        &[("User", Type), ("Name", Property), ("Role", Property), ("Admin", Constant), ("Guest", Constant)],
    );
    assert_tokens(
        "go",
        "var c = http.DefaultClient\nvar h http.Handler\nd := 5*time.Second\nxs := []*User{}",
        &[("DefaultClient", Constant), ("Handler", Type), ("Second", Constant), ("User", Type)],
    );
    // A lowercase name that is not a known package is a value with fields.
    assert_tokens("go", "u.Name = req.Header", &[("Name", Property), ("Header", Property)]);
    assert_tokens(
        "go",
        "func f(g func(T) U) (U, error) { v := x.(Stringer); p := new(Point) }",
        &[("T", Type), ("U", Type), ("U", Type), ("Stringer", Type), ("Point", Type)],
    );
}

#[test]
fn java_generics_and_annotations() {
    assert_tokens(
        "java",
        "@Override public Map<String, List<Integer>> load() { return new HashMap<>(); }",
        &[
            ("@Override", Meta),
            ("Map", Type),
            ("String", Type),
            ("List", Type),
            ("Integer", Type),
            ("load", Function),
            ("HashMap", Type),
        ],
    );
    assert_tokens(
        "java",
        "String s = \"\"\"\n  hi \"quoted\"\n  \"\"\";",
        &[("\"\"\"\n  hi \"quoted\"\n  \"\"\"", String)],
    );
    assert_tokens("java", "static final int MAX = 1; char c = 'x';", &[("MAX", Constant), ("'x'", String)]);
}

#[test]
fn csharp_strings() {
    assert_tokens(
        "csharp",
        "var s = $\"Hi {name}, {{literal}}\";",
        &[("$\"Hi ", String), ("{", Punctuation), ("name", Text), ("}", Punctuation), (", {{literal}}\"", String)],
    );
    assert_tokens("csharp", "var p = @\"C:\\dir\\\"\"x\"\"\";", &[("@\"C:\\dir\\\"\"x\"\"\"", String)]);
    assert_tokens("csharp", "var r = \"\"\"\n  {\"a\": 1}\n  \"\"\";", &[("\"\"\"\n  {\"a\": 1}\n  \"\"\"", String)]);
    assert_tokens(
        "csharp",
        "var j = $$\"\"\"{\"id\": {{id}}, \"n\": {{{Name}}}}\"\"\";",
        &[
            ("$$\"\"\"{\"id\": ", String),
            ("{{", Punctuation),
            ("id", Text),
            ("}}", Punctuation),
            (", \"n\": {", String),
            ("{{", Punctuation),
            ("Name", Property),
            ("}}", Punctuation),
            ("}}\"\"\"", String),
        ],
    );
    assert_tokens("csharp", "var r = \"\"\"\" a \"\"\" b \"\"\"\";", &[("\"\"\"\" a \"\"\" b \"\"\"\"", String)]);
    assert_tokens("csharp", "var s = $\"{Total:C2}\";", &[("Total", Property), ("C2", String)]);
    assert_tokens("csharp", "var @class = 1;", &[("var", Type)]);
    assert_not("csharp", "var @class = 1;", "class", Keyword);
}

#[test]
fn csharp_values_and_enums() {
    assert_tokens(
        "csharp",
        "var s = $\"{Id} {Name,10}\"; return (Id, Name);",
        &[("Id", Property), ("Name", Property), ("Id", Property), ("Name", Property)],
    );
    assert_tokens(
        "csharp",
        "if (Done) Run(Count, Limit); var t = (Id: 1, Name: \"x\"); x = Ok ? First : Last;",
        &[
            ("Done", Property),
            ("Count", Property),
            ("Limit", Property),
            ("Id", Property),
            ("Name", Property),
            ("First", Property),
            ("Last", Property),
        ],
    );
    assert_tokens(
        "csharp",
        "Dictionary<Foo, Bar> d; class A : IA, IB { } var c = (Foo)y; var t = typeof(Foo);",
        &[("Foo", Type), ("Bar", Type), ("IA", Type), ("IB", Type), ("Foo", Type), ("Foo", Type)],
    );
    assert_tokens(
        "csharp",
        "enum Color : byte\n{\n    // warm\n    Red,\n    Green = 2,\n    Both = Red | Green\n}",
        &[
            ("Color", Type),
            ("byte", Type),
            ("// warm", Comment),
            ("Red", Constant),
            ("Green", Constant),
            ("2", Number),
            ("Both", Constant),
            ("Red", Constant),
        ],
    );
    assert_tokens("csharp", "var xs = new[] { Red, Green };", &[("Red", Property), ("Green", Property)]);
}

#[test]
fn csharp_attributes_and_members() {
    assert_tokens(
        "csharp",
        "[Obsolete(\"old\")]\npublic void Run([FromBody] Order order) { }",
        &[("[Obsolete", Meta), ("\"old\"", String), ("]", Meta), ("Run", Function), ("[FromBody]", Meta)],
    );
    assert_tokens("csharp", "[assembly: InternalsVisibleTo(\"Tests\")]", &[("[assembly: InternalsVisibleTo", Meta)]);
    assert_tokens(
        "csharp",
        "public List<string> Names { get; set; } = new();",
        &[("List", Type), ("string", Type), ("Names", Property), ("get", Keyword)],
    );
    assert_tokens("csharp", "if (count >= MaxCount) Name = name;", &[("MaxCount", Property), ("Name", Property)]);
    assert_tokens(
        "csharp",
        "string? s = Customer?.Name ?? Fallback; Foo? f = null;",
        &[("string", Type), ("Customer", Property), ("Name", Property), ("Fallback", Property), ("Foo", Type)],
    );
    assert_tokens(
        "csharp",
        "Console.WriteLine(new Order { Id = 1 });",
        &[("Console", Type), ("WriteLine", Function), ("Order", Type), ("Id", Property)],
    );
    assert_tokens("csharp", "#region Helpers\nint[] xs = [1, 2];", &[("#region", Meta), ("int", Type), ("1", Number)]);
}

#[test]
fn kotlin_templates() {
    assert_tokens(
        "kotlin",
        "val s = \"Hi $name, ${user.age + 1}\"",
        &[("val", Keyword), ("$name", Variable), ("${", Punctuation), ("age", Property), ("1", Number)],
    );
    assert_tokens("kotlin", "val r = \"\"\"C:\\n $path\"\"\"", &[("\"\"\"C:\\n ", String), ("$path", Variable)]);
    assert_tokens(
        "kotlin",
        "fun <T> List<T>.second(): T = this[1]",
        &[("fun", Keyword), ("T", Type), ("List", Type), ("second", Function), ("this", Variable)],
    );
    assert_tokens("kotlin", "@Composable fun App() = Text(\"x\")", &[("@Composable", Meta), ("Text", Type)]);
}

#[test]
fn swift_features() {
    assert_tokens(
        "swift",
        "print(\"Hello \\(user.name)!\\n\")",
        &[("print", Builtin), ("\"Hello ", String), ("\\(", Punctuation), ("name", Property), ("\\n", Escape)],
    );
    assert_tokens(
        "swift",
        "#if DEBUG\n@MainActor func load() {}\n#endif",
        &[("#if", Meta), ("@MainActor", Meta), ("load", Function), ("#endif", Meta)],
    );
    assert_tokens("swift", "let r = #\"a \"b\" \\d\"#", &[("#\"a \"b\" \\d\"#", String)]);
    assert_tokens("swift", "xs.map { $0 * 2 }", &[("$0", Variable)]);
    assert_tokens(
        "swift",
        "let r = #\"\\d \\#(user.name)\\#n\"#",
        &[("#\"\\d ", String), ("\\#(", Punctuation), ("name", Property), ("\\#n", Escape), ("\"#", String)],
    );
    assert_tokens("swift", "let s = ##\"\"\"\n\"# x\n\"\"\"##", &[("##\"\"\"\n\"# x\n\"\"\"##", String)]);
    assert_tokens("swift", "let id = UUID(); let u = URL(string: s)", &[("UUID", Type), ("URL", Type)]);
}

#[test]
fn scala_interpolation_and_chars() {
    assert_tokens(
        "scala",
        "val s = s\"total $n ${xs.size}\"; val c = 'x'; val sym = 'tag",
        &[("s\"total ", String), ("$n", Variable), ("size", Property), ("'x'", String), ("'tag", Constant)],
    );
    assert_tokens("scala", "def apply[A](x: A): Option[A] = Some(x)", &[("apply", Function), ("Option", Type)]);
    assert_tokens(
        "scala",
        "val p = raw\"C:\\n$dir\"; val q = sql\"id = ${id}\"; val t = s\"\"\"a $b\"\"\"",
        &[("raw\"C:\\n", String), ("$dir", Variable), ("sql\"id = ", String), ("${", Punctuation), ("$b", Variable)],
    );
    assert_not("scala", "val p = raw\"C:\\n$dir\"", "\\n", Escape);
}

#[test]
fn dollar_interpolation_skips_parens() {
    for lang in ["kotlin", "groovy", "dart"] {
        assert_tokens(lang, "x = \"a $(b) $c\"", &[("\"a $(b) ", String), ("$c", Variable)]);
    }
}

#[test]
fn dart_strings() {
    assert_tokens("dart", "print('Hi $name ${a.b}');", &[("$name", Variable), ("b", Property)]);
    assert_tokens("dart", "final p = r'C:\\$dir';", &[("r'C:\\$dir'", String)]);
    assert_not("dart", "final p = r'C:\\$dir';", "$dir", Variable);
    assert_tokens("dart", "final q = r\"\"\"a\n$b\"\"\";", &[("r\"\"\"a\n$b\"\"\"", String)]);
}

#[test]
fn objective_c_messages() {
    assert_tokens(
        "objective-c",
        "@interface Foo : NSObject\n- (void)setName:(NSString *)name;\n@end",
        &[("@interface", Keyword), ("Foo", Type), ("setName", Function), ("NSString", Type), ("@end", Keyword)],
    );
    assert_not("objective-c", "- (void)setName:(NSString *)name;", "name", Function);
    assert_tokens(
        "objective-c",
        "NSString *s = [[NSString alloc] initWithFormat:@\"%d\\n\", n];",
        &[("alloc", Function), ("initWithFormat", Function), ("@\"", String), ("%d\\n", Escape)],
    );
    assert_tokens("objective-c", "- (void)dealloc {", &[("dealloc", Function)]);
    assert_tokens("objective-c", "switch (x) { case FOO: break; default: break; }", &[("FOO", Constant)]);
    assert_tokens(
        "objective-c",
        "+ (instancetype)shared;\n[self.view addSubview:v];\nint y = a ? b : c; if (f(x)) {}",
        &[("shared", Function), ("view", Property), ("addSubview", Function), ("f", Function)],
    );
    assert_not("objective-c", "int y = a ? b : c;", "b", Function);
    assert_not("objective-c", "[self.view addSubview:v];", "v", Function);
}

#[test]
fn zig_builtins_and_multiline_strings() {
    assert_tokens(
        "zig",
        "const std = @import(\"std\");\nconst s =\n    \\\\line one\n    \\\\line two\n;",
        &[("@import", Builtin), ("\\\\line one", String), ("\\\\line two", String)],
    );
    assert_tokens("zig", "fn add(a: i32, b: i32) i32 {}", &[("add", Function), ("i32", Type)]);
}

#[test]
fn d_and_groovy() {
    assert_tokens(
        "d",
        "/+ a /+ b +/ c +/ auto x = to!string(1);",
        &[("/+ a /+ b +/ c +/", Comment), ("to!", Function)],
    );
    assert_tokens(
        "d",
        "auto a = q{ x = \"}\"; { y } }; auto b = q\"(a (b))\" ~ q\"/c/\" ~ q\"EOS\nEOS x\nEOS\"; auto queue = 1;",
        &[
            ("q{ x = \"}\"; { y } }", String),
            ("q\"(a (b))\"", String),
            ("q\"/c/\"", String),
            ("q\"EOS\nEOS x\nEOS\"", String),
            ("auto", Keyword),
        ],
    );
    assert_tokens(
        "groovy",
        "#!/usr/bin/env groovy\ndef s = \"v${major}.$minor\"\ndef re = ~/\\d+/",
        &[
            ("#!/usr/bin/env groovy", Comment),
            ("def", Keyword),
            ("$minor", Variable),
            ("/", Regex),
            ("\\d", Escape),
            ("+", Operator),
        ],
    );
}

#[test]
fn solidity_types() {
    assert_tokens(
        "solidity",
        "IERC20 public token; uint256 constant MAX = 1 ether; function f(address a) external {}",
        &[("IERC20", Type), ("uint256", Type), ("MAX", Constant), ("ether", Constant), ("f", Function)],
    );
}

#[test]
fn shaders() {
    assert_tokens(
        "glsl",
        "#version 330 core\nuniform sampler2D tex;\nvec4 c = texture(tex, uv);",
        &[("#version", Meta), ("uniform", Keyword), ("sampler2D", Type), ("vec4", Type), ("texture", Builtin)],
    );
    assert_tokens(
        "hlsl",
        "[numthreads(8, 8, 1)]\nvoid CSMain(uint3 id : SV_DispatchThreadID) { Texture2D<float4> Tex; }",
        &[("[numthreads", Meta), ("CSMain", Function), ("SV_DispatchThreadID", Meta), ("Texture2D", Type)],
    );
    assert_tokens(
        "wgsl",
        "@group(0) @binding(1) var<uniform> cam: Camera;\n@vertex fn vs() -> @builtin(position) vec4f {}",
        &[("@group", Meta), ("var", Keyword), ("Camera", Type), ("vs", Function), ("vec4f", Type)],
    );
    assert_tokens(
        "cuda",
        "__global__ void k(float* __restrict__ a) { int i = threadIdx.x; }",
        &[("__global__", Keyword), ("k", Function), ("__restrict__", Keyword), ("threadIdx", Builtin)],
    );
}

#[test]
fn v_odin_haxe() {
    assert_tokens(
        "v",
        "@[inline]\nfn (mut c Counter) add(w string) { println('${w}') }",
        &[("@[inline]", Meta), ("Counter", Type), ("add", Function), ("${", Punctuation)],
    );
    assert_tokens(
        "odin",
        "Bag :: struct {}\nadd :: proc(b: ^Bag) {}\n#partial switch x {}",
        &[("Bag", Type), ("add", Function), ("proc", Keyword), ("#partial", Meta)],
    );
    assert_tokens(
        "haxe",
        "@:keep class A { var re = ~/a\\/b/g; var s = 'hi $name'; }",
        &[("@:keep", Meta), ("A", Type), ("~/a\\/b/g", Regex), ("$name", Variable)],
    );
}

#[test]
fn vala_apex_actionscript() {
    assert_tokens("vala", "var s = @\"Hello $name $(a.b)\";", &[("@\"Hello ", String), ("$name", Variable)]);
    assert_tokens(
        "apex",
        "List<Account> xs = [SELECT Id FROM Account WHERE Name = :name];",
        &[("List", Type), ("SELECT", Keyword), ("FROM", Keyword), (":name", Variable)],
    );
    assert_tokens(
        "actionscript",
        "[Bindable]\npublic var label:String = \"x\";",
        &[("[Bindable]", Meta), ("var", Keyword), ("String", Type)],
    );
}

#[test]
fn hare_and_carbon() {
    assert_tokens(
        "hare",
        "export fn main() void = { fmt::println(`raw\\n`)!; };",
        &[("export", Keyword), ("main", Function), ("void", Type), ("println", Function), ("`raw\\n`", String)],
    );
    assert_tokens(
        "carbon",
        "fn Area[self: Self]() -> f64 { return self.r; }",
        &[("Area", Function), ("self", Variable), ("Self", Type), ("f64", Type)],
    );
}

#[test]
fn d_manifest_constants() {
    assert_tokens("d", "enum answer = 42;", &[("enum", Keyword), ("answer", Constant), ("42", Number)]);
    assert_tokens("d", "enum int limit = 10;", &[("int", Type), ("limit", Constant)]);
    assert_tokens("d", "enum Color { red, green }", &[("enum", Keyword), ("Color", Type)]);
}
