#!/usr/bin/env bash
# Stand-in for the app in the startup benchmark smoke test: records that it ran and exits 0.
[ -z "${STUB_MARKER:-}" ] || echo "KB_TRACE_STARTUP=${KB_TRACE_STARTUP:-} args=$*" >>"$STUB_MARKER"
exit 0
