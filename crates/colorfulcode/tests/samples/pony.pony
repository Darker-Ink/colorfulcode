use "collections"

/* Counter actor
   /* nested */ */
actor Counter
  var _count: U64 = 0
  let _env: Env

  new create(env: Env) =>
    _env = env

  be increment(by: U64 = 1) =>
    _count = _count + by

  be report() =>
    _env.out.print("Count: " + _count.string() + "\n")

primitive Math
  fun square(x: I64): I64 => x * x

class ref Stack[A: Any val]
  """
  A simple LIFO stack.
  """
  let _items: Array[A] = Array[A]

  fun ref push(item: A) => _items.push(item)

  fun ref pop(): A ? => _items.pop()?

actor Main
  new create(env: Env) =>
    let counter = Counter(env)
    for i in Range(0, 10) do
      counter.increment(i.u64())
    end
    counter.report()
    let c: U8 = 'a'
    if Math.square(3) > 0x10 then env.out.print("big") else None end
    try
      let s = Stack[String]
      s.push("x")
      env.out.print(s.pop()?)
    end
