# 51-i18n: implementation tasks (part 1 of 1)

Priorities: P0 = needed for daily use (catalog, placeholders, plurals, runtime lookup, locale resolution, key enum). P1 = parity features users rely on (translations, picker, dates, relative time, RTL, tests). P2 = polish. P3 = Android-only, dropped or replaced by a stated decision.

Dependency tokens: task ids `T-51a-001` to `T-51a-043` (this file), `area:<id>` for other plan areas.

### T-51a-001 — Generate the English Fluent catalog from the base XML
- Priority: P0
- Effort: M
- Depends: T-51a-002, T-51a-003, T-51a-004, T-51a-005
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n/src/commonMain/moko-resources/base/plurals.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml
- Acceptance:
  - `locales/en/kurobayomi.ftl` contains exactly 1,434 message ids (857 + 570 strings, 19 + 15 plurals, minus the 27 overlap names taken from the Aniyomi module).
  - Each of the 22 `translatable="false"` names is present in the English file.
  - The file parses with `fluent_syntax` with zero errors.
  - Two runs of the generator produce byte-identical output.

### T-51a-002 — Key normalisation and overlap precedence
- Priority: P0
- Effort: S
- Depends: none
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - `extensionStoresScreen.addStore.title` becomes `extensionStoresScreen_addStore_title`; `action.addExtensionStore` becomes `action_addExtensionStore`.
  - Identifier `type` is emitted as `r#type` in the Rust enum (generator checks all ids against the Rust keyword list).
  - For the 27 names present in both modules, the Aniyomi value is used in English.
  - Two source keys that map to the same id after renaming cause a hard generator error (test with a fixture).

### T-51a-003 — Placeholder conversion from Java format to Fluent named arguments
- Priority: P0
- Effort: M
- Depends: none
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - `%s`, `%N$s`, `%d`, `%N$d`, `%0Nd`, `%.2f`, `%c`, `%%` convert per the table in p1-topic-xml-to-fluent.md section 3.
  - `restore_duration` (`%1$02d min, %2$02d sec`) renders `03 min, 07 sec` for 3 and 7.
  - `%d` renders `1000` for 1000 (no grouping) with the chosen NUMBER option form, confirmed by a unit test.
  - `player_speed` renders `1.50x` for 1.5.

### T-51a-004 — Plural conversion to Fluent select expressions
- Priority: P0
- Effort: M
- Depends: T-51a-003
- Sources: i18n/src/commonMain/moko-resources/base/plurals.xml, i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - All 34 plurals convert; every select has a `*[other]` variant.
  - `notification_chapters_multiple_and_more` and `notification_episodes_multiple_and_more` take `count` and `arg2` as one value (the API passes `remaining` twice).
  - `relative_time` renders `Yesterday` for 1 and `2 days ago` for 2 in English.

### T-51a-005 — Text escaping conversion
- Priority: P0
- Effort: S
- Depends: none
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt
- Acceptance:
  - `\'` → `'`, `\"` → `"`, `&amp;` → `&`, `\n` → `{"\u000A"}`, `{` → `{"{"}`, `}` → `{"}"}`.
  - `en.ftl` contains no backslash-quote and no `&amp;`.
  - Round-trip test over 20 representative strings (edge cases E2–E5, E17, E18, X1, X2) passes.
  - Literal `é` in a translation stays as text (not decoded).

### T-51a-006 — Runtime catalog: bundle chain, lookup, fallback
- Priority: P0
- Effort: M
- Depends: T-51a-001, T-51a-010
- Sources: core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt, presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt
- Acceptance:
  - `tr(MsgKey::…)` returns English when no locale is set.
  - A message missing in the chosen locale falls back to English; a message missing everywhere returns its id and logs one warning.
  - Every bundle calls `set_use_isolating(false)`; output has no U+2068 or U+2069.
  - Catalog is built once in a `OnceLock`; a second `init` is ignored with a warning.
  - Unit tests for edge cases F8, F9, X6 and the missing-key policy pass.

### T-51a-007 — Locale resolution and tag normalisation
- Priority: P0
- Effort: M
- Depends: T-51a-006, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt, buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt
- Acceptance:
  - Preference `app_language` (default `""`) wins when non-empty; otherwise `glib::language_names()` in order; otherwise `en`.
  - Cases L1–L16 in p1-edge-cases-and-tests.md pass, including `in` → `id`, `iw` → `he`, `zh_Hant_TW` → `zh-TW`, `pt_PT` → `pt`.
  - A saved tag that is not offered is ignored; the picker shows Default as checked.
  - Directory-to-tag mapping matches the table in p1-topic-locale-and-display-names.md section 1 for all 67 directories.

### T-51a-008 — Import the 66 translated locales
- Priority: P1
- Effort: M
- Depends: T-51a-001, T-51a-003, T-51a-004, T-51a-005
- Sources: i18n/src/commonMain/moko-resources/ (66 non-base directories), i18n-aniyomi/src/commonMain/moko-resources/ (same 66 non-empty directories; `ur` is empty)
- Acceptance:
  - `xtask i18n-import` writes `locales/<tag>/kurobayomi.ftl` for exactly 66 locales plus `en`.
  - `ur` produces no file (its Aniyomi file is empty) and is not offered in the picker.
  - Every output file parses; the per-locale message count is printed.
  - Aniyomi translations are used for the 27 overlap keys; core translations are never used for them.

### T-51a-009 — Translation validation rules
- Priority: P1
- Effort: M
- Depends: T-51a-003, T-51a-004
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - A translation whose placeholder set differs from the English message is rejected for that message only (edge E13), and English is used.
  - A plural without `other` falls back to English for that plural (E12).
  - A key in a translation that is not in the base is dropped with a warning (E9).
  - A translation of a `translatable="false"` name is ignored (E1).
  - Reordered placeholders are accepted (E14).

### T-51a-010 — Generated message-key enum and build script
- Priority: P0
- Effort: M
- Depends: T-51a-001
- Sources: none (new Kurobayomi code, see p1-rust-design.md section 2)
- Acceptance:
  - `build.rs` writes `src/i18n/keys.rs` with one `MsgKey` variant per English message id (1,434 total) and `MsgKey::id()`.
  - Referring to an unknown key fails compilation (test with a deliberately wrong key in a doc test that is expected to fail).
  - `build.rs` reruns when `locales/` changes.

### T-51a-011 — Language picker data (list builder)
- Priority: P1
- Effort: S
- Depends: T-51a-007, T-51a-012
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt
- Acceptance:
  - Result is the Default row (`""`, label `label_default`) followed by 67 locales (`en` plus the 66 translated tags; Aniyomi's `LocalesConfigTask` maps `base` to `en`, so English is listed).
  - Locales are sorted by native display name (code-point order); Default is always first.
  - Each row has a native name (primary) and an English name (secondary).
  - Tests D6 and D7 pass.

### T-51a-012 — Native and English display-name table
- Priority: P1
- Effort: S
- Depends: T-51a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - `src/i18n/names.rs` has one row for each of the 67 offered tags (`en` plus the 66 translated tags) plus `ur` (not offered until translated).
  - `zh-CN` native name is `简体中文`, `zh-TW` is `繁體中文`.
  - First character of the native name is uppercased where the script has case (test D2).
  - Table has unique native names.

### T-51a-013 — Language picker screen
- Priority: P1
- Effort: M
- Depends: T-51a-011, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt
- Acceptance:
  - Title is `pref_app_language` ("App language"); back control pops the screen.
  - Single-selection `gtk::ListBox`; the current row shows the `object-select-symbolic` icon.
  - Click or Enter selects and writes `app_language` immediately.
  - After a change, the inline label `requires_app_restart` is revealed (not an AdwToast).
  - Up and Down keys move focus; Enter activates; Escape goes back.

### T-51a-014 — RTL semantics for chapter-swipe and reader labels
- Priority: P1
- Effort: S
- Depends: T-51a-015, area:44-reader-viewers
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml (lines 310-313 comments on `pref_chapter_swipe_end` and `pref_chapter_swipe_start`)
- Acceptance:
  - In an RTL locale, the "Chapter on swipe to right" setting places its control on the left in the UI, and the label text is unchanged.
  - Tap-zone labels (`nav_zone_*`) keep their text; their positions are not mirrored automatically.
  - Test R4 passes (visual check recorded in the task).

### T-51a-015 — Startup language, RTL direction and restart semantics
- Priority: P1
- Effort: S
- Depends: T-51a-007, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt (restart behaviour differs: Android applies immediately)
- Acceptance:
  - `i18n::init` runs before any GTK widget is built.
  - `gtk::Widget::set_default_direction` is set to `Rtl` for `ar`, `fa`, `he`, `ur`, and to `Ltr` otherwise when the user chose a language that differs from the system direction (tests R1, R2).
  - The process never calls `std::env::set_var` for language selection.
  - Changing `app_language` at runtime does not change the displayed language until restart (documented in the picker).

### T-51a-016 — Relative date and datetime functions
- Priority: P1
- Effort: M
- Depends: T-51a-006, T-51a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt, app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt
- Acceptance:
  - `relative_date` and `relative_datetime` reproduce the thresholds in p1-behavior.md section 6.2 (cases T1–T13 pass with a fixed clock).
  - `relative_time_v2` false yields the absolute date.
  - Case T20 (`Never`) and T21 (`Just now`) pass.
  - Case T22 (5 minutes ago) output is decided and tested: the Rust output uses the catalog plural `relative_time_minutes` (`5 minutes ago`), not the Android framework text.

### T-51a-017 — Short date and fixed date formats
- Priority: P1
- Effort: M
- Depends: T-51a-007, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt
- Acceptance:
  - Preference `app_date_format` default `""` gives a locale short date from `icu_datetime` (length Short) for the resolved locale.
  - The five fixed patterns (`MM/dd/yy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd MMM yyyy`, `MMM dd, yyyy`) format with chrono, and the month name in `dd MMM yyyy` and `MMM dd, yyyy` comes from ICU (test T24).
  - Existing stored values load unchanged.

### T-51a-018 — Overlap cleanup and verification
- Priority: P2
- Effort: S
- Depends: T-51a-002
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - The 27 core copies of overlap names are not emitted in `en.ftl` (only the Aniyomi entries).
  - Test `overlap_precedence` (p1-edge-cases-and-tests.md 22) passes.
  - Decision recorded: the core copies have zero Kotlin references in the scan (MR count 0 for all 27).

### T-51a-019 — Decision on 133 names with no literal Kotlin reference
- Priority: P3
- Effort: S
- Depends: T-51a-001
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml (86 names), i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml (47 names)
- Acceptance:
  - Each of the 133 names is either kept (with a comment in the generated enum) or removed, and the decision is recorded in the key catalog.
  - Before any removal, a grep for dynamic lookups (for example the debanding names `pref_debanding_none`, `pref_debanding_cpu`, `pref_debanding_gpu`, `pref_debanding_yuv420p`) is done and recorded.
  - Default decision: keep all, to avoid losing translations for a later feature.

### T-51a-020 — Data-quality fixes in the Kurobayomi English catalog
- Priority: P3
- Effort: S
- Depends: T-51a-001
- Sources: i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml (`confirm_exit`, `pref_show_next_episode_airing_time`, `pref_audio_channels`, `pref_audio_pitch_correction_title`), i18n/src/commonMain/moko-resources/base/plurals.xml (`notification_episodes_multiple_and_more` in aniyomi), i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml
- Acceptance:
  - `confirm_exit` (value "Enable Horizontal Seek") is not used by Kurobayomi; its text is not reused for an exit prompt.
  - The trailing space in `pref_show_next_episode_airing_time` is kept as `{" "}` (edge E6).
  - Duplicate pairs (`pref_audio_channels` and `pref_player_audio_channels`, `pref_audio_pitch_correction_title` and `pref_player_audio_pitch_correction`) are both kept, with a note that the UI uses one of them.
  - Identical plural forms are kept as is.
  - The Aniyomi source tree stays unchanged (read-only).

### T-51a-021 — Appearance settings row for App language
- Priority: P1
- Effort: S
- Depends: T-51a-013, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt
- Acceptance:
  - Row title `pref_app_language`, in the group titled `pref_category_display` ("Display").
  - Subtitle is the native name of the saved language, or `label_default` when unset (desktop addition, UI parity §2.7).
  - Activating the row opens the picker.

### T-51a-022 — Drop Gradle, moko and Android manifest plumbing
- Priority: P3
- Effort: S
- Depends: none
- Sources: i18n/build.gradle.kts, i18n-aniyomi/build.gradle.kts, i18n/src/main/AndroidManifest.xml, buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt, buildSrc/src/main/kotlin/mihon.code.lint.gradle.kts
- Acceptance:
  - Decision recorded: no Gradle, moko, AGP, or locales-config code is ported. The replacement is `build.rs` (T-51a-010) and `xtask` (T-51a-008).
  - No Kurobayomi file is named after or generated from these files.
  - The plan in p1-data-and-config.md section 2 is cross-referenced from the Kurobayomi README.

### T-51a-023 — Drop the Android localisation API surface
- Priority: P3
- Effort: S
- Depends: T-51a-006
- Sources: core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt, presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt (AppCompatDelegate calls)
- Acceptance:
  - Decision recorded: `Context.stringResource`, `StringDesc`, `fixed()` (backslash workaround for moko issue 337), `AppCompatDelegate.setApplicationLocales` and `LocaleListCompat` are not ported. The replacements are `tr`, `trf`, `tr_plural`, `app_language`, and `glib::language_names()`.
  - No Kurobayomi code reproduces the backslash replacement.

### T-51a-024 — Translation workflow and locales README
- Priority: P2
- Effort: M
- Depends: T-51a-008
- Sources: i18n/README.md
- Acceptance:
  - `locales/README.md` states: English source is `locales/en/kurobayomi.ftl`, translations are edited in Weblate or as `.ftl` files, `xtask i18n-import` regenerates from the Aniyomi XML, and `xtask i18n-check` validates.
  - Weblate handoff is documented: which format is exported (Fluent `.ftl`) and that the import check runs in CI.
  - Command `xtask i18n-check` exits non-zero on any validation error from T-51a-009.

### T-51a-025 — Duration short formatter
- Priority: P1
- Effort: S
- Depends: T-51a-006
- Sources: app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt
- Acceptance:
  - Cases T14–T19 in p1-edge-cases-and-tests.md pass (day, hour, minute, second rules and the `" "` join, with fallback for empty).
  - Keys `day_short`, `hour_short`, `minute_short`, `seconds_short` are used.

### T-51a-026 — Missing-message policy and logging
- Priority: P2
- Effort: S
- Depends: T-51a-006
- Sources: core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt
- Acceptance:
  - Missing key logs one warning per key; the id is returned (never a panic).
  - Fluent formatting errors are logged at debug and the formatted text is still used.
  - Logging facade is chosen by area:01-foundation-app-bootstrap; until then `eprintln!` with an `[i18n]` prefix.

### T-51a-027 — Font coverage check for non-Latin scripts
- Priority: P2
- Effort: S
- Depends: T-51a-006, area:49-theme-and-design-system
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml (no font files exist in scope; list of scripts in p1-topic-locale-and-display-names.md section 10)
- Acceptance:
  - A manual check renders one sample string for each script group (Arabic, Hebrew, Thai, Khmer, Burmese, Bengali, Kannada, Malayalam, Tamil, Telugu, Devanagari, Ethiopic, Georgian, CJK) without tofu on a stock CachyOS install, and the missing fonts are listed in the task notes.
  - Kurobayomi does not bundle fonts.

### T-51a-028 — Runtime test suite for the catalog and locale code
- Priority: P1
- Effort: M
- Depends: T-51a-006, T-51a-007, T-51a-016, T-51a-025
- Sources: app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - Unit tests 1–22 in p1-edge-cases-and-tests.md section 5.1 exist and pass with `cargo test`.
  - Tests run without a display (no GTK init).
  - Relative-time tests use the fixed clock fixture, never the system clock.

### T-51a-029 — Importer tests and fixtures
- Priority: P1
- Effort: S
- Depends: T-51a-008, T-51a-009
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/ur/strings.xml
- Acceptance:
  - Tests 23–28 in p1-edge-cases-and-tests.md section 5.2 pass.
  - Count test fails if the parsed base counts differ from 857 + 19 (core) and 570 + 15 (aniyomi).
  - Fixtures live under `xtask/tests/fixtures/`.

### T-51a-030 — Spec JSON uses message keys instead of English text
- Priority: P1
- Effort: M
- Depends: T-51a-010
- Sources: spec/SCHEMA.md, spec/settings.json, spec/app.json, spec/entries.json, spec/reader_player.json, src/spec.rs
- Acceptance:
  - Every `title`, `subtitle` and `options` string in spec files is a `MsgKey` id (or a stored identifier such as `update_weekly`).
  - `src/spec.rs` resolves keys through `MsgKey::from_id` and fails the existing `spec_is_consistent` test on an unknown key.
  - Preference values stored in files stay English-independent identifiers.

### T-51a-031 — Route mockup literals through the catalog
- Priority: P1
- Effort: L
- Depends: T-51a-010, T-51a-030
- Sources: src/entry.rs, src/screens.rs, src/settings.rs, src/player.rs, src/ui.rs, src/main.rs (quoted literal counts 270, 204, 107, 101, 39, 30 respectively)
- Acceptance:
  - No user-visible English literal remains in `src/*.rs` except identifiers, icon names, CSS classes and test data. Check by grep in CI.
  - Each screen is converted in its own commit; each commit builds and runs.
  - Placeholders use `trf` with typed arguments.

### T-51a-032 — Background-thread text through UI messages
- Priority: P1
- Effort: S
- Depends: T-51a-010, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Tokio tasks send `UiMsg::Notify { key, args }`; text is produced on the GTK main thread.
  - A test sends `notification_chapters_generic` with count 3 and receives `3 new chapters` in English.
  - Notification text reflects the language active at display time.

### T-51a-033 — ASCII digits and no grouping policy
- Priority: P2
- Effort: S
- Depends: T-51a-003
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml (no `%,d` used)
- Acceptance:
  - Every numeric placeholder renders ASCII digits without grouping in every locale (test 8 and 9).
  - Decision recorded in the topic file (native digits deferred).

### T-51a-034 — Source-list display names
- Priority: P1
- Effort: S
- Depends: T-51a-010, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt (getSourceDisplayName)
- Acceptance:
  - `last_used` → `last_used_source`, `pinned` → `pinned_sources`, `other` → `other_source`, `all` → `multi_lang` ("Multi"), other tags → native display name.
  - The `all` entry sorts first in every language list.

### T-51a-035 — First-run source-language enabling uses the resolved UI language
- Priority: P1
- Effort: S
- Depends: T-51a-007, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt (lines 178-183), app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt (line 183)
- Acceptance:
  - The device language is the language part of the resolved locale (not a raw environment read).
  - A source language is enabled on first run if it is not the device language and starts with the device language, matching the Aniyomi rule.
  - AnimeExtensionManager and MangaExtensionManager share the same helper.

### T-51a-036 — Drop notification channel concept, keep the text
- Priority: P3
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml (`channel_common`, `channel_progress`, `channel_complete`, `channel_errors`, `channel_new_chapters`, `channel_app_updates`, `channel_ext_updates`), i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml (`channel_new_chapters_episodes`)
- Acceptance:
  - Decision recorded: no notification channels (GNotification has none). The strings are used as category labels in the notification settings view, or dropped if the notification area does not need them.
  - `channel_new_chapters` (unused) is not in the generated enum, if removed by T-51a-019.

### T-51a-037 — Drop S-Pen strings
- Priority: P3
- Effort: S
- Depends: none
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml (`spen_previous_page`, `spen_next_page`)
- Acceptance:
  - Decision recorded: no stylus actions on Linux desktop. The two keys are dropped from the generated enum (or kept unused, per T-51a-019).

### T-51a-038 — Data-saver and proxy strings: keys owned by the image area
- Priority: P2
- Effort: S
- Depends: area:34-images-covers-cache
- Sources: i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml (`data_saver*`, `bandwidth_hero`, `wsrv`, `resmush`, `bandwidth_data_saver_server`, `data_saver_server_summary`)
- Acceptance:
  - Keys are present in the English catalog and translated by T-51a-008.
  - The image area decides whether the feature exists; this task does not add it.
  - Remote endpoints named by these strings are not contacted by the i18n code.

### T-51a-039 — Crash-consent and analytics strings: keys owned by area 35
- Priority: P2
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml (`pref_firebase`, `firebase_summary`, `pref_enable_acra`, `pref_acra_summary`, `pref_dump_crash_logs`, `pref_dump_crash_logs_summary`, `onboarding_permission_crashlytics`, `onboarding_permission_analytics` and their descriptions)
- Acceptance:
  - Keys are present and translated.
  - Area 35 decides whether any telemetry exists; no telemetry is added by i18n.

### T-51a-040 — Determinism and size checks in CI
- Priority: P2
- Effort: S
- Depends: T-51a-008
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - CI regenerates the locales and fails on any diff.
  - Binary size increase from embedded `.ftl` files is recorded in the task notes (target under 10 MB added).

### T-51a-041 — Date-format preview in the Appearance settings
- Priority: P2
- Effort: S
- Depends: T-51a-017, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt (lines 148-160, DateFormats list)
- Acceptance:
  - Each date-format option shows `Default (<date>)` or `<pattern> (<date>)` with today's date formatted in that format.
  - The preview is computed when the screen opens, not on every frame.

### T-51a-042 — Relative-timestamps toggle
- Priority: P1
- Effort: S
- Depends: T-51a-016, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt (lines 162-170), app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt (`relativeTime`)
- Acceptance:
  - Preference `relative_time_v2`, default `true`, toggles relative dates everywhere the relative-date function is used.
  - Subtitle reads `"Today" instead of "<formatted now>"` in the active language.

### T-51a-043 — Overall i18n acceptance
- Priority: P0
- Effort: S
- Depends: T-51a-001, T-51a-008, T-51a-010
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - `cargo test` passes all i18n tests.
  - The app starts with `LANG=C`, `LANG=de_DE.UTF-8`, and `LANG=ar_EG.UTF-8` without panics, and the labels appear in the expected language.
  - Changing `app_language` to `ja` and restarting shows Japanese labels.
  - Every key in the inventory's scope files has a task listed in p1-inventory.md (self-check passes).
