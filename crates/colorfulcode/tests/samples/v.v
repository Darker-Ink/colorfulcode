module main

import os
import strings

// A simple word counter.
const max_words = 1_000

@[heap]
struct Counter {
mut:
	counts map[string]int
	total  int
pub:
	name string = 'default'
}

enum Mode {
	words
	lines
}

fn new_counter(name string) &Counter {
	return &Counter{
		name: name
	}
}

fn (mut c Counter) add(word string) ! {
	if c.total >= max_words {
		return error('too many words: ${c.total}')
	}
	c.counts[word.to_lower()]++
	c.total++
}

/* Returns the most frequent word. */
fn (c &Counter) top() ?string {
	mut best := ''
	for word, n in c.counts {
		if n > c.counts[best] or { 0 } {
			best = word
		}
	}
	return if best == '' { none } else { best }
}

fn main() {
	mut c := new_counter('demo')
	text := os.read_file('input.txt') or { 'hello world\thello' }
	for w in text.split_any(' \t\n') {
		c.add(w) or { panic(err) }
	}
	$if debug {
		println('total: ${c.total}, ratio: ${0.5e1}, raw: ${r'\n'}')
	}
	println(c.top() or { 'none' })
	_ = strings.new_builder(0x10)
	_ = `x`
}
