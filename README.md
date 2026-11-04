<p align="center">
  <img src="assets/logo.svg" alt="colorfulcode" width="520">
</p>

<p align="center">
  Fast syntax highlighting for __LANGS__ languages, written in Rust.<br>
  Use it from React, plain JavaScript, native apps or the terminal.
</p>

<p align="center">
  <a href="https://github.com/Darker-Ink/colorfulcode/actions/workflows/ci.yml"><img src="https://github.com/Darker-Ink/colorfulcode/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

<p align="center">
  <img src="assets/demo.webp" alt="Live highlighting while typing in the React playground" width="860">
</p>

## Why

- **Fast.** Over 200 MB/s natively and around 45 MB/s from JavaScript including HTML output. That is 10 to 30 times faster than highlight.js and Prism and over 100 times faster than Shiki ([benchmarks](#benchmarks)).
- **Lots of languages.** __LANGS__ languages, including embedded ones: scripts and styles in HTML, fenced code in Markdown, Vue and Svelte components, and TSX with generic arrow functions.
- **Small.** One __WASM__ KiB WebAssembly module (__GZIP__ KiB gzipped) with no dependencies.
- **Zero setup.** Synchronous, so it works in SSR, React Server Components, Node, Deno, Bun and every bundler without configuration.
- **Easy to render anywhere.** The output is a flat list of `(start, end, kind)` tokens plus theme colors, so native UIs can colorize text in a few lines.

## Install

```bash
npm install colorfulcode
```

```bash
cargo add colorfulcode
```

## React

```tsx
import { Code } from "colorfulcode/react";

export function Example({ source }: { source: string }) {
  return <Code code={source} language="tsx" theme="tokyo-night" lineNumbers highlightLines={[3]} />;
}
```

Everything is styled inline, so no CSS is needed. Pass `theme={false}` to get class names instead and bring your own stylesheet.

| Prop | Type | Default | |
| --- | --- | --- | --- |
| `code` | `string` | | The source. A string child works too. |
| `language` | `string` | `"text"` | Id, alias or file extension, for example `"ts"` or `"rs"`. |
| `theme` | `string \| false` | `"colorful"` | A [theme](#themes) for inline styles, or `false` for class names. |
| `lineNumbers` | `boolean` | `false` | Show line numbers. |
| `highlightLines` | `number[]` | | One based lines to highlight. |
| `inline` | `boolean` | `false` | Render a `<code>` for inline snippets instead of a `<pre>` block. |

Two hooks are exported as well: `useHighlight(code, language, options)` returns memoized HTML, and `useTokens(code, language)` returns memoized lines of `{ text, kind }` for rendering with your own components.

## JavaScript

```js
import { highlight, tokenize } from "colorfulcode";

const html = highlight("const answer = 42;", "js", { theme: "dracula" });
// <pre class="cc" style="background:#282a36;color:#f8f8f2"><code><span style="color:#ff79c6">const</span> ...

const tokens = tokenize("const answer = 42;", "js");
// [{ start: 0, end: 5, kind: "keyword", text: "const" }, ...]
```

| Function | Returns |
| --- | --- |
| `highlight(code, language, options?)` | HTML. Options: `theme` (name or `false`), `pre` (wrap in `<pre><code>`, default `true`), `lineNumbers`, `lines`, `highlightLines`. |
| `tokenize(code, language)` | `{ start, end, kind, text }[]` covering the whole input. Offsets are string indices. |
| `tokenizeLines(code, language)` | `{ text, kind }[][]`, one array per line. |
| `highlightAll(options?, root?)` | Highlights every `<code class="language-xyz">` in the page. |
| `highlightElement(element, options?)` | Highlights one element in place. |
| `detectLanguage({ path?, code? })` | A language id from a file name or a shebang. |
| `getLanguage(name)`, `listLanguages()` | Language info: `id`, `name`, `aliases`, `extensions`, `filenames`. |
| `getTheme(name)`, `listThemes()` | Theme info with every color. |
| `themeCss(theme, selector?)` | CSS for class based output. Also shipped as `colorfulcode/themes/<name>.css`. |

Unknown languages fall back to plain text.

## Rust

```rust
use colorfulcode::{highlight, html, Language, Theme};

let rust = Language::find("rust").unwrap();
let highlighted = highlight("fn main() {}", rust);

// Walk the tokens yourself...
for (text, kind) in highlighted.iter() {
    println!("{kind}: {text:?}");
}

// ...or render them.
let theme = Theme::find("nord").unwrap();
let page = highlighted.to_html(&html::Options { theme: Some(theme), wrap: true, ..Default::default() });
let terminal = highlighted.to_ansi(theme, colorfulcode::ansi::ColorMode::TrueColor);
```

| Item | |
| --- | --- |
| `highlight(code, language) -> Highlighted` | Tokenizes the code. `highlight_into` reuses a token buffer. |
| `Highlighted` | `tokens()`, `iter()` of `(text, kind)`, `lines()`, `utf16_tokens()`, `to_html()`, `to_ansi()`. |
| `Token { start, end, kind }` | Byte offsets. Tokens cover the input exactly, with no gaps. |
| `Language` | `find(name)`, `from_path(path)`, `from_content(code)`, `all()`, `id()`, `name()`. |
| `Theme` | `find(name)`, `all()`, `style(kind)`, `background`, `foreground`, `to_css(selector)`. |
| `Kind` | The token category. See [kinds](#token-kinds). |

## Native apps

`crates/colorfulcode-ffi` builds a C library (`cdylib` and `staticlib`) with a header in [`include/colorfulcode.h`](crates/colorfulcode-ffi/include/colorfulcode.h). Swift, Kotlin, C#, C++ and anything else that can call C can use it.

```c
int32_t lang = cc_language_find("python");
int32_t theme = cc_theme_find("github-light");
cc_tokens *tokens = cc_highlight(code, len, lang, CC_UTF16_OFFSETS);

for (size_t i = 0; i < cc_tokens_len(tokens); i++) {
    const cc_token *t = &cc_tokens_data(tokens)[i];
    cc_style style = cc_theme_style(theme, t->kind);  // style.color is 0xRRGGBB
    // apply style to the range [t->start, t->end)
}
cc_tokens_free(tokens);
```

`CC_UTF16_OFFSETS` returns offsets in UTF-16 code units, which is what `NSString`, Java and Kotlin strings and .NET strings use. A full example lives in [`examples/c`](examples/c).

## Terminal

```bash
cargo install --path crates/colorfulcode-cli
colorfulcode src/main.rs
colorfulcode -n --theme dracula README.md
cat script.py | colorfulcode -l python
```

<p align="center">
  <img src="assets/terminal.png" alt="colorfulcode in a terminal" width="760">
</p>

Options: `-l/--language`, `-t/--theme`, `-n/--line-numbers`, `--color auto|always|never`, `--html`, `--list-languages`, `--list-themes`.

## Token kinds

`text`, `comment`, `keyword`, `string`, `escape`, `number`, `constant`, `function`, `type`, `variable`, `property`, `builtin`, `operator`, `punctuation`, `tag`, `attribute`, `meta`, `regex`, `heading`, `emphasis`, `strong`, `link`, `inserted`, `deleted`.

With `theme: false` each token gets the class `cc-<kind>`, for example `cc-keyword`.

## Themes

<p align="center">
  <img src="assets/themes.png" alt="All built in themes" width="860">
</p>

`colorful` (default), `colorful-light`, `github-dark`, `github-light`, `dracula`, `nord`, `one-dark`, `monokai`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night`, `gruvbox-dark`, `solarized-dark`, `solarized-light`.

## Custom themes

A theme is a background, a foreground and colors for the [kinds](#token-kinds) you care about. Anything you leave out inherits something sensible: `tag` and `meta` borrow the `keyword` color, `builtin` borrows `function`, `escape` borrows `constant`, and so on.

For example, a theme in Discord's colors:

```js
import { defineTheme, highlight } from "colorfulcode";

defineTheme({
  name: "discord",
  background: "#2b2d31",
  foreground: "#dbdee1",
  styles: {
    comment: { color: "#80848e", italic: true },
    keyword: "#eb459e",
    string: "#57f287",
    number: "#f0b232",
    function: "#00a8fc",
    type: "#fee75c",
    property: "#c9cdfb",
    operator: "#949cf7",
    punctuation: "#b5bac1",
  },
});

highlight(code, "ts", { theme: "discord" });
```

In React, pass the object straight to `theme` and it is registered on first use:

```tsx
<Code code={source} language="ts" theme={discord} />
```

In Rust, build one with `Theme::new`:

```rust
use colorfulcode::{Color, Kind, Style, Theme};

static DISCORD: Theme = Theme::new("discord", true, Color::hex("#2b2d31"), Color::hex("#dbdee1"), &[
    (Kind::Comment, Style::new(Color::hex("#80848e")).italic()),
    (Kind::Keyword, Style::new(Color::hex("#eb459e"))),
    (Kind::String, Style::new(Color::hex("#57f287"))),
]);
```

Native apps usually skip themes altogether and map each kind to colors from their own design system.

## Smaller build

`colorfulcode` includes every language (__WASM__ KiB, __GZIP__ KiB gzipped). If you only need the popular ones, `colorfulcode/common` has __COMMON_LANGS__ of them (__COMMON_WASM__ KiB, __COMMON_GZIP__ KiB gzipped) with the same API:

```js
import { highlight } from "colorfulcode/common";
import { Code } from "colorfulcode/common/react";
```

It covers JavaScript, TypeScript, JSX, TSX, HTML, CSS, SCSS, Less, Sass, JSON, Markdown, MDX, Vue, Svelte, Astro, Python, Rust, Go, Java, Kotlin, Swift, C, C++, C#, Objective-C, Dart, Scala, Zig, Haskell, Elixir, Ruby, PHP, Perl, Lua, R, Bash, PowerShell, Batch, Dockerfile, Makefile, nginx, SQL, GraphQL, YAML, TOML, INI, dotenv, HCL, Protobuf, XML, SVG, diff and plain text. In Rust the same set is what you get with `default-features = false`.

## Benchmarks

<p align="center">
  <img src="assets/race.webp" alt="colorfulcode, highlight.js, Prism and Shiki highlighting the same 200 KB of TypeScript" width="760">
</p>

The race above is measured in Chromium and replayed in real time (`cd bench && npm run race` runs it in your browser).
In Node 22, HTML output for about 100 KB of code per language (`bench/compare.mjs`):

__BENCH__

The core alone tokenizes at __NATIVE__ MB/s on average across all sample files (`cargo run --release --example bench`).

## Languages

<details>
<summary>All __LANGS__ languages</summary>

__LANGUAGE_LIST__

</details>

## Examples

- [`examples/react`](examples/react): the playground from the video above (`npm run dev -w colorfulcode-react-example`).
- [`examples/vanilla`](examples/vanilla): a static page using `highlightAll`.
- [`examples/node`](examples/node): render a file to a standalone HTML page.
- [`examples/c`](examples/c): highlight a file through the C library.
- [`crates/colorfulcode/examples`](crates/colorfulcode/examples): Rust examples, including drawing tokens the way a native UI would.
- [`crates/colorfulcode-cli`](crates/colorfulcode-cli): the terminal app.

## Development

```bash
cargo test --workspace          # Rust tests, including snapshot tests for every language
npm install && npm run build    # build the WebAssembly module and the npm package
npm test                        # JavaScript and React tests
```

Each language has a sample in `crates/colorfulcode/tests/samples` and a snapshot of its tokens. Run `UPDATE_SNAPSHOTS=1 cargo test -p colorfulcode --test snapshots` after changing a lexer, then review the diff.

## License

MIT
