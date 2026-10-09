# Tasks: 54-build-release-distribution-docs (part 1 of 1)

Scope coverage: every file in the inventory (p1-inventory.md, 75 rows) is referenced by at least one task below, either as an implementation source or as an explicit P3 drop task. Each drop task states the replacement (or "none") and what the user loses (p1-behavior.md section 16).

Open decisions that block specific tasks. They are recorded here, not guessed:

- D-1 (blocks T-54a-001, T-54a-013, T-54a-019, T-54a-011): the GitHub owner segment of the app id `io.github.<owner>.Kurobayomi`. The checkout has no git remote.
- D-2 (blocks T-54a-056): release signing tool, GPG (`gpg --detach-sign`) or minisign. Default if not decided: GPG.
- D-3 (blocks T-54a-061): aarch64 release artifacts in v0.1. Default if not decided: x86_64 Flatpak and tarball first; aarch64 added when a runner is available.
- D-4 (blocks T-54a-017): contact channel in the Code of Conduct (the upstream Discord is not used).

### T-54a-001 — Cargo workspace root and toolchain pin
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: settings.gradle.kts, build.gradle.kts, buildSrc/settings.gradle.kts, buildSrc/build.gradle.kts, gradle.properties, gradle/gradle-daemon-jvm.properties, gradle/wrapper/gradle-wrapper.properties, gradlew, gradlew.bat
- Acceptance:
  - `Cargo.toml` at the repo root has `[workspace]` with `members = ["crates/*", "xtask"]`, `resolver = "3"`, and `[workspace.package]` with version `0.1.0`, edition `2024`, rust-version `1.85`, license `Apache-2.0`.
  - `rust-toolchain.toml` pins `channel = "stable"` with components `rustfmt` and `clippy`.
  - `cargo build --workspace --locked` succeeds on a clean checkout with no Gradle file present.
  - Decision D-1 is resolved and the owner segment is a single constant read by `build.rs` and `xtask`.
  - All nine listed Gradle files are deleted in the same change.

### T-54a-002 — Build metadata in build.rs (replaces Commands.kt)
- Priority: P0
- Effort: S
- Depends: T-54a-001
- Sources: buildSrc/src/main/kotlin/mihon/buildlogic/Commands.kt, app/build.gradle.kts
- Acceptance:
  - `build.rs` exports `KB_GIT_SHA`, `KB_COMMIT_COUNT`, `KB_BUILD_TIME`, `KB_APP_ID` through `cargo:rustc-env`.
  - With `git` removed from PATH, the build succeeds with `KB_GIT_SHA=unknown` and `KB_COMMIT_COUNT=0`.
  - With `SOURCE_DATE_EPOCH=0`, `KB_BUILD_TIME` is `1970-01-01T00:00:00Z`.
  - Unit tests TST-54-01 and TST-54-02 (p1-edge-cases-and-tests.md section 5) pass.
  - `KB_BUILD_TIME` format is RFC 3339 UTC, matching the Android format `yyyy-MM-dd'T'HH:mm:ss'Z'` (Commands.kt:21).

### T-54a-003 — Release profile and Cargo features (replaces build types and build flags)
- Priority: P0
- Effort: S
- Depends: T-54a-001
- Sources: buildSrc/src/main/kotlin/mihon/buildlogic/AndroidConfig.kt, buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt, buildSrc/src/main/kotlin/mihon/buildlogic/ProjectExtensions.kt, app/build.gradle.kts
- Acceptance:
  - `[profile.release]` has `lto = "thin"`, `codegen-units = 1`, `strip = "debuginfo"`, `debug = "line-tables-only"`, `panic = "unwind"`; `[profile.dist]` inherits release with `lto = "fat"`.
  - Cargo feature `updater` exists, is not in `default`, and `cargo build --release` produces a binary without the updater code path (checked by a `cfg!(feature = "updater")` assertion in a test).
  - `gtk4` dependency has feature `v4_16`, recorded as the minimum runtime in README (T-54a-062).
  - The three flags `enable-updater`, `disable-code-shrink`, `include-dependency-info` have a documented Cargo equivalent in CONTRIBUTING.md (T-54a-018).

### T-54a-004 — Replace Gradle version catalogs with workspace dependencies
- Priority: P1
- Effort: S
- Depends: T-54a-001
- Sources: gradle/libs.versions.toml, gradle/androidx.versions.toml, gradle/compose.versions.toml, gradle/kotlinx.versions.toml, gradle/aniyomi.versions.toml, buildSrc/settings.gradle.kts
- Acceptance:
  - Every crate named as "Replace-with-crate" in p1-topic-dependency-mapping.md sections 1 to 5 appears in `[workspace.dependencies]` or is marked as owned by a feature area in that document.
  - Every "Drop" row has no entry in `Cargo.toml`.
  - `cargo tree --duplicates` output is reviewed and recorded in the PR description.
  - No `gradle/*.versions.toml` file remains.

### T-54a-005 — rustfmt, clippy and xtask lint (replaces spotless and ktlint)
- Priority: P1
- Effort: S
- Depends: T-54a-001
- Sources: buildSrc/src/main/kotlin/mihon.code.lint.gradle.kts
- Acceptance:
  - `rustfmt.toml` sets `edition = "2024"`, `max_width = 120`, `use_small_heuristics = "Max"`, `newline_style = "Unix"`.
  - `[workspace.lints]` sets `unsafe_code = "forbid"` (or a per-crate exception), `unused_must_use = "deny"`, clippy `all = warn`, `unwrap_used = warn`.
  - `cargo xtask lint` fails on a seeded format error and on a seeded clippy warning, and passes on the clean tree.
  - CI runs `cargo clippy --workspace --all-targets --locked -- -D warnings`.

### T-54a-006 — .editorconfig and .gitattributes for the Rust tree
- Priority: P1
- Effort: S
- Depends: none
- Sources: .editorconfig, .gitattributes
- Acceptance:
  - `.editorconfig` keeps `root = true`, `charset = utf-8`, `indent_style = space`, `insert_final_newline = true`, `trim_trailing_whitespace = true`; sets `[*.rs] indent_size = 4`; drops the ktlint keys.
  - `.gitattributes` keeps `* text=auto` and `* text eol=lf`, keeps binary markers for png, webp, jpg, jpeg, gif, ico, gz, zip, 7z, ttf, woff; drops `/.idea/* text eol=crlf` and `*.jar`.
  - `editorconfig-checker` passes on the tree.

### T-54a-007 — .gitignore for Rust, Flatpak and dist output
- Priority: P1
- Effort: S
- Depends: none
- Sources: .gitignore
- Acceptance:
  - Ignores `/target`, `/.flatpak-builder`, `/repo`, `/build-dir`, `/dist`, `*.flatpak`, `.idea/`, `*.iml`, `.DS_Store`.
  - After `cargo build` and `flatpak-builder`, `git status --short` shows no build artifacts.
  - The existing `/target` entry is kept.

### T-54a-008 — CI pull request pipeline (ci.yml)
- Priority: P1
- Effort: M
- Depends: T-54a-005, T-54a-013, T-54a-054, area:51-i18n
- Sources: .github/workflows/build_pull_request.yml
- Acceptance:
  - Jobs `lint`, `test`, `build`, `deny`, `flatpak` run in the order in p1-topic-ci-and-release.md section 2; `flatpak` needs `build`.
  - Checkout uses `fetch-depth: 0` in every job that reads the build metadata.
  - `concurrency.group` is `${{ github.workflow }}-${{ github.ref }}` with `cancel-in-progress: true`; top-level `permissions: contents: read`.
  - Path filter entries all match existing files (TST-54-16); the `-aniyomi.xml` entries are gone.
  - A docs-only PR skips the `build` and `flatpak` jobs.
  - Dependency-review step is replaced by `cargo deny check`.

### T-54a-009 — Release workflow on version tags (release.yml)
- Priority: P1
- Effort: M
- Depends: T-54a-008, T-54a-010, T-54a-056, T-54a-021
- Sources: .github/workflows/build_push.yml
- Acceptance:
  - Trigger is `push` with `tags: ['v*.*.*']`; branches do not trigger the release workflow.
  - On the upstream repository, a tag run creates one draft release (`draft: true`, `prerelease` true when the version contains `-`) with: two tarballs (x86_64, and aarch64 when D-3 allows), the Flatpak bundle(s), `SHA256SUMS`, `SHA256SUMS.asc`, `THIRD-PARTY.html`.
  - On a fork, the release job is skipped and the build job still runs.
  - The release body is the CHANGELOG section for the tag, followed by a valid GFM checksum table (EC-54-12 fixed).
  - Step-by-step mapping in p1-topic-ci-and-release.md section 1 is implemented or explicitly dropped.

### T-54a-010 — Release check in xtask (tag, changelog, metainfo, LINGUAS, lockfile, Flatpak sources)
- Priority: P2
- Effort: S
- Depends: T-54a-034, T-54a-021
- Sources: .github/workflows/build_push.yml
- Acceptance:
  - `cargo xtask release-check <tag>` fails with a named message for each of the six checks in p1-topic-ci-and-release.md section 6.
  - TST-54-08, TST-54-09 and TST-54-19 pass.
  - The check runs before any artifact is built in the release workflow.

### T-54a-011 — AppStream metainfo and desktop entry (replaces fastlane metadata)
- Priority: P1
- Effort: M
- Depends: T-54a-001
- Sources: fastlane/metadata/android/en-US/short_description.txt, fastlane/metadata/android/en-US/full_description.txt
- Acceptance:
  - `data/io.github.<owner>.Kurobayomi.metainfo.xml` has the summary from `short_description.txt` (no trailing period) and a `<description>` with `<p>` and `<ul><li>` converted from `full_description.txt`; the `<h3>` heading and the "Plus much more..." item are removed.
  - `data/io.github.<owner>.Kurobayomi.desktop` has the four desktop actions Library, Updates, History, Browse (replaces `app/shortcuts.xml` shortcuts).
  - `appstreamcli validate --pedantic` and `desktop-file-validate` pass (TST-54-12, TST-54-13).
  - `<metadata_license>CC0-1.0</metadata_license>` and `<project_license>Apache-2.0</project_license>` are present.

### T-54a-012 — Real icon files (replaces the symlinked icon.png)
- Priority: P2
- Effort: S
- Depends: T-54a-011
- Sources: fastlane/metadata/android/en-US/images/icon.png
- Acceptance:
  - `data/icons/hicolor/scalable/apps/io.github.<owner>.Kurobayomi.svg` and `data/icons/hicolor/512x512/apps/io.github.<owner>.Kurobayomi.png` exist as regular files.
  - The symlink `fastlane/metadata/android/en-US/images/icon.png` is removed in the same change.
  - The icon shows in the KDE application launcher after `flatpak install --user`.

### T-54a-013 — Flatpak manifest with pinned mpv module and cargo sources
- Priority: P1
- Effort: M
- Depends: T-54a-001, T-54a-011
- Sources: build.gradle.kts, app/build.gradle.kts
- Acceptance:
  - `packaging/flatpak/io.github.<owner>.Kurobayomi.yml` uses runtime `org.gnome.Platform` branch `48` (or the newest branch that ships GTK 4.16 or newer, verified at implementation), the Rust SDK extension, and the finish-args in p1-rust-design.md section 7.
  - The mpv module is pinned by tag `v0.41.0` and by commit (added when the manifest is written); mpv meson option names are checked against that version.
  - `packaging/flatpak/cargo-sources.json` is generated from `Cargo.lock`; CI diffs it.
  - `flatpak-builder --user --install packaging/flatpak/io.github.<owner>.Kurobayomi.yml` succeeds, and `flatpak run io.github.<owner>.Kurobayomi --version` prints the version and the `KB_*` values.
  - No filesystem permission beyond the portal is requested.

### T-54a-014 — Drop upstream funding links
- Priority: P3
- Effort: S
- Depends: none
- Sources: .github/FUNDING.yml
- Acceptance:
  - `.github/FUNDING.yml` is deleted, or replaced by the maintainer's own links (decision recorded in CONTRIBUTING.md).
  - User impact: none for the app. The upstream maintainer's Sponsors and Ko-fi links are no longer shown on the repository.

### T-54a-015 — Issue templates for Linux
- Priority: P2
- Effort: S
- Depends: T-54a-019
- Sources: .github/ISSUE_TEMPLATE/config.yml, .github/ISSUE_TEMPLATE/report_issue.yml, .github/ISSUE_TEMPLATE/request_feature.yml
- Acceptance:
  - `config.yml` keeps `blank_issues_enabled: false` and links to the Kurobayomi README and issue tracker; upstream Discord and aniyomi.org links are removed.
  - `report_issue.yml` asks for: steps, expected, actual, log file path from `$XDG_STATE_HOME/kurobayomi/logs`, Kurobayomi version, Linux distribution, desktop environment, session type (Wayland or X11), GTK version, libmpv version, and an acknowledgement. The Android fields (Android version, device, More > Settings > Advanced) are removed.
  - `request_feature.yml` keeps the description and acknowledgement fields with the Kurobayomi version.
  - GitHub renders all three forms without errors.

### T-54a-016 — Pull request template for Linux
- Priority: P2
- Effort: S
- Depends: T-54a-005
- Sources: .github/pull_request_template.md
- Acceptance:
  - Template has a summary and issue-link section, checkboxes for `cargo xtask lint`, `cargo test`, and a manual GTK check in light and dark GTK themes.
  - The screenshot table is kept for visual changes; the tablet and "base themes" wording is removed.

### T-54a-017 — Code of Conduct (Contributor Covenant 2.1)
- Priority: P1
- Effort: S
- Depends: none
- Sources: CODE_OF_CONDUCT.md
- Acceptance:
  - Contributor Covenant 2.1 text and the four enforcement guideline levels are unchanged.
  - The enforcement contact is the channel chosen in D-4; the reference to `https://discord.gg/tachiyomi` is removed.
  - The Attribution section links to Contributor Covenant v2.1 as before.

### T-54a-018 — CONTRIBUTING with Rust prerequisites, fork rules and release runbook
- Priority: P1
- Effort: S
- Depends: T-54a-019, T-54a-034
- Sources: CONTRIBUTING.md
- Acceptance:
  - Prerequisites: stable Rust (rustup), GTK 4.16 development package, libmpv development package, pkg-config, gettext; Flatpak build instructions.
  - Lists only files that exist (no `google-services.json`, no ACRA).
  - Fork rules: change the app id (`KB_APP_ID`), replace the icon, leave the `updater` feature off, change the release signing key.
  - Release runbook: tag `vX.Y.Z`, update CHANGELOG and metainfo, run `cargo xtask release-check`, publish the draft (T-54a-009).
  - Translations: Weblate is replaced by the chosen process (decision recorded).

### T-54a-019 — README rewrite for Kurobayomi
- Priority: P1
- Effort: S
- Depends: T-54a-020, T-54a-062
- Sources: README.md
- Acceptance:
  - Installation: Flatpak bundle, and build from source with `cargo build --release`.
  - States that a native install follows the host GTK theme and the Flatpak build uses the runtime theme unless overridden (p1-rust-design.md section 14.1).
  - Feature list matches `data/*.metainfo.xml`.
  - Requirements table (T-54a-062) and an attribution section that names Aniyomi, Mihon and Tachiyomi with the copyright lines from NOTICE.
  - No Android links, badges, or Play-store wording remain.

### T-54a-020 — LICENSE unchanged and NOTICE with attribution
- Priority: P1
- Effort: S
- Depends: none
- Sources: LICENSE
- Acceptance:
  - `LICENSE` is byte-identical to the source file (176 lines, Apache-2.0).
  - `NOTICE` lists `Copyright © 2015 Javier Tomás`, `Copyright © 2024 Mihon Open Source Project`, `Copyright © 2024 Aniyomi Open Source Project` (from README:70-72) and states that Kurobayomi is a rewrite of Aniyomi in Rust.
  - Apache-2.0 section 4(d) is satisfied: the NOTICE text is shipped in the tarball and the Flatpak.

### T-54a-021 — CHANGELOG in Keep-a-Changelog form
- Priority: P1
- Effort: S
- Depends: none
- Sources: CHANGELOG.md
- Acceptance:
  - Legend lines 6 to 11 are kept; the stray trailing space in the `Changed ` legend item is removed.
  - A `## [v0.1.0]` section exists for Kurobayomi with Added entries; upstream sections are not imported (decision recorded in the first section).
  - The heading format `## [vX.Y.Z] - YYYY-MM-DD` is what `release-check` and the release body extraction parse.

### T-54a-022 — Drop the Android integer versionCode
- Priority: P3
- Effort: S
- Depends: T-54a-001
- Sources: app/build.gradle.kts
- Acceptance:
  - No integer `versionCode` exists. The only version is `[workspace.package] version`.
  - User impact: none. Flatpak updates are ordered by the metainfo `<release>` list and the branch.

### T-54a-023 — Startup timing script and in-app trace (replaces StartupBenchmark)
- Priority: P2
- Effort: M
- Depends: T-54a-001, area:01-foundation-app-bootstrap
- Sources: macrobenchmark/src/main/java/tachiyomi/macrobenchmark/StartupBenchmark.kt, macrobenchmark/src/main/java/tachiyomi/macrobenchmark/BaselineProfileGenerator.kt
- Acceptance:
  - `scripts/bench-startup.sh <binary> [runs]` uses hyperfine and exports JSON to `target/bench-startup.json`; TST-54-15 passes.
  - Modes cold (documented sudo cache drop), warm, and hot map to the three Android startup modes; compilation modes are dropped with a note.
  - With `KB_TRACE_STARTUP=1`, the binary writes `main_entry`, `window_presented`, `first_library_render` timestamps to stderr; `--exit-after-first-frame` exits after the first library frame.
  - The script takes a binary path, so the wrong-package-name defect (StartupBenchmark.kt:78) cannot occur.
  - `docs/benchmarking.md` describes the script; the GTK smoke test replaces the BaselineProfileGenerator tab walk (Library, Updates, History, More, Settings).

### T-54a-024 — Drop the baseline profile and its generator
- Priority: P3
- Effort: S
- Depends: T-54a-023
- Sources: app/src/main/baseline-prof.txt, macrobenchmark/README.md, macrobenchmark/src/main/java/tachiyomi/macrobenchmark/BaselineProfileGenerator.kt
- Acceptance:
  - The three files are deleted (baseline profile is 37,790 lines, 3,932,838 bytes, generated).
  - No script or doc refers to `baseline-prof` or `devBenchmark`.
  - User impact: none. A native binary has no JIT warm-up that a profile would replace; startup is measured by T-54a-023.

### T-54a-025 — Drop the macrobenchmark module
- Priority: P3
- Effort: S
- Depends: T-54a-023
- Sources: macrobenchmark/build.gradle.kts, macrobenchmark/src/main/AndroidManifest.xml, buildSrc/src/main/kotlin/mihon.benchmark.gradle.kts
- Acceptance:
  - The `macrobenchmark/` directory is deleted, and no build file references it.
  - User impact: none. The benchmark variant that the module needed is also removed (T-54a-030).

### T-54a-026 — Drop ProGuard and consumer rule files
- Priority: P3
- Effort: S
- Depends: T-54a-001
- Sources: app/proguard-rules.pro, core-metadata/proguard-rules.pro, core-metadata/consumer-rules.pro, data/proguard-rules.pro, data/consumer-rules.pro, domain/proguard-rules.pro, domain/consumer-rules.pro, presentation-core/proguard-rules.pro, presentation-core/consumer-rules.pro, presentation-widget/proguard-rules.pro, presentation-widget/consumer-rules.pro, source-local/proguard-rules.pro, source-local/consumer-rules.pro, source-api/consumer-proguard.pro
- Acceptance:
  - All 14 files are deleted.
  - The extension-API keep list (`source-api/consumer-proguard.pro`, `app/proguard-rules.pro:25-31`) is recorded as obsolete in area:20-extension-strategy-redesign's decision log.
  - No Rust build step reads a keep list.

### T-54a-028 — Dev-build icon overlay (replaces debug resources)
- Priority: P2
- Effort: S
- Depends: T-54a-012, T-54a-029
- Sources: app/src/debug/res/drawable/ic_launcher_foreground.xml, app/src/debug/res/mipmap/ic_launcher.xml, app/src/debug/res/mipmap/ic_launcher_round.xml
- Acceptance:
  - Dev builds (app id `io.github.<owner>.Kurobayomi.Devel`) use a distinct icon file `io.github.<owner>.Kurobayomi.Devel.svg` with the same red and grey palette as the source vector (`#C92824`, `#455A64`, `#607D8B`).
  - The dev desktop entry shows the name `Kurobayomi (dev)`.
  - The `app/src/debug/res/` tree is not carried over.

### T-54a-029 — Build variants: Devel id, beta channel, benchmark profile
- Priority: P1
- Effort: M
- Depends: T-54a-003, T-54a-013
- Sources: app/build.gradle.kts
- Acceptance:
  - Four variants map to: `debug` to `KB_APP_ID=...Devel` with `dev-tools` feature; `release` to `...Kurobayomi`; `preview` to the Flatpak `beta` branch with `KB_CHANNEL=beta` (About shows `Preview r<count> (<sha>)`); `benchmark` to the `dist` profile with debug symbols kept.
  - Devel and release can be installed side by side (different app ids).
  - Notification and D-Bus names use the variant's app id, not a hard-coded one (BackupCreator-style file name prefix follows the app id, area:21-backup-format-and-create).

### T-54a-030 — Drop the dead variant filter
- Priority: P3
- Effort: S
- Depends: T-54a-029
- Sources: app/build.gradle.kts
- Acceptance:
  - The `androidComponents` block at app/build.gradle.kts:320-334 has no Rust equivalent and is not ported; it referred to flavors that do not exist (p1-behavior.md section 2.1).
  - User impact: none; the code never ran.

### T-54a-031 — gettext catalogues replace the moko resource modules
- Priority: P1
- Effort: M
- Depends: T-54a-001, area:51-i18n
- Sources: i18n/build.gradle.kts, i18n-aniyomi/build.gradle.kts
- Acceptance:
  - `po/kurobayomi.pot` is generated from the merged English strings of both moko modules (857 + 570 strings and 19 + 15 plurals in the base files).
  - The two locale folder sets (67 and 68 folders, the second has `ur`) are merged into `po/<lang>.po`, with folder names mapped as in p1-rust-design.md section 9 (`ka-rGE` to `ka_GE`, `in` to `id`).
  - Android keys are kept as `msgctxt`; plural quantities map to `ngettext` with each language's `Plural-Forms`.
  - `cargo xtask i18n-check` passes (`msgfmt --check` on every `.po`).
  - `gettext-rs` loads the catalogue with domain `kurobayomi` in a test run.

### T-54a-032 — LINGUAS and AppStream languages (replaces LocalesConfigTask)
- Priority: P1
- Effort: S
- Depends: T-54a-031
- Sources: buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt
- Acceptance:
  - `po/LINGUAS` is generated from `.po` files that have at least one translated message; `--include-empty` lists the rest (EC-54-16).
  - The metainfo `<languages>` element is generated from the same list by `cargo xtask dist`.
  - TST-54-04, TST-54-05 and TST-54-06 pass, including the vectors `base`→`template`, `ka-rGE`→`ka_GE`, `zh-rTW`→`zh_TW`.
  - Locales with only `plurals.xml` are reported (EC-54-18).

### T-54a-034 — xtask crate with the Gradle task equivalents
- Priority: P1
- Effort: S
- Depends: T-54a-001
- Sources: build.gradle.kts
- Acceptance:
  - `cargo xtask --help` lists `lint`, `test`, `i18n-check`, `dist`, `flatpak-lint`, `bench-report`, `release-check`, `clean`.
  - `cargo xtask clean` runs `cargo clean` and removes `dist/`, `repo/` and `.flatpak-builder/`; this matches the root `clean` task (build.gradle.kts:14-16).
  - `xtask` depends only on `std` and `anyhow`.

### T-54a-035 — Test configuration and coverage job
- Priority: P2
- Effort: S
- Depends: T-54a-034
- Sources: buildSrc/src/main/kotlin/mihon/buildlogic/ProjectExtensions.kt
- Acceptance:
  - `cargo test --workspace --locked` lists every test name and exits non-zero on any failure.
  - An optional CI job runs `cargo llvm-cov` on `xtask` and `build.rs` helpers with the 80% target in p1-edge-cases-and-tests.md section 7.

### T-54a-036 — Drop buildSrc precompiled plugins
- Priority: P3
- Effort: S
- Depends: T-54a-001
- Sources: buildSrc/src/main/kotlin/mihon.android.application.compose.gradle.kts, buildSrc/src/main/kotlin/mihon.android.application.gradle.kts, buildSrc/src/main/kotlin/mihon.benchmark.gradle.kts, buildSrc/src/main/kotlin/mihon.code.lint.gradle.kts, buildSrc/src/main/kotlin/mihon.library.compose.gradle.kts, buildSrc/src/main/kotlin/mihon.library.gradle.kts
- Acceptance:
  - The `buildSrc/` directory is deleted.
  - The lint rules that these plugins applied are implemented in T-54a-005.
  - User impact: none.

### T-54a-037 — Module graph to crate graph
- Priority: P1
- Effort: S
- Depends: T-54a-001
- Sources: settings.gradle.kts
- Acceptance:
  - Each of the 13 included modules (`:app`, `:core-metadata`, `:core:archive`, `:core:common`, `:data`, `:domain`, `:i18n`, `:i18n-aniyomi`, `:macrobenchmark`, `:presentation-core`, `:presentation-widget`, `:source-api`, `:source-local`) has a crate in p1-rust-design.md section 2 or a drop task.
  - `cargo metadata` lists exactly the crates in that section.

### T-54a-038 — data module to kb-db (rusqlite, two databases)
- Priority: P1
- Effort: S
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: data/build.gradle.kts
- Acceptance:
  - `kb-db` opens two databases: the manga database (package `tachiyomi.data` origin) and the anime database (origin `tachiyomi.mi.data`, schema directory `src/main/sqldelightanime`).
  - The schema version after running all migrations equals the highest `.sqm` number in each directory.
  - The bundled SQLite version is at least 3.38 (the dialect in `sqlite-3-38-dialect`).
  - No GTK dependency is in `kb-db`.

### T-54a-039 — domain module to kb-domain
- Priority: P1
- Effort: S
- Depends: area:07-domain-entries-items, area:08-domain-library-category-history-updates, area:09-domain-source-track-download-release-misc
- Sources: domain/build.gradle.kts
- Acceptance:
  - `cargo tree -p kb-domain` contains no `gtk`, `glib`, or `webkit6` crate.
  - Natural sort uses `natord` (replaces `natural-comparator`).
  - Unit tests for domain interactors compile and run with `cargo test -p kb-domain`.

### T-54a-040 — source-api module to kb-source-api
- Priority: P1
- Effort: S
- Depends: area:15-source-api-manga, area:16-source-api-anime
- Sources: source-api/build.gradle.kts
- Acceptance:
  - `kb-source-api` has no GTK dependency.
  - Source trait names and model names match the Kotlin names in areas 15 and 16 (for example `Source`, `HttpSource`, `AnimeSource`).
  - Decision on the extension ABI is recorded by area:20-extension-strategy-redesign before this crate is merged.

### T-54a-041 — source-local module to kb-local
- Priority: P1
- Effort: S
- Depends: area:14-source-local-and-metadata
- Sources: source-local/build.gradle.kts
- Acceptance:
  - `kb-local` reads a fixture directory of manga folders and anime files without the `ffmpeg-kit` dependency.
  - Media metadata uses the `ffmpeg` CLI or `ffmpeg-next` as decided by area:24-downloads-anime.

### T-54a-042 — core-metadata module to kb-metadata
- Priority: P1
- Effort: S
- Depends: area:14-source-local-and-metadata
- Sources: core-metadata/build.gradle.kts
- Acceptance:
  - `kb-metadata` parses a ComicInfo.xml fixture with `quick-xml` and `serde`.
  - A `details.json` fixture is parsed with `serde_json`.

### T-54a-043 — Drop the Glance home-screen widget (presentation-widget)
- Priority: P3
- Effort: S
- Depends: T-54a-037
- Sources: presentation-widget/build.gradle.kts
- Acceptance:
  - No `kb-widget` crate exists in v1.
  - A KDE Plasma applet or a GNOME extension is recorded as a possible later feature with the same data source (area:42-ui-updates-history-stats-storage-upcoming).
  - User impact: the Android home-screen widget is gone; no Linux widget replaces it in v1.

### T-54a-044 — presentation-core module to kb-ui
- Priority: P1
- Effort: S
- Depends: area:49-theme-and-design-system, area:37-ui-library
- Sources: presentation-core/build.gradle.kts
- Acceptance:
  - `kb-ui` depends on `gtk4` 0.11 with feature `v4_16` and has no libadwaita dependency.
  - Two screens use its helpers (for example the list-row and empty-state widgets) in a test window.
  - Theme colours come from the GTK theme (CSS named colours), not from Kotlin theme constants.

### T-54a-045 — app module to the kurobayomi root crate
- Priority: P1
- Effort: S
- Depends: T-54a-001, T-54a-004
- Sources: app/build.gradle.kts
- Acceptance:
  - The root `Cargo.toml` depends on the crates in p1-rust-design.md section 2 and on the external crates in p1-topic-dependency-mapping.md, and on no Android-only crate.
  - The root package keeps `default-run = "kurobayomi"`.
  - The `main.rs` entry point calls `kb_core::init_logging()` before any GTK call.

### T-54a-054 — cargo-deny policy (replaces the dependency review action)
- Priority: P1
- Effort: S
- Depends: T-54a-001
- Sources: .github/workflows/build_pull_request.yml
- Acceptance:
  - `deny.toml` has the advisories, licenses, bans, and sources sections in p1-rust-design.md section 13.
  - `cargo deny check` fails in CI on a seeded crate with a GPL-3.0-only licence.
  - The LGPL decision for GTK and libmpv is recorded in NOTICE.

### T-54a-055 — Embedded dependency list (replaces include-dependency-info)
- Priority: P2
- Effort: S
- Depends: T-54a-003
- Sources: app/build.gradle.kts
- Acceptance:
  - The release CI build uses `cargo auditable build` (or the equivalent flag).
  - `cargo audit bin <release binary>` reads the embedded list.

### T-54a-056 — Release signing and checksum signature (decision D-2)
- Priority: P1
- Effort: S
- Depends: T-54a-009
- Sources: .github/workflows/build_push.yml
- Acceptance:
  - The chosen tool (GPG by default) signs `SHA256SUMS` as `SHA256SUMS.asc`.
  - The public key is published in the repository and in the README verification section.
  - The secrets `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE` replace `SIGNING_KEY`, `ALIAS`, `KEY_STORE_PASSWORD`, `KEY_PASSWORD`.
  - `gpg --verify SHA256SUMS.asc SHA256SUMS` passes on a downloaded release.

### T-54a-057 — Third-party licence list (replaces aboutLibraries)
- Priority: P2
- Effort: S
- Depends: T-54a-004
- Sources: gradle/libs.versions.toml
- Acceptance:
  - `cargo about generate` produces `data/THIRD-PARTY.html` during `cargo xtask dist`.
  - The About dialog has a "Licences" entry that shows the file (area:36-ui-shell-navigation-security-webview consumes it).
  - The aboutLibraries catalog entries are no longer referenced.

### T-54a-060 — Record drop decisions for Android-only CI steps
- Priority: P3
- Effort: S
- Depends: T-54a-009
- Sources: .github/workflows/build_pull_request.yml, .github/workflows/build_push.yml
- Acceptance:
  - Each step in p1-topic-ci-and-release.md section 1 has a status "kept", "replaced" or "dropped" and no `setup-java` or `setup-gradle` action remains.
  - The mapping step (`upload-artifact` of `app/build/outputs/mapping/release`) is dropped with the user impact "no R8 mapping file to share" (no user impact in practice).
  - The `r0adkll/sign-android-release` step is dropped and replaced by T-54a-056.

### T-54a-061 — Architecture targets (replaces ABI splits)
- Priority: P2
- Effort: S
- Depends: T-54a-013
- Sources: app/build.gradle.kts
- Acceptance:
  - Flatpak builds are produced for `x86_64` (and `aarch64` when D-3 allows).
  - The 32-bit ABIs `armeabi-v7a` and `x86` from the Android splits (app/build.gradle.kts:98) have no build target.
  - The tarball names carry the architecture.

### T-54a-062 — Runtime requirements in README (replaces "Requires Android 8.0")
- Priority: P1
- Effort: S
- Depends: T-54a-019
- Sources: README.md
- Acceptance:
  - README lists GTK 4.16 or newer, libmpv (version confirmed at implementation), glibc, and that SQLite is bundled.
  - The table matches the `gtk4` feature `v4_16` in Cargo.toml and the Flatpak runtime branch.
  - Replaces the source line `Requires Android 8.0 or higher` (README.md:24).

### T-54a-068 — User-Agent for outbound HTTP (replaces Aniyomi user-agent strings)
- Priority: P1
- Effort: S
- Depends: area:12-network-stack, T-54a-002
- Sources: app/build.gradle.kts
- Acceptance:
  - The shared HTTP client sends `Kurobayomi/<CARGO_PKG_VERSION> (Linux; <KB_APP_ID>)`.
  - Tracker clients (area:27-tracking-core through area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin) reuse the constant instead of their own strings (for example the Android format `Aniyomi v${VERSION_NAME} (${APPLICATION_ID})` at AnilistInterceptor.kt:46).
  - A test asserts the header value.
