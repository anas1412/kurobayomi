# 22-backup-restore — Part 1 UI parity

Scope UI files (all under `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/`): `RestoreBackupScreen.kt` (240 lines), `CreateBackupScreen.kt` (148), `StorageInfo.kt` (75). Entry rows live in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (outside scope; referenced for placement).

Kurobayomi target: GTK 4.16 with vanilla widgets (no libadwaita). Desktop only: there is no phone width target. Rules that still apply: no horizontal page scroll at narrow window widths (labels wrap), 16 px side gutter, and every text selectable where the Kotlin uses `SelectionContainer`.

---

## 1. Restore backup screen

### 1.1 Visible elements (Kotlin, top to bottom)
1. App bar: title `MR.strings.pref_restore_backup` ("Restore backup"), navigate-up icon (`navigateUp = navigator::pop`), scroll behaviour (`RestoreBackupScreen.kt:54-61`).
2. Warning banner (Android only): `MR.strings.restore_miui_warning` when `DeviceUtil.isMiui && isMiuiOptimizationDisabled()` (`:72-76`). Not shown on Linux (drop, T-22a-039).
3. Option card (`SectionCard`, no title) with 7 `LabeledCheckbox` rows, only when `state.canRestore` (`:78-92`). Labels in order: "Library", "Categories", "App settings", "Extension stores", "Custom buttons", "Source settings", "Extensions". Defaults: first six checked, "Extensions" unchecked.
4. Error card (`SectionCard`) when `state.error != null` (`:94-96`, `errorMessageItem` `:101-164`). Text is inside `SelectionContainer` (selectable).
5. Pinned action at the bottom: `LazyColumnWithAction` with label "Restore" (`MR.strings.action_restore`), enabled iff `canRestore && options.canRestore()` (`:63-70`).

### 1.2 Error card variants (exact text)
- Missing components (`MissingRestoreComponents`, `:112-136`):
  1. `backup_restore_content_full` ("You may need to install any missing extensions and log in to tracking services afterwards to use them.") followed by a newline.
  2. If sources not empty: a blank line; bold `backup_restore_missing_sources` ("Missing sources:") line; then each source as `- name`, joined by newline (`joinTo(separator="\n- ", prefix="- ")`).
  3. If trackers not empty: a blank line; bold `backup_restore_missing_trackers` ("Trackers not logged into:"); then each tracker as `- name`.
- Invalid file (`InvalidRestore`, `:138-150`): bold `invalid_backup_file` ("Invalid backup file:") line; the URI string on its own line; blank line; bold `invalid_backup_file_error` ("Full error:") line; the message line.
- Other (`:152-154`): `error.toString()` (not reachable in the current code).

### 1.3 States
| State | Checkboxes | Error card | Action | Source |
|---|---|---|---|---|
| Validating | not shown (validation is synchronous in the model init; no spinner in Kotlin) | none | disabled (`canRestore` false) | `RestoreBackupScreen.kt:172-174,227` |
| Invalid | not shown (`canRestore` false) | Invalid file card | disabled | `:192-201` |
| Ready (no missing) | shown | none | enabled iff at least one option | `:211` |
| Ready (missing sources or trackers) | shown | Missing card | enabled iff at least one option | `:203-209` |
| Restore started | (screen popped) | — | — | `:67-70` |

Linux adds a "Validating" state with a `gtk::Spinner` because validation runs off the UI thread (design §7.1). The text of the state is "Checking backup file…" (PROPOSAL key, not in the Kotlin strings).

### 1.4 Interactions
- Checkbox toggle: updates `options` immediately; re-evaluates the action enabled state (`toggle` `:176-182`).
- Action click: starts the job with the current options, then pops the screen (`:67-70`).
- Back / navigate-up: pops without starting anything.
- Keyboard (desktop equivalents): Tab moves through the checkboxes and then the action; Space toggles a checkbox; Enter on the focused action button starts the restore; Escape closes the page (pop). Mnemonics: none in Kotlin.
- Mouse: click on a row label toggles the checkbox (GTK: `gtk::CheckButton` with label, the whole row is the button).
- Text selection in the error card: `gtk::Label` with `set_selectable(true)`.
- Long lists (many missing sources): the error card grows; the page scrolls vertically (GTK `ScrolledWindow`). Kotlin `LazyColumn` behaves the same.

### 1.5 Mockup gaps (`/home/blackbox/kurobayomi/spec/settings.json`, id `settings_restore_backup`, line 2604)
- Controls are `switch` items; Kotlin uses labelled checkboxes. Use `gtk::CheckButton` for parity (decision in T-22a-044).
- No validation: the spec is static. There is no Missing or Invalid card driven by a backup file, no "Trackers not logged into" list, no disabled state logic.
- The action is labelled "Restore" but there is no file chooser before the screen (the settings row "Restore backup", spec line 2412, is type `action` with no `target`).
- No progress UI, no cancel, no completion or error notification, no error log action.
- The spec notes (line 2663) describe the card text correctly; the rendering logic is missing.

---

## 2. Create backup screen (in scope file `CreateBackupScreen.kt`)

### 2.1 Visible elements (Kotlin)
1. App bar: title `MR.strings.pref_create_backup` ("Create backup") with navigate-up (`CreateBackupScreen.kt:59-66`).
2. MIUI warning banner (Android only; drop).
3. Card "Library" (`MR.strings.label_library`): six checkboxes: "Library entries" (`AYMR.strings.entries`), "Chapters and episodes", "Tracking", "History", "Categories", "All read/watched entries" (`AYMR.strings.non_library_settings`) (`:90-94`).
4. Card "Settings" (`MR.strings.label_settings`): "App settings", "Extension stores", "Custom buttons", "Source settings", "Include sensitive settings (e.g., tracker login tokens)" (`:96-100`).
5. Card "Extensions" (`MR.strings.label_extensions`): "Extensions" (`:102-106`).
6. Pinned action "Create" (`MR.strings.action_create`), enabled iff `options.canCreate()` (`:68-72`).

Checkbox enabled rules (`BackupOptions.kt:46-110`):
- Chapters, Tracking, History, All read/watched entries: enabled iff "Library entries" is checked.
- Include sensitive settings: enabled iff App settings or Source settings is checked.
- All others always enabled.

Default values: Library entries, Chapters, Tracking, History, Categories, All read/watched = true; App settings, Extension stores, Custom buttons, Source settings = true; Include sensitive = false; Extensions = false.

### 2.2 Click behaviour (`:72-82`)
- Manual create job not running: open the save picker with suggested name (pattern in data file §7). If no picker is available: toast "No file picker app found" (`MR.strings.file_picker_error`).
- Manual create job running: toast "Backup is already in progress" (`MR.strings.backup_in_progress`).
- Picker cancelled: nothing happens; the screen stays open.
- Picker success: start the create job with the URI and options, then pop (`:49-56`, `:140-142`).

### 2.3 States
- Idle: all checkboxes shown with defaults; Create enabled.
- Nothing selected that can create (`canCreate()` false): Create disabled. Example: only "Extensions" checked, or only "Tracking" checked.
- Running: the screen is popped on Create, so there is no running state here. Progress is the create-job notification (area:21).

### 2.4 Interactions (Linux equivalents)
- Save dialog: `gtk::FileDialog::save` with the suggested name; filters: `*.tachibk` and all files.
- Toast replacement: no Android toast. Use an in-window banner (`gtk::Revealer` with a `gtk::Label`, auto-hide after 4 s) for both messages.

### 2.5 Mockup gaps (`settings_create_backup`, `spec/settings.json:2507-2600`)
- Switches instead of checkboxes (same fix as §1.5).
- No enabled rules: the spec items have no `enabled` field, so the dependency rules are missing.
- The Create action is present (`type: action`, title "Create") but there is no save dialog or create job.
- Notes at line 2601 describe the rules; implement them in code, not the spec.

---

## 3. Storage info (in scope file `StorageInfo.kt`)

### 3.1 Visible elements
- Per storage root (one block per mounted external storage): header text with the absolute path (typography `header`), progress bar 12 dp tall, full width, rounded small corners, caption "Available: X / Total: Y" (`bodySmall`, secondary alpha) (`StorageInfo.kt:173-223`).
- Parent column with small spacing between blocks.
- Placement: Settings > Data, the "Storage usage" group (`SettingsDataScreen.kt:305`).

### 3.2 States
- Normal: bar shows used fraction (1 - available/total), text with both sizes.
- Unknown size (`-1` from DiskUtil, D14): Kotlin shows an unclamped bar and the caption built from the raw `-1` values (exact `Formatter` output for negative input is not asserted here; see T-22a-035 acceptance). Linux: hide the bar and show "Unknown size".
- No mounted roots: empty column (no empty-state text in Kotlin). Linux: show no rows (same).

### 3.3 Mockup gap
The mockup's storage row (`spec/settings.json` `settings_data`, notes at line 2504: "Storage usage row shows a progress bar of used/total disk space") has no live numbers. Implement the row in code (`src/storage_info.rs`, design §7.5).

---

## 4. Notifications (replacing Android notifications)

Android surface (behaviour §8). Linux equivalents:

| Android | Linux (GNotification / GTK) | Behaviour |
|---|---|---|
| Ongoing progress notification (LOW, determinate bar, "Cancel" action) | GNotification `restore-progress` (low priority) with body "{done} / {total}: {title}" and a "Cancel" button; plus in-window `gtk::ProgressBar` banner | Updated at most every 500 ms and at 100 percent |
| Completion (HIGH) "Restore completed" with "Tap to see details" | GNotification `restore-complete` (high priority) with body "Done in … with N error(s)" and button "Tap to see details" | Button opens the error log in the default text viewer |
| Completion without errors (HIGH) | GNotification `restore-complete` (normal priority); no button | |
| Failure "Restoring backup failed" + message | GNotification `restore-complete` (high) with the message as body | |
| Cancelled "Canceled restore" (under title "Restoring backup failed") | GNotification `restore-complete` (normal) | |

Notification texts:
- Plural: one error: "Done in {time} with 1 error"; many: "Done in {time} with {n} errors" (`restore_completed_message`).
- Time: "{mm} min, {ss} sec" with two-digit zero padding (`restore_duration`).

Desktop notification servers decide display (for example, do-not-disturb mode). Kurobayomi adds no suppression logic of its own.

---

## 5. Toasts and dialogs

| Kotlin toast | Linux presentation | Trigger |
|---|---|---|
| "Restore is already in progress" (`restore_in_progress`) | in-window banner, 4 s | Restore entry clicked while a job runs (`SettingsDataScreen.kt:248`) |
| "No file selected" (`file_null_uri_error`) | in-window banner, 4 s | Restore file dialog returns no file (not cancel) |
| "Backup is already in progress" (`backup_in_progress`) | in-window banner, 4 s | Create clicked while the manual create job runs |
| "No file picker app found" (`file_picker_error`) | `gtk::AlertDialog` (no picker is not possible on GTK; keep the message for parity but unreachable) | Create click without picker |
| "Backup/restore may not function properly if MIUI Optimization is disabled." | not shown (Android only) | MIUI check |

Dialogs introduced on Linux (PROPOSAL, no Kotlin equivalent):
- Close-while-running confirmation: `gtk::AlertDialog` with "Keep running" and "Cancel restore and close" (design §11).

---

## 6. Keyboard and mouse summary (desktop)

| Action | Keyboard | Mouse |
|---|---|---|
| Open restore | activate the "Restore backup" row (Enter/Space) | click the row or its "Restore backup" segment |
| Toggle option | Space on the focused checkbox | click label or box |
| Start restore | Enter on focused "Restore" button | click "Restore" |
| Cancel running restore | Tab to the banner's Cancel button and press Enter (PROPOSAL; no global shortcut) | click banner "Cancel" or the notification "Cancel" button |
| Open error log | activate the notification (desktop-dependent) or the "Tap to see details" button | click |
| Close page | Escape or Alt+Left (GTK header back) | click back arrow |

---

## 7. Responsive behaviour

- Min content width: the restore and create pages must remain usable at 360 px content width (PROPOSAL; check against the window minimum set in `src/main.rs`).
- Labels wrap (`set_wrap(true)`, `set_wrap_mode(pango::WrapMode::WordChar)`) so long source names do not widen the page.
- No fixed-width widgets; the action button is full width of the content column.
- Storage rows: the caption wraps before the progress bar shrinks below 120 px.

---

## 8. What the mockup lacks (summary, consolidated)

1. Restore page logic: validation, missing and invalid cards, checkbox defaults and enable rules, loading state.
2. File chooser entry point from the "Restore backup" action row (no `target` in `spec/settings.json` line 2412).
3. Progress banner and cancel.
4. Notifications (progress, completion, failure, cancel) and the error-log action.
5. Create page logic: enable rules, save dialog, "already running" message, create job wiring.
6. Live storage numbers (StorageInfo).
7. Checkbox controls (mockup uses switches).
8. Close-while-running confirmation (new, PROPOSAL).

Tasks covering each item are in p1-tasks.md (T-22a-001, 004, 005, 033, 034, 035, 044, 046).
