//// A small inventory module.

import gleam/int
import gleam/io
import gleam/list
import gleam/result
import gleam/string

/// An item in stock.
pub type Item {
  Item(name: String, price: Float, quantity: Int)
}

pub type Error {
  NotFound(name: String)
  OutOfStock
}

pub opaque type Inventory {
  Inventory(items: List(Item))
}

const tax_rate = 0.2

pub fn new() -> Inventory {
  Inventory(items: [])
}

pub fn add(inventory: Inventory, item: Item) -> Inventory {
  Inventory(items: [item, ..inventory.items])
}

pub fn find(inventory: Inventory, name: String) -> Result(Item, Error) {
  inventory.items
  |> list.find(fn(item) { item.name == name })
  |> result.replace_error(NotFound(name))
}

fn describe(item: Item) -> String {
  case item.quantity {
    0 -> item.name <> ": out of stock"
    n if n > 100 -> item.name <> ": plenty\n"
    n -> item.name <> ": " <> int.to_string(n) <> " left"
  }
}

@external(erlang, "erlang", "system_time")
fn system_time() -> Int

pub fn main() {
  let inventory =
    new()
    |> add(Item("widget", 2.5, 10))
    |> add(Item(name: "gadget", price: 1.0e2, quantity: 0xFF))
  let assert Ok(widget) = find(inventory, "widget")
  io.println(describe(widget))
  io.debug(#(True, Nil, tax_rate, string.length("abc"), system_time()))
}
