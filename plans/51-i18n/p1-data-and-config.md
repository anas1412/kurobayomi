# 51-i18n: data models, preference keys, files and config

Paths are repo-relative to `/home/blackbox/aniyomi-src` unless marked as Kurobayomi (`/home/blackbox/kurobayomi`).

## 1. Source data: moko-resources XML

### 1.1 File schema (OBSERVED)

```xml
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="KEY" [translatable="false"]>TEXT</string>
    <plurals name="KEY">
        <item quantity="one|other|zero|two|few|many">TEXT</item>
    </plurals>
</resources>
```

- Root element `<resources>`. No other elements are used in the scope files.
- `name` values: `[A-Za-z0-9_.]+`. Dots occur only in the core module's extension-store keys (`action.addExtensionStore`, `extensionStoresScreen.emptyLabel`, `extensionStoresScreen.addStore.title`, `extensionStoresScreen.addStore.alreadyExists`, `extensionStoresScreen.addStore.processing`, `extensionStoresScreen.addStoreInput.inputLabel`, `extensionStoresScreen.addStoreDeeplink.bodyText`, `extensionStoresScreen.deleteStore.title`, `extensionStoresScreen.deleteStore.body`).
- `translatable="false"` is the only attribute used. Count: 5 in core (`app_name`, `action_web_view`, `ext_installer_packageinstaller`, `ext_installer_shizuku`, `ext_installer_private`), 17 in aniyomi (7 `pref_progress_*`, 5 `pref_skip_*`, `player_speed`, 3 `player_sheets_track_*`, `player_gesture_seek_indicator`).
- Comments are not data.

### 1.2 Counts

| File | Module | Entries | Notes |
|---|---|---|---|
| `i18n/src/commonMain/moko-resources/base/strings.xml` | core | 857 `<string>` (998 lines) | |
| `i18n/src/commonMain/moko-resources/base/plurals.xml` | core | 19 `<plurals>` (98 lines) | quantities: one, other |
| `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` | aniyomi | 570 `<string>` (615 lines) | |
| `i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml` | aniyomi | 15 `<plurals>` (63 lines) | quantities: one, other |

Translation files (every other directory under each `moko-resources/`): same two file names. Non-empty translations: core 66 locales, aniyomi 66 locales (67 directories minus empty `ur`). Plurals files exist in 64 core and 20 aniyomi translated locales.

### 1.3 Empty locale file (OBSERVED)

`i18n-aniyomi/src/commonMain/moko-resources/ur/strings.xml` has 3 lines, no `<string>` elements. It is excluded from the locale config by `LocalesConfigTask.kt`'s regex. Whether `ur` has a `plurals.xml` was not recorded; it does not affect the locale list.

### 1.4 Size and string-length facts

- Examples of long base values (approximate length): `data_saver_image_format_summary_on` and `_off` (about 190 characters each, aniyomi lines 583-584), `pref_player_torrents_notice_text` (about 190 characters), `onboarding_storage_info` (about 150 characters).
- Smallest translated coverage: `te` (192 core strings, 1 aniyomi string), `km` (207 core, 3 aniyomi), `ml` (195 core, 13 aniyomi), `mr` (209 core, 3 aniyomi).
- Full coverage: core `de`, `el`, `es`, `ru`, `tr` with 842 strings each (of 857 base; the 15 gap is probably the non-translatable names and some strings intentionally left in English; not verified by key diff).

## 2. Generated artifacts (OBSERVED build plumbing)

| Artifact | Producer | Path | Content |
|---|---|---|---|
| `MR` Kotlin accessor (core) | moko plugin, `i18n/build.gradle.kts` (`multiplatformResources { resourcesPackage.set("tachiyomi.i18n") }`) | generated, Gradle build dir | one property per key, `MR.strings.<key>` and `MR.plurals.<key>` |
| `AYMR` Kotlin accessor (aniyomi) | moko plugin, `i18n-aniyomi/build.gradle.kts` (`resourcesClassName.set("AYMR")`, `resourcesPackage.set("tachiyomi.i18n.aniyomi")`) | generated | same, `AYMR.*` |
| Android resources | moko and AGP, `generatedAndroidResourceDir = generatedBuildDir.resolve("android/res")` | generated | `R.string.*`, `R.plurals.*` |
| `xml/locales_config.xml` | `generateLocalesConfig` task (`LocalesConfigTask.kt`) | `<generatedBuildDir>/android/res/xml/locales_config.xml` | list of `<locale android:name="…"/>` |

The exact generated XML format from `LocalesConfigTask.kt` (template):

```xml
<?xml version="1.0" encoding="utf-8"?>
<locale-config xmlns:android="http://schemas.android.com/apk/res/android">
|   <locale android:name="af"/>
|   …
</locale-config>
```

(The `|` characters are the `trimMargin` delimiter used in the Kotlin source. The generated file does not contain them.)

Referenced from: `app/src/main/AndroidManifest.xml:44` (`android:localeConfig="@xml/locales_config"`) and `app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt:99` (`context.resources.getXml(R.xml.locales_config)`).

Lint and formatting (OBSERVED, `buildSrc/src/main/kotlin/mihon.code.lint.gradle.kts`): Spotless formats `**/*.xml` (trailing whitespace trimmed, final newline added). Its exclusion list contains every moko-resources file that is not under `base/`, so the base English files are formatted and the translation files are not. Android lint disables `MissingTranslation` and `ExtraTranslation` in both modules.

## 3. Kurobayomi data model (PROPOSAL)

### 3.1 Message catalog

```rust
/// One message in the English source catalog.
pub struct MessageDef {
    pub key: &'static str,            // Fluent id, e.g. "relative_time"
    pub kind: MessageKind,
    pub module: SourceModule,         // Core | Aniyomi
    pub translatable: bool,           // false = English only
    pub placeholders: &'static [Placeholder],
}

pub enum MessageKind {
    Plain,                            // <string>
    Plural { has_count_arg: bool },   // <plurals>
}

pub enum SourceModule { Core, Aniyomi }

pub struct Placeholder {
    pub arg: &'static str,            // "arg1", "count"
    pub spec: PlaceholderSpec,
}

pub enum PlaceholderSpec {
    Str,                              // %s, %N$s, %c
    Int { zero_pad: Option<u8> },     // %d, %N$d, %0Nd
    Decimal { fraction_digits: u8 },  // %.Nf
}
```

Counts for generation: 876 messages from the core module (857 + 19) and 585 from aniyomi (570 + 15) before the 27-key merge, which leaves 1,434 unique messages after merging (876 + 585 − 27). Not counting the 27 core duplicates, which are dropped. Verified count: 1,461 base names in total across both modules (see catalog), which is 1,434 unique keys plus 27 duplicates.

### 3.2 Locale record

```rust
pub struct LocaleInfo {
    pub tag: &'static str,            // BCP 47, e.g. "pt-BR"
    pub android_dir: &'static str,    // e.g. "pt-rBR" (for traceability)
    pub native_name: &'static str,    // "Português (Brasil)"
    pub english_name: &'static str,   // "Portuguese (Brazil)"
    pub rtl: bool,                    // ar, fa, he, ur
    pub translated_messages: u32,     // computed by xtask, not hand-maintained
}
```

### 3.3 Plural quantity enum

`PluralCategory`: `Zero`, `One`, `Two`, `Few`, `Many`, `Other`. Names and meanings follow Android (`quantity` attribute values, lowercase). Fluent variant names are the same lowercase words.

### 3.4 Application-level message key set (PROPOSAL)

`MsgKey` is a generated enum (one variant per message, 1,434 variants) so a typo in code fails to compile. Placeholders are passed as `&[(&str, FluentValue)]`.

## 4. Preference keys touched by this area

| Key | Type | Default | Used by (OBSERVED) | Kurobayomi |
|---|---|---|---|---|
| `app_date_format` | String | `""` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:35`; options in `SettingsAppearanceScreen.kt` | keep, area:02 preferences store |
| `relative_time_v2` | Boolean | `true` | `UiPreferences.kt` `relativeTime()` | keep |
| (AppCompat per-app locale store) | String tag, managed by Android | none (system) | `AppLanguageScreen.kt:47` `AppCompatDelegate.getApplicationLocales()` | replace with `app_language` |
| `app_language` (new) | String | `""` (system) | none in Aniyomi | Kurobayomi only: BCP 47 tag, `""` means follow the system |
| `enabledLanguages` (source languages) | Set<String> | `setOf("all", "en", <device language>)` | `LocaleHelper.getDefaultEnabledLanguages()` | owned by the source area |

Preference type notes: `app_date_format` stores a Java `DateTimeFormatter` pattern string verbatim. Kurobayomi must store the same strings (`""`, `MM/dd/yy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd MMM yyyy`, `MMM dd, yyyy`) so that existing values carry over. Conversion to chrono and ICU patterns happens at format time (topic file).

## 5. Files on disk

### 5.1 Aniyomi (read-only source, OBSERVED)

- Source XML files listed in section 1.
- No runtime files: strings live in the APK's compiled resources.

### 5.2 Kurobayomi (PROPOSAL)

| Path | Created by | Purpose |
|---|---|---|
| `/home/blackbox/kurobayomi/locales/en/kurobayomi.ftl` | `xtask i18n-import` | English source catalog (generated, committed) |
| `/home/blackbox/kurobayomi/locales/<tag>/kurobayomi.ftl` | `xtask i18n-import` | Translations (generated, committed) |
| `/home/blackbox/kurobayomi/locales/README.md` | hand-written | Translator and import instructions |
| `/home/blackbox/kurobayomi/xtask/src/i18n_import.rs` | hand-written | XML to Fluent converter |
| `/home/blackbox/kurobayomi/src/i18n/` | hand-written | Runtime catalog, locale resolution, display names |
| `$XDG_CONFIG_HOME/kurobayomi/` (preferences file) | area:02 | stores `app_language`, `app_date_format`, `relative_time_v2` |

No file is written at runtime by the i18n code.

## 6. Network endpoints

None in this scope. The XML files and the generated list do not contact the network. (Weblate is the external translation tool, used by translators; it is not contacted by the app.)

Data-saver strings (`bandwidth_hero`, `wsrv`, `resmush`, `bandwidth_data_saver_server`, `data_saver_server_summary`) describe a proxy server that the Aniyomi data-saver feature calls for image compression. That feature is outside this scope; the endpoints it uses are owned by the image area.

## 7. Database tables and columns

None. Strings are not stored in the database. Sources and extension data store language codes (for example `lang` on source records), which are BCP 47-like but not necessarily the same as Android directory names; that data is owned by the source and database areas.

## 8. Enumerations used by strings (value spaces)

| Enum or value set | Values (stored) | Display key |
|---|---|---|
| Date format preset | `""`, `MM/dd/yy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd MMM yyyy`, `MMM dd, yyyy` | `label_default` for `""`; otherwise the literal pattern |
| Library update interval (not in this scope; string keys only) | see behaviour §7.5 | `update_never`, `update_6hour`, `update_12hour`, `update_24hour`, `update_48hour`, `update_72hour`, `update_weekly` |
| Plural quantity | `zero`, `one`, `two`, `few`, `many`, `other` | per message |
| Reader flash style (`pref_flash_with`) | Black, White, White and Black | `pref_flash_style_black`, `pref_flash_style_white`, `pref_flash_style_white_black` |
| Tap-zone inversion | none, horizontal, vertical, both | `tapping_inverted_none`, `tapping_inverted_horizontal`, `tapping_inverted_vertical`, `tapping_inverted_both` |

## 9. Constants

- Language sentinel values in `LocaleHelper.kt`: `"all"` (multi-language), `"en"`, `"other"`, `"pinned"` (`PINNED_KEY`), `"last_used"` (`LAST_USED_KEY`), `"zh-CN"`, `"zh-TW"`, `"zh-Hans"`, `"zh-Hant"` (display normalisation).
- Empty-locale regex: `<resources>\s*</resources>|<resources\s*/>`.
- `sorted()` order on locale tags in `LocalesConfigTask.kt`.
- Relative-time thresholds: `-7`, `0`, `1`, `7` days; hour and minute thresholds: `< 0`, `< 1`, `== 0` (behaviour §6.2).
- Duration component thresholds: `days != 0`, `hours != 0`, `minutes != 0`, `seconds != 0` (behaviour §6.3).
- Just-now threshold: `1.minutes`.
