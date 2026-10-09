# Topic: notifications end to end (Aniyomi observed, Kurobayomi decided)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt`, `.../NotificationHandler.kt`, `.../NotificationReceiver.kt`, plus `AppUpdateNotifier.kt` (the updater's surfaces). Producers in other areas (library 26, downloads 23/24, backup 21/22, image save 34, extensions 18/19, server 13/53, incognito 01) are referenced, not specified here.

## 1. Lifecycle of one notification on Android

1. A producer (for example `MangaLibraryUpdateNotifier`) builds a notification with `context.notificationBuilder(Notifications.CHANNEL_LIBRARY_PROGRESS)`. The channel must exist; `createChannels` runs at app start (App.kt:240).
2. The builder gets actions. Each action wraps a `PendingIntent` built by a companion function of `NotificationReceiver` (for example `markAsViewedPendingBroadcast`, behaviour spec 1.6).
3. `Context.notify(id, notification)` posts through `NotificationManagerCompat` when POST_NOTIFICATIONS is granted (NotificationExtensions.kt:29-40).
4. The user taps an action. The PendingIntent is either a broadcast to `NotificationReceiver.onReceive` (the action set in section 2) or an activity start to `MainActivity` (shortcut actions, section 3).
5. The handler clears the notification (dismiss-then-act, behaviour 1.9.2) and performs the work in `launchIO` or `runBlocking`.
6. Progress updates replace the same id; the final update either replaces it or cancels it.

## 2. Action set (what Kurobayomi must support)

Grouped by purpose; each row = AppAction variant (p1-rust-design.md section 3.1) and its Android origin.

| Purpose | AppAction | Android origin | Notes |
|---|---|---|---|
| Open an entry | `OpenManga`, `OpenAnime` | `openMangaEntryPendingActivity`, `openAnimeEntryPendingActivity`, group variants | dismiss notice first |
| Open a chapter or episode | `OpenChapter`, `OpenEpisode` | `openChapterPendingActivity`, `openEpisodePendingActivity` | missing entity => toast "Error" |
| Mark read or seen | `MarkChaptersRead`, `MarkEpisodesSeen` | `markAsViewedPendingBroadcast` x2 | `removeAfterMarkedAsRead` deletes the download |
| Download from notice | `DownloadChapters`, `DownloadEpisodes` | `downloadChaptersPendingBroadcast`, `downloadEpisodesPendingBroadcast` | silent on missing rows |
| Queue control | `Resume*`, `Pause*`, `Clear*` (manga and anime) | six pending broadcast builders | no toast |
| Cancel long jobs | `CancelLibraryUpdate`, `CancelAnimeLibraryUpdate`, `CancelRestore` | `cancelLibraryUpdatePendingBroadcast`, `cancelAnimelibUpdatePendingBroadcast`, `cancelRestorePendingBroadcast` | job stop |
| App update | `StartAppUpdate`, `CancelAppUpdate` | `downloadAppUpdatePendingBroadcast`, `cancelDownloadAppUpdatePendingBroadcast` | URL validated |
| Reveal or open | `RevealFile`, `OpenUrl` | share intents, `openErrorLogPendingActivity`, `openUrl`, `openImagePendingActivity` | share sheet dropped |
| Dismiss | `Dismiss` | `dismissNotificationPendingBroadcast` | withdraw one id only |

Dropped or not carried over: APK install intent (`installApkPendingActivity`), share chooser (`shareImagePendingBroadcast`, `shareBackupPendingBroadcast`), the unused `ACTION_OPEN_ENTRY`, the group-summary dismissal logic, and the Android `cancelAll()` side effect (behaviour 1.5(a)).

## 3. Shortcut routing (activity intents)

`MainActivity.handleIntentAction` maps eleven shortcut actions to tabs (behaviour 1.7). In Kurobayomi these are not intents. Each becomes a navigation call inside `Router` (for example `OpenMangaDownloads` switches to More with downloads visible). The `notificationId` and `groupId` extras that MainActivity uses to dismiss a notice are replaced by the `notice: Option<NoticeId>` field carried by `OpenManga`/`OpenAnime`.

## 4. Replacement mechanism on Linux

- Transport: GAction `app.notice` with a string parameter. The notification's default action and buttons carry the JSON of an `AppAction` as their target (`gio::Notification::set_default_action_and_target_value`, `add_button_with_target_value`).
- Posting: `gtk::Application::send_notification(Some(id), &notification)`; removal: `withdraw_notification(id)`.
- Desktop server: GLib uses the xdg-desktop-portal Notification interface inside Flatpak and the freedesktop notification D-Bus service otherwise (PROPOSAL assumption; to verify on KDE in T-35a-002).
- Limitations: no progress bar, no ongoing flag, no per-channel settings, button count depends on the server, notices are delivered only while the app runs.

## 5. Design decisions (with reasons)

1. Categories replace channels. Channels exist to let users mute kinds of notice in the OS; GNOME and KDE do not expose per-app channels, so the app keeps its own switches (`notify.enabled.<category>`). Reason: the user keeps control without OS support.
2. Stable string ids replace integer ids. The Android registry (p1-data-and-config.md section 3) is kept as documentation; the collision at id 2 is removed.
3. Withdraw by id only. `cancelAll()` in the Android receiver removes foreground notices and every other notice; the port never calls a bulk withdraw.
4. Progress is text, throttled. GIO has no progress bar; the 200 ms and strictly-increasing rule from Android is kept so the desktop does not get flooded.
5. Actions carry data, not intents. Payloads are validated on arrival (URLs, paths, ids, size) because the session bus is shared with other processes of the same user.
6. Default action = open the thing the notice is about (chapter, entry, release page), not a destructive action. Android's update prompt started a download on body tap; Kurobayomi opens the release page (behaviour E-U4).
7. Fallback to in-app toast when no notification service exists, so no message is lost.

## 6. Threading

- Router and GAction callbacks run on the glib main context.
- Services run on tokio. Results return through `async_channel` as `UiEvent`s. No `block_on` on the main thread (Android's `runBlocking` in `openChapter`/`openEpisode` is not ported).
- `Notifier::show` and `withdraw` are callable from tokio; they marshal to the main context with `glib::MainContext::default().invoke`.

## 7. Tests that prove the topic

- Registry has no duplicate ids (`registry::ALL_IDS`).
- Action codec round trip for every variant; invalid payloads rejected (p1-edge-cases-and-tests.md section 5.4).
- `Notifier` fake records order: progress withdraw happens before error post (N5).
- Manual: KDE Plasma notification shows, body click routes, buttons route; with the notification server stopped, the toast appears (p1-edge-cases-and-tests.md section 7).

## 8. Open questions for the user (none block implementation)

- Whether Kurobayomi should ship a `.desktop` file with a desktop action so that notification clicks can start the app when it is closed. Default in this plan: no (parity: Android notifications also need the app process; the app is started by the user).
- Whether the update prompt should be shown as a notice at all when the app window is focused. Default in this plan: show it (parity).
