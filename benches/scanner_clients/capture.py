#!/usr/bin/env python3
"""Capture real Ferriki scanner lifetimes without modifying either checkout.

Requires a Ferriki checkout with locked assets materialized. Instrumentation
exists only in temporary source copies, never in the measured Ferroni library.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compact(path):
    scanners, subjects, events, caches, phases = [], [], [], {}, []
    scanner_ids, subject_ids = {}, {}
    live = set()
    for line in path.read_text().splitlines():
        row = json.loads(line)
        kind = row["kind"]
        if kind == "phase":
            phases.append({"language": row["language"], "event": len(events)})
        elif kind == "create":
            sid = row["scanner"]
            if sid in scanner_ids:
                raise ValueError("duplicate scanner creation")
            if (row["syntax"], row["options"], row["prefilter"], row["warmup"]) != ("Oniguruma", 256, True, 32):
                raise ValueError("capture configuration differs from benchmark defaults")
            index = scanner_ids[sid] = len(scanners)
            cache = row["cache"]
            if cache is not None:
                cache = caches.setdefault(cache, len(caches))
            scanners.append({"patterns": row["patterns"], "cache": cache})
            live.add(index)
            events.append(["create", index])
        elif kind == "find":
            sid = scanner_ids[row["scanner"]]
            if sid not in live:
                raise ValueError("search after scanner destruction")
            tid = row["subject_id"]
            if tid not in subject_ids:
                subject_ids[tid] = len(subjects)
                subjects.append(row["subject"])
            subject = subject_ids[tid]
            if subjects[subject] != row["subject"]:
                raise ValueError("subject identity changed content")
            expected = row["expected"]
            events.append(["find", sid, subject, row["start"], row["options"],
                           None if expected is None else expected["index"],
                           [] if expected is None else expected["captures"]])
        elif kind == "drop":
            sid = scanner_ids[row["scanner"]]
            if sid not in live:
                raise ValueError("duplicate scanner destruction")
            live.remove(sid)
            events.append(["drop", sid])
        elif kind in ("clear", "drop_cache"):
            cache = caches.setdefault(row["cache"], len(caches))
            events.append([kind, cache])
        else:
            raise ValueError(f"unknown trace event: {kind}")
    if live:
        raise ValueError("capture omitted scanner destruction")
    return dict(format_version=1, scanners=scanners, subjects=subjects,
                caches=len(caches), phases=phases, events=events)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ferriki", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--ferriki-ref", default="HEAD", help="revision whose copied runtime inputs must match")
    args = parser.parse_args()
    ferriki = args.ferriki.resolve()
    reference = subprocess.check_output(["git", "rev-parse", args.ferriki_ref], cwd=ferriki, text=True).strip()
    subprocess.run(["git", "diff", "--exit-code", reference, "--", "Cargo.toml", "Cargo.lock",
                    "build.rs", "src", "crates", "examples", "assets/catalog.json",
                    "node/benchmarks/curated/fixtures"], cwd=ferriki, check=True)
    work = args.work_dir or Path(tempfile.mkdtemp(prefix="ferroni-client-capture-"))
    work.mkdir(parents=True, exist_ok=True)
    engine, client = work / "engine", work / "client"
    for path in (engine, client):
        if path.exists():
            raise ValueError(f"refusing to overwrite {path}")
        path.mkdir()
    for name in ("Cargo.toml", "Cargo.lock", "build.rs", "README.md", "src"):
        source = ROOT / name
        if source.is_dir():
            shutil.copytree(source, engine / name)
        else:
            shutil.copy2(source, engine / name)
    subprocess.run(["git", "apply", "--unidiff-zero", str(HERE / "capture.patch")], cwd=engine, check=True)
    for name in ("Cargo.toml", "Cargo.lock", "build.rs", "README.md", "src", "crates", "examples"):
        source = ferriki / name
        if source.is_dir():
            shutil.copytree(source, client / name, ignore=shutil.ignore_patterns("target"))
        else:
            shutil.copy2(source, client / name)
    (client / "assets").mkdir()
    shutil.copy2(ferriki / "assets/catalog.json", client / "assets/catalog.json")
    (client / "assets/standard").symlink_to(ferriki / "assets/standard", target_is_directory=True)
    shutil.copy2(HERE / "capture.rs", client / "examples/capture_scanner_client.rs")
    config = f'patch.crates-io.ferroni.path="{engine}"'
    command = ["cargo", "run", "--offline", "--release", "--example", "capture_scanner_client",
               "--config", config, "--"]
    fixtures = ferriki / "node/benchmarks/curated/fixtures"
    provenance = {
        "ferroni_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "ferriki_revision": reference,
        "capture_patch_sha256": sha(HERE / "capture.patch"),
        "asset_catalog_sha256": sha(ferriki / "assets/catalog.json"),
        "fixture_sha256": {name: sha(fixtures / name) for name in ("orders.ts", "Orders.tsx", "Orders.astro")},
        "copies": 16,
        "boundary": "Actual Scanner constructor, UTF-16 search, drop, pattern-cache clear/drop events; captures before TextMate postprocessing.",
    }
    for case in ("tsx", "astro", "related"):
        raw = work / f"{case}.jsonl"
        raw.write_text("")
        subprocess.run(command + [str(client / "assets/standard"), str(fixtures), case],
                       cwd=client, env=dict(os.environ, FERRONI_CLIENT_TRACE=str(raw)), check=True)
        output = HERE / f"{case}.json"
        output.write_text(json.dumps(compact(raw), ensure_ascii=False, separators=(",", ":")) + "\n")
        provenance[f"{case}_trace_sha256"] = sha(output)
    for source, name in [(ferriki / "assets/standard/GRAMMAR-NOTICES.md", "GRAMMAR-NOTICES.md"),
                         (ferriki / "LICENSE-MIT", "LICENSE-ferriki-MIT"),
                         (ferriki / "LICENSE-APACHE", "LICENSE-ferriki-APACHE")]:
        shutil.copy2(source, HERE / name)
        provenance[name + "_sha256"] = sha(HERE / name)
    provenance["capture_driver_sha256"] = sha(HERE / "capture.rs")
    (HERE / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    print(f"Raw captures and instrumented copies retained at {work}")


if __name__ == "__main__":
    main()
