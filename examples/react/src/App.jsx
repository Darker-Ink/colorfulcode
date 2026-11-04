import { Code, defineTheme, getTheme, highlight, listLanguages, listThemes } from "colorfulcode/react";
import { useCallback, useState } from "react";
import { Editor } from "./Editor.jsx";
import { samples } from "./samples.js";

// A custom theme, defined at runtime. See the README for how it works.
defineTheme({
  name: "discord",
  background: "#2b2d31",
  foreground: "#dbdee1",
  styles: {
    comment: { color: "#80848e", italic: true },
    keyword: "#eb459e",
    string: "#57f287",
    number: "#f0b232",
    function: "#00a8fc",
    type: "#fee75c",
    property: "#c9cdfb",
    operator: "#949cf7",
    punctuation: "#b5bac1",
  },
});

const languages = listLanguages().filter((l) => samples[l.id] !== undefined);
const themes = listThemes();

const usage = `import { Code } from "colorfulcode/react";

export function Example({ source }) {
  return <Code code={source} language="tsx" theme="tokyo-night" lineNumbers highlightLines={[4]} />;
}`;

export default function App() {
  const [language, setLanguage] = useState("tsx");
  const [theme, setTheme] = useState("colorful");
  const [code, setCode] = useState(samples.tsx);
  const colors = getTheme(theme);
  const [speed, setSpeed] = useState({ ms: 0, chars: 0 });
  // Smooth the number a little so it is readable while typing.
  const onTiming = useCallback(
    (ms, chars) => setSpeed((s) => ({ ms: s.chars === chars || !s.ms ? ms : s.ms * 0.7 + ms * 0.3, chars })),
    [],
  );

  const pickLanguage = (id) => {
    setLanguage(id);
    setCode(samples[id]);
  };

  return (
    <main className={colors.dark ? "dark" : "light"} style={{ "--page": colors.dark ? "#0b0c12" : "#f4f5f9" }}>
      <header>
        <h1 className="logo">
          {"colorfulcode".split("").map((c, i) => (
            <span key={i} style={{ "--i": i }}>{c}</span>
          ))}
        </h1>
        <p className="tagline">
          {listLanguages().length} languages, {themes.length - 1} themes, written in Rust, running as WebAssembly.
        </p>
      </header>

      <section className="toolbar">
        <label>
          Language
          <select value={language} onChange={(e) => pickLanguage(e.target.value)}>
            {languages.map((l) => (
              <option key={l.id} value={l.id}>{l.name}</option>
            ))}
          </select>
        </label>
        <label>
          Theme
          <select value={theme} onChange={(e) => setTheme(e.target.value)}>
            {themes.map((t) => (
              <option key={t.name} value={t.name}>
                {t.name === "discord" ? "discord (custom)" : t.name}
              </option>
            ))}
          </select>
        </label>
        <span className="speed">
          {code.length.toLocaleString()} chars highlighted in <b>{speed.ms.toFixed(2)} ms</b>
        </span>
      </section>

      <Editor value={code} onChange={setCode} language={language} theme={theme} colors={colors} onTiming={onTiming} />

      <section className="usage">
        <h2>Using it in React</h2>
        <Code code={usage} language="tsx" theme={theme} lineNumbers highlightLines={[4]} />
        <p>
          Inline snippets work too: <Code inline language="js" theme={theme} code={"console.log(`hi ${name}`)"} />
        </p>
      </section>
    </main>
  );
}
