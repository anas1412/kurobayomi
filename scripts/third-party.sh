#!/usr/bin/env bash
# Generates the third-party licence page with cargo-about. Replaces the aboutLibraries catalogue.
# `cargo xtask dist` calls this; the About dialog's "Licences" entry shows the file.
#
# Usage: scripts/third-party.sh [output]      (default: data/THIRD-PARTY.html)
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
command -v cargo-about >/dev/null || { echo "cargo-about is not installed (cargo install cargo-about --locked)" >&2; exit 127; }

out="${1:-data/THIRD-PARTY.html}"
mkdir -p "$(dirname "$out")"
cargo about generate --locked --config packaging/about/about.toml packaging/about/about.hbs >"$out"
echo "wrote $out"
