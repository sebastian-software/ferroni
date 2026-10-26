#!/usr/bin/env python3
"""Local diagnostic only: selective prepared-record imports versus archive imports.

Build examples/prepared_record_import.rs with its required feature first. Each
variant gets fresh processes and fresh ScannerPatternCache instances. This is a
native scanner replay with populated local files, not a Ferriki startup/CDN test.
"""
import argparse
import hashlib
import json
import platform
import statistics
import subprocess
import sys
from pathlib import Path

CASES = ["json", "c", "cpp", "scss", "java", "rust", "tsx", "html", "astro", "php"]
VARIANTS = ["subset", "records", "archive"]
ROOT = Path(__file__).resolve().parent.parent


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def measure_memory(binary, inputs, cases, repetitions, call_limit=0):
    # One fresh wrapper has exactly one child: RUSAGE_CHILDREN then gives that
    # binary's peak, rather than the maximum across earlier benchmark children.
    worker = (
        "import subprocess,resource,json,sys; "
        "p=subprocess.run(sys.argv[1:],capture_output=True,text=True,check=True); "
        "rss=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss; "
        "print(json.dumps({'measurement':json.loads(p.stdout),"
        "'peak_rss_bytes':rss if sys.platform=='darwin' else rss*1024}))"
    )
    rows = []
    for rep in range(repetitions):
        for case in cases:
            for variant in VARIANTS if rep % 2 == 0 else VARIANTS[::-1]:
                row = json.loads(run(sys.executable, "-c", worker, str(binary), "measure",
                                     case, str(inputs), variant, "1", str(call_limit)))
                row.update(case=case, variant=variant, repetition=rep)
                rows.append(row)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--repetitions", type=int, default=6)
    parser.add_argument("--memory-repetitions", type=int, default=0, help="optional fresh-process RSS runs on macOS/Linux")
    parser.add_argument("--samples", type=int, default=12)
    parser.add_argument("--call-limit", type=int, default=0)
    parser.add_argument("--cases", nargs="+", choices=CASES, default=CASES)
    args = parser.parse_args()
    if args.repetitions < 3 or args.samples < 2:
        parser.error("use at least 3 processes and 2 fresh caches per process")
    if args.call_limit < 0 or args.memory_repetitions < 0:
        parser.error("call and memory limits must be nonnegative")
    if args.memory_repetitions and sys.platform not in ("darwin", "linux"):
        parser.error("RSS collection supports macOS and Linux")
    out = args.output.resolve()
    out.mkdir(exist_ok=False, parents=True)
    inputs = out / "inputs"
    binary = ROOT / "target/release/examples/prepared_record_import"
    provenance = {
        "base_commit": run("git", "rev-parse", "HEAD"),
        "git_status": run("git", "status", "--short"),
        "rustc": run("rustc", "-Vv"), "platform": platform.platform(),
        "binary_sha256": digest(binary), "features": "precompiled-eager",
        "profile": "release, thin LTO", "repetitions": args.repetitions,
        "cases": args.cases, "call_limit": args.call_limit,
        "samples_per_process": args.samples,
        "boundary": "manifest opening, payload file reads, import and all scanner construction; metadata bytes and trace parsing excluded",
        "order": "all six permutations repeated; one process at a time",
        "source_sha256": {str(p.relative_to(ROOT)): digest(p) for p in
            [*sorted((ROOT / "src/scanner").rglob("*.rs")), ROOT / "src/scanner.rs", ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "examples/prepared_record_import.rs"]},
    }
    (out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    preparation = []
    for case in args.cases:
        preparation.append(json.loads(run(str(binary), "prepare", case, str(inputs))))
    (out / "preparation.json").write_text(json.dumps(preparation, indent=2) + "\n")
    all_runs = []
    orders = [VARIANTS, VARIANTS[::-1], ["records", "archive", "subset"],
              ["subset", "archive", "records"], ["archive", "subset", "records"],
              ["records", "subset", "archive"]]
    for rep in range(args.repetitions):
        for case in args.cases if rep % 2 == 0 else args.cases[::-1]:
            for variant in orders[rep % len(orders)]:
                result = json.loads(run(str(binary), "measure", case, str(inputs), variant, str(args.samples), str(args.call_limit)))
                result["repetition"] = rep
                (out / f"{rep}-{case}-{variant}.json").write_text(json.dumps(result) + "\n")
                all_runs.append(result)
            print(f"{rep + 1}/{args.repetitions} {case}", flush=True)
    if args.memory_repetitions:
        rows = measure_memory(binary, inputs, args.cases, args.memory_repetitions, args.call_limit)
        (out / "memory.json").write_text(json.dumps(rows, indent=2) + "\n")
    summary = {}
    for case in args.cases:
        rows = {}
        for variant in VARIANTS:
            runs = [r for r in all_runs if r["corpus"] == case and r["variant"] == variant]
            loads = [statistics.median(s["load_ns"] for s in r["samples"]) / 1e6 for r in runs]
            search = [statistics.median(n for s in r["samples"] for n in s["search_ns"]) / 1e6 for r in runs]
            sample = runs[0]["samples"][0]
            rows[variant] = {"load_ms": statistics.median(loads), "process_medians_ms": loads,
                             "first_cache_ms": [r["samples"][0]["load_ns"] / 1e6 for r in runs],
                             "search_ms": statistics.median(search), "search_process_medians_ms": search,
                             "read_bytes": sample["read_bytes"], "copied_program_bytes": sample["copied_program_bytes"]}
        rows["record_load_gain_percent"] = 100 * (1 - rows["records"]["load_ms"] / rows["subset"]["load_ms"])
        summary[case] = rows
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    hashes = {str(p.relative_to(out)): digest(p) for p in sorted(out.rglob("*")) if p.is_file()}
    (out / "SHA256SUMS.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print(json.dumps({case: {"subset_ms": s["subset"]["load_ms"], "records_ms": s["records"]["load_ms"], "archive_ms": s["archive"]["load_ms"], "gain_percent": s["record_load_gain_percent"]} for case, s in summary.items()}, indent=2))


if __name__ == "__main__":
    main()
