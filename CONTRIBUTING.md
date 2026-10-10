# Contributing to Kurobayomi

## Crate layout

- `crates/kuro-core`: UI-free application logic. It depends on `kuro-platform` and must not depend on `gtk4`.
- `crates/kuro-platform`: the only place for OS-specific code (directories, secrets, disk space, detached processes).
- `crates/kuro-i18n`: localised strings.
- `crates/kuro-testkit`: test helpers (temporary directories, fixtures). Not a production dependency.
- The root package `kurobayomi` holds the GTK UI.

## Platform rule

OS-specific code lives only in `crates/kuro-platform`. That means `cfg(target_os)`, `xdg-open`, `libsecret`, `zbus` and `XDG_` strings.
Outside that crate:

- Ask `Paths` for a directory role (for example `paths.crash_dir()`). Never join a string literal onto a `Paths` result.
- `kuro-core` must not use `cfg(target_os)`.

Before you push, run:

```sh
scripts/check-platform-boundary.sh
scripts/check-path-literals.sh
cargo test --workspace
```

CI runs the same three commands on every push and pull request (`.github/workflows/ci.yml`).

## Building

The GTK UI needs GTK 4.16 or newer development files (`gtk4-devel` on Fedora, `libgtk-4-dev` on Debian and Ubuntu).
Older releases ship GTK below 4.16 (Ubuntu 24.04 ships 4.14) and fail on the `v4_16` feature. Check with `pkg-config --modversion gtk4`.
Rust is the stable toolchain from `rust-toolchain.toml`.

## Tests, benchmarks and release builds

- `scripts/test-workspace.sh` runs the whole workspace against the locked dependencies; `scripts/coverage.sh` is the
  optional coverage job (see `docs/testing.md`).
- `scripts/bench-startup.sh <binary> [runs]` times startup (see `docs/benchmarking.md`).
- Release builds use `scripts/build-auditable.sh`; the licence page comes from `scripts/third-party.sh` (see
  `docs/release-build.md`).
- Issue forms and the pull request template are kept in `packaging/github/` and copied to `.github/`.
  `scripts/check-github-templates.sh` checks them.

## Decisions recorded here

- Funding links: the upstream funding file is not carried over, and Kurobayomi has none. A maintainer who wants
  sponsor links adds their own file and records that here.
- Dropped Android build steps are listed with their user impact in `docs/drop-decisions.md`.
