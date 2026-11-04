/** The category of a highlighted token. */
export type Kind =
  | "text" | "comment" | "keyword" | "string" | "escape" | "number" | "constant" | "function" | "type"
  | "variable" | "property" | "builtin" | "operator" | "punctuation" | "tag" | "attribute" | "meta"
  | "regex" | "heading" | "emphasis" | "strong" | "link" | "inserted" | "deleted";

export type ThemeName =
  | "colorful" | "colorful-light" | "github-dark" | "github-light" | "dracula" | "nord" | "one-dark"
  | "monokai" | "catppuccin-mocha" | "catppuccin-latte" | "tokyo-night" | "gruvbox-dark"
  | "solarized-dark" | "solarized-light";

export interface Token {
  /** Start offset in the string (UTF-16 code units, like `String.prototype.slice`). */
  start: number;
  end: number;
  kind: Kind;
  text: string;
}

export interface LineToken {
  text: string;
  kind: Kind;
}

export interface LanguageInfo {
  id: string;
  name: string;
  aliases: string[];
  extensions: string[];
  filenames: string[];
}

export interface Style {
  color: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
}

export interface ThemeInfo {
  name: ThemeName;
  dark: boolean;
  background: string;
  foreground: string;
  styles: Record<Kind, Style>;
}

export interface HighlightOptions {
  /** Theme for inline styles, or `false` for class names. Defaults to "colorful". */
  theme?: ThemeName | false;
  /** Wrap the output in `<pre class="cc"><code>`. Defaults to true. */
  pre?: boolean;
  /** Wrap every line in `<span class="cc-line">`. */
  lines?: boolean;
  /** Prefix every line with its number. */
  lineNumbers?: boolean;
  /** One based line numbers to highlight. */
  highlightLines?: number[];
}

/** Highlights code and returns HTML. Unknown languages render as plain text. */
export function highlight(code: string, language: string, options?: HighlightOptions): string;
/** Tokens covering the whole input. */
export function tokenize(code: string, language: string): Token[];
/** Tokens split into lines, without line breaks. */
export function tokenizeLines(code: string, language: string): LineToken[][];
export function listLanguages(): LanguageInfo[];
export function listThemes(): ThemeInfo[];
export function getLanguage(name: string): LanguageInfo | undefined;
export function getTheme(name: string): ThemeInfo | undefined;
/** Guesses a language id from a file path and/or the code. */
export function detectLanguage(input: { path?: string; code?: string }): string | undefined;
/** CSS for the class based output, scoped to `selector` (default ".cc"). */
export function themeCss(theme?: ThemeName, selector?: string): string;
/** Highlights every `code.language-xyz` element inside `root`. */
export function highlightAll(options?: HighlightOptions, root?: ParentNode): void;
export function highlightElement(element: Element, options?: HighlightOptions & { language?: string }): void;
