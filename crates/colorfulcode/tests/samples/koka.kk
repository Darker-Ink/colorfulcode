// Effect handlers in Koka.
module main

import std/num/float64

/* Block comments /* nest */ in Koka. */

type shape
  Circle(radius : float64)
  Rect(width : float64, height : float64)

struct person
  name : string
  age : int = 0

effect ask
  ctl ask-number() : int

effect state<s>
  fun get() : s
  fun set(x : s) : ()

fun area( s : shape ) : float64
  match s
    Circle(r) -> 3.14159 * r * r
    Rect(w, h) -> w * h

fun sum-asked( n : int ) : ask int
  if n <= 0 then 0 else ask-number() + sum-asked(n - 1)

fun counter() : <state<int>, console> ()
  val x = get()
  println("count: " ++ x.show ++ "\n")
  set(x + 1)

fun is-adult( p : person ) : bool
  p.age >= 18

fun main()
  val total = with handler
    ctl ask-number() resume(0x10)
  sum-asked(3)
  println(total.show)
  val shapes = [Circle(2.0), Rect(3.0, 1.5e1)]
  shapes.map(area).foldl(0.0, (+)).show.println
  var i := 0
  while { i < 3 }
    i := i + 1
  println(is-adult(Person("Ada", 36)) && True)
  println('k')
