#!/usr/bin/env bash
# Renders or verifies the Ferramenta family block in README.md.
#
# The block is generated from the family registry in
# sebastian-software/ferramenta, so a new member, a renamed tool or a moved
# documentation URL reaches this repository on the next run instead of being
# hand-copied out of date. The generator is pinned to a commit so the check is
# reproducible: to pick up a registry change, bump FERRAMENTA_PIN below, run
# `./scripts/readme-family.sh write`, and commit the result.
#
# Requires pnpm and Node >= 22.13 (the generator strips the types off the
# registry source itself when it runs from node_modules).
set -euo pipefail

FERRAMENTA_PIN="5b84af1676b12b4b45661d5be4a42b11275a72da"
FERRAMENTA_SPEC="github:sebastian-software/ferramenta#${FERRAMENTA_PIN}&path:/packages/family"

mode=${1:-check}
case "$mode" in
  check) mode_flag="--check" ;;
  write) mode_flag="--write" ;;
  *)
    echo "Usage: $(basename -- "${BASH_SOURCE[0]}") [check|write]" >&2
    exit 2
    ;;
esac

repository_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# Ferroni ships a single crate, so the README crates.io renders is the root
# README: the full `github` variant is the only block this repository carries.
exec pnpm dlx "$FERRAMENTA_SPEC" \
  --current ferroni \
  --variant github \
  "$mode_flag" "$repository_root/README.md"
