import { Code, getTheme, highlight, listLanguages, listThemes } from "colorfulcode/react";
import { useMemo, useState } from "react";
import { Editor } from "./Editor.jsx";
import { samples } from "./samples.js";

const languages = listLanguages().filter((l) => samples[l.id] !== undefined);
const themes = listThemes();

const usage = `import { Code } from "colorfulcode/react";

export function Example({ source }) {
  return <Code code={source} language="tsx" theme="tokyo-night" lineNumbers highlightLines={[4]} />;
}`;

/** Times a full highlight of the current document, repeated for a stable number. */
function useSpeed(code, language) {
  return useMemo(() => {
    const runs = 20;
    const start = performance.now();
    for (let i = 0; i < runs; i++) highlight(code, language);
    const ms = (performance.now() - start) / runs;
    const mbps = code.length / 1e6 / (ms / 1000);
    return { ms, mbps };
  }, [code, language]);
}

export default function App() {
  const [language, setLanguage] = useState("tsx");
  const [theme, setTheme] = useState("colorful");
  const [code, setCode] = useState(samples.tsx);
  const colors = getTheme(theme);
  const speed = useSpeed(code, language);

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
          {listLanguages().length} languages, {themes.length} themes, written in Rust, running as WebAssembly.
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
              <option key={t.name} value={t.name}>{t.name}</option>
            ))}
          </select>
        </label>
        <span className="speed">
          {code.length.toLocaleString()} chars in <b>{speed.ms.toFixed(3)} ms</b> ({speed.mbps.toFixed(0)} MB/s)
        </span>
      </section>

      <Editor value={code} onChange={setCode} language={language} theme={theme} colors={colors} />

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
