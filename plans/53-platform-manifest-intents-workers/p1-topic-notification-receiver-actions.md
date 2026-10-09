# Topic: the notification receiver, end to end (Aniyomi observed, Kurobayomi decided)

Scope: `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt` (999 LOC, the dispatcher), the content and button builders in `NotificationHandler.kt` and the id registry `Notifications.kt`. The subsystem as a whole (what notices are posted, when, and their texts) is owned by `area:35-data-misc-updater-notification-export` (`p1-topic-notifications.md`). This topic covers the receiver mechanics: how a button press reaches code, how dismissal works, why the Android design has defects, and how the Linux dispatcher replaces each mechanism. Action-by-action behaviour is in `p1-behavior.md` section 4, and the value tables are in `p1-data-and-config.md` section 3.

## 1. The Android path of one button press (observed)

```
user taps "Mark as read" on a library notice
  │
  ├─ the notice's button holds a PendingIntent built by
  │    NotificationReceiver.markAsViewedPendingBroadcast(context, manga, chapters, groupId)  [NotificationReceiver.kt:717-736]
  │    intent: action = "<applicationId>.NotificationReceiver.MARK_AS_READ"
  │            extras: EXTRA_CHAPTER_URL (String[]), EXTRA_MANGA_ID (Long),
  │                    EXTRA_NOTIFICATION_ID (= manga.id.hashCode()), EXTRA_GROUP_ID (Int)
  │            PendingIntent: getBroadcast(context, requestCode = manga.id.hashCode(), FLAG_UPDATE_CURRENT | FLAG_IMMUTABLE)
  │
  ├─ the system delivers the broadcast to NotificationReceiver (manifest, exported=false) on the main thread
  │
  ├─ onReceive → MARK_AS_READ branch [:120-134]
  │     1. notificationId = EXTRA_NOTIFICATION_ID; if > -1: companion dismissNotification(context, id, groupId)  [:554-585]
  │     2. urls = EXTRA_CHAPTER_URL ?: return
  │     3. mangaId = EXTRA_MANGA_ID; if > -1: markAsRead(urls, mangaId)  [:298-319]
  │
  └─ markAsRead runs in launchIO (a coroutine on the IO dispatcher):
        chapters = urls.mapNotNull { getChapter.await(url, mangaId) }
        for each: copy(read = true); if removeAfterMarkedAsRead → delete downloads
        updateChapter.awaitAll(...)            → SQLite UPDATE chapters ... triggers version and last_modified_at
```

Three facts about this path drive the Linux design:

1. The button carries its payload in extras, but the `PendingIntent` identity is (request code, action, component, data). Extras are not part of the identity. Builders reuse request code `0` for many notices (section 3, defect E-05).
2. The receiver is process-internal (`exported=false`). Any broadcast is trusted, so the receiver does no validation: a missing extra throws (`!!`, defect E-04) or is ignored silently (`?: return`).
3. The work starts on the main thread, and two branches block on the database (`runBlocking`, defect E-03).

## 2. The Linux path of the same press (Proposal)

```
user clicks "Mark as read" on a desktop notice
  │
  ├─ the notice was posted with its button already carrying the action:
  │    gio::Notification::add_button_with_target_value("Mark as read", "app.notice", json(AppAction::MarkChaptersRead{...}))
  │
  ├─ the desktop server reports the activation to the application over D-Bus (GApplication action activation)
  │    (if the app is not running and the desktop entry is DBusActivatable, the application starts first)
  │
  ├─ app.notice handler on the GTK main thread:
  │     1. parse the JSON (size limit 64 KiB, then serde); a parse error is logged and the handler returns
  │     2. validate (kuro-core::notify::payload::validate): limits, schemes, ids, paths
  │     3. Dismiss → notifier.withdraw(one id)   |   other → spawn on tokio, result returns through glib
  │
  └─ tokio task: repository.mark_chapters_read(manga_id, urls) → one batch update in one transaction
        on success: notifier.withdraw(notice)     on failure: toast "Error" (AYMR download_error)
```

Differences from Android, with reasons:

| Aspect | Android | Linux | Reason |
|---|---|---|---|
| Identity of a button | request code plus action | the button's own target string | no identity collision (E-05) |
| Validation | none | size, schemes, ids, paths | the session bus is shared by all user processes |
| Thread | main thread with blocking reads | main thread for dispatch only, tokio for work | no freeze (E-03) |
| Withdraw | `cancelNotification` plus `cancelAll()` for dismiss | `withdraw_notification(id)` only | no collateral removal (E-01) |
| Failure | silent | toast | visible errors (E-16) |
| Batch | one `PendingIntent` per entry, URL list in extras | same URL list in the payload, capped at 500 | bounded work |
| Dead actions | `OPEN_CHAPTER`, `OPEN_EPISODE`, `OPEN_ENTRY` declared, not sent | not created | the enum has no dead variants (E-27) |

## 3. Why the Android design has defects (analysis)

- **Shared request codes.** A `PendingIntent` is cached by the system per (request code, intent filter). The second builder with the same action and request code updates the cached intent's extras (FLAG_UPDATE_CURRENT). When two update prompts use the same request code, the second overwrites the first. Affected builders and the defect IDs are in `p1-edge-cases-and-tests.md` (E-05).
- **Dismissal without ownership.** The app does not own the set of posted notices; it asks the platform for the active list and matches ids and group keys (`activeNotifications`, `NotificationReceiver.kt:569` to `:585`). The group rule then cancels the summary and leaves the child (E-06, device check pending). The Linux design owns its ids (`NoticeId`) and withdraws by id.
- **Id collisions across kinds.** Entry ids are `Long.hashCode()` values of manga and anime ids; a manga and an anime can share a value (E-31). Ids that are fixed constants share numbers across features (E-19: 2 for both the save-image notice and the app-update prompt).
- **Cross-feature side effect.** `ACTION_DISMISS_NOTIFICATION` calls `cancelAll()` (E-01). A user dismissing one update error removes the progress notice of a running backup.
- **Silent failures.** A missing extra, a missing entity and a failed database write produce no message (`context.toast` only for missing entities in `openChapter` and `openEpisode`, `:232` and `:252`).

## 4. Mapping of mechanisms (Android to Linux)

| Android mechanism | Where | Linux mechanism | Decision |
|---|---|---|---|
| `PendingIntent.getBroadcast` with request code | builders (20+) | GIO button with target string | replace |
| `BroadcastReceiver.onReceive` with action switch | `NotificationReceiver.kt:64-184` | GAction `app.notice` handler with `serde` enum match | replace |
| Extras `EXTRA_*` | constants `:412-417` | enum fields | replace |
| Notification id integers | `Notifications.kt` | `NoticeId` strings | replace (area:35 decision 2) |
| Channel per kind, user mute per channel | `createChannels` | categories with in-app switches | replace (area:35 decision 1) |
| `cancelAll()` | `:193` | none | drop (E-01) |
| `activeNotifications` lookup | `:569` | none (ids owned) | drop |
| `runBlocking` on receiver | `:224`, `:244` | tokio task | replace (E-03) |
| `PendingIntent.getActivity` with extras (MainActivity) | `:615` to `:709` | `LaunchRequest` through the same dispatcher | replace |
| `ACTION_VIEW` `PendingIntent` (open image, error log, URL) | `NotificationHandler.kt:58-93`, `:970-977` | `Opener` trait | replace |
| `ACTION_SEND` share chooser | `:202-214` | "Show in folder" and "Copy link" | replace (behaviour 12.1) |
| `ACTION_INSTALL_PACKAGE` PendingIntent | `NotificationHandler.kt:77-83` | none | drop (area:20) |
| Ongoing foreground notice for a service | `TorrentServerService.kt:94-127` | persistent notice while the server runs, withdrawn on stop | replace |
| `setDeleteIntent` restarting a service | `TorrentServerService.kt:120` | none | drop (E-08) |

## 5. Worked examples of the Linux payload (Proposal)

Serialised `AppAction` values as sent in the GAction target (serde tag `action`, snake case). Examples are the exact JSON shapes the dispatcher parses.

```json
{"action":"dismiss","notice":"notice:app-update-prompt"}
{"action":"mark_chapters_read","manga_id":7,"urls":["/series/7/ch-1","/series/7/ch-2"],"notice":"notice:entry:manga:7","group":"notice:new-chapters"}
{"action":"download_episodes","anime_id":12,"urls":["/anime/12/ep-4"],"notice":"notice:entry:anime:12","group":"notice:new-episodes"}
{"action":"start_app_update","url":"https://example.test/releases/kurobayomi-0.2.0.tar.zst","title":"Kurobayomi 0.2.0"}
{"action":"reveal_file","path":"/home/user/.cache/kurobayomi/update/kurobayomi-0.2.0.tar.zst"}
{"action":"disable_incognito"}
```

Rules shown by these examples: `notice` and `group` are optional and absent when not used; URLs are the source-relative URL strings stored in the database (not full links); the `title` in `start_app_update` is the release title (the Android extra `DOWNLOAD_TITLE` is carried and used, defect E-25); `reveal_file` paths must be under the Paths roots (test EN-05).

Invalid examples and their results (tests EN-02 to EN-07): a 501-URL `mark_chapters_read` returns `TooManyUrls(501)`; `start_app_update` with `http://` returns `BadScheme`; `reveal_file` with a relative path returns `NotAbsolute`; a payload with no `action` key is a serde error and is logged.

## 6. Dismissal semantics on Linux (Proposal)

- A notice is withdrawn when: its own button is pressed (for example "Dismiss"), its action succeeds (mark read, download started, update downloaded), or its underlying job finishes (library update finished, server stopped). The dispatcher calls `withdraw(id)` for exactly that id.
- A notice is never withdrawn as a side effect of another notice. There is no bulk withdraw in the `Notifier` trait (test TC-53-07).
- Group summaries are not used. Multiple new entries in one update batch are one notice whose body lists up to three titles and "and N more" (area:35 owns the text).

## 7. Concurrency

- Posting and withdrawing run on the GTK main thread (the `Notifier` implementation holds a `HashMap<NoticeId, PendingState>` so that a withdraw issued before a post is accepted cancels the post; test TC-53-31).
- Progress text updates are throttled by `ProgressThrottle` (200 ms, strictly increasing percent). Download progress is not a notice on Linux (area:35 decision 4); the throttle applies only to the app-update notice.
- Two buttons pressed quickly: both actions run (each is idempotent, test TC-53-28); the second sees the first's result in the database.

## 8. Open items

1. The device check for the Android group-summary behaviour (E-06) is not performed here; the Linux design does not depend on it.
2. Whether the GNOME Shell notification list groups notices by the `app` id is not verified; the design does not rely on grouping.
3. Button label localisation for notices comes from area:35 and area:51.
