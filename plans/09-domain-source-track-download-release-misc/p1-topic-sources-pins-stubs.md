# Topic: sources, pins, stubs, enabled-source list, remote browsing

Scope files: `domain/src/main/java/tachiyomi/domain/source/manga/**`, `domain/src/main/java/tachiyomi/domain/source/anime/**` (24 files). Read for exactness: `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt`, `.../anime/interactor/GetEnabledAnimeSources.kt`, `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt`, `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt`.

## 1. Concepts

| Term | Meaning |
|---|---|
| Live source | an object loaded from an installed extension (or the local source, id 0) that can browse and fetch |
| Domain `Source` | an immutable view of a live or stub source used by lists (id, lang, name, supportsLatest, isStub, pin, isUsedLast) |
| Stub source | placeholder for a source the user once used whose extension is gone; carries only (id, lang, name); every fetch fails with "not installed" |
| Enabled source | live source whose language is enabled (or is the local source) and whose id is not in the disabled set |
| Pinned source | enabled source whose id is in the pinned set; shown in the "Pinned" section |
| Last used source | the single source whose id equals `last_catalogue_source`; shown ADDITIONALLY in the "Last used" section |

Preference sets hold DECIMAL id strings. `last_*_catalogue_source` defaults to `-1` (no source).

## 2. Algorithm `enabled_sources` (test vectors)

Input sources (id, lang, name): (1, en, "Zeta"), (2, en, "alpha"), (3, ja, "Beta"), (0, "", "Local source"), (4, en, "Hidden"), (5, fr, "Omega").
Settings: languages `{en, ja}`, disabled `{4}`, pinned `{3}`, last used `2`.

Step 1 (language/local filter): keep 1, 2, 3, 0 (local: lang `""` not in set but id == 0), 4; drop 5 (fr). Step 2 (disabled): drop 4. Step 3 (sort case-insensitive by name): "alpha"(2), "Beta"(3), "Local source"(0), "Zeta"(1). Step 4 (pin): 3 -> `PinFlags(0b11)`, others `0b00`. Step 5 (last used = 2): after alpha(2) insert a copy with `is_used_last = true`, `pin = without_actual(0b00)` = `0b00` (Kotlin gives `0b10`).

Output order: `[2 alpha, 2 alpha(last used), 3 Beta(pinned), 0 Local source, 1 Zeta]`. Section grouping: `last_used` = `[2 alpha(last used)]`; `pinned` = `[3 Beta]` (Actual set); `en` = `[2 alpha, 1 Zeta]`; `ja` = none (Beta is in pinned); `""` (other) = `[0 Local source]`. Section order: `last_used`, `pinned`, `en`, `""` last. Keys: `2-lastused` and `2` distinct.

Edge cases:
- The same source can legitimately appear twice; consumers must key by `Source::key()`.
- A pinned source that is also last used: entries `(id, Pinned|Actual)` and `(id lastused, Pinned)`: the pinned section shows the first only, last-used section shows the second; the menu of BOTH says "Unpin" (`Pinned` bit set on both).
- A source that is pinned but whose language is disabled disappears from the list (filter happens before pinning) while staying in `pinned_catalogues`; re-enabling the language brings it back pinned.
- A disabled source that is last used does not appear in either place.
- Empty `source_languages` (the user unticked all) leaves only the local source.
- Duplicate source ids from two extensions cannot occur: extension loading rejects the second (area:18/19/20).

## 3. Toggle operations (exact semantics)

```rust
pub fn toggle_pin(set: &mut BTreeSet<i64>, id: i64)           // present -> remove, absent -> insert
pub fn set_enabled(set: &mut BTreeSet<i64>, ids: &[i64], enable: bool) // enable -> remove from disabled, disable -> insert
pub fn toggle_incognito(set: &mut BTreeSet<String>, package: &str, enable: bool)
```
`ToggleMangaSource.await(source)` with no explicit flag flips: enabled -> disabled, disabled -> enabled (the Kotlin helper named `isEnabled` returns "is currently disabled", see behavior 1.6). The bulk form `await(ids, enable)` is used by the Extension details screen buttons "Enable all" / "Disable all" (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeExtensionDetailsScreen.kt:42-43`, manga twin); the one-id form by the per-source switch on the same screen (`AnimeExtensionDetailsScreenModel.kt:129-130`) and by the Sources list menu entry "Disable" (`MangaSourcesScreenModel.toggleSource`). Enabling or disabling a LANGUAGE (`source_languages`) never touches the per-source disabled set (area:40/41). Write each change atomically through the settings store (area:02) so two quick toggles do not lose one (`getAndSet` in Kotlin is read-modify-write on the preference).

## 4. Stub sources: lifecycle with test vectors

| Event | Effect |
|---|---|
| extension installed with sources A(id 10, "Foo", en) | live map gains A; `registerStubSource(Stub(10,en,Foo))`: DB row absent -> upsert; no rename |
| extension updated and A renamed to "Foo2" | row exists with name "Foo" -> upsert; `rename_source(old=Foo, new=Foo2)` renames `downloads/Foo/` to `downloads/Foo2/` |
| extension uninstalled | live map loses A; stub row stays; `get(10)` = None; `get_or_stub(10)` = stub from DB; library entries of A render as "Foo2 (EN)" and fetch fails with `NotInstalled` |
| library entry references source 999 never seen | `get_or_stub(999)`: no DB row, no extension data -> `StubSource(999, "", "")`; `Display` prints `999` |
| two quick reinstalls | idempotent: upsert with identical data (with the equality fix no write happens) |

The download folder rename is owned by area:23/24/25 (`renameSource`); this area only supplies the (old, new) pair.

## 5. Browse paging selection

`BrowseQuery::{Popular, Latest, Search}` -> `Pager` factory (area:06). `Latest` is only offered when `Source::supports_latest`; the UI hides the "Latest" button otherwise. A search with an empty string is NOT `Popular`: the browse screen builds `Popular` when the user presses the Popular chip and `Search` when the user submits text (empty text clears the search and returns to the previous listing mode).

## 6. Non-library counts

Manga: `getSourceIdsWithNonLibraryManga` -> rows `(source, manga_count)`; screen sorts by source name. Anime: see behavior 1.9 for the season tree. Test vector: sources A(10): anime 1 (fav 0, Seasons) with children 2 (fav 1), 3 (fav 0, Episodes), 4 (fav 0, Seasons) with child 5 (fav 0); anime 6 (fav 0, Episodes, parent NULL). Result for A: `ids = [1, 3, 4, 5, 6]`, `orphaned = [2]`; delete issues `DELETE ... WHERE favorite = 0 AND _id IN (1,3,4,5,6)` and `UPDATE animes SET parent_id = NULL WHERE _id IN (2)`; final: anime 2 remains with no parent.

## 7. Tasks

T-09a-001, T-09a-002, T-09a-003, T-09a-004, T-09a-005, T-09a-006, T-09a-007, T-09a-008, T-09a-009, T-09a-010 in `p1-tasks.md`.
