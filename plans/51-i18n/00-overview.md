# 51-i18n: Localization overview

Area lead summary for Kurobayomi localization. Aniyomi paths are repo-relative to `/home/blackbox/aniyomi-src`. Kurobayomi paths are relative to `/home/blackbox/kurobayomi`. OBSERVED = what the Kotlin/XML does. PROPOSAL = what Kurobayomi should do. Task file: [p1-tasks.md](p1-tasks.md).

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

- Two moko-resources string modules, both English base plus translations in one directory per locale:
  - `i18n/` (core, Mihon-derived): namespace `tachiyomi.i18n`, accessor `MR`. Base: 857 `<string>` + 19 `<plurals>`.
  - `i18n-aniyomi/`: namespace `tachiyomi.i18n.aniyomi`, accessor `AYMR`. Base: 570 `<string>` + 15 `<plurals>`.
  - Total base names: 1,461. Unique after the 27 names defined in both modules (Aniyomi copy is the one the app reads): 1,434.
  - 22 names are `translatable="false"` (5 core, 17 Aniyomi).
- Locale directories: core has 67 (`base` plus 66 translated, all non-empty). Aniyomi has 68 (`base`, 66 non-empty translated, and `ur`, which is an empty `<resources>` file).
- Call sites: 335 Kotlin files reference `MR.`/`AYMR.` `strings` or `plurals` accessors. 25 Kotlin files still use legacy `R.string`/`R.plurals` (notification and widget code).
- Runtime helpers: `core/common/.../tachiyomi/core/common/i18n/Localize.kt` and `presentation-core/.../tachiyomi/presentation/core/i18n/Localize.kt` (`stringResource`, `pluralStringResource`, and a `fixed()` backslash workaround for moko issue 337).
- User-visible features that depend on this area:
  - App language picker (`AppLanguageScreen.kt`), reached from the Appearance settings.
  - Date format preference (`app_date_format`) and relative timestamps (`relative_time_v2`).
  - Relative date strings for chapter and episode release times, and duration strings (`1d 3h`).
  - Display names of languages used by the source/extension language filters.
  - Every notification body and every error, toast and dialog.

### 1.2 Why it matters for a usable Kurobayomi (PROPOSAL)

- The mockup has no i18n layer. It has about 756 quoted string literals in `src/*.rs` (`src/entry.rs` 270, `src/screens.rs` 204, `src/settings.rs` 107, `src/player.rs` 101, `src/ui.rs` 39, `src/main.rs` 30, `src/clamp.rs` 1, `src/spec.rs` 4), and `spec/*.json` holds English text directly.
- Every other area's tasks render text. If they do not go through a key catalog from the start, converting later means touching every screen twice. The catalog must exist before the UI areas are built out.
- The Aniyomi translations (66 locales) are the only free source of translated UI text for a user who reads a language other than English. Reusing them avoids a translation effort.
- A background thread must not build user-visible text (GTK widgets and locale state are main-thread concerns). The message-key design in this area is what lets tokio tasks report events safely.

### 1.3 In scope and out of scope

In scope: XML-to-Fluent conversion, placeholder and plural conversion, runtime catalog and fallback, locale resolution, language picker data and screen, native and English display names, relative date, duration and short-date formatting, RTL direction, translation import and validation, and the `app_language` / `app_date_format` / `relative_time_v2` preference values.

Out of scope (owned elsewhere):
- Fonts. Kurobayomi bundles no fonts. Script coverage is a check (T-51a-027), not a feature.
- GTK's own dialogs (file chooser, About). They follow the environment language, not `app_language`.
- The text of each feature (owned by the feature area that shows it). This area provides the mechanism and the English catalog.
- Native digits, thousands grouping (none in Aniyomi base strings), and Latin-serbian or other script variants beyond what is listed.

## 2. Module maps

### 2.1 Kotlin module map (OBSERVED)

| Aniyomi file or package | Responsibility | Rust target |
|---|---|---|
| `i18n/src/commonMain/moko-resources/base/strings.xml`, `plurals.xml` | Core English strings (857, 19) | `locales/en/kurobayomi.ftl` (section `core`) |
| `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`, `plurals.xml` | Aniyomi English strings (570, 15), wins on the 27 overlaps | `locales/en/kurobayomi.ftl` (section `aniyomi`) |
| `i18n/src/commonMain/moko-resources/<locale>/` and `i18n-aniyomi/.../<locale>/` | 66 translated locales per module | `locales/<tag>/kurobayomi.ftl` (generated) |
| `i18n/build.gradle.kts`, `i18n-aniyomi/build.gradle.kts` | moko code generation, namespace, `-Xexpect-actual-classes` | Dropped (T-51a-022). Replaced by `build.rs` and `xtask` |
| `buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt` | Writes `xml/locales_config.xml` from non-empty `strings.xml` files, `base` becomes `en` | Dropped. The Rust catalog's locale table replaces it (T-51a-007) |
| `core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt` | `Context.stringResource`, `pluralStringResource`, `StringDesc` wrapper, `fixed()` | `src/i18n/mod.rs` `tr`, `trf`, `tr_plural`. `fixed()` dropped (T-51a-023) |
| `presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt` | Compose `stringResource` and `pluralStringResource` | Same Rust API. Text is resolved on the GTK main thread |
| `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt` | Display names (`getLocalizedDisplayName`, `getDisplayName`), source display names, `getDefaultEnabledLanguages` | `src/i18n/locale.rs`, `src/i18n/names.rs` (T-51a-011, T-51a-012, T-51a-034) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt` | Language picker (Default row first, then sorted list, check on current) | Picker data in `src/i18n/names.rs` and a picker screen (T-51a-011, T-51a-013) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt` | App language row, date-format options with live preview, relative timestamps row | Settings rows (area:47-settings-screens-general, T-51a-021, T-51a-041, T-51a-042) |
| `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt` | `app_date_format` (default `""`), `relative_time_v2` (default `true`), `dateFormat(format)` | Preference keys in area:02-preferences-system (T-51a-017) |
| `app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt` | `LocalDate.toRelativeString`, `LocalDateTime.toRelativeString` | `src/i18n/time.rs` `relative_date`, `relative_datetime` (T-51a-016) |
| `app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt` | `Duration.toDurationString`, `relativeTimeSpanString` (uses `DateUtils`) | `src/i18n/time.rs` `duration_short`, `relative_time_span` (T-51a-025, T-51a-016) |
| `app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt` (178-183), `.../anime/AnimeExtensionManager.kt` (183) | First-run source-language enabling from device language | Owned by area:18-extensions-manga and area:19-extensions-anime, using the resolved locale (T-51a-035) |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt` (358-362), `.../anime/AnimeLibraryUpdateNotifier.kt` (372-376) | Plural notifications with `remaining` passed twice | Background-to-UI message channel (T-51a-032) |

### 2.2 Proposed Rust module and crate map (PROPOSAL)

Inside the existing mockup crate `kurobayomi` (edition 2024, `Cargo.toml` depends only on `gtk` = `gtk4` 0.11.5 with `v4_16`, `serde`, `serde_json`):

| Path | Content |
|---|---|
| `src/i18n/mod.rs` | Public API: `tr(MsgKey)`, `trf(MsgKey, &[(&str, Arg)])`, `tr_plural(MsgKey, count, args)`, `init(Option<&str>)`, `current_locale()`, `enum Arg { Str, Int, Dec }` |
| `src/i18n/keys.rs` | GENERATED by `build.rs`: `enum MsgKey` (one variant per id, 1,434), `MsgKey::id()`, `MsgKey::from_id()`, `MSG_COUNT` |
| `src/i18n/catalog.rs` | Loads embedded `.ftl`, builds `FluentBundle` chain (`concurrent` variant), lookup and fallback, missing-key warning set |
| `src/i18n/locale.rs` | Tag normalisation, startup resolution (`app_language`, then `glib::language_names()`, then `en`), RTL flag |
| `src/i18n/names.rs` | Committed native and English names for 67 offered tags plus `ur` |
| `src/i18n/time.rs` | `relative_date`, `relative_datetime`, `duration_short`, `relative_time_span`, short and fixed date formatting |
| `src/i18n/data.rs` | GENERATED: `LOCALES: &[(tag, ftl_text, rtl)]` |
| `build.rs` | Generates `src/i18n/keys.rs` and `src/i18n/data.rs` from `locales/`; reruns on `locales/` change |
| `xtask/src/main.rs` | Subcommands `i18n-import`, `i18n-check`, `i18n-keys` |
| `xtask/src/i18n_import.rs` | moko XML to Fluent converter, placeholder and plural checks, `fluent_syntax` validation |
| `locales/en/kurobayomi.ftl` | Generated English source (1,434 messages) |
| `locales/<tag>/kurobayomi.ftl` | Generated translations, 66 locales |
| `locales/README.md` | Translator and import instructions (T-51a-024) |

Crates (from [p1-rust-design.md](p1-rust-design.md) section 1): `fluent-bundle` (feature `concurrent`), `unic-langid`, `icu_datetime` (ICU4X, short dates and month names), `quick-xml` (xtask only), `fluent-syntax` (xtask only), `glib::language_names()` (already present through `gtk4`). Not used: `gettext-rs`, `rust-i18n`, `sys-locale`, `moko`. `icu_displaynames` is an offline generator only, used once to produce the committed names table.

## 3. Reading guide

All files live in `/home/blackbox/kurobayomi/plans/51-i18n/`.

| Doc | What it contains |
|---|---|
| [00-overview.md](00-overview.md) | This file: scope, module maps, decisions, dependencies, task summary, definition of done |
| [p1-inventory.md](p1-inventory.md) | One row per in-scope file (8 files): LOC, declared names, port decision, Rust target, priority, task ids, and coverage counts |
| [p1-data-and-config.md](p1-data-and-config.md) | moko XML schema, name and count tables, empty locale file, generated artifacts, Kurobayomi data model, preference keys, file layout, enumerations and constants |
| [p1-behavior.md](p1-behavior.md) | Resource lookup path, argument and plural semantics, plural category counts, format specifier counts, picker behaviour, display-name helpers, relative and duration rules, feature-by-feature message groups, Android behaviour dropped, Kurobayomi behaviour proposal |
| [p1-topic-string-key-catalog.md](p1-topic-string-key-catalog.md) | Full key catalog: 1,461 base names (i18n 876, i18n-aniyomi 585) with module, kind, `translatable`, English value, Kotlin reference counts and status |
| [p1-topic-xml-to-fluent.md](p1-topic-xml-to-fluent.md) | Conversion rules: escapes, placeholders, plurals, output layout, overlap precedence, locale edge cases, runtime loading |
| [p1-topic-locale-and-display-names.md](p1-topic-locale-and-display-names.md) | Directory-to-tag table (67 rows), generated locale list, display-name rules, source-language list, tag normalisation, detection order, RTL, date and digit rules, font coverage |
| [p1-ui-parity.md](p1-ui-parity.md) | Language picker layout, ordering, states, interactions, GTK replacement, restart note, settings row, RTL UI behaviour, date presentation |
| [p1-rust-design.md](p1-rust-design.md) | Crates, module layout, key types and signatures, startup and threading, error types, GTK details, converter design, Android-to-Rust mapping, pitfalls, mockup integration, open decisions |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E1-E18, F1-F12, L1-L16, D1-D7, T1-T24, R1-R5, N1-N4, X1-X6, failure modes, race conditions, 28 automated tests (22 unit, 6 importer) and 4 manual checks, fixtures, performance checks |
| [p1-tasks.md](p1-tasks.md) | 43 tasks, T-51a-001 to T-51a-043 |

## 4. Design decisions and open questions

### 4.1 Decisions with recommendation

| # | Decision | Recommendation | Reason |
|---|---|---|---|
| D1 | Message format: Fluent (`fluent-bundle`) | Adopt | Named arguments, CLDR plural selection per language, translator-friendly. gettext has no named arguments and needs libintl |
| D2 | Translation source stays as Android XML, converted once by `xtask` to `.ftl` | Adopt | Weblate exports this format. Converter is the single place where escapes and placeholders are checked |
| D3 | Aniyomi value wins on the 27 names defined in both modules, in English and in every locale. Core translations are never used for these keys | Adopt | The app reads only the `AYMR` copies; the `MR` copies have zero references |
| D4 | Ship every offered locale, with per-message fallback to English | Adopt, with a note | Aniyomi ships every non-empty locale. Coverage ranges from 192 core strings (`te`) to 842 (`de`, `el`, `es`, `ru`, `tr`). Mixed-language UI is acceptable for a sparsely translated locale. See open question Q1 |
| D5 | Language change takes effect on restart (no live rebuild of GTK widgets) | Adopt | GTK does not re-translate built widgets. Live rebuild would touch every label. Restart note uses the existing `requires_app_restart` string |
| D6 | Picker lists Default (`""`) first, then `en` and the 66 translated locales, sorted by native name | Adopt | Matches Aniyomi, which lists `en` because `LocalesConfigTask` maps `base` to `en` |
| D7 | Native and English display names: a committed table generated once from ICU4X, not a runtime dependency | Adopt | Stable output independent of the host's locale data. Sorting uses code-point order, a documented difference from Android `String.compareTo` for accented Latin names |
| D8 | Default short date from `icu_datetime` (length Short); fixed patterns through chrono; month names of `MMM` patterns from ICU | Adopt | chrono `%b` is English only. ICU short patterns differ from the JVM's; accept and document |
| D9 | ASCII digits, no grouping, everywhere | Adopt | Aniyomi never requests localized digits or `%,d` |
| D10 | Notification channels dropped; the strings reused as category labels or dropped by the notification area | Adopt | GNotification has no channels. See T-51a-036 |
| D11 | S-Pen strings dropped; Firebase/ACRA/analytics strings kept as keys for area:35 | Adopt | No stylus actions on the target. Telemetry decisions belong to area:35 |
| D12 | Keep all 133 names with no literal Kotlin reference, marked in the generated enum | Adopt for now | Dynamic lookups exist (for example `pref_debanding_*`). Final cleanup is T-51a-019 (P3) |
| D13 | Missing key: return the id, log one warning per key. Missing message in a locale: English. Fluent error: log and use the output. Never panic | Adopt | Matches Android fallback to `other` and to the default resource |
| D14 | Logging facade | Defer to area:01-foundation-app-bootstrap. Interim `eprintln!` with `[i18n]` prefix | Only the mockup's `serde`, `serde_json` and `gtk` are present today |
| D15 | Dependency versions in the design snippet written as `"*"` | Replace with the latest stable version at implementation and commit `Cargo.lock` | A wildcard is not reproducible. Versions must be resolved by cargo, as the design says |
| D16 | Language stored as `app_language` (BCP 47 string, default `""`), global, not per profile | Adopt | Kurobayomi has one profile. Aniyomi's per-app locale is also global |

### 4.2 Open questions for the user

| # | Question | Recommendation |
|---|---|---|
| Q1 | Ship all 67 offered locales (including sparse ones such as `te` with 192 core strings, or `ml` with 13 Aniyomi strings), or only locales above a coverage threshold? | Ship all (D4). Revisit after the first import reports per-locale counts (T-51a-008) |
| Q2 | Is "restart to apply language" acceptable for the first release? | Yes (D5). Live switching would need every label to re-read its key, which the mockup does not support |
| Q3 | Should `tr()` before `init()` be allowed? The edge-case file (section 4) says a call before `init` initialises the catalog as English, and a later `init` is ignored. A single early call would then lock English for the whole process | Recommend: `init` is the first statement of `main`. Debug builds panic if `tr` runs before `init`. Release builds fall back to English with one warning. This is a design change to T-51a-006 and to [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 4 |
| Q4 | Is `icu_datetime` compiled data acceptable in binary size? | Yes, check the size after the first build (T-51a-040). The target is under 10 MB added for `.ftl` plus ICU data |
| Q5 | Does the picker show native names in the picker sort by native name, or by English name? | Native name (D7), as Aniyomi sorts by the localized name |
| Q6 | Verify the exact accepted form of `useGrouping: "false"` in `fluent-bundle` NUMBER | Verify with a unit test in T-51a-003. If the string form is rejected, the converter emits a different option form. This is the one unverified library behaviour in the design |

## 5. Cross-area dependencies

### 5.1 What this area needs from other areas

| Area | What is needed | Used by tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | Startup order (`i18n::init` before GTK widgets), workspace layout for `build.rs` and `xtask`, logging facade (D14) | T-51a-015, T-51a-026 |
| area:02-preferences-system | Read and write of `app_language`, `app_date_format`, `relative_time_v2`; plain file read before GTK starts | T-51a-007, T-51a-017 |
| area:18-extensions-manga and area:19-extensions-anime | Source-language display names and the first-run device-language rule | T-51a-034, T-51a-035 |
| area:26-library-update-jobs | Background update results sent as `UiMsg` with keys and arguments; `remaining` counts for the multiple-and-more messages | T-51a-032 |
| area:34-images-covers-cache | Decision on whether the data-saver feature exists (its strings are kept either way) | T-51a-038 |
| area:35-data-misc-updater-notification-export | Notification categories replacing channels; telemetry decision (strings kept as keys) | T-51a-036, T-51a-039 |
| area:36-ui-shell-navigation-security-webview | Picker screen host, Escape or back shortcut, window-level layout | T-51a-013 |
| area:44-reader-viewers | Chapter-swipe labels and tap-zone labels in RTL, reader direction semantics | T-51a-014 |
| area:47-settings-screens-general | Appearance rows (language, date format preview, relative timestamps) | T-51a-021, T-51a-041, T-51a-042 |
| area:49-theme-and-design-system | Theme classes (`heading`, `dim-label`), no hard-coded colours in the picker, font coverage checks | T-51a-027 |

### 5.2 What other areas need from this area

Every area whose tasks show user-visible text needs the `MsgKey` enum, `tr`, `trf`, `tr_plural`, the English catalog keys for its screens, and the date and relative-time helpers. The areas that show the most text are:

- Feature screens: area:36, area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime, area:40-ui-browse-manga, area:41-ui-browse-anime, area:42-ui-updates-history-stats-storage-upcoming, area:43-reader-core, area:44-reader-viewers, area:45-player-core, area:46-player-controls-ui, area:47-settings-screens-general, area:47b-settings-widgets-search-more, area:48-settings-player-and-scripts.
- Background and notifications: area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared, area:26-library-update-jobs, area:35-data-misc-updater-notification-export.
- Backup, tracking and migration messages: area:21-backup-format-and-create, area:22-backup-restore, area:27-tracking-core and the tracker areas 28 to 33.
- Spec-driven settings text: `spec/*.json` (T-51a-030 and T-51a-031).

Interface contract for them (PROPOSAL): add a message to `locales/en/kurobayomi.ftl` with a Fluent id that matches the Kotlin key with dots turned into underscores. Use `MsgKey::<id>` in code. Never store translated strings. Background tasks send `UiMsg::Notify { key, args }`.

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, which validates all task files (3,801 tasks, 0 errors) and writes `/home/blackbox/kurobayomi/plans/TASKS.csv`. The rows for this area are the 43 rows whose file starts with `51-i18n/`.

### 6.1 Counts

| Priority | Tasks | S | M | L | XL |
|---|---|---|---|---|---|
| P0 | 9 | 3 | 6 | 0 | 0 |
| P1 | 19 | 11 | 7 | 1 | 0 |
| P2 | 9 | 8 | 1 | 0 | 0 |
| P3 | 6 | 6 | 0 | 0 | 0 |
| Total | 43 | 28 | 14 | 1 | 0 |

P0 tasks: T-51a-001, T-51a-002, T-51a-003, T-51a-004, T-51a-005, T-51a-006, T-51a-007, T-51a-010, T-51a-043.

### 6.2 P0 critical path, in order

1. T-51a-003 placeholder conversion (M), with T-51a-002 key normalisation (S) and T-51a-005 escaping (S) in parallel.
2. T-51a-004 plural conversion (M), depends on T-51a-003.
3. T-51a-001 English catalog generation (M), depends on T-51a-002 to T-51a-005. Produces `locales/en/kurobayomi.ftl` with 1,434 ids.
4. T-51a-010 generated `MsgKey` enum and `build.rs` (M), depends on T-51a-001.
5. T-51a-006 runtime catalog (M), depends on T-51a-001 and T-51a-010.
6. T-51a-007 locale resolution (M), depends on T-51a-006 and area:02-preferences-system.
7. T-51a-043 overall acceptance (S), depends on T-51a-001, T-51a-008 and T-51a-010.

Upper-bound effort on the P0 path: six M tasks at 3 days each, plus three S tasks at 1 day each, for 21 engineer-days. Parallel work on T-51a-002 and T-51a-005 shortens it.

Note on the P0 path: T-51a-043 is P0 but depends on T-51a-008 (P1, translation import). See risk R6.

### 6.3 Risks

| # | Risk | Likelihood and impact | Mitigation |
|---|---|---|---|
| R1 | `fluent-bundle` NUMBER option form for `useGrouping: "false"` not as the design assumes (Q6) | Medium, blocks T-51a-003 acceptance | Test first. Converter option form is a single place to change |
| R2 | Mockup literal migration (T-51a-031, about 756 literals) conflicts with parallel UI work in area:36 to area:48 | High, large churn | Sequence T-51a-031 after the UI areas settle. Convert screen by screen, one commit per screen, each building and running |
| R3 | Sparse translations show mixed English and translated text | High, cosmetic | Per-message fallback (D4). Picker shows coverage counts from the import (T-51a-008) |
| R4 | Missing fonts for Thai, Khmer, Burmese, Bengali, Kannada, Malayalam, Tamil, Telugu, Devanagari, Ethiopic, Georgian, CJK show boxes | Medium on stock installs | T-51a-027 check. Kurobayomi bundles no fonts |
| R5 | `tr()` before `init()` pins English for the process (Q3) | Low to medium, silent wrong language | Q3 recommendation: init first in `main`, debug panic |
| R6 | Priority inversion: T-51a-043 (P0) depends on T-51a-008 (P1) | Certain as written | Either promote T-51a-008 to P0, or split T-51a-043 so its English checks stay P0 and the translation checks move to a P1 task |
| R7 | Date output differs from the JVM (ICU short patterns; chrono month names) | Medium, visible in the Appearance preview | Snapshot tests per locale (T-51a-017, T-51a-041). Document the difference |
| R8 | GTK's own dialogs follow the system language while the app follows `app_language` | Certain for mixed settings | Documented. Accept |
| R9 | Binary size grows by embedded `.ftl` for 67 locales and ICU data | Medium | Measure after the first import (T-51a-040). Alternative: GResource (see [p1-topic-xml-to-fluent.md](p1-topic-xml-to-fluent.md) section 8) |
| R10 | Placeholder mismatch in a translation renders the raw `{$argN}` text | Low, caught by the import check | Import-time placeholder check (T-51a-009). Runtime tests call every message with its declared arguments |

## 7. Definition of done

Observable behaviours in Kurobayomi, all checked on the target machine:

1. `cargo test` passes every test in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5 (unit tests 1 to 22, importer tests 23 to 28), with no display and no system clock.
2. `cargo run --bin xtask -- i18n-check` exits 0. Running `i18n-import` twice produces no diff.
3. `locales/en/kurobayomi.ftl` has exactly 1,434 message ids, and all 22 `translatable="false"` names are present.
4. The app starts without panic with `LANG=C`, `LANG=de_DE.UTF-8` and `LANG=ar_EG.UTF-8`. Labels appear in English, German and Arabic. With `ar_EG`, the window is mirrored right-to-left.
5. Settings, Appearance, App language opens a picker. The first row is Default, then 67 locales in native-name order. The current row has a check icon. Choosing `ja` shows the restart note `requires_app_restart` without a toast. After restart, the labels are Japanese.
6. No user-visible literal remains in `src/*.rs` except identifiers, icon names, CSS classes and test data (grep check in T-51a-031).
7. A background library update with 3 new chapters shows `3 new chapters` in English, and the plural form in the active language. Text is built on the GTK main thread.
8. Relative timestamps show `Today`, `Yesterday`, `2 days ago`, `Never` and `Just now` at the thresholds in [p1-behavior.md](p1-behavior.md) section 6. With `relative_time_v2` off, absolute dates appear.
9. The date-format preview in Appearance shows one line per option, `Default (<date>)` or `<pattern> (<date>)`, and the stored values load unchanged.
10. No U+2068 or U+2069 character appears in any rendered label (`set_use_isolating(false)` works).
11. A message missing in the active locale shows its English text. A key missing everywhere shows its id and one warning on stderr. Neither causes a panic.
12. Chapter-swipe settings keep their text in RTL. Their control sits on the visually correct side.

## 8. Corrections made in this pass

Verified against the source tree (`/home/blackbox/aniyomi-src`):

- Directory counts: `i18n` has 67 directories including `base` (66 non-empty translations). `i18n-aniyomi` has 68 including `base`, with 66 non-empty translations and `ur` empty (`ur/strings.xml` is 63 bytes, `<resources>` and `</resources>` only).
- `translatable="false"`: 5 in core, 17 in Aniyomi. Base counts: 857 + 19 core, 570 + 15 Aniyomi.
- `LocalesConfigTask.kt` maps `base` to `en`, so English is listed in Aniyomi's language picker. The earlier task text said 66 locales, which is incorrect.
- `pref_show_next_episode_airing_time` ends with a space before `</string>` (E6 is correct).
- `Localize.kt` line 29-30: `fixed()` replaces the backslash character with a double quote, as described.

Edits made to [p1-tasks.md](p1-tasks.md):

- T-51a-011 acceptance: "66 locales" changed to "67 locales (`en` plus the 66 translated tags)".
- T-51a-012 acceptance: the names table covers 67 offered tags plus `ur`, not 66 plus `ur`.
- T-51a-035 dependency: `area:19-extensions-anime` added, because its sources include `AnimeExtensionManager.kt`.
- All area dependency tokens in the file were already real area ids (checked against the directories in `plans/`). No token needed a change.
- Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/51-i18n/p1-tasks.md` prints `ok`.

Doc gaps for the authors (not edited here):

- [p1-behavior.md](p1-behavior.md) section 9.6 says `requires_app_restart` is used at `SettingsAdvancedScreen.kt:116` and `:279`. It is also used at `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt:129` and `:140`, and at `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt:88`.
- [p1-ui-parity.md](p1-ui-parity.md) section 2.5 says "about 67 rows", which is Default plus 67 locales (68 rows). Update to "68 rows (Default plus 67 locales)".
- [p1-rust-design.md](p1-rust-design.md) section 1 writes crate versions as `"*"`. Replace with the version resolved at implementation (D15).
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 4 and [p1-rust-design.md](p1-rust-design.md) section 3 describe an init-once catalog where an early `tr()` fixes English. The Q3 recommendation changes that behaviour. Align both after Q3 is decided.
