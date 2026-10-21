#!/usr/bin/env python3
"""Validate, measure, and report the captured TextMate scanner replays.

The scanner lane of the Blacksmith comparison, separate from the source-backed
regex corpus (compare-production.py) and with the same evidence rules: every
case/engine pair is validated against the captured results before anything is
timed, each process repetition can run on its own host (`run --only-run N`),
`merge` joins one platform's repetitions and rejects a missing cell or a
different build, and Criterion's raw estimates and samples are retained. A
ratio only divides times from one repetition, so from one host. There is no
cross-case aggregate, and these numbers enter neither the regex ranking nor the
website (ADR-010).

The cases are the `document` and `cold_document` lanes of the replays in
REPLAYS, one per complexity tier. The engine profiles and the shared call
boundary are in PROFILES and ADR-010.

  compare-scanner-replays.py validate OUT
  compare-scanner-replays.py run OUT [--only-run N] [--cases ...] [--engines ...]
  compare-scanner-replays.py merge OUT RUN1 RUN2 RUN3
  compare-scanner-replays.py report OUT
"""

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Host description, labels and benchmark IDs of the internal diagnostic
# runner, and the balanced run order of the regex comparison.
ENGINE_RUNNER = load("compare_engines", "compare-engines.py")
PRODUCTION = load("compare_production", "compare-production.py")

# The lane's replays, one per complexity tier (benches/<replay>/trace.json),
# and the bench target that registers each. The other scanner replays stay
# local diagnostics (compare-engines.py --cases textmate): Ferroni is a regex
# engine first, and highlighting breadth belongs to Ferriki (ADR-010).
REPLAYS = {
    "json_scanner": "ferriki_scanner_bench",
    "rust_scanner": "ferriki_scanner_bench",
    "tsx_scanner": "ferriki_scanner_bench",
    "html_scanner": "ferriki_scanner_bench",
}

ENGINES = ("rust", "c", "onigmo", "pcre2_jit", "pcre2", "fancy_regex", "fancy_regex_seek", "fancy_regex_set")
# Every engine gets the same call boundary: a UTF-16 start converted through
# the subject's offset table, the search, and the winning pattern index with
# its captures in UTF-16 code units, materialized with an inline capacity of
# eight. Subjects, their tables and string identities are prepared outside
# timing. The search strategy is each engine's own.
PROFILES = {
    "rust": "Scanner::find_next_match_utf16 on fresh OnigString wrappers, no caller cache ID; "
            "default ScannerConfig (DFA pre-filter on)",
    "c": "vscode-oniguruma's C scanner (benches/vscode_scanner_native.c) over C Oniguruma: RegSet position-lead "
         "search below 1000 bytes, per-pattern loop with its string cache above; fresh cache IDs per replay",
    "onigmo": "pinned Ruby Onigmo, earliest match over the scanner's patterns in order, one region per pattern",
    "pcre2_jit": "PCRE2 UTF/UCP with JIT, earliest match over the scanner's patterns in order, NO_UTF_CHECK",
    "pcre2": "PCRE2 UTF/UCP interpreter, earliest match over the scanner's patterns in order, NO_UTF_CHECK",
    "fancy_regex": "fancy-regex Oniguruma mode, seek off, earliest match over the scanner's patterns in order",
    "fancy_regex_seek": "fancy-regex Oniguruma mode, seek on, earliest match over the scanner's patterns in order",
    "fancy_regex_set": "fancy-regex RegexSet over each scanner's patterns, built once per scanner",
}
LABELS = ENGINE_RUNNER.LABELS
# Every replay of the lane runs both.
LANES = {
    "document": "warm scanners: the document's calls in order, scanners compiled and reused before timing",
    "cold_document": "one-shot: every scanner of the trace compiled, then the document replayed once",
}
# The two Oniguruma implementations must reproduce every captured result; a
# comparator that cannot is reported, not timed.
REQUIRED = ("rust", "c")
TIMED_STATUS = "eligible"


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def command_output(command):
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=True).stdout.strip()


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def lane_cases(replays=REPLAYS):
    return [f"{replay}/{lane}" for replay in replays for lane in LANES]


def scanner_group(case):
    return case.split("/", 1)[0]


def bench_id(case, engine):
    return ENGINE_RUNNER.variant(case, engine)


def build(targets, features):
    result = subprocess.run(
        ["cargo", "bench", "--locked", "--features", features, "--no-run", "--message-format=json",
         *[argument for target in targets for argument in ("--bench", target)]],
        cwd=ROOT, env=dict(os.environ, PCRE2_SYS_STATIC="1"), text=True, stdout=subprocess.PIPE, check=True)
    binaries = {}
    for line in result.stdout.splitlines():
        record = json.loads(line)
        if (record.get("reason") == "compiler-artifact" and record.get("executable")
                and record["target"]["name"] in targets):
            binaries[record["target"]["name"]] = Path(record["executable"])
    if set(binaries) != set(targets):
        raise RuntimeError(f"Cargo did not return executables for {sorted(set(targets) - set(binaries))}")
    return binaries


def source_files(targets):
    """Every input of the measured binaries and of this runner, for the snapshot."""
    paths = ["Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml", ".cargo/config.toml", "LICENSE", "src",
             "benches/engine_replay.rs", "benches/engines.rs",
             "benches/onigmo_shim.c", "benches/vscode_scanner_native.c", "benches/battle_inputs.toml",
             "scripts/compare-scanner-replays.py", "scripts/compare-engines.py", "scripts/compare-production.py"]
    paths += [f"benches/{target}.rs" for target in targets]
    # The trace directories, with the shared replay module in benches/cpp_scanner.
    paths += sorted(str(path.parent.relative_to(ROOT)) for path in ROOT.glob("benches/*_scanner/trace.json"))
    return paths


def fingerprint(paths):
    """A content hash that does not depend on file times, so hosts can compare it."""
    digest = hashlib.sha256()
    for path in paths:
        base = ROOT / path
        for file in sorted([base] if base.is_file() else (p for p in base.rglob("*") if p.is_file())):
            if "__pycache__" not in file.parts:
                digest.update(f"{file.relative_to(ROOT)}\0{sha256(file)}\n".encode())
    return digest.hexdigest()


def snapshot_source(directory, paths):
    archive = directory / "source-snapshot.tar.gz"
    native = {}
    with tarfile.open(archive, "w:gz") as bundle:
        for path in paths:
            bundle.add(ROOT / path, arcname=path,
                       filter=lambda info: None if "__pycache__" in Path(info.name).parts else info)
        for name, variable, fallback in (
            ("oniguruma", "FERRONI_ONIGURUMA_DIR", ".cache/upstream/oniguruma-orig"),
            ("onigmo", "FERRONI_ONIGMO_DIR", ".cache/upstream/onigmo-ruby"),
        ):
            path = Path(os.environ.get(variable, str(ROOT / fallback))).resolve()
            if path.exists():
                bundle.add(path, arcname=fallback)
                stamp = path / ".ferroni-upstream-commit"
                native[name] = stamp.read_text().strip() if stamp.exists() else None
    return sha256(archive), native


def parse_notes(log):
    """The VALIDATED, UNSUPPORTED and EQUIVALENT lines of a replay bench, by benchmark ID."""
    notes = {}
    for line in log.splitlines():
        kind, _, payload = line.partition(" ")
        if kind in ("VALIDATED", "UNSUPPORTED", "EQUIVALENT"):
            note = json.loads(payload)
            if note["id"] in notes:
                raise RuntimeError(f"two setup records for {note['id']}")
            notes[note["id"]] = dict(note, kind=kind)
    return notes


def tested(log):
    """Benchmark IDs whose single `--test` iteration succeeded."""
    lines = log.splitlines()
    return {line.removeprefix("Testing ") for line, after in zip(lines, lines[1:] + [""])
            if line.startswith("Testing ") and after == "Success"}


def eligibility_record(case, engine, notes, passed):
    identity = {"case": case, "engine": engine, "benchmark": bench_id(case, engine)}
    note = notes.get(identity["benchmark"])
    if note is None:
        return dict(identity, status="missing", reason="the bench printed no setup record for this benchmark")
    if note["kind"] == "UNSUPPORTED":
        return dict(identity, status="unsupported", reason=note["reason"])
    if note["kind"] == "EQUIVALENT":
        # Highlighting would match, but the captured results are not
        # reproduced: an empty group reads as unset or the reverse.
        return dict(identity, status="empty_capture_difference", calls=note["calls"],
                    reason=f"{note['empty_capture_differences']} calls report an empty group as unset or the reverse")
    if identity["benchmark"] not in passed:
        return dict(identity, status="execution_error", reason="validated, but its test iteration did not succeed")
    return dict(identity, status=TIMED_STATUS, calls=note["calls"])


def fatal_preflight(records):
    """Comparator gaps are reportable; a broken harness or Oniguruma mismatch stops timings."""
    return [record for record in records
            if record["status"] in ("missing", "execution_error", "timeout")
            or (record["engine"] in REQUIRED and record["status"] != TIMED_STATUS)]


def preflight(binaries, groups, engines, directory, timeout):
    """One unfiltered setup per bench: validates every replay, then runs each selected benchmark once."""
    (directory / "validation").mkdir()
    notes, passed, failures = {}, set(), {}
    for target, binary in sorted(binaries.items()):
        mine = sorted(group for group in groups if REPLAYS[group] == target)
        pattern = "^(" + "|".join(mine) + ")/(cold_)?document(_[a-z0-9_]+)?$"
        try:
            env = {key: value for key, value in os.environ.items() if key != "SCANNER_REPLAY_ID"}
            result = subprocess.run([str(binary), pattern, "--test"], env=env, text=True, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=timeout)
            log, failure = result.stdout, result.returncode and f"exit status {result.returncode}"
        except subprocess.TimeoutExpired as error:
            output = error.stdout or b""
            log = output.decode(errors="replace") if isinstance(output, bytes) else output
            failure = f"validation exceeded {timeout}s"
        (directory / "validation" / f"{target}.log").write_text(log)
        notes.update(parse_notes(log))
        passed |= tested(log)
        if failure:
            failures.update({group: f"{target}: {failure}; see validation/{target}.log: {log[-1500:]}" for group in mine})
    cases = lane_cases(groups)
    records = []
    for case in cases:
        for engine in engines:
            if scanner_group(case) in failures:
                records.append({"case": case, "engine": engine, "benchmark": bench_id(case, engine),
                                "status": "execution_error", "reason": failures[scanner_group(case)]})
            else:
                records.append(eligibility_record(case, engine, notes, passed))
    return cases, records


def estimate(raw, benchmark):
    found = []
    for path in raw.rglob("new/benchmark.json"):
        if json.loads(path.read_text())["full_id"] == benchmark:
            mean = json.loads(path.with_name("estimates.json").read_text())["mean"]
            sample = json.loads(path.with_name("sample.json").read_text())
            found.append({"ns": mean["point_estimate"],
                          "lower_ns": mean["confidence_interval"]["lower_bound"],
                          "upper_ns": mean["confidence_interval"]["upper_bound"],
                          "sampling_mode": sample.get("sampling_mode"),
                          "iterations": sum(sample["iters"]), "samples": len(sample["iters"]),
                          "raw_directory": str(path.parent.relative_to(raw))})
    if len(found) != 1:
        raise RuntimeError(f"expected one Criterion estimate for {benchmark}, found {len(found)}")
    return found[0]


def timed_environment(benchmark, raw):
    return dict(os.environ, SCANNER_REPLAY_ID=benchmark, CRITERION_HOME=str(raw))


def measure(binary, benchmark, raw, args, environment_size):
    raw.mkdir(parents=True)
    command = [str(binary), "--exact", benchmark, "--bench", "--noplot", "--sample-size", str(args.samples),
               "--measurement-time", str(args.measure_seconds), "--warm-up-time", str(args.warmup_seconds)]
    # One environment size per repetition, as in the regex comparison; see
    # ALIGNMENT_VARIABLE in compare-production.py.
    env = PRODUCTION.aligned_environment(timed_environment(benchmark, raw), environment_size)
    with (raw / "process.log").open("w") as log:
        result = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=args.timeout)
    if result.returncode:
        raise RuntimeError(f"benchmark exited {result.returncode}")
    # The timed process checks its own replay against the capture first.
    if parse_notes((raw / "process.log").read_text()).get(benchmark, {}).get("kind") != "VALIDATED":
        raise RuntimeError("the timed process did not validate its replay")
    return command, estimate(raw, benchmark)


def summarize(cases, engines, measurements, repetitions):
    """Per case and engine: Ferroni's time, and each comparator's ratio to it within each repetition.

    `repetitions` lists the repetitions to use: all of a merged run, one of a shard.
    """
    by_key = {(r["case"], r["engine"], r["repetition"]): r["estimate"] for r in measurements if r["status"] == "measured"}
    rows = []
    for case in cases:
        base = [by_key.get((case, "rust", rep)) for rep in repetitions]
        if not all(base):
            continue
        row = {"case": case, "ferroni_ns": statistics.median(b["ns"] for b in base),
               "ferroni_range_ns": [min(b["ns"] for b in base), max(b["ns"] for b in base)], "ratios": {}}
        for engine in engines:
            other = [by_key.get((case, engine, rep)) for rep in repetitions]
            if engine == "rust" or not all(other):
                continue
            ratios = [o["ns"] / b["ns"] for b, o in zip(base, other)]
            lower = min(o["lower_ns"] / b["upper_ns"] for b, o in zip(base, other))
            upper = max(o["upper_ns"] / b["lower_ns"] for b, o in zip(base, other))
            conclusion = "inconclusive"
            if len(repetitions) >= 3 and lower > 1:
                conclusion = "Ferroni faster in every run interval"
            elif len(repetitions) >= 3 and upper < 1:
                conclusion = "comparator faster in every run interval"
            row["ratios"][engine] = {"median": statistics.median(ratios), "range": [min(ratios), max(ratios)],
                                     "interval_envelope": [lower, upper], "conclusion": conclusion}
        rows.append(row)
    return rows


def ratio_text(value):
    return f"{value:.3g}" if value < 100 else f"{value:.0f}"


def report(directory):
    context = json.loads((directory / "context.json").read_text())
    eligibility = json.loads((directory / "eligibility.json").read_text())
    measurements = json.loads((directory / "measurements.json").read_text()) if (directory / "measurements.json").exists() else []
    repetitions = [context["only_run"] - 1] if context.get("only_run") else list(range(context["repetitions"]))
    rows = summarize(context["cases"], context["engines"], measurements, repetitions)
    write_json(directory / "summary.json", {"lanes": LANES, "profiles": context["profiles"], "cases": rows,
                                            "aggregate_scope": "none: every row is its own case and lane"})
    hosts = context.get("hosts") or [context]
    lines = ["# TextMate scanner replays", "",
             f"Host: {context['platform']}; {context['machine']}; {(hosts[0].get('host') or {}).get('cpu', 'CPU not recorded')}.", "",
             "Diagnostic evidence; no publication claim." if context["diagnostic"] else
             "Publication candidate: review host stability, raw samples, eligibility and run intervals before publishing.", "",
             "Complete captured documents of scanner calls, replayed through each engine with the same call boundary "
             "(ADR-010). Every number below concerns one case and lane; there is no cross-case aggregate, and these "
             "results are not part of the regex comparison's ranking or the website.", "",
             (f"Ferroni: median of {len(repetitions)} process means, with their range. Other columns: comparator "
              "time / Ferroni time within the same repetition (and so the same host), median and range; above 1, "
              "Ferroni is faster. **Bold**: every repetition's confidence intervals put the comparator on the same "
              "side of Ferroni." if len(repetitions) > 1 else
              "One repetition (a shard before `merge`). Ferroni: its process mean. Other columns: comparator time / "
              "Ferroni time; above 1, Ferroni is faster. One repetition establishes no direction.")
             + " `n/a`: the engine does not reproduce the captured results (see Eligibility).", ""]
    status = {(r["case"], r["engine"]): r for r in eligibility}
    engines = [engine for engine in context["engines"] if engine != "rust"]
    for lane, description in LANES.items():
        lane_rows = [row for row in rows if row["case"].endswith(f"/{lane}")]
        if not lane_rows:
            continue
        lines += [f"## {lane}", "", f"{description.capitalize()}.", "",
                  "| Case | Calls | Ferroni | " + " | ".join(LABELS[e] for e in engines) + " |",
                  "| --- | ---: | ---: |" + " ---: |" * len(engines)]
        for row in lane_rows:
            low, high = row["ferroni_range_ns"]
            cells = []
            for engine in engines:
                ratio = row["ratios"].get(engine)
                if ratio:
                    text = ratio_text(ratio["median"])
                    if len(repetitions) > 1:
                        text += f" ({ratio_text(ratio['range'][0])}–{ratio_text(ratio['range'][1])})"
                    cells.append(f"**{text}**" if ratio["conclusion"] != "inconclusive" else text)
                elif status.get((row["case"], engine), {}).get("status") != TIMED_STATUS:
                    cells.append("n/a")
                else:
                    cells.append("missing")
            calls = status[(row["case"], "rust")].get("calls", "")
            ferroni = ENGINE_RUNNER.ns(row["ferroni_ns"])
            if len(repetitions) > 1:
                ferroni += f" ({ENGINE_RUNNER.ns(low)}–{ENGINE_RUNNER.ns(high)})"
            lines.append(f"| `{scanner_group(row['case'])}` | {calls} | {ferroni} | " + " | ".join(cells) + " |")
        lines.append("")
    gaps = [record for record in eligibility if record["status"] != TIMED_STATUS]
    lines += ["## Eligibility", "",
              "Every other case/engine pair reproduced every captured result before timing." if gaps else
              "Every case/engine pair reproduced every captured result before timing.", ""]
    if gaps:
        lines += ["| Case | Engine | Status | Reason |", "| --- | --- | --- | --- |"]
        lines += [f"| `{r['case']}` | {r['engine']} | {r['status']} | {ENGINE_RUNNER.short_reason(r.get('reason', ''))} |"
                  for r in gaps]
        lines.append("")
    binaries = [host.get("binaries") for host in hosts]
    lines += ["## Engines and build", "",
              *[f"- {LABELS[engine]}: {context['profiles'][engine]}." for engine in context["engines"]], "",
              f"Commit `{context['commit'][:12]}`{' with local changes' if context['git_status'] else ''}; "
              f"{context['build_profile']}; {context['rustc'].splitlines()[0]}; {context['cc']}.",
              "Bench binaries identical across repetitions." if all(b == binaries[0] for b in binaries) else
              "Bench binaries differ between repetitions although their build inputs match; see context.json.", "",
              "Raw Criterion samples and estimates, process logs, validation logs, eligibility reasons, execution "
              "order and the source snapshot are retained beside this report.", ""]
    (directory / "summary.md").write_text("\n".join(lines))


# Everything that defines the build and the measurement; merged shards must agree.
SHARED_CONTEXT = ("format_version", "lane", "commit", "git_status", "build_profile", "features", "source_sha256",
                  "trace_sha256", "cargo_lock_sha256", "rustflags", "rustc", "cargo", "cc", "native_sources",
                  "engine_source_overrides", "pcre2_build", "engines", "profiles", "cases", "repetitions", "samples",
                  "measure_seconds", "warmup_seconds", "timeout", "seed", "diagnostic", "order_policy", "machine",
                  "system", "environment_alignment")
HOST_CONTEXT = ("only_run", "started_utc", "platform", "host", "host_note", "python", "binaries",
                "source_snapshot_sha256", "command", "environment_sizes")


def admit(context, eligibility, measurements, repetitions, directory):
    """Every eligible cell measured exactly once per repetition, with sane retained estimates."""
    expected = {(r["case"], r["engine"], rep) for r in eligibility if r["status"] == TIMED_STATUS
                for rep in repetitions}
    found = [(r["case"], r["engine"], r["repetition"]) for r in measurements]
    problems = []
    if len(found) != len(set(found)):
        problems.append("duplicate measurements")
    missing, extra = expected - set(found), set(found) - expected
    problems += [f"missing {case}/{engine} in run {rep + 1}" for case, engine, rep in sorted(missing)]
    problems += [f"unexpected {case}/{engine} in run {rep + 1}" for case, engine, rep in sorted(extra)]
    for record in measurements:
        cell = f"{record['case']}/{record['engine']} in run {record['repetition'] + 1}"
        if record["status"] != "measured":
            problems.append(f"{cell}: {record['status']}")
            continue
        value = record["estimate"]
        if not 0 < value["lower_ns"] <= value["ns"] <= value["upper_ns"] < float("inf"):
            problems.append(f"{cell}: invalid estimate or interval")
        elif estimate(directory / record["raw"], bench_id(record["case"], record["engine"])) != value:
            problems.append(f"{cell}: stored estimate differs from the retained Criterion files")
    if problems:
        raise SystemExit("Rejected:\n  " + "\n  ".join(problems))


def merge(directory, shards):
    """Combine one-repetition shards from separate hosts of one platform into one evidence directory."""
    contexts = [json.loads((shard / "context.json").read_text()) for shard in shards]
    contexts, shards = zip(*sorted(zip(contexts, shards), key=lambda pair: pair[0].get("only_run") or 0))
    first = contexts[0]
    if [context.get("only_run") for context in contexts] != list(range(1, first["repetitions"] + 1)):
        raise SystemExit("merge needs exactly one shard per repetition")
    for context, shard in zip(contexts, shards):
        differing = [key for key in SHARED_CONTEXT if context.get(key) != first.get(key)]
        if differing:
            raise SystemExit(f"{shard} has a different build or configuration: {', '.join(differing)}")
        for name in ("eligibility.json", "Cargo.lock"):
            if (shard / name).read_bytes() != (shards[0] / name).read_bytes():
                raise SystemExit(f"{shard} differs in {name}")
    eligibility = json.loads((shards[0] / "eligibility.json").read_text())
    measurements = []
    for run, shard in enumerate(shards, 1):
        records = json.loads((shard / "measurements.json").read_text())
        admit(first, eligibility, records, [run - 1], shard)
        measurements += records
    if directory.exists() and any(directory.iterdir()):
        raise SystemExit("use a new empty output directory; retained evidence must not be overwritten")
    directory.mkdir(parents=True, exist_ok=True)
    for name in ("eligibility.json", "Cargo.lock", "source-snapshot.tar.gz"):
        shutil.copyfile(shards[0] / name, directory / name)
    order = []
    for run, shard in enumerate(shards, 1):
        order += json.loads((shard / "order.json").read_text())
        shutil.copytree(shard / "raw" / f"run-{run}", directory / "raw" / f"run-{run}")
        shutil.copytree(shard / "validation", directory / "validation" / f"run-{run}")
    context = {key: value for key, value in first.items() if key not in HOST_CONTEXT}
    context["hosts"] = [{key: shard_context.get(key) for key in HOST_CONTEXT} for shard_context in contexts]
    context["platform"], context["host_note"] = first["platform"], " ".join(
        dict.fromkeys(shard_context["host_note"] for shard_context in contexts))
    write_json(directory / "context.json", context)
    write_json(directory / "order.json", order)
    write_json(directory / "measurements.json", measurements)
    admit(context, eligibility, measurements, range(first["repetitions"]), directory)
    report(directory)
    print(f"Evidence: {directory / 'summary.md'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=("validate", "run", "merge", "report"))
    parser.add_argument("directory", type=Path)
    parser.add_argument("shards", nargs="*", type=Path)
    parser.add_argument("--cases", help="comma-separated case IDs such as cpp_scanner/document (default: all)")
    parser.add_argument("--engines", default=",".join(ENGINES))
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--measure-seconds", type=float, default=3)
    parser.add_argument("--warmup-seconds", type=float, default=1)
    parser.add_argument("--only-run", type=int, help="measure only this repetition (1-based), for one host per repetition")
    parser.add_argument("--timeout", type=float, default=300, help="seconds per timed process")
    parser.add_argument("--validation-timeout", type=float, default=1200, help="seconds per bench validation")
    parser.add_argument("--seed", type=int, default=20261016)
    parser.add_argument("--diagnostic", action="store_true", help="allow a short smoke run; cannot support publication")
    parser.add_argument("--host-note", default="Host load, power and thermal state not recorded; assess before publication.")
    args = parser.parse_args()
    directory = args.directory.resolve()
    if args.command == "report":
        report(directory)
        return
    if args.command == "merge":
        merge(directory, [shard.resolve() for shard in args.shards])
        return
    if args.repetitions < 1 or args.samples < 10 or min(args.measure_seconds, args.warmup_seconds, args.timeout) <= 0:
        parser.error("positive durations, at least one repetition and ten samples required")
    if args.only_run is not None and not 1 <= args.only_run <= args.repetitions:
        parser.error("--only-run must name one of the repetitions")
    if args.command == "run" and not args.diagnostic and (args.repetitions < 3 or args.samples < 30
                                                          or args.measure_seconds < 3 or args.warmup_seconds < 1):
        parser.error("publication candidates require >=3 processes, >=30 samples, a 1 s warmup and a 3 s window; "
                     "use --diagnostic for smoke runs")
    if "FERRONI_BENCH_PREFILTER" in os.environ:
        parser.error("the lane measures the default scanner configuration; unset FERRONI_BENCH_PREFILTER")
    engines = args.engines.split(",")
    if "rust" not in engines or not set(engines) <= set(ENGINES) or len(set(engines)) != len(engines):
        parser.error(f"engines must include rust and come from {', '.join(ENGINES)}")
    engines = [engine for engine in ENGINES if engine in engines]
    onigmo_dir = Path(os.environ.get("FERRONI_ONIGMO_DIR", ROOT / ".cache/upstream/onigmo-ruby"))
    if "onigmo" in engines and not (onigmo_dir / "regexec.c").is_file():
        parser.error("Onigmo sources are missing: run scripts/prepare-onigmo-sources.sh or leave onigmo out of --engines")
    wanted = args.cases.split(",") if args.cases else None
    groups = list(REPLAYS)
    if wanted:
        unknown = [case for case in wanted if case not in lane_cases()]
        if unknown or len(set(wanted)) != len(wanted):
            parser.error(f"unknown or duplicate case IDs: {', '.join(unknown) or 'duplicates'}")
        groups = [group for group in groups if any(scanner_group(case) == group for case in wanted)]
    absent = [group for group in groups if not (ROOT / "benches" / group / "trace.json").is_file()]
    if absent:
        parser.error(f"the lane's replays {', '.join(absent)} have no benches/<replay>/trace.json in this checkout")
    if directory.exists() and any(directory.iterdir()):
        parser.error("use a new empty output directory; retained evidence must not be overwritten")
    directory.mkdir(parents=True, exist_ok=True)

    features = "onigmo" if "onigmo" in engines else "ffi"
    targets = sorted({REPLAYS[group] for group in groups})
    binaries = build(targets, features)
    paths = source_files(targets)
    context = {"format_version": 1, "lane": "textmate-scanner-replays",
               "started_utc": datetime.now(timezone.utc).isoformat(), "platform": platform.platform(),
               "system": platform.system(), "machine": platform.machine(),
               "host": ENGINE_RUNNER.describe_host(None), "host_note": args.host_note,
               "commit": command_output(["git", "rev-parse", "HEAD"]),
               "git_status": command_output(["git", "status", "--porcelain"]),
               "rustc": command_output(["rustc", "-Vv"]), "cargo": command_output(["cargo", "-V"]),
               "cc": command_output(["cc", "--version"]).splitlines()[0], "python": sys.version,
               "build_profile": f"bench, thin LTO, features {features}", "features": features,
               "source_sha256": fingerprint(paths),
               "trace_sha256": {path.parent.name: sha256(path) for path in sorted(ROOT.glob("benches/*_scanner/trace.json"))},
               "cargo_lock_sha256": sha256(ROOT / "Cargo.lock"), "rustflags": os.environ.get("RUSTFLAGS", ""),
               "engine_source_overrides": {key: os.environ.get(key) for key in (
                   "FERRONI_ONIGURUMA_DIR", "FERRONI_ONIGMO_DIR", "CC", "CFLAGS", "CARGO_PROFILE_BENCH_LTO")},
               "pcre2_build": "PCRE2_SYS_STATIC=1; bundled source from locked pcre2-sys",
               "binaries": {target: sha256(binary) for target, binary in binaries.items()},
               "engines": engines, "profiles": {engine: PROFILES[engine] for engine in engines},
               "repetitions": args.repetitions, "samples": args.samples, "measure_seconds": args.measure_seconds,
               "warmup_seconds": args.warmup_seconds, "only_run": args.only_run, "timeout": args.timeout,
               "seed": args.seed, "diagnostic": args.diagnostic or args.command == "validate", "command": sys.argv,
               "order_policy": "seeded case shuffle; rotated engine order, reversed on alternating repetitions; "
                               "one process per case and engine",
               "environment_alignment": {"variable": PRODUCTION.ALIGNMENT_VARIABLE,
                                         "offsets_bytes": list(PRODUCTION.ALIGNMENT_OFFSETS),
                                         "rounding_bytes": PRODUCTION.ALIGNMENT_ROUNDING}}
    context["source_snapshot_sha256"], context["native_sources"] = snapshot_source(directory, paths)
    shutil.copyfile(ROOT / "Cargo.lock", directory / "Cargo.lock")

    cases, eligibility = preflight(binaries, groups, engines, directory, args.validation_timeout)
    if wanted:
        cases = [case for case in cases if case in wanted]
        eligibility = [record for record in eligibility if record["case"] in wanted]
    context["cases"] = cases
    write_json(directory / "context.json", context)
    write_json(directory / "eligibility.json", eligibility)
    for record in eligibility:
        print(f"{record['case']}/{record['engine']}: {record['status']}", flush=True)
    failures = fatal_preflight(eligibility)
    if failures:
        write_json(directory / "measurements.json", [])
        report(directory)
        raise SystemExit(f"Preflight failed for {len(failures)} pairs; no timing processes started. "
                         f"See {directory / 'validation'}.")

    eligible = {(r["case"], r["engine"]) for r in eligibility if r["status"] == TIMED_STATUS}
    measurements, order = [], []
    if args.command == "run":
        repetitions = [rep for rep in range(args.repetitions) if args.only_run is None or rep == args.only_run - 1]
        sizes = {rep: PRODUCTION.aligned_size([timed_environment(bench_id(case, engine), directory / f"raw/run-{rep + 1}/{case}/{engine}")
                                               for case, engine in PRODUCTION.ordered_pairs(cases, engines, rep, args.seed)
                                               if (case, engine) in eligible], rep)
                 for rep in repetitions if eligible}
        context["environment_sizes"] = {str(rep + 1): size for rep, size in sizes.items()}
        write_json(directory / "context.json", context)
        for rep in repetitions:
            for case, engine in PRODUCTION.ordered_pairs(cases, engines, rep, args.seed):
                if (case, engine) not in eligible:
                    continue
                benchmark = bench_id(case, engine)
                relative = f"raw/run-{rep + 1}/{case}/{engine}"
                record = {"case": case, "engine": engine, "repetition": rep, "raw": relative}
                print(f"run {rep + 1}: {benchmark}", flush=True)
                try:
                    command, value = measure(binaries[REPLAYS[scanner_group(case)]], benchmark, directory / relative, args,
                                             sizes[rep])
                    record.update(status="measured", estimate=value)
                except (subprocess.TimeoutExpired, RuntimeError) as error:
                    command = None
                    record.update(status="timeout" if isinstance(error, subprocess.TimeoutExpired) else "execution_error",
                                  reason=str(error))
                order.append(dict(case=case, engine=engine, repetition=rep, command=command))
                measurements.append(record)
                write_json(directory / "order.json", order)
                write_json(directory / "measurements.json", measurements)
        if any(sha256(binary) != context["binaries"][target] for target, binary in binaries.items()):
            raise SystemExit("a bench binary changed during the measurement")
    write_json(directory / "order.json", order)
    write_json(directory / "measurements.json", measurements)
    report(directory)
    print(f"Evidence: {directory / 'summary.md'}")
    if any(r["status"] != "measured" for r in measurements):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
