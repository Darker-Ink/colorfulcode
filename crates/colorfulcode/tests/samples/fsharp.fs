/// Shapes, pipelines and computation expressions.
module Shapes

open System
#nowarn "40"

(* Block comments (* nest *) too. *)

type Shape =
    | Circle of radius: float
    | Rect of width: float * height: float

[<Measure>] type cm

type Point(x: float, y: float) =
    member this.X = x
    member this.Y = y
    member this.Distance(other: Point) =
        sqrt ((x - other.X) ** 2.0 + (y - other.Y) ** 2.0)
    override _.ToString() = $"({x}, {y})"

let area shape =
    match shape with
    | Circle r -> Math.PI * r * r
    | Rect (w, h) -> w * h

let mutable counter = 0
let inline square x = x * x
let rec fib n = if n < 2 then n else fib (n - 1) + fib (n - 2)

let path = @"C:\temp\""file"".txt"
let json = """{"name": "F#"}"""
let letter = 'a'
let mul = (*) 3 4

let ``total area`` shapes =
    shapes |> List.map area |> List.sum

let fetch (url: string) = async {
    let! body = Async.Sleep 100 |> fun _ -> async { return url.Length }
    return body * 2
}

[<EntryPoint>]
let main argv =
    let shapes = [ Circle 1.0; Rect (2.0, 3.5) ]
    printfn "Total: %.2f (%d items)\n" (``total area`` shapes) shapes.Length
    counter <- counter + 0x10
    0
