import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { constants, zstdCompressSync, zstdDecompressSync } from "node:zlib";
import { createPackage, decodePackage, splitPayload } from "./portable-eager-package.mjs";

// Transport fixtures only. The captured-corpus smoke run validates real Rust archives.
const programs = Buffer.from("program bytes");
const groups = Buffer.from("prepared group bytes");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const fixture = createPackage(programs, groups);

test("compressed assets and already-decoded HTTP payloads preserve both archives", () => {
  const decoded = decodePackage(fixture.encoded, fixture.manifest);
  assert.deepEqual(decoded, { programs, groups });
  const payload = zstdDecompressSync(fixture.encoded);
  assert.deepEqual(splitPayload(payload, fixture.manifest), decoded);
  assert.equal(splitPayload(payload, fixture.manifest).programs.buffer, payload.buffer);
  assert.equal(fixture.manifest.compressionLevel, 19);
  assert.equal(fixture.manifest.windowLog, 23);
});

test("compressed and decoded corruption or truncation is rejected", () => {
  const corrupt = Buffer.from(fixture.encoded);
  corrupt[corrupt.length - 1] ^= 1;
  assert.throws(() => decodePackage(corrupt, fixture.manifest), /SHA-256 mismatch/);
  assert.throws(() => decodePackage(corrupt.subarray(1), fixture.manifest), /length or SHA-256 mismatch/);
  const payload = zstdDecompressSync(fixture.encoded);
  payload[payload.length - 1] ^= 1;
  assert.throws(() => splitPayload(payload, fixture.manifest), /SHA-256 mismatch/);
  // Even a matching outer digest must not disable Zstd's own frame checksum.
  assert.throws(() => decodePackage(corrupt, {
    ...fixture.manifest, compressedSha256: sha256(corrupt),
  }), { code: "ZSTD_error_checksum_wrong" });
});

test("the program boundary must match the decoded prefix", () => {
  const payload = zstdDecompressSync(fixture.encoded);
  payload.writeUInt32LE(programs.length + 1);
  assert.throws(() => splitPayload(payload, {
    ...fixture.manifest, payloadSha256: sha256(payload),
  }), /program length prefix mismatch/);
});

test("unsupported manifests and caller size limits fail before decoding", () => {
  assert.throws(() => decodePackage(fixture.encoded, { ...fixture.manifest, version: 2 }), /Unsupported/);
  assert.throws(() => decodePackage(fixture.encoded, { ...fixture.manifest, groupBytes: -1 }), /Invalid/);
  assert.throws(() => decodePackage(Buffer.alloc(0), fixture.manifest, {
    maxPayloadBytes: fixture.manifest.payloadBytes - 1,
  }), /oversized/);
  assert.throws(() => createPackage(programs, groups, { maxPayloadBytes: 6 }), /oversized/);
  // The frame's declared output length must agree with the manifest too.
  assert.throws(() => decodePackage(fixture.encoded, {
    ...fixture.manifest,
    groupBytes: fixture.manifest.groupBytes - 1,
    payloadBytes: fixture.manifest.payloadBytes - 1,
  }), /content size differs from manifest/);
});

test("frames with a window above the HTTP limit are rejected", () => {
  const largePrograms = Buffer.alloc(9 * 1024 * 1024, 65);
  const { encoded, manifest } = createPackage(largePrograms, groups);
  const payload = zstdDecompressSync(encoded);
  const oversizedFrame = zstdCompressSync(payload, {
    params: {
      [constants.ZSTD_c_compressionLevel]: 1,
      [constants.ZSTD_c_windowLog]: 24,
    },
  });
  assert.throws(() => decodePackage(oversizedFrame, {
    ...manifest, compressedBytes: oversizedFrame.length, compressedSha256: sha256(oversizedFrame),
  }), { name: "RangeError", message: "Zstd frame window exceeds 8 MiB" });
  assert.deepEqual(decodePackage(encoded, manifest), { programs: largePrograms, groups });
});

// A raw block gives these frame-header tests stable bytes independently of
// the compressor's choice of header fields. libzstd checks the valid fixtures.
function rawFrame(payload, sizeBytes, windowDescriptor) {
  const separateWindow = windowDescriptor !== undefined;
  const frame = Buffer.alloc(5 + Number(separateWindow) + sizeBytes + 3 + payload.length);
  frame.writeUInt32LE(0xfd2fb528);
  frame[4] = ([1, 2, 4, 8].indexOf(sizeBytes) << 6) | (separateWindow ? 0 : 0x20);
  let position = 5;
  if (separateWindow) frame[position++] = windowDescriptor;
  if (sizeBytes === 8) frame.writeBigUInt64LE(BigInt(payload.length), position);
  else frame.writeUIntLE(payload.length - (sizeBytes === 2 ? 256 : 0), position, sizeBytes);
  position += sizeBytes;
  frame.writeUIntLE((payload.length << 3) | 1, position, 3);
  payload.copy(frame, position + 3);
  return frame;
}

function withFrame(manifest, encoded) {
  return { ...manifest, compressedBytes: encoded.length, compressedSha256: sha256(encoded) };
}

test("all four frame content-size widths preserve both archives", () => {
  for (const width of [1, 2, 4, 8]) {
    const input = width === 2 ? Buffer.alloc(260, 65) : programs;
    const { encoded, manifest } = createPackage(input, groups);
    const payload = zstdDecompressSync(encoded);
    const frame = rawFrame(payload, width);
    assert.deepEqual(zstdDecompressSync(frame), payload);
    assert.deepEqual(decodePackage(frame, withFrame(manifest, frame)), { programs: input, groups });
  }
});

test("a tiny payload cannot hide a frame window above the limit", () => {
  const payload = zstdDecompressSync(fixture.encoded);
  const frame = rawFrame(payload, 4, 14 << 3); // A 16 MiB window.
  assert.deepEqual(zstdDecompressSync(frame), payload);
  // Some native one-shot paths skip the configured window limit when output fits.
  assert.throws(() => decodePackage(frame, withFrame(fixture.manifest, frame)), /window exceeds/);
});

test("malformed blocks and frames without a content size fail preflight", () => {
  const payload = zstdDecompressSync(fixture.encoded);
  const raw = rawFrame(payload, 1);
  // This fixture has four magic bytes, one descriptor and one size byte.
  const blockStart = 6;
  const reserved = Buffer.from(raw);
  reserved.writeUIntLE(1 | (3 << 1), blockStart, 3);
  assert.throws(() => decodePackage(reserved, withFrame(fixture.manifest, reserved)), /Invalid Zstd package block/);
  const oversized = Buffer.from(raw);
  oversized.writeUIntLE(((payload.length + 1) << 3) | 1, blockStart, 3);
  assert.throws(() => decodePackage(oversized, withFrame(fixture.manifest, oversized)), /Invalid Zstd package block/);
  const unfinished = Buffer.from(raw);
  unfinished[blockStart] &= ~1;
  assert.throws(() => decodePackage(unfinished, withFrame(fixture.manifest, unfinished)), /Truncated/);
  const truncated = raw.subarray(0, raw.length - 1);
  assert.throws(() => decodePackage(truncated, withFrame(fixture.manifest, truncated)), /Truncated/);
  const unknownSize = Buffer.from(raw);
  unknownSize[4] = 0; // The old size byte now serves as the window descriptor.
  assert.deepEqual(zstdDecompressSync(unknownSize), payload);
  assert.throws(() => decodePackage(unknownSize, withFrame(fixture.manifest, unknownSize)), /must declare its content size/);
});

test("frame truncation, trailing data, concatenation and unsupported headers are rejected", () => {
  const encoded = fixture.encoded;
  for (const frame of [encoded.subarray(0, 4), encoded.subarray(0, encoded.length - 1)]) {
    assert.throws(() => decodePackage(frame, withFrame(fixture.manifest, frame)), /Truncated/);
  }
  for (const tail of [Buffer.from([0]), zstdCompressSync(Buffer.alloc(0))]) {
    const frame = Buffer.concat([encoded, tail]);
    assert.throws(() => decodePackage(frame, withFrame(fixture.manifest, frame)), /exactly one frame/);
  }
  const skippable = Buffer.alloc(8);
  skippable.writeUInt32LE(0x184d2a50);
  assert.throws(() => decodePackage(skippable, withFrame(fixture.manifest, skippable)), /frame magic/);
  for (const flag of [1, 8, 16]) {
    const frame = Buffer.from(encoded);
    frame[4] |= flag;
    assert.throws(() => decodePackage(frame, withFrame(fixture.manifest, frame)), /frame flags/);
  }
});

test("CLI round trip and failed verification have observable filesystem behavior", (t) => {
  const root = mkdtempSync(join(tmpdir(), "ferroni-zstd-package-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const cli = fileURLToPath(new URL("./portable-eager-package.mjs", import.meta.url));
  const run = (...args) => spawnSync(process.execPath, [cli, ...args], { encoding: "utf8" });
  writeFileSync(join(root, "test.programs"), programs);
  writeFileSync(join(root, "test.portable"), groups);
  const packages = join(root, "packages");
  const output = join(root, "consumer");
  const packed = run("pack", root, "test", packages);
  assert.equal(packed.status, 0, packed.stderr);
  assert.equal(JSON.parse(packed.stdout).compressionLevel, 19);
  const unpacked = run("unpack", packages, "test", output);
  assert.equal(unpacked.status, 0, unpacked.stderr);
  assert.deepEqual(readFileSync(join(output, "test.programs")), programs);
  assert.deepEqual(readFileSync(join(output, "test.portable")), groups);
  writeFileSync(join(packages, "test.eager.zst"), Buffer.from("damaged"));
  const rejectedOutput = join(root, "rejected");
  const rejected = run("unpack", packages, "test", rejectedOutput);
  assert.equal(rejected.status, 1);
  assert.match(rejected.stderr, /mismatch/);
  assert.equal(existsSync(rejectedOutput), false);
  assert.equal(run("pack", root, "../test", output).status, 1);
});
