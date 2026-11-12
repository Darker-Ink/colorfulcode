// The default entry: every language, with the module embedded so it works
// synchronously everywhere (Node, SSR, workers and every bundler).

import { createColorfulcode, decodeBase64 } from "./core.js";
import wasmBase64 from "./wasm.js";

const api = createColorfulcode({
  loadSync: () => new WebAssembly.Instance(new WebAssembly.Module(decodeBase64(wasmBase64)), {}).exports,
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
