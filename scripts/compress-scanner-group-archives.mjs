#!/usr/bin/env node
// Optional CDN size check, separate from all Rust timing processes.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { brotliCompressSync, brotliDecompressSync, constants } from "node:zlib";
import { performance } from "node:perf_hooks";

const [artifacts, output, ...requestedVariants] = process.argv.slice(2);
if (!artifacts || !output) throw new Error("Usage: compress-scanner-group-archives.mjs ARTIFACTS OUTPUT_JSON");
const cases = ["cpp", "scss", "java", "json", "rust", "html", "astro", "tsx", "c", "php"];
const rows = [];
for (const corpus of cases) {
  const programs = readFileSync(join(artifacts, `${corpus}.programs`));
  for (const variant of (requestedVariants.length ? requestedVariants : ["eager", "nfa", "dfa", "dense"])) {
    const groups = variant === "eager" ? Buffer.alloc(0) : readFileSync(join(artifacts, `${corpus}.${variant}`));
    const prefix = Buffer.alloc(4);
    prefix.writeUInt32LE(programs.length);
    const payload = Buffer.concat([prefix, programs, groups]);
    const start = performance.now();
    const compressed = brotliCompressSync(payload, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_GENERIC },
    });
    const compressMs = performance.now() - start;
    if (!brotliDecompressSync(compressed).equals(payload)) throw new Error("Brotli round trip differs");
    const decodeMs = [];
    for (let i = 0; i < 9; i++) {
      const startDecode = performance.now();
      const decoded = brotliDecompressSync(compressed);
      decodeMs.push(performance.now() - startDecode);
      if (decoded.length !== payload.length) throw new Error("Wrong decoded size");
    }
    rows.push({ corpus, variant, node: process.version, rawBytes: payload.length,
      payloadSha256: createHash("sha256").update(payload).digest("hex"),
      brotli11Bytes: compressed.length, compressMs, decodeMs,
      decodeMedianMs: [...decodeMs].sort((a, b) => a - b)[4] });
    writeFileSync(output, `${JSON.stringify(rows, null, 2)}\n`);
    process.stdout.write(`${corpus}/${variant}: ${compressed.length} Brotli bytes\n`);
  }
}
