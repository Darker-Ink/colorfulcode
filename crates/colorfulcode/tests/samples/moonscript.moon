-- A small inventory class
import insert, concat from table

MAX_ITEMS = 50

class Inventory
  new: (@owner, @items = {}) =>
    @@instances = (@@instances or 0) + 1

  add: (item) =>
    if #@items >= MAX_ITEMS
      error "inventory full for #{@owner}\n"
    insert @items, item
    @

  names: =>
    [item.name for item in *@items when item.qty > 0]

  describe: =>
    "#{@owner} has #{#@items} items: #{concat @names!, ', '}"

class Shop extends Inventory
  sell: (name) =>
    for i, item in ipairs @items
      if item.name == name
        table.remove @items, i
        return true
    false

make_item = (name, qty = 1) -> { :name, :qty, price: 9.99 }

shop = Shop "Ada"
shop\add make_item "lamp", 2
doc = [[
long string]]
print shop\describe!, 'done', 0xFF
