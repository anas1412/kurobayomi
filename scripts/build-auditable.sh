#!/usr/bin/env bash
# Release build with the dependency list embedded in the binary (cargo-auditable).
# Replaces the AGP dependency-info flag. Read the list back with `cargo audit bin <binary>`.
#
# Usage: scripts/build-auditable.sh [cargo build arguments]
# Default arguments: --profile dist --locked. Check mode: scripts/build-auditable.sh --check <binary>
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ "${1:-}" = "--check" ]; then
    [ -f "${2:-}" ] || { echo "usage: $0 --check <binary>" >&2; exit 2; }
    command -v cargo-audit >/dev/null || { echo "cargo-audit is not installed" >&2; exit 127; }
    exec cargo audit bin "$2"
fi

command -v cargo-auditable >/dev/null || { echo "cargo-auditable is not installed (cargo install cargo-auditable --locked)" >&2; exit 127; }
if [ "$#" -eq 0 ]; then set -- --profile dist --locked; fi
exec cargo auditable build "$@"
