#!/usr/bin/env bash
# scripts/arch-name.sh names the two supported architectures and rejects the 32-bit ones.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
s="$here/../arch-name.sh"
fail() { echo "FAIL: $*" >&2; exit 1; }

[ "$("$s" x86_64)" = x86_64 ] || fail "x86_64"
[ "$("$s" amd64)" = x86_64 ] || fail "amd64"
[ "$("$s" aarch64)" = aarch64 ] || fail "aarch64"
[ "$("$s" arm64)" = aarch64 ] || fail "arm64"
for bad in armv7l armeabi-v7a i686 x86; do
    if "$s" "$bad" 2>/dev/null; then fail "$bad accepted"; fi
done
[ "$("$s" --tarball 0.1.0 aarch64)" = kurobayomi-0.1.0-aarch64.tar.gz ] || fail "tarball name"
[ "$("$s" --tarball 0.2.0-rc1 x86_64)" = kurobayomi-0.2.0-rc1-x86_64.tar.gz ] || fail "tarball name rc"
if "$s" --tarball 0.1.0 armv7l 2>/dev/null; then fail "32-bit tarball accepted"; fi
[ "$("$s")" = "$("$s" "$(uname -m)")" ] || fail "default machine"
echo "OK: arch-name test"
