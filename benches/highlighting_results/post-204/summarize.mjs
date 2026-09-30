// Reproduce the fixed-source highlighting comparison and candidate ranking.
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const [ferrikiPath, resultPath] = process.argv.slice(2);
assert.ok(resultPath, 'Usage: node summarize.mjs FERRIKI_ROOT RESULT_DIRECTORY');
const root = resolve(resultPath);
const { compareReports, readReport } = await import(pathToFileURL(join(resolve(ferrikiPath), 'node/scripts/tiobe-benchmark.mjs')));
const runs = [1, 2].map(n => readReport(join(root, `public-${n}.json.gz`)));
const oldControl = readReport(join(root, 'previous-control.json.gz'));
const previous = readReport(join(root, 'previous-candidate.json.gz'));
for (const report of runs) {
  assert.equal(report.languages.length, 20);
  assert.equal(report.revision.status, '');
  for (const row of report.languages) {
    assert.equal(row.status, 'measured', row.name);
    assert.equal(row.cases.length, 2);
    for (const entry of row.cases) {
      for (const [id, result] of Object.entries(entry.results)) {
        if (id === 'prism' && !row.prism) { assert.equal(result.status, 'unsupported'); continue; }
        assert.equal(result.status, 'ok', `${row.name}/${entry.size}/${id}`);
        assert.equal(result.validation.sourcePreserved, true);
        if (id !== 'prism') assert.deepEqual(result.validation.referenceParity, { tokens: true, html: true });
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
const languages = runs[0].languages.map((row, index) => {
  const large = runs.map(run => run.languages[index].cases.find(c => c.size === 'large'));
  const engines = Object.fromEntries(Object.entries(large[0].results).filter(([,v]) => v.status === 'ok').map(([id]) => [id, Object.fromEntries(['html', 'tokens'].map(api => [api, large.map(c => c.results[id][api].medianMs)]))]));
  const ratios = Object.fromEntries(['shiki-wasm', 'shiki-js'].map(id => [id, Object.fromEntries(['html', 'tokens'].map(api => [api, large.map(c => c.results.ferriki[api].medianMs / c.results[id][api].medianMs)]))]));
  const bestShiki = Object.fromEntries(['html', 'tokens'].map(api => [api, large.map(c => {
    const id = c.results['shiki-wasm'][api].medianMs < c.results['shiki-js'][api].medianMs ? 'shiki-wasm' : 'shiki-js';
    return { engine: id, medianMs: c.results[id][api].medianMs };
  })]));
  ratios['best-shiki'] = Object.fromEntries(['html', 'tokens'].map(api => [api, large.map((c,n) => c.results.ferriki[api].medianMs / bestShiki[api][n].medianMs)]));
  return { language: row.name, textmate: row.textmate, bytes: large[0].bytes, engines, bestShiki, ferrikiRatios: ratios,
    htmlGapVsBestShikiMs: large.map((c,n) => c.results.ferriki.html.medianMs - bestShiki.html[n].medianMs),
    tokenGapVsBestShikiMs: large.map((c,n) => c.results.ferriki.tokens.medianMs - bestShiki.tokens[n].medianMs) };
});
const wins = Object.fromEntries(['shiki-wasm', 'shiki-js', 'best-shiki'].map(id => [id, Object.fromEntries(['html', 'tokens'].map(api => [api, runs.map((_,n) => languages.filter(row => row.ferrikiRatios[id][api][n] < 1).length)]))]));
writeFileSync(join(root, 'summary.json'), JSON.stringify({
  boundary: 'Two sequential warm public highlighting matrices on merged Ferroni #204; historical comparisons do not isolate host drift',
  primaryMetric: { api: 'html', output: 'inline HTML', comparator: 'min(shiki-wasm, shiki-js)', ranking: 'largest absolute HTML median gap across the two runs' },
  diagnosticApis: ['tokens'],
  unmeasuredProductOutputs: ['HTML with CSS in classes mode'],
  nativeBuilds: runs.map(r => r.nativeBuild), method: runs[0].method, machine: runs[0].machine, versions: runs[0].versions,
  runComparison: checked(runs[0], runs[1]), beforeVsMerged: runs.map(r => checked(oldControl, r)), previousCandidateVsMerged: runs.map(r => checked(previous, r)),
  wins, languages: languages.sort((a,b) => Math.max(...b.htmlGapVsBestShikiMs) - Math.max(...a.htmlGapVsBestShikiMs)),
}, null, 2) + '\n');
console.log(JSON.stringify({ exactParity: '20 formats, two sizes, three TextMate engines', comparisons: '5 pairs × 80, zero exclusions', wins }));
