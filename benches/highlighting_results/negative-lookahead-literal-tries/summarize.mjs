// Compare the isolated negative-lookahead trial through the unchanged public harness.
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const [ferrikiPath, resultPath] = process.argv.slice(2);
assert.ok(resultPath, 'Usage: node summarize.mjs FERRIKI_ROOT RESULT_DIRECTORY');
const root = resolve(resultPath);
const { compareReports, readReport } = await import(pathToFileURL(join(resolve(ferrikiPath), 'node/scripts/tiobe-benchmark.mjs')));
const read = label => readReport(join(root, `public-${label}.json.gz`));
const variants = Object.fromEntries(['control', 'candidate'].map(v => [v, [1, 2].map(n => read(`${v}-${n}`))]));
for (const [variant, runs] of Object.entries(variants)) {
  for (const report of runs) {
    assert.equal(report.languages.length, 20);
    assert.equal(report.revision.status, '');
    assert.equal(report.nativeBuild.ferroni.revision.status, '');
    for (const row of report.languages) {
      assert.equal(row.status, 'measured', row.name);
      assert.equal(row.cases.length, 2);
      for (const entry of row.cases) {
        for (const [id, result] of Object.entries(entry.results)) {
          if (id === 'prism' && !row.prism) { assert.equal(result.status, 'unsupported'); continue; }
          assert.equal(result.status, 'ok', `${variant}/${row.name}/${entry.size}/${id}`);
          assert.equal(result.validation.sourcePreserved, true);
          if (id !== 'prism') assert.deepEqual(result.validation.referenceParity, { tokens: true, html: true });
          for (const api of ['html', 'tokens']) assert.ok(result[api].samplesMs.length >= report.method.minRounds);
        }
      }
    }
  }
}
const checked = (a, b) => {
  const result = compareReports(a, b);
  assert.equal(result.comparisons.length, 80);
  assert.deepEqual(result.excluded, []);
  return result;
};
const pairComparisons = variants.control.map((report,n) => checked(report, variants.candidate[n]));
const repeatComparisons = Object.fromEntries(Object.entries(variants).map(([v,runs]) => [v, checked(...runs)]));
const cases = variants.control[0].languages.flatMap((row, index) => row.cases.map(entry => {
  const values = Object.fromEntries(Object.entries(variants).map(([v,runs]) => [v,runs.map(report => {
    const current = report.languages[index].cases.find(c => c.size === entry.size);
    const best = ['shiki-wasm','shiki-js'].sort((a,b) => current.results[a].html.medianMs - current.results[b].html.medianMs)[0];
    return { ferrikiHtmlMs: current.results.ferriki.html.medianMs, ferrikiTokensMs: current.results.ferriki.tokens.medianMs,
      bestShikiHtmlMs: current.results[best].html.medianMs, bestShikiHtmlEngine: best,
      ferrikiVsBestShikiHtml: current.results.ferriki.html.medianMs / current.results[best].html.medianMs };
  })]));
  return {language: row.name, textmate: row.textmate, size: entry.size, bytes: entry.bytes, sha256: entry.sha256, ...values,
    pairedHtmlChangePercent: [0,1].map(n => 100*(values.candidate[n].ferrikiHtmlMs/values.control[n].ferrikiHtmlMs-1)),
    pairedRelativeToBestShikiChangePercent: [0,1].map(n => 100*(values.candidate[n].ferrikiVsBestShikiHtml/values.control[n].ferrikiVsBestShikiHtml-1)) };
}));
const wins = Object.fromEntries(Object.entries(variants).map(([variant]) => [variant, Object.fromEntries(['example','large'].map(size => [size,[0,1].map(n => cases.filter(c => c.size === size && c[variant][n].ferrikiVsBestShikiHtml < 1).length)]))]));
const summary = { primaryMetric: {api:'html',output:'inline HTML',target:'ferriki < min(shiki-wasm, shiki-js)'},
  diagnosticApis:['tokens'], unmeasuredProductOutputs:['HTML with CSS in classes mode'],
  order:['control-1','candidate-1','candidate-2','control-2'],
  method:variants.control[0].method, machine:variants.control[0].machine, versions:variants.control[0].versions,
  builds:Object.fromEntries(Object.entries(variants).map(([v,runs]) => [v,runs[0].nativeBuild])),
  pairComparisons, repeatComparisons, wins, cases };
writeFileSync(join(root,'summary.json'),JSON.stringify(summary,null,2)+'\n');
console.log(JSON.stringify({wins,cpp:cases.filter(c => c.textmate === 'cpp'),largestRawIncrease:cases.sort((a,b) => Math.max(...b.pairedHtmlChangePercent)-Math.max(...a.pairedHtmlChangePercent)).slice(0,6)},null,2));
