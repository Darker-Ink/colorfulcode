import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { highlight, registerAlias, tokenize, tokenizeLines, tokenizeRanges, isReady, init } from "../dist/index.js";
import * as web from "../dist/web.js";
import { Code as WebCode } from "../dist/web-react.js";

test("tokenizeLines gives offsets relative to each line", () => {
  const lines = tokenizeLines("let a = 1;\r\n  // two\nx", "javascript");
  assert.equal(lines.length, 3);
  assert.deepEqual(lines[0][0], { text: "let", kind: "keyword", start: 0, end: 3 });
  const comment = lines[1].find((t) => t.kind === "comment");
  assert.deepEqual(comment, { text: "// two", kind: "comment", start: 2, end: 8 });
  assert.ok(lines[0].every((t) => !t.text.includes("\r")));
});

test("tokenizeRanges returns flat triples", () => {
  const code = "const é = 'x';";
  const { ranges, kinds } = tokenizeRanges(code, "js");
  assert.ok(ranges instanceof Uint32Array);
  assert.equal(ranges.length % 3, 0);
  assert.equal(ranges[0], 0);
  assert.equal(ranges[ranges.length - 2], code.length);
  assert.equal(kinds[ranges[2]], "keyword");
  // A copy: later calls do not change it.
  const before = Array.from(ranges);
  tokenize("fn x() {}", "rust");
  assert.deepEqual(Array.from(ranges), before);
});

test("tokenize can skip the text", () => {
  const tokens = tokenize("let x", "js", { text: false });
  assert.deepEqual(tokens[0], { start: 0, end: 3, kind: "keyword" });
});

test("registerAlias maps names", () => {
  assert.equal(highlight("ls -la", "curl", { pre: false, theme: false }), "ls -la");
  registerAlias("curl", "bash");
  assert.ok(highlight("ls -la", "CURL", { pre: false, theme: false }).includes("cc-function"));
});

test("synchronous entries are always ready", async () => {
  assert.equal(isReady(), true);
  await init();
});

test("the web entry needs init()", async () => {
  assert.equal(web.isReady(), false);
  assert.throws(() => web.highlight("x", "js"), /init\(\)/);
  // Plain text before the module loads.
  const html = renderToStaticMarkup(createElement(WebCode, { code: "<a> & b", language: "js" }));
  assert.equal(html, '<pre class="cc"><code>&lt;a&gt; &amp; b</code></pre>');
  let notified = 0;
  web.subscribe(() => notified++);
  const bytes = readFileSync(new URL("../dist/colorfulcode.wasm", import.meta.url));
  const first = web.init(bytes);
  assert.equal(web.init(bytes), first, "init() returns the same promise while loading");
  await first;
  assert.equal(web.isReady(), true);
  assert.equal(notified, 1);
  assert.ok(web.highlight("let x", "js").includes(">let</span>"));
  // The web build has the common languages, including http.
  assert.equal(web.getLanguage("http").id, "http");
  assert.equal(web.getLanguage("clojure"), undefined);
});

test("the web module is a separate, smaller file", () => {
  const webSize = readFileSync(new URL("../dist/colorfulcode.wasm", import.meta.url)).length;
  const common = readFileSync(new URL("../dist/wasm-common.js", import.meta.url), "utf8");
  const commonSize = Buffer.from(common.match(/"([A-Za-z0-9+/=]+)"/)[1], "base64").length;
  assert.ok(webSize < commonSize, `${webSize} should be smaller than ${commonSize}`);
});
