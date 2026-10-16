#!/usr/bin/env python3
"""Retain a small Scanner application lane separately from regex comparisons.

Each repetition may run on its own VM. No cross-workload or cross-platform
aggregate is produced, and C is a correctness oracle, not a timed comparator.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import shutil
import statistics
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "benches/scanner_clients"


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def output(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def corpus():
    catalog = json.loads((CORPUS / "catalog.json").read_text())
    provenance = json.loads((CORPUS / "provenance.json").read_text())
    assert catalog["version"] == 1
    assert [c["id"] for c in catalog["cases"]] == ["tsx", "astro", "related"]
    for case in catalog["cases"]:
        assert case["trace"] == case["id"] + ".json"
        assert digest(CORPUS / case["trace"]) == provenance[case["id"] + "_trace_sha256"]
        assert case["lanes"] == (["compile", "first_use", "steady"] if case["id"] == "tsx" else ["compile", "first_use"])
        assert case["modes"] == (["recorded", "uncached"] if case["id"] == "related" else ["recorded"])
    for name, key in [("capture.patch", "capture_patch_sha256"), ("capture.rs", "capture_driver_sha256"),
                      *[(n, n + "_sha256") for n in ("GRAMMAR-NOTICES.md", "LICENSE-ferriki-MIT", "LICENSE-ferriki-APACHE")]]:
        assert digest(CORPUS / name) == provenance[key]
    return catalog


def build():
    result = subprocess.run(["cargo", "bench", "--locked", "--features", "ffi", "--bench",
                             "scanner_clients_bench", "--no-run", "--message-format=json"],
                            cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True)
    for line in result.stdout.splitlines():
        row = json.loads(line)
        if row.get("reason") == "compiler-artifact" and row.get("executable") and row["target"]["name"] == "scanner_clients_bench":
            return Path(row["executable"])
    raise RuntimeError("Cargo returned no scanner-client benchmark executable")


def estimates(raw, case, mode, lanes):
    found = {}
    for path in raw.rglob("new/benchmark.json"):
        identity = json.loads(path.read_text())["full_id"]
        for lane in lanes:
            if identity == f"scanner_clients/{case}/{lane}/{mode}":
                if lane in found:
                    raise ValueError("duplicate Criterion lane")
                value = json.loads(path.with_name("estimates.json").read_text())["mean"]
                found[lane] = {"ns": value["point_estimate"], "interval": value["confidence_interval"],
                               "raw": str(path.parent.relative_to(raw))}
    if set(found) != set(lanes):
        raise ValueError(f"missing Criterion lanes: {case}/{mode}")
    return found


def expected_keys(catalog, repetitions):
    return {(case["id"], mode, repetition) for case in catalog["cases"]
            for mode in case["modes"] for repetition in repetitions}


def admit(records, catalog, repetitions):
    keys = [(r["case"], r["mode"], r["repetition"]) for r in records]
    if len(keys) != len(set(keys)) or set(keys) != expected_keys(catalog, repetitions):
        raise ValueError("duplicate, missing or unexpected scanner-client measurements")
    specifications = {c["id"]: c for c in catalog["cases"]}
    for row in records:
        if row["status"] != "measured" or set(row["lanes"]) != set(specifications[row["case"]]["lanes"]):
            raise ValueError("failed or incomplete scanner-client measurement")
        for value in row["lanes"].values():
            if not 0 < value["ns"] < float("inf"):
                raise ValueError("invalid elapsed time")
            interval = value["interval"]
            if not (0 < interval["confidence_level"] < 1
                    and 0 <= interval["lower_bound"] <= value["ns"] <= interval["upper_bound"] < float("inf")):
                raise ValueError("invalid confidence interval")


def report(directory, catalog, records):
    rows = []
    for case in catalog["cases"]:
        for mode in case["modes"]:
            selected = [r for r in records if r["case"] == case["id"] and r["mode"] == mode]
            for lane in case["lanes"]:
                samples = [r["lanes"][lane]["ns"] for r in selected]
                rows.append(dict(case=case["id"], mode=mode, lane=lane,
                                 median_ns=statistics.median(samples), min_ns=min(samples), max_ns=max(samples)))
    write(directory / "summary.json", rows)
    lines = ["# Scanner application workloads", "", "Separate diagnostic evidence; not a general-regex ranking or aggregate.", "",
             "Times are medians of independent process mean estimates, with their process range. Architectures stay separate.", "",
             "| Case | Cache policy | Phase | Median ms | Process range ms |", "| --- | --- | --- | ---: | ---: |"]
    for row in rows:
        lines.append(f"| {row['case']} | {row['mode']} | {row['lane']} | {row['median_ns']/1e6:.3f} | {row['min_ns']/1e6:.3f}–{row['max_ns']/1e6:.3f} |")
    (directory / "summary.md").write_text("\n".join(lines) + "\n")


def merge(directory, shards):
    if directory.exists():
        raise ValueError("merge output must not exist")
    if len(shards) != 3:
        raise ValueError("merge requires exactly three shards")
    contexts = [json.loads((s / "context.json").read_text()) for s in shards]
    reference = contexts[0]
    for context in contexts:
        for key in ("commit", "platform", "machine", "binary_sha256", "source_sha256", "native_source_sha256", "catalog_sha256",
                    "cargo_lock_sha256", "rustc", "profile", "profile_env", "rustflags", "samples", "seconds", "warmup", "diagnostic"):
            if context[key] != reference[key]:
                raise ValueError(f"shards differ in {key}")
    repetitions = [c["only_run"] for c in contexts]
    if sorted(repetitions) != [1, 2, 3]:
        raise ValueError("merge requires exactly repetitions 1, 2 and 3")
    catalog = json.loads((shards[0] / "catalog.json").read_text())
    records = []
    for shard, context in zip(shards, contexts):
        if digest(shard / "catalog.json") != context["catalog_sha256"]:
            raise ValueError("catalog hash mismatch")
        rows = json.loads((shard / "measurements.json").read_text())
        admit(rows, catalog, [context["only_run"]])
        eligibility = json.loads((shard / "eligibility.json").read_text())
        if ({(r["case"], r["mode"]) for r in eligibility} != {(c["id"], m) for c in catalog["cases"] for m in c["modes"]}
                or len(eligibility) != len(rows)
                or any(r["status"] != "eligible" or not r["c_oracle"] for r in eligibility)):
            raise ValueError("missing independent C eligibility")
        for row in rows:
            raw = shard / "raw" / row["case"] / row["mode"]
            if estimates(raw, row["case"], row["mode"], row["lanes"]) != row["lanes"]:
                raise ValueError("stored estimates differ from retained Criterion evidence")
        records.extend(rows)
    admit(records, catalog, [1, 2, 3])
    directory.mkdir(parents=True)
    for shard, repetition in zip(shards, repetitions):
        shutil.copytree(shard, directory / f"run-{repetition}")
    write(directory / "catalog.json", catalog)
    write(directory / "context.json", dict(reference, only_run=None, repetitions=3))
    write(directory / "measurements.json", records)
    report(directory, catalog, records)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("validate", "run", "merge"))
    parser.add_argument("directory", type=Path)
    parser.add_argument("shards", nargs="*", type=Path)
    parser.add_argument("--only-run", type=int, choices=(1, 2, 3), default=1)
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--measure-seconds", type=float, default=3)
    parser.add_argument("--warmup-seconds", type=float, default=1)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--diagnostic", action="store_true")
    parser.add_argument("--host-note", default="Local run; host load, power and thermal state were not controlled.")
    args = parser.parse_args()
    directory = args.directory.resolve()
    if args.command == "merge":
        merge(directory, [s.resolve() for s in args.shards])
        return
    if args.samples < 10 or args.measure_seconds <= 0 or args.warmup_seconds <= 0 or args.timeout <= 0:
        parser.error("positive time budgets and at least 10 samples are required")
    if not args.diagnostic and (args.samples < 30 or args.measure_seconds < 3 or args.warmup_seconds < 1):
        parser.error("short runs require --diagnostic")
    if os.environ.get("FERRONI_BENCH_PREFILTER") is not None:
        parser.error("the recorded configuration requires automatic default prefiltering")
    if directory.exists():
        parser.error("use a new output directory; evidence must not be overwritten")
    catalog = corpus()
    binary = build()
    native = next(p for p in [Path(os.environ.get("FERRONI_ONIGURUMA_DIR", ROOT / ".cache/upstream/oniguruma-orig")), ROOT / "oniguruma-orig"] if (p / "src").is_dir())
    native_inputs = sorted(p for p in (native / "src").rglob("*") if p.suffix in (".c", ".h"))
    directory.mkdir(parents=True)
    context = dict(commit=output(["git", "rev-parse", "HEAD"]), git_status=output(["git", "status", "--porcelain"]),
                   platform=platform.platform(), machine=platform.machine(), rustc=output(["rustc", "-Vv"]),
                   binary_sha256=digest(binary), catalog_sha256=digest(CORPUS / "catalog.json"),
                   cargo_lock_sha256=digest(ROOT / "Cargo.lock"), profile="bench, thin LTO, features ffi",
                   rustflags=os.environ.get("RUSTFLAGS", ""), profile_env={k:v for k,v in os.environ.items() if k.startswith("CARGO_PROFILE_") or k == "CARGO_ENCODED_RUSTFLAGS"},
                   only_run=args.only_run, samples=args.samples, seconds=args.measure_seconds, warmup=args.warmup_seconds,
                   diagnostic=args.diagnostic or args.command == "validate", started_utc=datetime.now(timezone.utc).isoformat(),
                   host_note=args.host_note, workflow={k:os.environ[k] for k in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_JOB") if k in os.environ},
                   native_source_sha256=hashlib.sha256("".join(str(p.relative_to(native)) + digest(p) for p in native_inputs).encode()).hexdigest(),
                   command=sys.argv, python=sys.version, order="seeded case order; related cache modes alternate by repetition")
    source = directory / "source.tar.gz"
    with tarfile.open(source, "w:gz") as archive:
        for name in ("Cargo.toml", "Cargo.lock", "build.rs", "src", "benches/scanner_clients", "benches/scanner_clients_bench.rs",
                     "benches/cpp_scanner", "benches/battle_inputs.toml", "scripts/prepare-oniguruma-sources.sh", "scripts/compare-scanner-clients.py", ".cargo/config.toml", "rust-toolchain.toml"):
            archive.add(ROOT / name, arcname=name, filter=lambda info: None if "__pycache__" in Path(info.name).parts else info)
        for path in native_inputs:
            archive.add(path, arcname="oracle/oniguruma/" + str(path.relative_to(native)))
    # Tar gzip headers contain timestamps; compare content fingerprints between VMs.
    inputs = sorted(p for base in [ROOT / "src", CORPUS, ROOT / "benches/cpp_scanner"] for p in base.rglob("*") if p.is_file() and "__pycache__" not in p.parts)
    inputs += [ROOT / p for p in ("Cargo.toml", "Cargo.lock", "build.rs", "benches/scanner_clients_bench.rs", "benches/battle_inputs.toml", "scripts/prepare-oniguruma-sources.sh", "scripts/compare-scanner-clients.py", ".cargo/config.toml", "rust-toolchain.toml")]
    context["source_sha256"] = hashlib.sha256("".join(str(p.relative_to(ROOT)) + digest(p) for p in inputs).encode()).hexdigest()
    write(directory / "context.json", context)
    shutil.copy2(CORPUS / "catalog.json", directory / "catalog.json")
    eligibility, records, order = [], [], []
    pairs = [(case, mode) for case in catalog["cases"] for mode in case["modes"]]
    for case, mode in pairs:
        env = dict(os.environ, SCANNER_CLIENT_CASE=case["id"], SCANNER_CLIENT_MODE=mode)
        result = subprocess.run([str(binary), "--validate"], env=env, text=True, capture_output=True, timeout=args.timeout)
        (directory / f"validate-{case['id']}-{mode}.log").write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError(f"validation failed for {case['id']}/{mode}; no timings started")
        rows = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
        if len(rows) != 1 or rows[0].get("case") != case["id"] or rows[0].get("mode") != mode or rows[0].get("status") != "eligible" or not rows[0].get("c_oracle"):
            raise ValueError("invalid independent C validation identity")
        eligibility.extend(rows)
        write(directory / "eligibility.json", eligibility)
    write(directory / "eligibility.json", eligibility)
    if args.command == "validate":
        return
    # Keep related modes paired in case order, rotating which one goes first.
    cases = list(catalog["cases"])
    random.Random(20261016 + args.only_run).shuffle(cases)
    for case in cases:
        modes = case["modes"] if args.only_run % 2 else list(reversed(case["modes"]))
        for mode in modes:
            raw = directory / "raw" / case["id"] / mode
            raw.mkdir(parents=True)
            env = dict(os.environ, SCANNER_CLIENT_CASE=case["id"], SCANNER_CLIENT_MODE=mode, CRITERION_HOME=str(raw))
            command = [str(binary), "--bench", "--noplot", "--sample-size", str(args.samples), "--measurement-time", str(args.measure_seconds), "--warm-up-time", str(args.warmup_seconds)]
            order.append(dict(case=case["id"], mode=mode, command=command))
            write(directory / "order.json", order)
            with (raw / "process.log").open("w") as log:
                try:
                    subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=args.timeout)
                    record = dict(case=case["id"], mode=mode, repetition=args.only_run, status="measured", lanes=estimates(raw, case["id"], mode, case["lanes"]))
                except (subprocess.SubprocessError, ValueError):
                    records.append(dict(case=case["id"], mode=mode, repetition=args.only_run, status="failed", lanes={}))
                    write(directory / "measurements.json", records)
                    raise
            records.append(record)
            write(directory / "measurements.json", records)
    assert digest(binary) == context["binary_sha256"], "binary changed during measurement"
    admit(records, catalog, [args.only_run])
    report(directory, catalog, records)


if __name__ == "__main__":
    main()
