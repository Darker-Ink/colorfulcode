// The browser entry: the most used languages in a size optimized module that
// is loaded as a separate `colorfulcode.wasm` file. Call `init()` once and
// wait for it; everything is synchronous after that.

import { createColorfulcode } from "./core.js";

const api = createColorfulcode({
  defaultSource: () => new URL("./colorfulcode.wasm", import.meta.url),
});

export const {
  init,
  isReady,
  subscribe,
  highlight,
  tokenize,
  tokenizeLines,
  tokenizeRanges,
  listLanguages,
  listThemes,
  getLanguage,
  getTheme,
  defineTheme,
  registerAlias,
  detectLanguage,
  themeCss,
  highlightAll,
  highlightElement,
} = api;
