// Document setup
#set page(paper: "a4", margin: (x: 2cm, y: 1.5cm))
#set text(font: "Linux Libertine", size: 11pt, lang: "en")
#show heading: it => block(above: 1.4em)[#it.body]

#let title = [Notes on *Typst*]
#let total(xs) = xs.fold(0, (a, b) => a + b)

= Introduction <intro>
This is *strong*, _emphasized_ and `raw` text with an escaped \#hash.
See @intro and https://typst.app for details. Inline math: $x^2 + y^2 = r^2$.

== Lists
- First item
- Second item with #title
+ Numbered item
/ Term: A description

#figure(
  rect(width: 50%, fill: rgb("#e0f0ff"))[Content in a box],
  caption: [A *blue* box],
) <fig-box>

#if total((1, 2, 3)) > 5 [
  The sum is large: #total((1, 2, 3)).
] else [
  The sum is small.
]

#for (i, name) in ("Ada", "Grace").enumerate() {
  [#(i + 1). #name \ ]
}

```rust
fn main() { println!("hi"); }
```

/* A block comment
   over two lines */
#import "@preview/cetz:0.2.0": canvas, draw
