// Compares HTML highlighting throughput with other popular highlighters.
//
//   npm run build && cd bench && npm install && npm run bench

import { readFileSync } from "node:fs";
import hljs from "highlight.js";
import Prism from "prismjs";
import loadLanguages from "prismjs/components/index.js";
import { createHighlighter } from "shiki";
import { highlight } from "colorfulcode";

const samples = "../crates/colorfulcode/tests/samples";
const cases = [
  { name: "TypeScript", file: "typescript.ts", cc: "typescript", hljs: "typescript", prism: "typescript", shiki: "typescript" },
  { name: "Python", file: "python.py", cc: "python", hljs: "python", prism: "python", shiki: "python" },
  { name: "Rust", file: "rust.rs", cc: "rust", hljs: "rust", prism: "rust", shiki: "rust" },
  { name: "CSS", file: "css.css", cc: "css", hljs: "css", prism: "css", shiki: "css" },
];

loadLanguages(cases.map((c) => c.prism));
const shiki = await createHighlighter({ themes: ["github-dark"], langs: cases.map((c) => c.shiki) });

const engines = {
  colorfulcode: (code, c) => highlight(code, c.cc),
  Prism: (code, c) => Prism.highlight(code, Prism.languages[c.prism], c.prism),
  "highlight.js": (code, c) => hljs.highlight(code, { language: c.hljs }).value,
  "Shiki": (code, c) => shiki.codeToHtml(code, { lang: c.shiki, theme: "github-dark" }),
};

function measure(fn, code, c) {
  fn(code, c); // warm up
  let runs = 0;
  const start = performance.now();
  while (performance.now() - start < 1000) {
    fn(code, c);
    runs++;
  }
  const ms = (performance.now() - start) / runs;
  return code.length / 1e3 / ms; // MB/s
}

const rows = [];
for (const c of cases) {
  // About 100 KB of code per case.
  const sample = readFileSync(`${samples}/${c.file}`, "utf8");
  const code = sample.repeat(Math.ceil(100_000 / sample.length));
  const row = { language: c.name };
  for (const [name, fn] of Object.entries(engines)) row[name] = measure(fn, code, c);
  rows.push(row);
}

const names = Object.keys(engines);
console.log(`| Language | ${names.join(" | ")} |`);
console.log(`| --- | ${names.map(() => "---:").join(" | ")} |`);
for (const row of rows) {
  const cells = names.map((n) => {
    const x = row[n] / row.colorfulcode;
    return n === "colorfulcode" ? `**${row[n].toFixed(1)} MB/s**` : `${row[n].toFixed(1)} MB/s (${(1 / x).toFixed(0)}x slower)`;
  });
  console.log(`| ${row.language} | ${cells.join(" | ")} |`);
}
