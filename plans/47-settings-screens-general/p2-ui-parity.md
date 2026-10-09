# 47-settings-screens-general — part 2 UI parity

Every screen in the 18 scope files. "Mockup" means `/home/blackbox/kurobayomi/spec/settings.json` (screen ids) rendered by `/home/blackbox/kurobayomi/src/settings.rs` and `screens.rs`. Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`.

Conventions for Linux: GTK4 widgets, no libadwaita. Dialogs are `gtk::AlertDialog` (text-only) or transient modal `gtk::Window` (with entry). Toasts are the in-window overlay from `p2-behavior.md` section 11.

---

## 1. Settings shell (`SettingsScreen.kt`)

### 1.1 Layout
- **Narrow (< 720 px)**: one page at a time. Root page = settings main list. Pages push on the stack with a slide transition (`DefaultNavigatorScreenTransition`: horizontal slide, 300 ms in Compose defaults; proposal: `gtk::StackTransitionType::SlideLeft` with 200 ms).
- **Wide (>= 720 px)**: two panes. Left = main list (fixed ~ 340 px, `gtk::Paned` position). Right = page stack. Starting page on wide is Appearance.
- Insets: Compose used system bars; on Linux use the window's own content area, no padding tweaks.

### 1.2 States
- Page opened from a destination id (`About`, `DataAndStorage`, `Tracking`) pre-selects that item in the left pane on wide and pushes it on narrow.
- Back: inner page pops; with no inner page, the shell closes (back to the main window page).

### 1.3 Input
- Back button in the header bar (`go-previous-symbolic`), Escape, Alt+Left, and mouse back button (button 8) all trigger back.
- Keyboard focus: the main list gets focus when the shell opens; Up/Down moves rows (GTK ListBox default).

### 1.4 Mockup gaps
- The mockup `settings.json` has a `settings` root with `settings_main` items and separate `settings_*` screens. There is no two-pane mode; the mockup shows only the narrow layout. Gap: implement the two-pane switch (T-47b-002).
- Back from a sub-page always returns to the root in the mockup; Kotlin returns to the previous page. Gap: keep a stack (T-47b-003).

---

## 2. App language (`AppLanguageScreen.kt`)

### 2.1 Layout
- Title bar: "App language" (`pref_app_language`), back button.
- Scrolling list, one row per language, 1 row = `ListItem` in Compose:
  - Headline (primary text): native name (e.g. "Deutsch", "العربية"). For the default row: "Default".
  - Supporting text (secondary): English name, e.g. "German", "Arabic". Absent for "Default" (Kotlin: `localizedDisplayName = null`).
  - Trailing: check icon in the primary colour on the active row only.
- No dividers, no section headers.

### 2.2 States
- Loading: none (list is built synchronously in `remember`).
- Empty: not possible (the default row always exists).
- Selected: the row whose tag equals the current app locale; initially from `AppCompatDelegate.getApplicationLocales()`; empty means "Default".

### 2.3 Interaction
- Tap or Enter on a row: selects it. Compose applied immediately; Linux shows a toast "Restart Kurobayomi to apply the language." (new key, `p2-behavior.md` section 15.3) and moves the check mark.
- No confirm dialog.

### 2.4 Mouse and keyboard
- Click row, arrow keys to move focus, Enter to select, Escape to go back.

### 2.5 Responsive
- Same on narrow and wide. On wide it sits in the right pane.

### 2.6 Mockup gaps
- Mockup lists 7 languages (Default, English, Arabic, French, German, Japanese, Spanish). Kotlin lists every catalog folder (67 folders in this checkout). Gap: generate from catalogs (T-47b-005).
- Mockup applies nothing; the check mark moves on click only. Gap: persist and show the restart toast (T-47b-007).
- Mockup uses `icon "object-select-symbolic"` visibility toggling per row; keep, but compute from the stored key.

---

## 3. Anime extension stores (`AnimeExtensionStoresScreen.kt`, `components/anime/*`)

### 3.1 Page chrome
- Header bar: back, title "Extension stores" (`extensionStores`), right side: Refresh icon button (`view-refresh-symbolic`, tooltip "Refresh"; `action_webview_refresh`).
- Floating action: Extended "Add" button (`action_add`), bottom-right, icon `list-add-symbolic`. Shrinks to icon-only when the list scrolls forward and expands at the top or when scrolling back (see `shouldExpandFAB` in `p2-behavior.md` section 3.5). Linux: a `gtk::Button` overlay with `suggested-action` and `pill`.

### 3.2 States
- **Loading** (store list not yet loaded): full-page centred spinner (`gtk::Spinner`, 32 px). No header actions disabled. Mockup: no loading state (gap; add it).
- **Empty** (`stores.isEmpty`): centred message "You haven't added an extension store yet" (`extensionStoresScreen.emptyLabel`), FAB remains. No illustration.
- **Populated**: list of cards.
- **Refreshing**: the Refresh button has no visual state in Kotlin; Linux adds a spinner in the header while `refreshAll` runs (proposal, improvement, T-47b-012).

### 3.3 Card (one per store)
Layout (vertical box inside a `.card` frame, margins 12 px top/start/end, action row margin-bottom 6 px):
1. Row 1: icon `tag-symbolic` (16 px) + name in title style (bold, 1.1 em). Name may ellipsize at end.
2. Row 2 (right-aligned icon buttons, in order):
   - Globe `web-browser-symbolic`, tooltip "Open in browser", opens `contact.website`.
   - Discord (**only when `contact.discord` is set**), tooltip "Discord", opens the Discord link.
   - Copy `edit-copy-symbolic`, tooltip "Copy to clipboard", copies `indexUrl`, toast "Copied to clipboard: <truncated URL>".
   - Delete `user-trash-symbolic`, tooltip "Delete", opens delete dialog.
- Card hover: subtle background (theme). Card is not clickable as a whole (Kotlin is not).

### 3.4 Dialogs
Each dialog below is either a `gtk::AlertDialog` or a transient modal `gtk::Window`. Button order follows GNOME/KDE convention: primary on the right.

**Add store** (entry dialog, `gtk::Window`, width 420 px)
- Title "Add extension store".
- Single-line entry, placeholder "Store index URL" (`extensionStoresScreen.addStoreInput.inputLabel`), purpose URL, focused on open.
- Under the entry: helper text. Normal = "*required"; duplicate = "This extension store already exists" (error style, entry red outline); server error = raw error text (error style).
- Buttons: Cancel (left), Add (right, `suggested-action`). Add disabled when text empty, duplicate, or processing. Label becomes "Processing…" while processing; entry disabled during processing.
- Enter in the entry = Add (when enabled). Escape = Cancel.

**Remove store** (`gtk::AlertDialog`, destructive)
- Title "Remove extension store". Detail: `Do you wish to remove the extension store "<name>" (<url>)?`.
- Buttons: Cancel, OK (`destructive-action`).

**Deep-link add** (entry is read-only)
- Title "Add extension store". Body "Do you wish to add the extension store below?" then read-only entry with the URL.
- Helper text: duplicate message or error text (error style), otherwise none.
- Buttons: Cancel, Add (disabled on duplicate or processing), label Processing…/Add.

### 3.5 Toasts
- None from this page. Errors stay in the dialog.

### 3.6 Mouse and keyboard
- Refresh: F5 (proposal; Ctrl+R is also common, use F5 only).
- New store: Ctrl+N opens Add dialog (proposal).
- Card buttons reachable with Tab; Delete button is last in the card.
- Right-click on a card: no menu (Kotlin has none). Proposal: context menu with the same four actions (T-47b-059).

### 3.7 Responsive
- Cards stretch to the content width up to 720 px; beyond that, center the column (max width 720 px). Two columns on very wide windows (> 1280 px) is a proposal, not parity.

### 3.8 Mockup gaps
- Mockup cards are static "Store A", "Store B". Gap: backed by the DB (T-47b-009).
- Mockup card buttons: "Open website" and "Copy to clipboard" and delete only; no Discord button. Gap: Discord button conditional (T-47b-015).
- Mockup dialog ids `add_store`, `add_store_deeplink`, `delete_store` exist; no processing or error states. Gap (T-47b-016/017).
- Mockup Refresh toolbar is present; no spinner. Gap (T-47b-012).
- Mockup uses "Add" button in the FAB slot as pill bottom-right: matches.

---

## 4. Manga extension repos (`MangaExtensionReposScreen.kt`, `components/manga/*`, `components/ExtensionReposDialogs.kt`)

### 4.1 Page chrome
- Header: back, title "Extension repos" (`label_extension_repos`), Refresh icon (same as 3.1).
- FAB: Extended "Add" (`action_add`).

### 4.2 States
- **Loading**: centred spinner, as 3.2.
- **Empty**: "You have no repos set." (`information_empty_repos`).
- **Populated**: cards.

### 4.3 Card
- Row 1: `tag-symbolic` + repo name (bold).
- Row 2 (right-aligned): External link (`external-link-symbolic`, tooltip "Open in browser") -> website; Copy (`edit-copy-symbolic`, tooltip "Copy to clipboard") -> copies `<baseUrl>/index.min.json`; Delete (`user-trash-symbolic`, "Delete").
- No Discord button.

### 4.4 Dialogs

**Add repo** (entry dialog)
- Title "Add repo" (`action_add_repo`).
- Body: "Add additional repos to <app>. This should be a URL that ends with "index.min.json"." (`action_add_repo_message`, AYMR).
- Entry labelled "Repo URL" (`label_add_repo_input`), purpose URL. Helper text: "*required" or "This repo already exists!" (`error_repo_exists`) when the typed value equals a stored base URL (rare, see behaviour spec 4.4).
- Buttons: Cancel, Add (enabled when non-empty and not flagged). Add closes the dialog immediately (no spinner) and the result appears as a toast or as the conflict dialog.
- Enter = Add. Escape = Cancel.

**Remove repo** (`gtk::AlertDialog`, destructive)
- Title "Delete repo" (`action_delete_repo`). Body: `Do you wish to delete the repo "<base url>"?`. Buttons Cancel, OK (destructive).

**Fingerprint conflict** (`gtk::AlertDialog`)
- Title "Signing Key Fingerprint Already Exists". Body: "Repository <new name> has the same Signing Key Fingerprint as <old name>.\nIf this is expected, <old name> will be replaced, otherwise contact your repo maintainer."
- Buttons: Cancel, Replace (`suggested-action`? no: plain, since replace is the destructive action; use `destructive-action`).

**Deep-link add** (`gtk::AlertDialog`)
- Title "Add repo". Body: `Do you wish to add the repo "<index url>"?`. Buttons Cancel, Add.

### 4.5 Toasts
- "Invalid repo URL" (`invalid_repo_name`) and "This repo already exists!" (`error_repo_exists`), 2 s, bottom-centre overlay.

### 4.6 Mouse and keyboard
- Same as 3.6.

### 4.7 Mockup gaps
- Mockup dialogs `add_repo`, `delete_repo`, `replace_repo`, `confirm_add_repo` exist. Gaps: the entry dialog has no live duplicate state; Add does not close before the result; "Invalid repo URL" and "already exists" toasts are missing; no spinner.
- Mockup Replace button label "Replace" matches.

---

## 5. Create backup (`CreateBackupScreen.kt`)

### 5.1 Layout
- Header: back, title "Create backup".
- Scroll area with three cards in order:
  1. **Library** card: checkboxes "Library entries", "Chapters and episodes", "Tracking", "History", "Categories", "All read/watched entries".
  2. **Settings** card: "App settings", "Extension stores", "Custom buttons", "Source settings", "Include sensitive settings (e.g., tracker login tokens)".
  3. **Extensions** card: "Extensions".
- Card titles: "Library", "Settings", "Extensions" (`titleSmall`, secondary colour).
- Bottom bar pinned outside the scroll area: divider, then full-width "Create" button (`suggested-action`, 16 px side margins, 8 px vertical).

### 5.2 Checkbox rules
- Chapters, Tracking, History, All read/watched entries: disabled (insensitive, still show their value) when "Library entries" is off.
- Include sensitive settings: disabled when neither "App settings" nor "Source settings" is on.
- Defaults: all on except "Include sensitive settings" and "Extensions" (off).
- Create button: disabled unless `canCreate()` (library entries, categories, app settings, extension stores, custom buttons, or source settings on). "Extensions" alone does not enable it.

### 5.3 States
- Idle: as above.
- Busy (backup already running): Create click shows toast "Backup is already in progress" and does not open the dialog. Button remains enabled.
- Picker cancelled: no change, screen stays open.
- No file chooser: toast "No file chooser available" (proposal wording).
- After hand-off: screen pops to previous page; backup progress appears as a desktop notification (area:22).

### 5.4 Banner
- Android-only MIUI banner: dropped (`p2-behavior.md` section 12).

### 5.5 Mouse and keyboard
- Space toggles a focused checkbox. Tab order: checkboxes top to bottom, then Create.
- Enter on focused Create = click Create.

### 5.6 Responsive
- Cards max width 720 px, centered. Narrow: full width minus 16 px gutters.

### 5.7 Mockup gaps
- Mockup has the page in the spec (`settings_create_backup`), but `body()` has no arm for it. Gap: wire it (T-47b-029).
- Mockup has no busy check and no disabled rules. Gap (T-47b-029/030).

---

## 6. Restore backup (`RestoreBackupScreen.kt`)

### 6.1 Layout
- Header: back, title "Restore backup".
- Scroll area, top to bottom:
  1. (Optional) MIUI banner: dropped.
  2. Options card (only when the file is valid): seven checkboxes "Library", "Categories", "App settings", "Extension stores", "Custom buttons", "Source settings", "Extensions".
  3. Error card (when there is an error): selectable text, bold section headers.
- Bottom bar: "Restore" button (`suggested-action`), disabled when `!canRestore || !options.canRestore()`.

### 6.2 States
- **Validating** (new, proposal): spinner row "Checking backup file…" in place of options; Restore disabled. Kotlin validates synchronously and never shows this state; keep it for responsiveness.
- **Valid, no issues**: options card, no error card, Restore enabled when at least one option checked.
- **Valid, missing components**: options card AND error card with "You may need to install any missing extensions and log in to tracking services afterwards to use them." then "Missing sources:" list and/or "Trackers not logged into:" list. Restore enabled.
- **Invalid file**: no options card. Error card: bold "Invalid backup file:", path, blank, bold "Full error:", the message. Restore disabled.
- **Options all unchecked**: Restore disabled, options card still shown.

### 6.3 Error text rendering
- Bold headers via Pango markup (`<b>…</b>`) with escaping. Lists as "- item" lines.
- Selectable: user can select and copy (Kotlin `SelectionContainer`).

### 6.4 Interaction
- Restore: starts job then pops the page immediately (Kotlin). Linux: same, then notification.
- Busy restore already running: toast "Backup is already in progress" and stay on page (proposal; Kotlin silently drops).

### 6.5 Mouse and keyboard
- Same as 5.5. Error card text selectable with mouse.

### 6.6 Responsive
- Same as 5.6.

### 6.7 Mockup gaps
- Mockup: `settings_restore_backup` exists in spec; no `body()` arm. Gap (T-47b-031).
- Mockup spec has the missing-components text but no invalid-file path, no validating state. Gap (T-47b-031/033).

---

## 7. Data and storage usage card (`StorageInfo.kt`)

This composable is embedded in the Data and storage page (part 1 owns the page; this file owns the card).

### 7.1 Layout (per storage root)
- Header line: absolute path, style "header" (title-like, medium weight).
- Progress bar: full width, 12 px tall, rounded corners (small radius), fill = used fraction.
- Caption: "Available: <size> / Total: <size>" in small secondary text.
- Vertical spacing: 4 px between header, bar, caption; 8 px between roots.

### 7.2 States
- No roots: nothing rendered (the section just shows no storage lines). Proposal: show "No storage locations found" (new key), not parity; reason: empty silence is confusing.
- Stat failed (`total == None`): show path and caption "Storage size unavailable", no bar.
- Normal: bar and caption.

### 7.3 Interaction
- None. Values are static for the page's lifetime (Kotlin also). Proposal: refresh on page show.

### 7.4 Mockup gaps
- Mockup shows a single hard-coded "Available: 12.3 GB / Total: 64 GB" line. Gap: per-root bars from statvfs (T-47b-037/038).

---

## 8. Debug info (`DebugInfoScreen.kt`)

### 8.1 Layout
- Preference-style list (`PreferenceScaffold`): rows of title + subtitle, navigation rows with a right chevron.
- Order: "Worker info" (navigates), "Backup file schema" (navigates), group "App info" (Version, Build time, WebView version [Linux: WebKitGTK], plus Linux rows GTK version and Session), group "Device info" (Model, OS, Kernel).
- Group headers: small caps, secondary colour, hard-coded English (Kotlin does the same).

### 8.2 States
- Every row is synchronous except the profile status (dropped on Linux). No loading state.

### 8.3 Interaction
- Rows are read-only except the two navigation rows. Row click on a value row does nothing (Kotlin does nothing). Proposal: long-press or right-click copies the subtitle (T-47b-059 family). Not parity.

### 8.4 Mockup gaps
- Mockup lists "Version", "Build time", "Profile compilation status", "WebView version", "Model", "Android version" as action rows. Gap: remove the profile row and the Android-only wording, add GTK and session rows (T-47b-040/041/042).
- Mockup: the Worker info and Backup file schema rows are `link` items: match.

---

## 9. Worker info (`WorkerInfoScreen.kt`)

### 9.1 Layout
- Header: back, title "Worker info", right action copy (`edit-copy-symbolic`, tooltip "Copy to clipboard").
- Body: one horizontally scrollable monospace text area (no wrapping). Three sections, each with a heading (`titleMedium`, 8 px vertical padding): "Enqueued", "Finished", "Running".

### 9.2 States
- Each section shows "-" when empty.
- Live: updates as jobs change (watch channel, see behaviour spec 9.3).

### 9.3 Interaction
- Copy copies all text (Kotlin concatenation). Toast "Copied to clipboard: …" with truncation.
- Horizontal scrolling by scrollbar and Shift+wheel.

### 9.4 Mockup gaps
- Mockup has static text. Gap: live registry (T-47b-043/044).

---

## 10. Backup file schema (`BackupSchemaScreen.kt`)

### 10.1 Layout
- Header: back, title "Backup file schema", copy action.
- Body: vertically scrollable monospace text, 16 px padding, selectable.

### 10.2 States
- Static text, always present.

### 10.3 Mockup gaps
- Mockup shows an abbreviated three-field message. Gap: the full proto from `proto/backup.proto` (T-47b-045).

---

## 11. Shared visual rules for this area

- Card: frame with `.card` class, 12 px radius, no shadow beyond theme.
- Spacing: 8 px base; page margins 16 px; list row height min 48 px.
- Typography: titles via theme headers; monospace via `.monospace` class (for worker info and schema).
- Colour: never hard-code. Error text uses `.error`, dim text uses `.dim-label`, destructive buttons use `.destructive-action`, primary buttons `.suggested-action`.
- Icons: symbolic only (see `p2-rust-design.md` section 7).
- Motion: GTK default transitions only (Kotlin animates list items; dropped).

---

## 12. Keyboard and mouse summary (this area)

| Action | Keyboard | Mouse |
|---|---|---|
| Back | Escape, Alt+Left | back button, header back |
| Add store/repo | Ctrl+N (proposal) | FAB |
| Refresh list | F5 (proposal) | header refresh |
| Confirm dialog | Enter (default button) | click |
| Cancel dialog | Escape | click Cancel |
| Toggle checkbox | Space | click |
| Copy card URL | Tab to button, Enter | copy button |
| Open website | Tab to button, Enter | globe button |
| Select language | Up/Down, Enter | click row |

---

## 13. Accessibility

- Every icon-only button has a tooltip (= Compose content description). Use the Kotlin strings, listed in `p2-behavior.md` section 15.
- Decorative icons (label icon, Discord) have no accessible name (Kotlin passes `null`); keep that.
- Focus order follows reading order; dialogs trap focus.
- Error text is announced: set `accessible-role` alert on the helper label (proposal).
- Minimum hit target 24 px (GTK default is fine; do not shrink icon buttons).

---

## 14. Gap summary for the mockup (for the implementer)

1. Settings shell: two-pane, real back stack, destination ids (T-47b-001..003).
2. App language: real catalog list, persisted choice, restart toast (T-47b-005..008).
3. Anime stores: DB list, loading and error states, Discord button rule, dialog states, refresh spinner (T-47b-009..018).
4. Manga repos: DB list, toasts, conflict dialog, add dialog without duplicate false positives, deep link (T-47b-019..026).
5. Create backup: page wired, rules, busy check, save dialog (T-47b-027..030).
6. Restore backup: page wired, async validation, missing/invalid states, job start (T-47b-031..036).
7. Storage card: per-root statvfs bars (T-47b-037..039).
8. Debug info, worker info, schema: real data, Linux rows, drop profile row (T-47b-040..051).
