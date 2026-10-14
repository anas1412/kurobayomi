#!/usr/bin/env bash
# Prints the CHANGELOG.md section of one version: the notes of its GitHub release.
# Usage: scripts/release-notes.sh 0.1.0        (fails when the changelog has no `## [0.1.0]` section or it is empty)
set -euo pipefail
version="${1:?usage: release-notes.sh VERSION}"
changelog="$(dirname "${BASH_SOURCE[0]}")/../CHANGELOG.md"
notes="$(awk -v v="$version" '
    /^## \[/ { if (found) exit; if ($0 ~ "^## \\[" v "\\]") { found = 1; next } }
    found { print }
' "$changelog")"
[ -n "$(printf '%s' "$notes" | tr -d '[:space:]')" ] || { echo "CHANGELOG.md has no notes for $version" >&2; exit 1; }
printf '%s\n' "$notes"
