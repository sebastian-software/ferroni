// Replays the captured Shiki scanner calls (benches/<lang>_scanner/trace.json)
// through Shiki's JavaScript regex engine: oniguruma-to-es translations run by
// V8's RegExp, with the engine's defaults. Every call is checked against the
// captured result first; a pattern the engine rejects or a call it answers
// differently makes the replay "unsupported" instead of timed.
//
//   node replay.mjs <cpp|java|scss> <document|group_N> <validate|measure|profile> [seconds]
//
// Prints one JSON line. Like the Rust replays, scanners are created once and
// every replay gets fresh string identities, so the per-string search cache
// works within a replay but not across replays. For a CPU profile, run with
// `node --cpu-prof`; samples inside `measuredLoop` are the replay.
import { readFileSync } from 'node:fs';
import { createJavaScriptRegexEngine } from '@shikijs/engine-javascript';

const [language, selection, mode, seconds = '5'] = process.argv.slice(2);
if (!['cpp', 'java', 'scss'].includes(language) || !/^(document|group_\d+)$/.test(selection)
    || !['validate', 'measure', 'profile'].includes(mode)) {
  throw new Error('Usage: node replay.mjs <cpp|java|scss> <document|group_N> <validate|measure|profile> [seconds]');
}
const id = `${language}_scanner/${selection}_shiki_js`;
const fixture = JSON.parse(readFileSync(new URL(`../${language}_scanner/trace.json`, import.meta.url), 'utf8'));
const group = selection === 'document' ? null : Number(selection.slice('group_'.length));
const calls = fixture.calls
  .map(([scanner, subject, start, options, index, captures]) => ({ scanner, subject, start, options, index, captures }))
  .filter(call => group === null || call.scanner === group);
const version = JSON.parse(readFileSync(new URL('node_modules/@shikijs/engine-javascript/package.json', import.meta.url), 'utf8')).version;

let emptyCaptureDifferences = 0;
function report(fields) {
  console.log(JSON.stringify({ id, engine: 'shiki_js', engine_version: version, calls: calls.length, empty_capture_differences: emptyCaptureDifferences, ...fields }));
}

const engine = createJavaScriptRegexEngine();
const scanners = new Map();
try {
  for (const call of calls) {
    if (!scanners.has(call.scanner)) scanners.set(call.scanner, engine.createScanner(fixture.scanners[call.scanner]));
  }
} catch (error) {
  report({ status: 'unsupported', reason: `translation: ${error.message}` });
  process.exit(0);
}

const UNSET = 4294967295;
function replay() {
  const strings = fixture.subjects.map(subject => engine.createString(subject));
  let found = 0;
  for (const call of calls) {
    if (scanners.get(call.scanner).findNextMatchSync(strings[call.subject], call.start, call.options)) found++;
  }
  return found;
}

// Validation outside timing. vscode-textmate skips zero-length captures, so an
// empty group and an unset one highlight alike; they compare equal here and
// are counted. The whole match (group 0) must agree exactly.
const highlighted = result => result && [result[0], result[1].map((c, g) => (g > 0 && c[0] === c[1] ? [0, 0] : c))];
{
  const strings = fixture.subjects.map(subject => engine.createString(subject));
  for (const [i, call] of calls.entries()) {
    const match = scanners.get(call.scanner).findNextMatchSync(strings[call.subject], call.start, call.options);
    const actual = match && [match.index, match.captureIndices.map(c => (c.start === UNSET ? [0, 0] : [c.start, c.end]))];
    const expected = call.index === null ? null : [call.index, call.captures];
    if (JSON.stringify(actual) === JSON.stringify(expected)) continue;
    if (JSON.stringify(highlighted(actual)) === JSON.stringify(highlighted(expected))) {
      emptyCaptureDifferences++;
      continue;
    }
    report({ status: 'unsupported', reason: `call ${i} (scanner ${call.scanner}): ${JSON.stringify(actual)}, Shiki ${JSON.stringify(expected)}` });
    process.exit(0);
  }
}
if (mode === 'validate') {
  report({ status: 'ok' });
  process.exit(0);
}

function measuredLoop(deadline, batch, samples) {
  let sink = 0;
  while (process.hrtime.bigint() < deadline) {
    const start = process.hrtime.bigint();
    for (let i = 0; i < batch; i++) sink += replay();
    samples?.push(Number(process.hrtime.bigint() - start) / batch);
  }
  return sink;
}

const ns = s => BigInt(Math.round(s * 1e9));
// One second of warm-up; it also sizes a batch to about a millisecond.
let warm = 0;
const warmStart = process.hrtime.bigint();
while (process.hrtime.bigint() - warmStart < ns(1)) {
  replay();
  warm++;
}
const batch = Math.max(1, Math.round(warm / 1000));
const samples = mode === 'measure' ? [] : null;
measuredLoop(process.hrtime.bigint() + ns(Number(seconds)), batch, samples);
if (mode === 'measure') {
  samples.sort((a, b) => a - b);
  const mean = samples.reduce((sum, x) => sum + x, 0) / samples.length;
  report({ status: 'ok', mean_ns: mean, median_ns: samples[samples.length >> 1], samples: samples.length, batch });
} else {
  report({ status: 'ok' });
}
