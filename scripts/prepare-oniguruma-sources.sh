#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
METADATA_FILE="${ROOT_DIR}/benches/battle_inputs.toml"

read_toml_value() {
  local section="$1"
  local key="$2"

  awk -v section="$section" -v key="$key" '
    function trim(value) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
      gsub(/^"/, "", value)
      gsub(/"$/, "", value)
      return value
    }

    $0 ~ "^[[:space:]]*\\[" section "\\][[:space:]]*$" {
      in_section = 1
      next
    }

    in_section && $0 ~ "^[[:space:]]*\\[" {
      in_section = 0
    }

    in_section {
      split($0, parts, "=")
      current_key = trim(parts[1])
      if (current_key == key) {
        value = substr($0, index($0, "=") + 1)
        print trim(value)
        exit
      }
    }
  ' "${METADATA_FILE}"
}

ONIGURUMA_REPO_URL="$(read_toml_value oniguruma repo)"
ONIGURUMA_COMMIT="$(read_toml_value oniguruma commit)"
ONIGURUMA_CACHE_DIR_REL="$(read_toml_value oniguruma local_cache_dir)"

if [[ -z "${ONIGURUMA_REPO_URL}" ]] || [[ -z "${ONIGURUMA_COMMIT}" ]] || [[ -z "${ONIGURUMA_CACHE_DIR_REL}" ]]; then
  echo "Failed to read Oniguruma metadata from ${METADATA_FILE}" >&2
  exit 1
fi

TARGET_DIR="${ROOT_DIR}/${ONIGURUMA_CACHE_DIR_REL}"
CACHE_DIR="$(dirname "${TARGET_DIR}")"
STAMP_FILE="${TARGET_DIR}/.ferroni-upstream-commit"
ARCHIVE_URL="${ONIGURUMA_REPO_URL}/archive/${ONIGURUMA_COMMIT}.tar.gz"

mkdir -p "${CACHE_DIR}"

if [[ -f "${STAMP_FILE}" ]] && [[ "$(cat "${STAMP_FILE}")" == "${ONIGURUMA_COMMIT}" ]]; then
  echo "Oniguruma sources already prepared at ${TARGET_DIR}"
  exit 0
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ferroni-oniguruma.XXXXXX")"
ARCHIVE_PATH="${TMP_DIR}/oniguruma.tar.gz"
EXTRACT_DIR="${TMP_DIR}/extract"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

echo "Using pinned battle input metadata from ${METADATA_FILE}"
echo "Downloading pinned Oniguruma sources..."
curl --fail --location --retry 3 --output "${ARCHIVE_PATH}" "${ARCHIVE_URL}"

mkdir -p "${EXTRACT_DIR}"
tar -xzf "${ARCHIVE_PATH}" -C "${EXTRACT_DIR}"

EXTRACTED_ROOT="$(find "${EXTRACT_DIR}" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
if [[ -z "${EXTRACTED_ROOT}" ]]; then
  echo "Failed to extract Oniguruma archive." >&2
  exit 1
fi

rm -rf "${TARGET_DIR}"
mv "${EXTRACTED_ROOT}" "${TARGET_DIR}"
printf '%s\n' "${ONIGURUMA_COMMIT}" > "${STAMP_FILE}"

echo "Prepared Oniguruma sources at ${TARGET_DIR}"
echo "If needed, override the location via FERRONI_ONIGURUMA_DIR=/path/to/oniguruma"
