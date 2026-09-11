#!/usr/bin/env bash
# Compatibility command for the native project README generator.
set -euo pipefail
mode=${1:-check}
case "$mode" in
  check|write) ;;
  *) echo "Usage: $0 [check|write]" >&2; exit 2 ;;
esac
cd "$(dirname "${BASH_SOURCE[0]}")/.."
exec mise run "readme:$mode"
