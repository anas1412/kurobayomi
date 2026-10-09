# Area 53 — UI parity for the scope files (what the user sees and what the mockup lacks)

This area renders UI only through: the deep-link loading pages, the transitions used by reader and web view, the download row and its menu (layouts and menus), the reader and player roots and the reader error view, desktop notices, launcher actions, and the incognito and server notices. Other visible UI is owned by other areas and is referenced, not re-specified. Labels are the exact English values from the MR and AYMR base strings (see `p1-data-and-config.md` section 10.3). Kurobayomi's own strings are given as "Proposal" when they differ.

Mockup reference: `/home/blackbox/kurobayomi/src/screens.rs` (`downloads()` at line 354, `more()` at line 336), `src/ui.rs` (`open` at line 88), `src/main.rs` (`tabs()` at line 141, `refresh` at line 58), `spec/app.json` (`nav` ids `library`, `updates`, `history`, `browse`, `more`).

## 1. Screens owned here

### 1.1 Deep-link loading page (replaces `DeepLinkAnimeScreen` and `DeepLinkMangaScreen`)

| State | Visible content | Observed Aniyomi | Kurobayomi (Proposal) |
|---|---|---|---|
| Loading | Top bar titled "Search…" (`action_search_hint`), back arrow, centred loading indicator (`LoadingScreen`) | `DeepLinkAnimeScreen.kt:44` to `:47` | same title; spinner inside the page; the back arrow pops the page (Escape also) |
| NoResults | No visible state: the page replaces itself with the global search page for the same query | `DeepLinkAnimeScreen.kt:48` to `:50` | the global search page opens with the query filled in; a one-line banner "No source recognised this link" (Proposal, new string `deeplink_no_source`, English) is shown above the search results |
| Result (entry only) | Replaces with the entry page (`fromSource = true`) | `DeepLinkAnimeScreen.kt:53` to `:59` | the entry page for the entry; the loading page is removed from the stack (no back to a dead page) |
| Result (with child) | Pops the loading page, then opens the player or the reader | `DeepLinkAnimeScreen.kt:60` to `:66` | pops, then opens the player or reader through the player and reader routers (areas 43 and 45) |
| Error (new) | Not defined on Android (the page spins forever, edge E-07) | none | the page shows an error panel: text = the `ResolveError` message, button "Retry" (`action_retry`, English "Retry") and button "Search instead" (Proposal, new string `deeplink_search_instead`, English) that opens the global search page |

### 1.2 Global search from text (ACTION_SEND text)

On Android the shared text becomes the global-search query for the chosen kind (default anime). Kurobayomi shows the same global search page; if the text is an http or https URL the resolver runs first (behaviour section 2.8), and the global search page is pushed underneath. The user sees the loading page only while resolving.

### 1.3 Tracker OAuth return

Android shows `LoadingScreen` in `TrackLoginActivity` (`BaseOAuthLoginActivity.kt:21` to `:23`), then the main window. Kurobayomi shows the Settings page with a spinner row "Connecting…" (Proposal, new string `track_connecting`, English) while the token is exchanged, then the tracker row shows the account name or the error. The spinner is part of area:27's settings UI; this area only routes.

## 2. Notification surfaces (desktop)

Each row lists what the user sees on Android and what Kurobayomi shows. Kurobayomi notice texts keep the Android strings where they exist. Buttons are capped at three (`p1-rust-design.md` pitfall 10).

| Notice (Android id) | Title shown | Body shown | Buttons (Android) | Click on body (Android) | Kurobayomi buttons | Kurobayomi click |
|---|---|---|---|---|---|---|
| App update available (id 1 then 2) | "New version available!" (`update_check_notification_update_available`) | release version string | "Download" (`action_download`), "What's new" (`whats_new`, English "What's new") | download starts (`updateIntent`) | "Download" (starts `StartAppUpdate`), "What's new" (`OpenUrl` release link) | opens the release page (area:35 behaviour E-U4); download does not start from a click |
| App update downloading (id 1, foreground) | "Aniyomi" or the release title | "Downloading…" (`update_check_notification_download_in_progress`) | "Cancel" (`action_cancel`) | none | "Cancel" (`CancelAppUpdate`) | cancels; the notice is withdrawn |
| App update complete (id 2) | release title | "Tap to install update" (`update_check_notification_download_complete`) | install (APK) and "Dismiss" | opens installer | "Show in folder" (`RevealFile`) and "Dismiss" (`Dismiss`) | opens the folder containing the file; no APK install (dropped) |
| App update error (id 3) | release title | "Download error" (`update_check_notification_download_error`) | "Retry" (`action_retry`), "Dismiss" | none | "Retry" (`StartAppUpdate` with the same URL), "Dismiss" | retries or withdraws |
| Incognito on (id -701, ongoing) | "Incognito mode" (`pref_incognito_mode`) | "Disable incognito mode" (`notification_incognito_text`) | none; tap = content intent | sets incognito off | Proposal: one button "Disable incognito mode" (`DisableIncognito`) and no ongoing flag | sets incognito off, notice withdrawn |
| Torrent server running (id -801, ongoing) | app name "Aniyomi" (`app_name`) | "Torrent Server is running" (`torrentserver_is_running`) | "Stop" (hard-coded literal, `TorrentServerService.kt:124`) | none | "Stop" (Proposal: the same string, moved to a string resource, English "Stop") | stops the server (`Shutdown`) |
| HTTP server running (id -901, ongoing) | app name | "Http server is running" (`http_server_is_running`) | "Close" (`action_close`, English "Close") | none | "Close" | stops the server |
| New chapters or episodes (group -301 or -1301, per entry id) | per Android (area:35 owns text) | per area:35 | "Mark as read" / "Download" variants per area:35 | opens the entry or chapter | per area:35 (categories `NewEntries`) | per area:35 |
| Shared image or backup | none on Android (share chooser) | none | none | share chooser | none (dropped; replaced by "Show in folder" from the download screen) | none |
| Error log open | none | none | none | ACTION_VIEW text/plain | none (Proposal: "Open log" button in the library-update error notice, area:26) | none |
| Extension install in progress (id -402) | "Installing extension…" (`ext_install_service_notif`) | none | none | none | dropped (no APK install) | none |

Notice visibility rules (observed, Proposal kept): no notice is shown when the category switch is off (`notify.enabled.<category>`, default on), when `server_available()` is false the same text appears as an in-app toast.

## 3. Transitions (reader and web view)

| Event | Android (API 34 and above, `overrideActivityTransition`) | Android (API 33 and below, `overridePendingTransition`) | Kurobayomi (GTK Stack) | Difference |
|---|---|---|---|---|
| Open reader | enter `shared_axis_x_push_enter`: translate 30 dp to 0 (300 ms, fast_out_slow_in) plus alpha 0 to 1 (195 ms, linear_out_slow_in); the outgoing activity runs `shared_axis_x_push_exit` | enter translate 5%p to 0 (300 ms) plus alpha 195 ms | `SlideLeft`, duration 300 ms | GTK cannot fade during a slide; the alpha part is dropped. The slide distance is the full page width (not 30 dp). |
| Close reader | enter `shared_axis_x_pop_enter`: translate -30 dp to 0 (300 ms) plus alpha 195 ms; exit `shared_axis_x_pop_exit`: translate 0 to 30 dp plus alpha 1 to 0 | translate -5%p to 0 and 0 to 5%p | `SlideRight`, duration 300 ms | same as above |
| Open web view | push, as reader | push | `SlideLeft`, 300 ms | same |
| Close web view | pop, as reader | pop | `SlideRight`, 300 ms | same |
| Player open or close | no transition (`PlayerActivity` never calls `overridePendingTransition`) | none | none (immediate) | none; the `player_*` resources are not used |

Timings: translate 300 ms, alpha 195 ms, interpolators `fast_out_slow_in` (translate, enter and exit) and `linear_out_slow_in` / `fast_out_linear_in` (alpha). Interpolators are not configurable in GTK; GTK uses its own ease curve for Stack transitions. This is recorded as a known difference.

Reduced motion: GTK's `gtk-enable-animations` setting disables Stack transitions (the page switches immediately). Android has `animatorDurationScale`, which the Coil crossfade reads (`App.kt:203`). Proposal: when `gtk-enable-animations` is false, the transition duration is 0.

## 4. Desktop actions (launcher long-press and dock menu)

| Label shown (English) | Source Android label | Opens tab | Notes |
|---|---|---|---|
| Manga library | `label_manga_library` ("Manga") | Library | Android label "Manga" is kept; Kurobayomi uses "Manga library" to avoid a bare word (Proposal) |
| Anime library | `label_anime_library` ("Anime") | AnimeLib | Proposal wording |
| Updates | `label_recent_updates` ("Updates") | Updates | same |
| History | `label_recent_manga` ("History") | History | the Android duplicate id is removed (behaviour 2.4) |
| Browse | `browse` ("Browse") | Browse | same |

The desktop action list shows five entries. The action labels are translated through the packaging step (area:51).

## 5. Download queue row, header and menu (areas 23 and 24 own behaviour)

### 5.1 Section header (`download_header.xml`)

- Card with no elevation and the drag foreground colour while being dragged (`draggable_card_foreground`).
- Left: section title (style `TextAppearance.Tachiyomi.SectionHeader`, horizontal padding 16 dp, vertical padding 8 dp, weight 1, so the title fills the row).
- Right: reorder handle (`ic_drag_handle_24dp`, tint `textColorHint`, padding 10 dp horizontal and 8 dp vertical, end margin 4 dp).
- Top margin 8 dp between sections.

Mockup today (`src/screens.rs:354` to `:403`): a `heading()` with text "Source A (3)" and no drag handle. Missing: reorder handle with drag and drop between sections (the Android header is the drag handle for section order).

### 5.2 Item row (`download_item.xml`)

Visible parts, top to bottom, left to right:

1. Drag handle (`ic_drag_handle_24dp`, 16 dp padding, tint `textColorHint`).
2. Series title (`textAppearanceBodyMedium`, one line, end ellipsis).
3. Chapter or episode title (body medium, 12 sp, one line, end ellipsis, 2 dp top margin).
4. Progress bar (linear, 8 dp top margin, 4 dp bottom margin).
5. Counter text on the title line, right side (body medium, 12 sp, secondary colour), for example `0/10`.
6. Overflow button (`ic_overflow_24dp`, content description "Menu" (`action_menu`), tint `colorOnBackground`).

Mockup today (`src/screens.rs:354` to `:403`): handle icon, title, unit text ("Chapter 1" or "Episode 1"), counter label ("12/30" for manga, "45%" for anime), progress bar with margins 12 dp and 6 dp, overflow menu. Differences to resolve in areas 23 and 24: the anime counter is a percentage in the mockup while the Android layout has a counter text for both kinds (verify in `AnimeDownloadHolder.kt` and `MangaDownloadHolder.kt`, areas 24 and 23); the mockup places the counter below the title in a different order. The Kurobayomi row keeps Android's order.

### 5.3 Overflow menu (`download_single.xml`)

| Position | Item id | Label (English, Android) | Kurobayomi action |
|---|---|---|---|
| 1 | `move_to_top` | "Move to top" (`action_move_to_top`) | moves this item to the top of the queue |
| 2 | `move_to_top_series` | "Move series to top" (`action_move_to_top_all_for_series`) | moves all queued items of the same series to the top, keeping their order |
| 3 | `move_to_bottom` | "Move to bottom" (`action_move_to_bottom`) | moves to the bottom |
| 4 | `move_to_bottom_series` | "Move series to bottom" (`action_move_to_bottom_all_for_series`) | series to bottom |
| 5 | `cancel_download` | "Cancel" (`action_cancel`) | cancels this item |
| 6 | `cancel_series` | "Cancel all for this series" (`cancel_all_for_series`) | cancels all items of the series |

The mockup has the same six labels in the same order (`src/screens.rs` `menu_button(...)` list). Missing: the actions are not wired (the mockup callback is empty, `|_| {}`), and the menu is not the Android `PopupMenu` shape (a popover menu is the GTK equivalent: `gtk::PopoverMenu` with a `gio::Menu` model, sections separated by the move group and the cancel group).

### 5.4 Chapter download menu (dropped)

`menu/chapter_download.xml` has five items (Start downloading now, Use external downloader (hidden), Use internal downloader (hidden), Delete, Cancel) and no code references it. It is not shown on any screen. The actions Start, Delete and Cancel are provided by the row menu of section 5.3 and the chapter screens of area:38 and area:39. User loss: none.

## 6. Reader and player keyboard and mouse equivalents (S Pen gestures dropped)

Observed S Pen actions (`app/src/main/res/xml/s_pen_actions.xml`):

| Action id | Trigger key sequence | Gesture | Label |
|---|---|---|---|
| `tachiyomi_next_page` | `CTRL_LEFT+DPAD_RIGHT` | click | "Next page" |
| `tachiyomi_previous_page` | `CTRL_LEFT+DPAD_LEFT` | double click | "Previous page" |
| `tachiyomi_back` | `BACK` | circle counter-clockwise | "Back" |
| `tachiyomi_next_chapter` | `N` | swipe right | "Next chapter" |
| `tachiyomi_previous_chapter` | `P` | swipe left | "Previous chapter" |

Proposal (final keys decided in areas 43 and 46; this table is the parity check):

| Action | Keyboard | Mouse | Gesture (touchpad) |
|---|---|---|---|
| Next page | Right arrow, Page Down, Space | left click on right third (existing reader zone) | swipe left |
| Previous page | Left arrow, Page Up, Shift+Space | left click on left third | swipe right |
| Back | Escape, Alt+Left | mouse back button (button 8) | two-finger swipe right |
| Next chapter | N | none | none |
| Previous chapter | P | none | none |

Keys N and P are the same letters Aniyomi's S Pen list uses, so the key bindings carry over. Nothing is lost for keyboard users.

## 7. Reader and player root views (layouts)

### 7.1 Reader root (`reader_activity.xml`)

- A full-size root with a viewer container (`viewer_container`, blocks descendant focus) inside `reader_container`.
- Page number pill: bottom centre of the container, Compose view (`page_number`). Owner: area:43.
- Navigation overlay (`navigation_overlay`): hidden, not clickable or focusable, used to show tap-zone hints. Owner: area:43 (Proposal: shown only while a key is held, see section 6).
- Dialog root (`dialog_root`): Compose host for reader dialogs. Owner: area:43 and area:44.

Immersive mode: the sidebar and the tab bar hide while the reader is on screen (mockup `src/main.rs:58` to `:59`, `screen:reader`). Keep.

### 7.2 Player root (`player_layout.xml`)

- Constraint root (not fit to system windows).
- Full-size mpv view `player` below the controls.
- Full-size Compose controls `controls` above.
- Owner: area:45 (mpv render API into a `GtkGLArea`) and area:46 (controls). The immersive rule of 7.1 applies to `screen:player` (mockup `src/main.rs:58`).

### 7.3 Reader error view (`reader_error.xml`)

Shown inside a page whose image failed to decode.

| Element | Text (English) | Visibility | Action |
|---|---|---|---|
| Message | "The image couldn't be loaded" (`decode_image_error`) | always | none |
| Retry button | "Retry" (`action_retry`) | always | retries the page decode |
| Open in WebView button | "Open in WebView" (`action_open_in_web_view`) | initially gone; shown when the source page has a web URL | opens the page in the web view (area:36) |

Layout: vertical, centred, 8 dp margins on each element. Proposal: the same three elements in a GTK `Box` with `halign center`; the Open in WebView button is hidden for local (CBZ, folder) sources.

## 8. Web view and unlock

### 8.1 Web view (`WebViewActivity`)

- Title from the `title_key` extra; the top bar has a back action labelled "Back" (`action_webview_back`) for the S Pen action and the Android back gesture.
- Transitions as section 3.
- Not exported, no configChanges handling needed on desktop.

### 8.2 Unlock (`UnlockActivity`)

- Title "Unlock %s" with the app name: English "Unlock Aniyomi" on Android. Kurobayomi shows "Unlock Kurobayomi" (`unlock_app_title` with the app name "Kurobayomi"; the English string is reused unchanged).
- Biometric prompt on Android; passphrase or PAM/polkit on Linux (area:36 decision).
- On an authentication error Android calls `finishAffinity()`, which closes the task. Kurobayomi: the lock window quits the application on the same error, so the behaviour is the same for the user.

## 9. State matrix for the scope screens

| Screen | Loading | Empty | Error | Selection |
|---|---|---|---|---|
| Deep-link loading page | spinner (section 1.1) | not applicable (NoResults replaces) | error panel with Retry and Search instead (Proposal) | none |
| Download queue (section 5) | rows from the queue; a loading spinner while the queue loads (area:23/24) | "No downloads" (area:23/24 string) | per row: "Download error" line under the title (area:23/24) | multi-select is not in Android; single row only |
| Reader error (section 7.3) | not applicable | not applicable | the error panel | none |
| Notice list | not applicable | not applicable | toast when the desktop server is unavailable | not applicable |
| Web view | page load progress (Android WebView progress) | blank page with the URL | error page per WebKit (area:36) | none |

## 10. Responsive behaviour

- Download rows: the title ellipsises at one line; the counter and the overflow button keep their width; the progress bar fills the remaining width. Minimum window width of the download page is 360 px (Proposal, matches the 16 dp side gutter of the mockup pages).
- Section headers: the title ellipsises; the drag handle is always visible.
- Reader and player: immersive mode hides the sidebar below 760 px wide (mockup `src/main.rs:59`, `wide = win.default_width() >= 760 || is_maximized`). Keep.
- Notice buttons: at most three; the desktop server may show fewer (no layout consequence).
- Transition duration: 300 ms (section 3).

## 11. What the mockup lacks (for this area)

1. No `open` or `command-line` handling: `src/main.rs` only connects `activate` (line 18). Deep links, `.tachibk` file arguments and launcher actions cannot reach the window (T-53a-001, T-53a-004, T-53a-010, T-53a-011).
2. No deep-link loading page, no resolver (T-53a-047, T-53a-049, T-53a-048, T-53a-050).
3. No notice dispatch and no `app.notice` action; the mockup has no notifications at all (T-53a-017, T-53a-018).
4. No section drag handle on the download header and no drag reordering of items (`download_header.xml`, `download_item.xml` `reorder`) (T-53a-053).
5. Download row menu items are not wired (`|_| {}` callbacks in `src/screens.rs`), no "Move series" logic (T-53a-060).
6. Transitions: the Stack uses `SlideLeftRight` for every page (`src/main.rs:34`), not shared-axis push and pop (T-53a-051).
7. No reader error panel with Retry and Open in WebView (T-53a-058).
8. No incognito banner notice on the desktop and no `DisableIncognito` action (T-53a-022).
9. No compact or mini player mode (T-53a-033).
10. No strings for the Proposal additions (`deeplink_no_source`, `deeplink_search_instead`, `track_connecting`) (T-53a-049, T-53a-047, T-53a-044 string list).
