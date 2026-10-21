#!/usr/bin/env python3
"""Verify complete Blacksmith evidence and regenerate the public comparison data.

Publish only full, clean, repeated runs. Keep the original evidence compressed
in Git; regenerate every displayed estimate and comparison from those inputs.
"""

import argparse
import ast
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
# Keep retained measurements outside the source tree archived by the runner.
# Otherwise future source snapshots would recursively include previous runs.
RESULTS = ROOT / "benches/production_results"
# The website shows only this summary; every case and timing stays in RESULTS.
SUMMARY = ROOT / "docs/app/data/production-summary.json"
PROFILES = {"macos-arm64": ("arm64", "macOS"), "linux-x86-64": ("x86_64", "Linux")}
BASE_ENGINES = ("rust", "rust_erroraware", "c", "onigmo", "pcre2_jit", "pcre2", "fancy_regex", "fancy_regex_seek", "regex")
spec = importlib.util.spec_from_file_location("production_comparison", ROOT / "scripts/compare-production.py")
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


def read(path):
    return json.loads(path.read_text())


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def equivalent(left, right):
    """Allow libc's last-bit log/exp differences when replaying derived ratios."""
    if type(left) is bool or type(right) is bool:
        return type(left) is type(right) and left == right
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        return math.isclose(left, right, rel_tol=1e-12, abs_tol=1e-12)
    if isinstance(left, dict) and isinstance(right, dict):
        return left.keys() == right.keys() and all(equivalent(left[key], right[key]) for key in left)
    if isinstance(left, list) and isinstance(right, list):
        return len(left) == len(right) and all(equivalent(a, b) for a, b in zip(left, right))
    return left == right


def verify_context(context, catalog, receipt, profile, engines=BASE_ENGINES):
    require(context["diagnostic"] is False, "Diagnostic evidence cannot be published")
    require(not context["git_status"], "The measured checkout must be clean")
    require(context["commit"] == receipt["head_sha"], "Measured revision differs from the workflow revision")
    machine, platform = PROFILES[profile]
    require(context["machine"] == machine and context["platform"].startswith(platform), "Unexpected runner architecture/OS")
    require(context["engines"] == list(engines), "Publication requires the declared engine profiles")
    require(context["cases"] == [case["id"] for case in catalog["cases"]], "Publication requires the complete corpus")
    # Runs before per-lane budgets used the steady window for every lane.
    budgets = context.get("lane_budgets") or {lane: {"warmup_seconds": context["warmup_seconds"],
                                                     "measure_seconds": context["measure_seconds"]}
                                              for lane in comparison.LANES}
    require(context["repetitions"] >= 3 and context["samples"] >= 30
            and budgets["steady"]["measure_seconds"] >= 3 and budgets["steady"]["warmup_seconds"] >= 1
            and all(budgets[lane]["measure_seconds"] >= 1 and budgets[lane]["warmup_seconds"] >= 0.5
                    for lane in comparison.CONSTRUCTION_LANES),
            "Publication requires the agreed measurement budget")
    require(context.get("only_run") is None, "A single-repetition shard cannot be published")
    if "hosts" in context:
        require([host["only_run"] for host in context["hosts"]] == list(range(1, context["repetitions"] + 1))
                and all(host["platform"].startswith(platform) and str(receipt["id"]) in host["host_note"]
                        for host in context["hosts"]), "Every repetition must come from its own host of this run")
    features = "onigmo" if "onigmo" in engines else "ffi"
    require(context["build_profile"] == f"bench, thin LTO, features {features}", "Unexpected build profile")
    require(context["pcre2_build"] == "PCRE2_SYS_STATIC=1; bundled source from locked pcre2-sys", "Unverified PCRE2 build")
    require(str(receipt["id"]) in context["host_note"], "Workflow identity is missing from the host note")
    require(context["rustflags"] == "", "Unexpected RUSTFLAGS override")
    require(all(value is None for value in context["engine_source_overrides"].values()), "Unexpected engine/build override")


def verify_raw(directory, record, lane, estimate, samples):
    relative = Path(record["raw"]) / estimate["raw_directory"]
    require(not relative.is_absolute() and ".." not in relative.parts, "Unsafe raw evidence path")
    raw = directory / relative
    identity = read(raw / "benchmark.json")["full_id"]
    require(identity == f"production_regex/{record['case']}/{lane}/{record['engine']}", "Criterion identity mismatch")
    mean = read(raw / "estimates.json")["mean"]
    interval = mean["confidence_interval"]
    require(interval["confidence_level"] == 0.95, "Unexpected confidence level")
    expected = (mean["point_estimate"], interval["lower_bound"], interval["upper_bound"])
    actual = (estimate["ns"], estimate["lower_ns"], estimate["upper_ns"])
    require(actual == expected, "Stored estimate differs from raw Criterion evidence")
    require(all(math.isfinite(value) and value > 0 for value in actual)
            and actual[1] <= actual[0] <= actual[2], "Invalid estimate or interval")
    sample = read(raw / "sample.json")
    require(len(sample["iters"]) == len(sample["times"]) >= samples, "Incomplete raw samples")
    require(all(math.isfinite(value) and value > 0 for value in sample["iters"] + sample["times"]), "Invalid raw samples")
    sample_mean = statistics.mean(elapsed / iterations for elapsed, iterations in zip(sample["times"], sample["iters"]))
    require(math.isclose(sample_mean, estimate["ns"], rel_tol=1e-10), "Mean estimate differs from the normalized raw samples")


def verify_lock_additions(retained, current):
    """Keep prior evidence valid only when existing packages remain unchanged.

    Ferroni's own package is matched by name: a release changes only its
    version line, which says nothing about the measured dependencies.
    """
    def key(block):
        name = re.search(r'^name = "([^"]+)"', block, re.M)[1]
        return name if name == "ferroni" else name + "@" + re.search(r'^version = "([^"]+)"', block, re.M)[1]
    blocks = lambda text: {key(block): block for block in text.split("\n[[package]]\n")[1:]}
    before, after = blocks(retained), blocks(current)
    for key, old in before.items():
        require(key in after, "A retained dependency is missing from the current lock")
        new = after[key]
        if key == "ferroni":
            pattern = r'dependencies = \[\n(.*?)\n\]'
            old_deps, new_deps = (set(re.search(pattern, block, re.S)[1].splitlines()) for block in (old, new))
            require(old_deps == new_deps, "The retained root dependencies changed")
            strip = lambda block: re.sub(r'^version = ".*"$', "", re.sub(pattern, "", block, flags=re.S), flags=re.M)
            require(strip(old) == strip(new), "The retained root package changed")
        else:
            require(old == new, f"The retained dependency {key} changed")


def verify_method(retained):
    """Profile/corpus additions must not silently change prior ratio interpretation."""
    functions = lambda source: {node.name: ast.dump(node, include_attributes=False) for node in ast.parse(source).body if isinstance(node, ast.FunctionDef)}
    before, after = functions(retained), functions((ROOT / "scripts/compare-production.py").read_text())
    for name in ("summarize_pair", "ordered_pairs", "fatal_preflight"):
        require(before[name] == after[name], f"The measured interpretation function {name} changed")


def verify_source_snapshot(path, checksum, current_source=True):
    """Retained results describe their hashed snapshot, even after engine updates.

    Publishing a new run still requires byte-for-byte current implementation
    equality. Checking existing tables verifies the measured snapshot instead.
    The dependency lock follows the same rule where a caller requires it: a
    release that only bumps Ferroni's own version leaves the retained lock
    verifiable by its checksum.
    """
    require(sha256(path) == checksum, "Checksum mismatch: source-snapshot.tar.gz")
    with tarfile.open(path) as archive:
        sources = [member for member in archive.getmembers()
                   if member.isfile() and member.name.startswith("src/")]
        require(sources, "The measured implementation snapshot is missing")
        if current_source:
            for member in sources:
                require(archive.extractfile(member).read() == (ROOT / member.name).read_bytes(),
                        "The retained Ferroni implementation differs from the current source")


def verify(directory, receipt, profile, catalog_path=None, engines=BASE_ENGINES, current_lock=False, current_source=True):
    catalog_path = catalog_path or ROOT / "benches/production_regex/catalog.json"
    context, catalog = read(directory / "context.json"), read(directory / "catalog.json")
    verify_context(context, catalog, receipt, profile, engines)
    require(catalog == read(catalog_path), "The retained corpus differs from the current corpus")
    require(sha256(directory / "Cargo.lock") == context["cargo_lock_sha256"], "Checksum mismatch: Cargo.lock")
    verify_source_snapshot(directory / "source-snapshot.tar.gz", context["source_snapshot_sha256"], current_source)
    require(sha256(catalog_path) == context["corpus_sha256"], "Corpus checksum mismatch")
    if current_lock:
        require(sha256(ROOT / "Cargo.lock") == context["cargo_lock_sha256"], "The retained dependency lock differs from the current lock")
    else:
        verify_lock_additions((directory / "Cargo.lock").read_text(), (ROOT / "Cargo.lock").read_text())
    require((directory / "engine-inputs.toml").read_bytes() == (ROOT / "benches/battle_inputs.toml").read_bytes(), "Native engine pins differ from the current pins")
    with tarfile.open(directory / "source-snapshot.tar.gz") as archive:
        verify_method(archive.extractfile("scripts/compare-production.py").read().decode())
        pins = (directory / "engine-inputs.toml").read_text()
        for name, cache in (("oniguruma", "oniguruma-orig"), ("onigmo", "onigmo-ruby")):
            section = re.search(rf"\[{name}\]\n(.*?)(?=\n\[|\Z)", pins, re.S)[1]
            fields = ("commit", "st_commit") if name == "onigmo" else ("commit",)
            expected_stamp = " ".join(re.search(rf'^\s*{field} = "([a-f0-9]{{40}})"', section, re.M)[1] for field in fields)
            stamp = archive.extractfile(f".cache/upstream/{cache}/.ferroni-upstream-commit").read().decode().strip()
            require(stamp == expected_stamp == context["native_sources"][name]["revision_stamp"], f"Unverified {name} source revision")
    eligibility = read(directory / "eligibility.json")
    expected_pairs = {(case, engine) for case in context["cases"] for engine in context["engines"]}
    require(len(eligibility) == len(expected_pairs)
            and {(row["case"], row["engine"]) for row in eligibility} == expected_pairs, "Incomplete or duplicate eligibility records")
    require(not comparison.fatal_preflight(eligibility), "Fatal preflight result")
    require(all(row["status"] in ("eligible", "pending", "syntax_rejected", "semantic_difference", "api_unsupported") for row in eligibility), "Unknown eligibility status")
    eligible = {(row["case"], row["engine"]) for row in eligibility if row["status"] == "eligible"}
    records = read(directory / "measurements.json")
    expected = {(case, engine, rep) for case, engine in eligible for rep in range(context["repetitions"])}
    require(len(records) == len(expected)
            and {(row["case"], row["engine"], row["repetition"]) for row in records} == expected, "Incomplete or duplicate measurements")
    order = read(directory / "order.json")
    expected_order = [(case, engine, rep) for rep in range(context["repetitions"])
                      for case, engine in comparison.ordered_pairs(context["cases"], context["engines"], rep, context["seed"])
                      if (case, engine) in eligible]
    require([(row["case"], row["engine"], row["repetition"]) for row in order] == expected_order, "Execution order differs from the declared policy")
    for row in records:
        require(row["status"] == "measured" and set(row["lanes"]) == set(comparison.LANES), "Incomplete measurement lanes")
        for lane, estimate in row["lanes"].items():
            verify_raw(directory, row, lane, estimate, context["samples"])
    measurements = {(row["case"], row["engine"], row["repetition"]): row["lanes"] for row in records}
    summaries = [comparison.summarize_pair(catalog["cases"], measurements, engine, context["repetitions"], lane)
                 for engine in context["engines"] if engine != "rust" for lane in comparison.LANES]
    require(equivalent(read(directory / "summary.json"), {"comparisons": summaries}), "Retained summary differs from the raw measurements")
    production = [case for case in catalog["cases"] if case["pattern_origin"] == "production" and not case.get("pending_reason")]
    production_summaries = [comparison.summarize_pair(production, measurements, engine, context["repetitions"], lane)
                            for engine in context["engines"] if engine != "rust" for lane in comparison.LANES]
    estimates = []
    for case, engine in sorted(eligible):
        for lane in comparison.LANES:
            values = [measurements[(case, engine, rep)][lane] for rep in range(context["repetitions"])]
            means = [value["ns"] for value in values]
            estimates.append({"case": case, "engine": engine, "lane": lane,
                              "ns_median": statistics.median(means), "ns_range": [min(means), max(means)],
                              "interval_envelope_ns": [min(v["lower_ns"] for v in values), max(v["upper_ns"] for v in values)]})
    return {"profile": profile, "context": context, "eligibility": eligibility,
            "estimates": estimates, "comparisons": summaries, "production_comparisons": production_summaries}


def validate_receipt(receipt):
    require(receipt["conclusion"] == "success" and receipt["event"] == "workflow_dispatch", "The workflow must have completed successfully")
    require(receipt["path"] == ".github/workflows/blacksmith-comparison.yml", "Unexpected workflow path")
    require(re.fullmatch(r"[a-f0-9]{40}", receipt["head_sha"]), "Invalid measured revision")
    require(receipt["html_url"] == f"https://github.com/sebastian-software/ferroni/actions/runs/{receipt['id']}", "Unexpected workflow URL")
    jobs = receipt["jobs"]
    labels = {"macos-arm64": "blacksmith-6vcpu-macos-26", "linux-x86-64": "blacksmith-4vcpu-ubuntu-2404"}
    # Earlier runs measured every repetition in one job per platform; later
    # runs measure each repetition in its own job and merge them per platform.
    measuring = {profile: profile for profile in PROFILES}
    merging = set()
    if {job["name"] for job in jobs} != set(PROFILES):
        measuring = {f"{profile} run {run}": profile for profile in PROFILES for run in (1, 2, 3)}
        merging = {f"merge {profile}" for profile in PROFILES}
    require(len(jobs) == len(measuring) + len(merging) and {job["name"] for job in jobs} == set(measuring) | merging
            and all(job["conclusion"] == "success" for job in jobs), "Every measuring and merging job must succeed")
    require(all(labels[measuring[job["name"]]] in job["labels"] for job in jobs if job["name"] in measuring),
            "Unexpected provider runner label")


def hosts_sentence(data):
    if all("hosts" in platform["context"] for platform in data["platforms"]):
        return "Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic."
    return "These are three process repetitions on one VM per platform, not measurements across multiple independent hosts or a model of production traffic."


def document(data):
    lines = ["# Published source-backed regex results", "",
             f"Measured revision: `{data['run']['head_sha']}`. [Blacksmith run]({data['run']['html_url']}).", "",
             "These timings describe that measured revision. Later engine changes are not included.", "",
             "The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.", "",
             "The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Both API paths reuse Ferroni's thread-cached `MatchArg` when they enter the matcher. The steady-use difference below is the historical measurement for this revision; its retained evidence does not profile the cause. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.", "",
             "Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.", "",
             "Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.", ""]
    for platform in data["platforms"]:
        lines += [f"## {platform['profile']}", "", f"Host: {platform['context']['platform']}; {platform['context']['cpu_count']} visible CPUs.", "",
                  "| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |", "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
        for row in platform["production_comparisons"]:
            conclusions = [case["conclusion"] for case in row["cases"]]
            lines.append(f"| {row['engine']} | {row['lane']} | {row['case_count']} | {row['family_count']} | {row['family_weighted_ratio']:.3f} | {conclusions.count('Ferroni faster in every run interval')} | {conclusions.count('comparator faster in every run interval')} | {conclusions.count('inconclusive')} |")
        lines += ["", "The exact common cases and all absolute timings are available in `publication.json`.", ""]
    lines += ["## Regenerate and check", "", "```sh", "python3 scripts/publish-production.py --check", "```", "",
              "Host power and thermal state were unavailable. " + hosts_sentence(data) + " Memory was not measured in this run.", ""]
    return "\n".join(lines)


def generate(directories, receipt, catalog_path=None, engines=None, current_lock=False, current_source=True):
    catalog_path = catalog_path or ROOT / "benches/production_regex/catalog.json"
    if engines is None:
        engines = tuple(read(directories[next(iter(PROFILES))] / "context.json")["engines"])
        require(engines in (BASE_ENGINES, comparison.ENGINES), "Publication requires every engine profile")
    validate_receipt(receipt)
    platforms = [verify(directories[profile], receipt, profile, catalog_path, engines, current_lock, current_source) for profile in PROFILES]
    require(platforms[0]["context"]["corpus_sha256"] == platforms[1]["context"]["corpus_sha256"], "Platforms measured different corpora")
    catalog = read(catalog_path)
    cases = [{"id": case["id"], "family": case["family"], "tier": case["tier"], "operation": case["operation"],
              "pattern_origin": case["pattern_origin"], "input_count": len(case["inputs"]),
              "scope": case.get("scope", case.get("source_boundary", "Not timed: " + case.get("pending_reason", ""))),
              "source_url": catalog["sources"][case["source"]["file"]]["url"],
              "pending_reason": case.get("pending_reason")} for case in catalog["cases"]]
    return {"format_version": 1, "run": receipt, "cases": cases, "platforms": platforms}


def encoded(data):
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


def overview(data):
    """Derive a small homepage payload from the same verified production pairs."""
    production = [case for case in data["cases"]
                  if case["pattern_origin"] == "production" and not case["pending_reason"]]
    platforms = []
    for platform in data["platforms"]:
        rows = []
        for pair in platform["production_comparisons"]:
            conclusions = [case["conclusion"] for case in pair["cases"]]
            rows.append({key: pair[key] for key in
                         ("engine", "lane", "case_count", "family_count", "family_weighted_ratio")} | {
                "ferroni_cases": conclusions.count("Ferroni faster in every run interval"),
                "comparator_cases": conclusions.count("comparator faster in every run interval"),
                "inconclusive_cases": conclusions.count("inconclusive"),
            })
        platforms.append({"profile": platform["profile"],
                          "repetitions": platform["context"]["repetitions"], "comparisons": rows})
    return {"format_version": 1,
            "run": {key: data["run"][key] for key in ("head_sha", "html_url", "created_at")},
            "case_count": len(production), "input_count": sum(case["input_count"] for case in production),
            "family_count": len({case["family"] for case in production}), "platforms": platforms}


def fetch_run(run_id, destination):
    def github(endpoint):
        return json.loads(subprocess.check_output(["gh", "api", endpoint], cwd=ROOT, text=True))

    endpoint = f"repos/sebastian-software/ferroni/actions/runs/{run_id}"
    run = github(endpoint)
    receipt = {key: run[key] for key in ("id", "run_attempt", "head_sha", "html_url", "path", "event", "conclusion", "created_at", "updated_at")}
    receipt["jobs"] = [{key: job[key] for key in ("id", "name", "conclusion", "labels", "runner_name", "started_at", "completed_at", "html_url")}
                       for job in github(endpoint + "/jobs")["jobs"]]
    receipt["artifacts"] = [{key: artifact[key] for key in ("id", "name", "digest", "size_in_bytes", "expired")}
                            for artifact in github(endpoint + "/artifacts")["artifacts"]]
    validate_receipt(receipt)
    expected = {f"production-{profile}-{run_id}-{receipt['run_attempt']}" for profile in PROFILES}
    actual = {artifact["name"] for artifact in receipt["artifacts"] if not artifact["expired"]}
    require(expected <= actual, "Complete workflow artifacts are missing or expired")
    subprocess.run(["gh", "run", "download", str(run_id), "--repo", "sebastian-software/ferroni", "--dir", str(destination),
                    "--pattern", f"production-*-{run_id}-{receipt['run_attempt']}"], cwd=ROOT, check=True)
    return receipt


def publish(artifacts, receipt):
    validate_receipt(receipt)
    directories = {profile: artifacts / f"production-{profile}-{receipt['id']}-{receipt['run_attempt']}" for profile in PROFILES}
    data = generate(directories, receipt)
    formatter = ROOT / "docs/node_modules/.bin/oxfmt"
    require(formatter.is_file(), "Install the documentation dependencies before publishing the website summary")
    RESULTS.mkdir(parents=True, exist_ok=True)
    for profile, directory in directories.items():
        destination = RESULTS / profile
        destination.mkdir(parents=True, exist_ok=True)
        with tarfile.open(destination / "evidence.tar.gz", "w:gz") as archive:
            for path in sorted(directory.iterdir()):
                archive.add(path, arcname=path.name)
        shutil.copyfile(directory / "context.json", destination / "context.json")
    (RESULTS / "run.json").write_text(encoded(receipt))
    (RESULTS / "publication.json").write_text(encoded(data))
    (RESULTS / "README.md").write_text(document(data))
    SUMMARY.write_text(encoded(overview(data)))
    subprocess.run([str(formatter), "--write", str(SUMMARY)], cwd=ROOT / "docs", check=True)
    print(f"Verified both platforms; updated {SUMMARY} and {RESULTS}.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifacts", nargs="?", type=Path, help="downloaded workflow artifact directories")
    parser.add_argument("--receipt", type=Path, help="workflow receipt with successful job metadata")
    parser.add_argument("--run-id", type=int, help="fetch and publish one successful full Blacksmith workflow run")
    parser.add_argument("--check", action="store_true", help="verify retained evidence and generated data without writing")
    args = parser.parse_args()
    if args.check:
        receipt = read(RESULTS / "run.json")
        with tempfile.TemporaryDirectory(prefix="ferroni-publication-") as scratch:
            directories = {}
            for profile in PROFILES:
                directories[profile] = Path(scratch) / profile
                with tarfile.open(RESULTS / profile / "evidence.tar.gz") as archive:
                    # This archive is created locally by this publisher. Permit
                    # regular files/directories only, even on older Python.
                    for member in archive.getmembers():
                        path = Path(member.name)
                        require(not path.is_absolute() and ".." not in path.parts
                                and (member.isfile() or member.isdir()), "Unsafe evidence archive member")
                    archive.extractall(directories[profile])
            data = generate(directories, receipt, current_source=False)
        require(equivalent(read(RESULTS / "publication.json"), data), "Generated public data has drifted")
        require(equivalent(read(SUMMARY), overview(data)), "Generated homepage summary has drifted")
        require((RESULTS / "README.md").read_text() == document(data), "Generated result tables have drifted")
        print("Publication check: both complete Blacksmith runs, raw estimates, coverage and public tables agree.")
        return
    if args.run_id:
        require(not args.artifacts and not args.receipt, "Use either --run-id or downloaded artifacts with --receipt")
        with tempfile.TemporaryDirectory(prefix="ferroni-artifacts-") as scratch:
            directory = Path(scratch)
            publish(directory, fetch_run(args.run_id, directory))
        return
    require(args.artifacts and args.receipt, "Provide downloaded artifacts and --receipt, or use --check")
    publish(args.artifacts, read(args.receipt))


if __name__ == "__main__":
    main()
