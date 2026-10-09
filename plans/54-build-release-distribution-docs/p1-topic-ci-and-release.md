# Topic: CI, release and distribution (area 54)

Covers the two GitHub Actions workflows in scope (`.github/workflows/build_pull_request.yml`, `.github/workflows/build_push.yml`), their Gradle steps, the release signing and checksum steps, and the Linux replacement. Observed behaviour is in p1-behavior.md sections 11 and 14; this file gives the step-by-step mapping and the new pipeline.

## 1. Observed pipeline, step by step with decisions

| # | Workflow (file:line) | Step | Observed action | Decision for Kurobayomi | Replacement |
|---|---|---|---|---|---|
| 1 | PR `build_pull_request.yml:29-30`; push `build_push.yml:22-23` | Clone | `actions/checkout@11bd7190…` (v4.2.2), default fetch depth 1 | Keep, set `fetch-depth: 0` | checkout pinned to the same SHA; full history so `KB_COMMIT_COUNT` is right |
| 2 | PR `:33-34` | Dependency Review | `actions/dependency-review-action@3b139cfc…` (v4.5.0) | Replace | `cargo deny check` (advisories, licenses, bans, sources) plus `cargo audit` optional |
| 3 | PR `:36-40`; push `:26-30` | Set up JDK 17 temurin | `actions/setup-java@3a4f6e1a…` | Drop | Rust toolchain from `rust-toolchain.toml` via `dtolnay/rust-toolchain` (Proposal; pin SHA at implementation time) |
| 4 | PR `:42-43`; push `:32-33` | Set up gradle | `gradle/actions/setup-gradle@94baf225…` (v4.3.0) | Replace | `Swatinem/rust-cache` (Proposal; pin SHA) |
| 5 | PR `:45-46`; push `:35-36` | `./gradlew spotlessCheck` | Kotlin and XML formatting | Replace | `cargo xtask lint` |
| 6 | PR `:48-49`; push `:38-39` | `./gradlew assembleRelease` (push adds `-Penable-updater`) | APK build | Replace | `cargo build --profile dist --locked` (push job adds `--features updater`; Flatpak never) |
| 7 | PR `:51-52`; push `:41-42` | `./gradlew testReleaseUnitTest` | JUnit tests | Replace | `cargo test --workspace --locked` |
| 8 | PR `:54-58`; push `:44-48` | Upload APK | `upload-artifact` name `arm64-v8a-${sha}` | Replace | upload `dist/kurobayomi-*-x86_64.tar.gz` and the Flatpak bundle, name `kurobayomi-linux-x86_64-${sha}` |
| 9 | PR `:60-63`; push `:50-53` | Upload mapping | R8 mapping directory | Drop | nothing equivalent; the release tarball carries `kurobayomi.debug` split debug info |
| 10 | push `:56-60` | Get tag name | `VERSION_TAG` from `GITHUB_REF` | Keep | same logic; then assert the tag equals `v` + `version` in `Cargo.toml` (new check) |
| 11 | push `:65-73` | Sign APK | `r0adkll/sign-android-release` with 4 secrets, `BUILD_TOOLS_VERSION=35.0.1` | Replace | GPG-sign `SHA256SUMS` with the release key (`gpg --detach-sign --armor`); Flatpak repo signed with `flatpak build-update-repo --gpg-sign` |
| 12 | push `:77-100` | Clean up build artifacts | rename 5 APKs to `aniyomi-<variant>-${VERSION_TAG}.apk`, compute 5 SHA-256 values into `$GITHUB_ENV` | Replace | `cargo xtask dist` writes `dist/SHA256SUMS`; rename to `kurobayomi-<version>-<arch>.tar.gz` |
| 13 | push `:102-127` | Create Release | `softprops/action-gh-release@c95fe148…` (v2.2.1) with body table, `draft: true`, `prerelease: false` | Keep draft | same action; body from the CHANGELOG section for the tag (new) |

## 2. CI pull request workflow (PROPOSAL)

File: `.github/workflows/ci.yml`.

Triggers: `pull_request` (all branches) and `push` to `main`. Path filters: ignore `**.md` and `LICENSE`. Keep `po/**` in the triggers so `i18n-check` runs.

Permissions: `contents: read` at the top level.

Concurrency: `group: ${{ github.workflow }}-${{ github.ref }}`, `cancel-in-progress: true`.

Jobs and order (Proposal):

1. `lint` (ubuntu-24.04): checkout with `fetch-depth: 0`; install system packages (`libgtk-4-dev`, `libmpv-dev`, `gettext`, `pkg-config`); install toolchain; `cargo xtask lint`; `cargo xtask i18n-check`.
2. `test` (ubuntu-24.04, needs `lint`): `cargo test --workspace --locked`.
3. `build` (ubuntu-24.04, needs `lint`): `cargo build --profile dist --locked`; `cargo auditable build` on the release profile to check that the dependency list is embedded (T-54a-055); upload `kurobayomi-dist-${sha}` artifact (binary, 7-day retention).
4. `deny` (ubuntu-24.04): `cargo deny check`.
5. `flatpak` (ubuntu-24.04, needs `build`): `flatpak-builder` with the manifest in `packaging/flatpak/` and `--install-deps-from=flathub`; `cargo xtask flatpak-lint`. Uses the `flatpak/flatpak-github-actions` action (Proposal; pin SHA). Uploads the `.flatpak` bundle.

Retention: GitHub's default artifact retention is 90 days (Proposal: set `retention-days: 7` for PR artifacts, since the Android mapping artifacts were kept for the default period).

Path-filter defect to avoid: the Android PR workflow listed `strings-aniyomi.xml` and `plurals-aniyomi.xml`, which do not exist (p1-behavior.md section 17.3). The Kurobayomi filter must name only files that exist; `cargo xtask i18n-check` checks that the path filter and the `po/` directory agree.

## 3. CI release workflow (PROPOSAL)

File: `.github/workflows/release.yml`.

Trigger: `push` with `tags: ['v*.*.*']` only (the Android push trigger also matched branches; Proposal drops branch builds from the release workflow, since `ci.yml` builds branches).

Permissions: `contents: write` only on the release job. Other jobs stay read-only.

Guard: the job fails unless `github.repository` equals the configured owner repository (Proposal; the Android guard `github.repository == 'aniyomiorg/aniyomi'` at `build_push.yml:60,66,78,103` does the same job for signing and release creation, so forks do not publish).

Steps (Proposal, in order):

1. Checkout with `fetch-depth: 0` and `fetch-tags: true`.
2. Derive `VERSION_TAG=${GITHUB_REF_NAME}` and `VERSION=${VERSION_TAG#v}`. Fail if `Cargo.toml` `version` differs from `VERSION`.
3. Derive `KB_GIT_SHA=$(git rev-parse --short HEAD)`, `KB_COMMIT_COUNT=$(git rev-list --count HEAD)`, `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)`. Rewrite the Flatpak manifest env values with these (see p1-rust-design.md section 7), so the binary and the bundle report the tag commit.
4. `cargo xtask lint`, `cargo xtask test`.
5. `cargo auditable build --profile dist --locked --features updater` for the tarball (updater on, tarball only).
6. `cargo xtask dist`: creates `dist/kurobayomi-${VERSION}-x86_64.tar.gz` and `dist/kurobayomi-${VERSION}-aarch64.tar.gz` (the aarch64 job runs on `ubuntu-24.04-arm` (Proposal) or cross-compiles), `dist/SHA256SUMS`, `dist/THIRD-PARTY.html` (from `cargo about`).
7. Flatpak build for x86_64 and aarch64 with `flatpak-builder`, export to `repo/`, `flatpak build-bundle repo kurobayomi-${VERSION}-<arch>.flatpak io.github.OWNER.Kurobayomi`.
8. Sign: `gpg --batch --yes --detach-sign --armor -o dist/SHA256SUMS.asc dist/SHA256SUMS` using the release key stored as `GPG_PRIVATE_KEY` (secret) with `GPG_PASSPHRASE` (secret). Proposal: minisign is an alternative with a smaller key surface; decision D-2 in p1-tasks.md.
9. Body: extract the `## [vX.Y.Z]` section from `CHANGELOG.md` with `awk` (the heading format is `## [v0.18.2.1] - 2026-09-14`, from `CHANGELOG.md:13`). Append the checksum table (one row per artifact, SHA-256 values from `dist/SHA256SUMS`) in valid GFM (the Android table had malformed rows, p1-behavior.md section 11.2 step 12).
10. `softprops/action-gh-release` with `draft: true`, `prerelease: ${{ contains(env.VERSION, '-') }}` (Proposal: pre-release tags such as `v0.2.0-rc1` are marked pre-release automatically), `files:` the two tarballs, two flatpak bundles, `SHA256SUMS`, `SHA256SUMS.asc`, `THIRD-PARTY.html`.

The draft stays a draft until a maintainer publishes it (observed behaviour kept on purpose).

## 4. Secrets and variables (PROPOSAL)

| Name | Kind | Used by | Replaces |
|---|---|---|---|
| `GPG_PRIVATE_KEY` | secret | release step 8 | `SIGNING_KEY` (Android keystore, base64) |
| `GPG_PASSPHRASE` | secret | release step 8 | `KEY_PASSWORD` |
| `GITHUB_TOKEN` | automatic | release step 10 | same |
| `FLATHUB_TOKEN` | secret (optional) | publishing to Flathub, not in this area | none |

The Android `ALIAS` and `KEY_STORE_PASSWORD` have no equivalent (no keystore).

## 5. Checksum and signature format (PROPOSAL)

`SHA256SUMS` uses the `sha256sum` format: `<64 hex>  <filename>` per line, two spaces, sorted by filename. Verification: `sha256sum -c SHA256SUMS`. The signature is `SHA256SUMS.asc` (ASCII-armoured GPG). The README documents how to verify. The Android tag job wrote SHA-256 of each APK into the release body; keep that body table, generated from the same file.

## 6. Release checklist as code (PROPOSAL)

`cargo xtask release-check <tag>` fails when any of these hold:

1. The tag is not `v` followed by `Cargo.toml` `version`.
2. `CHANGELOG.md` has no `## [v<version>]` heading.
3. `data/io.github.OWNER.Kurobayomi.metainfo.xml` has no `<release version="<version>">` entry.
4. `po/LINGUAS` lists a language without a `.po` file, or a `.po` file is not in `LINGUAS`.
5. `Cargo.lock` is not up to date (`cargo metadata --locked` fails).
6. `packaging/flatpak/cargo-sources.json` does not match `Cargo.lock` (regenerated and diffed).

Each check is one test in `xtask`. The Android repository had no such check; version and changelog drift is observed (p1-behavior.md section 3).

## 7. Retention, caching and runners (PROPOSAL)

- Rust cache keyed on `Cargo.lock` and the toolchain.
- Flatpak builder cache: `.flatpak-builder/` restored by key `flatpak-${{ hashFiles('packaging/flatpak/*.yml', 'Cargo.lock') }}`.
- `ubuntu-24.04` for x86_64; `ubuntu-24.04-arm` for aarch64 if available to the repository; otherwise `flatpak-builder --arch=aarch64` under QEMU (slow; Proposal to skip aarch64 for v0.1 and say so in the release notes).

## 8. Failure modes (PROPOSAL)

| Failure | Detection | Behaviour |
|---|---|---|
| Missing GPG secret on a tag | release step 8 | fail the job; no draft release is created |
| Shallow clone | `KB_COMMIT_COUNT` = 1 vs `git rev-list --count HEAD` | `fetch-depth: 0` in every job that builds |
| Flatpak runtime missing | `flatpak-builder --install-deps-from=flathub` | fail |
| Cargo.lock drift | `cargo build --locked` | fail |
| Changelog missing heading | release check 2 | fail before any artifact is uploaded |
| Concurrent tag builds | concurrency group on `github.ref` | the newer run cancels the older one (`cancel-in-progress: true`) for branches; for tags, cancel is disabled so a release is never cut short (Proposal) |

## 9. Observed Android-only artifacts and their fate

| Artifact | Observed | Fate |
|---|---|---|
| `app-universal-release-unsigned.apk` and per-ABI APKs | built, renamed, hashed | dropped; replaced by tarballs and Flatpak bundles per arch |
| R8 mapping | uploaded | dropped |
| Signed APK | signed by `r0adkll/sign-android-release` | dropped; replaced by GPG-signed `SHA256SUMS` |
| Release body SHA table | 5 rows | kept with Linux artifacts |
| `draft: true` release | created | kept |
| CI artifacts `arm64-v8a-<sha>`, `mapping-<sha>` | uploaded | replaced by `kurobayomi-dist-<sha>` and `kurobayomi-flatpak-<arch>-<sha>` |
