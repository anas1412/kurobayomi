# Topic: migration source list, sorting and favourite counts

Files: `app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt` (23 lines), `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt` (58), `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt` (58), and the preference accessors in `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:30-38`.

Purpose: the "Migrate" flow lets a user move library entries from one source to another. The first screen lists the sources that currently hold library entries, with a count, so the user chooses the source to migrate FROM. This topic covers that list and its ordering.

## 1. Stored settings

| Key | Type | Default | Accessor |
|---|---|---|---|
| `pref_migration_sorting` | enum name of `SetMigrateSorting.Mode` | `ALPHABETICAL` | `migrationSortingMode()` (`SourcePreferences.kt:30-33`) |
| `pref_migration_direction` | enum name of `SetMigrateSorting.Direction` | `ASCENDING` | `migrationSortingDirection()` (`SourcePreferences.kt:35-38`) |

Enum storage: the enum `name` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:27-45`); an unknown stored value falls back to the default.

Modes (`SetMigrateSorting.kt:14-17`): `ALPHABETICAL`, `TOTAL`. Directions (`:19-22`): `ASCENDING`, `DESCENDING`.

## 2. Writing the settings

```kotlin
fun await(mode: Mode, direction: Direction) {
    preferences.migrationSortingMode().set(mode)
    preferences.migrationSortingDirection().set(direction)
}
```
(`SetMigrateSorting.kt:9-12`.) Two separate writes, mode first, not atomic.

UI toggles (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceScreenModel.kt:55-75`; anime `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceScreenModel.kt:55-75` is the same):
- `toggleSortingMode()`: `ALPHABETICAL → TOTAL`, `TOTAL → ALPHABETICAL`, keeps the direction, calls `await(newMode, currentDirection)`.
- `toggleSortingDirection()`: `ASCENDING ↔ DESCENDING`, keeps the mode, calls `await(currentMode, newDirection)`.

The screen model also observes both keys (`preferences.migrationSortingDirection().changes()` and `...Mode().changes()`) and copies them into its UI state (`sortingDirection`, `sortingMode`).

## 3. Data source

`repository.getMangaSourcesWithFavoriteCount(): Flow<List<Pair<Source, Long>>>` (manga). Implementation (OBSERVED, `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:41-55`):

```kotlin
combine(
    handler.subscribeToList { mangasQueries.getSourceIdWithFavoriteCount() },
    sourceManager.catalogueSources,
) { sourceIdWithFavoriteCount, _ -> sourceIdWithFavoriteCount }
    .map {
        it.map { (sourceId, count) ->
            val source = sourceManager.getOrStub(sourceId)
            val domainSource = mapSourceToDomainSource(source).copy(
                isStub = source is StubMangaSource,
            )
            domainSource to count
        }
    }
```

Steps:
1. SQL: `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source;` (`data/src/main/sqldelight/data/mangas.sq:103-109`). Output: one row per source id that has at least one library manga.
2. Every time the live source list changes (`sourceManager.catalogueSources` emits) the list is recomputed too.
3. For each row: resolve the source with `getOrStub` (live, else persisted stub, else fallback; `p1-topic-registry-and-stubs.md` section 5). The fallback is a stub, so libraries that reference uninstalled sources still appear.
4. `isStub` is true only for stub instances.
5. Name: `mapSourceToDomainSource` takes `name` and `lang` from the resolved object (a stub yields its stored name, or `""` for a fallback).

The anime twin: `repository.getAnimeSourcesWithFavoriteCount()` over `SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source;` (`data/src/main/sqldelightanime/dataanime/animes.sq:114-120`).

## 4. Interactor (`GetMangaSourcesWithFavoriteCount.subscribe()`, OBSERVED `:18-28`)

```kotlin
combine(
    preferences.migrationSortingDirection().changes(),
    preferences.migrationSortingMode().changes(),
    repository.getMangaSourcesWithFavoriteCount(),
) { direction, mode, list ->
    list
        .filterNot { it.first.id == LocalMangaSource.ID }
        .sortedWith(sortFn(direction, mode))
}
```

- Filter: the local source (id 0) is removed (library entries of the local source cannot be migrated this way).
- Sort: `sortFn(direction, mode)`.
- Anime: identical with `LocalAnimeSource.ID`.

No `distinctUntilChanged`; the list is re-emitted on each input change.

## 5. Comparator (OBSERVED `GetMangaSourcesWithFavoriteCount.kt:30-57`)

```kotlin
val sortFn: (Pair<Source, Long>, Pair<Source, Long>) -> Int = { a, b ->
    when (sorting) {
        ALPHABETICAL -> when {
            a.first.isStub && !b.first.isStub -> -1
            b.first.isStub && !a.first.isStub -> 1
            else -> a.first.name.lowercase().compareToWithCollator(b.first.name.lowercase())
        }
        TOTAL -> when {
            a.first.isStub && !b.first.isStub -> -1
            b.first.isStub && !a.first.isStub -> 1
            else -> a.second.compareTo(b.second)
        }
    }
}
return when (direction) {
    ASCENDING -> Comparator(sortFn)
    DESCENDING -> Collections.reverseOrder(sortFn)
}
```

Rules, in words:
1. Stubs come BEFORE live sources, in ASCENDING. (Stub-first is part of the comparator, so it is reversed too.)
2. ALPHABETICAL: compare the lowercase names with the default collator at PRIMARY strength (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:6-14`). Collator is the default locale's `java.text.Collator` (`Collator.PRIMARY`).
3. TOTAL: compare the favourite counts, ascending (fewest library entries first).
4. DESCENDING reverses the entire comparator: stubs go LAST, alphabetical reversed, counts from highest to lowest.
5. Ties: `sortedWith` is stable, so equal keys keep the repository order (SQL `GROUP BY` order, which in practice is ascending source id but is not guaranteed).

## 6. Test vectors

Input rows, in repository (GROUP BY) order `[a, b, c, d, e]`:
- a: "Zeta" (live), count 3
- b: "alpha" (live), count 10
- c: "Beta" (live), count 3
- d: "Gone" (stub), count 7
- e: "Mid" (live), count 10

Results, derived from the comparator in section 5:

| Mode | Direction | Result order | Why |
|---|---|---|---|
| ALPHABETICAL | ASCENDING | d, b, c, e, a | stub first; then "alpha" < "beta" < "mid" < "zeta" |
| ALPHABETICAL | DESCENDING | a, e, c, b, d | reversed comparator; stub goes last |
| TOTAL | ASCENDING | d, a, c, b, e | stub first; count 3 (a, c in input order), then count 10 (b, e in input order) |
| TOTAL | DESCENDING | b, e, a, c, d | reversed comparator; count 10 first (b, e keep input order because equal keys compare 0), then count 3 (a, c), stub last |

Tie rule: `Collections.reverseOrder` swaps the two arguments of the comparator. Equal keys still compare 0, so ties keep their INPUT order in both directions. Do not reverse the list after sorting; that would give `e, b` and `c, a` and is wrong.

Rust test: assert exactly these four sequences (`p1-edge-cases-and-tests.md` section 3).

## 7. Rust design (summary; full code in `p1-rust-design.md` 5.4)

```rust
pub fn sort_migration_sources(rows: &mut Vec<(SourceEntry, u64)>, mode: MigrationMode,
                              dir: MigrationDirection, collator: &Collator);
```
- `rows.sort_by(base)` for ASCENDING; `rows.sort_by(|a, b| base(b, a))` for DESCENDING. The second form is the comparator reversal, so ties keep the input order exactly as Kotlin.
- Remove id 0 before sorting.
- Counts are `u64`; the SQL count is `COUNT(*)` (non-negative).

Collator: ICU4X collator, primary strength, locale from the environment (`p1-rust-design.md` 6.2). Names are lowercased with `to_lowercase` before comparing (Kotlin `lowercase()`).

## 8. Persistence and events

- No DB writes in this area.
- Error path: if the favourite-count flow fails, the screen model sends `Event.FailedFetchingSourcesWithCount` (`MigrateMangaSourceScreenModel.kt` around line 40, the `catch` before `collectLatest`). Rust: the query error is shown as a toast with the screen's own key (not defined here); the list keeps its last value.
- Loading: `isLoading = true` until the first emission (`MigrateMangaSourceScreenModel.kt:47-49`).

## 9. Migration list items shown (UI contract)

Per row: the source's display name (`visualName`), the favourite count, and a stub marker when `is_stub` (the screen decides how to show it; the Kotlin screens show the stub's stored name or the id). Selecting a row opens the search for that source (owned by the migration screens).

## 10. Edge cases

1. A stub with an id that is also now live (reinstalled): the live object is returned by `getOrStub`, so `isStub = false`, and the row sorts as live.
2. Two sources with identical lowercase names: collator returns 0; stable order decides (repository order).
3. Empty library: the list is empty; the flow still emits `[]`.
4. Sorting mode changed while the list is loading: the next emission uses the new mode (the combine re-runs).
5. A direction value stored as an unknown string: the enum getter returns the default (`ASCENDING`).
