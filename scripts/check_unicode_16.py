#!/usr/bin/env python3
"""Verify that direct UCD 16 generation reproduces the recorded baseline tables."""

from __future__ import annotations

import hashlib
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

import gen_unicode_tables as generator

ROOT = Path(__file__).resolve().parents[1]
UCD_DIR = ROOT / ".cache" / "unicode" / "16.0.0" / "ucd"
TABLES = {
    "property_data.rs": "property_data",
    "fold_data.rs": "fold_data",
    "egcb_data.rs": "egcb_data",
    "wb_data.rs": "wb_data",
}


def payload_digest(path: Path) -> str:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    while lines and lines[0].startswith("//"):
        lines.pop(0)
    if lines and not lines[0].strip():
        lines.pop(0)
    lines = [
        line
        for line in lines
        if not line.startswith("pub const PROP_INDEX_EXTENDEDPICTOGRAPHIC:")
    ]
    return hashlib.sha256("".join(lines).encode("utf-8")).hexdigest()


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ucd-dir", type=Path, default=UCD_DIR)
    args = parser.parse_args()
    ucd_dir = args.ucd_dir.resolve()
    generator.validate_version(ucd_dir, "16.0.0")

    generated = {
        "property_data.rs": generator.make_property_data(ucd_dir, "16.0.0"),
        "fold_data.rs": generator.make_fold_data(ucd_dir, "16.0.0"),
        "egcb_data.rs": generator.make_break_data(
            ucd_dir / "auxiliary" / "GraphemeBreakProperty.txt", "16.0.0", "egcb"
        ),
        "wb_data.rs": generator.make_break_data(
            ucd_dir / "auxiliary" / "WordBreakProperty.txt", "16.0.0", "wb"
        ),
    }
    with tempfile.TemporaryDirectory(prefix="ferroni-unicode-16-") as temporary:
        output_dir = Path(temporary)
        for filename, content in generated.items():
            (output_dir / filename).write_text(content, encoding="utf-8")
        subprocess.run(
            ["rustfmt", "--edition", "2024", *(str(output_dir / name) for name in generated)],
            check=True,
        )
        with (ROOT / "unicode_data.toml").open("rb") as source:
            expected = tomllib.load(source)["baseline"]["16.0.0"]
        failures = []
        for filename, key in TABLES.items():
            actual = payload_digest(output_dir / filename)
            if actual != expected[key]:
                failures.append(f"{filename}: expected {expected[key]}, got {actual}")
            else:
                print(f"Unicode 16.0 table body matches: {filename} ({actual})")
        property_source = (output_dir / "property_data.rs").read_text(encoding="utf-8")
        expected_index = expected["property_index_extended_pictographic"]
        if f"pub const PROP_INDEX_EXTENDEDPICTOGRAPHIC: u32 = {expected_index};" not in property_source:
            failures.append(
                f"Unicode 16.0 Extended_Pictographic ctype index should be {expected_index}"
            )
        if failures:
            raise SystemExit("\n".join(failures))
    print("All Unicode 16.0 table bodies match the recorded baseline.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
