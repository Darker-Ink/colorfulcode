// Highlights the same TypeScript with each library in this browser, then
// replays the measured times as a race. Bars move in real time.

import hljs from "highlight.js/lib/core";
import hljsTypescript from "highlight.js/lib/languages/typescript";
import Prism from "prismjs";
import "prismjs/components/prism-typescript.js";
import { createHighlighter } from "shiki";
import { highlight } from "colorfulcode";
import sample from "../../crates/colorfulcode/tests/samples/typescript.ts?raw";

hljs.registerLanguage("typescript", hljsTypescript);
const shiki = await createHighlighter({ themes: ["github-dark"], langs: ["typescript"] });

const code = sample.repeat(Math.ceil(200_000 / sample.length));
document.getElementById("size").textContent = `${Math.round(code.length / 1000)} KB`;

const racers = [
  { name: "colorfulcode", color: "linear-gradient(90deg, #ff5fa2, #ffc23d, #2fd9b5, #7b7cff)", run: () => highlight(code, "typescript") },
  { name: "Prism", color: "#5c6370", run: () => Prism.highlight(code, Prism.languages.typescript, "typescript") },
  { name: "highlight.js", color: "#5c6370", run: () => hljs.highlight(code, { language: "typescript" }).value },
  { name: "Shiki", color: "#5c6370", run: () => shiki.codeToHtml(code, { lang: "typescript", theme: "github-dark" }) },
];

// Median of a few runs after a warm up. `?times=a,b,c,d` replays earlier
// measurements instead, which is handy for recording.
const given = new URLSearchParams(location.search).get("times")?.split(",").map(Number);
if (given) racers.forEach((r, i) => (r.ms = given[i]));
else for (const r of racers) {
  r.run();
  const times = [];
  for (let i = 0; i < 5; i++) {
    const t = performance.now();
    r.run();
    times.push(performance.now() - t);
  }
  r.ms = times.sort((a, b) => a - b)[2];
}

const fastest = Math.min(...racers.map((r) => r.ms));
document.getElementById("note").textContent = given
  ? "Measured in Chromium, replayed in real time."
  : "Median of 5 runs in this browser, replayed in real time.";
const lanes = document.getElementById("lanes");
for (const r of racers) {
  r.el = document.createElement("div");
  r.el.className = "lane";
  r.el.innerHTML = `<div class="name">${r.name}</div><div class="track"><div class="bar" style="background:${r.color}"></div></div><div class="time">0 ms</div>`;
  lanes.append(r.el);
}

/** Draws the race as it looks `elapsed` milliseconds after the start. */
function render(elapsed) {
  let running = false;
  for (const r of racers) {
    const progress = Math.max(0, Math.min(1, elapsed / r.ms));
    r.el.querySelector(".bar").style.width = `${progress * 100}%`;
    const time = r.el.querySelector(".time");
    const done = progress >= 1;
    r.el.classList.toggle("done", done);
    if (!done) {
      running = true;
      time.textContent = `${Math.max(0, Math.round(elapsed))} ms`;
    } else {
      const ratio = r.ms / fastest;
      time.innerHTML = ratio < 1.05 ? `${r.ms.toFixed(1)} ms` : `${Math.round(r.ms)} ms <span class="slower">${ratio.toFixed(0)}x slower</span>`;
    }
  }
  return running;
}

render(0);
// `?manual` leaves the clock to the caller (used to record frames).
window.renderAt = render;
window.raceReady = true;
if (!new URLSearchParams(location.search).has("manual")) {
  await new Promise((resolve) => setTimeout(resolve, 700));
  const start = performance.now();
  const frame = (now) => render(now - start) && requestAnimationFrame(frame);
  requestAnimationFrame(frame);
}
