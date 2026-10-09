# Topic — Selection state machine and bulk actions

Companion to p1-behavior.md §B-10 and §B-11, p1-rust-design.md §R-6, p1-edge-cases-and-tests.md E-09, E-10, E-34 to E-39. Upstream paths are repo-relative to `/home/blackbox/aniyomi-src/`.

## 1. State

```
Selection = ordered list S of entries (snapshot), unique by id
mode      = (S nonempty)          // selectionMode
anchor    = last element of S     // used by range
```
Upstream stores `LibraryAnime` values (data class). Rust stores `LibraryRow` snapshots keyed by id plus an insertion order vector (design R-6).

## 2. Transitions

| Event | Precondition | Effect |
|---|---|---|
| Click item | S empty | Open entry (navigate). S unchanged. |
| Click item | S nonempty | Toggle item (add if absent, remove if present). |
| Long-press / right-click item | any | Range (below). |
| Ctrl+click (desktop extra) | any | Toggle. |
| Shift+click (desktop extra) | any | Range. |
| Select all (page) | any | Add unselected page items. |
| Invert (page) | any | Flip membership of page items. |
| Cancel / Escape | S nonempty | Clear S. |
| Escape | S empty and search active | Clear search. |
| Bulk action finished (mark, download, delete, category) | any | Clear S (upstream: immediately after launch). |
| Change page | any | S unchanged (E-10). |
| Change search or filter | any | S unchanged; items may be hidden (E-10). |

## 3. Range algorithm

Inputs: clicked row `r`, displayed rows `P` (the category page after filter, sort and search, E-34).

```
last = anchor(S)
if last is none or last.category != r.category:
    add r; done                       # single add, may start a new range
a = index of last.id in P             # by id (fix; upstream uses data-class equality, E-09)
b = index of r.id in P
if a or b missing:
    add r; done                       # fix: no crash
if a == b: done
add every p in P[min(a,b) .. max(a,b)] not already in S   # never removes
```

Examples (page P = [w, x, y, z], all in category 1):
1. S = {}, click y → add y. S = [y].
2. Click w → last = y (cat 1, same) → a = 2, b = 0 → add w, x, y(already) → S = [y, w, x].
3. The anchor is the last element of S, which is `x` (step 2 inserted `w` then `x`). Click z → a = index(x) = 1, b = 3 → range x..z = x, y, z → y and x are present, so only z is added. S = [y, w, x, z].

Note: step 3 demonstrates that the anchor is the most recently **inserted**, not the most recently clicked.

Edge: the anchor is removed by a toggle → the new last element becomes the anchor.

## 4. Select all and invert

- `select_all(P)`: append every p in P not in S, in P order.
- `invert(P)`: for p in P: if p ∈ S → remove; else append. Entries outside P are untouched.

Upstream `invertSelection` removes the page's selected entries and appends the unselected ones in list order (`AnimeLibraryScreenModel.kt:676-689`). Same result.

## 5. Bulk actions and their effects

| Action | Writes | Scope | Clears S |
|---|---|---|---|
| Mark viewed/read | `SetSeenStatus` / `SetReadStatus` per entry | all of S | yes (immediate upstream) |
| Mark unviewed/unread | same with false | all of S | yes |
| Download next N | queue via download manager, per entry: next N not downloaded or queued | all of S, skipped if any local (button hidden) | yes |
| Download all unviewed | queue all unseen not downloaded or queued | same | yes |
| Set categories | replace links per entry (topic: category membership) | all of S | yes (immediate) |
| Delete (from library and/or downloads) | favorite false; delete downloads for HTTP sources | all of S, distinct by id | yes (after confirm) |

### 5.1 Order of operations for "Download next N" (E-36)

Per entry, upstream (`AnimeLibraryScreenModel.kt:485-501`):
1. `episodes = getNextEpisodes(entry.id)` (ordered by the domain).
2. `episodes = episodes.filter { !queued(ep.id) && !downloaded(ep.name, ep.scanlator, title, source) }`
3. `episodes = episodes.take(N)` (or all for Unviewed).
4. `downloadEpisodes(entry, episodes)`.

Consequence: "Next 5" = the first five **not yet downloaded or queued** in domain order.

### 5.2 Delete dialog combinations (E-37)

| Library checkbox | Downloads checkbox | Result |
|---|---|---|
| on | off | Removed from library; downloads kept |
| off | on | Stays in library; downloads deleted (HTTP sources only) |
| on | on | Removed and downloads deleted |
| off | off | Confirm disabled |

Downloads checkbox is not offered when any selected entry is local.

### 5.3 Failure handling

- Upstream: writes run in `launchNonCancellable`; exceptions inside the interactors are logged; no UI message. Selection is cleared before the work finishes, so a failure is invisible.
- Rust (T-37a-027, E-35): clear selection after writes complete; on partial failure show "N of M updated". Per-entry failures are logged with entry id.

## 6. Interaction with search and filters

- Selection holds entries that may be hidden by the current search or filter (E-10).
- Bulk actions operate on all of S, not on the visible page. The confirm dialog for delete states the count of selected entries, including hidden ones (proposal).
- Select all and invert operate only on the displayed page (so hidden entries are never added by these two).

## 7. Keyboard and mouse matrix (desktop)

| Input | Action |
|---|---|
| Click (S empty) | open |
| Click (S nonempty) | toggle |
| Ctrl+click | toggle |
| Shift+click | range |
| Long-press | range (Android parity) |
| Right-click | range (mockup parity) |
| Ctrl+A | select all (page) |
| Ctrl+I | invert (page) |
| Escape | clear S, else clear search |
| Delete key | open delete dialog for S (proposal) |

## 8. Snapshot refresh

When the pipeline output changes (new counts, new badges), S keeps its ids. Snapshots are refreshed from the new output; ids missing from the library (removed in another window or by an update) are dropped and the count is updated. The action bar hides when S becomes empty.

## 9. Tests

- T2-SEL-01 to T2-SEL-09 (edge-case file §T-2).
- T4-UI-01 for the keyboard matrix.
- T2-BULK-01 to T2-BULK-04, T2-DEL-01 to T2-DEL-04 for bulk semantics.
