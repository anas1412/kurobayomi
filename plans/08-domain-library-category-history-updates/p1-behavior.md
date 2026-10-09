# Area 08 — Domain: library, category, history, updates — behaviour specification

Everything under "OBSERVED" is what the Kotlin does (source paths are repo-relative to `/home/blackbox/aniyomi-src`, line numbers where useful). Everything under "Proposal" is what Rust should do. Companion files: `p1-data-and-config.md` (field tables, bit layouts, preference keys, SQL), `p1-rust-design.md`, `p1-topic-*.md` (deep dives referenced below).

Conventions used throughout:

- "twin" = the manga and anime copy of the same class. Unless a difference is called out, both behave identically with the vocabulary swapped (`chapter`/`episode`, `read`/`seen`, `Manga`/`Anime`). Proposal: one generic implementation `K: EntryKind` (`Manga` | `Anime`).
- Kotlin `withNonCancellableContext { ... }` wraps writes so a cancelled UI scope cannot abort a half-done write (source: `core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt:65`, `withContext(NonCancellable, block)`). Rust equivalent: run the write on a detached `tokio::spawn`ed task (or `spawn_blocking` against the writer connection) and await its `JoinHandle`; dropping the awaiting future must not cancel the write (see `p1-rust-design.md` section 6).
- `Flow<T>` (SQLDelight `subscribeToList`) = re-run the query and emit whenever a table it reads changes; Rust equivalent is the change bus of area:03-db-manga-schema-migrations / area:04-db-anime-schema-migrations (`ChangeBus::subscribe(tables) -> watch/broadcast`), re-querying and emitting the full list (see T-08a-032).
- Two databases: manga data lives in `manga.db`, anime data in `anime.db`; each has its own `categories` table, its own category ids and its own history. Category id 5 in manga is unrelated to category id 5 in anime.

---

## 1. Category

### 1.1 The system ("Default") category

OBSERVED (`data/src/main/sqldelight/data/categories.sq`, `data/src/main/sqldelightanime/dataanime/categories.sq`):

- The table is created with a seed row `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);` so id 0 always exists; `hidden` defaults to 0.
- Trigger `system_category_delete_trigger BEFORE DELETE ON categories` raises `ABORT "System category can't be deleted"` when `old._id <= 0`.
- `Category.isSystemCategory = (id == UNCATEGORIZED_ID)` with `UNCATEGORIZED_ID = 0L` (`domain/src/main/java/tachiyomi/domain/category/model/Category.kt:11-17`).
- The row's `name` is the empty string; UI shows `MR.strings.label_default` ("Default") through `Category.visualName` (`app/src/main/java/eu/kanade/presentation/category/CategoryExtensions.kt`: `isSystemCategory -> label_default, else -> name`). The Category management screen never lists it (`filterNot(Category::isSystemCategory)`).
- Semantics: an entry in the library with no row in `mangas_categories` / `animes_categories` is shown in the Default category (the library view does `coalesce(MC.category_id, 0)`). The Default category is shown in the library only if at least one library entry is in it, unless the library is completely empty (rule in section 2.3).
- The seed row has `flags = 0`. Flag value 0 decodes (section 2.5 / `p1-topic-flag-algebra-and-sort-modes.md`) as sort type Alphabetical, direction **Descending** (bit 0x40 clear). The library therefore sorts the Default category Z to A until the user picks a sort for it or "Reset" runs; this is OBSERVED behaviour, kept (Proposal: seed the Rust schema with the same row, but have the first-run initialiser write `flags = 0b01000000` for id 0 so a fresh install sorts A to Z; this is a deliberate deviation recorded in T-08a-006 and is invisible to backups because the system category is never exported, see section 1.12).

### 1.2 Reads (`GetXCategories`, `GetVisibleXCategories`, repository)

OBSERVED:

| Method | SQL (statement name in the `.sq`) | Order |
|---|---|---|
| `getAllXCategories` / `AsFlow` variants | `getCategories`: `SELECT _id AS id, name, sort AS "order", flags, hidden FROM categories ORDER BY sort` | by `sort` ascending; system category (`sort=-1`) is first |
| `getAllVisibleXCategories` / `AsFlow` variants | `getVisibleCategories`: same with `WHERE hidden = 0` | by `sort` |
| `getXCategory(id)` | `getCategory`: `SELECT * FROM categories WHERE _id = :id LIMIT 1` | n/a (single) |
| `getCategoriesByXId(id)` / `AsFlow` variants | `getCategoriesByMangaId` / `getCategoriesByAnimeId`: `SELECT C._id AS id, C.name, C.sort AS "order", C.flags, C.hidden FROM categories C JOIN mangas_categories MC ON C._id = MC.category_id WHERE MC.manga_id = :mangaId` | **no ORDER BY** |
| `getVisibleCategoriesByXId(id)` | same plus `AND C.hidden = 0` | **no ORDER BY** |
| `mapCategory` | `hidden == 1L` becomes the Kotlin Boolean | n/a |

Interactor surface (4 methods each, both for `GetXCategories` and `GetVisibleXCategories`): `subscribe()`, `subscribe(entryId)`, `await()`, `await(entryId)`. Neither interactor catches errors.

Who reads which:

- Library tab: `GetVisibleXCategories.subscribe()` (hidden categories never appear as tabs, and entries that are only in hidden categories do not appear anywhere in the library).
- Change-category dialogs: the *library tab's* bulk dialog lists the displayed (visible) categories (`state.categories.filter { it.id != 0L }`) and looks up entries' categories with the **visible** by-entry variant `GetVisibleXCategories.await(entryId)`; the history tab and the entry details screen use `GetXCategories` (all, hidden included) followed by `filterNot { it.isSystemCategory }`.
- Category management screen: `GetVisibleXCategories.subscribe()` when `hidden_categories` is true, else `GetXCategories.subscribe()` (both followed by `filterNot(isSystemCategory)`); see `app/src/main/java/eu/kanade/tachiyomi/ui/category/manga/MangaCategoryScreenModel.kt:41-55`. Note the choice is made once at screen-model construction (`.get()` is read once, not observed).
- Storage screen: `categories(hideHiddenCategories)` chooses visible/all by the same preference (`app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt:40`).
- Settings, library update include/exclude dialog, default-category list preference: `GetXCategories.subscribe()` (all categories including the system one; the system category is labelled "Default").

Proposal: add `ORDER BY C.sort, C._id` to the two by-entry queries and `ORDER BY sort, _id` to the two list queries (stable tie-break, see section 1.6 on why ties occur). No observable change otherwise.

### 1.3 Create (`CreateXCategoryWithName.await(name)`)

OBSERVED (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/CreateMangaCategoryWithName.kt:17-37`), all inside `withNonCancellableContext`:

1. `categories = repository.getAllXCategories()` (includes the system category).
2. `nextOrder = categories.maxOfOrNull { it.order }?.plus(1) ?: 0`. With only the system category (`order = -1`) this is `0`; with user categories ordered 0..n-1 it is `n`. After a delete (section 1.6) the system category can have order 0 and the user categories 1..n, then `nextOrder = n+1`.
3. `initialFlags = sort.type.flag or sort.direction.flag` where `sort` is the global sort preference of that kind (`mangaSortingMode()` for manga, `animeSortingMode()` for anime; defaults `Alphabetical, Ascending` = `0b01000000` = 64). A new category therefore copies the *current global sort*, not the default.
4. Insert `Category(id = 0, name, order = nextOrder, flags = initialFlags, hidden = false)` with `INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags)` (id is auto assigned; `hidden` takes the column default 0).
5. Success returns `Result.Success`; any exception is logged (`logcat(ERROR)`) and returned as `Result.InternalError(e)`.
6. The UI maps `InternalError` to the toast/snackbar `MR.strings.internal_error` ("InternalError: Check crash logs for further information").

No trimming, no length limit, no uniqueness check, no check for the empty string here. The dialog (`app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt`) disables the Add button when `name.isEmpty() || categories.contains(name)` where `categories` is the list of existing category names of the shown list (exact, case-sensitive match; it excludes the system category and, when `hidden_categories` is on, hidden categories). Error text: `MR.strings.error_category_exists` ("A category with this name already exists!"); helper text otherwise `MR.strings.information_required_plain` ("*required"). A name consisting only of spaces is accepted.

Proposal: the domain function rejects (returns `CategoryError::InvalidName`) when `name.trim().is_empty()`, stores `name.trim()`, and rejects when any existing category (all, including hidden, excluding system) has the same `name` after trimming (case-sensitive, to stay compatible with Aniyomi backups which key categories by exact name, see `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaCategoriesRestorer.kt`: `dbCategories.associateBy { it.name }`). The UI shows `error_category_exists` for `DuplicateName` and keeps the Add button disabled for `InvalidName`.

### 1.4 Rename, Hide, generic Update

OBSERVED:

- `RenameXCategory.await(categoryId, name)` and `await(category, name)`: `CategoryUpdate(id, name = name)` then `updatePartialXCategory`; `Success` / `InternalError`. The rename dialog (`CategoryRenameDialog`) enables OK only if `valueHasChanged && !nameAlreadyExists`; `valueHasChanged = name != it` is computed in `onValueChange` **before** `name = it`, i.e. it compares the previous text with the new text, so it becomes true on the first edit and stays whatever the last keystroke implies (typing a character then deleting it leaves `true` for the delete step, and the original name matches itself in `categories.contains(name)` so OK stays disabled while the text equals the original name; the practical effect is: OK is enabled once the text differs from every existing name, including after returning to a non-colliding edit).
- `HideXCategory.await(category)`: `CategoryUpdate(id, hidden = !category.hidden)`; `Success` or `InternalError`. UI icon: `Visibility` when hidden, `VisibilityOff` when visible; content description `AYMR.strings.action_hide` ("Hide").
- `UpdateXCategory.await(payload: CategoryUpdate)`: generic partial update; `Success` or `Error(e)` (this one does not log).
- Persistence: `UPDATE categories SET name = coalesce(:name, name), sort = coalesce(:order, sort), flags = coalesce(:flags, flags), hidden = coalesce(:hidden, hidden) WHERE _id = :categoryId;` `hidden` is written as `1L`/`0L`. `updatePartialXCategories(updates)` runs all updates in one transaction (`handler.await(inTransaction = true)`).
- A `CategoryUpdate` with every optional field null is a legal no-op.

Proposal: same semantics, `Option<T>` fields with `COALESCE(?, col)`; rename applies the same trim/duplicate rules as create (excluding the category being renamed).

### 1.5 Reorder (`ReorderXCategory.await(category, newIndex)`)

OBSERVED (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/ReorderMangaCategory.kt:18-45`):

1. The interactor owns a `Mutex` (one per interactor instance, so manga and anime reorder independently). Everything below runs inside `withNonCancellableContext { mutex.withLock { ... } }`.
2. `categories = repository.getAllXCategories().filterNot(Category::isSystemCategory).toMutableList()` (order by `sort`).
3. `currentIndex = categories.indexOfFirst { it.id == category.id }`; if `-1` return `Result.Unchanged` (category vanished or is the system category).
4. Inside a `try`: `categories.add(newIndex, categories.removeAt(currentIndex))`, i.e. `newIndex` is an index into the list *after* removal and is interpreted among user categories only; a `newIndex` outside `0..size-1` after removal (valid range for `add` is `0..size` of the shortened list) throws `IndexOutOfBoundsException`, caught, logged and returned as `InternalError`.
5. `updates = categories.mapIndexed { index, c -> CategoryUpdate(id = c.id, order = index.toLong()) }` and `updatePartialXCategories(updates)` in one transaction. Every user category is rewritten; the system category's `sort` is never touched here.
6. Returns `Success`.

Drag behaviour in the UI (`app/src/main/java/eu/kanade/presentation/category/MangaCategoryScreen.kt`, `rememberReorderableLazyListState`): the callback runs on every item swap while dragging (`onChangeOrder(item, to.index)` for each crossing), so one drag across k rows issues k reorder calls; the screen keeps its own `categoriesState` list during a drag and re-syncs from the database flow only when no drag is active (`LaunchedEffect(categories) { if (!isAnyItemDragging) ... }`). The index passed is the position in the **displayed** list, which is the user-category list, matching the interactor's indexing. When `hidden_categories` is on and hidden categories are not displayed, the displayed index no longer matches the interactor's index space (the interactor lists all non-system categories, hidden ones included): an upstream defect. Proposal: the Rust reorder takes the moved category id and the **id of the category it should be placed before/after** (or the new index in the all-non-system list) so the UI cannot desynchronise; the Kobayomi category screen always shows hidden categories dimmed instead of dropping them, which removes the problem (see `p1-topic-category-lifecycle.md` section 5).

### 1.6 Delete (`DeleteXCategory.await(categoryId)`)

OBSERVED (`domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt:19-59`, `domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt:19-59`), in `withNonCancellableContext`:

1. `repository.deleteXCategory(categoryId)` = `DELETE FROM categories WHERE _id = :categoryId;`. The system-category trigger aborts for id 0 (exception, caught, `InternalError`). Foreign keys `ON DELETE CASCADE` on `mangas_categories.category_id` / `animes_categories.category_id` remove the membership rows (no `version` bump trigger on delete; the version trigger is `AFTER INSERT` only). Entries that lose their last category fall back to Default (id 0) through the library view's `coalesce`. If this call throws: log, return `Result.InternalError(e)` and stop (nothing else done).
2. `categories = repository.getAllXCategories()`; `updates = categories.mapIndexed { index, c -> CategoryUpdate(id = c.id, order = index.toLong()) }`. This **includes the system category**, so after the first delete the system category gets `sort = 0` and user categories `1..n`. (Before the first delete: system `-1`, user `0..n-1`.) `ORDER BY sort` plus the tie that `Reorder` later creates (system 0 and first user category 0) is why Proposal 1.2 adds `, _id` as tie-break (the system row has id 0 and sorts first, preserving "Default first").
3. `defaultCategory = libraryPreferences.defaultXCategory().get()` (Int, default `-1`); `if (defaultCategory == categoryId.toInt()) libraryPreferences.defaultXCategory().delete()` (key removed, so reads return the default `-1` = "always ask").
4. Remove the id (as a decimal string) from these string-set preferences, writing back only when the set contained it (`if (categoryIdString !in ids) return`):

   | | manga | anime |
   |---|---|---|
   | library update include | `library_update_categories` | `animelib_update_categories` |
   | library update exclude | `library_update_categories_exclude` | `animelib_update_categories_exclude` |
   | auto-delete-after-read exclude | `remove_exclude_categories` | `remove_exclude_anime_categories` |
   | download-new include | `download_new_categories` | `download_new_anime_categories` |
   | download-new exclude | `download_new_categories_exclude` | `download_new_anime_categories_exclude` |

   Upstream defect (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt:36-42`): the manga list is `[mangaUpdateCategories(), mangaUpdateCategories(), removeExcludeCategories(), downloadNewChapterCategories(), downloadNewChapterCategoriesExclude()]`, i.e. `mangaUpdateCategoriesExclude()` is **missing** and the include set is processed twice. A deleted manga category id therefore stays in `library_update_categories_exclude` forever (harmless in practice because ids are never reused within a database... they can be: SQLite `INTEGER PRIMARY KEY` without AUTOINCREMENT reuses the max rowid+1, so after deleting the highest id a new category can inherit a stale exclude entry and silently be excluded from updates). Proposal: process the five sets listed in the table for both kinds.
5. `repository.updatePartialXCategories(updates)` (transaction). Exceptions here return `InternalError` *after* the category is already deleted and the preferences are already cleaned (no rollback).
6. Return `Result.Success`.

UI: confirmation dialog `MR.strings.delete_category` ("Delete category") with text `MR.strings.delete_category_confirmation` ("Do you wish to delete the category \"%s\"?") and OK/Cancel (`MR.strings.action_ok`, `action_cancel`); a failure result shows `internal_error`.

Proposal: do steps 1, 2 and 5 plus the preference cleanup of step 3/4 in the order given but execute the three database steps in one SQL transaction (preferences are outside SQLite and are written after commit), so a crash cannot leave a gap in the numbering.

### 1.7 Assigning categories to an entry (`SetXCategories.await(entryId, categoryIds)`)

OBSERVED: calls `animeRepository.setAnimeCategories(animeId, categoryIds)` / `mangaRepository.setMangaCategories` and swallows any exception after `logcat(ERROR)` (returns Unit either way). The repository does, in one transaction (`data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:91-98`): `DELETE FROM mangas_categories WHERE manga_id = :mangaId;` then one `INSERT INTO mangas_categories(manga_id, category_id) VALUES (:mangaId, :categoryId);` per id. The insert trigger `insert_manga_category_update_version` (`AFTER INSERT`) does `UPDATE mangas SET version = version + 1 WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0` (sync bookkeeping; see area:03/04). Duplicates in `categoryIds` produce duplicate rows (no unique constraint on the pair); the Default category is represented by an empty list, never by id 0 (callers pass `listOfNotNull(category).map { it.id }` where `category` is null for Default, and the change-category dialog hides id 0).

Proposal: de-duplicate the ids, drop id 0 and ids that do not exist in `categories`, return `Result<(), DbError>` (the UI still ignores errors but logs).

### 1.8 Sort mode per category (`SetSortModeForXCategory.await(category|categoryId, type, direction)`)

OBSERVED (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetSortModeForMangaCategory.kt:14-37`):

1. `category = categoryId?.let { repository.getXCategory(it) }`.
2. `flags = (category?.flags ?: 0) + type + direction` using the flag algebra (`Long.plus(Flag)`, see `p1-topic-flag-algebra-and-sort-modes.md`): the type bits (mask `0b00111100`) and the direction bit (mask `0b01000000`) are replaced, other bits of the category's flags are preserved.
3. If `type == Random`: `randomXSortSeed().set(Random.nextInt())` (new 32-bit seed each time the user taps Random, including re-tapping it).
4. If `category != null && categorizedDisplaySettings().get()`: write `CategoryUpdate(id = category.id, flags = flags)` to that category only.
5. Else: `xSortingMode().set(XLibrarySort(type, direction))` (global) and `categoryRepository.updateAllXCategoryFlags(flags)` which runs `UPDATE categories SET flags = coalesce(?, flags);` — **every category gets the same `flags` value computed from the one category passed in** (including its non-sort bits, there are none today).

Callers: `MangaLibrarySettingsScreenModel.setSort(category, mode, direction)` launched on `Dispatchers.IO`, with `category` = the active tab's category (`state.categories.getOrNull(activeCategoryIndex)`, the dialog closes if it is null).

`ResetXCategoryFlags.await()`: `updateAllXCategoryFlags(sort.type + sort.direction)` where `sort` is the global preference (`Type + Direction` = `Flag.plus(Flag)`). Wired only to the manga switch (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt:138-148`: `categorizedDisplaySettings` `onValueChanged { if (!it) ResetMangaCategoryFlags.await() }`); the anime twin exists but is never called, so switching the shared switch off resets manga categories only. Proposal: call both.

`SetXDisplayMode.await(mode)`: `displayMode().set(mode)`; one global key `pref_display_mode_library` shared by manga and anime libraries (the Library settings display tab writes the same value for both).

`categorized_display` also gets rewritten by backup restore: after restoring categories `categorizedDisplaySettings().set(distinctBy { it.flags }.size > 1)` over all DB categories plus restored ones (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaCategoriesRestorer.kt:35-39`, `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeCategoriesRestorer.kt:35-39`): "per-category sort is on iff categories do not all have the same flags value" (system category included in `dbCategories`, so a fresh install with flag 0 plus any backed up category with 64 turns it on).

### 1.9 Add-to-library default category resolution

OBSERVED in four places with the same algorithm (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:349-372`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt:254-282`, `app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt:addFavorite`, plus the anime twins and the migration dialog):

```
categories = GetXCategories.await() without the system category      // all, hidden included
defaultId  = LibraryPreferences.defaultXCategory().get()             // -1 = always ask, 0 = Default, else a category id
defaultCategory = categories.find { it.id == defaultId }
when {
  defaultCategory != null                    -> set favorite=true, then SetXCategories(entry, [defaultCategory.id])
  defaultId == 0 || categories.isEmpty()     -> set favorite=true, then SetXCategories(entry, [])           // Default category
  else                                       -> open the Set-categories dialog (nothing is changed yet)
}
```

`defaultId == -1` (the preference default) falls into the third branch: "Always ask" (`MR.strings.default_category_summary`, "Always ask"). A stale id that no longer exists (should not happen after delete cleanup) also falls into "ask". After a dialog confirm the callers do `SetXCategories` with the checked ids then favourite the entry. The Settings list preference offers `-1 -> "Always ask"` plus every category id -> `visualName` (system category shows "Default"): entries `listOf(defaultValue()) + allCategories.map { it.id.toInt() }`, labels `listOf(default_category_summary) + allCategories.map { it.visualName }` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt:64-72`).

Proposal: one function `resolve_add_target(kind) -> AddTarget { Categories(Vec<i64>) | Ask }` in `kuro-domain::category::membership`, used by all entry points (T-08a-030).

### 1.10 Bulk change categories of the library selection

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt` `openChangeCategoryDialog`, `getCommonCategories`, `getMixCategories`, `setMangaCategories`):

- Dialog categories = displayed categories filtered `it.id != 0L`. For the selected manga list: `common` = intersection of the per-manga category sets (`GetXCategories.await(manga.id).toSet()` reduce `intersect`), `mix` = union minus common. Initial checkbox state per category: `Checked` if in `common`, `TriState.Exclude` if in `mix`, else `None`.
- On confirm the dialog returns `include` = ids whose state is `Checked` or `TriState.Include`, `exclude` = ids whose state is `None` or `TriState.None`; mixed (`Exclude`) ids are in neither list, so they are left untouched. Tapping cycles `CheckboxState.next()` (`core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt`): plain states `Checked -> None -> Checked`; tri-state `None -> Include -> Exclude -> None`. A mixed category (`TriState.Exclude`) therefore goes Exclude (untouched) -> None (remove from all) -> Include (add to all) -> Exclude.
- Apply: for each selected entry: `currentIds = GetVisibleXCategories.await(entry.id).map { it.id }` (**visible categories only**, a hidden membership is therefore dropped by the replacement below: upstream defect, fixed in Rust by reading all categories), `newIds = (currentIds - removeCategories) + addCategories` (as a `List`, duplicates removed by the set subtraction then `plus` appends possibly duplicated ids), then `SetXCategories.await(entry.id, newIds)` (full replacement), all inside `launchNonCancellable`. If the dialog has no categories at all, the dialog shows `MR.strings.information_empty_category_dialog` ("You don't have any categories yet.") and a single button `MR.strings.action_edit_categories` ("Edit categories") that opens the Categories screen on the right (manga or anime) tab.
- Dialog title `MR.strings.action_move_category` ("Set categories"), buttons: Edit (`action_edit`), Cancel, OK.

### 1.11 Categories included in library updates (consumer semantics owned by this domain's preferences)

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:155-178`, mirrored in `app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt:getGlobalUpdateItemCount`): with `include = libraryUpdateCategories` (set of id strings) and `exclude = libraryUpdateCategoriesExclude`:

```
if triggered for one category (categoryId != -1): list = library.filter { it.category == categoryId }   // include/exclude ignored
else:
   includedManga = if include nonEmpty: library.filter { it.category in include } else library
   excludedIds   = if exclude nonEmpty: library.filter { it.category in exclude }.map { id } else []
   list = includedManga.filterNot { id in excludedIds }.distinctBy { id }
```

Because the library view yields one row per (entry, category), an entry in two categories where one is excluded is excluded entirely. The preference dialog is a tri-state list (`TriStateListDialog`): Checked = include, Inversed = exclude (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt:getGlobalUpdateGroup`), confirmed values written as `include.map{id.toString()}.toSet()` / `exclude...`. Summary label from `getCategoriesLabel` (all selected -> labels). Message strings `AYMR.strings.pref_manga_library_update_categories_details` ("Manga in excluded categories will not be updated even if they are also in included categories.") and the anime twin. Implemented by area:26-library-update-jobs; the shared helper is T-08a-028.

### 1.12 Backup interplay (documented here because the type is here)

- Export: only non-system categories (`filterNot(Category::isSystemCategory)`), mapped by `backupCategoryMapper` (fields `name`, `order`, `flags`; area:21-backup-format-and-create).
- Restore (`MangaCategoriesRestorer`): backup categories sorted by `order`; a backup category whose `name` equals an existing category's name maps onto it (no new row); new ones get `order = nextOrder++` starting at `max(existing order)+1` (0 if none... note `maxOfOrNull{...}?.plus(1) ?: 0`, with the system category present the max is at least -1 so the first is 0) and their backed-up `flags`; `hidden` is not part of the backup, so restored categories are visible. Then `categorized_display` is recomputed as in 1.8. Preference sets of category ids in the backup are remapped through the backup's category id -> name -> new id maps (`PreferenceRestorer.restoreCategoriesPreference`) and **appended** to the current set (`preferenceStore.getStringSet(key) += ids`), trying both the manga and the anime name map and keeping whatever resolves.
- Category preference keys that get remapped: `LibraryPreferences.categoryPreferenceKeys` (6 keys: `default_category`, `default_anime_category`, `library_update_categories`, `animelib_update_categories`, `library_update_categories_exclude`, `animelib_update_categories_exclude`) plus `DownloadPreferences.categoryPreferenceKeys` (6 keys: `remove_exclude_categories`, `remove_exclude_anime_categories`, `download_new_categories`, `download_new_anime_categories`, `download_new_categories_exclude`, `download_new_anime_categories_exclude`).

---

## 2. Library

### 2.1 Library read models

OBSERVED (`domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt`, `domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt`):

- `LibraryManga(manga, category, totalChapters, readCount, bookmarkCount, latestUpload, chapterFetchedAt, lastRead)`; `id = manga.id`; `unreadCount = totalChapters - readCount`; `hasBookmarks = bookmarkCount > 0`; `hasStarted = readCount > 0`.
- `LibraryAnime(anime, category, totalCount, seenCount, bookmarkCount, fillermarkCount, latestUpload, episodeFetchedAt, lastSeen)`; `unseenCount = totalCount - seenCount`; `hasBookmarks`, `hasStarted = seenCount > 0`. There is no `hasFillermarks`.
- Mapping from SQL: `MangaMapper.mapLibraryManga(...)` takes `readCount: Double` and `bookmarkCount: Double` (SQLite `sum()` is typed REAL by SQLDelight) and converts with `.toLong()`; `totalCount` is a Long. Rust reads them as `i64` (SQLite `sum` of integer columns returns INTEGER, no rounding concern).

SQL (`data/src/main/sqldelight/view/libraryView.sq`, `libraryView` view) per manga row:

```
M.*, coalesce(C.total,0) AS totalCount, coalesce(C.readCount,0) AS readCount,
coalesce(C.latestUpload,0), coalesce(C.fetchedAt,0) AS chapterFetchedAt, coalesce(C.lastRead,0),
coalesce(C.bookmarkCount,0), coalesce(MC.category_id,0) AS category
FROM mangas M LEFT JOIN (SELECT chapters.manga_id, count(*) AS total, sum(read) AS readCount,
   coalesce(max(chapters.date_upload),0) AS latestUpload, coalesce(max(history.last_read),0) AS lastRead,
   coalesce(max(chapters.date_fetch),0) AS fetchedAt, sum(chapters.bookmark) AS bookmarkCount
   FROM chapters LEFT JOIN excluded_scanlators ON chapters.manga_id = excluded_scanlators.manga_id
        AND chapters.scanlator = excluded_scanlators.scanlator
   LEFT JOIN history ON chapters._id = history.chapter_id
   WHERE excluded_scanlators.scanlator IS NULL GROUP BY chapters.manga_id) AS C ON M._id = C.manga_id
LEFT JOIN mangas_categories AS MC ON MC.manga_id = M._id WHERE M.favorite = 1;
```

Consequences: (a) one row per (favourite manga, category); an entry in three categories appears three times; an entry in none appears once with `category = 0`. (b) Chapters from excluded scanlators are not counted anywhere (total, read, bookmark, latest upload, fetch date, last read). (c) `lastRead` is the max `history.last_read` (epoch ms) over the manga's chapters, including reset rows (value 0 never wins). The anime view `animelibView` (`data/src/main/sqldelightanime/view/animelibView.sq`) is the same shape built from `episodestatsView`, `animehistorystatsView`, `animeseasonstatsView` with a `CASE M.fetch_type WHEN 1 (episodes) / 0 (seasons)` switch: for a season-parent anime (`fetch_type = 0`) `totalCount` is the number of child seasons, `seenCount` the number of fully seen child seasons (`fetch_type = 1` children whose total equals seen), and the other aggregates are max/sum over the children; there is no excluded-scanlator join for anime. Details in area:04-db-anime-schema-migrations (`p1-topic-views-and-aggregates.md`).

`GetLibraryManga.subscribe()` (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetLibraryManga.kt`) wraps the flow in `.retry { if (it is NullPointerException) { delay(0.5.seconds); true } else false }.catch { logcat(ERROR) }`: a NullPointerException from the cursor mapper (a view row with NULL where a non-null Kotlin type is expected) is retried every 500 ms forever; any other error ends the flow silently (the screen then keeps its previous list). Proposal: map NULLs defensively with `COALESCE` in Rust queries so neither the retry nor the swallow is needed; propagate errors to the screen as `LibraryState::Error(msg)` with a retry button.

### 2.2 Display mode, columns, badges, tabs (global preferences)

OBSERVED (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt`, `domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`):

- Display mode: one global key `pref_display_mode_library`, values (string codec) `COMPACT_GRID` (default), `COMFORTABLE_GRID`, `COVER_ONLY_GRID`, `LIST`; any other stored string deserialises to `COMPACT_GRID`. Menu labels: `action_display_grid` "Compact grid", `action_display_comfortable_grid` "Comfortable grid", `action_display_cover_only_grid` "Cover-only grid", `action_display_list` "List". Rendering rule in the pager: `List` -> list; `CompactGrid` and `CoverOnlyGrid` -> compact grid component with `showTitle = (mode is CompactGrid)`; `ComfortableGrid` -> comfortable grid. The per-page `getDisplayMode(page)` parameter exists but returns the same global preference for every page.
- Columns: four Int preferences, default 0 = auto: `pref_library_columns_portrait_key`, `pref_library_columns_landscape_key` (manga), `pref_animelib_columns_portrait_key`, `pref_animelib_columns_landscape_key` (anime). Slider range 0..10, value text `MR.strings.label_auto` ("Auto") when 0 else the number; label `MR.strings.pref_library_columns` ("Items per row") for grids and `AYMR.strings.pref_library_rows` ("List size") when the display mode is List. Chosen by device orientation (`LocalConfiguration.orientation`). Desktop mapping: see `p1-rust-design.md` section 8 (window width breakpoints replace portrait/landscape; both keys stay in the registry for backup compatibility).
- Badges: `display_download_badge` (default false), `display_unread_badge` (true), `display_local_badge` (true), `display_language_badge` (false), `display_continue_reading_button` (false; a "play/read next" button on each cover, one key for both kinds). Dialog labels: manga `action_display_download_badge` "Downloaded chapters", `action_display_unread_badge` "Unread chapters", `action_display_local_badge` "Local source", `action_display_language_badge` "Language", `action_display_show_continue_reading_button` "Continue watching/reading button"; anime uses `action_display_download_badge_anime` "Downloaded episodes", `action_display_unseen_badge` "Unseen episodes" and the same other labels. The anime unseen badge reads the **manga** key `display_unread_badge` (`libraryPreferences.unreadBadge()` in `AnimeLibraryScreenModel.getAnimelibItemPreferencesFlow`), so one switch controls both libraries' unread/unseen badge.
- Tabs: `display_category_tabs` (default true) and `display_number_of_items` (default false); labels `action_display_show_tabs` "Show category tabs", `action_display_show_number_of_items` "Show number of items". Section headings `overlay_header` "Overlay", `tabs_header` "Tabs".
- Tracker filter keys are dynamic: `pref_filter_library_tracked_${trackerId}_v2` (manga) and `pref_filter_animelib_tracked_${trackerId}_v2` (anime), enum `TriState`, default `DISABLED` (area:02 family `filter_tracked_manga`/`filter_tracked_anime`).

### 2.3 Building the displayed library map (`getLibraryFlow`)

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt:355-397`, anime identical):

1. Inputs, combined: `getLibraryManga.subscribe()` (all rows of `libraryView`), the item preference flow (badges, filters), and `downloadCache.changes` (invalidation tick when the download index changes).
2. For each row build a `MangaLibraryItem(libraryManga, downloadCount, unreadCount, isLocal, sourceLanguage)`:
   - `downloadCount = if (downloadBadge) downloadManager.getDownloadCount(manga).toLong() else 0`
   - `unreadCount = if (unreadBadge) libraryManga.unreadCount else 0` (anime: `unseenCount`)
   - `isLocal = if (localBadge) manga.isLocal() else false` (`source == 0`, the Local source id)
   - `sourceLanguage = if (languageBadge) sourceManager.getOrStub(manga.source).lang else ""`
   - Item defaults when constructed bare: `downloadCount = -1`, `unreadCount = -1`, `isLocal = false`, `sourceLanguage = ""`.
3. `groupBy { it.libraryManga.category }` -> `Map<Long, List<Item>>`.
4. Categories = `GetVisibleXCategories.subscribe()`. `displayCategories = if (libraryByCategory.isNotEmpty() && !libraryByCategory.containsKey(0)) categories.filterNot { isSystemCategory } else categories`. So: library has entries but none in Default -> Default tab hidden; library completely empty -> all visible categories (including Default) kept.
5. Result: `displayCategories.associateWith { libraryByCategory[it.id].orEmpty() }`, an ordered map (LinkedHashMap, categories in `sort` order). Categories that are hidden are absent; entries whose only categories are hidden are absent from every page. Visible categories with no matching entries get an empty list (page shows the empty state).

### 2.4 Filters (`applyFilters`)

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt:119-199`), evaluated per entry in this short-circuit order (`&&`): downloaded, unread, started, bookmarked, completed, interval-custom, tracking. `applyFilter(filter, predicate)` (`domain/src/main/java/tachiyomi/domain/entries/TriState.kt`): `DISABLED -> true`, `ENABLED_IS -> predicate()`, `ENABLED_NOT -> !predicate()`. TriState enum order and toggle (`TriState.next()`): `DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`), persisted as the enum name.

| Filter | Manga preference key (default DISABLED) | Anime key | Predicate for `ENABLED_IS` |
|---|---|---|---|
| Downloaded | `pref_filter_library_downloaded_v2` | `pref_filter_animelib_downloaded_v2` | `isLocal() || item.downloadCount > 0 || downloadManager.getDownloadCount(entry) > 0`. If global "Downloaded only" (`pref_downloaded_only`, BasePreferences) is on, the filter is forced to `ENABLED_IS` and the toggle is disabled in the UI |
| Unread / Unseen | `pref_filter_library_unread_v2` | `pref_filter_animelib_unread_v2` (function `filterUnseen`) | `unreadCount > 0` (the raw `LibraryManga.unreadCount`, not the badge value) |
| Started | `pref_filter_library_started_v2` | `pref_filter_animelib_started_v2` | `hasStarted` (`readCount > 0`) |
| Bookmarked | `pref_filter_library_bookmarked_v2` | `pref_filter_animelib_bookmarked_v2` | `hasBookmarks` (`bookmarkCount > 0`) |
| Completed | `pref_filter_library_completed_v2` | `pref_filter_animelib_completed_v2` | `manga.status.toInt() == SManga.COMPLETED` (value 2) |
| Customized update frequency | `pref_filter_library_interval_custom` (shared with anime) | same key | applies only if `ENTRY_OUTSIDE_RELEASE_PERIOD` ("manga_outside_release_period") is in `autoUpdateItemRestrictions`; predicate `manga.fetchInterval < 0`; otherwise the filter function returns `true` (no effect) |
| Tracking | per tracker `pref_filter_library_tracked_{id}_v2` | `pref_filter_animelib_tracked_{id}_v2` | see below |

Tracking filter: `trackingFilter` is the map `trackerId -> TriState` for **logged-in trackers only** (`trackerManager.loggedInTrackersFlow()`); if there are none, `isNotLoggedInAnyTrack = true`. `excludedTracks` = ids with `ENABLED_NOT`, `includedTracks` = ids with `ENABLED_IS`; if both empty the filter is ignored. Otherwise with `entryTracks` = the set of tracker ids the entry is tracked on: `isExcluded = excludedTracks.isNotEmpty() && entryTracks.any { it in excludedTracks }`, `isIncluded = includedTracks.isEmpty() || entryTracks.any { it in includedTracks }`, keep iff `!isExcluded && isIncluded`.

`hasActiveFilters` (drives the filter icon tint and the empty-state message) = any of {downloaded, unread, started, bookmarked, completed, intervalCustom} or any tracker filter is not `DISABLED` (note: raw preference values, so a forced "downloaded only" does not count and an interval-custom value that is currently hidden still counts).

UI visibility: the "Customized update frequency" row is shown only when `!isReleaseBuildType && ENTRY_OUTSIDE_RELEASE_PERIOD in restrictions` (an upstream code comment says to re-enable it once custom intervals are ready for stable). Tracker rows: 0 trackers -> none; 1 -> one row labelled `action_filter_tracked` ("Tracked"); more -> a heading "Tracked" and one row per tracker named by `tracker.name`. Labels: `label_downloaded` "Downloaded", `action_filter_unread` "Unread" (anime `action_filter_unseen` "Unseen"), `label_started` "Started", `action_filter_bookmarked` "Bookmarked", `completed` "Completed", `action_filter_interval_custom` "Customized update frequency".

### 2.5 Sort (`applySort`, comparators)

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt:201-330`): applied after filtering, **per category** using that category's own `flags`:

```
for (category, items) in library:
    sort = MangaLibrarySort.valueOf(category.flags)            // Category?.sort extension
    if sort.type == Random:  items.shuffled(Random(seed = randomMangaSortSeed))   // same order until the seed changes
    else:  comparator = sort.type.comparator(); if !sort.isAscending: comparator = comparator.reversed()
           items.sortedWith(comparator.thenComparator(sortAlphabetically))
```

- `sortAlphabetically(i1, i2) = i1.title.lowercase().compareToWithCollator(i2.title.lowercase())` where the collator is `java.text.Collator.getInstance(Locale.getDefault())` with `strength = PRIMARY` (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt`): case, accent and punctuation-width differences ignored at primary strength; locale-aware ordering. The tie-break `thenComparator(sortAlphabetically)` is applied **after** the optional `reversed()`, so ties on the main key are always ordered A to Z regardless of direction.
- Type comparators (all compare `i1` to `i2`, ascending semantics before the direction flip):

| Type | Manga comparator | Anime comparator |
|---|---|---|
| Alphabetical | `sortAlphabetically` | same |
| LastRead / LastSeen | `libraryManga.lastRead` | `libraryAnime.lastSeen` |
| LastUpdate | `manga.lastUpdate` (epoch ms, 0 = never) | `anime.lastUpdate` |
| UnreadCount / UnseenCount | special: equal -> 0; `i1.unread == 0` -> `if ascending 1 else -1`; `i2.unread == 0` -> `if ascending -1 else 1`; else `unread.compareTo` | same with `unseenCount`. Intent: entries with nothing to read always sort last regardless of direction (the later `reversed()` flips the `1/-1` result back, producing the "zeros last" order in both directions); remaining entries by count |
| TotalChapters / TotalEpisodes | `totalChapters` | `totalCount` |
| LatestChapter / LatestEpisode | `latestUpload` (max `date_upload`) | `latestUpload` |
| ChapterFetchDate / EpisodeFetchDate | `chapterFetchedAt` (max `date_fetch`) | `episodeFetchedAt` |
| DateAdded | `manga.dateAdded` (epoch ms) | `anime.dateAdded` |
| TrackerMean | `trackerScores[id] ?: -1.0` with `Double.compareTo` | same |
| AiringTime | n/a | by `unseenCount` first (`compareTo`, if different), then by `anime.nextEpisodeAiringAt` where `0` always sorts last (`i1 == 0 -> if ascending 1 else -1`, `i2 == 0 -> if ascending -1 else 1`), then equal -> 0 |
| Random | not a comparator (the code `error("Why Are We Still Here? Just To Suffer?")` if reached) | same |

- `trackerScores` (lazy, computed once per sort pass): `trackerManager.getAll(loggedInTrackerIds)` mapped by id; for each entry id: `tracks.isEmpty() -> null`, else `tracks.mapNotNull { trackerMap[it.trackerId]?.mangaService?.get10PointScore(it) }.average()` (so an entry whose tracks are all unscored/logged-out gives `NaN`, since `average()` of an empty list is `NaN`, and `NaN` compares greater than everything with `Double.compareTo`; Proposal below). `defaultTrackerScoreSortValue = -1.0` for entries not in the map or with `null`.
- `nextEpisodeAiringAt`: decoded from `anime.viewerFlags` bits: `(viewerFlags and ANIME_AIRING_TIME_MASK(0x0FFFFFFFF000000)).removeHexZeros(6)` i.e. `value / 16^6` (see `p1-data-and-config.md` section 3.4).

Proposal (Rust):

- Alphabetical key: precompute `title.to_lowercase()` once per item and compare with an ICU4X collator (`icu_collator`) at primary strength for the user's locale (`LANG`/`LC_COLLATE`), fallback root locale. This is the only locale-sensitive operation in the domain.
- `TrackerMean`: treat `NaN` as `-1.0` (sorts like "unscored"); behaviour change vs. upstream only for the NaN edge case.
- Random: use a deterministic shuffle seeded with the stored `i32` seed (`rand_chacha::ChaCha8Rng::seed_from_u64(seed as u32 as u64)` + Fisher-Yates). Shuffle order is not byte-compatible with Kotlin's `java.util.Random`; acceptable because the seed is device-local state and not part of backups (the seed keys are in the backed-up normal namespace but only their stability within a session matters).
- Sorting is stable (`slice::sort_by` is stable), same as `sortedWith`.

### 2.6 Search (`MangaLibraryItem.matches`, debounce)

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryItem.kt:20-60`): search runs after filter and sort, on each category list, only when the search text is non-null: `state.map { it.searchQuery }.debounce(SEARCH_DEBOUNCE_MILLIS)` with `SEARCH_DEBOUNCE_MILLIS = 250L` (`app/src/main/java/eu/kanade/presentation/components/AppBar.kt:65`). Rules (`constraint` = raw text, no trimming):

1. If `constraint.startsWith("id:", ignoreCase = true)`: `id = constraint.substringAfter("id:").toLongOrNull()`; return `libraryManga.id == id` (null id -> false). Used by in-app "go to library entry" deep links.
2. Else return true if **any** of: `title.contains(constraint, ignoreCase = true)`; `author?.contains(...)`; `artist?.contains(...)`; `description?.contains(...)`; or, for **every** sub-constraint of `constraint.split(",").map { trim }` (logical AND), `checkNegatableConstraint(sub) { sourceName.contains(it, true) || genre.any { g -> g.equals(it, true) } }` where a sub-constraint starting with `-` has the minus stripped, `trimStart()` applied, and the predicate result inverted. `sourceName = sourceManager.getOrStub(manga.source).getNameForMangaInfo()` (lazy).
3. Quirk: because step 2 ORs the plain-text fields with the AND of sub-constraints, a query like `-xyz` matches every entry whose source/genres do not contain `xyz` and also, trivially, entries whose title contains the literal `-xyz`. A query that is empty string (not null) matches everything (`"".contains("")` is true for the title).
4. `getMangaCountForCategory(category)` returns `library[category].size` when `showMangaCount || !searchQuery.isNullOrEmpty()`; with a search active the tab badges therefore show per-category match counts. Tabs are forced visible while a search is active: `showPageTabs = state.showCategoryTabs || !state.searchQuery.isNullOrEmpty()` (still requires `categories.size > 1`).
5. Empty-page message order: search non-empty -> `no_results_found` ("No results found") plus a "Search globally" item (`GlobalSearchItem`) that opens global search with the query; else filters active -> `error_no_match` ("No match found"); else `information_no_manga_category` ("Category is empty"). Whole library empty with no search and no filters: `information_empty_library` ("Your library is empty") with a "Getting started guide" action.

### 2.7 Selection model

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt:575-651`): `selection` is an ordered persistent list of `LibraryManga` rows (identity by `id`; an entry in two categories can be selected only once per id check but the row of the category you tapped is stored). `selectionMode = selection.isNotEmpty()`.

- `toggleSelection(m)`: remove if any selected has the same id, else append.
- `toggleRangeSelection(m)` (long-press while in selection): `last = selection.lastOrNull()`; if `last?.category != m.category` just append `m`; else take that category's displayed items (after filter/sort/search), `lastIndex = indexOf(last)`, `curIndex = indexOf(m)`, select the inclusive index range between them (either direction) skipping ids already selected; `lastIndex == curIndex` selects nothing new. Haptic `LongPress` feedback.
- `selectAll(index)`: all items of the page `categories.getOrNull(index)?.id ?: -1` not yet selected (unknown page selects nothing).
- `invertSelection(index)`: items of that category: already-selected ones are removed from the selection, others are added.
- `clearSelection`. Back press: clears selection first, then clears search (`BackHandler`).
- Bottom action menu (selection mode): change category, mark as read, mark as unread, download (menu `NEXT_1_ITEM`, `NEXT_5_ITEMS`, `NEXT_10_ITEMS`, `NEXT_25_ITEMS`, `UNVIEWED_ITEMS`) shown only if no selected entry is local, delete (dialog: remove from library and/or delete downloaded chapters). Download action semantics: for each selected manga `GetNextChapters.await(manga.id)` (unread only, ascending), drop chapters already queued or already downloaded, `take(amount)` if limited, queue; run inside `launchNonCancellable`. Removal: `distinctBy id`; if "remove from library": `removeCovers` then `MangaUpdate(favorite = false, id)` batch; if "delete chapters": for each manga whose source is an `HttpSource` call `downloadManager.deleteManga`. These belong to area:37-ui-library and area:23-downloads-manga; they are listed here because they call this area's `GetNextChapters`.

### 2.8 Active category, titles, random entry, "continue" button

- `activeCategoryIndex` is persisted as the app-state Int `__APP_STATE_last_used_category` (manga) / `__APP_STATE_last_used_anime_category` (anime), default 0, and is an **index into the displayed category list, not an id** (`asState(screenModelScope)` writes through on every page change via `onChangeCurrentPage`). The pager is created with `currentPage().coerceAtMost(categories.lastIndex)`; if the category list shrinks below the current page the pager scrolls to the last page (`LaunchedEffect(categories)`). `LibraryTabs` uses `pagerState.currentPage.coerceAtMost(categories.lastIndex)`.
- Toolbar title (`State.getToolbarTitle(defaultTitle, defaultCategoryTitle, page)`): if no category at `page` -> `LibraryToolbarTitle(defaultTitle)` (no count). `categoryName = if (category.isSystemCategory) defaultCategoryTitle("Default") else category.name`. `title = if (showCategoryTabs) defaultTitle else categoryName`. `count = null if !showMangaCount; else if !showCategoryTabs -> items in this category (null when neither count pref nor search); else whole-library distinct count (distinctBy manga.id over all categories)`. `defaultTitle` = `AYMR.strings.label_manga_library` ("Manga") / `label_anime_library` ("Anime").
- `isLibraryEmpty` = distinct entry count over all pages is 0.
- Random entry: current category's displayed items `randomOrNull()`; if none: snackbar `MR.strings.information_no_entries_found` ("No entries found in this category").
- Continue button (`showContinueViewingButton`): manga: `GetChaptersByMangaId.await(id, applyScanlatorFilter = true).getNextUnread(manga, downloadManager)` = apply the details-screen filters (read/downloaded/bookmark filters of the manga's chapter flags), then in ascending reading order the first unread (`findLast { !read }` when the manga is sorted descending, else `find { !read }`; `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt`); none -> snackbar `no_next_chapter` ("Next chapter not found"). Anime: `GetEpisodesByAnimeId.await(id).getNextUnseen(anime, downloadManager)` (`app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt`), none -> `AYMR.strings.no_next_episode` ("Next Episode not found!"); if found open the player (honouring `alwaysUseExternalPlayer`).
- Global/category refresh from the library: toolbar actions "Update library" (`MangaLibraryUpdateJob.startNow(context, null)`) and "Update category" (current category); snackbar `updating_category` ("Updating category") or `update_already_running` ("An update is already running") depending on whether the job started. Pull-to-refresh shows the spinner for a fixed 1 second (`delay(1.seconds)`) and only if the job started.
- Re-selecting the Library bottom tab opens the settings sheet (`onReselect -> requestOpenSettingsSheet`).

### 2.9 Sort/filter/display dialog behaviour

OBSERVED (`app/src/main/java/eu/kanade/presentation/library/manga/MangaLibrarySettingsDialog.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibrarySettingsScreenModel.kt`): three tabs (`action_filter` "Filter", `action_sort` "Sort", `action_display` "Display").

- Filter tab: `TriStateItem` per filter; click calls `screenModel.toggleFilter(LibraryPreferences::filterX)` = `preference.getAndSet { it.next() }`; `toggleTracker(id)` likewise on `filterTrackedManga(id)`.
- Sort tab options in this order (manga): Alphabetically (`action_sort_alpha`), Total chapters (`action_sort_total`), Last read (`action_sort_last_read`), Last manga update (`AYMR.strings.action_sort_last_manga_update`), Unread count (`action_sort_unread_count`), Latest chapter, Chapter fetch date, Date added, Tracker score (only when at least one tracker is logged in), Random. Anime order: Alphabetically, Total episodes, Last seen, Last anime update, Unseen count, Latest episode, Episode fetch date, Date added, Tracker score (if trackers), Airing time, Random.
- Click semantics: `sortingMode = category.sort.type`, `sortDescending = !category.sort.isAscending`. Clicking the **current** type toggles direction (descending -> Ascending, ascending -> Descending); clicking a **different** type keeps the current direction (`sortDescending -> Descending else Ascending`). Random has no direction: clicking sets `(Random, Ascending)` and shows a refresh icon when currently selected (re-tapping re-rolls the seed, section 1.8). `setSort(category, type, direction)` runs on IO.
- Display tab: chips for display mode, slider for columns, badge checkboxes, tab checkboxes (section 2.2); each writes its preference immediately.

---

## 3. History

### 3.1 Recording history

OBSERVED:

- Manga (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:579-603`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:259-270`): `chapterReadStartTime` (epoch ms) is set by `restartReadTimer()` when a chapter is loaded (`loadNewChapter` calls `flushReadTimer(); restartReadTimer()`) and on activity `onResume`. `flushReadTimer()` (called on activity `onPause` and when switching chapters) launches `updateHistory(currentChapter)`:

  ```
  if (incognitoMode) return
  readAt = Date(); sessionReadDuration = chapterReadStartTime?.let { readAt.time - it } ?: 0
  upsertHistory.await(MangaHistoryUpdate(chapterId, readAt, sessionReadDuration))
  chapterReadStartTime = null
  ```
  So each pause/chapter change adds the elapsed session time; after a flush the start time is null until the next `restartReadTimer` (a second flush without a restart adds 0 ms but still refreshes `last_read`).
- Anime (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1655-1700, 1756-1799`, external players `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:441-447`): `saveWatchingProgress(episode)` = `saveEpisodeProgress` then `saveEpisodeHistory` (launched non-cancellable). It is called **from `onSecondReached(position, duration)`, i.e. once for every second the player reports**, and also from `saveCurrentEpisodeWatchingProgress()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:201` and `:326`: on pause/stop paths), on episode change and in `onCleared()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1099`). `saveEpisodeHistory`: `if (!incognitoMode) upsertHistory.await(AnimeHistoryUpdate(episodeId, Date()))`. Net effect: during playback `animehistory.last_seen` and the episode's `last_second_seen`/`total_seconds` are rewritten every second; the history order is therefore always "most recently playing". No duration is stored for anime. Proposal: coalesce to one write per 5 s of playback plus unconditional writes on pause, seek-commit, episode change and shutdown (T-08a-019).
- Incognito (`BasePreferences.incognitoMode`, app-state key `incognito_mode`): history is **not** written. (Chapter/episode progress is also not written in incognito unless a tracker exists for anime; that is area:43/45.)

### 3.2 Upsert, reset, delete

OBSERVED (`data/src/main/sqldelight/data/history.sq`, `data/src/main/sqldelightanime/dataanime/animehistory.sq`):

```
-- manga
upsert: INSERT INTO history(chapter_id, last_read, time_read) VALUES (:chapterId, :readAt, :time_read)
        ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read WHERE chapter_id = :chapterId;
-- anime
upsert: INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt)
        ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt WHERE episode_id = :episodeId;
```

- `history.chapter_id` and `animehistory.episode_id` are `UNIQUE`: **one history row per chapter/episode**, ever (re-reading updates the row). `time_read` (ms) accumulates across sessions. Dates are epoch milliseconds (`INTEGER AS Date` via `DateColumnAdapter`: `Date(long)` / `date.time`).
- `resetXHistoryById` (`UPDATE history SET last_read = 0 WHERE _id = :historyId`), `resetHistoryByMangaId` (set `last_read = 0` for every history row of the manga's chapters). These are **soft resets**: the row (and `time_read`) is kept so total read duration statistics survive; `last_read = 0` hides it from the history list. `removeAllHistory` is a hard `DELETE FROM history` (and wipes `time_read`: total read duration drops to 0). `removeResettedHistory` (`DELETE FROM history WHERE last_read = 0`) is used only by the Settings > Advanced "Clear database" screens (`app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt:213`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt:296`).
- Errors: `resetMangaHistory`, `resetHistoryByMangaId`, `upsertMangaHistory` catch `Exception`, `logcat(ERROR)` and return normally; `deleteAllMangaHistory` returns `false` on exception, `true` otherwise (UI shows the "History deleted" snackbar only on `true`). Anime twins identical.
- `getHistoryByMangaId(mangaId)`: history rows of all chapters of a manga (`JOIN chapters C ON H.chapter_id = C._id WHERE C.manga_id = :mangaId`), no order.

### 3.3 History list query and view

OBSERVED (`data/src/main/sqldelight/view/historyView.sq`): view `historyView` joins `mangas`, `chapters`, `history`, plus a sub-select `max_last_read` = per manga the `(chapter_id, MAX(last_read))` (SQLite bare-column rule: `chapter_id` is taken from the row holding the maximum). Query `history(query)`:

```
SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration
FROM historyView
WHERE historyView.readAt > 0
  AND maxReadAtChapterId = historyView.chapterId
  AND lower(historyView.title) LIKE ('%' || :query || '%')
ORDER BY readAt DESC;
```

- Result: **one row per manga** — its most recently read chapter — newest first, only rows with `readAt > 0`, and only titles containing `query` (SQLite `LIKE` is ASCII-case-insensitive; `lower()` is ASCII-only; the query text is not lower-cased and `%`/`_` in it act as wildcards). Reset rows (`last_read = 0`) never win the max unless all are 0 (then excluded by `readAt > 0`): resetting the newest row makes the previous chapter of the same manga appear instead; the entry disappears only when all its rows are reset.
- `getLatestHistory`: same select without the title/max predicates, `ORDER BY readAt DESC LIMIT 1` = the single most recently read chapter overall (used for "resume last").
- Mapper (`MangaHistoryMapper.mapMangaHistoryWithRelations`) builds `coverData = MangaCover(mangaId, sourceId, isMangaFavorite = favorite, url = thumbnailUrl, lastModified = coverLastModified)`. Anime view `animehistoryView` is identical with `episodes`/`animehistory` and no duration.
- `GetXHistory.subscribe(query)` re-emits on any change to the joined tables. The History tab collects `_query.collectLatest { getHistory.subscribe(query ?: "").distinctUntilChanged().catch { log; send InternalError }.map { toHistoryUiModels() }.flowOn(IO).collect }` — no debounce in the screen model (the search toolbar typing speed drives re-queries; `collectLatest` cancels stale queries).

### 3.4 History tab behaviour

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt`, `presentation/history/**`):

- Date headers: `toHistoryUiModels` = items with `insertSeparators`: before = previous item, after = next item; `beforeDate = before?.item?.readAt?.time?.toLocalDate()`, `afterDate` likewise; a `Header(afterDate)` is inserted when `beforeDate != afterDate && afterDate != null` (so a header precedes the first item, then at every local-date change). `Long.toLocalDate()` uses `ZoneId.systemDefault()`. Header text = `relativeDateText(date)`: "Today" (`relative_time_today`), "Yesterday"/"%d days ago" (plural `relative_time`, 1..6 days), else `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)` (or the user's chosen format) — controlled by UI preferences `relative_time_v2` (default true) and `app_date_format` (default empty = locale short date); `N/A` (`not_applicable`) for missing dates.
- Row text: `if (chapterNumber > -1) stringResource(recent_manga_time /* "Ch. %1$s - %2$s" */, formatChapterNumber(n), readAt.toTimestampString()) else readAt.toTimestampString()`; anime `recent_anime_time` "Ep. %1$s - %2$s". `formatChapterNumber` = `DecimalFormat("#.###")` with `.` decimal separator (so `12.0 -> "12"`, `12.5 -> "12.5"`, `0.1234 -> "0.123"`). `toTimestampString()` = `DateFormat.getTimeInstance(DateFormat.SHORT)` (locale short time, e.g. `21:30`).
- Row actions: click row = resume (`getNextChapterForManga(mangaId, chapterId)` -> `GetNextChapters.await(mangaId, chapterId, onlyUnread = false)`, take `firstOrNull()`; `Event.OpenChapter(chapter)`; null -> snackbar `no_next_chapter`/`no_next_episode`); click cover = open entry; heart icon (only when `!coverData.isMangaFavorite`) = add to library with the default-category algorithm (1.9), duplicate dialog if a library entry with the same title exists (`GetDuplicateLibraryManga`); trash icon = Remove dialog: title `action_remove` "Remove", text `dialog_with_checkbox_remove_description` ("This will remove the read date of this chapter. Are you sure?"; anime `dialog_with_checkbox_remove_description_anime` "This will remove the watch date of this episode. Are you sure?"), checkbox `dialog_with_checkbox_reset` ("Reset all chapters for this manga"; anime "Reset all episodes for this anime"); confirm with the box ticked -> `RemoveXHistory.await(entryId)` (reset all rows of the entry), else `await(history)` (reset that one row). Toolbar "Clear history" (`pref_clear_history`, DeleteSweep icon) -> dialog `action_remove_everything` "Remove everything" / `clear_history_confirmation` ("Are you sure? All history will be lost.") with OK/Cancel -> `removeAllHistory()` -> on true snackbar `clear_history_completed` ("History deleted").
- Errors from the flow: snackbar `internal_error`. Empty state: `information_no_recent_manga` ("Nothing read recently"; anime `information_no_recent_anime` "Nothing watched recently"), with a non-empty search `no_results_found`.
- External resume channel: `resumeLastChapterReadEvent` (a `Channel<Unit>`, triggered by the app's "resume" shortcut / widget): `getNextChapter()` = `GetNextChapters.await(onlyUnread = false).firstOrNull()` computed from the last history entry; null -> `no_next_chapter` snackbar.

### 3.5 Next chapter / next episode (`GetNextChapters`, `GetNextEpisodes`)

OBSERVED (`domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt`, three overloads):

1. `await(onlyUnread = true)`: `history = repository.getLastMangaHistory() ?: return emptyList()` (the single most recent history row with `last_read > 0`), then `await(history.mangaId, history.chapterId, onlyUnread)`.
2. `await(mangaId, onlyUnread = true)`: `manga = getManga.await(mangaId) ?: return emptyList()`; `chapters = getChaptersByMangaId.await(mangaId, applyScanlatorFilter = true).sortedWith(getChapterSort(manga, sortDescending = false))`; return `chapters.filterNot { it.read }` if `onlyUnread` else all `chapters`. `getChaptersByMangaId` swallows exceptions and returns `emptyList()`. Scanlator filter = hide chapters whose scanlator is in `excluded_scanlators` for that manga. **Anime has no scanlator filter**: `GetNextEpisodes` calls `getEpisodesByAnimeId.await(animeId)` without a flag.
3. `await(mangaId, fromChapterId, onlyUnread = true)`: `chapters = await(mangaId, onlyUnread)`; `currIndex = chapters.indexOfFirst { it.id == fromChapterId }` (−1 if absent, e.g. because it is already read and `onlyUnread` removed it); `next = chapters.subList(max(0, currIndex), chapters.size)`; if `onlyUnread` return `next`. Otherwise `fromChapter = chapters.getOrNull(currIndex)`; return `next` if `fromChapter != null && !fromChapter.read` (the current chapter is unfinished: resume it) else `next.drop(1)` (current chapter finished: go to the one after it). When `currIndex == -1` and `onlyUnread == false` the whole list is returned minus its first element... precisely: `subList(0, size)`, `fromChapter = null` so `drop(1)` removes the first chapter (an upstream edge case; Rust keeps it, see edge cases).
- `getChapterSort(manga, sortDescending = false)` ascending **reading order** per the manga's sort mode (`Manga.sorting` bits of `chapter_flags & 0x300`): `CHAPTER_SORTING_SOURCE (0x000)`: with `sortDescending=false` -> `c2.sourceOrder.compareTo(c1.sourceOrder)` (source order 0 = newest, so descending source order = oldest first); `CHAPTER_SORTING_NUMBER (0x100)`: `chapterNumber` ascending; `CHAPTER_SORTING_UPLOAD_DATE (0x200)`: `dateUpload` ascending; `CHAPTER_SORTING_ALPHABET (0x300)`: `name.compareToWithCollator` ascending; anything else throws `NotImplementedError`. Episode twin: masks `0x200/0x400/0x600` (`Anime.EPISODE_SORTING_*`) with `episodeNumber`.
- Callers: library "download next N" (`getNextChapters.await(mangaId)`), reader auto-download-ahead (`await(manga.id, nextChapter.id!!)`, `onlyUnread = true` default), player auto-download-ahead (`GetNextEpisodes.await(anime.id, nextEpisode.id!!)`), history resume (`onlyUnread = false`).

### 3.6 Total read duration

`GetTotalReadDuration.await()` -> `SELECT coalesce(sum(time_read), 0) FROM history;` (ms, manga only; includes soft-reset rows, excludes hard-deleted history). Used by the Statistics screen overview `totalReadDuration` (area:42). There is no anime equivalent upstream (anime history has no duration).

---

## 4. Updates

### 4.1 Interactors and SQL

OBSERVED (`domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt`, `data/src/main/sqldelight/view/updatesView.sq`, `data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt`):

- Constants: every call passes `limit = 500`.
- `await(read: Boolean, after: Long)` -> `getUpdatesByReadStatus(read, after, 500)`; `subscribe(instant: Instant)` -> `getRecentUpdates(instant.toEpochMilli(), 500)`; `subscribe(read, after)` -> `getUpdatesByReadStatus(read, after, 500)` (used by home-screen widgets with `read = false`).
- View `updatesView`:

  ```
  SELECT mangas._id AS mangaId, mangas.title AS mangaTitle, chapters._id AS chapterId, chapters.name AS chapterName,
         chapters.scanlator, chapters.read, chapters.bookmark, chapters.last_page_read, mangas.source, mangas.favorite,
         mangas.thumbnail_url AS thumbnailUrl, mangas.cover_last_modified AS coverLastModified,
         chapters.date_upload AS dateUpload, chapters.date_fetch AS datefetch
  FROM mangas JOIN chapters ON mangas._id = chapters.manga_id
  WHERE favorite = 1 AND date_fetch > date_added
  ORDER BY date_fetch DESC;
  getRecentUpdates:     SELECT * FROM updatesView WHERE dateUpload > :after LIMIT :limit;
  getUpdatesByReadStatus: SELECT * FROM updatesView WHERE read = :read AND dateUpload > :after LIMIT :limit;
  ```
  Notes: only favourites; only chapters whose `date_fetch` is later than the manga's `date_added` (chapters that existed when the manga was first added are not "updates"); the **time window is applied to `date_upload`, not `date_fetch`**; the newest `date_fetch` first, then cut to 500. Because the outer `SELECT *` has no `ORDER BY`, ordering relies on SQLite honouring the view's `ORDER BY` (it does in practice because the planner keeps the view subquery order); Proposal: write the `ORDER BY date_fetch DESC, chapters._id DESC` explicitly in the Rust query. Excluded scanlators are **not** filtered in updates. `read`/`bookmark` are SQLite INTEGER booleans.
- Anime: `animeupdatesView` adds `episodes.fillermark`, `episodes.last_second_seen`, `episodes.total_seconds AS totalSeconds`; filter `seen = :seen`; mapper `AnimeUpdatesWithRelations`.
- Mapper builds `coverData` as in history (with `favorite` as `isXFavorite`, which is always true here).
- Dispatch: `awaitList` / `subscribeToList` of the handler (area:06).

### 4.2 Updates tab behaviour

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt`):

- Window: `limit = ZonedDateTime.now().minusMonths(3).toInstant()` computed once when the screen model starts (so a long-lived screen keeps a fixed cutoff) and `getUpdates.subscribe(limit).distinctUntilChanged()`, combined with `downloadCache.changes` and `downloadManager.queueState` for download state. Home-screen widgets use `ZonedDateTime.now().minusMonths(3)` at each evaluation.
- Items: for each update look up the active download (`getQueuedDownloadOrNull(chapterId)`) or `isChapterDownloaded(name, scanlator, mangaTitle, sourceId)` to compute `NOT_DOWNLOADED | queued status | DOWNLOADED`.
- Day grouping: `getUiModel` = `insertSeparators` on `dateFetch.toLocalDate()` with the same rule as history (header before the first item and on every change). Header text via `relativeDateText`.
- First row: "Library last updated: <relative time>" from `lastUpdatedTimestamp()` (app-state `library_update_last_timestamp`, written by both library update jobs at the start of each run; manga and anime share this one key).
- Update library action: `MangaLibraryUpdateJob.startNow(...)`; snackbar `updating_library` ("Updating library") or `update_already_running` ("An update is already running"); events delivered through an unlimited channel.
- New-updates counters: `newMangaUpdatesCount` (`library_unread_updates_count`, default 0) and `newAnimeUpdatesCount` (`library_unseen_updates_count`, default 0) are incremented by the library update jobs (`getAndSet { it + newChapters.size }`) and reset to 0 by `resetNewUpdatesCount()` on entering and on leaving the Updates tab (`DisposableEffect(Unit) { reset(); onDispose { reset() } }`). The bottom-nav badge on the Updates tab shows `anime + manga` counts if `library_show_updates_count` (`newShowUpdatesCount`, default true) else 0; hidden at 0; its accessibility label is the plural `notification_chapters_generic` ("%1$d new chapter(s)"), launched from `app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt:262-285`.
- Selection: `toggleSelection(item, selected, userSelected, fromLongPress)` with first/last selected position bookkeeping: a long-press while something is selected selects the contiguous range between the nearest end and the pressed item; plain taps update the position bounds. Actions: bookmark/unbookmark (only items whose flag differs: `filterNot { it.update.bookmark == bookmark }` -> `ChapterUpdate(id, bookmark)` batch), mark read/unread (`SetReadStatus`), download (group by manga, skip if source unavailable), delete downloads (confirm dialog `UpdatesDeleteConfirmationDialog`), anime adds fillermark/unmark and a "show qualities" dialog. See area:42.

---

## 5. Library preferences (behaviour of `LibraryPreferences`)

The full 87-row key/default table is in `p1-data-and-config.md` section 5; the quirks (shared keys, crossed swipe keys, wrong-source `setSeasonSettingsDefault`) are in `p1-topic-library-preferences-quirks.md`. Behavioural notes:

- `setChapterSettingsDefault(manga)` copies the manga's six flag components (`unreadFilterRaw`, `downloadedFilterRaw`, `bookmarkedFilterRaw`, `sorting`, `displayMode`, direction as `CHAPTER_SORT_DESC|ASC`) into the six `default_chapter_*` preferences; `setEpisodeSettingsDefault(anime)` copies nine components (adds fillermark filter, show-previews raw, show-summaries raw); `setSeasonSettingsDefault(anime)` copies sixteen. The details screen offers "Set as default" with an "apply to existing library" option which then runs `SetXDefaultYFlags.awaitAll()` over all favourites (writes the default prefs into every favourite's flags). Snackbar `chapter_settings_updated` ("Updated default chapter settings").
- New entries receive their initial chapter/episode flags from these defaults when added (`SetMangaDefaultChapterFlags.await(manga)` after insert; area:07/10).
- `autoUpdateInterval` (`pref_library_update_interval_key`, Int hours; options 0 = Off, 12, 24, 48, 72, 168) and `autoUpdateDeviceRestrictions` (`wifi`, `network_not_metered`, `ac`) are rescheduling triggers for the library update jobs (area:26; `MangaLibraryUpdateJob.setupTask(context, interval)`), item restrictions set `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` are evaluated per entry at queue time with the skip reasons `skipped_reason_not_always_update` ("Skipped because series does not require updates"), `skipped_reason_completed`, `skipped_reason_not_caught_up`, `skipped_reason_not_started`, `skipped_reason_not_in_release_period` (logged only; an upstream code comment asks whether to surface the reasons to the user).
- `autoClearItemCache` (`auto_clear_chapter_cache`): on cold start `MainActivity` clears the chapter cache (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:324`).
- Swipe actions (`ChapterSwipeAction`: ToggleRead, ToggleBookmark, Download, Disabled; `EpisodeSwipeAction`: ToggleSeen, ToggleBookmark, ToggleFillermark, Download, Disabled) are stored by enum name; on desktop they become row context actions and optional touch gestures (section 8 of `p1-rust-design.md`).
- Duplicate marking (`mark_duplicate_read_chapter_read` set of `"existing"` / `"new"`; episodes `"existing_episode"` / `"new_episode"`): `existing` = after finishing a chapter, mark other unread chapters with the same recognised `chapter_number` as read (`ReaderViewModel.updateChapterProgressOnComplete`); `new` = after fetching new chapters (`SyncChaptersWithSource`).
