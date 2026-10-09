# 51-i18n topic: locales, tags, display names, RTL and date formats

Source behaviour: `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt`, `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt`, `app/src/main/AndroidManifest.xml` (line 44: `android:localeConfig="@xml/locales_config"`), `buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt`.

## 1. Locale directories to BCP 47 tags (OBSERVED directory names, PROPOSAL tags)

`LocalesConfigTask.kt` turns each directory name into a tag: `base` → `en`, `-r` → `-`, `+` → `-`. Those are the only rewrites. The Android directory names are therefore the same as BCP 47 except for the `-r` region prefix and a few legacy codes.

| Directory (core `i18n`) | Directory in `i18n-aniyomi` | Android tag (from LocalesConfigTask) | Kurobayomi tag | Notes |
|---|---|---|---|---|
| `am` | yes | `am` | `am` | Amharic |
| `ar` | yes | `ar` | `ar` | RTL |
| `as` | yes | `as` | `as` | Assamese |
| `be` | yes | `be` | `be` | Belarusian |
| `bg` | yes | `bg` | `bg` | Bulgarian |
| `bn` | yes | `bn` | `bn` | Bengali |
| `ca` | yes | `ca` | `ca` | Catalan |
| `ceb` | yes | `ceb` | `ceb` | Cebuano |
| `cs` | yes | `cs` | `cs` | Czech |
| `cv` | yes | `cv` | `cv` | Chuvash |
| `da` | yes | `da` | `da` | Danish |
| `de` | yes | `de` | `de` | German |
| `el` | yes | `el` | `el` | Greek |
| `eo` | yes | `eo` | `eo` | Esperanto |
| `es` | yes | `es` | `es` | Spanish |
| `eu` | yes | `eu` | `eu` | Basque |
| `fa` | yes | `fa` | `fa` | Persian, RTL |
| `fi` | yes | `fi` | `fi` | Finnish |
| `fil` | yes | `fil` | `fil` | Filipino |
| `fr` | yes | `fr` | `fr` | French |
| `gl` | yes | `gl` | `gl` | Galician |
| `he` | yes | `he` | `he` | Hebrew, RTL |
| `hi` | yes | `hi` | `hi` | Hindi |
| `hr` | yes | `hr` | `hr` | Croatian |
| `hu` | yes | `hu` | `hu` | Hungarian |
| `in` | yes | `in` | `id` | Indonesian, legacy Android code. Kurobayomi maps it to `id`. |
| `it` | yes | `it` | `it` | Italian |
| `ja` | yes | `ja` | `ja` | Japanese |
| `jv` | yes | `jv` | `jv` | Javanese |
| `ka-rGE` | yes | `ka-GE` | `ka-GE` | Georgian |
| `kk` | yes | `kk` | `kk` | Kazakh |
| `km` | yes | `km` | `km` | Khmer |
| `kn` | yes | `kn` | `kn` | Kannada |
| `ko` | yes | `ko` | `ko` | Korean |
| `lt` | yes | `lt` | `lt` | Lithuanian |
| `lv` | yes | `lv` | `lv` | Latvian |
| `ml` | yes | `ml` | `ml` | Malayalam |
| `mr` | yes | `mr` | `mr` | Marathi |
| `ms` | yes | `ms` | `ms` | Malay |
| `my` | yes | `my` | `my` | Burmese |
| `nb-rNO` | yes | `nb-NO` | `nb-NO` | Norwegian Bokmål |
| `ne` | yes | `ne` | `ne` | Nepali |
| `nl` | yes | `nl` | `nl` | Dutch |
| `nn` | yes | `nn` | `nn` | Norwegian Nynorsk |
| `pl` | yes | `pl` | `pl` | Polish |
| `pt` | yes | `pt` | `pt` | Portuguese |
| `pt-rBR` | yes | `pt-BR` | `pt-BR` | Brazilian Portuguese |
| `ro` | yes | `ro` | `ro` | Romanian |
| `ru` | yes | `ru` | `ru` | Russian |
| `sa` | yes | `sa` | `sa` | Sanskrit |
| `sah` | yes | `sah` | `sah` | Yakut |
| `sc` | yes | `sc` | `sc` | Sardinian |
| `sdh` | yes | `sdh` | `sdh` | Southern Kurdish |
| `sk` | yes | `sk` | `sk` | Slovak |
| `sq` | yes | `sq` | `sq` | Albanian |
| `sr` | yes | `sr` | `sr` | Serbian |
| `sv` | yes | `sv` | `sv` | Swedish |
| `ta` | yes | `ta` | `ta` | Tamil |
| `te` | yes | `te` | `te` | Telugu |
| `th` | yes | `th` | `th` | Thai |
| `tr` | yes | `tr` | `tr` | Turkish |
| `uk` | yes | `uk` | `uk` | Ukrainian |
| `ur` | yes (empty file) | not listed (empty) | `ur` (not offered until translated) | Urdu, RTL. Aniyomi only. |
| `uz` | yes | `uz` | `uz` | Uzbek |
| `vi` | yes | `vi` | `vi` | Vietnamese |
| `zh-rCN` | yes | `zh-CN` | `zh-CN` | Simplified Chinese. Display name uses `zh-Hans`. |
| `zh-rTW` | yes | `zh-TW` | `zh-TW` | Traditional Chinese. Display name uses `zh-Hant`. |
| `base` | yes | `en` | `en` | English source. |

Counts: core has 66 translated directories plus `base` (67 directories). Aniyomi has 67 translated directories (including the empty `ur`) plus `base` (68 directories).

Locales with RTL script: `ar`, `fa`, `he`, `ur`.

Locale tags present in the Aniyomi module but not in core: none except `ur`. Core has no locale that Aniyomi lacks.

## 2. Generated locale list (OBSERVED, `LocalesConfigTask.kt`)

- Filter: a directory is listed only if its `strings.xml` does not match `<resources>\s*</resources>` or `<resources\s*/>`. The regex is `"<resources>\\s*</resources>|<resources\\s*/>".toRegex()`.
- Result: `xml/locales_config.xml` with one `<locale android:name="<tag>"/>` line per listed directory, sorted by the tag string (`sorted()` on the mapped names).
- Both `i18n` and `i18n-aniyomi` write this file into their own generated Android resource directory. The app merges both. Which file the app ships is decided by the Android resource merger. That was NOT verified by a build in this analysis. It is an open question for the Aniyomi project, and does not block Kurobayomi, which has one list.

## 3. Display names (OBSERVED, `LocaleHelper.kt`)

Function `getLocalizedDisplayName(lang)`:
- `null` → `""`.
- `""` → the system default language: `LocaleListCompat.getAdjustedDefault()[0]`.
- `"zh-CN"` → `Locale.forLanguageTag("zh-Hans")`. `"zh-TW"` → `Locale.forLanguageTag("zh-Hant")`.
- Otherwise `Locale.forLanguageTag(lang)`.
- Output: `locale.getDisplayName(locale)` (the name in that language, for example `Deutsch` for `de`), with the first character uppercased in that locale (`replaceFirstChar { it.uppercase(locale) }`).

Function `getDisplayName(lang)` (the supporting line in the picker):
- Normalises `zh-CN` → `zh-Hans`, `zh-TW` → `zh-Hant`, then `Locale.forLanguageTag(normalized).displayName`. This is the name in the JVM default locale (English when the system is English), not the native name.

Picker sort: by the localized name (`langs.sortBy { it.displayName }`), with the Default row first.

Source-language sort (`LocaleHelper.comparator`): `"all"` first, then the localized name.

PROPOSAL for Kurobayomi:
- Store two strings per locale in a Rust table: `native` (name in its own language, the primary line) and `english` (the English name, the supporting line). Hand-maintained table of 66 plus `ur`. Generated once with the ICU4X display-names data (crate `icu_displaynames`) and committed, so there is no runtime data dependency. Reason: the Aniyomi output depends on the JVM's locale data version, a committed table gives stable names.
- Apply the same first-letter uppercasing rule to the native name (`to_uppercase` on the first `char`).
- Sort the picker by the native name using `str::cmp` on the Unicode string (Android uses `String.compareTo`, which is also code-unit order). For Latin-script languages with accents, this is the same as Android's order only if the display strings match exactly; accept the difference and note it in the picker tests.
- Default row first with label `label_default` ("Default").

## 4. Language list used by source filters (OBSERVED)

- `LocaleHelper.getDefaultEnabledLanguages()` returns `setOf("all", "en", Locale.getDefault().language)`. The device language is the JVM default language (for example `de`).
- `getSourceDisplayName` maps the three special keys (`last_used`, `pinned`, `other`) and `all` (shown as "Multi") to strings. Full rules in [p1-behavior.md](p1-behavior.md) §5.
- The source-language first-run rule in `MangaExtensionManager.kt:178-183` and `AnimeExtensionManager.kt:183` uses `Locale.getDefault().language` and `startsWith`. PROPOSAL: in Kurobayomi this rule takes its device language from the resolved UI locale (section 6). The rule itself stays as it is. Owner: the source/extension area.

## 5. Tag normalisation for user input and system input (PROPOSAL)

Input examples: `pt_BR.UTF-8`, `zh_CN.UTF-8`, `de_DE@euro`, `C`, `POSIX`, `zh-Hans-CN`, `zh-Hant`, `sr-Latn`, `in`, `iw`, `ka`.

Rules, applied in order:
1. Strip encoding (`.UTF-8`) and modifier (`@euro`). Replace `_` with `-`.
2. `C` and `POSIX` → `en`. Empty → skip.
3. Parse with `unic-langid` (`LanguageIdentifier::from_str`). If parsing fails, skip the entry.
4. Legacy codes: `in` → `id`, `iw` → `he`, `ji` → `yi`. (Java's legacy names.)
5. Script: `zh-Hans*` or `zh` alone → `zh-CN`; `zh-Hant*`, `zh-HK`, `zh-MO`, `zh-TW` → `zh-TW`. `sr-Latn` → `sr` (no Latin Serbian file, falls back to `sr`). Other scripts are dropped.
6. Lookup against the offered list: exact tag first; then language-only match; then `en`.

Lookup chain for a requested tag `T` (PROPOSAL): `[T, language(T), "en"]` with duplicates removed. Example: `pt-PT` → `[pt-PT, pt, en]`, and `pt` is offered → Portuguese.

## 6. Locale detection order at startup (PROPOSAL)

1. Preference `app_language` (string, default `""`). If non-empty, use it through the normalisation in section 5.
2. Otherwise `glib::language_names()`. GLib builds this from `LANGUAGE` (colon-separated priority list), then `LC_ALL`, `LC_MESSAGES`, `LANG`. Use the list in order; the first entry that resolves to an offered tag wins.
3. Otherwise `en`.

GTK's own translations are not involved: Kurobayomi's strings come from the catalog only.

## 7. RTL behaviour (PROPOSAL, GTK-specific)

- GTK chooses the default text direction from the locale at startup (for `ar`, `fa`, `he`, `ur` it is right-to-left). When the user picks an RTL language but the environment is not RTL, call `gtk::Widget::set_default_direction(gtk::TextDirection::Rtl)` before any widget is built. When the user picks a non-RTL language in an RTL environment, call it with `Ltr`.
- Direction changes do not affect widgets already built. Show the restart note (behaviour §9.6).
- Semantics that must be mirrored, not just the layout:
  - Reader chapter swipes: Aniyomi's `pref_chapter_swipe_end` (label "Chapter on swipe to right") is documented as "This should be to the left for RTL locales". Kurobayomi must make the setting names follow the visual direction: in RTL, "swipe to right" (end) moves to the left side in the UI.
  - Navigation tap zones (`nav_zone_prev`, `nav_zone_next`, `nav_zone_left`, `nav_zone_right`) and the reader modes `right_to_left_viewer` are not mirrored automatically by the locale. Keep them as set by the user. Only the labels follow direction.
  - Numbers, times and Latin text inside RTL sentences: Fluent output is passed through unchanged; Pango handles the bidi algorithm.

## 8. Date and time (OBSERVED and PROPOSAL)

OBSERVED:
- Date format preference `app_date_format` (default `""`). `""` → `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)` with the JVM default locale. Any other value → `DateTimeFormatter.ofPattern(format, Locale.getDefault())`.
- The five fixed formats offered (`SettingsAppearanceScreen.kt`): `MM/dd/yy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd MMM yyyy`, `MMM dd, yyyy` plus the Default entry.
- Relative date and time strings: section 6 of [p1-behavior.md](p1-behavior.md).
- `SimpleDateFormat("yyyy-MM-dd", Locale.getDefault())` is used for tracker search previews and `SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.getDefault())` in the backup restorer (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt:307`). The restorer one is a file-level timestamp and must not be localised.

PROPOSAL:
- Default short date: ICU4X `icu_datetime` with length `Short` for the resolved locale. (ICU short patterns differ per locale from the JVM's; accept the difference and document it.)
- Fixed formats use chrono `strftime` strings: `MM/dd/yy` → `%m/%d/%y`, `dd/MM/yy` → `%d/%m/%y`, `yyyy-MM-dd` → `%Y-%m-%d`, `dd MMM yyyy` → `%d %b %Y`, `MMM dd, yyyy` → `%b %d, %Y`. Caveat: chrono's `%b` gives English month names. For `MMM` formats in non-English locales use ICU4X month names (`icu_datetime` symbols), not chrono.
- Backup and file-name timestamps: fixed `yyyy-MM-dd HH:mm:ss.SSS` in UTC or local time without localisation. Keep the Android behaviour (`Locale.getDefault()` only affects digits, which are ASCII here).

## 9. Digits and numbers

- Java `%d` uses the locale's `Formatter` digits only for locales with the `nu` extension. Aniyomi's base files never request localised digits. PROPOSAL: ASCII digits everywhere (Fluent `NUMBER` with `useGrouping: "false"`), matching Aniyomi's output for every offered locale. Revisit if a locale needs native digits.
- Thousands grouping: none in Aniyomi's base strings (no `%,d`). PROPOSAL: keep none.

## 10. Font coverage

Scripts used by offered locales and their system font needs on Linux (PROPOSAL, to be checked by the UI layer):
- Arabic (`ar`, `fa`, `ur`), Hebrew (`he`), Thai (`th`), Khmer (`km`), Burmese (`my`), Bengali (`bn`), Assamese (`as`), Kannada (`kn`), Malayalam (`ml`), Tamil (`ta`), Telugu (`te`), Marathi and Hindi and Sanskrit (`mr`, `hi`, `sa`, Devanagari), Amharic (`am`, Ethiopic), Georgian (`ka-GE`), Chinese (`zh-CN`, `zh-TW`), Japanese (`ja`), Korean (`ko`).
- GTK and Pango pick fonts through fontconfig. Kurobayomi must not bundle fonts (no Android font assets exist in this scope). The app depends on the user having a font for the script; the Settings "About" page does not list missing fonts. Note this as a known gap in the edge-case file.
