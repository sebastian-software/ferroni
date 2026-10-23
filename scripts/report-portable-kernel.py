#!/usr/bin/env python3
"""Report the optimized portable kernel against frozen and current baselines."""
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
    spec = importlib.util.spec_from_file_location(
        "groups", Path(__file__).with_name("report-scanner-group-archives.py"))
    groups = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(groups)
    summary = groups.summarize(args.run)
    by = {(row["corpus"], row["variant"]): row for row in summary["rows"]}
    br = json.loads(args.brotli.read_text())
    br_by = {(row["corpus"], row["variant"]): row for row in br}
    pairs = []
    for case in groups.CASES:
        for baseline in ["eager", "prepared", "before", "control"]:
            pair = {"corpus": case, "baseline": baseline, "variant": "portable", "metrics": {}}
            for metric in groups.METRICS:
                old = by[case, baseline]["timings"][metric]["per_run_mean_ns"]
                new = by[case, "portable"]["timings"][metric]["per_run_mean_ns"]
                changes = [100 * (a / b - 1) for a, b in zip(new, old)]
                pair["metrics"][metric] = {
                    "median_percent": statistics.median(changes),
                    "range_percent": [min(changes), max(changes)],
                    "per_run_percent": changes,
                    "all_runs_gain_3_percent": max(changes) <= -3,
                    "all_runs_regress_3_percent": min(changes) >= 3,
                }
            pairs.append(pair)
    summary["portable_pairs"] = pairs
    summary["brotli"] = br
    summary["brotli_provenance"] = (
        "Size measurements from the previous experiment, reused only after identical payload SHA-256 checks. "
        "Codec timing fields are historical diagnostics, not new measurements.")
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (args.output / "brotli.json").write_text(json.dumps(br, indent=2) + "\n")
    (args.output / "measurements.md").write_text(groups.tables(summary))
    lines = ["# Portable kernel: complete local comparison", "",
             "`portable` is the optimized fork-free kernel. `before` is the frozen original portable executable, "
             "`prepared` is the frozen dependency snapshot, and `control` runs ordinary Eager through the new portable-feature executable.", "",
             "## Absolute measurements", "",
             "Central timings are medians of three Criterion process means. RSS is the median of three fresh-process peaks. "
             "Brotli-11 sizes are reused from the previous experiment after verifying identical payload hashes; its codec timings were not rerun. "
             "The before image is identical to the optimized image, and control uses the Eager image.", "",
             "| Corpus | Variant | Load ms | Warm search ms | First document ms | Brotli-11 KiB | gzip-9 KiB | Peak RSS MiB |",
             "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for case in groups.CASES:
        for variant in ["eager", "prepared", "before", "portable", "control"]:
            row = by[case, variant]
            codec = br_by[case, {"before": "portable", "control": "eager"}.get(variant, variant)]
            assert codec["payloadSha256"] == row["sizes"]["payload_sha256"]
            t = row["timings"]
            lines.append(
                f"| {case} | {variant} | {t['load']['median_mean_ns']/1e6:.3f} | "
                f"{t['search']['median_mean_ns']/1e6:.3f} | {t['cold_document']['median_mean_ns']/1e6:.3f} | "
                f"{codec['brotli11Bytes']/1024:.1f} | {row['sizes']['gzip9_bytes']/1024:.1f} | "
                f"{row['memory']['rss_median_bytes']/1048576:.1f} |")
    lines += ["", "## Optimized portable versus each reference", "",
              "Negative changes mean less time. Brackets show the range of three paired process changes, not a confidence interval. "
              "A repeatable regression here means at least 3% in each repetition. Build and code-layout effects remain possible; "
              "the control helps expose them but cannot remove them from the frozen historical builds.", "",
              "| Corpus | Reference | Load change | Warm search change | First document change |",
              "| --- | --- | ---: | ---: | ---: |"]
    for pair in pairs:
        def change(metric):
            t = pair["metrics"][metric]
            lo, hi = t["range_percent"]
            return f"{t['median_percent']:+.1f}% [{lo:+.1f}, {hi:+.1f}]"
        lines.append(f"| {pair['corpus']} | {pair['baseline']} | {change('load')} | "
                     f"{change('search')} | {change('cold_document')} |")
    lines += ["", "## Memory counters", "",
              "These are partial internal estimates after warm searches, not all heap allocations. Flat rows include power-of-two "
              "padding; retained input images and allocator high-water marks are represented by process RSS above.", "",
              "| Corpus | Variant | Immutable MiB | Scanner scratch MiB | Cache clears, three processes | Retired groups |",
              "| --- | --- | ---: | ---: | --- | --- |"]
    for row in summary["rows"]:
        m = row["memory"]
        lines.append(f"| {row['corpus']} | {row['variant']} | {m['immutable_after_search_bytes']/1048576:.2f} | "
                     f"{m['scratch_after_search_bytes']/1048576:.2f} | {m['cache_clears_after_search']} | "
                     f"{m['retired_after_search']} |")
    (args.output / "comparison.md").write_text("\n".join(lines) + "\n")
    native = [by[case, "portable"] for case in groups.CASES]
    load = [-r["timings"]["load"]["median_change_percent"] for r in native]
    search = [r["timings"]["search"]["median_change_percent"] for r in native]
    old_pairs = [p for p in pairs if p["baseline"] == "before"]
    speedup = [-p["metrics"]["search"]["median_percent"] for p in old_pairs]
    regress = [r["corpus"] for r in native if r["timings"]["search"]["all_runs_regress_3_percent"]]
    snapshot_regress = [p["corpus"] for p in pairs if p["baseline"] == "prepared"
                        and p["metrics"]["search"]["all_runs_regress_3_percent"]]
    readme = f"""# Optimized fork-free portable Eager kernel

The optimized kernel cuts warm search time by {min(speedup):.1f}–{max(speedup):.1f}% against the frozen original portable prototype. Load remains {min(load):.1f}–{max(load):.1f}% faster than ordinary Eager, with byte-identical packages. Warm search changes versus Eager range from {min(search):+.1f}% to {max(search):+.1f}% (median paired changes across ten corpora). A regression of at least 3% in all three repetitions remains for {len(regress)} corpora versus Eager ({', '.join(regress) or 'none'}) and {len(snapshot_regress)} versus Snapshot ({', '.join(snapshot_regress) or 'none'}). This remains an opt-in prototype, not a default-on release decision.

The [five-way comparison](comparison.md) covers load, warm search, the first document, compressed sizes, fresh-process RSS, and internal memory counters. [measurements.md](measurements.md) retains all paired ranges versus Eager and the feature control. [summary.json](summary.json) contains raw mean estimates and all reference pairs; [evidence.tar.gz](evidence.tar.gz) binds source, build provenance, profiles, ablations, validation, and raw Criterion samples.

## Cause and selected changes

The original TSX and HTML sampling profiles put 53.7% and 42.7% of leaf samples in the portable candidate walk. Hashing, state interning, and allocator paths were also visible on warm searches. This is execution overhead: each start rebuilt a vector and hash key even after warming, each cached transition followed separately allocated state storage, and the byte loop mixed the initial bounded walk with later candidate admission. This was not compilation work accidentally included in the warm timer.

The [owned runtime](../../../src/dfa_prefilter/portable.rs) now caches ten start states per machine (anchored/unanchored × five byte contexts), stores transition rows in one flat buffer with precomputed row offsets, and splits the bounded walk from candidate admission. Match recording stays outside the common byte loop. Cache clearing invalidates the start table before row IDs can be reused; row padding is included in the bounded cache budget. No unsafe code or dependency forks were introduced.

[ablation-summary.json](ablation-summary.json) retains the stepwise screen of starts, flat rows, and the split walk. These are rotating, three-process, document-replay screens, not the final Criterion matrix. They establish the large sequential contribution of each change; percentages are not additive. Context lookup tables, boxed caches, bulk self-loop skipping, and an outlined start miss were also screened and rejected because results were small or mixed. Their raw screens are in the evidence archive.

## Unchanged loading contract

The program archives, Snapshot images, and portable `FPRT001` images match the previous experiment byte-for-byte for every corpus. The producer and consumer still use public APIs with registry regex-automata 0.4.18 and aho-corasick 1.1.5. Cargo metadata, the transformed manifest/lock, source hashes, and all executable hashes are retained. The Snapshot and original portable executables are reused unchanged, and were rerun in this matrix rather than compared to historical timing numbers.

Only the native execution cache changed. The format still stores scanner-group seek instructions and forward/reverse entry points. Per-program literal helpers and plans still use normal Eager preparation; the older Snapshot includes those helpers too. Mutable transition caches warm during execution, as with normal Eager. Off/Auto and invalid-image fallback retain their existing behavior. Ferroni's existing VM still determines final matches and captures.

## Measurement contract and limits

- Ten captured corpora: C++, SCSS, Java, JSON, Rust, HTML, Astro, TSX, C, and PHP; all {summary['validated_calls_per_variant']:,} UTF-16 calls, winning patterns, and capture arrays agree in every lane before timing. Fresh benchmark processes validate again.
- Five variants × ten corpora × three rotating process repetitions. Criterion uses 40 samples, 300 ms warmup, a 1 s measurement target, and 10,000 bootstrap resamples. Each ratio pairs the same repetition. Ranges are observed process ranges, not confidence intervals or universal bounds.
- Load includes in-memory structural validation, decoding, helpers, scanners, and initial scratch. It excludes file I/O, codec work, network transfer, strings, and teardown. Warm searches follow three document passes and create fresh string identities outside the clock. First document includes load and the first complete replay.
- gzip-9 is recalculated and round-trip checked. Brotli-11 sizes are reused only after payload SHA-256 identity checks against the previous compression run; historical codec timing fields are labeled as such. No decompression or CDN transfer claim follows from the Rust load numbers.
- RSS uses one fresh driver child per memory process, retaining fixture parsing, source images, decoded programs, scanners, construction high-water marks, and four document passes. Internal counters are partial. Packages and decoded arrays are both retained.
- Local M1 Ultra Mac Studio, 64 GB, macOS arm64, rustc 1.99.0, release/bench ThinLTO. Time Machine was idle before the matrix. No compilation or compression ran beside the main timings. Core placement, thermals, and unrelated background activity were not controlled.

The remaining single-digit differences must be assessed with their ranges and feature-control effects. This experiment does not establish cross-platform behavior, complete grammar coverage, actual Ferriki startup, authenticated distribution, or adversarial resource behavior. No CI run or stable archive/API promise is implied.

## Validation and reproduction

The isolated registry source passes:

```bash
cargo test --offline --release --features portable-eager-prototype,prefilter-self-check --lib --test scanner_group_prototype --test compat_prefilter_differential --test precompiled_scanner --test scanner_prefilter_regressions -- --test-threads=1
cargo clippy --offline --all-targets --features portable-eager-prototype -- -D warnings
```

The source checkout passes `cargo fmt --all --check`, `git diff --check`, and `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings`. The root all-features check retains the historical Snapshot dependency extensions only for that comparator. Test output and `scripts/count-tests.sh` output are in the evidence; the latter counts the source tree, including feature-gated tests, rather than only this selected run.

Differential coverage includes unseen inputs, arbitrary start/end spans, ASCII assertions, full candidate sets, early/late admission, long loops, stamp rollover, and deliberately tiny caches that force state-ID reuse. Archive tests cover truncation, round-trip re-export, captures/order/duplicates, fallback, and mode handling.

With the preceding caches retained:

```bash
python3 scripts/prepare-portable-eager-comparison.py .cache/portable-kernel-20261023 --snapshot-cache .cache/prepared-eager-20261023 --portable-reference-cache .cache/portable-eager-20261023
# Export each corpus through bin/portable prepare-portable CORPUS ARTIFACT_DIR.
python3 scripts/compare-scanner-group-archives.py .cache/portable-kernel-20261023/run-01 --cache .cache/portable-kernel-20261023 --prebuilt --reuse-artifacts --prepare-only --variants eager prepared before portable control
python3 scripts/compare-scanner-group-archives.py .cache/portable-kernel-20261023/run-01 --cache .cache/portable-kernel-20261023 --measure-only --repetitions 3
python3 scripts/report-portable-kernel.py .cache/portable-kernel-20261023/run-01 .cache/portable-eager-20261023/brotli.json benches/diagnostics/portable-kernel-20261023
```

Use a fresh run directory. `--refresh` explicitly updates the selected isolated build directory. Base commit and exact source overrides are recorded in the evidence. Previous reports remain intact.
"""
    (args.output / "README.md").write_text(readme)


if __name__ == "__main__":
    main()
