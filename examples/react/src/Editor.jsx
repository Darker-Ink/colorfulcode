import { useHighlight } from "colorfulcode/react";
import { useRef } from "react";

/**
 * A live editor: a transparent textarea on top of the highlighted code.
 * Every keystroke re-highlights the whole document.
 */
export function Editor({ value, onChange, language, theme, colors }) {
  const html = useHighlight(value + "\n", language, { theme });
  const pre = useRef(null);
  const syncScroll = (e) => {
    pre.current.scrollTop = e.target.scrollTop;
    pre.current.scrollLeft = e.target.scrollLeft;
  };
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
      <pre ref={pre} aria-hidden="true" dangerouslySetInnerHTML={{ __html: html }} />
      <textarea
        value={value}
        spellCheck={false}
        autoCapitalize="off"
        autoComplete="off"
        onChange={(e) => onChange(e.target.value)}
        onScroll={syncScroll}
        onKeyDown={onKeyDown}
        style={{ caretColor: colors.foreground }}
      />
    </div>
  );
}
