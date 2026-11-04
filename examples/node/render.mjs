// Renders a source file to a standalone HTML page.
//
//   node examples/node/render.mjs src/main.rs > main.html

import { readFileSync } from "node:fs";
import { detectLanguage, highlight } from "colorfulcode";

const path = process.argv[2];
if (!path) {
  console.error("usage: node render.mjs <file> [theme]");
  process.exit(2);
}

const code = readFileSync(path, "utf8");
const language = detectLanguage({ path, code }) ?? "text";
const theme = process.argv[3] ?? "github-dark";

const body = highlight(code, language, { theme, lineNumbers: true });

console.log(`<!doctype html>
<meta charset="utf-8">
<title>${path}</title>
<style>body { margin: 0 } pre { margin: 0; padding: 24px; min-height: 100vh; font: 14px/1.6 ui-monospace, monospace }</style>
${body}`);
