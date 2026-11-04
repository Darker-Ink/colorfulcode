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
    assert_tokens("go", "w.WriteHeader(http.StatusOK)", &[("WriteHeader", Function), ("StatusOK", Property)]);
    assert_tokens("go", "func Map[T any](xs []T) []T", &[("Map", Function), ("any", Type), ("T", Type)]);
    assert_tokens("go", "s := `raw \\n` + \"a\\tb\"", &[("`raw \\n`", String), ("\\t", Escape)]);
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
    assert_tokens("csharp", "var j = $$\"\"\"{\"id\": {{id}}}\"\"\";", &[("$$\"\"\"{\"id\": {{id}}}\"\"\"", String)]);
    assert_tokens("csharp", "var @class = 1;", &[("var", Type)]);
    assert_not("csharp", "var @class = 1;", "class", Keyword);
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
}

#[test]
fn scala_interpolation_and_chars() {
    assert_tokens(
        "scala",
        "val s = s\"total $n ${xs.size}\"; val c = 'x'; val sym = 'tag",
        &[("s\"total ", String), ("$n", Variable), ("size", Property), ("'x'", String), ("'tag", Constant)],
    );
    assert_tokens("scala", "def apply[A](x: A): Option[A] = Some(x)", &[("apply", Function), ("Option", Type)]);
}

#[test]
fn dart_strings() {
    assert_tokens("dart", "print('Hi $name ${a.b}');", &[("$name", Variable), ("b", Property)]);
    assert_tokens("dart", "final p = r'C:\\$dir';", &[("r'C:\\$dir'", String)]);
    assert_not("dart", "final p = r'C:\\$dir';", "$dir", Variable);
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
        &[("alloc", Function), ("initWithFormat", Function), ("@\"%d", String), ("\\n", Escape)],
    );
    assert_tokens("objective-c", "- (void)dealloc {", &[("dealloc", Function)]);
    assert_tokens("objective-c", "switch (x) { case FOO: break; default: break; }", &[("FOO", Constant)]);
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
        "groovy",
        "#!/usr/bin/env groovy\ndef s = \"v${major}.$minor\"\ndef re = ~/\\d+/",
        &[("#!/usr/bin/env groovy", Comment), ("def", Keyword), ("$minor", Variable), ("/\\d+/", Regex)],
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
