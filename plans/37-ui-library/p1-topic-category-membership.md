# Topic — Category membership, the Change-Categories dialog, and category management

Companion to p1-behavior.md §B-13 and §B-15, p1-edge-cases-and-tests.md E-07, E-12, E-31, E-39, E-40, E-41. Upstream paths are repo-relative to `/home/blackbox/aniyomi-src/`.

## 1. Data model recap

- `categories(_id, name, sort, flags, hidden)`. Row `_id = 0` is the system ("Default") row, seeded with `sort = -1`, `flags = 0`, `hidden = 0` (`data/src/main/sqldelightanime/dataanime/categories.sq:10`).
- `animes_categories(anime_id, category_id)`: many-to-many. No row = Default (`coalesce(MC.category_id, 0)` in the library view).
- Visible set = `hidden = 0`. Hidden categories are invisible in the library and in the dialog.

## 2. Membership sets

For an entry `e`:
- `All(e)` = every `category_id` in `animes_categories` for `e` (includes hidden).
- `Visible(e)` = `All(e)` restricted to `hidden = 0` (what the upstream dialog and apply step read).

Upstream dialog uses `Visible(e)` for `common` and `mix`; apply uses `Visible(e)` as the base for the new set and then replaces **All** links with that set. This drops hidden memberships (E-07).

## 3. Dialog preselection

Inputs: selected entries `E = {e1..en}`, categories offered `C = visible categories with id != 0`.

```
common = ∩ Visible(ei)            (empty E → ∅)
mix    = (∪ Visible(ei)) \ common
for c in C:
   c ∈ common → Checked(c)        (State)
   c ∈ mix    → Exclude(c)         (TriState, shown as mixed)
   else       → None(c)            (State)
```

Worked example:
- e1 ∈ {Watching(1), Later(2)}, e2 ∈ {Later(2)}, e3 ∈ {}.
- common = ∩ = {} (e3 empty makes the intersection empty; upstream `reduce` over all lists).
- mix = {1, 2}.
- Dialog: Watching = Exclude (mixed), Later = Exclude (mixed).

Note the intersection uses `reduce` over the list of sets. With an empty entry in the list, `common` is empty.

## 4. Tri-state cycle and output

Controls:
- `State` (Checked / None): row click flips.
- `TriState` (Include / Exclude / None): Exclude → None → Include → Exclude.

Upstream initialization for a `mix` category is `Exclude` (visually indeterminate). Clicking goes to `None` (remove from all), then `Include` (add to all), then back to `Exclude` (leave as-is).

Output on OK:
- `include = {c | state is Checked or Include}`
- `exclude = {c | state is None (State) or None (TriState)}`
- Exclude-state (mixed, untouched) appears in neither list.

Apply, per entry `e`:
```
new = (Visible(e) \ exclude) ∪ include        // upstream
```
Fixed apply (T-37a-030): `new = (All(e) \ exclude) ∪ include`. Then replace all links with `new`.

## 5. Worked apply example (E-07 fix)

Entry e has links {1 (Watching, visible), 9 (Private, hidden)}. The dialog shows Watching as Checked (common to e) and Later(2) as None. The user checks Later and presses OK.

- include = {1, 2} (Watching is Checked, Later is Checked now); exclude = {} (no None rows remain that were initially checked).
- Upstream: `new = (Visible(e) \ exclude) ∪ include = ({1} \ {}) ∪ {1,2} = {1,2}`; replace all links → Private(9) is lost. **Bug.**
- Fixed: `new = (All(e) \ exclude) ∪ include = ({1,9} \ {}) ∪ {1,2} = {1,2,9}`; Private kept.

Test: T2-CAT-02.

## 6. Apply semantics when a category is not in the dialog

- Default (id 0) is never offered (E-12). Membership in Default is implicit; applying `new = {}` puts the entry in Default.
- Hidden categories are not offered (E-07). Their memberships are preserved only by the fixed apply.
- Deleted categories disappear from `All(e)` by cascade.

## 7. Category management operations

| Op | Interactor | Writes | Reads |
|---|---|---|---|
| Create | `CreateAnimeCategoryWithName` | `INSERT categories(name, sort=max+1, flags=global sort, hidden=0)` | all categories; global sort pref |
| Rename | `RenameAnimeCategory` | `UPDATE name` | — |
| Hide / show | `HideAnimeCategory` | `UPDATE hidden = !hidden` | — |
| Reorder | `ReorderAnimeCategory` (mutex) | `UPDATE sort = index` for all non-system | all non-system, in order |
| Delete | `DeleteAnimeCategory` | `DELETE` row; `UPDATE sort = index` for all (incl. system); pref cleanup | all categories |

Delete details (`domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt:17-59`):
1. Delete row. On exception → InternalError, stop.
2. Re-read all categories; build `order = index` updates.
3. If `default_anime_category == id` → delete that pref.
4. For each StringSet pref in [`animelib_update_categories`, `animelib_update_categories_exclude`, `remove_exclude_anime_categories`, `download_new_anime_categories`, `download_new_anime_categories_exclude`]: remove `"$id"` if present.
5. Apply the order updates.

Hide details: the `hidden` flag hides the category from the library and from the dialog. The management list shows it only if `hidden_categories` is off. Its entries are invisible in the library (E-08).

## 8. Category screen model

- Source of truth: `GetAnimeCategories` (all) or `GetVisibleAnimeCategories` (visible), chosen by `hidden_categories` (`AnimeCategoryScreenModel.kt:41-59`).
- System category filtered out of the list (`filterNot(Category::isSystemCategory)`).
- Every emission replaces the state with `dialog = null` (E-29). Rust fix: keep dialog if the target still exists.
- Name checks in dialogs compare against the full name list including hidden categories (the list passed to the dialog is the screen's list, which may exclude hidden categories when the setting is on). **Observed inconsistency:** with `hidden_categories` on, a duplicate of a hidden category's name is allowed. Proposal: check uniqueness against all categories (T-37a-042).

## 9. Reorder algorithm

Upstream (`ReorderAnimeCategory.kt:18-46`):
```
list = all categories where !system (in current order)
i = index of category id; if missing → Unchanged
list.add(newIndex, list.removeAt(i))       // newIndex is an index in the displayed list
write order = 0..n-1 for list
```
Displayed list equals `list` only when `hidden_categories` is off, or when there are no hidden categories; when hidden ones exist and the setting is on, the UI list is shorter than `list` and `newIndex` addresses the shorter list. **Observed inconsistency** (rare): drag indices can point at a different slot than the user sees. Rust rule: reorder over the same list the UI shows, then write orders for all non-system categories (hidden included, relative order kept).

## 10. Edge cases (summary)

| Case | Result |
|---|---|
| Apply on an entry with only hidden memberships | Visible set empty → upstream replaces all links with the delta, dropping hidden ones. Fixed: hidden kept. |
| All categories hidden | Library empty screen (E-08); dialog options empty → "You don't have any categories yet." |
| Delete the category currently shown | Pager clamps index (E-17). |
| Create with name equal to an existing hidden category, setting on | Allowed upstream (E-40 family); fixed by checking all names. |
| Reorder during an update job | Independent (job reads categories at start). |
