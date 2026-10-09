# Topic: clear non-library entries (manga and anime)

Covers T-47a-075. Sources: `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt`, `data/src/main/sqldelight/data/mangas.sq`, `data/src/main/sqldelight/data/history.sq`, `data/src/main/sqldelightanime/dataanime/animes.sq`, `data/src/main/sqldelightanime/dataanime/animehistory.sq`, `data/src/main/sqldelightanime/view/animedeletableView.sq`, `domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetAnimeSourcesWithNonLibraryAnime.kt`, `domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetMangaSourcesWithNonLibraryManga.kt`, `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt`.

## 1. What "non-library" means

A row is deletable when `favorite = 0`. Manga: any row with `favorite = 0`. Anime: a row with `favorite = 0` **and** `parent_id IS NULL` is a deletable *root* (view `animedeletableView`). Non-library children of a root are found through `parent_id`. Library rows (`favorite = 1`) are never deleted by this screen.

## 2. Data flow

```
manga:  subscribe() -> SELECT source, COUNT(*) FROM mangas WHERE favorite=0 GROUP BY source
        -> per source: sourceManager.getOrStub(source) -> MangaSourceWithCount(source, count)
        -> sortBy(name)

anime:  subscribe() -> SELECT * FROM animedeletableView   (roots)
        -> groupBy(sourceId)
        -> per source:  ids = [], orphaned = []
             for each root r in group:
                 ids += r.animeId
                 if r.fetchType == Seasons:
                     (cIds, cOrph) = deletableChildren(r.animeId)
                     ids += cIds ; orphaned += cOrph
             AnimeSourceWithIds(source, ids, orphaned)
        -> sortBy(name)
```

## 3. Recursive child algorithm (anime)

```
deletableChildren(parentId):
    ids = [], orphaned = []
    for c in SELECT * FROM animes WHERE parent_id = parentId:       # getChildrenByParentId
        if c.favorite:                       # library child: must be detached, not deleted
            orphaned += c.id
        else:
            ids += c.id
            if c.fetchType == Seasons:       # grandchildren
                (ci, co) = deletableChildren(c.id)
                ids += ci ; orphaned += co
    return (ids, orphaned)
```

Note: a favourite child is orphaned but its own children are not visited (the Kotlin only recurses into non-favourite children, `ClearAnimeDatabaseScreen.kt:267-284`). A favourite child's non-favourite grandchildren therefore remain (they are still linked to the favourite, which is kept). Rust keeps this rule (parity).

## 4. Delete algorithm

Manga (`ClearDatabaseScreen.kt:210-214`):
```
withNonCancellableContext:
    DELETE FROM mangas WHERE favorite = 0 AND source IN (:selectedSourceIds)
    DELETE FROM history WHERE last_read = 0
```

Anime (`ClearAnimeDatabaseScreen.kt:286-297`):
```
withNonCancellableContext:
    selected   = items.filter { it.id in selection }             # selection = source ids
    animeIds   = selected.flatMap { it.ids }
    orphaned   = selected.flatMap { it.orphaned }.filterNot { it in animeIds }
    DELETE FROM animes WHERE favorite = 0 AND _id IN (:animeIds)
    UPDATE animes SET parent_id = NULL WHERE _id IN (:orphaned)
    DELETE FROM animehistory WHERE last_seen = 0
```

Order matters: deletion first, then detaching orphans. An orphan whose id also appears in `animeIds` is removed from `orphaned` by `filterNot` (it cannot be both).

## 5. Rust implementation notes (Proposal)

- Run both statements in one SQLite transaction (`BEGIN IMMEDIATE`), so a failure rolls back both the delete and the detach.
- The `IN (:ids)` lists are bound parameters; for more than 900 ids, chunk the statement (SQLite's parameter limit). A chunk size of 500 is safe for all supported SQLite builds.
- Empty `IN ()` must not be sent: skip the statement when the id list is empty.
- Subscribe to the view with a change notification from the repository layer (area 05/06), not a timer.

## 6. Test vectors

| Input tree (anime) | Result |
|---|---|
| P(root, Seasons) → C1(non-lib), C2(lib) | ids=[P, C1]; orphaned=[C2] |
| P → C1(non-lib, Seasons) → G(non-lib) | ids=[P, C1, G] |
| P → C1(lib, Seasons) → G(non-lib) | ids=[P]; orphaned=[C1]; G **not** visited, stays in DB with parent C1 |
| P(root, Single) (no children) | ids=[P] |

## 7. UI contract summary

- Select all / invert / toggle act on source ids only.
- Confirm dialog text: "Are you sure? Completed episodes and chapters and progress of non-library entries will be lost".
- After delete: toast "Entries deleted"; selection cleared; list refreshed by the subscription.
- Empty list: "Nothing to clear".
