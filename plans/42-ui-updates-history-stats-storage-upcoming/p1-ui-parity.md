# Area 42 part 1 — UI parity (Updates, History, Stats, Storage)

Every visible element, state, dialog, snackbar, menu and gesture of the 41 part-1 files, the Linux (GTK 4.16, vanilla, no libadwaita) equivalent, and what the current mockup lacks. Source references are repo-relative under `/home/blackbox/aniyomi-src`; mockup references are under `/home/blackbox/kurobayomi`.

Mockup baseline (`src/screens.rs`): `updates()` line 277, `history()` line 305, `stats()` line 443, `stat_card()` line 426, `storage()` after line 470, `kind_tabs()` line 70 (Anime | Manga switcher), `dialog_of()` (spec dialog ids), `list_row()`, `title_n()`, `thumb()`, `download_button()`, `icon_button()`, `heading()`, `dim()`, `boxed_list()`, `scrolled()`, `toolbar()`, `built()`. All content is literal.

---

## 1. Global page frame (all four screens)

### 1.1 Header bar

| Element | Aniyomi (OBSERVED) | Linux equivalent (PROPOSAL) | Mockup gap |
|---|---|---|---|
| Title | `TabbedScreen` `AppBarTitle(stringResource(titleRes), numberTitle)`; titles: History = "History" (`MR.strings.label_recent_manga`), Updates = "Updates" (`label_recent_updates`), Statistics = "Statistics" (`label_stats`), Storage = "Storage" (`AYMR.label_storage`) | `gtk::HeaderBar` title widget `gtk::Label` (`css-classes: title`) | mockup uses `toolbar(&s)` from spec; titles come from `spec/app.json` (present) |
| Search | `SearchToolbar(searchEnabled = tab.searchEnabled)` — enabled for both History sub-tabs; disabled for Updates, Stats, Storage | `gtk::SearchEntry` shown via a toggle button in the header; hidden when not enabled | missing (no search in mockup history) |
| Search query per sub-tab | `mangaSearchQuery` for page 1, `animeSearchQuery` for page 0 (`TabbedScreen.kt:57-63`) | one `SearchEntry` per sub-tab (store in view model) | missing |
| Actions | History: "Clear history" icon `Icons.Outlined.DeleteSweep` (`pref_clear_history`); others: none | `gtk::Button` icon `edit-clear-all-symbolic`, tooltip "Clear history" | present in mockup (`history()` line 328) |
| Back/navigate-up | `navigateUp` only when `fromMore` (history), else none | `gtk::Button` `go-previous-symbolic` in header when pushed from the More page; Escape also goes back (area:36-ui-shell-navigation-security-webview) | mockup: none (navigation area:36-ui-shell-navigation-security-webview) |
| Snackbar host | `SnackbarHost` in `Scaffold` (`TabbedScreen.kt:44`, `MangaHistoryScreen.kt:36-38`) | `gtk::Overlay` with a bottom-centred `gtk::Revealer` label, auto-hide 4 s | missing |

### 1.2 Sub-tab strip

- Two tabs: anime then manga for History and Updates (`HistoriesTab.kt:65-68`, `UpdatesTab.kt:54-57`); anime then manga for Stats and Storage (`StatsTab.kt:40-43`, `StorageTab.kt:40-43`).
- Labels: History — "Anime" (`AYMR.label_anime_history`), "Manga" (`AYMR.label_history`); Stats/Storage — "Anime" (`AYMR.label_anime`), "Manga" (`AYMR.label_manga`).
- Tab badge: `TabContent.badgeNumber` (`TabbedScreen.kt:133`). Updates tabs set it from part 2 (not verified here). Mockup: `main.rs:175` shows "8" on the Updates sidebar entry (hard-coded).
- Unselected tab colour `onSurface`. Selected tab indicator: `PrimaryTabRow` (`TabbedScreen.kt:91-109`). Linux: `gtk::ToggleButton` group styled as `linked`, or `gtk::Notebook` without border; keep order and labels.
- Swipe: `HorizontalPager` (swipe between tabs, `TabbedScreen.kt:111-122`). Linux: `gtk::Stack` with `transition-type: slide-left-right`; touchpad swipe via `gtk::GestureSwipe` optional (P2).
- Mockup: `kind_tabs()` (`screens.rs:70`) provides the switcher with an Anime/Manga toggle — keep; add the tab badge.

### 1.3 Responsive behavior

- Storage only: tablet (`isTabletUi()`, `WindowSize.kt:10`) = two columns (chart left 2/5, list right 3/5, `StorageScreenContent.kt:73-109`); phone = one column (chart on top). Linux: use `adw`-free breakpoint via `gtk::Window` width signal `notify::default-width`: width >= 720 px → two columns, else one column.
- Stats and history/updates: single column at all widths; max content width 720 px centred (proposal, Compose's `LazyColumn` fills width).
- Minimum window size: 360 px wide (phone-like); no horizontal scrolling anywhere on this page (cover + text + icons must wrap, title `maxLines = 2` in history, `1` in updates).

---

## 2. History screens

### 2.1 List structure

- Date header rows (`ListGroupHeader`, text = `relativeDateText(date)`: "Today", "Yesterday"/"N days ago", "Upcoming" family for future dates, or formatted date) (`MangaHistoryScreen.kt:89-94`, `DateText.kt:45-63`, `DateExtensions.kt:52-80`).
- Item rows of height 96 dp (`MangaHistoryItem.kt:35`).
- Keyboard: arrow keys move focus; Enter activates the row (= resume); Delete key removes (proposal; Aniyomi has no keyboard shortcut).

### 2.2 Row anatomy (manga and anime identical except text and icon set)

| Element | Specification | Source | Mockup |
|---|---|---|---|
| Cover | `ItemCover.Book`, full row height, click opens entry | `MangaHistoryItem.kt:56-60` | `thumb(48, 70)` placeholder (`screens.rs:318`) |
| Title | SemiBold, `bodyMedium`, max 2 lines, ellipsis | `MangaHistoryItem.kt:67-73` | `title_n(i)` text |
| Subtitle | `Ch. {n} - {time}` (`MR.recent_manga_time`) or `Ep. {n} - {time}` (`AYMR.recent_anime_time`); time only (short, locale); if number ≤ −1 only time | `:74-87`, `AnimeHistoryItem.kt:75-85` | literal "Ch. 39 - 21:30" (`screens.rs:314`) |
| Favorite button | shown when entry is NOT in library; `Icons.Outlined.FavoriteBorder`, description `MR.add_to_library` | `:90-98` | `icon_button("emblem-favorite-symbolic", "Add to library")` (present, `screens.rs:316`) |
| Delete button | always; `Icons.Outlined.Delete`, description `MR.action_delete` | `:100-106` | `icon_button("user-trash-symbolic", "Delete")` (present) |
| Row click | resume next unit (behavior 1.7) | `:46-53` | `connect_activate` → `open_media` (wrong target, `screens.rs:322`) |

Mockup gap: the mockup never shows a favorite-state difference (always shows the heart) and uses literal times.

### 2.3 Empty, loading and no-results states

| State | Aniyomi | Linux | Mockup |
|---|---|---|---|
| Loading (list null) | `LoadingScreen` (centred spinner) (`MangaHistoryScreen.kt:41`) | `gtk::Spinner` centred in `gtk::Stack` page "loading" | missing |
| Empty, no search | `EmptyScreen(information_no_recent_manga)` "Nothing read recently" / anime "Nothing watched recently" (`MangaHistoryScreen.kt:43-51`, `AnimeHistoryScreen.kt:43-51`) | `gtk::Box` with icon `document-open-recent-symbolic` (proposal) + label, centred | missing |
| Empty, search active (non-empty query) | "No results found" (`MR.no_results_found`) | same label, with the search entry still focused | missing |
| Error | snackbar "InternalError: Check crash logs for further information" (`MR.internal_error`) while list keeps old data | toast (2.6) | missing |

### 2.4 Dialogs (history)

| Dialog | Trigger | Content (OBSERVED) | Buttons | Linux widget | Mockup |
|---|---|---|---|---|---|
| Remove entry from history | Trash button on row | title "Remove" (`MR.action_remove`); text: manga "This will remove the read date of this chapter. Are you sure?" / anime "…watch date of this episode. Are you sure?"; checkbox "Reset all chapters for this entry" / "Reset all episodes for this anime" (unchecked) | "Remove" (confirm), "Cancel" | `gtk::Window` (modal, transient for main) with `gtk::CheckButton` | present as spec dialog `history_remove_manga` / `history_remove_anime` (`screens.rs:317`) — checkbox absent in spec? (the mockup dialog copy must be verified against `spec/app.json`; add the checkbox if missing) |
| Clear history | header button | title "Remove everything" (`MR.action_remove_everything`); text "Are you sure? All history will be lost." (`MR.clear_history_confirmation`) | "OK", "Cancel" | `gtk::AlertDialog`-style window | present as `history_remove_everything` (`screens.rs:329`) |
| Duplicate entry | Favorite button when a duplicate library entry exists | `DuplicateMangaDialog` (out of scope, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime): confirm add anyway, open existing, migrate | per dialog | spec id `history_duplicate_entry` exists in `spec/app.json` (not wired in history()) | missing wiring |
| Change category | Favorite button when default category is not set and categories exist | `ChangeCategoryDialog` (area:08-domain-library-category-history-updates, area:37-ui-library): checkboxes per user category; "Edit categories" button | confirm/cancel/edit | `gtk::CheckButton` list | missing |
| Migrate | from duplicate dialog | `MigrateMangaDialog` / `MigrateAnimeDialog` (area:40-ui-browse-manga, area:41-ui-browse-anime) | — | area:40-ui-browse-manga, area:41-ui-browse-anime | missing |

Notes (OBSERVED): only one dialog at a time; dismiss sets `dialog = null` (`MangaHistoryTab.kt:91`); the history dialog closes immediately after confirm (`onDelete` then `onDismissRequest`).

### 2.5 Menus and gestures (history)

- Long press on a row: not defined in Aniyomi (no selection in history). Keep none (Linux: right-click opens nothing new; do not invent a menu, per the "no invented scaffolding" memory).
- Right-click: none in Aniyomi. Proposal: context menu with Resume, Remove, Open entry — only if the user asks (not in parity scope).
- Hover: none.

### 2.6 Snackbars (history)

| Event | Text (English) | Key | Duration |
|---|---|---|---|
| Next unit not found (manga) | "Next chapter not found" | `MR.no_next_chapter` | Compose Short (~4 s) → Linux 4 s |
| Next unit not found (anime) | "Next Episode not found!" | `AYMR.no_next_episode` | 4 s |
| Clear completed | "History deleted" | `MR.clear_history_completed` | 4 s |
| Internal error | "InternalError: Check crash logs for further information" | `MR.internal_error` | 4 s |

### 2.7 Keyboard and mouse equivalents

| Action | Aniyomi | Linux (PROPOSAL) |
|---|---|---|
| Resume | tap row | click or Enter on focused row |
| Open entry | tap cover | click cover |
| Remove | tap trash | click trash; Delete key on focused row = same dialog |
| Favorite | tap heart | click heart |
| Search | search icon in top bar | Ctrl+F focuses the search entry |
| Clear | header icon | header icon; same dialog |
| Reselect the History tab | `onReselect` (resume) | double-click the sidebar entry resumes (see behavior 1.7 fix T-42a-021) |

### 2.8 Animation and motion

- Compose `animateItemFastScroll()` (item placement) and `AnimatedImageVector` tab icon (`anim_history_enter`). Linux: no list animation; sidebar icon uses a static symbolic icon (`document-open-recent-symbolic`). Drop (inventory row 25).

### 2.9 Mockup gaps (history) — summary

1. No data: `history()` (`screens.rs:305`) builds fixed rows.
2. Heart shown always; should hide when already in library.
3. Only two literal dates (Today, Yesterday) and no empty/loading/no-results states.
4. Row click calls `open_media` (opens the entry or the player). Aniyomi resumes the next unit (`onClickResume`); the cover opens the entry.
5. No search, no per-sub-tab search state.
6. No "Remove everything" checkbox semantics in the dialog body (verify spec copy).
7. No snackbars.
8. Clear-history icon is present (good).

---

## 3. Updates screens

### 3.1 List structure (both kinds)

- Top row: "Library last updated: {relative}" (`MR.updates_last_update_info`, italic) (`MangaUpdatesUiItem.kt:52-67`, `AnimeUpdatesUiItem.kt:55-73`). Relative: "Never" / "Just now" / Android-style "N minutes ago" etc. (behavior 2.3).
- Date header rows (grouped by `dateFetch` local date, not by upload date) (`MangaUpdatesScreenModel.kt:372-385`).
- Item rows of height 56 dp (`MangaUpdatesUiItem.kt:166`, `AnimeUpdatesUiItem.kt:173`).

### 3.2 Row anatomy

| Element | Manga | Anime | Source | Mockup |
|---|---|---|---|---|
| Cover | `ItemCover.Square`, padding vertical 6 dp, full height | same | `MangaUpdatesUiItem.kt:170-176`, `AnimeUpdatesUiItem.kt:177-183` | `thumb(40,40)` in a `pre` box (`screens.rs:291`) |
| Entry title | `bodyMedium`, 1 line, ellipsis; grey (`DISABLED_ALPHA`) when read | same when seen | `:182-188` | `title_n(i)` |
| Unread dot | `Icons.Filled.Circle` 8 dp, primary, when unread | same when unseen | `:191-200` | literal "●" in subtitle (`screens.rs:298`) |
| Bookmark | `Icons.Filled.Bookmark` primary, sized to text height − 2 dp | same | `:201-212`, `AnimeUpdatesUiItem.kt:209-220` | missing |
| Fillermark | — | `Icons.AutoMirrored.Filled.Label` tertiary colour | `AnimeUpdatesUiItem.kt:221-232` | missing |
| Chapter/episode name | `bodySmall`, 1 line, ellipsis, grey when read | same | `:213-222`, `AnimeUpdatesUiItem.kt:233-242` | literal "Chapter N" |
| Progress | "Page: N" (N = lastPageRead + 1) when unread and page > 0 (`MR.chapter_progress`) | "M:SS/M:SS" or "H:MM:SS/…" (`AYMR.episode_progress`) when unseen and last second > 0 | `MangaUpdatesUiItem.kt:105-112`, `AnimeUpdatesUiItem.kt:111-119` | literal "Page: 5" / "12:34/24:00" (`screens.rs:286`) |
| Download indicator | circle icon (state-dependent: not downloaded / queued / downloading progress / downloaded / error) | same plus quality menu | `ChapterDownloadIndicator.kt`, `EpisodeDownloadIndicator.kt` | `download_button()` static |

Row states: normal, unread (dot), selected (`selectedBackground(selected)`, primary tinted background), read (dimmed text), disabled indicator in selection mode.

### 3.3 Download indicator states (both kinds)

| State | Icon (Kotlin resource) | Tap | Long press | Source |
|---|---|---|---|---|
| NOT_DOWNLOADED | `R.drawable.ic_download_item_24dp` (manga "Download" description `MR.manga_download`) | START | manga START_NOW; anime SHOW_QUALITIES | `ChapterDownloadIndicator.kt:95-96`, `EpisodeDownloadIndicator.kt:96-97`; icon `EpisodeDownloadIndicator.kt:65` |
| QUEUE / DOWNLOADING | progress ring with percent | — | CANCEL | `:125`, `:126` |
| DOWNLOADED | check icon | — | menu with DELETE (anime and manga) | `:203-219`, `:204-219` |
| ERROR | error icon | START | START | `:238-240`, `:239-240` |
| Disabled (selection mode) | no indicator interaction | — | — | `MangaUpdatesUiItem.kt:235-241` |

Mockup: `download_button()` only; no states.

### 3.4 Selection mode

- Enter: long press a row. Selected rows highlight; top header changes? — no: Aniyomi keeps the top bar and shows the bottom `EntryBottomActionMenu` (visible when selection non-empty) (`MangaUpdatesScreen.kt:116-118`).
- Exit: Back (`BackHandler`, `MangaUpdatesScreen.kt:49`), or deselect all, or any bulk action (which clears selection, behavior 2.6).
- Tap in selection mode toggles one row; shift-range via long press extends (behavior 2.5). Linux: Ctrl+click toggles, Shift+click range-selects (proposal; Aniyomi uses long press for range — keep long press too).
- Pull-to-refresh disabled in selection mode (`:85`).
- Cover tap and download indicator disabled in selection mode (`MangaUpdatesUiItem.kt:127`, `:130`).

### 3.5 Bottom action bar

`EntryBottomActionMenu` (`app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt:72`): a bottom bar with icon+label buttons, shown when selection is non-empty, actions filtered by predicates (behavior 2.6). Icons (Material outlined): BookmarkAdd, BookmarkRemove, NewLabel (fillermark), LabelOff (remove fillermark), DoneAll (mark viewed/read), RemoveDone (mark unviewed/unread), Download, Delete, OpenInNew (play externally), Input (play internally). Labels (English): "Bookmark" (`MR.action_bookmark`, anime `AYMR.action_bookmark_episode`), "Remove bookmark", "Fillermark" (`AYMR.action_fillermark_episode`), "Remove fillermark" (`action_remove_fillermark_episode`), "Mark as read" / "Mark as seen", "Mark as unread" / "Mark as unseen", "Download", "Delete", "Play externally", "Play internally".

Linux: `gtk::ActionBar` at bottom of the page with `gtk::Button`s (icon + label) and a count label "N selected" at the left (proposal: count is not in Aniyomi's bar; add only as a label next to the close button). Close button clears selection.

### 3.6 Confirm dialogs (updates)

| Dialog | Trigger | Text | Buttons | Source |
|---|---|---|---|---|
| Delete downloaded chapters/episodes | bottom bar "Delete" (multi, through part-2 wiring) | "Are you sure you want to delete the selected chapters?" / "...episodes?" | OK, Cancel | `UpdatesDialog.kt:11-37` |
| Quality chooser (anime) | long press download indicator (NOT_DOWNLOADED) | `ShowQualities` dialog (part 2) | — | `AnimeUpdatesScreenModel.kt:288-297` |

Observed: per-row DELETE from the download indicator menu skips the confirmation (behavior 2.7). Parity: keep (documented as a gap in EC-42-U-08).

### 3.7 Pull to refresh / library update

- Aniyomi: pull gesture on the list → library update job; spinner for 1 s (fake) (`MangaUpdatesScreen.kt:70-90`).
- Linux: header refresh button `view-refresh-symbolic` with tooltip "Update library" → starts the job (area:26-library-update-jobs); while the job runs the icon is replaced by `gtk::Spinner` until the job's completion signal. Disabled while selection is active. A "Library update started" / "Library update already running" toast (events `LibraryUpdateTriggered(started)`) — part-2 strings.
- Keyboard: F5 triggers refresh (proposal).

### 3.8 Empty and loading (updates)

- Loading: spinner centred. Empty: "No recent updates" (`MR.information_no_recent`), icon `view-refresh-symbolic` optional (proposal).

### 3.9 Mockup gaps (updates)

1. Literal rows "Chapter 39", literal progress; no selection, no bookmark/fillermark icons, no download states.
2. "Library last updated: 5 minutes ago" literal; no live value.
3. Day groups "Today/Yesterday/3 days ago" literal; must come from `dateFetch`.
4. No bottom action bar, no confirm dialog wiring for delete.
5. No update-library action and no badge count (sidebar badge "8" is literal, `main.rs:175`).
6. No empty/loading states.

---

## 4. Statistics screens

### 4.1 Layout

- Two sub-tabs (Anime, Manga). Each page is a vertical list of four section cards with spacing `padding.small` (`AnimeStatsScreenContent.kt:38-56`).
- Each card: heading (section title, `SectionCard`), then a row of equal-width tiles (`StatsItem`/`StatsOverviewItem`).
- Overview card has icons under each tile (`CollectionsBookmark`, `Schedule`, `LocalLibrary`). Titles use `titleLarge` bold for overview, `bodyMedium` bold for others; captions `bodyMedium` (overview) / `labelSmall` (others) at 0.x alpha (`SECONDARY_ALPHA`).

### 4.2 Values and captions (full list)

| Card | Tile | Value | Caption (English) | Icon |
|---|---|---|---|---|
| Overview | 1 | library count | "In library" | CollectionsBookmark |
| Overview | 2 | duration (5.4) or "None" | anime "Watched duration" / manga "Read duration" | Schedule |
| Overview | 3 | completed count | "Completed entries" | LocalLibrary |
| Entries (titles) | 1 | global-update count | "In global update" | — |
| Entries | 2 | started count | "Started" | — |
| Entries | 3 | local count | "Local" | — |
| Episodes (anime) / Chapters (manga) | 1 | total | "Total" | — |
| Episodes / Chapters | 2 | watched (anime "Watched") / read (manga "Read") | — | — |
| Episodes / Chapters | 3 | downloaded | "Downloaded" | — |
| Trackers | 1 | tracked count | "Tracked entries" | — |
| Trackers | 2 | mean score "x.xx ★" or "N/A" | "Mean score" | — |
| Trackers | 3 | tracker count | "Used" | — |

Card titles: "Overview", "Entries", "Episodes" (anime) / "Chapters" (manga), "Trackers". (Source: `MR.label_overview_section`, `MR.label_titles_section`, `AYMR.episodes` / `MR.chapters`, `MR.label_tracker_section`.)

Duration zero shows "None" (`MR.none`).

### 4.3 States

- Loading: `LoadingScreen()` (spinner). No error UI (behavior 5.7). Linux: spinner until `Ready`; `Failed` sections show "N/A".
- No empty state: a fresh install shows zeros, "None", and "N/A" (matches Aniyomi).

### 4.4 Interaction

- Read-only; no tap actions, no hover. Stats do not auto-refresh (snapshot on open) — parity. Proposal (P2): refresh button in the header.

### 4.5 Mockup gaps (stats)

- Literal values in `stats()` (`screens.rs:447`): `[["42","2d 4h 10m","7"], ...]` — replace.
- Caption text swapped by string replace ("Watched"→"Read") (`screens.rs:452-453`), which is a hack; use the per-kind caption table in 4.2.
- Card title "Episodes" → "Chapters" via string replace (`screens.rs:456`); use per-kind title.
- The mockup's stats card builds `title_n` style values; the trackers mean score literal "7.40 ★" is not the N/A rule.
- No loading state.

---

## 5. Storage screens

### 5.1 Layout

Phone (width < 720): one scrolling column: category dropdown → donut chart (with total in its upper area) → list of entries (sorted size descending). Tablet/wide: left column (2/5) with dropdown + donut; right column (3/5) list (`StorageScreenContent.kt:73-109`).

### 5.2 Category dropdown

- Label "Category" (`AYMR.label_category`), value = selected category name. Options: "All" (id −1) first, then categories in `GetCategories` order; "Default" for id 0.
- Linux: `gtk::DropDown` with a `gtk::StringList` model; label above it via `gtk::Label` ("Category").

### 5.3 Donut chart

- Half donut (top half), stroke 15 px, round caps, segments in size-descending order, colours per entry (behavior 6.4 / 3.5 rust design).
- Centre-top text: total size (`displaySmall`) e.g. "1.25 GB" (`Long.toSize()`), or "0 B" when empty.
- Linux: `gtk::DrawingArea` aspect 1:1 with height = half width; text via `pango` layout centred in upper half.

### 5.4 Entry row

- Cover 48 px high (`ItemCover.Square`), title (bold, 1 line), second line: colour dot (12 px), size "1.25 GB", separator dot, count "12 chapters" / "3 episodes" (pluralised: `MR.plurals.manga_num_chapters`, `AYMR.plurals.anime_num_episodes`).
- Trailing trash icon button (`Icons.Default.Delete`, description `MR.action_delete`) → confirm dialog.
- Spacing: `padding.medium` between rows.

### 5.5 Delete confirmation

- Title: "Delete downloaded chapters?" (`MR.delete_downloads_for_manga`) / "Delete downloaded episodes?" (`AYMR.delete_downloads_for_anime`).
- Text: `Are you sure you wish to delete "{title}"?` (`AYMR.delete_confirmation`).
- Buttons: "OK" (`MR.action_ok`) → delete; "Cancel" (`MR.action_cancel`).
- Linux: modal `gtk::Window` with title label and text; default focus on Cancel (destructive action) — proposal.

### 5.6 States

- Loading → spinner. Success with no entries → only the dropdown and the chart "0 B" (no empty message, behavior 6.6). Proposal: add "No downloads in this category" helper text (small, dimmed) — flagged as a Rust addition (not in Aniyomi; decide before shipping; see EC-42-T-03).

### 5.7 Mockup gaps (storage)

- `storage()` is a placeholder; no category dropdown, no donut, no per-entry delete, no confirmation.

---

## 6. Dialogs and text inventory (part 1)

| Key (MR / AYMR) | English | Used on |
|---|---|---|
| action_remove | Remove | history delete dialog title + confirm |
| dialog_with_checkbox_remove_description | This will remove the read date of this chapter. Are you sure? | history delete (manga) |
| dialog_with_checkbox_remove_description_anime | This will remove the watch date of this episode. Are you sure? | history delete (anime) |
| dialog_with_checkbox_reset | Reset all chapters for this entry | history checkbox (manga) |
| dialog_with_checkbox_reset_anime | Reset all episodes for this anime | history checkbox (anime) |
| action_remove_everything | Remove everything | clear-history dialog title |
| clear_history_confirmation | Are you sure? All history will be lost. | clear-history text |
| action_ok | OK | confirms |
| action_cancel | Cancel | dismiss |
| clear_history_completed | History deleted | snackbar |
| pref_clear_history | Clear history | header action label |
| no_results_found | No results found | empty search |
| information_no_recent_manga | Nothing read recently | empty history (manga) |
| information_no_recent_anime | Nothing watched recently | empty history (anime) |
| information_no_recent | No recent updates | empty updates |
| recent_manga_time | Ch. %1$s - %2$s | history subtitle |
| recent_anime_time | Ep. %1$s - %2$s | history subtitle |
| add_to_library | Add to library | favorite button description |
| action_delete | Delete | trash description |
| no_next_chapter | Next chapter not found | snackbar |
| no_next_episode | Next Episode not found! | snackbar |
| internal_error | InternalError: Check crash logs for further information | snackbar |
| updates_last_update_info | Library last updated: %s | updates header |
| updates_last_update_info_just_now | Just now | header value |
| relative_time_span_never | Never | header value |
| unread | Unread | dot description |
| action_filter_bookmarked | Bookmarked | bookmark icon description |
| action_filter_fillermarked | Fillermarked | fillermark icon description |
| chapter_progress | Page: %1$d | updates progress (manga) |
| episode_progress | %1$s/%2$s | updates progress (anime) |
| confirm_delete_chapters | Are you sure you want to delete the selected chapters? | updates delete dialog |
| confirm_delete_episodes | Are you sure you want to delete the selected episodes? | updates delete dialog |
| label_stats | Statistics | stats header |
| label_overview_section | Overview | stats card |
| label_titles_section | Entries | stats card |
| label_tracker_section | Trackers | stats card |
| in_library | In library | stats tile |
| label_read_duration | Read duration | stats tile (manga) |
| label_watched_duration | Watched duration | stats tile (anime) |
| label_completed_titles | Completed entries | stats tile |
| label_titles_in_global_update | In global update | stats tile |
| label_started | Started | stats tile |
| label_local | Local | stats tile |
| chapters | Chapters | stats/storage section title |
| label_total_chapters | Total | stats tile (both kinds) |
| label_read_chapters | Read | stats tile (manga) |
| label_watched_episodes | Watched | stats tile (anime) |
| episodes | Episodes | stats section title (anime) |
| label_downloaded | Downloaded | stats tile |
| label_tracked_titles | Tracked entries | stats tile |
| label_mean_score | Mean score | stats tile |
| label_used | Used | stats tile |
| not_applicable | N/A | mean score fallback, relative date fallback |
| none | None | duration fallback |
| day_short | %dd | duration |
| hour_short | %dh | duration |
| minute_short | %dm | duration |
| seconds_short | %ds | duration |
| label_storage | Storage | storage header |
| label_all | All | storage category |
| label_category | Category | storage dropdown |
| label_default | Default | storage category (id 0) |
| delete_downloads_for_manga | Delete downloaded chapters? | storage dialog title |
| delete_downloads_for_anime | Delete downloaded episodes? | storage dialog title |
| delete_confirmation | Are you sure you wish to delete "%s"? | storage dialog text |
| manga_num_chapters (plural) | chapter counts | storage row |
| anime_num_episodes (plural) | episode counts | storage row |
| relative_time_today | Today | date header |
| relative_time (plural) | N days ago | date header |
| upcoming_relative_time (plural) | future day counts | date header |
| label_history / history / label_recent_manga | History / Manga | tab titles (see 0.2 in behavior) |
| label_recent_updates | Updates | updates header |
| label_anime / label_manga / label_anime_history | Anime / Manga / Anime | sub-tab labels |
| manga_download | (download content description) | download indicator |
| action_bookmark, action_bookmark_episode, action_remove_bookmark*, action_fillermark_episode, action_remove_fillermark_episode, action_mark_as_read, action_mark_as_seen, action_mark_as_unread, action_mark_as_unseen, action_download, action_play_externally, action_play_internally, action_move_category | menu labels | updates bottom bar |

Exact English for keys not quoted above is owned by the i18n files (`i18n/src/commonMain/moko-resources/base/strings.xml`, `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`); the Rust build loads the same keys into a `gettext`/Fluent-style table (area:52-util-app) so translations are reused.

---

## 7. Accessibility

- Every icon-only button has a tooltip and an accessible name (`gtk::Widget::set_tooltip_text`, `accessible-label`), using the same description strings as Aniyomi (`add_to_library`, `action_delete`, `unread`, `action_filter_bookmarked`, `action_filter_fillermarked`).
- Colour is never the only signal: unread is the dot (Aniyomi's only unread marker, kept); bookmarked, downloaded and fillermark each have their own icon.
- Focus order: header → search → tab strip → list rows (top to bottom) → bottom action bar.
- Text contrast: all text uses theme colours (vanilla GTK, user theme applies, memory: match current theme).
- Font scaling: use `pango` relative sizes (GTK `title-1`, `heading` CSS classes); no fixed pixel font sizes.

---

## 8. Platform specifics (Linux)

- Theme: vanilla GTK 4; the user's Tokyo Night GTK theme applies (no hard-coded colours; use CSS classes `dim-label`, `heading`, `accent`, `error`, `success`, `warning`). Donut segment colours are per-entry generated colours, not theme colours (exception).
- Dark/light: follow the GTK theme; no separate dark palette in part 1.
- Wayland and X11: no platform-specific code in this area.
- HiDPI: all sizes in logical px; cover thumbnails loaded at 2x scale.
- Notifications: none from this area (library-update notifications are area:35-data-misc-updater-notification-export).
