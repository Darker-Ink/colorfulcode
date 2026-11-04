import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Code } from "../dist/react.js";

test("renders a themed block", () => {
  const html = renderToStaticMarkup(createElement(Code, { code: "fn main() {}", language: "rust", theme: "nord" }));
  assert.match(html, /^<pre class="cc" data-language="rust" style="padding:1em;overflow-x:auto;border-radius:0.5em;background:#2e3440;color:#d8dee9"><code>/);
  assert.ok(html.includes(">fn</span>"));
});

test("accepts children, class names and inline mode", () => {
  const html = renderToStaticMarkup(createElement(Code, { language: "js", inline: true, theme: false, className: "x" }, "a()"));
  assert.equal(html, '<code class="cc x" data-language="javascript"><span class="cc-function">a</span><span class="cc-punctuation">()</span></code>');
});
