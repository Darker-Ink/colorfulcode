//! Targeted checks for assembly, hardware, legacy and niche languages.

mod common;

use colorfulcode::Kind::*;
use colorfulcode::Language;
use common::{assert_not, assert_tokens};

#[test]
fn lookups() {
    for (name, id) in [
        ("nasm", "asm"),
        ("file.s", "gas"),
        ("aarch64", "arm"),
        ("x.ll", "llvm"),
        ("wast", "wat"),
        ("x.f90", "fortran"),
        ("delphi", "pascal"),
        ("x.ads", "ada"),
        ("x.vhd", "vhdl"),
        ("x.v", "verilog"),
        ("x.sv", "systemverilog"),
        ("x.cbl", "cobol"),
        ("vb", "vbnet"),
        ("vbscript", "vba"),
        ("qbasic", "basic"),
        ("x.4th", "forth"),
        ("octave", "matlab"),
        ("mathematica", "wolfram"),
        ("x.do", "stata"),
        ("x.st", "smalltalk"),
        ("tla+", "tla"),
        ("x.als", "alloy"),
        ("x.scad", "openscad"),
        ("x.nc", "gcode"),
        ("kql", "kusto"),
        ("mql5", "mql"),
    ] {
        let found = if name.contains('.') { Language::from_path(name) } else { Language::find(name) };
        assert_eq!(found.map(|l| l.id()), Some(id), "{name}");
    }
}

#[test]
fn nasm_labels_mnemonics_and_registers() {
    let code = "_start:\n    mov eax, [ebx+4]\nmsg db 'hi', 0\n%define X 1\n    rep movsb\n.loop: jmp .loop";
    assert_tokens(
        "asm",
        code,
        &[
            ("_start", Function),
            ("mov", Keyword),
            ("eax", Variable),
            ("ebx", Variable),
            ("msg", Function),
            ("db", Keyword),
            ("'hi'", String),
            ("%define", Meta),
            ("rep", Keyword),
            ("movsb", Keyword),
            (".loop", Function),
            ("jmp", Keyword),
        ],
    );
    assert_not("asm", "    mov ecx, MOV", "MOV", Keyword);
    assert_tokens("asm", "    mov r8d, 0FFh", &[("r8d", Variable), ("0FFh", Number)]);
}

#[test]
fn gas_att_syntax() {
    let code = "    .globl main\nmain:\n    movl $42, %eax # answer\n    ret";
    assert_tokens(
        "gas",
        code,
        &[
            (".globl", Meta),
            ("main", Function),
            ("movl", Keyword),
            ("$42", Number),
            ("%eax", Variable),
            ("# answer", Comment),
        ],
    );
    assert_tokens("gas", "#include <sys.h>", &[("#include", Meta), ("<sys.h>", String)]);
}

#[test]
fn arm_and_riscv_operands() {
    assert_tokens(
        "arm",
        "    ldr x0, [sp, #16]  // load\n    b.ne 1b",
        &[
            ("ldr", Keyword),
            ("x0", Variable),
            ("sp", Variable),
            ("#16", Number),
            ("// load", Comment),
            ("b.ne", Keyword),
        ],
    );
    assert_tokens(
        "riscv",
        "    addi a0, zero, -1\n    lui t0, %hi(sym)",
        &[("addi", Keyword), ("a0", Variable), ("zero", Variable), ("%hi", Builtin)],
    );
}

#[test]
fn llvm_ir() {
    let code = "define i32 @f(i32 %x) #0 {\nentry:\n  %y = add nsw i32 %x, 1\n  ret i32 %y, !dbg !7\n}";
    assert_tokens(
        "llvm",
        code,
        &[
            ("define", Keyword),
            ("i32", Type),
            ("@f", Function),
            ("%x", Variable),
            ("#0", Meta),
            ("entry", Function),
            ("add", Keyword),
            ("!dbg", Meta),
        ],
    );
    assert_tokens("llvm", r#"@s = constant [3 x i8] c"a\0A\00""#, &[("@s", Variable), ("\\0A\\00", Escape)]);
}

#[test]
fn wat_instructions_and_names() {
    let code = "(func $add (param $a i32) (result i32)\n  local.get $a\n  (call $add) i32.const 1) ;; done";
    assert_tokens(
        "wat",
        code,
        &[
            ("func", Keyword),
            ("$add", Function),
            ("$a", Variable),
            ("i32", Type),
            ("local.get", Keyword),
            (";; done", Comment),
        ],
    );
    assert_tokens("wat", "(; a (; nested ;) b ;) x", &[("(; a (; nested ;) b ;)", Comment)]);
}

#[test]
fn fortran_dot_operators_and_components() {
    let code = "IF (x .GT. 1 .and. .not. done) p%x = .TRUE. ! note";
    assert_tokens(
        "fortran",
        code,
        &[
            ("IF", Keyword),
            (".GT.", Keyword),
            (".and.", Keyword),
            ("x", Property),
            (".TRUE.", Constant),
            ("! note", Comment),
        ],
    );
    assert_tokens("fortran", "subroutine solve(n)\nend subroutine solve", &[("solve", Function), ("solve", Function)]);
}

#[test]
fn pascal_directives_and_literals() {
    let code = "{$mode objfpc} x := $FF + #13; { c } (* d *) S := 'it''s';";
    assert_tokens(
        "pascal",
        code,
        &[
            ("{$mode objfpc}", Meta),
            ("$FF", Number),
            ("#13", String),
            ("{ c }", Comment),
            ("(* d *)", Comment),
            ("'it''s'", String),
        ],
    );
    assert_tokens("pascal", "procedure TFoo.Bar;", &[("TFoo", Type), ("Bar", Function)]);
}

#[test]
fn ada_ticks() {
    let code = "C : Character := 'A'; N := Arr'Length + 16#FF#;";
    assert_tokens("ada", code, &[("'A'", String), ("'Length", Meta), ("16#FF#", Number)]);
    assert_not("ada", "X := Integer'Image (Y);", "'Image (Y);", String);
}

#[test]
fn vhdl_ports_and_literals() {
    let code = "port (clk : in std_logic; q : out my_t);\ns <= '1' when x'event else x\"FF\";";
    assert_tokens(
        "vhdl",
        code,
        &[
            ("clk", Property),
            ("std_logic", Type),
            ("my_t", Type),
            ("'1'", String),
            ("'event", Meta),
            ("x\"FF\"", String),
        ],
    );
    assert_not("vhdl", "signal s : state_t := IDLE;", "state_t", Property);
}

#[test]
fn verilog_literals_directives_and_instances() {
    let code =
        "`define W 8\nassign y = 8'hFF & 4'b10x0;\ncounter #(.W(4)) u0 (.clk(clk));\ninitial $display(\"%d\", y);";
    assert_tokens(
        "verilog",
        code,
        &[
            ("`define", Meta),
            ("8'hFF", Number),
            ("4'b10x0", Number),
            ("counter", Type),
            ("clk", Property),
            ("$display", Builtin),
        ],
    );
    assert_not("verilog", "counter #(.W(4)) u0 (.clk(clk));", "u0", Function);
    assert_tokens("systemverilog", "q.push_back(x); count <= '0;", &[("push_back", Function), ("'0", Number)]);
}

#[test]
fn cobol_fixed_format() {
    let code =
        "000100* comment line\n000200 MAIN-PARA.\n           MOVE 1 TO WS-X. *> note\n       05 WS-A PIC 9(3)V99.";
    assert_tokens(
        "cobol",
        code,
        &[
            ("000100* comment line", Comment),
            ("MAIN-PARA", Function),
            ("MOVE", Keyword),
            ("*> note", Comment),
            ("PIC", Keyword),
            ("9(3)V99", Type),
        ],
    );
}

#[test]
fn visual_basic() {
    let code = "Dim s As String = $\"Hi {name}\" ' greet\nEnd Sub\nx = &HFF\nREM old";
    assert_tokens(
        "vbnet",
        code,
        &[
            ("Dim", Keyword),
            ("String", Type),
            ("name", Text),
            ("' greet", Comment),
            ("Sub", Keyword),
            ("&HFF", Number),
            ("REM old", Comment),
        ],
    );
    assert_not("vbnet", "End Class\nx = 1", "x", Type);
    assert_tokens("vba", "d = #1/2/2020#: s = \"a\"\"b\"", &[("#1/2/2020#", String), ("\"a\"\"b\"", String)]);
    assert_tokens("basic", "PRINT LEFT$(a$, 2)", &[("PRINT", Keyword), ("LEFT$", Builtin)]);
}

#[test]
fn forth_words() {
    let code = ": square ( n -- n*n ) dup * ; \\ comment\n.\" hi\" $FF 1+";
    assert_tokens(
        "forth",
        code,
        &[
            (":", Keyword),
            ("square", Function),
            ("( n -- n*n )", Comment),
            ("dup", Builtin),
            ("\\ comment", Comment),
            (" hi\"", String),
            ("$FF", Number),
            ("1+", Builtin),
        ],
    );
}

#[test]
fn matlab_transpose_and_strings() {
    let code = "function [a, b] = f(x)\ny = x' + A.'; s = 'it''s'; % note";
    assert_tokens(
        "matlab",
        code,
        &[
            ("function", Keyword),
            ("f", Function),
            ("'", Operator),
            ("'", Operator),
            ("'it''s'", String),
            ("% note", Comment),
        ],
    );
    assert_tokens("matlab", "%{\nblock\n%}\nx = [a' b'];", &[("%{\nblock\n%}", Comment)]);
    assert_not("matlab", "disp('a b')", "'a b'", Operator);
}

#[test]
fn wolfram_patterns_and_calls() {
    let code = "f[x_Integer, y_] := Module[{z = #1 &}, x + y] (* c (* n *) *)";
    assert_tokens(
        "wolfram",
        code,
        &[
            ("f", Function),
            ("x_", Variable),
            ("Integer", Type),
            ("y_", Variable),
            ("Module", Keyword),
            ("#1", Variable),
        ],
    );
    assert_tokens("wolfram", "(* c (* n *) *) x", &[("(* c (* n *) *)", Comment)]);
}

#[test]
fn sas_and_stata() {
    let code = "%let n = 5;\n* comment;\ndata out; set in(where=(x > &n)); run;\n%macro go(a); %mend;";
    assert_tokens(
        "sas",
        code,
        &[
            ("%let", Keyword),
            ("n", Variable),
            ("* comment;", Comment),
            ("&n", Variable),
            ("%macro", Keyword),
            ("go", Function),
        ],
    );
    assert_tokens(
        "stata",
        "* note\nforeach v of local vars {\n  di \"`v'\" `v'\n}",
        &[("* note", Comment), ("foreach", Keyword), ("`v'", Variable)],
    );
}

#[test]
fn abap_statements() {
    let code = "* comment\nDATA lv TYPE string. \" note\nlv = |Hi { name }|.\nls_row-field = <fs>-x.\nobj->run( ).";
    assert_tokens(
        "abap",
        code,
        &[
            ("* comment", Comment),
            ("DATA", Keyword),
            ("string", Type),
            ("\" note", Comment),
            ("|Hi ", String),
            ("field", Property),
            ("<fs>", Variable),
            ("run", Function),
        ],
    );
}

#[test]
fn apl_glyphs() {
    let code = "avg←{(+⌿⍵)÷≢⍵} ⍝ mean\n⎕IO←0\n'it''s' ¯1.5";
    assert_tokens(
        "apl",
        code,
        &[
            ("←", Operator),
            ("+", Operator),
            ("⌿", Keyword),
            ("⍵", Variable),
            ("⍝ mean", Comment),
            ("⎕IO", Builtin),
            ("'it''s'", String),
            ("¯1.5", Number),
        ],
    );
}

#[test]
fn smalltalk_messages() {
    let code = "\"note\" x := #(1 $a #foo:bar:) inject: 0 into: [:a :b | a + b]. ^ self";
    assert_tokens(
        "smalltalk",
        code,
        &[
            ("\"note\"", Comment),
            ("$a", String),
            ("#foo:bar:", Constant),
            ("inject:", Function),
            (":a", Variable),
            ("self", Variable),
        ],
    );
}

#[test]
fn tla_definitions_and_operators() {
    let code = "Init == x \\in Nat /\\ y' = 1 \\* note\nNext(a) == UNCHANGED x";
    assert_tokens(
        "tla",
        code,
        &[
            ("Init", Function),
            ("\\in", Operator),
            ("Nat", Type),
            ("\\* note", Comment),
            ("Next", Function),
            ("UNCHANGED", Keyword),
        ],
    );
    assert_tokens(
        "tla",
        "(* --algorithm a\nbegin skip; end algorithm; *)",
        &[("algorithm", Keyword), ("skip", Keyword)],
    );
}

#[test]
fn small_dsls() {
    assert_tokens(
        "alloy",
        "sig File extends Obj {} pred ok[f: File] { some f }",
        &[("sig", Keyword), ("File", Type), ("Obj", Type), ("ok", Function), ("some", Keyword)],
    );
    assert_tokens(
        "openscad",
        "include <lib.scad>\n$fn = 32; module m() { cube(1); }",
        &[("<lib.scad>", String), ("$fn", Variable), ("m", Function), ("cube", Builtin)],
    );
    assert_tokens(
        "gcode",
        "N10 G1 X1.5 Y-2 F300 ; move\nM117 Hello (c)",
        &[
            ("N10", Meta),
            ("G1", Keyword),
            ("X", Property),
            ("1.5", Number),
            ("-2", Number),
            ("; move", Comment),
            ("Hello (c)", String),
        ],
    );
    assert_tokens(
        "qml",
        "Rectangle { width: 10; property int n: 0 }",
        &[("Rectangle", Type), ("width", Property), ("property", Keyword), ("int", Type), ("n", Property)],
    );
    assert_tokens(
        "kusto",
        "T | where x > ago(1d) | project-away y | extend p = @\"C:\\x\"",
        &[("where", Keyword), ("ago", Builtin), ("1d", Number), ("project-away", Keyword), ("@\"C:\\x\"", String)],
    );
    assert_tokens(
        "dax",
        "M = SUMX ( 'My Table', 'My Table'[Qty] * Sales[Price] ) -- c",
        &[("SUMX", Builtin), ("'My Table'", Type), ("[Qty]", Property), ("Sales", Type), ("-- c", Comment)],
    );
    assert_tokens(
        "mql",
        "#property strict\ninput int Period = 14;\nvoid OnTick() { Print(MODE_SMA); }",
        &[
            ("#property", Meta),
            ("input", Keyword),
            ("int", Type),
            ("OnTick", Function),
            ("Print", Builtin),
            ("MODE_SMA", Constant),
        ],
    );
}

#[test]
fn gnu_numeric_labels_and_symbol_immediates() {
    assert_tokens(
        "gas",
        "1:  decl %ecx\n    jnz 1b\n    movl $msg, %eax",
        &[("1", Function), ("decl", Keyword), ("1b", Text), ("$msg", Constant), ("%eax", Variable)],
    );
    assert_tokens(
        "riscv",
        "2:\n    j 2b\n    li a0, 2",
        &[("2", Function), ("2b", Text), ("a0", Variable), ("2", Number)],
    );
    assert_not("gas", "    movl -4(%rbp), %eax", "4", Function);
    assert_tokens("asm", "    mov ax, [rsp+r8]", &[("ax", Variable), ("rsp", Variable), ("r8", Variable)]);
}

#[test]
fn fortran_fixed_form_comments() {
    let code = "C     Compute the sum\nc-----\n*     note\n      X = 1";
    assert_tokens(
        "fortran",
        code,
        &[("C     Compute the sum", Comment), ("c-----", Comment), ("*     note", Comment), ("1", Number)],
    );
    assert_not("fortran", "c = 2\nc(1) = 3\nc%x = 1\ncontains", "c", Comment);
    assert_tokens("fortran", "call foo(c)\ncontains", &[("call", Keyword), ("contains", Keyword)]);
}

#[test]
fn matlab_command_syntax_and_signatures() {
    assert_tokens(
        "matlab",
        "clc; close all\nhold on\nformat long g % c",
        &[
            ("clc", Builtin),
            ("close", Builtin),
            ("all", String),
            ("hold", Builtin),
            ("on", String),
            ("format", Builtin),
            ("long g", String),
            ("% c", Comment),
        ],
    );
    assert_not("matlab", "hold(ax, 'on')\nfigure = 3;", "on", String);
    assert_tokens("matlab", "clear - 1", &[("-", Operator), ("1", Number)]);
    assert_tokens(
        "matlab",
        "function [a, b] = stats(x)\nfunction out = get.Name(obj)\nfunction run",
        &[("a", Text), ("stats", Function), ("out", Text), ("get.Name", Function), ("run", Function)],
    );
}

#[test]
fn macro_references_in_strings() {
    assert_tokens(
        "sas",
        "title \"&title for &&year&i.\" 'not &this';\n%macro report(x);\n%let data = 1;",
        &[
            ("&title", Variable),
            (" for ", String),
            ("&&year&i.", Variable),
            ("'not &this'", String),
            ("%macro", Keyword),
            ("report", Function),
            ("data", Variable),
        ],
    );
    assert_tokens(
        "stata",
        "display \"`g' and $path and ${dir}\"\ndi `\"say \"`x'\"\"'",
        &[("`g'", Variable), ("$path", Variable), ("${dir}", Variable), ("`x'", Variable)],
    );
}

#[test]
fn vbnet_and_vba_keywords() {
    assert_tokens("vbnet", "Try\nCatch ex As Exception\nEnd Try", &[("Try", Keyword), ("Catch", Keyword)]);
    assert_not("vbnet", "Wend", "Wend", Keyword);
    assert_tokens(
        "vba",
        "While x\nWend\nType Point\nEnd Type",
        &[("Wend", Keyword), ("Type", Keyword), ("Point", Type)],
    );
    assert_not("vba", "x = a AndAlso b", "AndAlso", Keyword);
    assert_not("vba", "Try", "Try", Keyword);
}

#[test]
fn forth_definitions_and_wolfram_named_characters() {
    assert_tokens(
        "forth",
        "variable count 10 constant ten create buf",
        &[("count", Variable), ("10", Number), ("ten", Variable), ("buf", Variable)],
    );
    assert_tokens(
        "wolfram",
        "f[\\[Alpha]_] := \\[Alpha] \\[Rule] \\[Pi]; s = \"\\[Beta] \\n\"",
        &[
            ("\\[Alpha]_", Variable),
            ("\\[Rule]", Operator),
            ("\\[Pi]", Constant),
            ("\\[Beta]", Escape),
            ("\\n", Escape),
        ],
    );
}
