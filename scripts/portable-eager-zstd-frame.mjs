// Single-frame preflight for the portable package profile (RFC 8878, 3.1.1.1-2).
// libzstd still validates the compressed content and its optional checksum.
export function checkFrame(encoded, expectedBytes) {
  let position = 0;
  function take(size) {
    if (size > encoded.length - position) throw new Error("Truncated Zstd package frame");
    const start = position;
    position += size;
    return start;
  }
  if (encoded.readUInt32LE(take(4)) !== 0xfd2fb528) throw new Error("Unsupported Zstd frame magic");
  const flags = encoded[take(1)];
  // The package producer uses neither dictionaries nor reserved header bits.
  if (flags & 0x1b) throw new Error("Unsupported Zstd package frame flags");
  const single = (flags & 0x20) !== 0;
  let window;
  if (!single) {
    const descriptor = encoded[take(1)];
    const base = 2 ** (10 + (descriptor >>> 3));
    window = base + (base / 8) * (descriptor & 7);
  }
  const contentBytes = [single ? 1 : 0, 2, 4, 8][flags >>> 6];
  if (!contentBytes) throw new Error("Zstd package must declare its content size");
  const start = take(contentBytes);
  const contentSize = contentBytes === 8
    ? encoded.readBigUInt64LE(start)
    : BigInt(encoded.readUIntLE(start, contentBytes) + (contentBytes === 2 ? 256 : 0));
  window ??= Number(contentSize);
  if (window > 8 * 1024 * 1024) throw new RangeError("Zstd frame window exceeds 8 MiB");
  if (contentSize !== BigInt(expectedBytes)) throw new Error("Zstd frame content size differs from manifest");
  let last;
  do {
    const block = encoded.readUIntLE(take(3), 3);
    last = (block & 1) !== 0;
    const kind = (block >>> 1) & 3;
    const size = block >>> 3;
    if (kind === 3 || size > Math.min(window, 128 * 1024)) throw new Error("Invalid Zstd package block");
    take(kind === 1 ? 1 : size);
  } while (!last);
  if (flags & 4) take(4);
  if (position !== encoded.length) throw new Error("Zstd package must contain exactly one frame");
}
