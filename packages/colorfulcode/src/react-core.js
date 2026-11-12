// React bindings for one colorfulcode API. `react.js`, `common-react.js` and
// `web-react.js` each call `createReact` with their own entry point.

import { createElement, useEffect, useMemo, useState } from "react";
import { escapeHtml } from "./core.js";

export function createReact(cc) {
  /**
   * Whether the module is ready. With `colorfulcode/web` this starts loading
   * it and turns true once loaded; with the other entries it is always true.
   */
  function useReady() {
    const [ready, setReady] = useState(cc.isReady);
    useEffect(() => {
      if (ready) return undefined;
      const unsubscribe = cc.subscribe(() => setReady(true));
      cc.init().catch((error) => console.error(error));
      if (cc.isReady()) setReady(true);
      return unsubscribe;
    }, [ready]);
    return ready;
  }

  /** Memoized `highlight()` without the `<pre>` wrapper. Plain text until ready. */
  function useHighlight(code, language, options = {}) {
    const ready = useReady();
    const { theme, lines, lineNumbers, highlightLines } = options;
    const marked = highlightLines ? highlightLines.join(",") : "";
    return useMemo(
      () => (ready ? cc.highlight(code, language, { theme, lines, lineNumbers, highlightLines, pre: false }) : escapeHtml(code)),
      // eslint-disable-next-line react-hooks/exhaustive-deps
      [ready, code, language, theme, lines, lineNumbers, marked],
    );
  }

  /** Memoized `tokenizeLines()`, for rendering tokens with your own components. */
  function useTokens(code, language) {
    const ready = useReady();
    return useMemo(() => {
      if (ready) return cc.tokenizeLines(code, language);
      // Before the module loads every line is a single plain token.
      return code.split("\n").map((line) => {
        const text = line.endsWith("\r") ? line.slice(0, -1) : line;
        return text ? [{ text, kind: "text", start: 0, end: text.length }] : [];
      });
    }, [ready, code, language]);
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
  function Code({
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
    const ready = useReady();
    // Guess once here, so the highlighting and `data-language` agree.
    const resolved = useMemo(() => {
      if (language !== "auto") return language;
      return ready ? (cc.detectLanguage({ code: source, guess: true }) ?? "text") : "text";
    }, [ready, source, language]);
    const html = useHighlight(source, resolved, { theme, lineNumbers, highlightLines });
    const colors = ready && theme !== false ? cc.getTheme(theme) : undefined;
    const lang = ready ? cc.getLanguage(resolved) : undefined;
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

  return { Code, useHighlight, useTokens, useReady };
}
