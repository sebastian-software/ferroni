#!/usr/bin/env python3
"""Report load-phase optimizations against Eager, Snapshot, and merged Portable."""
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
    parser.add_argument("--phases", type=Path, required=True)
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
        "New portable packages were round-trip measured with Brotli-11. Unchanged reference sizes "
        "were reused only after payload SHA-256 checks. Each row labels its timing provenance.")
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (args.output / "brotli.json").write_text(json.dumps(br, indent=2) + "\n")
    (args.output / "measurements.md").write_text(groups.tables(summary))
    lines = ["# Portable load optimization: complete local comparison", "",
             "`portable` is the load-optimized fork-free kernel. `before` is the frozen merged portable executable, "
             "`prepared` is the frozen dependency snapshot, and `control` runs ordinary Eager through the new portable-feature executable.", "",
             "## Absolute measurements", "",
             "Central timings are medians of three Criterion process means. RSS is the median of three fresh-process peaks. "
             "Portable Brotli-11 sizes are new; unchanged reference sizes are reused after payload hash checks. "
             "Before uses FPRT001, optimized uses FPRT002, and control uses only the Eager image.", "",
             "| Corpus | Variant | Load ms | Warm search ms | First document ms | Brotli-11 KiB | gzip-9 KiB | Peak RSS MiB |",
             "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for case in groups.CASES:
        for variant in ["eager", "prepared", "before", "portable", "control"]:
            row = by[case, variant]
            codec = br_by[case, variant]
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
    phase_by = {}
    for stage in ["before", "final"]:
        rows = json.loads((args.phases / f"phases-{stage}-summary.json").read_text())
        for row in rows:
            if row["variant"] == "portable":
                phase_by[stage, row["corpus"]] = {p["phase"]: p for p in row["phases"]}
    phase_lines = ["# Portable load phases", "",
        "Diagnostic feature builds: three rotating processes, 20 fresh loads per process. "
        "Cells are medians of per-process means in milliseconds. Nested time is subtracted from phases; "
        "total is inclusive. Instrumentation overhead is included here and excluded from the final Criterion matrix. "
        "Per-phase medians need not add up to the median total.", "",
        "| Corpus | Total, before → new | Group checksum | Program binding checksum | Instruction decode | Instruction validation |",
        "| --- | ---: | ---: | ---: | ---: | ---: |"]
    for case in groups.CASES:
        def phase_time(name):
            field = "median_process_mean_inclusive_ns" if name == "total" else "median_process_mean_exclusive_ns"
            return " → ".join(f"{phase_by[stage,case][name][field]/1e6:.3f}" for stage in ["before","final"])
        phase_lines.append(f"| {case} | {phase_time('total')} | {phase_time('group_checksum')} | "
            f"{phase_time('group_binding_checksum')} | {phase_time('portable_decode')} | {phase_time('portable_validate')} |")
    phase_lines += ["", "## Remaining exclusive costs", "",
        "| Corpus | Phase | Time ms | Calls per load |", "| --- | --- | ---: | ---: |"]
    for case in groups.CASES:
        for name, phase in sorted(phase_by["final",case].items(),
                key=lambda p:p[1]["median_process_mean_exclusive_ns"],reverse=True)[:6]:
            phase_lines.append(f"| {case} | {name} | {phase['median_process_mean_exclusive_ns']/1e6:.3f} | {phase['calls_per_load']} |")
    (args.output / "phases.md").write_text("\n".join(phase_lines) + "\n")
    for baseline in ["eager", "prepared", "before", "control"]:
        ps=[p for p in pairs if p["baseline"]==baseline]
        print(baseline, {metric: [round(min(p["metrics"][metric]["median_percent"] for p in ps),1),
            round(max(p["metrics"][metric]["median_percent"] for p in ps),1)] for metric in groups.METRICS})


if __name__ == "__main__":
    main()
