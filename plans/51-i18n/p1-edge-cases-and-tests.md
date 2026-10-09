# 51-i18n: edge cases, failure modes, tests

## 1. Upstream tests to port

No `*Test.kt` file in the Aniyomi tree exercises the code in this scope or its direct callers (search for `LocaleHelper`, `toRelativeString`, `toDurationString`, `UiPreferences.dateFormat`, `getLocalizedDisplayName` in `*Test*.kt` files returned nothing). Nothing to port. The tests below are new, written for Kurobayomi.

## 2. Edge cases (per file and per behaviour)

### 2.1 XML sources

| # | Case | Source evidence | Expected behaviour (Kurobayomi) |
|---|---|---|---|
| E1 | `<string>` with `translatable="false"` in a translation file | 22 names in base are non-translatable (5 core, 17 aniyomi) | Translation value ignored; English used. Importer warns. |
| E2 | Escaped apostrophe `\'` | `onboarding_description`, `pref_show_next_episode_airing_time` (aniyomi, `episode\'s`) | Renders `'`. |
| E3 | Escaped quote `\"` | `action_add_repo_message`, `pref_player_torrents_proxy_url_invalid_protocol`, `ext_*` | Renders `"`. |
| E4 | `\n` in value | `onboarding_storage_info`, `data_saver_image_format_summary_on` | Renders newline. |
| E5 | `&amp;` in value | `pref_appearance_summary`, `theme_tealturquoise` ("Teal &amp; Turquoise") | Renders `&`. |
| E6 | Trailing space in value | aniyomi `pref_show_next_episode_airing_time` = "Show next episode's airing time " | Keep (Fluent trims trailing whitespace in patterns, so the converter must write it as `{" "}` to keep it; decide with T-51a-020). |
| E7 | Empty `<resources/>` locale | aniyomi `ur` | Locale not listed, not offered in picker; `ur` strings fall back to English. |
| E8 | Locale with only a few strings | aniyomi `te` (1 string), `ml` (13), `da` (4) | Works; per-message fallback to English. The picker still lists them (Aniyomi lists any non-empty locale). |
| E9 | Stale key in translation (not in base) | not observed in this scan; possible in Weblate | Dropped with a warning (converter). |
| E10 | Duplicate base key across the two modules | 27 names (see catalog) | Aniyomi value wins; core value discarded (decision in topic xml-to-fluent §6). |
| E11 | Plural with only `one` and `other` in base, translation adds `few`/`many` | e.g. ru, pl, cs (Slavic) | Uses `few`/`many` rules via Fluent CLDR; other locales fall back to `other`. |
| E12 | Plural translation missing `other` | not observed; defensive | Plural falls back to English for that message (converter error). |
| E13 | Placeholder count differs between base and translation | not observed; defensive | Converter error; that message falls back to English. |
| E14 | Placeholder reordered by translator (`%2$s` before `%1$s`) | normal for translations | Allowed: same set of argument numbers required, any order. |
| E15 | Literal `%` in translation without `%%` | not observed; defensive | Converter treats as the start of a specifier and fails if invalid; repair manually in Weblate. |
| E16 | Non-ASCII text (Arabic, Thai, Khmer, Chinese) | many locales | UTF-8 throughout; no escaping needed. |
| E17 | Translator text containing `{` or `}` | defensive | Converter writes `{"{"}` and `{"}"}`. |
| E18 | Translator text starting with a space | defensive | Converter writes `{" "}` for each leading space. |

### 2.2 Formatting and placeholders

| # | Case | Expected |
|---|---|---|
| F1 | `%d` with a negative count | `upcoming_relative_time` is never called with negatives (callers use `absoluteValue`). Rust wrapper also takes `abs()` in `time.rs`. |
| F2 | `%02d` with 0 | `00`. `restore_duration` with 0 minutes → `00 min, 05 sec`. |
| F3 | `%02d` with more than 99 | Three digits (no truncation). Matches Java. |
| F4 | `%.2f` with NaN or infinity | Not reachable (speed values are finite). Rust wrapper must not pass NaN; assert in debug builds. |
| F5 | `%c` with an empty string | Produces no sign character; `player_gesture_seek_indicator` then shows the value with no sign. Callers must pass `+`, `-`, or a space. |
| F6 | `%s` with a 10 000-character title | Rendered in full; the layout layer ellipsises (UI area). |
| F7 | `%s` with a value containing `{` `}` | Fluent treats argument values as data, not syntax: no escaping needed. Test it. |
| F8 | Missing argument at runtime | Fluent renders the placeholder as `{$argN}` text and logs an error. Must never happen: covered by the import-time placeholder check and a unit test that calls every message with its declared args. |
| F9 | Extra argument at runtime | Ignored by Fluent (no error). Tests fail on extra args to catch call-site bugs. |
| F10 | Count of 1 for `notification_chapters_multiple_and_more` | `one` form: `Chapters %1$s and 1 more` (literal 1). |
| F11 | Count of 0 for plurals | Category `other` in English (`0 days ago`). Languages with `zero` category (`ar`, `lv`, and others) use `zero` if translated. |
| F12 | Count of 2 in Arabic (`two` category) | Uses `two` if translated, otherwise `other`. |

### 2.3 Locale resolution

| # | Case | Expected |
|---|---|---|
| L1 | `app_language = ""` and `LANG=C` | `glib::language_names()` yields `C`, mapped to `en`. |
| L2 | `LANGUAGE=de:en` | Tries `de` first. If `de` is offered, use it. |
| L3 | `LANG=pt_BR.UTF-8` | Normalised to `pt-BR`, offered. |
| L4 | `LANG=pt_PT.UTF-8` | `pt-PT` not offered → `pt` (language match) → offered. |
| L5 | `LANG=zh_CN.UTF-8` | `zh-CN`. |
| L6 | `LANG=zh_Hant_TW` | Script rule → `zh-TW`. |
| L7 | `LANG=zh` alone | `zh-CN` (default for bare Chinese, decision in topic locale §5). |
| L8 | `LANG=in_ID` (legacy Indonesian) | `id-ID` → `id` (Kurobayomi tag for the Android directory `in`). |
| L9 | `LANG=he_IL` (legacy `iw`) | `he-IL` → `he`. RTL on. |
| L10 | `LANG=xx_YY` (unknown language) | Not offered → `en`. |
| L11 | `app_language = "xx"` (saved but no longer offered) | Treated as not set; `en` or system fallback; picker shows Default checked. |
| L12 | `app_language = "bogus tag!"` | Unparsable → ignored (`normalise` returns None). Log once. |
| L13 | `LANGUAGE` with empty entries (`:de`) | Empty entries skipped. |
| L14 | `LANG=POSIX` | Treated as `en`. |
| L15 | `LANG=de_DE@euro` | Modifier stripped → `de-DE` → `de`. |
| L16 | `LANG=sr_RS@latin` | Modifier dropped, `sr-RS` → `sr`. Latin Serbian is not separately offered. |

### 2.4 Display names

| # | Case | Expected |
|---|---|---|
| D1 | `ar` native name | Arabic script, first character uppercased (no effect on Arabic). |
| D2 | `tr` native name `Türkçe` | First char `T` uppercased (no change). |
| D3 | `ka-GE` native | `ქართული` (no case). |
| D4 | `zh-CN` native | `简体中文` (Simplified). Uses the `zh-Hans` name, not Java's `zh-CN` name. |
| D5 | Sorting picker by native name | Code-point order. Documented difference from Android's `compareTo` only for locale-collated Latin names. |
| D6 | `""` (Default) row | Always first, label `label_default`. |
| D7 | Two languages with equal native names | Not present in the table; test asserts the table is unique. |

### 2.5 Relative time and dates

| # | Case | Expected (from behaviour §6) |
|---|---|---|
| T1 | difference = -8 days | Absolute date (`dateFormat.format`). |
| T2 | difference = -1 day | `upcoming_relative_time` one: `Tomorrow`. |
| T3 | difference = -3 days | `In 3 days`. |
| T4 | difference = 0 | `Today`. |
| T5 | difference = 1 | `Yesterday`. |
| T6 | difference = 2 | `2 days ago`. |
| T7 | difference = 6 | `6 days ago`. |
| T8 | difference = 7 | Absolute date (`< 7` fails). |
| T9 | LocalDateTime, time difference -0.5 days, dateDifference 0 | Hour branch: `hourDifference` computed from times: depends on the exact hours. Test with fixed clock. |
| T10 | Hour difference = 0, minute difference = 0 | `Now`. |
| T11 | Hour difference = 1 | `1 hour ago`. |
| T12 | Minute difference = 1 | `1 minute ago`. |
| T13 | Future minute difference -1 | `In 1 minute`. |
| T14 | Duration 0 | `fallback` string (caller-provided). |
| T15 | Duration 1 day 0 hours 5 minutes | `1d 5m` (minutes shown because `hours == 0`; the seconds part is 0). |
| T16 | Duration 1 day 3 hours 5 minutes | `1d 3h` (minutes hidden because days != 0 and hours != 0). |
| T17 | Duration 45 seconds | `45s`. |
| T18 | Duration 2 minutes 30 seconds | `2m 30s` (seconds shown because days == 0 and hours == 0). |
| T19 | Duration 1 hour 30 seconds | `1h` (seconds hidden since hours != 0). |
| T20 | epoch 0 | `Never`. |
| T21 | 30 seconds ago | `Just now`. |
| T22 | 5 minutes ago | Android framework text (`DateUtils`). Rust output must be the catalog form: decide the Rust output in T-51a-016 (proposal: `5 minutes ago` through `relative_time_minutes`). Test it. |
| T23 | Short date for `en-US` | Locale-specific; test snapshot per locale. |
| T24 | Fixed pattern `MMM dd, yyyy` in `de` | Month name in German (ICU4X), not English (chrono). |

### 2.6 RTL and GTK

| # | Case | Expected |
|---|---|---|
| R1 | `ar` selected, system `en` | `set_default_direction(Rtl)` before widgets; the whole UI mirrors after restart. |
| R2 | RTL system, `app_language = "en"` | `set_default_direction(Ltr)`. |
| R3 | Mixed text (Arabic title with a Latin number `Chapter 12`) | Pango bidi rendering; no manual marks. Visual check. |
| R4 | Swipe setting label in RTL | Still "Chapter on swipe to right" (text), position visually left (UI). |
| R5 | Icon without RTL variant in RTL | Listed in UI area task, flipped explicitly. |

### 2.7 Notifications and background threads

| # | Case | Expected |
|---|---|---|
| N1 | Library update finishes on a tokio thread with 3 new chapters | Thread sends `UiMsg::Notify { key: notification_chapters_generic, args }`; text built on the main thread. |
| N2 | Language changed while a notification is pending | Pending notification uses the language at display time (not at creation). Restart clears it. |
| N3 | 0 remaining in `notification_chapters_multiple_and_more` | Not called (Aniyomi only calls it when `remaining > 0`; verify in the notifier area). Rust wrapper asserts `remaining >= 1`. |
| N4 | Very long joined chapter list (`Chapters 1, 2, …`) | Rendered in full; notification body length limit is owned by the notification area. |

### 2.8 Escapes and Fluent-specific

| # | Case | Expected |
|---|---|---|
| X1 | `\\` in XML value | Converter writes a single `\` (text; Fluent text has no escapes). |
| X2 | Unicode escape in translation value `é` written literally | Not an XML escape: stays `é` text. Converter must not decode it. Test. |
| X3 | Message id beginning with a digit | Not produced (no such key). Converter rejects it. |
| X4 | Message ids that are Rust keywords | Aniyomi has `type` (`strings.xml` line 379, value "Type"), a Rust keyword. The generator emits `r#type` for it. The generator must check every id against the keyword list. Test that the generated enum compiles. |
| X5 | Key `player_sheets_filters_Saturation` (capital S) | Kept. Rust enum variant `player_sheets_filters_Saturation` is allowed with `#[allow(non_camel_case_types)]`. |
| X6 | Isolation marks | None (`set_use_isolating(false)`). Test: rendered text has no U+2068 or U+2069. |

## 3. Failure modes

| Failure | Impact | Mitigation |
|---|---|---|
| Missing `.ftl` for a locale in the binary | Language offered but not loaded | Build script refuses an empty locale directory (except `en`); CI test loads every offered locale. |
| Fluent parse error in one message | That message falls back to English | Converter validates; runtime logs once. |
| Invalid `app_language` value | Falls back to system or English | `normalise` returns None; picker shows Default checked. |
| Panic on missing key | Crash | Forbidden: missing key returns the id. Test. |
| Clock change during relative-time calculation | Off-by-one label | Acceptable; the label is recomputed when the list is redrawn. |
| Font missing for a script | Boxes or wrong glyphs | Documented gap (locale topic §10). Not solved by i18n. |
| GTK's own dialogs (file chooser, about) in another language | Mixed UI | GTK follows the environment, not `app_language`. Document; ships as-is. |
| Restart note ignored by user | Old language stays until restart | Expected. |
| Translation adds a key not in English | Dropped with warning | Test counts stale keys = 0 for shipped files. |
| Weblate export with CRLF line endings | Parse difference | `quick-xml` handles CRLF; converter normalises to LF in output. |

## 4. Race conditions

- `OnceLock` initialisation race: two threads call `tr()` before `init()`. `tr()` must initialise with English if `init()` has not run, and a later `init()` must be ignored with a warning. Test with two threads.
- Preference write while the picker is open: the picker holds the pending selection in memory; the write happens on click. No race beyond last-writer-wins.
- Two notifications with the same key on different threads: formatting happens on the main thread, serialised by the channel. No race.

## 5. Tests to write

### 5.1 Unit tests (`src/i18n/`)

1. `normalise_tags`: table-driven, covers L1–L16 inputs and expected tags.
2. `resolve_chain`: `["pt-BR"]` → `["pt-BR", "pt", "en"]`, `["zh-Hant-TW"]` → `["zh-TW", "en"]`.
3. `format_plain`: each `MsgKey` with no args renders non-empty and has no `{$` in the output (checks for unfilled placeholders).
4. `format_args_match`: for every message, calling with its declared placeholder names produces no Fluent error.
5. `plural_categories_en`: `count` 0, 1, 2, 5 selects `other` except 1 → `one`.
6. `plural_categories_ru`: 1, 2, 5, 11, 21, 22, 25 produce the correct CLDR categories (`one`, `few`, `many`, `other`) for `relative_time` if `ru` has those translated (fixture file).
7. `plural_fallback`: a locale without `few` falls back to `other`.
8. `int_formatting`: `Arg::Int(1000)` in `%d`-style message → `1000` (no grouping); `Arg::Int(7)` with `%02d` → `07`.
9. `decimal_formatting`: `Arg::Dec(1.5)` with `%.2f` → `1.50`.
10. `no_isolation_marks`: no U+2068 or U+2069 in any output.
11. `escape_roundtrip`: for each base XML string, the converter's output rendered with dummy args equals the expected unescaped XML text (fixture of 20 representative strings: quotes, newlines, ampersand, braces).
12. `keys_unique`: generated `MsgKey` ids are unique; the enum has exactly the count in `locales/en/kurobayomi.ftl`.
13. `rtl_flag`: `ar`, `fa`, `he`, `ur` are RTL; `en`, `de` are not.
14. `default_row_first`: picker list builder returns `""` first.
15. `picker_sort`: rows after the first are sorted by native name.
16. `unknown_saved_tag`: picker shows Default as the checked row.
17. `relative_date_thresholds`: table of T1–T8 with a fixed clock value.
18. `relative_datetime_thresholds`: T9–T13.
19. `duration_short`: T14–T19 table.
20. `just_now_and_never`: T20, T21.
21. `missing_key_no_panic`: `MsgKey` absent in one locale returns English; an id absent everywhere returns the id.
22. `overlap_precedence`: for each of the 27 overlap keys, the English text equals the Aniyomi base value (fixture from the XML).

### 5.2 Importer tests (`xtask`)

23. `parse_moko_counts`: core 857 strings + 19 plurals; aniyomi 570 strings + 15 plurals (the counts in the inventory). Fails the build if the numbers change without updating the fixture.
24. `placeholder_check`: a translation with a missing `%2$s` is rejected for that message only.
25. `stale_key_dropped`: a translation key not in base is dropped and reported.
26. `empty_locale_skipped`: `ur` in aniyomi is skipped (the core module has no `ur`).
27. `non_translatable_skipped`: translation value for `app_name` in a fixture locale is ignored.
28. `converter_validates`: every generated `.ftl` parses with `fluent_syntax`.

### 5.3 Integration / manual

29. Launch with `LANG=ar_EG.UTF-8`: window mirrors, labels in Arabic.
30. Launch with `LANG=de_DE.UTF-8`, change `app_language` to `ja`, restart: Japanese labels, restart note shown before restart.
31. Notification from a background update in `ru` shows the plural form for the count.
32. Font check: `th`, `km`, `my`, `bn` render (no tofu) on a system with fonts; record which fonts are needed.

## 6. Fixtures needed

- `xtask/tests/fixtures/base/strings.xml` and `plurals.xml`: 40 representative entries copied from the Aniyomi base (including escapes, `translatable="false"`, one plural with `%2$d`, one `%02d`, one `%.2f`).
- `xtask/tests/fixtures/locales/ru/strings.xml` and `plurals.xml`: one plural with `few` and `many`, one string with a reordered placeholder, one missing key, one stale key.
- `xtask/tests/fixtures/locales/ur/strings.xml`: the empty `<resources>` file shape (copied from the Aniyomi `ur` file).
- `src/i18n/tests/clock.rs` fixture: a fixed `now` for relative-time tests (no system clock).
- A GTK-free test for `locale.rs` and `catalog.rs` (no display needed). GTK-dependent checks (`set_default_direction`) run in a headless `xvfb`-like CI job or are manual.

## 7. Performance checks

- Catalog build for the chain of 3 bundles: under 20 ms on the target machine (measure with `Instant` in a test; not a hard CI gate).
- Full catalog memory: about 66 bundles at most, but only the chain (2–3) is built at startup. Others are built only if a language is switched (not supported at runtime). Expected memory: under 5 MB for the chain.
- Binary size increase from embedded `.ftl` files: measure after the first real import; record in the task acceptance.
