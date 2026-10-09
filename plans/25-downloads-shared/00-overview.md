# 25-downloads-shared: overview

Area lead overview for the shared download infrastructure. It covers the two delete interactors, the download preference store, the "Download queue" page (DownloadsTab), the queue view models and the on-disk naming rules that the delete path depends on. The engine itself (downloader, page fetching, job scheduling) belongs to area:23-downloads-manga and area:24-downloads-anime.

All repo-relative paths are relative to `/home/blackbox/aniyomi-src/`. `OBSERVED` means what the Kotlin does. `Proposal:` means what the Rust port should do.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

Aniyomi downloads manga chapters and anime episodes to local storage so they can be read or watched offline. This area holds the parts that do not belong to one media kind:

- **Delete path.** When a user marks an episode or chapter as seen or read, and the "remove after marked as read" preference is on, the downloaded files are removed. Two interactors start that path: `app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt` and `app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt`.
- **Preference store.** `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt` holds all 23 download preferences and the six category-set keys used by backup restore.
- **Queue page.** `app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt` is the "Download queue" screen (tab index 6, line 86). It has two pages (Anime, Manga), each with a source-sectioned list of queued downloads, a Start/Stop or Pause/Resume button, sort, Cancel all, and per-row move and cancel actions.

Verified in source: the delete interactor is a two-line guard around a manager call. `deleteEpisodes` and `deleteChapters` both run inside `launchIO`, so the Kotlin caller returns before any file is removed (`AnimeDownloadManager.kt:269-270`, `MangaDownloadManager.kt:264-265`).

### 1.2 Why it matters for a usable Kurobayomi

- Without the preference store, no other download behaviour can be configured. Every engine and reader consumer reads these keys.
- Without the delete path, "remove after read" does nothing, and disk use grows with every finished title.
- Without the queue page, a user has no view of what is downloading and no way to reorder or cancel work.

This area is P1 by the brief's definition (downloads are parity). Its only P0 task is the preference store (T-25a-001). Downloading itself is owned by areas 23 and 24. Kurobayomi can read manga and watch anime from a source without this area, but it cannot run offline reading or the auto-cleanup that users rely on.

### 1.3 Out of scope (owned elsewhere)

- Download engine, queue persistence, page and video fetching: area:23-downloads-manga, area:24-downloads-anime.
- Entry-screen download menus and manual "delete download" (`download_button`, and the entry-level download actions): area:38-ui-entry-details-manga, area:39-ui-entry-details-anime.
- Settings screen for download preferences (`SettingsDownloadScreen.kt`): area:47-settings-screens-general.
- Category deletion, which reads the category-set preferences: area:08-domain-library-category-history-updates.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Package or file (repo-relative) | Responsibility | Owner area |
|---|---|---|
| `app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt` | Guarded call to `AnimeDownloadManager.deleteEpisodes` for an anime's episodes | this area |
| `app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt` | Same for manga chapters | this area |
| `app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt` | Queue page shell: title, count pill, pager, FAB, sort and overflow menus | this area |
| `app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/` and `.../manga/` | Queue screen models, screens, holders, adapters, headers and items | this area (queue model and row UI) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueTab.kt`, `.../manga/MangaDownloadQueueTab.kt` | Pager page wrappers; each creates a second model instance (E-36) | this area |
| `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt` | Typed access to 23 preference keys with defaults; 6 category-set keys | this area |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt` | Manager: delete filter, queue removal, directory deletes, cache updates, start/stop/clear | area:24-downloads-anime |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt` | Same for manga | area:23-downloads-manga |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt`, `.../manga/MangaDownloader.kt` | Queue state, downloader job, status and progress flows; manga speed limit at `MangaDownloader.kt:525` | areas 23 and 24 |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt`, `.../manga/MangaDownloadProvider.kt` | Directory naming and lookup (`findEpisodeDirs`, `getValidEpisodeDirNames`) | areas 23 and 24 |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt`, `.../manga/model/MangaDownload.kt` | Queue item types; `State` enums with values 0 to 4 | areas 23 and 24 (state enum used here) |
| `core/common/src/main/java/tachiyomi/core/common/preference/` (`Preference.kt`, `AndroidPreferenceStore.kt`, `InMemoryPreferenceStore.kt`) | Preference store contract | area:02-preferences-system |
| `core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt` | `launchIO` (line 32, 49), `withNonCancellableContext` (line 65) | area:13-core-util-storage-archive-torrent |
| `app/src/main/res/menu/download_single.xml` | Row menu resource (six items) | this area |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt` | Settings UI for the download keys | area:47-settings-screens-general |
| `app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt`, `.../chapter/interactor/SetReadStatus.kt` | Callers that trigger delete after mark-as-read | area:07-domain-entries-items |
| `domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt`, `.../episode/interactor/FilterEpisodesForDownload.kt` | New-item auto-download filters (read `download_new*` keys) | area:23 and area:24 |
| `app/src/main/java/eu/kanade/domain/DomainModule.kt` | DI registration (lines 339, 351) | area:10-app-domain-layer |
| `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Tabs.kt` | `TabText` badge pill (line 28) | area:49-theme-and-design-system |
| `i18n/src/commonMain/moko-resources/base/strings.xml`, `i18n-aniyomi/...` | User-visible strings | area:51-i18n |

### 2.2 Proposed Rust module map

Crate `kurobayomi` (edition 2024). The existing dependencies are `gtk` (gtk4 0.11.5, feature `v4_16`), `serde` and `serde_json`. This area adds `tokio`, `async-channel`, `thiserror`, `tracing`, and reuses `rusqlite` from area:02 (see `plans/25-downloads-shared/p1-rust-design.md` section 3).

| Module | Contents | Pure (unit-testable without GTK) |
|---|---|---|
| `src/download/mod.rs` | Re-exports only | n/a |
| `src/download/kind.rs` | `EntryKind { Anime, Manga }`; `DownloadState` (`#[repr(i64)]`, 0 to 4) | yes |
| `src/download/prefs.rs` | `keys` module with the 23 key strings and the 6-entry `CATEGORY_KEYS`; typed accessors | yes (keys and defaults) |
| `src/download/policy.rs` | `DeletePolicy`, `DeleteCandidate`, `select_for_delete`, remove-after-read helper | yes |
| `src/download/queue.rs` | `QueueItem`, `SortKey`, `Section`, `QueueModel` (sections, stable sort and reverse, move, series moves, removals) | yes |
| `src/download/delete.rs` | `DeleteDownloads` service: source check, filter, queue removal, FS removal, cache, empty cascade, per-series lock | no (uses tokio) |
| `src/download/store.rs` | `DownloadStore` trait and filesystem implementation | partly (tests use tempdir) |
| `src/download/engine.rs` | `QueueEngine` trait (implemented by areas 23 and 24) | no |
| `src/download/tab.rs` | GTK page: header, Notebook (Anime, Manga), FAB, sort and cancel menus | no |
| `src/download/row.rs` | GTK row: progress, move and cancel menu, drag and drop | partly (`can_move_top(index)`, progress throttling logic) |

Current mockup entry points that this replaces: `src/screens.rs::downloads()` (line 354) and `src/screens.rs::download_button()` (line 134). The spec entry is `spec/app.json`, screen id `download_queue` (line 732). `screens.rs:25` dispatches to `downloads()`.

---

## 3. Reading guide

All documents are in this folder. Read them in this order.

1. [p1-inventory.md](p1-inventory.md): scope files, line counts, port decision for each file, callees owned by other areas.
2. [p1-behavior.md](p1-behavior.md): OBSERVED behaviour of the delete interactors, delete filter, queue tab, queue model, row menu and concurrency.
3. [p1-data-and-config.md](p1-data-and-config.md): preference table (23 keys, defaults, readers), model types, category-set keys, delete data path and storage facts.
4. [p1-topic-queue-model.md](p1-topic-queue-model.md): the view-side queue model as one diagram: flat list versus sections, the exact list transform of each operation, and the engine calls.
5. [p1-rust-design.md](p1-rust-design.md): module layout, Rust types, the `QueueEngine` trait, the delete algorithm, the store trait, GTK widget plan, Android-to-Linux mapping and pitfalls.
6. [p1-ui-parity.md](p1-ui-parity.md): the visible surface of the queue page, its states, row menu, sort menu, keyboard proposals, and the gap list against the existing mockup.
7. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases E-01 to E-38, failure modes, race table, and the test plan (T-E ids, manual M-01 to M-07, fixtures F-01 to F-05).
8. [p1-tasks.md](p1-tasks.md): 35 implementation tasks (T-25a-001 to T-25a-040) with priority, effort, dependencies, sources and acceptance criteria.

---

## 4. Key design decisions and open questions

### 4.1 Decisions already made in the docs (recommendation: keep)

| # | Decision | Reference | Why |
|---|---|---|---|
| D1 | `await_items` awaits the file removal. Kotlin does not (fire-and-forget). | E-03, rust-design section 5 | The UI can refresh row state deterministically. Work runs on `spawn_blocking`, and the GTK side uses `glib::spawn_future_local`, so the main loop does not block. |
| D2 | `QueueEngine::reorder(ids)` treats the argument as an ordering over the live queue. Unknown ids are ignored, live items not listed keep their order, and `reorder([])` is a no-op. Kotlin `updateQueue` replaces the queue and `[]` clears it. | E-15, T-25a-017 | A stale snapshot can otherwise resurrect or drop items. **Needs agreement from areas 23 and 24.** |
| D3 | Keep the same preference key strings and defaults, including the inverted `pref_remove_bookmarked` (default `false` protects bookmarked items). | p1-data-and-config section 3, section 3.1 | Backups restore 1:1 (area:21, area:22). |
| D4 | Keep the sort quirk: stable sort, then reverse. Equal keys come out reversed for "Newest" and "Descending". | E-07 | This is what users see today. |
| D5 | Keep the section model: sort never moves items across sources, and series moves act on the flat list (sections can reorder). | E-08, E-09 | Matches Kotlin. |
| D6 | Keep permanent deletion, no trash. | p1-data-and-config section 6 | Matches Aniyomi. |
| D7 | Tab badge hidden at 0 (Kotlin shows "0"). | E-14, T-25a-022 | Small, deliberate deviation. |
| D8 | Row "Move to top" and "Move to bottom" visibility uses the flat queue index, not the adapter position with headers. | E-10, T-25a-027 | Kotlin hides "Move to top" on the wrong rows (flat position includes headers). |
| D9 | Manga progress polling stops after 600 ticks of 50 ms (30 s) with a warning. | E-13, T-25a-029 | Kotlin polls forever if pages never load. |
| D10 | Per-series async lock keyed by `(kind, entry id)` for the removal steps. | E-22, T-25a-013 | Two rapid "mark all read" clicks can race on the empty-directory cascade. |

### 4.2 Open questions for the user (each with a recommendation)

| # | Question | Recommendation | Why it needs the user |
|---|---|---|---|
| Q1 | Honour `download_slots` (Int, default 1, UI offers 1 to 5) as the parallel download count? | Yes, clamp to 1 to 5, implemented by area:23 and area:24. | No code in the Kotlin tree reads it (E-12, verified: only `SettingsDownloadScreen.kt:102` and the definition at `DownloadPreferences.kt:69`). The setting is dead upstream. Honouring it changes engine behaviour. |
| Q2 | Apply `download_speed_limit` to anime downloads as well? | Yes, same `value * 1024` bytes/s conversion. | Verified: the only reader is `MangaDownloader.kt:525`. The anime downloader ignores the limit, although the setting is on the shared Download settings screen. This is a parity bug in Aniyomi, so the user chooses between copying the bug and fixing it. |
| Q3 | When a source is uninstalled, leave its downloads on disk (Kotlin behaviour, E-02)? | Yes, log a warning with entry and source id. Add a P2 "Clean orphaned downloads" action in the storage screen (area:42). | Leftover files cost disk space and the user will not see them. |
| Q4 | Keep permanent deletion for "remove after read" and the delete actions? | Yes. | Trash would need `gio trash` and a restore path. Not in Aniyomi. |
| Q5 | Allow dragging a row into another source's section? | P1: allow, as Kotlin. P2: refuse with a visual cue. | The flat order is sent to the engine, but the next snapshot regroups the item back into its own source (E-34). That is confusing to a user. |
| Q6 | Keyboard shortcuts for the queue (Alt+Up / Alt+Down move, Delete cancels, Space start or pause)? | Yes, add in P1 (T-25a-026). | Kotlin is touch-only. These are desktop proposals, not parity. |
| Q7 | Cancel all with no confirmation (Kotlin behaviour)? | Keep no confirmation, disabled when the queue is empty. | A confirmation is a small change, but it differs from Aniyomi. |
| Q8 | Keep the title count pill as anime-only (Kotlin behaviour)? | Keep for parity. Manga count appears only on the tab badge. | Showing both is a UI choice. |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Needs | From | Used by | Notes |
|---|---|---|---|
| `PrefStore`, typed `Pref<T>` with `get`, `set`, `watch` | area:02-preferences-system | T-25a-001 | Keys must be stored unchanged. |
| Seen and read call sites that invoke `await_items` with changed items only, grouped by parent | area:07-domain-entries-items | T-25a-014 | Contract: only items changed by this call (E-06), partially watched items included (E-06b). |
| Category lookup per entry id (`getCategories.await(id)`, empty becomes `[0]`) | area:08-domain-library-category-history-updates | T-25a-012, T-25a-013 | Only the category ids are read. |
| Category-set key list in backup restore (`PreferenceRestorer.kt:136`) and migration cleanup (`CategoryPreferencesCleanupMigration.kt:16`) | area:21-backup-format-and-create, area:22-backup-restore, area:11-app-migrations | T-25a-006 | Non-numeric ids handled in E-05. |
| The engine: `queueState` as a watch, `start`, `stop`, `pause_and_stop`, `clear`, `reorder`, `cancel`, `remove_from_queue`, status and progress flows | area:23-downloads-manga, area:24-downloads-anime | T-25a-017, T-25a-013, T-25a-029 | The trait in rust-design section 2.5 is the contract. Areas 23 and 24 must implement it (D2, Q1, Q2). |
| Directory naming: `getValidEpisodeDirNames` and `getValidChapterDirNames`, plus `DiskUtil.buildValidFilename` | area:23-downloads-manga, area:24-downloads-anime | T-25a-013 | See section 6.3. |
| Download cache: `removeEpisodes`, `removeChapters`, `removeAnime`, `removeManga`, `removeSource` | area:23, area:24 | T-25a-013 | Cache is rebuilt from disk at startup (E-23). |
| Sidebar or stack slot for the page (index 6), header bar, back action | area:36-ui-shell-navigation-security-webview | T-25a-021 | |
| Reader and player consumers of `remove_after_read_slots` | area:43-reader-core, area:45-player-core | T-25a-036 | The helper lives in `download::policy`. |
| DI registration (`DomainModule.kt:339, 351`) | area:10-app-domain-layer | T-25a-010, T-25a-011 | Constructor injection in Rust. |
| Settings UI for all keys | area:47-settings-screens-general | T-25a-002 to T-25a-006 | This area stores keys; area:47 renders them. |
| Entry-level download and delete menus (call `await_items`) | area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | T-25a-016 | |
| Theme tokens and fonts | area:49-theme-and-design-system | T-25a-039 | GTK theme CSS, not libadwaita. |
| Strings | area:51-i18n | T-25a-021 to T-25a-030 | English base strings only in this area. |

### 5.2 What others need from this area

| Consumer | Gets from this area |
|---|---|
| area:23-downloads-manga, area:24-downloads-anime | `policy::select_for_delete`, `download::kind::DownloadState` (numeric values), `QueueModel` ordering rules (for reorder semantics), `QueueEngine` trait (their implementation target) |
| area:07-domain-entries-items | `DeleteDownloads::await_items(entry, items, names) -> Result<usize, DeleteError>` |
| area:37 to area:41 (library and entry screens) | Same delete API; the queue page is reachable from them |
| area:36-ui-shell-navigation-security-webview | `DownloadsPage::new(engine)` as a sidebar destination |
| area:43-reader-core, area:45-player-core | `remove_after_read_slots` helper (`policy::remove_after_read_target`) |
| area:47-settings-screens-general | Typed `DownloadPrefs` struct and the key table with defaults |
| area:21, area:22 | `CATEGORY_KEYS` (six keys, same order as `DownloadPreferences.kt:121-128`) |
| area:42-ui-updates-history-stats-storage-upcoming | Optional orphan-cleanup action (Q3, P2) |

---

## 6. Task summary

### 6.1 Counts (from `plans/TASKS.csv`, rows with file starting `25-downloads-shared/`)

Total: 35 tasks.

| Priority | Count | Efforts |
|---|---|---|
| P0 | 1 | M: 1 (T-25a-001) |
| P1 | 27 | S: 14, M: 12, L: 1 (T-25a-026) |
| P2 | 6 | S: 5, M: 1 (T-25a-028) |
| P3 | 1 | S: 1 (T-25a-034) |

Effort totals: S 20, M 14, L 1, XL 0.

### 6.2 P0 critical path, in order

1. T-25a-001: typed download preference store (depends on area:02-preferences-system). This is the only P0 task in this area. It is also the root of every other preference task.

Area:23 and area:24 must also deliver their P0 engine before the queue page can show real data. Within this area, the next items are in the order that unblocks daily use:

2. T-25a-020: pure `QueueModel` (no dependencies). Lets the page be built and tested without GTK or the engine.
3. T-25a-021: page shell (needs area:36). Depends on T-25a-020.
4. T-25a-026: queue list with sections, rows and empty state (L). Depends on T-25a-020 and T-25a-021.
5. T-25a-033: replace the mockup page in `src/screens.rs` and `spec/app.json`. Depends on T-25a-021, 023, 024, 025, 026.

For delete, in order: T-25a-012 (pure filter), T-25a-017 (engine adapter, needs areas 23 and 24), T-25a-013 (pipeline), T-25a-010 and T-25a-011 (interactors), T-25a-014 (consumer hook, needs area:07), T-25a-015 and T-25a-016 (no-op and await semantics), then T-25a-035 (test suite) last.

### 6.3 Storage layout facts (spot-checked, not fully specified in the folder)

The docs refer to "the shared rule in p1-rust-design.md section 5" for directory layout. That section does not contain it. Section 6 of the same file only refers to a provider rule. The layout, as read from `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt`:

- Root: `downloadsDir`, then source directory, then anime directory, then episode directory.
- Source directory name: `DiskUtil.buildValidFilename(source.toString())` (`AnimeDownloadProvider.kt:115-117`).
- Anime directory name: `DiskUtil.buildValidFilename(animeTitle)` (`:124-126`).
- Episode directory name: `DiskUtil.buildValidFilename("${scanlator}_$name")` when the scanlator is not blank, otherwise `DiskUtil.buildValidFilename(name)`. A blank name becomes `"Episode"` (`:134-152`).
- Lookup candidates: the new name first, then the old name `DiskUtil.buildValidFilename("${scanlator}_$name")` without the blank fallback (`:161-167`, `:181-185`). The first match wins per item (`:101-107`).

Manga follows the same pattern in `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt`, with three differences:

- Chapter directory name: `DiskUtil.buildValidFilename("${scanlator}_$name")` or `buildValidFilename(name)`, with a blank name becoming `"Chapter"` (`:134-152`).
- Lookup candidates are `[chapterDirName, "$chapterDirName.cbz"]` (`:166-177`). The CBZ archive candidate is the one that matters for "Save chapters as CBZ" (`save_chapter_as_cbz`, default `true`). Anime has no archive candidate.
- The old-name candidate used by anime (`getOldEpisodeDirName`) has no manga equivalent in the lookup list.

Gap for areas 23 and 24: `DiskUtil.buildValidFilename` itself is not described in any document in this folder. Area 24 (and 23 for manga) must specify it, or the docs must point to where it is specified. Area 13 (`plans/13-core-util-storage-archive-torrent`) is the likely owner.

Whether this layout must be compatible with Aniyomi installs is a question for the user. It matters only if the user copies an Android download folder over. If so, the layout above must be kept exactly.

### 6.4 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Engine semantics differ (D2, reorder replaces vs ids-as-ordering) | Items resurrect or disappear from the queue after reorder | Agree the trait in `plans/25-downloads-shared/p1-rust-design.md` section 2.5 with areas 23 and 24 before T-25a-017 is built. |
| `download_slots` and the anime speed limit are unread upstream (Q1, Q2) | Engine work may be wasted, or settings may silently do nothing | Decide Q1 and Q2 before area 23 and 24 implement concurrency and limits. |
| No upstream tests exist for this area (verified: no download-related `*Test*.kt`) | Parity rests on reading the Kotlin, not on test evidence | All tests in p1-edge-cases-and-tests.md section 5 are new. Write the fakes (F-01 to F-05) first. |
| Delete races and partial removals (E-22, E-23) | Empty series or source dir left behind, or a cache disagreeing with disk | Per-series lock; idempotent per-item removal; cache rebuild at startup (area 23, 24). |
| Orphaned downloads when a source is removed (E-02) | Disk use grows silently | Warning log now; P2 cleanup action (Q3). |
| GTK 4.16 widget gaps: no swipe on `gtk::Notebook`, FAB label collapse needs `gtk::Revealer` | Layout polish and touchpad swipe | Swipe is P2 (T-25a-037). Use `Revealer` for the label. |
| Pill and badge text sizes and alpha (`alpha(@theme_fg_color, 0.08)`) | Visual mismatch with the user's Tokyo Night theme | T-25a-039 verifies in the user's theme. |

### 6.5 Doc hygiene findings (not blocking)

- `p1-data-and-config.md` section 5.3 points to `p1-rust-design.md` section 5 for the layout rule. That section does not contain it (see 6.3).
- `p1-behavior.md` section 1.5 step 4 leaves Kotlin's directory delete semantics to `UniFile.delete()`, and proposes a recursive remove for Rust. This is a deliberate change; it is not recorded in the edge-case list (E-23 covers partial removal only).
- Manga directory naming (CBZ candidate, "Chapter" fallback) is not described anywhere in this folder. It is recorded in 6.3 above and should be copied into area 23's docs.
- Dependency tokens in `p1-tasks.md` all resolve to real area ids. `tasks.py lint` prints `ok` for the file. No `area:` token needed changing.

---

## 7. Definition of done for this area

Each item is observable in the running Kurobayomi build.

1. **Preferences.** Every one of the 23 keys reads its default value on a fresh database, with the exact key strings in the table in `plans/25-downloads-shared/p1-data-and-config.md`. Changing one value persists across restart and is visible to the engine through `watch`.
2. **Category sets.** The six category-set keys hold numeric ids as strings. A non-numeric value makes the delete path log an error and do nothing for that call; no crash.
3. **Mark as read with "remove after marked as read" on.** Marking one episode or chapter seen or read removes its downloaded directory, and the row leaves the queue. Marking an already-read item again does nothing to its files.
4. **Filter.** For an entry in an excluded category, watched items are deleted and unwatched items are not. Bookmarked items are kept unless "remove bookmarked" is on. An entry with no categories uses category 0.
5. **Unknown source.** Deleting downloads for a source that is not installed leaves files on disk, shows no error, and writes a warning log naming the entry and source ids.
6. **Empty cascade.** When the last item in a series is removed, the series directory is removed. When the source directory is then empty, it is removed too.
7. **Queue page.** The "Download queue" page shows Anime and Manga pages with tab badges (hidden at 0). The title shows a count pill for anime only, when the anime count is above 0.
8. **Sections.** Queue rows are grouped into one section per source, in the order each source first appears in the queue. Section headers show the source name and item count.
9. **Start and pause.** The Anime button reads "Continue" when stopped and "Stop" when running. The Manga button reads "Resume" when stopped and "Pause" when running. The icons change with the state. The button is hidden when the page list is empty.
10. **Sort.** The Sort menu offers "By upload date" (Newest, Oldest) and "By episode number" or "By chapter number" (Ascending, Descending). Sorting reorders only within each source section, and the engine downloads in the sorted order.
11. **Row menu.** The row menu shows "Move to top", "Move series to top", "Move to bottom", "Move series to bottom", "Cancel", "Cancel all for this series" in that order. "Move to top" is hidden on the first queue row. "Move to bottom" is hidden on the last queue row.
12. **Cancel all.** "Cancel all" empties the queue and stops downloads without a confirmation dialog. The action is disabled when the queue is empty.
13. **Progress.** Anime rows show "Downloading…" at 0 percent and "NN%" after. Manga rows show "done/total" (for example "12/30") and an empty text while the page list is unknown. A row in error stops updating.
14. **Empty state.** Each page shows "No downloads" when its list is empty, and no FAB, sort or cancel-all.
15. **Reorder.** After a reorder, the engine downloads items in the new order. A reorder never adds or removes queue items (E-15).
16. **Responsiveness.** A slow delete (200 ms) does not freeze the window. The row updates when the delete finishes.
17. **Tests.** `cargo test download::` passes all test ids in `plans/25-downloads-shared/p1-edge-cases-and-tests.md` section 5. `cargo clippy --all-targets -- -D warnings` is clean.
18. **Mockup gone.** `src/screens.rs::downloads()` no longer shows "Source A" or "Source B" fake rows, and `spec/app.json` screen `download_queue` matches the toolbar and tabs in `p1-rust-design.md` section 7.
