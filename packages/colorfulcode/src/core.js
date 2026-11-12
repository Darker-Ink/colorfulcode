// The JavaScript API around one colorfulcode WebAssembly module. Every entry
// point (`colorfulcode`, `colorfulcode/common`, `colorfulcode/web`) calls
// `createColorfulcode` with its own module, so each keeps its own state.

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// Node's Buffer decodes UTF-8 several times faster than its TextDecoder.
const NodeBuffer = typeof Buffer === "function" && typeof Buffer.from === "function" ? Buffer : undefined;

/** Decodes the base64 module embedded by the synchronous entry points. */
export function decodeBase64(text) {
  if (NodeBuffer) return NodeBuffer.from(text, "base64");
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** Compiles a module from a URL, a fetch Response, bytes or an existing module. */
async function compile(source) {
  if (source instanceof WebAssembly.Module) return source;
  if (typeof source === "string" || source instanceof URL) source = fetch(source);
  source = await source;
  if (typeof Response === "function" && source instanceof Response) {
    if (!source.ok) throw new Error(`colorfulcode: could not load the module (${source.status} ${source.url})`);
    // Streaming compiles while downloading, but needs the application/wasm type.
    if (WebAssembly.compileStreaming && source.headers.get("content-type")?.startsWith("application/wasm")) {
      return WebAssembly.compileStreaming(source);
    }
    return WebAssembly.compile(await source.arrayBuffer());
  }
  return WebAssembly.compile(source);
}

/**
 * Creates the API. `loadSync` instantiates the module synchronously on first
 * use (the base64 entries); without it, `init()` must finish first and
 * `defaultSource()` says where the module lives.
 */
export function createColorfulcode({ loadSync, defaultSource }) {
  let wasm;
  let loading;
  let metadata;
  const languageCache = new Map();
  const aliases = new Map();
  const listeners = new Set();
  const definedThemes = new WeakMap();

  function ready(exports) {
    wasm = exports;
    for (const listener of listeners) listener();
    listeners.clear();
  }

  function instance() {
    if (!wasm) {
      if (!loadSync) {
        throw new Error("colorfulcode: the module is not loaded yet. Call init() and wait for it, or check isReady() first.");
      }
      ready(loadSync());
    }
    return wasm;
  }

  /**
   * Loads the module. Only needed for `colorfulcode/web`; elsewhere it
   * resolves at once. Pass a URL, a fetch Response, bytes or a compiled
   * module to load it from somewhere other than next to this file. Calling
   * it again returns the same promise.
   */
  function init(source) {
    if (wasm || loadSync) {
      instance();
      return Promise.resolve();
    }
    loading ??= compile(source ?? defaultSource())
      .then((module) => WebAssembly.instantiate(module, {}))
      .then((inst) => ready(inst.exports))
      .catch((error) => {
        loading = undefined;
        throw error;
      });
    return loading;
  }

  /** Whether highlighting works right now without waiting for `init()`. */
  function isReady() {
    return Boolean(wasm || loadSync);
  }

  /** Calls `listener` once the module has loaded. Returns an unsubscribe function. */
  function subscribe(listener) {
    if (isReady()) return () => {};
    listeners.add(listener);
    return () => listeners.delete(listener);
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
      const target = aliases.get(name.toLowerCase()) ?? name;
      index = instance().cc_language_find(writeInput(target));
      languageCache.set(name, index);
    }
    return index;
  }

  /**
   * Makes `alias` mean another language, for names this library does not
   * know or that should map differently: `registerAlias("curl", "bash")`.
   * Aliases are case insensitive and win over built in names.
   */
  function registerAlias(alias, language) {
    aliases.set(alias.toLowerCase(), language);
    languageCache.clear();
  }

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
  function highlight(code, language, options = {}) {
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
   * Tokens as a flat `Uint32Array` of `start, end, kind` triples, with offsets
   * in UTF-16 code units and `kind` an index into `kinds`. The cheapest way to
   * get tokens: no objects and no strings are created per token.
   */
  function tokenizeRanges(code, language) {
    const w = instance();
    // Resolve names first: lookups reuse the input and output buffers.
    const kinds = meta().kinds;
    const languageIndex = languageFor(language, code);
    const count = w.cc_tokenize(writeInput(code), languageIndex);
    const ranges = new Uint32Array(w.memory.buffer, w.cc_output_ptr(), count * 3).slice();
    return { ranges, kinds };
  }

  /**
   * Tokenizes code. Every token has `start` and `end` offsets into the string
   * (UTF-16, like `String.prototype.slice`) and its `kind`, plus its `text`
   * unless `{ text: false }` is passed. The tokens cover the whole input.
   */
  function tokenize(code, language, options = {}) {
    const { ranges, kinds } = tokenizeRanges(code, language);
    const withText = options.text !== false;
    const tokens = new Array(ranges.length / 3);
    for (let i = 0, j = 0; j < ranges.length; i++, j += 3) {
      const start = ranges[j];
      const end = ranges[j + 1];
      const kind = kinds[ranges[j + 2]];
      tokens[i] = withText ? { start, end, kind, text: code.slice(start, end) } : { start, end, kind };
    }
    return tokens;
  }

  /**
   * Like `tokenize`, split into lines without the line breaks. Each line is an
   * array of `{ text, kind, start, end }` with offsets relative to the start
   * of the line.
   */
  function tokenizeLines(code, language) {
    const { ranges, kinds } = tokenizeRanges(code, language);
    const lines = [[]];
    let lineStart = 0;
    for (let j = 0; j < ranges.length; j += 3) {
      const kind = kinds[ranges[j + 2]];
      let start = ranges[j];
      const end = ranges[j + 1];
      while (start < end) {
        const newline = code.indexOf("\n", start);
        const breaks = newline !== -1 && newline < end;
        const pieceEnd = breaks ? newline : end;
        // A `\r` before the line break belongs to neither line.
        const textEnd = breaks && code[pieceEnd - 1] === "\r" ? pieceEnd - 1 : pieceEnd;
        if (textEnd > start) {
          lines[lines.length - 1].push({
            text: code.slice(start, textEnd),
            kind,
            start: start - lineStart,
            end: textEnd - lineStart,
          });
        }
        if (breaks) {
          lines.push([]);
          lineStart = newline + 1;
          start = newline + 1;
        } else {
          start = pieceEnd;
        }
      }
    }
    return lines;
  }

  /** Every supported language. */
  function listLanguages() {
    return meta().languages;
  }

  /** Every theme with its colors: the built in ones and any you defined. */
  function listThemes() {
    return meta().themes;
  }

  /** Finds a language by id, alias, name or extension. */
  function getLanguage(name) {
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
  function defineTheme(theme) {
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

  /** Finds a theme by name, or returns the colors of a theme object. */
  function getTheme(name) {
    if (name && typeof name === "object") name = themeName(name);
    const index = meta().themeIndex.get(name);
    return index === undefined ? undefined : meta().themes[index];
  }

  /**
   * Guesses a language id from a file path, then from a shebang or doctype in
   * the code. With `guess: true` the code itself is scored against every
   * language too. Returns undefined when unsure.
   */
  function detectLanguage({ path, code, guess = false } = {}) {
    const w = instance();
    let index = -1;
    if (path) index = w.cc_language_from_path(writeInput(path));
    if (index < 0 && code) {
      index = guess ? guessLanguage(code) : w.cc_language_from_content(writeInput(code.slice(0, 256)));
    }
    return index < 0 ? undefined : meta().languages[index].id;
  }

  /** CSS for the class based output (`theme: false`), scoped to `selector`. */
  function themeCss(theme = "colorful", selector = ".cc") {
    const themeIndex = resolveTheme(theme);
    return readOutput(instance().cc_theme_css(themeIndex, writeInput(selector)));
  }

  /**
   * Highlights every `<code class="language-xyz">` inside `root` in place.
   * Handy for static pages and Markdown output.
   */
  function highlightAll(options = {}, root = globalThis.document) {
    for (const el of root.querySelectorAll('code[class*="language-"], code[class*="lang-"]')) {
      highlightElement(el, options);
    }
  }

  /** Highlights a single `<code>` element in place using its `language-xyz` class. */
  function highlightElement(el, options = {}) {
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

  return {
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
  };
}

function isDark(hex) {
  const n = parseInt(hex.replace("#", ""), 16);
  const luma = 0.299 * (n >> 16) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255);
  return luma < 128;
}

/** Escapes text for HTML, for the plain text shown before a module loads. */
export function escapeHtml(text) {
  return text.replace(/[&<>"']/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot", "'": "#39" }[c]};`);
}
