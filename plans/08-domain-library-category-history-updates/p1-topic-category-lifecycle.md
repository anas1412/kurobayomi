# Area 08 — Topic: category lifecycle (ordering numbers, hidden, per-category state)

Sources: `domain/src/main/java/tachiyomi/domain/category/**` (26 interactor/repository files), `data/src/main/sqldelight/data/categories.sq`, `data/src/main/sqldelightanime/dataanime/categories.sq`, `app/src/main/java/eu/kanade/tachiyomi/ui/category/**`, `app/src/main/java/eu/kanade/presentation/category/**`.

## 1. The `sort` column is "order values with drift", not a dense index

Worked example (manga database; the anime database behaves identically):

| Step | Operation | Rows after (`_id`:`name` sort) |
|---|---|---|
| 0 | fresh install | 0:`""` -1 |
| 1 | create "Reading" (max -1 +1 = 0) | 0:`""` -1, 1:Reading 0 |
| 2 | create "Plan" (max 0 + 1) | 0:`""` -1, 1:Reading 0, 2:Plan 1 |
| 3 | create "Done" (max 1 + 1) | 0:`""` -1, 1:Reading 0, 2:Plan 1, 3:Done 2 |
| 4 | delete "Plan" (id 2): renumber all remaining by index: Default 0, Reading 1, Done 2 | 0:`""` 0, 1:Reading 1, 3:Done 2 |
| 5 | create "Hold" (max 2 + 1) | 0:`""` 0, 1:Reading 1, 3:Done 2, 4:Hold 3 |
| 6 | reorder Hold to index 0 among user categories [Reading, Done, Hold] -> [Hold, Reading, Done] => orders 0, 1, 2 (system untouched, still 0) | 0:`""` 0, 4:Hold 0, 1:Reading 1, 3:Done 2 |

After step 6 two rows tie at `sort = 0`. SQLite's `ORDER BY sort` without a tie-break returns them in an unspecified (in practice rowid) order, so Default (id 0) comes first. The Rust queries add `, _id` to make this deterministic and keep Default first even when ids are not ordered (they always are for the seeded id 0).

Numbers are only ever compared, never displayed. Backups keep the `order` of the source database; restore rewrites order starting from `max(existing)+1` in backup order (area:22).

## 2. Hidden categories

- `hidden` is set only through `HideXCategory` (toggle). It is **not** in the backup format; a restored category is always visible and a backup of a database with hidden categories loses the hidden flag.
- Library: hidden categories vanish together with entries that live only in them. An entry in both a hidden and a visible category appears under the visible one only.
- Category management screen: shows hidden ones with the "show" icon unless `hidden_categories` ("Hide hidden categories from categories and storage screen") is on, in which case they are listed neither on the Categories screen nor in the Storage screen's category filter; the user then cannot unhide them from the Categories screen until the preference is switched off again (an upstream usability trap).
- Update jobs (area:26): the category include/exclude sets work on ids regardless of hidden; hidden categories still take part in library updates.
- The library screen model injects `GetVisibleXCategories` (not `GetXCategories`) and uses it for the per-entry lookups too (`getCommonCategories`, `getMixCategories`, `setXCategories`: `getCategories.await(entry.id)`). Consequence (upstream defect): the bulk "Set categories" action rebuilds each entry's membership from its **visible** categories only, so an entry that was also in a hidden category silently loses that membership when the user bulk-edits categories from the library. Proposal: the bulk action reads the entry's categories with the *all* variant (hidden included) before applying add/remove, so hidden memberships are preserved (T-08a-017).

Proposal (Kurobayomi): the Categories screen always lists hidden categories (rendered dimmed with the closed-eye icon), and the `hidden_categories` preference keeps its key and only affects the Storage screen's filter. This removes the unhide trap and the reorder index mismatch described in behaviour 1.5.

## 3. Per-category sort state vs global state

| `categorized_display` | Setting a sort in the sheet | Reading the sort of a tab |
|---|---|---|
| off (default) | writes the global preference **and** writes the same flags to every category | each category's flags (all equal) |
| on | writes flags to the active category only (global preference untouched) | each category's own flags |

Switching the preference off triggers `ResetXCategoryFlags` (upstream for manga only): every category gets `global.type + global.direction`, discarding per-category choices. Switching on changes nothing until the user sets a sort on a tab. Backup restore may flip the preference to `true` automatically (behaviour 1.8).

## 4. The Default category inside every algorithm

| Algorithm | Treatment of id 0 |
|---|---|
| Category management screen | never listed, never editable |
| Library tabs | listed (label "Default") only if it has entries or the library is empty |
| Change-category dialog | not listed (`id != 0`); "no category" = empty id list |
| Add-to-library default | `default_category == 0` means "put in Default (no rows)" |
| Reorder | excluded from the list and never rewritten |
| Delete | cannot be deleted (trigger); is renumbered by the post-delete pass |
| Backup | not exported |
| Library update include/exclude lists | listed in the dialog (as "Default") and may be selected; entries with no category have `category = 0` so `0 in include` works |
| Default-category preference list | listed first after "Always ask" |

## 5. Rust API surface (Proposal)

```rust
pub trait CategoryService {
    async fn create(&self, kind: EntryKind, name: &str) -> Result<Category, CategoryError>;
    async fn rename(&self, kind: EntryKind, id: i64, name: &str) -> Result<(), CategoryError>;
    async fn toggle_hidden(&self, kind: EntryKind, id: i64) -> Result<(), CategoryError>;
    async fn move_before(&self, kind: EntryKind, id: i64, before: Option<i64>) -> Result<(), CategoryError>; // None = to the end
    async fn delete(&self, kind: EntryKind, id: i64) -> Result<(), CategoryError>;
    fn subscribe_all(&self, kind: EntryKind) -> watch::Receiver<Arc<Vec<Category>>>;
    fn subscribe_visible(&self, kind: EntryKind) -> watch::Receiver<Arc<Vec<Category>>>;
    async fn of_entry(&self, kind: EntryKind, entry_id: i64) -> Result<Vec<Category>, CategoryError>;
    async fn set_entry_categories(&self, kind: EntryKind, entry_id: i64, ids: &[i64]) -> Result<(), CategoryError>;
}
```
`CategoryError`: `InvalidName`, `DuplicateName`, `NotFound`, `SystemCategory`, `Db(DbError)`. The three Kotlin `Result` shapes (`Success`, `InternalError`, `Unchanged`) map to `Ok(())` / `Err(Db)` / `Err(NotFound)` — the UI shows `MR.strings.internal_error` for `Db` and swallows `NotFound` from reorder like upstream's `Unchanged`.
