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
  // A lying output length is also enforced by the real decompressor.
  assert.throws(() => decodePackage(fixture.encoded, {
    ...fixture.manifest,
    groupBytes: fixture.manifest.groupBytes - 1,
    payloadBytes: fixture.manifest.payloadBytes - 1,
  }), { code: "ERR_BUFFER_TOO_LARGE" });
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
  }), { code: "ZSTD_error_frameParameter_windowTooLarge" });
  assert.deepEqual(decodePackage(encoded, manifest), { programs: largePrograms, groups });
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
