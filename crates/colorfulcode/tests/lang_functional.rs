//! Targeted checks for the functional languages and the Lisp family.

mod common;

use colorfulcode::Kind::*;
use colorfulcode::Language;
use common::{assert_not, assert_tokens};

#[test]
fn haskell_comments_and_operators() {
    assert_tokens("haskell", "x = 1 -- note", &[("-- note", Comment)]);
    assert_tokens("haskell", "x = 1 --- dashes", &[("--- dashes", Comment)]);
    assert_tokens("haskell", "a --> b", &[("-->", Operator)]);
    assert_tokens("haskell", "a |-- b", &[("|--", Operator)]);
    assert_not("haskell", "a --> b", "--> b", Comment);
    assert_tokens("haskell", "{- a {- nested -} b -} x", &[("{- a {- nested -} b -}", Comment)]);
    assert_tokens("haskell", "{-# LANGUAGE GADTs #-}\nmodule M where", &[("{-# LANGUAGE GADTs #-}", Meta)]);
}

#[test]
fn haskell_primes_and_chars() {
    let code = "go' x' = ['a', '\\n', '\\''] ++ show x'";
    assert_tokens("haskell", code, &[("go'", Function), ("'a'", String), ("\\n", Escape), ("\\'", Escape)]);
    assert_not("haskell", code, "' x'", String);
    // Promotion ticks are not character literals.
    assert_tokens("haskell", "type T = '[Int, Bool]", &[("'", Operator), ("Int", Type)]);
}

#[test]
fn haskell_qualified_names_and_declarations() {
    assert_tokens(
        "haskell",
        "import qualified Data.Map.Strict as M\nf = M.insert k v Map.empty",
        &[("Data.Map.Strict", Type), ("M", Type), (".", Punctuation), ("Map", Type)],
    );
    assert_not("haskell", "f = M.insert k v", ".", Operator);
    assert_tokens(
        "haskell",
        "data P = P { name :: String, age :: Int }\nlen :: [a] -> Int\nlen xs = 0",
        &[("name", Property), ("age", Property), ("len", Function), ("len", Function)],
    );
    // Column 0 infix definitions name the operator, not the argument.
    assert_not("haskell", "a <+> b = a", "a", Function);
}

#[test]
fn elm_and_purescript() {
    assert_tokens(
        "elm",
        "update : Msg -> Model\nupdate msg model = { model | count = model.count + 1 }",
        &[("update", Function), ("Msg", Type), ("update", Function), ("count", Property)],
    );
    assert_tokens("elm", "view = Html.div [] [ text \"\"\"a\nb\"\"\" ]", &[("Html", Type), ("div", Text)]);
    assert_tokens("purescript", "main :: Effect Unit\nmain = log \"hi\"", &[("main", Function), ("Effect", Type)]);
}

#[test]
fn idris_agda_and_unison() {
    assert_tokens("idris", "%default total\n||| Docs\nf : Nat -> Nat", &[("%default", Meta), ("||| Docs", Comment)]);
    assert_tokens(
        "agda",
        "_+_ : ℕ → ℕ → ℕ\n+-comm : ∀ m n → m + n ≡ n + m",
        &[("_+_", Function), ("+-comm", Function)],
    );
    assert_tokens("agda", "f x = x -> y", &[("=", Operator), ("->", Operator)]);
    assert_tokens(
        "unison",
        "c = ?a\n{{ Doc }}\nx = '(f 1)",
        &[("?a", String), ("{{ Doc }}", Comment), ("'", Operator)],
    );
}

#[test]
fn lean_and_coq() {
    assert_tokens(
        "lean",
        "@[simp] theorem foo : 1 + 1 = 2 := by decide\n#eval s!\"x {y}\"",
        &[("@[simp]", Meta), ("foo", Function), ("decide", Builtin), ("#eval", Meta), ("s!\"x ", String)],
    );
    assert_tokens("lean", "/- a /- b -/ c -/ def h' := 'x'", &[("/- a /- b -/ c -/", Comment), ("h'", Function)]);
    assert_tokens(
        "coq",
        "(* a (* b *) *) Fixpoint size (t : tree) : nat := \"say \"\"hi\"\"\".",
        &[("(* a (* b *) *)", Comment), ("size", Function), ("nat", Type), ("\"say \"\"hi\"\"\"", String)],
    );
}

#[test]
fn ocaml_features() {
    assert_tokens(
        "ocaml",
        "let rec map f = function [] -> [] | x :: xs -> f x :: map f xs",
        &[("let", Keyword), ("rec", Keyword), ("map", Function)],
    );
    assert_tokens("ocaml", "let x = 1 and y = 2", &[("x", Text), ("y", Text)]);
    assert_tokens(
        "ocaml",
        "type 'a t = 'a list [@@deriving show]\nlet c = 'x' and d = '\\n'",
        &[("'a", Type), ("t", Type), ("[@@deriving", Meta), ("'x'", String), ("\\n", Escape)],
    );
    assert_tokens(
        "ocaml",
        "let s = {|raw \"x\"|} ^ {id|a|b|id} in f ~cmp:compare `Red (a +. b)",
        &[("{|raw \"x\"|}", String), ("{id|a|b|id}", String), ("~cmp", Property), ("`Red", Constant), ("+.", Operator)],
    );
}

#[test]
fn fsharp_features() {
    assert_tokens(
        "fsharp",
        "[<EntryPoint>]\nlet main argv =\n    let! x = y\n    printfn $\"{x}\" (*) 1",
        &[
            ("[<EntryPoint>]", Meta),
            ("main", Function),
            ("let!", Keyword),
            ("$\"", String),
            ("x", Text),
            ("*", Operator),
        ],
    );
    assert_tokens(
        "fsharp",
        "type P() =\n    member this.Area = 1\n    member _.Scale(f) = f\nlet p = @\"C:\\\"\"x\"\"\"",
        &[("this", Variable), ("Area", Property), ("Scale", Function), ("@\"C:\\\"\"x\"\"\"", String)],
    );
    assert_tokens("fsharp", "let ``my test`` () = ()", &[("``my test``", Function)]);
    assert_tokens("fsharp", "#if DEBUG\n// c\n(* b *)", &[("#if", Meta), ("// c", Comment), ("(* b *)", Comment)]);
}

#[test]
fn sml_and_reason() {
    assert_tokens(
        "sml",
        "fun len nil = 0 | len (_ :: xs) = 1 + len xs\nval c = #\"a\" val x = #name r",
        &[("fun", Keyword), ("len", Function), ("#\"a\"", String), ("#name", Property)],
    );
    assert_tokens(
        "reason",
        "let add = (a, b) => a + b;\nlet s = {j|hi $x|j};\nlet f = (~name) => <div className=\"x\" />;",
        &[("add", Function), ("{j|hi $x|j}", String), ("~name", Property), ("div", Tag)],
    );
}

#[test]
fn lisp_calls_keywords_and_numbers() {
    assert_tokens(
        "clojure",
        "(defn greet [name] (str \"hi \" name :done ::local -1 22/7 \\a \\newline))",
        &[
            ("defn", Keyword),
            ("greet", Function),
            ("str", Builtin),
            (":done", Constant),
            ("::local", Constant),
            ("-1", Number),
            ("22/7", Number),
            ("\\a", String),
            ("\\newline", String),
        ],
    );
    assert_tokens(
        "clojure",
        "(my-fn #\"\\d+\" #_(ignored form) ^:private *out* (-> x inc))",
        &[("my-fn", Function), ("#\"", Regex), ("#_(ignored form)", Comment), ("^:private", Meta), ("*out*", Variable)],
    );
    assert_tokens("clojure", "(- x 1)", &[("-", Function), ("1", Number)]);
}

#[test]
fn scheme_and_racket_reader_syntax() {
    assert_tokens(
        "scheme",
        "(define (f x) (list #t #f #\\space #xFF)) #| a #| b |# |# #;(gone)",
        &[
            ("f", Function),
            ("#t", Constant),
            ("#\\space", String),
            ("#xFF", Number),
            ("#| a #| b |# |#", Comment),
            ("#;(gone)", Comment),
        ],
    );
    assert_tokens(
        "racket",
        "#lang racket\n(define (g #:limit [n 1]) (regexp-match #rx\"^a\" #\"b\"))",
        &[("#lang racket", Meta), ("g", Function), ("#:limit", Constant), ("#rx\"^a\"", Regex), ("#\"b\"", String)],
    );
}

#[test]
fn common_lisp_and_emacs_lisp() {
    assert_tokens(
        "commonlisp",
        "(defun f (x &key y) (mapcar #'car *list*) (1+ +max+) #+sbcl :ok)",
        &[
            ("f", Function),
            ("&key", Keyword),
            ("#'", Operator),
            ("car", Function),
            ("*list*", Variable),
            ("1+", Builtin),
            ("+max+", Constant),
            ("#+sbcl", Meta),
            (":ok", Constant),
        ],
    );
    assert_tokens(
        "emacslisp",
        "(defun my-cmd () (insert ?a ?\\n ?\\C-x) t)",
        &[("my-cmd", Function), ("?a", String), ("?\\n", String), ("?\\C-x", String), ("t", Constant)],
    );
}

#[test]
fn fennel_janet_and_hy() {
    assert_tokens(
        "fennel",
        "(fn add [a b] (+ a b)) (fn [x] x) (local t {:k 1})",
        &[("fn", Keyword), ("add", Function), ("fn", Keyword), ("x", Text), (":k", Constant)],
    );
    assert_not("fennel", "(fn [x] x)", "x", Function);
    assert_tokens("janet", "# comment\n(defn f [x] ``long\nstring``)", &[("# comment", Comment), ("f", Function)]);
    assert_tokens("janet", "(def s ``a\nb``)", &[("``a\nb``", String)]);
    assert_tokens(
        "hy",
        "(defn f [self] (print f\"{x}\" #[[raw]] True))",
        &[("f", Function), ("self", Variable), ("#[[raw]]", String), ("True", Constant)],
    );
}

#[test]
fn erlang_features() {
    assert_tokens(
        "erlang",
        "-module(m).\n-export([f/1]).\nf(X) -> io:format(\"~p\", [X, ok, $a, 16#FF, ?MODULE, #rec{a = 1}]).",
        &[
            ("-module", Meta),
            ("f", Function),
            ("f", Function),
            ("X", Variable),
            ("io", Type),
            ("format", Function),
            ("ok", Constant),
            ("$a", String),
            ("16#FF", Number),
            ("?MODULE", Meta),
            ("#rec", Type),
            ("a", Property),
        ],
    );
    assert_tokens("erlang", "X = 'quoted atom', % note", &[("'quoted atom'", Constant), ("% note", Comment)]);
}

#[test]
fn prolog_features() {
    assert_tokens(
        "prolog",
        ":- use_module(library(lists)).\nanc(X, Y) :- parent(X, Z), anc(Z, Y), C = 0'a, !.",
        &[
            (":-", Meta),
            ("use_module", Meta),
            ("anc", Function),
            ("X", Variable),
            (":-", Operator),
            ("parent", Function),
            ("0'a", Number),
            ("!", Operator),
        ],
    );
    assert_tokens("prolog", "greet --> [hello]. % c\n/* b */", &[("greet", Function), ("% c", Comment)]);
}

#[test]
fn newer_ml_languages() {
    assert_tokens(
        "gleam",
        "pub fn main() { io.println(\"hi\") |> Ok }\n@external(erlang, \"a\", \"b\")",
        &[("main", Function), ("println", Function), ("Ok", Type), ("@external", Meta)],
    );
    assert_tokens(
        "roc",
        "area : Shape -> F64\narea = |s| s.width\ngreet = \\name -> \"Hi ${name}\"\nmain! = Stdout.line!(\"x\")",
        &[("area", Function), ("Shape", Type), ("area", Function), ("width", Property), ("greet", Function)],
    );
    assert_tokens("roc", "x = Stdout.line!(\"x\") # c", &[("Stdout", Type), ("line!", Function), ("# c", Comment)]);
    assert_tokens(
        "grain",
        "let add = (a, b) => a + b\nenum Shape { Circle(Number) }",
        &[("add", Function), ("Shape", Type), ("Circle", Type)],
    );
    assert_tokens(
        "koka",
        "fun is-empty( xs : list<a> ) : bool\n  xs.length == 0 /* a /* b */ c */",
        &[("is-empty", Function), ("list", Type), ("length", Property), ("/* a /* b */ c */", Comment)],
    );
}

#[test]
fn lisp_binding_forms() {
    let code = "(let loop ((x 1) (y (f 2))) (g x y))";
    assert_tokens("scheme", code, &[("let", Keyword), ("loop", Function), ("x", Text), ("y", Text), ("f", Function)]);
    assert_not("scheme", code, "x", Function);
    assert_not("scheme", code, "y", Function);
    assert_not("racket", "(let ([p (make-point 1 2)]) (λ (x) x))", "x", Function);
    assert_not("scheme", "(do ((i 0 (+ i 1))) ((= i 3)) i)", "i", Function);
    let code = "(flet ((double (x) (* 2 x))) (double 3))\n(defun withdraw (acct &key (force nil)) acct)";
    assert_tokens(
        "commonlisp",
        code,
        &[("double", Function), ("x", Text), ("withdraw", Function), ("acct", Text), ("&key", Keyword)],
    );
    assert_not("commonlisp", code, "force", Function);
    assert_not("commonlisp", "(dolist (x (list 1 2)) (print x))", "x", Function);
    assert_tokens("commonlisp", "(dolist (x (list 1 2)) x)", &[("list", Builtin)]);
    assert_tokens(
        "commonlisp",
        "(defmethod area :around ((s circle)) s)",
        &[("area", Function), (":around", Constant)],
    );
    assert_tokens("commonlisp", "(let ((x 1) ; one\n (y 2)) x)", &[("; one", Comment), ("y", Text)]);
    assert_not("emacslisp", "(let* ((a 1)) (lambda (b) (+ a b)))", "a", Function);
    // Unclosed forms stay linear and keep their text.
    assert_tokens("scheme", "(let ((x (f", &[("x", Text), ("f", Function)]);
    // Vectors never mark names as calls; `fn` names the function that follows.
    assert_tokens("clojure", "(fn add [x] x) (let [y 1] y)", &[("add", Function), ("x", Text), ("y", Text)]);
    assert_tokens("janet", "(fn add [x] x)", &[("add", Function)]);
}

#[test]
fn unicode_symbols() {
    let code = "def MyList.length : MyList α → Nat := ⟨a, b⟩\nexample : ∀ n, n ≤ m×n := h▸rfl";
    assert_tokens(
        "lean",
        code,
        &[
            ("MyList.length", Function),
            ("α", Text),
            ("→", Operator),
            ("⟨", Punctuation),
            ("a", Text),
            ("b", Text),
            ("⟩", Punctuation),
            ("∀", Keyword),
            ("≤", Operator),
            ("m", Text),
            ("×", Operator),
            ("h", Text),
            ("▸", Operator),
        ],
    );
    assert_tokens(
        "lean",
        "theorem foo (x₁ : ℕ) := s!\"{x₁}\"",
        &[("foo", Function), ("x₁", Parameter), ("s!\"", String)],
    );
    let code = "+-comm : ∀ m → m + n ≡ n\nproof = begin m ≡⟨ refl ⟩ n ∎";
    assert_tokens(
        "agda",
        code,
        &[("+-comm", Function), ("∀", Keyword), ("→", Keyword), ("≡", Operator), ("≡⟨", Operator), ("⟩", Operator)],
    );
    assert_tokens("koka", "val f := x → y", &[("→", Operator)]);
}

#[test]
fn koka_names_and_types() {
    let code = "fun sum-asked( n : int ) : ask int\n  sum-asked(n-1) + ask-number()";
    assert_tokens(
        "koka",
        code,
        &[("sum-asked", Function), ("int", Type), ("ask", Type), ("sum-asked", Function), ("n", Text), ("-", Operator)],
    );
    assert_tokens("koka", "fun area( s : shape ) : float64", &[("shape", Type)]);
    assert_not("koka", "val x := 1\nfun f() : console ()", "console", Function);
}

#[test]
fn reason_type_annotations() {
    let code = "let describe = (status: status(user)) => 1;\nlet x: result(int, string) = f(1);";
    assert_tokens("reason", code, &[("status", Type), ("result", Type), ("f", Function)]);
    assert_tokens("reason", "let r = {a: compute(x)};", &[("compute", Function)]);
    assert_tokens("reason", "let t = c ? a : g(x);", &[("g", Function)]);
}

#[test]
fn haskell_quasi_quotes() {
    let code = "q = [sql| SELECT * FROM t |]\npage = [hamlet|\n  <p>Hi\n|]\ndone = 1";
    assert_tokens(
        "haskell",
        code,
        &[
            ("sql", Function),
            (" SELECT * FROM t ", String),
            ("|]", Punctuation),
            ("hamlet", Function),
            ("\n  <p>Hi\n", String),
            ("done", Function),
        ],
    );
    // List comprehensions are not quasi-quotes.
    let code = "evens = [x|x<-xs, even x]";
    assert_tokens("haskell", code, &[("x", Text), ("|", Operator), ("<-", Operator)]);
    assert_not("haskell", code, "x", Function);
}

#[test]
fn erlang_macro_and_record_definitions() {
    let code = "-define(TIMEOUT, 5000).\n-record(point, {x = 0}).\nf() -> ?TIMEOUT.";
    assert_tokens("erlang", code, &[("TIMEOUT", Meta), ("point", Type), ("x", Property), ("?TIMEOUT", Meta)]);
}

#[test]
fn lookups() {
    let cases = [
        ("hs", "haskell"),
        ("Main.hs", "haskell"),
        ("a.mli", "ocaml"),
        ("F#", "fsharp"),
        ("x.fsx", "fsharp"),
        ("b.cljs", "clojure"),
        ("deps.edn", "clojure"),
        ("elisp", "emacslisp"),
        ("init.el", "emacslisp"),
        ("x.rkt", "racket"),
        ("x.scm", "scheme"),
        ("x.lisp", "commonlisp"),
        ("x.hrl", "erlang"),
        ("x.pro", "prolog"),
        ("x.purs", "purescript"),
        ("x.re", "reason"),
        ("x.kk", "koka"),
        ("x.gr", "grain"),
    ];
    for (name, id) in cases {
        let lang = Language::find(name).or_else(|| Language::from_path(name));
        assert_eq!(lang.map(|l| l.id()), Some(id), "{name}");
    }
    assert!(Language::from_path("proof.v").is_none_or(|l| l.id() != "coq"));
    assert!(Language::from_path("script.pl").is_none_or(|l| l.id() != "prolog"));
}
