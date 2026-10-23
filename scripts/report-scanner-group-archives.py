#!/usr/bin/env python3
"""Summarize a complete compare-scanner-group-archives.py matrix."""
import argparse
import json
from pathlib import Path
import statistics

CASES = ["cpp", "scss", "java", "json", "rust", "html", "astro", "tsx", "c", "php"]
VARIANTS = ["eager", "nfa", "dfa"]
METRICS = ["load", "search", "cold_document"]


def read(path):
    return json.loads(path.read_text())


def summarize(root):
    context = read(root / "context.json")
    reps = context["repetitions"]
    variants = context.get("variants", VARIANTS)
    sizes = {(r["corpus"], r["variant"]): r for r in read(root / "sizes.json")}
    memory = read(root / "memory.json")
    validation = read(root / "validation.json")
    assert len(validation) == len(CASES) * len(variants)
    estimates = {}
    raw = []
    for rep in range(1, reps + 1):
        for case in CASES:
            for variant in variants + (["control"] if rep == 1 and "control" not in variants else []):
                for metric in METRICS:
                    matches = list(root.glob(f"criterion/r{rep}-{case}-{variant}/**/{metric}/new/estimates.json"))
                    assert len(matches) == 1, (rep, case, variant, metric, matches)
                    estimate = read(matches[0])["mean"]
                    estimates[rep, case, variant, metric] = estimate["point_estimate"]
                    raw.append({"repetition": rep, "corpus": case, "variant": variant,
                                "metric": metric, "mean_ns": estimate})
    rows = []
    for case in CASES:
        for variant in variants:
            row = {"corpus": case, "variant": variant, "sizes": sizes[case, variant], "timings": {}}
            selected = [r for r in memory if r["corpus"] == case and r["variant"] == variant]
            assert len(selected) == reps
            row["memory"] = {
                "rss_median_bytes": statistics.median(r["max_rss_bytes"] for r in selected),
                "rss_min_bytes": min(r["max_rss_bytes"] for r in selected),
                "rss_max_bytes": max(r["max_rss_bytes"] for r in selected),
                "immutable_after_search_bytes": statistics.median(r["after_search"]["immutable_automata_bytes"] for r in selected),
                "scratch_after_search_bytes": statistics.median(r["after_search"]["scanner_cache_bytes"] for r in selected),
                "retired_after_search": [r["after_search"]["retired_groups"] for r in selected],
                "cache_clears_after_search": [r["after_search"]["cache_clears"] for r in selected],
            }
            for metric in METRICS:
                ns = [estimates[rep, case, variant, metric] for rep in range(1, reps + 1)]
                ratios = [estimates[rep, case, variant, metric] / estimates[rep, case, "eager", metric]
                          for rep in range(1, reps + 1)]
                row["timings"][metric] = {
                    "median_mean_ns": statistics.median(ns), "per_run_mean_ns": ns,
                    "median_change_percent": (statistics.median(ratios) - 1) * 100,
                    "change_range_percent": [(min(ratios) - 1) * 100, (max(ratios) - 1) * 100],
                    "all_runs_gain_3_percent": max(ratios) <= 0.97,
                    "all_runs_regress_3_percent": min(ratios) >= 1.03,
                }
            rows.append(row)
    control_reps = range(1, reps + 1) if "control" in variants else [1]
    control = [{"corpus": case, "metric": metric,
                "change_percent": 100 * (statistics.median(estimates[rep, case, "control", metric] / estimates[rep, case, "eager", metric] for rep in control_reps) - 1)}
               for case in CASES for metric in METRICS]
    return {"repetitions": reps, "control_repetitions": len(control_reps), "rows": rows, "control": control, "raw_estimates": raw,
            "validated_calls_per_variant": sum(r["validated_calls"] for r in validation if r["variant"] == "eager")}


def tables(summary):
    rows = summary["rows"]
    lines = ["# Scanner-group archive measurements", "", "Negative timing changes mean less time. Ratios pair the same process repetition. Each central value is the median of three Criterion mean estimates; ranges retain all three paired ratios.", ""]
    for variant in dict.fromkeys(r["variant"] for r in rows if r["variant"] != "eager"):
        lines += [f"## {variant.upper()} versus current Eager", "",
                  "| Corpus | Load change | Search change | Cold document change | gzip KiB, Eager → candidate | Peak RSS MiB, Eager → candidate |",
                  "| --- | ---: | ---: | ---: | ---: | ---: |"]
        for case in CASES:
            base = next(r for r in rows if r["corpus"] == case and r["variant"] == "eager")
            r = next(r for r in rows if r["corpus"] == case and r["variant"] == variant)
            def delta(metric):
                t = r["timings"][metric]
                a, b = t["change_range_percent"]
                return f"{t['median_change_percent']:+.1f}% [{a:+.1f}, {b:+.1f}]"
            lines.append(f"| {case} | {delta('load')} | {delta('search')} | {delta('cold_document')} | "
                         f"{base['sizes']['gzip9_bytes']/1024:.1f} → {r['sizes']['gzip9_bytes']/1024:.1f} | "
                         f"{base['memory']['rss_median_bytes']/1048576:.1f} → {r['memory']['rss_median_bytes']/1048576:.1f} |")
        lines += [""]
    lines += ["## Absolute times", "", "Milliseconds per complete captured corpus; input I/O, gzip decompression, string setup, and teardown are outside these Rust measurements.", "",
              "| Corpus | Variant | Load ms | Warm search ms | Cold document ms | gzip decode ms (Python) |",
              "| --- | --- | ---: | ---: | ---: | ---: |"]
    for r in rows:
        t = r["timings"]
        lines.append(f"| {r['corpus']} | {r['variant']} | {t['load']['median_mean_ns']/1e6:.3f} | "
                     f"{t['search']['median_mean_ns']/1e6:.3f} | {t['cold_document']['median_mean_ns']/1e6:.3f} | "
                     f"{r['sizes']['decompress_median_ns']/1e6:.3f} |")
    lines += ["", "## Feature control", "", "Ordinary Eager with the prototype feature compiled and no prepared archive. This isolates feature/layout/code-generation effects; the context records the repetition count.", "",
              "| Corpus | Load change | Search change | Cold document change |", "| --- | ---: | ---: | ---: |"]
    for case in CASES:
        c = {r["metric"]: r["change_percent"] for r in summary["control"] if r["corpus"] == case}
        lines.append(f"| {case} | {c['load']:+.1f}% | {c['search']:+.1f}% | {c['cold_document']:+.1f}% |")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    summary = summarize(args.input)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (args.output / "measurements.md").write_text(tables(summary))


if __name__ == "__main__":
    main()
