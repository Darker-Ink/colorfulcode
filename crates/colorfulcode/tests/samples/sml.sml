(* Binary search trees in Standard ML.
   (* Comments nest. *) *)
signature ORDERED =
sig
  type t
  val compare : t * t -> order
end

structure IntOrder : ORDERED =
struct
  type t = int
  val compare = Int.compare
end

datatype 'a tree = Leaf | Node of 'a tree * 'a * 'a tree

exception Empty

fun insert cmp (x, Leaf) = Node (Leaf, x, Leaf)
  | insert cmp (x, t as Node (l, v, r)) =
      case cmp (x, v) of
          LESS => Node (insert cmp (x, l), v, r)
        | GREATER => Node (l, v, insert cmp (x, r))
        | EQUAL => t

fun toList Leaf = nil
  | toList (Node (l, v, r)) = toList l @ [v] @ toList r

fun smallest Leaf = raise Empty
  | smallest (Node (Leaf, v, _)) = v
  | smallest (Node (l, _, _)) = smallest l

val point = {x = 1.5, y = ~2.0}
val xCoord = #x point
val tab = #"\t"
val letter = #"a"

val tree = foldl (fn (x, t) => insert Int.compare (x, t)) Leaf [5, 3, 8, 0x1F]

val () =
  let
    val items = toList tree
  in
    print ("items: " ^ String.concatWith ", " (map Int.toString items) ^ "\n");
    if length items > 2 andalso not (null items) then print "many\n" else ()
  end
  handle Empty => print "empty tree\n"
