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

// Node's Buffer decodes UTF-8 several times faster than its TextDecoder.
const NodeBuffer = typeof Buffer === "function" && typeof Buffer.from === "function" ? Buffer : undefined;

function readOutput(length) {
  const w = instance();
  const ptr = w.cc_output_ptr();
  if (NodeBuffer) return NodeBuffer.from(w.memory.buffer, ptr, length).toString("utf8");
  return decoder.decode(new Uint8Array(w.memory.buffer, ptr, length));
}

function meta() {
  if (!metadata) {
    metadata = JSON.parse(readOutput(instance().cc_metadata()));
    metadata.themeIndex = new Map(metadata.themes.map((t, i) => [t.name, i]));
  }
  return metadata;
}

/** Detection only reads the first few KB, so there is no point copying more. */
const DETECT_CHARS = 4096;

/** Guesses a language index from the code alone, or -1. */
function guessLanguage(code) {
  return instance().cc_language_detect(writeInput(code.slice(0, DETECT_CHARS)));
}

/** The index of a language name, or of the guess for `code` when the name is "auto". */
function languageFor(name, code) {
  return name === "auto" ? guessLanguage(code) : resolveLanguage(name);
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

const definedThemes = new WeakMap();

/** The name of a theme given by name or as an object (registered on first use). */
function themeName(theme) {
  if (typeof theme !== "object") return theme;
  let name = definedThemes.get(theme);
  if (name === undefined) {
    name = defineTheme(theme);
    definedThemes.set(theme, name);
  }
  return name;
}

function resolveTheme(theme) {
  if (theme === false || theme === null) return -1;
  theme = themeName(theme);
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
 * with `themeCss()` or `colorfulcode/css/<name>.css`.
 *
 * Pass `"auto"` as the language to guess it from the code first.
 */
export function highlight(code, language, options = {}) {
  const w = instance();
  const { theme, pre = true, lines = false, lineNumbers = false, highlightLines } = options;
  const themeIndex = resolveTheme(theme);
  const languageIndex = languageFor(language, code);
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
 * tokens cover the whole input. The language can be `"auto"`, as in `highlight()`.
 */
export function tokenize(code, language) {
  const w = instance();
  // Resolve names first: lookups reuse the input and output buffers.
  const kinds = meta().kinds;
  const languageIndex = languageFor(language, code);
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

/**
 * Registers a custom theme and returns its name, which then works anywhere a
 * built in theme name does. Kinds you leave out inherit a sensible style, so
 * a handful of colors is enough:
 *
 *   defineTheme({
 *     name: "discord",
 *     background: "#2b2d31",
 *     foreground: "#dbdee1",
 *     styles: { keyword: "#ff7ab2", comment: { color: "#80848e", italic: true } },
 *   });
 */
export function defineTheme(theme) {
  const { name, dark, background, foreground, styles = {} } = theme;
  if (!name || !background || !foreground) {
    throw new Error("colorfulcode: a theme needs a name, a background and a foreground");
  }
  const lines = [name, (dark ?? isDark(background)) ? "dark" : "light", `${background} ${foreground}`];
  for (const [kind, value] of Object.entries(styles)) {
    const style = typeof value === "string" ? { color: value } : value;
    const flags = (style.bold ? "b" : "") + (style.italic ? "i" : "") + (style.underline ? "u" : "");
    lines.push(`${kind} ${style.color} ${flags}`);
  }
  if (instance().cc_theme_define(writeInput(lines.join("\n"))) < 0) {
    throw new Error(`colorfulcode: invalid theme "${name}" (colors must be #rrggbb)`);
  }
  metadata = undefined; // Reload so listThemes() and getTheme() include it.
  return name;
}

function isDark(hex) {
  const n = parseInt(hex.replace("#", ""), 16);
  const luma = 0.299 * (n >> 16) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255);
  return luma < 128;
}

/** Finds a theme by name, or returns the colors of a theme object. */
export function getTheme(name) {
  if (name && typeof name === "object") name = themeName(name);
  const index = meta().themeIndex.get(name);
  return index === undefined ? undefined : meta().themes[index];
}

/**
 * Guesses a language id from a file path, then from a shebang or doctype in
 * the code. With `guess: true` the code itself is scored against every
 * language too. Returns undefined when unsure.
 */
export function detectLanguage({ path, code, guess = false } = {}) {
  const w = instance();
  let index = -1;
  if (path) index = w.cc_language_from_path(writeInput(path));
  if (index < 0 && code) {
    index = guess ? guessLanguage(code) : w.cc_language_from_content(writeInput(code.slice(0, 256)));
  }
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
