// Run against a Ferriki checkout built with the separate capture branch.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [ferrikiPath, tracePath] = process.argv.slice(2);
if (!ferrikiPath || !tracePath)
  throw new Error('Usage: node capture.mjs FERRIKI_ROOT TRACE.jsonl');
const root = resolve(ferrikiPath);
const { loadFerrikiNativeBinding } = await import(pathToFileURL(join(root, 'node/ferriki/native.mjs')));
const { createEngine, loadCases, loadCorpus, sha256, theme } = await import(pathToFileURL(join(root, 'node/scripts/tiobe-benchmark.mjs')));
const language = loadCorpus('curated').languages.find(entry => entry.textmate === 'java');
const { code, ...workload } = loadCases(language, ['large'], 'curated')[0];
const oracle = await createEngine('shiki-wasm', language);
const expected = oracle.html(code);
oracle.dispose();
const highlighter = loadFerrikiNativeBinding().createHighlighter(JSON.stringify({ standardAssetRoot: join(root, 'node/ferriki/assets/shiki') }));
highlighter.loadStandardGrammar('java');
highlighter.loadStandardTheme(theme);
writeFileSync(tracePath, '');
let actual;
try {
  process.env.FERRONI_CPP_TRACE = resolve(tracePath);
  actual = highlighter.codeToHtml(code, JSON.stringify({ lang: 'java', theme }));
} finally {
  delete process.env.FERRONI_CPP_TRACE;
  highlighter.dispose();
}
assert.equal(actual, expected, 'Exact HTML parity with Shiki WASM');
console.log(JSON.stringify({ workload, htmlSha256: sha256(actual), documentSha256: sha256(code), nativeBuild: JSON.parse(readFileSync(join(root, 'node/ferriki/.benchmark-build.json'), 'utf8')) }, null, 2));
