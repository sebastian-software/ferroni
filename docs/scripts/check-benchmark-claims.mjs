/**
 * Contract check: the engine comparison figures on the home page and in the
 * README must come from the retained run, not from memory.
 *
 * `scripts/compare-engines.py figures` condenses a run under
 * `benches/results/` into `app/data/engine-comparison.json`. The home page
 * renders that file directly. This script fails when
 *
 * - the README table between the `engine-comparison` markers differs from the
 *   rows the JSON describes,
 * - the JSON names a different commit or date than the measurement context of
 *   `perf/engine-comparison.mdx`, or
 * - the home page stops reading the JSON.
 *
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

/** The README rows, exactly as `readme_table` in compare-engines.py writes them. */
function expectedReadmeTable() {
  const { workloads, engines } = figures;
  const lines = [
    `| Ferroni compared with | ${workloads.map((workload) => workload.label).join(" | ")} |`,
    `| --- |${" ---: |".repeat(workloads.length)}`,
  ];
  for (const engine of engines) {
    const cells = workloads.map((workload) => {
      const cell = engine.cells[workload.id];
      return cell.note ? `${cell.text} (${cell.note})` : cell.text;
    });
    lines.push(`| ${engine.label} | ${cells.join(" | ")} |`);
  }
  return lines.join("\n");
}

const block = /<!-- engine-comparison -->\n([\s\S]*?)\n<!-- \/engine-comparison -->/.exec(readme);
if (block === null) {
  errors.push("README.md.src: the engine-comparison markers are missing.");
} else if (block[1].trim() !== expectedReadmeTable()) {
  errors.push(
    "README.md.src: the engine-comparison table differs from app/data/engine-comparison.json.\n" +
      `    Expected:\n${expectedReadmeTable().replaceAll(/^/gm, "      ")}`,
  );
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
if (!readme.includes(`Measured ${figures.measured}`)) {
  errors.push(`README.md.src does not name the measurement date ${figures.measured}.`);
}
if (!home.includes('from "../data/engine-comparison.json"')) {
  errors.push("home.tsx no longer renders app/data/engine-comparison.json.");
}

if (errors.length > 0) {
  console.error("Benchmark claims do not match their source:\n");
  for (const error of errors) console.error(`  - ${error}`);
  console.error(
    "\nRegenerate with `scripts/compare-engines.py figures <run> docs/app/data/engine-comparison.json`" +
      " and paste its table into README.md.src.",
  );
  process.exit(1);
}

console.log(
  `Benchmark claims check: ${figures.engines.length} engines match the ${figures.measured} run.`,
);
