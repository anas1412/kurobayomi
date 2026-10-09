# Area 15 — UI parity: what the scope's types render, and what the Kurobayomi mockup lacks

The scope itself has no Compose screens. It defines types that these screens render, so this file covers only those visible elements. Source files are cited with repo-relative paths. Keys are the `MR.strings.*` / `AYMR.strings.*` names; English text is taken from `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR).

## 1. Browse source screen (entry point for every source type)

Files: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt`, `app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaSourceToolbar.kt`.

### 1.1 Toolbar actions (OBSERVED)
| Action | Visibility | Icon/text | Behaviour |
|---|---|---|---|
| Display mode | always | title `MR.strings.action_display_mode` ("Display mode"); icon `ViewList` when List, else `ViewModule` | opens the display-mode chooser (Comfortable grid / Compact grid / List) |
| Help | only for the local source (`isLocalSource`) | `MR.strings.label_help` ("Help") | opens help |
| Open in WebView | `HttpSource` only (BrowseMangaSourceScreen.kt:115-124) | `MR.strings.action_open_in_web_view` ("Open in WebView") | opens `WebViewScreen(url = source.baseUrl, initialTitle = source.name, sourceId = source.id)` (the source base URL, not a manga URL) |
| Settings | only when `source is ConfigurableSource` (BrowseMangaSourceToolbar.kt:44, :85) | `MR.strings.action_settings` ("Settings") | opens the source settings screen (section 3) |

### 1.2 Listing states rendered from the paging source (OBSERVED)
| State | Trigger | Shown |
|---|---|---|
| Loading first page | Pager initial load | standard loading indicator |
| Results | page with `mangas` non-empty | grid or list of entries (cover, title; badges for library membership) |
| Empty page (after the first page or first page) | `NoChaptersException` from `SourcePagingSource.load` | error text `MR.strings.no_results_found` = "No results found" (via `ExceptionFormatter`, `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:28-30`) |
| Network / HTTP error | any other exception | formatted message: offline → `MR.strings.exception_offline` ("No Internet connection"); unknown host → `MR.strings.exception_unknown_host` ("Couldn't reach %s"); otherwise `"$className: $message"` or the message for `Exception`/`IOException` (ExceptionFormatter.kt:12-36) |
| Source not installed | `SourceNotInstalledException` | `MR.strings.loader_not_implemented_error` = "Source not found" |

### 1.3 Source list empty
`MR.strings.source_empty_screen` = "No source found" (Browse tab empty state; mockup already has `"empty": "No source found"` in `spec/entries.json` browse screen).

## 2. Filter sheet (SourceFilterMangaDialog)

Files: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt`, `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt`.

### 2.1 Container
- Adaptive sheet (`AdaptiveSheet`): bottom sheet on phones, dialog-like on large layouts; dismiss on outside tap or back.
- Sticky header row (8 dp padding, background colour): `TextButton` Reset (`MR.strings.action_reset` = "Reset", primary colour) at start; `Button` Filter (`MR.strings.action_filter` = "Filter") at end; `HorizontalDivider` below.
- Scrollable body in source order.

### 2.2 Items (exact widget per kind)
| Kind | Widget | Label | Value display | Interaction |
|---|---|---|---|---|
| Header | `HeadingItem(name)` | name | none | none |
| Separator | `HorizontalDivider()` | none | none | none |
| CheckBox | `CheckboxItem` | name | checked state | tap toggles |
| TriState | `TriStateItem` | name | icon for DISABLED / ENABLED_IS / ENABLED_NOT (SettingsItems.kt:355) | tap cycles DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED |
| Text | `TextItem` | name | current text | typing writes on every change (SettingsItems.kt:556) |
| Select | `SelectItem` (exposed dropdown, read-only field) | name | `options[selectedIndex].toString()` (SettingsItems.kt:302-330) | open menu, pick an option |
| Sort | `CollapsibleBox(name)` containing one `SortItem` per value (SettingsItems.kt:123) | value label | arrow icon on selected row: up for ascending, down for descending | tap selected row toggles direction; tap other row selects it and keeps direction; with no selection the current code crashes (filter topic section 5.7) |
| Group | `CollapsibleBox(name)` with nested items | name | nested items | expands/collapses; nested items behave as their own kind |

### 2.3 Desktop equivalents (PROPOSAL for Kurobayomi)
- Keyboard: Tab moves focus through rows; Space or Enter activates CheckBox and TriState rows; Up/Down move in a DropDown list; Escape closes the sheet; Enter on the Filter button runs the search.
- Mouse: click targets are full rows (not only the checkbox); hover highlight from the GTK theme.
- Sort rows: keep arrow icons (`go-up-symbolic`, `go-down-symbolic`); screen readers announce "ascending" or "descending" through the accessible label.
- Responsive: the sheet is a `gtk::Window` of 360-420 px width; below 480 px of window width it covers the window; the list scrolls inside the sheet.

### 2.4 Mockup gap (existing `spec/entries.json` browse_source `filter_sheet`)
- The mockup lists sample item types `info`, `switch`, `tristate`, `text`, `list`, `sort`, `action` with sample option values ("Option 1", "Option 2", "Popularity", "Latest", "Title"). The real sheet is built from the source's `FilterList` at runtime, so the spec items are not used for filter values.
- `info` covers both Header and Separator in the mockup; they must be distinct kinds.
- No Reset/Filter sticky header in the mockup spec; no per-kind TriState cycle; no Sort direction behaviour.
- Group (`action` in the mockup) must be a nested container, not a button.

## 3. Source settings screen

Files: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt` (`MangaSourcePreferencesScreen` and `MangaSourcePreferencesFragment`), `app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt:435` (entry point that checks `is ConfigurableSource`).

### 3.1 Container (OBSERVED)
- Loading screen until sources are loaded (`ifMangaSourcesLoaded()`).
- Top bar title: `MangaSourceManager.getOrStub(sourceId).toString()` → "Name (LANG)" (see behaviour 1.2).
- Body: an embedded AndroidX preference fragment; the source's `setupPreferenceScreen` decides the rows.
- Preference rows get: no icon space, single-line titles off, dialog title = preference title when empty; EditText preferences use incognito keyboards (behaviour 15).

### 3.2 Kurobayomi rendering of `SourceSetting` (PROPOSAL)
| SourceSetting | Row | Dialog on tap | Stored when |
|---|---|---|---|
| Switch | title, summary, `gtk::Switch` on the right | none | switch toggled |
| Text | title, current value as subtitle | `gtk::Window` with `gtk::Entry` (password entry for `password: true`), OK and Cancel | OK pressed |
| List | title, selected entry as subtitle | dialog with radio rows; OK and Cancel | OK pressed |
| MultiSelect | title, count as subtitle | dialog with check rows; OK and Cancel | OK pressed |
| Info | title (bold heading when used as a category header), summary | none | never |
| Unsupported | title, no subtitle | none | never |

- Default values come from the schema; a missing stored value shows the default.

## 4. Source options (browse source list, long-press / menu)

Mockup dialog `source_options` (spec/entries.json): items `Pin` ("'Unpin' when pinned"), `Disable` (hidden for the local source), `Exclude from data saver` (manga only; "'Stop excluding from data saver' when excluded"). These map to: pin list (area:17), disable (area:17), `data_saver_excluded` StringSet (behaviour data file 4.2). The exclusion set is read by `GetEnabledMangaSources` (app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt:26) to build the source list state; the option UI itself belongs to area:17. Key and default: `p1-data-and-config.md` section 4.2.

## 5. Manga status badge (detail screen)

File: `app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt:503-535`.
- Row with an icon (16 dp, secondary alpha) and text.
- Icon and text per status code (behaviour section 12): ONGOING Schedule / "Ongoing"; COMPLETED DoneAll / "Completed"; LICENSED AttachMoney / "Licensed"; PUBLISHING_FINISHED Done / "Publishing finished"; CANCELLED Close / "Cancelled"; ON_HIATUS Pause / "On hiatus"; UNKNOWN and any other value Block / "Unknown".
- Text is `maxLines = 1`, `overflow = Ellipsis`; a dot separator (`DotSeparatorText`) follows; a stub-source warning icon appears when the source is a stub (`isStubSource`).

Mockup: `spec/entries.json` entry screens do not define the status badge; add it in area:38 (ui-entry-details-manga), not here.

## 6. Genre chips and search-by-genre

- Genres are shown from `getGenres()` (behaviour section 10). Tapping a genre triggers `searchGenre(genreName)` (section 3.3 of the filter topic) and opens the browse screen with the matching filter set, or a text search for the genre name when no filter matches.
- Empty genres: no chip row.

## 7. Deep link screen

Files: `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt`; UI file for the screen is not part of this scope (states only).
- States: `Loading` (initial), `NoResults`, `Result(manga)`, `Result(manga, chapterId)`.
- `Result(manga, chapterId)` carries the chapter id. The screen that consumes it is not read in this area (area:36). The reader intent used by the manga screen is `ReaderActivity.newIntent(context, mangaId, chapterId)` (app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt).
- `NoResults` shows the no-results message; the Kotlin text key is not defined in this model (area:36 owns the screen).

## 8. Notifications and warnings tied to scope fields

| Event | Surface | Text key | Source of text (English) | Timeout |
|---|---|---|---|---|
| Bulk download queue warning | notification (download channel) | `AYMR.strings.download_queue_size_warning` | "Warning: large bulk downloads may lead to sources becoming slower and/or blocking Aniyomi. Tap to learn more." | 30 000 ms (MangaDownloader.kt:767) |
| Library update warning | notification (library-size) | `MR.strings.label_warning` + `MR.strings.notification_size_warning` | "Warning" / "Large updates harm sources and may lead to slower updates and also increased battery usage. Tap to learn more." | 30 000 ms (`MangaDownloader.WARNING_NOTIF_TIMEOUT_MS`, MangaLibraryUpdateNotifier.kt:135) |
| Library skip reasons | library update summary | `MR.strings.skipped_reason_not_always_update`, `MR.strings.skipped_reason_completed` | "Skipped because series does not require updates" / "Skipped because series is complete" | n/a |
| Page list empty (download) | download error | `MR.strings.page_list_empty_error` | "No pages found" | n/a |
| Chapter list empty (library) | library error | `MR.strings.no_chapters_error` | "No chapters found" | n/a |
| Download page error | download notification | `notifier.onError(e.message, …)` | exception message | n/a |

Kurobayomi (PROPOSAL): the same texts through area:51 keys; notification thresholds as constants in area:23/area:26; timeouts as `Duration::from_secs(30)`.

## 9. What the mockup does not have (complete list for this scope)

1. Real source list. The mockup's `Source A` / `Source B` rows (`src/screens.rs` near line 359) are invented; the browse tabs in `spec/entries.json` (`Anime Sources`, `Manga Sources`, ...) are spec only.
2. Filter sheet built from `FilterList` (all eight kinds, Reset and Filter sticky header, Sort direction rules, Group nesting, TriState cycle order, Select bounds check).
3. Source settings screen fed by `SourceSetting` (section 3.2).
4. Listing states: loading, results, empty ("No results found"), error formats (section 1.2).
5. Open in WebView action with the source base URL (`WebViewScreen`, area:36).
6. Settings action shown only for configurable sources (`ConfigurableSource` capability).
7. Status badge text and icons (section 5) on the entry screen.
8. Genre search from genre chips.
9. Deep link states (section 7).
10. Warning notifications (section 8).

## 10. Accessibility and layout rules (PROPOSAL)

- All icon-only buttons carry a tooltip and an accessible label (the Kotlin text key).
- Text sizes follow the GTK theme; the sheet never clips labels (ellipsize with tooltip for long source names).
- Minimum touch target 24 px for rows on desktop; full-row click targets.
- Colour is never the only signal: TriState uses an icon plus label; Sort uses an arrow plus accessible text.
- Dark and light: colours come from the GTK theme and Tokyo Night CSS classes only (user memory: match the current theme; no fixed Breeze colours).
