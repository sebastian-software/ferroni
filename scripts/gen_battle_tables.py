#!/usr/bin/env python3
"""Render the battle_bench tables of docs/app/routes/perf/benchmark-results.mdx.

Reads the Criterion results that `cargo bench --features ffi --bench
battle_bench` leaves in target/criterion and prints the "Reference suite"
tables in the layout of the benchmark page, ready to paste. Each value is
Criterion's point estimate as printed on the console: the slope estimate when
Criterion has one, otherwise the mean.

Usage:
    ./scripts/gen_battle_tables.py [--criterion-dir target/criterion]

The script fails when an expected benchmark has no result, so a partial run
cannot produce a partial table.
"""

import argparse
import json
import sys
from pathlib import Path

ENGINES = ("rust", "c", "regex")

TEXT_SCANNING_ROWS = [
    ("Literal in 50 KB", "literal_50k"),
    ("No match, 50 KB", "no_match_50k"),
    ("No match, 10 KB", "no_match_10k"),
    ("Field extract, 50 KB", "field_extract_50k"),
    ("Timestamp, 50 KB", "timestamp_50k"),
]

PATTERN_ROWS = [
    ("Literal exact", "literal_exact"),
    ("Quantifier greedy", "quantifier_greedy"),
    ("Lookaround combined", "lookaround_combined"),
    ("Unicode `\\p{Greek}+`", "unicode_greek"),
    ("Backref `(\\w+) \\1`", "backref_simple"),
    ("Case-insensitive phrase", "case_insensitive_phrase"),
    ("Alternation, 2 branches", "alternation_2_branch"),
    ("Alternation, 10 branches", "alternation_10_branch"),
    ("Named capture date", "named_capture_date"),
]

COMPILATION_ROWS = [
    ("Literal", "literal"),
    ("Named capture", "named_capture"),
    ("Lookbehind", "lookbehind"),
]

# (group label, benchmark prefix, [(row label, scenario)]) for the warm
# scanner_highlighting group. The pattern count is read from the results.
SCANNER_GRAMMARS = [
    (
        "TypeScript",
        "ts",
        [
            ("Compile", "compile"),
            ("First match, short line", "first_match"),
            ("Tokenize full line", "tokenize"),
        ],
    ),
    ("CSS", "css", [("Compile", "compile"), ("Tokenize (multi-line)", "tokenize")]),
    (
        "Rust",
        "rust",
        [
            ("Compile", "compile"),
            ("First match", "first_match"),
            ("Tokenize full line", "tokenize"),
        ],
    ),
]


def load_results(criterion_dir):
    """Map (group_id, function_id, value_str) to the point estimate in ns."""
    results = {}
    for benchmark_file in criterion_dir.glob("**/new/benchmark.json"):
        benchmark = json.loads(benchmark_file.read_text())
        estimates = json.loads((benchmark_file.parent / "estimates.json").read_text())
        estimate = estimates.get("slope") or estimates["mean"]
        key = (
            benchmark["group_id"],
            benchmark.get("function_id"),
            benchmark.get("value_str"),
        )
        results[key] = estimate["point_estimate"]
    return results


class Results:
    def __init__(self, raw):
        self.raw = raw
        self.missing = []

    def get(self, group, function, value=None, optional=False):
        value_ns = self.raw.get((group, function, value))
        if value_ns is None and not optional:
            self.missing.append("/".join(part for part in (group, function, value) if part))
        return value_ns

    def functions(self, group):
        return sorted(function for (g, function, _) in self.raw if g == group and function)


def fmt_time(ns):
    if ns is None:
        return "—"
    for unit, scale in (("ms", 1e6), ("us", 1e3)):
        if ns >= scale:
            return f"{ns / scale:.3f} {unit}"
    return f"{ns:.3f} ns"


def fmt_factor(factor):
    return f"{factor:.2f}x" if factor < 2 else f"{factor:.1f}x"


def table(header, align, rows):
    """Render a Markdown table padded the way oxfmt formats it."""
    widths = [len(cell) for cell in header]
    for row in rows:
        widths = [max(width, len(cell)) for width, cell in zip(widths, row)]

    def line(cells):
        padded = [
            cell.ljust(width) if side == "l" else cell.rjust(width)
            for cell, width, side in zip(cells, widths, align)
        ]
        return "| " + " | ".join(padded) + " |"

    separator = "| " + " | ".join(
        "-" * width if side == "l" else "-" * (width - 1) + ":"
        for width, side in zip(widths, align)
    ) + " |"
    return "\n".join([line(header), separator, *(line(row) for row in rows)])


def bold_fastest(values):
    present = [value for value in values if value is not None]
    fastest = min(present) if present else None
    return [
        f"**{fmt_time(value)}**" if value is not None and value == fastest else fmt_time(value)
        for value in values
    ]


def engine_table(first_header, rows):
    header = [first_header, "Ferroni", "Oniguruma", "`regex`"]
    body = [[label, *bold_fastest(values)] for label, values in rows]
    return table(header, "lrrr", body)


def engine_rows(results, group, spec, regex_optional=True):
    return [
        (
            label,
            [
                results.get(group, "rust", name),
                results.get(group, "c", name),
                results.get(group, "regex", name, optional=regex_optional),
            ],
        )
        for label, name in spec
    ]


def scanner_row(label, rust_ns, c_ns):
    factor = fmt_factor(c_ns / rust_ns)
    ferroni, oniguruma = bold_fastest([rust_ns, c_ns])
    return [label, ferroni, oniguruma, f"**{factor}**" if rust_ns < c_ns else factor]


def pattern_count(results, group, prefix):
    for function in results.functions(group):
        parts = function.split("_")
        if parts[0] == prefix and parts[1].isdigit():
            return int(parts[1])
    results.missing.append(f"{group}/{prefix}_*")
    return None


def scanner_table(results):
    body = []
    for grammar, prefix, spec in SCANNER_GRAMMARS:
        count = pattern_count(results, "scanner_highlighting", prefix)
        if count is None:
            continue
        body.append([f"**{grammar} ({count} patterns)**", "", "", ""])
        for label, scenario in spec:
            rust_ns = results.get("scanner_highlighting", f"{prefix}_{count}_{scenario}_rust")
            c_ns = results.get("scanner_highlighting", f"{prefix}_{count}_{scenario}_c")
            if rust_ns is not None and c_ns is not None:
                body.append(scanner_row(label, rust_ns, c_ns))
    return table(["Scenario", "Ferroni", "Oniguruma", "Factor"], "lrrr", body)


def document_table(results):
    body = []
    group = "scanner_documents"
    for grammar, prefix, _ in SCANNER_GRAMMARS:
        matches = [
            function
            for function in results.functions(group)
            if function.startswith(f"{prefix}_") and function.endswith("_lines_rust")
        ]
        if len(matches) != 1:
            results.missing.append(f"{group}/{prefix}_*_lines_rust")
            continue
        base = matches[0].removesuffix("_rust")
        parts = base.split("_")
        count, lines = parts[1], parts[3]
        rust_ns = results.get(group, f"{base}_rust")
        c_ns = results.get(group, f"{base}_c")
        if rust_ns is not None and c_ns is not None:
            body.append(scanner_row(f"{grammar} ({count} patterns), {lines} lines", rust_ns, c_ns))
    return table(["Document", "Ferroni", "Oniguruma", "Factor"], "lrrr", body)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--criterion-dir", type=Path, default=Path("target/criterion"))
    args = parser.parse_args()

    results = Results(load_results(args.criterion_dir))

    text_rows = engine_rows(results, "text_scanning", TEXT_SCANNING_ROWS, regex_optional=False)
    text_rows.append(
        (
            "RegSet multi-pattern (5)",
            [
                results.get("text_scanning", "regset_position_lead_rust"),
                results.get("text_scanning", "regset_position_lead_c"),
                None,
            ],
        )
    )

    sections = [
        ("Text search and log scanning", engine_table("Scenario", text_rows)),
        (
            "Pattern matching",
            engine_table("Category", engine_rows(results, "single_pattern", PATTERN_ROWS)),
        ),
        (
            "Compilation",
            engine_table("Pattern", engine_rows(results, "compilation", COMPILATION_ROWS)),
        ),
        ("Scanner with full Shiki TextMate grammars", scanner_table(results)),
        ("Scanner on whole documents, line by line", document_table(results)),
    ]

    if results.missing:
        print("Missing Criterion results (run the full battle_bench first):", file=sys.stderr)
        for name in results.missing:
            print(f"  {name}", file=sys.stderr)
        return 1

    print("\n\n".join(f"### {title}\n\n{body}" for title, body in sections))
    return 0


if __name__ == "__main__":
    sys.exit(main())
