# Area 54: build, release, distribution and docs (overview)

Area id: `54-build-release-distribution-docs`. Source root: `/home/blackbox/aniyomi-src` (git HEAD `97414446`, tag `v0.18.2.1`). Every path below is repo-relative to that root unless it starts with `plans/` or `/home/blackbox/kurobayomi`. OBSERVED means what the Kotlin, Gradle or YAML does. Proposal means what Kurobayomi should do. Proposal statements are labelled `Proposal:` in the detail docs.

## 1. Purpose and scope

### 1.1 What Aniyomi does in this area

Area 54 holds everything that builds, packages, tests in CI, signs, publishes and documents the Android app. It has no runtime app logic and no UI. The 75 files in scope (see [p1-inventory.md](p1-inventory.md)) are:

- Gradle build logic: `settings.gradle.kts`, root `build.gradle.kts`, `buildSrc/` (precompiled plugins and `mihon/buildlogic` helpers), `gradle.properties`, the five version catalogs under `gradle/`, the Gradle wrapper, and the per-module build files.
- Android packaging: the ABI splits, R8 and ProGuard keep rules, the baseline profile `app/src/main/baseline-prof.txt` (37,790 lines, 3,932,838 bytes), and the macrobenchmark module.
- Localisation build: two moko multiplatform resource modules (`i18n/`, `i18n-aniyomi/`) and the generated locale list.
- Two GitHub Actions workflows (`.github/workflows/build_pull_request.yml`, `.github/workflows/build_push.yml`) that build, sign, rename, hash and create a draft release.
- Store metadata in `fastlane/metadata/android/en-US/`.
- Repository documents: `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `LICENSE`, issue and PR templates, `.editorconfig`, `.gitattributes`, `.gitignore`.

### 1.2 Why it matters for a usable Kurobayomi

Kurobayomi is a Rust and GTK4 (gtk4-rs 0.11, GTK 4.16 minimum, no libadwaita) rewrite. Nothing in this area is a feature a user sees directly, but four things decide whether the app is usable day to day:

1. The workspace, build metadata and release profile. Without `Cargo.toml`, `build.rs` and the `v4_16` feature, no other area compiles. This is P0 (T-54a-001, T-54a-002, T-54a-003).
2. The runtime requirements. The README must state GTK 4.16 or newer, libmpv, and a glibc-based system. The Android "Requires Android 8.0 or higher" line is replaced (T-54a-062).
3. The install path. The user runs CachyOS with KDE Plasma, and wants the host GTK theme (Tokyo Night) applied. Only a native install follows that theme. The Flatpak build uses the runtime theme unless the user adds a filesystem override (see section 4, Q4). Without a desktop entry and icons, the app does not appear in the launcher.
4. The release and CI gate that every other area's code passes through: `cargo xtask lint`, `cargo xtask test`, `cargo deny check`, `cargo xtask i18n-check`.

### 1.3 Explicit scope limits

- Not in this area: the Android Glance widget (dropped here, T-54a-043), the desktop entry content for launcher actions (owned by area 53, see section 5), and the translation catalogue content (area 51).
- Crate-level build logic for each functional crate is owned by its feature area. Area 54 owns the workspace, the lints, the packaging and the release tooling.

## 2. Module map

### 2.1 Kotlin (OBSERVED): Gradle modules to packages to responsibility

Source of truth: `settings.gradle.kts` lists 13 included modules. Namespaces come from each module's build file.

| Gradle module | Namespace (build file) | Responsibility (observed) | Port decision | Rust home |
|---|---|---|---|---|
| `:app` | `eu.kanade.tachiyomi` (`app/build.gradle.kts:17`) | Application shell, version and BuildConfig, ABI splits, shortcuts plugin, depends on every module | Rewrite | root package `kurobayomi` (`src/`) |
| `:core-metadata` | `tachiyomi.core.metadata` (`core-metadata/build.gradle.kts:8`) | Metadata model with serialization (ComicInfo-style data) | Replace-with-crate | `crates/kb-metadata` (area 14) |
| `:core:archive`, `:core:common` | outside this area (build files owned by area 13) | Archive and shared utilities; listed only in `settings.gradle.kts` | owned by area 13 | area 13 crates |
| `:data` | `tachiyomi.data` and `tachiyomi.mi.data` (`data/build.gradle.kts:20-29`) | Two SQLDelight databases: `Database` (schema dir `src/main/sqldelight`) and `AnimeDatabase` (schema dir `src/main/sqldelightanime`), dialect `sqlite-3-38` | Replace-with-crate | `crates/kb-db` (areas 03 and 04) |
| `:domain` | `tachiyomi.domain` (`domain/build.gradle.kts:8`) | Domain models and interactors; exposes the SQLDelight paging extension in its API (`domain/build.gradle.kts:32`) | Replace-with-crate | `crates/kb-domain` (areas 07, 08, 09) |
| `:i18n` | `tachiyomi.i18n`, class `MR` (`i18n/build.gradle.kts:33`) | moko resources: 857 base strings and 19 plurals, 67 locale folders including `base` | Replace-with-crate | `po/` gettext catalogues, `crates/kb-i18n` (area 51) |
| `:i18n-aniyomi` | `tachiyomi.i18n.aniyomi`, class `AYMR` (`i18n-aniyomi/build.gradle.kts:33`) | moko resources: 570 base strings and 15 plurals, 68 locale folders (adds `ur`) | Replace-with-crate | merged into the same `po/` domain (area 51) |
| `:macrobenchmark` | `tachiyomi.macrobenchmark` (`macrobenchmark/build.gradle.kts:6`) | Macrobenchmark startup and baseline-profile tests; the app benchmark variant it needs is disabled (`app/build.gradle.kts:320-334`) | Rewrite as script, then drop | `scripts/bench-startup.sh` (T-54a-023) |
| `:presentation-core` | `tachiyomi.presentation.core` (`presentation-core/build.gradle.kts:8`) | Compose UI component library | Replace-with-crate | `crates/kb-ui` (areas 37, 49) |
| `:presentation-widget` | `tachiyomi.presentation.widget` (`presentation-widget/build.gradle.kts:8`) | Glance home-screen widget | Drop | none in v1; Plasma applet later (T-54a-043) |
| `:source-api` | `eu.kanade.tachiyomi.source` (`source-api/build.gradle.kts:43`) | Public extension ABI (KMP, `androidTarget` only) | Replace-with-crate | `crates/kb-source-api` (areas 15, 16, 20) |
| `:source-local` | `tachiyomi.source.local` (`source-local/build.gradle.kts:44`) | Local manga and anime source, archive and media reading | Replace-with-crate | `crates/kb-local` (area 14) |

Build logic (not a module, but owned by this area):

| Kotlin file | Responsibility | Rust home |
|---|---|---|
| `buildSrc/src/main/kotlin/mihon/buildlogic/AndroidConfig.kt` | SDK constants (compile and target 36, min 26, NDK `27.1.12297006`, JVM 17) | `rust-version`, `gtk4` feature `v4_16`, README requirements |
| `buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt` | Build flags from `-P` properties (`enable-updater`, `disable-code-shrink`, `include-dependency-info`) | Cargo features `updater`, `dev-tools`, profiles |
| `buildSrc/src/main/kotlin/mihon/buildlogic/Commands.kt` | git commit count, git SHA, build time | `build.rs` |
| `buildSrc/src/main/kotlin/mihon/buildlogic/ProjectExtensions.kt` | Shared Android, Compose and test configuration | `[workspace.lints]`, `xtask` |
| `buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt` | Generates `xml/locales_config.xml` from locale folders | `po/LINGUAS` and AppStream `<languages>` |
| `buildSrc/src/main/kotlin/mihon.code.lint.gradle.kts` | Spotless with ktlint 1.7.0 | `rustfmt.toml`, `cargo clippy -D warnings` |
| `buildSrc/src/main/kotlin/mihon.*.gradle.kts` (plugins) | Precompiled plugins for application, library and benchmark modules | dropped (no Gradle) |

Runtime consumers of build metadata in the Kotlin app (outside this area, listed so the Rust replacement covers them):

- `app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt` (`updaterEnabled`, `isDebugBuildType`, `isPreviewBuildType`, `isReleaseBuildType`).
- `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:236-283` (version string formula).
- `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:230,234` (backup file name from the application id; area 21).
- Tracker and HTTP clients send `Aniyomi v${BuildConfig.VERSION_NAME} (${BuildConfig.APPLICATION_ID})` (Bangumi variant at `app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt:38`).

### 2.2 Rust (PROPOSAL): workspace and crate map

Source: [p1-rust-design.md](p1-rust-design.md) section 2. The root package stays at `/home/blackbox/kurobayomi` (it already holds `src/`, `spec/`, `shots/`, `plans/`, `Cargo.toml` with `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde` 1.0.229, `serde_json` 1.0.151).

```
kurobayomi/
  Cargo.toml                 root package kurobayomi, [workspace] members crates/* and xtask
  rust-toolchain.toml        channel stable, components rustfmt and clippy
  rustfmt.toml  deny.toml  build.rs
  src/                       binary: main.rs, ui.rs, screens.rs, settings.rs, ...
  crates/kb-core             paths (XDG), prefs file, tracing logs, build info
  crates/kb-db               rusqlite schema, migrations, repos (two databases)
  crates/kb-domain           interactors and models
  crates/kb-source-api       source traits and models
  crates/kb-local            local manga and anime source
  crates/kb-metadata         ComicInfo and details.json parsing
  crates/kb-net              reqwest client, cookies, rate limiting
  crates/kb-i18n             gettext wrapper
  crates/kb-ui               GTK widget helpers
  xtask/                     lint, test, i18n-check, dist, flatpak-lint, bench-report, release-check, clean
  po/                        kurobayomi.pot, LINGUAS, <lang>.po
  data/                      .desktop, .metainfo.xml, icons (io.github.OWNER.Kurobayomi)
  packaging/flatpak/         manifest and cargo-sources.json
  scripts/bench-startup.sh
  .github/workflows/ci.yml, release.yml
```

Gradle to Rust mapping for the build itself (from [p1-rust-design.md](p1-rust-design.md) section 12): AGP to Cargo, `buildSrc` to `[workspace.lints]` and `xtask`, version catalogs to `[workspace.dependencies]`, `BuildConfig` to `build.rs` with `rustc-env` and `env!()`, `-Penable-updater` to the `updater` feature, build types to profiles plus `KB_APP_ID` and `KB_CHANNEL`, R8 to the `dist` profile, APK signing to GPG-signed `SHA256SUMS`, moko to gettext, `locales_config.xml` to `po/LINGUAS`, fastlane to AppStream and the desktop file, `shortcuts.xml` to `[Desktop Action]` entries, and the dependency review action to `cargo deny`.

Crate names: this map uses the `kb-` prefix. Area 01 uses `kuro-` (see section 4, Q2). The name is unresolved and must be settled before any crate is created.

## 3. Reading guide

All files are in this folder.

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Table of all 75 in-scope files with LOC, top-level declarations, purpose, port decision, Rust target, priority and task ids. |
| [p1-behavior.md](p1-behavior.md) | OBSERVED behaviour in full: build graph, build types, version strings, BuildConfig fields, flags, Android constants, Kotlin options, packaging, lint, tests, both CI workflows step by step, benchmarks, localisation, release process, documentation, source defects, and the proposed Kurobayomi behaviour. |
| [p1-data-and-config.md](p1-data-and-config.md) | Gradle properties, generated BuildConfig constants per build type, packaging lists, all five version catalogs with exact versions, CI environment variables and secrets, artifact file names, external endpoints, fastlane texts, repository config files, and the proposed Linux configuration values (app id, XDG paths, UA, backup name). |
| [p1-topic-ci-and-release.md](p1-topic-ci-and-release.md) | Step-by-step mapping of the two workflows to Kurobayomi steps, the proposed `ci.yml` and `release.yml` jobs, secrets, checksum and signature format, the six release checks, runner and cache policy, and failure modes. |
| [p1-topic-dependency-mapping.md](p1-topic-dependency-mapping.md) | Every Gradle catalog entry and module dependency with a decision (Replace-with-crate, Replace-native, Drop, Owned elsewhere) and the Rust or Linux replacement. |
| [p1-rust-design.md](p1-rust-design.md) | Workspace layout, root `Cargo.toml`, complete `build.rs`, lint configuration, the `xtask` subcommands, the complete Flatpak manifest, desktop entry and metainfo, gettext design, benchmark script, CI outline, `deny.toml`, and pitfalls. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases observed in the Android build, failure modes of the new tooling, race conditions, the fate of each upstream test or benchmark, the new tests TST-54-01 to TST-54-21, and the fixtures they need. |
| [p1-tasks.md](p1-tasks.md) | The 51 tasks of this area (T-54a-NNN) in the required format, with open decisions D-1 to D-4 that block specific tasks. Linted with `tasks.py lint` (result: ok). |

## 4. Key design decisions and open questions

Decisions already proposed in the detail docs (recommendation accepted unless the user objects):

| # | Decision | Chosen in | Why |
|---|---|---|---|
| A-1 | Cargo workspace and `xtask` replace Gradle entirely; no Gradle file survives. | p1-rust-design.md section 3 and 6 | Single toolchain, no JVM. |
| A-2 | `build.rs` emits `KB_GIT_SHA`, `KB_COMMIT_COUNT`, `KB_BUILD_TIME`, `KB_APP_ID` and never panics when git is missing. | p1-rust-design.md section 4 | EC-54-01 and EC-54-02 show the Android version panics. |
| A-3 | Release profile `lto = "thin"`, `codegen-units = 1`, `strip = "debuginfo"`, `debug = "line-tables-only"`, `panic = "unwind"`. Profile `dist` uses fat LTO. | p1-rust-design.md section 3 | Replaces R8. `unwind` keeps the log flush working when a GTK callback panics. |
| A-4 | Updater is the Cargo feature `updater`, off by default, never in the Flatpak build. | p1-rust-design.md section 4 | Flathub policy against self-update inside the sandbox. |
| A-5 | Locales use gettext `po/` with Android keys kept as `msgctxt`; folder names map `ka-rGE` to `ka_GE`, `in` to `id`, `base` to the template. | p1-rust-design.md section 9 | Replaces moko and `LocalesConfigTask`. |
| A-6 | Distribution is a GitHub draft release with per-arch tarballs, a Flatpak bundle, `SHA256SUMS` and a detached signature. | p1-topic-ci-and-release.md section 3 | Keeps the upstream behaviour that a maintainer publishes the draft by hand. |
| A-7 | Store texts move from fastlane to AppStream metainfo and a desktop file under `data/`. | p1-rust-design.md section 8 | Flathub and Linux desktops read AppStream. |
| A-8 | Upstream history is not imported into CHANGELOG; the file keeps the Keep-a-Changelog legend. | p1-tasks.md T-54a-021 | Kurobayomi's own history starts at `v0.1.0`. |
| A-9 | Dropped with no Linux equivalent: APK signing, ABI splits, R8 mapping, baseline profile, macrobenchmark module, Glance widget, Shizuku, LeakCanary, biometric, splash screen, profile installer, RxJava 1, desugaring, Compose stable marker, `google-services.json` and ACRA references. | p1-behavior.md section 16 | Android-only. The user-visible losses are listed there. |

Open questions that need the user's call. Each has a recommendation.

| # | Question | Blocks | Options | Recommendation |
|---|---|---|---|---|
| Q1 (D-1) | GitHub owner segment for `io.github.<owner>.Kurobayomi`. The checkout has no git remote, so it cannot be derived. | T-54a-001, T-54a-011, T-54a-013, T-54a-019, the app id in area 21 backup names, D-Bus names, UA string (T-54a-068) | Any GitHub user or org the maintainer controls | Use the GitHub username that will host the repository. Decide before the first commit, because the id appears in backup file names and desktop registration. Changing it later breaks existing backups and launcher entries. |
| Q2 | Crate prefix: this area proposes `kb-`, area 01 (T-01a-001) and areas 13, 15 and 53 use `kuro-`. | Every crate name in section 2.2, T-54a-037, T-54a-038 to T-54a-045 | `kb-` (this area) or `kuro-` (four other areas) | Adopt `kuro-` for every crate. It matches the product name and four areas already use it. Update p1-rust-design.md section 2 and the crate names in this area's tasks. |
| Q3 | Ownership overlap. Area 01 says "no build.rs in the root; build info lives in kuro-core" (01 p1-rust-design.md line 10) and T-01a-002 owns build identity. This area's T-54a-002 also owns `build.rs`. Area 53 (T-53a-002, T-53a-011, T-53a-041) and this area (T-54a-011) both write the desktop entry. Area 47b (T-47b-049) also lists build metadata via `build.rs`. | T-54a-002, T-54a-011, T-01a-002, T-47b-049 | One owner per file | Area 54 owns the root `build.rs` (it emits the `KB_*` variables). Area 01's `kuro-core` re-exports them through `env!()`. Area 53 owns the desktop entry content (actions, MIME, D-Bus). Area 54 owns packaging, install paths and `desktop-file-validate`. |
| Q4 | Daily driver and theme. The user runs Tokyo Night everywhere (project memory). A Flatpak sees the runtime GTK theme (Adwaita) unless `--filesystem=xdg-config/gtk-4.0:ro` is granted. | T-54a-019 wording, T-54a-013 finish-args | (a) native install as the daily driver, Flatpak for distribution; (b) Flatpak only | Native install for daily use. Add a task to install the binary, `.desktop`, icons and `.mo` files under `~/.local` (no task does this today; see section 6.4). Document the Flatpak theme override as optional. |
| Q5 | ffmpeg and libmpv inside the Flatpak. Area 24 decided the ffmpeg CLI (T-24a-018, T-24a-067 check for a missing binary), and area 45 uses libmpv. The manifest in p1-rust-design.md section 7 builds `mpv` with `-Dlibmpv=true -Dgpl=false` but declares no ffmpeg module, and mpv needs FFmpeg libraries to build. | T-54a-013, T-54a-041, area 24, area 45 | (a) add an LGPL ffmpeg module before mpv; (b) use a Flatpak extension that ships ffmpeg for the chosen runtime; (c) drop video from the Flatpak | Verify at implementation which option the chosen runtime supports. Option (a) keeps the LGPL-only configuration already implied by `-Dgpl=false`. Record the result in p1-rust-design.md section 7. |
| Q6 (D-2) | Signing tool for `SHA256SUMS`: GPG or minisign. | T-54a-056 | GPG (default in the tasks), minisign | GPG. The Flatpak repository signing also uses GPG (`flatpak build-update-repo --gpg-sign`), so one key family covers both. |
| Q7 (D-3) | aarch64 release artifacts in v0.1. | T-54a-061, T-54a-009 | x86_64 only now; aarch64 with an `ubuntu-24.04-arm` runner or QEMU | x86_64 only for v0.1. Say so in the release notes. Add aarch64 when a runner is confirmed. |
| Q8 (D-4) | Contact channel for the Code of Conduct. The upstream Discord is not usable for a fork. | T-54a-017 | a maintainer email, GitHub private vulnerability and conduct reporting | Use the maintainer's email plus the repository's private reporting. Remove every upstream link. |
| Q9 | Translation workflow, replacing Weblate. | T-54a-018, T-54a-032 | Hosted Weblate (accepts gettext `.po`), or PRs against `po/` only | Hosted Weblate, which accepts gettext files and is free for open-source projects. Document in CONTRIBUTING. |
| Q10 | Distribution channels for v1. | T-54a-013, T-54a-009 | GitHub releases only; a self-hosted Flatpak repository; Flathub | GitHub releases with a Flatpak bundle now. Flathub later, once the app id is verified against the owner's GitHub account (check the current Flathub policy before submitting). |
| Q11 | Updater in v0.1. | T-54a-003, T-54a-009 | Off everywhere; on in tarballs only | Off in v0.1. Ship the feature code later, behind the feature flag, when the update source is decided. |
| Q12 | Priority of the launcher files. T-54a-011 (desktop entry and metainfo) is P1, but without it the app has no launcher entry on KDE. | daily-use path | keep P1; promote to P0 | Promote T-54a-011 to P0 when area 53 agrees on its content (Q3). This is a priority decision for the planner, not a change made in this document. |
| Q13 | CI build environment. The `ubuntu-24.04` image is expected to package a GTK 4 release older than 4.16 (4.14 at the time of writing; confirm on the runner). The `gtk4` feature `v4_16` checks the system version at build time, so T-54a-008 fails if `libgtk-4-dev` from that image is used. | T-54a-008, T-54a-009 | (a) build CI jobs in a GNOME 48 SDK container; (b) build GTK 4.16 from source; (c) drop `v4_16` in CI | Option (a), a GNOME SDK container (for example the `flatpak/flatpak-github-actions` image), which also serves the Flatpak job. Verify the image's GTK version in the first CI run. |

## 5. Cross-area dependencies

### 5.1 What this area needs from other areas

| Area | What is needed | Where it is used |
|---|---|---|
| area:01-foundation-app-bootstrap | Workspace ownership (T-01a-001 overlaps T-54a-001); build identity and `AppInfo` (T-01a-002 overlaps T-54a-002); `kb_core::init_logging()` before any GTK call | T-54a-001, T-54a-002, T-54a-045 |
| area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations | Migration count per database and the highest `.sqm` number per directory; the SQL dialect (SQLite 3.38 or newer) | T-54a-038 |
| area:07-domain-entries-items, area:08-domain-library-category-history-updates, area:09-domain-source-track-download-release-misc | Domain crate contents; natural sort requirement (`natord`) | T-54a-039 |
| area:12-network-stack | HTTP client features (`reqwest` with `rustls`, `gzip`, `brotli`, `zstd`, `cookies`), the shared User-Agent constant | T-54a-068, p1-topic-dependency-mapping.md section 1 |
| area:14-source-local-and-metadata | Local source and metadata behaviour; whether media metadata uses the ffmpeg CLI | T-54a-041, T-54a-042 |
| area:15-source-api-manga, area:16-source-api-anime | Trait and model names for `kb-source-api` | T-54a-040 |
| area:20-extension-strategy-redesign | Extension ABI decision, required before `kb-source-api` is merged; the obsolete keep list is recorded there | T-54a-040, T-54a-026 |
| area:21-backup-format-and-create | Backup file name prefix derived from the app id (`io.github.<owner>.Kurobayomi_yyyy-MM-dd_HH-mm.tachibk`) | T-54a-029, T-54a-068 |
| area:24-downloads-anime | Decision that the ffmpeg CLI is the media tool; presence check and error text | T-54a-041, Q5 |
| area:35-data-misc-updater-notification-export | Updater behaviour (feature `updater`); log directory `$XDG_STATE_HOME/kurobayomi/logs`; startup check of the app id and desktop file (T-35a-011) | T-54a-003, T-54a-015 |
| area:36-ui-shell-navigation-security-webview | Consumes `THIRD-PARTY.html` for the Licences entry; WebKit usage for source login | T-54a-057 |
| area:37-ui-library, area:49-theme-and-design-system | GTK widget and theme rules that `kb-ui` implements | T-54a-044 |
| area:51-i18n | String keys, plural rules and the merged English template | T-54a-008, T-54a-031, T-54a-032 |
| area:53-platform-manifest-intents-workers | Desktop entry content: `[Desktop Action]` names and the `--open` argument parser; MIME and D-Bus names | T-54a-011, Q3 |

### 5.2 What other areas need from this one

| Consumer | What it needs | Provided by |
|---|---|---|
| All areas | A compiling workspace, crate names, the `xtask lint` and `xtask test` gates, `cargo deny` license policy | T-54a-001, T-54a-005, T-54a-034, T-54a-054 |
| area:01, area:36 (About dialog) | `env!("KB_GIT_SHA")`, `KB_COMMIT_COUNT`, `KB_BUILD_TIME`, `KB_APP_ID`, `CARGO_PKG_VERSION`; the three-way version string (`Stable`, `Preview r<count> (<sha>)`, `Debug <sha> (<time>)`) | T-54a-002 |
| area:12, areas 27 to 33 (trackers) | One User-Agent constant `Kurobayomi/<version> (Linux; <KB_APP_ID>)` | T-54a-068 |
| area:21 | The application id value | T-54a-029 |
| area:44, area:45 | libmpv and GL requirements in the Flatpak manifest (`--device=dri`); the mpv build options | T-54a-013 |
| area:51-i18n | `po/` layout, `textdomain("kurobayomi")`, `bindtextdomain` path (`/app/share/locale` in Flatpak) | T-54a-031, T-54a-032 |
| area:53 | Flatpak `finish-args` (Wayland, fallback X11, `org.freedesktop.Notifications` talk name, no filesystem access) and the portal-only file dialog rule | T-54a-013 |
| area:01-foundation-app-bootstrap, area:37-ui-library | Startup markers `main_entry` and `window_presented` (area 01) and `first_library_render` (area 37), written when `KB_TRACE_STARTUP=1` | T-54a-023 |
| area:35 | `updater` and `dev-tools` Cargo features | T-54a-003 |

## 6. Task summary

Counts come from `TASKS.csv` rows whose `file` starts with `54-build-release-distribution-docs/` (51 rows). `python3 plans/_tools/tasks.py lint` on [p1-tasks.md](p1-tasks.md) prints `ok`.

### 6.1 Counts

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 3 | 0 | 0 | 0 | 3 |
| P1 | 23 | 6 | 0 | 0 | 29 |
| P2 | 9 | 1 | 0 | 0 | 10 |
| P3 | 9 | 0 | 0 | 0 | 9 |
| **Total** | **44** | **7** | **0** | **0** | **51** |

No task is L or XL, so none needs splitting. Task numbers T-54a-027, T-54a-033, T-54a-046 to T-54a-053, T-54a-058, T-54a-059, T-54a-063 to T-54a-067 and T-54a-069 and above are not defined in this area. They are not referenced by any other document.

### 6.2 P0 critical path (in order)

1. T-54a-001 (S, depends on area:01): root `Cargo.toml` workspace, `rust-toolchain.toml`, and deletion of all nine listed Gradle files.
2. T-54a-002 (S) and T-54a-003 (S), in parallel, both after T-54a-001: `build.rs` metadata and the release profile with features.

P0 is three tasks, so the daily-use path also needs the P1 tasks below. They are not P0 in the file, but without them the binary has no real dependencies, no launcher entry and no working CI gate:

3. T-54a-004 (depends on T-54a-001): workspace dependencies from the catalogs.
4. T-54a-045 (depends on T-54a-001 and T-54a-004): root crate wiring and `kb_core::init_logging()` before any GTK call.
5. T-54a-068 (depends on area:12 and T-54a-002): the User-Agent constant.
6. T-54a-011 (depends on T-54a-001): desktop entry and metainfo (see Q12).
7. T-54a-013 (depends on T-54a-001 and T-54a-011): Flatpak manifest with mpv and cargo sources, once Q5 is resolved.

Release gate, in order: T-54a-034 (xtask), T-54a-005 (lint), T-54a-054 (cargo-deny), T-54a-008 (`ci.yml`), T-54a-020 and T-54a-021 (LICENSE, NOTICE, CHANGELOG), T-54a-009 (`release.yml`), T-54a-056 (signing), T-54a-010 (release check, P2), T-54a-062 (README requirements).

### 6.3 Full task list by priority

| Priority | Tasks |
|---|---|
| P0 | T-54a-001, T-54a-002, T-54a-003 |
| P1 | T-54a-004, T-54a-005, T-54a-006, T-54a-007, T-54a-008, T-54a-009, T-54a-011, T-54a-013, T-54a-017, T-54a-018, T-54a-019, T-54a-020, T-54a-021, T-54a-029, T-54a-031, T-54a-032, T-54a-034, T-54a-037, T-54a-038, T-54a-039, T-54a-040, T-54a-041, T-54a-042, T-54a-044, T-54a-045, T-54a-054, T-54a-056, T-54a-062, T-54a-068 |
| P2 | T-54a-010, T-54a-012, T-54a-015, T-54a-016, T-54a-023, T-54a-028, T-54a-035, T-54a-055, T-54a-057, T-54a-061 |
| P3 | T-54a-014, T-54a-022, T-54a-024, T-54a-025, T-54a-026, T-54a-030, T-54a-036, T-54a-043, T-54a-060 |

### 6.4 Risks and gaps

| Risk or gap | Effect | Mitigation |
|---|---|---|
| The owner segment is undecided (Q1). | Blocks the manifest, metainfo, desktop file and the app id that area 21 uses in backup names. | Decide before the first commit. |
| Crate prefix conflict (Q2) and file ownership overlaps (Q3). | Two workspaces, two `build.rs` files, two desktop entries. | Settle in the planner before any crate is created. |
| CI image has GTK 4.14 (Q13). | `gtk4` with `v4_16` fails to build on `ubuntu-24.04` as written in T-54a-008. | GNOME SDK container for CI. |
| mpv and ffmpeg in the Flatpak (Q5). | The manifest in p1-rust-design.md section 7 cannot build mpv without FFmpeg libraries. | Add an ffmpeg source before mpv. |
| No install path outside Flatpak. | A native CachyOS install has no launcher entry, icons or `.mo` files. There is no task for this today. | Proposed new task (not in TASKS.csv; the planner must assign an ID): `xtask install --prefix ~/.local` that copies the binary, `data/*.desktop`, `data/icons/`, `po/*.mo` and runs `desktop-file-validate`. Depends on T-54a-011 and T-54a-029. |
| Path filters contradict each other. | p1-topic-ci-and-release.md section 2 says `paths-ignore` covers `**.md` and `LICENSE`, p1-rust-design.md section 11 also lists `po/**/*.po` in `paths-ignore` with the opposite note, and T-54a-008 expects a docs-only PR to skip build and flatpak while EC-54-14 expects a Markdown link check. | Recommendation: no `paths-ignore` at workflow level for `po/**` (keep translations triggering `i18n-check`); a `changes` job with a paths filter gates `build` and `flatpak`; a `docs` job runs a link check (`lychee`). Record in both files when they are next edited. |
| Concurrency for tags. | p1-topic-ci-and-release.md section 8 disables cancel for tags on `github.ref`, RC-54-01 uses `release-${{ github.ref_name }}`. | Use the RC-54-01 form: group per tag, `cancel-in-progress: false`. |
| No Kotlin test is in scope for this area. | The TST-54-01 to TST-54-21 tests have no existing counterpart to port. | The implementer writes every test listed in p1-edge-cases-and-tests.md section 5. |
| Release secret handling. | GPG private key and passphrase in GitHub secrets; a leaked key forges releases. | Document rotation in CONTRIBUTING. Keep the key offline except for the release run. |
| Upstream `v4_16` and runtime version drift. | `org.gnome.Platform` branch `48` may be superseded before release. | Verify the newest branch that ships GTK 4.16 or newer at implementation (T-54a-013). |

## 7. Definition of done for this area

The area is done when each of these holds on the CachyOS machine and in CI, and is checked by the command shown.

1. A clean clone with no Gradle files builds with `cargo build --release --locked`. Check: `find . -name 'build.gradle*' -o -name gradlew` prints nothing.
2. `kurobayomi --version` prints `Stable 0.1.0` for a release build, `Preview r<count> (<sha>)` for the beta branch, and `Debug <sha> (<time>)` for a debug build. The `<sha>` matches `git rev-parse --short HEAD` at build time. Check: TST-54-03 and the manual run.
3. `SOURCE_DATE_EPOCH=0 cargo build` gives `KB_BUILD_TIME=1970-01-01T00:00:00Z`. With git removed from `PATH`, the build succeeds with `KB_GIT_SHA=unknown` and `KB_COMMIT_COUNT=0`. Check: TST-54-01 and TST-54-02.
4. `cargo xtask lint`, `cargo xtask test`, `cargo deny check` and `cargo xtask i18n-check` pass on a clean tree, and each fails on a seeded error. Check: T-54a-005, T-54a-054.
5. `cargo build --release` without the `updater` feature has no updater code path. Check: a test asserting `cfg!(feature = "updater")` is false.
6. A release build sends `User-Agent: Kurobayomi/0.1.0 (Linux; io.github.<owner>.Kurobayomi)` on outbound requests. Check: TST for T-54a-068.
7. `desktop-file-validate data/*.desktop` and `appstreamcli validate --pedantic data/*.metainfo.xml` both pass. The desktop file has the four actions Library, Updates, History and Browse, and each action starts the app with `--open <name>`. Check: TST-54-12 and TST-54-13.
8. `po/LINGUAS` lists exactly the `.po` files that have at least one translated message, and the metainfo `<languages>` list matches it. Check: TST-54-04 to TST-54-06.
9. `flatpak-builder --user --install` succeeds, and `flatpak run io.github.<owner>.Kurobayomi --version` prints the same version string as the tag build, with `KB_GIT_SHA` equal to the tag's commit. Check: TST-54-20.
10. A tag push `vX.Y.Z` creates one draft release with the x86_64 tarball, the Flatpak bundle, `SHA256SUMS`, `SHA256SUMS.asc` and `THIRD-PARTY.html`, and the body is the CHANGELOG section for that tag with a valid GFM checksum table. A push from a fork runs the build job and creates no release. Check: T-54a-009 acceptance.
11. `cargo xtask release-check vX.Y.Z` fails with a named message for each of its six conditions, and passes on a correct tree. Check: T-54a-010 with TST-54-19.
12. `gpg --verify SHA256SUMS.asc SHA256SUMS` passes on a downloaded release, and the public key is in the README. Check: T-54a-056.
13. `scripts/bench-startup.sh target/release/kurobayomi 3` writes `target/bench-startup.json` with three runs, and no script or document mentions `baseline-prof`, `devBenchmark` or `macrobenchmark`. Check: TST-54-15 and grep.
14. `README.md` states the Linux requirements (GTK 4.16 or newer, libmpv, glibc, bundled SQLite), the native install, the Flatpak theme note, and the attribution lines from `NOTICE`. `LICENSE` is byte-identical to the source file. Check: T-54a-019, T-54a-020, T-54a-062.
15. `CONTRIBUTING.md` lists only files that exist in the repository. Check: EC-54-26 is fixed (no `google-services.json`, no ACRA).
16. A `.desktop`-driven launch from KDE's launcher starts the installed app with the host GTK theme applied (native install). Check: TST-54-21.
