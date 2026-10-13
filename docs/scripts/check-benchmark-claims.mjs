/**
 * Check that the website's benchmark data agrees with the retained measurement
 * context, the home page reads that data, and the README links to the evidence.
 * Run with `pnpm check:numbers`; `pnpm build` runs it first.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const here = import.meta.dirname;
const figuresPath = resolve(here, "../app/data/engine-comparison.json");
const readmePath = resolve(here, "../../README.md.src");
const pagePath = resolve(here, "../app/routes/perf/engine-comparison.mdx");
const homePath = resolve(here, "../app/routes/home.tsx");

const figures = JSON.parse(readFileSync(figuresPath, "utf8"));
const readme = readFileSync(readmePath, "utf8");
const page = readFileSync(pagePath, "utf8");
const home = readFileSync(homePath, "utf8");

const errors = [];

// The README summarizes the same figures and links to the full evidence.
if (!readme.includes("https://ferroni.dev/perf/engine-comparison")) {
  errors.push("README.md.src must link to the benchmark evidence.");
}
if (/<!-- \/?engine-comparison -->/.test(readme)) {
  errors.push(
    "Keep benchmark tables on the website rather than duplicating them in README.md.src.",
  );
}

const summary = /## Measured performance\n([\s\S]*?)(?=\n## )/.exec(readme)?.[1];
const summaryText = summary?.replaceAll(/\s+/g, " ");
const measuredDate = new Date(`${figures.measured}T00:00:00Z`).toLocaleDateString("en-US", {
  year: "numeric",
  month: "long",
  day: "numeric",
  timeZone: "UTC",
});
const c = figures.engines.find((engine) => engine.id === "c").cells;
const jit = figures.engines.find((engine) => engine.id === "pcre2_jit").cells;
const phrase = (cell) => `${cell.text} ${cell.direction}`;
for (const expected of [
  measuredDate,
  `commit \`${figures.commit.slice(0, 8)}\``,
  `${phrase(c.shared)} than C Oniguruma on everyday patterns`,
  `${phrase(c.oniguruma)} on advanced patterns`,
  `${phrase(c.highlighting)} on three portable highlighting documents`,
  `${phrase(jit.shared)} on compatible everyday patterns`,
  `${phrase(jit.oniguruma)} on compatible advanced patterns`,
  `${phrase(jit.highlighting)} on those scanner replays`,
]) {
  if (!summaryText?.includes(expected)) {
    errors.push(`README.md.src benchmark summary must include: ${expected}`);
  }
}

for (const [field, label, pattern] of [
  ["commit", "Ferroni commit", /Ferroni commit\s*\| `([^`]+)`/],
  ["measured", "Measurement date", /Measurement date\s*\| `([^`]+)`/],
]) {
  const value = pattern.exec(page)?.[1];
  if (value === undefined) {
    errors.push(`perf/engine-comparison.mdx: "${label}" is missing from the measurement context.`);
  } else if (value !== figures[field]) {
    errors.push(
      `perf/engine-comparison.mdx names ${label} ${value}, the figures name ${figures[field]}.`,
    );
  }
}
if (!home.includes('from "../data/engine-comparison.json"')) {
  errors.push("home.tsx no longer renders app/data/engine-comparison.json.");
}

if (errors.length > 0) {
  console.error("Benchmark claims do not match their source:\n");
  for (const error of errors) console.error(`  - ${error}`);
  console.error(
    "\nRegenerate with `scripts/compare-engines.py figures <run> docs/app/data/engine-comparison.json`" +
      ".",
  );
  process.exit(1);
}

console.log(
  `Benchmark claims check: ${figures.engines.length} engines match the ${figures.measured} run.`,
);
