#!/usr/bin/env bash
# Runs every workspace test against the locked dependency set. Replaces the Gradle test-logging setup.
# Cargo prints one line per test name, and the exit status is non-zero when any test fails.
# Extra arguments go to the test binaries, for example: scripts/test-workspace.sh -- --test-threads=1
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
exec cargo test --workspace --locked --no-fail-fast "$@"
