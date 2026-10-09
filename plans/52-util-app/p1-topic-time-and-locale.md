# Topic: relative time, absolute formats, plurals, locale names and collation

Sources
- `app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt` (131 lines)
- `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt` (85 lines)
- `core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt` (collator)
- Callers of relative time: `app/src/main/java/eu/kanade/presentation/components/DateText.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/EpisodeListDialog.kt`
- Callers of absolute timestamps: `app/src/main/java/eu/kanade/presentation/history/anime/components/AnimeHistoryItem.kt`, `.../history/manga/components/MangaHistoryItem.kt`

Behaviour text is in `p1-behavior.md` sections 10 and 11. This file explains the decision model and the Rust structure.

## 1. Decision model for relative text

The upstream code is a sequence of threshold tests. The Rust port separates the DECISION (pure, returns an enum) from the RENDERING (localised text). This makes the decision testable without a locale.

```rust
pub enum RelativeTime {
    Today,                        // LocalDate: 0 days
    Days { n: i64, upcoming: bool },
    Hours { n: i64, upcoming: bool },
    Minutes { n: i64, upcoming: bool },
    Now,                          // 0 minutes
    Absolute,                     // caller formats the date
}
```

Rendering table (keys in `plurals.xml` / `strings.xml`):

| Variant | Key | `one` text (en) | `other` text (en) |
|---|---|---|---|
| `Days{upcoming:false}` | `MR.plurals.relative_time` | `Yesterday` | `%1$d days ago` |
| `Days{upcoming:true}` | `MR.plurals.upcoming_relative_time` | `Tomorrow` | `In %1$d days` |
| `Today` | `MR.strings.relative_time_today` | `Today` | — |
| `Hours{upcoming:false}` | `AYMR.plurals.relative_time_hours` | `1 hour ago` | `%1$d hours ago` |
| `Hours{upcoming:true}` | `AYMR.plurals.upcoming_relative_time_hours` | `In 1 hour` | `In %1$d hours` |
| `Minutes{upcoming:false}` | `AYMR.plurals.relative_time_minutes` | `1 minute ago` | `%1$d minutes ago` |
| `Minutes{upcoming:true}` | `AYMR.plurals.upcoming_relative_time_minutes` | `In 1 minute` | `In %1$d minutes` |
| `Now` | `AYMR.strings.relative_time_now` | `Now` | — |

Plural category is chosen with `icu_plurals::PluralRules::try_new_cardinal(locale)`. Categories without a key (zero, two, few, many) fall back to `other`. The English base file only has `one` and `other`; other languages add their own keys in their resource files (translation pipeline out of scope).

## 2. The LocalDate decision (`LocalDate.toRelativeString`)

Let `d = DAYS.between(date, today)` (signed; today minus date).

```
d < -7           -> Absolute
d < 0            -> Days{ n = |d|, upcoming = true }      (d = -1 -> Tomorrow)
d < 1  (d = 0)   -> Today
d < 7            -> Days{ n = d, upcoming = false }       (d = 1 -> Yesterday)
otherwise        -> Absolute
```

Boundaries: -8 Absolute, -7 `In 7 days`, -1 Tomorrow, 0 Today, 1 Yesterday, 6 `6 days ago`, 7 Absolute.

## 3. The LocalDateTime decision (`LocalDateTime.toRelativeString`)

Used for release times of chapters and episodes. Let:
- `td = DAYS.between(release, now)` (24 h periods, truncated toward zero),
- `dd = DAYS.between(release.date, now.date)` (calendar days),
- `hd = HOURS.between(release, now)`,
- `md = MINUTES.between(release, now)`.

```
td < -7                 -> Absolute
td < 0                  -> Days{ n = |dd|, upcoming = true }
td < 1   (td = 0)       ->
    hd < 0              -> Hours{ n = |hd|, upcoming = true }
    hd < 1  (hd = 0)    ->
        md < 0          -> Minutes{ n = |md|, upcoming = true }
        md == 0         -> Now
        otherwise       -> Minutes{ n = md, upcoming = false }
    otherwise           -> Hours{ n = hd, upcoming = false }
td < 7                  -> Days{ n = dd, upcoming = false }
otherwise               -> Absolute
```

Observed consequences (kept on purpose):
- Release 30 s in the future is `Now`.
- A release 25 h ahead on the next calendar day is `Tomorrow`.
- For `td` in 1..6, the count is the calendar difference `dd`, not `td`.

Test table: `p1-edge-cases-and-tests.md` section 5.8 (TC-26-a) and section 5.7 (TC-25-a).

## 4. Absolute formats

| Use | Upstream | Rust (`icu_datetime`, `Length::Short`) |
|---|---|---|
| Date only (`dateFormat` default) | `DateTimeFormatter.ofLocalizedDate(SHORT)` | `DateFormatter` short date |
| Time only (history) | `DateFormat.getTimeInstance(SHORT)` | `TimeFormatter` short time |
| Date + time (`toDateTimestampString`) | `"$date $time"` with `ofLocalizedTime(SHORT)` | `"{date} {time}"` |

Example output (en-US): `1/5/26`, `3:04 PM`, `1/5/26 3:04 PM`. The exact separator and ordering follow ICU for the locale; the upstream `"$date $time"` template (single space) is kept.

## 5. Zones

- `Long.toLocalDate()` and `Instant.toLocalDate(zone)` use the system zone (`chrono::Local`).
- `Long.convertEpochMillisZone(from, to)`: reinterpret the wall-clock time from `from` in `to`. Used by the tracker dialogs (area 27). Rust: `chrono-tz` for explicit zones. Test vector: `(0, UTC, Asia/Tokyo) = -32400000`.
- Naive arithmetic ignores DST (matches `LocalDateTime` upstream).

## 6. Locale display names

| Function | Input | Output rule |
|---|---|---|
| `getLocalizedDisplayName(lang)` | `null` | `""` |
| | `""` | system default language, self name |
| | `zh-CN` | self name of `zh-Hans` |
| | `zh-TW` | self name of `zh-Hant` |
| | other | self name of the tag, first character uppercased in its own locale |
| `getDisplayName(lang)` | tag | name in the default display locale (English in the UI); `zh-CN`/`zh-TW` normalised |
| `getSimpleLocaleDisplayName()` | — | English name of the system language |
| `getSourceDisplayName(lang)` | sentinels | `last_used` -> `Last used`, `pinned` -> `Pinned`, `other` -> `Other`, `all` -> `Multi`, else localized name |

Rust (ICU4X): `icu_displaynames::LanguageDisplayNames::try_new(locale, options)` with the locale equal to the language itself for self names. Cache results in `LocaleNames` (a `Mutex<HashMap>`). The cache is keyed by language tag only; a runtime locale change needs a restart (EC-72).

## 7. Comparator and collation

`LocaleHelper.comparator`: `all` first; otherwise compare localized display names with plain string comparison (not the collator). Rust: same rule; the both-`all` case returns `Equal` (upstream returns -1; EC-68).

Name sorting of chapters and episodes uses the collator (`compareToWithCollator`, PRIMARY strength). Rust: `icu_collator::Collator` with `Strength::Primary` and the system locale, created once.

## 8. Default enabled languages

`getDefaultEnabledLanguages()` = `{all, en, <system language>}`. Rust: drop an empty system language (EC-70). System language from `glib::language_names()` (first entry, `.UTF-8` and `@modifier` stripped, `_` replaced by `-`), fallback `en`.

## 9. Rust module (Proposal)

- `kuro-core::time::relative` — `relative_date(NaiveDate, NaiveDate) -> RelativeTime`, `relative_datetime(NaiveDateTime, NaiveDateTime) -> RelativeTime`.
- `kuro-core::time::format` — `short_date`, `short_time`, `short_datetime` via `icu_datetime`.
- `kuro-i18n` — `render(RelativeTime, &Locale) -> String` using the key table above.
- `kuro-core::locale` — `LocaleNames`, `default_enabled_languages`, `system_language`.

Tests: TC-25, TC-26, TC-27, TC-28, TC-29.
