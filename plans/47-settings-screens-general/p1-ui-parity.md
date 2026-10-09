# Area 47 (part 1) — UI parity

Scope: the rendered settings screens in the 18 scope files. Each section lists every visible element, its states, dialogs, feedback (toasts), menus, gestures, animations, keyboard equivalent on the desktop, and what the current mockup (`spec/settings.json`, `src/settings.rs`) lacks. Element labels are the English strings from `p1-behavior.md` section 18 (key in brackets where useful).

Desktop conventions used throughout (Proposal): rows are `gtk::ListBoxRow`; activation = click or Enter/Space; Escape cancels a dialog; Tab moves between rows and controls; `Ctrl+F` opens the settings search; `Alt+Left` or the back button pops a screen.

---

## 1. Global chrome (all settings screens)

| Element | Android (observed) | Desktop (Proposal) | Mockup gap |
|---|---|---|---|
| Top bar | `AppBar`: title, up arrow (when a back handler exists), optional actions, pinned scroll behaviour | `page()` header bar with title, back button, optional end actions | none |
| Group header | secondary colour, `bodyMedium`, 14 dp above, 8 dp below | dim label (`.heading`-like CSS), 14 px above, 8 px below | none |
| Row (text) | min height 56 dp, title `titleLarge` 16 sp, max 2 lines, subtitle `bodyMedium`/`bodySmall` | `ListBoxRow` min height 56 px; title 16 px; subtitle dim 13 px; ellipsis at 2 lines | none |
| Hidden rows (`enabled = false`) | removed with an expand/shrink animation (300 ms) | removed with `gtk::Revealer` slide-down, 300 ms | **mockup greys out disabled rows** (notes in spec say "disabled"), which is incorrect. Fix: hide. |
| Hidden groups | not rendered (header and rows) | not built | mockup shows the header; fix. |
| Spacer between groups | 12 dp | 12 px | none |
| Loading state | the first composition shows the list immediately; async values (categories, repo counts) appear when loaded (`collectAsState(initial = emptyList())`) | show rows immediately with placeholder subtitle `…`, then update | mockup has no loading state. Add `…`. |
| Empty state | not used for these screens (only search and clear DB) | n/a | none |
| Error state | toasts (see per-screen) | toast overlay | mockup has no toasts. Add overlay. |
| Toast | `Toast` bottom-centre, system style, ~2 s | in-window overlay label at the bottom centre, 3 s, `.osd` style | mockup lacks the toast component. |
| Scrolling | `ScrollbarLazyColumn` with the content padding | `gtk::ScrolledWindow` with overlay scrollbars | none |
| Two-pane (tablet) | surface lightened/darkened; selected row highlighted with a 24 dp rounded `surfaceVariant` background | sidebar: selected row uses the `:selected` state; sidebar width 280 px; content padded 24 px | mockup has single-pane only. Covered by T-47a-098. |

Responsive behaviour (Proposal, T-47a-098)
- Content column max width 720 px, centred; below 720 px the content fills the window.
- At ≥ 960 px the settings tree is a left sidebar (280 px) and the selected screen is on the right (two-pane). Below 960 px it is single-pane with push navigation.
- Titles wrap to two lines then ellipsise; subtitles wrap to three lines.

---

## 2. Main settings menu (`SettingsMainScreen`)

Elements (top to bottom), each row: leading icon (24 px), title, subtitle, trailing chevron.

| # | Title | Subtitle | Icon (Material) | Desktop icon (Proposal) |
|---|---|---|---|---|
| 1 | Appearance | Theme, date & time format | Palette | `preferences-desktop-appearance-symbolic` |
| 2 | Library | Categories, global update, chapter/episode swipe | CollectionsBookmark | `view-grid-symbolic` |
| 3 | Reader | Reading mode, display, navigation | ChromeReaderMode | `x-office-document-symbolic` |
| 4 | Player | Internal player, gestures, decoders, subtitles | VideoSettings | `video-display-symbolic` |
| 5 | Downloads | Automatic download, download ahead | GetApp | `folder-download-symbolic` |
| 6 | Tracking | One-way progress sync, enhanced sync | Sync | `emblem-synchronizing-symbolic` |
| 7 | Browse | Sources, extensions, global search | Explore | `compass-symbolic` |
| 8 | Data and storage | Manual & automatic backups, storage space | Storage | `drive-harddisk-symbolic` |
| 9 | Security and privacy | App lock, secure screen | Security | `security-high-symbolic` |
| 10 | Advanced | Dump crash logs, battery optimizations | Code | `applications-engineering-symbolic` |
| 11 | About | Aniyomi Stable <version> | Info | `help-about-symbolic` |

Note: the mockup subtitle for Advanced is "Dump crash logs, battery optimizations". The Rust build drops battery optimisation (task 073); its subtitle becomes "Dump crash logs, networking, data".

Search: magnifier button in the header (`action_search`, "Search"). Keyboard `Ctrl+F`.

Mockup gap: the mockup's `main` list (`spec/settings.json`) has the same 11 rows, in the same order, with the same titles and icon intent; only the Advanced subtitle differs in the Rust plan (battery text removed, task 073), and the About subtitle is generated from the version.

---

## 3. Settings search (`SettingsSearchScreen`)

| State | Android | Desktop (Proposal) |
|---|---|---|
| Initial | empty text field focused; placeholder "Search settings" (`action_search_settings`); no list | same; `gtk::SearchEntry` with placeholder; focus on open |
| Typing | results update per keystroke (no debounce); `take(10)` | same; per-change search (no debounce needed, index is in memory) |
| Results | rows: title (`titleMedium`, one line, ellipsis); breadcrumbs (`bodySmall`, secondary, one line, 16 dp top padding); row padding 24 dp × 14 dp | `ListBoxRow`: title and breadcrumb label (dim); same padding |
| Empty | `EmptyScreen` "No results found" (`no_results_found`) | status page with the same text |
| Clear | trailing `Close` icon button when text non-empty | `gtk::SearchEntry` clear icon (built in) |
| Keyboard | IME `Search`; Enter clears focus; scroll clears focus; leaving hides keyboard | Enter activates the first result; Up/Down move through results; Escape clears the text, then leaves |
| Activate result | `navigator.replace(route)` with highlight key | `ui::replace(route)` with highlight key |
| Player search | placeholder "Search player settings" (`action_search_player_settings`); index = player screens only | same, from the Player screen header |

Mockup gap: the mockup search shows results as "Category > Group" breadcrumbs ✓ and "No results found" ✓. It does not highlight the target row after navigation (T-47a-007), and it indexes the Player entry as an ordinary main-tree screen (breadcrumb "Player > …") rather than the Android `Player settings > …` form.

---

## 4. Appearance screen

| Element | Type | Options / states | Dialog | Feedback | Mockup gap |
|---|---|---|---|---|---|
| "App theme" group: theme-mode pair | custom | `System` / `Light` / `Dark` segmented (rows: `System`, `Light`, `Dark`) plus the palette strip | none (inline) | — | mockup shows a `chips` row. Rust: a `gtk::Box` with three toggle buttons. |
| App theme (palette strip) | custom | Default, Dynamic, plus 17 named palettes (dropped in Rust, task 012) | none | — | mockup shows the 19 chips with preview cards; Rust drops the named ones and keeps Default + (if portal accent exists) Dynamic. |
| Pure black dark mode | switch | default off; hidden when theme = Light | none | none (live) | ok; mockup's "enabled" text is wrong: hidden not greyed. |
| App language | link | → part 2 screen | — | — | ok |
| Tablet UI | list (Auto, Always, Landscape, Never) | default Auto | radio dialog | toast "Requires app restart to take effect" on change | **dropped** (task 017). |
| Start Screen | list | Anime / Manga / Updates / History / Browse | radio dialog | toast "Requires app restart…" → Rust: subtitle text, no toast | ok (mockup matches labels) |
| Navigation Style | list | four "Move … to the More tab" options | radio dialog | none | mockup matches; Rust renames to the sidebar concept (task 018). |
| Date format | list | Default + 5 patterns, each with today's date in brackets | radio dialog | none | ok |
| Relative timestamps | switch | default on; subtitle `"Today" instead of "<today>"` | none | none | ok |

Element states: date format labels show today's date at the time of opening (`LocalDate.now()` once per composition).

---

## 5. Library screen

| Element | Type | Options / states | Dialog | Feedback | Mockup gap |
|---|---|---|---|---|---|
| Edit anime categories | link | subtitle "N categories" | → categories screen | — | mockup "0 categories" ✓ |
| Default anime category | list | "Always ask" + categories | radio | none | ok |
| Edit manga categories | link | as above; opens manga tab | → categories screen (manga) | — | ok |
| Default manga category | list | as above | radio | none | ok |
| Per-category settings for sort | switch | default off | none | none; turning off resets flags | ok |
| Hide hidden categories … | switch | default off | none | none | ok |
| Automatic updates | list | Off / Every 12 hours / Daily / Every 2 days / Every 3 days / Weekly | radio | none | mockup has the same six labels ✓ |
| Automatic updates device restrictions | multi | Only on Wi-Fi / Only on unmetered network / When charging; hidden when interval is Off (`enabled = autoUpdateInterval > 0`, behaviour §3.2 item 2) | check dialog (OK/Cancel) | none | mockup always shows the row; Rust hides it when interval is Off. |
| Anime Categories (update) | tri-state link | subtitle "Include: All / Exclude: None" | tri-state dialog | none | ok |
| Manga Categories (update) | tri-state link | as above | tri-state dialog | none | ok |
| Automatically refresh metadata | switch | default off | none | none | ok |
| Smart update | multi | four items | check dialog | none | ok |
| Show unseen/unread count on Updates icon | switch | default on | none | none | ok |
| Season group (2 switches) | switch | default off | none | none | ok |
| Episode swipe (left/right) | list | Disabled / Bookmark / Fillermark / Mark as watched / Download | radio | none | ok |
| Mark duplicate seen episode | multi | after watching / after fetching | check dialog | none | ok |
| Chapter swipe (left/right) | list | Disabled / Bookmark chapter / Mark as read / Download | radio | none | ok |
| Mark duplicate read chapter | multi | after reading / after fetching | check dialog | none | ok |

Visual tri-state dialog (TriStateListDialog): each row shows an icon that cycles through
- Unchecked: `Icons.Rounded.CheckBoxOutlineBlank`, default colour;
- Checked (include): `Icons.Rounded.CheckBox`, primary colour;
- Inverted (exclude): `Icons.Rounded.DisabledByDefault`, primary colour.
Tapping the row cycles Unchecked → Checked → Inverted → Unchecked. The icon's content description is "Disabled" for all three states (Android bug, behaviour §2 notes it). Rust: accessible names "Not selected", "Included", "Excluded".

---

## 6. Reader screen

Top-level rows (no header): Default reading mode (list), Double tap animation speed (list), Show reading mode (switch), Show tap zones overlay (switch), Animate page transitions (switch).

Groups and rows: as in behaviour §4. Visual details:
- Sliders (E-ink duration 1–15 with step 1; interval 1–10; webtoon side padding 0–25 with percent subtitle). Desktop: `gtk::Scale` with 280 px width, value shown right of the slider as the subtitle text.
- Lists open a radio dialog; the currently selected value is pre-checked.
- Switches are whole-row toggles.

Enabled-state behaviour (Android hides; Rust hides with the same rules, behaviour §4 table):
- Invert tap zones (paged): hidden when tap zones = Disabled.
- Landscape zoom: hidden unless scale type = Fit screen.
- Pan wide images: hidden when tap zones = Disabled.
- Invert split placement: hidden unless split is on.
- Flip rotated pages: hidden unless rotate is on.
- E-ink duration, interval, colour: hidden unless flash on.
- Volume key inversion: hidden unless volume keys on.

Mockup gaps:
- Mockup shows "Show content in cutout area" in Display → remove (task 043).
- Mockup disables (greys) the rows above; Rust hides them.
- Mockup "Background color" options match ✓.
- Mockup has "Show tap zones overlay" ✓ (in top-level, matches).

---

## 7. Downloads screen

| Element | Type | Notes | Mockup gap |
|---|---|---|---|
| Only on Wi-Fi | switch | default on | ok |
| Chapter download speed limit | link (opens numeric dialog) | subtitle "Off" or "N KiB/s"; dialog: `OutlinedNumericChooser` (label, placeholder 0, suffix KiB/s, step 100, min 0), hint "Set to 0 to disable the speed limit.", Cancel/OK | mockup "Off" ✓ dialog ✓ |
| Save as CBZ archive | switch | default on | ok |
| Split tall images | switch | subtitle "Improves reader performance" | ok |
| Maximum downloads | list 1–5 | info "Will only download concurrently from self-hosted or unmetered sources" | ok |
| Delete group | 6 rows | list entries: Disabled, Last read chapter, Second to last…, Third…, Fourth…, Fifth… | ok |
| Excluded anime / manga categories | multi | check dialog | ok |
| Auto-download group | 6 rows | Anime Categories and Manga Categories links show "Include: … / Exclude: …" | ok |
| Download ahead | 2 lists + info | "Next N unseen episodes" / "Next N unread chapters" | ok |
| External downloader | 2 rows | **dropped** (task 051) | mockup has it; remove |

Element states: the speed-limit subtitle reflects the stored value; the dialog keeps its own working copy (see edge cases §7).

---

## 8. Tracking screen

| Element | Type | States | Feedback | Mockup gap |
|---|---|---|---|---|
| Update progress after reading/watching | switch | default on | none | ok |
| Open track menu on adding to library | switch | default on | none | ok |
| Show next episode's airing time | switch | default on | none | mockup label has a trailing space; fix |
| Update progress when marked as read/watched | list | Always / Always ask / Never | none | ok |
| Tracker row (7) | tracker row | logo + name; green check when logged in (`Color(0xFF4CAF50)`, 32 dp `Done` icon, content description "Logged in") | login → browser or dialog; logout → dialog | ok |
| Login dialog | dialog | title "Log in to <name>"; fields (email/username + password with visibility toggle); Login button ("Loading…" while busy) | on failure: `tracker.logout()` and error text; on success "Logged in" toast | mockup dialog ✓; the mockup says "Login" only; add the processing label and the failure behaviour. |
| Logout dialog | dialog | title "Log out from <name>?"; Cancel (outlined) and red "Log out" | "You are now logged out" toast | ok |
| Info (services) | info | "One-way sync … tracking button." | — | ok |
| Enhanced trackers | rows | **only installed-source trackers are shown**; others listed in the info text | — | mockup's notes state the conditional rule, but the item list hard-codes Komga, Kavita, Suwayomi and Jellyfin as always-visible rows. Rust derives visibility from installed sources. |
| Info (enhanced) | info | "Provides enhanced features … automatically tracked when added to your library." + optional "Available but source not installed: …" | — | mockup lacks the "not installed" suffix. |
| Help icon (header) | icon button | tooltip "Tracking guide"; opens guide URL | — | mockup lacks the help icon; add. |

---

## 9. Browse screen

| Element | Type | States | Feedback | Mockup gap |
|---|---|---|---|---|
| Hide anime/manga entries already in library | switch | default off | none | ok |
| Anime extension repos | link | subtitle "N stores" | → repos screen (part 2) | ok ("0 stores") |
| Manga extension repos | link | subtitle "N stores" | → repos screen (part 2) | ok |
| NSFW (18+) group | header | — | — | ok |
| Show in sources and extensions lists | switch | default on; subtitle "Requires app restart to take effect" | device-auth prompt before the change; on cancel/fail the switch does not change | mockup has the note "Toggling the NSFW switch requires device authentication" in notes; add as the auth prompt (task 061). |
| Info (parental) | info | text | — | ok |

Auth prompt (Proposal, task 068): modal with title "NSFW (18+) sources" (`pref_category_nsfw_content`), body "Authenticate to confirm change" (`confirm_lock_change`), and "Cancel".

---

## 10. Data and storage screen (scope rows)

| Element | Type | States | Feedback | Mockup gap |
|---|---|---|---|---|
| Storage location | link (folder chooser) | "No storage location set" / path / "Invalid location: …" | toasts: "No file picker app found"; "Failed to acquire persistent folder access…" | ok (mockup not checked here—part 2) |
| Info | info | "Used for automatic backups, chapter downloads, and local source." | — | — |
| Create backup / Restore backup | segmented buttons (part 2) | restore disabled-looking while running → toast "Restore is already in progress" | "No file selected" on null | (part 2) |
| Automatic backup frequency | list | Off / Every 6 hours / Every 12 hours / Daily / Every 2 days / Weekly | none | ok (mockup: part 2) |
| Info (backup) | info | warning text + "Last automatically backed up: …" | — | — |
| Storage usage | custom | `StorageInfo` bar (part 2) | — | (part 2) |
| Storage (link) | link | → storage tab (part 2) | — | — |
| Clear chapter and episode cache | link | subtitle "Used: <size>" (computed on composition in Android; Rust computes off the UI thread, shows "Calculating…" first) | "Cache cleared, N files deleted" / "Error occurred while clearing" | — |
| Clear episode/chapter cache on app launch | switch | default off | none | — |
| Library List (export) | link | opens dialog "Select data to include" | "Library Exported" toast | — |

Export dialog: four checkboxes (Title, Type, Author, Artist). Unchecking Title disables and clears the other three. Buttons: Save (`action_save`), Cancel.

---

## 11. Security screen

| Element | Type | States | Feedback | Mockup gap |
|---|---|---|---|---|
| Require unlock | switch | enabled only when a lock method is available (Android: biometric/credential); Rust: when the chosen method is configured | auth prompt on change | mockup label "Require unlock" ✓; note the mockup's "Biometric lock disabled …" text, Rust text per the chosen method (task 063). |
| Lock when idle | list | Always / After 1 minute / 2 / 5 / 10 / Never; shown only when lock on | auth prompt on change | ok |
| Hide notification content | switch | default off | none | ok |
| Secure screen | list | Always / Incognito mode / Never | — | **dropped** (task 066); mockup has it. |
| Info (secure screen) | info | "Secure screen hides app contents…" | — | **dropped** with the row. |

---

## 12. Advanced screen

| Element | Type | States | Feedback / dialog | Mockup gap |
|---|---|---|---|---|
| Share crash logs | action | — | opens file manager folder (Rust) | ok (mockup label/subtitle ✓) |
| Verbose logging | switch | default off | toast requires restart (Android) → Rust live | ok |
| Debug info | link | — | → debug screen (part 2) | ok |
| Onboarding guide | link | — | → onboarding (area 36) | ok |
| Manage notifications | action | hidden if no KDE settings found (Rust) | — | ok |
| Background activity (group) | rows | **dropped** (task 073) | — | mockup has group; remove. |
| Data group | 3 rows | Reindex downloads (toast "Downloads index invalidated"); Clear Manga database (link); Clear Anime database (link) | — | ok |
| Networking group | 6 rows | Clear cookies (toast "Cookies cleared"); Clear WebView data (toast "WebView data cleared" / error); DoH list (14 entries; restart note removed in Rust); Default user agent string (edit dialog; error "Invalid user agent string"); Reset default user agent string (enabled when changed) | per row | ok (mockup matches labels) |
| Library group | 2 rows | Refresh library covers (no toast); Reset per-series reader settings (toast success/error) | — | ok |
| Reader group | 3 rows | Custom hardware bitmap threshold (list; Rust derives options from GL max texture size, task 081); Use legacy decoder for long strip reader (switch); Custom display profile (**dropped**, task 083) | — | mockup "Default (4096)", "3072", "2048" list is static; Rust computes from device. Remove display profile. |
| Extensions group | 2 rows | Installer list (**dropped**, task 085); Revoke trusted unknown extensions (action) | Shizuku dialog (dropped) | mockup has the Shizuku dialog; remove. |
| Data Saver group | 9 rows | Data Saver list (4); Bandwidth Hero Proxy Server (text, enabled only for Bandwidth Hero); Use data saver in downloader; Ignore Jpeg Images; Ignore Gif Animations; Image Quality (8 values); Compress to Jpeg (subtitle on/off); Convert to Black And White | — | mockup has all 9 ✓ (task 086-087) |

Clear WebView / cookies toast texts are as behaviour §10.

---

## 13. About screen

Layout (top to bottom): logo header (`LogoHeader`), then rows:

| Row | Subtitle / trailing | Click |
|---|---|---|
| Version | `Stable <name> (<date>)` | copies debug info; toast "Copied to clipboard" (Proposal) |
| Check for updates | trailing spinner while checking | forced update check |
| What's new | — (hidden in debug builds) | opens release page |
| Help translate | — | opens translation guide |
| Open source licenses | — | → licence list |
| Privacy policy | — | opens privacy page |
| Link row (centred, 8 dp vertical) | icons with labels: Website, Discord, GitHub | opens each URL |

States: while checking, the row shows a 28 dp spinner (stroke 3 dp) and a second click is ignored.
Feedback: "No new updates available"; "This Android version is no longer supported" (Rust: rephrased, task 094); error text.

Mockup gap: the mockup's About shows Version (with date), Check for updates, What's new, Help translate, Open source licenses, Privacy policy; it lacks the Website/Discord/GitHub link row (task 095).

---

## 14. Clear database screens

| Element | States | Notes |
|---|---|---|
| Top-bar actions | "Select all" (icon `SelectAll`), "Select inverse" (icon `FlipToBack`); shown only when list non-empty | anime screen label bug: second action labelled "Select all" in Android. Rust uses "Select inverse". |
| Loading | `LoadingScreen` (spinner) | — |
| Empty | "Nothing to clear" (`database_clean`) | — |
| Source row | 56 dp; source icon; name (`bodyMedium`); "N non-library entries in database"; trailing checkbox; selected rows show selected background | whole row toggles |
| Manga bottom action | `LazyColumnWithAction`: action "Delete" (enabled when selection non-empty) | Rust: bottom bar with a destructive-styled button |
| Anime bottom action | full-width `Button` "Delete" (16 dp × 8 dp margins), enabled when selection non-empty | Rust: same as manga |
| Confirm dialog | "Are you sure? Completed episodes and chapters and progress of non-library entries will be lost"; OK / Cancel | OK runs delete; toast "Entries deleted" |

Mockup gap: the mockup shows a dialog titled "Are you sure? …" ✓ and a notes block; it lacks the select-all/inverse header actions and the empty state text.

---

## 15. Open source licence screens

| Screen | Elements | Notes |
|---|---|---|
| Licences | list of libraries (name, optional website, licence summary) | sorted by name (Proposal); tap opens detail |
| Library detail | title = name; header action "Website" (globe) when website exists; body text (scrollable, 16 px padding) | Android renders HTML; Rust shows text |

Mockup: `settings_open_source_licenses` and `settings_open_source_library_license` show a static sample ("Apache License … Version 2.0"); Rust uses the embedded list.

---

## 16. Dialog catalogue (all dialogs in scope)

| Dialog | Title | Body | Buttons | Dismiss | Confirm effect |
|---|---|---|---|---|---|
| Radio list (ListPreference) | pref title | radio rows | Cancel | Cancel / Escape / outside click | selecting a row stores it and closes |
| Multi-select (MultiSelect) | pref title | check rows | Cancel, OK | Cancel / Escape | OK stores the set |
| Text edit (EditText) | pref title (+ dialog subtitle) | outlined text field, trailing clear or error icon, supporting error text | Cancel, OK (OK enabled only when valid and changed) | Cancel / Escape | OK stores |
| Tri-state (categories) | category title | optional message + tri-state rows | Cancel, OK | Cancel / Escape | OK stores include and exclude sets |
| Speed limit | "Chapter download speed limit" | numeric chooser + hint | Cancel, OK | Cancel / Escape | OK stores |
| Library export columns | "Select data to include" | 4 checkboxes | Cancel, Save | Cancel / Escape | Save writes CSV |
| Tracker login | "Log in to <name>" | 2 fields + Login | Login (no cancel button; close icon in title) | close icon / Escape | success closes |
| Tracker logout | "Log out from <name>?" | — | Cancel (outlined), Log out (red) | Cancel / Escape | logs out |
| Shizuku missing | "Shizuku" | "Install and start Shizuku…" | Cancel, OK | Cancel / Escape | OK opens download page (dropped in Rust) |
| Clear DB confirm | — | "Are you sure? …" | Cancel, OK | Cancel / Escape | deletes |
| NSFW / lock auth | title per setting | "Authenticate to confirm change" | (system prompt) | cancel | success returns true |

Dialog window behaviour (Proposal): modal, transient for the main window, default width 320 px (mirrors `usePlatformDefaultWidth`), centred, dims the parent, closes on Escape.

---

## 17. Gestures and animations

| Element | Android | Desktop |
|---|---|---|
| Row press | ripple | GTK `:hover` and `:active` styles from the user's theme |
| Switch toggle | row tap; switch animates | row click; `gtk::Switch` animates (built in) |
| Hidden-row reveal/collapse | expandVertically + fadeIn / shrinkVertically + fadeOut, 300 ms | `gtk::Revealer` slide-down/up, 300 ms, crossfade |
| Search-jump highlight | `surfaceTint` α 0.12 pulse ×5 after 600 ms; 3 s total | CSS class `.highlight` with an `@keyframes` pulse (Proposal) |
| Loading indicator (about) | 28 dp circular spinner, 3 dp stroke | `gtk::Spinner` 16 px |
| Search results | crossfade on result change ("results" label, default Crossfade duration 300 ms) | immediate update (no crossfade) |
| Drag on slider | `BaseSliderItem` continuous | `gtk::Scale` continuous |

Keyboard equivalents (desktop, Proposal): see section 1 and each section's Element table. Mouse: right-click on a row shows nothing (no context menus in these screens).

---

## 18. What the mockup lacks (consolidated)

1. Hidden-row behaviour: the spec marks disabled rows greyed; the Rust build hides them (sections 1, 4, 5, 6).
2. Toasts and their texts (all screens).
3. Loading and placeholder states (section 1).
4. Dialog catalogue details: tri-state icons, text-edit validation, Shizuku dialog (remove), restart-required subtitles.
5. Tracking: enhanced trackers conditional display; help icon; login failure behaviour.
6. Browse: NSFW auth prompt.
7. Security: method-dependent switch text; secure screen row (remove).
8. Advanced: several rows to remove (battery, Don't kill my app, installer, display profile); hardware bitmap options derived from the device.
9. About: Website/Discord/GitHub link row.
10. Clear DB: select-all and inverse header actions, empty state, bottom action.
11. Responsive/two-pane behaviour (T-47a-098).
12. Keyboard shortcuts (`Ctrl+F` search; Escape; Enter).
13. Mockup text errors: "Show next episode's airing time " (trailing space), "Aniyomi" copy (the Rust app uses "Kurobayomi" in the window title; About uses `app_name` = "Aniyomi" in the Android source; Rust uses "Kurobayomi").
