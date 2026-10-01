// Records the scanner calls Shiki makes while it highlights one document, as
// a trace in the format of benches/cpp_scanner/trace.json: the patterns of
// every scanner in priority order, the subject strings, and per call the
// scanner, subject, UTF-16 start, find options, winning pattern and captures.
//
//   node capture.mjs <shiki language> <document> <copies> <trace.json>
//
// Shiki runs with its Oniguruma (WASM) engine, the reference the replays are
// validated against. Results are read from the engine's raw output so that a
// capture group that did not participate is recorded as [0, 0], as in the
// existing traces; Shiki itself maps it to an offset.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";

const [lang, documentPath, copies, tracePath] = process.argv.slice(2);
if (!lang || !documentPath || !/^\d+$/.test(copies ?? "") || !tracePath) {
  throw new Error("Usage: node capture.mjs <language> <document> <copies> <trace.json>");
}

const UNSET = 0xffffffff;
const scanners = [];
const subjects = [];
const subjectIds = new Map();
const calls = [];

function subjectId(string) {
  let id = subjectIds.get(string);
  if (id === undefined) {
    id = subjects.push(typeof string === "string" ? string : string.content) - 1;
    subjectIds.set(string, id);
  }
  return id;
}

const inner = await createOnigurumaEngine(import("shiki/wasm"));
const engine = {
  createString: (content) => inner.createString(content),
  createScanner(patterns) {
    const scanner = inner.createScanner(patterns);
    const id = scanners.push(patterns.map((p) => (typeof p === "string" ? p : p.source))) - 1;
    // Same call into the WASM binding as OnigScanner#_findNextMatchSync,
    // keeping the raw capture offsets for the trace.
    scanner._findNextMatchSync = function (string, start, debugCall, options) {
      const binding = this._onigBinding;
      const resultPtr = binding.findNextOnigScannerMatch(
        this._ptr,
        string.id,
        string.ptr,
        string.utf8Length,
        string.convertUtf16OffsetToUtf8(start),
        options,
      );
      const call = [id, subjectId(string), start, options, null, []];
      calls.push(call);
      if (resultPtr === 0) return null;
      let offset = resultPtr / 4;
      const index = binding.HEAPU32[offset++];
      const count = binding.HEAPU32[offset++];
      const captureIndices = [];
      const recorded = [];
      for (let i = 0; i < count; i++) {
        const rawStart = binding.HEAPU32[offset++];
        const rawEnd = binding.HEAPU32[offset++];
        const begin = string.convertUtf8OffsetToUtf16(rawStart);
        const end = string.convertUtf8OffsetToUtf16(rawEnd);
        captureIndices.push({ start: begin, end, length: end - begin });
        recorded.push(rawStart === UNSET ? [0, 0] : [begin, end]);
      }
      call[4] = index;
      call[5] = recorded;
      return { index, captureIndices };
    };
    return scanner;
  },
};

const source = readFileSync(documentPath, "utf8");
const code = source.repeat(Number(copies));
const highlighter = await createHighlighterCore({
  engine,
  langs: [import(`shiki/langs/${lang}.mjs`)],
  themes: [import("shiki/themes/github-dark.mjs")],
});
highlighter.codeToTokensBase(code, { lang, theme: "github-dark" });

// The scanners with the most calls, for focused group benchmarks.
const perScanner = new Map();
for (const [scanner] of calls) perScanner.set(scanner, (perScanner.get(scanner) ?? 0) + 1);
const hot = [...perScanner].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([scanner]) => scanner);

const trace = { format_version: 1, scanners, subjects, calls, hot_groups: hot };
writeFileSync(tracePath, JSON.stringify(trace));
const versions = JSON.parse(
  readFileSync(new URL("node_modules/shiki/package.json", import.meta.url), "utf8"),
).version;
console.log(
  JSON.stringify({
    lang,
    shiki: versions,
    document: documentPath.split("/").pop(),
    document_sha256: createHash("sha256").update(source).digest("hex"),
    copies: Number(copies),
    bytes: Buffer.byteLength(code),
    lines: code.split("\n").length,
    calls: calls.length,
    scanners: scanners.length,
    subjects: subjects.length,
    hot_groups: hot.map((scanner) => ({ id: scanner, calls: perScanner.get(scanner) })),
  }),
);
