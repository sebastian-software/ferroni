/**
 * Keep the public workload description aligned with the source-backed corpus.
 * No performance figures are published until repeated-run evidence is reviewed.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const catalog = JSON.parse(
  readFileSync(resolve(root, "benches/production_regex/catalog.json"), "utf8"),
);
const page = readFileSync(resolve(root, "docs/app/routes/perf/regex-workloads.mdx"), "utf8");
const home = readFileSync(resolve(root, "docs/app/routes/home.tsx"), "utf8");
const readme = readFileSync(resolve(root, "README.md.src"), "utf8");
const errors = [];

const productionInputs = catalog.cases
  .filter((entry) => entry.pattern_origin === "production" && !entry.pending_reason)
  .reduce((total, entry) => total + entry.inputs.length, 0);
if (!page.includes(`The ${productionInputs}\nupstream-derived inputs`)) {
  errors.push("Update the workload page's fixture count from catalog.json.");
}
for (const [name, content] of [
  ["README", readme],
  ["homepage", home],
]) {
  if (!content.includes("/perf/regex-workloads")) {
    errors.push(`${name} must link to the current regex workload scope.`);
  }
  if (/\/perf\/(?:engine-comparison|benchmark-results)/.test(content)) {
    errors.push(`${name} must not link to retired measurement pages.`);
  }
}
const sources = new Set(Object.values(catalog.sources).map((source) => source.url));
for (const [url] of page.matchAll(/https:\/\/github\.com\/[^)\s]+\/blob\/[a-f0-9]{40}\/[^)\s]+/g)) {
  if (!sources.has(url)) errors.push(`Workload source link is not in the pinned catalog: ${url}`);
}
if (!page.includes("it makes no new performance claim")) {
  errors.push("Do not replace the measurement-status note without reviewed publication evidence.");
}
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(
  `Workload claims check: ${productionInputs} production inputs and pinned source links match the corpus.`,
);
