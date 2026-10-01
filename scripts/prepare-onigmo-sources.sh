#!/usr/bin/env bash
# Fetch the pinned Onigmo sources (Ruby's copy) for the `onigmo` benchmark
# feature. See the [onigmo] section of benches/battle_inputs.toml.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
METADATA_FILE="${ROOT_DIR}/benches/battle_inputs.toml"

read_value() {
  awk -v key="$1" '
    /^\[onigmo\]/ { in_section = 1; next }
    in_section && /^\[/ { in_section = 0 }
    in_section && $1 == key {
      value = substr($0, index($0, "=") + 1)
      gsub(/^[[:space:]]*"|"[[:space:]]*$/, "", value)
      print value
      exit
    }
  ' "${METADATA_FILE}"
}

REPO="$(read_value repo)"
COMMIT="$(read_value commit)"
ST_REPO="$(read_value st_repo)"
ST_COMMIT="$(read_value st_commit)"
TARGET_DIR="${ROOT_DIR}/$(read_value local_cache_dir)"
STAMP_FILE="${TARGET_DIR}/.ferroni-upstream-commit"
STAMP="${COMMIT} ${ST_COMMIT}"

if [[ -z "${REPO}" || -z "${COMMIT}" || -z "${ST_REPO}" || -z "${ST_COMMIT}" ]]; then
  echo "Failed to read the [onigmo] metadata from ${METADATA_FILE}" >&2
  exit 1
fi

if [[ -f "${STAMP_FILE}" ]] && [[ "$(cat "${STAMP_FILE}")" == "${STAMP}" ]]; then
  echo "Onigmo sources already prepared at ${TARGET_DIR}"
  exit 0
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ferroni-onigmo.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "Downloading Ruby ${COMMIT} for its Onigmo sources..."
curl --fail --silent --show-error --location --retry 3 --output "${TMP_DIR}/ruby.tar.gz" "${REPO}/archive/${COMMIT}.tar.gz"
mkdir -p "${TMP_DIR}/extract"
tar -xzf "${TMP_DIR}/ruby.tar.gz" -C "${TMP_DIR}/extract"
SOURCE="$(find "${TMP_DIR}/extract" -mindepth 1 -maxdepth 1 -type d | head -n 1)"

STAGE="${TMP_DIR}/onigmo"
mkdir -p "${STAGE}/enc" "${STAGE}/include/ruby"
cp "${SOURCE}"/reg*.c "${SOURCE}"/reg*.h "${STAGE}/"
cp "${SOURCE}/include/ruby/onigmo.h" "${STAGE}/include/ruby/"
cp "${SOURCE}"/enc/{unicode,utf_8,ascii,us_ascii}.c "${STAGE}/enc/"
cp -R "${SOURCE}/enc/unicode" "${STAGE}/enc/"
cp "${SOURCE}/BSDL" "${STAGE}/BSDL"

for file in st.c st.h; do
  curl --fail --silent --show-error --location --retry 3 --output "${STAGE}/${file}" \
    "${ST_REPO/github.com/raw.githubusercontent.com}/${ST_COMMIT}/${file}"
done

rm -rf "${TARGET_DIR}"
mkdir -p "$(dirname "${TARGET_DIR}")"
mv "${STAGE}" "${TARGET_DIR}"
printf '%s\n' "${STAMP}" > "${STAMP_FILE}"
echo "Prepared Onigmo sources at ${TARGET_DIR}"
