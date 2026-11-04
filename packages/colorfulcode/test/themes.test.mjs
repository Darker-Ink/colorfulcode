import assert from "node:assert/strict";
import { test } from "node:test";
import { defineTheme, getTheme, highlight, listThemes } from "../dist/index.js";
import * as themes from "../dist/themes.js";

const builtins = new Set(listThemes().map((t) => t.name));
const kinds = new Set(Object.keys(listThemes()[0].styles));
const all = Object.entries(themes);
const hex = /^#[0-9a-f]{6}$/i;

// WCAG relative luminance and contrast ratio.
function luminance(color) {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(color.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
const color = (style) => (typeof style === "string" ? style : style.color);

test("there are optional themes", () => {
  assert.ok(all.length >= 24);
});

test("names are unique and do not clash with built in themes", () => {
  const names = all.map(([, t]) => t.name);
  assert.equal(new Set(names).size, names.length);
  for (const name of names) {
    assert.match(name, /^[a-z0-9]+(-[a-z0-9]+)*$/);
    assert.ok(!builtins.has(name), `${name} is a built in theme`);
  }
});

for (const [exportName, theme] of all) {
  test(`${exportName} (${theme.name})`, () => {
    assert.equal(typeof theme.dark, "boolean");
    assert.match(theme.background, hex);
    assert.match(theme.foreground, hex);
    for (const [kind, style] of Object.entries(theme.styles)) {
      assert.ok(kinds.has(kind), `unknown kind ${kind}`);
      assert.match(color(style), hex, `${kind} color`);
    }
    for (const kind of ["comment", "keyword", "string", "function", "type", "variable", "parameter", "property"]) {
      assert.ok(theme.styles[kind], `${kind} is not styled`);
    }
    const fg = contrast(theme.foreground, theme.background);
    assert.ok(fg >= 4.5, `foreground contrast ${fg.toFixed(2)}`);
    const comment = contrast(color(theme.styles.comment), theme.background);
    assert.ok(comment >= 3, `comment contrast ${comment.toFixed(2)}`);

    assert.equal(defineTheme(theme), theme.name);
    assert.equal(getTheme(theme.name).dark, theme.dark);
    const html = highlight("const x = 1; // hi", "ts", { theme });
    assert.ok(html.includes(`background:${theme.background}`));
    assert.ok(html.includes(`color:${color(theme.styles.keyword)}`));
  });
}
