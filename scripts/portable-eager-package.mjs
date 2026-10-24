#!/usr/bin/env node
// Prototype transport wrapper; the Rust archive formats remain unchanged.
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { constants, zstdCompressSync, zstdDecompressSync } from "node:zlib";
import { checkFrame } from "./portable-eager-zstd-frame.mjs";

const PROFILE = Object.freeze({
  format: "ferroni-portable-eager", version: 1,
  encoding: "zstd", compressionLevel: 19, windowLog: 23,
});
export const DEFAULT_MAX_PAYLOAD_BYTES = 256 * 1024 * 1024;
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

function checkLengths(programBytes, groupBytes, maxPayloadBytes) {
  if (!Number.isSafeInteger(maxPayloadBytes) || maxPayloadBytes < 6) {
    throw new Error("Invalid payload limit");
  }
  const payloadBytes = 4 + programBytes + groupBytes;
  if (
    !Number.isSafeInteger(programBytes) || programBytes <= 0 || programBytes > 0xffffffff ||
    !Number.isSafeInteger(groupBytes) || groupBytes <= 0 ||
    !Number.isSafeInteger(payloadBytes) || payloadBytes > maxPayloadBytes
  ) {
    throw new Error("Invalid or oversized package payload");
  }
  return payloadBytes;
}

function checkManifest(manifest, maxPayloadBytes) {
  if (!manifest || Object.entries(PROFILE).some(([field, value]) => manifest[field] !== value)) {
    throw new Error("Unsupported portable package manifest");
  }
  if (
    checkLengths(manifest.programBytes, manifest.groupBytes, maxPayloadBytes) !== manifest.payloadBytes ||
    !Number.isSafeInteger(manifest.compressedBytes) || manifest.compressedBytes <= 0
  ) {
    throw new Error("Invalid manifest length");
  }
  for (const field of ["payloadSha256", "compressedSha256"]) {
    if (typeof manifest[field] !== "string" || !/^[a-f0-9]{64}$/.test(manifest[field])) {
      throw new Error(`Invalid manifest digest: ${field}`);
    }
  }
}

export function createPackage(programs, groups, { maxPayloadBytes = DEFAULT_MAX_PAYLOAD_BYTES } = {}) {
  // Check the size before allocating or compressing the combined payload.
  const payloadBytes = checkLengths(programs.length, groups.length, maxPayloadBytes);
  const prefix = Buffer.alloc(4);
  prefix.writeUInt32LE(programs.length);
  const payload = Buffer.concat([prefix, programs, groups]);
  const encoded = zstdCompressSync(payload, {
    params: {
      [constants.ZSTD_c_compressionLevel]: 19,
      [constants.ZSTD_c_windowLog]: 23,
      [constants.ZSTD_c_checksumFlag]: 1,
    },
  });
  const manifest = {
    ...PROFILE,
    programBytes: programs.length, groupBytes: groups.length, payloadBytes,
    payloadSha256: sha256(payload), compressedBytes: encoded.length,
    compressedSha256: sha256(encoded),
  };
  return { encoded, manifest };
}

// Use this for an HTTP response that the client has already decompressed.
// The returned views retain the payload buffer; Rust still validates both archives.
export function splitPayload(payload, manifest, { maxPayloadBytes = DEFAULT_MAX_PAYLOAD_BYTES } = {}) {
  checkManifest(manifest, maxPayloadBytes);
  if (payload.length !== manifest.payloadBytes || sha256(payload) !== manifest.payloadSha256) {
    throw new Error("Package payload length or SHA-256 mismatch");
  }
  if (payload.readUInt32LE(0) !== manifest.programBytes) {
    throw new Error("Package program length prefix mismatch");
  }
  return {
    programs: payload.subarray(4, 4 + manifest.programBytes),
    groups: payload.subarray(4 + manifest.programBytes),
  };
}

// Use this for an explicitly downloaded .zst file, without HTTP content decoding.
export function decodePackage(encoded, manifest, { maxPayloadBytes = DEFAULT_MAX_PAYLOAD_BYTES } = {}) {
  checkManifest(manifest, maxPayloadBytes);
  if (encoded.length !== manifest.compressedBytes || sha256(encoded) !== manifest.compressedSha256) {
    throw new Error("Compressed package length or SHA-256 mismatch");
  }
  checkFrame(encoded, manifest.payloadBytes);
  const payload = zstdDecompressSync(encoded, {
    maxOutputLength: manifest.payloadBytes,
    // One spare byte avoids Node allocating another chunk when this one fills.
    chunkSize: Math.max(64, manifest.payloadBytes + 1),
    params: { [constants.ZSTD_d_windowLogMax]: 23 },
  });
  return splitPayload(payload, manifest, { maxPayloadBytes });
}

function main(args) {
  const [command, input, name, output] = args;
  if (
    args.length !== 4 || !["pack", "unpack"].includes(command) ||
    !/^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(name)
  ) {
    throw new Error("Usage: portable-eager-package.mjs pack|unpack INPUT_DIR NAME OUTPUT_DIR");
  }
  if (command === "pack") {
    const { encoded, manifest } = createPackage(
      readFileSync(join(input, `${name}.programs`)),
      readFileSync(join(input, `${name}.portable`)),
    );
    // Verify the actual frame before publishing either output file.
    decodePackage(encoded, manifest);
    mkdirSync(output, { recursive: true });
    writeFileSync(join(output, `${name}.eager.zst`), encoded);
    writeFileSync(join(output, `${name}.eager.json`), JSON.stringify(manifest, null, 2) + "\n");
    process.stdout.write(JSON.stringify({ name, ...manifest }) + "\n");
  } else {
    const manifest = JSON.parse(readFileSync(join(input, `${name}.eager.json`), "utf8"));
    const { programs, groups } = decodePackage(readFileSync(join(input, `${name}.eager.zst`)), manifest);
    mkdirSync(output, { recursive: true });
    writeFileSync(join(output, `${name}.programs`), programs);
    writeFileSync(join(output, `${name}.portable`), groups);
    process.stdout.write(JSON.stringify({ name, programBytes: programs.length, groupBytes: groups.length }) + "\n");
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
