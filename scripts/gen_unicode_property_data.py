#!/usr/bin/env python3
"""
Generate src/unicode/property_data.rs from oniguruma-orig/src/unicode_property_data.c

Extracts:
1. CR_* code range arrays (strip count prefix, emit start/end pairs)
2. CodeRanges[] index mapping (ctype index -> CR_* name)
3. gperf wordlist (property name -> ctype index)

Outputs a single Rust file with static data.
"""

import re
import sys
import os

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT_DIR = os.path.dirname(SCRIPT_DIR)
C_FILE = os.path.join(ROOT_DIR, "oniguruma-orig", "src", "unicode_property_data.c")
OUT_FILE = os.path.join(ROOT_DIR, "src", "unicode", "property_data.rs")


def parse_cr_aliases(text):
    """Parse #define CR_Foo CR_Bar aliases."""
    aliases = {}
    for m in re.finditer(r'#define\s+(CR_\w+)\s+(CR_\w+)', text):
        aliases[m.group(1)] = m.group(2)
    return aliases


def parse_cr_arrays(text):
    """Parse all CR_* arrays, returning dict of name -> list of (start, end) pairs."""
    arrays = {}
    # Match: CR_Name[] = { count, hex, hex, ... };
    pattern = re.compile(
        r'static const OnigCodePoint\s*\n\s*(CR_\w+)\[\]\s*=\s*\{\s*(\d+),\s*\n(.*?)\};\s*/\*\s*END of',
        re.DOTALL
    )
    for m in pattern.finditer(text):
        name = m.group(1)
        count = int(m.group(2))
        body = m.group(3)
        # Extract hex values
        vals = re.findall(r'0x([0-9a-fA-F]+)', body)
        vals = [int(v, 16) for v in vals]
        assert len(vals) == count * 2, f"{name}: expected {count*2} values, got {len(vals)}"
        arrays[name] = vals

    # Resolve aliases into the arrays dict so lookups work
    aliases = parse_cr_aliases(text)
    for alias, target in aliases.items():
        if alias not in arrays and target in arrays:
            arrays[alias] = arrays[target]

    return arrays, aliases


def parse_code_ranges(text):
    """Parse CodeRanges[] array, returning ordered list of CR_* names."""
    m = re.search(r'const CodeRanges\[\]\s*=\s*\{(.*?)\};', text, re.DOTALL)
    if not m:
        raise ValueError("Could not find CodeRanges[] array")
    body = m.group(1)
    names = re.findall(r'(CR_\w+)', body)
    return names


def parse_pool_strings(text):
    """Parse the unicode_prop_name_pool_t struct to get member number -> string."""
    pool = {}
    pattern = re.compile(
        r'char\s+unicode_prop_name_pool_str(\d+)\[sizeof\("([^"]+)"\)\]'
    )
    for m in pattern.finditer(text):
        idx = int(m.group(1))
        s = m.group(2)
        pool[idx] = s
    return pool


def parse_wordlist(text, pool):
    """Parse the gperf wordlist[] to get (name, ctype_index) pairs."""
    # Find the wordlist
    m = re.search(r'static const struct PoolPropertyNameCtype wordlist\[\]\s*=\s*\{(.*?)\};',
                  text, re.DOTALL)
    if not m:
        raise ValueError("Could not find wordlist[]")
    body = m.group(1)

    entries = []
    # Match {pool_offset(N), ctype}
    for m2 in re.finditer(r'\{pool_offset\((\d+)\),\s*(\d+)\}', body):
        pool_idx = int(m2.group(1))
        ctype = int(m2.group(2))
        if pool_idx not in pool:
            print(f"WARNING: pool_offset({pool_idx}) not found in pool struct", file=sys.stderr)
            continue
        name = pool[pool_idx]
        entries.append((name, ctype))

    return entries


def cr_name_to_rust(name):
    """Convert CR_Foo_Bar to CR_FOO_BAR for Rust const naming."""
    # Keep original case for readability but ensure valid Rust ident
    return name


def generate_rust(arrays, aliases, code_ranges, wordlist):
    """Generate the Rust source file."""
    lines = []
    lines.append("//! Auto-generated Unicode property data. Do not edit.")
    lines.append("//! Generated from oniguruma-orig/src/unicode_property_data.c")
    lines.append("//! by scripts/gen_unicode_property_data.py")
    lines.append("")
    lines.append("#![allow(dead_code, non_upper_case_globals)]")
    lines.append("")
    lines.append(f"pub const CODE_RANGES_NUM: usize = {len(code_ranges)};")
    lines.append("")

    # Emit CR_* arrays (without the count prefix)
    lines.append("// --- Code Range Arrays ---")
    lines.append("// Each array contains pairs of (start, end) code points.")
    lines.append("")

    # Determine which names need their own arrays vs are aliases
    # An alias just references the target's data in CODE_RANGES
    emitted_arrays = set()
    for name in code_ranges:
        # Resolve to the real array name
        real = aliases.get(name, name)
        if real in emitted_arrays:
            continue
        emitted_arrays.add(real)
        if real not in arrays:
            print(f"WARNING: {real} referenced in CodeRanges but not found", file=sys.stderr)
            continue
        vals = arrays[real]
        n = len(vals)
        lines.append(f"static {real}: [u32; {n}] = [")
        # Format: 6 values per line
        for i in range(0, n, 6):
            chunk = vals[i:i+6]
            parts = [f"0x{v:06x}" for v in chunk]
            lines.append("    " + ", ".join(parts) + ",")
        lines.append("];")
        lines.append("")

    # Emit CODE_RANGES index
    # For aliases, reference the target array
    lines.append("// --- Index: ctype -> code ranges ---")
    lines.append(f"pub static CODE_RANGES: [&[u32]; {len(code_ranges)}] = [")
    for i, name in enumerate(code_ranges):
        real = aliases.get(name, name)
        lines.append(f"    &{real},  // {i}: {name}")
    lines.append("];")
    lines.append("")

    # Emit property name lookup table (sorted)
    # Sort by normalized name for binary search
    sorted_names = sorted(wordlist, key=lambda x: x[0])
    lines.append("// --- Property name lookup table (sorted, normalized) ---")
    lines.append("// Names are already lowercase with spaces/hyphens/underscores removed.")
    lines.append(f"pub static PROPERTY_NAMES: [(&str, u16); {len(sorted_names)}] = [")
    for name, ctype in sorted_names:
        lines.append(f'    ("{name}", {ctype}),')
    lines.append("];")
    lines.append("")

    return "\n".join(lines)


def main():
    with open(C_FILE, 'r') as f:
        text = f.read()

    print(f"Parsing {C_FILE}...")
    arrays, aliases = parse_cr_arrays(text)
    print(f"  Found {len(arrays)} CR_* arrays ({len(aliases)} aliases)")

    code_ranges = parse_code_ranges(text)
    print(f"  Found {len(code_ranges)} entries in CodeRanges[]")

    pool = parse_pool_strings(text)
    print(f"  Found {len(pool)} pool strings")

    wordlist = parse_wordlist(text, pool)
    print(f"  Found {len(wordlist)} wordlist entries")

    # Verify
    for name in code_ranges:
        real = aliases.get(name, name)
        if real not in arrays:
            print(f"  ERROR: {name} (-> {real}) in CodeRanges but not parsed!")

    rust = generate_rust(arrays, aliases, code_ranges, wordlist)

    with open(OUT_FILE, 'w') as f:
        f.write(rust)

    line_count = rust.count('\n') + 1
    print(f"  Written {OUT_FILE} ({line_count} lines)")


if __name__ == '__main__':
    main()
