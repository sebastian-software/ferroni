#!/usr/bin/env node
// Standalone codec experiment; it does not change Ferroni's archive format.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { performance } from "node:perf_hooks";
import {
  brotliCompressSync, brotliDecompressSync,
  zstdCompressSync, zstdDecompressSync, constants,
} from "node:zlib";

const cases = ["cpp", "scss", "java", "json", "rust", "html", "astro", "tsx", "c", "php"];
const codecs = process.env.PORTABLE_CODECS?.split(",") ?? ["brotli11", "zstd3", "zstd9", "zstd19"];
const [command, artifacts, cache, name, repetitionText] = process.argv.slice(2);
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
function validCodec(codec) {
  return codec === "brotli11" || /^zstd\d+(?:w23)?$/.test(codec) && Number(codec.match(/^zstd(\d+)/)[1]) >= 1 && Number(codec.match(/^zstd(\d+)/)[1]) <= 22;
}
if (!codecs.every(validCodec)) throw new Error("Unsupported codec selection");
function payload(name) {
  const programs = readFileSync(join(artifacts, `${name}.programs`));
  const groups = readFileSync(join(artifacts, `${name}.portable`));
  const length = Buffer.alloc(4);
  length.writeUInt32LE(programs.length);
  return Buffer.concat([length, programs, groups]);
}
function decode(codec, bytes) {
  return codec === "brotli11" ? brotliDecompressSync(bytes) : zstdDecompressSync(bytes);
}
function prepare(name, codec, isolated) {
  if (!cases.includes(name) || !validCodec(codec)) throw new Error("Unsupported input");
  mkdirSync(cache, { recursive: true });
  const raw = payload(name);
  const reference = process.env.PORTABLE_CODEC_REFERENCE;
  if (reference && existsSync(join(reference, "preparation.json"))) {
    const previous = JSON.parse(readFileSync(join(reference, "preparation.json"), "utf8"))
      .find(row => row.corpus === name && row.codec === codec);
    if (previous && (codec === "brotli11" || previous.zstdFrameChecksum === true)
        && previous.payloadSha256 === sha(raw)
        && JSON.stringify(previous.versions) === JSON.stringify(process.versions)) {
      const encoded = readFileSync(join(reference, `${name}.${codec}`));
      if (sha(encoded) !== previous.compressedSha256 || !decode(codec, encoded).equals(raw))
        throw new Error("Frozen reference differs");
      writeFileSync(join(cache, `${name}.${codec}`), encoded);
      return { ...previous, reusedFrom: reference,
        encodingProvenance: "Frozen encoded bytes; historical single encode timing; revalidated by hash and full round trip" };
    }
  }
  const start = performance.now();
  const encoded = codec === "brotli11"
    ? brotliCompressSync(raw, { params: {
      [constants.BROTLI_PARAM_QUALITY]: 11,
      [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_GENERIC,
    } })
    : zstdCompressSync(raw, { params: {
      [constants.ZSTD_c_compressionLevel]: Number(codec.match(/^zstd(\d+)/)[1]),
      ...(codec.endsWith("w23") ? { [constants.ZSTD_c_windowLog]: 23 } : {}),
      [constants.ZSTD_c_checksumFlag]: 1,
    } });
  const compressMs = performance.now() - start;
  const encodeMaxRssKiB = process.resourceUsage().maxRSS;
  if (!decode(codec, encoded).equals(raw)) throw new Error("Round trip differs");
  writeFileSync(join(cache, `${name}.${codec}`), encoded);
  return { corpus: name, codec, rawBytes: raw.length, compressedBytes: encoded.length,
    payloadSha256: sha(raw), compressedSha256: sha(encoded), compressMs,
    encodeMaxRssKiB, encodingProcessIsolated: isolated,
    encodingProvenance: "Fresh single encode; RSS includes payload construction and compression, captured before round-trip decode",
    versions: process.versions, zstdFrameChecksum: codec !== "brotli11",
    zstdWindowLogCap: codec.endsWith("w23") ? 23 : null };
}
if (command === "prepare-one") {
  const row = prepare(name, repetitionText, true);
  writeFileSync(join(cache, `${name}.${repetitionText}.meta.json`), JSON.stringify(row, null, 2) + "\n");
  process.stdout.write(JSON.stringify(row) + "\n");
} else if (command === "prepare") {
  const rows = [];
  for (const name of cases) {
    for (const codec of codecs) {
      rows.push(prepare(name, codec, false));
      writeFileSync(join(cache, "preparation.json"), JSON.stringify(rows, null, 2) + "\n");
    }
    process.stdout.write(`Prepared ${name}\n`);
  }
} else if (command === "measure") {
  const repetition = Number(repetitionText);
  const raw = payload(name);
  const prepared = JSON.parse(readFileSync(join(cache, "preparation.json"), "utf8"));
  const offset = repetition % codecs.length;
  const ordered = codecs.slice(offset).concat(codecs.slice(0, offset));
  const rows = [];
  for (const codec of ordered) {
    const encoded = readFileSync(join(cache, `${name}.${codec}`));
    const reference = prepared.find(row => row.corpus === name && row.codec === codec);
    if (sha(raw) !== reference.payloadSha256 || sha(encoded) !== reference.compressedSha256)
      throw new Error("Input hash changed");
    for (let i = 0; i < 3; i++) if (!decode(codec, encoded).equals(raw)) throw new Error("Warmup differs");
    const samplesMs = [];
    for (let i = 0; i < 15; i++) {
      const start = performance.now();
      const decoded = decode(codec, encoded);
      const elapsed = performance.now() - start;
      if (!decoded.equals(raw)) throw new Error("Decode differs");
      samplesMs.push(elapsed);
    }
    rows.push({ corpus: name, codec, repetition: repetition + 1, samplesMs,
      decodeMaxRssKiB: process.resourceUsage().maxRSS,
      decodingProcessIsolated: codecs.length === 1 });
  }
  process.stdout.write(JSON.stringify(rows) + "\n");
} else throw new Error("Usage: compare-portable-codecs.mjs prepare|prepare-one|measure ARTIFACTS CACHE [CORPUS REPETITION|CODEC]");
