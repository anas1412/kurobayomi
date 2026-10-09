# Kurobayomi: test and verification strategy

Status: synthesis document. Inputs are every `NN-*/00-overview.md`, the detailed docs they link (`pNN-edge-cases-and-tests.md` in every area, the `pN-tasks.md` files), `catalogs/`, `addenda/`, and the task table `TASKS.csv` (3801 tasks: P0 1196, P1 1793, P2 503, P3 309). Upstream facts were checked against the Aniyomi clone at `/home/blackbox/aniyomi-src`. Kurobayomi facts were checked against `/home/blackbox/kurobayomi` (`Cargo.toml`, `src/*.rs`, `spec/*.json`).

Labels used below: **OBSERVED** = verified in a file on disk. **Proposal:** = what this document recommends. Every area reference is a relative link to that area's overview, and every task is cited by ID from `TASKS.csv`.

---

## 1. Current state (OBSERVED)

### 1.1 Kurobayomi repository

| Item | Observed value | Source |
|---|---|---|
| Crate | `kurobayomi` 0.1.0, edition 2024 | `/home/blackbox/kurobayomi/Cargo.toml` |
| GTK binding | `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }` | `/home/blackbox/kurobayomi/Cargo.toml` |
| Other dependencies | `serde 1.0.229` (derive), `serde_json 1.0.151` | `/home/blackbox/kurobayomi/Cargo.toml` |
| Test code | none. There is no `tests/` directory, no `#[cfg(test)]` module, and no `[dev-dependencies]` | `ls` of the repository |
| CI | none. There is no `.github/` directory in the repository | `ls -la` of the repository |
| Ignore file | `/target` only | `/home/blackbox/kurobayomi/.gitignore` |
| Source files | `src/main.rs` (237 lines), `src/ui.rs` (525), `src/screens.rs` (545), `src/entry.rs` (585), `src/player.rs` (337), `src/settings.rs` (264), `src/spec.rs` (191), `src/clamp.rs` (62) | `wc -l src/*.rs` |
| Spec files | `spec/app.json`, `spec/entries.json`, `spec/reader_player.json`, `spec/settings.json`, `spec/SCHEMA.md` | `ls spec/` |

Dev hooks that already exist in `src/main.rs` and are the starting point for UI smoke testing:

| Variable | Behaviour (as written in the code) | Location |
|---|---|---|
| `KUROBAYOMI_SMOKE` | Opens every screen id from the spec in sorted order, then every sheet and dialog of each screen, then calls `screens::open_entry("Title 1", true)` and `screens::open_entry("Title 1", false)`, prints `smoke: {n} screens, {sheets} sheets, {dialogs} dialogs opened`, and quits. Destroys every other toplevel window after each screen. | `src/main.rs:110-131` |
| `KUROBAYOMI_OPEN=<screen id>` | Starts on that screen with `ui::open(&id)`. | `src/main.rs:92-93` |
| `KUROBAYOMI_SHEET=<sheet id>` | With `KUROBAYOMI_OPEN`, shows the named sheet of that screen. | `src/main.rs:94-96` |
| `KUROBAYOMI_SHOT=<file.png>` | Waits 1200 ms, renders the window to a PNG with `gtk::WidgetPaintable` and `render_texture`, then quits. | `src/main.rs:98-108` |
| `KUROBAYOMI_SIZE=WxH` | Sets the default window size. Defaults in the parser are 1180 and 780. | `src/main.rs:89-91` |
| `KUROBAYOMI_RAIL` | Starts with the sidebar in rail (collapsed) mode. | `src/main.rs:233-235` |
| Non-unique flag | If any of `KUROBAYOMI_SMOKE`, `KUROBAYOMI_OPEN` or `KUROBAYOMI_SHOT` is set, the application runs with `ApplicationFlags::NON_UNIQUE`, so a dev run never hands off to an open window. | `src/main.rs:15-17` |

Limits of the current hooks (OBSERVED): the smoke run has no assertions. It passes if the process does not crash, and it does not fail on GTK critical warnings. The screenshot hook has no reference image and no comparison.

### 1.2 Aniyomi upstream tests (OBSERVED)

Upstream has 8 unit-test files. There are no instrumented tests (`androidTest` contains no `.kt` files). The `@Test` counts are grep counts; parameterised tests may expand to more cases.

| Upstream file (repo-relative) | `@Test` count | Port target in Kurobayomi | Port tasks |
|---|---|---|---|
| `app/src/test/java/mihon/core/migration/MigratorTest.kt` | 6 | Migration framework in `kuro-core` (area:11-app-migrations, area:01-foundation-app-bootstrap) | T-11a-001, T-11a-003 |
| `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt` | 8 | Missing-item gap computation (area:07-domain-entries-items, area:43-reader-core) | No dedicated task. Covered by the T-07a-034 builders and the area:07 suite (T-07a-010, T-43a-021 for the gap rule) |
| `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt` | 5 | Library flag algebra (area:08-domain-library-category-history-updates) | T-08a-002, T-07a-035 |
| `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt` | 4 | Release check (area:35-data-misc-updater-notification-export, area:09) | T-35a-094, T-09a-057 |
| `domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt` | 10 | Anime fetch interval (area:26-library-update-jobs, area:07) | T-26a-074, T-07a-019 |
| `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt` | 10 | Manga fetch interval (area:26-library-update-jobs, area:07) | T-26a-074, T-07a-019 |
| `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` | 34 | Chapter number recognition (area:07) | T-07a-010 |
| `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt` | 1 | Torrent helpers (area:13-core-util-storage-archive-torrent) | T-13a-058 |

Upstream test resources: `core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-directory.torrent` and `test-torrent-file.torrent` (both needed by T-13a-058).

Upstream CI (OBSERVED, `.github/workflows/build_pull_request.yml`): runs on `ubuntu-24.04`, then steps `./gradlew spotlessCheck` (line 46), `./gradlew assembleRelease` (line 49), `./gradlew testReleaseUnitTest` (line 52), dependency review, and APK and mapping upload. The release workflow is `.github/workflows/build_push.yml`. The Kurobayomi replacement CI is designed in [54-build-release-distribution-docs/p1-topic-ci-and-release.md](54-build-release-distribution-docs/p1-topic-ci-and-release.md) (T-54a-008, T-54a-009).

### 1.3 Edge-case and test-plan documents

Every one of the 54 area folders has a `p1-edge-cases-and-tests.md` (several have `p2-` twins). Each file has the same shape: upstream tests, an edge-case table with IDs `E-nn`, race conditions, failure modes, a numbered test plan, a list of new tests, and a fixture table. Test IDs are area-local (`TC-REPO-*`, `TST-19-012`, `TST-54-01`, `EC-38`, `E-SL`). This document keeps them as they are and adds a global index in section 9.

---

## 2. Test layers

Layers run in this order in CI and are gated as described in section 8.

| Layer | Name | What it covers | Runner | Speed budget | Examples in the area docs |
|---|---|---|---|---|---|
| L0 | Pure unit | Pure functions in `kuro-core`, `kuro-domain`, `kuro-source`, `kuro-track`, and the view-model code in the GTK crate that does not touch GTK | `cargo test --lib` | whole layer under 60 s | T-07a-010, T-08a-002, T-44a-080 (zoom mathematics), T-15a-012 (source id) |
| L1 | Property and golden | Invariants over random inputs; byte-exact outputs compared with checked-in files | `cargo test`, `proptest` (dev-dependency, Proposal) | under 120 s | T-07a-043, T-44a-081, T-34a-069, T-19a-068 (protobuf golden and fuzz) |
| L2 | Database integration | SQL semantics, triggers, migrations, schema equality, golden dumps. In-memory SQLite and temp-file SQLite through `rusqlite` | `cargo test` in `kuro-db` and the data crates | under 180 s | T-03a-020, T-03a-021, T-04a-026, T-04a-027, T-05a-023, T-06a-005 |
| L3 | HTTP and network integration | Source and tracker clients against a fake HTTP server; retries, challenges, auth. Fake server: `wiremock` (T-30a-033 names it; Proposal for the crate choice) | `cargo test --test '*'` | under 240 s | T-12a-042, T-12a-043, T-29a-043, T-30a-033, T-33a-102 |
| L4 | Filesystem, archive and image | Local source scanning, download folders, archives (ZIP, CBZ, EPUB), image decode and transforms | `cargo test`, `tempfile` | under 120 s | T-14a-052, T-23a-036, T-34a-069, T-43a-083 |
| L5 | GTK widget (headless) | Widgets built and measured under a real GTK display; snapshot checks; mapping of spec screens | `xvfb-run -a cargo test` (CI); the GTK test harness is T-47ba-071 | under 300 s | T-43a-085, T-47ba-071, T-40b-085 |
| L6 | UI smoke | Every screen, sheet and dialog opens with no panic and no GTK critical (`KUROBAYOMI_SMOKE`) | `xvfb-run` and the binary with `KUROBAYOMI_SMOKE=1` | under 120 s | Section 5 |
| L7 | Visual reference | Rendered PNG per screen compared with checked-in references (`KUROBAYOMI_SHOT`) | `xvfb-run` and the binary; comparison tool in `xtask` (Proposal) | under 180 s | Section 5.3 |
| L8 | Player integration | libmpv property and command bridge, key table, screenshot path, process restart snapshot | `cargo test` with a fake `ffmpeg` and a headless mpv build | under 240 s | T-45b-062, T-24a-061 |
| L9 | Packaging and release checks | Desktop file, AppStream, MIME, Flatpak manifest, tarball contents, checksum format, i18n check | `cargo xtask` plus validators | under 300 s | TST-54-10 to TST-54-14 |
| L10 | Manual QA | Behaviour that needs a real account, a real source, a real device, or a real GPU | Checklists in section 8 | per release | T-29a-058, T-32a-069, TST-54-20, TST-54-21 |

Test IDs follow the pattern already used in area 54: `TST-<area>-<nn>` with the area number as written (for example `TST-19-012`, `TST-54-01`). New IDs in this document use `TST-G-<nn>` for global checks in section 9.

---

## 3. Shared test kit

The kit is one workspace crate, `kuro-testkit`, planned by T-01a-001 (workspace) and built by T-01a-045. It must not be a dependency of any production crate. Items below are the names used by the area docs; each area cites the one it needs.

| Item | Purpose | Defined in | Used by (examples) |
|---|---|---|---|
| `TempPaths` | `Paths` trait implementation rooted in a `tempfile::TempDir`, one subdirectory per `Paths` method (data, config, cache, log, temp, downloads, backups, covers) | T-01a-045 | area:01, area:23, area:34, area:21 |
| `InMemorySettings` / `Prefs::in_memory()` | In-memory preference store for tests and previews | T-13a-003, T-02a-033 | area:02, area:08, area:21 |
| `TestDb::manga()`, `TestDb::anime()` | In-memory SQLite with pragmas as in production, schema from `schema.sql` and `views.sql`; `TestDb::anime_file(dir)` for hook and WAL tests | area:04 (in `p1-edge-cases-and-tests.md` section 4) | area:04, area:05, area:06 |
| `Db::open_temp(DbKind, &SCHEMA)` | Temp-file database for trigger and concurrency tests | area:06 test kit | area:06 |
| `Fixture::manga(&db)` and builders | Builder methods that default every NOT NULL column so tests state only what matters (`f.manga("One Piece").favorite(true).insert()`) | area:06 (`p1-edge-cases-and-tests.md`) | area:06, area:08 |
| `AnimeBuilder`, `EpisodeBuilder`, `HistoryBuilder`, `CategoryBuilder`, `TrackBuilder` | Anime builders. Defaults match `Anime::create()` (`id -1`, `fetch_type Episodes`, `season_number -1.0`) and `Episode::create()` (`episode_number -1.0`, `date_upload -1`, `version 1`) | area:04 | area:39, area:45 |
| `kuro_core::db::testkit` / `kuro-testkit::domain::{manga(), anime(), chapter(n), episode(n), season(title, number)}` | Domain builders | T-07a-034 | area:07 |
| `ItemDates::every_n_days(n, count)`, `every_n_hours(h, count)` | Generators matching the upstream helper `withTime` | T-07a-034 | area:07, area:26 |
| `FixedClock { now_ms, zone }` and `FakeClock` | Deterministic time. Zones used: UTC and `Europe/Berlin` (`chrono-tz` as dev-dependency only) | area:07, area:08 | area:07, area:08, area:26 |
| `FakeTracker` | Implements `AnimeTracker` and `MangaTracker`; scripted responses; records call order; `valCompletionStatus = 2`, `valScoreList = "0".."10"`, `valStatuses = 1..6` (mirrors `app/src/main/java/eu/kanade/test/DummyTracker.kt:14-51`) | area:27 | area:27, area:28 to area:33 |
| `FakeTrackRepo` | In-memory `AnimeTrackRepo` and `MangaTrackRepo` with the upsert rules | area:27 | area:27 |
| `FakeSource` (implements `AnimeSource`) | Scripted popular, latest and search pages; `has_next_page` per page; `filter_list` with every `Filter` variant; `supports_latest` configurable; `SourceError` per call | area:41 | area:40, area:41 |
| `FakeCatalog` (implements `SourceCatalog`) | Installed sources: local (id 0, lang `other`, name `Local anime source`), `en`, `all`, and others | area:41 | area:40, area:41 |
| `FakeScheduler`, `FakePowerNetworkState`, `MemoryRawPrefStore` (BTreeMap) | Scheduler, power and network state fakes | area:11 | area:11, area:26 |
| `FakeNotifier` | Records notifications; used for router and progress tests | T-35a-098 | area:35, area:53 |
| `FakeDownloadIndex`, `FakeSourceLookup`, `FakeTrackScores` | Fakes for the library and domain layers | area:08 | area:08 |
| `captured-logs helper` | `tracing-subscriber` with a `Vec<String>` writer, for asserting log lines | area:11 | area:01, area:11 |
| `fake_ffmpeg.sh` | Shell script that records its arguments and writes a fixed output file; used on `PATH` for the download engine and the screenshot path | `fixtures/ffmpeg/fake_ffmpeg.sh` (area:24) | area:24, area:45 |
| `hickory-proto` (dev-dependency) | Builds DNS fixtures for the network stack | area:12 | area:12 |
| `webkit6` plus a display | WebKit integration tests run only in the `xvfb` job | area:12 | area:12, area:36 |

Rules for the kit (Proposal):

1. No test calls the network. Every HTTP test uses a local fake server and a fixture body. A test that needs the real internet is marked `#[ignore = "live"]` and runs only in the manual checklist (section 8).
2. No test writes outside its `TempPaths`. Tests that use the real XDG directories fail in CI. The check is the `Paths` trait itself, so no test can reach a real home directory.
3. Clocks are injected. No test reads `SystemTime::now()` directly.
4. Golden updates go through one command, `UPDATE_GOLDEN=1 cargo test`, and the diff is reviewed in the pull request.

---

## 4. Fixtures and golden files

### 4.1 Fixture policy

| Policy | Rule |
|---|---|
| Generated, not stored | Binary images, fonts and archives are generated by test helpers from a seed (the area:43 and area:44 approach). A generated fixture is reproducible with the committed generator. |
| Stored when small | Text fixtures (JSON, SQL, XML, Markdown, TOML) are stored. Any single file is under 100 KiB (the area:34 rule). |
| No copyrighted content | Manga pages are never stored. Page images are synthetic (area:44 "the repository holds no copyrighted pages"). |
| Real captures are labelled | Fixtures copied from real responses record the capture date and the command used. Example: `github_latest_aniyomi.json` captured with `gh api repos/aniyomiorg/aniyomi/releases/latest` on 2026-10-08 (area:35). |
| Hand-written fixtures are labelled | Tracker fixtures written from DTO shapes are not live data. Area:29 says "Record real responses from a test account before relying on them (task T-29a-041)". |
| Backup fixtures are binary | `.tachibk` fixtures are generated by the creator (area:21) and checked against the wire format; they are never edited by hand. |

### 4.2 Fixtures by area (planned paths, OBSERVED from area docs)

Paths are as proposed in each area. The owning task creates the file.

| Area | Fixture paths (as named in the area doc) | Content summary | Tasks |
|---|---|---|---|
| area:01-foundation-app-bootstrap | `fixtures/settings/valid.toml`, `corrupt.toml`, `unknown_keys.toml`; `fixtures/db/v0_empty.db`, `fixtures/db/too_new.db` (`PRAGMA user_version = 9999`); `fixtures/crash/valid.txt`, `unknown_magic.txt`, `huge.txt` (2 MiB, generated); `fixtures/logs/mixed.log` (1000 lines, 600 ERROR) | Settings, DB version guard, crash report and log rotation | T-01a-045, T-01a-048 |
| area:02-preferences-system | `fixtures/config-default.toml`, `config-corrupt.toml` (truncated at `[library`), `config-wrongtypes.toml` (`auto_update_interval = "12"`, `show_nsfw_source = 3`), `config-future.toml` (`[experimental]` table, unknown `[ui]` key) | Preference load, repair and forward compatibility | T-02a-033, T-02a-030 |
| area:03-db-manga-schema-migrations | `crates/kuro-db/tests/fixtures/aniyomi_fresh_v33.sql` (ends with `PRAGMA user_version = 33;`), `legacy_v13.sql` | Fresh schema and legacy replay | T-03a-021, T-03a-022 |
| area:04-db-anime-schema-migrations | Test kit and golden dumps | Fresh versus upgraded anime schema | T-04a-027, T-04a-026 |
| area:06-data-repos-history-track-source-misc | `kuro_core::db::testkit` builders (no files) | History, track, source rows | T-06a-005 |
| area:07-domain-entries-items | Recognition vectors as Rust constant arrays; builders | Chapter and episode recognition | T-07a-010, T-07a-011, T-07a-034 |
| area:08-domain-library-category-history-updates | `kuro-domain/tests/fixtures/library_small.sql` (4-entry library and categories), `history_basic.sql`, `updates_window.sql`, `tests/golden/sort_orders.json` | Library sort and filter, history, updates window | T-08a-040 |
| area:09-domain-source-track-download-release-misc | `fixtures/repo.json` (`signingKeyFingerprint` `abc123`), `repo_nofingerprint.json` (`NOFINGERPRINT-1`), `store_current.json` | Repository and store decoding | T-09a-058 |
| area:10-app-domain-layer | `fixtures/chapters_worked_example.json` (and episode twin), `fixtures/sources.json` (30 sources over 8 languages), `expected_enabled_sources.json`, `extensions.json` (installed 6, untrusted 2, available 10), `java_ci_order.json` | Enabled-source pipeline, extension ordering | T-10a-036 |
| area:11-app-migrations | `tests/fixtures/legacy/*.json`, `tests/fixtures/lang_codes.json` (pairs `(code, jdk_valid)` from a JDK `jshell` run; rows not yet produced are tagged `jdk-unverified`) | Migration steps, language code predicate | T-11a-003, T-11a-028 |
| area:12-network-stack | Fake server bodies; `hickory-proto` builders | Challenge, retry, DNS | T-12a-042, T-12a-043 |
| area:14-source-local-and-metadata | Generated trees in `TempDir`: `manga_tree/` (`Foo/Ch 01/` with 3 jpg, `Foo/Ch 02.cbz` with 2 jpg, `Foo/Ch 03.epub`, `.hidden/`, `Bar/` with only `cover.png`), `anime_tree/` (`Show/Ep 01.mkv`) | Local source browse, chapters, cover | T-14a-052 |
| area:15-source-api-manga | `fixtures/source/list_popular.html` (3 cards, `li.next a`), `list_popular_no_next.html`, `list_empty.html`, `details.html` (genres with `, ` separators) | HTML source parsing | T-15a-048, T-15a-031 |
| area:16-source-api-anime | Test corpus for the anime source API | Listing, detail, filters | T-16a-051 |
| area:17-source-app-layer | `fixtures/sources/manga_rows.json` (6 rows, plus 5 rows), `stub_rows.sql` (`INSERT INTO sources VALUES (10,'en','Foo'),(20,'','Name'),(21,'en',' '),(999,'ja','Bar')`), `fixtures/remote/sanime_*.json` | Source registry, stubs, remote updates | T-17a-038 |
| area:18-extensions-manga | `fixtures/index/valid.min.json` (three valid entries; one with `sources`, one with `nsfw: 1`, one with lib 1.5), `mixed.min.json` (one missing `pkg`, one with `code` as a string, one with `version: "abc.1"`), `not-array.json` (`{ "a": 1 }`), `fixtures/repo/repo.json` (64-hex fingerprint) | Extension index validation | T-18a-045 |
| area:19-extensions-anime | `tests/fixtures/anime_ext/store_ok.json`, `store_zero_sources.json`; generated `.kuroext` packages from `tests/support/build_kuroext.rs` | Store decoding, container, verification | T-19a-061, T-19a-068 |
| area:21-backup-format-and-create | Seed SQL for the in-memory DB; fake registries (sources installed and stub, trackers logged in and not, extension artifacts present and missing, prefs with `__APP_STATE_` and `__PRIVATE_` keys); clock fixture | Backup creation | T-21a-040, T-21a-034 |
| area:22-backup-restore | `fixtures/restore/mihon_1x_small.tachibk` (Q-01: 3 manga, 1 category, 6 chapters, 1 history, 1 track), `fixtures/restore/aniyomi_legacy_0x.tachibk` (Q-02: anime 2, manga 1, fields 1-4 and 103 populated) | Restore counts and legacy decode | T-22a-042, T-22a-051 |
| area:23-downloads-manga | `fixtures/img/tiny.jpg` (8x8 JPEG, q75), `tiny.png`, `tiny.webp`, `tiny.gif`, `animated.gif` (2 frames, 16x16), `tall_800x4000.png`, `broken.jpg` (JPEG header then garbage); `fixtures/index/tree/` with `MangaDex (EN)` and `mangadex` | Download index and image conversion | T-23a-036 |
| area:24-downloads-anime | `fixtures/dl/tree_new/` (`Example (EN)/Show/Scan_Episode 1/Episode 1.mkv` and `.nomedia`), `tree_legacy/` (`Scan_Episode 2/`), `tree_tmp/` (`Episode 3_tmp/Episode 3.tmp`, `Episode 4_tmp/Episode 4.mkv`), `tree_case/` (`EXAMPLE (en)` and `example (en)`), `fixtures/ffmpeg/fake_ffmpeg.sh` | Anime download layout and rename | T-24a-061 |
| area:26-library-update-jobs | `fixtures/library_small.json` (12 manga, 3 categories, 2 excluded, 1 completed, 1 `ONLY_FETCH_ONCE`, 1 with 0 chapters, 1 in two categories), `library_anime_seasons.json` (3 anime), `chapters_newest_first.json` (30 chapters over 60 days) | Library update selection | T-26a-074, T-26a-089 |
| area:27-tracking-core | `FakeTracker`, `FakeTrackRepo` (no files) | Tracking rules | T-27a-065 |
| area:28-tracker-anilist | `tests/fixtures/anilist/` JSON bodies: `viewer_point10.json` (`{"data":{"Viewer":{"id":4242,"mediaListOptions":{"scoreFormat":"POINT_10"}}}}`), `viewer_point3.json`, `viewer_point_decimal.json` (`POINT_10_DECIMAL`), `viewer_unknown_format.json` (`"SOMETHING_NEW"`), search fixtures | AniList DTOs and score formats | T-28a-038 |
| area:29-tracker-myanimelist | `crates/kuro-track/tests/fixtures/mal/`: `oauth_token.json` (`token_type` Bearer, `expires_in` 2592000, no `created_at`), the remaining DTO fixtures named in area:29 `p1-edge-cases-and-tests.md` | MAL OAuth and DTOs | T-29a-041, T-29a-043 |
| area:30-tracker-kitsu | `src/track/kitsu/tests/fixtures/`: `token_ok.json` (`created_at` 1700000000, `expires_in` 2592000, `scope` public); values are fixture values, not Kitsu constants | Kitsu OAuth and JSON:API | T-30a-033 |
| area:31-tracker-shikimori-bangumi | `crates/kuro-track/tests/fixtures/shikimori/` (`oauth_token.json`, `oauth_token_no_refresh.json`, `whoami.json`) and `bangumi/` | Shikimori and Bangumi | T-31a-019, T-31a-036 |
| area:32-tracker-mangaupdates-simkl | `crates/kuro-track/tests/fixtures/mangaupdates/`: `list_item_reading.json`, `list_item_null_status.json`, `rating_7_5.json`, `rating_null.json`, `login_ok.json` (`context.session_token`, `uid` 42), `login_missing_context.json`, `search_ok.json` | MangaUpdates and Simkl | T-32a-061, T-32a-069 |
| area:33-tracker-enhanced | `kavita/series_ok.json`, `series_thumbnail_null.json`, `series_empty.json` (`pages: 0`), `volumes_mixed.json`; the Komga, Suwayomi and Jellyfin fixture sets are listed in area:33 `p1-edge-cases-and-tests.md` | Enhanced trackers (wire) | T-33a-013, T-33a-102 |
| area:34-images-covers-cache | `kuro-images/tests/fixtures/`: F-01 `jpeg_baseline_320x480.jpg`, F-02 `jpeg_exif_rot90.jpg` (480x320, EXIF orientation 6), F-03 `png_rgba_100x100.png` (alpha gradient); the remaining F-nn files are listed in area:34 `p1-edge-cases-and-tests.md`; each under 100 KiB, generated by a committed script | Decode, orientation, cache | T-34a-069, T-34a-070 |
| area:35-data-misc-updater-notification-export | `crates/kuro-platform/tests/fixtures/github_latest_aniyomi.json` (captured, tag `v0.18.2.1`, five assets), `tests/fixtures/github_latest_kurobayomi.json` (synthetic), CSV golden | Release check, notifications, CSV export | T-35a-096, T-35a-095 |
| area:36-ui-shell-navigation-security-webview | `tests/fixtures/changelog_with_checksums.md`, `cloudflare_challenge.html` (contains `window._cf_chl_opt`), `cloudflare_ray.html` (contains `Ray ID is` only), `plain_page.html` | Changelog parse, Cloudflare detection | T-36a-067 |
| area:38-ui-entry-details-manga | `fixtures/manga_basic.json` (30 chapters, numbers 1..30 with 5 gaps), `manga_desc_asc.json`, `manga_unicode_titles.json`, `numbers.txt` (double literals for formatting), `chapters_mixed.json` (50 chapters, numbers 1..=40 with 13 and 14 missing, 20.5, -1, scanlators "Alpha", "Beta" and null), `manga_flags.json` (27 combinations), `track_dates.json` | Details pipeline, formatting, flags | T-38a-051, T-38b-041 |
| area:39-ui-entry-details-anime | F-1 `anime_seasons_sample` (season numbers 1, 2, 2.5, 3, 4), F-2 `anime_episodes_gap` (`[1, 2, 4, 4.5, 7, -1]` plus 16.5), F-3 `anime_episodes_50`; `episodes_gap.json`, `episodes_upload.json`, `episodes_names.json` | Episode list, seasons, separators | T-39a-051, T-39b-062 |
| area:40-ui-browse-manga | `sources_mixed` (9 sources), `langs_equal_display` (`xx-A`, `xx-B`), `filters_genre` | Browse source list and filters | T-40a-038, T-40b-084 |
| area:41-ui-browse-anime | `ext_installed.json` (4 installed), `ext_available.json` (5 available), `FakeSource`, `FakeCatalog` | Sources list, pager, filters, global search | T-41a-049, T-41b-047 |
| area:42-ui-updates-history-stats-storage-upcoming | `tests/fixtures/updates_upcoming.sql` (M1 favourite, C1..C3 chapters, E-U1 and E-U2 cases); history fixtures derived from the Kotlin previews (`MangaHistoryWithRelationsProvider.kt:9-58`, `AnimeHistoryWithRelationsProvider.kt:9-55`) | Updates, history, calendar | T-42a-060, T-42b-044 |
| area:43-reader-core | `fixtures/reader/page_1x1.png`, `page_800x600.jpg`, `page_1000x2000.png`, `page_2000x1000.png`, `page_5000x7000.png` (tiling and cap), `anim_3frames.gif` (delays 100, 200, 300 ms), `corrupt.jpg` (first 100 bytes of a JPEG), `empty.bin`; `fixtures/prefs/legacy_kv.sql` | Reader loader, pages, legacy prefs | T-43a-083, T-43a-084 |
| area:44-reader-viewers | `tests/fixtures/reader/spread_2to1.png` (200x100, left red, right blue), `spread_odd_101.png` (101x50), `tall_strip_1x20000.png`, `tiny_49x2.png` (49x2 image; the area:44 table gives its colour pattern) | Viewer layout, split, zoom | T-44a-081 |
| area:45-player-core | `tests/fixtures/mpv/golden_defaults.txt` (43 settings, `kind<TAB>name<TAB>value`), `golden_debanding_cpu.txt` (`vf` = `gradfun=radius=12,format=yuv420p`), `tests/fixtures/keys/gtk_keys.txt`, `tests/fixtures/bridge/*.txt`, `fixtures/anime_flags.json`, `episodes_20.json`, `tracks_mixed.json` | mpv options, key table, track selection | T-45b-062 |
| area:46-player-controls-ui | `tests/fixtures/fonts/Sample-Regular.ttf` (family `Kurobayomi Sample`, generated with `fontTools.fontBuilder`, CC0), `Broken.ttf` (16 bytes of `0x00`), inline `hwdec` strings `"no"`, `"vaapi-copy"`, `"vaapi"`, `"nvdec-copy"` | Font loading, hwdec choice | T-46a-056, T-46b-045 |
| area:47-settings-screens-general | `tests/fixtures/settings/backup/complete.tachibk`, `missing-numeric.tachibk`, `empty-message.tachibk` | Settings and backup integration | T-47b-053 |
| area:49-theme-and-design-system | `fixtures/schemes/*.json` (18 schemes, 36 base tokens each), `fixtures/monet/vectors.json`, `fixtures/kdeglobals/*.ini` (`AccentColor`), `tests/fixtures/css/*.css` (golden CSS), `tests/fixtures/vectors.json` | Colour schemes and generated CSS | T-49a-059, T-49b-038 |
| area:49b-android-res-values-drawables | `tests/fixtures/android/` (4 colour XML files, `themes.xml`, 18 drawables exported to SVG) | Palette matrix and SVG export | T-49ba-044 |
| area:50-android-widgets | `fixtures/feed-basic.sql` (3 favourites with 2 unread each; 1 non-favourite; 1 with `date_fetch == date_added`; 1 older than the window; 1 read), `feed-100.sql`, `feed-500.sql` (600 unread over 3 manga) | Feed query, dedupe, window, performance | T-50a-035 |
| area:51-i18n | `xtask/tests/fixtures/base/strings.xml`, `plurals.xml` (40 entries with escapes, `translatable="false"`, `%2$d`, `%02d`, `%.2f`), `locales/ru/`, `locales/ur/` | i18n check and importer | T-51a-028, T-51a-029 |
| area:52-util-app | `anilist_airing_ok.json`, `anilist_airing_null.json`, `anilist_mal_ok.json`, `anilist_mal_null.json`, `simkl_s3_mal_null_then_mal.json` | Airing and MAL mapping | T-52a-064 |
| area:53-platform-manifest-intents-workers | `fixtures/argv/*.txt` (one argument list per line), `fixtures/uri/*.txt`, `fixtures/notice/*.json`, `fixtures/progress/*.csv` | Routing, notices, progress | T-53a-072 to T-53a-075 |
| area:54-build-release-distribution-docs | `xtask/tests/fixtures/po/` (`de.po` complete; `fr.po` with bad header), `LINGUAS` with an extra code, `CHANGELOG.md` (`v0.1.0`, `v0.1.1`), `locales/` (`base`, `ka-rGE`, `nb-rNO`, `pt-rBR`, `zh-rCN`, `zh-rTW`, `in`), `scripts/tests/stub-*` | Build metadata, locales, changelog, checksums | T-54a-035, TST-54-04 to TST-54-10 |
| addenda/requirements-cross-platform.md | No fixtures. The Windows check is a build, not a data test. | Cross-compile check | T-req-platform-010 |

Note on area:01 "Crash test hooks" (T-01a-048): crash fixtures must exercise the reporter in its dry-run mode, so the reporter never writes to a real log directory during tests.

### 4.3 Golden files

A golden file is a checked-in expected output. Each golden has a generator or a documented way to regenerate it, and a test that fails on any byte difference.

| Golden | Path (planned) | What it pins | Owner task | Regenerate with |
|---|---|---|---|---|
| Chapter number recognition | the 34 upstream cases as a Rust table (`T-07a-010`) | Upstream `ChapterRecognitionTest` expectations | T-07a-010 | edit the table; review in PR |
| Sort orders | `kuro-domain/tests/golden/sort_orders.json` | Library and chapter sort results per sort type and direction | area:08 | `UPDATE_GOLDEN=1` |
| Protobuf backup decode | area:19 `T-19a-068` golden and area:21 wire golden | Wire bytes of `Backup` | T-19a-068, T-21a-034 | generator in `tests/support/` |
| Scheme colour tokens | `fixtures/schemes/*.json` generated by `p1-tool-scheme_tables.py` (area:49) | 18 schemes, 36 tokens | T-49a-059 | run generator; output is source of truth |
| Monet vectors | `fixtures/monet/vectors.json` | Seed, dark, contrast, 36 role values | T-49a-059 | generated once with the Android library |
| CSS output | `tests/fixtures/css/*.css` (area:49 section 4.6) | Generated CSS for theme | T-49b-038 | `UPDATE_GOLDEN=1` |
| Decode transforms and borders | area:44 `T-44a-081` | Pixels after transform | T-44a-081 | `UPDATE_GOLDEN=1`, pixel diff tolerance 0 |
| Image decode | area:34 `T-34a-069` | Decoded RGBA hash for each F-nn fixture | T-34a-069 | committed generator |
| mpv defaults | `tests/fixtures/mpv/golden_defaults.txt` (area:45) | 43 settings, one per line | T-45b-062 | reviewed manually |
| CSV export | area:35 `T-35a-095` | Exact CSV bytes | T-35a-095 | `UPDATE_GOLDEN=1` |
| Locale mapping | area:54 `TST-54-04` | `base` to `template`, `ka-rGE` to `ka_GE` and others | TST-54-04 | vectors in the test |
| Date formatting | area:38 `E-07` numbers, area:42 calendar | Formatted numbers and dates under two timezones | T-28a-050 (dates) | fixed zone list |
| Library gap table | area:43 | Missing-chapter gaps | T-43a-021 | upstream expectations |
| Changelog and checksums | area:54 `TST-54-07`, `TST-54-08` | Section extraction, `<64 hex>  <name>` lines | TST-54-07, TST-54-08 | vectors in the test |

Golden policy (Proposal): a golden is never updated by hand. `UPDATE_GOLDEN=1 cargo test` rewrites the files and the pull request shows the diff. CI never sets `UPDATE_GOLDEN`. A golden diff with no matching code change is rejected in review.

### 4.4 Backup and database goldens

| Golden | Source of truth | Test | Notes |
|---|---|---|---|
| `aniyomi_fresh_v33.sql` | the `.sq` DDL with the mechanical fixes from area:03 `p1-topic-baseline-schema.md`, ending with `PRAGMA user_version = 33;` | fresh-versus-upgraded equivalence (T-03a-020, T-04a-026) | regenerated by the script in area:03; `history.last_read` is nullable |
| `legacy_v13.sql` | six legacy tables | legacy importer, feature `legacy-import` (area:03 section 5.5) | |
| Schema statement prepare | every statement in the schema | T-03a-021 prepares each statement against a fresh database | |
| Q-01 `mihon_1x_small.tachibk` | generated (area:22) | restore count assertions: manga 3, categories 1, chapters 6, history 1, tracks 1 | |
| Q-02 `aniyomi_legacy_0x.tachibk` | generated (area:22) | anime 2, manga 1, anime categories 1 | |
| `complete.tachibk` | generated (area:47) | `T-IT-03`: sources installed in the test catalog, trackers logged in | |
| `missing-numeric.tachibk` | generated (area:47) | sources with numeric ids missing from the catalog; `E49` | |

Float precision (OBSERVED in area:21 `E-07`): `chapterNumber`, `episodeNumber`, `lastChapterRead`, `lastEpisodeSeen` and `score` are stored as float32 in the format. Backup tests assert the widened values (example from area:21: wire value of 1.1 becomes `1.100000023841858`). Do not widen the format to fix this; the tests pin the behaviour.

### 4.5 Real Aniyomi databases (blocking)

Area:21 section 4.10 and T-21a-034 state that the Aniyomi fixtures must come from a real user app and are blocking for wire verification. Area:05 T-05a-032 ("Compatibility test against real Aniyomi databases") has the same dependency. Proposal: the repository holds only sanitised, schema-only copies of real databases. The user exports these from a device, the export is reviewed for personal data (chapter titles, URLs, tracker tokens), and the sanitised file is committed as `crates/kuro-db/tests/fixtures/aniyomi_user_v<N>.db`. Until those exist, these tests are marked `#[ignore = "needs captured Aniyomi db"]` and are listed in the manual checklist.

---

## 5. UI verification

### 5.1 Smoke (L6)

Command (CI, Proposal): `xvfb-run -a env KUROBAYOMI_SMOKE=1 G_DEBUG=fatal-criticals ./target/debug/kurobayomi`

Expected output: a line matching `smoke: <n> screens, <s> sheets, <d> dialogs opened` with `n` equal to the number of screens in `spec/app.json` (the test computes the expected `n` from the spec, not from a constant) and `s` and `d` equal to the sums of `sheets` and `dialogs` in the spec.

`G_DEBUG=fatal-criticals` turns every GTK critical into an abort, so a critical fails the run. Proposal: add a `KUROBAYOMI_SMOKE` exit code of 0 only when the printed counts match the spec, and 2 otherwise. This closes the current gap where the smoke run has no assertions (section 1.1).

The smoke run also needs to cover `screens::open_entry("Title 1", true)` and `("Title 1", false)`. These two calls become named test fixtures (section 4.2, area:38 `manga_basic.json` and area:39 `F-1..F-3`) once the entries are wired to the repositories (T-38b-041, T-39b-063).

### 5.2 Screen-level checks (L5)

For each screen id, the L5 test checks:

1. The screen opens without a panic.
2. The screen's header title equals the value in `spec/app.json`.
3. Each action button in the header exists and is sensitive or insensitive as the spec says.
4. Lists have the number of rows the fixture predicts (for example area:38 `manga_basic.json`: 30 chapter rows).
5. No widget is created with a Linux theme name hard-coded (area:01 rule 6 in [addenda/requirements-cross-platform.md](addenda/requirements-cross-platform.md)).

These checks use GTK inspection (widget tree walk), not pixel comparison. Reference tasks: T-43a-085 (reader chrome, headless), T-47ba-071 (settings harness), T-40b-085 (browse integration with fakes and in-memory SQLite), T-42b-044 (SQL behaviour and set-read on an in-memory DB).

### 5.3 Visual references (L7)

| Item | Rule |
|---|---|
| Capture | `KUROBAYOMI_SIZE=1180x780 KUROBAYOMI_OPEN=<id> KUROBAYOMI_SHOT=<out>.png ./kurobayomi`, one run per screen id, under `xvfb-run` with a fixed font configuration (`FONTCONFIG_FILE` pointing to a committed file) |
| Reference | `tests/visual/<screen id>.png`, committed |
| Comparison | Pixel-exact is not required. Mean absolute error per channel must be at most 2/255 over the whole image, and at most 8/255 in any 32x32 block. Proposal; the thresholds are chosen to absorb anti-aliasing differences between Mesa builds. |
| Update | `UPDATE_VISUAL=1` replaces the references. Only the release owner runs this, and the new references are reviewed in the pull request. |
| Dark and light | The Tokyo Night dark scheme is the reference for every screen, because the user runs Tokyo Night everywhere. Light schemes from area:49 (`fixtures/schemes/*.json`) get a second reference only for the screens area:49 names. |
| Scope | Every spec screen, sheet and dialog. Area:47b T-47ba-072 names "Screenshot checks for the scope pages". |

The `KUROBAYOMI_SHOT` hook waits 1200 ms before rendering (`src/main.rs:101`). A capture that happens before the first frame is blank. Proposal: the test waits for `gtk::Widget::is_mapped` on the window and for one frame clock tick, not a fixed sleep, so slow CI runners do not produce blank references.

### 5.4 Player verification (L8)

| Check | Method | Tasks |
|---|---|---|
| mpv option defaults | Assert the 43 settings in `tests/fixtures/mpv/golden_defaults.txt` against the options set at player creation | T-45b-062 |
| Debanding variant | Second golden with `vf` = `gradfun=radius=12,format=yuv420p` | T-45b-062 |
| Key table | `tests/fixtures/keys/gtk_keys.txt` (`gtk_name<TAB>mods<TAB>expected`) | T-45a-049 family, T-45b-075 |
| Bridge protocol | `tests/fixtures/bridge/*.txt`, one message per line, `property<TAB>value<TAB>expected` | T-45b-062 |
| Restart snapshot | Save then restart then assert position and track selection | T-45b-030 |
| Screenshot path | `fake_ffmpeg.sh` on `PATH`; assert the argument list and the output file name | T-45a-049, T-45b-039 |
| Episode flags | `fixtures/anime_flags.json` values for `T6`-`T10` and `T77`-`T79` (area:45 p2) | T-45b-062 |

Real playback needs a GPU and a display. It is never run in CI. It is in the manual checklist (section 8.3).

---

## 6. Upstream test port map

Every upstream `@Test` becomes a Rust test with the same inputs and expected outputs. The Rust name keeps the upstream test name in snake case, so a reviewer can match them. The port is verified by a count check (Proposal): `cargo xtask test-port-count` reads a table of upstream names and fails if a port is missing.

| Upstream test class | Cases | Rust target | Port rule |
|---|---|---|---|
| `ChapterRecognitionTest` | 34 | `kuro-domain::items::chapter::recognition` tests (T-07a-010) | every upstream input and expected number; the table is copied verbatim |
| `AnimeFetchIntervalTest` | 10 | fetch interval for anime (T-26a-074, T-07a-019) | same clock, same zone `Europe/Berlin` as upstream |
| `MangaFetchIntervalTest` | 10 | fetch interval for manga (T-26a-074, T-07a-019) | same |
| `LibraryFlagsTest` | 5 | flags (T-08a-002, T-07a-035) | bit layout must match the upstream constants |
| `MissingItemsTest` | 8 | missing items (area:07, area:43) | |
| `GetApplicationReleaseTest` | 4 | release check (T-35a-094, T-09a-057) | the four cases of area:35 section 4 |
| `MigratorTest` | 6 | migration framework (T-11a-001) | conformance tests; the framework semantics are sequential, the steps are not ported here |
| `TorrentHelpersTest` | 1 | torrent helpers (T-13a-058) | uses the two `.torrent` resources |

Additional upstream evidence used as expected values (OBSERVED in the area docs): area:21 `E-01` to `E-08` edge cases, area:22 restore cases X-23, and the `DummyTracker` constants of area:27.

Upstream tests that are dropped with a reason: none of the 78 upstream tests is dropped. The Android-only pieces of `MigratorTest` (WorkManager scheduling, `Context`) are ported to the scheduler fake, not dropped.

---

## 7. Per-area test ownership

Each row lists the test-suite tasks from `TASKS.csv` whose titles name tests, fixtures, golden files, smoke or fakes. The edge-case table of each area (`pNN-edge-cases-and-tests.md`) holds the detailed case list; this table is the index into the task table. An empty cell means the area has no dedicated test-suite task in `TASKS.csv` yet (section 10 lists these as gaps).

| Area | Test-suite and fixture tasks | Layers |
|---|---|---|
| 01-foundation-app-bootstrap | T-01a-001 (workspace with `kuro-testkit`), T-01a-039 (FakeTracker, P2), T-01a-045 (testkit), T-01a-048 (crash hooks) | L0, L4 |
| 02-preferences-system | T-02a-033 (in-memory backend), T-02a-030 (bind mockup settings) | L0, L2 |
| 03-db-manga-schema-migrations | T-03a-020 (trigger and versioning), T-03a-021 (schema conformance, prepare), T-03a-022 (legacy 1..32 replay, P2) | L2 |
| 04-db-anime-schema-migrations | T-04a-026 (fresh vs upgraded, P2), T-04a-027 (test kit, fixtures, golden dumps) | L2 |
| 05-data-repos-entries-items | T-05a-023 (repository harness), T-05a-024 (trigger regression), T-05a-032 (real Aniyomi db compatibility) | L2 |
| 06-data-repos-history-track-source-misc | T-06a-005 (temp-file DB harness and builders) | L2 |
| 07-domain-entries-items | T-07a-010 (34 upstream tests), T-07a-011, T-07a-012, T-07a-019 (fetch interval), T-07a-034 (builders), T-07a-035 (LibraryFlagsTest), T-07a-041 (quirk regressions), T-07a-043 (property tests) | L0, L1 |
| 08-domain-library-category-history-updates | T-08a-002 (flag algebra), T-08a-040 (suite, fixtures, manual checklist) | L0, L2 |
| 09-domain-source-track-download-release-misc | T-09a-057 (GetApplicationReleaseTest), T-09a-058 (in-memory DB, fake HTTP server, temp dirs) | L2, L3 |
| 10-app-domain-layer | T-10a-036 (golden and integration suite for the domain layer) | L1, L2 |
| 11-app-migrations | T-11a-001 (MigratorTest conformance), T-11a-003 (idempotency harness), T-11a-028 (non-reproduction tests) | L0, L2 |
| 12-network-stack | T-12a-042 (challenge with fake server), T-12a-043 (retry semantics) | L3, L5 (webkit6) |
| 13-core-util-storage-archive-torrent | T-13a-003 (in-memory prefs), T-13a-058 (torrent tests) | L0, L4 |
| 14-source-local-and-metadata | T-14a-030 (ComicInfo golden), T-14a-052 (suite and fixtures) | L4 |
| 15-source-api-manga | T-15a-012 (source id test vectors), T-15a-036 (merge rules), T-15a-048 (fixtures and suite) | L0, L3 |
| 16-source-api-anime | T-16a-051 (test corpus) | L0, L3 |
| 17-source-app-layer | T-17a-038 (fakes and fixtures), T-17a-042 (enabled pipeline), T-17a-043 (remote update rules), T-17a-049 (language grouping) | L0, L2 |
| 18-extensions-manga | T-18a-044 (core suite), T-18a-045 (fixtures) | L0, L2 |
| 19-extensions-anime | T-19a-061 (store, mapping, container, verification, loader), T-19a-062 (install, queue, cancel, trust), T-19a-063 (update checks, reconciliation), T-19a-068 (protobuf golden and fuzz, P2), T-19a-069 (end-to-end smoke) | L0, L1, L3 |
| 20-extension-strategy-redesign | none in `TASKS.csv` (gap) | |
| 21-backup-format-and-create | T-21a-034 (wire fixtures and compatibility), T-21a-040 (suite) | L0, L1, L2 |
| 22-backup-restore | T-22a-042 (suite and fixtures), T-22a-051 (wire fixtures for sealed PreferenceValue and enum encodings) | L1, L2 |
| 23-downloads-manga | T-23a-036 (manga download core suite) | L2, L4 |
| 24-downloads-anime | T-24a-061 (engine suite) | L4, L8 |
| 25-downloads-shared | T-25a-035 (pure modules and fakes) | L0 |
| 26-library-update-jobs | T-26a-046 (golden tests for every proxy), T-26a-074 (fetch interval suite), T-26a-089 (sync fixture tests) | L0, L1, L2 |
| 27-tracking-core | T-27a-065 (tracking rules suite), T-27b-032 (track dialog logic and login parser) | L0, L2 |
| 28-tracker-anilist | T-28a-023 (fuzzy dates), T-28a-038 (unit and behaviour suite), T-28a-050 (date round trip under two timezones) | L0, L1 |
| 29-tracker-myanimelist | T-29a-041 (recorded fixtures and DTO tests), T-29a-042 (mapping, status, score), T-29a-043 (HTTP fake server), T-29a-058 (manual live checklist) | L0, L3, L10 |
| 30-tracker-kitsu | T-30a-033 (wiremock fixtures for every endpoint) | L3 |
| 31-tracker-shikimori-bangumi | T-31a-019 (Shikimori suite), T-31a-036 (Bangumi suite) | L3 |
| 32-tracker-mangaupdates-simkl | T-32a-061 (unit and fixture suite), T-32a-069 (live verification tests and fixture recording) | L0, L3, L10 |
| 33-tracker-enhanced-kavita-komga-suwayomi-jellyfin | T-33a-013 (Jellyfin producer fixture contract), T-33a-037, T-33a-057, T-33a-074, T-33a-094 (per-server wire tests), T-33a-101 (framework tests), T-33a-102 (wire harness) | L3 |
| 34-images-covers-cache | T-34a-069 (golden and unit suite), T-34a-070 (decoder fixtures) | L1, L4 |
| 35-data-misc-updater-notification-export | T-35a-042 (fetch_latest), T-35a-090 to T-35a-093 (version, asset, action codec, ProgressThrottle), T-35a-094 (GetApplicationReleaseTest port), T-35a-095 (CSV golden), T-35a-096 (fetch against local server), T-35a-097 (downloader integration), T-35a-098 (router with fake notifier) | L0, L1, L3 |
| 36-ui-shell-navigation-security-webview | T-36a-067 (shell test suite) | L0, L5 |
| 37-ui-library | T-37a-051 (pure modules and fixtures), T-37a-059 (selection snapshot refresh) | L0, L5 |
| 38-ui-entry-details-manga | T-38a-051 (pure logic and fixtures), T-38b-041 (upstream tests and Rust suite) | L0, L5 |
| 39-ui-entry-details-anime | T-39a-051 (upstream and unit tests), T-39b-062 (logic and integration suite) | L0, L5 |
| 40-ui-browse-manga | T-40a-038 (pure logic), T-40a-039 (GTK and integration fixtures), T-40b-084 (pure models), T-40b-085 (fakes and in-memory SQLite) | L0, L2, L5 |
| 41-ui-browse-anime | T-41a-049 (unit and integration), T-41b-047 (fixtures), T-41b-048 to T-41b-053 (sources list, pager, filters, global search, migration sort, favourites) | L0, L2, L5 |
| 42-ui-updates-history-stats-storage-upcoming | T-42a-060 (fixtures and DB-backed tests), T-42b-043 (model, selection, grouping, calendar), T-42b-044 (SQL and set-read semantics) | L0, L2 |
| 43-reader-core | T-43a-021 (gap, ported upstream), T-43a-082 (pure logic), T-43a-083 (loader fixtures), T-43a-084 (HTTP loader with scripted source), T-43a-085 (headless reader chrome), T-43b-027 (calculate_chapter_gap upstream), T-43b-038 (tap hit test), T-43b-054 (pure logic suite) | L0, L4, L5 |
| 44-reader-viewers | T-44a-078 (navigation tables), T-44a-079 (list builders, gap, transitions), T-44a-080 (zoom mathematics), T-44a-081 (golden transforms and borders) | L0, L1 |
| 45-player-core | T-45a-049 (screenshot result messages), T-45b-030 (restart snapshot), T-45b-039 (screenshot capture), T-45b-062 (player core suite and fixtures) | L2, L8 |
| 46-player-controls-ui | T-46a-056 (scope logic suite), T-46b-045 (unit and fixture suite) | L0, L5 |
| 47-settings-screens-general | T-47b-053 (pure logic suite) | L0 |
| 47b-settings-widgets-search-more | T-47ba-070 (pure logic), T-47ba-071 (GTK harness), T-47ba-072 (screenshot checks) | L0, L5, L7 |
| 48-settings-player-and-scripts | T-48a-056 (unit and integration) | L0, L2 |
| 49-theme-and-design-system | T-49a-059 (theme and components suite), T-49b-038 (suite for area 49) | L0, L1 |
| 49b-android-res-values-drawables | T-49ba-044 (resource validation scripts and tests) | L9 |
| 50-android-widgets | T-50a-035 (feed, CLI, thumbnails suite) | L0, L2 |
| 51-i18n | T-51a-028 (catalog and locale runtime tests), T-51a-029 (importer tests), T-51a-040 (determinism and size checks in CI) | L0, L9 |
| 52-util-app | T-52a-052 (network state snapshot), T-52a-064 (unit tests for every ported pure function) | L0 |
| 53-platform-manifest-intents-workers | T-53a-072 (route and argv), T-53a-073 (notice dispatch), T-53a-074 (progress, network, resolver, worker), T-53a-075 (desktop file, MIME, shortcut golden), T-53a-077 (dead-code guard) | L0, L1, L9 |
| 54-build-release-distribution-docs | T-54a-008 (ci.yml), T-54a-035 (test configuration and coverage job), T-54a-023 (startup timing script) | L9 |
| addenda (cross-platform) | T-req-platform-010 (Windows cross-compile check in CI) | L9 |

Areas with fewer than two test-titled tasks in `TASKS.csv` (20, 30, 36, 48, 49b, 50, 51 partially) rely on the edge-case table of their area for the test list. Those edge-case tables are the authority for the case list; the gap is that no task owns the suite yet (section 10).

---

## 8. CI matrix and manual QA

### 8.1 Pull request CI (`ci.yml`, T-54a-008)

Triggers: `pull_request` and `push` to `main`. Concurrency group `${{ github.workflow }}-${{ github.ref }}` with `cancel-in-progress: true`. Top-level `permissions: contents: read` (from T-54a-008 acceptance). The `paths` filter names only files that exist (area:54 section 2; `TST-54-16` checks this).

| Job | Runner | Steps | Blocks merge |
|---|---|---|---|
| `fmt` | `ubuntu-24.04` | `cargo fmt --all -- --check` | yes |
| `clippy` | `ubuntu-24.04` | `cargo clippy --workspace --all-targets --locked -- -D warnings` (Proposal; the `-D warnings` rule matches area:54 T-54a-005) | yes |
| `test` | `ubuntu-24.04` | system packages `libgtk-4-dev libmpv-dev xvfb xauth`, then `xvfb-run -a cargo test --workspace --locked` (covers L0 to L5 and L8, with `#[ignore]` tests excluded) | yes |
| `smoke` | `ubuntu-24.04` | `cargo build --locked`, then `xvfb-run -a env KUROBAYOMI_SMOKE=1 G_DEBUG=fatal-criticals ./target/debug/kurobayomi` (L6) | yes |
| `visual` | `ubuntu-24.04` | capture and compare every screen (L7), upload diff images as an artifact on failure | yes once references exist (T-47ba-072) |
| `deny` | `ubuntu-24.04` | `cargo deny check` against `deny.toml` (TST-54-18) | yes |
| `locked` | `ubuntu-24.04` | `cargo build --locked` (TST-54-17) | yes |
| `windows-check` | `ubuntu-24.04` with the `x86_64-pc-windows-gnu` target | `cargo check --target x86_64-pc-windows-gnu --workspace` (T-req-platform-010; addenda rule 7) | yes from the first milestone that has code |
| `i18n` | `ubuntu-24.04` | `cargo xtask i18n-check` (T-51a-040) | yes |
| `docs` | `ubuntu-24.04` | Markdown link check with `lychee` on `plans/**/*.md` and `README.md` (area:54 section 2 decision) | no, advisory |
| `coverage` | `ubuntu-24.04` | `cargo llvm-cov` report, no threshold (T-54a-035, P2) | no |
| `flatpak` | `ubuntu-24.04` with the Flatpak runtime | `flatpak-builder` build (TST-54-20) | only on `release.yml` |

Caching (Proposal, from area:54 section 7): `Swatinem/rust-cache` keyed on `Cargo.lock` and the toolchain. No Gradle cache.

Toolchain: `rust-toolchain.toml` pins a stable release (T-54a-001). Nightly is not used.

Upstream comparison (OBSERVED): upstream PR CI runs `spotlessCheck`, `assembleRelease` and `testReleaseUnitTest` on one job. Kurobayomi splits these into separate jobs so a failing test does not hide a failing format check.

### 8.2 Release CI (`release.yml`, T-54a-009)

Trigger: `push` with `tags: ['v*.*.*']` only (T-54a-009 acceptance). The first job is the release check (`cargo xtask release-check <tag>`, T-54a-010), which runs before any artifact is built. It runs all of L0 to L9 and then the tag and version match (`TST-54-09`), the changelog section (`TST-54-07`), and the checksum format (`TST-54-08`). The `flatpak` job runs TST-54-20 on the produced bundle.

### 8.3 Manual QA checklists

Manual checks are run by the user on a real KDE Plasma session (the target in the project context: CachyOS, KDE Plasma with GTK apps, Wayland and X11). Each item has an expected result. A failure opens an issue with the screen id and the output of `KUROBAYOMI_SMOKE=1`.

Gate names (Proposal): the milestones directory `milestones/` is empty and `TASKS.csv` has no milestone column, so the gates are defined by priority tier, which the project context fixes (P0 is needed for daily use). Each gate lists the tasks whose completion it needs.

**Gate P0 (usable daily: read manga, watch anime from one working source).** All of these must pass before any P1 task is marked done.

| # | Check | Expected | Tasks |
|---|---|---|---|
| P0-1 | Start the app with no data directory | Creates the data, config, cache and log directories through `Paths`; no error dialog | T-01a-004, T-01a-045 |
| P0-2 | Start with a corrupt `config.toml` (`[library`) | Starts with defaults and a visible warning; the corrupt file is kept beside the new one | T-02a-033 family, `config-corrupt.toml` |
| P0-3 | Add a manga to the library from a source | Appears in the library; favourite flag set; history row not created | T-08a-033, T-37a-051 |
| P0-4 | Open a chapter and read to the last page | Last-read marker advances; the next chapter is offered; the row updates in the database | T-43a-082, T-44a-079 |
| P0-5 | Quit in the middle of a chapter and reopen | Reader resumes on the same page | T-43b-038 family, T-45b-030 for the player |
| P0-6 | Watch an episode from one anime source | Playback starts; position saved on quit; episode marked seen at the threshold in the anime rule | T-45b-062, T-45b-075 |
| P0-7 | Restart the player mid-episode | Position and track selection restored (`T-45b-030`) | T-45b-030 |
| P0-8 | Set a preference in settings, restart | Value persists; the same value appears in the widget on the next start | T-02a-030 |
| P0-9 | Open every screen from the sidebar | No warning in the terminal; screen matches the reference in `tests/visual/` | section 5 |
| P0-10 | Install an extension from a configured repository | Install succeeds; trust prompt shown for an untrusted signing key | T-19a-062, T-18a-044 |

**Gate P1 (parity features).** Downloads, tracking, backup and restore, library updates with notifications, categories, global search, source and extension management, history and updates, migration.

| # | Check | Expected | Tasks |
|---|---|---|---|
| P1-1 | Download a chapter and go offline | The downloaded chapter reads from disk; the download index lists it | T-23a-036, T-25a-033 |
| P1-2 | Create a backup, then restore it on a second data directory | Counts match the backup summary; categories and tracks restored | T-21a-040, T-22a-042 |
| P1-3 | Restore Q-01 and Q-02 golden backups | Counts match section 4.4 | T-22a-042 |
| P1-4 | Log in to AniList | Browser opens the OAuth page; the loopback or WebView returns a token; the token is stored in the Secret Service | T-28a-038, addenda/requirements-cross-platform.md (`SecretStore` trait) |
| P1-5 | Push a progress update to a tracker | Remote value matches the local value (manual compare against the tracker's website) | T-29a-058, T-32a-069 |
| P1-6 | Run a library update | Selected manga only; notification shows the count; skipped entries as the rule says | T-26a-074, T-35a-098 |
| P1-7 | Global search across sources | Results grouped per source; cancel stops pending requests | T-41b-051 |
| P1-8 | Migrate an entry to another source | Chapters transfer with read state; original kept until the user confirms | T-41b-052, T-17a-046 |
| P1-9 | Check for an application update | Compares with the latest release tag; no update when the same version | T-35a-090, T-35a-094 |
| P1-10 | Open the updates and history tabs | Rows match the database; set-read marks all visible rows | T-42b-036, T-42b-044 |

**Gate P2 and P3.** The P2 list is the long tail (`503` tasks). Its manual pass is one item per area for the area's last task. The P3 items are drops and replacements; a P3 check confirms the replacement is present (for example the tray or notification replacement) or that the dropped feature is absent from the settings and the menus (T-47a-066, T-52a-062, T-54a-025).

### 8.4 Release manual checklist (every tag)

| # | Check | Expected |
|---|---|---|
| R-1 | Install the Flatpak bundle on the target system (`flatpak run io.github.OWNER.Kurobayomi --version`, TST-54-20) | Version and build metadata printed |
| R-2 | Launch on Wayland and on X11 | Window appears, title bar follows the KDE decoration layout (T-47ba-072 scope) |
| R-3 | GTK theme follows the desktop (TST-54-21) | Native build on KDE Plasma uses the chosen GTK theme, not the Flatpak runtime default |
| R-4 | Play a local file with hardware decoding on, then off | Both play; the hwdec setting shows the value in use |
| R-5 | Check the checksum file | `sha256sum -c SHA256SUMS` passes for every artifact |
| R-6 | Open the changelog from the About screen | Section for the tag is shown (TST-54-07) |

---

## 9. Global test index

These global checks cover things no single area owns. Each has an ID so the release check can list them.

| ID | Layer | Check | Source of the rule |
|---|---|---|---|
| TST-G-01 | plan | Proposal: a check to add to `plans/_tools/verify.py` that every upstream source path cited in `TASKS.csv` is either mapped to a Kurobayomi counterpart or listed as dropped in its area overview. | `plans/_tools/verify.py` (existing coverage check) |
| TST-G-02 | L0 | No test file contains the strings `TODO`, `FIXME`, `TBD` or `unimplemented!()` outside `#[ignore]` tests. | Proposal |
| TST-G-03 | L3 | No test contacts a host outside `127.0.0.1` or `localhost`. Enforced by a test-only proxy that refuses other hosts (Proposal). | Section 3 rule 1 |
| TST-G-04 | L2 | Every SQL file in `crates/*/src/**/*.sq` and every statement of the schema prepares against a fresh database (T-03a-021). | area:03 |
| TST-G-05 | L2 | `PRAGMA user_version` after the first migration equals the value in area:03 (33 for manga, the value in area:04 for anime). | area:03, area:04 |
| TST-G-06 | L4 | Every generated fixture in section 4.2 regenerates byte-identically from its committed generator. | Section 4.1 |
| TST-G-07 | L1 | Every golden in section 4.3 matches on a clean checkout. | Section 4.3 |
| TST-G-08 | L5 | The smoke run prints counts that equal the counts computed from `spec/app.json`. | Section 5.1 |
| TST-G-09 | L9 | `cargo xtask i18n-check` passes and the `po/` directory matches the `LINGUAS` file (area:51, area:54). | area:51, area:54 |
| TST-G-10 | L9 | Every path listed in a `paths` filter of `ci.yml` exists (TST-54-16). | area:54 |

---

## 10. Gaps and open decisions

These are facts that limit the plan. Each one names the person who must decide.

| # | Gap | Effect | Decision owner |
|---|---|---|---|
| G-1 | `milestones/` is empty and `TASKS.csv` has no milestone column | Gates in section 8.3 are defined by priority, not by dated milestones | Project owner |
| G-2 | Area 20 (`20-extension-strategy-redesign`) has no test-suite task in `TASKS.csv`; area 30 has one; area 36 has one (T-36a-067) | Those areas' edge-case tables have no owner for the suite | Plan owner: add a suite task per area |
| G-3 | Real Aniyomi databases and backups are not in the repository (section 4.5) | Compatibility tests for areas 03, 05, 21, 22 are ignored until the user supplies sanitised captures | User |
| G-4 | Cloudflare challenges cannot be tested in CI (no network; area:12 section 4.4) | Challenge solving is covered by fixture HTML only; the live path is manual (R-series) | User |
| G-5 | libmpv playback needs a GPU and a display | Player integration is L8 with fake `ffmpeg` and a headless mpv build; real playback is manual (section 5.4) | User |
| G-6 | Tracker live verification needs real accounts (T-29a-058, T-32a-069) | Live checks are manual; fixtures are hand-written from DTO shapes until recorded | User (accounts) |
| G-7 | `KUROBAYOMI_SMOKE` has no assertions and does not fail on GTK criticals today (section 1.1) | A broken screen can pass the current smoke run | Plan owner: section 5.1 proposal |
| G-8 | The `KUROBAYOMI_SHOT` hook uses a fixed 1200 ms sleep (`src/main.rs:101`) | Blank captures on slow runners | Plan owner: section 5.3 proposal |
| G-9 | Upstream tests cover 8 classes; the rest of the 3801 tasks rely on new tests written from the area edge-case tables | Most behaviour has no upstream oracle. Expected values come from the Kotlin code (observed) and from these tables. | Accepted; documented per area |

---

## 11. Definition of done for the test strategy

The strategy is in force when all of these hold:

1. `kuro-testkit` exists and every item in section 3 that its area needs is present (T-01a-045).
2. Each of the 8 upstream test classes has a Rust port with the same case count, checked by `cargo xtask test-port-count` (section 6).
3. `ci.yml` runs the jobs in section 8.1, and every blocking job passes on `main`.
4. The smoke run in section 5.1 fails when a screen panics or a GTK critical is emitted.
5. Every golden in section 4.3 has a generator or a regeneration command, and `UPDATE_GOLDEN=1` is documented in `CONTRIBUTING.md`.
6. The gate checklists in section 8.3 are run for P0 before any P1 task is marked complete, and the results are recorded in the release notes.
7. The gaps in section 10 are either closed or accepted in writing by their owner.
