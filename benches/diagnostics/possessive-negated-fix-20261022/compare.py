#!/usr/bin/env python3
"""Verify retained binaries with counterbalanced orders at each environment size.

Run from the repository root. This is a local diagnostic, not a replacement
for compare-production.py or its publication protocol. Build and validate both
binaries before starting; do not run builds or other benchmarks concurrently.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location(
    "comparison", ROOT / "scripts/compare-production.py"
)
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--cases", default="all", help="comma-separated IDs, or all")
    parser.add_argument("--repetitions", type=int, default=16)
    parser.add_argument("--warmup", type=float, default=0.5)
    parser.add_argument("--measure", type=float, default=1.5)
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--lanes", default="steady")
    parser.add_argument("--engines", default="rust")
    parser.add_argument("--alignment-offsets", default="0,8,16,24,32,40,48,56",
                        help="environment-size offsets in bytes, cycled across repetitions")
    parser.add_argument("--seed", type=int, default=20261022)
    parser.add_argument("--environment-base", type=int,
                        help="fixed padded environment base shared by focused and full matrices")
    args = parser.parse_args()
    if min(args.repetitions, args.warmup, args.measure) <= 0 or args.samples < 10:
        parser.error("positive durations/repetitions and at least ten samples required")
    lanes = args.lanes.split(",")
    if not set(lanes) <= set(comparison.LANES):
        parser.error("unknown lane")
    # The existing estimate reader also enforces the selected lane set.
    comparison.LANES = lanes
    try:
        offsets = tuple(int(value) for value in args.alignment_offsets.split(","))
    except ValueError:
        parser.error("alignment offsets must be nonnegative integers")
    if not offsets or min(offsets) < 0:
        parser.error("alignment offsets must be nonnegative integers")
    comparison.ALIGNMENT_OFFSETS = offsets
    binaries = [args.baseline.resolve(), args.candidate.resolve()]
    if len(os.fsencode(binaries[0])) != len(os.fsencode(binaries[1])):
        parser.error("binary path lengths must agree (argv also affects stack alignment)")
    catalogs = {
        name: json.loads((ROOT / f"benches/{folder}/catalog.json").read_text())
        for name, folder in [
            ("production", "production_regex"),
            ("tokenizer", "tokenizer_regex"),
        ]
    }
    case_corpus = {
        case["id"]: name
        for name, catalog in catalogs.items()
        for case in catalog["cases"]
        if case.get("inputs")
    }
    cases = list(case_corpus) if args.cases == "all" else args.cases.split(",")
    engines = args.engines.split(",")
    if not set(cases) <= set(case_corpus) or not set(engines) <= set(comparison.ENGINES):
        parser.error("unknown case or engine")
    for binary in binaries:
        for corpus in {case_corpus[case] for case in cases}:
            embedded = json.loads(subprocess.check_output(
                [str(binary), "--catalog"], cwd=ROOT,
                env=dict(os.environ, PRODUCTION_CORPUS=corpus), text=True,
            ))
            if embedded != catalogs[corpus]:
                parser.error(f"{binary} embeds a different {corpus} corpus")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    rows = []
    context = {
        "diagnostic": True,
        "started_utc": datetime.now(timezone.utc).isoformat(),
        "checkout_commit": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "platform": platform.platform(),
        "args": {key: str(value) if isinstance(value, Path) else value
                 for key, value in vars(args).items()},
        "binaries": [{"path": str(binary),
                      "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
                     for binary in binaries],
        "environment_sizes": [],
        "order": [],
    }
    # Plan every process first, so the padding base cannot change when the
    # repetition index gains a digit or a longer case runs in another block.
    plan = []
    for repetition in range(args.repetitions):
        pairs = [(case, engine) for case in cases for engine in engines]
        random.Random(args.seed + repetition).shuffle(pairs)
        jobs = []
        for index, (case, engine) in enumerate(pairs):
            # The second cycle reverses each case's order at the same offset.
            first = (cases.index(case) + repetition // len(offsets)) % 2
            variants = [first, 1 - first]
            for variant in variants:
                raw = output / f"raw/{repetition:02d}/{case}/{engine}/{variant}"
                env = dict(os.environ, PRODUCTION_CASE=case, PRODUCTION_ENGINE=engine,
                           PRODUCTION_CORPUS=case_corpus[case], CRITERION_HOME=str(raw))
                env.pop("PRODUCTION_LANE_BUDGETS", None)
                jobs.append((case, engine, variant, raw, env))
        plan.append(jobs)
    base_size = comparison.aligned_size([job[-1] for jobs in plan for job in jobs], 0) - offsets[0]
    if args.environment_base is not None:
        if args.environment_base < base_size:
            parser.error(f"environment base must be at least {base_size} bytes")
        base_size = args.environment_base
    context["order_policy"] = "shuffled cases; opposite binary order for each case at the same offset in the second cycle"
    context["environment_base_bytes"] = base_size
    context["runner_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    context["git_status"] = subprocess.check_output(["git", "status", "--short"], cwd=ROOT, text=True)
    for repetition, jobs in enumerate(plan):
        size = base_size + offsets[repetition % len(offsets)]
        context["environment_sizes"].append(size)
        for case, engine, variant, raw, env in jobs:
            raw.mkdir(parents=True)
            lane_filter = f"/({'|'.join(lanes)})/{engine}$"
            command = [str(binaries[variant]), "--bench", "--noplot", "--sample-size",
                       str(args.samples), "--nresamples", "10000", "--warm-up-time",
                       str(args.warmup), "--measurement-time", str(args.measure), lane_filter]
            record = dict(repetition=repetition, case=case, engine=engine, variant=variant,
                          alignment_offset=offsets[repetition % len(offsets)],
                          started_utc=datetime.now(timezone.utc).isoformat(), load_average=os.getloadavg())
            context["order"].append(dict(record, command=command))
            write_json(output / "context.json", context)
            with (raw / "process.log").open("w") as log:
                subprocess.run(command, cwd=ROOT, env=comparison.aligned_environment(env, size),
                               stdout=log, stderr=subprocess.STDOUT, check=True, timeout=90)
            record["finished_utc"] = datetime.now(timezone.utc).isoformat()
            estimates = comparison.estimates(raw, case, engine)
            rows.append(dict(record, lanes=estimates))
            write_json(output / "measurements.json", rows)
            print(repetition + 1, case, engine, variant,
                  " ".join(f'{lane}={data["ns"]:.1f}ns' for lane, data in estimates.items()),
                  flush=True)
    summary = []
    for case in cases:
        for engine in engines:
            for lane in lanes:
                times = [[row["lanes"][lane]["ns"] for row in rows
                          if row["case"] == case and row["engine"] == engine
                          and row["variant"] == variant] for variant in [0, 1]]
                baseline, candidate = times
                changes = [100 * (new / old - 1) for old, new in zip(baseline, candidate)]
                summary.append(dict(case=case, engine=engine, lane=lane,
                                    baseline_ns=statistics.median(baseline),
                                    candidate_ns=statistics.median(candidate),
                                    changes_percent=changes,
                                    median_change_percent=statistics.median(changes)))
    write_json(output / "summary.json", summary)
    for row in summary:
        print(row["case"], row["engine"], row["lane"],
              f'{row["median_change_percent"]:+.2f}%', row["changes_percent"])


if __name__ == "__main__":
    main()
