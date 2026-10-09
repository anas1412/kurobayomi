# Topic: enabled sources, pins, languages, incognito and data-saver exclusion

Files in scope: `GetEnabledMangaSources.kt`, `GetEnabledAnimeSources.kt`, `GetLanguagesWithMangaSources.kt`, `GetLanguagesWithAnimeSources.kt`, `ToggleMangaSource.kt`, `ToggleAnimeSource.kt`, `ToggleMangaSourcePin.kt`, `ToggleAnimeSourcePin.kt`, `ToggleLanguage.kt`, `GetMangaIncognitoState.kt`, `GetAnimeIncognitoState.kt`, `ToggleMangaIncognito.kt`, `ToggleAnimeIncognito.kt`, `ToggleExcludeFromMangaDataSaver.kt`, `SourcePreferences.kt`, `LocaleHelper` (used, not in scope). All under `app/src/main/java/eu/kanade/domain/source/` unless stated. Line-level detail: `p1-behavior.md` sections 4 to 8.

## 1. The five persistent sets

All are string sets in the settings store. Element format is decimal source id (`"4294967296"`) or package name (incognito). Keys in `p1-data-and-config.md` section 1.

| Set | Key | Meaning |
|---|---|---|
| Disabled (manga) | `hidden_catalogues` | source hidden from the browse list and from language groups' "enabled" marking |
| Disabled (anime) | `hidden_anime_catalogues` | same for anime |
| Pinned (manga) | `pinned_catalogues` | pinned section |
| Pinned (anime) | `pinned_anime_catalogues` | same for anime |
| Data-saver excluded (manga) | `data_saver_excluded` | image pipeline bypasses data saver for these sources |
| Incognito packages (manga) | `incognito_manga_extensions` | extension packages whose sources are incognito |
| Incognito packages (anime) | `incognito_anime_extensions` | same for anime |
| Languages | `source_languages` | enabled language codes (not per source) |

A source is "disabled" when its id is in the disabled set. The name "enabled" is the absence of that membership plus the language rule.

## 2. Enabled-source list (what the browse list shows)

Given all live and stub rows `R`, settings `S`, last-used `L`:

1. Keep `r` if `r.lang ∈ S.languages` OR `r.id == 0`.
2. Drop `r` if `r.id ∈ S.disabled`.
3. Sort by name, case-insensitive, stable.
4. Pin flag: `r.id ∈ S.pinned` → `PINNED (0b11)`, else `UNPINNED (0b00)`.
5. If `r.id == L`, emit a second row `r'` with `is_used_last = true` and pin with the ACTUAL bit cleared (Proposal) or XOR-toggled (Kotlin).
6. Manga only: `data_saver_excluded = r.id ∈ S.data_saver_excluded`.
7. Publish only if the list differs from the previous publication.

Important consequences (observed, and kept):
- A disabled source that is also last-used appears nowhere.
- A pinned source whose language is disabled disappears but keeps its pin in storage.
- The local source (id 0, lang `"other"`) is shown even when `"other"` is not an enabled language (rule 1).

Test vectors: `p1-edge-cases-and-tests.md` section 2.

## 3. Language groups (Extension and Source filter screens)

Input: online (HTTP) sources only. Output: groups keyed by language.

Order inside a group: disabled sources last, then name case-insensitive.
Order of groups: enabled languages first; then `LocaleHelper.comparator` (`"all"` first, then localized display name). Ties broken by raw code (Proposal).

Example. Sources: `A (en)` enabled, `B (en)` disabled, `C (ja)` enabled, `D (fr)`; settings: languages `{all, en, ja}`, disabled `{B}`.
- `en` group: `[A, B]` (A first because B is disabled).
- `ja` group: `[C]`.
- `fr` group: `[D]`.
- Group order: `en`, `ja` (enabled, comparator: en before ja by display name "English" < "Japanese"), then `fr`.

Note: the `enabled` flag of a group is a language-level flag, not "any source enabled". A group whose sources are all disabled is still listed under its language.

## 4. Toggle interactors

### 4.1 Source enable and disable (`ToggleMangaSource.kt`, `ToggleAnimeSource.kt`)

Three forms:
- `await(source, enable = <default>)` → `await(source.id, enable)`.
- `await(sourceId, enable = <default>)`: add to disabled when `enable == false`, remove when `enable == true`.
- `await(sourceIds: List<Long>, enable: Boolean)`: bulk; the same rule applied to each id, one write.

Default argument: `isEnabled(sourceId)` returns whether the id is currently in the disabled set. Passed as `enable`, the default is therefore "enable if currently disabled", which is a flip (see the naming trap in `p1-behavior.md` 6.1).

Callers (OBSERVED):
- Per-source switch on the source list: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt:109-110` `toggleSource(source)` → `toggleSource.await(source)` (flip).
- Extension details "enable/disable all" buttons: `MangaExtensionDetailsScreenModel.kt:135` `toggleSource.await(it, enable)` (bulk over the extension's sources) and anime `AnimeExtensionDetailsScreenModel.kt:136`.
- Single source on the extension details screen: `MangaExtensionDetailsScreenModel.kt:128-130` (`toggleSource.await(sourceId)`, flip).
- Sources filter screens: `MangaSourcesFilterScreenModel.kt:53-54`, `AnimeSourcesFilterScreenModel.kt:53-54` (`toggleSource.await(source)`).

Rust: `set_disabled(set, ids, disable)` (bulk) and `flip_disabled(set, id) -> bool` (returns the new state). The UI calls `flip_disabled` for single toggles and `set_disabled` for bulk. Each persists in one write.

### 4.2 Pins (`ToggleMangaSourcePin.kt`, `ToggleAnimeSourcePin.kt`)

`isPinned = id ∈ pinned`, then add when not pinned, remove when pinned. Called from `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt:114` and the anime twin `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt:96`.

Rust: `toggle_pin(set, id) -> bool` returning the new pinned state; read and write in one lock (Kotlin reads outside `getAndSet`, which allows a lost update under two quick toggles).

Menu label (OBSERVED, `MangaSourcesScreen.kt:184`): "Unpin" when `Pin.Pinned` is set on the row, else "Pin". The last-used duplicate shows the same label as its source (the Pinned bit is carried over).

### 4.3 Languages (`ToggleLanguage.kt`)

`isEnabled = lang ∈ source_languages`; add when not enabled, remove when enabled. Called from the Extension filter screens (`MangaExtensionFilterScreenModel.kt:58`, `AnimeExtensionFilterScreenModel.kt:58`) and the Source filter screens (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesFilterScreenModel.kt:57-58`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreenModel.kt:57-58`).

Language toggles never touch per-source disabled sets (behavior area 09 agrees). Unticking a language hides its sources but keeps their disabled and pinned state.

### 4.4 Data-saver exclusion (`ToggleExcludeFromMangaDataSaver.kt`)

Pure flip of `data_saver_excluded`. Manga only. Caller: the data-saver source settings (UI area). Effect: rows get `data_saver_excluded = true` in the enabled list.

## 5. Incognito

### 5.1 Rule

`is_incognito(source) = global_incognito OR package(source) ∈ incognito_set`.

`package(source)` is the package of the INSTALLED extension that owns the id; stubs and local source return no package, so they are incognito only through the global flag.

`global_incognito` is `__APP_STATE_incognito_mode` (default `false`, `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21`).

### 5.2 Toggles

`ToggleMangaIncognito.await(extensions: String, enable: Boolean)` and anime twin: add or remove a PACKAGE NAME. The parameter named `extensions` is a package name; the Rust API names it `pkg_name`.

Caller: `MangaExtensionDetailsScreenModel.kt:138-141` `toggleIncognito(enable)` → `toggleIncognito.await(packageName, enable)`. This is the switch on the extension details screen.

### 5.3 Subscriptions

`subscribe(sourceId)` = `combine(global, set, package-flow(sourceId))`. Rust: recompute on any change of the settings `watch` or of the extension snapshot (`ExtensionCatalog::installed`). No separate stream type needed.

## 6. Persistence and atomicity

| Operation | Kotlin | Rust |
|---|---|---|
| Read-modify-write of one set | `getAndSet { ... }` (in-memory copy, store write) | one `prefs` method with a mutex held over read-modify-write |
| Two sets changed together (none in this area) | n/a | n/a |
| Migration sort mode and direction | two writes (`SetMigrateSorting.kt:10-11`) | one write transaction over the settings file, or two writes in order (mode, then direction); a crash between them leaves mode new and direction old, which is acceptable |

Serialization format: decimal strings for ids (backup compatibility, `p1-data-and-config.md` section 7).

## 7. Default values and first run

- `source_languages` is unset on a fresh install: default `{all, en, <system language>}` (`LocaleHelper.getDefaultEnabledLanguages`, `LocaleHelper.kt:72-74`).
- Disabled, pinned, incognito, data-saver sets: empty.
- `last_*_catalogue_source`: `-1`.
- The extension manager sets `subLanguagesEnabledOnFirstRun` (`MangaExtensionManager.kt:20`) from `preferences.enabledLanguages().isSet()`; this is an extension-area rule (areas 18-20), recorded here because it depends on the language key.

## 8. Edge cases

1. Language list empty (all unticked): enabled list shows only the local source; the language filter screen still lists every language.
2. A pinned source's language is disabled: pin kept, row hidden (see section 2).
3. Two quick pin toggles on the same source: with a non-atomic read the second toggle can re-pin; with the Rust lock the result is the parity of the number of toggles.
4. A bulk disable of an extension with 0 sources: no-op (empty id list).
5. Uninstalling an extension while its package is in the incognito set: the set keeps the package; if the extension is reinstalled later, incognito applies again (Kotlin behaviour, kept).
6. Language code case: codes are compared exactly (`"en"` ≠ `"EN"`). Extensions publish lowercase codes; the Rust side must not normalise, to match stored sets.
7. A disabled source id that is not installed: stays in the set; the row is simply not in the list (no cleanup).

## 9. Rust surface

```rust
pub struct SourceSettings { /* section 5.1 of p1-rust-design.md */ }
pub fn enabled_sources(all: &[SourceEntry], s: &SourceSettings) -> Vec<SourceEntry>;
pub fn group_by_language(online: &[SourceEntry], s: &SourceSettings, cmp: &dyn Fn(&str,&str)->Ordering) -> Vec<LanguageGroup>;
pub fn toggle_pin(set: &mut BTreeSet<i64>, id: i64) -> bool;
pub fn flip_disabled(set: &mut BTreeSet<i64>, id: i64) -> bool;
pub fn set_disabled(set: &mut BTreeSet<i64>, ids: &[i64], disable: bool);
pub fn toggle_language(set: &mut BTreeSet<String>, lang: &str) -> bool;
pub fn toggle_data_saver_excluded(set: &mut BTreeSet<i64>, id: i64) -> bool;
pub fn set_incognito_pkg(set: &mut BTreeSet<String>, pkg: &str, enable: bool);
pub fn is_incognito(global: bool, pkg: Option<&str>, set: &BTreeSet<String>) -> bool;
```

Each returns the new membership so the UI can update without re-reading. All are pure over the sets; persistence is one `prefs` call per user action.

## 10. What the mockup needs (see `p1-rust-design.md` section 12)

The Browse tab, the extension filter screen, and the source filter screen in `/home/blackbox/kurobayomi/src/entry.rs` and `src/screens.rs` are placeholders ("Source A/B/C"). The toggles above are the only state changes the screens need. Wire them to `flip_disabled` and `toggle_pin`, and redraw from `enabled_sources` output on the generation `watch`.
