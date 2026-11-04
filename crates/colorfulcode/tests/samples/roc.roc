app [main!] { pf: platform "https://github.com/roc-lang/basic-cli/releases/download/0.19.0/cli.tar.br" }

import pf.Stdout
import pf.Arg exposing [Arg]

## A shape with a tag union.
Shape : [Circle F64, Rect F64 F64]

Inventory : {
    name : Str,
    items : List Str,
}

# Computes the area of a shape.
area : Shape -> F64
area = |shape|
    when shape is
        Circle(r) -> 3.14159 * r * r
        Rect(w, h) -> w * h

greet = \name -> "Hello, ${name}!\n"

total_area : List Shape -> F64
total_area = |shapes|
    List.walk(shapes, 0, |sum, s| sum + area(s))

describe : Inventory -> Str
describe = |inv|
    count = List.len(inv.items) |> Num.to_str
    """
    ${inv.name} has ${count} items
    """

main! : List Arg => Result {} _
main! = |_args|
    shapes = [Circle(2.0), Rect(3, 0x10)]
    Stdout.line!(greet("Roc"))?
    Stdout.line!(Num.to_str(total_area(shapes)))?
    Stdout.line!(describe({ name: "shelf", items: ["a", "b"] }))?
    initial = 'R'
    expect initial == 'R'
    if total_area(shapes) > 10.5 then
        Stdout.line!("big")
    else
        Ok({})
