# Area 21: UI parity (create side, settings rows, notifications)

Scope of this file: the Create Backup screen (its state model is in scope: `create/BackupOptions.kt`), the notifications produced by `BackupNotifier.kt`, and the "Backup and restore" settings group that starts create and schedules automatic backups (`SettingsDataScreen.kt`, `getBackupAndRestoreGroup`). The Restore screen and restore progress belong to area:22-backup-restore; they are described only where they share a row or a notification.

Kotlin sources (for every element below):
- `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt`
- `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (group built at `getBackupAndRestoreGroup`)
- `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt`

Mockup sources: `spec/settings.json` (screens 22, 23, 24, 27, 29), `src/settings.rs` (`body()` arms), `src/screens.rs`, `src/ui.rs`.

---

## 1. Settings: "Backup and restore" group

Location in Settings: Settings > Data (`SettingsDataScreen`). Group title `MR.strings.label_backup` = "Backup and restore". Order inside the group (top to bottom):

| # | Element | Kotlin | Text / value | State | Desktop equivalent (Kurobayomi) |
|---|---|---|---|---|---|
| 1 | Segmented two-button row, full width, height intrinsic min | `CustomPreference` + `MultiChoiceSegmentedButtonRow` | left segment "Create backup" (`MR.strings.pref_create_backup`), right "Restore backup" (`MR.strings.pref_restore_backup`) | Both always enabled. Restore is refused with toast `restore_in_progress` when restore is running | `gtk::Box` with `linked` style (`ui::linked` exists in the mockup) with two toggle buttons; no selection state (`checked=false` always) |
| 2 | "Create backup" action | `navigator.push(CreateBackupScreen())` | opens the Create screen (section 2) | | push screen via `ui::push` |
| 3 | "Restore backup" action | if no restore running: on MIUI-with-optimisation-off show toast `restore_miui_warning` (dropped on Linux); launch `GetContent("*/*")` chooser titled `file_select_backup` ("Select backup file"); on null toast `file_null_uri_error` ("No file selected") | opens restore flow | if restore running: toast `restore_in_progress` ("Restore is already in progress") | GTK `FileDialog::open` with filter `*.tachibk` and `*` (see section 5) |
| 4 | "Automatic backup frequency" list preference | `ListPreference(backupInterval)`; entries `0` "Off", `6` "Every 6 hours", `12` "Every 12 hours", `24` "Daily", `48` "Every 2 days", `168` "Weekly"; default 12 | Changing it calls `setupTask` at once | Value persisted in `backup_interval` (Kurobayomi: `backup.interval_hours`) | No libadwaita: a `gtk::ListBoxRow` with title, subtitle and a `gtk::DropDown` suffix |
| 5 | Info text | `InfoPreference(backup_info + "\n\n" + last_auto_backup_info(relativeTimeSpanString(lastAutoBackup)))` | "You should keep copies of backups in other places as well. Backups may contain sensitive data including any stored passwords; be careful if sharing." then blank line then "Last automatically backed up: <relative time>" | `lastAutoBackup` is observed live (`collectAsState`), so it updates after a run | `dim()` label; relative time formatted as in section 6 |

Visibility: the whole group is always shown.

Kurobayomi notes:
- The info text must also state the automatic-backup limitation that Kotlin does not state: automatic backups never include extensions or sensitive settings (B-01, E-27 area). Proposal text appended as a second paragraph: "Automatic backups do not include extension files or sensitive settings."
- The interval list must show exactly the six values; no custom value.

---

## 2. Create Backup screen

Title: `MR.strings.pref_create_backup` = "Create backup". Top bar with back arrow (`navigateUp = navigator::pop`), scroll behaviour collapse.

Body: `LazyColumnWithAction` (scrollable list with a pinned full-width action button at the bottom).

### 2.1 Layout (top to bottom)

| # | Element | Condition | Content |
|---|---|---|---|
| 1 | MIUI warning banner (`WarningBanner`) | `DeviceUtil.isMiui && isMiuiOptimizationDisabled()` | `restore_miui_warning` text. DROPPED on Linux (no MIUI) |
| 2 | Section card "Library" (`SectionCard(MR.strings.label_library)`) | always | six checkboxes (below) |
| 3 | Section card "Settings" (`label_settings`) | always | five checkboxes |
| 4 | Section card "Extensions" (`label_extensions`) | always | one checkbox |
| 5 | Pinned action button | always | label `MR.strings.action_create` = "Create"; enabled iff `canCreate()` |

Checkbox rows: `LabeledCheckbox(label, checked = getter(options), onCheckedChange = toggle(setter, it), enabled = enabled(options))`.

Library card, in order:
| Label (key) | Default | Enabled when | Notes |
|---|---|---|---|
| "Library entries" (`AYMR.strings.entries`) | on | always | master switch for entry data |
| "Chapters and episodes" (`AYMR.strings.chapters_episodes`) | on | Library entries on | |
| "Tracking" (`MR.strings.track`) | on | Library entries on | |
| "History" (`MR.strings.history`) | on | Library entries on | |
| "Categories" (`MR.strings.categories`) | on | always | not nested under entries |
| "All read/watched entries" (`AYMR.strings.non_library_settings`) | on | Library entries on | non-favourite read or watched entries |

Settings card, in order:
| Label (key) | Default | Enabled when |
|---|---|---|
| "App settings" (`MR.strings.app_settings`) | on | always |
| "Extension stores" (`MR.strings.extensionStores`) | on | always |
| "Custom buttons" (`AYMR.strings.custom_button_settings`) | on | always |
| "Source settings" (`MR.strings.source_settings`) | on | always |
| "Include sensitive settings (e.g., tracker login tokens)" (`MR.strings.private_settings`) | off | App settings on OR Source settings on |

Extensions card:
| "Extensions" (`MR.strings.label_extensions`) | off | always |

### 2.2 Behaviour

- Toggling a parent does not reset children (B-02). Children keep their value and are only disabled.
- The Create button is disabled unless at least one of: Library entries, Categories, App settings, Extension stores, Custom buttons, Source settings. (Chapters, tracking, history, read entries, sensitive settings and extensions alone do not enable it.)
- Click Create: if a manual backup is already running -> toast "Backup is already in progress". Else open the save dialog with suggested name `xyz.jmir.tachiyomi.mi_<yyyy-MM-dd_HH-mm>.tachibk` (Aniyomi) -> Kurobayomi suggests `kurobayomi_<yyyy-MM-dd_HH-mm-ss>.tachibk`.
- If the dialog cannot be opened -> toast "No file picker app found". (Kurobayomi: GTK always has the dialog; no equivalent toast. Dropped.)
- After a file is chosen: the job starts and the screen closes immediately (`navigator.pop()`). Progress is shown only through notifications.
- Cancel the dialog: nothing happens, the screen stays open with the same selection.
- Options are not remembered between visits (in-memory state). Kurobayomi keeps the same behaviour; persisting the last selection would be new behaviour and is not proposed.

### 2.3 Keyboard, mouse and accessibility (desktop)

| Action | Kurobayomi |
|---|---|
| Move focus through checkboxes | Tab / Shift+Tab; each checkbox is a `gtk::CheckButton` with its label as the accessible name |
| Toggle | Space (native check button behaviour) |
| Create | Activate the Create button (Enter when focused) or Ctrl+S (accelerator on the window while this screen is shown) |
| Cancel and go back | Escape or Alt+Left (`ui::pop`) |
| Disabled checkbox | `set_sensitive(false)` (not focusable), tooltip not needed |
| Disabled Create | `set_sensitive(false)` plus `set_tooltip_text("Select at least one library or settings option")` (new string, Proposal) |

### 2.4 Responsive behaviour

- Window width below 600 px: section cards stay full width; checkbox labels wrap; no horizontal scrolling.
- Height: list scrolls; the Create button stays pinned at the bottom.

### 2.5 What the mockup lacks (Create screen)

Mockup: `spec/settings.json` screen 23 `settings_create_backup` has the same six, five and one switches with the same defaults and labels, and a "Create" action. The notes describe the enable rules. Missing in the mockup:
1. The enable rules are not encoded: all switches are always enabled in the spec renderer (the spec has no `enabled_when`). Needed: `enabled_when` expressions for the Chapters/Tracking/History/Read-entries rows (Library entries) and for "Include sensitive settings" (App settings OR Source settings).
2. The Create button does not depend on `canCreate()`.
3. No save dialog, no job, no toast, no "already in progress" check.
4. Private-settings reset when both parents go off (E-27) is not modelled.
5. The MIUI banner is correctly absent on Linux (nothing to add).

---

## 3. Notifications (`BackupNotifier`)

Desktop mapping: freedesktop notifications (`notify-rust`). Android channels are replaced by urgency. Texts are the same English strings as Kotlin.

| Moment | Summary (title) | Body | Icon (freedesktop name) | Urgency | Behaviour | Actions |
|---|---|---|---|---|---|---|
| Manual or automatic run starts | "Creating backup" | (none) | `document-save` | low | persistent (timeout never), same id updated; indeterminate (no bar) | none |
| Manual run succeeds | "Backup created" | the file path (display path) | `document-save` | normal | persistent until dismissed; replaces the progress notification | "Show in folder" (reveal) |
| Manual run fails | "Backup failed" | reason text (may be empty) | `dialog-error` | normal | persistent | none |
| Automatic run succeeds | none (silent) | | | | the Settings info text updates | |
| Automatic run fails | after 3 consecutive failures only: "Automatic backup failed" | "Check the backup folder and free space." | `dialog-warning` | normal | persistent until dismissed | "Open settings" (opens the Backup settings page) |
| Restore progress (area:22) | "Restoring backup" / "Syncing library" | `content` unless `hide_notification_content` | `document-revert` | low | determinate bar | "Cancel" (cancel restore) |
| Restore error (area:22) | "Restoring backup failed" | reason | `dialog-error` | normal | persistent | none |
| Restore done (area:22) | "Restore completed" / "Library sync complete" | "Done in 01 min, 05 sec with 2 errors" | `emblem-ok-symbolic` | normal | persistent | "Tap to see details" (opens the error log) when errors > 0 |

Notes:
- Kotlin shows an error notification only for manual runs. Kurobayomi adds the automatic warning after 3 consecutive failures (E-10); this is a deliberate addition.
- Progress notification is removed on completion or failure (replaces Kotlin `cancelNotification(ID_BACKUP_PROGRESS)`).
- Backup texts are never hidden by `hide_notification_content`; restore body text is (Kotlin behaviour, B-15).

### 3.1 What the mockup lacks (notifications)

The mockup has no notification output at all. Needed: the `Notifier` trait implementation and the handful of calls above.

---

## 4. Schema debug screen (`settings_backup_schema`)

Kotlin: `app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt`. Vertical scroll, monospace text of the generated protobuf schema for `Backup.serializer().descriptor` (kotlinx `ProtoBufSchemaGenerator`). Copy action copies the whole text.

Mockup: `spec/settings.json` screen 29 (`Backup file schema`) and `src/settings.rs` arm `"settings_backup_schema"` which shows a three-field hard-coded text (`message Backup { backupManga = 1; backupCategories = 2; backupAnime = 3; }`). This is WRONG for the current format (field 3 is not in the current `Backup`; 501 is the anime field). Kurobayomi (T-21a-035): generate the text from the Rust field tables (`schema_text::TEXT`) and show all 13 fields of `Backup` with the correct numbers, plus the message list. The screen is a P3 debug aid.

---

## 5. File dialogs

| Dialog | Kotlin | Kurobayomi (GTK 4.16) |
|---|---|---|
| Save backup | `CreateDocument("application/*")`, name suggested | `gtk::FileDialog::save` with `set_initial_name(Some(name))`, filter `*.tachibk` (added but not forced); result path appended with `.tachibk` if missing |
| Pick backup to restore | `GetContent("*/*")` with chooser title | `gtk::FileDialog::open`, filter `*.tachibk` first, then "All files" |
| Cancel | nothing | nothing |

Under the xdg desktop portal (KDE with portal enabled) the GTK dialog delegates to the native KDE file dialog; the same calls work.

---

## 6. Relative time string

`relativeTimeSpanString(lastAutoBackup)` (Kotlin, Android). Kurobayomi: shared helper (area:01/02 utilities) with these buckets (Proposal, same as Android `DateUtils.getRelativeTimeSpanString` defaults): under 1 minute "Just now"; under 1 hour "N minutes ago"; under 1 day "N hours ago"; otherwise a date `yyyy-MM-dd HH:mm` in local time. When the value is 0 (never): Kotlin renders the epoch relative time (about "54 years ago"); PROPOSAL: show "Never" instead (new string, deliberate).

---

## 7. Mockup gaps summary (backup UI)

1. Settings rows: the segmented create/restore row, the interval dropdown and the info text with live "last backed up" are not wired (the mockup has static text "Last automatically backed up" in notes only; `spec/settings.json` lists the interval options and the info string).
2. Create screen: no enable rules, no Create gating, no save dialog, no job, no toasts (section 2.5).
3. Notifications: none (section 3.1).
4. Schema screen: wrong content (section 4).
5. Restore-side rows and screens: area:22.
6. No persisted interval or last-run timestamp.
7. The info text lacks the Kurobayomi automatic-backup limitation (section 1).
