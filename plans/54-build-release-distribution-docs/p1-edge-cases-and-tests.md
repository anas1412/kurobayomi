# Edge cases, failure modes and tests: area 54

No scope file is a unit test. The upstream tests in scope are instrumentation or macrobenchmark tests (`macrobenchmark/src/main/java/tachiyomi/macrobenchmark/StartupBenchmark.kt`, `BaselineProfileGenerator.kt`), and both are dropped or converted (section 4). The tests below are new tests for the Kurobayomi build and release tooling.

## 1. Edge cases in the observed build (from source reading)

| ID | Where | Case | Observed effect | Port requirement |
|---|---|---|---|---|
| EC-54-01 | `buildSrc/src/main/kotlin/mihon/buildlogic/Commands.kt:11-14` | No git on PATH, or not a git repository | `providers.exec` fails; the comment says to return a constant instead | `build.rs` falls back to `unknown` / `0` / now (never panics) |
| EC-54-02 | `Commands.kt:30` | `git log -1 --format=%ct` returns empty (no commits) | `.toLong()` throws `NumberFormatException` for release builds | fallback to `SOURCE_DATE_EPOCH` then now |
| EC-54-03 | `actions/checkout` default in `build_pull_request.yml:30-31` and `build_push.yml:23-24` | Shallow clone, depth 1 | `git rev-list --count HEAD` is `1`, so `COMMIT_COUNT` is wrong in CI builds | `fetch-depth: 0` in every job that reads the count |
| EC-54-04 | `app/build.gradle.kts:48` | Debug `versionNameSuffix` uses `getCommitCount()` | the suffix changes on every commit and differs between a shallow CI clone and a local clone | Kurobayomi uses `KB_COMMIT_COUNT` only for display, not for `CARGO_PKG_VERSION` |
| EC-54-05 | `app/build.gradle.kts:20` and `:65,:80` | applicationId changes per build type | side-by-side install of release, preview, debug, benchmark | Kurobayomi: `.Devel` suffix for dev builds; `beta` branch for preview |
| EC-54-06 | `app/build.gradle.kts:320-334` | `productFlavors` empty, so the benchmark variant filter disables the variant | macrobenchmark target missing (not verified by a Gradle run) | no equivalent: do not port the filter; keep the startup bench as a script |
| EC-54-07 | `StartupBenchmark.kt:78` | package `xyz.jmir.tachiyomi.mibenchmark` | measures a non-existent package | `scripts/bench-startup.sh` takes the binary path, not a package name |
| EC-54-08 | `build_push.yml:60,66,78,103` | Tag push from a fork | the `if:` guard skips signing and release | Kurobayomi: same guard with the owner repository name |
| EC-54-09 | `build_push.yml:58-63` | `VERSION_TAG` from `GITHUB_REF` | `refs/tags/v0.18.2.1` gives `v0.18.2.1`; the step runs only when `GITHUB_REF` starts with `refs/tags/` (its `if:` at line 60) | Kurobayomi asserts `VERSION_TAG` equals `v` + `Cargo.toml` version |
| EC-54-10 | `build_push.yml:77-100` | rename step assumes `-signed.apk` names exist | if signing fails, `mv` fails and the job stops (`set -e`) | Kurobayomi: the release job fails before any upload if a file is missing |
| EC-54-11 | `build_push.yml:102-127` | `draft: true` | the release is never public until edited | keep: maintainer publishes |
| EC-54-12 | `build_push.yml:108-119` | malformed markdown table rows (Universal, x86_64) | the table renders with broken rows | generated table is validated by `cargo xtask release-check` |
| EC-54-13 | `build_pull_request.yml:7-14` | path filter names `strings-aniyomi.xml` and `plurals-aniyomi.xml`, which do not exist | those entries never match; the filter is partly dead | `xtask i18n-check` verifies filter entries exist |
| EC-54-14 | `build_pull_request.yml:5-6` | `'!**.md'` excludes Markdown-only PRs | docs-only PRs skip CI, so broken links in docs are not checked | Kurobayomi runs a link check on Markdown (`lychee`, Proposal) in a docs job |
| EC-54-15 | `buildSrc/.../ProjectExtensions.kt:52-55` | `warningsAsErrors` defaults to false | warnings do not fail the build | Kurobayomi: `clippy -D warnings` in CI (default-deny for warnings) |
| EC-54-16 | `buildSrc/.../LocalesConfigTask.kt:14` | locales whose `strings.xml` is empty (`<resources/>`) are excluded | new empty translation files do not appear in locale config | `i18n-check`: a `.po` with 0 translated messages is listed as "untranslated" and excluded from `LINGUAS` unless `--include-empty` |
| EC-54-17 | `LocalesConfigTask.kt:17-19` | folder `base` maps to `en`; `-r` and `+` map to `-` | `ka-rGE` to `ka-GE`; `base` to `en`; `zh-rCN` to `zh-CN` | gettext codes: `base` is the template; `ka-rGE` to `ka_GE`; `zh-rCN` to `zh_CN` |
| EC-54-18 | `LocalesConfigTask.kt` (reads `strings.xml` only) | locale folders with only `plurals.xml` | excluded from locale config | `i18n-check` looks at `.po` files only |
| EC-54-19 | `i18n-aniyomi` `moko-resources` has `ur`; `i18n` does not | the two modules disagree on locales | the locale set is the union in the merged domain | union, then report the difference |
| EC-54-20 | `in` folder (legacy Indonesian code) | Android legacy code | BCP 47 is `id` | rename to `id_ID`/`id` in `po/` |
| EC-54-21 | `app/proguard-rules.pro:3-5` | `-keep,allowoptimization class eu.kanade.**` | keeps classes that R8 might remove | no Rust equivalent; the Rust release profile has no keep list |
| EC-54-22 | `app/build.gradle.kts:103-128` | `keepDebugSymbols` list (22 entries) | keeps native debug symbols for crash reports | Kurobayomi: split debuginfo file in `dist` (`objcopy --only-keep-debug`) |
| EC-54-23 | `gradle.properties:9` `-Xmx4g` | Gradle memory | none for Rust | none |
| EC-54-24 | `gradle/gradle-daemon-jvm.properties` | toolchain download URLs by platform | network needed at configuration time on a fresh machine | Rust toolchain via `rust-toolchain.toml`; network only for crates |
| EC-54-25 | `gradle/wrapper/gradle-wrapper.properties:3` | `validateDistributionUrl=true`, no checksum | Gradle distribution integrity relies on HTTPS only | rustup verifies toolchain signatures (Proposal: no action needed) |
| EC-54-26 | `CONTRIBUTING.md:45-50` | fork rules mention `google-services.json` and ACRA | files do not exist | Kurobayomi CONTRIBUTING lists only real files |
| EC-54-27 | `README.md:24` "Requires Android 8.0 or higher" | min requirement | Linux equivalent: GTK 4.16, libmpv, glibc | README lists the Linux requirements (T-54a-062) |
| EC-54-28 | `ISSUE_TEMPLATE/report_issue.yml:45` | "Dump crash logs" in More > Settings > Advanced | not in Linux Kurobayomi | the issue form asks for the log file from `$XDG_STATE_HOME/kurobayomi/logs` |
| EC-54-29 | `ISSUE_TEMPLATE/report_issue.yml:100-105` | acknowledgement says "I understand that Aniyomi does not have or fix any extensions" | upstream-specific policy | Kurobayomi policy text is written in T-54a-015 |
| EC-54-30 | `fastlane/metadata/android/en-US/images/icon.png` | symlink to `.github/assets/logo.png` (1,606 bytes) | Flathub needs a 512 px icon; a symlink in a git tree on Windows checkouts breaks | real files under `data/icons/`; the symlink is removed |
| EC-54-31 | `.gitattributes:5` `/.idea/* text eol=crlf` | Windows line endings for IDE files | Linux-only project | dropped |
| EC-54-32 | `.gitignore:8-9` `.idea/*`, `!.idea/icon.png` | IDE files | the repository has an `.idea/icon.png` in the Android tree; Kurobayomi has none | keep `.idea/` ignored; no re-include |
| EC-54-33 | `macrobenchmark/README.md:9` `devBenchmark` | variant name that does not exist | wrong instruction | dropped with the module |
| EC-54-34 | `app/src/main/baseline-prof.txt` | 3.9 MB generated file in git | repository size; regenerated per release | dropped |

## 2. Failure modes of the new tooling (PROPOSAL)

| ID | Component | Failure | Expected behaviour |
|---|---|---|---|
| FM-54-01 | `build.rs` | `git` binary missing | warning-free build with `KB_GIT_SHA=unknown`, `KB_COMMIT_COUNT=0` |
| FM-54-02 | `build.rs` | `SOURCE_DATE_EPOCH` non-numeric | ignore it, fall back to git, then now |
| FM-54-03 | `build.rs` | `KB_APP_ID` contains a character invalid for a D-Bus name (`-` in a component start, digit first) | build fails with a message naming the variable |
| FM-54-04 | `xtask dist` | `strip` or `objcopy` missing | fail with the tool name; no partial tarball is left in `dist/` |
| FM-54-05 | `xtask dist` | a file listed in the tarball manifest does not exist | fail; list the missing paths |
| FM-54-06 | `xtask i18n-check` | `.po` file with a syntax error | `msgfmt --check` fails; report the file and line |
| FM-54-07 | `xtask i18n-check` | `LINGUAS` lists a code with no `.po` | fail |
| FM-54-08 | `xtask release-check` | tag does not match `Cargo.toml` version | fail before build |
| FM-54-09 | `xtask release-check` | CHANGELOG section missing | fail before build |
| FM-54-10 | `cargo deny` | a GPL-only crate appears in the tree | fail; the licence allow-list is in `deny.toml` |
| FM-54-11 | `flatpak-builder` | `cargo-sources.json` out of date | `cargo --offline` fails in the sandbox; CI check regenerates and diffs first |
| FM-54-12 | `bench-startup.sh` | `hyperfine` missing | exit 127 with install hint; no partial JSON |
| FM-54-13 | `bench-startup.sh` | the binary exits non-zero on `--exit-after-first-frame` | hyperfine reports the failure; script exits non-zero |
| FM-54-14 | GPG signing | key missing or passphrase wrong | job fails before the draft release is created |
| FM-54-15 | `release.yml` | run is for a fork | `if:` guard skips the release job; the build job still runs |

## 3. Race conditions and ordering (PROPOSAL)

- RC-54-01: two tag pushes for the same version in quick succession. Both build; the GitHub release action creates or updates a draft by tag name. Proposal: `concurrency.group: release-${{ github.ref_name }}` with `cancel-in-progress: false` so the first release run finishes.
- RC-54-02: `KB_GIT_SHA` read by `build.rs` and rewritten by the release workflow in the Flatpak manifest. The rewrite must happen before `flatpak-builder` starts, and the tarball build must use the same env. Proposal: a single shell step writes both the manifest and exports the variables.
- RC-54-03: `po/LINGUAS` and `.po` files change in the same PR. `i18n-check` runs after checkout, so it sees both.
- RC-54-04: `CHANGELOG.md` edited after the tag. The release body is generated from the tag's commit. Proposal: release body uses `git show <tag>:CHANGELOG.md`, not the working tree.
- RC-54-05: Flatpak cache restore and `Cargo.lock` change. Cache key includes `Cargo.lock` (see topic-ci-and-release section 7).

## 4. Upstream tests and benchmarks in scope and their fate

| Upstream | Kind | Assertions (OBSERVED) | Fate |
|---|---|---|---|
| `StartupBenchmark.kt` `ColdStartupBenchmark.startupNoCompilation` | macrobenchmark, 10 iterations, `StartupTimingMetric` | measures cold start with no compilation | port as `scripts/bench-startup.sh --mode cold` (T-54a-023) |
| `StartupBenchmark.kt` `*.startupBaselineProfileDisabled` | same, `CompilationMode.Partial(Disable, warmup 1)` | ART-only | dropped (no compilation modes in a native binary) |
| `StartupBenchmark.kt` `*.startupBaselineProfile` | `Partial(Require)` | ART-only | dropped |
| `StartupBenchmark.kt` `*.startupFullCompilation` | `CompilationMode.Full()` | ART-only | dropped |
| `StartupBenchmark.kt` `Warm*`, `Hot*` | startup modes WARM and HOT | start mode variants | port as the warm and hot run variants of the bench script (T-54a-023) |
| `BaselineProfileGenerator.kt` `generate` | `BaselineProfileRule`, taps through tabs | records a profile | dropped; a GTK smoke test replaces the UI walk (T-54a-023 acceptance) |

No `*Test.kt` file is in this area's scope.

## 5. New tests to write (PROPOSAL)

| ID | Level | Test | Assertion |
|---|---|---|---|
| TST-54-01 | unit (`build.rs` logic in `build_support` module or a `#[cfg(test)]` in `xtask`) | `build_time_from_epoch` | `SOURCE_DATE_EPOCH=0` gives `1970-01-01T00:00:00Z` |
| TST-54-02 | unit | `build_time_invalid_epoch_falls_back` | non-numeric epoch uses git then now |
| TST-54-03 | unit | `version_string_channels` | `stable` gives `Stable 0.1.0`; `beta` gives `Preview r<count> (<sha>)`; `debug` gives `Debug <sha> (<time>)` (matches behaviour section 4 logic) |
| TST-54-04 | unit | `locale_code_mapping` | vectors: `base`→`template`; `ka-rGE`→`ka_GE`; `nb-rNO`→`nb_NO`; `pt-rBR`→`pt_BR`; `zh-rCN`→`zh_CN`; `zh-rTW`→`zh_TW`; `in`→`id`; `ka`→`ka` |
| TST-54-05 | unit | `locales_config_excludes_empty` | a `strings.xml` containing `<resources/>` or `<resources>   </resources>` is excluded (regex from `LocalesConfigTask.kt:8`) |
| TST-54-06 | unit | `linguas_sorted_and_unique` | output sorted, no duplicates |
| TST-54-07 | unit | `changelog_section_extract` | from `CHANGELOG.md` text, the `## [v0.18.2.1]` heading is extracted up to the next `## [` heading |
| TST-54-08 | unit | `sha256sums_format` | line format `<64 hex>  <name>`, sorted |
| TST-54-09 | unit | `tag_version_match` | `v0.1.0` vs `0.1.0` passes; `v0.1.1` vs `0.1.0` fails |
| TST-54-10 | integration | `i18n_check_on_fixture` | a fixture `po/` with one missing `.po` and one bad header fails with both messages |
| TST-54-11 | integration | `dist_tarball_contents` | the tarball contains exactly: binary, `.desktop`, `.metainfo.xml`, icons, `*.mo`, `LICENSE`, `NOTICE`, `THIRD-PARTY.html`, `SHA256SUMS`; no `target/` paths |
| TST-54-12 | integration | `appstream_validate` | `appstreamcli validate --pedantic` passes on `data/*.metainfo.xml` |
| TST-54-13 | integration | `desktop_file_validate` | `desktop-file-validate data/*.desktop` passes |
| TST-54-14 | integration | `flatpak_manifest_lint` | `flatpak-builder-lint manifest` passes |
| TST-54-15 | integration | `bench_script_smoke` | `scripts/bench-startup.sh` runs with `--runs 1` against a stub binary that exits 0 |
| TST-54-16 | CI | `path_filter_entries_exist` | every path in `ci.yml` `paths-ignore` or `paths` exists (or is a glob that matches at least one file) |
| TST-54-17 | CI | `cargo_lock_locked` | `cargo build --locked` passes |
| TST-54-18 | CI | `cargo_deny_clean` | `cargo deny check` passes with `deny.toml` |
| TST-54-19 | CI | `release_check_dry_run` | `cargo xtask release-check v0.1.0` on the current tree reports the exact failures |
| TST-54-20 | manual | `flatpak_install_run` | `flatpak-builder --user --install` then `flatpak run io.github.OWNER.Kurobayomi --version` prints the version and build metadata |
| TST-54-21 | manual | `theme_follow` | in a native build on KDE Plasma with a GTK theme set, the window uses that theme (not the Flatpak runtime default); see p1-rust-design.md section 14.1 |

## 6. Fixtures needed (PROPOSAL)

- `xtask/tests/fixtures/po/`: two `.po` files (`de.po` complete, `fr.po` with bad header), one `LINGUAS` listing an extra code.
- `xtask/tests/fixtures/CHANGELOG.md`: two sections (`v0.1.0`, `v0.1.1`).
- `xtask/tests/fixtures/locales/`: copies of the Android locale folder names from the source tree (`base`, `ka-rGE`, `nb-rNO`, `pt-rBR`, `zh-rCN`, `zh-rTW`, `in`) with `strings.xml` containing `<resources/>` for one of them.
- `scripts/tests/stub-binary.sh`: exits 0 and prints nothing.
- A real `.metainfo.xml` and `.desktop` file from `data/` for validators.

## 7. Coverage targets (PROPOSAL)

- `xtask`: 80% line coverage on `dist`, `i18n-check`, `release-check` modules (measured with `cargo llvm-cov` in an optional CI job).
- `build.rs`: all branches covered by TST-54-01 to TST-54-03 via a `build_support` module included with `#[path]`.
