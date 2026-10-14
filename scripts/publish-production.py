#!/usr/bin/env python3
"""Verify complete Blacksmith evidence and regenerate the public comparison data.

Publish only full, clean, repeated runs. Keep the original evidence compressed
in Git; regenerate every displayed estimate and comparison from those inputs.
"""

import argparse
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
DATA = ROOT / "docs/app/data/production-comparison.json"
PROFILES = {"macos-arm64": ("arm64", "macOS"), "linux-x86-64": ("x86_64", "Linux")}
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


def verify_context(context, catalog, receipt, profile):
    require(context["diagnostic"] is False, "Diagnostic evidence cannot be published")
    require(not context["git_status"], "The measured checkout must be clean")
    require(context["commit"] == receipt["head_sha"], "Measured revision differs from the workflow revision")
    machine, platform = PROFILES[profile]
    require(context["machine"] == machine and context["platform"].startswith(platform), "Unexpected runner architecture/OS")
    require(context["engines"] == list(comparison.ENGINES), "Publication requires every engine profile")
    require(context["cases"] == [case["id"] for case in catalog["cases"]], "Publication requires the complete corpus")
    require(context["repetitions"] >= 3 and context["samples"] >= 30
            and context["measure_seconds"] >= 3 and context["warmup_seconds"] >= 1,
            "Publication requires the agreed measurement budget")
    require(context["build_profile"] == "bench, thin LTO, features onigmo", "Unexpected build profile")
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


def verify(directory, receipt, profile):
    context, catalog = read(directory / "context.json"), read(directory / "catalog.json")
    verify_context(context, catalog, receipt, profile)
    require(catalog == read(ROOT / "benches/production_regex/catalog.json"), "The retained corpus differs from the current corpus")
    for filename, key in (("Cargo.lock", "cargo_lock_sha256"), ("source-snapshot.tar.gz", "source_snapshot_sha256")):
        require(sha256(directory / filename) == context[key], f"Checksum mismatch: {filename}")
    require(sha256(ROOT / "benches/production_regex/catalog.json") == context["corpus_sha256"], "Corpus checksum mismatch")
    require(sha256(ROOT / "Cargo.lock") == context["cargo_lock_sha256"], "The retained dependency lock differs from the current lock")
    require((directory / "engine-inputs.toml").read_bytes() == (ROOT / "benches/battle_inputs.toml").read_bytes(), "Native engine pins differ from the current pins")
    with tarfile.open(directory / "source-snapshot.tar.gz") as archive:
        require(archive.extractfile("scripts/compare-production.py").read() == (ROOT / "scripts/compare-production.py").read_bytes(),
                "The interpretation script differs from the measured source snapshot")
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
    require(all(row["status"] in ("eligible", "pending", "syntax_rejected", "semantic_difference") for row in eligibility), "Unknown eligibility status")
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
    require(len(jobs) == len(PROFILES) and {job["name"] for job in jobs} == set(PROFILES)
            and all(job["conclusion"] == "success" for job in jobs), "Both platform jobs must succeed")
    labels = {"macos-arm64": "blacksmith-6vcpu-macos-26", "linux-x86-64": "blacksmith-4vcpu-ubuntu-2404"}
    require(all(labels[job["name"]] in job["labels"] for job in jobs), "Unexpected provider runner label")


def document(data):
    lines = ["# Current source-backed regex results", "",
             f"Measured revision: `{data['run']['head_sha']}`. [Blacksmith run]({data['run']['html_url']}).", "",
             "The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.", "",
             "The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Standard searches can reuse cached match workspace; parameterized searches currently create fresh workspace. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.", "",
             "Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.", "",
             "Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.", ""]
    for platform in data["platforms"]:
        lines += [f"## {platform['profile']}", "", f"Host: {platform['context']['platform']}; {platform['context']['cpu_count']} visible CPUs.", "",
                  "| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |", "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
        for row in platform["production_comparisons"]:
            conclusions = [case["conclusion"] for case in row["cases"]]
            lines.append(f"| {row['engine']} | {row['lane']} | {row['case_count']} | {row['family_count']} | {row['family_weighted_ratio']:.3f} | {conclusions.count('Ferroni faster in every run interval')} | {conclusions.count('comparator faster in every run interval')} | {conclusions.count('inconclusive')} |")
        lines += ["", "The exact common cases and all absolute timings are available in `publication.json` and in the website's case tables.", ""]
    lines += ["## Regenerate and check", "", "```sh", "python3 scripts/publish-production.py --check", "```", "",
              "Host power and thermal state were unavailable. These are three process repetitions on one VM per platform, not measurements across multiple independent hosts or a model of production traffic. Memory was not measured in this run.", ""]
    return "\n".join(lines)


def generate(directories, receipt):
    validate_receipt(receipt)
    platforms = [verify(directories[profile], receipt, profile) for profile in PROFILES]
    require(platforms[0]["context"]["corpus_sha256"] == platforms[1]["context"]["corpus_sha256"], "Platforms measured different corpora")
    catalog = read(ROOT / "benches/production_regex/catalog.json")
    cases = [{"id": case["id"], "family": case["family"], "tier": case["tier"], "operation": case["operation"],
              "pattern_origin": case["pattern_origin"], "input_count": len(case["inputs"]),
              "scope": case.get("scope", case.get("source_boundary", "Not timed: " + case.get("pending_reason", ""))),
              "source_url": catalog["sources"][case["source"]["file"]]["url"],
              "pending_reason": case.get("pending_reason")} for case in catalog["cases"]]
    return {"format_version": 1, "run": receipt, "cases": cases, "platforms": platforms}


def encoded(data):
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


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
    require(formatter.is_file(), "Install the documentation dependencies before publishing website data")
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
    DATA.write_text(encoded(data))
    subprocess.run([str(formatter), "--write", str(DATA)], cwd=ROOT / "docs", check=True)
    print(f"Verified both platforms; updated {DATA} and {RESULTS}.")


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
            data = generate(directories, receipt)
        require(equivalent(read(DATA), data) and equivalent(read(RESULTS / "publication.json"), data), "Generated public data has drifted")
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
