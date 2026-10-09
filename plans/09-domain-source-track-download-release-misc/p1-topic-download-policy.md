# Topic: download policy (auto-download, removal, throttling, network gating)

Deep dive for area 09. Covers the policy decisions that the downloader (areas 23, 24) consumes: which new items to download, which old items to remove, how fast to download, and when to download at all. The policy code is `domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt`, `domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt`, and `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt`. The execution (threads, files, queue) is outside this scope.

## 1. Policy overview

```
new items discovered by the library update (area 26)
  └─ filter_new(enabled = download_new / download_new_episode,
                eligible = should_download_new(favorite, categories, include/exclude),
                unread_only = download_new_unread_* ,
                new_items, existing_items)
       └─ queue for downloader (areas 23/24, concurrency = download_slots 1..5, speed = download_speed_limit KiB/s, wifi gate = download_only_over_wifi)

reader/player events (area 36/46)
  ├─ download-ahead: auto_download_while_reading / _watching ∈ {0,2,3,5,10} next unread/unseen
  └─ remove-after-read: remove_after_read_slots ∈ {-1,0,1,2,3,4}; remove_after_marked_as_read (bool)
```

## 2. Auto-download new items

### 2.1 Decision order (observed)
1. No new items → nothing.
2. Master switch off (`download_new` / `download_new_episode`, default false) → nothing.
3. Entry not in library (`favorite = false`) → nothing.
4. Category rule (table in section 2.2) → nothing when false.
5. Unread-only off → everything new is queued.
6. Unread-only on → drop new items whose number matches a read, recognised item of the same entry.

### 2.2 Category rule (observed, `FilterChaptersForDownload.kt:52-67`)

| # | Included set | Excluded set | Entry categories | Result |
|---|---|---|---|---|
| 1 | empty | empty | any | download |
| 2 | any | any | in excluded | skip |
| 3 | empty | non-empty | not in excluded | download |
| 4 | non-empty | any | not in excluded and in included | download |
| 5 | non-empty | any | not in excluded and not in included | skip |

Notes:
- Excluded wins over included (row 2 is checked before row 4).
- An entry with no categories counts as the default category `0`. The default category is `UNCATEGORIZED_ID = 0` (`domain/src/main/java/tachiyomi/domain/category/model/Category.kt:16`), so "default category" is a real choice the user can include or exclude.
- Rows 3 and 4 need at least one of the sets to be non-empty; row 1 is the default configuration.

### 2.3 Unread-only (observed)
- Read set: items with `read = true` (or `seen = true`) and `number >= 0`.
- A new item is dropped when its number is in the read set.
- Compared as numbers, not identities (Q-DL-1): a source that renumbers a read chapter suppresses the new one. Kept for parity; the Proposal (E-DL-09) compares `f32` bit patterns.
- Items with unrecognised numbers (`< 0`) never suppress and are never suppressed (E-DL-10).

## 3. Remove after read

| Value | Meaning (UI label) | Target chosen when a chapter is marked read |
|---|---|---|
| -1 | Disabled | none |
| 0 | Last read chapter | the chapter just read |
| 1 | Second to last | `index - 1` (the chapter before the one just read) |
| 2 | Third to last | `index - 2` |
| 3 | Fourth to last | `index - 3` |
| 4 | Fifth to last | `index - 4` |

Observed rule (`ReaderViewModel.kt:503-519`): `target = chapterList.getOrNull(indexOf(current) - slots)`; `null` → nothing. The labels in the UI (`MR.strings.last_read_chapter`, `second_to_last`, `third_to_last`, `fourth_to_last`, `fifth_to_last`) describe the slot number, so "second to last" means one chapter before the just-read one. The Kotlin code is consistent with that reading.

Proposal (E-DL-04): any negative value other than -1 is treated as disabled. Kotlin would compute `index + 2` for -2 and delete a later chapter.

`remove_after_marked_as_read` (default false) is a separate switch that removes a download as soon as the item is marked read (`SetReadStatus.kt:49`, `SetSeenStatus.kt:49`).

Exclusions: `pref_remove_bookmarked` (default false) keeps bookmarked items; `remove_exclude_categories` / `remove_exclude_anime_categories` (empty sets) keep items of listed categories.

## 4. Download speed and slots

- `download_slots` (default 1, UI 1..5): number of concurrent item downloads (executed by area 23/24).
- `download_speed_limit` (default 0, unit KiB/s, UI dialog `DownloadLimitDialog`): per-download throttle, `bytes_per_second = limit × 1024`. Zero means no throttle (throttler convention; verify in area 23).
- Rust: `i64` multiplication (TC-DL-09); negative → 0 (E-DL-02).

## 5. Network gating

- `download_only_over_wifi` (default true): the download job pauses on metered or cellular links. Linux replacement: `gio::NetworkMonitor` `is_network_metered()` and `connectivity()`; on a desktop with no metered flag the gate is open (documented limitation, T-09a-021).
- Changes to the flag re-evaluate the job (observed: `MangaDownloadJob.kt:72`, `AnimeDownloadJob.kt:72`).

## 6. Download-ahead

- Values `0, 2, 3, 5, 10` (UI). `0` disables. Otherwise the next N unread chapters or unseen episodes from the current position are queued.
- Defaults: `auto_download_while_reading = 0`, `auto_download_while_watching = 0`.
- Consumers: `ReaderViewModel.kt:226`, `PlayerViewModel.kt:1112`. Execution is area 36/46 plus area 23/24.

## 7. Drop and keep decisions

| Item | Decision | Reason | User loses |
|---|---|---|---|
| External downloader (`use_external_downloader`, `external_downloader_selection`) | Drop UI; keep keys for backup parity (T-09a-022) | it hands downloads to other Android apps by package name | using a third-party Android download app |
| `DiskUtil.createNoMediaFile` (downloads folder) | Drop (T-09a-050) | Android media scanner only | nothing on Linux |
| Wi-Fi gate | Keep, Linux meter detection (T-09a-021) | parity | on machines without a metered flag, the gate never closes |
| Download speed UI units | Keep KiB/s | parity with backups | none |

## 8. Decisions for the user

1. Remove-after-read "second to last" semantics: keep Kotlin (one chapter before the just-read one) or shift to "keep last N"? Proposal: keep.
2. Should the Wi-Fi-only gate be advisory on Linux (warn only)? Proposal: enforce when `gio` reports metered, otherwise ignore.
3. Concurrency default: Kotlin default is 1 slot. Confirm 1 for v1.
