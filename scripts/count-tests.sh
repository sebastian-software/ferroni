#!/usr/bin/env bash
#
# Count the `#[test]` functions in the tree.
#
# This is the single source of truth for the test counts quoted in README.md
# ("Correctness and safety") and CONTRIBUTING.md. The README quotes the grand
# total rounded down to the hundred, so routine additions need no README edit;
# scripts/check-readme-figures.sh (run in CI) fails when the rounded figure moves.
#
# Note: the parity table in docs/app/routes/guide/compatibility.mdx counts
# *upstream C test cases*, which is a different metric -- some compat
# functions bundle several upstream cases.

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
