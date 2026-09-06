#!/usr/bin/env bash
#
# Count the `#[test]` functions in the tree.
#
# This is the single source of truth for the test counts quoted in README.md
# ("Test parity") and CONTRIBUTING.md. Run it after adding or removing tests
# and update the README number if it changed.
#
# Note: the parity table in README.md counts *upstream C test cases*, which is
# a different metric -- some compat functions bundle several upstream cases.

set -euo pipefail

cd "$(dirname "$0")/.."

count() {
  grep -rc --include='*.rs' -E '^[[:space:]]*#\[test\]' "$1" 2>/dev/null |
    awk -F: '{ sum += $NF } END { print sum + 0 }'
}

printf 'Integration tests (tests/):\n'
for file in tests/*.rs; do
  printf '  %-28s %6d\n' "$(basename "$file")" "$(count "$file")"
done

integration=$(count tests)
unit=$(count src)

printf '\n  %-28s %6d\n' 'tests/ total' "$integration"
printf '  %-28s %6d\n' 'src/ unit tests' "$unit"
printf '  %-28s %6d\n' 'grand total' "$((integration + unit))"
