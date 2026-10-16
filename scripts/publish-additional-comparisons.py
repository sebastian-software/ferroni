#!/usr/bin/env python3
"""Publish the LLM pre-tokenization Blacksmith evidence."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "benches/additional_results"
spec = importlib.util.spec_from_file_location("publication", ROOT / "scripts/publish-production.py")
publication = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publication)
SCOPES = {
    "tokenizers": ("tokenizers", "tokenizer_regex", publication.comparison.ENGINES),
}


def document(datasets):
    run = datasets["tokenizers"]["run"]
    lines = ["# Additional source-backed regex comparisons", "",
             f"Measured revision: `{run['head_sha']}`. [Blacksmith run]({run['html_url']}).", "",
             "The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.", ""]
    for scope, data in datasets.items():
        lines += [f"## {scope}", ""]
        for platform in data["platforms"]:
            lines += [f"### {platform['profile']}", "", "| Profile | Phase | Common cases | Ratio: comparator / Ferroni |", "| --- | --- | ---: | ---: |"]
            for row in platform["production_comparisons"]:
                value = "—" if row["family_weighted_ratio"] is None else f"{row['family_weighted_ratio']:.3f}"
                lines.append(f"| {row['engine']} | {row['lane']} | {row['case_count']} | {value} |")
            lines.append("")
    lines += ["Raw samples, logs, source snapshots, dependency locks and eligibility are retained per scope/platform in evidence.tar.gz. " + publication.hosts_sentence(datasets["tokenizers"]) + " Host power/thermal state is unavailable. Memory is not measured.", "", "```sh", "python3 scripts/publish-additional-comparisons.py --check", "```", ""]
    return "\n".join(lines)


def generate(directories, receipt, current_source=True):
    return {scope: publication.generate(directories[scope], receipt,
                ROOT / f"benches/{catalog}/catalog.json", engines, current_lock=current_source, current_source=current_source)
            for scope, (_, catalog, engines) in SCOPES.items()}


# Every case and timing stays in the repository; the website links here.
def write_data(datasets):
    for scope in SCOPES:
        (RESULTS / scope / "publication.json").write_text(publication.encoded(datasets[scope]))
    (RESULTS / "README.md").write_text(document(datasets))


def check():
    if not RESULTS.exists():
        print("Additional comparisons have not been published yet.")
        return
    receipt = publication.read(RESULTS / "run.json")
    with tempfile.TemporaryDirectory(prefix="ferroni-additional-check-") as scratch:
        directories = {}
        for scope in SCOPES:
            directories[scope] = {}
            for profile in publication.PROFILES:
                destination = Path(scratch) / scope / profile
                destination.mkdir(parents=True)
                with tarfile.open(RESULTS / scope / profile / "evidence.tar.gz") as archive:
                    for member in archive.getmembers():
                        path = Path(member.name)
                        publication.require(not path.is_absolute() and ".." not in path.parts and (member.isfile() or member.isdir()), "Unsafe evidence archive member")
                    archive.extractall(destination)
                directories[scope][profile] = destination
        datasets = generate(directories, receipt, current_source=False)
    for scope in SCOPES:
        publication.require(publication.equivalent(publication.read(RESULTS / scope / "publication.json"), datasets[scope]), f"{scope} public data has drifted")
    publication.require((RESULTS / "README.md").read_text() == document(datasets), "Additional result tables have drifted")
    print("Additional publication check: paired baselines, both platforms, raw samples and published data agree.")


def publish(run_id):
    with tempfile.TemporaryDirectory(prefix="ferroni-additional-artifacts-") as scratch:
        scratch = Path(scratch)
        receipt = publication.fetch_run(run_id, scratch)
        subprocess.run(["gh", "run", "download", str(run_id), "--repo", "sebastian-software/ferroni", "--dir", str(scratch), "--pattern", f"tokenizers-*-{run_id}-{receipt['run_attempt']}"], cwd=ROOT, check=True)
        directories = {scope: {profile: scratch / f"{artifact}-{profile}-{run_id}-{receipt['run_attempt']}" for profile in publication.PROFILES} for scope, (artifact, _, _) in SCOPES.items()}
        datasets = generate(directories, receipt)
        RESULTS.mkdir(parents=True, exist_ok=True)
        for scope, profiles in directories.items():
            for profile, directory in profiles.items():
                destination = RESULTS / scope / profile
                destination.mkdir(parents=True, exist_ok=True)
                with tarfile.open(destination / "evidence.tar.gz", "w:gz") as archive:
                    for path in sorted(directory.iterdir()):
                        archive.add(path, arcname=path.name)
                shutil.copyfile(directory / "context.json", destination / "context.json")
        (RESULTS / "run.json").write_text(publication.encoded(receipt))
        write_data(datasets)
    print("Published the tokenizer comparison.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--run-id", type=int)
    args = parser.parse_args()
    if args.check:
        check()
    elif args.run_id:
        publish(args.run_id)
    else:
        parser.error("Use --check or --run-id")


if __name__ == "__main__":
    main()
