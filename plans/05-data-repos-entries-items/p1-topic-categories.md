# Area 05 — topic: categories (repositories plus the interactor rules that define their contract)

Paths repo-relative to `/home/blackbox/aniyomi-src`. Repositories in scope: `data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt` and `data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt` (identical except names; confirmed with `diff`). The interactors in `domain/src/main/java/tachiyomi/domain/category/{manga,anime}/interactor/` belong to area:08-domain-library-category-history-updates; they are summarised here because the repository API only makes sense with them, and T-05a-025 implements the data-side rules.

## 1. Data model recap

Table `categories(_id PK, name TEXT NOT NULL, sort INTEGER NOT NULL, flags INTEGER NOT NULL, hidden INTEGER NOT NULL DEFAULT 0)` exists once in `tachiyomi.db` (manga) and once in `tachiyomi.animedb` (anime). Seed row `(0, "", -1, 0)` is the system "Default" category (domain `Category.UNCATEGORIZED_ID = 0L`, `isSystemCategory`). It cannot be deleted (trigger). Link tables `mangas_categories(manga_id, category_id)` and `animes_categories(anime_id, category_id)` cascade on both sides. No unique constraint on `name`, on `sort`, or on `(entry_id, category_id)`.

## 2. Repository method contract

| Method (manga name / anime name) | SQL | Notes |
|---|---|---|
| `getMangaCategory(id)` / `getAnimeCategory` | `getCategory`: `SELECT * FROM categories WHERE _id = :id LIMIT 1`, `awaitOneOrNull` | returns null when missing |
| `getAllMangaCategories()` | `getCategories ... ORDER BY sort` | INCLUDES the system category (sort -1 until renumbered); callers filter `Category::isSystemCategory` |
| `getAllVisibleMangaCategories()` | `getVisibleCategories ... WHERE hidden = 0 ORDER BY sort` | includes system category (hidden is 0 for it unless a bug sets it) |
| `getAllMangaCategoriesAsFlow()`, `getAllVisibleMangaCategoriesAsFlow()` | same queries, subscription on table `categories` | |
| `getCategoriesByMangaId(id)`, `getVisibleCategoriesByMangaId(id)` | join `mangas_categories` | no ORDER BY; excludes the system category unless a link row to category 0 exists (links to 0 are not created by the UI: an entry with no links is "in Default") |
| `getCategoriesByMangaIdAsFlow(id)`, `getVisibleCategoriesByMangaIdAsFlow(id)` | same, subscription on `categories, mangas_categories` | |
| `insertMangaCategory(category)` | `INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags)` | `category.id` and `category.hidden` are ignored: the id is the new rowid, hidden takes the DDL default 0. Returns Unit (no id returned) |
| `updatePartialMangaCategory(update)` | `categories.update` coalesce; not in an explicit transaction | `hidden = update.hidden?.let { if (it) 1L else 0L }` |
| `updatePartialMangaCategories(updates)` | one transaction, loop of the same update | all-or-nothing |
| `updateAllMangaCategoryFlags(flags: Long?)` | `UPDATE categories SET flags = coalesce(?, flags)` | all rows incl. system; `null` is a no-op |
| `deleteMangaCategory(id)` | `DELETE FROM categories WHERE _id = :categoryId` | cascades link rows; id 0 raises the trigger error; deleting a non-existent id is a silent no-op (0 rows) |

All exceptions propagate to the caller in the category repositories (no try/catch here, unlike the entry repositories); the interactors catch them (`Result.InternalError`).

`mapCategory(id, name, order, flags, hidden)`: `Category(id, name, order, flags, hidden = hidden == 1L)`. A `hidden` value of 2 or any non-1 integer reads as visible.

## 3. Interactor rules that the data layer must support (OBSERVED)

### 3.1 Create (CreateMangaCategoryWithName.kt)

1. `categories = getAllMangaCategories()` (includes system row).
2. `nextOrder = categories.maxOfOrNull { it.order }?.plus(1) ?: 0`. With only the system row (order -1) the first user category gets order 0; after a delete renumbering (section 3.2) the system row has order 0, so the next new category gets max+1.
3. `initialFlags = preferences.mangaSortingMode().get().let { it.type.flag or it.direction.flag }` (preference key `library_sorting_mode` default `ALPHABETICAL,ASCENDING`; anime `animelib_sorting_mode`). A new category thus inherits the current global library sort.
4. Insert `Category(id = 0, name = name, order = nextOrder, flags = initialFlags, hidden = false)` (id ignored). Result `Success` or `InternalError(e)` (exception logged with `logcat(LogPriority.ERROR, e)`). Runs under `withNonCancellableContext` so a closing dialog cannot cancel the write.
5. Name rules are NOT enforced here: the UI refuses empty names and exact duplicates (case-sensitive `categories.contains(name)` over the displayed list, `CategoryDialogs.kt:49-89, 113-153`; message `MR.strings.error_category_exists` = "A category with this name already exists!"; empty shows the "*required" supporting text). Whitespace is not trimmed. The repository would accept both.

### 3.2 Delete (DeleteMangaCategory.kt, DeleteAnimeCategory.kt)

1. `deleteMangaCategory(categoryId)`; on exception return `Result.InternalError(e)` immediately (nothing else runs).
2. `categories = getAllMangaCategories()` AGAIN (includes system row, sorted by `sort`); build `updates = categories.mapIndexed { index, category -> CategoryUpdate(id = category.id, order = index.toLong()) }`. Consequence: the system category (first because its sort is lowest) gets order 0 and the user categories get 1, 2, 3 ... (they were 0,1,2 before). The system row's order stops being -1 after the first delete.
3. If `libraryPreferences.defaultMangaCategory().get() == categoryId.toInt()` then `defaultMangaCategory().delete()` (back to -1 = "always ask").
4. Remove the id (as decimal string) from the string-set preferences. MANGA version (OBSERVED BUG): the list is `[mangaUpdateCategories(), mangaUpdateCategories(), downloadPreferences.removeExcludeCategories(), downloadNewChapterCategories(), downloadNewChapterCategoriesExclude()]`, so `mangaUpdateCategoriesExclude()` is never cleaned and `mangaUpdateCategories()` is cleaned twice. ANIME version (correct): `[animeUpdateCategories(), animeUpdateCategoriesExclude(), removeExcludeAnimeCategories(), downloadNewEpisodeCategories(), downloadNewEpisodeCategoriesExclude()]`. Keys are listed in `p1-data-and-config.md` section 7.
5. `updatePartialMangaCategories(updates)` in one transaction; `Success` or `InternalError`.

Proposal: Rust fixes the manga bug by cleaning `library_update_categories_exclude` (strictly a bug fix; the effect is that a deleted category id no longer lingers in the exclusion set where a later new category could reuse the id because `_id` is a rowid without AUTOINCREMENT and reuses the highest freed id). The fix is task T-05a-025.

### 3.3 Reorder (ReorderMangaCategory.kt)

Guarded by a `Mutex` (one reorder at a time per interactor instance) and `withNonCancellableContext`:

1. `categories = getAllMangaCategories().filterNot(Category::isSystemCategory).toMutableList()`.
2. `currentIndex = categories.indexOfFirst { it.id == category.id }`; -1 returns `Result.Unchanged`.
3. `categories.add(newIndex, categories.removeAt(currentIndex))` (an out-of-range `newIndex` throws `IndexOutOfBoundsException`, caught and returned as `InternalError`).
4. `updates = categories.mapIndexed { i, c -> CategoryUpdate(id = c.id, order = i.toLong()) }`; `updatePartialMangaCategories(updates)` (transaction). The system row is not touched, so after a delete it keeps order 0 while the first user category also gets 0 (tie).

### 3.4 Hide, rename, update

`HideMangaCategory`: `CategoryUpdate(id, hidden = !category.hidden)` (toggle from the caller's possibly stale copy). `RenameMangaCategory`: `CategoryUpdate(id, name = name)`. `UpdateMangaCategory.await(payload)`: runs `updatePartialMangaCategory` and returns `Success` or `Error(e)`. All three non-cancellable.

### 3.5 Flags: reset and set sort

`ResetMangaCategoryFlags.await()`: `updateAllMangaCategoryFlags(sort.type + sort.direction)` where `sort = mangaSortingMode().get()`; `type + direction` is `Flag.plus(Flag)` = `(type.flag and direction.mask.inv()) or (direction.flag and direction.mask)`, which equals `type.flag or direction.flag` because the masks do not overlap.

`SetSortModeForMangaCategory.await(categoryId: Long?, type, direction)`:

1. `category = categoryId?.let { repo.getMangaCategory(it) }`.
2. `flags = (category?.flags ?: 0) + type + direction` using `Long.plus(Flag)` twice: `(flags and type.mask.inv()) or (type.flag and type.mask)`, then the same with the direction mask. Other bits of the stored flags survive.
3. If `type == Random`: `preferences.randomMangaSortSeed().set(Random.nextInt())` (key `library_random_manga_sort_seed`, anime `library_random_anime_sort_seed`, default 0) so the shuffled order changes on each selection.
4. If `category != null && preferences.categorizedDisplaySettings().get()` (key `categorized_display`, default false): update only that category's flags. Otherwise: set the global preference `mangaSortingMode().set(MangaLibrarySort(type, direction))` and `updateAllMangaCategoryFlags(flags)` (all categories get the same flags value computed from the single category, or from 0 when `categoryId` is null).

### 3.6 Flag layout of `categories.flags` (OBSERVED)

| Field | Mask | Values |
|---|---|---|
| Sort type | `0b00111100` (0x3C) | manga: Alphabetical 0b000000, LastRead 0b000100, LastUpdate 0b001000, UnreadCount 0b001100, TotalChapters 0b010000, LatestChapter 0b010100, ChapterFetchDate 0b011000, DateAdded 0b011100, TrackerMean 0b100000, Random 0b111100. anime: same bit patterns for Alphabetical, LastSeen (0b000100), LastUpdate, UnseenCount, TotalEpisodes, LatestEpisode, EpisodeFetchDate, DateAdded, TrackerMean, Random and an additional AiringTime 0b110000 |
| Direction | `0b01000000` (0x40) | Ascending 0b01000000, Descending 0b00000000 |

Decoding: `MangaLibrarySort.valueOf(flags)` = (`Type.valueOf` = first type whose `flag == flags and mask`, default Alphabetical; `Direction.valueOf` similarly default Ascending). A null `flags` (no category) decodes to the default `Alphabetical + Ascending` (note: default direction is Ascending while a raw flag 0 means Descending: raw 0 decodes to Alphabetical + Descending because 0 matches `Descending.flag`; only the `null` category yields Ascending). Serialisation of the preference object: `"<TYPE>,<ASCENDING|DESCENDING>"` with TYPE in {ALPHABETICAL, LAST_READ, LAST_MANGA_UPDATE, UNREAD_COUNT, TOTAL_CHAPTERS, LATEST_CHAPTER, CHAPTER_FETCH_DATE, DATE_ADDED, TRACKER_MEAN, RANDOM}; empty string or any parse error gives the default. These layouts are re-specified for the library screen in area:08-domain-library-category-history-updates and area:37-ui-library; the repository stores them as an opaque `i64`.

## 4. Proposal (Rust)

```rust
pub struct Category { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
impl Category { pub const UNCATEGORIZED_ID: i64 = 0; pub fn is_system(&self) -> bool { self.id == 0 } }

pub struct CategoryUpdate { pub id: i64, pub name: Option<String>, pub order: Option<i64>,
                            pub flags: Option<i64>, pub hidden: Option<bool> }

pub struct CategoryRepo { db: Db, kind: EntryKind /* Manga | Anime: selects database and link table */ }
impl CategoryRepo {
    pub async fn get(&self, id: i64) -> Result<Option<Category>>;
    pub async fn all(&self) -> Result<Vec<Category>>;                 // incl. system, ORDER BY sort, _id
    pub async fn all_visible(&self) -> Result<Vec<Category>>;
    pub fn subscribe_all(&self) -> Subscription<Vec<Category>>;
    pub fn subscribe_all_visible(&self) -> Subscription<Vec<Category>>;
    pub async fn by_entry(&self, entry_id: i64) -> Result<Vec<Category>>;
    pub async fn visible_by_entry(&self, entry_id: i64) -> Result<Vec<Category>>;
    pub fn subscribe_by_entry(&self, entry_id: i64) -> Subscription<Vec<Category>>;
    pub fn subscribe_visible_by_entry(&self, entry_id: i64) -> Subscription<Vec<Category>>;
    pub async fn insert(&self, name: &str, order: i64, flags: i64) -> Result<i64>; // returns new id (improvement)
    pub async fn update_partial(&self, u: CategoryUpdate) -> Result<()>;
    pub async fn update_partials(&self, us: Vec<CategoryUpdate>) -> Result<()>;   // one transaction
    pub async fn update_all_flags(&self, flags: Option<i64>) -> Result<()>;
    pub async fn delete(&self, id: i64) -> Result<()>;
}
```

Deliberate deviations (each covered by a test): (a) `ORDER BY sort, _id` for a deterministic order when two rows share `sort` (the post-delete tie described above); (b) `insert` returns the new id (free with `last_insert_rowid()`); (c) the manga delete-cleanup bug fix (3.2 step 4). Everything else is identical, including that the repository performs no name validation.

The existing mockup `categories()` in `src/screens.rs:404-422` renders three hard-coded names with `kind_tabs` for Anime and Manga; it must bind to `CategoryRepo::subscribe_all` of the matching kind and filter `is_system()`; details in `p1-rust-design.md` section 9.
