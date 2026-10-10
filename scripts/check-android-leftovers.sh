#!/usr/bin/env bash
# Fails when a script, doc or workflow in the packaging lane still refers to a dropped Android build step
# (T-54a-024, T-54a-025, T-54a-026, T-54a-030, T-54a-036, T-54a-060). Plans and crates are not searched.
# The patterns are assembled from parts so this file does not match itself.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
pattern="baseline-""prof|devBench""mark|setup-""java|setup-""gradle|proguard|buildSrc|androidComponents|version""Code|macro""benchmark"
paths=()
for p in scripts docs dist packaging .github CONTRIBUTING.md; do [ -e "$p" ] && paths+=("$p"); done

# drop-decisions.md records the drops, so it may name most of them, but never the two removed profile names
strict="baseline-""prof|devBench""mark"
status=0
matches=$(rg --line-number --no-heading --color never --hidden -i \
    --glob '!scripts/check-android-leftovers.sh' --glob '!docs/drop-decisions.md' \
    -e "$pattern" "${paths[@]}") || status=$?
if [ "$status" -eq 1 ]; then
    status=0
    matches=$(rg --line-number --no-heading --color never --hidden -i \
        --glob '!scripts/check-android-leftovers.sh' -e "$strict" "${paths[@]}") || status=$?
fi
case "$status" in
    0) echo "$matches"; echo "FAIL: dropped Android build steps still referenced" >&2; exit 1 ;;
    1) echo "OK: no dropped Android build steps referenced" ;;
    *) echo "ERROR: rg failed with exit status $status" >&2; exit "$status" ;;
esac
