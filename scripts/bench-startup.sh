#!/usr/bin/env bash
# Startup timing with hyperfine. Replaces the Android startup benchmark.
#
# Usage: scripts/bench-startup.sh [--mode cold|warm|hot] <binary> [runs]
#
#   warm (default)  one warm-up run, then <runs> timed runs. The page cache is primed by the warm-up run.
#   hot             three warm-up runs first: caches, the database files and the settings file are all in place.
#   cold            drops the page cache before every timed run. Needs root: the script runs
#                   `sync; echo 3 > /proc/sys/vm/drop_caches` through sudo, so sudo asks for a password once.
#
# The app writes `main_entry`, `window_presented` and `first_library_render` timestamps to stderr when
# KB_TRACE_STARTUP=1 and quits after the first library frame with --exit-after-first-frame. Result: JSON in
# $BENCH_OUT (default target/bench-startup.json). Compilation modes of the Android benchmark do not exist for a
# native binary and are dropped. The binary is a plain argument, so there is no package name to get wrong.
set -euo pipefail

usage() {
    echo "usage: $0 [--mode cold|warm|hot] <binary> [runs]" >&2
    exit 2
}

mode=warm
if [ "${1:-}" = "--mode" ]; then
    mode="${2:-}"
    shift 2 || usage
fi
bin="${1:-}"
[ -n "$bin" ] || usage
runs="${2:-10}"
case "$runs" in '' | *[!0-9]* | 0) echo "runs must be a positive integer, got '$runs'" >&2; exit 2 ;; esac
[ -x "$bin" ] || { echo "binary not found or not executable: $bin" >&2; exit 2; }
command -v hyperfine >/dev/null || { echo "hyperfine is not installed (system package 'hyperfine')" >&2; exit 127; }

out="${BENCH_OUT:-target/bench-startup.json}"
mkdir -p "$(dirname "$out")"

args=(--runs "$runs" --export-json "$out")
case "$mode" in
    warm) args+=(--warmup 1) ;;
    hot) args+=(--warmup 3) ;;
    cold) args+=(--warmup 0 --prepare 'sync; echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null') ;;
    *) usage ;;
esac

printf -v cmd 'env KB_TRACE_STARTUP=1 %q --exit-after-first-frame' "$bin"
hyperfine "${args[@]}" "$cmd"
echo "wrote $out (mode: $mode)"
