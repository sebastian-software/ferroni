#!/usr/bin/env bash
#
# Measures line coverage and enforces the repository's coverage gate.
#
# This script is the single source of truth for the threshold and for the exact
# cargo-llvm-cov invocations. The `coverage` job in .github/workflows/ci.yml
# runs it unchanged, so a local run reproduces CI exactly.
#
# Requires the nightly toolchain (cargo-llvm-cov needs the nightly `coverage`
# attribute), cargo-llvm-cov, and python3.

set -euo pipefail

# The gate. Raising it is a normal change; lowering it needs a reason in the
# pull request. This assignment is the only definition of the threshold: the CI
# workflow runs this script instead of spelling the number out again, and the
# README badge only quotes it.
COVERAGE_MIN_LINES=87

# Files excluded from the measurement:
#   src/ffi.rs                FFI bindings for benchmark comparison only
#   src/unicode/*_data.rs     generated Unicode data tables
#   src/unicode/mod.rs        internal C-port of the case-fold logic, covered
#                             indirectly through the compat suites
#   tests/                    the test files themselves
IGNORE_FILENAME_REGEX='(^|/)(src/ffi\.rs|src/unicode/(egcb_data|wb_data|property_data|fold_data|mod)\.rs|tests/)'

cd -- "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

# rust-toolchain.toml pins stable for everything else; coverage needs nightly.
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-nightly}"

echo "==> Coverage (library unit tests)"
cargo llvm-cov --no-report --release --lib

echo "==> Coverage (API + non-stack-heavy suites)"
cargo llvm-cov --release --no-clean \
  --test api_test \
  --test compat_syntax \
  --test compat_options \
  --test compat_regset

# Coverage instrumentation adds stack frames, so compat_utf8 needs the release
# value ADR-013 documents, and the deepest recursive and backreference tests
# still overflow it. Those ~42 tests are skipped here only; the regular test
# lanes run them unskipped.
echo "==> Coverage (compat_utf8, skipping stack-heavy tests)"
RUST_MIN_STACK=268435456 \
  cargo llvm-cov --release --no-clean \
  --test compat_utf8 \
  -- --test-threads=4 \
  --skip recursive_ \
  --skip backref_nested_no_match \
  --skip backref_self_no_match \
  --skip named_group_underscore_backref \
  --skip conditional_recursion \
  --skip lookbehind_backref_circular

# Read the total first so the number reaches the log and the run summary even
# when the gate below fails.
totals=$(cargo llvm-cov report --release \
  --ignore-filename-regex "$IGNORE_FILENAME_REGEX" \
  --json --summary-only)

measured=$(printf '%s' "$totals" | python3 -c \
  'import json, sys; print("%.2f" % json.load(sys.stdin)["data"][0]["totals"]["lines"]["percent"])')

summary="Line coverage: ${measured}% (gate: ≥ ${COVERAGE_MIN_LINES}%)"
echo "$summary"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  echo "$summary" >>"$GITHUB_STEP_SUMMARY"
fi

echo "==> Coverage report and gate"
cargo llvm-cov report --release \
  --ignore-filename-regex "$IGNORE_FILENAME_REGEX" \
  --fail-under-lines "$COVERAGE_MIN_LINES"
