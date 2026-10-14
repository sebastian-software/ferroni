import { spawnSync } from "node:child_process";
/**
 * Keep the public workload description aligned with the source-backed corpus.
 * Published figures must regenerate from complete retained Blacksmith evidence.
 */
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const catalog = JSON.parse(
  readFileSync(resolve(root, "benches/production_regex/catalog.json"), "utf8"),
);
const page = readFileSync(resolve(root, "docs/app/routes/perf/regex-workloads.mdx"), "utf8");
const home = readFileSync(resolve(root, "docs/app/routes/home.tsx"), "utf8");
const readme = readFileSync(resolve(root, "README.md.src"), "utf8");
const errors = [];

// The readable overview makes qualitative claims alongside generated numbers.
// Fail publication when a later run no longer supports those statements.
const summary = JSON.parse(
  readFileSync(resolve(root, "docs/app/data/production-summary.json"), "utf8"),
);
for (const platform of summary.platforms) {
  const pair = (engine, lane = "steady") =>
    platform.comparisons.find((entry) => entry.engine === engine && entry.lane === lane);
  const jit = pair("pcre2_jit");
  if (!jit || jit.comparator_cases !== jit.case_count || jit.case_count === 0) {
    errors.push("Revise the overview and README: PCRE2 JIT no longer leads every reuse case.");
  }
  const fancy = pair("fancy_regex");
  if (!fancy || Math.abs(fancy.family_weighted_ratio - 1) >= 0.02) {
    errors.push("Revise the overview: fancy-regex aggregate reuse time is no longer within 2%.");
  }
  const compile = pair("c", "compile");
  if (!compile || compile.family_weighted_ratio >= 1) {
    errors.push(
      "Revise the overview: C Oniguruma no longer has a compile-time aggregate advantage.",
    );
  }
  const regex = pair("regex");
  if (!regex || regex.family_weighted_ratio >= 1 || regex.case_count >= summary.case_count) {
    errors.push("Revise the README: Rust regex no longer leads on its shared reuse subset.");
  }
  const c = pair("c");
  if (!c || c.family_weighted_ratio <= 1) {
    errors.push("Revise the README: Ferroni no longer leads C's shared reuse aggregate.");
  }
  const errorAware = pair("rust_erroraware");
  if (!errorAware || errorAware.family_weighted_ratio <= 1) {
    errors.push(
      "Revise the overview: the error-aware API no longer takes more aggregate reuse time.",
    );
  }
}
if (
  summary.platforms.some((platform) => platform.repetitions !== summary.platforms[0].repetitions)
) {
  errors.push("The overview must describe differing platform repetition counts.");
}

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
const publication = resolve(root, "benches/production_results/publication.json");
if (existsSync(publication)) {
  const check = spawnSync("python3", [resolve(root, "scripts/publish-production.py"), "--check"], {
    cwd: root,
    encoding: "utf8",
  });
  if (check.status !== 0) {
    errors.push(`Published benchmark evidence did not verify: ${check.stderr || check.error}`);
  }
  if (!page.includes("<ProductionComparison />")) {
    errors.push("The workload page must expose the generated per-case results.");
  }
} else if (!page.includes("it makes no new performance claim")) {
  errors.push("Do not replace the measurement-status note without verified publication evidence.");
}
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(
  `Workload claims check: ${productionInputs} production inputs and pinned source links match the corpus.`,
);
