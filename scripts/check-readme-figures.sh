#!/usr/bin/env bash
#
# Check that the figures quoted in README.md.src and in the "Current State" of
# ADR-002 match the counts derived from the tree.
#
# - The `#[test]` total is quoted rounded down to the nearest hundred, as
#   "at least N `#[test]` functions": a total of 2,526 is quoted as "at least
#   2,500". Routine test additions leave it alone; it moves only when the
#   rounded total does.
# - The unsafe site count is quoted exactly, from count-unsafe.sh: "N sites"
#   in README.md.src and "N `unsafe` sites" in ADR-002. A change in that
#   number is meant to be a conscious edit.
#
# A failure names the file, the phrase expected and the line found, so the
# sentence to edit is clear. CI runs this script in the README themes job.

set -euo pipefail

cd "$(dirname "$0")/.."

readme=README.md.src
adr=docs/app/routes/adr/002-unsafe-code-policy.mdx

# Thousands separators as the README writes them: 2500 -> 2,500.
with_commas() {
  local value=$1 groups="" chunk
  while ((value >= 1000)); do
    printf -v chunk '%03d' $((value % 1000))
    groups=",${chunk}${groups}"
    value=$((value / 1000))
  done
  printf '%d%s' "$value" "$groups"
}

tests_report=$(./scripts/count-tests.sh)
unsafe_report=$(./scripts/count-unsafe.sh)

tests=$(awk '$1 == "grand" && $2 == "total" { print $NF }' <<<"$tests_report")
sites=$(awk '/^unsafe sites \(counted\)/ { print $NF }' <<<"$unsafe_report")

if [[ ! "$tests" =~ ^[0-9]+$ || ! "$sites" =~ ^[0-9]+$ ]]; then
  echo "check-readme-figures: could not read the totals from count-tests.sh and count-unsafe.sh." >&2
  exit 1
fi

rounded=$((tests / 100 * 100))
expected_tests="at least $(with_commas "$rounded") \`#[test]\` functions"

failures=0

# First line of FILE that contains the fixed string ANCHOR, for the report.
locate() {
  awk -v anchor="$2" '
    index($0, anchor) { sub(/^[ \t]+/, ""); print FILENAME ":" FNR ": " $0; found = 1; exit }
    END { if (!found) print FILENAME ": no line contains " anchor }
  ' "$1"
}

# fail FILE ANCHOR WHAT EXPECTED
fail() {
  failures=$((failures + 1))
  echo "check-readme-figures: $1 does not quote $3." >&2
  echo "  expected: $4" >&2
  echo "  line:     $(locate "$1" "$2")" >&2
}

if ! grep -q -F -- "$expected_tests" "$readme"; then
  fail "$readme" '`#[test]` functions' "the test total" \
    "$expected_tests (count-tests.sh reports $tests, rounded down to the hundred)"
fi

if ! grep -q -E "(^|[^0-9,])${sites} sites" "$readme"; then
  fail "$readme" 'remaining `unsafe`' "the unsafe site count" \
    "$sites sites (count-unsafe.sh reports $sites)"
fi

if ! grep -q -F -- "${sites} \`unsafe\` sites in \`src/\`" "$adr"; then
  fail "$adr" '`unsafe` sites in `src/`' "the unsafe site count" \
    "${sites} \`unsafe\` sites in \`src/\` (count-unsafe.sh reports $sites)"
fi

if ((failures > 0)); then
  echo "check-readme-figures: $failures figure(s) out of date. Edit the lines named above." >&2
  exit 1
fi

echo "check-readme-figures: $tests tests (quoted as $expected_tests), $sites unsafe sites: README and ADR-002 match."
