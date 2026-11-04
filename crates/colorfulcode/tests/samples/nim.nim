## Inventory module with a few helpers.
import std/[strutils, tables]

#[ Multi line
   #[ nested ]# comment ]#

type
  ItemKind* = enum
    ikTool, ikFood
  Item* = object
    name*: string
    price: float
    kind: ItemKind

const MaxItems = 64'u8

proc newItem*(name: string, price = 0.0): Item {.inline.} =
  result = Item(name: name, price: price, kind: ikTool)

func total(items: seq[Item]): float =
  for it in items:
    result += it.price

iterator expensive(items: openArray[Item]): Item =
  for it in items:
    if it.price > 100.0 and it.kind != ikFood:
      yield it

var stock = initTable[string, int]()
stock["hammer"] = 3
let items = @[newItem("hammer", 12.5), newItem("saw")]
echo "Total: ", total(items), "\t", 'x'
let path = r"C:\temp\new"
let doc = """Raw "multi"
line"""
when isMainModule:
  doAssert items.len == 2
  echo fmt"{items[0].name} costs {items[0].price:.2f}"
