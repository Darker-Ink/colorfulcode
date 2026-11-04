import { createElement, useMemo } from "react";
import { detectLanguage, getLanguage, getTheme, highlight, tokenizeLines } from "./index.js";

export * from "./index.js";

/** Memoized `highlight()`: returns the HTML for `code`. */
export function useHighlight(code, language, options = {}) {
  const { theme, lines, lineNumbers, highlightLines } = options;
  const marked = highlightLines ? highlightLines.join(",") : "";
  return useMemo(
    () => highlight(code, language, { theme, lines, lineNumbers, highlightLines, pre: false }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [code, language, theme, lines, lineNumbers, marked],
  );
}

/** Memoized `tokenizeLines()`, for rendering tokens with your own components. */
export function useTokens(code, language) {
  return useMemo(() => tokenizeLines(code, language), [code, language]);
}

/**
 * Renders highlighted code.
 *
 * `<Code code={source} language="tsx" theme="dracula" lineNumbers />`
 *
 * The code can also be passed as a string child. With a theme (default
 * "colorful") everything is styled inline and no CSS is required. Pass
 * `theme={false}` to get class names for your own stylesheet, and
 * `language="auto"` to guess the language from the code.
 */
export function Code({
  code,
  children,
  language = "text",
  theme = "colorful",
  lineNumbers = false,
  highlightLines,
  inline = false,
  className,
  style,
  ...rest
}) {
  const source = code ?? (typeof children === "string" ? children : "");
  // Guess once here, so the highlighting and `data-language` agree.
  const resolved = useMemo(
    () => (language === "auto" ? (detectLanguage({ code: source, guess: true }) ?? "text") : language),
    [source, language],
  );
  const html = useHighlight(source, resolved, { theme, lineNumbers, highlightLines });
  const colors = theme === false ? undefined : getTheme(theme);
  const lang = getLanguage(resolved);
  const classes = className ? `cc ${className}` : "cc";
  const themed = colors ? { background: colors.background, color: colors.foreground } : undefined;
  if (inline) {
    return createElement("code", {
      ...rest,
      className: classes,
      "data-language": lang?.id,
      style: themed ? { padding: "0.15em 0.35em", borderRadius: "0.3em", ...themed, ...style } : style,
      dangerouslySetInnerHTML: { __html: html },
    });
  }
  return createElement(
    "pre",
    {
      ...rest,
      className: classes,
      "data-language": lang?.id,
      style: themed ? { padding: "1em", overflowX: "auto", borderRadius: "0.5em", ...themed, ...style } : style,
    },
    createElement("code", { dangerouslySetInnerHTML: { __html: html } }),
  );
}

export default Code;
