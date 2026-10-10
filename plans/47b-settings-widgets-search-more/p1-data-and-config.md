# Area 47b — Data models, preference keys, files, endpoints

Paths are repo-relative to `/home/blackbox/aniyomi-src`. This area has no database tables and no network endpoints of its own. Its data is: (1) the in-memory settings model (`Preference` tree), (2) preference keys read or written by the More tab, theme widgets, onboarding and update dialog, (3) enums and values those keys hold, and (4) the two download-queue inputs. Endpoints and files touched indirectly are listed in §4 and §5 for completeness.

## 1. In-memory model

### 1.1 `Preference` tree (`app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt`)

Rust mapping in `p1-rust-design.md` §2. Field-by-field:

| Kotlin type | Field | Type | Nullable | Default | Notes |
|---|---|---|---|---|---|
| `Preference` (sealed) | `title` | String | no | none | `TrackerPreference` uses `""` |
| `Preference` | `enabled` | Boolean | no | true | |
| `PreferenceGroup` | `preferenceItems` | ImmutableList<PreferenceItem<out Any>> | no | none | |
| `PreferenceItem<T>` | `subtitle` | String? | yes | per variant (see below) | |
| `PreferenceItem<T>` | `icon` | ImageVector? | yes | null | Replaced by an icon name in Rust |
| `PreferenceItem<T>` | `onValueChanged` | suspend (T) -> Boolean | no | `{ true }` | veto |
| `TextPreference` | `onClick` | (() -> Unit)? | yes | null | |
| `SwitchPreference` | `preference` | PreferenceData<Boolean> | no | none | bound |
| `SliderPreference` | `value` | Int | no | none | not bound |
| `SliderPreference` | `valueRange` | IntProgression | no | 0..1 | |
| `SliderPreference` | `steps` | Int | no | `(last - first) - 1`, annotated `@IntRange(from = 0)` | for 0..1 that is 0 |
| `ListPreference<T>` | `preference` | PreferenceData<T> | no | none | bound |
| `ListPreference<T>` | `entries` | ImmutableMap<T, String> | no | none | key -> English label |
| `ListPreference<T>` | `subtitle` | String? | yes | `"%s"` | |
| `ListPreference<T>` | `subtitleProvider` | @Composable (T, entries) -> String? | no | `{ v, e -> subtitle?.format(e[v]) }` | |
| `BasicListPreference` | `value` | String | no | none | not bound |
| `BasicListPreference` | `entries` | ImmutableMap<String, String> | no | none | |
| `MultiSelectListPreference` | `preference` | PreferenceData<Set<String>> | no | none | bound |
| `MultiSelectListPreference` | `entries` | ImmutableMap<String, String> | no | none | |
| `MultiSelectListPreference` | `subtitle` | String? | yes | `"%s"` | |
| `MultiSelectListPreference` | `subtitleProvider` | default joins labels, `"None"` if empty | no | see behavior B.5 | |
| `EditTextPreference` | `preference` | PreferenceData<String> | no | none | bound |
| `EditTextPreference` | `subtitle` | String? | yes | `"%s"` | |
| `MultiLineEditTextPreference` | `canBeBlank` | Boolean | no | false | |
| `MPVConfPreference` | `preference` | PreferenceData<String> | no | none | bound |
| `MPVConfPreference` | `scope` | CoroutineScope | no | none | Kotlin-only |
| `MPVConfPreference` | `context` | Context | no | none | Android-only |
| `MPVConfPreference` | `fileName` | String? | yes | null | |
| `MPVConfPreference` | `canBeBlank` | Boolean | no | true | |
| `MPVConfPreference` | `subtitle` | String? | yes | first 2 lines of value, `"\n..."` appended if >2 lines | computed in constructor |
| `EditTextInfoPreference` | `dialogSubtitle` | String? | yes | none (required param, may be null) | |
| `EditTextInfoPreference` | `validate` | (String) -> Boolean | no | `{ true }` | |
| `EditTextInfoPreference` | `errorMessage` | @Composable ((String) -> String)? | yes | null | |
| `EditTextInfoPreference` | `keyboardOptions` | KeyboardOptions | no | `KeyboardOptions.Default` | Compose type |
| `TrackerPreference` | `tracker` | Tracker | no | none | `app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt` |
| `TrackerPreference` | `login` / `logout` | () -> Unit | no | none | |
| `InfoPreference` | `title` | String | no | none | the body text |
| `CustomPreference` | `content` | @Composable () -> Unit | no | none | |

### 1.2 `TriStateListDialog` state (`widget/TriStateListDialog.kt:35`)
- `private enum class State { CHECKED, INVERSED, UNCHECKED }`. Not persisted. Rust: `enum TriState { Checked, Inversed, Unchecked }` with `Default = Unchecked`.
- Inputs: `items: List<T>`, `initialChecked: List<T>`, `initialInversed: List<T>`. Outputs: `included: List<T>` (items in CHECKED), `excluded: List<T>` (items in INVERSED), both in original `items` order.

### 1.3 `DownloadQueueState` (`app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt:134-138`)
- `sealed interface DownloadQueueState { data object Stopped; data class Paused(val pending: Int); data class Downloading(val pending: Int) }`.
- Not persisted. Derived from the two download managers each time either changes.
- Rust: `enum DownloadQueueState { Stopped, Paused { pending: usize }, Downloading { pending: usize } }`. `pending` is `usize` (Kotlin `Int`, never negative because it is `size`).

### 1.4 `MoreScreenModel` (`MoreTab.kt:88-132`)
- Fields: `downloadedOnly: Boolean` (get/set bound to preference, see §2), `incognitoMode: Boolean` (get/set bound), `downloadQueueState: StateFlow<DownloadQueueState>` (initial `Stopped`).
- Dependencies: `MangaDownloadManager`, `AnimeDownloadManager`, `BasePreferences`.

### 1.5 `NewUpdateScreen` inputs (`ui/more/NewUpdateScreen.kt:13-18`)
- `versionName: String` (shown as subtitle and as the job title), `changelogInfo: String` (raw markdown, cleaned at display time), `releaseLink: String` (browser URL), `downloadLink: String` (direct asset URL used by the job).
- Not persisted. Provided by the caller that runs the update check (area:35).

### 1.6 `Tab` option values (`MoreTab.kt:50-54`)
- `TabOptions(index: UShort, title: String, icon: Painter)`. `index = 4u` is the slot for More in the Compose bottom bar. Rust: a sidebar entry in `spec/app.json` `nav` (order is data, not a constant).

---

## 2. Preference keys (read or written in scope)

All keys below are defined in `BasePreferences`, `UiPreferences` or the scope files themselves. Defaults are exact. "Key" is the literal string passed to `PreferenceStore`. APP_STATE prefix: `Preference.appStateKey(key)` returns `"__APP_STATE_" + key` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`).

| Key (literal, as stored) | Accessor | Type | Default | Defined at | Read/written by | Notes |
|---|---|---|---|---|---|---|
| `__APP_STATE_pref_downloaded_only` | `BasePreferences.downloadedOnly()` | Boolean | false | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-19` | MoreScreenModel (rw) | "Downloaded only" switch |
| `__APP_STATE_incognito_mode` | `BasePreferences.incognitoMode()` | Boolean | false | `BasePreferences.kt:21` | MoreScreenModel (rw) | "Incognito mode" switch |
| `__APP_STATE_onboarding_complete` | `BasePreferences.shownOnboardingFlow()` | Boolean | false | `BasePreferences.kt:29` | OnboardingScreen (rw), MainActivity gate (r) | onboarding done flag |
| `pref_theme_mode_key` | `UiPreferences.themeMode()` | enum ThemeMode | `ThemeMode.SYSTEM` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:20` | AppThemeMode widget (rw) | enum ThemeMode values in §3.1 |
| `pref_app_theme` | `UiPreferences.appTheme()` | enum AppTheme | `MONET` if `DeviceUtil.isDynamicColorAvailable`, else `DEFAULT` | `UiPreferences.kt:22-30` | AppTheme widget (rw) | Linux: always `DEFAULT` unless set |
| `pref_theme_dark_amoled_key` | `UiPreferences.themeDarkAmoled()` | Boolean | false | `UiPreferences.kt:31` | AppTheme widget `amoled` param (r) | |
| `bottom_rail_nav_style` | `UiPreferences.navStyle()` | enum NavStyle | `MOVE_HISTORY_TO_MORE` | `UiPreferences.kt:41` | MoreScreen alt-nav row (r) | |

Notes on Rust storage:
- Proposal: keep these exact key strings in the Rust prefs schema (area:02) so a migrated Kotlin backup JSON or a shared config is readable. The `__APP_STATE_` prefix is kept for the three app-state flags.
- None of these keys is a per-viewer or per-device key; they belong in the user's Kurobayomi config store.

Keys that the scope mentions only indirectly (owned elsewhere):
- `pref_bottom_nav_*` strings are labels, not keys.
- `MPVConfPreference` writes to a file, not to a key, but also calls `preference.set(value)` on a `PreferenceData<String>` whose key is passed by the caller (area:48 owns those keys, e.g. the mpv config key).

## 3. Enums and stored values

### 3.1 `ThemeMode` (`app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt:5-9`)
Declaration order and stored name (assumed: `getEnum` persists the enum's `name` string; confirm in `core/common/src/main/java/tachiyomi/core/common/preference/`):
- `LIGHT`
- `DARK`
- `SYSTEM` (default)

Mapping to UI (`AppThemeModePreferenceWidget.kt:15-19`): SYSTEM → "System" (`theme_system`), LIGHT → "Light" (`theme_light`), DARK → "Dark" (`theme_dark`). The widget's segment order is SYSTEM, LIGHT, DARK, which differs from the enum declaration order. Rust: the segment order is the UI order; the stored value is the enum name string.

`setAppCompatDelegateThemeMode` (same file, lines 11-14) maps ThemeMode to AppCompat night modes on Android. Linux equivalent: `gtk::Settings::set_gtk_application_prefer_dark_theme` for DARK/LIGHT, and for SYSTEM leave the GTK default so the desktop's colour scheme applies. See rust-design §7.

### 3.2 `AppTheme` (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt:7-32`)
`enum class AppTheme(val titleRes: StringResource?)`. Stored name = enum name.

| Name | titleRes key (English) | Shown in settings strip |
|---|---|---|
| DEFAULT | label_default ("Default") | yes |
| MONET | theme_monet ("Dynamic") | Android only (hidden on Linux) |
| CLOUDFLARE | AYMR theme_cloudflare ("Cloudflare") | yes |
| COTTONCANDY | AYMR theme_cottoncandy ("Cotton Candy") | yes |
| DOOM | AYMR theme_doom ("Doom") | yes |
| GREEN_APPLE | theme_greenapple | yes |
| LAVENDER | theme_lavender | yes |
| MATRIX | AYMR theme_matrix ("Matrix") | yes |
| MIDNIGHT_DUSK | theme_midnightdusk | yes |
| MOCHA | AYMR theme_mocha ("Mocha") | yes |
| SAPPHIRE | AYMR theme_sapphire ("Sapphire") | yes |
| NORD | theme_nord | yes |
| STRAWBERRY_DAIQUIRI | theme_strawberrydaiquiri | yes |
| TAKO | theme_tako | yes |
| TEALTURQUOISE | theme_tealturquoise | yes |
| TIDAL_WAVE | theme_tidalwave | yes |
| YINYANG | theme_yinyang | yes |
| YOTSUBA | theme_yotsuba | yes |
| MONOCHROME | theme_monochrome | yes |
| DARK_BLUE | null (deprecated) | never |
| HOT_PINK | null (deprecated) | never |
| BLUE | null (deprecated) | never |

- Stored values of deprecated names are still valid enum names. Reading a stored `DARK_BLUE` returns the enum value; the strip hides it, so the currently selected theme card is absent. Proposal: on read, map DARK_BLUE/HOT_PINK/BLUE to DEFAULT.
- The English labels for the non-AYMR names (`theme_greenapple` and others) are the strings in `i18n/src/commonMain/moko-resources/base/strings.xml` (resolved at build time; the Rust spec must copy them exactly from that file, per `spec/SCHEMA.md`).

### 3.3 `NavStyle` (`app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt:20-25`)
Stored name. Values and their alt-tab targets:
- `MOVE_MANGA_TO_MORE` → `MangaLibraryTab`, title `pref_bottom_nav_no_manga`, icon `Icons.Outlined.CollectionsBookmark`
- `MOVE_UPDATES_TO_MORE` → `UpdatesTab`, `pref_bottom_nav_no_updates`, icon `ic_updates_outline_24dp`
- `MOVE_HISTORY_TO_MORE` (default) → `HistoriesTab`, `pref_bottom_nav_no_history`, icon `Icons.Outlined.History`
- `MOVE_BROWSE_TO_MORE` → `BrowseTab`, `pref_bottom_nav_no_browse`, icon `Icons.Outlined.Explore`

`NavStyle.tabs` (getter in the same file) is the bottom-bar list `[AnimeLibraryTab, MangaLibraryTab, UpdatesTab, HistoriesTab, BrowseTab, MoreTab]` minus the moved tab.

### 3.4 `Tracker` login state
- `Tracker.isLoggedIn: Boolean` (`app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt:38`) and `Tracker.isLoggedInFlow: Flow<Boolean>` (line 40). Written by the tracker's login flow (area:27).

### 3.5 `TriState` for filters
- The tri-state dialog does not persist. The caller maps `(included, excluded)` to its own filter model (area:37/40).

### 3.6 `State` (TriStateListDialog) and `DownloadQueueState`
- See §1.2 and §1.3.

## 4. Network endpoints touched

None directly in scope. Indirect:

| Endpoint | Method | Caller in scope | Purpose | Owner |
|---|---|---|---|---|
| (release link, from update check) | GET (browser) | `ui/more/NewUpdateScreen.kt:31` via `openInBrowser` | opens the GitHub release page | area:35 |
| (download link, from update check) | GET (download) | `AppUpdateDownloadJob` triggered at `ui/more/NewUpdateScreen.kt:34-38` | downloads the new build | area:35 |
| `https://aniyomi.org/docs/guides/troubleshooting/` (`Constants.URL_HELP`) | GET (browser) | `MoreScreen.kt:172` | Help row opens docs | `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:4` |

- Proposal (Linux): the Help URL stays identical. The download/update URLs come from area:35 unchanged.

## 5. Files and folders on disk (touched in scope)

| Path | Owner | Written by | Format | Notes |
|---|---|---|---|---|
| MPV config dir (`StorageManager.getMPVConfigDirectory()`) + `fileName` | area:48 | `MPVConfPreference.onValueChanged` (`Preference.kt:184-200`) | plain text, written via `bufferedWriter` (UTF-8) | Android only; permission-gated (see behavior B.14) |
| App update download (via `AppUpdateDownloadJob`) | area:35 | `AppUpdateDownloadJob` | APK on Android | Linux: proposal in §U.3 of behavior |
| Preferences store (SharedPreferences on Android) | area:02 | all `preference.set()` calls in scope | key/value | Linux: Rust prefs file, see rust-design §2 |

Linux proposals for file locations:
- Config (mpv overrides): `$XDG_CONFIG_HOME/kurobayomi/mpv/<fileName>`, default `~/.config/kurobayomi/mpv/<fileName>`.
- Update download: `$XDG_DOWNLOAD_DIR/Kurobayomi/updates/<version>/<asset-file-name>`, fallback `~/Downloads/Kurobayomi/updates/`.

## 6. Download manager inputs (read only)

| Source | Type | Used by | Notes |
|---|---|---|---|
| `MangaDownloadManager.isDownloaderRunning` | Flow<Boolean> (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:71`) | MoreScreenModel | |
| `MangaDownloadManager.queueState` | Flow<List<...>> (`MangaDownloadManager.kt:64`, backed by `MangaDownloader.queueState` at `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:104`) | MoreScreenModel, `.size` | Only `size` is used |
| `AnimeDownloadManager.isDownloaderRunning` | Flow<Boolean> (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt:69`) | MoreScreenModel | |
| `AnimeDownloadManager.queueState` | Flow<List<...>> (`AnimeDownloadManager.kt:62`, backed by `AnimeDownloader.queueState` at `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt:101`) | MoreScreenModel, `.size` | Only `size` is used |

Rust: `tokio::sync::watch::Receiver<usize>` per manager (rust-design §4). Area:23/24/25 own the producers.

## 7. Plural and string resources used as data

- `download_queue_summary` (plural): `one` = `%1$s remaining`, `other` = `%1$s remaining` (`i18n/src/commonMain/moko-resources/base/plurals.xml:33-36`). English output is identical for every count. Rust: a single format string `"{n} remaining"` (no plural rules needed for English). Other locales must provide their own plural forms via area:51.
- `paused` = "Paused" (`i18n/src/commonMain/moko-resources/base/strings.xml`).
- `label_backup` = "Backup and restore" (onboarding highlight key, §O of behavior).

## 8. Icon resources (Android drawables, Kurobayomi replacements)

| Android resource | Used in | Kurobayomi replacement (GTK symbolic icon name, to verify against the Adwaita/Papirus set on the user's machine) |
|---|---|---|
| `R.drawable.ic_ani` | LogoHeader | `applications-multimedia-symbolic` (matches mockup `src/screens.rs:339`) or the app's own `io.github.anas1412.Kurobayomi` icon (area:49b) |
| `R.drawable.ic_glasses_24dp` | Incognito row | `dialog-password-symbolic` or a bundled SVG (area:49b). Proposal: bundle the SVG so the glasses look is kept |
| `R.drawable.ic_updates_outline_24dp` | NavStyle MOVE_UPDATES_TO_MORE | `document-open-recent-symbolic` |
| `R.drawable.anim_more_enter` | MoreTab animated tab icon | static `open-menu-symbolic`; animation dropped (see rust-design §8) |

Compose Material icons used (name → Linux name):
- `Icons.Outlined.CloudOff` → `network-offline-symbolic`
- `Icons.Outlined.GetApp` → `folder-download-symbolic`
- `Icons.AutoMirrored.Outlined.Label` → `tag-symbolic`
- `Icons.Outlined.QueryStats` → `utilities-system-monitor-symbolic`
- `Icons.Outlined.Storage` → `drive-harddisk-symbolic`
- `Icons.Outlined.Settings` → `emblem-system-symbolic`
- `Icons.Outlined.VideoSettings` → `video-display-symbolic`
- `Icons.Outlined.Info` → `dialog-information-symbolic`
- `Icons.AutoMirrored.Outlined.HelpOutline` → `help-about-symbolic`
- `Icons.Outlined.NewReleases` → `software-update-available-symbolic`
- `Icons.AutoMirrored.Outlined.OpenInNew` → `external-link-symbolic`
- `Icons.Filled.CheckCircle` → `emblem-ok-symbolic`
- `Icons.Filled.Error` → `dialog-error-symbolic`
- `Icons.Filled.Cancel` → `edit-clear-symbolic`
- `Icons.Outlined.Done` → `object-select-symbolic`
- `Icons.Rounded.CheckBox` → `checkbox-checked-symbolic`
- `Icons.Rounded.CheckBoxOutlineBlank` → `checkbox-symbolic`
- `Icons.Rounded.DisabledByDefault` → `action-unavailable-symbolic`
- `Icons.Filled.Preview` (previews only) → dropped
- `Icons.Outlined.CollectionsBookmark` → `bookmark-new-symbolic`
- `Icons.Outlined.History` → `document-open-recent-symbolic`
- `Icons.Outlined.Explore` → `compass-symbolic`

Verify each name exists in the installed icon theme with `gtk4-icon-browser` before relying on it (pitfall P-icon in rust-design §10).

## 9. Layout constants (dp values, for the GTK CSS mapping)

| Token | Value | Source |
|---|---|---|
| `PrefsHorizontalPadding` | 16 | `BasePreferenceWidget.kt:123` |
| `PrefsVerticalPadding` | 16 | `BasePreferenceWidget.kt:124` |
| `TrailingWidgetBuffer` | 16 | `BasePreferenceWidget.kt:122` |
| `TitleFontSize` | 16 sp | `BasePreferenceWidget.kt:125` |
| `LocalPreferenceMinHeight` | 56 | `PreferenceItem.kt:34` |
| Group spacer | 12 | `PreferenceScreen.kt:68` |
| Group header top | 14 | `PreferenceGroupHeader.kt:19` |
| Group header bottom | 8 | `PreferenceGroupHeader.kt:19` |
| Text row subtitle max lines | 10 | `TextPreferenceWidget.kt:40` |
| Text row title max lines | 2 | `BasePreferenceWidget.kt:73` |
| Tracker row vertical padding | 8 | `TrackingPreferenceWidget.kt:38` |
| Tracker check icon size | 32 (padding 4) | `TrackingPreferenceWidget.kt:54-56` |
| Tracker check colour | #4CAF50 | `TrackingPreferenceWidget.kt:57` |
| Theme card width | 114 | `AppThemePreferenceWidget.kt:97` |
| Theme card frame | 9:16, border 4, radius 17 / inner 13, padding 4 | `AppThemePreferenceWidget.kt:136-149` |
| Theme app-bar height | 40 (padding 8) | `AppThemePreferenceWidget.kt:157-158` |
| Theme cover ratio | 2/3 (`ItemCover.Book`) | `ItemCover.kt:19` |
| Theme cover sub-chip | 24 x 16, radius 5, 12 dp bars | `AppThemePreferenceWidget.kt:200-214` |
| Theme bottom bar | 32 | `AppThemePreferenceWidget.kt:230` |
| Tri-state row min height | 48 | `TriStateListDialog.kt:91` |
| Tri-state icon end padding | 20 | `TriStateListDialog.kt:96` |
| Dialog radio row min touch | 48 (`minimumInteractiveComponentSize`) | `ListPreferenceWidget.kt:98` |
| Radio label start padding | 24 | `ListPreferenceWidget.kt:107` |
| Checkbox row min height | 48 | `LabeledCheckbox.kt` |
| Logo icon | 64, vertical padding 56 | `LogoHeader.kt:28-29` |
| Update body padding | 24 (large) | `NewUpdateScreen.kt:48` |
| Slider wrapper padding (preference path) | 16 / 16 | `PreferenceItem.kt:97-100` |
| `SettingsItemsPaddings.Horizontal` (other slider path) | 24 | `SettingsItems.kt:76` |
| `SettingsItemsPaddings.Vertical` (other slider path) | 10 | `SettingsItems.kt:77` |
| Secondary alpha | 0.78 | `presentation-core/.../components/material/Constants.kt` (`SECONDARY_ALPHA`) |
| Disabled alpha | 0.38 | same file (`DISABLED_ALPHA`) |
| Padding tokens | extraLarge 32, large 24, medium 16, mediumSmall 12, small 8, extraSmall 4 | `presentation-core/.../components/material/Constants.kt:14-24` |

## 10. Scope of data ownership (summary)
- This area owns no database table. All state is preferences or in-memory flows.
- It reads four preference keys directly (downloaded_only, incognito_mode, onboarding_complete, nav style) plus the theme keys (theme mode, app theme, amoled) via the theme widgets.
- It writes three keys directly (downloaded_only, incognito_mode, onboarding_complete) and the theme keys through the caller.
