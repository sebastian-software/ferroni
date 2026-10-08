#!/usr/bin/env bash
#
# Count the `unsafe` sites in the library source and the line total they are
# measured against.
#
# This is the single source of truth for the unsafe figures in "Current State"
# of docs/app/routes/adr/002-unsafe-code-policy.mdx. Run it after adding or
# removing unsafe code and update the ADR if the numbers changed. The README
# quotes the site count too; scripts/check-readme-figures.sh (run in CI) fails
# when the README or the ADR falls behind.
#
# Counted, per file under src/:
#   blocks      `unsafe {` blocks (including `unsafe {` after `=`, `(`, etc.)
#   unsafe fn   `unsafe fn` items
#   unsafe impl `unsafe impl` items
#   other       `unsafe extern` and `unsafe trait` items
# Lines whose first non-blank characters are `//` are skipped, so comments that
# mention `unsafe` are not counted. Occurrences after code on the same line are
# counted.
#
# Excluded from the unsafe count and from the line total, and printed separately
# for transparency:
#   src/ffi.rs              FFI bindings for the benchmark-only `ffi` feature
#   src/unicode/*_data.rs   generated Unicode tables (scripts/gen_unicode_tables.py)
#
# The line total is the physical line count (`wc -l` semantics) of the counted
# files. Only find, sort, and awk are used, and the awk programs stay within
# POSIX syntax, so the script runs the same on macOS (BSD) and GNU systems.

set -euo pipefail

cd "$(dirname "$0")/.."

is_excluded_ffi() {
  [[ "$1" == "src/ffi.rs" ]]
}

is_generated() {
  case "$1" in
    src/unicode/property_data.rs | src/unicode/fold_data.rs | \
      src/unicode/egcb_data.rs | src/unicode/wb_data.rs) return 0 ;;
    *) return 1 ;;
  esac
}

# Prints "blocks fns impls others lines" for one file.
measure() {
  awk '
    /^[ \t]*\/\// { next }
    {
      blocks += gsub(/unsafe[ \t]*[{]/, "&")
      fns    += gsub(/unsafe[ \t]+fn[^A-Za-z0-9_]/, "&")
      impls  += gsub(/unsafe[ \t]+impl[^A-Za-z0-9_]/, "&")
      others += gsub(/unsafe[ \t]+(extern|trait)[^A-Za-z0-9_]/, "&")
    }
    END { print blocks + 0, fns + 0, impls + 0, others + 0, NR + 0 }
  ' "$1"
}

printf 'Unsafe sites in src/ (ffi.rs and generated Unicode tables excluded):\n\n'
printf '  %-36s %8s %10s %12s %8s %8s\n' 'file' 'blocks' 'unsafe fn' 'unsafe impl' 'other' 'lines'

sites_total=0
lines_total=0
blocks_total=0
fns_total=0
impls_total=0
others_total=0
ffi_sites=0
ffi_lines=0
generated_lines=0

while IFS= read -r file; do
  read -r blocks fns impls others lines <<<"$(measure "$file")"
  sites=$((blocks + fns + impls + others))

  if is_excluded_ffi "$file"; then
    ffi_sites=$sites
    ffi_lines=$lines
    continue
  fi
  if is_generated "$file"; then
    generated_lines=$((generated_lines + lines))
    continue
  fi

  printf '  %-36s %8d %10d %12d %8d %8d\n' "$file" "$blocks" "$fns" "$impls" "$others" "$lines"
  sites_total=$((sites_total + sites))
  lines_total=$((lines_total + lines))
  blocks_total=$((blocks_total + blocks))
  fns_total=$((fns_total + fns))
  impls_total=$((impls_total + impls))
  others_total=$((others_total + others))
done < <(find src -name '*.rs' -type f | sort)

printf '\n  %-36s %8d %10d %12d %8d %8d\n' 'total' \
  "$blocks_total" "$fns_total" "$impls_total" "$others_total" "$lines_total"

share=$(awk -v s="$sites_total" -v l="$lines_total" 'BEGIN { printf "%.2f", (l > 0 ? 100 * s / l : 0) }')

printf '\nunsafe sites (counted)      %8d\n' "$sites_total"
printf 'lines (denominator)         %8d\n' "$lines_total"
printf 'share of lines              %7s%%\n' "$share"

printf '\nExcluded from the figures above:\n'
printf '  %-36s %8d sites %8d lines (benchmark-only ffi feature)\n' 'src/ffi.rs' "$ffi_sites" "$ffi_lines"
printf '  %-36s %8s %8d lines (generated)\n' 'src/unicode/*_data.rs' '' "$generated_lines"
