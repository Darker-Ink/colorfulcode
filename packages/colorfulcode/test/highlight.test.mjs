import assert from "node:assert/strict";
import { test } from "node:test";
import {
  detectLanguage,
  getLanguage,
  getTheme,
  highlight,
  listLanguages,
  listThemes,
  themeCss,
  tokenize,
  tokenizeLines,
} from "../dist/index.js";

test("highlight returns themed HTML by default", () => {
  const html = highlight("const x = 1;", "js");
  assert.match(html, /^<pre class="cc" style="background:#12131c;color:#e4e6f1"><code>/);
  assert.match(html, /<span style="color:#ff6ac1">const<\/span>/);
});

test("class mode and escaping", () => {
  const html = highlight("a < b && c", "js", { theme: false, pre: false });
  assert.ok(html.includes("&lt;"));
  assert.ok(html.includes('<span class="cc-operator">&amp;&amp;</span>'));
  assert.ok(!html.includes("style="));
});

test("line numbers and highlighted lines", () => {
  const html = highlight("a\nb\nc", "text", { pre: false, lineNumbers: true, highlightLines: [2], theme: false });
  assert.equal((html.match(/class="cc-line/g) || []).length, 3);
  assert.ok(html.includes('<span class="cc-line cc-hl"><span class="cc-ln">2</span>b</span>'));
});

test("unknown languages render as plain text", () => {
  assert.equal(highlight("<b>", "nope", { pre: false }), "&lt;b&gt;");
});

test("tokens cover the input and use UTF-16 offsets", () => {
  const code = "const s = 'é🎨'; // done";
  const tokens = tokenize(code, "typescript");
  assert.equal(tokens.map((t) => t.text).join(""), code);
  let pos = 0;
  for (const t of tokens) {
    assert.equal(t.start, pos);
    assert.equal(code.slice(t.start, t.end), t.text);
    pos = t.end;
  }
  assert.ok(tokens.some((t) => t.kind === "string" && t.text === "'é🎨'"));
  assert.ok(tokens.some((t) => t.kind === "comment" && t.text === "// done"));
});

test("tokenizeLines splits multi line tokens", () => {
  const lines = tokenizeLines("/* a\nb */\nx", "c");
  assert.equal(lines.length, 3);
  assert.deepEqual(lines[0], [{ text: "/* a", kind: "comment", start: 0, end: 4 }]);
  assert.deepEqual(lines[1], [{ text: "b */", kind: "comment", start: 0, end: 4 }]);
});

test("metadata lookups", () => {
  assert.ok(listLanguages().length > 20);
  assert.equal(getLanguage("ts").id, "typescript");
  assert.equal(getLanguage("nope"), undefined);
  assert.equal(detectLanguage({ path: "src/App.tsx" }), "tsx");
  assert.equal(detectLanguage({ code: "#!/usr/bin/env python3\nprint(1)" }), "python");
  assert.ok(listThemes().some((t) => t.name === "dracula"));
  assert.equal(getTheme("dracula").background, "#282a36");
  assert.match(themeCss("nord", ".code"), /^\.code\{background:#2e3440/);
});

test("detection from content is opt-in", () => {
  const code = "def greet(name):\n    if not name:\n        return None\n    print(f'hi {name}')\n";
  // By default only paths, shebangs and doctypes count.
  assert.equal(detectLanguage({ code }), undefined);
  assert.equal(detectLanguage({ code, guess: true }), "python");
  assert.equal(detectLanguage({ path: "a.rb", code, guess: true }), "ruby");
  assert.equal(detectLanguage({ code: "just a sentence", guess: true }), undefined);
  // Unknown names never guess, "auto" does.
  assert.equal(highlight(code, "nope", { pre: false, theme: false }), highlight(code, "text", { pre: false, theme: false }));
  assert.equal(highlight(code, "auto", { theme: false }), highlight(code, "python", { theme: false }));
  assert.deepEqual(tokenize(code, "auto"), tokenize(code, "python"));
  assert.deepEqual(tokenizeLines("SELECT 1 FROM t;", "auto"), tokenizeLines("SELECT 1 FROM t;", "sql"));
  assert.equal(highlight("hello", "auto", { pre: false }), "hello");
});

test("large input", () => {
  const code = "let x = `a ${b}`; // c\n".repeat(20000);
  const html = highlight(code, "javascript");
  assert.ok(html.length > code.length);
  assert.equal(tokenize(code, "javascript").at(-1).end, code.length);
});

test("custom themes", async () => {
  const { defineTheme } = await import("../dist/index.js");
  const discord = {
    name: "discord-test",
    background: "#2b2d31",
    foreground: "#dbdee1",
    styles: { keyword: "#ff7ab2", comment: { color: "#80848e", italic: true } },
  };
  assert.equal(defineTheme(discord), "discord-test");
  const html = highlight("let x = 1; // hi", "js", { theme: "discord-test" });
  assert.match(html, /^<pre class="cc" style="background:#2b2d31;color:#dbdee1">/);
  assert.ok(html.includes('<span style="color:#ff7ab2">let</span>'));
  assert.ok(html.includes('<span style="color:#80848e;font-style:italic">// hi</span>'));
  const info = getTheme("discord-test");
  assert.equal(info.dark, true);
  assert.equal(info.styles.tag.color, "#ff7ab2"); // tags fall back to keywords
  // Objects work directly too.
  const inline = { name: "inline-test", background: "#ffffff", foreground: "#111111", styles: { string: "#00aa00" } };
  assert.ok(highlight("'a'", "js", { theme: inline }).includes("color:#00aa00"));
  assert.equal(getTheme(inline).dark, false);
  assert.throws(() => defineTheme({ name: "bad", background: "red", foreground: "#000000" }));
});
