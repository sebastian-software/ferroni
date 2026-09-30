// Reproduce the fixed-source comparison with Ferriki's existing report gates.
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [ferrikiPath, baselinePath, candidatePath, outputPath] = process.argv.slice(2);
assert.ok(outputPath, 'Usage: node summarize-public.mjs FERRIKI_ROOT BASELINE.json[.gz] CANDIDATE.json[.gz] SUMMARY.json');
const { compareReports, readReport } = await import(pathToFileURL(join(resolve(ferrikiPath), 'node/scripts/tiobe-benchmark.mjs')));
const baseline = readReport(baselinePath);
const candidate = readReport(candidatePath);
for (const report of [baseline, candidate]) {
  assert.equal(report.languages.length, 20);
  assert.equal(report.revision.status, '');
  for (const row of report.languages) {
    assert.equal(row.status, 'measured', row.name);
    assert.equal(row.cases.length, 2);
    for (const entry of row.cases) {
      for (const [id, result] of Object.entries(entry.results)) {
        if (id === 'prism' && !row.prism) {
          assert.equal(result.status, 'unsupported');
          continue;
        }
        assert.equal(result.status, 'ok', `${row.name}/${entry.size}/${id}`);
        assert.equal(result.validation.sourcePreserved, true);
        if (id !== 'prism') assert.deepEqual(result.validation.referenceParity, { tokens: true, html: true });
      }
    }
  }
}
const comparison = compareReports(baseline, candidate);
assert.equal(comparison.comparisons.length, 80);
assert.deepEqual(comparison.excluded, []);
writeFileSync(outputPath, JSON.stringify({
  ...comparison,
  boundary: 'One fresh full control/candidate pair; retain raw samples and comparator movements when interpreting small changes',
  controlMeasuredAt: baseline.measuredAt,
  candidateMeasuredAt: candidate.measuredAt,
  nativeBuilds: { baseline: baseline.nativeBuild, candidate: candidate.nativeBuild },
}, null, 2) + '\n');
console.log(JSON.stringify({ comparisons: 80, excluded: 0, exactParity: '20 formats, two sizes, all three TextMate engines' }));
