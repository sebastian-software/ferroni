#!/usr/bin/env python3
"""Validate, measure, and report the production regex corpus on one host.

Only Python's standard library is needed. Criterion estimates and raw samples
are retained unchanged, alongside the exact compiled corpus and run order.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import random
import resource
import shutil
import statistics
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
ENGINES = ("rust", "rust_erroraware", "c", "onigmo", "pcre2_jit", "pcre2", "fancy_regex", "fancy_regex_seek", "regex")
PROFILES = {
    "rust": "Ferroni standard public Regex API; explicit source flags; fallible preflight checks",
    "rust_erroraware": "Ferroni fallible *_with searches/iteration with SearchOptions::new(); standard replacement (no fallible replacement API)",
    "c": "C Oniguruma UTF-8/Oniguruma syntax, -O3; public C API plus output adapters",
    "onigmo": "pinned Ruby Onigmo standalone build; public C API plus output adapters",
    "pcre2_jit": "PCRE2 UTF/UCP, JIT required, NO_UTF_CHECK on trusted &str subjects",
    "pcre2": "PCRE2 UTF/UCP interpreter, NO_UTF_CHECK on trusted &str subjects",
    "fancy_regex": "fancy-regex Oniguruma mode, seek off, explicit source flags",
    "fancy_regex_seek": "fancy-regex Oniguruma mode, seek on, explicit source flags",
    "regex": "Rust regex public API with explicit source flags; default limits",
}
LANES = ("steady", "compile", "first_use")


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def command_output(command):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=True)
    return result.stdout.strip()


def build(features):
    result = subprocess.run([
        "cargo", "bench", "--locked", "--features", features, "--bench",
        "production_regex_bench", "--no-run", "--message-format=json",
    ], cwd=ROOT, env=dict(os.environ, PCRE2_SYS_STATIC="1"), text=True, stdout=subprocess.PIPE, check=True)
    for line in result.stdout.splitlines():
        record = json.loads(line)
        if record.get("reason") == "compiler-artifact" and record.get("executable") and record["target"]["name"] == "production_regex_bench":
            return Path(record["executable"])
    raise RuntimeError("Cargo did not return a production benchmark executable")


def environment(case, engine):
    return dict(os.environ, PRODUCTION_CASE=case, PRODUCTION_ENGINE=engine)


def snapshot_source(directory):
    """Retain current engine/harness code even for a diagnostic dirty checkout."""
    paths = ["Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml", "README.md",
             ".cargo/config.toml", "LICENSE", "src", "benches/engines.rs", "benches/onigmo_shim.c",
             "benches/production_regex_bench.rs", "benches/production_regex", "benches/tokenizer_regex", "benches/battle_inputs.toml",
             "scripts/compare-production.py", "scripts/check-production-corpus.py", "scripts/verify-production-oracles.py", "scripts/verify-tokenizer-oracles.py"]
    archive = directory / "source-snapshot.tar.gz"
    native = {}
    with tarfile.open(archive, "w:gz") as bundle:
        for path in paths:
            bundle.add(ROOT / path, arcname=path)
        for name, variable, fallback in (
            ("oniguruma", "FERRONI_ONIGURUMA_DIR", ".cache/upstream/oniguruma-orig"),
            ("onigmo", "FERRONI_ONIGMO_DIR", ".cache/upstream/onigmo-ruby"),
        ):
            path = Path(os.environ.get(variable, str(ROOT / fallback))).resolve()
            if path.exists():
                bundle.add(path, arcname=fallback)
                stamp = path / ".ferroni-upstream-commit"
                native[name] = {"source_path": str(path), "revision_stamp": stamp.read_text().strip() if stamp.exists() else None}
    return {"source_snapshot_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(), "native_sources": native}


def preflight(binary, case, engine, timeout):
    identity = {"case": case, "engine": engine}
    try:
        result = subprocess.run([str(binary), "--validate"], env=environment(case, engine),
                                text=True, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return dict(identity, status="timeout", reason=f"validation exceeded {timeout}s")
    if result.returncode:
        return dict(identity, status="execution_error", reason=result.stderr[-4000:], returncode=result.returncode)
    records = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    if len(records) != 1 or any(records[0].get(k) != v for k, v in identity.items()):
        raise RuntimeError(f"Invalid validation record for {case}/{engine}")
    return records[0]


def ordered_pairs(cases, engines, repetition, seed):
    """Shuffle families/cases deterministically and balance each engine's position."""
    order = list(cases)
    random.Random(seed + repetition).shuffle(order)
    for index, case in enumerate(order):
        shift = (index + repetition) % len(engines)
        engine_order = list(engines[shift:] + engines[:shift])
        if repetition % 2:
            engine_order.reverse()
        for engine in engine_order:
            yield case, engine


def fatal_preflight(records):
    """Capability gaps are expected; broken execution must stop paid timings."""
    return [record for record in records
            if record["status"] in ("execution_error", "timeout")
            or (record["engine"] in ("rust", "rust_erroraware") and record["status"] not in ("eligible", "pending"))]


def memory_worker(binary, case, engine, mode, timeout):
    result = subprocess.run([binary, "--compile-once" if mode == "compile" else "--execute-once"],
                            env=environment(case, engine), stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=timeout)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace")[-2000:])
    # A fresh Python worker has exactly one measured child. Linux reports KiB,
    # Darwin bytes. This is whole-process peak RSS, not retained regex size.
    rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    return {"peak_rss_bytes": int(rss if sys.platform == "darwin" else rss * 1024)}


def measure_memory(binary, case, engine, timeout):
    results = {}
    for mode in ("compile", "execute"):
        worker = subprocess.run([sys.executable, __file__, "memory-worker", str(binary), case, engine, mode,
                                 "--timeout", str(timeout)], text=True, capture_output=True, timeout=timeout + 5)
        results[mode] = json.loads(worker.stdout) if worker.returncode == 0 else {"error": worker.stderr[-2000:]}
    return results


def estimates(directory, case, engine):
    rows = {}
    for path in directory.rglob("new/benchmark.json"):
        identity = json.loads(path.read_text())["full_id"]
        for lane in LANES:
            if identity == f"production_regex/{case}/{lane}/{engine}":
                data = json.loads(path.with_name("estimates.json").read_text())["mean"]
                rows[lane] = {"ns": data["point_estimate"],
                              "lower_ns": data["confidence_interval"]["lower_bound"],
                              "upper_ns": data["confidence_interval"]["upper_bound"],
                              "raw_directory": str(path.parent.relative_to(directory))}
    if set(rows) != set(LANES):
        raise RuntimeError(f"Missing Criterion lanes for {case}/{engine}: {list(rows)}")
    return rows


def summarize_pair(cases, measurements, engine, repetitions, lane):
    pairs = []
    for case in cases:
        values = []
        for rep in range(repetitions):
            base = measurements.get((case["id"], "rust", rep), {}).get(lane)
            other = measurements.get((case["id"], engine, rep), {}).get(lane)
            if not base or not other:
                break
            values.append((base, other))
        if len(values) != repetitions:
            continue
        ratios = [other["ns"] / base["ns"] for base, other in values]
        lower = min(other["lower_ns"] / base["upper_ns"] for base, other in values)
        upper = max(other["upper_ns"] / base["lower_ns"] for base, other in values)
        direction = "inconclusive"
        if repetitions >= 3:
            if lower > 1:
                direction = "Ferroni faster in every run interval"
            elif upper < 1:
                direction = "comparator faster in every run interval"
        pairs.append({"case": case["id"], "family": case["family"],
                      "ratio_median": statistics.median(ratios), "ratio_range": [min(ratios), max(ratios)],
                      "interval_envelope": [lower, upper], "conclusion": direction})
    families = {}
    for pair in pairs:
        families.setdefault(pair["family"], []).append(pair["ratio_median"])
    geometric = lambda values: math.exp(statistics.mean(math.log(value) for value in values))
    return {"engine": engine, "lane": lane, "cases": pairs,
            "family_count": len(families), "case_count": len(pairs),
            "family_weighted_ratio": geometric([geometric(values) for values in families.values()]) if pairs else None,
            "aggregate_scope": "This pair's listed common cases only. Different comparator subsets are not a ranking."}


def report(directory):
    context = json.loads((directory / "context.json").read_text())
    catalog = json.loads((directory / "catalog.json").read_text())
    eligibility = json.loads((directory / "eligibility.json").read_text())
    records = json.loads((directory / "measurements.json").read_text()) if (directory / "measurements.json").exists() else []
    measurements = {(r["case"], r["engine"], r["repetition"]): r["lanes"] for r in records if r["status"] == "measured"}
    cases = [c for c in catalog["cases"] if c["id"] in context["cases"]]
    comparisons = [summarize_pair(cases, measurements, engine, context["repetitions"], lane)
                   for engine in context["engines"] if engine != "rust" for lane in LANES]
    write_json(directory / "summary.json", {"comparisons": comparisons})
    lines = ["# Production regex comparison", "", f"Host: {context['platform']}; {context['machine']}.", "",
             "Diagnostic evidence; no publication claim." if context["diagnostic"] else "Publication candidate: review host stability, raw samples, coverage and run intervals before publishing.", "",
             "Ratios are comparator / Ferroni. Compile and first-use times concern one pattern; steady times concern the complete fixture batch.", "",
             "Each comparison below uses only its explicitly listed common tasks. Families have equal aggregate weight; architectures are never combined.", "",
             "## Eligibility", "", "| Case | Engine | Status |", "| --- | --- | --- |"]
    lines.extend(f"| `{r['case']}` | {r['engine']} | {r['status']} |" for r in eligibility)
    for comparison in comparisons:
        if not comparison["cases"]:
            continue
        lines += ["", f"## {comparison['engine']} / {comparison['lane']}", "",
                  f"Common cases: {comparison['case_count']}; families: {comparison['family_count']}. Pair-specific family-weighted ratio: {comparison['family_weighted_ratio']:.3f}.", "",
                  "| Case | Median ratio | Run range | Interpretation |", "| --- | ---: | ---: | --- |"]
        for pair in comparison["cases"]:
            lo, hi = pair["ratio_range"]
            lines.append(f"| `{pair['case']}` | {pair['ratio_median']:.3f} | {lo:.3f}–{hi:.3f} | {pair['conclusion']} |")
    lines += ["", "Raw samples, estimates, confidence intervals, failures, eligibility reasons, corpus snapshot and execution order are retained beside this report.", "",
              "Memory, where requested, is fresh-process peak RSS including the binary, runtime, parsed catalog, compilation, input/output and allocator. It is not isolated compiled-pattern memory.", ""]
    if (directory / "memory.json").exists():
        grouped = {}
        for record in json.loads((directory / "memory.json").read_text()):
            for mode, value in record["measurements"].items():
                grouped.setdefault((record["case"], record["engine"], mode), []).append(value)
        lines += ["## Fresh-process peak RSS", "", "| Case | Engine | Phase | Median MiB | Run range MiB |", "| --- | --- | --- | ---: | ---: |"]
        for (case, engine, mode), values in sorted(grouped.items()):
            if any("error" in value for value in values):
                lines.append(f"| `{case}` | {engine} | {mode} | error | see memory.json |")
                continue
            rss = [value["peak_rss_bytes"] / 1024**2 for value in values]
            lines.append(f"| `{case}` | {engine} | {mode} | {statistics.median(rss):.2f} | {min(rss):.2f}–{max(rss):.2f} |")
        lines.append("")
    (directory / "summary.md").write_text("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("validate", "run", "report", "memory-worker"))
    parser.add_argument("directory", type=Path)
    parser.add_argument("worker_arguments", nargs="*")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--cases", help="comma-separated case IDs (default: complete catalog)")
    parser.add_argument("--engines", default=",".join(ENGINES))
    parser.add_argument("--corpus", choices=("production", "tokenizer"), default="production")
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--measure-seconds", type=float, default=3)
    parser.add_argument("--warmup-seconds", type=float, default=1)
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--seed", type=int, default=20261013)
    parser.add_argument("--diagnostic", action="store_true", help="allow a short smoke run; cannot support publication")
    parser.add_argument("--memory-cases", default="", help="selected eligible case IDs for fresh-process peak RSS")
    parser.add_argument("--host-note", default="Host load, power and thermal state not recorded; assess before publication.")
    args = parser.parse_args()
    if args.command == "memory-worker":
        print(json.dumps(memory_worker(str(args.directory), *args.worker_arguments, args.timeout)))
        return
    directory = args.directory.resolve()
    if args.command == "report":
        report(directory)
        return
    if args.repetitions < 1 or args.samples < 10 or min(args.measure_seconds, args.warmup_seconds, args.timeout) <= 0:
        parser.error("positive durations, at least one repetition and ten samples required")
    if args.command == "run" and not args.diagnostic and (args.repetitions < 3 or args.samples < 30 or args.measure_seconds < 3):
        parser.error("publication candidates require >=3 processes, >=30 samples, >=3 seconds per lane; use --diagnostic for smoke runs")
    if directory.exists() and any(directory.iterdir()):
        parser.error("use a new empty output directory; retained evidence must not be overwritten")
    directory.mkdir(parents=True, exist_ok=True)
    spec = importlib.util.spec_from_file_location("corpus_check", ROOT / "scripts/check-production-corpus.py")
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    if args.corpus == "tokenizer":
        checker.CATALOG = ROOT / "benches/tokenizer_regex/catalog.json"
    os.environ["PRODUCTION_CORPUS"] = args.corpus
    checker.check(json.loads(checker.CATALOG.read_text()), checker.CATALOG.parent)
    engines = args.engines.split(",")
    features = "onigmo" if "onigmo" in engines else "ffi"
    binary = args.binary.resolve() if args.binary else build(features)
    catalog = json.loads(command_output([str(binary), "--catalog"]))
    if catalog != json.loads(checker.CATALOG.read_text()):
        parser.error("binary embeds a different corpus; rebuild before running")
    cases = args.cases.split(",") if args.cases else [c["id"] for c in catalog["cases"]]
    if not set(cases) <= {c["id"] for c in catalog["cases"]} or not set(engines) <= set(ENGINES) or len(set(cases)) != len(cases) or len(set(engines)) != len(engines):
        parser.error("unknown or duplicate case/engine ID")
    if not set(filter(None, args.memory_cases.split(","))) <= set(cases):
        parser.error("memory cases must be included in the selected cases")
    context = {"format_version": 1, "started_utc": datetime.now(timezone.utc).isoformat(),
               "platform": platform.platform(), "machine": platform.machine(), "cpu_count": os.cpu_count(), "corpus": args.corpus,
               "host_note": args.host_note, "commit": command_output(["git", "rev-parse", "HEAD"]),
               "git_status": command_output(["git", "status", "--porcelain"]),
               "rustc": command_output(["rustc", "-Vv"]), "cargo": command_output(["cargo", "-V"]),
               "python": sys.version, "build_profile": "provided binary; build flags unverified" if args.binary else f"bench, thin LTO, features {features}",
               "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
               "corpus_sha256": hashlib.sha256(checker.CATALOG.read_bytes()).hexdigest(),
               "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
               "rustflags": os.environ.get("RUSTFLAGS", ""), "engines": engines, "profiles": PROFILES,
               "cases": cases, "repetitions": args.repetitions, "samples": args.samples,
               "measure_seconds": args.measure_seconds, "warmup_seconds": args.warmup_seconds,
               "timeout": args.timeout, "seed": args.seed, "diagnostic": args.diagnostic or args.command == "validate" or bool(args.binary),
               "command": sys.argv, "order_policy": "seeded case shuffle; rotated engine order, reversed on alternating repetitions"}
    write_json(directory / "context.json", context)
    write_json(directory / "catalog.json", catalog)
    shutil.copyfile(ROOT / "Cargo.lock", directory / "Cargo.lock")
    shutil.copyfile(ROOT / "benches/battle_inputs.toml", directory / "engine-inputs.toml")
    # Record overrides as well as the normal revision stamps. The source
    # preparation scripts own these stamps; Cargo may also use another path.
    context["engine_source_overrides"] = {key: os.environ.get(key) for key in (
        "FERRONI_ONIGURUMA_DIR", "FERRONI_ONIGMO_DIR", "CC", "CFLAGS", "CARGO_PROFILE_BENCH_LTO")}
    context["pcre2_build"] = "provided binary; unverified" if args.binary else "PCRE2_SYS_STATIC=1; bundled source from locked pcre2-sys"
    context.update(snapshot_source(directory))
    write_json(directory / "context.json", context)
    eligibility = []
    for case in cases:
        for engine in engines:
            record = preflight(binary, case, engine, args.timeout)
            eligibility.append(record)
            print(f"{case}/{engine}: {record['status']}", flush=True)
    write_json(directory / "eligibility.json", eligibility)
    failures = fatal_preflight(eligibility)
    if failures:
        report(directory)
        print(f"Evidence: {directory / 'summary.md'}")
        raise SystemExit(f"Preflight failed for {len(failures)} pairs; no timing processes started.")
    eligible = {(r["case"], r["engine"]) for r in eligibility if r["status"] == "eligible"}
    measurements = []
    order = []
    memory = []
    if args.command == "run":
        for rep in range(args.repetitions):
            for case, engine in ordered_pairs(cases, engines, rep, args.seed):
                if (case, engine) not in eligible:
                    continue
                relative = f"raw/run-{rep + 1}/{case}/{engine}"
                raw = directory / relative
                raw.mkdir(parents=True)
                env = dict(environment(case, engine), CRITERION_HOME=str(raw))
                command = [str(binary), "--bench", "--noplot", "--sample-size", str(args.samples),
                           "--measurement-time", str(args.measure_seconds), "--warm-up-time", str(args.warmup_seconds)]
                record = {"case": case, "engine": engine, "repetition": rep, "raw": relative}
                order.append(dict(record, command=command))
                write_json(directory / "order.json", order)
                print(f"run {rep + 1}: {case}/{engine}", flush=True)
                with (raw / "process.log").open("w") as log:
                    try:
                        result = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=args.timeout)
                        if result.returncode:
                            raise RuntimeError(f"benchmark exited {result.returncode}")
                        record.update(status="measured", lanes=estimates(raw, case, engine))
                    except (subprocess.TimeoutExpired, RuntimeError) as error:
                        record.update(status="timeout" if isinstance(error, subprocess.TimeoutExpired) else "execution_error", reason=str(error))
                measurements.append(record)
                write_json(directory / "measurements.json", measurements)
                if case in args.memory_cases.split(","):
                    memory.append(dict(case=case, engine=engine, repetition=rep,
                                       measurements=measure_memory(binary, case, engine, args.timeout)))
                    write_json(directory / "memory.json", memory)
    report(directory)
    print(f"Evidence: {directory / 'summary.md'}")
    # Unsupported comparator syntax is expected. Ferroni must reproduce every
    # admitted source oracle; otherwise this suite is not ready to benchmark.
    if any(r["engine"] == "rust" and r["status"] not in ("eligible", "pending") for r in eligibility):
        raise SystemExit(1)
    if any(r["status"] != "measured" for r in measurements):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
