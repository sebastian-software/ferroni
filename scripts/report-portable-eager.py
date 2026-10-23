#!/usr/bin/env python3
"""Report the fork-free bytecode, frozen snapshot, and ordinary Eager matrix."""
import argparse
import importlib.util
import json
from pathlib import Path
import statistics


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=Path)
    parser.add_argument("brotli", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("groups", Path(__file__).with_name("report-scanner-group-archives.py"))
    groups = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(groups)
    summary = groups.summarize(args.run)
    br = json.loads(args.brotli.read_text())
    by = {(r["corpus"], r["variant"]): r for r in summary["rows"]}
    br_by = {(r["corpus"], r["variant"]): r for r in br}
    pairs = []
    for case in groups.CASES:
        for base in ["eager", "prepared", "control"]:
            record = {"corpus": case, "baseline": base, "variant": "portable", "metrics": {}}
            for metric in groups.METRICS:
                before = by[case, base]["timings"][metric]["per_run_mean_ns"]
                after = by[case, "portable"]["timings"][metric]["per_run_mean_ns"]
                changes = [100 * (a / b - 1) for a, b in zip(after, before)]
                record["metrics"][metric] = {"median_percent": statistics.median(changes),
                    "range_percent": [min(changes), max(changes)], "per_run_percent": changes}
            pairs.append(record)
    summary["portable_pairs"] = pairs
    summary["brotli"] = br
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (args.output / "measurements.md").write_text(groups.tables(summary))
    lines = ["# Portable scanner bytecode: local comparison", "",
        "This experiment replaces dependency-private scanner-group snapshots with an owned instruction format and a native interpreter. The producer and consumer were built against unmodified registry releases of regex-automata 0.4.18 and aho-corasick 1.1.5. The old snapshot executable is retained unchanged as the comparison.", "",
        "## All ten corpora", "",
        "Each time is the median of three Criterion process means. Sizes contain the ordinary v2 program archive plus the optional scanner-group image. RSS is the median of three fresh-process peaks, including retained archives, fixture parsing, programs, scanners, scratch, and four document passes.", "",
        "| Corpus | Variant | Load ms | Warm search ms | Cold document ms | Brotli-11 KiB | gzip-9 KiB | Peak RSS MiB |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for case in groups.CASES:
        for variant in ["eager", "prepared", "portable"]:
            row = by[case, variant]
            codec = br_by[case, variant]
            assert codec["payloadSha256"] == row["sizes"]["payload_sha256"]
            t = row["timings"]
            lines.append(f"| {case} | {variant} | {t['load']['median_mean_ns']/1e6:.3f} | {t['search']['median_mean_ns']/1e6:.3f} | {t['cold_document']['median_mean_ns']/1e6:.3f} | {codec['brotli11Bytes']/1024:.1f} | {row['sizes']['gzip9_bytes']/1024:.1f} | {row['memory']['rss_median_bytes']/1048576:.1f} |")
    lines += ["", "## Portable versus both references", "",
        "Negative percentages mean less time. Brackets retain the minimum and maximum paired change across all three repetitions; they are not confidence intervals.", "",
        "| Corpus | Baseline | Load change | Warm search change | Cold document change |",
        "| --- | --- | ---: | ---: | ---: |"]
    for row in pairs:
        if row["baseline"] == "control":
            continue
        def change(metric):
            value = row["metrics"][metric]
            a, b = value["range_percent"]
            return f"{value['median_percent']:+.1f}% [{a:+.1f}, {b:+.1f}]"
        lines.append(f"| {row['corpus']} | {row['baseline']} | {change('load')} | {change('search')} | {change('cold_document')} |")
    lines += ["", "## Decompression is additional work", "",
        "Rust load excludes decompression, file I/O, and transfer. The following sum combines separately measured medians; it is an estimate, not a measured application-startup duration. Codec timings are diagnostic (Node Brotli, CPython gzip); compression ran during validation, separately from the Rust comparison. Network transfer and the actual Ferriki integration remain unmeasured.", "",
        "| Corpus | Variant | Brotli decode ms | Rust load ms | Sum ms |",
        "| --- | --- | ---: | ---: | ---: |"]
    for case in groups.CASES:
        for variant in ["eager", "prepared", "portable"]:
            decode = br_by[case, variant]["decodeMedianMs"]
            load = by[case, variant]["timings"]["load"]["median_mean_ns"] / 1e6
            lines.append(f"| {case} | {variant} | {decode:.3f} | {load:.3f} | {decode + load:.3f} |")
    (args.output / "comparison.md").write_text("\n".join(lines) + "\n")
    native = [by[case, "portable"] for case in groups.CASES]
    gains = [-r["timings"]["load"]["median_change_percent"] for r in native]
    search = [r["timings"]["search"]["median_change_percent"] for r in native]
    repeatable = [r["corpus"] for r in native if r["timings"]["search"]["all_runs_regress_3_percent"]]
    intro = f"""# Fork-free portable Eager prototype

The portable scanner-group image loads with unmodified dependencies and reproduces all {summary['validated_calls_per_variant']:,} recorded calls, including winning patterns and captures. This implementation is **not a merge candidate**: load time falls by {min(gains):.1f}–{max(gains):.1f}% versus ordinary Eager, but warm search takes {min(search):.1f}–{max(search):.1f}% more time (median paired change per corpus). A search regression of at least 3% occurs in every repetition for {len(repeatable)} of ten corpora ({', '.join(repeatable)}).

The [complete three-way comparison](comparison.md) covers load time, compressed size, fresh-process peak RSS, warm search, and the first document. [Paired ranges, the feature control, and raw per-process estimates](measurements.md) retain the variance. [summary.json](summary.json) contains all values, and [evidence.tar.gz](evidence.tar.gz) contains source overrides, provenance, raw samples, and successful checks.

## What runs without forks

The producer exports scanner seeks through public Thompson NFA APIs into Ferroni-owned instructions. `FPRT001` stores little-endian 32-bit instruction operands, byte classes, ordered branches, ASCII assertion IDs, match IDs, and forward/reverse entry points. It contains no Rust pointers, Arc internals, vtables, mutable caches, or machine code.

The consumer validates instruction tags, references, ranges, equivalence classes, lengths, and group identity. It decodes immutable arrays directly and executes them with the [native interpreter](../../../src/dfa_prefilter/portable.rs). Ordered epsilon closure and bounded lazy transition caches implement forward leftmost-first search, reverse start discovery, and overlapping candidate walks. The existing Ferroni VM still decides matches and captures, including constructs approximated by the scanner seek.

This is a scoped scanner-group prototype. Per-pattern literal helpers and search plans still use ordinary Eager preparation. The old snapshot also stores those helpers. Small unused compatibility handles/caches remain in the scanner wrapper and are included in the measurements. The custom runtime's transition caches start empty, as ordinary Eager's hybrid caches do; this is not complete ahead-of-time determinization. Off and Auto ignore the image. Missing or invalid individual groups retain the existing Eager fallback.

The isolated producer **and** consumer resolve regex-automata 0.4.18 and aho-corasick 1.1.5 from crates.io, with checksums in Cargo.lock. No snapshot feature or path patch is active there. The root manifest retains the previous snapshot experiment solely for the historical comparator and existing checks. The [build script](../../../scripts/prepare-portable-eager-comparison.py) removes those patches and snapshot-only feature edges in an isolated source copy, verifies Cargo metadata, and records hashes. The snapshot executable is reused byte-for-byte from the [previous experiment](../prepared-eager-20261023/README.md).

## Measurement contract

- Ten captured grammar corpora: C++, SCSS, Java, JSON, Rust, HTML, Astro, TSX, C, and PHP. Each candidate reproduces all recorded UTF-16 calls and capture arrays before timing and inside each fresh benchmark process.
- Three process repetitions, rotating corpus and variant order. Ordinary Eager and the portable feature control use registry builds; the existing prepared snapshot uses its frozen executable. The control loads the ordinary archive through the portable-feature executable, without a group image.
- Criterion: 40 samples, 300 ms warmup, 1 s target, and 10,000 bootstrap resamples. Load decodes in-memory uncompressed bytes and creates requested scanners and scratch; file I/O, codecs, strings, and teardown are excluded. Warm search follows three document passes and uses fresh string identities prepared outside timing. Cold document includes load and the first replay.
- Sizes include a four-byte program-length prefix, the ordinary program archive, and any group image. gzip uses level 9; Brotli uses quality 11. Both codecs verify round trips and payload SHA-256. Codec timings are separate diagnostics; the comparison includes an explicitly labeled decode-plus-load estimate.
- RSS uses a fresh wrapper and one driver child for each corpus/variant/repetition. It includes fixtures, retained input archives, decoded programs, scanners, construction high-water marks, and four search passes. Internal counters are partial estimates and do not replace RSS.
- macOS arm64 on an M1 Ultra Mac Studio with 64 GB, rustc 1.99.0, release/bench ThinLTO. Time Machine was idle. Core placement, thermals, and unrelated background activity were not controlled. No compilation or compression ran alongside the main Rust timing matrix.

## Interpretation and remaining work

The architecture works without upstream serialization APIs. An independent format also makes Ferroni responsible for a fast interpreter and its correctness. This prototype uses per-state transition allocations and more checks in its byte loops than the existing hybrid implementation; the search regression is real in this matrix and does not preserve ordinary Eager search performance. The native feature control in the measurements separates part of the build/layout effect from the image execution path. The frozen snapshot's small search differences remain subject to process and build variation.

The raw image and decoded arrays are both retained, so smaller mutable caches do not imply a lower process peak. Compressed images are also larger than ordinary program archives. CDN transfer, codec cost, and a local download cache must be considered alongside the in-memory load gain.

The next focused experiment would flatten transition storage and tighten the byte loop, then repeat this matrix. Preparing per-program literal helpers without dependency-private objects is a separate missing part. Keeping the existing hybrid runtime with the older public-API NFA archive is another available no-fork tradeoff; its [earlier measurements](../scanner-group-archives-20261023/README.md) should not be confused with this new interpreter.

The wire version is experimental. Cross-platform execution, complete grammar enumeration, the actual Ferriki startup path, authenticated artifact distribution, and adversarial resource behavior have not been validated. The existing record checksum detects accidental damage; it is not authenticity verification. No new unsafe code was added.

## Validation and reproduction

PASSED in the isolated registry source:

```bash
cargo test --offline --release --features portable-eager-prototype,prefilter-self-check --lib --test scanner_group_prototype --test compat_prefilter_differential --test precompiled_scanner --test scanner_prefilter_regressions -- --test-threads=1
cargo clippy --offline --all-targets --features portable-eager-prototype -- -D warnings
```

The tests report 568 unit tests, three compatibility differential tests (one external-corpus test ignored), two precompiled scanner tests, three archive tests, and one scanner regression test. They include deterministic unseen inputs and spans, full candidate-set parity, small-cache state-ID reuse, truncation rejection, order/duplicates/captures, fallback, Auto/Off, and re-export after execution.

PASSED in the source checkout: `cargo fmt --all --check`, `git diff --check`, and `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings`. The latter retains the historical snapshot dependencies; its upstream Rust/C warnings are preserved in the check log. No CI run or cross-platform claim is implied.

With the previous experiment's local cache available, build the isolated source and immutable reference binaries using:

```bash
python3 scripts/prepare-portable-eager-comparison.py .cache/portable-eager-20261023 --snapshot-cache .cache/prepared-eager-20261023
```

Export each corpus with the isolated `bin/portable prepare-portable CORPUS ARTIFACT_DIR`, retaining identical ordinary program bytes. Then validate/prepare and run the matrix:

```bash
python3 scripts/compare-scanner-group-archives.py .cache/portable-eager-20261023/run-01 --cache .cache/portable-eager-20261023 --prebuilt --reuse-artifacts --prepare-only --variants eager prepared portable control
node scripts/compress-scanner-group-archives.mjs .cache/portable-eager-20261023/artifacts .cache/portable-eager-20261023/brotli.json eager prepared portable
python3 scripts/compare-scanner-group-archives.py .cache/portable-eager-20261023/run-01 --cache .cache/portable-eager-20261023 --measure-only --variants eager prepared portable control --repetitions 3
python3 scripts/report-portable-eager.py .cache/portable-eager-20261023/run-01 .cache/portable-eager-20261023/brotli.json benches/diagnostics/portable-eager-20261023
```

Use a fresh output/cache directory, or the build script's explicit `--refresh` for its own isolated build directory. The evidence retains the transformed manifest/lock and measured source overrides against base commit 4a6679e6df2cca2ac28238f2a8335b731d8b3d32. The previous snapshot's source and binary hashes are retained separately.
"""
    (args.output / "README.md").write_text(intro)


if __name__ == "__main__":
    main()
