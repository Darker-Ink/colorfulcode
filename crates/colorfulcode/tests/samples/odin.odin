package inventory

import "core:fmt"
import "core:strings"

// Items stored in a fixed size bag.
MAX_ITEMS :: 32

Rarity :: enum u8 {
	Common,
	Rare,
	Legendary,
}

Item :: struct {
	name:   string,
	weight: f32,
	rarity: Rarity,
}

Bag :: struct {
	items: [dynamic]Item,
	limit: int,
}

/* Adds an item, returning false when the bag is full. */
add_item :: proc(bag: ^Bag, item: Item) -> bool {
	if len(bag.items) >= bag.limit {
		return false
	}
	append(&bag.items, item)
	return true
}

total_weight :: proc(bag: Bag) -> (total: f32) {
	for item in bag.items {
		total += item.weight
	}
	return
}

@(test)
test_add :: proc(t: ^testing.T) {
	bag := Bag{limit = 1}
	defer delete(bag.items)
	assert(add_item(&bag, Item{"sword", 3.5, .Rare}))
}

main :: proc() {
	bag := Bag{limit = MAX_ITEMS}
	add_item(&bag, Item{name = "potion", weight = 0.25, rarity = .Common})
	#partial switch bag.items[0].rarity {
	case .Rare: fmt.println("rare!")
	}
	path := `C:\games\bag.txt`
	fmt.printf("weight: %.2f, first: %c, hex: %d\n", total_weight(bag), 'p', 0xFF)
	_ = strings.to_upper(path)
}
