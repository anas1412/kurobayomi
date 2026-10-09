# Deep dive: anime sources list, pins, languages, ordering

Covers the Sources tab (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt`), the data pipeline (`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt`), the visibility page data (`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt`) and the ordering helper (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt`). Behavior summary: `p2-behavior.md` sections 3, 4, 9.1. Model code: `p2-rust-design.md` section 3.1.

## 1. Inputs and their meaning

| Input | Source | Kind | Effect |
|---|---|---|---|
| Installed sources | `AnimeSourceRepository.getAnimeSources()` | live list | rows |
| `source_languages` | `SourcePreferences.enabledLanguages()` | set of strings | row filter (`lang in set`) |
| `hidden_anime_catalogues` | `SourcePreferences.disabledAnimeSources()` | set of decimal ids | row filter (not in set) |
| `pinned_anime_catalogues` | `SourcePreferences.pinnedAnimeSources()` | set of decimal ids | pin flag, pinned section |
| `__APP_STATE_last_anime_catalogue_source` | `SourcePreferences.lastUsedAnimeSource()` | long, default -1 | adds one extra row (last-used copy) |
| Local source | `LocalAnimeSource.ID = 0`, `lang = "other"` | always installed | passes the language filter regardless (`GetEnabledAnimeSources.kt:27`) |

## 2. Pipeline in six steps (exact)

1. Filter: keep `s` where `s.lang ∈ enabled_langs || s.id == 0`.
2. Remove: drop `s` where `s.id ∈ disabled`.
3. Sort by `s.name.to_lowercase()` ascending. Stable. No language tiebreak.
4. For each remaining `s`: emit `(s, pinned = s.id ∈ pinned)`. If `s.id == last_used`, also emit `(s, pinned, is_last_used_copy = true)` directly after it.
5. Group by section: `LastUsed` if copy, else `Pinned` if pinned, else `Language(s.lang)`.
6. Order sections: `LastUsed`, `Pinned`, then `Language(l)` for non-empty `l` in the language order (section 4), then `Language("")`.

Worked example (inputs chosen so every rule fires):

| Source | id | lang | name | pinned | last used |
|---|---|---|---|---|---|
| Local anime source | 0 | other | Local anime source | no | no |
| Alpha | 11 | en | Alpha | no | no |
| beta | 12 | en | beta | yes | yes |
| Gamma | 13 | all | Gamma | no | no |
| Delta | 14 | ar | Delta | no | no |
| Epsilon | 15 | (empty) | Epsilon | no | no |
| Zeta | 16 | en | Zeta | no | no |

`enabled_langs = {all, en, ar}`, `disabled = {}`, `last_used = 12`.

Step 1 keeps: Local (0, lang other, passes by id rule), Alpha, beta, Gamma, Delta, Zeta. Epsilon (lang empty) is dropped because `"" ∉ enabled_langs`.

Step 3 order by lowercase name: Alpha, beta, Delta, Gamma, Local anime source, Zeta.

Step 4 adds the last-used copy of beta after beta: Alpha, beta, beta(copy), Delta, Gamma, Local anime source, Zeta.

Step 5 sections:
- `LastUsed`: beta (copy)
- `Pinned`: beta
- `Language(en)`: Alpha, Zeta
- `Language(all)`: Gamma
- `Language(ar)`: Delta
- `Language(other)`: Local anime source

Step 6 order: LastUsed, Pinned, then languages, then the empty language. Within languages this topic uses decision D-1 (section 4): `all` first, then the other languages by display name.

Display names for the example: `all` → "Multi" (Kotlin header text), `ar` → "Arabic", `en` → "English", `other` → "Other".

Final order with D-1: LastUsed (beta), Pinned (beta), Multi / `all` (Gamma), Arabic / `ar` (Delta), English / `en` (Alpha, Zeta), Other / `other` (Local anime source).

Kotlin order (raw codes, `AnimeSourcesScreenModel.kt:55-66`): LastUsed, Pinned, `all`, `ar`, `en`, `other`. For this example the two orders are identical. They differ when code order and display-name order disagree. Example: codes `ja` and `zh` give `ja` before `zh`, but the display names "Japanese" and "Chinese (Simplified)" give Chinese first. Under D-1 the Chinese section comes first; under Kotlin the Japanese section comes first.

## 3. Pin representation

Kotlin packs pin state into bits (`Pin.kt`): `Pinned = 0b01`, `Actual = 0b10`, `Pins.pinned = 0b11`. Observable semantics are only two booleans: "user pinned" and "belongs to the pinned section". `Actual` is set exactly when `Pinned` is set on the primary copy. The last-used copy's bits are changed by XOR, which is harmless because `isUsedLast` is checked first in grouping (`AnimeSourcesScreenModel.kt:67-73`).

Rust: store `pinned: bool` and `is_last_used_copy: bool`. Do not replicate the XOR bits. Test T-TEST-LAST-USED-DUP covers the observable result.

## 4. Language order (D-1)

Kotlin uses two different rules:
- Sources list (`AnimeSourcesScreenModel.kt:57-65`): raw code order, with "last used", "pinned" and "" at the ends.
- Visibility page (`LocaleHelper.comparator`, `LocaleHelper.kt:17-25`): "all" first, then display name, case sensitive.

D-1 (decision): both screens use this comparator for language sections:

```rust
pub fn language_rank_cmp(a: &str, b: &str, names: &dyn LocaleText) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a.is_empty(), b.is_empty()) {
        (true, false) => return Greater,      // "" always last (sources list rule 5/6)
        (false, true) => return Less,
        (true, true) => return Equal,
        _ => {}
    }
    match (a == "all", b == "all") {
        (true, false) => return Less,         // "all" first
        (false, true) => return Greater,
        _ => {}
    }
    names.language_display_name(a).cmp(&names.language_display_name(b))
}
```

Notes:
- The "" check is applied first on the sources list, which places the empty language after "all" and every other language, matching Kotlin rule 5/6 for that screen.
- Visibility page keeps its own enabled-first split on top of this comparator (section 5).
- `language_display_name` uses `LocaleText` (`p2-rust-design.md` 3.6). For `""` it returns the system language name (Kotlin `LocaleHelper.kt:61`).

## 5. Visibility page order (fully specified)

Kotlin `GetLanguagesWithAnimeSources` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt:22-31`):
1. Sources: online only (`AnimeHttpSource`), sorted by `(id ∈ disabled, name lowercase)`.
2. Group by lang.
3. Language sections sorted by `(lang ∉ enabled_langs, language_rank)` where `language_rank` is `LocaleHelper.comparator`.

Rust:

```rust
pub fn visibility_language_cmp(a: &str, b: &str, enabled: &HashSet<String>, names: &dyn LocaleText) -> std::cmp::Ordering {
    let ea = !enabled.contains(a);   // false (enabled) sorts first
    let eb = !enabled.contains(b);
    ea.cmp(&eb).then_with(|| {
        // LocaleHelper.comparator semantics (no empty-language special case on this page)
        match (a == "all", b == "all") {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => names.language_display_name(a).cmp(&names.language_display_name(b)),
        }
    })
}
```

Differences from the sources list in Kotlin:
- The visibility page has no empty-language rule. An empty language is ordered by its display name (system language). PROPOSAL: apply the same "" last rule on both screens (decision D-1 covers it). Test: T-TEST-VIS-ORDER.

## 6. Header text table (exact)

| Section | Kotlin text source | Text for `en` system with `ar` default |
|---|---|---|
| LastUsed | `MR.strings.last_used_source` | "Last used" |
| Pinned | `MR.strings.pinned_sources` | "Pinned" |
| `all` | `MR.strings.multi_lang` | "Multi" |
| `other` | `MR.strings.other_source` | "Other" |
| `""` | `getLocalizedDisplayName("")` = system language | system language name |
| other code | `getLocalizedDisplayName(code)`: `Locale.forLanguageTag(code).getDisplayName(locale)`, first character uppercased | e.g. `ar` → "Arabic" (in English UI), `ja` → "Japanese" |
| `zh-CN` | mapped to `zh-Hans` before display | "Chinese (Simplified)" (JVM name) |
| `zh-TW` | mapped to `zh-Hant` before display | "Chinese (Traditional)" (JVM name) |

Rust: `LocaleText::language_display_name` must produce the same strings for the language of the GTK UI. Use `glib::language_names()` for the UI locale. For the display name of a tag, use the ICU display names if the `icu` crate is added (`p2-rust-design.md` 5), otherwise the code itself as a fallback (documented, E-SRC-04).

## 7. Failure path

Kotlin (`AnimeSourcesScreenModel.kt:43-49`): `catch` then `FailedFetchingSources` event, then the flow ends. Visible result: snackbar `MR.strings.internal_error`, page stays in Loading if no emission ever happened. Kurobayomi: see `p2-behavior.md` 3.8 and `p2-ui-parity.md` 1.3 (inline error row with Retry, E-SRC-02).

## 8. Pin and disable actions

| Action | Kotlin call | Preference effect | Rust |
|---|---|---|---|
| Pin | `ToggleAnimeSourcePin.await(source)` (`ToggleAnimeSourcePin.kt:11-16`) | `pinned` set add or remove the id | `prefs.toggle_pin(id)` (single writer, `watch` update) |
| Disable | `ToggleAnimeSource.await(source)` (`ToggleAnimeSource.kt:11-19`) | `hidden` set add or remove | `prefs.toggle_hidden(id)` |
| Enable from visibility page | same as disable toggle | same | same |
| Enable language | `ToggleLanguage.await(language)` (`app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt:10-15`) | `source_languages` add or remove | `prefs.toggle_language(lang)` |

Toggle semantics are plain set flips. The Kotlin parameter name `enable` on `ToggleAnimeSource` is inverted (it means "currently disabled") but the effect is a flip. Rust names the function for what it does.

## 9. Tests (new)

- T-TEST-SECTIONS: the worked example in section 2 produces exactly the listed section order and members.
- T-TEST-LAST-USED-DUP: beta appears twice (LastUsed and Pinned); pin toggle updates both.
- T-TEST-VIS-ORDER: enabled languages first; `all` first within enabled; disabled languages after; `""` last on the sources list.
- T-TEST-LOCALE-DISPLAY: names for `all`, `other`, `""`, `zh-CN`, `zh-TW` with the stub `LocaleText`.
