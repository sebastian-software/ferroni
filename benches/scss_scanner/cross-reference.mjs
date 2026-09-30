// Diagnostic reference only: never changes a grammar or Ferroni runtime.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [ferrikiPath, fixturePath, reportPath, optimizedFixturePath] = process.argv.slice(2);
assert.ok(reportPath, 'Usage: node cross-reference.mjs FERRIKI_ROOT TRACE.json REPORT.json [OPTIMIZED-TRACE.json]');
const packages = join(resolve(ferrikiPath), 'node/node_modules/.pnpm');
const toEsRoot = join(packages, 'oniguruma-to-es@4.3.4/node_modules/oniguruma-to-es');
const parserRoot = join(packages, 'oniguruma-parser@0.12.1/node_modules/oniguruma-parser');
const { toRegExpDetails } = await import(pathToFileURL(join(toEsRoot, 'dist/esm/index.js')));
const { optimize } = await import(pathToFileURL(join(parserRoot, 'dist/optimizer/optimize.js')));
const fixture = JSON.parse(readFileSync(fixturePath, 'utf8'));
const unique = new Map();
fixture.scanners.forEach((patterns, group) => patterns.forEach((pattern, index) => {
  if (!unique.has(pattern)) unique.set(pattern, []);
  unique.get(pattern).push([group, index]);
}));
const records = [];
for (const [original, locations] of unique) {
  const record = { original, locations };
  try {
    record.shikiTranslation = toRegExpDetails(original, {
      global: true, hasIndices: true,
      rules: { allowOrphanBackrefs: true, asciiWordBoundaries: true, captureGroup: true, recursionLimit: 5, singleline: true },
    });
  } catch (error) { record.translationError = error.message; }
  try {
    const result = optimize(original, { rules: { allowOrphanBackrefs: true, captureGroup: true } });
    record.optimizedOniguruma = (result.flags ? `(?${result.flags})` : '') + result.pattern;
    record.changed = record.optimizedOniguruma !== original;
    if (record.changed && optimizedFixturePath) {
      for (const [group, index] of locations) fixture.scanners[group][index] = record.optimizedOniguruma;
    }
  } catch (error) { record.optimizerError = error.message; }
  records.push(record);
}
const report = {
  versions: { onigurumaToEs: '4.3.4', onigurumaParser: '0.12.1' },
  boundary: 'Shiki translation is a reference; optional parser optimizer is a separate experiment and is not enabled by Shiki',
  uniquePatterns: records.length,
  changedPatterns: records.filter(record => record.changed).length,
  translationErrors: records.filter(record => record.translationError).length,
  optimizerErrors: records.filter(record => record.optimizerError).length,
  records,
};
writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n');
if (optimizedFixturePath) writeFileSync(optimizedFixturePath, JSON.stringify(fixture) + '\n');
console.log(JSON.stringify({ ...report, records: undefined }, null, 2));
