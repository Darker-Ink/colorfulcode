(* A small expression evaluator.
   (* Comments nest in OCaml. *) *)
open Printf

type 'a tree =
  | Leaf
  | Node of 'a tree * 'a * 'a tree
[@@deriving show]

type expr =
  | Num of float
  | Add of expr * expr
  | Var of string

exception Unbound of string

let rec eval env = function
  | Num n -> n
  | Add (a, b) -> eval env a +. eval env b
  | Var name ->
    (try List.assoc name env with Not_found -> raise (Unbound name))

let max_depth = 64
let greeting = "hello\tworld\n"
let newline = '\n'
and quote = '\''

let rec insert ~cmp x = function
  | Leaf -> Node (Leaf, x, Leaf)
  | Node (l, v, r) as t ->
    if cmp x v < 0 then Node (insert ~cmp x l, v, r)
    else if cmp x v > 0 then Node (l, v, insert ~cmp x r)
    else t

let color_name = function
  | `Red -> "red"
  | `Green -> "green"
  | `Rgb (r, g, b) -> sprintf "#%02x%02x%02x" r g b

let html = {|<p class="x">raw \n text</p>|}

module StringMap = Map.Make (String)

let () =
  let env = [ ("x", 1.5); ("y", 0x10 |> float_of_int) ] in
  let result = eval env (Add (Var "x", Num 2.5e1)) in
  printf "result = %f, depth = %d\n" result max_depth;
  print_endline (color_name `Red ^ html);
  ignore (insert ~cmp:compare 3 Leaf : int tree)
