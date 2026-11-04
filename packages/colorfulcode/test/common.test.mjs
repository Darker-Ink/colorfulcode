import assert from "node:assert/strict";
import { test } from "node:test";
import { getLanguage, highlight, listLanguages } from "../dist/common.js";
import * as full from "../dist/index.js";

test("the common build has the popular languages only", () => {
  const count = listLanguages().length;
  assert.ok(count >= 40 && count < full.listLanguages().length);
  assert.equal(getLanguage("ts").id, "typescript");
  assert.equal(getLanguage("clojure"), undefined);
  assert.ok(highlight("fn main() {}", "rust").includes(">fn</span>"));
});
