# 51-i18n: behaviour spec

Paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" = what the Kotlin/XML does. "Proposal:" = what Kurobayomi should do. Full key list: [p1-topic-string-key-catalog.md](p1-topic-string-key-catalog.md). Format conversion rules: [p1-topic-xml-to-fluent.md](p1-topic-xml-to-fluent.md). Locale and display-name rules: [p1-topic-locale-and-display-names.md](p1-topic-locale-and-display-names.md).

## 1. Resource model

### 1.1 Two string modules, one runtime

OBSERVED: `app/build.gradle.kts` depends on both `projects.i18n` and `projects.i18nAniyomi` (lines 187-188). `presentation-core` exposes `projects.i18n` as `api` (line 32 of `presentation-core/build.gradle.kts`).

| Module | moko class | Namespace | Base file |
|---|---|---|---|
| `i18n` (core, Mihon-derived) | `MR` (default) | `tachiyomi.i18n` | `i18n/src/commonMain/moko-resources/base/strings.xml` and `plurals.xml` |
| `i18n-aniyomi` | `AYMR` (set by `resourcesClassName.set("AYMR")`) | `tachiyomi.i18n.aniyomi` | `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` and `plurals.xml` |

Kotlin accessor forms seen in the app:
- `MR.strings.<key>` (2,336 occurrences of `MR.strings.` in `.kt` files), `AYMR.strings.<key>` (656 occurrences).
- `MR.plurals.<key>` (37 occurrences), `AYMR.plurals.<key>` (13 occurrences).
- Legacy Android `R.string.<key>` and `R.plurals.<key>` (about 35 distinct keys, for example `R.plurals.notification_episodes_generic` in `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt:339`). These resolve to the Android resources generated from the same XML.
- Dotted XML keys are referenced with underscores in Kotlin. Example: XML `extensionStoresScreen.addStore.title` is `MR.strings.extensionStoresScreen_addStore_title`. OBSERVED in the reference counts, see the catalog.

### 1.2 Lookup path (OBSERVED)

1. Composables call `stringResource(res)` or `stringResource(res, vararg args)` from `presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt:13-21`. The value comes from `LocalContext.current`.
2. Non-composable code calls `Context.stringResource(...)` from `core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt:12-18`.
3. Plurals: `pluralStringResource(res, count)` and `pluralStringResource(res, count, vararg args)` (`presentation-core/.../Localize.kt:25-33`, `core/common/.../Localize.kt:20-26`).
4. Each call builds a moko `StringDesc` (`Resource`, `ResourceFormatted`, `Plural`, `PluralFormatted`) and calls `.toString(context).fixed()`.
5. `fixed()` (`core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt:28-31`) replaces every backslash character with one double-quote character. The comment says it is a workaround for `icerockdev/moko-resources#337`. Effect: an escape `\"` that survives generation is rendered as `"`. Proposal: not needed in Kurobayomi, see section 9.

### 1.3 Argument and count semantics (OBSERVED)

- With arguments, Android formats with `String.format` semantics. Positional (`%1$s`) and sequential (`%s`, `%d`) specifiers both occur in the base files. Both forms are counted in section 3.
- Plurals are called as `pluralStringResource(MR.plurals.x, n, n)` in most call sites: the first `n` selects the quantity category, the second `n` fills the `%d` or `%1$d` slot. Example: `app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:50-54` passes `difference.toInt().absoluteValue` twice for `upcoming_relative_time`.
- Some call sites pass only the count and no arguments, for example `pluralStringResource(MR.plurals.x, count)`. The base one-form then must not contain a placeholder that needs an argument. Verified case: `relative_time` one = `Yesterday` (no placeholder).

## 2. Plural selection

OBSERVED:
- The Android resource system picks one of `zero`, `one`, `two`, `few`, `many`, `other` using the device language's CLDR plural rules.
- Base (English) files define only `one` and `other`. The translation files define additional categories. Counts over all locale files:
  - Core (`i18n`) translation items: `few` 181, `many` 207, `one` 811, `other` 973, `two` 36, `zero` 35.
  - Aniyomi (`i18n-aniyomi`) translation items: `few` 34, `many` 62, `one` 127, `other` 144, `two` 14, `zero` 7.
- When a locale lacks a category, the lookup falls back to `other`. This is Android behaviour and is the rule Kurobayomi must reproduce.
- Negative counts are not passed into plurals. Callers use `.absoluteValue` (see `DateExtensions.kt`).

Plural forms that are identical in `one` and `other` (no real plural distinction in English; still valid):
- `download_queue_summary`: both `%1$s remaining`.
- `notification_episodes_multiple_and_more` (aniyomi): both `Episodes %1$s and %2$d more`.

Plural forms that mix placeholder styles (valid, but worth knowing):
- `notification_episodes_generic` (aniyomi): one = `%d new episode`, other = `%1$d new episodes`.
- `day` (core): one = `1 day` (no placeholder), other = `%d days`. Used 4 times as `R.plurals.day`.
- `download_amount`: one = `Next chapter` (no number), other = `Next %d chapters`.
- `missing_chapters_warning`: both forms carry `%d`.

## 3. Format specifiers (counted over base files)

Counts below are over both base XML files together (core `i18n` plus `i18n-aniyomi`, `strings.xml` only, plural forms included), counted with one grep over the two `base` directories.

| Specifier | Occurrences | Examples |
|---|---|---|
| `%1$s` | 57 | `pref_flash_duration_summary` `%1$s ms`; `restore_completed_message` `Done in %1$s with %2$s error` |
| `%s` | 40 | `unlock_app_title` `Unlock %s`; `restrictions` `Restrictions: %s` |
| `%d` | 48 | `pref_hardware_bitmap_threshold_default` `Default (%d)`; `day_short` `%dd` |
| `%2$s` | 18 | `action_replace_repo_message` `Repository %1$s has the same Signing Key Fingerprint as %2$s.` |
| `%1$d` | 15 | `cache_deleted` `Cache cleared, %1$d files deleted` |
| `%2$d` | 6 | `notification_chapters_multiple_and_more` other `Chapters %1$s and %2$d more` |
| `%3$s` | 2 | aniyomi `mpv_media_title` `%1$s - E%2$s - %3$s` |
| `%3$d` | 1 | `share_page_info` `%1$s: %2$s, page %3$d` |
| `%%` | 2 | aniyomi `episode_download_progress` `%1$d%%` |
| `%c` | 1 | aniyomi `player_gesture_seek_indicator` `%c%s\n[%s]` |
| `%1$02d`, `%2$02d` | 1 each | `restore_duration` `%1$02d min, %2$02d sec` |
| `%.2f` | 1 | aniyomi `player_speed` `%.2fx` (non-translatable) |

Special semantics:
- `restore_duration`: minutes (`%1$02d`) and seconds (`%2$02d`), both zero-padded to two digits. Example result: `03 min, 07 sec`.
- `player_speed`: `%.2fx`, two decimals, locale-independent in the sense that it is non-translatable.
- `player_gesture_seek_indicator`: `%c` is the sign character, `%s` the signed-or-unsigned seek value, `%s` the bracketed total, separated by a newline. Non-translatable.
- `mpv_media_title`: `%1$s - E%2$s - %3$s` (aniyomi). Used as the player window title in mpv (not translatable in practice; it is in the translatable set).
- `player_skip_action`: `Skip %1$s` where `%1$s` is one of the chapter-type strings `player_chapter_type_opening` (Opening), `_ending` (Ending), `_recap` (Recap), `_mixedop` (Mixed opening).
- `cancel_all_for_series` and other keys without placeholders must not receive arguments.

## 4. Locale selection (App language)

### 4.1 Screen (OBSERVED)

File: `app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt`. Title `MR.strings.pref_app_language` = "App language". Reached from `SettingsAppearanceScreen.kt` (line 119).

Steps:
1. `langs = getLangs(context)`:
   - Parse `R.xml.locales_config` (generated, see [p1-data-and-config.md](p1-data-and-config.md)).
   - For each `<locale android:name="…">` take `langTag`. Display name = `LocaleHelper.getLocalizedDisplayName(langTag)`. If empty, skip. Supporting text = `LocaleHelper.getDisplayName(langTag)`.
   - `langs.sortBy { it.displayName }` (natural string order of the localized name).
   - Insert at index 0: `Language("", context.stringResource(MR.strings.label_default), null)`. No supporting text.
2. `currentLanguage` = `AppCompatDelegate.getApplicationLocales().get(0)?.toLanguageTag() ?: ""`.
3. Tapping a row sets `currentLanguage = langTag`.
4. `LaunchedEffect(currentLanguage)`: if empty, `setApplicationLocales(LocaleListCompat.getEmptyLocaleList())` (follow system); else `setApplicationLocales(LocaleListCompat.forLanguageTags(tag))`.
5. The row whose `langTag == currentLanguage` shows a check icon (`Icons.Default.Check`, tint `MaterialTheme.colorScheme.primary`).
6. No restart message, no confirmation dialog. Activities are recreated by AppCompat (Android behaviour).

Edge cases:
- The "Default" row has `langTag == ""`. It is always first, regardless of sort.
- A stored tag that is not in the generated list still stays in effect (AppCompat), but the screen shows no check on any row.
- `ur` (Urdu) is not listed in the core locale config (its core directory does not exist) and is listed only if its aniyomi `strings.xml` is non-empty. It is empty, so it is not listed (see [p1-data-and-config.md](p1-data-and-config.md)).

### 4.2 Proposal (Kurobayomi)

- Same list semantics: "Default" (`""`, follow the system) first, then every locale in the Kurobayomi catalog sorted by localized display name.
- Store the choice in preference key `app_language` (string, default `""`), in the area:02 preferences store. Android keeps this in AppCompat's per-app store; Kurobayomi has no such store.
- Apply on next start. GTK does not re-translate existing widgets. See [p1-ui-parity.md](p1-ui-parity.md) for the UI text.

## 5. Display names and language-related helpers

File: `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt` (OBSERVED, whole file).

- `comparator`: `"all"` always sorts first. Otherwise compare `getLocalizedDisplayName(a)` to `getLocalizedDisplayName(b)` with `String.compareTo`.
- `getSourceDisplayName(lang, context)`: `"last_used"` (constant `LAST_USED_KEY`) → `MR.strings.last_used_source`; `"pinned"` (`PINNED_KEY`) → `MR.strings.pinned_sources`; `"other"` → `MR.strings.other_source`; `"all"` → `MR.strings.multi_lang` ("Multi"); anything else → `getLocalizedDisplayName(lang)`.
- `getDisplayName(lang)`: normalises `"zh-CN"` to `"zh-Hans"` and `"zh-TW"` to `"zh-Hant"`, then returns `Locale.forLanguageTag(normalized).displayName` (name shown in the default locale).
- `getLocalizedDisplayName(lang)`:
  - `null` → `""`.
  - `""` → `LocaleListCompat.getAdjustedDefault()[0]` (system language).
  - `"zh-CN"` → `Locale.forLanguageTag("zh-Hans")`; `"zh-TW"` → `Locale.forLanguageTag("zh-Hant")`; otherwise `Locale.forLanguageTag(lang)`.
  - Result = `locale.getDisplayName(locale)` (name in its own language) with the first character uppercased in that locale.
- `getDefaultEnabledLanguages()`: `setOf("all", "en", Locale.getDefault().language)`.
- `getSimpleLocaleDisplayName()`: `LocaleListCompat.getDefault()[0]!!.displayLanguage`.
- Constants: `PINNED_KEY = "pinned"`, `LAST_USED_KEY = "last_used"` (internal).

Device language used elsewhere (OBSERVED): `app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt:178-183` and `.../anime/AnimeExtensionManager.kt:183`: `val deviceLanguage = Locale.getDefault().language`. On first run, a source language is enabled if `it != deviceLanguage && it.startsWith(deviceLanguage)`, added to `preferences.enabledLanguages().defaultValue()`. Proposal: keep this rule (area:18/19 owns it), and take the device language from the Kurobayomi resolved locale (section 6 in the topic file), not from the raw environment.

## 6. Date, time and relative strings

### 6.1 Date format preference (OBSERVED)

- `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:35`: `fun dateFormat() = preferenceStore.getString("app_date_format", "")`. Default is `""`.
- `UiPreferences.dateFormat(format)` (companion, lines 43-47):
  - `""` → `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)` (no explicit locale: uses the JVM default locale).
  - otherwise → `DateTimeFormatter.ofPattern(format, Locale.getDefault())`.
- Options (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt`, list `DateFormats`, lines ~181-186, in order): `""` (Default), `"MM/dd/yy"`, `"dd/MM/yy"`, `"yyyy-MM-dd"`, `"dd MMM yyyy"`, `"MMM dd, yyyy"`.
- Each option's label is `"${it.ifEmpty { label_default }} ($formattedDate)"` where `formattedDate = dateFormat(it).format(now)`.
- Title: `MR.strings.pref_date_format` = "Date format".

### 6.2 Relative timestamps (OBSERVED)

- Preference `relative_time_v2` (`UiPreferences.relativeTime()`), default `true`. Title `pref_relative_format` "Relative timestamps". Subtitle `pref_relative_format_summary` = `"%1$s" instead of "%2$s"`, args `relative_time_today` ("Today") and the formatted current date (`SettingsAppearanceScreen.kt:162-170`).

`LocalDate.toRelativeString` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:52-72`). `difference = ChronoUnit.DAYS.between(this, now)`:

| Condition | Output |
|---|---|
| `relative == false` | `dateFormat.format(this)` (absolute) |
| `difference < -7` | absolute date |
| `difference < 0` | `MR.plurals.upcoming_relative_time` with `abs(difference)` (one: "Tomorrow", other: "In %d days") |
| `difference < 1` | `MR.strings.relative_time_today` ("Today") |
| `difference < 7` | `MR.plurals.relative_time` with `difference` (one: "Yesterday", other: "%d days ago") |
| otherwise | absolute date |

`LocalDateTime.toRelativeString` (same file, lines 75-135) for chapter and episode release times:
- `timeDifference = DAYS.between(this, now)` on date-times; `dateDifference = DAYS.between(this.toLocalDate(), now.toLocalDate())`.
- `timeDifference < -7` → absolute.
- `timeDifference < 0` → `MR.plurals.upcoming_relative_time` with `abs(dateDifference)`.
- `timeDifference < 1` → hour logic:
  - `hourDifference = HOURS.between(this, now)`.
  - `hourDifference < 0` → `AYMR.plurals.upcoming_relative_time_hours` with `abs(hourDifference)` (one: "In 1 hour", other: "In %1$d hours").
  - `hourDifference < 1` → minute logic with `minuteDifference = MINUTES.between(this, now)`:
    - `< 0` → `AYMR.plurals.upcoming_relative_time_minutes` with `abs` (one: "In 1 minute", other: "In %1$d minutes").
    - `== 0` → `AYMR.strings.relative_time_now` ("Now").
    - otherwise → `AYMR.plurals.relative_time_minutes` (one: "1 minute ago", other: "%1$d minutes ago").
  - otherwise → `AYMR.plurals.relative_time_hours` (one: "1 hour ago", other: "%1$d hours ago").
- `timeDifference < 7` → `MR.plurals.relative_time` with `dateDifference` (one: "Yesterday", other: "%d days ago").
- otherwise → absolute.

Note the asymmetry: the `< 1` branch uses `timeDifference` (fractional days truncated to whole days), the `< 7` branch uses `dateDifference` (calendar days).

### 6.3 Duration and "last updated" text (OBSERVED)

`app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt`:
- `Duration.toDurationString(context, fallback)`: `toComponents { days, hours, minutes, seconds, _ -> … }` builds a list:
  - `days != 0` → `MR.strings.day_short` (`%dd`).
  - `hours != 0` → `MR.strings.hour_short` (`%dh`).
  - `minutes != 0 && (days == 0 || hours == 0)` → `MR.strings.minute_short` (`%dm`).
  - `seconds != 0 && days == 0 && hours == 0` → `MR.strings.seconds_short` (`%ds`).
  - Joined with `" "`. If empty, returns `fallback`.
- `relativeTimeSpanString(epochMillis)`:
  - `epochMillis <= 0` → `MR.strings.relative_time_span_never` ("Never").
  - `now - epochMillis < 1 minute` → `MR.strings.updates_last_update_info_just_now` ("Just now").
  - otherwise → `android.text.format.DateUtils.getRelativeTimeSpanString(epoch, now, MINUTE_IN_MILLIS)`. This text is produced by the Android framework, not by these XML files. See the Rust mapping in the topic file.
- `updates_last_update_info` = `Library last updated: %s`.

## 7. Messages by feature (user-visible key groups)

Every key below is in the catalog. Grouped by feature so an engineer can find the owning area. Placeholders are listed in section 3.

### 7.1 Notifications (library update, downloads, extensions, app update)

- Library update service (core): `notification_updating_progress` ("Updating library… (%s)"), `notification_size_warning`, `notification_new_chapters` ("New chapters found"), `notification_chapters_single` ("Chapter %1$s"), `notification_chapters_single_and_more` ("Chapter %1$s and %2$d more"), `notification_chapters_multiple` ("Chapters %1$s"), `notification_chapters_multiple_and_more` (plural), `notification_chapters_generic` (plural: "%1$d new chapter"/"…chapters"), `notification_new_chapters_summary` (plural: "For %d entry"/"For %d entries"), `notification_update_error` ("%1$d update(s) failed"), `learn_more` ("Tap to learn more"), `notification_cover_update_failed`, `notification_first_add_to_library`, `library_errors_help` (`%1$s` = link), `notification_background_update_failed` (aniyomi).
- Skip reasons (core): `skipped_reason_completed`, `skipped_reason_not_caught_up`, `skipped_reason_not_started`, `skipped_reason_not_always_update`, `skipped_reason_not_in_release_period`.
- Aniyomi equivalents: `notification_new_episodes` ("New episodes found"), `notification_episodes_single`, `notification_episodes_single_and_more`, `notification_episodes_multiple`, `notification_episodes_multiple_and_more` (plural), `notification_episodes_generic` (plural), `notification_new_episodes_summary` (plural).
- Download notifier (core): `download_notifier_downloader_title` ("Downloader"), `download_notifier_title_error` ("Error"), `download_notifier_unknown_error`, `download_notifier_text_only_wifi`, `download_notifier_no_network`, `download_notifier_download_paused`, `download_notifier_split_page_not_found` (`%d`), `download_notifier_split_page_path_not_found` (`%d`), `download_notifier_cache_renewal` ("Checking downloads").
- Download notifier (aniyomi): `download_notifier_download_finish` ("Download completed"), `download_notifier_download_paused_chapters`, `download_notifier_download_paused_episodes`.
- Update check: `update_check_notification_download_in_progress`, `update_check_notification_download_complete` ("Tap to install update"), `update_check_notification_download_error`, `update_check_notification_update_available`, and the plural `update_check_notification_ext_updates` ("Extension update available" / "%d extension updates available").
- Extension install service: `ext_install_service_notif` ("Installing extension…").
- Notification channel names (Android channels; see section 8): `channel_common`, `channel_progress`, `channel_complete`, `channel_errors`, `channel_new_chapters` (core, unused), `channel_new_chapters_episodes` (aniyomi, used: "Chapter/Episode updates"), `channel_app_updates`, `channel_ext_updates`.
- Incognito notification action: `notification_incognito_text` ("Disable incognito mode").
- Wi-Fi/no-network downloads: `download_notifier_text_only_wifi`, `download_notifier_no_network`.

### 7.2 Downloads and storage

- Download queue (core): `download_queue_error`, `download_insufficient_space`, `download_queue_size_warning`, `download_queue_summary` (plural, "%1$s remaining"), `label_download_queue`.
- Downloads: `label_downloaded` ("Downloaded"), `download_unread`, `download_unseen` (aniyomi), `download_error`, `download_paused`, `episode_downloading_progress`, `chapter_downloading_progress` (`Downloading (%1$d/%2$d)`), `chapter_error`, `chapter_paused`, `episode_download_progress` (`%1$d%%`).
- Auto-download prefs: `pref_category_auto_download`, `pref_download_new`, `pref_download_new_unread_chapters_only`, `pref_download_new_categories_details`, `download_ahead`, `auto_download_while_reading`, `download_ahead_info`, `save_chapter_as_cbz`, `split_tall_images`, `split_tall_images_summary`. Aniyomi overrides: `download_ahead_info` (core) vs `download_ahead_info_anime` (aniyomi), `pref_download_new_episodes`, `pref_download_new_unseen_episodes_only`, `auto_download_while_watching`, `pref_download_fillermarked_items`, `pref_use_external_downloader`, `pref_external_downloader_selection`, `action_start_download_externally`, `action_start_download_internally`, `pref_download_slots` ("Maximum downloads"), `download_slots_info`, `download_speed_limit` ("Chapter download speed limit"), `download_speed_limit_hint` ("Set to 0 to disable the speed limit.").
- Storage: `no_location_set`, `invalid_location` (`Invalid location: %s`), `pref_storage_location`, `pref_storage_location_info`, `onboarding_storage_*` (section 7.8).
- Cache: `pref_clear_chapter_cache` (core "Clear chapter cache"; aniyomi overrides it as "Clear chapter and episode cache"), `pref_auto_clear_chapter_cache` (aniyomi "Clear episode/chapter cache on app launch"), `cache_deleted` (`Cache cleared, %1$d files deleted`), `cache_delete_error`, `used_cache` (`Used: %1$s`), `used_cache_both` (`Used by anime: %1$s, used by manga: %2$s`), `available_disk_space_info` (`Available: %1$s / Total: %2$s`).

### 7.3 Library and categories

- Sections: `pref_category_library_update`, `pref_library_update_interval`, `update_*` interval names (section 7.5 below), `pref_library_update_restriction`, `connected_to_wifi`, `network_not_metered`, `charging`, `restrictions` (`Restrictions: %s`).
- Smart update: `pref_library_update_smart_update`, `pref_update_only_completely_read`, `pref_update_only_non_completed`, `pref_update_only_started`, `pref_update_only_in_release_period`, `pref_library_update_show_tab_badge`.
- Categories: `default_category`, `default_category_summary` ("Always ask"), `categorized_display_settings`, `pref_library_update_categories_details`, `num_categories` (plural), `category_*` strings such as `delete_category_confirmation` (`Do you wish to delete the category "%s"?`), `error_category_exists`, `snack_categories_deleted`, `information_empty_category`, `information_empty_category_dialog`, `information_no_manga_category`, `information_no_entries_found`.
- Library display: `action_display_*` (grid, list, badges, tabs), `portrait`, `landscape`, `pref_library_columns`.
- Entries: `in_library`, `add_to_library`, `remove_from_library`, `manga_added_library`, `manga_removed_library`, `snack_add_to_library`, `confirm_add_duplicate_manga`, `remove_manga` (`You are about to remove "%s" from your library`), `delete_downloads_for_manga`.

### 7.4 Reader, viewers and page transitions (core)

- Mode names: `left_to_right_viewer`, `right_to_left_viewer`, `vertical_viewer`, `webtoon_viewer` ("Long strip"), `vertical_plus_viewer`, `pager_viewer`, `l_nav`, `kindlish_nav`, `edge_nav`, `right_and_left_nav`, `disabled_nav`, `nav_zone_prev`, `nav_zone_next`, `nav_zone_left`, `nav_zone_right`.
- Transitions: `transition_finished`, `transition_current`, `transition_next`, `transition_previous`, `transition_no_next`, `transition_no_previous`, `transition_pages_loading`, `transition_pages_error` (`Failed to load pages: %1$s`).
- Reader errors: `page_list_empty_error`, `loader_not_implemented_error`, `no_next_chapter`, `decode_image_error`, `chapter_progress` (`Page: %1$d`), `share_page_info` (`%1$s: %2$s, page %3$d`), `confirm_set_image_as_cover`.
- Flash and tap: `pref_flash_page`, `pref_flash_page_summ`, `pref_flash_duration_summary` (`%1$s ms`), `pref_flash_style_*`, `tapping_inverted_*`, `pref_read_with_tapping_inverted`.

### 7.5 Interval and frequency labels

`update_never` ("Off"), `update_6hour` ("Every 6 hours"), `update_12hour` ("Every 12 hours"), `update_24hour` ("Daily"), `update_48hour` ("Every 2 days"), `update_72hour` ("Every 3 days"), `update_weekly` ("Weekly"). Used by the library update interval picker (area:26, area:47).

Manga/anime interval strings: `manga_interval_expected_update` (`New chapters predicted to be released in around %1$s, checking around every %2$s.`), `manga_interval_expected_update_null`, `manga_interval_expected_update_soon` ("Soon"), `manga_interval_custom_amount`, `anime_interval_expected_update`, `anime_interval_expected_update_null`, `action_filter_interval_custom` ("Customized update frequency"), `action_set_interval`.

### 7.6 Tracking

Core: `tracking_guide`, `services`, `tracking_info`, `enhanced_services`, `enhanced_services_not_installed` (`Available but source not installed: %s`), `enhanced_tracking_info`, `track_activity_name`, `pref_auto_update_manga_sync`, `pref_auto_update_manga_on_mark_read`, `track_error` (`%1$s error: %2$s`), `track_remove_date_conf_title`, `track_remove_start_date_conf_text` (`%s`), `track_remove_finish_date_conf_text` (`%s`), `track_delete_title` (`Remove %s tracking?`), `track_delete_text`, `track_delete_remote_text` (`Also remove from %s`), `myanimelist_relogin`, `source_unsupported`, `error_no_match`, `confirm_tracker_update` (`Update trackers to chapter %d?`), `trackers_updated_summary` (`Trackers updated to chapter %d`), `num_trackers` (plural).
Aniyomi: `confirm_tracker_update_anime` (`Update trackers to episode %d?`), `trackers_updated_summary_manga`, `trackers_updated_summary_anime`, `pref_track_on_add_library`, `pref_show_next_episode_airing_time`.
Status and tracker-specific labels: `reading`, `completed`, `dropped`, `on_hold`, `paused`, `plan_to_read`, `repeating`, `reading_list`, `wish_list`, `complete_list`, `on_hold_list`, `unfinished_list`, `score`, `track_status`, `track_started_reading_date`, `track_finished_reading_date`, `tracked_privately`, `action_toggle_private_on`, `action_toggle_private_off`, `track_type`. Aniyomi: `currently_reading`, `currently_watching`, `plan_to_watch`, `not_interesting`, `want_to_read`, `want_to_watch`, `repeating_anime`, `watching`.

### 7.7 Backup and restore (core unless marked)

- Create: `pref_create_backup`, `pref_create_backup_summ`, `creating_backup`, `creating_backup_error`, `backup_created`, `backup_in_progress`, `backup_choice`, `app_settings`, `source_settings`, `extensionRepo_settings`, `private_settings`, `non_library_settings`, `empty_backup_error`, `create_backup_file_error`, `backup_info`, `backup_settings_warning` (aniyomi), `pref_backup_flags`, `pref_backup_flags_summary` (aniyomi), `pref_backup_interval`, `last_auto_backup_info` (`Last automatically backed up: %s`).
- Restore: `pref_restore_backup`, `pref_restore_backup_summ`, `restoring_backup`, `restoring_backup_error`, `restoring_backup_canceled`, `restore_in_progress`, `restore_completed`, `restore_duration` (`%1$02d min, %2$02d sec`), `restore_completed_message` (plural `Done in %1$s with %2$s error(s)`), `restore_miui_warning`, `missing_storage_permission`, `backup_restore_missing_sources`, `backup_restore_missing_trackers`, `backup_restore_content_full`.
- Errors: `invalid_backup_file` ("Invalid backup file:"), `invalid_backup_file_error` ("Full error:"), `invalid_backup_file_missing_manga` ("Backup does not contain any library entries."), `invalid_backup_file_json` ("JSON backup not supported"), `invalid_backup_file_unknown` ("Backup file is corrupted").
- File pickers: `file_select_backup`, `file_select_cover`, `file_picker_error`, `file_picker_uri_permission_unsupported`, `file_null_uri_error`.
- Library export: `export`, `library_list`, `library_exported`, `library_sync_complete`, `syncing_library`.

### 7.8 Onboarding and app setup

`onboarding_heading` ("Welcome!"), `onboarding_description`, `onboarding_action_next`, `onboarding_action_finish` ("Get started"), `onboarding_action_skip`, `onboarding_storage_info` (args: app name, folder path), `onboarding_storage_action_select`, `onboarding_storage_action_create_folder` (aniyomi, "Create default Aniyomi folder"), `onboarding_storage_selection_required`, `onboarding_storage_help_info`, `onboarding_storage_help_action`, `onboarding_permission_*` (install apps, notifications, battery optimisation, crash logs, analytics; each with `_description`), `onboarding_permission_action_grant` ("Grant"), `onboarding_guides_new_user` (`New to %s?…`), `onboarding_guides_returning_user`.

### 7.9 Player (aniyomi only)

The player keys are all in the aniyomi base file. Groups:
- Settings screens: `label_player_settings`, `label_player`, `pref_player_*`, `pref_mpv_*`, `pref_reset_mpv_conf`, `pref_player_torrents_*` (includes `pref_player_torrents_notice_text` legal text, `pref_player_torrents_port_error` "Port must be a number between 0 and 65535", `pref_player_torrents_proxy_url_dialog` "Supports http, https, socks4, socks4a, socks5, socks5h", `pref_player_torrents_proxy_url_invalid_protocol` `Unsupported protocol: "%s"`).
- Gestures and seek: `enable_horizontal_seek_gesture`, `enable_horizontal_seek_gesture_summary`, `pref_skip_*` (10s, 20s, 30s, 5s, 3s), `pref_skip_disable`, `single_action_*`, `player_seek_n_seconds` (`%d seconds`), `player_gesture_seek_indicator`.
- Sheets: `player_sheets_*` (audio/subtitle delay, subtitle colors, typography, border styles, miscellaneous, filters, quality list, decoders, more, stats, chapters, speed, tracks). Full list in the catalog.
- Auto-skip and intro: `player_skip_action`, `player_intro_skipped` (`%1$s skipped`), `player_aniskip_dontskip`, `player_aniskip_dontskip_toast` (`Skipping %s in %d seconds`), `player_aniskip_skip` (`%s skipped`), `pref_enable_intro_skip`, `pref_enable_auto_skip_ani_skip`, `pref_enable_netflix_style_aniskip`, `pref_waiting_time_aniskip` and `pref_waiting_time_aniskip_5..10`, `pref_category_player_aniskip_info` ("AniSkip requires the anime to be tracked with MAL or Anilist to work"), `pref_enable_aniskip`, `pref_disable_aniskip_chapter`.
- Errors: `no_next_episode` ("Next Episode not found!"), `no_prev_episode`, `no_available_videos`, `no_hosters`, `no_episode_loaded`, `select_hoster_from_empty_list`, `player_hoster_failed`, `player_hoster_tap_to_load`, `video_list_empty_error`.
- Sleep timer: `timer_title`, `timer_remaining` (`%s remaining`), `timer_picker_enter_timer`, `timer_picker_pick_time`, `timer_cancel_timer`, `toast_sleep_timer_ended`.
- Script editor and custom buttons: see the catalog (`editor_*`, `pref_player_custom_button_*`). `pref_player_custom_button_delete_confirm` = `Do you wish to delete the button "%s"?`.
- Torrent server: `torrserver_disabled`, `torrentserver_is_running`, `http_server_is_running`, `http_server_start_failure`.

### 7.10 Anime library, history, updates (aniyomi)

`label_anime`, `label_manga`, `label_anime_library`, `label_manga_library`, `label_recent_anime_updates`, `label_anime_history`, `label_anime_updates`, `label_history`, `label_updates`, `label_anime_sources`, `label_manga_sources`, `label_anime_extensions`, `label_manga_extensions`, `label_migration_manga`, `label_migration_anime`, `recent_anime_time` (`Ep. %1$s - %2$s`), `information_no_recent_anime`, `anime_num_*` plurals, `display_mode_episode` ("Episode %1$s"), `display_mode_season` ("Season %1$s"), `show_episode_number`, `show_season_number`, `sort_by_episode_number`, `sort_by_season_number`, `no_episodes_error`, `no_seasons_error`, `episode_settings`, `season_settings`, `episode_settings_updated`, `season_settings_updated`.

### 7.11 Browse and extensions (core, aniyomi overrides)

Core: `ext_*` (status and actions), `untrusted_extension`, `untrusted_extension_message`, `obsolete_extension_message`, `remove_private_extension_message` (`Do you really want to remove "%s" extension?`), `extension_api_error`, `ext_info_*`, `ext_nsfw_*`, `ext_permission_install_apps_warning`, `ext_installer_*`, `ext_revoke_trust`, extension repos (`action_add_repo`, `label_add_repo_input`, `action_add_repo_message`, `error_repo_exists`, `invalid_repo_name`, `delete_repo_confirmation`, `add_repo_confirmation`, `action_replace_repo_title`, `action_replace_repo_message` with `%1$s` and `%2$s`), extension stores (`extensionStores`, `action.addExtensionStore`, `extensionStoresScreen.*`).
Aniyomi: `unofficial_extension_message_aniyomi`, `unofficial_anime_extension_message`, `label_manga_extension_repos`, `label_anime_extension_repos`, `pref_hide_in_manga_library_items`, `pref_hide_in_anime_library_items`, `pref_search_pinned_*_sources_only`, `pref_category_hide_hidden`.
Browse: `latest`, `popular`, `browse`, `has_results`, `no_results_found`, `no_more_results`, `local_source`, `local_manga_source`, `local_anime_source`, `local_source_help_guide`, `last_used_source`, `pinned_sources`, `no_pinned_sources`, `other_source`, `source_empty_screen`, `source_filter_empty_screen`, `action_global_search_hint`, `action_global_search_query` (`Search for "%1$s" globally`).

### 7.12 Crash, about, updates, misc

- Crash screen: `crash_screen_title` ("Whoops!"), `crash_screen_description` (`%s ran into an unexpected error. We suggest you share the crash logs in our support channel on Discord.`), `crash_screen_restart_application`.
- About: `website`, `version`, `whats_new`, `help_translate`, `licenses`, `privacy_policy`, `check_for_updates`, `updated_version` (`Updated to v%1$s`).
- Errors and exceptions: `internal_error`, `unknown_error`, `exception_http` (`HTTP %d, check website in WebView`), `exception_offline` ("No Internet connection"), `exception_unknown_host` (`Couldn't reach %s`), `information_cloudflare_bypass_failure`, `information_cloudflare_help`, `information_webview_required`, `information_webview_outdated`.
- Update check: `update_check_confirm` ("Download"), `update_check_open` ("Open on GitHub"), `update_check_eol` (reserved), `update_check_no_new_updates`.
- Stats: `label_overview_section`, `label_completed_titles`, `label_read_duration`, `label_titles_section`, `label_titles_in_global_update`, `label_total_chapters`, `label_read_chapters`, `label_tracker_section`, `label_tracked_titles`, `label_mean_score`, `label_used`, `label_watched_duration`, `label_watched_episodes`, `not_applicable` ("N/A"), `day_short`, `hour_short`, `minute_short`, `seconds_short`.
- Widget: `appwidget_updates_description`, `appwidget_unavailable_locked`.
- S-Pen: `spen_previous_page`, `spen_next_page` (Android hardware, see section 8).
- Lock and security: `lock_with_biometrics` ("Require unlock"), `lock_when_idle`, `lock_always`, `lock_never`, `lock_after_mins` (plural "After %1$s minute(s)"), `unlock_app_title` ("Unlock %s"), `confirm_lock_change`, `hide_notification_content`, `secure_screen`, `secure_screen_summary`.
- Firebase/ACRA (see section 8): `pref_firebase`, `firebase_summary`, `pref_enable_acra`, `pref_acra_summary`, `pref_dump_crash_logs`, `pref_dump_crash_logs_summary`, `onboarding_permission_crashlytics*`, `onboarding_permission_analytics*`.
- Battery: `pref_disable_battery_optimization`, `pref_disable_battery_optimization_summary`, `battery_optimization_disabled`, `battery_optimization_setting_activity_not_found`, `about_dont_kill_my_app`, `label_background_activity`, `onboarding_permission_ignore_battery_opts*`.
- Data saver (aniyomi): `data_saver*` keys, `bandwidth_hero`, `wsrv` ("wsrv.nl"), `resmush` ("resmush.it"), `bandwidth_data_saver_server`, `data_saver_server_summary`. These describe a remote image-compression service. Its strings are stable text, see section 8 for the network consequence.

### 7.13 Shared generic labels

`on`, `off`, `selected`, `not_selected`, `action_*` (see catalog; 62 Kotlin uses of `action_cancel`, 37 of `action_ok`, 17 of `action_filter`, 17 of `action_delete`), `loading`, `label_default` ("Default"), `label_auto` ("Auto"), `all`, `none`, `include` (`Include: %s`), `exclude` (`Exclude: %s`), `unknown`, `unknown_title`, `unknown_author`, `unknown_status`, `ongoing`, `licensed`, `publishing_finished`, `cancelled`, `on_hiatus`, `not_installed`, `empty_screen` ("Well, this is awkward"), `copy` (noun, "Copy" for migration, not for clipboard; see the comment at strings.xml line 874).

## 8. Android-specific behaviour that Kurobayomi drops or replaces

| Android behaviour (OBSERVED) | Where | Kurobayomi decision |
|---|---|---|
| Per-app language via AppCompat (`setApplicationLocales`) | `AppLanguageScreen.kt:47-57` | Replace: preference `app_language` + catalog lookup at start (section 4.2). |
| `Context.stringResource` on any thread | `Localize.kt` (core) | Replace: resolve strings on the GTK main thread; background code sends keys (section 9). |
| Notification channels created with localized names | `channel_*` keys | Drop channels. GNotification has no channels. Keep the text, use it as notification category labels or in the notification body. |
| `DateUtils.getRelativeTimeSpanString` | `TimeUtils.kt:41` | Replace with Rust relative time built on the catalog (topic file). |
| `DateTimeFormatter.ofLocalizedDate(SHORT)` | `UiPreferences.kt:44-46` | Replace with an ICU-style short date for the resolved locale (topic file). |
| Android `LocaleListCompat` | `LocaleHelper.kt` | Replace with `glib::language_names()` and env vars (topic file). |
| Compose `LayoutDirection` mirroring | implicit | Replace with GTK text direction (section 9.4). |
| Firebase and ACRA crash-log consent | `pref_firebase`, `pref_enable_acra`, onboarding keys | Out of this area. The strings are kept for the mapping in area:35. |
| S-Pen actions | `spen_previous_page`, `spen_next_page` | Drop: no S-Pen hardware on Linux desktops. Keep the strings only if a stylus action is ported. |
| `R.xml.locales_config` (Android's list of supported app languages) | generated file | Replace with the Rust catalog list (topic file). |
| `icerockdev/moko-resources#337` workaround (`fixed()`) | `core/common/.../Localize.kt:28-31` | Drop. Fluent escaping (section 9.3) does not produce the backslash problem. |

## 9. Proposal: Kurobayomi behaviour

### 9.1 Catalog model

- One catalog for both modules. Source: `i18n/…/base/strings.xml` + `plurals.xml` and `i18n-aniyomi/…/base/strings.xml` + `plurals.xml`.
- Precedence on the 27 duplicates: the Aniyomi value wins (it is what the app shows, because the app only references the AYMR copy). Core copies are not generated.
- Keys are converted: dots become underscores (`action.addExtensionStore` → `action_addExtensionStore`) so the Rust key constants are valid identifiers.
- Only keys with at least one reference or an explicit keep decision are generated. Decision for the 133 "no direct reference" names: keep all in the catalog for parity (cost is negligible), and mark them in the key enum with a comment. Final cleanup is in T-51a-019.

### 9.2 Lookup and fallback

1. Requested language list = `[app_language]` if set, else the system list from `glib::language_names()` (section 6 of the topic file).
2. For each tag, try an exact match, then the language-only match (for example `pt-BR` → `pt-BR`, then `pt`), then `en`.
3. Missing message in the chosen bundle: fall back to the English message. This matches Android's `other` fallback and missing-string fallback.
4. Missing key: return the key string and log a warning once per key.
5. Formatting errors reported by Fluent: log, and use the formatted output anyway (never panic).

### 9.3 Escaping

- Source XML escapes (`\'`, `\"`, `&amp;`, `\n`, `\t`) are decoded during conversion, not at runtime. Full rules: [p1-topic-xml-to-fluent.md](p1-topic-xml-to-fluent.md).
- In Fluent, `{` and `}` in text must be written as `{"{"}` and `{"}"}`. Converter must escape them.
- Disable Fluent's isolating marks (`set_use_isolating(false)`), because GTK labels show them as boxes in some fonts.

### 9.4 RTL

- GTK derives the default text direction from the locale at start (`gtk_get_locale_direction`). For a manual `app_language` set to an RTL tag (`ar`, `fa`, `he`, `ur`), call `gtk::Widget::set_default_direction(TextDirection::Rtl)` before building widgets.
- The base strings contain two comments that define direction semantics: `pref_chapter_swipe_end` should sit on the left for RTL locales, `pref_chapter_swipe_start` on the right (`strings.xml` lines 310-313). The Rust settings screen must mirror these labels and the swipe action in RTL (T-51a-014).

### 9.5 Threading

- The catalog is loaded once at start into an immutable structure (concurrent bundle variant so it can be shared).
- Any thread may call `tr()`. UI widgets are created on the GTK main thread. Background tasks (tokio) send `MsgKey` values plus arguments through the UI channel; they never store translated strings.

### 9.6 Locale change

- Setting `app_language` writes the preference. The language change takes effect after restart. The Settings row shows the existing `requires_app_restart` text ("Requires app restart to take effect") next to the picker. This text already exists and is used by `SettingsAdvancedScreen.kt:116` and `:279`.
