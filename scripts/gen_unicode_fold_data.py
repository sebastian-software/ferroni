#!/usr/bin/env python3
"""
Generate src/unicode/fold_data.rs from oniguruma-orig/src/unicode_fold*.c

Extracts:
1. OnigUnicodeFolds1/2/3 arrays (flat u32 arrays)
2. Fold key lookup tables (codepoint(s) -> index) for binary search
3. Unfold key lookup table (codepoint -> index, fold_len) for binary search
"""

import re
import sys
import os

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT_DIR = os.path.dirname(SCRIPT_DIR)
SRC = os.path.join(ROOT_DIR, "oniguruma-orig", "src")
OUT_FILE = os.path.join(ROOT_DIR, "src", "unicode", "fold_data.rs")


def parse_fold_array(text, name):
    """Parse OnigUnicodeFoldsN[] array, returning list of u32 values.
    Values can be hex (0xNNNN) or plain decimal (count fields like 1, 2)."""
    pattern = re.compile(
        rf'OnigCodePoint\s+{re.escape(name)}\[\]\s*=\s*\{{(.*?)\}};',
        re.DOTALL
    )
    m = pattern.search(text)
    if not m:
        raise ValueError(f"Could not find {name}[] array")
    body = m.group(1)
    # Strip C comments (/* ... */) and #define lines
    body = re.sub(r'/\*.*?\*/', '', body)
    body = re.sub(r'#define\s+\w+\s+\d+', '', body)
    # Match hex values (0xNNNN) and plain decimal integers
    vals = []
    for tok in re.findall(r'0x[0-9a-fA-F]+|\d+', body):
        if tok.startswith('0x'):
            vals.append(int(tok, 16))
        else:
            vals.append(int(tok))
    return vals


def parse_constants(text):
    """Parse #define constants from fold_data.c."""
    consts = {}
    for m in re.finditer(r'#define\s+(\w+)\s+(\d+)', text):
        consts[m.group(1)] = int(m.group(2))
    return consts


def parse_unfold_key(text):
    """Parse ByUnfoldKey entries from unicode_unfold_key.c.
    Returns list of (code, index, fold_len) for non-empty entries."""
    # Find the wordlist array
    m = re.search(r'static\s+const\s+struct\s+ByUnfoldKey\s+wordlist\[\]\s*=\s*\{(.*?)\};',
                  text, re.DOTALL)
    if not m:
        raise ValueError("Could not find ByUnfoldKey wordlist[]")
    body = m.group(1)

    entries = []
    for m2 in re.finditer(r'\{(0x[0-9a-fA-F]+),\s*(-?\d+),\s*(\d+)\}', body):
        code = int(m2.group(1), 16)
        index = int(m2.group(2))
        fold_len = int(m2.group(3))
        if index >= 0:  # skip empty/sentinel entries
            entries.append((code, index, fold_len))

    # Sort by code for binary search
    entries.sort(key=lambda e: e[0])
    return entries


def build_fold_key_from_data(folds, n_codes):
    """Build a sorted lookup table from the fold array itself.
    n_codes: number of source codepoints per entry (1, 2, or 3).
    Returns list of (codepoints_tuple, index) sorted by codepoints."""
    entries = []
    i = 0
    while i < len(folds):
        # Read n_codes source codepoints
        if i + n_codes >= len(folds):
            break
        codes = tuple(folds[i:i + n_codes])
        count = folds[i + n_codes]
        entries.append((codes, i))
        # Skip: n_codes source cps + 1 count + count fold targets
        i += n_codes + 1 + count

    entries.sort(key=lambda e: e[0])
    return entries


def generate_rust(folds1, folds2, folds3, consts, fold1_key, fold2_key, fold3_key, unfold_key):
    """Generate the Rust source file."""
    lines = []
    lines.append("//! Auto-generated Unicode case fold data. Do not edit.")
    lines.append("//! Generated from oniguruma-orig/src/unicode_fold*.c")
    lines.append("//! by scripts/gen_unicode_fold_data.py")
    lines.append("")
    lines.append("#![allow(dead_code)]")
    lines.append("")

    # Constants
    for name in ['FOLDS1_NORMAL_END_INDEX', 'FOLDS1_END_INDEX',
                 'FOLDS2_NORMAL_END_INDEX', 'FOLDS2_END_INDEX',
                 'FOLDS3_NORMAL_END_INDEX', 'FOLDS3_END_INDEX']:
        lines.append(f"pub const {name}: usize = {consts[name]};")
    lines.append("")

    # Fold arrays
    for name, data in [("UNICODE_FOLDS1", folds1),
                        ("UNICODE_FOLDS2", folds2),
                        ("UNICODE_FOLDS3", folds3)]:
        lines.append(f"pub static {name}: [u32; {len(data)}] = [")
        for i in range(0, len(data), 8):
            chunk = data[i:i+8]
            parts = [f"0x{v:06x}" for v in chunk]
            lines.append("    " + ", ".join(parts) + ",")
        lines.append("];")
        lines.append("")

    # Fold1 key: sorted (codepoint, index) for binary search
    lines.append(f"// Fold1 key: codepoint -> index into UNICODE_FOLDS1")
    lines.append(f"pub static FOLD1_KEY: [(u32, u16); {len(fold1_key)}] = [")
    for (codes,), idx in fold1_key:
        lines.append(f"    (0x{codes:06x}, {idx}),")
    lines.append("];")
    lines.append("")

    # Fold2 key: sorted ((cp1, cp2), index) for binary search
    lines.append(f"// Fold2 key: (cp1, cp2) -> index into UNICODE_FOLDS2")
    lines.append(f"pub static FOLD2_KEY: [([u32; 2], u16); {len(fold2_key)}] = [")
    for (cp1, cp2), idx in fold2_key:
        lines.append(f"    ([0x{cp1:06x}, 0x{cp2:06x}], {idx}),")
    lines.append("];")
    lines.append("")

    # Fold3 key: sorted ((cp1, cp2, cp3), index) for binary search
    lines.append(f"// Fold3 key: (cp1, cp2, cp3) -> index into UNICODE_FOLDS3")
    lines.append(f"pub static FOLD3_KEY: [([u32; 3], u16); {len(fold3_key)}] = [")
    for (cp1, cp2, cp3), idx in fold3_key:
        lines.append(f"    ([0x{cp1:06x}, 0x{cp2:06x}, 0x{cp3:06x}], {idx}),")
    lines.append("];")
    lines.append("")

    # Unfold key: sorted (code, index, fold_len) for binary search
    lines.append(f"// Unfold key: codepoint -> (index, fold_len)")
    lines.append(f"pub static UNFOLD_KEY: [(u32, i16, u8); {len(unfold_key)}] = [")
    for code, idx, fl in unfold_key:
        lines.append(f"    (0x{code:06x}, {idx}, {fl}),")
    lines.append("];")
    lines.append("")

    return "\n".join(lines)


def main():
    # Read source files
    with open(os.path.join(SRC, "unicode_fold_data.c"), 'r') as f:
        fold_text = f.read()
    with open(os.path.join(SRC, "unicode_unfold_key.c"), 'r') as f:
        unfold_text = f.read()

    print("Parsing fold data...")
    consts = parse_constants(fold_text)
    folds1 = parse_fold_array(fold_text, "OnigUnicodeFolds1")
    folds2 = parse_fold_array(fold_text, "OnigUnicodeFolds2")
    folds3 = parse_fold_array(fold_text, "OnigUnicodeFolds3")
    print(f"  Folds1: {len(folds1)} values, Folds2: {len(folds2)} values, Folds3: {len(folds3)} values")

    # Build fold key tables from the fold data itself
    fold1_key = build_fold_key_from_data(folds1, 1)
    fold2_key = build_fold_key_from_data(folds2, 2)
    fold3_key = build_fold_key_from_data(folds3, 3)
    print(f"  Fold1 key: {len(fold1_key)} entries")
    print(f"  Fold2 key: {len(fold2_key)} entries")
    print(f"  Fold3 key: {len(fold3_key)} entries")

    # Parse unfold key
    unfold_key = parse_unfold_key(unfold_text)
    print(f"  Unfold key: {len(unfold_key)} entries")

    # Generate Rust
    rust = generate_rust(folds1, folds2, folds3, consts,
                         fold1_key, fold2_key, fold3_key, unfold_key)

    with open(OUT_FILE, 'w') as f:
        f.write(rust)

    line_count = rust.count('\n') + 1
    print(f"  Written {OUT_FILE} ({line_count} lines)")


if __name__ == '__main__':
    main()
