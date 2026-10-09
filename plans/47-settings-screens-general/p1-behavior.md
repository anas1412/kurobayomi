# Area 47 (part 1) — behaviour specification

OBSERVED = what the Kotlin/Compose code does (file:line given). PROPOSAL = what the Rust build should do (label `Proposal:`). Strings are quoted by their English value; the key is in the appendix (section 18) and the full key table is `p1-data-and-config.md` section 2 for preference keys. Key naming: `MR.strings.x` = `i18n/src/commonMain/moko-resources/base/strings.xml`, `AYMR.strings.x` = `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`. Plurals live in the matching `plurals.xml`.

---

## 1. Preference tree and rendering rules (all screens)

OBSERVED
- A screen's `getPreferences()` returns `List<Preference>`. Each element is either a `Preference.PreferenceGroup(title, enabled, preferenceItems)` or a top-level `Preference.PreferenceItem` (`app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt`).
- `PreferenceScreen` (`app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt:26-83`):
  - For a group: if `!enabled` the group is skipped entirely (line 53). Otherwise a `PreferenceGroupHeader(title)` is emitted, then each item, then a 12 dp spacer unless it is the last top-level element (lines 66-70).
  - For a top-level item: rendered directly with no header.
- `StatusWrapper` (`PreferenceItem.kt:37-55`): every item is inside `AnimatedVisibility(visible = item.enabled, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut())`. A disabled item is therefore hidden, not greyed. (The Rust mockup's `spec/settings.json` shows greyed rows; this is a parity bug, see `p1-ui-parity.md`.)
- `BasePreferenceWidget` (`widget/BasePreferenceWidget.kt:38-87`): row min height 56 dp (`LocalPreferenceMinHeight`), clickable only when `onClick != null`, title `titleLarge` at 16 sp, max 2 lines with ellipsis, horizontal padding 16 dp, vertical padding 16 dp.
- `PreferenceGroupHeader` (`widget/PreferenceGroupHeader.kt:14-28`): `bodyMedium`, secondary colour, padding bottom 8 dp / top 14 dp, horizontal 16 dp.
- Highlight (search jump): `PreferenceScreen.kt:32-42` — if `SearchableSettings.highlightKey != null`, after `delay(0.5 s)` scroll to the row whose title equals the key (`findHighlightedIndex`, lines 85-97), then reset the key to null. Highlight animation (`BasePreferenceWidget.kt:89-120`): `surfaceTint` at alpha 0.12 while `highlightFlag` is true; the flag is set true, held 3 s, then false; colour animates via `repeatable(iterations = 5, tween(200), RepeatMode.Reverse, StartOffset 600 ms delay)`; when off, `tween(200)`.

PROPOSAL
- Hide disabled rows and groups as above, with a 300 ms `gtk::Revealer` slide (the Compose default `expandVertically` is 300 ms).
- Keep the highlight: a CSS class `highlight` on the row, added after 500 ms, removed after 3 s, with 5 alpha pulses of 200 ms each.

## 2. Widget kinds and interaction

OBSERVED (each kind, `app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt` lines 67-236 and widgets)

| Kind (Kotlin) | Renderer | Interaction | Value write rule |
|---|---|---|---|
| `TextPreference(title, subtitle, icon, enabled, onClick)` | `TextPreferenceWidget` | Row click calls `onClick` if non-null. | No value. |
| `SwitchPreference(preference, title, subtitle, enabled, onValueChanged)` | `SwitchPreferenceWidget` (`widget/SwitchPreferenceWidget.kt:16-38`): switch glyph `onCheckedChange = null`, the **row** toggles | Row click calls `onCheckedChanged(!checked)`, which runs `onValueChanged(new)` in a coroutine (`PreferenceItem.kt:75-81`). | Stored only if `onValueChanged` returns true. |
| `SliderPreference(value, title, valueRange, steps, subtitle, enabled, onValueChanged)` | `BaseSliderItem` (presentation-core) | Drag; `valueText` = subtitle or the value. `steps` default = `(last - first) - 1` (`Preference.kt:68`). | `onValueChanged(it)` called on each change; the setter inside the lambda decides storage. |
| `ListPreference(preference, entries, title, subtitle="%s", subtitleProvider, enabled, onValueChanged)` | `ListPreferenceWidget` (`widget/ListPreferenceWidget.kt:31-81`): modal `AlertDialog` with a radio list, only a Cancel button. Selecting a row (if not already selected) runs `onValueChange(key)` and closes; `isDialogShown = false`. | Stored only if `onValueChanged` returns true (`PreferenceItem.kt:113-115`). Subtitle = `subtitle.format(entries[value])`. | — |
| `MultiSelectListPreference(preference, entries, title, subtitle, enabled, onValueChanged)` | `MultiSelectListPreferenceWidget` (`widget/MultiSelectListPreferenceWidget.kt`): `AlertDialog` with `LabeledCheckbox` per entry, OK and Cancel. Working copy is `selected` (initialised from current values). | OK → `onValuesChange(selected)`; Cancel discards. | Stored as a set only if `onValueChanged` returns true. Default subtitle: labels joined by `, `, or `None` when empty (`Preference.kt:126-133`). |
| `EditTextPreference(preference, title, subtitle="%s", enabled, onValueChanged)` | `EditTextPreferenceWidget` (`widget/EditTextPreferenceWidget.kt:32-122`) | Dialog with one `OutlinedTextField`. Trailing icon: error icon when the text is blank (and blank not allowed) or invalid, else a clear (`Cancel`) icon. Confirm enabled only when `text != value && (notBlank || canBeBlank) && validate(text)`. Confirm runs `onConfirm`, which closes only when `onValueChanged` returns true. | — |
| `InfoPreference(title, enabled)` | `InfoWidget` (`widget/InfoWidget.kt:22-41`): info glyph (`Icons.Outlined.Info`) above `bodySmall` text, secondary alpha, padding 16 dp / 16 dp, spacing 16 dp. | Not interactive. Excluded from search (`SearchResult` `filterNot InfoPreference`). | — |
| `CustomPreference(title, content)` | Calls `content()` directly. Used for segmented buttons (backup), storage bar, and the theme pair. | Owned by the screen. | — |
| `TrackerPreference(tracker, login, logout)` | `TrackingPreferenceWidget` (`widget/TrackingPreferenceWidget.kt:26-63`): logo, name `titleLarge`, green check (`Color(0xFF4CAF50)`, `Icons.Outlined.Done`, 32 dp) when logged in | Click: if logged in → `logout()`, else `login()` (`PreferenceItem.kt:219-228`). `title` is `""` (excluded from search). | — |

Additional observed rules
- `ListPreference` value type is generic (`Int`, `String`, enum). Subtitle default `"%s"`.
- `MultiSelectListPreference` value is stored as `Set<String>`; `onValueChanged` may be the default `{ true }`.

PROPOSAL
- Map each kind to a GTK row: switch → `gtk::Switch` (non-focusable, row toggles); list → modal with `gtk::CheckButton` radio group (`group`), Cancel only; multi → modal with `gtk::CheckButton`, OK+Cancel; slider → `gtk::Scale` with `draw-value`; text → modal with `gtk::Entry`, OK+Cancel; info → dim `gtk::Label` with `dialog-information-symbolic`; tracker → logo + name + `emblem-ok-symbolic` (CSS class `tracker-ok`).
- Keyboard: Escape closes modals (Cancel); Enter in an entry triggers OK when it is enabled.

## 3. Library screen (`SettingsLibraryScreen.kt`)

OBSERVED (`getPreferences()` order: categories, global update, season, anime behaviour, chapter behaviour; lines 60-79)

### 3.1 Group "Categories" (`AYMR.strings.general_categories`, lines 81-155)
1. `TextPreference` `action_edit_anime_categories`; subtitle `num_categories` plural over `userAnimeCategoriesCount` = count of anime categories where `!isSystemCategory` (line 90). `onClick` pushes `CategoriesTab` (line 113).
2. `ListPreference` `default_anime_category` (`AYMR.strings.default_anime_category`). Entries: `animeIds.zip(animeLabels)`, where `animeIds = [libraryPreferences.defaultAnimeCategory().defaultValue()] + allAnimeCategories.map { id.toInt() }` (line 95) and labels `[default_category_summary] + visualName` (line 100). Default value `-1` → label "Always ask".
3. `TextPreference` `action_edit_manga_categories`; `num_categories` for manga user categories; `onClick` pushes `CategoriesTab` and calls `CategoriesTab.showMangaCategory()` (lines 127-130).
4. `ListPreference` `default_manga_category` (`AYMR.strings.default_manga_category`); same shape with manga categories.
5. `SwitchPreference` `categorized_display` (`categorized_display_settings`); `onValueChanged`: when the new value is false, launch `ResetMangaCategoryFlags().await()` in `scope`, then return true (lines 140-147).
6. `SwitchPreference` `hidden_categories` (`pref_category_hide_hidden`).

### 3.2 Group "Global update" (`pref_category_library_update`, lines 157-300)
1. `ListPreference` `pref_library_update_interval_key` (default `0`). Entries (line 227): `0` `update_never`, `12` `update_12hour`, `24` `update_24hour`, `48` `update_48hour`, `72` `update_72hour`, `168` `update_weekly`. `onValueChanged`: `MangaLibraryUpdateJob.setupTask(context, it)` and `AnimeLibraryUpdateJob.setupTask(context, it)`, return true (lines 236-240).
2. `MultiSelectListPreference` `library_update_restriction` (default `{wifi}`). Entries: `DEVICE_ONLY_ON_WIFI` ("wifi") = `connected_to_wifi`; `DEVICE_NETWORK_NOT_METERED` ("network_not_metered") = `network_not_metered`; `DEVICE_CHARGING` ("ac") = `charging`. Subtitle = `restrictions` ("Restrictions: %s"). `enabled = autoUpdateInterval > 0` (line 251). `onValueChanged` posts `setupTask(context)` on the main executor for both jobs (lines 254-257), return true.
3. `TextPreference` `anime_categories` (AYMR); subtitle `getCategoriesLabel(allAnimeCategories, includedAnime, excludedAnime)`; onClick opens `TriStateListDialog` (lines 262-269). Dialog `title` = `anime_categories`, `message` = `pref_anime_library_update_categories_details`. On confirm: sets `animelib_update_categories` (included ids) and `animelib_update_categories_exclude` (excluded ids), closes (lines 176-192).
4. `TextPreference` `manga_categories` (AYMR); same with `library_update_categories` and `library_update_categories_exclude`, message `pref_manga_library_update_categories_details` (lines 195-220, 270-278).
5. `SwitchPreference` `auto_update_metadata` (default false), title `pref_library_update_refresh_metadata`, subtitle `pref_library_update_refresh_metadata_summary`.
6. `MultiSelectListPreference` `library_update_manga_restriction` (smart update). Entries (persistentMapOf, order preserved): `manga_fully_read` = `pref_update_only_completely_read`; `manga_started` = `pref_update_only_started`; `manga_ongoing` = `pref_update_only_non_completed`; `manga_outside_release_period` = `pref_update_only_in_release_period`. Title `pref_library_update_smart_update`. **No `enabled` clause** (default `true`).
7. `SwitchPreference` `library_show_updates_count` default true, title `pref_library_update_show_tab_badge`.

### 3.3 Group "Season" (`pref_library_season`, lines 302-319)
- `pref_update_season_on_refresh` (default false) and `pref_update_season_on_library_update` (default false), titles `pref_update_seasons_refresh` and `pref_update_seasons_update`.

### 3.4 Group "Episode behavior" (`pref_behavior_episode`, lines 370-421)
- `ListPreference` (start / "swipe to left") with `EpisodeSwipeAction` entries: `Disabled` ("Disabled"), `ToggleBookmark` ("Bookmark episode"), `ToggleFillermark` ("Fillermark episode"), `ToggleSeen` ("Mark as watched"), `Download` ("Download"). Title `pref_episode_swipe_start`.
- `ListPreference` (end / "swipe to right") same entries. Title `pref_episode_swipe_end`.
- `MultiSelectListPreference` `mark_duplicate_seen_episode_seen`: `existing_episode` = `pref_mark_duplicate_seen_episode_seen_existing`, `new_episode` = `pref_mark_duplicate_seen_episode_seen_new`.

### 3.5 Group "Chapter behavior" (`pref_behavior`, lines 322-368)
- Same structure. Entries: `Disabled`, `ToggleBookmark` ("Bookmark chapter"), `ToggleRead` ("Mark as read"), `Download`. Titles `pref_chapter_swipe_start` / `pref_chapter_swipe_end`.
- `MultiSelectListPreference` `mark_duplicate_read_chapter_read`: `existing` = `pref_mark_duplicate_read_chapter_read_existing`, `new` = `pref_mark_duplicate_read_chapter_read_new`.

### 3.6 Observed key mapping (important)
- Start/end naming is inverted between the function and the key. `swipeEpisodeStartAction()` reads key `pref_episode_swipe_end_action` (default `ToggleSeen`) (`LibraryPreferences.kt:361-363`). `swipeEpisodeEndAction()` reads `pref_episode_swipe_start_action` (default `ToggleBookmark`) (lines 364-367). Chapter: `swipeChapterStartAction()` → `pref_chapter_swipe_end_action` (default `ToggleRead`); `swipeChapterEndAction()` → `pref_chapter_swipe_start_action` (default `ToggleBookmark`) (lines 369-375).
- `getCategoriesLabel` (Commons.kt) is used for all four category pairs.

PROPOSAL
- Rust keeps the Android keys (for backup compatibility) and names the accessors by UI position (`swipe_left` = key `pref_episode_swipe_end_action`), documented in one comment.

## 4. Reader screen (`SettingsReaderScreen.kt`)

OBSERVED: top-level rows (no header) first, then seven groups.

### 4.1 Top-level rows (lines 36-65)
1. `ListPreference` `pref_default_reading_mode_key` (default `ReadingMode.RIGHT_TO_LEFT.flagValue` = 2). Entries: `ReadingMode.entries.drop(1)` → `LEFT_TO_RIGHT`=1 `left_to_right_viewer`; `RIGHT_TO_LEFT`=2 `right_to_left_viewer`; `VERTICAL`=3 `vertical_viewer`; `WEBTOON`=4 `webtoon_viewer`; `CONTINUOUS_VERTICAL`=5 `vertical_plus_viewer`. Title `pref_viewer_type`.
2. `ListPreference` `pref_double_tap_anim_speed` default 500: `1`→`double_tap_anim_speed_0` ("No animation"), `500`→`double_tap_anim_speed_normal` ("Normal"), `250`→`double_tap_anim_speed_fast` ("Fast").
3. `SwitchPreference` `pref_show_reading_mode` default true (`pref_show_reading_mode_summary`).
4. `SwitchPreference` `reader_navigation_overlay_on_start` default false; title `pref_show_navigation_mode`, subtitle `pref_show_navigation_mode_summary`.
5. `SwitchPreference` `pref_enable_transitions_key` default true; title `pref_page_transitions`.

### 4.2 Group "Display" (`pref_category_display`, lines 77-121)
- `ListPreference` `defaultOrientationType` (`pref_default_orientation_type_key`, default 8 = FREE). Entries: `ReaderOrientation.entries.drop(1)`: FREE 8 (`rotation_free`), PORTRAIT 16 (`rotation_portrait`), LANDSCAPE 24 (`rotation_landscape`), LOCKED_PORTRAIT 32 (`rotation_force_portrait`), LOCKED_LANDSCAPE 40 (`rotation_force_landscape`), REVERSE_PORTRAIT 48 (`rotation_reverse_portrait`). Title `pref_rotation_type`.
- `ListPreference` `readerTheme` (`pref_reader_theme_key`, default 1): `1` black_background, `2` gray_background, `0` white_background, `3` automatic_background. Title `pref_reader_theme`.
- `SwitchPreference` `fullscreen` default true, title `pref_fullscreen`.
- `SwitchPreference` `cutout_short` default true, title `pref_cutout_short`, `enabled = fullscreen && SDK>=P && rootWindowInsets.displayCutout != null` (lines 107-109).
- `SwitchPreference` `pref_keep_screen_on_key` default true, title `pref_keep_screen_on`.
- `SwitchPreference` `pref_show_page_number_key` default true, title `pref_show_page_number`.

### 4.3 Group "E-Ink" (lines 123-178; title is the literal "E-Ink")
- `SwitchPreference` `pref_reader_flash` (default false): title `pref_flash_page`, subtitle `pref_flash_page_summ`.
- `SliderPreference` `value = flashMillis / MILLI_CONVERSION` (integer division; `MILLI_CONVERSION` = 100), `valueRange = 1..15`, title `pref_flash_duration`, subtitle `pref_flash_duration_summary` with the raw ms value (`%1$s ms`), `enabled = flashPageState`, `onValueChanged`: store `it * 100`.
- `SliderPreference` `pref_reader_flash_interval` (default 1), `valueRange = 1..10`, title `pref_flash_page_interval`, subtitle plural `pref_pages` over the value, enabled by flash.
- `ListPreference` `pref_reader_flash_mode` (default `BLACK`): `BLACK`=`pref_flash_style_black`, `WHITE`=`pref_flash_style_white`, `WHITE_BLACK`=`pref_flash_style_white_black`. Title `pref_flash_with`, enabled by flash.

### 4.4 Group "Reading" (lines 181-207)
- `skip_read` (default false) `pref_skip_read_chapters`; `skip_filtered` (default true) `pref_skip_filtered_chapters`; `skip_dupe` (default false) `pref_skip_dupe_chapters`; `always_show_chapter_transition` (default true) `pref_always_show_chapter_transition`; `pref_preserve_reading_position` (default false, AYMR) `pref_preserve_reading_position`.

### 4.5 Group "Paged" (`pager_viewer`, lines 210-304)
- `ListPreference` `reader_navigation_mode_pager` default 0. Entries are `ReaderPreferences.TapZones` indices: `0`=`label_default`, `1`=`l_nav`, `2`=`kindlish_nav`, `3`=`edge_nav`, `4`=`right_and_left_nav`, `5`=`disabled_nav`. Title `pref_viewer_nav`.
- `ListPreference` `reader_tapping_inverted` (default `NONE`): entries NONE, HORIZONTAL, VERTICAL, BOTH (`tapping_inverted_*`). Title `pref_read_with_tapping_inverted`. `enabled = navMode != 5`.
- `ListPreference` `pref_image_scale_type_key` default 1: entries index+1: `1`=`scale_type_fit_screen`, `2`=`scale_type_stretch`, `3`=`scale_type_fit_width`, `4`=`scale_type_fit_height`, `5`=`scale_type_original_size`, `6`=`scale_type_smart_fit`. Title `pref_image_scale_type`.
- `ListPreference` `pref_zoom_start_key` default 1: `1`=`zoom_start_automatic`, `2`=`zoom_start_left`, `3`=`zoom_start_right`, `4`=`zoom_start_center`. Title `pref_zoom_start`.
- `SwitchPreference` `crop_borders` default false, title `pref_crop_borders`.
- `SwitchPreference` `landscape_zoom` default true, title `pref_landscape_zoom`, `enabled = imageScaleType == 1`.
- `SwitchPreference` `navigate_pan` default true, title `pref_navigate_pan`, `enabled = navMode != 5`.
- `SwitchPreference` `pref_dual_page_split` default false, title `pref_dual_page_split`, `onValueChanged`: set `pref_dual_page_rotate` to false, return true.
- `SwitchPreference` `pref_dual_page_invert` default false, title `pref_dual_page_invert`, subtitle `pref_dual_page_invert_summary`, `enabled = dualPageSplit`.
- `SwitchPreference` `pref_dual_page_rotate` default false, title `pref_page_rotate`, `onValueChanged`: set `pref_dual_page_split` to false.
- `SwitchPreference` `pref_dual_page_rotate_invert` default false, title `pref_page_rotate_invert`, `enabled = rotateToFit`.

### 4.6 Group "Long strip" (`webtoon_viewer`, lines 307-407)
- `ListPreference` `reader_navigation_mode_webtoon` default 0, same tap-zone entries.
- `ListPreference` `reader_tapping_inverted_webtoon` default NONE, `enabled = navMode != 5` (uses the webtoon nav mode).
- `SliderPreference` `webtoon_side_padding` (default `WEBTOON_PADDING_MIN` = 0), `valueRange = WEBTOON_PADDING_MIN..WEBTOON_PADDING_MAX` = `0..25`; subtitle `NumberFormat.getPercentInstance().format(value / 100f)` (line 350).
- `ListPreference` `reader_hide_threshold` default `LOW`: `HIGHEST`=`pref_highest`, `HIGH`=`pref_high`, `LOW`=`pref_low`, `LOWEST`=`pref_lowest`. Title `pref_hide_threshold`. (`ReaderHideThreshold` threshold values 5/13/31/47.)
- `SwitchPreference` `crop_borders_webtoon` default false.
- `SwitchPreference` `pref_dual_page_split_webtoon` (default false) — onValueChanged sets `pref_dual_page_rotate_webtoon` false.
- `SwitchPreference` `pref_dual_page_invert_webtoon` (default false), `enabled = dualPageSplit`.
- `SwitchPreference` `pref_dual_page_rotate_webtoon` (default false) — onValueChanged sets `pref_dual_page_split_webtoon` false.
- `SwitchPreference` `pref_dual_page_rotate_invert_webtoon` (default false), `enabled = rotateToFit`.
- `SwitchPreference` `pref_enable_double_tap_zoom_webtoon` default true, title `pref_double_tap_zoom`.
- `SwitchPreference` `webtoon_disable_zoom_out` default false, title `pref_webtoon_disable_zoom_out`.

### 4.7 Group "Navigation" (`pref_reader_navigation`, lines 410-427)
- `reader_volume_keys` (default false) `pref_read_with_volume_keys`; `reader_volume_keys_inverted` (default false) `pref_read_with_volume_keys_inverted`, `enabled = readWithVolumeKeys`.

### 4.8 Group "Actions" (`pref_reader_actions`, lines 430-445)
- `reader_long_tap` (default true) `pref_read_with_long_tap`; `create_folder_per_manga` (default false) `pref_create_folder_per_manga` / `pref_create_folder_per_manga_summary`.

PROPOSAL — dependency graph (task 041). Rules are exactly those above:

| Row | Enabled when |
|---|---|
| Paged: invert tap zones | paged tap zones ≠ 5 |
| Paged: landscape zoom | paged scale type = 1 |
| Paged: pan wide images | paged tap zones ≠ 5 |
| Paged: invert split placement | paged split on |
| Paged: flip rotated pages | paged rotate on |
| Long strip: invert tap zones | webtoon tap zones ≠ 5 |
| Long strip: invert split placement | webtoon split on |
| Long strip: flip rotated pages | webtoon rotate on |
| E-ink duration, interval, colour | e-ink flash on |
| Navigation: invert volume keys | volume keys on |

Mutual exclusion (both viewers): split on ⇒ rotate off; rotate on ⇒ split off (setter side effects, applied on the value write, not as a disabled state).

## 5. Appearance screen (`SettingsAppearanceScreen.kt`)

OBSERVED
- Group "Theme" (`pref_category_theme`, lines 52-99):
  - `CustomPreference` titled `pref_app_theme`: column of `AppThemeModePreferenceWidget(themeMode)` (LIGHT/DARK/SYSTEM) and `AppThemePreferenceWidget(appTheme, amoled)`. Theme-mode change stores the value and calls `setAppCompatDelegateThemeMode`: `LIGHT`→`MODE_NIGHT_NO`, `DARK`→`MODE_NIGHT_YES`, `SYSTEM`→`MODE_NIGHT_FOLLOW_SYSTEM` (`ThemeMode.kt:124-132`).
  - `SwitchPreference` `pref_theme_dark_amoled_key` default false, title `pref_dark_theme_pure_black`, `enabled = themeMode != LIGHT`, `onValueChanged`: `ActivityCompat.recreate(activity)` then true.
- Group "Display" (`pref_category_display`, lines 102-173):
  - `TextPreference` `pref_app_language` → push `AppLanguageScreen()` (part 2 of area 47).
  - `ListPreference` `tablet_ui_mode` (default `AUTOMATIC`): entries `AUTOMATIC`=`automatic_background` ("Auto"), `ALWAYS`=`lock_always` ("Always"), `LANDSCAPE`=`landscape` ("Landscape"), `NEVER`=`lock_never` ("Never"). Title `pref_tablet_ui_mode`. `onValueChanged` toasts `requires_app_restart`.
  - `ListPreference` `start_screen` (default `ANIME`): `ANIME`=`label_anime` ("Anime"), `MANGA`=`manga` ("Manga"), `UPDATES`=`label_recent_updates` ("Updates"), `HISTORY`=`label_recent_manga` ("History"), `BROWSE`=`browse` ("Browse"). Title `pref_start_screen`. Toast `requires_app_restart`.
  - `ListPreference` `bottom_rail_nav_style` (default `MOVE_HISTORY_TO_MORE`), title literal "Navigation Style" (line 149; not translated). Entries `MOVE_MANGA_TO_MORE`=`pref_bottom_nav_no_manga` ("Move Manga to the More tab"), `MOVE_UPDATES_TO_MORE`=`pref_bottom_nav_no_updates`, `MOVE_HISTORY_TO_MORE`=`pref_bottom_nav_no_history`, `MOVE_BROWSE_TO_MORE`=`pref_bottom_nav_no_browse`. `onValueChanged = { true }` (no toast).
  - `ListPreference` `app_date_format` (default `""`). Entries (line 176-182): `""`, `MM/dd/yy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd MMM yyyy`, `MMM dd, yyyy`. Label: `"${pattern.ifEmpty { label_default }} ($formattedDate)"` where `formattedDate = UiPreferences.dateFormat(it).format(LocalDate.now())`.
  - `SwitchPreference` `relative_time_v2` (default true), title `pref_relative_format`, subtitle `pref_relative_format_summary` with args `relative_time_today` and the formatted today string.
- `UiPreferences.dateFormat(format)` (`UiPreferences.kt:44-47`): `""` → `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`; otherwise `ofPattern(format, Locale.getDefault())`.

PROPOSAL
- Pattern mapping for chrono: `MM/dd/yy`→`%m/%d/%y`; `dd/MM/yy`→`%d/%m/%y`; `yyyy-MM-dd`→`%Y-%m-%d`; `dd MMM yyyy`→`%d %b %Y`; `MMM dd, yyyy`→`%b %d, %Y`; `""`→`%x` (C locale short date) — labels formatted from the same function.

## 6. Downloads screen (`SettingsDownloadScreen.kt`)

OBSERVED
- Top-level rows (lines 79-106): `SwitchPreference` `pref_download_only_over_wifi_key` (default true, title `connected_to_wifi`); `TextPreference` `download_speed_limit` (subtitle `off` when 0, else `"$speedLimit KiB/s"`); `SwitchPreference` `save_chapter_as_cbz` (default true, `save_chapter_as_cbz`); `SwitchPreference` `split_tall_images` (default true, subtitle `split_tall_images_summary`); `ListPreference` `download_slots` (default 1, entries `(1..5)` labelled as strings, title `pref_download_slots`); `InfoPreference` `download_slots_info`.
- Speed-limit dialog (lines 65-77, 394-440): `AlertDialog` title `download_speed_limit`, `OutlinedNumericChooser(label = download_speed_limit, placeholder = "0", suffix = "KiB/s", value, step = 100, min = 0)`, text `download_speed_limit_hint`, buttons `action_cancel` and `action_ok`. Confirm stores `currentSpeedLimit`. Note: `currentSpeedLimit` is initialised once via `remember { mutableIntStateOf(speedLimit) }` (line 63), so an external change while the dialog is open is not reflected (see edge cases).
- Group "Delete chapters/episodes" (`pref_category_delete_chapters`, lines 125-168):
  - `SwitchPreference` `pref_remove_after_marked_as_read_key` default false, `pref_remove_after_marked_as_read`.
  - `ListPreference` `remove_after_read_slots` default -1: `-1`=`disabled`, `0`=`last_read_chapter`, `1`=`second_to_last`, `2`=`third_to_last`, `3`=`fourth_to_last`, `4`=`fifth_to_last`. Title `pref_remove_after_read`.
  - `SwitchPreference` `pref_remove_bookmarked` default false, `pref_remove_bookmarked_chapters`.
  - `SwitchPreference` `pref_download_fillermarked` default false, `pref_download_fillermarked_items`.
  - `MultiSelectListPreference` `remove_exclude_anime_categories` (entries = anime categories id→visualName) title `pref_remove_exclude_categories_anime`; then `remove_exclude_categories` (manga) title `pref_remove_exclude_categories_manga`.
- Group "Auto-download" (`pref_category_auto_download`, lines 198-309):
  - `SwitchPreference` `download_new_episode` default false, `pref_download_new_episodes`.
  - `SwitchPreference` `download_new_unread_episodes_only` default false, `pref_download_new_unseen_episodes_only`, enabled by `download_new_episode`.
  - `TextPreference` anime categories (`anime_categories`), subtitle from `getCategoriesLabel` over `download_new_anime_categories` / `_exclude`, `enabled = downloadNewEpisodes`, dialog message `pref_download_new_categories_details`.
  - `SwitchPreference` `download_new` default false, `pref_download_new` ("Download new chapters").
  - `SwitchPreference` `download_new_unread_chapters_only` default false, `pref_download_new_unread_chapters_only`, enabled by `download_new`.
  - `TextPreference` manga categories (`manga_categories`), keys `download_new_categories` / `_exclude`, `enabled = downloadNewChapters`.
- Group "Download ahead" (`download_ahead`, lines 311-349): `ListPreference` `auto_download_while_watching` default 0 with entries `[0, 2, 3, 5, 10]`: 0 → `disabled`; N → plural `next_unseen_episodes` (`one`: "Next unseen episode", `other`: "Next %d unseen episodes"). `ListPreference` `auto_download_while_reading` default 0, same entries, plural `next_unread_chapters`. `InfoPreference` `download_ahead_info`.
- Group "External downloader" (lines 351-391): `SwitchPreference` `use_external_downloader` default false (`pref_use_external_downloader`); `ListPreference` `external_downloader_selection` default `""`, entries `{"" → "None"}` plus installed packages among `idm.internet.download.manager`, `idm.internet.download.manager.plus`, `idm.internet.download.manager.adm.lite`, `com.dv.adm`, labelled with the app label. Title `pref_external_downloader_selection`.

### 6.1 Enable/disable table (task 052)

| Row | Enabled when |
|---|---|
| Auto-download: unseen-only (episodes) | new episodes on |
| Auto-download: anime categories row | new episodes on |
| Auto-download: unread-only (chapters) | new chapters on |
| Auto-download: manga categories row | new chapters on |

## 7. Tracking screen (`SettingsTrackingScreen.kt`)

OBSERVED
- Top-level rows (lines 138-157): `SwitchPreference` `pref_auto_update_manga_sync_key` (default true, `pref_auto_update_manga_sync`); `SwitchPreference` `track_on_adding_to_library` (default true, `pref_track_on_add_library`); `SwitchPreference` `show_next_episode_airing_time` (default true, `pref_show_next_episode_airing_time`); `ListPreference` `pref_auto_update_manga_on_mark_read` (default `ALWAYS`; entries `AutoTrackState.entries` via `titleRes`: ALWAYS=`lock_always` "Always", ASK=`default_category_summary` "Always ask", NEVER=`lock_never` "Never"; title `pref_auto_update_manga_on_mark_read`).
- Group "Trackers" (`services`, lines 158-223): seven `TrackerPreference`s in this order with these login behaviours:
  - MyAnimeList: `context.openInBrowser(MyAnimeListApi.authUrl(), forceDefaultBrowser = true)`; logout → dialog.
  - AniList: `AnilistApi.authUrl()` in browser.
  - Kitsu: `LoginDialog(kitsu, MR.strings.email)`.
  - MangaUpdates: `LoginDialog(mangaUpdates, MR.strings.username)`.
  - Shikimori: `ShikimoriApi.authUrl()`.
  - Simkl: `SimklApi.authUrl()`.
  - Bangumi: `BangumiApi.authUrl()`.
  - Then `InfoPreference(tracking_info)`.
- Group "Enhanced trackers" (`enhanced_services`, lines 224-245): `EnhancedMangaTracker` and `EnhancedAnimeTracker` rows that have an installed accepted source (`partition` first), each `login = loginNoop()`, `logout = service::logout`; then `InfoPreference(enhancedTrackerInfo)`.
- `enhancedTrackerInfo` = `enhanced_tracking_info` + (if any not-installed: `"\n\n" + enhanced_services_not_installed(names joined by ", ")`) (lines 129-136).
- Acceptance rule (lines 116-127): an enhanced manga tracker is "installed" when `mangaSourceManager.getCatalogueSources().any { it::class.qualifiedName in acceptedMangaSources }`; anime uses `animeSourceManager.getAll()` with the same test.

### 7.1 Login dialog (`TrackingLoginDialog`, lines 250-347)
- Title: `login_title` with the tracker name (`"Log in to %1$s"`); close icon button (`action_close`) at the right.
- Fields: username (`uNameStringRes`, `singleLine`, `ImeAction.Next`), password (`password`, visibility toggle icon, `KeyboardType.Password`, `ImeAction.Done`, starts hidden).
- Initial values: `tracker.getUsername()` and `tracker.getPassword()`.
- `isError = inputError && !processing` on both fields.
- Confirm button (`Button`, full width): enabled when `!processing && username.isNotBlank() && password.isNotBlank()`. Label `loading` ("Loading…") while processing else `login` ("Login").
- Click: `scope.launchIO { processing = true; result = checkLogin(context, tracker, username, password); inputError = !result; if (result) onDismissRequest(); processing = false }`.
- `checkLogin` (lines 349-364): `tracker.login(u, p)`; on success toast `login_success` ("Logged in") and return true. On any `Throwable`: `tracker.logout()` (credentials are wiped), toast `e.message.toString()`, return false.

### 7.2 Logout dialog (lines 366-406)
- Title `logout_title` ("Log out from %1$s?"), centred.
- Two equal-width buttons: `OutlinedButton` `action_cancel`, and error-coloured `Button` `logout` ("Log out"). Logout click: `tracker.logout()`, dismiss, toast `logout_success` ("You are now logged out").

### 7.3 App bar action
- Help icon (`HelpOutline`), content description `tracking_guide`, opens `https://aniyomi.org/help/guides/tracking/` via `LocalUriHandler`.

## 8. Browse screen (`SettingsBrowseScreen.kt`)

OBSERVED
- Group "Sources" (`label_sources`, lines 47-81):
  - `SwitchPreference` `browse_hide_in_anime_library_items` default false: `pref_hide_in_anime_library_items`.
  - `SwitchPreference` `browse_hide_in_library_items` default false: `pref_hide_in_manga_library_items`.
  - `TextPreference` `label_anime_extension_repos`; subtitle `num_repos` plural over `animeReposCount` (flow `GetAnimeExtensionStoreCountAsFlow`, collected with initial 0); push `AnimeExtensionStoresScreen`.
  - `TextPreference` `label_manga_extension_repos`; subtitle `num_repos` over `mangaReposCount` (from `GetMangaExtensionRepoCount.subscribe()`); push `MangaExtensionReposScreen`.
- Group "NSFW (18+) sources" (`pref_category_nsfw_content`, lines 82-100):
  - `SwitchPreference` `show_nsfw_source` default **true**: title `pref_show_nsfw_source`, subtitle `requires_app_restart`; `onValueChanged` calls `(context as FragmentActivity).authenticate(title = pref_category_nsfw_content)` — its Boolean result gates the change (the prompt is shown for both directions).
  - `InfoPreference` `parental_controls_info`.

## 9. Security screen (`SettingsSecurityScreen.kt`)

OBSERVED
- `authSupported = context.isAuthenticationSupported()` = `BiometricManager.canAuthenticate(BIOMETRIC_WEAK | DEVICE_CREDENTIAL) == BIOMETRIC_SUCCESS` (`AuthenticatorUtil.kt:87-90`).
- `SwitchPreference` `use_biometric_lock` default false, title `lock_with_biometrics` ("Require unlock"), `enabled = authSupported`, `onValueChanged` → `authenticate(title = lock_with_biometrics)`.
- `ListPreference` `lock_app_after` default 0, entries from `LockAfterValues = [0, 1, 2, 5, 10, -1]` (lines 88-95): `0` → `lock_always`; `-1` → `lock_never`; else plural `lock_after_mins` with count = value. Title `lock_when_idle`. `enabled = authSupported && useAuth`. `onValueChanged` → `authenticate(title = lock_when_idle)`.
- `SwitchPreference` `hide_notification_content` default false, `hide_notification_content`.
- `ListPreference` `secure_screen_v2` default `INCOGNITO`: `ALWAYS`=`lock_always`, `INCOGNITO`=`pref_incognito_mode` ("Incognito mode"), `NEVER`=`lock_never`. Title `secure_screen`.
- `InfoPreference` `secure_screen_summary`.

`AuthenticatorUtil.authenticate` (`AuthenticatorUtil.kt:50-82`): if not supported → resolve **true** immediately. Else start a class-2 biometric or credential prompt; success → true; error → `toast(errString)`, false. Default subtitle `confirm_lock_change` ("Authenticate to confirm change").

## 10. Advanced screen (`SettingsAdvancedScreen.kt`)

OBSERVED (`getPreferences()`, lines 66-111)
1. `TextPreference` `pref_dump_crash_logs` / `pref_dump_crash_logs_summary`: `CrashLogUtil(context).dumpLogs()` in `scope.launch`.
2. `SwitchPreference` `verbose_logging` (default `isDebugBuildType`, i.e. false in release); subtitle `pref_verbose_logging_summary`; `onValueChanged`: toast `requires_app_restart`, return true.
3. `TextPreference` `pref_debug_info` → `DebugInfoScreen()` (part 2).
4. `TextPreference` `pref_onboarding_guide` → `OnboardingScreen()`.
5. `TextPreference` `pref_manage_notifications`: `Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)` with `EXTRA_APP_PACKAGE` = package name.
6. Group `label_background_activity` (lines 115-151):
   - `pref_disable_battery_optimization` (+ `_summary`): if `!isIgnoringBatteryOptimizations(pkg)` → start `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` with data `package:<pkg>` (`FLAG_ACTIVITY_NEW_TASK`); `ActivityNotFoundException` → toast `battery_optimization_setting_activity_not_found`. Else → toast `battery_optimization_disabled`.
   - Literal "Don't kill my app!" with subtitle `about_dont_kill_my_app`, opens `https://dontkillmyapp.com/`.
7. Group `label_data` (lines 154-182):
   - `pref_invalidate_download_cache` (+ summary `pref_invalidate_download_cache_summary`): invalidate `MangaDownloadCache` and `AnimeDownloadCache`, toast `download_cache_invalidated`.
   - `pref_clear_manga_database` (+ summary) → `ClearDatabaseScreen()`.
   - `pref_clear_anime_database` (+ summary) → `ClearAnimeDatabaseScreen()`.
8. Group `label_network` (lines 185-262):
   - `pref_clear_cookies`: `networkHelper.cookieJar.removeAll()`, toast `cookies_cleared`.
   - `pref_clear_webview_data`: `WebView(context)` with `setDefaultSettings()`, `clearCache(true)`, `clearFormData()`, `clearHistory()`, `clearSslPreferences()`; `WebStorage.getInstance().deleteAllData()`; delete `<dataDir>/app_webview/` recursively; toast `webview_data_deleted`. On `Throwable`: log, toast `cache_delete_error`.
   - `doh_provider` ListPreference (default -1): entries `-1`=`disabled`, then names: Cloudflare(1), Google(2), AdGuard(3), Quad9(4), AliDNS(5), DNSPod(6), "360"(7), "Quad 101"(8), Mullvad(9), "Control D"(10), Njalla(11), Shecan(12), LibreDNS(13). Title `pref_dns_over_https`. `onValueChanged` toast `requires_app_restart`.
   - `default_user_agent` EditTextPreference: `onValueChanged`: `Headers.Builder().add("User-Agent", it)` (OkHttp validation); on `IllegalArgumentException` toast `error_user_agent_string_invalid` and return false; else toast `requires_app_restart`, true.
   - `pref_reset_user_agent_string`: `enabled = userAgent != default`; click deletes the key, toast `requires_app_restart`.
9. Group `label_library` (lines 299-347):
   - `pref_refresh_library_covers`: `AnimeLibraryUpdateJob.startNow`, `MangaLibraryUpdateJob.startNow`, `AnimeMetadataUpdateJob.startNow`, `MangaMetadataUpdateJob.startNow`.
   - `pref_reset_viewer_flags` (+ summary `pref_reset_viewer_flags_summary`): `ResetMangaViewerFlags().await()`; toast `pref_reset_viewer_flags_success` or `pref_reset_viewer_flags_error`.
10. Group `pref_category_reader` (lines 350-398):
   - `pref_hardware_bitmap_threshold` ListPreference (default `GLUtil.SAFE_TEXTURE_LIMIT` = 2048). Entries from `CUSTOM_TEXTURE_LIMIT_OPTIONS`: index 0 = `pref_hardware_bitmap_threshold_default` with the value ("Default (%d)"), others the number. Subtitle `pref_hardware_bitmap_threshold_summary` (`"…\nSelected: %s"`). `enabled = !ImageUtil.HARDWARE_BITMAP_UNSUPPORTED && GLUtil.DEVICE_TEXTURE_LIMIT > GLUtil.SAFE_TEXTURE_LIMIT`.
   - `pref_always_decode_long_strip_with_ssiv_2` (+ summary) default false.
   - `pref_display_profile_key`: TextPreference; subtitle = stored URI; click → `OpenDocument` with `"*/*"`, then `takePersistableUriPermission(uri, FLAG_GRANT_READ_URI_PERMISSION)` (no try/catch, line 358) and store the URI.
11. Group `label_extensions` (lines 423-456):
   - `ext_installer_pref` ListPreference over `extension_installer` entries (excluding PRIVATE in release builds). Values: `LEGACY`=`ext_installer_legacy`, `PACKAGEINSTALLER`=`ext_installer_packageinstaller`, `SHIZUKU`=`ext_installer_shizuku`, `PRIVATE`=`ext_installer_private`. Default: LEGACY on MIUI, else PACKAGEINSTALLER. `onValueChanged`: SHIZUKU selected while `!isShizukuInstalled` → show dialog and return false.
   - Dialog: title `ext_installer_shizuku`, text `ext_installer_shizuku_unavailable_dialog`, Cancel and OK (opens `https://shizuku.rikka.app/download`).
   - `ext_revoke_trust`: revoke for both manga and anime extensions, toast `requires_app_restart`.
12. Group `data_saver` (SY block, lines 180-257): see task 086-087 for the full list. Summary of OBSERVED values: `data_saver` default NONE; `data_saver_server` (EditText, default ""); `data_saver_downloader` default true; `ignore_jpeg` default false; `ignore_gif` default true; `data_saver_image_quality` default 80 (entries 10..95); `data_saver_image_format_jpeg` default false (subtitle: `data_saver_image_format_summary_on` / `_off`); `data_saver_color_bw` default false.

## 11. Data and storage screen — scope rows (`SettingsDataScreen.kt`)

OBSERVED
- Row order (lines 111-118): storage location, info text, backup/restore group, data group, export group.
- Storage location (lines 121-188):
  - `storageLocationPicker`: `OpenDocumentTree`; on result: `takePersistableUriPermission(uri, READ|WRITE)` in `try`; `SecurityException` → log and toast `file_picker_uri_permission_unsupported` (the SAF flags comment, lines 134-138, documents Samsung/InkBook behaviour); then `UniFile.fromUri(context, uri)?.let { storageDirPref.set(it.uri.toString()) }`.
  - `storageLocationText`: when not set → `no_location_set`; else `UniFile.fromUri(context, uri).displayablePath` or `invalid_location: <value>` when null.
  - Click: `launch(null)`; `ActivityNotFoundException` → toast `file_picker_error`.
  - Key `__APP_STATE_storage_dir`, default `folderProvider.path()`, but `isSet()` is false until written (`StoragePreferences.kt:122`).
- Backup and restore group (`label_backup`, lines 190-284):
  - Segmented row: "Create backup" (`pref_create_backup`) → `CreateBackupScreen()`; "Restore backup" (`pref_restore_backup`): if `!BackupRestoreJob.isRunning(context)` → (MIUI warning `restore_miui_warning` if `DeviceUtil.isMiui && isMiuiOptimizationDisabled()`) → chooser `GetContent("*/*")` titled `file_select_backup`; else toast `restore_in_progress`. Null result → toast `file_null_uri_error`. Non-null → push `RestoreBackupScreen(uri.toString())`.
  - `ListPreference` `backup_interval` (default 12): entries `0`=`off`, `6`=`update_6hour`, `12`=`update_12hour`, `24`=`update_24hour`, `48`=`update_48hour`, `168`=`update_weekly`. Title `pref_backup_interval`. `onValueChanged` → `BackupCreateJob.setupTask(context, it)`.
  - `InfoPreference`: `backup_info` + `"\n\n"` + `last_auto_backup_info` with `relativeTimeSpanString(lastAutoBackupTimestamp)`.
- Data group (`pref_storage_usage`, lines 287-344):
  - `CustomPreference` titled `pref_storage_usage` containing `StorageInfo` (part 2).
  - `TextPreference` `label_storage` with `Icons.Outlined.Storage` → `StorageTab`.
  - `TextPreference` `pref_clear_chapter_cache` with subtitle `used_cache` (`"Used: %1$s"`) of `chapterCache.readableSize` (cached per `cacheReadableSizeSema`). Click: `chapterCache.clear()` in `launchNonCancellable`; success → toast `cache_deleted` (`"Cache cleared, %1$d files deleted"`) and increment sema; failure → log and toast `cache_delete_error`.
  - `SwitchPreference` `auto_clear_chapter_cache` default false, `pref_auto_clear_chapter_cache`.
- Export group (`export`, lines 347-412):
  - Single `TextPreference` `library_list` ("Library List") → dialog.
  - Dialog state default: all four includes true.
  - `favorites` loaded in `LaunchedEffect(Unit)` from `GetAnimeFavorites.await()` + `GetMangaFavorites.await()` mapped to `ExportEntry`.
  - Save launcher: `CreateDocument("text/csv")`; on URI → `LibraryExporter.exportToCsv(context, uri, favorites, options, onExportComplete)` then toast `library_exported` on main dispatcher.
  - Dialog (`ColumnSelectionDialog`, lines 414-498): title `migration_dialog_what_to_include` ("Select data to include"); checkboxes `title`, `type` (`AYMR.strings.type` "Type"), `author`, `artist`. Unchecking Title sets type/author/artist false; the other three are disabled while Title is off. Confirm (`action_save`) stores options and launches save with filename `aniyomi_library.csv`. Cancel (`action_cancel`).

CSV specification (`LibraryExporter.kt`)
- No header row. One row per favourite entry (anime list first, then manga, because `getAnimeFavorites + getMangaFavorites`).
- Columns in order (only included ones): title, type (`entry.type.name.lowercase()` → `anime` or `manga`), author, artist.
- Null or blank values are empty strings (`isNullOrBlank → ""`).
- Field escaping: if the value contains any of `\r`, `\n`, `"`, `,` → replace `"` with `""` and wrap in `"..."`.
- Row separator `\r\n`; no trailing separator; UTF-8 bytes (`toByteArray()` default charset, which is UTF-8 on Android).
- Empty favourites → empty file (0 bytes), still reports success.

## 12. Restart-required versus live-apply (task 010)

OBSERVED toast or restart uses in scope (all `requires_app_restart` unless noted):

| Setting | Android behaviour | Rust proposal |
|---|---|---|
| Tablet UI mode | toast | Dropped (task 017) |
| Start screen | toast | Applied at next launch; subtitle "Requires app restart to take effect" |
| Verbose logging | toast | Live (tracing filter reload) |
| DoH provider | toast | Live (HTTP client rebuilt) |
| User agent (set/reset) | toast | Live (new requests use it) |
| Extension trust revoke | toast | Live (list refresh) |
| Extension installer | Shizuku-missing dialog | Dropped (task 085) |
| NSFW source | subtitle + restart | Live filter; subtitle kept as in Android |
| Pure black | `ActivityCompat.recreate` | Live CSS swap (task 014) |
| Language | part 2 | part 2 |

## 13. About screen (`about/AboutScreen.kt`)

OBSERVED
- Header `LogoHeader()`, then rows in this order:
  1. `version` (`MR.strings.version` "Version"), subtitle `getVersionName(withBuildDate = true)`, click → `copyToClipboard("Debug information", CrashLogUtil.getDebugInfo())` (lines 88-97).
  2. `check_for_updates` if `updaterEnabled`: `CircularProgressIndicator` (28 dp, stroke 3 dp) while `isCheckingUpdates`. Click when not checking → `checkVersion(context, onAvailableUpdate, onFinish)` in `scope.launch`.
  3. `whats_new` if `!BuildConfig.DEBUG` → `RELEASE_URL`.
  4. `help_translate` → `https://aniyomi.org/docs/contribute#translation`.
  5. `licenses` → `OpenSourceLicensesScreen()`.
  6. `privacy_policy` → `https://aniyomi.org/privacy/`.
  7. Link row (vertical padding 8 dp, centred): `LinkIcon` "Website" (`Icons.Outlined.Public`) → `https://aniyomi.org`; "Discord" (`CustomIcons.Discord`) → `https://discord.gg/F32UjdJZrR`; "GitHub" (`CustomIcons.Github`) → `https://github.com/aniyomiorg/aniyomi`.
- `checkVersion` (lines 203-237): `AppUpdateChecker().checkForUpdate(context, forceCheck = true)` on IO; `NewUpdate` → push `NewUpdateScreen(versionName = release.version, changelogInfo = release.info, releaseLink, downloadLink)`; `NoNewUpdate` → toast `update_check_no_new_updates`; `OsTooOld` → toast `update_check_eol`; other → nothing; exception → toast `e.message`, log; finally `isCheckingUpdates = false`.
- `getVersionName(withBuildDate)` (lines 239-269):
  - `BuildConfig.DEBUG`: `"Debug ${COMMIT_SHA}"` (+ ` (<build date>)`).
  - `isPreviewBuildType`: `"Preview r${COMMIT_COUNT}"` + (`" (${COMMIT_SHA})"` or `" (${COMMIT_SHA}, <date>)"`).
  - else: `"Stable ${VERSION_NAME}"` (+ ` (<date>)`).
- `getFormattedBuildTime` (lines 271-285): `LocalDateTime.ofInstant(Instant.parse(BuildConfig.BUILD_TIME), ZoneId.systemDefault()).toDateTimestampString(UiPreferences.dateFormat(dateFormat))`; on exception returns the raw `BUILD_TIME`.
- `RELEASE_URL` = `https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG`; `RELEASE_TAG` = `r${COMMIT_COUNT}` for preview, else `v${VERSION_NAME}` (`AppUpdateChecker.kt:43-59`).
- `updaterEnabled` = `BuildConfig.UPDATER_ENABLED`; `isReleaseBuildType` = `BUILD_TYPE == "release"` (`BuildConfig.kt`).

PROPOSAL: the Rust about screen shows the same rows; the version string uses `VERSION` from `Cargo.toml` (`0.1.0` in the mockup) with `Stable` prefix for tagged builds and `Debug <git sha>` for dev builds.

## 14. Clear database screens (`advanced/ClearDatabaseScreen.kt`, `advanced/ClearAnimeDatabaseScreen.kt`)

OBSERVED
- States: `Loading` → `LoadingScreen()`; `Ready(items, selection, showConfirmation)`.
- Load (manga): `GetMangaSourcesWithNonLibraryManga.subscribe()` → `collectLatest` → items `sortedBy { it.name }`; state moves Loading → Ready, later emissions replace `items` and keep selection.
- Load (anime): `GetAnimeSourcesWithNonLibraryAnime.subscribe()` → `groupBy { sourceId }` → for each: source via `sourceManager.getOrStub(sourceId)`, `mapSourceToDomainSource(source).copy(isStub = source is StubAnimeSource)`, ids and orphaned lists built by iterating the group's anime; each anime with `fetchType == Seasons` adds its deletable children via `getDeletableChildren(animeId)` (recursive; favourite children go to `orphaned`, non-favourite to `ids`, recursing again into Seasons children). Result list sorted by name.
- Top bar: title `pref_clear_manga_database` / `pref_clear_anime_database`; actions only when items exist: `action_select_all` (`SelectAll` icon) and the inverse action (`FlipToBack` icon). Manga inverse label `action_select_inverse`; **anime inverse label is `action_select_all`** (ClearAnimeDatabaseScreen.kt:122, a copy-paste bug).
- Body: when empty → `EmptyScreen(database_clean)` ("Nothing to clear"). Otherwise:
  - Manga: `LazyColumnWithAction` with action label `action_delete`, action enabled when selection is non-empty, action → `showConfirmation`.
  - Anime: `Column` of a `LazyColumn` (weight 1) then `HorizontalDivider`, then a full-width `Button` (padding 16 dp horizontal, 8 dp vertical) with `action_delete`, enabled when selection non-empty.
- Item row: height 56 dp, horizontal padding 8 dp, `selectedBackground(isSelected)`, click toggles selection, source icon (`MangaSourceIcon` / `AnimeSourceIcon`), source `visualName` (`bodyMedium`), `clear_database_source_item_count` text with the count (`"%1$d non-library entries in database"`), trailing checkbox.
- Selection operations (both): toggle (add/remove by source id), clear (empty list), select all (all item ids), invert (all ids not currently selected). All are no-ops unless state is Ready.
- Confirm dialog: text `clear_database_confirmation`; OK → `removeXBySourceId()`, clear selection, hide dialog, toast `clear_database_completed` ("Entries deleted"); Cancel → hide.
- Delete (manga): `database.mangasQueries.deleteMangasNotInLibraryBySourceIds(selection)` then `database.historyQueries.removeResettedHistory()`, inside `withNonCancellableContext`.
- Delete (anime): `selected = items.filter { it.id in selection }`; `animeIds = selected.flatMap { ids }`; `orphaned = selected.flatMap { orphaned }.filterNot { it in animeIds }`; `animesQueries.deleteAnimesNotInLibraryByAnimeIds(animeIds)`; `animesQueries.removeParentIdByIds(orphaned)`; `animehistoryQueries.removeResettedHistory()`.

PROPOSAL
- The SQL semantics are listed in `p1-data-and-config.md` section 5 and tested in `p1-edge-cases-and-tests.md` section 6.

## 15. Settings search (`SettingsSearchScreen.kt`)

OBSERVED (full rules, lines 173-238, 288-309)
- Index = `settingScreens` (non-player) list `[Appearance, Library, Reader, Download, Tracking, Browse, Data, Security, Advanced]` (note: Download comes before Tracking here; the main menu order differs). Player index = `playerSettingScreens` (`[Player, Gestures, Decoder, Subtitle, Audio, Torrent, Advanced]`) appended when not in player mode, with `playerSettings = true`.
- Each index entry: `title = stringResource(screen.getTitleRes())`, `contents = screen.getPreferences()`.
- Flatten: for each entry, for each content element: groups only if `enabled` (their items are then filtered by `enabled && title.isNotBlank()`); top-level items as `(null, item)`; filter `enabled && title.isNotBlank()` applied on the group or item; drop `InfoPreference`; match `title.contains(query, ignoreCase)` **or** `subtitle?.contains(query, ignoreCase)`.
- Map each to `SearchResultItem(route = screen, title, breadcrumbs, highlightKey = title)`. Breadcrumb nodes: `[AYMR.label_player_settings]` if not player mode and the entry is a player entry; then `entry.title`; then the group title if present. Joined with ` > ` (LTR) or reversed with ` < ` (RTL).
- `.take(10)` after flattening (line 236), so the first 10 hits in index order (not ranked).
- Empty query → `SearchResult` returns immediately (no list). Results `null` (loading) → nothing shown; empty → `EmptyScreen(no_results_found)`.
- Selecting a result: `SearchableSettings.highlightKey = result.highlightKey; navigator.replace(result.route)` (lines 166-167).
- Keyboard: text field requests focus on launch; `ImeAction.Search`; `runOnEnterKeyPressed(focusManager::clearFocus)`; list scroll clears focus; leaving the screen hides the soft keyboard.

PROPOSAL: the Rust search index is built once per settings opening from the same Rust tree (no rebuilding per keystroke). Ranking stays "first 10 in index order" for parity; the breadcrumbs keep the same text.

## 16. Settings main menu and shared helper (`SettingsMainScreen.kt`, `Commons.kt`)

OBSERVED (menu rows, lines 178-249) — title, subtitle key, icon, destination:
1. `pref_category_appearance` / `pref_appearance_summary` / Palette / `SettingsAppearanceScreen`
2. `pref_category_library` / `pref_library_summary` / CollectionsBookmark / `SettingsLibraryScreen`
3. `pref_category_reader` / `pref_reader_summary` / ChromeReaderMode / `SettingsReaderScreen`
4. `label_player` / `pref_player_settings_summary` / VideoSettings / `PlayerSettingsScreen(mainSettings = true)` (player area)
5. `pref_category_downloads` / `pref_downloads_summary` / GetApp / `SettingsDownloadScreen`
6. `pref_category_tracking` / `pref_tracking_summary` / Sync / `SettingsTrackingScreen`
7. `browse` / `pref_browse_summary` / Explore / `SettingsBrowseScreen`
8. `label_data_storage` / `pref_backup_summary` / Storage / `SettingsDataScreen`
9. `pref_category_security` / `pref_security_summary` / Security / `SettingsSecurityScreen`
10. `pref_category_advanced` / `pref_advanced_summary` / Code / `SettingsAdvancedScreen`
11. `pref_category_about` / subtitle = `"${app_name} ${getVersionName(false)}"` / Info / `AboutScreen`

- Top bar: title `label_settings` ("Settings"); action `action_search` (Search icon) → `SettingsSearchScreen`.
- Two-pane: container colour = `ColorUtils.colorToHSL(surface)`, lightness `+0.02` (light) or `−0.05` (dark), clamped to [0,1]; selected row gets `surfaceVariant` background, 24 dp rounded corners, 8 dp horizontal padding, and `onSurfaceVariant` content colour; selection is by screen class; initial scroll animates to the selected item and lifts the top bar when index > 0.

`getCategoriesLabel` (Commons.kt:16-53) — see T-47a-005 for the full rules; summary: included/excluded ids resolved and sorted by category `order`; labels from `visualName`; "All"/"None" logic as tasks.

## 17. Open-source licence screens

OBSERVED
- `OpenSourceLicensesScreen`: `LibrariesContainer` (AboutLibraries m3) fills the page; click on a library → `OpenSourceLibraryLicenseScreen(name = it.name, website = it.website, license = it.licenses.firstOrNull()?.htmlReadyLicenseContent.orEmpty())`.
- `OpenSourceLibraryLicenseScreen`: top bar title = library name; action "Website" (`Icons.Default.Public`, label `website`) only when `website` is non-empty → `openUri(website)`. Body: vertical scroll, padding 16 dp, `AndroidView(MaterialTextView)` with `HtmlCompat.fromHtml(html, FROM_HTML_MODE_COMPACT)`.

PROPOSAL: `cargo-about` JSON gives name, website, licence text; the detail page shows the licence as plain text in a `gtk::Label` (wrap, selectable). HTML is not interpreted.

## 18. Appendix — user-visible strings used by these screens

Generated from the moko-resources files for every `MR.strings.*`, `MR.plurals.*`, `AYMR.strings.*` and `AYMR.plurals.*` reference in the 18 scope files plus the enum and helper files they depend on (`.keys_build.py`, 425 keys). `files` lists the source files that reference the key. Strings with `\n` are shown with the escape. Plurals show `quantity: text`.

| Key | English text | Used in |
|---|---|---|
| `AYMR.strings.action_bookmark_episode` | Bookmark episode | SettingsLibraryScreen.kt |
| `AYMR.strings.action_edit_anime_categories` | Edit anime categories | SettingsLibraryScreen.kt |
| `AYMR.strings.action_edit_manga_categories` | Edit manga categories | SettingsLibraryScreen.kt |
| `AYMR.strings.action_fillermark_episode` | Fillermark episode | SettingsLibraryScreen.kt |
| `AYMR.strings.action_mark_as_seen` | Mark as watched | SettingsLibraryScreen.kt |
| `AYMR.strings.action_search_player_settings` | Search player settings | SettingsSearchScreen.kt |
| `AYMR.strings.anime_categories` | Anime Categories | SettingsDownloadScreen.kt, SettingsLibraryScreen.kt |
| `AYMR.strings.auto_download_while_watching` | Auto download while watching | SettingsDownloadScreen.kt |
| `AYMR.strings.bandwidth_data_saver_server` | Bandwidth Hero Proxy Server | SettingsAdvancedScreen.kt |
| `AYMR.strings.bandwidth_hero` | Bandwidth Hero (requires a Bandwidth Hero Proxy server) | SettingsAdvancedScreen.kt |
| `AYMR.strings.clear_database_confirmation` | Are you sure? Completed episodes and chapters and progress of non-library entries will be lost | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt |
| `AYMR.strings.data_saver` | Data Saver | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_color_bw` | Convert to Black And White | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_downloader` | Use data saver in the downloader | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_ignore_gif` | Ignore Gif Animations | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_ignore_jpeg` | Ignore Jpeg Images | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_image_format` | Compress to Jpeg | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_image_format_summary_off` | The Jpeg file size is considerably smaller then Webp is(meaning more data is saved), but it makes the images lose more quality as well.\nCurrently compresses to Webp | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_image_format_summary_on` | The Jpeg file size is considerably smaller then Webp is(meaning more data is saved), but it makes the images lose more quality as well.\nCurrently compresses to Jpeg | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_image_quality` | Image Quality | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_image_quality_summary` | Higher values mean that a higher percentage of the image quality is saved, but it also means the file size is larger, 80 percent is a good median between file size and image quality | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_server_summary` | Put Bandwidth Hero Proxy server url here | SettingsAdvancedScreen.kt |
| `AYMR.strings.data_saver_summary` | Compress images before downloading or loading in reader | SettingsAdvancedScreen.kt |
| `AYMR.strings.default_anime_category` | Default anime category | SettingsLibraryScreen.kt |
| `AYMR.strings.default_manga_category` | Default manga category | SettingsLibraryScreen.kt |
| `AYMR.strings.download_ahead_info` | Only works if the current chapter/episode + the next one are already downloaded. | SettingsDownloadScreen.kt |
| `AYMR.strings.download_slots_info` | Will only download concurrently from self-hosted or unmetered sources | SettingsDownloadScreen.kt |
| `AYMR.strings.download_speed_limit` | Chapter download speed limit | SettingsDownloadScreen.kt |
| `AYMR.strings.download_speed_limit_hint` | Set to 0 to disable the speed limit. | SettingsDownloadScreen.kt |
| `AYMR.strings.general_categories` | Categories | SettingsLibraryScreen.kt |
| `AYMR.strings.label_anime` | Anime | StartScreen.kt |
| `AYMR.strings.label_anime_extension_repos` | Anime extension repos | SettingsBrowseScreen.kt |
| `AYMR.strings.label_manga_extension_repos` | Manga extension repos | SettingsBrowseScreen.kt |
| `AYMR.strings.label_player` | Player | SettingsMainScreen.kt |
| `AYMR.strings.label_player_settings` | Player settings | SettingsSearchScreen.kt |
| `AYMR.strings.label_storage` | Storage | SettingsDataScreen.kt |
| `AYMR.strings.manga` | Manga | StartScreen.kt |
| `AYMR.strings.manga_categories` | Manga Categories | SettingsDownloadScreen.kt, SettingsLibraryScreen.kt |
| `AYMR.plurals.next_unseen_episodes` | one: Next unseen episode; other: Next %d unseen episodes | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_anime_library_update_categories_details` | Anime in excluded categories will not be updated even if they are also in included categories. | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_auto_clear_chapter_cache` | Clear episode/chapter cache on app launch | SettingsDataScreen.kt |
| `AYMR.strings.pref_auto_update_manga_on_mark_read` | Update progress when marked as read/watched | SettingsTrackingScreen.kt |
| `AYMR.strings.pref_auto_update_manga_sync` | Update progress after reading/watching | SettingsTrackingScreen.kt |
| `AYMR.strings.pref_behavior` | Chapter behavior | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_behavior_episode` | Episode behavior | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_bottom_nav_no_browse` | Move Browse to the More tab | NavStyle.kt |
| `AYMR.strings.pref_bottom_nav_no_history` | Move History to the More tab | NavStyle.kt |
| `AYMR.strings.pref_bottom_nav_no_manga` | Move Manga to the More tab | NavStyle.kt |
| `AYMR.strings.pref_bottom_nav_no_updates` | Move Updates to the More tab | NavStyle.kt |
| `AYMR.strings.pref_category_delete_chapters` | Delete chapters/episodes | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_category_external_downloader` | External downloader | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_category_hide_hidden` | Hide hidden categories from categories and storage screen | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_clear_anime_database` | Clear Anime database | ClearAnimeDatabaseScreen.kt, SettingsAdvancedScreen.kt |
| `AYMR.strings.pref_clear_anime_database_summary` | Delete history for anime that are not saved in your library | SettingsAdvancedScreen.kt |
| `AYMR.strings.pref_clear_chapter_cache` | Clear chapter and episode cache | SettingsDataScreen.kt |
| `AYMR.strings.pref_clear_manga_database` | Clear Manga database | ClearDatabaseScreen.kt, SettingsAdvancedScreen.kt |
| `AYMR.strings.pref_clear_manga_database_summary` | Delete history for manga that are not saved in your library | SettingsAdvancedScreen.kt |
| `AYMR.strings.pref_download_fillermarked_items` | Allow downloading of fillermarked episodes | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_download_new_episodes` | Download new episodes | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_download_new_unseen_episodes_only` | Skip downloading duplicate seen episodes | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_download_slots` | Maximum downloads | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_episode_swipe_end` | Episode on swipe to right | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_episode_swipe_start` | Episode on swipe to left | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_external_downloader_selection` | Downloader app preference | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_hide_in_anime_library_items` | Hide anime entries already in library | SettingsBrowseScreen.kt |
| `AYMR.strings.pref_hide_in_manga_library_items` | Hide manga entries already in library | SettingsBrowseScreen.kt |
| `AYMR.strings.pref_invalidate_download_cache_summary` | Force app to recheck downloaded chapters and episodes | SettingsAdvancedScreen.kt |
| `AYMR.strings.pref_library_season` | Season | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_library_summary` | Categories, global update, chapter/episode swipe | SettingsMainScreen.kt |
| `AYMR.strings.pref_library_update_show_tab_badge` | Show unseen/unread count on Updates icon | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_manga_library_update_categories_details` | Manga in excluded categories will not be updated even if they are also in included categories. | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_mark_duplicate_seen_episode_seen` | Mark duplicate seen episode as seen | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_mark_duplicate_seen_episode_seen_existing` | After watching an episode | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_mark_duplicate_seen_episode_seen_new` | After fetching new episode | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_player_settings_summary` | Internal player, gestures, decoders, subtitles | SettingsMainScreen.kt |
| `AYMR.strings.pref_preserve_reading_position` | Preserve reading position on read chapters | SettingsReaderScreen.kt |
| `AYMR.strings.pref_remove_after_marked_as_read` | After manually marked as read/watched | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_remove_after_read` | After reading/watching automatically delete | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_remove_bookmarked_chapters` | Allow deleting bookmarked chapters/episodes | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_remove_exclude_categories_anime` | Excluded anime categories | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_remove_exclude_categories_manga` | Excluded manga categories | SettingsDownloadScreen.kt |
| `AYMR.strings.pref_show_next_episode_airing_time` | Show next episode\'s airing time  | SettingsTrackingScreen.kt |
| `AYMR.strings.pref_start_screen` | Start Screen | SettingsAppearanceScreen.kt |
| `AYMR.strings.pref_track_on_add_library` | Open track menu on adding to library | SettingsTrackingScreen.kt |
| `AYMR.strings.pref_update_only_completely_read` | Skip entries with unseen episode(s)/unread chapter(s) | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_update_seasons_refresh` | Update seasons with episodes on refresh | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_update_seasons_update` | Update seasons with episodes on library update | SettingsLibraryScreen.kt |
| `AYMR.strings.pref_use_external_downloader` | Always use external downloader for anime | SettingsDownloadScreen.kt |
| `AYMR.strings.resmush` | resmush.it | SettingsAdvancedScreen.kt |
| `AYMR.strings.theme_cloudflare` | Cloudflare | AppTheme.kt |
| `AYMR.strings.theme_cottoncandy` | Cotton Candy | AppTheme.kt |
| `AYMR.strings.theme_doom` | Doom | AppTheme.kt |
| `AYMR.strings.theme_matrix` | Matrix | AppTheme.kt |
| `AYMR.strings.theme_mocha` | Mocha | AppTheme.kt |
| `AYMR.strings.theme_sapphire` | Sapphire | AppTheme.kt |
| `AYMR.strings.type` | Type | SettingsDataScreen.kt |
| `AYMR.strings.wsrv` | wsrv.nl | SettingsAdvancedScreen.kt |
| `MR.strings.about_dont_kill_my_app` | Some manufacturers have additional app restrictions that kill background services. This website has more info on how to fix it. | SettingsAdvancedScreen.kt |
| `MR.strings.action_bookmark` | Bookmark chapter | SettingsLibraryScreen.kt |
| `MR.strings.action_cancel` | Cancel | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt, ListPreferenceWidget.kt, SettingsAdvancedScreen.kt, SettingsDataScreen.kt, SettingsDownloadScreen.kt, SettingsTrackingScreen.kt, TriStateListDialog.kt |
| `MR.strings.action_close` | Close | SettingsTrackingScreen.kt |
| `MR.strings.action_delete` | Delete | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt |
| `MR.strings.action_download` | Download | SettingsLibraryScreen.kt |
| `MR.strings.action_mark_as_read` | Mark as read | SettingsLibraryScreen.kt |
| `MR.strings.action_ok` | OK | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt, SettingsAdvancedScreen.kt, SettingsDownloadScreen.kt, TriStateListDialog.kt |
| `MR.strings.action_save` | Save | SettingsDataScreen.kt |
| `MR.strings.action_search` | Search | SettingsMainScreen.kt |
| `MR.strings.action_search_settings` | Search settings | SettingsSearchScreen.kt |
| `MR.strings.action_select_all` | Select all | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt |
| `MR.strings.action_select_inverse` | Select inverse | ClearDatabaseScreen.kt |
| `MR.strings.all` | All | Commons.kt |
| `MR.strings.app_name` | Aniyomi | SettingsMainScreen.kt |
| `MR.strings.artist` | Artist | SettingsDataScreen.kt |
| `MR.strings.author` | Author | SettingsDataScreen.kt |
| `MR.strings.auto_download_while_reading` | Auto download while reading | SettingsDownloadScreen.kt |
| `MR.strings.automatic_background` | Auto | SettingsReaderScreen.kt, TabletUiMode.kt |
| `MR.strings.backup_info` | You should keep copies of backups in other places as well. Backups may contain sensitive data including any stored passwords; be careful if sharing. | SettingsDataScreen.kt |
| `MR.strings.battery_optimization_disabled` | Battery optimization is already disabled | SettingsAdvancedScreen.kt |
| `MR.strings.battery_optimization_setting_activity_not_found` | Couldn\'t open device settings | SettingsAdvancedScreen.kt |
| `MR.strings.black_background` | Black | SettingsReaderScreen.kt |
| `MR.strings.browse` | Browse | SettingsBrowseScreen.kt, SettingsMainScreen.kt, StartScreen.kt |
| `MR.strings.cache_delete_error` | Error occurred while clearing | SettingsAdvancedScreen.kt, SettingsDataScreen.kt |
| `MR.strings.cache_deleted` | Cache cleared, %1$d files deleted | SettingsDataScreen.kt |
| `MR.strings.categorized_display_settings` | Per-category settings for sort | SettingsLibraryScreen.kt |
| `MR.strings.charging` | When charging | SettingsLibraryScreen.kt |
| `MR.strings.check_for_updates` | Check for updates | AboutScreen.kt |
| `MR.strings.clear_database_completed` | Entries deleted | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt |
| `MR.strings.clear_database_source_item_count` | %1$d non-library entries in database | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt |
| `MR.strings.confirm_lock_change` | Authenticate to confirm change | AuthenticatorUtil.kt |
| `MR.strings.connected_to_wifi` | Only on Wi-Fi | SettingsDownloadScreen.kt, SettingsLibraryScreen.kt |
| `MR.strings.cookies_cleared` | Cookies cleared | SettingsAdvancedScreen.kt |
| `MR.strings.database_clean` | Nothing to clear | ClearAnimeDatabaseScreen.kt, ClearDatabaseScreen.kt |
| `MR.strings.day_short` | %dd | TimeUtils.kt |
| `MR.strings.default_category_summary` | Always ask | AutoTrackState.kt, SettingsLibraryScreen.kt |
| `MR.strings.disabled` | Disabled | SettingsAdvancedScreen.kt, SettingsDownloadScreen.kt, SettingsLibraryScreen.kt, TriStateListDialog.kt |
| `MR.strings.disabled_nav` | Disabled | ReaderPreferences.kt |
| `MR.strings.double_tap_anim_speed_0` | No animation | SettingsReaderScreen.kt |
| `MR.strings.double_tap_anim_speed_fast` | Fast | SettingsReaderScreen.kt |
| `MR.strings.double_tap_anim_speed_normal` | Normal | SettingsReaderScreen.kt |
| `MR.strings.download_ahead` | Download ahead | SettingsDownloadScreen.kt |
| `MR.strings.download_cache_invalidated` | Downloads index invalidated | SettingsAdvancedScreen.kt |
| `MR.strings.edge_nav` | Edge | ReaderPreferences.kt |
| `MR.strings.email` | Email address | SettingsTrackingScreen.kt |
| `MR.strings.enhanced_services` | Enhanced trackers | SettingsTrackingScreen.kt |
| `MR.strings.enhanced_services_not_installed` | Available but source not installed: %s | SettingsTrackingScreen.kt |
| `MR.strings.enhanced_tracking_info` | Provides enhanced features for specific sources. Entries are automatically tracked when added to your library. | SettingsTrackingScreen.kt |
| `MR.strings.error_user_agent_string_invalid` | Invalid user agent string | SettingsAdvancedScreen.kt |
| `MR.strings.exclude` | Exclude: %s | Commons.kt |
| `MR.strings.export` | Export | SettingsDataScreen.kt |
| `MR.strings.ext_installer_legacy` | Legacy | BasePreferences.kt |
| `MR.strings.ext_installer_packageinstaller` | PackageInstaller | BasePreferences.kt |
| `MR.strings.ext_installer_pref` | Installer | SettingsAdvancedScreen.kt |
| `MR.strings.ext_installer_private` | Private | BasePreferences.kt |
| `MR.strings.ext_installer_shizuku` | Shizuku | BasePreferences.kt, SettingsAdvancedScreen.kt |
| `MR.strings.ext_installer_shizuku_unavailable_dialog` | Install and start Shizuku to use Shizuku as extension installer. | SettingsAdvancedScreen.kt |
| `MR.strings.ext_revoke_trust` | Revoke trusted unknown extensions | SettingsAdvancedScreen.kt |
| `MR.strings.fifth_to_last` | Fifth to last read chapter | SettingsDownloadScreen.kt |
| `MR.strings.file_null_uri_error` | No file selected | SettingsDataScreen.kt |
| `MR.strings.file_picker_error` | No file picker app found | SettingsDataScreen.kt |
| `MR.strings.file_picker_uri_permission_unsupported` | Failed to acquire persistent folder access. The app may behave unexpectedly. | SettingsDataScreen.kt |
| `MR.strings.file_select_backup` | Select backup file | SettingsDataScreen.kt |
| `MR.strings.filter_mode_darken` | Burn / Darken | ReaderPreferences.kt |
| `MR.strings.filter_mode_lighten` | Dodge / Lighten | ReaderPreferences.kt |
| `MR.strings.filter_mode_multiply` | Multiply | ReaderPreferences.kt |
| `MR.strings.filter_mode_overlay` | Overlay | ReaderPreferences.kt |
| `MR.strings.filter_mode_screen` | Screen | ReaderPreferences.kt |
| `MR.strings.fourth_to_last` | Fourth to last read chapter | SettingsDownloadScreen.kt |
| `MR.strings.gray_background` | Gray | SettingsReaderScreen.kt |
| `MR.strings.help_translate` | Help translate | AboutScreen.kt |
| `MR.strings.hide_notification_content` | Hide notification content | SettingsSecurityScreen.kt |
| `MR.strings.hour_short` | %dh | TimeUtils.kt |
| `MR.strings.include` | Include: %s | Commons.kt |
| `MR.strings.invalid_location` | Invalid location: %s | SettingsDataScreen.kt |
| `MR.strings.kindlish_nav` | Kindle-ish | ReaderPreferences.kt |
| `MR.strings.l_nav` | L shaped | ReaderPreferences.kt |
| `MR.strings.label_background_activity` | Background activity | SettingsAdvancedScreen.kt |
| `MR.strings.label_backup` | Backup and restore | SettingsDataScreen.kt |
| `MR.strings.label_data` | Data | SettingsAdvancedScreen.kt |
| `MR.strings.label_data_storage` | Data and storage | SettingsDataScreen.kt, SettingsMainScreen.kt |
| `MR.strings.label_default` | Default | AppTheme.kt, CategoryExtensions.kt, ReaderOrientation.kt, ReaderPreferences.kt, ReadingMode.kt, SettingsAppearanceScreen.kt |
| `MR.strings.label_extensions` | Extensions | SettingsAdvancedScreen.kt |
| `MR.strings.label_library` | Library | SettingsAdvancedScreen.kt |
| `MR.strings.label_network` | Networking | SettingsAdvancedScreen.kt |
| `MR.strings.label_recent_manga` | History | StartScreen.kt |
| `MR.strings.label_recent_updates` | Updates | StartScreen.kt |
| `MR.strings.label_settings` | Settings | SettingsMainScreen.kt |
| `MR.strings.label_sources` | Sources | SettingsBrowseScreen.kt |
| `MR.strings.landscape` | Landscape | TabletUiMode.kt |
| `MR.strings.last_auto_backup_info` | Last automatically backed up: %s | SettingsDataScreen.kt |
| `MR.strings.last_read_chapter` | Last read chapter | SettingsDownloadScreen.kt |
| `MR.strings.left_to_right_viewer` | Paged (left to right) | ReadingMode.kt |
| `MR.strings.library_exported` | Library Exported | SettingsDataScreen.kt |
| `MR.strings.library_list` | Library List | SettingsDataScreen.kt |
| `MR.strings.licenses` | Open source licenses | AboutScreen.kt, OpenSourceLicensesScreen.kt |
| `MR.strings.loading` | Loading… | SettingsTrackingScreen.kt |
| `MR.plurals.lock_after_mins` | one: After %1$s minute; other: After %1$s minutes | SettingsSecurityScreen.kt |
| `MR.strings.lock_always` | Always | AutoTrackState.kt, SecurityPreferences.kt, SettingsSecurityScreen.kt, TabletUiMode.kt |
| `MR.strings.lock_never` | Never | AutoTrackState.kt, SecurityPreferences.kt, SettingsSecurityScreen.kt, TabletUiMode.kt |
| `MR.strings.lock_when_idle` | Lock when idle | SettingsSecurityScreen.kt |
| `MR.strings.lock_with_biometrics` | Require unlock | SettingsSecurityScreen.kt |
| `MR.strings.login` | Login | SettingsTrackingScreen.kt |
| `MR.strings.login_success` | Logged in | SettingsTrackingScreen.kt |
| `MR.strings.login_title` | Log in to %1$s | SettingsTrackingScreen.kt |
| `MR.strings.logout` | Log out | SettingsTrackingScreen.kt |
| `MR.strings.logout_success` | You are now logged out | SettingsTrackingScreen.kt |
| `MR.strings.logout_title` | Log out from %1$s? | SettingsTrackingScreen.kt |
| `MR.strings.migration_dialog_what_to_include` | Select data to include | SettingsDataScreen.kt |
| `MR.strings.minute_short` | %dm | TimeUtils.kt |
| `MR.strings.network_not_metered` | Only on unmetered network | SettingsLibraryScreen.kt |
| `MR.plurals.next_unread_chapters` | one: Next unread chapter; other: Next %d unread chapters | SettingsDownloadScreen.kt |
| `MR.strings.no_location_set` | No storage location set | SettingsDataScreen.kt |
| `MR.strings.no_results_found` | No results found | SettingsSearchScreen.kt |
| `MR.strings.none` | None | Commons.kt |
| `MR.plurals.num_categories` | one: %d category; other: %d categories | SettingsLibraryScreen.kt |
| `MR.plurals.num_repos` | one: %d store; other: %d stores | SettingsBrowseScreen.kt |
| `MR.strings.off` | Off | SettingsDataScreen.kt, SettingsDownloadScreen.kt |
| `MR.strings.pager_viewer` | Paged | SettingsReaderScreen.kt |
| `MR.strings.parental_controls_info` | This does not prevent unofficial or potentially incorrectly flagged extensions from surfacing NSFW (18+) content within the app. | SettingsBrowseScreen.kt |
| `MR.strings.password` | Password | SettingsTrackingScreen.kt |
| `MR.strings.pref_advanced_summary` | Dump crash logs, battery optimizations | SettingsMainScreen.kt |
| `MR.strings.pref_always_decode_long_strip_with_ssiv_2` | Use legacy decoder for long strip reader | SettingsAdvancedScreen.kt |
| `MR.strings.pref_always_decode_long_strip_with_ssiv_summary` | Affects performance. Only enable if reducing bitmap threshold doesn\'t fix blank image issues | SettingsAdvancedScreen.kt |
| `MR.strings.pref_always_show_chapter_transition` | Always show chapter transition | SettingsReaderScreen.kt |
| `MR.strings.pref_app_language` | App language | SettingsAppearanceScreen.kt |
| `MR.strings.pref_app_theme` | App theme | SettingsAppearanceScreen.kt |
| `MR.strings.pref_appearance_summary` | Theme, date & time format | SettingsMainScreen.kt |
| `MR.strings.pref_backup_interval` | Automatic backup frequency | SettingsDataScreen.kt |
| `MR.strings.pref_backup_summary` | Manual & automatic backups, storage space | SettingsMainScreen.kt |
| `MR.strings.pref_browse_summary` | Sources, extensions, global search | SettingsMainScreen.kt |
| `MR.strings.pref_category_about` | About | AboutScreen.kt, SettingsMainScreen.kt |
| `MR.strings.pref_category_advanced` | Advanced | SettingsAdvancedScreen.kt, SettingsMainScreen.kt |
| `MR.strings.pref_category_appearance` | Appearance | SettingsAppearanceScreen.kt, SettingsMainScreen.kt |
| `MR.strings.pref_category_auto_download` | Auto-download | SettingsDownloadScreen.kt |
| `MR.strings.pref_category_display` | Display | SettingsAppearanceScreen.kt, SettingsReaderScreen.kt |
| `MR.strings.pref_category_downloads` | Downloads | SettingsDownloadScreen.kt, SettingsMainScreen.kt |
| `MR.strings.pref_category_library` | Library | SettingsLibraryScreen.kt, SettingsMainScreen.kt |
| `MR.strings.pref_category_library_update` | Global update | SettingsLibraryScreen.kt |
| `MR.strings.pref_category_nsfw_content` | NSFW (18+) sources | SettingsBrowseScreen.kt |
| `MR.strings.pref_category_reader` | Reader | SettingsAdvancedScreen.kt, SettingsMainScreen.kt, SettingsReaderScreen.kt |
| `MR.strings.pref_category_reading` | Reading | SettingsReaderScreen.kt |
| `MR.strings.pref_category_security` | Security and privacy | SettingsMainScreen.kt, SettingsSecurityScreen.kt |
| `MR.strings.pref_category_theme` | Theme | SettingsAppearanceScreen.kt |
| `MR.strings.pref_category_tracking` | Tracking | SettingsMainScreen.kt, SettingsTrackingScreen.kt |
| `MR.strings.pref_chapter_swipe_end` | Chapter on swipe to right | SettingsLibraryScreen.kt |
| `MR.strings.pref_chapter_swipe_start` | Chapter on swipe to left | SettingsLibraryScreen.kt |
| `MR.strings.pref_clear_cookies` | Clear cookies | SettingsAdvancedScreen.kt |
| `MR.strings.pref_clear_webview_data` | Clear WebView data | SettingsAdvancedScreen.kt |
| `MR.strings.pref_create_backup` | Create backup | SettingsDataScreen.kt |
| `MR.strings.pref_create_folder_per_manga` | Save pages into separate folders | SettingsReaderScreen.kt |
| `MR.strings.pref_create_folder_per_manga_summary` | Creates folders according to entries\' title | SettingsReaderScreen.kt |
| `MR.strings.pref_crop_borders` | Crop borders | SettingsReaderScreen.kt |
| `MR.strings.pref_cutout_short` | Show content in cutout area | SettingsReaderScreen.kt |
| `MR.strings.pref_dark_theme_pure_black` | Pure black dark mode | SettingsAppearanceScreen.kt |
| `MR.strings.pref_date_format` | Date format | SettingsAppearanceScreen.kt |
| `MR.strings.pref_debug_info` | Debug info | SettingsAdvancedScreen.kt |
| `MR.strings.pref_disable_battery_optimization` | Disable battery optimization | SettingsAdvancedScreen.kt |
| `MR.strings.pref_disable_battery_optimization_summary` | Helps with background library updates and backups | SettingsAdvancedScreen.kt |
| `MR.strings.pref_display_profile` | Custom display profile | SettingsAdvancedScreen.kt |
| `MR.strings.pref_dns_over_https` | DNS over HTTPS (DoH) | SettingsAdvancedScreen.kt |
| `MR.strings.pref_double_tap_anim_speed` | Double tap animation speed | SettingsReaderScreen.kt |
| `MR.strings.pref_double_tap_zoom` | Double tap to zoom | SettingsReaderScreen.kt |
| `MR.strings.pref_download_new` | Download new chapters | SettingsDownloadScreen.kt |
| `MR.strings.pref_download_new_categories_details` | Entries in excluded categories will not be downloaded even if they are also in included categories. | SettingsDownloadScreen.kt |
| `MR.strings.pref_download_new_unread_chapters_only` | Skip downloading duplicate read chapters | SettingsDownloadScreen.kt |
| `MR.strings.pref_downloads_summary` | Automatic download, download ahead | SettingsMainScreen.kt |
| `MR.strings.pref_dual_page_invert` | Invert split page placement | SettingsReaderScreen.kt |
| `MR.strings.pref_dual_page_invert_summary` | If the placement of the split wide pages don\'t match reading direction | SettingsReaderScreen.kt |
| `MR.strings.pref_dual_page_split` | Split wide pages | SettingsReaderScreen.kt |
| `MR.strings.pref_dump_crash_logs` | Share crash logs | SettingsAdvancedScreen.kt |
| `MR.strings.pref_dump_crash_logs_summary` | Saves error logs to a file for sharing with the developers | SettingsAdvancedScreen.kt |
| `MR.strings.pref_flash_duration` | Flash duration | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_duration_summary` | %1$s ms | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_page` | Flash on page change | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_page_interval` | Flash every | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_page_summ` | Reduces ghosting on e-ink displays | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_style_black` | Black | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_style_white` | White | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_style_white_black` | White and Black | SettingsReaderScreen.kt |
| `MR.strings.pref_flash_with` | Flash with | SettingsReaderScreen.kt |
| `MR.strings.pref_fullscreen` | Fullscreen | SettingsReaderScreen.kt |
| `MR.strings.pref_hardware_bitmap_threshold` | Custom hardware bitmap threshold | SettingsAdvancedScreen.kt |
| `MR.strings.pref_hardware_bitmap_threshold_default` | Default (%d) | SettingsAdvancedScreen.kt |
| `MR.strings.pref_hardware_bitmap_threshold_summary` | If reader loads a blank image incrementally reduce the threshold.\nSelected: %s | SettingsAdvancedScreen.kt |
| `MR.strings.pref_hide_threshold` | Sensitivity for hiding menu on scroll | SettingsReaderScreen.kt |
| `MR.strings.pref_high` | High | SettingsReaderScreen.kt |
| `MR.strings.pref_highest` | Highest | SettingsReaderScreen.kt |
| `MR.strings.pref_image_scale_type` | Scale type | SettingsReaderScreen.kt |
| `MR.strings.pref_incognito_mode` | Incognito mode | SecurityPreferences.kt |
| `MR.strings.pref_invalidate_download_cache` | Reindex downloads | SettingsAdvancedScreen.kt |
| `MR.strings.pref_keep_screen_on` | Keep screen on | SettingsReaderScreen.kt |
| `MR.strings.pref_landscape_zoom` | Automatically zoom into wide images | SettingsReaderScreen.kt |
| `MR.strings.pref_library_update_interval` | Automatic updates | SettingsLibraryScreen.kt |
| `MR.strings.pref_library_update_refresh_metadata` | Automatically refresh metadata | SettingsLibraryScreen.kt |
| `MR.strings.pref_library_update_refresh_metadata_summary` | Check for new cover and details when updating library | SettingsLibraryScreen.kt |
| `MR.strings.pref_library_update_restriction` | Automatic updates device restrictions | SettingsLibraryScreen.kt |
| `MR.strings.pref_library_update_smart_update` | Smart update | SettingsLibraryScreen.kt |
| `MR.strings.pref_low` | Low | SettingsReaderScreen.kt |
| `MR.strings.pref_lowest` | Lowest | SettingsReaderScreen.kt |
| `MR.strings.pref_manage_notifications` | Manage notifications | SettingsAdvancedScreen.kt |
| `MR.strings.pref_mark_duplicate_read_chapter_read` | Mark duplicate read chapter as read | SettingsLibraryScreen.kt |
| `MR.strings.pref_mark_duplicate_read_chapter_read_existing` | After reading a chapter | SettingsLibraryScreen.kt |
| `MR.strings.pref_mark_duplicate_read_chapter_read_new` | After fetching new chapter | SettingsLibraryScreen.kt |
| `MR.strings.pref_navigate_pan` | Pan wide images | SettingsReaderScreen.kt |
| `MR.strings.pref_onboarding_guide` | Onboarding guide | SettingsAdvancedScreen.kt |
| `MR.strings.pref_page_rotate` | Rotate wide pages to fit | SettingsReaderScreen.kt |
| `MR.strings.pref_page_rotate_invert` | Flip orientation of rotated wide pages | SettingsReaderScreen.kt |
| `MR.strings.pref_page_transitions` | Animate page transitions | SettingsReaderScreen.kt |
| `MR.plurals.pref_pages` | one: 1 page; other: %1$s pages | SettingsReaderScreen.kt |
| `MR.strings.pref_read_with_long_tap` | Show actions on long tap | SettingsReaderScreen.kt |
| `MR.strings.pref_read_with_tapping_inverted` | Invert tap zones | SettingsReaderScreen.kt |
| `MR.strings.pref_read_with_volume_keys` | Volume keys | SettingsReaderScreen.kt |
| `MR.strings.pref_read_with_volume_keys_inverted` | Invert volume keys | SettingsReaderScreen.kt |
| `MR.strings.pref_reader_actions` | Actions | SettingsReaderScreen.kt |
| `MR.strings.pref_reader_navigation` | Navigation | SettingsReaderScreen.kt |
| `MR.strings.pref_reader_summary` | Reading mode, display, navigation | SettingsMainScreen.kt |
| `MR.strings.pref_reader_theme` | Background color | SettingsReaderScreen.kt |
| `MR.strings.pref_refresh_library_covers` | Refresh library covers | SettingsAdvancedScreen.kt |
| `MR.strings.pref_relative_format` | Relative timestamps | SettingsAppearanceScreen.kt |
| `MR.strings.pref_relative_format_summary` | \"%1$s\" instead of \"%2$s\" | SettingsAppearanceScreen.kt |
| `MR.strings.pref_reset_user_agent_string` | Reset default user agent string | SettingsAdvancedScreen.kt |
| `MR.strings.pref_reset_viewer_flags` | Reset per-series reader settings | SettingsAdvancedScreen.kt |
| `MR.strings.pref_reset_viewer_flags_error` | Couldn\'t reset reader settings | SettingsAdvancedScreen.kt |
| `MR.strings.pref_reset_viewer_flags_success` | All reader settings reset | SettingsAdvancedScreen.kt |
| `MR.strings.pref_reset_viewer_flags_summary` | Resets reading mode and orientation of all series | SettingsAdvancedScreen.kt |
| `MR.strings.pref_restore_backup` | Restore backup | SettingsDataScreen.kt |
| `MR.strings.pref_rotation_type` | Default rotation | SettingsReaderScreen.kt |
| `MR.strings.pref_security_summary` | App lock, secure screen | SettingsMainScreen.kt |
| `MR.strings.pref_show_navigation_mode` | Show tap zones overlay | SettingsReaderScreen.kt |
| `MR.strings.pref_show_navigation_mode_summary` | Briefly show when reader is opened | SettingsReaderScreen.kt |
| `MR.strings.pref_show_nsfw_source` | Show in sources and extensions lists | SettingsBrowseScreen.kt |
| `MR.strings.pref_show_page_number` | Show page number | SettingsReaderScreen.kt |
| `MR.strings.pref_show_reading_mode` | Show reading mode | SettingsReaderScreen.kt |
| `MR.strings.pref_show_reading_mode_summary` | Briefly show current mode when reader is opened | SettingsReaderScreen.kt |
| `MR.strings.pref_skip_dupe_chapters` | Skip duplicate chapters | SettingsReaderScreen.kt |
| `MR.strings.pref_skip_filtered_chapters` | Skip filtered chapters | SettingsReaderScreen.kt |
| `MR.strings.pref_skip_read_chapters` | Skip chapters marked read | SettingsReaderScreen.kt |
| `MR.strings.pref_storage_location` | Storage location | SettingsDataScreen.kt |
| `MR.strings.pref_storage_location_info` | Used for automatic backups, chapter downloads, and local source. | SettingsDataScreen.kt |
| `MR.strings.pref_storage_usage` | Storage usage | SettingsDataScreen.kt |
| `MR.strings.pref_tablet_ui_mode` | Tablet UI | SettingsAppearanceScreen.kt |
| `MR.strings.pref_tracking_summary` | One-way progress sync, enhanced sync | SettingsMainScreen.kt |
| `MR.strings.pref_update_only_in_release_period` | Predict next release time | SettingsLibraryScreen.kt |
| `MR.strings.pref_update_only_non_completed` | Skip entries with \"Completed\" status | SettingsLibraryScreen.kt |
| `MR.strings.pref_update_only_started` | Skip unstarted entries | SettingsLibraryScreen.kt |
| `MR.strings.pref_user_agent_string` | Default user agent string | SettingsAdvancedScreen.kt |
| `MR.strings.pref_verbose_logging` | Verbose logging | SettingsAdvancedScreen.kt |
| `MR.strings.pref_verbose_logging_summary` | Print verbose logs to system log (reduces app performance) | SettingsAdvancedScreen.kt |
| `MR.strings.pref_viewer_nav` | Tap zones | SettingsReaderScreen.kt |
| `MR.strings.pref_viewer_type` | Default reading mode | SettingsReaderScreen.kt |
| `MR.strings.pref_webtoon_disable_zoom_out` | Disable zoom out | SettingsReaderScreen.kt |
| `MR.strings.pref_webtoon_side_padding` | Side padding | SettingsReaderScreen.kt |
| `MR.strings.pref_zoom_start` | Zoom start position | SettingsReaderScreen.kt |
| `MR.strings.privacy_policy` | Privacy policy | AboutScreen.kt |
| `MR.strings.relative_time_span_never` | Never | TimeUtils.kt |
| `MR.strings.relative_time_today` | Today | SettingsAppearanceScreen.kt |
| `MR.strings.requires_app_restart` | Requires app restart to take effect | SettingsAdvancedScreen.kt, SettingsAppearanceScreen.kt, SettingsBrowseScreen.kt |
| `MR.strings.restore_in_progress` | Restore is already in progress | SettingsDataScreen.kt |
| `MR.strings.restore_miui_warning` | Backup/restore may not function properly if MIUI Optimization is disabled. | SettingsDataScreen.kt |
| `MR.strings.restrictions` | Restrictions: %s | SettingsLibraryScreen.kt |
| `MR.strings.right_and_left_nav` | Right and Left | ReaderPreferences.kt |
| `MR.strings.right_to_left_viewer` | Paged (right to left) | ReadingMode.kt |
| `MR.strings.rotation_force_landscape` | Locked landscape | ReaderOrientation.kt |
| `MR.strings.rotation_force_portrait` | Locked portrait | ReaderOrientation.kt |
| `MR.strings.rotation_free` | Free | ReaderOrientation.kt |
| `MR.strings.rotation_landscape` | Landscape | ReaderOrientation.kt |
| `MR.strings.rotation_portrait` | Portrait | ReaderOrientation.kt |
| `MR.strings.rotation_reverse_portrait` | Reverse portrait | ReaderOrientation.kt |
| `MR.strings.save_chapter_as_cbz` | Save as CBZ archive | SettingsDownloadScreen.kt |
| `MR.strings.scale_type_fit_height` | Fit height | ReaderPreferences.kt |
| `MR.strings.scale_type_fit_screen` | Fit screen | ReaderPreferences.kt |
| `MR.strings.scale_type_fit_width` | Fit width | ReaderPreferences.kt |
| `MR.strings.scale_type_original_size` | Original size | ReaderPreferences.kt |
| `MR.strings.scale_type_smart_fit` | Smart fit | ReaderPreferences.kt |
| `MR.strings.scale_type_stretch` | Stretch | ReaderPreferences.kt |
| `MR.strings.second_to_last` | Second to last read chapter | SettingsDownloadScreen.kt |
| `MR.strings.seconds_short` | %ds | TimeUtils.kt |
| `MR.strings.secure_screen` | Secure screen | SettingsSecurityScreen.kt |
| `MR.strings.secure_screen_summary` | Secure screen hides app contents when switching apps and block screenshots | SettingsSecurityScreen.kt |
| `MR.strings.services` | Trackers | SettingsTrackingScreen.kt |
| `MR.strings.split_tall_images` | Split tall images | SettingsDownloadScreen.kt |
| `MR.strings.split_tall_images_summary` | Improves reader performance | SettingsDownloadScreen.kt |
| `MR.strings.tapping_inverted_both` | Both | ReaderPreferences.kt |
| `MR.strings.tapping_inverted_horizontal` | Horizontal | ReaderPreferences.kt |
| `MR.strings.tapping_inverted_none` | None | ReaderPreferences.kt |
| `MR.strings.tapping_inverted_vertical` | Vertical | ReaderPreferences.kt |
| `MR.strings.theme_greenapple` | Green Apple | AppTheme.kt |
| `MR.strings.theme_lavender` | Lavender | AppTheme.kt |
| `MR.strings.theme_midnightdusk` | Midnight Dusk | AppTheme.kt |
| `MR.strings.theme_monet` | Dynamic | AppTheme.kt |
| `MR.strings.theme_monochrome` | Monochrome | AppTheme.kt |
| `MR.strings.theme_nord` | Nord | AppTheme.kt |
| `MR.strings.theme_strawberrydaiquiri` | Strawberry Daiquiri | AppTheme.kt |
| `MR.strings.theme_tako` | Tako | AppTheme.kt |
| `MR.strings.theme_tealturquoise` | Teal & Turquoise | AppTheme.kt |
| `MR.strings.theme_tidalwave` | Tidal Wave | AppTheme.kt |
| `MR.strings.theme_yinyang` | Yin & Yang | AppTheme.kt |
| `MR.strings.theme_yotsuba` | Yotsuba | AppTheme.kt |
| `MR.strings.third_to_last` | Third to last read chapter | SettingsDownloadScreen.kt |
| `MR.strings.title` | Title | SettingsDataScreen.kt |
| `MR.strings.tracking_guide` | Tracking guide | SettingsDataScreen.kt, SettingsTrackingScreen.kt |
| `MR.strings.tracking_info` | One-way sync to update the chapter progress in external tracker services. Set up tracking for individual entries from their tracking button. | SettingsTrackingScreen.kt |
| `MR.strings.update_12hour` | Every 12 hours | SettingsDataScreen.kt, SettingsLibraryScreen.kt |
| `MR.strings.update_24hour` | Daily | SettingsDataScreen.kt, SettingsLibraryScreen.kt |
| `MR.strings.update_48hour` | Every 2 days | SettingsDataScreen.kt, SettingsLibraryScreen.kt |
| `MR.strings.update_6hour` | Every 6 hours | SettingsDataScreen.kt |
| `MR.strings.update_72hour` | Every 3 days | SettingsLibraryScreen.kt |
| `MR.strings.update_check_eol` | This Android version is no longer supported | AboutScreen.kt |
| `MR.strings.update_check_no_new_updates` | No new updates available | AboutScreen.kt |
| `MR.strings.update_never` | Off | SettingsLibraryScreen.kt |
| `MR.strings.update_weekly` | Weekly | SettingsDataScreen.kt, SettingsLibraryScreen.kt |
| `MR.strings.updates_last_update_info_just_now` | Just now | TimeUtils.kt |
| `MR.strings.used_cache` | Used: %1$s | SettingsDataScreen.kt |
| `MR.strings.username` | Username | SettingsTrackingScreen.kt |
| `MR.strings.version` | Version | AboutScreen.kt |
| `MR.strings.vertical_plus_viewer` | Long strip with gaps | ReadingMode.kt |
| `MR.strings.vertical_viewer` | Paged (vertical) | ReadingMode.kt |
| `MR.strings.website` | Website | AboutScreen.kt, OpenSourceLibraryLicenseScreen.kt |
| `MR.strings.webtoon_viewer` | Long strip | ReadingMode.kt, SettingsReaderScreen.kt |
| `MR.strings.webview_data_deleted` | WebView data cleared | SettingsAdvancedScreen.kt |
| `MR.strings.whats_new` | What\'s new | AboutScreen.kt |
| `MR.strings.white_background` | White | SettingsReaderScreen.kt |
| `MR.strings.zoom_start_automatic` | Automatic | ReaderPreferences.kt |
| `MR.strings.zoom_start_center` | Center | ReaderPreferences.kt |
| `MR.strings.zoom_start_left` | Left | ReaderPreferences.kt |
| `MR.strings.zoom_start_right` | Right | ReaderPreferences.kt |
