#!/usr/bin/env python3
"""Offline integrity checks for the source-backed regex corpus."""

import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "benches/production_regex/catalog.json"


def check(catalog, directory=CATALOG.parent):
    assert catalog["format_version"] == 2
    assert catalog["offset_unit"] == "UTF-8 bytes"
    sources = catalog["sources"]
    for source in sources.values():
        assert re.fullmatch(r"[0-9a-f]{40}", source["commit"])
        assert source["commit"] in source["url"]
        assert re.fullmatch(r"[0-9a-f]{64}", source["sha256"])
    for license in catalog["licenses"].values():
        data = (directory / license["file"]).read_bytes()
        assert hashlib.sha256(data).hexdigest() == license["sha256"]
    ids = set()
    count = 0
    for case in catalog["cases"]:
        assert case["id"] not in ids, case["id"]
        ids.add(case["id"])
        assert case["source"]["file"] in sources
        assert case["family"] and case["tier"] in ("basic", "complex", "advanced")
        if case.get("pending_reason"):
            assert not case["inputs"], "pending cases must not accidentally be timed"
            continue
        assert case["license"] in catalog["licenses"]
        assert case["scope"] and case["oracle_runtime"]
        assert case["pattern"] and case["inputs"]
        assert hashlib.sha256(case["original_pattern"].encode()).hexdigest() == case["original_pattern_sha256"]
        operation = case["operation"]
        assert operation in ("is_match", "find", "captures", "captures_at", "all_captures", "all_matches", "replace", "graphemes")
        if case.get("anchor_mode") == "slice":
            assert case["pattern"].startswith(r"\A")
            assert case["id"] in ("jinja_string_token", "jinja_integer_token")
        if case.get("anchor_mode") == "context":
            assert case["pattern"].startswith(r"\G")
        for fixture in case["inputs"]:
            assert fixture["source"]["file"] in sources
            assert fixture["derivation"]
            data = fixture["text"].encode()
            boundaries = {len(fixture["text"][:n].encode()) for n in range(len(fixture["text"]) + 1)}
            assert fixture["start"] in boundaries
            expected = fixture["expected"]
            if operation == "is_match":
                assert type(expected) is bool
            elif operation == "replace":
                assert isinstance(expected, str)
            elif operation == "graphemes":
                assert all(isinstance(chunk, str) and chunk for chunk in expected)
                assert "".join(expected) == fixture["text"]
            else:
                groups = ([expected] if operation == "find" else expected) if expected is not None else []
                if operation == "all_captures":
                    groups = [bound for captures in expected for bound in captures]
                for bound in groups:
                    if bound is not None:
                        assert len(bound) == 2 and 0 <= bound[0] <= bound[1] <= len(data)
                        assert all(position in boundaries for position in bound)
            count += 1
    return len(ids), count


if __name__ == "__main__":
    cases, fixtures = check(json.loads(CATALOG.read_text()))
    print(f"Production corpus: {cases} cases, {fixtures} oracle fixtures; integrity checks passed.")
