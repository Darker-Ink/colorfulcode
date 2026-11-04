#!/usr/bin/env node
'use strict';

const fs = require("fs");
const { join } = require("path");

/** Counts words in every file of a directory. */
async function countWords(dir, { ignore = [] } = {}) {
  const totals = new Map();
  for (const name of await fs.promises.readdir(dir)) {
    if (ignore.includes(name)) continue;
    const text = await fs.promises.readFile(join(dir, name), "utf8");
    const words = text.split(/\s+/).filter(Boolean);
    totals.set(name, words.length);
  }
  return totals;
}

const format = (count) => `${count.toLocaleString()} words`;
const half = 10 / 2, re = /^#{1,6}\s/m;

class Counter extends EventTarget {
  #value = 0;
  static from(n) { return Object.assign(new Counter(), { start: n }); }
  increment() { this.#value++; return this; }
  get value() { return this.#value; }
}

countWords(process.argv[2] ?? ".").then((totals) => {
  for (const [file, n] of totals) console.log(file.padEnd(20), format(n));
}).catch(console.error);

export const element = <div className="note">{half > 1 ? "big" : null}</div>;
