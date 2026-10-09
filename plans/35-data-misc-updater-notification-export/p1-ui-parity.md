# Area 35 - UI parity

Scope files with UI: the notification surfaces rendered by `AppUpdateNotifier.kt`, `AppUpdateDownloadJob.kt` and `Notifications.kt`, and the export dialog in `LibraryExporter.kt` (whose composable lives in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt`, outside the scope file list, but it is the only UI for the exporter). The update screen (`app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt`, `app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt`) and the About row (`app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt`) are included because the updater's user flow ends there. Kurobayomi equivalents use GTK 4 with no libadwaita, the user's theme, and Tokyo Night colours from the app CSS.

Sources for every value: behaviour spec sections 1.10, 2.8, 3.1; data file section 7.

---

## 1. Notification surfaces (desktop)

Each notice is a GNotification (`gio::Notification`), so its look is the desktop's. The table gives the content and the behaviour the user must see. Android-only visuals (small icon tint, `setOngoing`, `setOnlyAlertOnce`, progress bar) are replaced as stated.

| Surface | Android (id) | Title | Body | Buttons (in order) | Body click (default action) | Linux replacement | Lifetime |
|---|---|---|---|---|---|---|---|
| Update available | 1, channel app updates, DEFAULT | "New version available!" | release tag (for example "v0.2.0") | "Download"; "What's new" | Android: starts download (E-U4). Kurobayomi: opens release page | slot `app-update.progress`, priority Normal, icon `software-update-available` | until dismissed by the user or replaced by download progress |
| Download progress | 1, ongoing, progress bar 0..100 | the version (from "Download"); if started from Retry, the app name (defect E-U3, fixed here) | "Downloading…" then "Downloading 42%" (text, throttled 200 ms, strictly increasing) | "Cancel" | none | slot `app-update.progress`, priority Low, body text updates; the bar is not available in GIO | until download ends (withdrawn) |
| Install ready | 2, ongoing, no bar | the version | "Downloaded <file>" (Kurobayomi) / "Tap to install update" (Android) | "Open folder"; "Dismiss" | opens folder (reveal file) | slot `app-update.install`, priority Normal | until dismissed |
| Download error | 3, ongoing (inherited, defect), no bar | builder title | "Download error" | "Retry"; "Cancel" | none | slot `app-update.error`, priority Normal | until dismissed or retry |
| Library/download/backup notices | owned by areas 21-26 | - | - | - | - | same `Notifier` | - |

Visual details (Android, for parity of intent):
- Small icon: `stat_sys_download_done` for the prompt and the install prompt, `stat_sys_download` for progress, `ic_warning_white_24dp` for the error. Kurobayomi: bundled symbolic icons (`software-update-available`, `folder-download-symbolic` or the bundled equivalents, `dialog-warning-symbolic`), never theme-dependent names (addenda rule 6). The exact bundled file names are decided by T-35a-087.
- Colour: Android `R.color.accent_blue` (NotificationExtensions.kt:71). Kurobayomi has no tint in GIO; the notification takes the desktop's colour scheme (KDE Plasma colours the notification from the theme). No accent is set by the app.
- Sound: default. The backup-complete notice is silent on Android (`setSound(null, null)`, Notifications.kt:174). Kurobayomi: priority High for the backup-complete category; sound is the desktop's decision (GIO cannot request silence portably; documented limitation).
- Title and body length: Android has no limit; Kurobayomi truncates the body at 200 characters with an ellipsis (PROPOSAL, because freedesktop servers may cut long text).
- Escaping: `&`, `<`, `>` in title and body are escaped when the server supports markup (PROPOSAL, Rust design pitfall 11).

Notification behaviour in the desktop shell:
- Clicking the body performs the default action (the router) and then withdraws the notice when the action carries `notice: Some(id)` (behaviour 1.9.2).
- When the desktop notification service is not running, `Notifier::show` returns `NotifyError::Unavailable`. The UI then shows the same text as an in-app toast (T-35a-010 scope: `gtk::Overlay` toast) so that the update prompt is never lost. Toast text = notice title + ": " + body.
- Do-not-disturb: the desktop decides; the app does not override it.

---

## 2. Update screen (in-app page replacing NewUpdateScreen)

Opened by the automatic check (MainActivity) and by the About manual check. Android: an `InfoScreen` with a Markdown body (NewUpdateScreen.kt, presentation/more).

Layout, top to bottom (Android order preserved):
1. Icon: `Icons.Outlined.NewReleases` (Kurobayomi: bundled symbolic "software-update-available").
2. Heading: "New version available!" (`update.available.title`), style: headline.
3. Subtitle: the version string (for example "v0.2.0"), style: title-small, secondary colour.
4. Scrollable Markdown body: the changelog with the "Checksums" section removed (behaviour 2.8). Links are styled with the primary colour (`SpanStyle(color = primary)` in Android). Kurobayomi: render the Markdown subset (headings, lists, emphasis, links, inline code, paragraphs) into a `gtk::TextView` with tags, links opened through `Opener::open_url`.
5. Text button "Open on GitHub" (`update.open_github`) with an external-link icon, below the body; opens `release_url`.
6. Bottom bar: "Not now" (`action_not_now`, secondary) and "Download" (`update_check_confirm`, primary). Android: InfoScreen's accept/reject pair.

Behaviour:
- "Download": starts the download (behaviour 2.8) and closes the screen. The progress notice then appears (section 1).
- "Not now": closes the screen; no state is changed (the 3-day throttle was already set when the check found the update).
- Escape key: same as "Not now" (PROPOSAL; Android back gesture).
- The screen is a page in the app's navigation stack (area:36-ui-shell-navigation-security-webview), not a modal, so the back button closes it.
- Empty changelog (body empty): show "No release notes." (new string `update.no_notes`, PROPOSAL; Android shows an empty body).

States:
- Loading: not applicable (the check already completed before the screen opens).
- Error: not applicable; a failed download shows the error notice (section 1), not this screen.
- Long changelog: scrolls inside the body; the buttons stay visible (bottom bar not in the scroll area).

Phone width: not applicable (desktop only).

---

## 3. About page "Check for updates" row

Android (AboutScreen.kt:98-134): a `TextPreferenceWidget` titled "Check for updates" (`MR.strings.check_for_updates`), shown only when `updaterEnabled`, with a trailing `CircularProgressIndicator` (28 dp, 3 dp stroke) visible while checking.

Kurobayomi:
- Row title "Check for updates" (spec/settings.json:3173). Subtitle while idle: the current version "Version 0.1.0" (PROPOSAL: from `env!("CARGO_PKG_VERSION")`).
- When the updater feature is off or `update.repo` is empty: the row is shown disabled with the subtitle "Set update.repo in settings to enable update checks" (`update.disabled_hint`). Android hides the row entirely when the updater is off; Kurobayomi shows it disabled so the user knows why (deliberate difference, PROPOSAL).
- While checking: spinner (GTK `gtk::Spinner`, 16 px) in the row's suffix; the row is insensitive (no double click; Android `if (!isCheckingUpdates)` guard).
- Results (behaviour 2.7):
  - New version: open the update screen (section 2).
  - No new version: toast "No new updates available" (`update.none`).
  - No release for this platform: toast "No build for this platform in the latest release" (`update.no_asset`) (Android shows the misleading "No new updates" here; PROPOSAL change).
  - No releases at all (404): toast "No releases published yet".
  - Rate limited: toast "GitHub rate limit reached, try again later" (`update.rate_limited`).
  - Other errors: toast with the error text (Android: `e.message`).
- Toast duration: GTK toast default (short, about 3 seconds).

---

## 4. Export dialog (library list)

Entry: Settings > Data > Export group > "Library List" row (spec/settings.json "Export" section, line 2457; the row title is "Library List").

Android layout (SettingsDataScreen.kt:414-498):
- `AlertDialog`, title "Select data to include" (`migration_dialog_what_to_include`).
- Body: a column of four rows, each a `Checkbox` followed by its label. Order: Title, Type, Author, Artist. Rows are not clickable as a whole (only the checkbox and its label Text; Android Row contains Checkbox + Text, label is not a click target) (OBSERVED; PROPOSAL: the whole row toggles, a usability improvement).
- Buttons: `TextButton` "Save" (`action_save`) confirm, `TextButton` "Cancel" (`action_cancel`) dismiss.
- Dialog dismissible by tapping outside or Back (`onDismissRequest`).

Sensitivity rules (OBSERVED, exact):
- Title checkbox always enabled.
- Unchecking Title sets Type, Author and Artist to false and makes them disabled.
- Type, Author, Artist are enabled only while Title is checked (their values are kept while disabled but unchecked by the Title rule).
- Re-checking Title does NOT restore the earlier Type/Author/Artist values (they stay unchecked).

Kurobayomi (GTK 4, no libadwaita):
- A `gtk::Window` (modal, transient for main window) with a vertical `gtk::Box` of four `gtk::CheckButton`s (labels "Title", "Type", "Author", "Artist"), then a button row: "Cancel" (`gtk::Button`) and "Save" (suggested-action style, `.suggested-action`).
- Enter activates Save (default widget); Escape activates Cancel.
- Sensitivity: `title.connect_toggled` sets `sensitive` on the three dependent buttons and clears them when Title turns off (rule set above).
- The window is 360 px wide (PROPOSAL), resizable false.

After Save (behaviour 3.1 step 4-5):
- The save file chooser opens (`gtk::FileDialog::save`), title "Export library" (PROPOSAL string `export.title`), default name `kurobayomi_library.csv`, filter "CSV files" (`text/csv`).
- Cancelling the chooser: nothing (no toast, dialog closes).
- Choosing a file: the CSV is written (section 5 below).

Options memory: the last-used options are kept while the settings page is open; leaving the page resets them to all checked (behaviour 3.1 step 7).

Accessibility: each CheckButton has its label as the accessible name; the dependent checkboxes are announced as "dimmed" when insensitive (GTK default).

---

## 5. Export progress and result

Android: no progress UI; the write is fast; result is a toast "Library Exported" (`library_exported`). Errors: none shown (defect 3.5).

Kurobayomi:
- Start: no spinner for small libraries. For libraries above 5 000 favourites (PROPOSAL threshold) show a `gtk::Spinner` in the dialog's place until done; the window is closed when the save starts.
- Success toast: "Library Exported" (`export.done`), exact Android text kept.
- Failure toast: "Export failed: <error text>" (`export.failed`).
- Empty library: the file is written (empty, 0 bytes) and the success toast shows. PROPOSAL: show "Nothing to export: your library has no favourites" (`export.empty`) and still write the empty file? Decision: write the file and show the success toast (parity); the note is added in the toast text only when zero rows: "Library Exported (0 entries)" (`export.done_empty`, PROPOSAL).

---

## 6. Keyboard and mouse equivalents

| Action | Android | Kurobayomi |
|---|---|---|
| Open update screen from prompt | tap notification body (download) | click notification body (opens release page, see section 1) |
| Download from prompt | "Download" action | "Download" button |
| Cancel download | "Cancel" action | "Cancel" button in progress notice; Escape is not bound |
| Install | "Install" action | "Open folder" button |
| Mark chapters read | "Mark as read" action (library notices, areas 26) | same button |
| Export dialog Save | button | Enter or click |
| Export dialog Cancel | button / outside tap | Escape or click |
| S Pen next/previous page | remote action | keyboard: arrow keys (reader area) |
| S Pen next/previous chapter | remote action | keyboard: N and P (reader and player) |
| S Pen back | remote action | Back navigation (Alt+Left, Backspace) |

Keyboard shortcuts are owned by areas 43-45 (reader and player) and area:36-ui-shell-navigation-security-webview (navigation). This scope only lists them.

---

## 7. Responsive behaviour

- Desktop window widths from 640 px. The update screen body is capped at 720 px line length; the export dialog is fixed at 360 px.
- No phone layout (the Android app has phone and tablet layouts; Kurobayomi is desktop only).
- Font scaling: the dialog and update screen grow with the user's font size; no fixed heights for text.

---

## 8. What the mockup lacks (OBSERVED from /home/blackbox/kurobayomi)

The mockup (`src/main.rs`, `src/settings.rs`, `src/ui.rs`, `spec/settings.json`) has:
- A spec-driven settings screen with an "Export" group and a "Library List" action row (spec/settings.json lines 2452-2466) whose dialog `export_columns` has four switches (lines 2469-2500). The switches are spec-driven and have no sensitivity rules and no Save behaviour.
- An About spec with "Check for updates" (spec/settings.json:3173) and the notes at line 3197.
- A "Manage notifications" action row (spec/settings.json:2763) that the spec describes as opening system notification settings; Kurobayomi has no equivalent screen.
- A "Hide notification content" switch (spec/settings.json:2701-2704, default false).

The mockup lacks:
1. Any notification code: no `Notifier`, no GAction, no `app.notice`, no notice ids. No notification is posted anywhere.
2. The update prompt, the update screen (as a real page), the download progress, the install-ready notice and the error notice.
3. The About check behaviour (spinner, toasts, disabled state when the repo is not configured).
4. The export dialog behaviour (sensitivity rules, Save, file chooser, CSV write, toasts).
5. The "Manage notifications" replacement (per-category toggles, P1 per T-35a-004).
6. The `hide_notification_content` gate (no notifier to gate).
7. Toast infrastructure (`gtk::Overlay` toast) for the no-notification-service fallback and for export and update messages.
8. The Markdown renderer for release notes.
9. The `kuro-platform` crate (traits and Linux implementations).

Spec changes proposed (not made here; the spec is area:47-settings-screens-general and area:47b-settings-widgets-search-more territory):
- Replace the "Manage notifications" action with "Notifications" (in-app toggles per category, section 3.2 of the data file) (T-35a-004).
- Add an `update.repo` text row in the About group, below "Check for updates" (T-35a-046).
- Change the export dialog note: the "Save" button is the default widget.
