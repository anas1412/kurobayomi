# Area 36 — UI parity: visible elements, states, dialogs, gestures, and mockup gaps

Scope UI: shell (bottom bar / rail), top app bar and search, state banners, tabbed screen and tabbed dialog, adaptive sheets, dropdown menus, FAB, download indicator, empty state, WebView screen, onboarding wizard, update prompt, deep-link resolve page, lock screen. Mockup reference: `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/entry.rs` (`webview()`), `spec/app.json` (`onboarding`, `new_update`).

Labels use the Aniyomi string keys (behaviour §16). Visible English strings are the values of those keys in English; keep the keys in the Rust i18n table (area 51).

---

## 1. Window shell

### 1.1 Layout, phone (narrow)
- Top: `AppStateBanners` stack (0 to 3 banners), then the page header (`HeaderBar`), then the page content.
- Bottom: `NavigationBar` with 5 entries (after NavStyle removes one of 6). Each entry: icon (24 dp) plus label (`labelLarge`, 1 line, ellipsis, always shown). Selected entry uses the primary indicator.
- Bar hides (slide up/down) when the current page is the More-moved tab (behaviour §2.3).
- Badges: a count bubble at the top-end of the icon. Updates count and Browse extension count (§1.4).

### 1.2 Layout, wide (tablet / desktop)
- Left: `NavigationRail` equivalent: a sidebar `gtk::ListBox` (`navigation-sidebar` class), icon + label per entry, a collapse toggle in the sidebar header (mockup: `sidebar-show-symbolic`, tooltip "Collapse sidebar").
- Collapsed (icon-only) mode is the mockup's existing behaviour (square selection, dots for badges). Keep.
- Hidden on reader and player pages (mockup `immersive` rule, src/main.rs refresh closure).

### 1.3 Banners (AppStateBanners)
| Banner | Shown when | Text key | Colour token (GTK) |
|---|---|---|---|
| Indexing | download cache initialising (manga or anime) | `download_notifier_cache_renewal` with a small spinner (size = text height, stroke = height/8) | secondary |
| Downloaded only | `__APP_STATE_pref_downloaded_only` true | `label_downloaded_only` | tertiary |
| Incognito | `__APP_STATE_incognito_mode` true | `pref_incognito_mode` | primary |

- Order top to bottom: Indexing, Downloaded only, Incognito. Each has `expandVertically`/`shrinkVertically` on show/hide.
- Text centered, `labelMedium`, padding 4 dp (indexing 8 dp).
- Proposal (GTK): `gtk::Revealer` per banner with `SlideDown`, transition 200 ms; the stack is a `gtk::Box` (vertical).
- Status accent: the header bar tints with the first matching banner colour (behaviour §1.2), Proposal only.

### 1.4 Badges
- Updates tab (`Tab.Updates`): count = anime + manga new updates if `library_show_updates_count` else 0. Hidden at 0. Description (accessibility) plural `notification_chapters_generic`.
- Browse tab: count = manga extension updates + anime extension updates. Hidden at 0. Description plural `update_check_notification_ext_updates`.
- Badge text is the raw number (no cap at 99 in the Kotlin).
- Mockup: hard-coded `"8"` and `"1"` in `sidebar()` (src/main.rs:175). Must be replaced by live values.

### 1.5 Back behaviour
- Escape and the mouse back button behave like Android back (behaviour §2.6): leave a non-start tab to the start tab; on the start tab nothing happens (the window stays).
- With a page stack deeper than the root, Escape pops one page first (navigator), then tab rules apply.

---

## 2. Top app bar (AppBar and variants)

### 2.1 Standard
- Left: back arrow (`ArrowBack`, tooltip/accessible name "Back" = `action_bar_up_description`) when the page has a parent; or a custom icon (WebView uses Close X).
- Title: one line, ellipsis. Subtitle: `bodyMedium`, one line (WebView shows the current URL; the Kotlin marquee is replaced by ellipsis plus tooltip, Proposal).
- Count pill (for selection counts): rounded label, 14 pt, text colour onBackground at 8% (light) or 12% (dark) opacity background.
- Right: up to N icon buttons (tooltip = title), then the overflow button (`MoreVert`, tooltip `action_menu_overflow_description`). Overflow opens a popover menu with one item per overflow action; the menu item text uses normal weight.

### 2.2 Action mode
- Left: Close (X) with `action_cancel` name. Title: the selected count as text. Right: the page's action-mode actions.
- Container tint: surface elevated (3 dp equivalent) in action mode.
- Escape cancels action mode (Proposal; Kotlin cancels via the Close button only).

### 2.3 Search toolbar
- Closed state: title on the left; search icon (`action_search`) on the right, tooltip "Search".
- Open state: the title area becomes a text entry (`gtk::SearchEntry`), font `titleMedium` 18 pt, placeholder `action_search_hint` (or the caller's text). The left icon becomes the close icon (see next item).
  - Left icon when open: the close icon (in place of the back arrow) calls `onClickCloseSearch`, which sets the query to null and closes search.
  - Right: a Reset (X) button `action_reset` when the query is non-empty; it clears the query and refocuses the field.
- Enter (or `ImeAction.Search`): runs `onSearch(query)` if the query is non-blank, clears focus, and hides the on-screen keyboard. Blank query: nothing happens.
- Focus on open: the field takes focus; when the query is empty the keyboard is shown (Android `showSoftKeyboard(remember { isEmpty })`).
- Clearing with the reset button refocuses the entry.
- Debounce: none in `SearchToolbar`. Consumers (library/browse) debounce with `SEARCH_DEBOUNCE_MILLIS` = 250 ms (behaviour §5.1). Proposal: apply the same 250 ms debounce for live filtering in the shell's search entry.
- `searchEnabled = false` hides the search icon entirely.

### 2.4 Title count (tabbed screens)
`numberTitle > 0`: the title is followed by a count pill (behaviour §14.6).

---

## 3. Tabbed screen and tabbed dialog

### 3.1 TabbedScreen
- Tab row: primary tab row (fixed) or scrollable (`scrollable = true`, edge padding 13 dp). Tabs are text; each may show a badge (`badgeNumber`). The selected tab has the primary underline indicator.
- Pages: horizontal swipe in Android; in GTK, click or Ctrl+PageUp/PageDown (Proposal) switches pages with a 200 ms slide.
- Search and actions per page come from the `TabContent` of the current page (behaviour §14.6). **Parity rule**: the Kotlin picks the manga query for odd page indexes and the anime query for even ones. Proposal: each `TabContent` carries its own query binding so the rule does not depend on index parity.
- Snackbar host: one per screen, bottom-centered; used by the page content (area 37-42 messages).

### 3.2 TabbedDialog (settings-like sheet)
- Sheet with a tab row (`surfaceContainerHigh` background), an overflow button at the row end (`MoreVert` or the caller's icon, tooltip `label_more`), a divider, and the pager body.
- Padding inside the body: 24 dp horizontal, 8 dp vertical (`TabbedDialogPaddings`).
- Size: adaptive sheet (below).

---

## 4. Adaptive sheet and dialogs

### 4.1 AdaptiveSheet
- Phone: sheet anchored to the bottom, drag handle, swipe-to-dismiss (Android). Desktop Proposal: a modal `gtk::Window` attached to the main window, anchored to the bottom edge of the window when narrow (< 760 px) and centred when wide.
- Max content width 460 dp (Kotlin doc comment, AdaptiveSheet.kt:66). Desktop: `max-width: 460px` on the sheet body.
- Dismiss: Escape, click on the scrim outside the sheet (Android `onDismissRequest`), close button when the sheet is a dialog.
- Swipe dismiss: dropped on desktop (no touch gesture requirement); the `enableSwipeDismiss` flag (behaviour §3.2) is kept in the Rust API and ignored by the widget.
- Nested navigation inside a sheet: a `gtk::Stack` inside the sheet; Escape pops the inner stack first (behaviour §3.2 BackHandler `size > 1`).
- Transition inside sheet: fade 220 ms, delay 90 ms (in); fade 90 ms (out).

### 4.2 Dropdown menus
- Width fixed at 196 dp (desktop: `min-width: 196px; max-width: 196px` on the popover content). Default offset (8 dp, -56 dp) is an Android anchor offset; desktop uses the popover's default placement under the anchor.
- Radio row: checked icon `RadioButtonChecked` tinted primary (accessible name `selected`), unchecked `RadioButtonUnchecked` (`not_selected`). Clicking selects and closes.
- Nested menu: a row with a trailing right arrow; click or Right arrow key opens the sub-popover; Left arrow or Escape closes it.
- Keyboard: Up/Down move focus; Enter activates; Escape closes the whole menu.

### 4.3 Entry download menu
Options (behaviour §14.3): "Next 1", "Next 5", "Next 10", "Next 25" (plural, `download_amount` / `download_amount_anime`), and "Unread" (manga `download_unread`) or "Unseen" (anime `download_unseen`). Clicking an option dispatches the action then closes the menu. `DownloadAction` values: NEXT_1_ITEM, NEXT_5_ITEMS, NEXT_10_ITEMS, NEXT_25_ITEMS, UNVIEWED_ITEMS.

### 4.4 Dialogs in scope
Only one dialog is in this scope: the changelog dialog (behaviour §7.3).
- Title: "Updated to version X" (`updated_version` with the version name).
- Buttons: "What's new" (opens the release page) and "OK" (closes).
- Shown once after a migration, non-debug only.
- Desktop: `gtk::AlertDialog` (GTK 4.10+), modal to the main window.

---

## 5. Floating action button

- Extended FAB with a plus icon and label "Add" (`action_add`), bottom-end of the page, 16 dp inset (Proposal: 16 px).
- Collapses to an icon-only round button while the list scrolls forward; expands when at the top, on any backward scroll, or at the end of the list (behaviour §14.4).
- Desktop: a `gtk::Button` with CSS class `suggested-action` and a transition on width (200 ms).

---

## 6. Download indicator (per item)

- Size 26 px; inner padding 2 px; arrow glyph 19 px.
- Long press: fires long-click; haptics dropped.
- Click: toggles download/cancel as defined by the owning list area (areas 37-42).
- Ring shows progress; colour from the theme's primary.

---

## 7. Empty state

- `EmptyScreen`: centered icon (96 px), text (`bodyLarge`, centered), optional up to two action buttons (`action_retry` with a refresh icon; `getting_started_guide` with a help icon).
- Rendered by the list areas; the shell only provides the component (area 37 onward uses it).

---

## 8. WebView screen

### 8.1 Header
- Left: Close (X). Title: page title (or initial title). Subtitle: current URL (one line, ellipsis).
- Right, in order: Back (`ArrowBack`, enabled only when `can_go_back`, name `action_webview_back`), Forward (`ArrowForward`, enabled only when `can_go_forward`, name `action_webview_forward`), overflow.
- Overflow items: Refresh (`action_webview_refresh`), Share (`action_share`), Open in browser (`action_open_in_browser`), Clear cookies (`pref_clear_cookies`).

### 8.2 Progress
- Initializing: indeterminate progress bar along the bottom of the header area.
- Loading: determinate progress from the page's load progress (0.0 to 1.0; missing value treated as 1.0).
- Idle: no bar. Desktop: `gtk::ProgressBar` with `visible` toggled; 3 px tall.

### 8.3 Cloudflare help banner
- Appears under the header when the page HTML contains `window._cf_chl_opt` or `Ray ID is` (checked after each page finish; cleared when the next page finish does not contain them).
- Surface with 8 dp (8 px) padding, error-coloured warning text `information_cloudflare_help`, clickable: opens `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare`.

### 8.4 States
| State | Visible |
|---|---|
| Loading (start) | indeterminate bar, page area blank |
| Loaded | page, progress hidden |
| Failed load | WebKit default error page (Proposal: custom message with the URL and Retry) |
| Cloudflare challenge | banner visible; page shows challenge |
| WebView unavailable | toast `information_webview_required` (long), window closes |

### 8.5 Gestures and keys
- Mouse back/forward buttons (8/9): back/forward (Proposal).
- Ctrl+R: refresh (Proposal). Ctrl+L: focus URL (Proposal; the URL is read-only in the Kotlin subtitle, so this is new).
- Ctrl+Shift+C: copy URL (Proposal; the Kotlin share copies via the intent sheet).
- Zoom: Ctrl+plus / Ctrl+minus / Ctrl+0 (WebKit defaults; `builtInZoomControls` was on in Kotlin, controls not visible).

### 8.6 Share and open in browser
- Share: copy the URL to the clipboard and show a toast (behaviour §10). Proposal.
- Open in browser: `gio::AppInfo::launch_default_for_uri`. The Kotlin forces the default browser, not Kurobayomi itself, because the app handles the `tachiyomi://` / `aniyomi://` schemes; on Linux the same concern applies to the `x-scheme-handler` only if the link uses those schemes, so filter those first (Proposal).

---

## 9. Onboarding wizard

### 9.1 Frame
- Full-window page, no app chrome (no bottom bar, no sidebar). Title: "Welcome!" (`onboarding_heading`). Subtitle: `onboarding_description`. Rocket icon.
- Content box: rounded, `surface-variant` background, padding 8 dp vertical (`MaterialTheme.padding.small`), content fills the box.
- Footer: Next / Finish button, enabled when `isComplete`; Back not present in the footer on step 1 (Kotlin: back is the system back, which is disabled on step 1 through the outer wrapper, see below). Proposal: a Back button on steps 2 to 4.
- Step transition: shared axis X slide, forward when target > initial (Kotlin `materialSharedAxisX`, slide distance from `rememberSlideDistance`). Desktop: `gtk::Stack` `SlideLeftRight` 250 ms.

### 9.2 Steps
1. Theme: two pickers (mode: Light / Dark / System; app theme list with an AMOLED toggle). Changing mode applies immediately.
2. Storage: text "Downloads go to <path>" (`onboarding_storage_info` with app name and the current path), button "Select folder" (`onboarding_storage_action_select`), divider, text `onboarding_storage_help_info`, button `onboarding_storage_help_action` (opens storage FAQ). Next disabled until a folder is set.
3. Desktop (replaces Permission): Proposal rows: "Notifications: available" (status, no action), "Autostart: not configured" (informational). No buttons that need Android permissions.
4. Guides: text `onboarding_guides_new_user` plus button "Getting started guide" (opens external link); divider; text `onboarding_guides_returning_user` plus button "Restore backup" (`pref_restore_backup`), which finishes onboarding and opens Settings > Data and storage with the restore row highlighted.

### 9.3 Completion and blocking
- Finish sets `onboarding_complete = true` then pops to the home page.
- Back is blocked until complete (the outer wrapper's BackHandler). Proposal: closing the window does not set the flag; next launch shows onboarding again.

---

## 10. Update prompt (NewUpdateScreen)

- Title: "New version available!" (`spec/app.json` `new_update`).
- Body: version name; changelog text with the checksum section removed (regex in behaviour §7.2), rendered as plain text in a scrolled view.
- Buttons: "Open in browser" (release page), "Not now" (reject, pops), "Update" (accept: starts the download job with the URL and title; pops).
- Desktop: the same three actions; accept downloads via the area 35 job.

---

## 11. Deep-link resolve page

- Header: back arrow, title "Search" (`action_search_hint`).
- Loading: centered spinner with the text "Loading" (Proposal wording; Kotlin shows a loading screen with no text).
- No results: immediately replaced by the global search page for the query (Kotlin `replace`).
- Result: replaced by the entry page (`fromSource = true`) or, when there is a child id (episode / chapter), the player or reader opens and the resolve page is popped.
- Failure (Proposal): an error row with the message and a Retry button.

---

## 12. Lock screen (UnlockActivity replacement)

- Full window overlay, no chrome, centered: lock icon, title = "Unlock Kurobayomi" (`unlock_app_title` with app name), and a passphrase entry (or a fingerprint prompt when fprintd is present).
- Passphrase entry: password field, Enter submits, a "Show" toggle (Proposal). Error text under the field on failure (Proposal message: "Incorrect passphrase").
- Cancel / close: closes the overlay and returns to the lock screen (does not quit the app). Kotlin quits on cancel; this is an intentional change (behaviour §11.8).
- While locked: main content is replaced by a blank page before the overlay appears (pitfall, design §7).

---

## 13. Toasts and snackbars

| Message key | Surface | Duration | Trigger |
|---|---|---|---|
| `http_server_start_failure` (AYMR) | toast | short | external player HTTP server failed |
| `information_webview_required` | toast | long (3.5 s) | no WebView |
| `file_picker_error` | toast | short | no file chooser available |
| `unlock_app_title` | lock title | — | lock |
| `confirm_lock_change` | prompt subtitle | — | settings change |
| Clear cookies | toast (Proposal: "Cookies cleared (N)") | short | overflow action |
| Copy URL | toast (Proposal "Copied") | short | share on desktop |

Snackbar component (shared, area 37 onward) is not defined in this area; the shell provides `ToastSink` with `show(text, duration)`.

---

## 14. Keyboard and mouse map (shell)

| Action | Key / pointer | Notes |
|---|---|---|
| Back / leave tab | Escape, mouse back | §1.5 |
| Bar tab 1..5 | Alt+1..5 | Proposal |
| Search | Ctrl+F | opens search on current page |
| Cancel action mode / close search | Escape | §2.2, §2.3 |
| Settings | Ctrl+, | Proposal |
| Lock | Ctrl+L | Proposal |
| Next/previous tab page | Ctrl+PageDown / Ctrl+PageUp | Proposal |
| Context menu on item | right click | Proposal (Android long press) |
| Long press (touch) | long press / right click | download indicator, selection |
| Select range | Shift+click | list areas; the shell does not own it |

Focus: every interactive widget has a visible focus ring (GTK default). Tab order follows visual order: banners are not focusable; header buttons, then page content.

---

## 15. Responsive behaviour

| Width | Chrome | Notes |
|---|---|---|
| < 760 px (phone-like) | bottom switcher, header full width | sheets anchored bottom |
| >= 760 px or maximised | sidebar (expanded 240 px, collapsible to icon-only 56 px) | sheets centred |
| Landscape with `tablet_ui_mode = LANDSCAPE` | sidebar when width > height | Kotlin rule |
| Any | minimum window 360 x 480 px (Proposal) | prevents layouts below 16 px gutters |

- Gutters: 16 px side gutter on content (Proposal, matching the design system), 8 px in dense lists.
- No horizontal scroll on the shell itself; wide tables are a list-area concern.

---

## 16. Animations and timing (desktop equivalents)

| Kotlin | Duration | Desktop |
|---|---|---|
| Tab fade through | 200 ms in, 200 ms out | `gtk::Stack` `Crossfade` 200 ms |
| Screen shared axis X | slide distance from `rememberSlideDistance` | `gtk::Stack` `SlideLeftRight` 250 ms |
| Banner expand/shrink | default Compose animation | `gtk::Revealer` `SlideDown` 200 ms |
| Sheet fade in / out | 220 ms (delay 90 ms) / 90 ms | `gtk::Window` fade via CSS `transition` 220 ms |
| FAB expand/collapse | default | CSS width transition 200 ms |
| Splash exit (pre-S) | 400 ms | dropped |
| Reveal animation | 350 ms show, 500 ms hide | dropped |
| Marquee subtitle | repeat delay 2000 ms | dropped (ellipsis + tooltip) |

Respect the system "reduce motion" setting: when GTK `gtk-enable-animations` is false, all transitions above are 0 ms.

---

## 17. What the mockup lacks (complete list)

Mockup files: `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/entry.rs`, `src/settings.rs`, `spec/app.json`.

1. Bottom bar and sidebar nav ids are `library, updates, history, browse, more`; Aniyomi has `AnimeLibraryTab` and `MangaLibraryTab` as separate entries, plus `NavStyle` moving one to More. Missing: the NavStyle rule and the two library entries.
2. Start screen (`start_screen` preference) not implemented; the mockup always opens `library`.
3. Badges are hard-coded (`"8"`, `"1"`).
4. No navigator stack (no push, pop, replace, pop-to-root). Entry pages are not routed.
5. No back-button rule (`BackHandler`), no Escape behaviour.
6. No state banners (downloaded-only, incognito, indexing).
7. No changelog dialog.
8. No update prompt route (screen exists in spec, no wiring).
9. No deep-link routes (search from URL, pasted URL resolve page).
10. No intent / URI / file open handling.
11. No app lock, no unlock window, no passphrase.
12. No onboarding gate or wizard routing (screen exists, not a stepper, no back-block).
13. WebView screen is static (`entry.rs` `webview()`); no navigation, no cookie clear, no share.
14. No theme mode / app theme / AMOLED handling.
15. No startup gate (window appears immediately).
16. Sheets: no adaptive phone/tablet variant, no nested navigation, no 460 px max width.
17. Dropdown menus: no fixed 196 px width, no radio row, no nested menu (checked in `settings.rs` only partly).
18. FAB: no collapse-on-scroll.
19. Download indicator: not present in this area's mockup.
20. Keyboard map (Escape, Ctrl+F, Alt+1..5) not implemented.
21. Accessibility names on icon-only buttons are not all set (tooltips exist in the mockup `ui.rs` but are not uniform).
22. No toast or snackbar sink.
23. Reduced-motion handling absent.
