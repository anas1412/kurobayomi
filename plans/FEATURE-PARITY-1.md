# Feature parity matrix 1/3: reading and library

Scope of this matrix: every user-visible feature of the library, categories, entry details (manga and anime), browse, global search and migration screens, updates, history, the manga reader (all viewers and settings), and stats, storage and upcoming.

Out of scope here (covered by the sibling matrices): the anime player, the extensions runtime, downloads internals, backup, trackers' remote APIs, settings screens. Where a feature in this scope depends on them, the dependency is named by area id.

Reference tree for every Kotlin path: `/home/blackbox/aniyomi-src` (repo-relative paths, read-only, Apache-2.0). Kurobayomi paths are under `/home/blackbox/kurobayomi`.

## 0. How to read this document

- **Feature**: one user-visible capability.
- **Aniyomi behavior**: one line, OBSERVED from the Kotlin unless marked otherwise.
- **Area doc(s)**: relative links to the area overview and detail folders in this plans directory. Area ids are written as `area:<id>`.
- **Task ids**: task ids from `TASKS.csv`. Priority codes: P0 needed for daily use, P1 parity most users rely on, P2 long tail and polish, P3 Android-only or obsolete (the task states a replacement or a drop decision).
- **Priority column**: the highest priority of the feature's primary tasks. Secondary tasks may be lower.
- **Notes**: milestone-independent notes only. Proposal decisions are labelled `Proposal:`.

Labels used across the matrix: OBSERVED (what the Kotlin does), Proposal (what Kurobayomi does).

## 1. Library screen (manga and anime)

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Library entry shows favourites split by category | Shows entries with `favorite = true`, grouped by category tab; separate manga and anime libraries (`Kind::Manga`, `Kind::Anime`). | [37-ui-library](37-ui-library/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-37a-005, T-08a-005, T-37a-007, T-37a-061 | P0 | Two independent libraries; shared preferences decided in T-37a-049. |
| Library row model (library view query) | Reads `libraryView` (manga) and `animelibView` (anime) rows with unread, downloaded, bookmark and seen counts. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-001, T-37a-005, T-08a-005 | P0 | SQL in [sql catalogs](catalogs/sql-01.md). |
| Category tab strip with counts | Tab per visible category with an item count; tabs can be hidden. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-016, T-37a-047 | P0 | Empty visible categories still show a tab. |
| Paged category view | Each category is a page; the pager keeps a lazy window and a persisted index. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-017, T-08a-013 | P0 | Index is persisted per kind. |
| Default-category visibility rule | The Default category is shown when it has items or when it is the only category. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-013, T-37a-007 | P0 | Edge case in T-37a-047. |
| Compact grid mode | Dense grid of covers with title, badges and unread counts. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-018 | P0 | |
| Cover-only grid mode | Grid with covers only, no text. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-019 (dup T-40a-049 for browse) | P0 | |
| Comfortable grid mode | Larger grid with title under the cover. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-020 | P0 | |
| List mode | Rows whose height follows the list-size slider. | [37-ui-library](37-ui-library/00-overview.md), [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-37a-021, T-39a-068 | P1 | Row height derived from the size setting. |
| Display mode selection (global) | Global display mode stored in one library preference, switchable from the settings tab. | [37-ui-library](37-ui-library/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-012, T-37a-039 | P0 | |
| Badges on covers | Download count, unseen or unread count, local-source marker, language marker. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-006, T-37a-024, T-08a-041 | P0 | Badge widgets are P1 (T-37a-024). |
| Filter engine: six tri-state predicates | Tri-state (`Include`, `Exclude`, `Off`) filters for unread/unseen, downloaded, bookmarked, completed, started, and interval-custom gating; plus a tracker predicate. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-003, T-37a-008, T-08a-014, T-37a-057 | P0 | Interval-custom gating is P2 (T-37a-057). |
| Tracker filter | Filter by tracked or untracked and by tracker membership. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-008, T-37a-054 | P0 | Tracker lookups precomputed (T-37a-054, P1) to avoid O(n²). |
| Downloaded-only override | A global downloaded-only switch overrides the downloaded filter. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-056, T-39a-004 | P1 | Counted in `hasActiveFilters`. |
| hasActiveFilters indicator | Filter button shows active state when any filter is non-Off. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-014 | P0 | |
| Sort engine | Sort keys over title, last-read, unread count, latest chapter, fetch date, added date, total chapters, tracker mean, airing time, random. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-002, T-37a-009, T-08a-015 | P0 | Sort flag codec in T-08a-003. |
| Tracker-mean sort | Sort by mean tracker score. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-010, T-37a-058 | P1 | NaN decision in T-37a-058. |
| Random sort with per-kind seed | Random order with a per-kind seed. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-011 | P1 | |
| Per-category sort persistence | Each category can keep its own sort; a global sort applies otherwise. | [37-ui-library](37-ui-library/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-37a-040, T-08a-012 | P0 | |
| Library search (`id:`, genre, source, author, title, negation) | Search grammar with prefixes and negation; debounced. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-012, T-08a-016 | P0 | |
| Library settings tabs (Filter, Sort, Display) | Three-tab sheet bound to the library preferences. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-037, T-37a-038, T-37a-039, T-08a-044 | P0 | |
| Typed library preferences | Exact keys and defaults for 87 library accessors. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-026, T-37a-004 | P0 | Keys in [preferences catalogs](catalogs/preferences-01.md). |
| Library pipeline (debounce and generations) | Filter, sort and search recomputed off the GTK thread with a generation counter. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-013 | P0 | Proposal: generation-guarded pipeline so stale results are dropped. |
| Empty, no-results and filtered-empty states | Distinct messages for an empty library, no search matches, and an empty category under filters. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-023 | P0 | |
| Toolbar title, count and pill | Title, item count and the active-filter pill follow the rules in T-37a-015. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-015 | P0 | |
| Global search row | Row at the top of the library that opens global search with the current query. | [37-ui-library](37-ui-library/00-overview.md), [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-37a-022 | P0 | |
| Selection (single, range, select all, invert) | Long-press starts selection; tap toggles; range via shift-style gesture; select all and invert. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-025, T-37a-026, T-37a-059 | P0 | Proposal: keyboard equivalents in T-37a-046. |
| Selection action bar | Bottom bar with bulk actions when a selection exists. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-026, T-38a-057 | P0 | Shared component with details screen. |
| Mark viewed and read for selection | Marks all selected entries' units as seen or read. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-027 | P0 | |
| Download next N and download all unviewed | Enqueues the next N or all unviewed units for each selected entry. | [37-ui-library](37-ui-library/00-overview.md), [23-downloads-manga](23-downloads-manga/00-overview.md), [24-downloads-anime](24-downloads-anime/00-overview.md) | T-37a-028 | P1 | Download queue belongs to the downloads matrix. |
| Change categories for selection | Dialog with common and mixed tri-state per category; applies a membership delta. | [37-ui-library](37-ui-library/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-37a-029, T-37a-030, T-08a-017 | P0 | Delta preserves hidden category links. |
| Remove from library for selection | Confirmation dialog; optional deletion of downloads. | [37-ui-library](37-ui-library/00-overview.md), [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-37a-031, T-38b-013 | P1 | Cache invalidation in T-37a-032. |
| Continue button on a cover | Opens the next unread chapter (manga) or next unseen episode (anime) for that entry. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-033, T-37a-044, T-26a-061, T-26a-064 | P0 | Next-unit algorithm in T-42a-015. |
| Open random entry | Opens a random entry from the current page. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-034, T-08a-031 | P1 | |
| Library and category update triggers | Toolbar button starts a library update; per-category refresh with toast. | [37-ui-library](37-ui-library/00-overview.md), [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-37a-035, T-26a-102, T-26a-113 | P0 | Pass semantics in T-26a-016. |
| Global update message wording | Message after library update starts. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-050 | P2 | |
| Keyboard and mouse equivalents | Keys for select all, search, refresh, and tab movement. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-046 | P1 | Proposal: GTK shortcut controller. |
| Library settings persistence | Changes persist to the Rust settings store. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-063, T-08a-026 | P0 | |
| Library toast component | In-window toast for library actions. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-062, T-26a-113 | P1 | |
| Season-parent counts (anime) | Anime entries with seasons show a count and label for the parent. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-048 | P1 | |
| Shared manga and anime preferences decision | Decision whether the two libraries share display and sort settings. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-049 | P1 | Decision required before implementing sort persistence. |
| Hidden-category handling | Hidden categories are excluded from tabs but still count for membership. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-036, T-37a-047 | P2 | |
| Library precomputed tracker lookups | Tracker and membership lookups are precomputed per pipeline run. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-054 | P1 | Replaces the O(n²) filter. |
| Crash-proof indexing | Indexing never panics on stale anchors after a removal. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-055 | P0 | |
| Library badge decoration contract | Decoration data for badges and counters. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-041 | P1 | |
| Library item cache invalidation | Cache invalidated and timestamp bumped on library removal. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-032 | P1 | |
| Library tab position under navigation style | Manga tab position and back behaviour depend on the navigation style. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-045 | P1 | |

## 2. Categories

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Category model and system category seed | Categories have id, name, order, flags; a system Default category is seeded. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-006 | P0 | |
| Category reads and per-entry lookup | Reactive list of categories and category ids per entry. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-007, T-37a-060 | P0 | |
| Create, rename, hide, partial update | Name validation through category dialogs; hide toggle per category. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md), [37-ui-library](37-ui-library/00-overview.md) | T-08a-008, T-37a-042 | P1 | |
| Reorder | Drag or move reorders with id-based move and a per-kind lock. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md), [37-ui-library](37-ui-library/00-overview.md) | T-08a-009, T-37a-041 | P1 | |
| Delete category | Deletes and renumbers order and cleans preferences; upstream bug fixed. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-010 | P1 | |
| Category management screens | Management page listing categories with actions. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-041, T-37a-043 | P1 | |
| Set entry categories | Writes the full category set of one entry in one transaction. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md), [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-08a-011, T-38b-012 | P0 | |
| Add-to-library default category | Default category resolution is shared between manga and anime. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-030, T-39b-026 | P0 | |
| Category data in backup | Categories and preference id remapping survive backup and restore. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-035 | P1 | Backup format is in the backup matrix. |
| Active category persistence | The last active category page is remembered. | [37-ui-library](37-ui-library/00-overview.md) | T-08a-031, T-37a-017 | P1 | |
| Auto-clear item cache on cold start | Preference to clear the item cache on startup. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-037 | P2 | |

## 3. Entry details: manga

Screen: manga details page (`MangaScreen`), its ScreenModel, chapter list engine, chapter rows, settings sheets, dialogs, cover window and tracking sheet. Area docs: [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Details route and load gate | Opens for a manga id; shows a loading state until the row is read; NotFound handled. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-001, T-38b-003 | P0 | Defaults shown before first paint (T-38b-003). |
| Info header: cover, title, author, artist, source | Cover, title, author, artist and source name; copy and search gestures on text. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-012 | P0 | |
| Status text and icon | Status enum mapped to icon and text. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-013 | P0 | |
| Backdrop blur and gradient | Blurred cover backdrop behind the header. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-014 | P2 | Cosmetic. |
| Description with expand and collapse | Collapsible description; whitespace rule and defaults. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-015 | P0 | |
| Genre chips with search and copy | Chip per genre; tap searches, menu copies. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-016 | P1 | |
| Action row: library toggle, fetch interval, tracking, WebView | Four actions in one row. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-017 | P0 | Tracking in the tracking row below. |
| Next update label | Shows the next scheduled update time. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-018 | P1 | Time rules in T-38a-018. |
| Top toolbar with title fade and overflow | Title fades on scroll; overflow menu. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-019 | P0 | |
| Back and Escape | Back pops the details page; Escape also closes it. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-020 | P0 | |
| Resume and Start floating button | Floating button starts or resumes reading from the next unread chapter. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-011, T-38b-005 | P0 | Next unread rule in T-26a-061. |
| Chapter list with count header | Header with the chapter count and the missing-chapter warning. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-006, T-38b-010 | P0 | |
| Chapter view pipeline | Filter (unread, bookmarked, downloaded), then sort, then local-manga rules. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-005, T-38b-010 | P0 | |
| Chapter sort comparator | Collation-based primary comparator for chapter number and source order. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-004, T-38b-002 | P0 | |
| Chapter number formatting | Formats decimal and float chapter numbers. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-007 | P0 | |
| Chapter flags codec | Bit layout for filter, sort and display flags stored on the manga row. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-003, T-38b-001 | P0 | Bit layout is in the sql catalogs. |
| Chapter row | Name, scanlator, date, read and bookmark state, download state. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-008, T-38b-017 | P0 | |
| Chapter list widget | Virtualised list of rows with separators. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-009 | P0 | |
| Row open, selection and gestures | Tap opens the reader; long-press starts selection; range by long-press. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-010, T-38b-008, T-38b-023 | P0 | |
| Missing-chapter markers and separators | Gap markers between chapter numbers; header warning count. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-006, T-38a-052, T-38b-045, T-43a-021 | P1 | Gap arithmetic is shared with the reader (T-43a-021). |
| Bulk action bottom bar | Mark read, bookmark, download, delete for selected chapters. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-024, T-38b-016, T-38b-018 | P1 | |
| Download indicator state machine | Not downloaded, queued, downloading, downloaded, error states. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-025, T-38b-017 | P1 | Downloads themselves are in [23-downloads-manga](23-downloads-manga/00-overview.md). |
| Chapter swipe actions | Per-chapter swipe start and end actions (preferences). | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-026, T-38a-027 | P1 | Proposal: pointer drag plus context menu. |
| Mark previous as read | Marks all chapters before the selected one as read. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-055, T-38b-018 | P1 | |
| Local manga rules | Local (on-disk) manga skip source fetch and use local chapter rules. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-056, T-38b-010 | P1 | |
| Filter, Sort, Display sheet | Bottom sheet with filters, sort order and display options stored as flags on the manga row. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-021, T-38b-026 | P0 | |
| Set as default and reset | Saves the sheet values as library defaults; reset restores them. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-022, T-38b-014 | P1 | Apply-to-all-favorites option in T-38b-014. |
| Scanlator exclusion | Dialog and filter row to exclude scanlator groups. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-023, T-38b-015 | P1 | |
| Fetch interval dialog | Set the per-entry update interval. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-028, T-38b-022 | P1 | |
| Duplicate-entry sheet | Shown when adding an entry that already exists under another source. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-029, T-38b-011 | P1 | |
| Delete chapter downloads confirmation | Confirms deletion of downloaded chapters. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-030, T-38b-020 | P1 | |
| Category change dialog | Dialog to change the entry's categories; one-transaction confirm. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-031, T-38b-012 | P0 | |
| Library add and remove | Add to library with duplicate detection and default-category routing; remove with side effects. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38b-011, T-38b-013 | P0 | |
| Tracking action and tracking sheet | Tracking count, tracking sheet with per-tracker rows, status, chapter, score, dates. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md), [27-tracking-core](27-tracking-core/00-overview.md) | T-38a-032, T-38b-029, T-38b-030, T-38b-043, T-38b-044 | P1 | Remote tracker APIs are in the tracker matrices. |
| Auto-open tracking after add | Opens the tracking sheet after adding to library when enabled. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38b-029, T-38a-031 | P1 | |
| Auto-track after mark read | ALWAYS or ASK prompt to track after marking read. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md), [27-tracking-core](27-tracking-core/00-overview.md) | T-38b-040 | P1 | |
| Tracker links | Opens tracker pages in the default browser. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38b-038 | P1 | Proposal: `xdg-open`. |
| Migrate trigger | Opens the migrate flow from the details page and replaces the entry on completion. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md), [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-38a-033, T-38b-044 | P1 | |
| Refresh from source | Manual refresh fetches details and chapters; error mapping shown as toast. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-035, T-38b-004, T-38b-025 | P0 | |
| Auto-download new chapters on refresh | Optional download of newly found chapters after manual refresh. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38b-019 | P1 | |
| Small and large layout switch | Narrow layout stacks header; wide layout puts header beside the list. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-036 | P1 | |
| Full cover window | Full-screen cover with save, share, edit and delete. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-037, T-38a-038, T-38a-039, T-38b-027 | P2 | Save and share write files through the desktop file chooser. |
| Share, copy and WebView actions | Share the entry link, copy title, open the source WebView. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-040, T-38a-042 | P2 | |
| Search actions | Search title or genre from the details page. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-041 | P2 | |
| Keyboard and pointer shortcuts | Keys for resume, select all, mark read. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-046, T-38b-023 | P2 | |
| Toast and banner host | Window-scoped banners replace Compose snackbars. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-034, T-38b-024 | P1 | |
| Loading, empty and error states | Distinct states for loading, no chapters, and fetch error. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-053 | P1 | |
| Cover and backdrop image pipeline | Cover load, cache and placeholder. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md), [34-images-covers-cache](34-images-covers-cache/00-overview.md) | T-38a-059, T-38a-039 | P1 | |

## 4. Entry details: anime

Screen: anime details page (`AnimeScreen`), episode and season lists, settings sheets, quality window, images dialog, tracking sheet and airing section. Area docs: [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md). Player hand-off belongs to [45-player-core](45-player-core/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Entry load sequence | Loads the anime row, runs the first-open refresh, and shows state machine states. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-001 | P0 | |
| Header: backdrop, cover, title, author, artist, status, source | Blurred backdrop, cover and metadata. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-023, T-39a-025, T-39a-063 | P0 | Source display name rules in T-39a-063. |
| Info box copy and search gestures | Tap copies or searches title, author, artist, source. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-024 | P1 | |
| Action row: start or continue watching, library toggle, tracking, download and overflow menu | Four actions plus overflow. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-026, T-39a-039 | P0 | Bottom-bar predicates in T-39a-039. |
| Floating start or continue button | Floating button with label rule. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-037, T-39b-018 | P0 | Continue uses the next unseen episode. |
| Next update label | Shows the next episode update time. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-027 | P1 | |
| Description collapse | Collapsible description with trimming. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-028 | P0 | |
| Genre chips and menu | Chips with copy and search menu. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-029, T-39b-050 | P1 | |
| Related anime rows | Related section with packing and display modes; grouping from related sync. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-030, T-39a-031, T-39b-035 | P1 | Related sync is P2 (T-39b-035). |
| Episode list with count header | Header with count, plural form, missing count and filter button. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-066, T-39a-015 | P0 | |
| Season grid (fetch type Seasons) | Entries whose fetch type is Seasons show a season tile grid instead of episodes. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-032, T-39a-034, T-39b-032, T-39b-033, T-39b-034 | P1 | Four display modes; grid columns in T-39a-034. |
| Season continue overlay and badges | Season tile shows continue and unseen badges. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-033 | P1 | |
| Season settings window (Filter, Sort, Display) | Sheet for season filter, sort and display. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-008, T-39b-011, T-39b-007, T-39b-008 | P1 | |
| Season overlay and creation defaults | Overlay for season flags with defaults used at creation. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-006 | P1 | |
| Episode settings window (Filter, Sort, Display) | Sheet with episode filter, sort and display. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-007, T-39b-010, T-39b-004, T-39b-003 | P0 | |
| Episode sort engine | Four sort keys, both directions, source-order fallback. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-003 | P0 | |
| Episode filter engine | Tri-state filters and downloaded-only override. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-004, T-39a-004 | P0 | |
| Episode flag codec | Bit layout and SetAnimeEpisodeFlags semantics. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-001, T-39b-006 | P0 | |
| Viewer flags (skip intro and airing bits) | Skip-intro length and airing flag bits on the viewer flags. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-003, T-39b-009, T-39b-029 | P1 | Skip-intro length dialog is P2. |
| Episode number formatting | Formats episode numbers for display. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-048 | P0 | |
| Episode title display mode | Setting for how the episode title is shown. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-019 | P0 | |
| Episode row, simple layout | Name, watch state and date. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-010 | P0 | |
| Episode row, rich layout | Thumbnail and summary layout. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-011, T-39a-012, T-39b-070 | P1 | Preview thumbnails and summary toggles in T-39b-070 (P2). |
| Watch progress string | Progress text for an episode. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-013, T-39a-022 | P0 | formatTime rules in T-39a-022. |
| Filler label and bookmark indicator | Filler and bookmark marks on rows. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-014, T-39b-015 | P0 | |
| Missing-episode separators | Separator row and count rule for gaps. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-015, T-39b-005 | P1 | |
| Selection and gestures | Tap toggles; long-press range; select all. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-016, T-39b-016 | P0 | |
| Episode swipe actions | Per-anime start and end swipe actions. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-017, T-39b-053 | P1 | |
| Episode download indicator | Download state machine for episode rows. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-018, T-39b-056, T-39b-057 | P1 | Downloads are in [24-downloads-anime](24-downloads-anime/00-overview.md). |
| Airing countdown row | Next-episode airing countdown text and row. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-020, T-39a-021, T-39b-036, T-39b-037 | P1 | Airing data overwrite fix in T-39b-037. |
| Mark seen and unseen | Marks episodes as seen; removes downloads when configured. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-013 | P0 | SetSeenStatus semantics. |
| Mark previous as seen | Marks all episodes before the selected one as seen. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-040, T-39b-014 | P0 | |
| Bookmark and filler bulk toggles | Bulk toggles on selected episodes. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-015 | P0 | |
| Download next N and unseen | Enqueues next N or all unseen. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-055 | P1 | |
| Delete episode downloads | Confirm and delete downloads. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-041, T-39b-058 | P1 | |
| Episode open routing | Opens an episode internally or externally; alt key inverts the default. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md), [45-player-core](45-player-core/00-overview.md) | T-39b-017 | P0 | |
| Continue watching: next unseen | Next unseen episode for the continue action. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-018, T-26a-064 | P0 | |
| Refresh pipeline for episodes | Manual and automatic refresh of the episode list; error mapping. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-021, T-39b-020, T-39a-038 | P0 | |
| Season refresh pipeline | Per-season episode fetch. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-022 | P1 | |
| Auto-download new episodes after refresh | Optional download after manual refresh. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-023, T-26a-024 | P1 | |
| Library add and remove | Favorite flag, cover removal, provenance. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-024, T-39b-027 | P0 | |
| Duplicate-library check | Dialog when adding an entry already in the library. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-025, T-39a-047 | P1 | |
| Category choice on add | Default, no-category or chosen category. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-026 | P1 | |
| Custom update interval dialog | Sets the per-entry interval; expected update recomputed. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-028 | P1 | |
| Apply library defaults for non-library entries | Defaults applied once, not per operation. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-012 | P0 | |
| Set as default (episode and season) | Writes defaults for episode and season settings; key collisions resolved. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-009, T-39a-062, T-39b-010, T-39b-011 | P1 | Episode and manga chapter keys split (T-39a-062). |
| Tracking summary and sheet | Tracking count, logged-in flag, per-tracker rows, status, episode, score, dates, search and bind, remove. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md), [27-tracking-core](27-tracking-core/00-overview.md) | T-39b-038, T-39b-039, T-39b-040, T-39b-041, T-39b-042, T-39b-043, T-39b-044, T-39b-046 | P1 | Remote APIs in tracker matrices. |
| Auto-track prompt on mark seen | ALWAYS, ASK or NEVER. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-047, T-39b-048 | P1 | |
| Tracking refresh on open and tracker errors | Refreshes tracker state on open; shows error toasts. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-049 | P1 | |
| Enhanced tracker registration on search | Registers enhanced tracker on search. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-045 | P2 | |
| Quality window (hosters and videos) | Hoster and video selection with state machine; preferred video load loop. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md), [45-player-core](45-player-core/00-overview.md) | T-39a-042, T-39a-043, T-39a-044 | P1 | Video URL resolution is in the player matrix. |
| Images window (cover and background) | Pager, zoom, save, share, edit, delete. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-045, T-39a-046, T-39a-058 | P1 | Defect fixes D-2, D-3, D-4, D-13, D-14, D-15 in T-39a-058. |
| Cache file naming | Keeps cache file names compatible with existing caches. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-065 | P1 | |
| Duplicate-add choice | Choice window for duplicates. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-047 | P1 | |
| Mark previous and delete downloaded from toolbar | Toolbar action items for anime entries. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-075, T-39a-040, T-39a-041 | P1 | |
| Refresh replacement | Toolbar refresh replacement. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-038 | P1 | |
| Keyboard and mouse map | Keys for select, play, toggle seen. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-049, T-39a-055 | P1 | |
| Narrow and wide layouts | Layout switch with large-layout fixes. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-035, T-39a-036 | P0 | |
| Settings item only for configurable sources | Source settings entry shown only if the source is configurable. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-057 | P1 | |
| Episode and season list binding | ListStore binding with splice and row widgets. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-064, T-39b-063 | P0 | Model-driven screen replaces the mockup. |
| Retire spec-driven anime dialogs | Spec-driven dialog mockups removed. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-065 | P2 | |
| Share, copy and open in browser | Desktop replacements for Android share and clipboard. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-054, T-39b-051 | P2 | Proposal: `xdg-open` and the GTK clipboard. |
| Torrent server start before playback | Starts the torrent server as a child process before playback. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md), [45-player-core](45-player-core/00-overview.md) | T-39b-052 | P2 | |

## 5. Browse: manga

Area docs: [40-ui-browse-manga](40-ui-browse-manga/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Browse tab with six sub-pages | One tab hosting sources, source browse, global search, extensions, migration and related pages. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-001, T-40a-034 | P0 | Route table in T-40a-034. |
| Installed sources grouped by language | List of installed manga sources grouped by language, with a pinned group and last-used. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-007, T-40b-010, T-40b-020 | P0 | |
| Source row and source icon | Row with name, language and icon. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-005, T-40a-006 | P0 | |
| Source options: pin, unpin, disable | Options dialog per source. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-008, T-40b-011, T-40b-012 | P1 | |
| Latest button per source | Opens the Latest listing of the source. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-007, T-40b-030 | P0 | |
| Show and hide sources | Language switches and per-source checkboxes. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-009, T-40b-021, T-40b-022 | P1 | |
| Browse one source: Popular, Latest, search | Paged listing with Popular, Latest, text search. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-032, T-40b-036, T-40b-030 | P0 | |
| Browse source states | Loading, error, empty and stub (not installed) states. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-003, T-40b-040 | P0 | |
| Browse grid and list display modes | Comfortable, compact, cover-only and list modes; column preference. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-004, T-40a-049, T-40b-035 | P0 | |
| Favorite alpha and in-library marker | Browse cells show in-library state. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-004 | P0 | |
| Filter sheet for browse | Filter tree with TriState cycle; sort filter semantics. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-031, T-40b-050, T-40b-051, T-40b-052 | P0 | |
| Genre search | Genre-based search from the details page or filter. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-037 | P1 | |
| Hide-in-library filter | Option to hide entries already in the library. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-034 | P1 | |
| Favorite toggle with duplicate detection | Long-click favorite: remove confirm or direct add; duplicate dialog. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-041, T-40b-042, T-40b-043, T-40b-044, T-40b-047 | P0 | |
| Category chooser on add | Category picker for add-to-library. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-045 | P1 | |
| Migrate hand-off from duplicate dialog | Duplicate dialog hands off to migrate. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-046 | P1 | |
| Browse context menus | Right-click menus on rows. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-036 | P1 | |
| Extensions tab: grouped list | Extensions grouped by status with headers; refresh. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [18-extensions-manga](18-extensions-manga/00-overview.md) | T-40a-017, T-40a-024, T-40a-025 | P0 | Extension runtime is in the extensions matrix. |
| Extension install, update, uninstall, cancel | State machine for install lifecycle. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [20-extension-strategy-redesign](20-extension-strategy-redesign/00-overview.md) | T-40a-018, T-40a-046, T-40a-047 | P0 | Strategy in area 20. |
| Trust dialog for untrusted extensions | Confirms trust before install. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-019 | P1 | |
| Extension details page | Header, info, incognito toggle, repo link, source rows, enable/disable all, clear cookies. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-021, T-40a-022 | P1 | |
| Extension language filter | Page to filter extensions by language. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-020 | P1 | |
| Source preferences page | Declarative per-source preference rows. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-023 | P1 | |
| Extension icons | Available via URL; installed from bundle; untrusted. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-027 | P1 | |
| Long-press semantics for extensions | Long-press opens actions; private uninstall confirm. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-028 | P1 | |
| Remove-entry and generic confirm dialogs | Shared confirm dialogs. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-029 | P1 | |
| Browse tab re-select opens global search | Re-selecting the Browse tab opens matching global search. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-050 | P2 | |
| Copy-to-clipboard helpers | Copy source id and debug info. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-044 | P2 | |
| Extension updates badge and update-all | Badge with count; update-all button. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-025, T-40a-046 | P1 | |
| Browse keyboard map | Keys for browse pages. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-035 | P1 | |
| Responsive layout | Width breakpoints for browse pages. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-046, T-40a-037 | P1 | |

## 6. Browse: anime

Area docs: [41-ui-browse-anime](41-ui-browse-anime/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Sources list (Last used, Pinned, language sections) | Grouped sources; each row opens Popular or Latest. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-001, T-41b-001, T-41b-002, T-41b-003 | P0 | Section order in T-41b-002. |
| Source options popover | Pin, unpin, disable. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-002, T-41b-006, T-41b-007 | P0 | |
| Source icon loading | Icon fetch and cache for sources and extensions. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-004 | P0 | |
| Visibility page (show and hide sources) | Ordered visibility model per language. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-008, T-41b-009 | P1 | |
| Browse one source | Paged remote listing, source pager, Popular, Latest, Filter chips. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-012, T-41b-015, T-41a-005 | P0 | |
| Browse display modes | Comfortable, compact, cover-only and list modes; per-mode columns. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-006, T-41b-017 | P0 | |
| Browse toolbar and overflow | Search, display-mode menu, overflow. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-007, T-41b-016, T-41b-023 | P0 | |
| Stub page for missing source | Not-installed page for a source without an extension. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-008, T-41b-010 | P0 | |
| Network-to-library upsert | Remote entry is written to the local DB when added. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-013 | P0 | |
| Favorite add and remove | Long-press remove with confirm, direct add. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-019, T-41b-020 | P0 | |
| Duplicate detection and category picker | Duplicate dialog; category resolution. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-021, T-41b-022 | P1 | |
| Browse genre and text search | Genre search and toolbar text search. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-024, T-41b-016 | P1 | |
| Last-used write and incognito gating | Last used source is recorded unless incognito. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-025 | P1 | |
| Browse list virtualisation | Per-row subscriptions for large listings. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-026 | P1 | |
| Filter data model and rendering | Filter tree with tri-state rules and sheet actions. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-029, T-41b-030, T-41b-031 | P1 | |
| Hide-in-library filter | Hides entries already added. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41b-014 | P1 | |
| Anime extensions tab | Grouped extension list, install lifecycle, language filter, details page, source preferences. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md), [19-extensions-anime](19-extensions-anime/00-overview.md) | T-41a-009, T-41a-010, T-41a-011, T-41a-015, T-41a-016 | P0 | Extension runtime is in the extensions matrix. |
| Extension grouping and query filter | Query filter with debounce. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-009, T-41a-017 | P0 | |
| Untrusted extension trust flow | Trust dialog and flow. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-012 | P0 | |
| Extension details and cookie clear | Details model and page; clear cookies. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-019, T-41a-020, T-41a-021 | P1 | |
| Source preferences page | Native renderer for source preferences. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-023 | P1 | |
| Open in WebView for available extension | Action for available extension. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-026, T-41b-044 | P1 | |
| Extension stores navigation link | Link to extension stores. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-053 | P1 | |
| Update all extensions | Updates every extension with an update. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-014 | P1 | |

## 7. Global search and migration

Area docs: [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Global search across enabled sources | Parallel search over enabled sources with result ordering; per-source blocks. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40b-060, T-40b-061, T-41b-032, T-41b-033 | P1 | Fan-out uses a shared 5-permit semaphore (T-41b-033). |
| Global search source filter and pinned default | Source selection and pinned default; extension filter. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-062, T-41b-032 | P1 | |
| Has-results toggle | Hides sources with no results; persisted. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40b-063, T-41a-028 | P1 | |
| Single-result auto-open | Opens the entry when exactly one result is found. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40b-064, T-41b-037 | P1 | |
| Global search toolbar: progress, filter chips | Progress bar and chips. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-010, T-41a-028 | P1 | |
| Global search result rows and per-source states | Card rows, source header, origin marker, subtitle. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-011, T-41a-029, T-41a-030 | P1 | |
| Global search error and cancellation | Per-source errors; cancellation on page drop; stale result guard. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-054, T-41b-036 | P1 | |
| Global search entry points and deep links | Entry points from library, browse and search intents. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-065, T-40b-066 | P1 | |
| Migration source picker | Source picker with sort header and counts; copy id. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-012, T-41a-036, T-41b-039 | P1 | |
| Migration favorites list | Favorites of the old source to migrate. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-013, T-41a-037 | P1 | |
| Migrate search (origin-first order) | Search for each favorite on the other sources, origin source first; seeded query. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-014, T-40b-067, T-41a-038, T-41a-039 | P1 | Per-source migrate search is T-41a-039. |
| Season migration picker (anime) | Picker for season migration. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-040, T-41a-041 | P1 | Fetch-type rules in T-41a-041. |
| Migration flags and confirm dialog | Flag bitmap with option list; confirm dialog; separate anime flags key. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-015, T-41a-032, T-41a-033, T-41a-035 | P1 | |
| Migration engine | Ordered side effects, pure chapter merge, transactional. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-016, T-40a-041, T-41a-034 | P1 | Atomicity fixes in T-40a-041. |
| Migrate same-manga guard and double submit | Refuses migrating an entry onto itself; guards double submit. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-051, T-40a-052 | P1 | |
| Migration disclosure for always-copied tracks | Text line disclosing tracks that are always copied. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-045 | P1 | |
| Migration sort and preference persistence | Sort mode and direction persisted. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40b-071, T-40b-072, T-41b-039 | P1 | |
| Migration help link | Help link. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-073 | P2 | |
| Migration picker failure toast | Toast on picker failure. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40b-074 | P1 | |
| Migration from entry details | Migrate trigger on details screen replaces the entry. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md), [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-38a-033, T-39b-025 | P1 | |
| Migration from history | Migrate dialog from history rows. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-020 | P1 | |
| Migration dialog and short string row | Anime migration uses a 76 dp row. | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39a-061 | P2 | |

## 8. Updates

Area docs: [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md), [26-library-update-jobs](26-library-update-jobs/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Updates feed (manga) | New chapters of library favorites in the last 3 months, newest first. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-001, T-42b-002, T-08a-024 | P1 | Subscription window is 3 months with limit 500 (T-42b-002). |
| Updates feed (anime) | Same for episodes; `animeupdatesView`; `GetAnimeUpdates` uses `limit = 500`. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-002, T-42b-004 | P1 | Source: `domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt:13-21`. |
| Updates date grouping and last-updated header | Date headers; last-updated line with relative time. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-005, T-42b-016, T-42b-017, T-42a-002, T-42a-005, T-42a-026 | P1 | Relative date text ports `LocalDate.toRelativeString`. |
| Updates tab shell and loading states | Tab shell; loading, empty, populated and error states. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-025, T-42a-038, T-42b-023 | P1 | |
| Updates rows (manga and anime) | 56 px row with indicators, progress and gestures. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-027, T-42a-028, T-42b-022 | P1 | |
| Updates row download state | Live download state patched into the feed. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-029, T-42b-008 | P1 | |
| Updates download actions | Start, start now, cancel, delete, enqueue and delete planners. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-031, T-42b-009, T-42b-010, T-42b-014 | P1 | Downloads are in the downloads matrices. |
| Updates bulk read, bookmark and fillermark | Mark read or unread for chapters; mark seen, bookmark, fillermark for episodes. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-030, T-42b-011, T-42b-012, T-42b-013 | P1 | |
| Updates bulk delete confirmation | Confirms deleting downloads. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-032, T-42b-014 | P1 | |
| Updates selection mode and bottom action bar | Selection with range select, select all, invert; bottom bar. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-006, T-42a-034, T-42a-036, T-42b-006, T-42b-007, T-42b-021, T-42b-024 | P1 | Selection anchor arithmetic ported exactly (T-42b-006). |
| Open update | Manga opens the reader; anime opens the player with routing rules. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-018, T-42a-035 | P1 | Player is in [45-player-core](45-player-core/00-overview.md). |
| Anime episode quality options | Dialog to choose quality for an episode from updates. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-019, T-42a-037 | P2 | |
| Reset new-updates counter | Counter reset on enter and leave of the Updates tab. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md), [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-42b-020, T-26a-101 | P1 | Badge with live counts in T-26a-101. |
| Library refresh from the Updates tab | Refresh button on the updates page starts a library update; single-flight guard. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md), [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-42b-015, T-42a-033, T-26a-103, T-42b-041 | P1 | |
| Library update scheduler | Automatic interval, device restrictions, entry restrictions, category include and exclude. | [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-26a-014, T-26a-104, T-26a-105, T-26a-106, T-26a-107 | P0 | Interval and restrictions are P0 or P1 by task. |
| Library update per-entry flow | Manga and anime per-entry flows including seasons. | [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-26a-008, T-26a-009, T-26a-080, T-26a-081 | P0 | Chapter and episode sync diffs. |
| Fetch window and next update computation | Yesterday-to-tomorrow fetch window; interval from upload and fetch dates; interval increase. | [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-26a-070, T-26a-071, T-26a-072, T-26a-074 | P0 | Port upstream tests (T-26a-074). |
| Library update notifications | Progress, summary of new items, per-entry notification, error notification. | [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-26a-030, T-26a-032, T-26a-033, T-26a-034, T-26a-035 | P0 | Notification text rules in T-26a-033. |
| Library update error log | Error log writer and opener. | [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-26a-095, T-26a-096 | P1 | |
| Library update data saver for page images | Data saver proxies (bandwidth hero, wsrv.nl, reSmush.it) for page images. | [26-library-update-jobs](26-library-update-jobs/00-overview.md), [43-reader-core](43-reader-core/00-overview.md) | T-26a-040, T-26a-041, T-26a-042, T-26a-043, T-43a-017 | P1 | Reader use in T-43a-017. |
| Library update worker info | Read-only worker info from the job-run table. | [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-26a-110 | P2 | |
| Global update message | Message after update starts. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-050 | P2 | |
| Updates badge with live counts | Badge counts update live. | [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-26a-101, T-26a-100 | P1 | |
| Updates tab keyboard and focus model | Keyboard and focus model. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-038 | P2 | |
| Updates feed consistency and performance | Change coalescing and index. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-040, T-26a-088 | P1 | |
| Updates tab navigation and back behaviour | Anime and manga tabs; navigate-up behaviour. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-037 | P1 | |
| Updates toasts | Toast for updates and upcoming pages. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-039 | P1 | |

## 9. History

Area docs: [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Manga and anime history list | Last-read and last-watched unit per entry, read from `history` and `animehistory` via views. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-42a-011, T-42a-012, T-08a-018 | P0 | |
| History view model and date grouping | Date-grouped list for both kinds. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-013, T-08a-020 | P0 | |
| History row | Row with cover, title, last unit, progress and actions. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-014 | P0 | |
| Resume from history | Opens the next unit (manga reader, anime player) for the entry. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-015, T-42a-016, T-26a-061, T-26a-064 | P0 | Next-unit algorithm in T-42a-015. |
| History recording | Upsert on each read or watch; session timer; throttle; incognito skips recording. | [43-reader-core](43-reader-core/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-019, T-43a-033, T-43a-038 | P0 | Upsert semantics in T-08a-019. |
| Remove history: single row, whole entry, clear all | Three removal scopes. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-017, T-08a-021, T-42a-024 | P1 | Clear all asks for confirmation (T-42a-024). |
| History delete dialogs | Dialog copy for each removal. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-018, T-42a-062 | P1 | |
| Favorite from history | Adds the entry to the library from a history row. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-019 | P1 | |
| History search | Search within history. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-023, T-42a-007 | P1 | |
| History tab reselect | Re-selecting the tab scrolls or resets. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-021 | P0 | |
| History empty, loading and no-results states | Distinct states. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-022 | P0 | |
| History clear all | Clears all history with confirmation and snackbar. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-024 | P1 | |
| Total read duration | Sum of read duration used by statistics. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-023 | P2 | |
| Next chapter and next episode algorithm | Computes the next unit after the last read or watched one. | [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-022, T-42a-015 | P0 | |

## 10. Manga reader: core

Area docs: [43-reader-core](43-reader-core/00-overview.md). Viewer engines are in section 11.

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Open chapter from library, details, history or updates | Opens the reader at the chapter; restores the last page. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-023, T-43a-092, T-43a-091 | P0 | Restore uses the saved chapter id and page index (T-43a-039). |
| Chapter page loaders: directory, archive, EPUB, downloaded, HTTP | Loader precedence chooses one of five loaders. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-007, T-43a-008, T-43a-009, T-43a-010, T-43a-011, T-43a-013, T-43a-014, T-43a-015 | P0 | EPUB and downloaded-chapter loaders are P1. |
| Image type detection and natural sort | Image file rule and natural name sort. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-012 | P0 | |
| Page-list and image disk cache | 100 MiB cache, md5 keys, LRU eviction. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-016, T-43a-092 | P0 | Constants in [43-reader-core](43-reader-core/00-overview.md). |
| Chapter list in reader | Filters, current chapter re-appended, duplicate removal. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-018, T-43a-019, T-43a-020, T-43a-021 | P0 | Duplicate-number collapse is P1 (T-43a-020). |
| Chapter transition card | Layout and strings for the chapter boundary card. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-003, T-43a-022, T-43b-033, T-43b-032 | P0 | |
| Chapter load errors | Initial load error card with Retry; Downloaded-only blocked message. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-024, T-43a-074, T-43a-076 | P0 | |
| Chapter switch on page change | Loads the new current chapter and adjacent chapters. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-025, T-43a-026, T-43a-027 | P0 | Adjacent navigation uses N and P keys. |
| Page-change progress save | Saves last page read; error skip; incognito skip. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-028, T-43a-088 | P0 | |
| Completion on the last page | Marks chapter read; idempotent. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-029 | P0 | |
| Tracker update on completion | Tracker updated when a chapter completes. | [43-reader-core](43-reader-core/00-overview.md), [27-tracking-core](27-tracking-core/00-overview.md) | T-43a-030 | P1 | |
| Mark duplicate chapters as read on completion | Duplicate-number chapters are marked read together. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-031 | P1 | |
| Delete after read | Deletes read chapter downloads; deferred until leaving the reader. | [43-reader-core](43-reader-core/00-overview.md), [23-downloads-manga](23-downloads-manga/00-overview.md) | T-43a-032 | P1 | |
| Download ahead while reading | Downloads the next chapters while reading a downloaded chapter. | [43-reader-core](43-reader-core/00-overview.md), [23-downloads-manga](23-downloads-manga/00-overview.md) | T-43a-034 | P1 | |
| Bookmark toggle | Toggle with persistence and revert on failure. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-035 | P1 | |
| Incognito reading | No history and no progress writes in incognito. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-038 | P1 | |
| Reader top bar | Back, title block, bookmark, source-dependent action. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-040 | P0 | |
| Chapter navigator | Buttons, page slider, RTL mirror. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-041 | P0 | |
| Reader bottom bar | Reading mode, rotation, crop borders, settings. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-042 | P0 | |
| Reading mode sheet | Select, apply, revert. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-043 | P0 | |
| Rotation sheet | Select rotation; no-effect note. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-044 | P1 | |
| Page actions sheet and long-press | Save, share, copy, set cover. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-045, T-43a-054, T-43a-056, T-43a-057, T-43a-058 | P1 | Share replaced by file manager open (T-43a-056, P2). |
| Save page to Pictures | Saves the page image to the Pictures folder; notification. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-054, T-43a-055 | P1 | |
| Copy page to clipboard | Copies the page image. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-057 | P1 | |
| Set page as manga cover | Sets the page as the cover. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-058 | P1 | |
| Open chapter in browser or share link | Opens chapter URL or shares it. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-059 | P1 | |
| Open manga from reader title | Opens the details page. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-060 | P0 | |
| Reader settings window: reading mode tab | Pager and webtoon reading-mode settings. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-046, T-43b-031 | P0 | |
| Reader settings window: general tab | General reader settings. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-047, T-43b-030 | P0 | |
| Reader settings window: custom filter tab | Custom color filter settings. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-048, T-43b-049 | P1 | |
| Page indicator | Current page and total. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-049 | P0 | |
| Chrome visibility, fullscreen, Escape | Toggles chrome; fullscreen; Escape exits. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-050, T-43b-042 | P0 | |
| Keep screen on while reading | Inhibits screen blanking. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-051, T-43b-042 | P0 | Proposal: GTK inhibit or portal inhibit. |
| Reader background themes | Background color choices. | [43-reader-core](43-reader-core/00-overview.md), [44-reader-viewers](44-reader-viewers/00-overview.md) | T-43a-052, T-44a-059 | P0 | |
| Reader keyboard shortcuts | N, P and desktop shortcuts. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-053, T-43b-058 | P1 | |
| Reading mode and orientation per manga | Per-manga reading mode and orientation with default resolution. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-036, T-43b-010, T-43b-011, T-43b-053 | P0 | ReadingMode flags in section 11. |
| Crop borders toggle | Toggle per viewer type with On and Off toast. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-037 | P0 | |
| Custom brightness and color filter | Software overlay for negative brightness; grayscale and invert layers. | [43-reader-core](43-reader-core/00-overview.md) | T-43b-049, T-43a-064, T-43a-066 | P1 | Positive brightness via backlight is P2 (T-43a-065). |
| Page loading indicator | Spinner with clamped percentage. | [43-reader-core](43-reader-core/00-overview.md) | T-43b-029 | P0 | |
| Navigation overlay and flash on page change | Tap-zone labels and e-ink flash. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-062, T-43a-063, T-43b-043, T-43b-044 | P2 | |
| Reader settings persistence | Typed preference store with Aniyomi key names and defaults. | [43-reader-core](43-reader-core/00-overview.md) | T-43b-001, T-43b-002, T-43b-003, T-43b-004, T-43b-005, T-43a-081 | P0 | Key catalogue in [preferences catalogs](catalogs/preferences-01.md). |
| Reader settings propagate to open reader | Live changes apply to the open reader. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-080, T-44a-005 | P0 | |
| Backup and restore of reader preferences | Reader preferences and viewer flags included in backup. | [43-reader-core](43-reader-core/00-overview.md) | T-43b-055 | P1 | Format in the backup matrix. |
| Create folder per manga | Option to create a folder per manga in downloads. | [43-reader-core](43-reader-core/00-overview.md), [25-downloads-shared](25-downloads-shared/00-overview.md) | T-43b-056 | P1 | |
| Reader session crash restore | Restores chapter id and page index after a crash. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-039, T-43a-091 | P1 | Crash-safe write of the session row. |
| Database writer actor | Single writer for progress, bookmark, history and viewer flags. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-088 | P0 | Proposal: one writer task on tokio. |
| Reader event channel | Unbounded, ordered channel owned by the UI. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-086, T-43a-093 | P0 | |
| Reader session state machine | Main-thread owner for session state. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-075, T-43a-006 | P0 | |
| Downloaded-only blocked chapter | Message and retry when a chapter is not downloaded in downloaded-only mode. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-074 | P0 | |
| Reader strings | English keys for reader strings. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-061, T-43a-094, T-43b-057 | P0 | |
| Reader mockup replacement | Replaces the placeholder reader. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-077, T-43a-078 | P0 | |
| Reader keyboard focus and accessibility | Focus model and basics. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-089, T-44a-084 | P1 | |
| Platform orientation lock | Screen orientation lock and auto-rotate. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-068 | P3 | Dropped. Desktop has no orientation lock; values kept. |
| Display cutout mode | Android cutout short-edge mode. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-069, T-43b-050 | P3 | Dropped on Linux. |
| Volume-key page turns | Volume keys page turning. | [43-reader-core](43-reader-core/00-overview.md), [44-reader-viewers](44-reader-viewers/00-overview.md) | T-43a-071, T-44a-015, T-43b-041 | P2 | Dropped in T-43a-071 for desktop; XF86 keys kept as P1 in T-43b-041. |
| Haptic feedback in reader | Haptics on slider and long press. | [43-reader-core](43-reader-core/00-overview.md), [44-reader-viewers](44-reader-viewers/00-overview.md) | T-43a-070, T-44a-017 | P3 | Dropped. |
| Activity transitions | Android activity transitions. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-072 | P3 | Replaced with GTK stack slide. |
| Reader Android plumbing | Android view and platform plumbing. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-073, T-43b-052 | P3 | Dropped. |

## 11. Manga reader: viewers, modes and settings

Area docs: [44-reader-viewers](44-reader-viewers/00-overview.md) and the reader settings detail in [43-reader-core](43-reader-core/00-overview.md).

Reading modes (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`): `DEFAULT` (flag 0x00000000, resolved per manga or globally), `LEFT_TO_RIGHT` (0x00000001, horizontal, pager), `RIGHT_TO_LEFT` (0x00000002, horizontal, pager), `VERTICAL` (0x00000003, vertical, pager), `WEBTOON` (0x00000004, vertical, webtoon), `CONTINUOUS_VERTICAL` (0x00000005, vertical, webtoon with `isContinuous = false`). Mask `0x00000007` (line 60). `DEFAULT` must be resolved before a viewer is created (`toViewer` throws otherwise).

Tap zones (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt`, `TapZones` list): default, L-shaped, Kindle-like, edge, right-and-left, disabled.

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Reading mode enum and flags | Six values above with mask; flag stored in the manga viewer flags. | [43-reader-core](43-reader-core/00-overview.md) | T-43b-009, T-43b-010, T-43b-011, T-43a-004 | P0 | Mask-merge writes in T-43a-004. |
| Left to right pager | Horizontal pager, left page first. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-021, T-44a-023, T-44a-024 | P0 | |
| Right to left pager | Horizontal pager reversed. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-022 | P0 | Direction mapping in T-44a-022. |
| Vertical pager | Vertical pager. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-021, T-44a-023 | P0 | |
| Webtoon (long strip) | Virtual vertical list of pages with side padding and gap mode. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-031, T-44a-032, T-44a-033, T-44a-034 | P0 | Padding range 0 to 25 (`WEBTOON_PADDING_MIN` and `WEBTOON_PADDING_MAX` in ReaderPreferences). |
| Long strip with gaps (continuous vertical) | Webtoon viewer with `isContinuous = false`. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-033, T-44a-058 | P1 | |
| Tap zone tables | Six navigation tables. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-008, T-43b-037, T-43b-038 | P0 | |
| Tap zone inversion | Inverts zones for paged and webtoon viewers. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-009, T-43b-039 | P0 | |
| Tap hit test and dispatch | Menu fallback; disabled navigation. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-010, T-43b-038 | P0 | |
| Single tap, double tap, drag | Gesture recognition. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-011, T-43b-016 | P0 | |
| Long-tap page actions | Long-press opens page actions. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-012, T-43b-012, T-43b-013 | P1 | Long-press state machine in T-43b-012. |
| Keyboard paging (pager and webtoon) | Keys page the pager and scroll the webtoon. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-013, T-44a-014 | P0 | |
| Scroll wheel paging | Wheel pages the reader. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-016 | P1 | |
| Live navigation mode switching | Changes tap zones without restart. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-018 | P1 | |
| Navigation overlay | Shows tap-zone labels on first run and on request. | [44-reader-viewers](44-reader-viewers/00-overview.md), [43-reader-core](43-reader-core/00-overview.md) | T-44a-019, T-44a-020, T-44a-085, T-43b-036, T-43b-040 | P1 | |
| Pager deferred chapter application | Applies chapter changes after first layout. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-025 | P0 | |
| Pager preload and end-of-book menu | Preload policy; end menu at the last page. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-027, T-44a-028, T-44a-030 | P0 | Chapter transition gating in T-44a-030. |
| Webtoon current page detection | Page events and detection. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-034, T-44a-035 | P0 | |
| Webtoon menu hide on scroll | Menu hides after a scroll threshold. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-036, T-43b-046 | P0 | |
| Webtoon zoom model | Zoom, double-tap zoom, quick scale, zoom fling, zoom-out disable. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-038, T-44a-039, T-44a-040, T-44a-041, T-44a-042, T-43b-047 | P0 | Quick scale and fling are P1. |
| Page image widget: scale types | Fit screen, stretch, fit width, fit height, original size (`ImageScaleType`). | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-045, T-43b-015, T-43b-007 | P0 | Stored values 1 to 6 (T-43b-015). |
| Zoom start position | Automatic, left, right, center. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-046, T-43b-017 | P0 | |
| Landscape auto zoom | Auto zoom for wide pages. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-047, T-43b-018 | P1 | |
| Pager double tap and pinch zoom | Gesture zoom for pager pages. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-048, T-43b-016 | P0 | |
| Tiling for large pages | Tiled rendering with texture size cap and hardware threshold. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-049, T-43b-021, T-44a-060 | P0 | Max tile size is P2 (T-44a-060). |
| Decode pipeline | Decode with failure path and session guard. | [44-reader-viewers](44-reader-viewers/00-overview.md), [43-reader-core](43-reader-core/00-overview.md) | T-44a-050, T-44a-065, T-43b-059 | P0 | |
| Animated GIF, APNG and animated WebP | Animated pages; unsupported formats fall back to first frame. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-051, T-44a-052, T-43b-022 | P1 | Fallback is P2. |
| Crop borders | Crops borders on paged and webtoon pages. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-053, T-43b-020 | P1 | |
| Automatic background colour | Background from page colours or desktop colour scheme. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-054, T-44a-077, T-43b-025 | P1 | |
| Dual-page split | Split spread for pager and webtoon; invert; rotate-to-fit. | [44-reader-viewers](44-reader-viewers/00-overview.md), [43-reader-core](43-reader-core/00-overview.md) | T-44a-055, T-44a-056, T-44a-057, T-43b-048 | P1 | Insert pages in T-44a-058. |
| Page status visuals | Queued, loading, downloading, error and progress indicator. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-062, T-44a-063, T-44a-064 | P0 | |
| Chapter transition card in viewer | Adjacent chapter loading and error inside the card. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-066, T-44a-067, T-44a-068 | P0 | |
| Memory window and recycle | Limits decoded pages in memory. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-069, T-43b-024 | P0 | |
| Viewer teardown | Releases viewer resources on leave. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-070 | P0 | |
| Reader background themes in viewer | Theme change without restart. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-059, T-44a-043 | P0 | |
| Page transitions and double-tap animation speed | Toggle and speed. | [44-reader-viewers](44-reader-viewers/00-overview.md), [43-reader-core](43-reader-core/00-overview.md) | T-43b-045, T-44a-074, T-44a-075 | P1 | Reduced motion from desktop setting in T-44a-074 (P2). |
| Colour filter layers | Brightness overlay, colour filter, blend modes. | [43-reader-core](43-reader-core/00-overview.md) | T-43b-049, T-43a-066 | P1 | |
| ICC display profile | Page colour through display profile. | [43-reader-core](43-reader-core/00-overview.md) | T-43a-067 | P2 | |
| Viewport metrics on resize | Recomputed on window resize. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-076 | P0 | |
| Platform gesture timings | Timings from GTK settings. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-075 | P1 | |
| Viewer settings store and live propagation | Observed defaults; live settings propagation; in-place refresh of images on display setting change. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-004, T-44a-005, T-44a-007, T-43b-035 | P0 | |
| Viewer trait and factory | Engine trait, factory, lifecycle. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-001, T-43b-034 | P0 | |
| Pager and webtoon insert items | Insert pages and chapter transitions in the item list. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-002, T-44a-021 | P0 | |
| Chapter gap calculation in viewers | Gap between chapter numbers. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-003, T-43b-027 | P0 | |
| Webtoon alwaysDecodeLongStripWithSSIV | Dropped: always-decode flag is not ported. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-061, T-43b-023 | P3 | Dropped. Long strip uses the webtoon page variant (T-43b-023). |
| Replace DirectionalViewPager | Replacement of the Android pager library. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-072, T-44a-073 | P3 | Replaced by the pager engine in T-44a-023. |
| Reader viewer logging | Logging through tracing. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-083 | P2 | |
| Reader viewer ordinary features under desktop | Pan wide images, tap zone overlay labels, keyboard. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-43b-019, T-44a-029, T-43b-028 | P1 | |
| Navigation tables: unit tests | Tests for tap zones and inversion. | [44-reader-viewers](44-reader-viewers/00-overview.md) | T-44a-078, T-44a-079, T-44a-080 | P0 | |

## 12. Stats, storage and upcoming

Area docs: [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md).

| Feature | Aniyomi behavior | Area doc(s) | Task ids | Priority | Notes |
|---|---|---|---|---|---|
| Statistics page: counts, durations, tracker mean, update count | Library counts, read duration, tracker mean score, global update count, per kind. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-039, T-42a-040, T-42a-041, T-42a-042, T-42a-043 | P1 | Tracker mean is shared with the library sort (T-37a-010). |
| Statistics error isolation | Each stat fails independently. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-044 | P1 | |
| Statistics download count | Download count from the cache. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-045, T-42a-009 | P1 | Download-cache signal fan-out (T-42a-009). |
| Storage page: usage by category | Storage size by manga and anime, colour per category, aggregation. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-046, T-42a-047, T-42a-048, T-42a-050 | P1 | Byte formatting in T-42a-004. |
| Storage donut chart | Donut chart of usage. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-049 | P2 | |
| Storage delete | Deletes storage for the selected entries. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-051 | P1 | |
| Storage page layout and readiness | Shell, two-column layout, no stuck loading state. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-052, T-42a-053, T-42a-054 | P1 | |
| Upcoming: query and entry model | Upcoming episodes and chapters for library entries (manga and anime). | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-025, T-42b-026 | P2 | |
| Upcoming: list and date headers | 96 px rows, date headers with count badge. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-030 | P2 | |
| Upcoming: calendar grid | Month grid, per-day counts, event dots, day-jump, slide transition. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-027, T-42b-028, T-42b-029 | P2 | Calendar math tests in T-42b-043. |
| Upcoming: layouts | Single-column and two-panel breakpoints. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-031 | P2 | |
| Upcoming: tap opens entry | Opens the entry screen. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-034 | P2 | |
| Upcoming: tie-break and midnight refresh | Ordering tie-break and refresh at midnight. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-035 | P2 | |
| Upcoming: week start and locale | Week start from locale. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-045 | P2 | |
| Upcoming: Home key | Optional key returns to the current month; off by default. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-046 | P3 | Optional. |
| Upcoming: help link | Help link. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-032 | P2 | |
| Upcoming: navigation and toolbar | Page navigation and toolbar. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-033 | P2 | |
| Upcoming: Android-only drop decisions | Drop record for Android-only behaviours. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42b-042, T-42a-055, T-42a-056, T-42a-057, T-42a-058, T-42a-059, T-42a-061 | P3 | Dropped. Tablet detection replaced by width breakpoints. |

## 13. Dropped or replaced in this scope

Each entry states the Android feature, the replacement or drop decision, and what the user loses.

| Android feature | Decision | Task ids | What the user loses |
|---|---|---|---|
| Home-screen widgets fed by updates | Dropped. Replaced by tray and desktop notifications. | T-08a-038 | No home-screen widget. The updates badge and notifications remain. |
| Android haptic feedback on long press (library, browse, details, reader) | Dropped. | T-38a-047, T-39a-053, T-41b-055, T-44a-017, T-43a-070, T-42a-058, T-08a-042 | No vibration feedback. |
| Android assist content URL (details, browse, extension pages) | Dropped. | T-38a-048, T-39b-066, T-40b-080, T-41b-056 | No assist-app integration. |
| Compose preview providers and Compose-only mechanics | Dropped; fixtures kept. | T-42a-057, T-41a-046, T-41b-057, T-39a-059 | None for users. |
| Fast scroller in episode list | Dropped. | T-39a-064 | No fast scroller. Keyboard and scroll bar remain. |
| Ignore-padding layout trick | Dropped. | T-39a-052 | None for users. |
| Foreground service declaration and notification channels | Dropped. Replaced by desktop notifications. | T-26a-120, T-26a-121 | None beyond the Android-specific channel grouping. |
| Wi-Fi pre-check for API below 28 | Dropped. | T-26a-122 | None on Linux. |
| WorkManager migrations | Dropped. | T-26a-125 | None. |
| Android notification bitmap decode | Dropped; cover loaded through the desktop image path. | T-26a-126, T-26a-127 | None. |
| FileProvider URIs | Replaced by file paths and portal file access. | T-26a-123 | None. |
| Injekt dependency lookup | Replaced by an explicit services struct. | T-26a-124, T-41b-059 | None for users. |
| Cutout short-edge mode | Dropped on Linux; setting value preserved. | T-43a-069, T-43b-050 | The hide-cutout option has no effect. |
| Screen orientation lock and auto-rotate | Dropped; values preserved. | T-43a-068 | Orientation lock is not available. |
| Volume-key page turning | Dropped for the desktop build; preference kept. XF86 volume keys are kept as an optional P1 binding. | T-43a-071, T-43b-041 | Volume keys do not turn pages by default. |
| Hardware bitmap and BitmapRegionDecoder | Dropped; replaced by the Rust decode path. | T-43b-051 | None. |
| Always decode long strip with SSIV | Dropped. | T-44a-061 | None observable. |
| DirectionalViewPager library | Replaced by the pager engine. | T-44a-072, T-44a-073 | None observable. |
| Activity transitions | Replaced by a GTK stack slide. | T-43a-072 | Transition styling differs. |
| Voyager navigator | Replaced by GTK stack navigation. | T-39b-068, T-39b-069, T-40b-083, T-41b-058, T-42a-056 | None observable. |
| Browse data-saver row and state | Dropped in the sources UI; data-saver proxies kept for page images. | T-40a-042, T-40b-013, T-40b-082 | The data-saver toggle is not shown in the sources list. |
| Package-install permission banner and app-info button | Dropped; replaced by "Open folder". | T-40a-026, T-40a-031, T-41a-024, T-41a-025 | None on Linux. |
| Stale timer and confirm-by-long-press | Dropped. | T-38a-060 | None. |
| Pull-to-refresh gesture | Replaced by a refresh button and shortcut. | T-38a-050, T-37a-036 | Gesture is missing; button and key remain. |
| Android tablet detection | Replaced by width breakpoints. | T-42a-055 | None. |
| Android-only behaviours in upcoming | Replaced or dropped; decision record. | T-42b-042 | None. |
| Compose item animations | Dropped. | T-41b-057 | Animations on item change are not shown. |
| Share and clipboard on Android | Replaced by the GTK clipboard and `xdg-open`. | T-39a-054, T-39b-051, T-43a-056, T-43a-059 | The share sheet is replaced by a file manager open or a copy. |

## 14. Cross-cutting notes for this scope

- Reading and library share one persistence model: the chapter flags codec (T-38a-003, T-38b-001) and the episode flags codec (T-39a-001) are bit layouts stored on rows. Bit layouts are in [sql catalogs](catalogs/sql-01.md) and [sql catalogs](catalogs/sql-02.md).
- Reader progress and history writes go through one writer (T-43a-088). History upsert semantics are in T-08a-019. Next-unit algorithms (T-42a-015, T-08a-022) are shared by the Continue button, the history resume action and the reader.
- Global search fan-out (T-41b-033, 5-permit semaphore) and the library update per-source politeness permit (T-26a-022) are separate limits and are not shared.
- Migration (T-40a-016, T-41a-034) and duplicate detection (T-38b-011, T-41b-021, T-40b-043) must agree on the same-manga guard (T-40a-051).
- Open questions raised by the area docs: shared manga and anime library preferences (T-37a-049) and the anime interval-custom gating (T-37a-057). Both need a decision before the library preferences are final.

## 15. Proofread

Checked before finishing: every task id in this file exists in `TASKS.csv` (validated by script), every relative link resolves to an existing file, and the file contains no open-ended markers. Every row names at least one task id and at least one area doc link. Area ids use the full folder name, for example `area:37-ui-library`.

Known limits of this matrix: the one-line Aniyomi behavior column summarizes the area docs, and the area docs remain the authority for exact constants, SQL and edge cases. Priorities are copied from `TASKS.csv`; rows with several tasks show the highest-priority primary task.
