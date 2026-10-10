#!/usr/bin/env bash
# TST-54-15: scripts/bench-startup.sh runs with one run against a stub binary that exits 0.
# A fake hyperfine on PATH stands in for the real one, so the test needs no extra package.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

mkdir "$tmp/bin"
cat >"$tmp/bin/hyperfine" <<'FAKE'
#!/usr/bin/env bash
# Minimal hyperfine: runs the last argument once through the shell and writes the --export-json file.
out=""; prev=""
for a in "$@"; do [ "$prev" = "--export-json" ] && out="$a"; prev="$a"; done
echo "$*" >>"$FAKE_LOG"
eval "${@: -1}"
[ -z "$out" ] || echo '{"results":[]}' >"$out"
FAKE
chmod +x "$tmp/bin/hyperfine"

export PATH="$tmp/bin:$PATH" FAKE_LOG="$tmp/hyperfine.log" STUB_MARKER="$tmp/marker" BENCH_OUT="$tmp/out/bench.json"
fail() { echo "FAIL: $*" >&2; exit 1; }

"$here/../bench-startup.sh" "$here/stub-binary.sh" 1 >/dev/null
[ -s "$BENCH_OUT" ] || fail "no JSON written"
grep -q 'KB_TRACE_STARTUP=1 args=--exit-after-first-frame' "$STUB_MARKER" || fail "stub not run with the trace flag"
grep -q -- '--warmup 1' "$FAKE_LOG" || fail "warm mode should use one warm-up run"

"$here/../bench-startup.sh" --mode hot "$here/stub-binary.sh" 2 >/dev/null
grep -q -- '--warmup 3' "$FAKE_LOG" || fail "hot mode should use three warm-up runs"

"$here/../bench-startup.sh" --mode cold "$here/stub-binary.sh" 1 >/dev/null
grep -q 'drop_caches' "$FAKE_LOG" || fail "cold mode should drop the page cache"

if "$here/../bench-startup.sh" "$tmp/missing" 1 2>/dev/null; then fail "missing binary accepted"; fi
if "$here/../bench-startup.sh" --mode bogus "$here/stub-binary.sh" 2>/dev/null; then fail "bad mode accepted"; fi
if "$here/../bench-startup.sh" "$here/stub-binary.sh" 0 2>/dev/null; then fail "zero runs accepted"; fi

echo "OK: bench-startup smoke test"
