import wasmBase64 from "./wasm.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

let wasm;
let metadata;
const languageCache = new Map();

function decodeBase64(text) {
  if (typeof Buffer === "function") return Buffer.from(text, "base64");
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** Instantiates the module on first use. Synchronous, so it works during SSR too. */
function instance() {
  if (!wasm) {
    const module = new WebAssembly.Module(decodeBase64(wasmBase64));
    wasm = new WebAssembly.Instance(module, {}).exports;
  }
  return wasm;
}

/** Writes a string into the input buffer and returns its length in bytes. */
function writeInput(text) {
  const w = instance();
  const capacity = text.length * 3;
  const ptr = w.cc_input(capacity);
  return encoder.encodeInto(text, new Uint8Array(w.memory.buffer, ptr, capacity)).written;
}

function readOutput(length) {
  const w = instance();
  return decoder.decode(new Uint8Array(w.memory.buffer, w.cc_output_ptr(), length));
}

function meta() {
  if (!metadata) {
    metadata = JSON.parse(readOutput(instance().cc_metadata()));
    metadata.themeIndex = new Map(metadata.themes.map((t, i) => [t.name, i]));
  }
  return metadata;
}

function resolveLanguage(name) {
  if (typeof name !== "string" || name === "") return -1;
  let index = languageCache.get(name);
  if (index === undefined) {
    index = instance().cc_language_find(writeInput(name));
    languageCache.set(name, index);
  }
  return index;
}

function resolveTheme(theme) {
  if (theme === false || theme === null) return -1;
  const index = meta().themeIndex.get(theme ?? "colorful");
  if (index === undefined) throw new Error(`colorfulcode: unknown theme "${theme}"`);
  return index;
}

const WRAP = 1;
const LINES = 2;
const LINE_NUMBERS = 4;

/**
 * Highlights code and returns HTML.
 *
 * With a theme (the default is "colorful") the HTML uses inline styles and
 * needs no CSS. Pass `theme: false` to get class names instead and style them
 * with `themeCss()` or `colorfulcode/themes/<name>.css`.
 */
export function highlight(code, language, options = {}) {
  const w = instance();
  const { theme, pre = true, lines = false, lineNumbers = false, highlightLines } = options;
  const themeIndex = resolveTheme(theme);
  const languageIndex = resolveLanguage(language);
  let lineCount = 0;
  if (highlightLines && highlightLines.length) {
    lineCount = highlightLines.length;
    new Uint32Array(w.memory.buffer, w.cc_lines(lineCount), lineCount).set(highlightLines);
  }
  const flags = (pre ? WRAP : 0) | (lines ? LINES : 0) | (lineNumbers ? LINE_NUMBERS : 0);
  const length = writeInput(code);
  return readOutput(w.cc_highlight(length, languageIndex, themeIndex, flags, lineCount));
}

/**
 * Tokenizes code. Every token has `start` and `end` offsets into the string
 * (UTF-16, like `String.prototype.slice`), its `kind` and its `text`. The
 * tokens cover the whole input.
 */
export function tokenize(code, language) {
  const w = instance();
  // Resolve names first: lookups reuse the input and output buffers.
  const kinds = meta().kinds;
  const languageIndex = resolveLanguage(language);
  const count = w.cc_tokenize(writeInput(code), languageIndex);
  const raw = new Uint32Array(w.memory.buffer, w.cc_output_ptr(), count * 3);
  const tokens = new Array(count);
  for (let i = 0, j = 0; i < count; i++, j += 3) {
    const start = raw[j];
    const end = raw[j + 1];
    tokens[i] = { start, end, kind: kinds[raw[j + 2]], text: code.slice(start, end) };
  }
  return tokens;
}

/** Like `tokenize`, but split into lines of `{ text, kind }` without line breaks. */
export function tokenizeLines(code, language) {
  const lines = [[]];
  for (const { text, kind } of tokenize(code, language)) {
    const parts = text.split("\n");
    for (let i = 0; i < parts.length; i++) {
      if (i > 0) lines.push([]);
      const part = parts[i].endsWith("\r") ? parts[i].slice(0, -1) : parts[i];
      if (part) lines[lines.length - 1].push({ text: part, kind });
    }
  }
  return lines;
}

/** Every supported language. */
export function listLanguages() {
  return meta().languages;
}

/** Every built in theme with its colors. */
export function listThemes() {
  return meta().themes;
}

/** Finds a language by id, alias, name or extension. */
export function getLanguage(name) {
  const index = resolveLanguage(name);
  return index < 0 ? undefined : meta().languages[index];
}

/** Finds a theme by name. */
export function getTheme(name) {
  const index = meta().themeIndex.get(name);
  return index === undefined ? undefined : meta().themes[index];
}

/** Guesses a language id from a file path and/or the code (shebangs). */
export function detectLanguage({ path, code } = {}) {
  const w = instance();
  let index = -1;
  if (path) index = w.cc_language_from_path(writeInput(path));
  if (index < 0 && code) index = w.cc_language_from_content(writeInput(code.slice(0, 256)));
  return index < 0 ? undefined : meta().languages[index].id;
}

/** CSS for the class based output (`theme: false`), scoped to `selector`. */
export function themeCss(theme = "colorful", selector = ".cc") {
  const themeIndex = resolveTheme(theme);
  return readOutput(instance().cc_theme_css(themeIndex, writeInput(selector)));
}

/**
 * Highlights every `<code class="language-xyz">` inside `root` in place.
 * Handy for static pages and Markdown output.
 */
export function highlightAll(options = {}, root = globalThis.document) {
  for (const el of root.querySelectorAll('code[class*="language-"], code[class*="lang-"]')) {
    highlightElement(el, options);
  }
}

/** Highlights a single `<code>` element in place using its `language-xyz` class. */
export function highlightElement(el, options = {}) {
  const match = /(?:^|\s)lang(?:uage)?-([\w+#.-]+)/.exec(el.className);
  const language = options.language ?? (match ? match[1] : "text");
  el.innerHTML = highlight(el.textContent ?? "", language, { ...options, pre: false });
  const theme = options.theme === false ? undefined : getTheme(options.theme ?? "colorful");
  const target = el.parentElement && el.parentElement.tagName === "PRE" ? el.parentElement : el;
  target.classList.add("cc");
  if (theme) {
    target.style.background = theme.background;
    target.style.color = theme.foreground;
  }
}
