#!/usr/bin/env python3
"""Bounded local comparison of Eager, prepared NFAs, and complete group DFAs.

Artifacts are local build products. The output retains raw Criterion samples,
capture validation, process-isolated memory, compression sizes, and provenance.
No network/download timing or production archive-format promise is implied.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
CASES = ["cpp", "scss", "java", "json", "rust", "html", "astro", "tsx", "c", "php"]
VARIANTS = ["eager", "nfa", "dfa"]


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def command(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, text=True, capture_output=True, **kwargs)


def build(cache, features, label):
    result = {}
    for kind, target in [("example", "scanner_group_archive"), ("bench", "scanner_group_archive_bench")]:
        args = ["cargo", "build" if kind == "example" else "bench", "--offline", "--locked"]
        args += ["--release"] if kind == "example" else ["--no-run"]
        args += ["--features", features, "--" + kind, target, "--message-format=json"]
        print("Building", label, kind, flush=True)
        completed = command(args, timeout=300)
        artifacts = [json.loads(line) for line in completed.stdout.splitlines() if line.startswith("{")]
        executable = next(a["executable"] for a in reversed(artifacts)
                          if a.get("reason") == "compiler-artifact" and a.get("executable")
                          and a["target"]["name"] == target)
        dest = cache / "bin" / (label + ("-bench" if kind == "bench" else ""))
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(executable, dest)
        result[kind] = dest
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--reuse-artifacts", action="store_true")
    parser.add_argument("--prebuilt", action="store_true", help="Use verified binaries from prepare-portable-eager-comparison.py")
    parser.add_argument("--prepare-only", action="store_true", help="Build, export, validate, and compress without timing")
    parser.add_argument("--measure-only", action="store_true", help="Resume validated artifacts without rebuilding or exporting")
    parser.add_argument("--variants", nargs="+", choices=["eager", "control", "nfa", "dfa", "dense", "prepared", "portable", "before"], default=VARIANTS)
    parser.add_argument("--prepare-dense", action="store_true")
    parser.add_argument("--before-artifacts", type=Path, help="Artifact directory understood by the frozen before binary")
    parser.add_argument("--repetitions", type=int, default=3)
    args = parser.parse_args()
    if not (args.prebuilt or args.measure_only) and any(v in args.variants for v in ["prepared", "before"]):
        parser.error("prepared and before are frozen historical comparators; use --prebuilt with their verified binaries")
    out, cache = args.output.resolve(), args.cache.resolve()
    if args.measure_only:
        context = json.loads((out / "context.json").read_text())
        binaries = {label: {kind: cache / "bin" / (label + ("-bench" if kind == "bench" else ""))
                            for kind in ["example", "bench"]} for label in context["binaries"]}
        for label, bins in binaries.items():
            for kind, path in bins.items():
                assert hashlib.sha256(path.read_bytes()).hexdigest() == context["binaries"][label][kind]
        for source, digest in context["source_sha256"].items():
            if source != "scripts/compare-scanner-group-archives.py":
                assert hashlib.sha256((ROOT / source).read_bytes()).hexdigest() == digest, source
        context["preparation_runner_sha256"] = context["source_sha256"]["scripts/compare-scanner-group-archives.py"]
        context["source_sha256"]["scripts/compare-scanner-group-archives.py"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        context["boundaries"]["memory"] = "fresh Python wrapper process using resource.getrusage(RUSAGE_CHILDREN) for one driver child; Darwin ru_maxrss bytes; fixture parsing, archive bytes/indexes, decoded programs, scanners, and four search passes"
        write(out / "context.json", context)
        measure(out, cache, binaries, args.repetitions, context.get("variants", args.variants), context.get("before_artifacts"))
        return
    out.mkdir(parents=True, exist_ok=False)
    artifacts = cache / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    if args.prebuilt:
        assert args.reuse_artifacts
        provenance = json.loads((cache / "build-provenance.json").read_text())
        labels = ["eager"] + [label for label in ["portable", "prepared", "before"]
                              if label in args.variants or (label == "portable" and "control" in args.variants)]
        binaries = {label: {kind: cache / "bin" / (label + ("-bench" if kind == "bench" else "")) for kind in ["example", "bench"]} for label in labels}
        for bins in binaries.values():
            for path in bins.values():
                assert hashlib.sha256(path.read_bytes()).hexdigest() == provenance["binaries"][path.name]
    else:
        binaries = {"eager": build(cache, "precompiled", "eager")}

    if not args.prebuilt and "nfa" in args.variants: binaries["nfa"] = build(cache, "scanner-group-nfa-prototype", "nfa")
    if not args.prebuilt and ("dfa" in args.variants or (not args.reuse_artifacts and "prepared" not in args.variants)): binaries["prototype"] = build(cache, "scanner-group-prototype", "prototype")
    if not args.prebuilt and "dense" in args.variants: binaries["dense"] = build(cache, "scanner-group-dense-prototype", "dense")
    if not args.prebuilt and "prepared" in args.variants: binaries["prepared"] = build(cache, "prepared-eager-prototype", "prepared")
    # Capture the exact source state, including new, uncommitted experiment files.
    files = command(["git", "ls-files", "-m", "-o", "--exclude-standard"]).stdout.splitlines()
    files = [f for f in files if (ROOT / f).is_file()]
    context = {
        "classification": "local diagnostic comparison",
        "base_commit": command(["git", "rev-parse", "HEAD"]).stdout.strip(),
        "branch": command(["git", "branch", "--show-current"]).stdout.strip(),
        "source_sha256": {f: hashlib.sha256((ROOT / f).read_bytes()).hexdigest() for f in files},
        "rustc": command(["rustc", "-Vv"]).stdout,
        "platform": platform.platform(), "machine": platform.machine(),
        "hardware": command(["system_profiler", "SPHardwareDataType", "-detailLevel", "mini"]).stdout,
        "binaries": {label: {kind: hashlib.sha256(path.read_bytes()).hexdigest() for kind, path in bins.items()}
                     for label, bins in binaries.items()},
        "fixture_sha256": {case: hashlib.sha256((ROOT / f"benches/{case}_scanner/trace.json").read_bytes()).hexdigest() for case in CASES},
        "repetitions": args.repetitions, "variants": args.variants,
        "before_artifacts": str(args.before_artifacts.resolve()) if args.before_artifacts else None,
        "criterion": {"samples": 40, "warmup_seconds": 0.3, "measurement_seconds": 1, "bootstrap_resamples": 10000},
        "build_environment": {key: os.environ.get(key) for key in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_BENCH_LTO"]},
        "dfa_limits": {"dense_bytes_per_automaton": 16 << 20, "determinize_workspace_bytes": 64 << 20,
                       "automata_per_group": 3, "fallback": "prepared NFA"},
        "boundaries": {
            "load": "in-memory archive indexing, structural validation, decoding, helpers, scanner construction; excludes input I/O, gzip, and teardown",
            "search": "three warm document passes, then captured UTF-16 calls with fresh OnigString identities outside timing; includes captures",
            "cold_document": "load plus first complete replay; string preparation and teardown outside timing",
            "memory": "fresh Python wrapper using resource.getrusage(RUSAGE_CHILDREN) for one child (Darwin ru_maxrss bytes); fixture parsing, archive bytes/indexes, decoded programs, scanners, four search passes retained",
            "compressed": "gzip -9 equivalent via Python gzip.compress, mtime=0; a u32 program length plus program archive and group bytes as one payload",
            "decompress": "CPython gzip/zlib, nine timed calls after one warmup; kept separate from Rust load timing",
            "control": "when selected as a variant, three paired repetitions of ordinary Eager with the prepared feature compiled; otherwise one diagnostic process; no prepared archive loaded"},
        "limits": ["Local macOS arm64 evidence only; no CDN transfer, complete grammar enumeration, or Ferriki end-to-end benchmark.",
                   "Thermals and unrelated host background activity were not controlled.",
                   "Wire data and public methods behind prototype features are experimental; the prepared variant uses locally patched, pinned dependency snapshot APIs.",
                   "RSS includes fixtures and parser high-water marks; allocator/automata counters do not represent all allocations."]}
    if args.prebuilt:
        context["isolated_build_provenance"] = provenance
        context["boundaries"]["control"] = "three paired repetitions of ordinary Eager through the current portable-feature executable, without loading any group image"
        context["limits"].append("Portable bytecode covers scanner groups; ordinary per-program literal helpers are rebuilt by unchanged Eager. The prepared reference also contains exact per-program helper snapshots.")
    # Avoid retaining hardware serials or UUIDs in the report.
    context["hardware"] = "\n".join(line.strip() for line in context["hardware"].splitlines()
                                   if any(k in line for k in ["Chip:", "Memory:", "Total Number of Cores:", "Model Name:"]))
    write(out / "context.json", context)
    prepared, validations, sizes, memory = [], [], [], []
    for case in CASES:
        if not args.reuse_artifacts:
            print("Preparing", case, flush=True)
            p = command([str(binaries["prepared" if "prepared" in args.variants else "prototype"]["example"]), "prepare-eager" if "prepared" in args.variants else "prepare", case, str(artifacts)], timeout=600)
            prepared.extend(json.loads(line) for line in p.stdout.splitlines())
            (out / f"{case}-prepare.stderr").write_text(p.stderr)
            write(out / "preparation.json", prepared)
        if args.prepare_dense:
            print("Preparing Dense", case, flush=True)
            before = {ext: hashlib.sha256((artifacts / f"{case}.{ext}").read_bytes()).hexdigest() for ext in ["programs", "nfa"]}
            p = command([str(binaries["dense"]["example"]), "prepare", case, str(artifacts)], timeout=600)
            prepared.extend(json.loads(line) for line in p.stdout.splitlines())
            write(out / "preparation.json", prepared)
            for ext, digest in before.items():
                assert hashlib.sha256((artifacts / f"{case}.{ext}").read_bytes()).hexdigest() == digest, "shared program and NFA payloads must remain identical"
        programs = (artifacts / f"{case}.programs").read_bytes()
        for variant in args.variants:
            bins = binaries[{"eager":"eager", "nfa":"nfa", "dfa":"prototype", "dense":"dense", "prepared":"prepared", "portable":"portable", "before":"before", "control":"portable" if "portable" in binaries else "prepared"}[variant]]
            variant_artifacts = args.before_artifacts.resolve() if variant == "before" and args.before_artifacts else artifacts
            assert (variant_artifacts / f"{case}.programs").read_bytes() == programs
            validated = command([str(bins["example"]), "validate", case, str(variant_artifacts), "portable" if variant == "before" else variant], timeout=90)
            validation = json.loads(validated.stdout)
            validation["variant"] = variant
            validations.append(validation)
            write(out / "validation.json", validations)
            groups = b"" if variant in ["eager", "control"] else (variant_artifacts / f"{case}.{'portable' if variant == 'before' else variant}").read_bytes()
            payload = struct.pack("<I", len(programs)) + programs + groups
            compressed = gzip.compress(payload, compresslevel=9, mtime=0)
            assert gzip.decompress(compressed) == payload
            times = []
            for i in range(10):
                start = time.perf_counter_ns()
                decoded = gzip.decompress(compressed)
                elapsed = time.perf_counter_ns() - start
                if i: times.append(elapsed)
                del decoded
            sizes.append({"corpus": case, "variant": variant, "program_bytes": len(programs),
                          "group_bytes": len(groups), "raw_bytes": len(payload), "gzip9_bytes": len(compressed),
                          "payload_sha256": hashlib.sha256(payload).hexdigest(), "gzip_sha256": hashlib.sha256(compressed).hexdigest(),
                          "decompress_ns": times, "decompress_median_ns": statistics.median(times)})
            write(out / "sizes.json", sizes)
        print("Validated and compressed", case, flush=True)
    if not args.prepare_only:
        measure(out, cache, binaries, args.repetitions, args.variants, context["before_artifacts"])


def measure(out, cache, binaries, repetitions, variants, before_artifacts=None):
    # All lanes must reproduce every recorded capture before timing starts.
    memory = []
    artifacts = cache / "artifacts"
    for row in json.loads((out / "sizes.json").read_text()):
        selected = Path(before_artifacts) if row["variant"] == "before" and before_artifacts else artifacts
        programs = (selected / f"{row['corpus']}.programs").read_bytes()
        variant = "portable" if row["variant"] == "before" else row["variant"]
        groups = b"" if variant in ["eager", "control"] else (selected / f"{row['corpus']}.{variant}").read_bytes()
        payload = struct.pack("<I", len(programs)) + programs + groups
        assert hashlib.sha256(payload).hexdigest() == row["payload_sha256"], (row["corpus"], row["variant"])
    for repetition in range(repetitions):
        order = variants[repetition % len(variants):] + variants[:repetition % len(variants)]
        cases = CASES[repetition:] + CASES[:repetition]
        for case in cases:
            for variant in order + (["control"] if repetition == 0 and "control" not in variants else []):
                label = f"r{repetition + 1}-{case}-{variant}"
                bins = binaries[{"eager":"eager", "nfa":"nfa", "dfa":"prototype", "dense":"dense", "prepared":"prepared", "portable":"portable", "before":"before", "control":"portable" if "portable" in binaries else "prepared" if "prepared" in binaries else ("dense" if "dense" in binaries else "prototype")}[variant]]
                helper = """import json,resource,subprocess,sys
p=subprocess.run(sys.argv[1:],capture_output=True,text=True,check=True)
r=json.loads(p.stdout)
r['max_rss_bytes']=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
print(json.dumps(r))
"""
                variant_artifacts = Path(before_artifacts) if variant == "before" and before_artifacts else artifacts
                timed = command([sys.executable, "-c", helper, str(bins["example"]), "memory", case,
                                 str(variant_artifacts), "portable" if variant == "before" else variant], timeout=90)
                row = json.loads(timed.stdout)
                row["variant"] = variant
                row["repetition"] = repetition + 1
                memory.append(row)
                write(out / "memory.json", memory)
                print("Benchmark", label, flush=True)
                env = {**os.environ, "LC_ALL": "C", "GROUP_ARCHIVE_CORPUS": case,
                       "GROUP_ARCHIVE_VARIANT": "portable" if variant == "before" else variant, "GROUP_ARCHIVE_DIR": str(variant_artifacts),
                       "GROUP_ARCHIVE_OUTPUT": str(out / "criterion" / label)}
                completed = subprocess.run([str(bins["bench"]), "--bench", "--noplot"], cwd=ROOT, env=env,
                                           text=True, capture_output=True, timeout=180, check=True)
                (out / (label + ".log")).write_text(completed.stdout + completed.stderr)
    print("Finished", out, flush=True)


if __name__ == "__main__":
    main()
