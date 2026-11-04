import { highlight } from "colorfulcode/react";
import { useLayoutEffect, useMemo, useRef, useState } from "react";

const OVERSCAN = 30;

/**
 * A live editor: a transparent textarea on top of the highlighted code.
 *
 * Every keystroke re-highlights the whole document, which takes a few
 * milliseconds even for large files. What is expensive is the browser laying
 * out tens of thousands of spans, so only the lines in view are put in the DOM.
 */
export function Editor({ value, onChange, language, theme, colors, onTiming }) {
  const area = useRef(null);
  const code = useRef(null);
  const [view, setView] = useState({ top: 0, height: 800 });
  // Distance between lines as laid out by the browser. CSS says 22.4px, but
  // layout rounds it, and the rounding would add up over thousands of lines.
  const pitch = useRef(22.4);

  const lines = useMemo(() => {
    const start = performance.now();
    const html = highlight(value, language, { theme, pre: false, lines: true });
    const ms = performance.now() - start;
    return { ms, html: html.split("\n") };
  }, [value, language, theme]);

  useLayoutEffect(() => onTiming?.(lines.ms, value.length), [lines, onTiming, value.length]);

  const first = Math.max(0, Math.floor(view.top / pitch.current) - OVERSCAN);
  const last = Math.min(lines.html.length, Math.ceil((view.top + view.height) / pitch.current) + OVERSCAN);
  const visible = useMemo(() => lines.html.slice(first, last).join(""), [lines, first, last]);

  // Moves the visible slice with the textarea's scroll position, so the two
  // layers line up exactly however far the document scrolls.
  const sync = () => {
    const el = area.current;
    const rows = code.current.children;
    if (rows.length > 1) pitch.current = (rows[rows.length - 1].offsetTop - rows[0].offsetTop) / (rows.length - 1);
    code.current.style.transform = `translate(${-el.scrollLeft}px, ${first * pitch.current - el.scrollTop}px)`;
    if (Math.abs(el.scrollTop - view.top) > pitch.current * (OVERSCAN / 2) || el.clientHeight !== view.height) {
      setView({ top: el.scrollTop, height: el.clientHeight });
    }
  };
  useLayoutEffect(sync);

  const onKeyDown = (e) => {
    if (e.key !== "Tab") return;
    e.preventDefault();
    const el = e.target;
    const { selectionStart: start, selectionEnd: end } = el;
    onChange(value.slice(0, start) + "  " + value.slice(end));
    requestAnimationFrame(() => el.setSelectionRange(start + 2, start + 2));
  };

  return (
    <div className="editor" style={{ background: colors.background, color: colors.foreground }}>
      <pre aria-hidden="true">
        <code ref={code} dangerouslySetInnerHTML={{ __html: visible }} />
      </pre>
      <textarea
        ref={area}
        value={value}
        spellCheck={false}
        autoCapitalize="off"
        autoComplete="off"
        onChange={(e) => onChange(e.target.value)}
        onScroll={sync}
        onKeyDown={onKeyDown}
        style={{ caretColor: colors.foreground }}
      />
    </div>
  );
}
