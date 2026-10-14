#!/usr/bin/env python3
"""Replay current tiktoken patterns with Python regex, outside the timed harness.

Install regex==2026.1.15 to reproduce the retained oracle runtime. This checks
regex pre-tokenization only, without downloading vocabularies or running BPE.
"""
import json
from pathlib import Path
import regex

catalog = json.loads((Path(__file__).resolve().parents[1] / "benches/tokenizer_regex/catalog.json").read_text())
for case in catalog["cases"]:
    original = regex.compile(case["original_pattern"])
    adapted = regex.compile(case["pattern"])
    for index, fixture in enumerate(case["inputs"]):
        text = fixture["text"]
        spans = lambda pattern: [[len(text[:match.start()].encode()), len(text[:match.end()].encode())] for match in pattern.finditer(text)]
        assert spans(original) == spans(adapted) == fixture["expected"], (case["id"], index)
    print(f"{case['id']}: {len(case['inputs'])} original and adapted oracle inputs pass with Python regex {regex.__version__}.")
