import type { HTMLAttributes, ReactElement } from "react";
import type { HighlightOptions, LanguageName, LineToken } from "./index.js";

export * from "./index.js";

export interface CodeProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  /** The source code. A string child works too. */
  code?: string;
  children?: string;
  /** Language id, alias or extension, or "auto" to guess it. Defaults to plain text. */
  language?: LanguageName;
  /** Theme for inline styles, or `false` for class names. Defaults to "colorful". */
  theme?: HighlightOptions["theme"];
  lineNumbers?: boolean;
  /** One based line numbers to highlight. */
  highlightLines?: number[];
  /** Render a `<code>` element for inline snippets instead of a `<pre>` block. */
  inline?: boolean;
}

export function Code(props: CodeProps): ReactElement;
export default Code;

/** Memoized `highlight()` without the `<pre>` wrapper. */
export function useHighlight(
  code: string,
  language: LanguageName,
  options?: Omit<HighlightOptions, "pre">,
): string;

/** Memoized `tokenizeLines()`, for custom rendering. */
export function useTokens(code: string, language: LanguageName): LineToken[][];
