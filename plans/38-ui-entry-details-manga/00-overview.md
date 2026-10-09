# Area 38: Manga entry details screen and ScreenModel (overview)

Area id: `38-ui-entry-details-manga`. Scope: the manga details page (`MangaScreen`), its `MangaScreenModel`, the chapter list engine (filter, sort, missing-chapter markers, selection, bulk actions), chapter row and download indicator, the info header, action row, description and tags, the chapter settings sheet, the duplicate / category / interval / scanlator / delete dialogs, the full cover window and its model, and the tracking sheet with its sub-screens.

All source paths are repo-relative to `/home/blackbox/aniyomi-src`.

Status of this overview: written by the area lead from the two part documents, their inventories, decision tables and task files. Per-part docs were not re-derived line by line. Verification done for this overview is listed in section 6.4.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

The manga details page is the hub for one manga. It shows:

- An info header: backdrop, cover, title, source name, author, artist, status, description, and genre tags.
- An action row: library toggle, fetch interval, tracking, WebView.
- A chapter list with a header count, missing-chapter markers, per-row read / bookmark / download state, and swipe actions.
- A bottom bar for bulk actions on selected chapters.
- A Filter / Sort / Display sheet whose values are stored as bit flags on the manga row.
- Dialogs for category choice, duplicate detection, migration, fetch interval, delete downloads, scanlator exclusion, and the full cover.
- A tracking sheet with per-tracker rows and pickers (status, chapter, score, start and finish dates) and a tracker search.

The ScreenModel, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt` (1243 lines), owns the state. The presentation layer is `app/src/main/java/eu/kanade/presentation/entries/manga/` and `app/src/main/java/eu/kanade/presentation/entries/components/`.

### 1.2 Why it matters for a usable Kurobayomi (PROPOSAL)

Every reading path starts here. Library, history, updates, browse, and the reader title all open this page (see section 5). Without it the reader has no chapter list, "continue reading" has no entry point, and downloads have no bulk action. The chapter filter and sort rules are also read by the reader (`skip_filtered`, see `catalogs/preferences-08.md:40`), so the filter-flag evaluation has to exist as one shared function.

### 1.3 Scope boundaries

In scope: 25 Kotlin files, 6,575 LOC in total (`python3 plans/_tools/areas.py files 38-ui-entry-details-manga`). Part 1 covers 19 presentation and route files (4,377 LOC). Part 2 covers 6 ScreenModel, tracking and chapter-utility files (2,198 LOC).

Out of scope, owned elsewhere (see section 5): the SQL schema and repositories (area:05, area:03), the domain interactors (area:07), downloads (area:23, area:25), tracker backends (area:27 to area:33), covers cache (area:34), shell and navigation (area:36), library screen (area:37), and anime details (area:39).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities (OBSERVED)

| Package or directory (repo-relative) | Files in scope | Responsibility |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/` | `MangaScreen.kt` (413), `MangaScreenModel.kt` (1243), `MangaCoverScreenModel.kt` (165) | Route screen, dispatch of dialogs and callbacks, search and navigation rules, the ScreenModel and its State and Dialog types, cover model |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/` | `MangaTrackInfoDialog.kt` (874), `MangaTrackItem.kt` (6) | Tracking home list, status / chapter / score / date pickers, tracker search, tracker removal (all as Voyager screens with small ScreenModels) |
| `app/src/main/java/eu/kanade/presentation/entries/manga/` | `MangaScreen.kt` (853), `ChapterSettingsDialog.kt` (269), `DuplicateMangaDialog.kt` (127), `ScanlatorFilterDialog.kt` (136) | Compose presentation root (small and large layouts), settings sheet, duplicate sheet, scanlator dialog |
| `app/src/main/java/eu/kanade/presentation/entries/manga/components/` | `MangaInfoHeader.kt` (688), `MangaChapterListItem.kt` (242), `ChapterDownloadIndicator.kt` (250), `MangaCoverDialog.kt` (208), `BaseMangaListItem.kt` (63) | Info header and action row, chapter row, download state indicator, full cover window, generic 76 px row |
| `app/src/main/java/eu/kanade/presentation/entries/components/` | `EntryToolbar.kt` (171), `EntryBottomActionMenu.kt` (387), `ItemHeader.kt` (78), `ItemsDialogs.kt` (172), `MissingItemCountListItem.kt` (52), `ItemCover.kt` (55), `DotSeparatorText.kt` (25) | Shared (manga and anime) top bar, bulk bar and library bottom bar, list header, delete and interval dialogs, cover ratio component, separators |
| `app/src/main/java/eu/kanade/presentation/entries/` | `EntryScreenConstants.kt` (23) | Shared enums: `DownloadAction`, `EditCoverAction`, `EntryScreenItem` |
| `app/src/main/java/eu/kanade/domain/...` and `app/src/main/java/eu/kanade/tachiyomi/util/chapter/` | `ChapterFilterDownloaded.kt` (27), `ChapterGetNextUnread.kt` (33), `ChapterRemoveDuplicates.kt` (15) | Pure chapter list helpers: downloaded-only filter, next unread, one chapter per number (used by the reader) |
| `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt` | (read, owned by area:07) | Chapter flag constants (`CHAPTER_SHOW_*`, `CHAPTER_SORT*`, `CHAPTER_DISPLAY*`) |
| `domain/src/main/java/tachiyomi/domain/items/chapter/service/` | (read, area:07) | `ChapterSorter.kt` (`getChapterSort`), `MissingChapters.kt` (`calculateChapterGap`) |
| `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` | (read, area:02) | Chapter default keys, swipe action keys |
| `data/src/main/sqldelight/data/` | (read, area:05) | `mangas.sq`, `chapters.sq`, `excluded_scanlators.sq`, `manga_sync.sq` |

### 2.2 Rust module map (PROPOSAL, conflict to resolve)

The two part documents disagree on the crate layout. This is open question Q1 in section 4.

- Part 2 (`plans/38-ui-entry-details-manga/p2-rust-design.md`, section 1) proposes a Cargo workspace: `crates/kuro-platform`, `crates/kuro-chapters` (pure logic: flags, sort, filter, missing, selection, next unread, dedupe, download pick), `crates/kuro-core` (reducer and use-cases: `detail/`, `tracking/`, `errors.rs`, `strings.rs`, `ports.rs`), `crates/kuro-db` (rusqlite, owned by area:03 and area:05), and an `app/` member holding GTK views.
- Part 1 (`p1-rust-design.md`, section 1) proposes one crate with `src/details/` (`model.rs`, `chapters.rs`, `flags.rs`, `compare.rs`, `numfmt.rs`, `text.rs`, `download.rs`, `swipe.rs`, `view/`, `dialogs/`, `cover_model.rs`, `store.rs`).

Recommendation: adopt the part 2 workspace. The cross-platform addendum (`plans/addenda/requirements-cross-platform.md`, cited in `p2-rust-design.md` line 3) requires no `cfg(target_os)` outside `kuro-platform`, and the pure crates must not import `gtk`, `glib`, `tokio::fs`. A single crate would make that rule hard to enforce.

Mapping of part 1 paths into the part 2 workspace (PROPOSAL):

| Part 1 path (`src/details/...`) | Target in workspace | Note |
|---|---|---|
| `flags.rs` | `kuro-chapters/src/flags.rs` | Same bit layout |
| `compare.rs` | `kuro-chapters/src/sort.rs` | Same comparator table |
| `chapters.rs` (`process`, `with_missing_markers`, `missing_chapters_count`) | `kuro-chapters/src/missing.rs`, `filter.rs` | Split |
| `numfmt.rs` | `kuro-chapters` (new `numfmt.rs`) | Not in part 2; add |
| `text.rs` (description whitespace, tag normalisation, status map, next-update text) | `kuro-core/src/detail/text.rs` (new) | Not in part 2; add |
| `download.rs` | `kuro-core/src/detail/download.rs` | Same name in part 2 |
| `swipe.rs` | none | Unassigned; add to `kuro-core/src/detail/swipe.rs` |
| `model.rs` (`EntryDetailsModel`) | `kuro-core/src/detail/mod.rs` (`DetailModel`) | Same role, different name |
| `store.rs` | `kuro-core/src/ports.rs` plus `detail/msg.rs` | Same role |
| `view/*` | `app/src/ui/detail_manga.rs`, `chapter_row.rs`, `snackbar.rs`, `pickers.rs` | Part 2 names the GTK side `app/`; the inventory names it `kuro-ui`. Resolve with Q1 |
| `dialogs/*` | `app/src/ui/track_sheet.rs`, `pickers.rs`, and new dialog modules | Not spelled out in part 2 |
| `cover_model.rs` | none | Unassigned |
| `strings.rs` | `kuro-core/src/strings.rs` | Same |

---

## 3. Reading guide

Read in this order: inventories, then the topic files, then the behavior and design documents. The tasks files are the implementation contract.

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Part 1 file table: each of 19 files, its declarations, port decision, target module and task ids, with a coverage cross-check |
| [p2-inventory.md](p2-inventory.md) | Part 2 file table: each of 6 files, declarations, port decision, target crate module and task ids |
| [p1-topic-chapter-list-engine.md](p1-topic-chapter-list-engine.md) | Short topic: chapter list pipeline inputs and outputs, missing markers, header text, selection and sort/filter write rules |
| [p2-topic-chapter-list-engine.md](p2-topic-chapter-list-engine.md) | Worked examples of sort (five steps), filter tri-states, separators, selection, next unread, mark previous read, and the tests they imply |
| [p2-topic-track-dialogs.md](p2-topic-track-dialogs.md) | Tracking sheet navigation graph, home model, and the contract of each picker, remover, search and remove dialog |
| [p1-behavior.md](p1-behavior.md) | Part 1 behavior spec: screen entry, layouts, info header, description, toolbar, chapter list pipeline, row, dialogs, callbacks, navigation, share, refresh, gestures, preferences |
| [p2-behavior.md](p2-behavior.md) | Part 2 behavior spec: exact constants and enums, ScreenModel lifecycle, favorite and categories, chapter list engine, bulk actions, persistence, tracking sub-screens, messages |
| [p1-data-and-config.md](p1-data-and-config.md) | Part 1 data: domain models, chapter flag layout, download states, enums, preference keys, files, endpoints, tables, serialization |
| [p2-data-and-config.md](p2-data-and-config.md) | Part 2 data: domain models, enums and stored values, bit layouts, preference keys with defaults, SQL tables, files, endpoints, error types |
| [p1-ui-parity.md](p1-ui-parity.md) | Part 1 UI parity checklist per screen region, including what the Kurobayomi mockup lacks |
| [p2-ui-parity.md](p2-ui-parity.md) | Part 2 UI parity: screen states, selection UI, dialogs, tracking sub-screens, snackbars, keyboard map, responsive rules, mockup gap table, and decision table D-1 to D-10 |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Part 1 edge cases E-01 to E-68 (UI and state), open questions |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Part 2 edge cases (loading, favorites, chapter engine, tracking, platform), upstream tests to port, new Rust tests, fixtures, execution rules |
| [p1-rust-design.md](p1-rust-design.md) | Part 1 Rust design: single-crate `src/details/` layout, pure logic, threading, Android to Linux API mapping, crates, error model, pitfalls |
| [p2-rust-design.md](p2-rust-design.md) | Part 2 Rust design: workspace layout, ports (traits), chapter engine, details reducer, tracking models, error model, strings, integration with the mockup, pitfalls |
| [p1-tasks.md](p1-tasks.md) | Part 1 tasks T-38a-001 to T-38a-060 (60 tasks) |
| [p2-tasks.md](p2-tasks.md) | Part 2 tasks T-38b-001 to T-38b-045 (45 tasks) |

---

## 4. Key design decisions and open questions

Each item lists the question, the options, and a recommendation.

### 4.1 Questions that block implementation

**Q1. Crate layout.** Part 1 proposes one crate with `src/details/`. Part 2 proposes a workspace with `kuro-chapters`, `kuro-core`, `kuro-db`, and `app/`. The inventory also names the GTK side `kuro-ui`.
- Recommendation: part 2 workspace. Name the GTK crate once, `app/` (as `p2-rust-design.md` section 1 does), and fix the `kuro-ui` name in `p2-inventory.md`. Apply the mapping in section 2.2.

**Q2. Duplicate task ownership between part 1 and part 2.** Both parts define the same capabilities. Examples (from `p1-tasks.md` and `p2-tasks.md`):

| Capability | Part 1 task | Part 2 task |
|---|---|---|
| ChapterFlags codec | T-38a-003 | T-38b-001 (plus masked writes T-38b-009) |
| Chapter sort comparator | T-38a-004 | T-38b-002 |
| Chapter view pipeline | T-38a-005 | T-38b-010 |
| Details model, load, route | T-38a-001, T-38a-002, T-38a-053 | T-38b-003, T-38b-021, T-38b-028 |
| Selection | T-38a-010 | T-38b-008, T-38b-023 |
| Download state and indicator | T-38a-025, T-38a-058 | T-38b-016, T-38b-017, T-38b-020 |
| Swipe actions | T-38a-026, T-38a-027 | T-38b-017 |
| Mark previous read, read writes | T-38a-055 | T-38b-017, T-38b-018 |
| Fetch interval | T-38a-028 | T-38b-022 |
| Duplicate and migrate triggers | T-38a-029, T-38a-033 | T-38b-044 |
| Tracking trigger | T-38a-032 | T-38b-029 |
| Category change | T-38a-031 | T-38b-011, T-38b-012 |
| Toast / snackbar host | T-38a-034 | T-38b-024 |
| Share, copy, clipboard | T-38a-040, T-38a-042 | T-38b-038 |
| Keyboard and pointer | T-38a-046, T-38a-054 | T-38b-023 |
| Refresh from source | T-38a-035 | T-38b-004 |
| Test suite | T-38a-051 | T-38b-041 |
| Scanlator exclusion | T-38a-023 | T-38b-015 |
| Chapter settings defaults | T-38a-021, T-38a-022 | T-38b-014, T-38b-026 |

- Recommendation: part 2 owns logic and state (`kuro-chapters`, `kuro-core`), part 1 owns view and glue (`app/`). Mark the part 1 logic tasks as superseded by their part 2 counterparts in a follow-up edit. Do not renumber. This edit was not made in this pass, because the instruction was not to change task ids and superseding needs the other author's agreement.

**Q3. Priority inversions.** These P0 tasks depend on lower-priority tasks in this area, so the P0 chain cannot finish without them:
- T-38a-009 (P0) depends on T-38a-006 (P1).
- T-38a-017 (P0) depends on T-38a-018 (P1), T-38a-032 (P1), and T-38a-040 (P2).
- T-38a-019 (P0) depends on T-38a-036 (P1).
- T-38b-003 (P0) depends on T-38b-014 (P1).
- T-38b-011 (P0) depends on T-38b-013 (P1).
- T-38b-041 (P0) depends on T-38b-006 and T-38b-045 (P1).
- T-38a-058 (P1) depends on T-38a-045 (P2). This one is minor.

- Recommendation: promote those P1 and P2 tasks to P0, or drop the P0 dependency. The P0 set must stay the minimum for daily reading, so the cheaper change is to promote the four blocking ones (T-38a-006, T-38a-036, T-38b-014, T-38b-013) and move the tracker and share items out of the P0 action row (T-38a-017 may ship without the tracking trigger until P1). This changes priorities only and was not applied here.

**Q4. Behavior decisions D-1 to D-10.** These come from `p2-ui-parity.md` section 9 and are repeated in `p2-edge-cases-and-tests.md`. The user decides each one. The table below shows the plan default that the tasks currently implement.

| ID | Question | Upstream | Default in tasks | Recommendation |
|---|---|---|---|---|
| D-1 | Opening a non-library manga again resets its chapter filters and sort to the saved defaults (`MSM:220-222`) | yes | keep (T-38b-014 documents it) | Keep; document in help text |
| D-2 | Select all and invert act on hidden (filtered-out) chapters too (`MSM:1035-1057`) | yes | keep (parity) | Keep parity for select all. For delete downloads, act on visible rows only, and show the count in the confirm dialog |
| D-3 | "Next N" downloads oldest unread first | yes | keep | Keep |
| D-4 | "Continue reading" returns the oldest unread in descending mode | yes | keep | Keep |
| D-5 | Remote tracker delete failure is silent (`MTID:860-868`) | yes | banner on failure (Proposal, T-38b-036) | Keep the banner |
| D-6 | Enhanced tracker auto-match shows "No match found" for any error (`MTID:232-236`) | yes | parity: "No match found" still shown; real text logged | Ask the user. Proposal in section 9 says show the real error text too. Pick one |
| D-7 | Chapter sort direction key is shared with the anime default (`LibraryPreferences.kt:215-223`) | yes | separate keys in the Rust settings table; backup import copies to both (T-38b-014) | Keep separate keys. This changes how anime settings in old backups are read |
| D-8 | Marking already-read chapters read again can trigger the tracker prompt (`MSM:763-773`) | yes | parity (prompt uses all passed chapters) | Ask the user. Proposal in section 9 says prompt only when a chapter changed. The two parts disagree on the default; see Q5 |
| D-9 | Duplicate detection ignores the source (`mangas.sq:117-122`) | yes | keep | Keep |
| D-10 | Quick filter changes can be lost (stale flags, E-FLAG-1) | yes (bug) | fix with masked atomic write (T-38b-009) | Fix |

**Q5. Defaults that disagree between documents.** `p2-ui-parity.md` section 9 marks D-5, D-6, and D-8 as "Proposal" and gives the fix as the plan default. The task files (`p2-tasks.md` lines 395, 417, 462) keep upstream parity for D-6 and D-8 and implement the banner for D-5. Resolve by asking the user (Q4) and then update whichever document is wrong.

**Q6. Open questions raised by the part 1 edge file (`p1-edge-cases-and-tests.md` section 6).**
1. Output of `relativeDateTimeText` for a zero date (E-41). Needs one Kotlin run to pin.
2. Exact Java output for `-0.0`, NaN and infinity in chapter numbers (E-07). Needs one Java run.
3. Whether `SwipeableActionsBox` start and end flip under RTL (E-30). Needs a check of the Compose library source.
4. `EntryDownloadDropdownMenu` labels (behavior 7.6). Listed as "read its file in the next pass", so this is still open.
- Recommendation: items 1 and 2 can be settled by a one-off Kotlin run in CI or on a dev machine. Item 3 needs no Linux decision (GTK uses the locale direction, so swipe directions follow it). Item 4 must be read before T-38a-058 is implemented.

**Q7. Drops and replacements.** These are final unless the user objects.
- Haptic feedback is dropped (T-38a-047). Linux has no portable haptics.
- AssistContent (assist URL) is dropped (T-38a-048). No Linux equivalent is planned.
- The Compose pull-to-refresh gesture is replaced by Ctrl+R, a toolbar item, and a header spinner. The pull gesture is kept only when a touch device exists (T-38a-050).
- The Android stale-timer and confirm-by-long-press quirk is dropped (T-38a-060, E-12).
- Android Toast and `SnackbarHostState` are replaced by an in-window banner (T-38b-024, T-38a-034).

**Q8. Mockup strategy.** `p2-ui-parity.md` section 8 shows that the mockup's manga branch (`src/entry.rs`, 585 lines, sample data) has none of the model behavior. Recommendation: replace only the manga branch with the model-bound page. Leave the anime branch and the migrate screens in `src/entry.rs` until area:39 is done.

### 4.2 Decisions already taken in the docs (for reference)

- Threading: the reducer runs on the GTK main thread; commands run on tokio; writes are detached and report back by message (`p2-rust-design.md` section 4.2, T-38b-039).
- Pure crates have no GTK, glib, tokio, or rusqlite imports (`p2-rust-design.md` section 1).
- Chapter flags stay a single `Long` (`INTEGER`) with the bit layout in `p2-data-and-config.md` section 3. Bit layout checked against source in section 6.4.
- Sort comparators use a collator trait (`p2-rust-design.md` section 3.2). T-38a-004 depends on area:13 for collation.
- Strings use message keys with English text in `kuro-core::strings` (T-38b-037).

---

## 5. Cross-area dependencies

### 5.1 What area 38 needs from others

| Area | What it provides to area 38 | Tasks that depend on it |
|---|---|---|
| area:02-preferences-system | Chapter default keys in `LibraryPreferences.kt`, `TrackPreferences`, `DownloadPreferences`, `ReaderPreferences.skipFiltered` | T-38b-014, T-38a-027 |
| area:03-db-manga-schema-migrations | `chapter_flags` column, `mangas` schema, version column for lost-update checks | T-38b-009 |
| area:05-data-repos-entries-items | Manga and chapter queries, `getMangaById`, `getChaptersByMangaId` (with excluded-scanlator join), masked flag writes, `getDuplicateLibraryManga` (`mangas.sq:117-122`), `excluded_scanlators`, `manga_sync` | T-38a-002, T-38b-003, T-38b-009, T-38b-010, T-38b-011, T-38b-018 |
| area:07-domain-entries-items | `getChapterSort`, TriState `applyFilter`, `calculateChapterGap`, interactors `SetReadStatus`, `SetMangaChapterFlags`, `SetMangaDefaultChapterFlags`, `SyncChaptersWithSource`, `UpdateManga`, `MangaFetchInterval`, `ChapterFilter.kt` public copy | T-38a-002, T-38b-001, T-38b-004 |
| area:08-domain-library-category-history-updates | Category model and `isSystemCategory`, category assignment, library add and remove | T-38b-011, T-38b-012, T-38b-013 |
| area:12-network-stack | HTTP client used by the source fetch path | (via area:15) |
| area:13-core-util-storage-archive-torrent | Collator used by alphabetical sort | T-38a-004 |
| area:15-source-api-manga | Manga and chapter fetch from a source, error types | T-38b-004, T-15a-042 (reverse) |
| area:17-source-app-layer | Source lookup for the screen route | T-38a-001 |
| area:23-downloads-manga | Download manager, download state events, START_NOW, CANCEL, delete downloads, NEXT_N input | T-38a-005, T-38a-025, T-38b-016, T-38b-017, T-38b-020 |
| area:25-downloads-shared | Shared download preference keys | T-38b-020 (via 23) |
| area:27-tracking-core | `TrackChapter`, `RefreshMangaTracks`, `AddMangaTracks`, `TrackerManager`, `MangaTracker`, `EnhancedMangaTracker`, `DeletableMangaTracker`, `supportsPrivateTracking` | T-38a-017, T-38a-032, T-38b-029, T-38b-030, T-38b-031, T-38b-034 to T-38b-036 |
| area:28 to area:33 (individual trackers) | Tracker capabilities (score scales, enhanced match, deletable flag) | T-38b-035, T-38b-043 |
| area:34-images-covers-cache | Cover loading, cache, and custom cover storage | T-38a-012, T-38a-037, T-38a-038 |
| area:36-ui-shell-navigation-security-webview | Navigation stack, window and banner host, `formattedMessage` (`ExceptionFormatter`), WebView, Escape and back handling | T-38a-001, T-38a-020, T-38b-025, T-38b-024 |
| area:37-ui-library | Library bottom action bar (shared component with the details bar), library-side swipe settings | T-38a-057 |
| area:47-settings-screens-general | Chapter swipe preference screens (`pref_chapter_swipe_*`) | T-47a-028 is a reverse dependency |
| T-req-platform-001 (cross-platform addendum) | Paths, opener (open in browser), clipboard, notifier, secret store, DI context | T-38b-037, T-38b-038, T-38b-039 |

### 5.2 What others need from area 38

| Area and task | What it needs from area 38 |
|---|---|
| area:15 T-15a-042 | Manga status badge on the entry screen (depends on area:38 status mapping, T-38a-013) |
| area:34 T-34a-050 | Edit and delete custom cover from the cover window (T-38a-038) |
| area:36 T-36a-060 | Entry open routing from tabs. Requires the manga route (T-38a-001) and the anime route (area:39) |
| area:39 T-39a-066 and T-39a-075 | Item header patterns and the shared toolbar and count rules |
| area:42 T-42a-019 | Favorite from history uses the library-add routing (T-38b-011) |
| area:42 T-42b-018 | Opening an update routes to this screen for manga (T-38a-001) |
| area:42 T-42b-034 | Tapping an upcoming item opens this screen |
| area:43 T-43a-060 | Reader title opens this screen |
| area:47 T-47a-028 | Chapter swipe settings use the swipe action table (T-38a-027) |
| area:44 and area:43 (reader) | The chapter filter evaluation (`skip_filtered`, see `catalogs/preferences-08.md:40`) must be the same function as the details view filter (T-38b-010, `kuro-chapters::filter`). Not yet a task in area:43; recommend adding one |
| area:40 and area:41 (browse, migrate) | `BaseMangaListItem` (T-38a-044) is a shared row. Migrate uses the details duplicate sheet (T-38b-044). Not yet referenced by a task in area:40 or area:41; recommend adding |

---

## 6. Task summary

### 6.1 Counts (from `TASKS.csv`, rows whose file starts with `38-ui-entry-details-manga/`)

Total 105 tasks: 60 in part 1 (`p1-tasks.md`, T-38a-001 to T-38a-060) and 45 in part 2 (`p2-tasks.md`, T-38b-001 to T-38b-045).

| Priority | Count | Effort breakdown |
|---|---|---|
| P0 | 38 | S 14, M 23, L 1 |
| P1 | 48 | S 26, M 22 |
| P2 | 13 | S 10, M 3 |
| P3 | 6 | S 6 |
| Total | 105 | S 56, M 48, L 1, XL 0 |

By part: part 1 (`p1-tasks.md`) P0 17, P1 26, P2 12, P3 5 (60). Part 2 (`p2-tasks.md`) P0 21, P1 22, P2 1, P3 1 (45).

### 6.2 P0 critical path (dependency order, proposed)

Starting point: area:07 (flags, sort inputs) and area:05 (queries) must be available. `T-req-platform-001` must be available for threading and strings.

1. T-req-platform-001 (external) → T-38b-037 (string keys) and T-38b-039 (threading model) → verify: keys resolve in tests; reducer runs on GTK thread.
2. T-38b-001 (flags codec, needs area:07) → T-38b-002 (sort comparator) → verify: fixture table in `p2-edge-cases-and-tests.md` section 3.2 passes.
3. T-38b-010 (view pipeline, needs area:05) → verify: filter and sort order matches fixtures in section 3.3.
4. T-38b-014 (defaults, P1 inversion, see Q3) → T-38b-003 (load pipeline, needs area:05) → verify: a missing manga yields NotFound, never endless Loading (E-LOAD-1).
5. T-38b-004 (source fetch, needs area:15 and area:07) → verify: manual refresh runs once per manga (E-LOAD-4).
6. T-38b-028 (live list model, needs T-38b-010 and T-38b-039) → verify: download progress updates a row in place.
7. T-38b-005 (next unread) and T-38b-008 (selection engine) → verify: continue reading picks the first unread in view order.
8. T-38b-018 (read and bookmark writes, needs T-38b-024 banner and area:05) → T-38b-017 (row actions, needs area:23) → verify: swipe and START_NOW mapping match `ChapterDownloadIndicator` table.
9. Part 1 UI: T-38a-001 (route, needs area:17 and area:36) → T-38a-002 (model store protocol, size L, needs area:05 and area:07) → T-38a-008 (row) → T-38a-009 (list, P0 inversion on T-38a-006) → T-38a-010 (gestures) → T-38a-011 (resume button) → T-38a-019 (toolbar, P0 inversion on T-38a-036) → T-38a-020 (back and Escape).
10. Part 1 header: T-38a-012 (info header, needs T-38a-002 and area:34) → T-38a-013 and T-38a-015.
11. T-38a-017 (action row, P0 inversion, see Q3).
12. T-38a-021 (filter, sort, display sheet) → T-38b-026 (sheet wiring).
13. T-38b-041 (test suite, last, P0 inversion on T-38b-006 and T-38b-045).

Note: items 9 to 12 are blocked by part 2 items. Build the part 2 logic (items 1 to 8) first, then the part 1 view.

### 6.3 Risks

1. Duplicate work. About 20 capabilities have a task in both parts (Q2). Two implementations of the same flag or sort logic would diverge and break the reader's `skip_filtered`.
2. Blocked P0 items. Most P0 tasks depend on area:05, area:07, area:23, area:27, area:34, area:36, or area:17. Those areas must reach the same milestone first.
3. Priority inversions (Q3). Some P0 tasks cannot finish until P1 tasks are done.
4. Large Kotlin sources. `MangaScreenModel.kt` (1243 lines) and `MangaInfoHeader.kt` (688 lines) mix logic with Compose state. Risk of over-porting Compose structure. Mitigation: port only the logic listed in the inventory's "Port" column.
5. Shared components. The bottom bar (T-38a-057, library), download dropdown (T-38a-058), and cover pipeline (T-38a-059) are used by areas 37, 39, and 34. Owner must be fixed to one area or the work is done twice.
6. Stale state (E-FLAG-1, lost updates). Filter writes from the UI can race. Mitigation: masked atomic write with a version bump (T-38b-009) and a single writer on the GTK thread.
7. Unresolved defaults (Q4, Q5). Implementation can proceed on the default, but a late change to D-6 or D-8 changes behavior tests.
8. Verification debt. The 17 documents were not re-verified line by line by the area lead (section 6.4).
9. Untested Kotlin-side facts (Q6 items 1 to 4). Small, but the zero-date text and number formatting must match Aniyomi.

### 6.4 Verification done for this overview

- `python3 plans/_tools/tasks.py lint` on `p1-tasks.md` and `p2-tasks.md`: both print `ok`. No edits were needed. All `area:` tokens already name real area ids, and all `T-` tokens resolve.
- `python3 plans/_tools/tasks.py export`: 3801 tasks in total, 4 errors, all in `40-ui-browse-manga/p1-tasks.md` (lines 37, 83, 158, 537: `area:41-ui-browse-manga-part-2` is not an area id). Outside this area; not changed.
- Inventory self-check: every path printed by `areas.py files 38-ui-entry-details-manga 1` and `... 2` appears in `p1-inventory.md` and `p2-inventory.md`. No MISSING lines.
- Spot-check against source: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt` companion object (lines 75 to 103) matches the constant table in `p1-data-and-config.md` section 2 (all 21 constants checked by value).
- Not verified line by line in this pass: the behavior docs, edge-case lists, and the Rust design code samples. Those rely on the authors' citations.

---

## 7. Definition of done for this area (Kurobayomi)

Each item is observable on the running app, with a manga from a working source.

1. Opening a manga from library, history, updates, browse, or the reader title shows the details page with the cover, title, source, author, artist, status, and description within one frame of cached data. If the manga row is missing, the page shows NotFound with a Back action and never spins forever.
2. The chapter list shows one row per chapter. Order follows the sort mode (source, number, upload date, alphabetical) and the direction bit, for every mode. Verified with the fixture table in `p2-edge-cases-and-tests.md` section 3.2.
3. The header shows the chapter count. When the source reports missing chapters, separator rows appear between chapters with the missing count, in the order the topic document specifies.
4. The Filter, Sort, and Display sheet changes the list immediately. Each change is written to the manga's flags in the database and survives an app restart.
5. "Set as default" stores the current settings. "Reset to default" restores them. Opening a non-library manga applies the defaults (D-1). Apply-to-all-favorites writes to every library manga.
6. Clicking a chapter opens the reader at that chapter. The Resume (Start) button opens the first unread chapter in view order, or the first chapter when none is unread (`p1-behavior.md` section 6.8).
7. Selection: a click selects one chapter, a range gesture selects a range, select all and invert act as D-2 decides. The bottom bar shows only the actions that apply to the selection.
8. Bulk actions: mark read, mark unread, bookmark, remove bookmark, mark previous as read, download NEXT_1 / NEXT_5 / NEXT_10 / NEXT_25 / UNVIEWED, delete downloads (with confirm), and cancel. Each produces the message keys in `p2-ui-parity.md` section 5.
9. A row swipe performs the configured left and right actions (defaults from `LibraryPreferences`). Right-to-left and mouse drag behave per T-38a-054.
10. The download indicator on each row shows not-downloaded, queued, downloading (with a progress ring), downloaded, and error states. Clicking follows the table in `p1-behavior.md` section 7.5.
11. The library button adds the manga to the library and routes category choice as T-38b-011 and T-38b-012 specify. A duplicate in the library shows the duplicate sheet (show, migrate, add anyway). Removing from library asks whether to delete downloads.
12. The fetch interval dialog stores the custom interval as T-38b-022 and `p1-behavior.md` section 8.7 specify.
13. The tracking sheet lists each tracker, shows its linked state, and opens the status, chapter, score, and date pickers with the constraints in `p2-topic-track-dialogs.md`. Tracker search finds results and links one. Removing a track asks whether to delete it remotely if the tracker allows.
14. The full cover window shows the cover at full size. Save, share, and edit (custom cover from a file) work, and delete works for a custom cover. Failures show a banner with the message key.
15. Share, copy URL, and open in browser work and report failures in the banner.
16. Keyboard and pointer behavior follows T-38a-046 and T-38b-023.
17. Mark-read with the tracker prompt, auto-track after mark read (ALWAYS or ASK), and the tracker prompt for chapters already read follow D-8.
18. `cargo test` passes for `kuro-chapters` and `kuro-core` without a display, including the ported `MissingItemsTest` vectors and the fixtures in section 3 of `p2-edge-cases-and-tests.md`.
19. No `cfg(target_os)` appears in `kuro-chapters` or `kuro-core`.
20. Each dropped behavior (section 4.1 Q7) is absent from the UI and listed in the release notes with its reason.
