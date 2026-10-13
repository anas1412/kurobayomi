# UI shared-piece audit (src/ only, read-only, no cargo run)

Method: grep sweep of src/ (tests excluded) plus reading of every shared piece named in the brief. "Callers" were found by grep, so confirm with the compiler before deleting anything marked dead.

## Status 2026-10-13

Done since the audit: F1 (shared selection header on Library, Updates and the entry page), F2 (`look::SMALL`/`MEDIUM` row
covers), F3 (real cover art on season and related tiles), F8 (one toast layer), F9 (one entry page for both kinds),
F11 (one search width), F12 (the queue page scrolls), F13 (the dead pieces were deleted by the dead-code cleanup),
F14 (theme colours), F15 (`ui::row_title`), the global search box (shared search bar). Still open and small: F4 (three empty-state helpers:
`look::empty_state`, `look::empty_state_with`, `browse::view::empty_state`), F5 (10 spinners, one `ui::loading`), F6 (two
`HeaderBar::new()` left in `ui.rs`), F7 (7 hand-built popovers), F10 (linked buttons and StackSwitcher side by side),
F16, F17 (box spacing numbers), F18 (dialog widths, a few more English button literals: reporter, queue "Clear", tracker
"Connect AniList"), F19. They are listed in the later-list memory; none changes behaviour.

## Summary table

| # | Pattern | Count | Impact | Proposed shared piece |
|---|---------|-------|--------|-----------------------|
| F1 | Selection mode UI built 4 different ways | 4 screens | high | `ui::selection_bar(...)` + `ui::selection_header(...)` |
| F2 | Row cover sizes and row metrics differ per screen | 7 row kinds | high | `look::row_cover` fixed presets + `ui::list_row_box(...)` |
| F3 | Anime season grid and related items never show cover art (`ui::cover` is a grey stub) | 3 sites | high | `look::cover(title, art, w, h)` |
| F4 | Empty/error state built 4 other ways besides `look::empty_state` | 12 sites | medium | one `look::empty_state` (+ actions variant) |
| F5 | Loading spinner built ~20 ways, 5 sizes | 20 sites | medium | `ui::loading(label: Option<&str>) -> GBox` |
| F6 | Header bars and Back buttons outside `ui::page` | 8 sites | medium | `ui::back_button()` + `ui::title_label()` + `ui::page_custom_header` |
| F7 | Popover menus of flat buttons hand-built (spacing 0/2/6, margins 0/6) | 9 sites | medium | `ui::popover_menu(entries)` / `ui::menu_column()` |
| F8 | Toasts / banners / snackbars: 4 toast + 6 banner implementations | 10 sites | medium | one `toast` layer; one `ui::banner(...)` |
| F9 | Anime vs manga entry page: copy-pasted view code that already differs visibly | ~15 fns | medium | shared `entry_core::page_chrome` (FAB, menus, selection, banner, scroller) |
| F10 | Anime|Manga / choice switch built by hand (3 ways) | 6 sites | medium | `ui::linked` reused, or `ui::kind_switch` |
| F11 | Search entry in header: widths and margins differ | 9 sites | low-med | `ui::search_entry()` + `ui::SEARCH_WIDTH` everywhere |
| F12 | Dialog body scrolls and checklists hand-built inside `ui::dialog` | 5 sites | low-med | export `ui::scrolled_max`; `ui::checklist_dialog(...)` |
| F13 | Dead or shadowed duplicates of shared pieces | 10 files | low (cleanup) | delete |
| F14 | Colours not from the theme (kuro_* palette, hard-coded hex/rgba) | 8 sites | medium | named GTK colours / `warning`, `accent` |
| F15 | Row title style differs (bold `heading` vs plain) | 6 vs 5 screens | medium | one `ui::row_title()` |
| F16 | Settings page scaffold re-implements `ui::column`/`ui::row` | 2 | low | `pref_model` builds on `ui::column`, `ui::row` |
| F17 | Spacing/size numbers that differ for the same job (collected at the end) | many | medium | constants in `ui.rs` |
| F18 | Dialog widths, button labels (English literals vs `tr`) | 28 dialogs | low | `ui::DIALOG_W_*`, always `tr()` |
| F19 | Style classes applied but never defined / defined but never applied | 8 | low | add rule or drop class |

---

## F1. Selection mode: four screens, four UIs (impact high)

Same job (select rows, show count, select all/invert/cancel, bulk actions), four looks:
- Library: count label (`title-2`) + buttons row under header, bottom `gtk::ActionBar` in a Revealer. `src/library/view/mod.rs:165-241,580-615`.
- Updates: a copy-paste of the Library one (`sel_header` with SPACE_L/SPACE_S margins, `ActionBar`, bar scroller). `src/updates/updates_screen.rs:116-160,778-794`.
- Manga entry page: select-all / invert as icon buttons in the header, bulk bar is a plain Box with class `toolbar`, no count, no cancel. `src/entry_core/route.rs:237,288-300,884-960`; `bulk_bar.rs`.
- Anime entry page: a cancel + count + select-all + invert "toolbar" Box in a SlideUp Revealer, plus a second bottom "toolbar" Box. `src/entry_core/anime/route.rs:394-410,841-884`.

Replace with: `ui::selection_header(count_label, on_all, on_invert, on_cancel) -> GBox` and `ui::action_bar(buttons) -> Revealer` in `src/ui.rs`. Both entry pages and Library/Updates call them.
Impact: high. Same gesture looks different on each page; the manga page has no way to see the count or cancel from the bar.

## F2. List-row anatomy and cover sizes (impact high)

Every list row is a hand-built Box with its own margins and cover size:

| Row | file:line | cover (w x h) | margins start/end/top/bottom | min height |
|-----|-----------|---------------|------------------------------|-----------|
| `ui::row` (settings, boxed lists) | `ui.rs:329` | none | 12/12/8/8 | 48 |
| pref row | `pref_model/widgets/row.rs:40` | none | 12/12/8/8 (constants alias ui) | 48 |
| `screens::list_row` | `screens.rs:174` | prefix | 12/12/8/8 | 48 (only `live.rs:141` uses it) |
| Updates feed | `updates/updates_screen.rs:384-390` | 40 x 44 (56-2*6) | 12/12/0/0 | 56 |
| History | `updates/history_row.rs:42-48` | 53 x 80 | 12/8/8/8 | 96 (its own `ROW_MIN_HEIGHT=96` shadows `ui::ROW_MIN_HEIGHT=48`) |
| Upcoming | `updates/upcoming/page.rs:279-285` | 53 x 80 | 12/12/8/8 | 96 |
| Storage | `updates/storage_row.rs:37-41` | 48 x 48 | 8/8/8/8 | none |
| Browse list cell | `browse/view.rs:410-418` | 40 x 40 | 12/12/8/8 | none |
| Browse source row | `browse/view.rs:186-196` | 40 x 40 | 12/8/8/8 | none |
| Library list card | `library/view/card.rs:170-175` | row_height*2/3 x h | 16/16/0/0 | computed |
| Season list row | `entry_core/anime/season_grid.rs:126-131` | h*2/3 x h | none | row_px |
| Download queue row | `downloads/queue_page.rs:146,170` | none | none, plain Boxes spacing 8 | none |
| Chapter/episode (`kb-item`) | `style.css` `.kb-item` | none | 12 side / 8 | CSS |

Replace with: in `ui.rs` add `ui::list_row_box(cover: Option<Widget>, title, subtitle, suffix) -> ListBoxRow` built from `ROW_PAD_H/ROW_PAD_V/ROW_MIN_HEIGHT`, and in `look.rs` add `ROW_COVER_SMALL = (40, 56)` (Updates, Browse) and `ROW_COVER_LARGE = (53, 80)` (History, Upcoming) so one number set exists. Updates/Browse should share 40x56 (Browse is 40x40 square today).
Impact: high. This is the most visible inconsistency between sibling tabs (Updates, History, Upcoming, Storage, Browse).

## F3. Season grid and related items have no cover art (impact high, verify)

`ui::cover(w,h)` (`ui.rs:369`) is a grey frame with a generic icon; its own comment says "stands in until there is an image loader". It is used for real content:
`entry_core/anime/season_grid.rs:103,131` and `entry_core/anime/related_view.rs:18`. `look::cover(title, art, w, h)` (`browse/look.rs:45`) already does the real image or initials tile.
Fix: pass the `ArtRef` into those tiles and call `look::cover`; delete `ui::cover` and `theme/cover.rs::placeholder` (F13).
Impact: high if the model has art for these items (user sees grey boxes instead of covers).

## F4. Empty / error / "missing" states (impact medium)

The shared one is `look::empty_state` over `ui::status` (icon 64 + `kb-empty-title`). Everything below does it differently:
- `browse/view.rs:38 empty_state(message, actions, on_action)`: text only, no icon, own buttons-with-icons row (used by `browse/listing_view.rs:520`). Same page type as `pages.rs:88` which uses the shared one for its error. Fold the action-row into `look::empty_state` (accept `&[Button]`).
- Plain centred `Label`, no icon, no `kb-empty-title`: `browse/source_prefs_page.rs:102`, `browse/migrate_screen.rs:75,83` (the same label twice), `browse/lang_pages.rs:107 empty_label`, `browse/listing_view.rs:138`, `browse/pages.rs:209-215` (source not installed).
- `ui::dim` caption line as the empty state: `entry_core/route.rs:165`, `entry_core/anime/route.rs:241` (chapter/episode list is empty).
- Hand column with `title-2` label: `shell/deep_link_page.rs:87-120` (own Retry button, title, spinner).
- `screens.rs:277-300 info_screen` (new_update, crash): hand layout, `title-1`, a Frame with a Label; `reporter.rs:41-95` is a third copy of "crash page".
- Dead alternatives: `shell/empty.rs` (icon + dim label, max 2 actions) and `theme/empty_state.rs` (kaomoji faces, max 3 per row) have no callers (F13).
- Settings: `settings_ui/search.rs:224-226` and `settings.rs:99` call `ui::status` directly (fine) but are duplicated (F13).
Replace with: only `look::empty_state(icon, message, actions: &[Button])`. Retry goes through one helper `look::error_state(message, on_retry)` (the Updates `state_page.rs:29` and Browse `pages.rs:85` both build the Retry button by hand, `shell/deep_link_page.rs:108` a third time, with an English literal "Retry").
Impact: medium. Some blank pages show a lone label, others icon + message + Retry.

## F5. Loading spinner (impact medium)

About 20 `gtk::Spinner` builds; sizes in use: 16 (`browse/primitives.rs:7`, `view.rs:17 spinner_row`), 32 (`entry_core/page_shell.rs:19`, `pref_model/stores_page.rs:194`, `toast.rs:28`, `startup.rs:121`), 48 (`browse/migrate_dialog.rs:213`, `player_view/mod.rs:307`, `reader/progress.rs:11`), and default size (no request) at `updates/state_page.rs:22`, `source_gate.rs:54`, `entry_core/route.rs:164`, `anime/route.rs:240`, `settings_ui/storage` rows. Some show a "Loading..." label (`page_shell.rs:23`, `view.rs:30`), most do not. `theme/loading.rs::delayed_spinner` (waits 300 ms to avoid flicker) exists and is called by nobody.
Other sites: `pref_model/backup_pages.rs:236`, `settings_ui/about.rs:192,218`, `updates/storage_screen.rs:94`, `updates/mount.rs:206`, `shell/deep_link_page.rs:100`, `backup_host.rs:92`, `player_view/sheets/quality.rs:194`, `reader/screen.rs:722`, `reader/transition.rs:283`, `reader/navigator.rs:109`.
Replace with: `ui::loading(label: Option<&str>) -> GBox` (32 px spinner, centred, delayed 300 ms via `theme::loading`), used by `state_stack`, `PageStates`, `source_gate`, `spinner_row`, stores page, source prefs, lang pages, migrate screen, listing view. Inline small spinners (buttons, rows) keep one `ui::spinner_small()`.
Impact: medium (size and label differ between the same "loading list" state on different tabs).

## F6. Headers and Back buttons outside `ui::page` (impact medium)

`ui::page` (`ui.rs:129`) is the way. Hand-built headers / Back buttons:
- `browse/header_bar.rs:38-55` (source page) and `:67-77` (`stub_header`), `browse/shell.rs:85-110` (Browse tab header, own `pack_start/pack_end`, search as title widget, skips `SEARCH_WIDTH` clamp: `set_width_chars(28)` vs 420), `screens.rs:225-245 library_page` (header bar built by hand for Library, Updates, History).
- `pref_model/page.rs:151-163` (`HeaderBar::new()` not `ui::header_bar()`, so it is outside the equal-height size group and has no decoration layout; Back tooltip from `strings::back()`).
- `shell/webview.rs:136-150` (`HeaderBar::new()` again, Back tooltip via `back_label()`).
- `widgets/page_header.rs:57` (reader/player; uses `ui::icon_button(.., "Back")`).
- `entry_core/page_shell.rs:71 header_with_back` appends a second Back button next to the one `ui::page(.., back=false)` did not add (works, but it is a third Back implementation).
Back button definitions: `ui.rs:144`, `page_shell.rs:74`, `browse/header_bar.rs:42,70`, `pref_model/page.rs:158`, `shell/webview.rs:142`, `widgets/page_header.rs:61`. Tooltips: literal `"Back"` (ui.rs), `tr(action_webview_back)` (others), `strings::back()` (settings). Only `ui::page` sets the accessible label.
Replace with: `ui::back_button(on_click) -> Button` (tooltip `tr(action_webview_back)`, accessible label, no sticky focus) and `ui::title_label(text)`; give `ui::page` an `ui::page_with_header(header, content)` variant so Browse, Library, source page and settings keep their custom centre widget but share the bar, size group and decoration layout.
Impact: medium (a11y label, header height jumps, English-only tooltip).

## F7. Popover menus (impact medium)

Same pattern, "Popover with a Box of flat Buttons that close the popover and run":
- `ui.rs:394 menu_button` (spacing 0, margin 0) - used only by `browse/shell.rs:227` and `ui.rs:781`.
- `entry_core/page_shell.rs:85 plain_row` + `entry_core/route.rs:993,1008` and `anime/route.rs:1200,1303` (spacing 2, margins 6).
- `browse/header_bar.rs:117-135` (overflow, spacing 0, margin 0); `:82-110` radio display menu (CheckButton, spacing 0).
- `shell/webview.rs:166-191` (spacing 0).
- `entry_core/description_view.rs:36-60` (genre chip, spacing 2, margin 0).
- `library/view/mod.rs:608-624` (bulk Download menu, spacing 2, plain buttons without `flat`).
- `player_view/sheets/mod.rs:27-44` (spacing 6, margin 6, boxed list of check rows) and `widgets/download_button.rs:114-130` (one Delete button).
- `browse/filter_sheet.rs:252` (popover of 360 x up to 480 hosting the filter form) vs the `ViewOptions` menu in `widgets/view_options.rs` for other sort/filter menus.
Spacing in use: 0, 2, 6. Margins in use: 0 and 6. `ui::quiet_menu` is applied to some MenuButtons only: `browse/header_bar.rs:82,117` and `entry_core/tracking/page.rs:360` and `browse/pages.rs:346` create MenuButtons without it (focus sticks to the button after the popover closes).
Replace with: `ui::popover_menu(entries: Vec<(String, Rc<dyn Fn()>)>) -> gtk::Popover` (one spacing, one margin, flat buttons, popdown then run, optional `sensitive`), and make `ui::menu_button` take it. Download menus of manga and anime pages then become one `entry_core::download_menu(kind)`.
Impact: medium.

## F8. Toasts and banners (impact medium)

Toasts (all "label at the bottom, auto-hide"):
1. `toast.rs` global layer (2 s / 3.5 s) with `osd` label and `busy` spinner card.
2. `library/toast.rs` own presenter, 4 s, Revealer.
3. `updates/screen_toast.rs` own presenter, 4 s, queue of 3.
4. `browse/view.rs:64 toast_bar` with class `toast` (a libadwaita class, no rule exists in style.css, so unstyled) used by `browse/listing_view.rs:539`.
Plus `entry_core/banner_view.rs` (snack with action, `osd`, margins 24/12) for entry pages.
Replace with: extend `toast.rs` with `notify_for(text, duration)` and `notify_action(text, action_label, f)`; delete 2, 3 and 4. Differences today: manga/anime pages show a snack with Retry, Library 4 s, global 2 s.

Banners (top strips): `shell/banners.rs` (indexing/downloaded only/incognito, coloured by `kuro-banner-*` palette classes), `shell/warning.rs` (`kuro-warning-banner`), `incognito.rs:50-62` (builds its own banner that `main.rs:235` discards with `let _`), `backup_host.rs:92-97` (class `toolbar`, margins 12/6, spinner), `browse/banner.rs InfoBanner` (no styling), `entry_core/banner_view.rs` (osd, bottom). Replace with one `ui::banner(icon, text, action) -> Revealer` styled with stock `.toolbar`/`.info`/`.warning` look; remove the kuro-banner classes (F14).
Impact: medium.

## F9. Anime vs manga entry page copies (impact medium)

`entry_core/route.rs` (1438 lines) and `entry_core/anime/route.rs` (1862 lines) carry parallel UI code. Same-named methods: `assemble`, `render`, `download_popover` (route.rs:993 / anime:1303), `overflow_popover` (1008 / 1199), `open_interval` (601 / 1012), `open_tracking`, `tracking_pressed`, `webview_pressed`, `migrate`, `share`, `list_option`, `use_as_default`, `open_source_preferences`, `key_controller`, `handlers`, plus separate files for the pairs `header.rs`/`anime/header.rs`, `selection.rs`/`anime/selection.rs`, `dialog.rs`/`anime/dialog_state.rs`, `chapter*.rs`/`anime/episode_row.rs`, `backend.rs`/`anime/download_backend.rs`.
Visible divergences already (same widget, different code):
- Scroll area: manga `ui::scrolled(&col)` (`route.rs:262`), anime hand `ScrolledWindow` (`anime/route.rs:331-336`, no 720 clamp).
- FAB: manga wraps in a Revealer with margins 16 (`route.rs:261-269`), anime sets margins directly (`anime/route.rs:361-366`, `set_visible`).
- Overflow button tooltip: manga none (`route.rs:288`), anime set (`anime/route.rs:422`). Refresh: anime has a header button (`:419`), manga only in the overflow menu.
- Selection UI: see F1. Snack/banner: manga `banner_view` at overlay, anime `snack` at overlay (same widget, different wiring).
- Airing row margins 16/16/8/8 (`anime/route.rs:349-353`) vs item header row 16/16/6/6 (`page_shell.rs:48-52`).
Replace with: move FAB wiring, download menu, overflow menu, selection bar, scroller+TwoPane and banner into `entry_core/page_chrome.rs` (parameterised by a small `Kind` struct of labels/callbacks).
Impact: medium (consistency of the two details pages; also halves future bugs).

## F10. Segmented switches and Anime|Manga switches (impact medium)

- `ui::linked(labels, active)` (`ui.rs:376`) - used only by spec items. Same loop re-written: `browse/shell.rs:55-70 switch()` (group, linked, no `set_active` of first), `pref_model/widgets/theme_mode.rs:39-50` (linked, own loop), `ui::choice_pills` (`ui.rs:254`, not linked, pills with margin 8/4).
- Anime|Manga switch: `gtk::StackSwitcher` in Library/Updates/History/Storage/Categories (`screens.rs:147,152,234`, `updates/mount.rs:57`) but `browse/shell.rs` uses linked ToggleButtons. Looks the same only by luck; keyboard/accessibility differ.
- Sidebar tabs `StackSwitcher` (`shell/sidebar.rs:308`); `ui::show_sheet` StackSwitcher in a modal title (`ui.rs:601`).
Replace with: `ui::linked` as the only segmented control (make it return the buttons so Browse and theme mode use it); `ui::kind_switch` only if the StackSwitcher look is wanted everywhere.
Impact: medium.

## F11. Search entries (impact low-medium)

Header search widths: Library `set_width_chars(28)` (`library/view/mod.rs:177`), History `28` (`history_screen.rs:85`), Browse tab `28` (`browse/shell.rs:83`), settings search `hexpand` clamped to `SEARCH_WIDTH=420` by `ui::page` (`settings_ui/search.rs:219`). In-page search: Browse source `margin 12/12/6/0` in a Revealer (`browse/pages.rs:247-253`), global search margins 12/12/6 in a Box spacing 6 (`pages.rs:521-528`), tracker search inside a `ui::column` (16 gutter) (`entry_core/tracking/page.rs:110`). `shell/search.rs:67` (plain `Entry` + reset button) has no callers. `settings.rs:95` has a fourth. `placeholder`/accessible label only set on some.
Replace with: `ui::search_entry(placeholder) -> SearchEntry` (placeholder, accessible label, width via one constant) and `ui::search_row(entry, extra)` for in-page search (16 gutter, 8 top).
Impact: low-medium; the width differs when moving between tabs.

## F12. Scroll areas and dialog bodies (impact low-medium)

`ui::scrolled` / `scrolled_max` (private fn, `ui.rs:553`) exist. Hand-built scrollers:
- Dialog checklists that scroll at 360 inside `ui::dialog` (which already scrolls at 560): `library/view/dialog.rs:59`, `entry_core/category_dialog_view.rs:36`, `entry_core/scanlator_sheet.rs:127`, `updates/history_dialog.rs:67`. Three of them rebuild the same boxed list of CheckButtons ("Set categories" in the Library, "Edit categories" on entry pages, History categories: `library/view/dialog.rs`, `entry_core/category_dialog_view.rs`, `updates/history_dialog.rs`, plus `anime/route.rs:1096 show_category_dialog`).
- Page scrollers that bypass `ui::scrolled` (so no 720 clamp): `browse/source_prefs_page.rs:98`, `browse/lang_pages.rs:117`, `browse/pages.rs:547`, `browse/listing_view.rs:130`, `pref_model/page.rs:83`, `anime/route.rs:331`, `entry_core/info_pane.rs:45`, `downloads/queue_page.rs` (no scroller at all: rows are appended to a Box, so a long queue cannot scroll on its own).
- Horizontal scrollers: `library/view/tabs.rs:22`, `entry_core/anime/related_view.rs:80`, `browse/pages.rs:682`, `updates/updates_screen.rs:142` (fine, but four copies of the "never vertical + natural height" setup).
Replace with: make `ui::scrolled_max` public; add `ui::checklist(items) -> (ListBox, Vec<CheckButton>)` and one `category_dialog` used by Library, entry pages and History.
Impact: low-medium. The queue page missing a scroller is a real bug if more rows than the window fit.

## F13. Dead or shadowed duplicates (impact: cleanup, safe to delete after a compile)

No callers found by grep (outside their own tests):
- `src/theme/empty_state.rs` (kaomoji empty screen) and `src/shell/empty.rs` - two more empty states.
- `src/shell/search.rs` `toolbar()` - search toolbar.
- `src/theme/loading.rs::delayed_spinner` - unused; revive via F5.
- `src/theme/cover.rs::placeholder` - fourth cover placeholder (`ui::cover`, `look::cover` initials, `browse/icons.rs::placeholder_rgba` with `.kb-cover-placeholder` CSS).
- `src/settings.rs:17,92-130 search()` and its `Hit` index: shadowed by `settings_ui/search.rs` (`settings_ui/route.rs:31,338` handles `settings_search` first).
- `src/incognito.rs` banner built in `attach` (returned widget dropped at `main.rs:235`); `shell/banners.rs` shows the banner instead.
- `src/screens.rs:174 list_row` (one caller, `live.rs:141`) and `screens.rs:277 info_screen` (crash/new_update stub pages; `reporter.rs` is the real crash UI).
- `src/pref_model/page.rs:42 build()` with its own `header_bar()` (only `body()` is used by the shell).
- `src/entry.rs:20 entry()` has a `ponytail:` note to delete.
- Style classes with rules but applied nowhere outside tests: `kb-surface`, `kb-pill`, `kb-tab-pill`, `kb-hit-target`, `kb-noindication`, `kb-info-bar`, `kb-sheet*`, `kb-button-filled`, `kb-button-text`, `kb-header`, `kb-active`, `kb-primary-icon`, `kb-fast-*`, `kuro-pill`, `kuro-group-header`, `kuro-check`, and the `kuro-*` type-scale classes (`theme/tokens.rs`, `theme/components/mod.rs`).

## F14. Colours not from the theme (impact medium)

The design rule is stock-theme look with named GTK colours. Exceptions:
- `@kuro_*` palette (MD3 roles defined by `theme/tokens.rs`, `define_colors_css`): `.kuro-banner-primary/secondary/tertiary`, `.kuro-warning-banner`, `.chapter-primary` (`@kuro_primary`), `.episode-tertiary` (`@kuro_tertiary`) in `style.css`. On Adwaita/Breeze these are not the theme's colours (a Breeze user gets a different accent on the bookmark and the unread dot than on every other accent in the app). `updates/updates_screen.rs:410-418` already works around this with `accent` and `warning` classes for the same icons, so the chapter list and the Updates list colour bookmarks differently. Use `accent` / `warning` / `@theme_selected_bg_color`.
- `src/theme/style.css`: `.kb-splash #54759E/#202125` (splash only, acceptable), `rgba(136,136,136,0.1216)` in `.kb-cover-placeholder` duplicated in `browse/icons.rs:130 placeholder_rgba` and `theme/cover.rs`.
- `settings_ui/player/highlight.rs:19-25` hard-coded GitHub-dark syntax colours used in any theme (unreadable on a light theme?). Use theme-derived or two light/dark sets.
- `reader/progress.rs:36-39` white cairo colours, `.reader-bg-*` fixed on purpose (reader), `entry_core/header_view.rs:150` uses `lookup_color` (fine).
- `storage_row.rs:20-30` fixed per-entry dot colours (data colour, fine).
Impact: medium for the kuro_* items and the highlight palette; the rest is fine.

## F15. Row title style (impact medium)

Updates feed (`updates_screen.rs:424`), History (`history_row.rs:67`), Upcoming (`upcoming/page.rs:304`), Storage (`storage_row.rs:51`), season/related tiles (`season_grid.rs:135`, `related_view.rs:88`), Library list `kb-card-title` use the bold `heading` class. `ui::row` (all settings/boxed lists), Browse source rows (`browse/view.rs:197-203`), Browse list cell (`view.rs:419`), sources and migrate lists use the plain body label. The same list-of-entries looks bold in one tab and light in the next.
Replace with: `ui::row_title(text)` used by every row builder (one weight, one ellipsize).

## F16. Settings scaffold re-implements `ui::column` / `ui::row` (impact low)

`pref_model/page.rs:57-69` builds its own column (margins 16/16/16/24, spacing `GROUP_SPACER=24`, `clamp(PAGE_MAX_WIDTH)`) instead of `ui::column()` (12 spacing, heading adds 12 margin = 24, same total); `pref_model/widgets/row.rs` rebuilds `ui::row` (box spacing 0 with `TRAILING_WIDGET_BUFFER 12`, icon column) and `widgets/group.rs` rebuilds `ui::heading` with `margin_bottom=12` instead of `margin_top=12`. Split panes in `pref_model/shell.rs:178` use `SIDEBAR_WIDTH 240`, the main sidebar `rail::WIDTH 200`. Settings sidebar list `pref_model/shell.rs:186`, main sidebar `shell/sidebar.rs:214,258` build `ListBox` + rail margins separately.
Keep the pref-specific parts (flash, revealer) but build on `ui::column/ui::row` so a change to row metrics changes everything.

## F17. Numbers that differ for the same job

Page gutter (start margin of page content): `ui::column` 16; Library title/hint/cards 16 (`SPACE_L`) / tabs `CARD_EDGE` + end 12 (`tabs.rs:20`); Browse search/chips/global-search column 12 (`browse/pages.rs:249,313,525,538`), browse sort row 16 (`pages.rs:73`); `lang_pages.rs:85` 16/16/8/8; entry item header 16 (`page_shell.rs:51`), card 16 (`list_view.rs:133`), info block 16 (`header_view.rs:196`, `action_row_view.rs:89`), banner 12 (`banner_view.rs:31`), filter sheet 12 (`filter_sheet.rs:36`), queue page 16 / spacing 8 (`queue_page.rs:32-36`), storage page 16/bottom 24 (`storage_screen.rs:63`), Browse list cells 12, grid 8. Result: 12 vs 16 px content inset between sibling tabs.
Top-of-page pills (`pill_row`, margin top 8, spacing 8): Library tabs start at `CARD_EDGE`, Browse chips at 12, queue actions at 16, so the pills sit at three different x positions.
Row insets: see F2 table (12/12/8/8, 12/8/8/8, 8/8/8/8, 16/16/0/0, 12/12/0/0).
Box spacing histogram (`Box::new(.., n)`): 0 x126, 8 x39, 12 x39, 6 x30, 2 x26, 4 x23, 16 x4.
Spinner sizes 16/32/48/default (F5). Icon sizes: row icon 40 (sources), 48 (storage cover), cover placeholder icon w/3; header icon 112 (`DETAILS_ICON_PX`). Cover aspect: 2:3 in cards/History/Upcoming, 1:1 in Browse list and Storage.
Dialog body margin: `ui::dialog` 18, `filter_sheet` 12, `pref dialog` column spacing 16 vs 12 default (`pref_model/widgets/dialog.rs:73`), `cloudflare_window.rs:36-44` 12.
Proposal: add to `ui.rs` `PAGE_GUTTER=16`, `ROW_COVER_*`, `SPACE_*` (the Library's `SPACE_XS..L` in `library/layout.rs:78` re-exported as `ui::SPACE_*`, since Browse/Updates/History already import it from `library::layout`), and use them in the sites listed above.

## F18. Dialogs

All real dialogs go through `ui::dialog` (good). Remaining oddities:
- Hand `gtk::Window` dialogs: `cloudflare_window.rs:29-60` (no header bar, own margins 12, English-less buttons from `tr`), `shell/webview.rs` (full page), `startup.rs:132` splash and `reporter.rs:97` (separate process): only the first should move to `ui::dialog`.
- Width in use: 320, 360 (x12), 380, 400 (x5), 420 (x8), 520. Pick two: 360 (forms) and 420 (text).
- Button labels as English literals instead of `tr(...)`: "Close", "OK", "Cancel", "Set categories", "Stop tracking", "Keyboard shortcuts" etc. (`reader/settings_window.rs:52`, `library/view/dialog.rs:22,66,70`, `entry_core/shortcut_help_view.rs:29`, `entry_core/tracking/page.rs:425`, `settings.rs:310-319`, `settings_ui/library_export.rs:96`).
- `ui::show_dialog_then` picks Destructive by sniffing the label text ("delete"/"remove", `ui.rs:722`): breaks when the label is translated.

## F19. Style classes

Applied but no CSS rule anywhere: `kb-card` (`library/view/card.rs:150,176`), `kb-library-header` (`screens.rs:232`), `kb-sidebar` (`shell/sidebar.rs:294`), `toast` (`browse/view.rs:68`), `chapter-error` (only a test label). Harmless, but they look like styling that is missing (the Browse toast has none, F8).
No inline CSS strings or extra CssProviders outside `theme/`: `theme/apply.rs` is the only place (rule satisfied).

---

## Suggested order

1. F3, F1, F2 (visible, user-facing), then F4/F5/F7 helpers (mostly mechanical replacements).
2. F8 + F14 together (decide: no `kuro_*` colours, one toast layer).
3. F6, F10, F11, F12, F15, F17 (small helpers in `ui.rs`).
4. F9 last (larger refactor), F13 deletions whenever a compile is available.

Not audited in depth: `reader/*` and `player_view/*` (immersive screens with their own chrome; they do use `ui::dialog`), tests.
