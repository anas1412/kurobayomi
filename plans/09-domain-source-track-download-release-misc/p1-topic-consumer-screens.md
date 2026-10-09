# Topic: screens that consume this area's domain code (UI parity from the consumer side)

No file in the scope renders UI, so there is no `p1-ui-parity.md`. This file lists the visible behavior of the screens that are driven by the scope's models and rules, and what the existing mockup (`src/ui.rs`, `src/screens.rs`, `src/settings.rs`, `src/spec.rs`, `spec/*.json`) lacks. The screens themselves are owned by: area:40-ui-browse-manga / area:41-ui-browse-anime (Sources list), area:42-ui-updates-history-stats-storage-upcoming (Clear database, Upcoming, Storage), area:47-settings-screens-general and area:48-settings-player-and-scripts (settings pages), area:45-player-core (player). Where this area adds behavior (validation, state, strings) it is specified here so those areas can implement it without re-reading Kotlin.

Keyboard and mouse equivalents used throughout: Tab/Shift+Tab focus order, Enter activates the default button, Escape closes a dialog (= Cancel), right-click on a list row opens the same menu as the Android long press, Ctrl+F focuses the search entry on list screens.

## 1. Browse > Sources list (`MangaSourcesScreen.kt`, `AnimeSourcesScreen.kt`)

Elements: scrollable list of section headers (text, `header` typography) and source rows. Row (`BaseMangaSourceItem`, `app/src/main/java/eu/kanade/presentation/browse/manga/components/BaseMangaSourceItem.kt:19-64`): source icon (extension icon, area:17/34), first line `source.name` (bodyMedium, one line, ellipsis), second line the language display name (`LocaleHelper.getSourceDisplayName(lang)`, bodySmall, secondary alpha; shown because `showLanguageInContent` defaults to true); trailing buttons: "Latest" text button (`MR.strings.latest`, only when `supportsLatest`), pin icon button (outlined = unpinned, filled primary = pinned; description `action_pin` / `action_unpin`). Tap row = browse Popular; tap Latest = browse Latest; pin icon toggles pin; long press = options dialog.

States: loading (spinner) while `isLoading`; empty (`MR.strings.source_empty_screen` "No source found"); error event `FailedFetchingSources` (toast; list stays empty). Selection: none. Options dialog: title `visualName`; rows "Pin"/"Unpin" (by `Pin.Pinned in source.pin`), "Disable" (`MR.strings.action_disable`, hidden for the local source id 0), (SY, manga) data saver exclude rows (dropped). Section headers: "Last used", "Pinned", language display names ascending, "Other" last (`MR.strings.last_used_source`, `pinned_sources`, `other_source`). Animation: row move/insert animation (`animateItem`). Filter action in the tab app bar opens the "Sources" language/source enable screen (`sources_filter`, mockup screen exists).

Desktop: GTK `ListView` with a section header factory (`gtk::ListHeader` via `GtkSectionModel` is GTK 4.12 `gtk::ListView` + custom `SectionModel` impl); `gtk::ListBox` with header function is simpler and enough for hundreds of sources: `set_header_func` shows a header when `row.section != prev.section`. Right-click or the row's menu button opens a `gtk::PopoverMenu` with Pin/Unpin and Disable. Responsive: one column at all widths; the trailing buttons collapse into a single overflow menu button below 360px.

Mockup lacks: real data (rows are samples), pin toggle state, the Latest button visibility rule, options menu, section ordering and the duplicated "Last used" entry.

## 2. Settings > Downloads (`SettingsDownloadScreen.kt`)

Rows in order and their dependencies are in `p1-behavior.md` 3.1. Dialog details: speed limit dialog (title `download_speed_limit` "Chapter download speed limit"; body hint "Set to 0 to disable the speed limit."; numeric chooser, suffix KiB/s, step 100, min 0; Cancel/OK), category tri-state dialogs (titles "Manga Categories"/"Anime Categories"; each category is a tri-state checkbox: unchecked, checked (include), crossed (exclude); message text; Cancel/OK), list dialogs for "Maximum downloads" (dropped in Kurobayomi), remove-after-read slots and download-ahead choices. Categories subtitle (`getCategoriesLabel(allCategories, included, excluded)`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/Commons.kt:16-52`) is two lines: `MR.strings.include` + included text, newline, `MR.strings.exclude` ("Exclude: %s") + excluded text. Included text: if some but not all categories are included, their `visualName`s joined by `, ` and sorted by category `order`; if all are included (or none included and not all excluded, the final `else` branch) the word `MR.strings.all` ("All"); if every category is excluded `MR.strings.none` ("None"). Excluded text: empty set -> "None"; every category excluded -> "All"; else the sorted `visualName`s joined by `, `. (Category ids in the sets that match no category are ignored.) Disabled rows are drawn insensitive.

Mockup lacks: the speed limit numeric entry (it has a slider only), tri-state category dialogs (dialog entries `auto_download_anime_categories`/`auto_download_manga_categories` have empty `groups`), live category lists, row dependency sensitivity, the External downloader group semantics (Android packages), real values, persistence.

## 3. Settings > Browse > Extension stores / repos

Anime store list (`AnimeExtensionStoresScreen.kt`): app bar title "Extension stores" with a refresh icon button (`action_webview_refresh`); extended FAB "Add"; empty state "You haven't added an extension store yet"; elevated card per store: label icon, `name` (title medium); icon button row aligned right: open website (`public` icon, `action_open_in_browser`), Discord (only when `contact.discord != null`), copy index URL (`action_copy_to_clipboard`), delete (`action_delete`). Dialogs: Create (text field "Store index URL", helper text `*required` or the error string, error state when duplicate or fetch failed; confirm "Add" / "Processing…" disabled while processing; Cancel), Confirm (deep link: read-only field with URL, body text; Add disabled with "This extension store already exists" when present), Delete (title "Remove extension store", body with name and URL; OK/Cancel). Manga repo list: same layout; card shows `name` only, buttons: open website, copy `<baseUrl>/index.min.json`, delete; empty "You have no repos set."; dialogs Create (with the message "Add additional repos to <app name>. This should be a URL that ends with "index.min.json"."), Delete, Conflict ("Signing Key Fingerprint Already Exists", Replace/Cancel), Confirm. Errors surface as toasts (manga) or inline text (anime).

Mockup (`settings.rs` `cards()`) has the right visual structure (tag icon + name + website/copy/delete buttons + Add button) but: hard-coded names, no Discord button, no refresh action wiring, Add is a pill button instead of an extended FAB (fine on desktop), no validation, no processing state, deep link dialog not reachable, store and repo screens share one builder so the copy button copies nothing.

Desktop: refresh = `F5` and the header button; copy uses `gdk::Clipboard`; opening links uses `gtk::UriLauncher`; processing state = spinner inside the dialog's confirm button plus insensitive entry.

## 4. Settings > Player > Custom buttons

Described fully in `p1-behavior.md` 9.4 (dialogs, validation, strings). Mockup (`cards(.., star = true)`) has the list, drag-handle icon (non-functional), star toggle (local toggle only, not persisted, not exclusive), edit (opens dialog `edit_button` which lacks the "(ID: n)" header and validation), delete. Missing: Lua field monospace/highlight, min/max line heights (3-5, 3-5, 2-4), duplicate-name error text, required rules, reorder, persistence, empty state, help link, toast `internal_error`, card body preview of the Lua (3 lines max, ellipsis, alpha 0.75).

Desktop: confirm button sensitivity bound to `!title.is_empty() && !content.is_empty() && !duplicate`; the star is a `gtk::ToggleButton` bound to `is_favorite` with exclusive behavior enforced by the service (list refresh after commit). Keyboard: Delete key on a focused row asks to delete; Alt+Up/Down reorders.

## 5. Settings > Advanced > Clear database (manga and anime)

App bar title `AYMR.strings.pref_clear_anime_database` (manga: its twin), back arrow; actions (only when the list is non-empty) Select all and Invert selection (upstream defect: the invert action carries the title `action_select_all` too; Kurobayomi labels it "Invert selection", key `action_invert_selection`). List: 56dp rows, source icon, `visualName` (bodyMedium), second line `MR.strings.clear_database_source_item_count` ("%1$d non-library entries in database"), selected rows tinted, whole row toggles selection. Divider, then a full-width "Delete" button (`MR.strings.action_delete`), enabled only when the selection is non-empty. Empty state `MR.strings.database_clean`. Delete opens a confirmation dialog with text `AYMR.strings.clear_database_confirmation` (anime: "Are you sure? Completed episodes and chapters and progress of non-library entries will be lost"; manga screen uses `MR.strings.clear_database_confirmation`: "Are you sure? Read chapters and progress of non-library entries will be lost"); during load a spinner. The anime screen builds the id lists with the season-tree walk (behavior 1.9); count shown = `ids.len()`. Mockup (`clear_db` in `settings.rs:229`) shows 12 x i sample counts; lacks selection toolbar, confirmation and real data.

## 6. About / new version

About: version row (copies debug info), "Check for updates" row with spinner (only when the updater is enabled), "What's new" (non-debug builds), help translate etc. (area:47-settings-screens-general). New-version page (`NewUpdateScreen`): icon `NewReleases`, heading "New version available!", subtitle version, markdown changelog, "Open on GitHub" button, bottom buttons "Not now" (pops) and "Download" (starts the update). Mockup `new_update` screen exists with these labels; lacks Markdown rendering and the three actions' wiring.

## 7. Upcoming

Calendar header with month selection and a list grouped by date; events map date -> count; header indexes for scroll-to-date (UI in area:42). Data: `GetUpcomingManga/Anime` stream (behavior section 10), items grouped by `expectedNextUpdate.toLocalDate()`; header count = number of entries in the group. Entries with `status == COMPLETED` have no expected date and are excluded by the SQL status filter. Mockup: no screen; area:42 to add.

## 8. Strings introduced by Kurobayomi (new keys)

`custom_button_script_error`, `custom_button_lua_missing`, `store_error_network`, `store_error_http`, `store_error_format`, `store_error_legacy_url`, `store_error_url`, `storage_move_notice`, `update_check_failed` (all defined with English text in the sections above or below; i18n catalog is area:01/02):

| Key | English |
|---|---|
| `custom_button_script_error` | Custom buttons script has an error. See the log for details. |
| `custom_button_lua_missing` | This mpv build has no Lua support; custom buttons are disabled. |
| `store_error_network` | Could not reach the store. Check the address and your connection. |
| `store_error_http` | The store answered with HTTP error {code}. |
| `store_error_format` | The address did not return a valid extension store. |
| `store_error_legacy_url` | Legacy stores must use an address ending in /index.min.json. |
| `store_error_url` | Enter a valid https address. |
| `storage_move_notice` | Existing downloads and backups stay in the old folder. Move them yourself, then reindex downloads. |
| `update_check_failed` | Could not check for updates: {reason} |
| `repo_key_changed` | Signing key changed: not updated |
