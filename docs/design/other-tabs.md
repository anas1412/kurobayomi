# Other tabs

Updates, History, Browse, More and the Settings pages in the language of `main-window.md`: plain GTK 4, theme colours
and stock style classes only, the spacing scale 4, 8, 12, 16, 24.

## Shared chrome (`src/ui.rs`)

- `ui::page(title, content, end, back)` is the one header row of every page that is not the Library. A sidebar
  destination (`back == false`) shows no title: the sidebar names it. A pushed page shows the back button and its
  title. When the first `end` widget is a `gtk::SearchEntry` it takes the middle of the row (at most 420 px) in place
  of the title, so a search page never shows its name twice.
- `ui::column()` + `ui::scrolled()`: a centred column at most `ui::PAGE_WIDTH` (720 px) wide, 16 px gutters, 12 px
  between a heading and its list, 24 px between groups (`ui::heading` carries the extra 12 above).
- Rows (`ui::row` and `pref_model::widgets::row`, same metrics): 12 px sides, 8 px top and bottom, at least 48 px,
  title in the body size, subtitle `dim-label caption`. A row that opens a page ends in the dim `ui::chevron()`.
- Empty state (`ui::status`): 64 px dim icon over a `kb-empty-title` message, the Library's look; a caller with one
  matching action appends its button.

## Updates, History and Browse

### Audit of the old pages

| Where | What the user saw | Problem |
| --- | --- | --- |
| Updates, History, Browse header | the tab name as title, icons only | Repeats the sidebar; the calendar alone does not say "Upcoming". |
| Under it | a centred Anime / Manga switcher on its own row | A second header row; the Library has the switch in its header. |
| History | a full-width search field under the switcher | A third row; the Library puts its search in the header. |
| History rows | a 68 x 96 blank area where the cover goes (an empty `GtkPicture`) | Looks broken; a click on it also resumed. |
| Updates rows | the grey image icon as cover | The placeholder the Library replaced with initials. |
| Updates selection | close, count, ten labelled buttons, select all and invert in one top row | Too wide for a narrow window; the Library splits it (count row on top, action bar at the bottom). |
| Empty Updates / History | one dim line of text | No icon, no way forward. |
| Browse | title, then a strip of six tabs ("Anime sources", "Manga sources", …) | Six tabs for two choices (kind x section); scrolls sideways when narrow. |
| Browse extensions | a second toolbar with the title "Extensions", refresh and repos | Third title on one page. |
| Anime Sources | its own row of global search and filter icons | The same two actions as the Browse header. |
| Source, extension, migrate rows | flat rows on the window background | Every other list is a boxed list. |
| Badges | "Extensions (2)", a bare "12" for library counts, a bookmark icon for "in library" | Numbers and icons without words. |
| Listing cells | the grey frame placeholder | The Library cards show initials and rounded covers. |

### Header (the main window rule)

A sidebar page whose content starts with its own `GtkHeaderBar` is used as it is (`main.rs`, `tabs`); every other
destination gets `ui::page`. Updates and History use the Library's header (`screens::library_page`):

`[Anime | Manga]   [ search ......... ]   [actions]`

- Updates: no search (Aniyomi has none); actions `Upcoming` (calendar icon and the word) and the refresh of the
  visible kind (spins while the pass runs; F5 does the same).
- History: the visible kind's search in the middle; the Clear history button.
- Browse: `[Anime | Manga]   [ search ]   [extension refresh] [filter] [menu]`, and under it the centred
  `[Sources | Extensions (n updates) | Migrate]` switch. The two switches pick one of the six pages (Ctrl+1…6 still
  work). On an extensions page the entry filters the list as you type ("Search…"); on Sources and Migrate it is
  "Global search…" and Enter opens global search of the visible kind. The menu holds Global search, Update all,
  Extension stores (anime) and Extension repos (manga).

### Lists

- `ui::column` + `ui::scrolled` (720 px, the More and Settings family; extensions keep their 960 px from T-41a-050),
  `ui::heading` over each section, every section a boxed list.
- Rows: 12 px inset, 8 px top and bottom; a cover is the Library's (stored image, else initials) with 6 px corners
  (`browse::look::row_cover`, `.kb-row-cover`).
- Updates selection: the count with Select all, Select inverse and Cancel under the header; the actions in an action
  bar that slides up at the bottom and scrolls sideways when it does not fit.
- History: a click on the cover opens the entry and no longer also resumes.
- Badges in words (`browse::look::badge`, `kb-badge`): `2 updates` on the Extensions switch, `12 in library` on a
  migrate source, `In library` on a listing cell.

### Empty states

`browse::look::empty_state` (`ui::status` and one button): Updates "No recent updates" + Update library; History
"Nothing read recently" + Library (no button for a search without results); Sources "No source found" +
Extensions; Extensions + the repos page;
errors + Retry.

### Kept

Every action and shortcut: selection gestures and keys, bulk and download actions, the download indicator menu,
Upcoming per kind, the new-updates reset, history resume, remove and clear dialogs, add to library with its category
and duplicate dialogs, reselect (resume newest / open global search), the six Browse pages and their order, the
capacity-one tab channel, the debounced extension search, Escape steps, F5, Ctrl+R, Ctrl+U, the update badges and the
stored manga count, pin, latest and options popovers, filters, migrate sort and help.

### Not done here

Pushed pages (global search, a source's listing, extension details, Upcoming, Statistics, Storage) keep their back
and title header; Statistics and Storage still show a centred Anime / Manga switch under it.

### Not checked by eye

Checked by tests only (header structure, switches, search routing, badges, empty states, rows); nobody has looked
at the three tabs in the Tokyo Night theme yet.

## More and Settings

### Audit of the old pages

| Where | What the user saw | Problem |
| --- | --- | --- |
| More header | title "More" | Repeats the sidebar. |
| More top | 64 px multimedia icon and a large "Kurobayomi" | A third name for the app (sidebar header, window); pushes the rows down. |
| More rows | plain rows, no icons | Aniyomi's rows have icons; the three groups read as one grey block. |
| More Help | a row that did nothing | Aniyomi opens the help site. |
| Settings pages | rows 56 px+ with 16 px padding, 16 px titles, subtitles dimmed twice (`dim-label` and 0.78 opacity) | Taller and fainter than every other list in the app. |
| Settings pages | lists as wide as the window | Other pages clamp at a column; a 1600 px row with a switch at the far end is hard to scan. |
| Group titles | `dim-label` text, 14 px above | Weaker than the `heading` the other boxed lists use. |
| Link and choice rows | nothing at the row's end | No hint that a click opens a page or a chooser; spec link rows on the generic page did show an arrow. |
| Wide settings | left list of boxed rows with subtitles in 300 px, right pane with its own `title-4` bar | Two different "sidebars" in one app; Player inside it opened a second sidebar (three columns). |
| Settings search | page title "Search settings" and a search field under it | The same words twice; a blank page before typing. |
| Info rows | icon above text, opacity 0.78 | Android layout; faint. |
| About row | "Aniyomi Stable 0.18.2.1" | Not this app. |
| Worker info, restore backup | `title-4` headings, a square `GtkFrame` | Off the family. |
| Download queue (from More) | a centred Anime / Manga switch on its own row, fixed "Source A (3)" / "Title 1" rows at 45 %, sort and row menus that did nothing, a floating Pause button | Mockup content; no download manager runs in the app yet. |
| Categories (from More) | the same switch row under the header, a floating Add button, a dim line "Tap the plus button…" when empty | Second header row; Android wording; no way forward from the empty page. |
| Custom buttons, Script editor (Player) | a left-aligned "Tap the plus button…" line under an empty boxed list; Retry as a plain line | Android wording; the empty list frame stays. |
| Typed link rows (extension repos, Edit categories, Onboarding guide, Worker info and schema) | no arrow | Spec link rows had one, so the same kind of row looked different from page to page. |
| Tracking | the logged-in check in a fixed green | Ignores the theme. |

### More

- No title, no logo. Three boxed lists in the page column, each row with its icon:
  1. Downloaded only, Incognito mode (switches bound to their preferences).
  2. Download queue (derived state as subtitle), Categories, Statistics, Data and storage.
  3. Settings, Player settings, About, Help (opens the help site in the browser).
- History stays out: it has its own sidebar entry (Aniyomi's moved-tab rule).
- The lists have no headings, so the page column spaces them 24 px apart (the gap between groups).
- Download queue: no download manager runs yet, so the page is the empty state "No downloads" with a Library button
  (no switch, sort, menus or pause button until the manager exists).
- Categories: one header row, `[back] Categories   [Anime | Manga] [+]`; Add creates a category of the visible kind.
  Each kind is a boxed list in the page column (shared row metrics); with no category the empty state "You have no
  categories" with an Add category button.

### Settings

- Every settings page is one family: `pref_model::page::body` builds the same column as `ui::column` (clamped at
  720 px, 16 px gutters, `heading` over each boxed list, 24 px between groups). Pushed pages get the standard header
  (back, title, the page's actions).
- Link rows end in the dim arrow, choice and multi-select rows in a dim `pan-down` arrow; switches, sliders and edit
  rows keep their control at the end. The current value of a choice stays in the subtitle.
- Narrow window (under 720 x 720): the settings list is one boxed list (icon, title, summary, arrow); About shows
  "Kurobayomi <version>".
- Wide window: `pref_model::shell::split` — a `navigation-sidebar` list (icon and title, summary as tooltip, the
  open page selected; the app sidebar's look) and the page beside it. The selected row names the page, so the page
  has no title of its own; its actions sit on a slim row at the top end. Player settings use the same split view on
  their own page, and inside the settings split they show their plain list (no nested sidebar).
- Settings search: the field is in the header; before a query the page says "Type to search every setting", a query
  with no hit shows "No results found".
- Custom buttons and Script editor: with nothing to list the boxed list hides and the empty state shows ("You have no
  custom buttons" + Add a custom button, "This folder has no files" + Add a file); a failed read shows the error state
  with Retry. The floating Add button stays for a non-empty list.
- Every typed row that opens a page ends in the arrow too (extension repos, Edit categories, Onboarding guide, the
  Debug info links). The logged-in tracker check uses the theme's `@success_color`.
- Worker info uses the page column; its long monospace lines scroll sideways inside their row.

### Kept

Every preference binding, the enable rules and live rebuilds, the search-jump highlight, back handling of the shell,
the resize switch between one and two panes, extension store and repo lists with their add button, backup create and
restore, Debug info, Worker info, licenses, and the Player sub-pages.

### Not checked by eye

The pages were checked by tests only (structure, rows, icons, header contents, the split view, the search states,
link and choice markers, the Help launch; every routed settings page is built in a child test process); nobody has
looked at them in the Tokyo Night theme yet.
