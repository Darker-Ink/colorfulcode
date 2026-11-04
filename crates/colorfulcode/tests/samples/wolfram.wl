(* Collatz sequences and a few plots *)
BeginPackage["Collatz`"];

collatz::usage = "collatz[n] gives the next term of the Collatz sequence.";

Begin["`Private`"];

collatz[n_Integer?EvenQ] := n/2
collatz[n_Integer] := 3 n + 1

orbit[n_Integer /; n > 0, max_:1000] := NestWhileList[collatz, n, # != 1 &, 1, max]

stoppingTime[n_] := Module[{len = Length[orbit[n]]},
  If[len > 100, Print["long orbit: ", n]];
  len - 1
]

(* nested (* comments *) are allowed *)
data = Table[{k, stoppingTime[k]}, {k, 1, 500}];
ListPlot[data, PlotStyle -> Red, AxesLabel -> {"n", "steps"}]

squares = Map[#^2 &, Range[10]];
total = Total[squares] // N;
assoc = <|"a" -> 1, "b" -> 2.5*^3|>;
assoc[["a"]] + Lookup[assoc, "b", Missing["NotFound"]]
f = Function[{x, y}, x^y];
rules = {x_ -> x + 1, {a__, b_} :> {b, a}};
$RecursionLimit = 4096;
Solve[x^2 - 2 == 0, x] /. {x -> Sqrt[2]}
If[True && !False, "yes\n", Null]

End[];
EndPackage[];
