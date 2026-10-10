#!/usr/bin/env bash
# Optional coverage job: line coverage of the build tooling must stay at or above 80% (area 54 target).
# Needs cargo-llvm-cov (`cargo install cargo-llvm-cov --locked`) and the llvm-tools-preview component.
#
#   scripts/coverage.sh                      # xtask only
#   COVERAGE_PACKAGES="xtask kurobayomi" scripts/coverage.sh
#
# COVERAGE_MIN overrides the threshold. Writes target/llvm-cov/lcov.info for upload.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
command -v cargo-llvm-cov >/dev/null || { echo "cargo-llvm-cov is not installed" >&2; exit 127; }

min="${COVERAGE_MIN:-80}"
pkgs=()
for p in ${COVERAGE_PACKAGES:-xtask}; do pkgs+=(-p "$p"); done

cargo llvm-cov --locked --no-report "${pkgs[@]}"
mkdir -p target/llvm-cov
cargo llvm-cov report --lcov --output-path target/llvm-cov/lcov.info
cargo llvm-cov report --fail-under-lines "$min"
