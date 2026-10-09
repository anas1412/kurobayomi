# Area 26 - Library update jobs: UI parity

Scope UI: the scope files render no Compose screen. They render Android notifications (`NotificationCompat`) and the foreground progress notification. The in-app surfaces that start and show these jobs live in files outside this area (`MangaLibraryTab.kt`, `AnimeLibraryTab.kt`, `MangaUpdatesScreenModel.kt`, `AnimeUpdatesScreenModel.kt`, `SettingsLibraryScreen.kt`, `SettingsAdvancedScreen.kt`). They are described here only where they trigger or reflect a scope behaviour. Source references are repo-relative.

---

## 1. Notifications (rendered by scope files)

### 1.1 Progress notification (`ID_LIBRARY_PROGRESS = -101`, channel `library_progress_channel`, LOW importance)

| Element | Visible content (exact) | State | Source |
|---|---|---|---|
| Title | "Updating library… (N%)" (`notification_updating_progress`); percentage = floor(current/total*100), `RoundingMode.DOWN`, 0 fraction digits | running; updated after each entry starts and finishes | `MangaLibraryUpdateNotifier.kt:95-115`, `AnimeLibraryUpdateNotifier.kt:96-116` |
| App name line | "Aniyomi" (`app_name`, untranslated) | constant | `MangaLibraryUpdateNotifier.kt:75` |
| Small icon | `ic_refresh_24dp` | constant | `:76` |
| Large icon | launcher bitmap | constant | `:67, 77` |
| Body (BigText) | each in-progress entry title, `chop(40)`, joined with `\n`; **omitted when `hideNotificationContent` is true** | depends on currently updating list | `:104-107` |
| Progress bar | determinate, max = queue size, current = completed count, indeterminate = false | | `:112` |
| Ongoing | yes (cannot be swiped) | | `:78` |
| Alert once | yes (`setOnlyAlertOnce(true)`), so updates do not beep | | `:79` |
| Action | "Cancel" (`action_cancel`) with icon `ic_close_24dp`; triggers `ACTION_CANCEL_LIBRARY_UPDATE` (manga) or `ACTION_CANCEL_ANIMELIB_UPDATE` (anime) -> `Job.stop` | | `:80-85`, `NotificationReceiver.kt:270-281` |
| Removal | cancelled when the pass ends (`cancelProgressNotification`) | | `:299-301` |

Shared identity: manga and anime passes reuse the same id and channel (`-101`), so the last pass to post owns the notification.

Foreground service type: `DATA_SYNC` on API 29+. Desktop equivalent: none needed for the notification itself.

### 1.2 Queue size warning (`ID_LIBRARY_SIZE_WARNING = -103`, same channel)

| Element | Text (exact) | Behaviour |
|---|---|---|
| Title | "Warning" (`label_warning`) | |
| Body | "Large updates harm sources and may lead to slower updates and also increased battery usage. Tap to learn more." (`notification_size_warning`) | BigText |
| Icon | `ic_warning_white_24dp` | |
| Timeout | 30 000 ms (auto-dismiss) | `setTimeoutAfter` |
| Tap | opens `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads` | `NotificationHandler.openUrl` |
| Trigger | any single source (excluding `UnmeteredSource`) has more than 60 entries in the queue | |
| Shown | at queue time, before updates start | |

Note: this notification is not suppressed by `hideNotificationContent`.

### 1.3 Summary notification (`ID_NEW_CHAPTERS = -301` manga, `ID_NEW_EPISODES = -1301` anime)

Channel `new_chapters_episodes_channel` (DEFAULT importance). Group `eu.kanade.tachiyomi.NEW_CHAPTERS` / `NEW_EPISODES`, group summary, alert behaviour `GROUP_ALERT_SUMMARY`, priority HIGH, auto-cancel, tap opens the main window with action `Constants.SHORTCUT_UPDATES`.

| Case | Title | Text | BigText |
|---|---|---|---|
| exactly 1 entry, content visible | "New chapters found" / "New episodes found" | entry title, `chop(45)` | none |
| 1 entry, content hidden | same title | "For 1 entry" / "For 1 title" (plural `notification_new_chapters_summary` / `notification_new_episodes_summary`) | none |
| 2+ entries, content visible | same title | "For N entries" / "For N titles" | each title `chop(45)`, joined `\n` |
| 2+ entries, content hidden | same title | "For N entries" / "For N titles" | none |

Plural rules: "one" = `For 1 entry`, "other" = `For %d entries`. (Anime: "For 1 title" / "For %d titles".)

### 1.4 Per-entry "new items" notification (id = entry id hash, no tag)

Only posted when `hideNotificationContent` is false. Channel `new_chapters_episodes_channel`, group as above, priority HIGH, auto-cancel, BigText.

| Element | Manga text | Anime text |
|---|---|---|
| Title | entry title (not chopped) | entry title |
| Body | description from `getNewChaptersDescription` | from `getNewEpisodesDescription` |
| Large icon | cover, circle-cropped, 192 px; falls back to no large icon if loading fails | same |
| Small icon | `ic_ani` | same |
| Tap | opens reader on `chapters.first()` (see behaviour P-8) | opens player on `episodes.first()` |

Body examples (exact formatting from strings; numbers via `formatChapterNumber`):

| Items | Body |
|---|---|
| 1 item, no parseable number | "1 new chapter" (plural generic) / "1 new episode" |
| 3 items, no parseable number | "3 new chapters" / "3 new episodes" |
| 1 parseable number 2.5, 1 item | "Chapter 2.5" / "Episode 2.5" |
| 1 parseable 2.5 plus 9 unparseable (10 items) | "Chapter 2.5 and 9 more" / "Episode 2.5 and 9 more" (count = `items.size - 1`) |
| numbers 1, 2.5, 3 | "Chapters 1, 2.5, 3" / "Episodes 1, 2.5, 3" |
| numbers 1, 2, 3, 4, 5, 6 | "Chapters 1, 2, 3, 4, 5 and 1 more" (plural one) / "... and 2 more" (other); anime "Episodes 1, 2, 3, 4, 5 and 1 more" |

Action buttons (in order; Android shows up to three; desktop shows all that the server supports):

| # | Label (manga / anime) | Key | Icon | Effect | Shown when |
|---|---|---|---|---|---|
| 1 | "Mark as read" / "Mark as watched" | `action_mark_as_read` / AYMR `action_mark_as_seen` | `ic_done_24dp` | marks the listed items read/seen; if `removeAfterMarkedAsRead` also deletes their downloads; dismisses the notification and the group if needed | always |
| 2 | "View chapters" / "View episodes" | `action_view_chapters` / AYMR `action_view_episodes` | `ic_book_24dp` | opens the entry screen (manga: `SHORTCUT_MANGA` with entry id; anime: the anime screen) | always |
| 3 | "Download" | `action_download` | `stat_sys_download_done` (framework icon) | enqueues the listed items (autoStart true) | only when item count <= 15 (manga) or <= 10 (anime) |

### 1.5 Error notification (`ID_LIBRARY_ERROR = -102`, channel `library_errors_channel`, LOW)

| Element | Text | Behaviour |
|---|---|---|
| Title | "N update(s) failed" (`notification_update_error`) | N = failure count |
| Text | "Tap to see details" (`action_show_errors`) | |
| Icon | `ic_ani` | |
| Tap | opens `aniyomi_update_errors.txt` in a text viewer (`ACTION_VIEW`, `text/plain`, read grant) | |
| Hidden content | not affected | always shown when failures exist |

The error file content is defined in behaviour section 7.

### 1.6 Notification progress end states

- Success with new items: summary + per-entry notifications remain until the user taps or dismisses them.
- Success, nothing new: progress notification removed; no other notification.
- Failure of the worker outside entries (`Result.failure`): progress removed; no failure notification (Kotlin does not post one for this case). Rust: same.
- Cancel: progress removed; no completion notification; partially processed entries keep their DB writes.

---

## 2. In-app surfaces that start or reflect the jobs (outside scope; behaviour documented for the port)

These files are outside this area, but the port must provide equivalent controls. Each entry gives the Kotlin control, the message shown, and the Rust requirement.

### 2.1 Library tab refresh (per category)

- Kotlin: `MangaLibraryTab.kt:105-109`, `AnimeLibraryTab.kt:106`. The refresh action passes the category. Result snackbar: "Updating category" (`updating_category`) when started; "An update is already running" (`update_already_running`) when not.
- Rust requirement: the library header refresh button (per category, or the global one) calls `Scheduler::start_now(kind, Some(category_id))`; shows toast with the same two strings; the button shows a spinner while a pass of that kind runs.

### 2.2 Updates tab refresh

- Kotlin: `MangaUpdatesScreenModel.kt:129`, `AnimeUpdatesScreenModel.kt:133`: starts a global pass; emits `LibraryUpdateTriggered(started)`.
- Rust: the Updates screen header button starts a global pass of the kind selected by the Anime/Manga switch. Same two toast texts.

### 2.3 Settings: Global update group (`SettingsLibraryScreen.kt`)

- Group title: "Global update" (`pref_category_library_update`).
- "Automatic updates" (`pref_library_update_interval`), list: "Off", "Every 12 hours", "Daily", "Every 2 days", "Every 3 days", "Weekly" (values 0, 12, 24, 48, 72, 168 hours). Changing it reschedules both kinds immediately.
- "Automatic updates device restrictions" (`pref_library_update_restriction`), multi-select, subtitle "Restrictions: %s" listing selected labels; entries "Only on Wi-Fi", "Only on unmetered network", "When charging". Disabled while interval is Off.
- Manga categories dialog: `TriStateListDialog` titled "Manga categories" (`AYMR manga_categories`), message `pref_manga_library_update_categories_details`. Included/excluded tri-state per category. Writes `library_update_categories` and `library_update_categories_exclude`.
- Anime categories: same dialog for `animelib_update_categories` (AYMR `anime_categories`).
- Entry restrictions (`library_update_manga_restriction`) and "Update season on library update" (`pref_update_season_on_library_update`) and "Automatic updates for metadata" (`auto_update_metadata`) appear in the same area of the screen (see `SettingsLibraryScreen.kt` after line 262; not all rows were read in this pass; the port must present all restriction tokens listed in `p1-data-and-config.md` 2.5).

Rust requirement: a settings page with the same grouping, same labels, same six interval choices, same device-restriction labels, and the tri-state category dialog. The mockup's `Dialog` struct (`src/spec.rs:59`) can host the tri-state dialog.

### 2.4 Settings: Advanced > Library (`SettingsAdvancedScreen.kt:310-330`)

- "Refresh library covers" (`pref_refresh_library_covers`) starts the four jobs in order: anime pass, manga pass, anime metadata, manga metadata. Each start is silent (no snackbar in the Kotlin code). Rust: the same button, silent like the Kotlin version (no toast). The four starts are independent: a metadata pass is refused only if a metadata pass is already running (shared lock, see p1-behavior 9.3 and pitfall P-3 fix in T-26a-051).

### 2.5 Notification channel settings

- Kotlin creates channels in `Notifications.createChannels` (see `Notifications.kt:137-196`). Users control channel importance in Android settings. Desktop: freedesktop notifications have no channels; the Rust settings page exposes a single toggle "Show library update notifications" (default on) and the `hideNotificationContent` setting (area:02-preferences-system) for content privacy.

### 2.6 Updates screen badge and last-updated text

- The Updates tab badge shows the count of new items since the user last opened the tab (`library_unread_updates_count` + `library_unseen_updates_count`, data doc 3). Mockup: `src/main.rs:175` shows a placeholder "8"; replace with the live count.
- "Library last updated" text: Kotlin reads `library_update_last_timestamp`; the mockup `src/screens.rs:282` shows "Library last updated: 5 minutes ago" as a static string. Rust: relative time with the thresholds "just now" (< 1 min), "N minutes ago", "N hours ago", "N days ago" (proposal; Kotlin uses the relative-time helper of the Updates screen, out of scope).

### 2.7 Worker info screen (debug)

- Kotlin: `settings_worker_info` (mockup `src/settings.rs:17`) dumps WorkManager state (Enqueued/Finished/Tags/State). Rust: dump of `library_job_run` rows (kind, trigger, category, state, started, finished, total, done, failed, new_items), newest first, last 50 rows. Keep the screen id `settings_worker_info` and the text-dump style.

---

## 3. Gestures, keyboard and mouse (desktop equivalents)

| Android gesture/control | Desktop equivalent |
|---|---|
| Tap notification | click on freedesktop notification body (default action `default`) |
| Notification action button | action button in notification (`notify-rust` actions) |
| Swipe to dismiss notification | notification close (server-dependent) |
| Long-press on library tab refresh (none in Kotlin) | none |
| Pull-to-refresh on library (not in scope; check area:08-domain-library-category-history-updates) | Ctrl+R on library screen triggers the current category refresh (proposal) |
| Keyboard shortcut for "Update library" | `Ctrl+Shift+U` (proposal; document in keybindings area) |

---

## 4. Responsive behaviour

- Notifications: no layout; server decides placement.
- Settings group and dialogs: the tri-state category dialog must scroll and fit at 720 px height (mockup `show_dialog` uses fixed sizes; use `scrolled_max` helper in `src/ui.rs:354`).
- Updates screen header button: disabled + spinner during a running pass; label "Updating…" under 360 px width, icon only (proposal).

---

## 5. Animation

- None in scope. The progress notification updates in place (no animation). The in-app progress uses a determinate `gtk::ProgressBar` on the library header while a pass runs (proposal; hidden when idle).

---

## 6. What the existing mockup lacks (checked against `src/`)

| Needed | Mockup status (checked) |
|---|---|
| Live last-updated time | static text `src/screens.rs:282` |
| Live update badge count | placeholder `"8"` in `src/main.rs:175` |
| Refresh button wired to a job | none; `updates()` in `src/screens.rs:277` has no button action |
| Progress UI (per-category spinner, header progress bar) | none |
| Toast/snackbar for "Updating category" / "An update is already running" | no toast helper in `src/ui.rs` (add one) |
| Desktop notifications | none (`notify-rust` not in `Cargo.toml`) |
| Worker info dump | static dump `src/settings.rs:17` |
| Settings rows for interval, restrictions, categories, season children, metadata | not verified in `spec/settings.json` in this pass; must be added with keys from `p1-data-and-config.md` |
| Error log opener | none |
| Confirmation before refresh-all-covers | Kotlin has none; do not add one |

---

## 7. Strings to add to the Rust resource table

Exact English strings (see `p1-behavior.md` section 13 for keys). Add all of these: "Updating library… (%s)" (with `%` substitution by the percent helper), "Large updates harm sources and may lead to slower updates and also increased battery usage. Tap to learn more.", "Warning", "New chapters found", "New episodes found", "For %d entry" / "For %d entries", "For %d title" / "For %d titles", "%d new chapter" / "%d new chapters", "%d new episode" / "%d new episodes", "Chapter %s", "Chapter %s and %d more", "Chapters %s", "Chapters %s and %d more", "Chapters %s and 1 more", "Episode %s", "Episode %s and %d more", "Episodes %s", "Episodes %s and %d more", "Mark as read", "Mark as watched", "View chapters", "View episodes", "Download", "Cancel", "%d update(s) failed", "Tap to see details", "No chapters found", "No episodes found", "Source not found", "Updating category", "An update is already running", "Automatic updates", "Off", "Every 12 hours", "Daily", "Every 2 days", "Every 3 days", "Weekly", "Automatic updates device restrictions", "Only on Wi-Fi", "Only on unmetered network", "When charging", "Restrictions: %s", "Global update", "Refresh library covers".
