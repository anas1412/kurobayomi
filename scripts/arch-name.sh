#!/usr/bin/env bash
# Prints the architecture name used in tarball and bundle names: x86_64 or aarch64.
# The 32-bit targets of the Android splits (armeabi-v7a, x86) have no build target and are rejected.
#
# Usage: scripts/arch-name.sh [machine]        (default: uname -m)
#        scripts/arch-name.sh --tarball <version> [machine]    ->  kurobayomi-<version>-<arch>.tar.gz
set -euo pipefail

arch_of() {
    case "$1" in
        x86_64 | amd64) echo x86_64 ;;
        aarch64 | arm64) echo aarch64 ;;
        *) echo "unsupported architecture: $1 (supported: x86_64, aarch64)" >&2; return 1 ;;
    esac
}

if [ "${1:-}" = "--tarball" ]; then
    [ -n "${2:-}" ] || { echo "usage: $0 --tarball <version> [machine]" >&2; exit 2; }
    arch="$(arch_of "${3:-$(uname -m)}")"
    echo "kurobayomi-$2-$arch.tar.gz"
else
    arch_of "${1:-$(uname -m)}"
fi
