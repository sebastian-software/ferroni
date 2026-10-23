#!/usr/bin/env python3
"""Build a fork-free producer/consumer with optional frozen comparators.

This script copies source into an isolated build directory and verifies that
both dependencies resolve to checksummed crates.io packages. Historical caches
may supply immutable comparator executables; no snapshot dependency is built.
Nothing in the source checkout is rewritten.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cache", type=Path)
    parser.add_argument("--snapshot-cache", type=Path, help="Retain the frozen historical snapshot comparator")
    parser.add_argument("--portable-reference-cache", type=Path, help="Retain a frozen older portable executable as the before variant")
    parser.add_argument("--refresh", action="store_true", help="Refresh this experiment's isolated source directory")
    args = parser.parse_args()
    cache = args.cache.resolve()
    source = cache / "registry-source"
    source.mkdir(parents=True, exist_ok=args.refresh)
    paths = ["src", "tests", "benches", "examples", ".cargo", "docs", "build.rs",
             "README.md", "README.md.src", "LICENSE", "Cargo.toml", "Cargo.lock"]
    for name in paths:
        original, dest = ROOT / name, source / name
        if original.is_dir():
            shutil.copytree(original, dest, dirs_exist_ok=args.refresh, ignore=shutil.ignore_patterns("diagnostics", "node_modules", ".react-router"))
        elif original.is_file():
            shutil.copy2(original, dest)
    manifest = (source / "Cargo.toml").read_text()
    manifest = manifest.split("# Local experiment: upstream execution")[0]
    manifest = manifest.replace('"regex-automata/ferroni-snapshot", "aho-corasick/ferroni-snapshot"', '')
    (source / "Cargo.toml").write_text(manifest)
    log = cache / "build-logs"
    log.mkdir(exist_ok=True)
    metadata = subprocess.run(["cargo", "metadata", "--offline", "--format-version=1"], cwd=source,
                              check=True, capture_output=True, text=True)
    (log / "metadata.json").write_text(metadata.stdout)
    packages = [p for p in json.loads(metadata.stdout)["packages"] if p["name"] in ["regex-automata", "aho-corasick"]]
    assert len(packages) == 2
    assert all(p["source"] == "registry+https://github.com/rust-lang/crates.io-index" for p in packages)
    assert "prototype-deps" not in metadata.stdout
    provenance = {"registry_dependencies": [{k: p[k] for k in ["name", "version", "source", "manifest_path"]} for p in packages],
                  "transformed_manifest_sha256": digest(source / "Cargo.toml"),
                  "registry_lock_sha256": digest(source / "Cargo.lock"), "source_sha256": {}, "binaries": {}}
    for path in source.rglob("*"):
        if path.is_file() and "target" not in path.relative_to(source).parts:
            provenance["source_sha256"][str(path.relative_to(source))] = digest(path)
    bins = cache / "bin"
    bins.mkdir(exist_ok=True)
    for label, features in [("eager", "precompiled"), ("portable", "portable-eager-prototype")]:
        for kind, target in [("example", "scanner_group_archive"), ("bench", "scanner_group_archive_bench")]:
            cmd = ["cargo", "build" if kind == "example" else "bench", "--offline", "--locked"]
            cmd += ["--release"] if kind == "example" else ["--no-run"]
            cmd += ["--features", features, "--" + kind, target, "--message-format=json"]
            print("Building registry", label, kind, flush=True)
            result = subprocess.run(cmd, cwd=source, check=True, capture_output=True, text=True, timeout=300)
            (log / f"{label}-{kind}.log").write_text(result.stdout + result.stderr)
            records = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
            executable = next(r["executable"] for r in reversed(records) if r.get("reason") == "compiler-artifact"
                              and r.get("executable") and r["target"]["name"] == target)
            dest = bins / (label + ("-bench" if kind == "bench" else ""))
            shutil.copy2(executable, dest)
            provenance["binaries"][dest.name] = digest(dest)
    artifacts = cache / "artifacts"
    artifacts.mkdir(exist_ok=True)
    if args.snapshot_cache:
        reference = args.snapshot_cache.resolve()
        for name in ["prepared", "prepared-bench"]:
            original = reference / "bin" / name
            shutil.copy2(original, bins / name)
            provenance["binaries"][name] = digest(original)
        for path in (reference / "artifacts").glob("*"):
            if path.suffix in [".programs", ".prepared"]:
                shutil.copy2(path, artifacts / path.name)
        provenance["snapshot_context"] = str(reference / "run-01/context.json")
        provenance["snapshot_context_sha256"] = digest(reference / "run-01/context.json")
        expected = json.loads((reference / "run-01/context.json").read_text())["binaries"]["prepared"]
        assert provenance["binaries"]["prepared"] == expected["example"]
        assert provenance["binaries"]["prepared-bench"] == expected["bench"]
    if args.portable_reference_cache:
        previous = args.portable_reference_cache.resolve()
        previous_context = json.loads((previous / "run-01/context.json").read_text())
        for name, key in [("portable", "example"), ("portable-bench", "bench")]:
            dest = "before" + ("-bench" if key == "bench" else "")
            original = previous / "bin" / name
            assert digest(original) == previous_context["binaries"]["portable"][key]
            shutil.copy2(original, bins / dest)
            provenance["binaries"][dest] = digest(original)
        provenance["portable_reference_context_sha256"] = digest(previous / "run-01/context.json")
    (cache / "build-provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    with tarfile.open(cache / "registry-source.tar.gz", "w:gz") as archive:
        for name in provenance["source_sha256"]:
            archive.add(source / name, arcname=name, recursive=False)
    print("Verified registry packages and selected frozen references", flush=True)


if __name__ == "__main__":
    main()
