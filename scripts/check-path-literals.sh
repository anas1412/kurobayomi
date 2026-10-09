#!/usr/bin/env bash
# Fails when a directory is built from a string literal on a Paths accessor outside crates/kuro-platform,
# for example `paths.cache_dir().join("crash")`. Ask Paths for the role instead.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

pattern='_dir\(\)\s*\.join\('
status=0
matches=$(rg --line-number --no-heading --color never --hidden \
    --glob '!.git/**' \
    --glob '*.rs' \
    --glob '!crates/kuro-platform/**' \
    --glob '!target/**' \
    -e "$pattern" .) || status=$?

case "$status" in
    0)
        echo "$matches"
        echo "FAIL: directory path built from a string literal outside crates/kuro-platform (see lines above)" >&2
        exit 1
        ;;
    1)
        echo "OK: no literal directory joins outside crates/kuro-platform"
        ;;
    *)
        echo "ERROR: rg failed with exit status $status" >&2
        exit "$status"
        ;;
esac
