# colorfulcode

Fast syntax highlighting for 200 languages, written in Rust and running as WebAssembly. Synchronous, no setup, works in React, Node, Deno, Bun and every bundler.

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

`colorfulcode/common` (and `colorfulcode/common/react`) is the same API with the 55 most used languages in a smaller module.

Full documentation, themes, custom themes and benchmarks: https://github.com/Darker-Ink/colorfulcode
