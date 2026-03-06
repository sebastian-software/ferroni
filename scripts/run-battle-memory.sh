#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

extract_bench_path() {
  local bench_name="$1"
  local cargo_output="$2"

  local path
  path="$(printf '%s\n' "${cargo_output}" | sed -n "s|^  Executable benches/${bench_name}\\.rs (\\(target/release/deps/${bench_name}-[^)]*\\))$|\\1|p" | tail -n 1)"
  if [[ -z "${path}" ]]; then
    echo "Could not resolve executable path for ${bench_name}" >&2
    exit 1
  fi
  printf '%s\n' "${ROOT_DIR}/${path}"
}

run_and_prefix() {
  local label="$1"
  local bin_path="$2"

  "${bin_path}" | while IFS= read -r line; do
    printf '[%s] %s\n' "${label}" "${line}"
  done
}

echo "Preparing Oniguruma sources..."
./scripts/prepare-oniguruma-sources.sh >/dev/null

echo "Building Rust memory harness..."
rust_build_output="$(cargo bench --no-run --bench battle_mem_rust 2>&1)"
rust_bin="$(extract_bench_path battle_mem_rust "${rust_build_output}")"

echo "Building Oniguruma memory harness..."
onig_build_output="$(cargo bench --no-run --features ffi --bench battle_mem_onig 2>&1)"
onig_bin="$(extract_bench_path battle_mem_onig "${onig_build_output}")"

echo
echo "Running Rust memory harness..."
rust_output="$(run_and_prefix rust "${rust_bin}")"
printf '%s\n' "${rust_output}"

echo
echo "Running Oniguruma memory harness..."
onig_output="$(run_and_prefix oniguruma "${onig_bin}")"
printf '%s\n' "${onig_output}"
