// Run from a Ferriki checkout built against the diagnostic Ferroni branch.
import { pathToFileURL } from 'node:url';
import { resolve, join } from 'node:path';
import { writeFileSync } from 'node:fs';
const [ferrikiPath, tracePath] = process.argv.slice(2);
if (!ferrikiPath || !tracePath) throw new Error('Usage: node capture.mjs FERRIKI_ROOT TRACE.jsonl');
const root = resolve(ferrikiPath);
const { loadFerrikiNativeBinding } = await import(pathToFileURL(join(root, 'node/ferriki/native.mjs')));
const { loadCases, manifest, theme } = await import(pathToFileURL(join(root, 'node/scripts/tiobe-benchmark.mjs')));
const language = manifest.languages.find(x => x.textmate === 'cpp');
const { code } = loadCases(language, ['large'])[0];
const highlighter = loadFerrikiNativeBinding().createHighlighter(JSON.stringify({standardAssetRoot: join(root, 'node/ferriki/assets/shiki')}));
highlighter.loadStandardGrammar('cpp');
highlighter.loadStandardTheme(theme);
writeFileSync(tracePath, '');
process.env.FERRONI_CPP_TRACE = resolve(tracePath);
highlighter.codeToHtml(code, JSON.stringify({lang: 'cpp', theme}));
delete process.env.FERRONI_CPP_TRACE;
highlighter.dispose();
