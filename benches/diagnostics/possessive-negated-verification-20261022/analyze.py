#!/usr/bin/env python3
"""Summarize retained paired processes without dropping samples or outliers."""

import argparse
import json
import math
from pathlib import Path
import random
import statistics


def quantile(values, probability):
    ordered = sorted(values)
    index = (len(ordered) - 1) * probability
    lower = math.floor(index)
    upper = math.ceil(index)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (index - lower)


def median_interval(clusters, seed):
    """Percentile interval, resampling layout clusters with both order pairs."""
    rng = random.Random(seed)
    estimates = []
    for _ in range(10000):
        selected = [value for _ in clusters for value in rng.choice(clusters)]
        estimates.append(statistics.median(selected))
    return [quantile(estimates, 0.025), quantile(estimates, 0.975)]


def summarize(directory):
    records = json.loads((directory / "measurements.json").read_text())
    context = json.loads((directory / "context.json").read_text())
    cases = list(dict.fromkeys(row["case"] for row in records))
    repetitions = context["args"]["repetitions"]
    assert len(records) == len(cases) * repetitions * 2
    results = []
    for case in cases:
        pairs = []
        for repetition in range(repetitions):
            rows = [row for row in records if row["case"] == case and row["repetition"] == repetition]
            assert sorted(row["variant"] for row in rows) == [0, 1]
            baseline, candidate = sorted(rows, key=lambda row: row["variant"])
            old, new = [row["lanes"]["steady"]["ns"] for row in [baseline, candidate]]
            assert old > 0 and new > 0
            pairs.append({"repetition": repetition, "offset": baseline["alignment_offset"],
                          "baseline_ns": old, "candidate_ns": new,
                          "change_percent": 100 * (new / old - 1),
                          "first_variant": rows[0]["variant"],
                          "baseline_load": baseline["load_average"],
                          "candidate_load": candidate["load_average"]})
        offsets = sorted({row["offset"] for row in pairs})
        clusters = [[row["change_percent"] for row in pairs if row["offset"] == offset] for offset in offsets]
        if repetitions == 2 * len(offsets):
            for offset in offsets:
                assert sorted(row["first_variant"] for row in pairs if row["offset"] == offset) == [0, 1]
        changes = [row["change_percent"] for row in pairs]
        interval = median_interval(clusters, 20261022 + cases.index(case))
        results.append({"case": case, "pairs": pairs,
                        "median_change_percent": statistics.median(changes),
                        "median_interval_95_percent": interval,
                        "range_percent": [min(changes), max(changes)],
                        "p10_p90_percent": [quantile(changes, 0.1), quantile(changes, 0.9)],
                        "pairs_faster": sum(value < 0 for value in changes),
                        "pairs_slower": sum(value > 0 for value in changes),
                        "pairs_at_least_3_percent_faster": sum(value <= -3 for value in changes),
                        "pairs_at_least_3_percent_slower": sum(value >= 3 for value in changes),
                        "cycle_medians_percent": [statistics.median(changes[i:i + len(offsets)])
                                                   for i in range(0, repetitions, len(offsets))],
                        "order_medians_percent": {str(first): statistics.median([row["change_percent"] for row in pairs if row["first_variant"] == first])
                                                  for first in {row["first_variant"] for row in pairs}},
                        "layout_medians_percent": {str(offset): statistics.median(cluster) for offset, cluster in zip(offsets, clusters)}})
    return sorted(results, key=lambda row: row["case"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    # Deterministic sanity checks of the statistical boundary.
    assert median_interval([[0, 0]] * 8, 1) == [0, 0]
    assert median_interval([[5, 5]] * 8, 1) == [5, 5]
    result = {"metric": "percent change of paired process means; negative is faster",
              "interval_method": "10,000 percentile-bootstrap replicates of the median; cluster by environment offset, retaining both opposite-order pairs together. Descriptive 95% intervals, not corrected for the 29 comparisons or uncontrolled host activity.",
              "broad": summarize(args.root / "broad"),
              "control_aa": summarize(args.root / "control-aa")}
    for row in result["broad"]:
        median = row["median_change_percent"]
        lower, upper = row["median_interval_95_percent"]
        cycles = row["cycle_medians_percent"]
        if median <= -3 and max(cycles) <= -3 and upper < 0:
            decision = "repeatable_gain_at_3_percent_target"
        elif median >= 3 and min(cycles) >= 3 and lower > 0:
            decision = "repeatable_material_regression"
        elif upper >= 3:
            decision = "material_slowdown_unresolved"
        elif lower > -3 and upper < 3:
            decision = "descriptive_interval_within_3_percent"
        else:
            decision = "target_sized_effect_not_repeatable"
        row["interpretation"] = decision
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    for lane in ["control_aa", "broad"]:
        print(lane)
        for row in result[lane]:
            print(row["case"], f'{row["median_change_percent"]:+.2f}%',
                  'interval', [round(v, 2) for v in row["median_interval_95_percent"]],
                  'cycles', [round(v, 2) for v in row["cycle_medians_percent"]],
                  'slower', row["pairs_slower"], '/', len(row["pairs"]))


if __name__ == "__main__":
    main()
