# colorfulcode

Fast syntax highlighting for 200 languages, written in Rust and running as WebAssembly. Works in React, Node, Deno, Bun and every bundler.

```bash
npm install colorfulcode
```

```tsx
import { Code } from "colorfulcode/react";

<Code code={source} language="tsx" theme="tokyo-night" lineNumbers />;
```

```js
import { highlight, tokenize } from "colorfulcode";

highlight("const answer = 42;", "js", { theme: "dracula" }); // HTML with inline styles
tokenize("const answer = 42;", "js"); // [{ start, end, kind, text }, ...]
```

| Entry | Languages | Download (gzipped) | Loading |
| --- | --- | --- | --- |
| `colorfulcode` | all 200 | 467 KiB | embedded, synchronous |
| `colorfulcode/common` | 56 most used | 230 KiB | embedded, synchronous |
| `colorfulcode/web` | 56 most used | 156 KiB | separate `.wasm`, call `await init()` first |

Each has a React entry next to it (`/react`, `/common/react`, `/web/react`). Optional themes live in `colorfulcode/themes`.

Full documentation, themes, custom themes and benchmarks: https://github.com/Darker-Ink/colorfulcode
