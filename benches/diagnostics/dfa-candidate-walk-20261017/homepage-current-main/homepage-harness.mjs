// Compare saved Ferriki package directories on the homepage's repository
// corpus. Freeze --corpus-root before editing any measured source fixture.
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import { join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { resolveFerrikiPlatformTarget } from "../ferriki/platforms.mjs";
import { comparisonCorpus } from "./shiki-comparison-corpus.mjs";
import { standardGrammarRegistrations } from "./standard-grammar-reference.mjs";
import "./test-asset-env.mjs";

const { values } = parseArgs({
  options: {
    engine: { type: "string", multiple: true, default: [] },
    multi: { type: "boolean" },
    "ferroni-source-comparison": { type: "boolean" },
    "corpus-root": { type: "string" },
    processes: { type: "string", default: "6" },
    rounds: { type: "string", default: "40" },
    warmups: { type: "string", default: "12" },
    "cold-runs": { type: "string", default: "30" },
    worker: { type: "string" },
    cold: { type: "string" },
    write: { type: "string" },
  },
});
const root = fileURLToPath(new URL("../../", import.meta.url));
const corpusRoot = resolve(values["corpus-root"] ?? root);
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const median = (values) => {
  const sorted = values.toSorted((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
};
// Resample independent worker estimates, rather than treating every timed call
// in one process as independent. Intervals describe this local experiment.
const interval95 = (ratios) => {
  let state = 20261017;
  const estimates = [];
  for (let sample = 0; sample < 10_000; sample++) {
    const selected = [];
    for (let i = 0; i < ratios.length; i++) {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      selected.push(ratios[Math.floor((state / 2 ** 32) * ratios.length)]);
    }
    estimates.push(median(selected) - 1);
  }
  estimates.sort((a, b) => a - b);
  return [estimates[249], estimates[9749]];
};
const engines = values.engine.map((spec) => {
  const equals = spec.indexOf("=");
  assert.ok(equals > 0, "Use --engine name=/absolute/package/directory");
  return { name: spec.slice(0, equals), path: resolve(spec.slice(equals + 1)) };
});
assert.ok(engines.length >= 2, "Pass at least two package directories");
assert.equal(
  new Set(engines.map(({ name }) => name)).size,
  engines.length,
  "Duplicate engine name",
);
for (const key of ["processes", "rounds", "warmups", "cold-runs"])
  assert.ok(
    Number.isSafeInteger(Number(values[key])) && Number(values[key]) >= 0,
    `Invalid ${key}`,
  );
assert.ok(Number(values.processes) > 0 && Number(values.rounds) > 0);
const theme = "github-dark";
const themes = values.multi ? ["github-light", theme] : [theme];
const renderOptions = (lang) => values.multi
  ? { lang, themes: { light: "github-light", dark: theme }, defaultColor: false }
  : { lang, theme };
const langs = [...new Set(comparisonCorpus.map(([lang]) => lang))];
const loadDocs = () =>
  comparisonCorpus.map(([lang, path]) => {
    const code = readFileSync(join(corpusRoot, path), "utf8");
    return { lang, path, code, sha256: sha256(code), bytes: Buffer.byteLength(code) };
  });
async function createEngine(engine) {
  const api = await import(pathToFileURL(join(engine.path, "index.mjs")));
  return api.createHighlighter({ langs, themes });
}

if (values.cold !== undefined) {
  // Same boundary as bench-shiki-comparison: imports, corpus reading, grammar
  // loading, and the first highlight of every document; process spawn excluded.
  const start = performance.now();
  const docs = loadDocs();
  const highlighter = await createEngine(engines[Number(values.cold)]);
  let consumed = 0;
  for (const doc of docs)
    consumed += highlighter.codeToHtml(doc.code, renderOptions(doc.lang)).length;
  const elapsedMs = performance.now() - start;
  highlighter.dispose();
  console.log(JSON.stringify({ elapsedMs, consumed }));
} else if (values.worker !== undefined) {
  const seed = Number(values.worker);
  const docs = loadDocs();
  const highlighters = [];
  // Alternate initialization order too; measured call order rotates each round.
  for (let offset = 0; offset < engines.length; offset++) {
    const index = (seed + offset) % engines.length;
    highlighters[index] = await createEngine(engines[index]);
  }
  const shiki = await import("shiki");
  const oracle = await shiki.createHighlighter({
    langs: standardGrammarRegistrations(langs),
    themes,
    engine: shiki.createOnigurumaEngine(import("shiki/wasm")),
  });
  const cases = [];
  let consumed = 0;
  try {
    for (const doc of docs) {
      const options = renderOptions(doc.lang);
      const reference = oracle.codeToHtml(doc.code, options);
      const calls = highlighters.map((h) => () => h.codeToHtml(doc.code, options));
      for (const call of calls) assert.equal(call(), reference, `HTML differs: ${doc.path}`);
      for (let round = 0; round < Number(values.warmups); round++)
        for (const call of calls) consumed += call().length;
      const start = performance.now();
      calls[0]();
      const batch = Math.min(100, Math.max(1, Math.ceil(1 / (performance.now() - start))));
      const samplesMs = Object.fromEntries(engines.map(({ name }) => [name, []]));
      for (let round = 0; round < Number(values.rounds); round++) {
        for (let offset = 0; offset < calls.length; offset++) {
          const index = (round + seed + offset) % calls.length;
          const start = performance.now();
          let length = 0;
          for (let call = 0; call < batch; call++) length += calls[index]().length;
          const elapsedMs = performance.now() - start;
          consumed += length;
          samplesMs[engines[index].name].push(elapsedMs / batch);
        }
      }
      cases.push({ ...doc, code: undefined, outputSha256: sha256(reference), batch, samplesMs });
    }
  } finally {
    for (const h of highlighters) h.dispose();
    oracle.dispose();
  }
  console.log(JSON.stringify({ seed, consumed, cases }));
} else {
  const provenance = engines.map((engine) => {
    const require = createRequire(pathToFileURL(join(engine.path, "native.mjs")));
    const target = resolveFerrikiPlatformTarget();
    const addon = require.resolve(`${target.packageName}/ferriki.node`);
    const nativeBuild = JSON.parse(
      readFileSync(join(engine.path, ".benchmark-build.json"), "utf8"),
    );
    assert.equal(sha256(readFileSync(addon)), nativeBuild.binarySha256, "Stale native receipt");
    return {
      ...engine,
      nativeBuild,
      facadeSha256: Object.fromEntries(
        [
          "index.mjs",
          "src/index.mjs",
          "src/asset-download.mjs",
          "native.mjs",
          "platforms.mjs",
          "transformers.mjs",
          "classes.mjs",
          "assets/standard/catalog.mjs",
        ].map((path) => [path, sha256(readFileSync(join(engine.path, path)))]),
      ),
    };
  });
  for (const candidate of provenance.slice(1))
    for (const key of [
      "rustc",
      "cargoLockSha256",
      "rustflags",
      "encodedRustflags",
      "targetRustflags",
      "cargoProfileEnv",
      ...(values["ferroni-source-comparison"] ? [] : ["ferroni"]),
    ])
      assert.deepEqual(
        candidate.nativeBuild[key],
        provenance[0].nativeBuild[key],
        `Builds differ: ${key}`,
      );
  if (values["ferroni-source-comparison"]) {
    for (const candidate of provenance.slice(1)) {
      assert.equal(candidate.nativeBuild.rustSourceSha256, provenance[0].nativeBuild.rustSourceSha256);
      const stableFerroni = ({ revision, sourceSha256, ...rest }) => ({ ...rest, commit: revision?.commit });
      assert.deepEqual(stableFerroni(candidate.nativeBuild.ferroni), stableFerroni(provenance[0].nativeBuild.ferroni));
      assert.equal(candidate.nativeBuild.ferroni.source, null);
    }
  }
  const child = (...extra) => {
    const result = spawnSync(
      process.execPath,
      [fileURLToPath(import.meta.url), ...process.argv.slice(2), ...extra],
      { encoding: "utf8", timeout: 300_000, maxBuffer: 16 * 1024 * 1024 },
    );
    assert.equal(result.status, 0, result.stderr || String(result.error));
    return JSON.parse(result.stdout);
  };
  const runs = [];
  for (let seed = 0; seed < Number(values.processes); seed++) {
    runs.push(child("--worker", String(seed)));
    console.error(`[homepage] warm process ${seed + 1}/${values.processes}`);
  }
  const cold = [];
  let coldOutputLength;
  for (let round = 0; round < Number(values["cold-runs"]); round++) {
    const samplesMs = {};
    for (let offset = 0; offset < engines.length; offset++) {
      const index = (round + offset) % engines.length;
      const result = child("--cold", String(index));
      coldOutputLength ??= result.consumed;
      assert.equal(result.consumed, coldOutputLength, "Cold output length differs");
      samplesMs[engines[index].name] = result.elapsedMs;
    }
    cold.push(samplesMs);
    console.error(`[homepage] cold round ${round + 1}/${values["cold-runs"]}`);
  }
  const [base, ...candidates] = engines.map(({ name }) => name);
  const summary = {};
  for (const name of candidates) {
    const rows = runs[0].cases.map((entry, index) => {
      const ratios = runs.map((run) => {
        const current = run.cases[index];
        assert.equal(current.sha256, entry.sha256);
        assert.equal(current.outputSha256, entry.outputSha256);
        return median(current.samplesMs[name]) / median(current.samplesMs[base]);
      });
      return {
        path: entry.path,
        change: median(ratios) - 1,
        interval95: interval95(ratios),
        ratios,
      };
    });
    const totals = runs.map((run) => {
      const sum = (engine) =>
        run.cases.reduce((sum, row) => sum + median(row.samplesMs[engine]), 0);
      return { [base]: sum(base), [name]: sum(name) };
    });
    const coldRatios = cold.map((row) => row[name] / row[base]);
    summary[name] = {
      rows,
      totals,
      totalChange: median(totals.map((row) => row[name] / row[base])) - 1,
      coldChange: cold.length ? median(coldRatios) - 1 : null,
      coldInterval95: cold.length ? interval95(coldRatios) : null,
      coldRatios,
    };
  }
  const report = {
    measuredAt: new Date().toISOString(),
    machine: {
      cpu: os.cpus()[0].model,
      os: os.release(),
      platform: process.platform,
      arch: process.arch,
    },
    node: process.version,
    options: values,
    harnessSha256: sha256(readFileSync(fileURLToPath(import.meta.url))),
    engines: provenance,
    summary,
    runs,
    cold,
  };
  if (values.write) writeFileSync(values.write, `${JSON.stringify(report)}\n`);
  console.log(JSON.stringify(summary, null, 2));
}
