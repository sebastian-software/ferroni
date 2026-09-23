#!/usr/bin/env python3
"""Generate Ferroni's Unicode tables from a pinned Unicode Character Database."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import OrderedDict
from pathlib import Path

MAX_CODE_POINT = 0x10FFFF
POSIX_LIST = [
    "NEWLINE", "Alpha", "Blank", "Cntrl", "Digit", "Graph", "Lower",
    "Print", "PosixPunct", "Space", "Upper", "XDigit", "Word", "Alnum",
    "ASCII",
]

RANGE_RE = re.compile(r"([0-9A-Fa-f]+)(?:\.\.([0-9A-Fa-f]+))?\s*;\s*(\w+)")
TOTAL_RE = re.compile(r"#\s*Total\s+(?:code\s+points|elements):")
VERSION_RE = re.compile(r"#\s*.*-(\d+)\.(\d+)\.(\d+)\.txt")
EMOJI_VERSION_RE = re.compile(r"(?i)#.*Version\s*:?\s*(\d+)\.(\d+)")
PROPERTY_ALIAS_RE = re.compile(r"(\w+)\s*;\s*(\w+)")
PROPERTY_VALUE_ALIAS_RE = re.compile(r"(sc|gc)\s*;\s*(\w+)\s*;\s*(\w+)(?:\s*;\s*(\w+))?")


def normalize_ranges(ranges: list[tuple[int, int]], sort: bool = False) -> list[tuple[int, int]]:
    ordered = sorted(ranges) if sort else ranges
    result: list[tuple[int, int]] = []
    for start, end in ordered:
        if result and result[-1][1] >= start - 1:
            old_start, old_end = result.pop()
            result.append((old_start, max(old_end, end)))
        else:
            result.append((start, end))
    return result


def inverse_ranges(ranges: list[tuple[int, int]]) -> list[tuple[int, int]]:
    result = []
    previous = 0
    for start, end in ranges:
        if previous < start:
            result.append((previous, start - 1))
        previous = end + 1
    if previous < MAX_CODE_POINT:
        result.append((previous, MAX_CODE_POINT))
    return result


def add_ranges(*groups: list[tuple[int, int]]) -> list[tuple[int, int]]:
    return normalize_ranges([item for group in groups for item in group], sort=True)


def subtract_ranges(source: list[tuple[int, int]], removed: list[tuple[int, int]]) -> list[tuple[int, int]]:
    result: list[tuple[int, int]] = []
    for start, end in source:
        cursor = start
        for remove_start, remove_end in removed:
            if remove_end < cursor:
                continue
            if remove_start > end:
                break
            if remove_start > cursor:
                result.append((cursor, remove_start - 1))
            cursor = max(cursor, remove_end + 1)
            if cursor > end:
                break
        if cursor <= end:
            result.append((cursor, end))
    return result


def add_range(data: dict[str, list[tuple[int, int]]], name: str, start: int, end: int) -> None:
    data.setdefault(name, []).append((start, end))


def normalize_dictionary(data: dict[str, list[tuple[int, int]]]) -> None:
    for name, ranges in data.items():
        data[name] = normalize_ranges(ranges)


def parse_unicode_data(path: Path) -> tuple[dict[str, list[tuple[int, int]]], list[tuple[int, int]]]:
    properties: dict[str, list[tuple[int, int]]] = {}
    assigned: list[tuple[int, int]] = []
    pending_start: int | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split(";")
        if len(fields) < 3:
            continue
        code = int(fields[0], 16)
        name = fields[1]
        category = fields[2]
        if name.endswith(", First>"):
            pending_start = code
            continue
        if name.endswith(", Last>"):
            if pending_start is None:
                raise ValueError(f"UnicodeData has an unmatched First/Last range: {line}")
            start, end = pending_start, code
            pending_start = None
        else:
            start = end = code
        assigned.append((start, end))
        add_range(properties, category, start, end)
        if len(category) == 2:
            add_range(properties, category[0], start, end)
    if pending_start is not None:
        raise ValueError("UnicodeData ends with an unmatched First range")
    normalize_dictionary(properties)
    return properties, assigned


def parse_property_file(path: Path, prefix: str = "") -> tuple[dict[str, list[tuple[int, int]]], set[str]]:
    properties: dict[str, list[tuple[int, int]]] = {}
    names: set[str] = set()
    current: str | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        match = RANGE_RE.match(stripped)
        if match:
            current = prefix + match.group(3)
            start = int(match.group(1), 16)
            end = int(match.group(2), 16) if match.group(2) else start
            add_range(properties, current, start, end)
        elif TOTAL_RE.match(stripped):
            if current is not None:
                names.add(current)
    normalize_dictionary(properties)
    return properties, names


def parse_property_aliases(path: Path) -> dict[str, str]:
    aliases = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = PROPERTY_ALIAS_RE.match(line.strip())
        if match and match.group(1) != match.group(2):
            aliases[match.group(1)] = match.group(2)
    return aliases


def parse_property_value_aliases(path: Path) -> dict[str, str]:
    aliases = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = PROPERTY_VALUE_ALIAS_RE.match(line.strip())
        if not match:
            continue
        category, short_name, long_name, alias = match.groups()
        if category == "sc":
            if short_name != long_name:
                aliases[short_name] = long_name
            if alias and alias != long_name:
                aliases[alias] = long_name
        else:
            if short_name != long_name:
                aliases[long_name] = short_name
            if alias and alias != short_name:
                aliases[alias] = short_name
    return aliases


def parse_blocks(path: Path) -> tuple[dict[str, list[tuple[int, int]]], list[str]]:
    properties: dict[str, list[tuple[int, int]]] = {}
    blocks = []
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.match(r"([0-9A-Fa-f]+)\.\.([0-9A-Fa-f]+)\s*;\s*(.*)", line.strip())
        if not match:
            continue
        start, end = int(match.group(1), 16), int(match.group(2), 16)
        name = "In_" + re.sub(r"[- ]+", "_", match.group(3))
        add_range(properties, name, start, end)
        blocks.append(name)
    occupied = normalize_ranges([item for ranges in properties.values() for item in ranges], sort=True)
    no_block = "In_No_Block"
    properties[no_block] = inverse_ranges(occupied)
    blocks.append(no_block)
    normalize_dictionary(properties)
    return properties, blocks


def normalize_property_name(name: str) -> str:
    return re.sub(r"[ _]", "", name).lower()


def make_property_data(ucd: Path, version: str) -> str:
    data, assigned = parse_unicode_data(ucd / "UnicodeData.txt")
    data["Assigned"] = normalize_ranges(assigned)
    data["Any"] = [(0, MAX_CODE_POINT)]
    data["ASCII"] = [(0, 0x7F)]
    data["NEWLINE"] = [(0x0A, 0x0A)]
    data["Cn"] = inverse_ranges(data["Assigned"])
    data["C"].extend(data["Cn"])
    data["C"] = normalize_ranges(data["C"], sort=True)
    data["LC"] = add_ranges(data["Ll"], data["Lt"], data["Lu"])

    property_names: dict[str, bool] = {name: True for name in data if name not in POSIX_LIST}
    core, _ = parse_property_file(ucd / "DerivedCoreProperties.txt")
    data.update(core)
    property_names.update({name: True for name in core})

    scripts, _ = parse_property_file(ucd / "Scripts.txt")
    data.update(scripts)
    property_names.update({name: True for name in scripts})
    data["Unknown"] = inverse_ranges(normalize_ranges([r for group in scripts.values() for r in group], sort=True))

    prop_list, _ = parse_property_file(ucd / "PropList.txt")
    data.update(prop_list)
    property_names.update({name: True for name in prop_list})

    emoji_data, _ = parse_property_file(ucd / "emoji" / "emoji-data.txt")
    data.update(emoji_data)
    property_names.update({name: True for name in emoji_data})

    aliases = parse_property_aliases(ucd / "PropertyAliases.txt")
    aliases.update(parse_property_value_aliases(ucd / "PropertyValueAliases.txt"))

    block_data, blocks = parse_blocks(ucd / "Blocks.txt")
    data.update(block_data)

    # POSIX property definitions from Oniguruma's UCD generator.
    alnum = add_ranges(data["Alphabetic"], data["Nd"])
    blank = add_ranges([(0x09, 0x09)], data["Zs"])
    word = add_ranges(data["Alphabetic"], data["M"], data["Nd"], data["Pc"])
    graph = subtract_ranges(data["Any"], data["White_Space"])
    graph = subtract_ranges(graph, data["Cc"])
    graph = subtract_ranges(graph, data["Cs"])
    graph = subtract_ranges(graph, data["Cn"])
    printable = add_ranges(graph, data["Zs"])
    data.update(
        {
            "Alpha": data["Alphabetic"],
            "Upper": data["Uppercase"],
            "Lower": data["Lowercase"],
            "PosixPunct": add_ranges(data["P"], data["S"]),
            "Digit": data["Nd"],
            "XDigit": [(0x30, 0x39), (0x41, 0x46), (0x61, 0x66)],
            "Alnum": alnum,
            "Space": data["White_Space"],
            "Blank": blank,
            "Cntrl": data["Cc"],
            "Word": word,
            "Graph": graph,
            "Print": printable,
        }
    )

    if "Unknown" not in property_names:
        property_names["Unknown"] = True
    prop_list = sorted(property_names)
    code_range_names = POSIX_LIST + prop_list + blocks

    ctype_by_name = {name: index for index, name in enumerate(code_range_names)}
    normalized_names = {normalize_property_name(name): index for name, index in ctype_by_name.items()}
    for alias, target in sorted(aliases.items(), key=lambda item: normalize_property_name(item[0])):
        normalized_alias = normalize_property_name(alias)
        if normalized_alias in normalized_names:
            continue
        target_index = normalized_names.get(normalize_property_name(target))
        if target_index is not None:
            normalized_names[normalized_alias] = target_index

    arrays: OrderedDict[str, list[int]] = OrderedDict()
    range_aliases: dict[str, str] = {}
    seen_ranges: dict[tuple[tuple[int, int], ...], str] = {}
    for name in code_range_names:
        ranges = tuple(data[name])
        canonical = seen_ranges.get(ranges)
        if canonical is None:
            canonical = name
            seen_ranges[ranges] = name
            arrays[canonical] = [value for pair in ranges for value in pair]
        else:
            range_aliases[name] = canonical

    lines = [
        "//! Auto-generated Unicode property data. Do not edit.",
        f"//! Generated from the Unicode {version} UCD (see unicode_data.toml).",
        "//! by scripts/gen_unicode_tables.py",
        "",
        "#![allow(dead_code, non_upper_case_globals)]",
        "",
        f"pub const CODE_RANGES_NUM: usize = {len(code_range_names)};",
        f"pub const PROP_INDEX_EXTENDEDPICTOGRAPHIC: u32 = {normalized_names['extendedpictographic']};",
        "",
        "// --- Code Range Arrays ---",
        "// Each array contains pairs of (start, end) code points.",
        "",
    ]
    emitted = set()
    for name in code_range_names:
        canonical = range_aliases.get(name, name)
        if canonical in emitted:
            continue
        emitted.add(canonical)
        values = arrays[canonical]
        lines.append(f"static CR_{canonical}: [u32; {len(values)}] = [")
        for i in range(0, len(values), 6):
            lines.append("    " + ", ".join(f"0x{value:06x}" for value in values[i : i + 6]) + ",")
        lines.extend(["];", ""])
    lines.extend(["// --- Index: ctype -> code ranges ---", f"pub static CODE_RANGES: [&[u32]; {len(code_range_names)}] = ["])
    for index, name in enumerate(code_range_names):
        canonical = range_aliases.get(name, name)
        lines.append(f"    &CR_{canonical},  // {index}: CR_{name}")
    lines.extend(["];", "", "// --- Property name lookup table (sorted, normalized) ---",
                  "// Names are already lowercase with spaces/hyphens/underscores removed.",
                  f"pub static PROPERTY_NAMES: [(&str, u16); {len(normalized_names)}] = ["])
    for name, index in sorted(normalized_names.items()):
        lines.append(f'    ("{name}", {index}),')
    lines.extend(["];", ""])
    return "\n".join(lines)


class FoldEntry:
    def __init__(self, fold: tuple[int, ...]):
        self.fold = fold
        self.unfolds: list[int] = []
        self.index = -1


def fold_key(fold: tuple[int, ...]) -> str:
    return ":".join(f"{code:06x}" for code in fold)


def parse_case_folding(path: Path):
    folds: dict[str, FoldEntry] = {}
    unfolds: dict[int, FoldEntry] = {}
    turkish_folds: dict[str, FoldEntry] = {}
    turkish_unfolds: dict[int, FoldEntry] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split("#", 1)[0].strip().split(";")
        if len(parts) < 3:
            continue
        code = int(parts[0].strip(), 16)
        status = parts[1].strip()
        if status == "S":
            continue
        targets = tuple(int(item, 16) for item in parts[2].split())
        if status not in ("C", "F", "T") or not targets or len(targets) > 3:
            continue
        is_turkish = status == "T"
        fold_data = turkish_folds if is_turkish else folds
        unfold_data = turkish_unfolds if is_turkish else unfolds
        key = fold_key(targets)
        entry = fold_data.get(key)
        if entry is None:
            entry = FoldEntry(targets)
            fold_data[key] = entry
        entry.unfolds.append(code)
        unfold_data[code] = entry
    return folds, unfolds, turkish_folds, turkish_unfolds


def build_fold_arrays(path: Path):
    folds, unfolds, _, turkish_unfolds = parse_case_folding(path)
    locale_folds: dict[str, FoldEntry] = {}
    locale_unfolds: dict[int, FoldEntry] = {}
    for code in turkish_unfolds:
        entry = unfolds.get(code)
        if entry is None:
            continue
        key = fold_key(entry.fold)
        if len(entry.unfolds) == 1:
            del folds[key]
        else:
            entry.unfolds.remove(code)
            entry = FoldEntry(entry.fold)
            entry.unfolds.append(code)
        locale_folds[key] = entry
        locale_unfolds[code] = entry
        del unfolds[code]

    def make_array(default: dict[str, FoldEntry], locale: dict[str, FoldEntry]):
        values: list[int] = []
        normal_end = 0
        for entries in (default, locale):
            for _, entry in sorted(entries.items()):
                entry.index = len(values)
                values.extend(entry.fold)
                values.append(len(entry.unfolds))
                values.extend(entry.unfolds)
            if entries is default:
                normal_end = len(values)
        return values, normal_end

    arrays = {}
    normal_ends = {}
    for fold_len in (1, 2, 3):
        default = {key: entry for key, entry in folds.items() if len(entry.fold) == fold_len}
        locale = {key: entry for key, entry in locale_folds.items() if len(entry.fold) == fold_len}
        arrays[fold_len], normal_ends[fold_len] = make_array(default, locale)

    all_unfolds = dict(unfolds)
    all_unfolds.update(locale_unfolds)
    unfold_key = sorted((code, entry.index, len(entry.fold)) for code, entry in all_unfolds.items())
    return arrays, normal_ends, unfold_key


def build_fold_lookup(values: list[int], fold_len: int):
    entries = []
    index = 0
    while index + fold_len < len(values):
        codes = tuple(values[index : index + fold_len])
        count = values[index + fold_len]
        entries.append((codes, index))
        index += fold_len + 1 + count
    return sorted(entries)


def make_fold_data(ucd: Path, version: str) -> str:
    arrays, normal_ends, unfold_key = build_fold_arrays(ucd / "CaseFolding.txt")
    lines = [
        "//! Auto-generated Unicode case fold data. Do not edit.",
        f"//! Generated from the Unicode {version} UCD (see unicode_data.toml).",
        "//! by scripts/gen_unicode_tables.py",
        "",
        "#![allow(dead_code)]",
        "",
    ]
    for fold_len in (1, 2, 3):
        lines.append(f"pub const FOLDS{fold_len}_NORMAL_END_INDEX: usize = {normal_ends[fold_len]};")
        lines.append(f"pub const FOLDS{fold_len}_END_INDEX: usize = {len(arrays[fold_len])};")
    lines.append("")
    for fold_len in (1, 2, 3):
        values = arrays[fold_len]
        lines.append(f"pub static UNICODE_FOLDS{fold_len}: [u32; {len(values)}] = [")
        for i in range(0, len(values), 8):
            lines.append("    " + ", ".join(f"0x{value:06x}" for value in values[i : i + 8]) + ",")
        lines.extend(["];", ""])
    for fold_len in (1, 2, 3):
        entries = build_fold_lookup(arrays[fold_len], fold_len)
        description = "codepoint" if fold_len == 1 else f"{fold_len}-codepoint sequence"
        if fold_len == 2:
            description = "(cp1, cp2)"
        elif fold_len == 3:
            description = "(cp1, cp2, cp3)"
        lines.append(f"// Fold{fold_len} key: {description} -> index into UNICODE_FOLDS{fold_len}")
        if fold_len == 1:
            lines.append(f"pub static FOLD1_KEY: [(u32, u16); {len(entries)}] = [")
            for (code,), index in entries:
                lines.append(f"    (0x{code:06x}, {index}),")
        else:
            lines.append(f"pub static FOLD{fold_len}_KEY: [([u32; {fold_len}], u16); {len(entries)}] = [")
            for codes, index in entries:
                rendered = ", ".join(f"0x{code:06x}" for code in codes)
                lines.append(f"    ([{rendered}], {index}),")
        lines.extend(["];", ""])
    lines.append("// Unfold key: codepoint -> (index, fold_len)")
    lines.append(f"pub static UNFOLD_KEY: [(u32, i16, u8); {len(unfold_key)}] = [")
    for code, index, fold_len in unfold_key:
        lines.append(f"    (0x{code:06x}, {index}, {fold_len}),")
    lines.extend(["];", ""])
    return "\n".join(lines)


EGCB_VARIANTS = {
    "CR": "CR", "Control": "Control", "Extend": "Extend", "L": "L",
    "LF": "LF", "LV": "LV", "LVT": "LVT", "Prepend": "Prepend",
    "Regional_Indicator": "RegionalIndicator", "SpacingMark": "SpacingMark",
    "T": "T", "V": "V", "ZWJ": "ZWJ",
}
WB_VARIANTS = {
    "ALetter": "ALetter", "CR": "CR", "Double_Quote": "DoubleQuote",
    "Extend": "Extend", "ExtendNumLet": "ExtendNumLet", "Format": "Format",
    "Hebrew_Letter": "HebrewLetter", "Katakana": "Katakana", "LF": "LF",
    "MidLetter": "MidLetter", "MidNum": "MidNum", "MidNumLet": "MidNumLet",
    "Newline": "Newline", "Numeric": "Numeric",
    "Regional_Indicator": "RegionalIndicator", "Single_Quote": "SingleQuote",
    "WSegSpace": "WSegSpace", "ZWJ": "ZWJ",
}


def make_break_data(path: Path, version: str, kind: str) -> str:
    properties, _ = parse_property_file(path)
    variants = EGCB_VARIANTS if kind == "egcb" else WB_VARIANTS
    ranges = sorted((start, end, prop) for prop, values in properties.items() for start, end in values)
    for previous, current in zip(ranges, ranges[1:]):
        if previous[1] >= current[0]:
            raise ValueError(f"Overlapping {kind} ranges: {previous} and {current}")
    type_name = "EgcbType" if kind == "egcb" else "WbType"
    range_name = "EgcbRange" if kind == "egcb" else "WbRange"
    const_name = "EGCB_RANGES" if kind == "egcb" else "WB_RANGES"
    title = "Extended Grapheme Cluster Break" if kind == "egcb" else "Word Break"
    lines = [
        f"// Auto-generated {title} data from Unicode {version} (see unicode_data.toml).",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "#[repr(u8)]",
        f"pub enum {type_name} {{",
    ]
    if kind == "egcb":
        lines.extend([
            "    Other = 0,", "    CR = 1,", "    LF = 2,", "    Control = 3,",
            "    Extend = 4,", "    Prepend = 5,", "    RegionalIndicator = 6,",
            "    SpacingMark = 7,", "    ZWJ = 8,", "    // 9-12 obsoleted",
            "    L = 13,", "    LV = 14,", "    LVT = 15,", "    T = 16,", "    V = 17,",
        ])
    else:
        lines.append("    Any = 0,")
        for index, variant in enumerate(WB_VARIANTS.values(), start=1):
            lines.append(f"    {variant} = {index},")
    lines.extend([
        "}", "",
        f"pub struct {range_name} {{",
        "    pub start: u32,", "    pub end: u32,", f"    pub prop: {type_name},",
        "}", "",
        f"pub static {const_name}: [{range_name}; {len(ranges)}] = [",
    ])
    for start, end, prop in ranges:
        variant = variants.get(prop)
        if variant is None:
            raise ValueError(f"Unexpected {kind} property {prop!r} in {path}")
        hex_format = "06X" if kind == "wb" else "06x"
        lines.extend([
            f"    {range_name} {{",
            f"        start: 0x{start:{hex_format}},",
            f"        end: 0x{end:{hex_format}},",
            f"        prop: {type_name}::{variant},",
            "    },",
        ])
    lines.extend(["];", ""])
    return "\n".join(lines)


def validate_version(ucd: Path, expected: str) -> None:
    expected_tuple = tuple(int(part) for part in expected.split("."))
    header = (ucd / "DerivedCoreProperties.txt").read_text(encoding="utf-8")
    match = next((VERSION_RE.match(line) for line in header.splitlines() if line.startswith("#")), None)
    if not match:
        raise ValueError("Could not read Unicode version from DerivedCoreProperties.txt")
    actual = tuple(int(match.group(i)) for i in range(1, 4))
    if actual != expected_tuple:
        raise ValueError(f"Expected Unicode {expected}, found {'.'.join(map(str, actual))}")
    emoji = (ucd / "emoji" / "emoji-data.txt").read_text(encoding="utf-8")
    emoji_match = next((match for line in emoji.splitlines() if (match := EMOJI_VERSION_RE.match(line))), None)
    if not emoji_match or tuple(int(emoji_match.group(i)) for i in (1, 2)) != expected_tuple[:2]:
        raise ValueError(f"emoji-data.txt does not match Unicode {expected}")


def extract_property_names(source: str) -> list[str]:
    start = source.index("pub static PROPERTY_NAMES:")
    return re.findall(r'^\s+\("([^"]+)", \d+\),$', source[start:], re.MULTILINE)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="Pinned UCD version, such as 17.0.0")
    parser.add_argument("--ucd-dir", type=Path, help="Extracted UCD root; defaults to .cache/unicode/VERSION/ucd")
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).resolve().parents[1] / "src" / "unicode")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    ucd = (args.ucd_dir or root / ".cache" / "unicode" / args.version / "ucd").resolve()
    validate_version(ucd, args.version)
    generated = {
        "property_data.rs": make_property_data(ucd, args.version),
        "fold_data.rs": make_fold_data(ucd, args.version),
        "egcb_data.rs": make_break_data(ucd / "auxiliary" / "GraphemeBreakProperty.txt", args.version, "egcb"),
        "wb_data.rs": make_break_data(ucd / "auxiliary" / "WordBreakProperty.txt", args.version, "wb"),
    }
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for filename, content in generated.items():
        destination = args.output_dir / filename
        destination.write_text(content, encoding="utf-8")
    subprocess.run(
        ["rustfmt", "--edition", "2024", *(str(args.output_dir / name) for name in generated)],
        check=True,
    )
    for filename in generated:
        destination = args.output_dir / filename
        content = destination.read_text(encoding="utf-8")
        print(f"Wrote {destination} ({content.count(chr(10)) + 1} lines)")
    print(f"Generated Unicode {args.version} tables from {ucd}")
    print(f"Unicode property names: {len(extract_property_names(generated['property_data.rs']))}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
