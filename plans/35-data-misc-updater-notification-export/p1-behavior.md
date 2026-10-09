# Area 35 - behaviour specification

OBSERVED = what the Kotlin does (cited path:line). PROPOSAL = what Kurobayomi should do. Every path is repo-relative to `/home/blackbox/aniyomi-src`. Area references use `area:<id>`.

Sections: 1 notifications; 2 app updater; 3 library CSV export; 4 Android XML resources.

---

## 1. Notifications

### 1.1 Channel and group creation (OBSERVED)

Entry: `App.kt:240` calls `Notifications.createChannels(this)` from `setupNotificationChannels()`, wrapped in `try { } catch (e: Exception) { logcat(LogPriority.ERROR, e) { "Failed to modify notification channels" } }`.

`Notifications.createChannels(context)` (`app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt:114-197`) runs in this order:

1. Delete the 9 deprecated channel ids (`Notifications.kt:96-106`, in list order): `downloader_channel`, `downloader_complete_channel`, `backup_restore_complete_channel`, `library_channel`, `library_progress_channel`, `updates_ext_channel`, `downloader_cache_renewal`, `crash_logs_channel`, `library_skipped_channel`.
2. Create 4 channel groups (`Notifications.kt:120-135`):

| Group id | Name (MR key, English value) | Used by channels |
|---|---|---|
| `group_backup_restore` | `MR.strings.label_backup` = "Backup and restore" | backup progress, backup complete |
| `group_downloader` | `MR.strings.download_notifier_downloader_title` = "Downloader" | downloader progress, downloader errors |
| `group_library` | `MR.strings.label_library` = "Library" | library progress, library errors |
| `group_apk_updates` | `MR.strings.label_recent_updates` = "Updates" | app updates, extension updates |

3. Create 13 channels (`Notifications.kt:137-196`) with importance, group, badge and sound flags as in section 1.2.

Channel creation is idempotent; the platform keeps user changes to importance. Kurobayomi does not use channels (see section 1.2 and T-35a-012).

### 1.2 Channel table (OBSERVED, with every attribute)

| Channel id | Importance (`NotificationManagerCompat.IMPORTANCE_*`) | Name (key = English) | Group | Badge | Sound | Source line |
|---|---|---|---|---|---|---|
| `common_channel` | LOW | `MR.strings.channel_common` = "Common" | none | default | default | Notifications.kt:139-141 |
| `library_progress_channel` | LOW | `MR.strings.channel_progress` = "Progress" | `group_library` | `setShowBadge(false)` | default | Notifications.kt:142-146 |
| `library_errors_channel` | LOW | `MR.strings.channel_errors` = "Errors" | `group_library` | false | default | Notifications.kt:147-151 |
| `new_chapters_episodes_channel` | DEFAULT | `AYMR.strings.channel_new_chapters_episodes` = "Chapter/Episode updates" | none | default | default | Notifications.kt:152-154 |
| `downloader_progress_channel` | LOW | `MR.strings.channel_progress` = "Progress" | `group_downloader` | false | default | Notifications.kt:155-159 |
| `downloader_error_channel` | LOW | `MR.strings.channel_errors` = "Errors" | `group_downloader` | false | default | Notifications.kt:160-164 |
| `backup_restore_progress_channel` | LOW | `MR.strings.channel_progress` = "Progress" | `group_backup_restore` | false | default | Notifications.kt:165-169 |
| `backup_restore_complete_channel_v2` | HIGH | `MR.strings.channel_complete` = "Complete" | `group_backup_restore` | false | `setSound(null, null)` (silent) | Notifications.kt:170-175 |
| `incognito_mode_channel` | LOW | `MR.strings.pref_incognito_mode` = "Incognito mode" | none | default | default | Notifications.kt:176-178 |
| `torrent_server_channel` | LOW | `AYMR.strings.pref_category_torrentserver` = "Torrent Server Preferences" | none | false | default | Notifications.kt:179-182 |
| `http_server_channel` | LOW | `AYMR.strings.pref_http_server_name` = "Http server" | none | false | default | Notifications.kt:183-186 |
| `app_apk_update_channel` | DEFAULT | `MR.strings.channel_app_updates` = "App updates" | `group_apk_updates` | default | default | Notifications.kt:187-190 |
| `ext_apk_update_channel` | DEFAULT | `MR.strings.channel_ext_updates` = "Extension updates" | `group_apk_updates` | default | default | Notifications.kt:191-194 |

Note: `CHANNEL_NEW_CHAPTERS_EPISODES` has no group in its channel definition; the library notifiers set `setGroup(GROUP_NEW_CHAPTERS)` / `setGroup(GROUP_NEW_EPISODES)` on each notification (outside this scope, area:26-library-update-jobs).

### 1.3 Shared builder behaviour (OBSERVED)

- `Context.notificationBuilder(channelId, block)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt:66-76`) = `NotificationCompat.Builder(this, channelId).setColor(getColor(R.color.accent_blue))`, then the block.
- `Context.notify(id, notification)` (`NotificationExtensions.kt:29-40`): on API 33+ (TIRAMISU) returns silently when `POST_NOTIFICATIONS` is not granted; otherwise `NotificationManagerCompat.from(this).notify(id, notification)`.
- `Context.cancelNotification(id)` (`NotificationExtensions.kt:55-57`) = `NotificationManagerCompat.from(this).cancel(id)`.
- `Context.notificationManager` (`NotificationExtensions.kt:17-18`) = system `NotificationManager`.

PROPOSAL: all notification posting goes through `kuro-platform::Notifier::show`, which enforces the same guard by asking the desktop notification service instead of a permission (no runtime permission on Linux). If the service is unavailable, `show` returns `Err(NotifyError::Unavailable)` and the caller logs at warn level and continues (no toast).

### 1.4 Id registry (OBSERVED; full table in p1-data-and-config.md section 3)

Ids are integers. Positive ids are used by this scope's updater (1, 2, 3) and the image save notification (2). Negative ids are library/downloader/backup/incognito/server/extension. The registry has one collision: `ID_DOWNLOAD_IMAGE = 2` (`Notifications.kt:23`, used by `ui/reader/SaveImageNotifier.kt:28`) and `ID_APP_UPDATE_PROMPT = 2` (`Notifications.kt:90`, used by `AppUpdateNotifier.kt:300`). Both post to the same id; whichever is posted last replaces the other. PROPOSAL: give the save-image notice its own id (T-35a-005, T-35a-086).

### 1.5 Dismissal and cancel (OBSERVED, three different code paths)

There are three distinct behaviours that the Kotlin conflates by name:

**(a) Instance method `NotificationReceiver.dismissNotification(context, notificationId)`** (`NotificationReceiver.kt:191-194`, `private`):
```
context.cancelNotification(notificationId)
context.notificationManager.cancelAll()
```
It cancels the id and then cancels EVERY notification posted by the app (`cancelAll()`). It is reached from `onReceive` for `ACTION_DISMISS_NOTIFICATION` (`NotificationReceiver.kt:66-70`): a call with two arguments inside the class resolves to the member, not to the companion overload (member scope has priority over companion scope). Effect: tapping "Cancel" on the install prompt (`AppUpdateNotifier.kt:294-297`, id 2) removes the library, download, backup and foreground service notifications too. This is an OBSERVED defect; PROPOSAL: never call `cancelAll()` (T-35a-007).

**(b) Companion `NotificationReceiver.dismissNotification(context, notificationId, groupId: Int? = null)`** (`NotificationReceiver.kt:554-585`, `internal`). Algorithm:
1. `groupKey = activeNotifications.find { it.id == notificationId }?.groupKey`.
2. If `groupId != null && groupId != 0 && !groupKey.isNullOrEmpty()`:
   a. `notifications = activeNotifications.filter { it.groupKey == groupKey }`.
   b. If `notifications.size == 2`: `cancelNotification(groupId)` and `return` (the single notification `notificationId` is NOT cancelled in this branch). OBSERVED; looks unintended (the comment at lines 559-568 says the group summary should be dismissed when the last child goes). PROPOSAL: cancel both (T-35a-026 documents the drop).
3. Otherwise `cancelNotification(notificationId)`.

Callers of (b): `NotificationReceiver.onReceive` for MARK_AS_READ/MARK_AS_SEEN/DOWNLOAD_CHAPTER/DOWNLOAD_EPISODE (groupId from extra, default 0), `MainActivity.kt:464` (`notificationId` and `groupId` extras), library notifiers (`cancelNotification` directly for progress ids).

**(c) `Context.cancelNotification(id)`** (NotificationExtensions.kt:55): plain cancel.

External callers of (b) with two arguments: `AppUpdateNotifier.cancel()` at `AppUpdateNotifier.kt:189-191` calls `NotificationReceiver.dismissNotification(context, Notifications.ID_APP_UPDATER)`. Outside the class the companion overload is the only candidate, `groupId` defaults to null, so this is a plain cancel (c). Confirmed by reading the scope, no `cancelAll()` in that path.

### 1.6 PendingIntent builders (OBSERVED)

Every builder uses `PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE` unless stated. "rc" is the request code.

| Builder (file:line) | Component | Action / extras | rc | Flags beyond defaults |
|---|---|---|---|---|
| `NotificationHandler.openDownloadManagerPendingActivity` (NotificationHandler.kt:117-128) | `MainActivity` | action `Constants.SHORTCUT_DOWNLOADS`; flags `FLAG_ACTIVITY_CLEAR_TOP or FLAG_ACTIVITY_SINGLE_TOP` | 0 | - |
| `NotificationHandler.openAnimeDownloadManagerPendingActivity` (135-146) | `MainActivity` | action `SHORTCUT_ANIME_DOWNLOADS`; `FLAG_ACTIVITY_NEW_TASK or FLAG_ACTIVITY_REORDER_TO_FRONT` | 0 | - |
| `NotificationHandler.openImagePendingActivity(context, uri)` (154-165) | system chooser for `ACTION_VIEW` | data `uri`, type `image/*`; `FLAG_ACTIVITY_NEW_TASK or FLAG_GRANT_READ_URI_PERMISSION` | 0 | - |
| `NotificationHandler.installApkPendingActivity(context, uri)` (173-179, public) | `ACTION_VIEW` | type `application/vnd.android.package-archive` (`MangaExtensionInstaller.APK_MIME`, MangaExtensionInstaller.kt:323); `NEW_TASK or GRANT_READ_URI_PERMISSION` | 0 | only `FLAG_IMMUTABLE` (no UPDATE_CURRENT) |
| `NotificationHandler.openUrl(context, url)` (181-189, public) | `ACTION_VIEW` | data `url.toUri()` | 0 | only `FLAG_IMMUTABLE` |
| `NotificationReceiver.resumeDownloadsPendingBroadcast` (425-435) | Broadcast | `ACTION_RESUME_DOWNLOADS` | 0 | - |
| `pauseDownloadsPendingBroadcast` (443-453) | Broadcast | `ACTION_PAUSE_DOWNLOADS` | 0 | - |
| `clearDownloadsPendingBroadcast` (461-471) | Broadcast | `ACTION_CLEAR_DOWNLOADS` | 0 | - |
| `resumeAnimeDownloadsPendingBroadcast` (479-489) | Broadcast | `ACTION_RESUME_ANIME_DOWNLOADS` | 0 | - |
| `pauseAnimeDownloadsPendingBroadcast` (497-507) | Broadcast | `ACTION_PAUSE_ANIME_DOWNLOADS` | 0 | - |
| `clearAnimeDownloadsPendingBroadcast` (515-525) | Broadcast | `ACTION_CLEAR_ANIME_DOWNLOADS` | 0 | - |
| `dismissNotificationPendingBroadcast(context, id)` (534-545) | Broadcast | `ACTION_DISMISS_NOTIFICATION`; extra `EXTRA_NOTIFICATION_ID` | 0 | - |
| `shareImagePendingBroadcast(context, uri)` (595-606) | Broadcast | `ACTION_SHARE_IMAGE`; extra `EXTRA_URI` = `uri.toString()` (String) | 0 | - (no notification id extra, despite the KDoc) |
| `openEpisodePendingActivity(context, anime, episode)` (615-623) | `PlayerActivity` via `PlayerActivity.newIntent(context, anime.id, episode.id)` | - | `anime.id.hashCode()` | - |
| `openEpisodePendingActivity(context, anime, groupId: Int)` (631-644) | `MainActivity` | action `SHORTCUT_ANIME`, `addFlags(CLEAR_TOP)`, extras `ANIME_EXTRA`=anime.id (Long), `"notificationId"`=`anime.id.hashCode()` (Int), `"groupId"`=groupId | `anime.id.hashCode()` | - |
| `markAsViewedPendingBroadcast(context, anime, episodes: Array<Episode>, groupId)` (652-671) | Broadcast | `ACTION_MARK_AS_SEEN`; `EXTRA_CHAPTER_URL` = `episodes.map { it.url }` as `Array<String>`; `EXTRA_MANGA_ID` = anime.id (note: name says MANGA but holds the anime id); `EXTRA_NOTIFICATION_ID` = `anime.id.hashCode()`; `EXTRA_GROUP_ID` = groupId | `anime.id.hashCode()` | - |
| `openChapterPendingActivity(context, manga, chapter)` (680-688) | `ReaderActivity` via `ReaderActivity.newIntent(context, manga.id, chapter.id)` | - | `manga.id.hashCode()` | - |
| `openChapterPendingActivity(context, manga, groupId: Int)` (696-709) | `MainActivity` | action `SHORTCUT_MANGA`, `CLEAR_TOP`, `MANGA_EXTRA`=manga.id, `"notificationId"`, `"groupId"` | `manga.id.hashCode()` | - |
| `markAsViewedPendingBroadcast(context, manga, chapters: Array<Chapter>, groupId)` (717-736) | Broadcast | `ACTION_MARK_AS_READ`; `EXTRA_CHAPTER_URL`; `EXTRA_MANGA_ID`; `EXTRA_NOTIFICATION_ID`; `EXTRA_GROUP_ID` | `manga.id.hashCode()` | - |
| `downloadChaptersPendingBroadcast(context, manga, chapters, groupId)` (744-763) | Broadcast | `ACTION_DOWNLOAD_CHAPTER`; same extras as above | `manga.id.hashCode()` | - |
| `downloadEpisodesPendingBroadcast(context, anime, episodes, groupId)` (771-790) | Broadcast | `ACTION_DOWNLOAD_EPISODE`; same extras | `anime.id.hashCode()` | - |
| `openMangaEntryPendingActivity(context, mangaId: Long)` (798-809) | `MainActivity` | action `SHORTCUT_MANGA`, `CLEAR_TOP`, `MANGA_EXTRA`, `"notificationId"`=`mangaId.hashCode()` | `mangaId.hashCode()` | - |
| `openAnimeEntryPendingActivity(context, animeId: Long)` (817-828) | `MainActivity` | action `SHORTCUT_ANIME`, `CLEAR_TOP`, `ANIME_EXTRA`, `"notificationId"` | `animeId.hashCode()` | - |
| `cancelLibraryUpdatePendingBroadcast(context)` (836-846) | Broadcast | `ACTION_CANCEL_LIBRARY_UPDATE` | 0 | - |
| `cancelAnimelibUpdatePendingBroadcast(context)` (854-864) | Broadcast | `ACTION_CANCEL_ANIMELIB_UPDATE` | 0 | - |
| `downloadAppUpdatePendingBroadcast(context, url, title: String? = null)` (872-888) | Broadcast | `ACTION_START_APP_UPDATE`; extra `AppUpdateDownloadJob.EXTRA_DOWNLOAD_URL`; optional `EXTRA_DOWNLOAD_TITLE` | 0 | - |
| `cancelDownloadAppUpdatePendingBroadcast(context)` (893-903) | Broadcast | `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` | 0 | - |
| `openExtensionsPendingActivity(context)` (911-922) | `MainActivity` | action `SHORTCUT_EXTENSIONS`, `CLEAR_TOP` | 0 | - |
| `openAnimeExtensionsPendingActivity(context)` (930-941) | `MainActivity` | action `SHORTCUT_ANIMEEXTENSIONS`, `CLEAR_TOP` | 0 | - |
| `shareBackupPendingBroadcast(context, uri)` (950-961) | Broadcast | `ACTION_SHARE_BACKUP`; extra `EXTRA_URI` = `uri` (Parcelable `Uri`) | 0 | - |
| `openErrorLogPendingActivity(context, uri)` (970-977) | `ACTION_VIEW` | type `text/plain`; `NEW_TASK or GRANT_READ_URI_PERMISSION`; only `FLAG_IMMUTABLE` | 0 | - |
| `cancelRestorePendingBroadcast(context, notificationId)` (986-997) | Broadcast | `ACTION_CANCEL_RESTORE`; extra `EXTRA_NOTIFICATION_ID` | 0 | - |

Request-code collisions (OBSERVED): all rc=0 builders for different actions share one request code; Android distinguishes PendingIntents by intent `filterEquals` (action + data + type + component), so distinct actions do not collide. Builders keyed on `x.id.hashCode()` for the same entry (for example `openChapterPendingActivity` and `downloadChaptersPendingBroadcast` for one manga) share rc `manga.id.hashCode()` but differ in action, so they also do not collide. Kurobayomi does not use request codes (section 1.7).

### 1.7 Activity shortcut routing (OBSERVED, MainActivity.kt:461-500)

`MainActivity.handleIntentAction(intent, navigator)`:
1. If `intent.getIntExtra("notificationId", -1) > -1` then `NotificationReceiver.dismissNotification(applicationContext, notificationId, intent.getIntExtra("groupId", 0))` (companion overload (b)).
2. Route on `intent.action`:

| Action constant (value in Constants.kt) | Destination | Extra consumed | Side effect |
|---|---|---|---|
| `SHORTCUT_ANIMELIB` = "eu.kanade.tachiyomi.SHOW_ANIMELIB" | `HomeScreen.Tab.AnimeLib()` | - | - |
| `SHORTCUT_LIBRARY` = "eu.kanade.tachiyomi.SHOW_LIBRARY" | `HomeScreen.Tab.Library()` | - | - |
| `SHORTCUT_MANGA` = "eu.kanade.tachiyomi.SHOW_MANGA" | `HomeScreen.Tab.Library(idToOpen)` | `MANGA_EXTRA` (Long) or return false | `navigator.popUntilRoot()` |
| `SHORTCUT_ANIME` = "eu.kanade.tachiyomi.SHOW_ANIME" | `HomeScreen.Tab.AnimeLib(idToOpen)` | `ANIME_EXTRA` (Long) or return false | `popUntilRoot()` |
| `SHORTCUT_UPDATES` = "eu.kanade.tachiyomi.SHOW_RECENTLY_UPDATED" | `HomeScreen.Tab.Updates` | - | - |
| `SHORTCUT_HISTORY` = "eu.kanade.tachiyomi.SHOW_RECENTLY_READ" | `HomeScreen.Tab.History` | - | - |
| `SHORTCUT_SOURCES` = "eu.kanade.tachiyomi.SHOW_CATALOGUES" | `HomeScreen.Tab.Browse(false)` | - | - |
| `SHORTCUT_ANIMEEXTENSIONS` = "eu.kanade.tachiyomi.ANIMEEXTENSIONS" | `HomeScreen.Tab.Browse(true, true)` | - | - |
| `SHORTCUT_EXTENSIONS` = "eu.kanade.tachiyomi.EXTENSIONS" | `HomeScreen.Tab.Browse(true)` | - | - |
| `SHORTCUT_DOWNLOADS` = "eu.kanade.tachiyomi.SHOW_DOWNLOADS" | `HomeScreen.Tab.More(toDownloads = true)` | - | `popUntilRoot()` |
| `SHORTCUT_ANIME_DOWNLOADS` = "eu.kanade.tachiyomi.SHOW_ANIME_DOWNLOADS" | `HomeScreen.Tab.More(toDownloads = true)` | - | `popUntilRoot()` |

Constants used for `MANGA_EXTRA` = "manga" and `ANIME_EXTRA` = "anime" (core/common Constants.kt:7,9).

Note: `SHORTCUT_MANGA`/`SHORTCUT_ANIME` use `intent.extras?.getLong(...) ?: return false`. The `?:` only fires when the intent has no extras bundle at all; a bundle without the key yields `0L` (Bundle default), so the tab opens with id 0 (OBSERVED quirk, MainActivity.kt:474-481).

### 1.8 Receiver dispatch (OBSERVED, NotificationReceiver.kt:64-184)

`onReceive(context, intent)` switches on `intent.action`; unknown actions are ignored. Runs on the main thread (BroadcastReceiver). Table of all 22 action constants:

| # | Constant (value suffix after `"$ID.NotificationReceiver."`) | Handler | Preconditions and missing-extra behaviour | Section |
|---|---|---|---|---|
| 1 | `SHARE_IMAGE` | `shareImage(context, Uri)` | `EXTRA_URI` string `!!` (crash if absent) | 1.9.6 |
| 2 | `SEND_BACKUP` (constant name `ACTION_SHARE_BACKUP`) | `shareFile(context, uri, "application/x-protobuf+gzip")` | `getParcelableExtraCompat(EXTRA_URI)!!` (crash if absent) | 1.9.6 |
| 3 | `CANCEL_RESTORE` | `cancelRestore` -> `BackupRestoreJob.stop(context)` | none | 1.9.8 |
| 4 | `CANCEL_LIBRARY_UPDATE` | `cancelLibraryUpdate` -> `MangaLibraryUpdateJob.stop(context)` | none | 1.9.8 |
| 5 | `CANCEL_ANIMELIB_UPDATE` | `cancelAnimelibUpdate` -> `AnimeLibraryUpdateJob.stop(context)` | none | 1.9.8 |
| 6 | `ACTION_START_APP_UPDATE` | `startDownloadAppUpdate` | `EXTRA_DOWNLOAD_URL` missing -> return (no-op); `EXTRA_DOWNLOAD_TITLE` is ignored | 2.9 |
| 7 | `CANCEL_APP_UPDATE_DOWNLOAD` | `cancelDownloadAppUpdate` -> `AppUpdateDownloadJob.stop` | none | 2.9 |
| 8 | `ACTION_OPEN_CHAPTER` | `openChapter(context, mangaId, chapterId)` | ids default -1 | 1.9.4 |
| 9 | `ACTION_OPEN_EPISODE` | `openEpisode(context, animeId, episodeId)` | ids default -1 | 1.9.4 |
| 10 | `MARK_AS_READ` | `markAsRead(urls, mangaId)` after dismiss | `EXTRA_CHAPTER_URL` null -> `return` (no-op, dismiss already done); `mangaId > -1` required | 1.9.5 |
| 11 | `MARK_AS_SEEN` | `markAsSeen(urls, animeId)` after dismiss | same as 10 | 1.9.5 |
| 12 | `ACTION_DOWNLOAD_CHAPTER` | `downloadChapters(urls, mangaId)` after dismiss | same | 1.9.7 |
| 13 | `ACTION_DOWNLOAD_EPISODE` | `downloadEpisodes(urls, animeId)` after dismiss | same | 1.9.7 |
| 14 | `ACTION_RESUME_DOWNLOADS` | `mangaDownloadManager.startDownloads()` | - | 1.9.8 |
| 15 | `ACTION_PAUSE_DOWNLOADS` | `mangaDownloadManager.pauseDownloads()` | - | 1.9.8 |
| 16 | `ACTION_CLEAR_DOWNLOADS` | `mangaDownloadManager.clearQueue()` | - | 1.9.8 |
| 17 | `ACTION_RESUME_ANIME_DOWNLOADS` | `animeDownloadManager.startDownloads()` | - | 1.9.8 |
| 18 | `ACTION_PAUSE_ANIME_DOWNLOADS` | `animeDownloadManager.pauseDownloads()` | - | 1.9.8 |
| 19 | `ACTION_CLEAR_ANIME_DOWNLOADS` | `animeDownloadManager.clearQueue()` | - | 1.9.8 |
| 20 | `ACTION_DISMISS_NOTIFICATION` | private `dismissNotification(context, id)` = cancel id + `cancelAll()` | `EXTRA_NOTIFICATION_ID` default -1 | 1.5(a) |
| 21 | `ACTION_OPEN_ENTRY` | NONE: constant declared at NotificationReceiver.kt:400, never referenced by any builder or by onReceive | - | 1.9.9 |
| - | (no others) | | | |

Count check: the `when` has 19 handled branches (SHARE_IMAGE, SHARE_BACKUP, CANCEL_RESTORE, CANCEL_LIBRARY_UPDATE, CANCEL_ANIMELIB_UPDATE, START_APP_UPDATE, CANCEL_APP_UPDATE_DOWNLOAD, OPEN_CHAPTER, OPEN_EPISODE, MARK_AS_READ, MARK_AS_SEEN, DOWNLOAD_CHAPTER, DOWNLOAD_EPISODE, RESUME/PAUSE/CLEAR x2 = 6, DISMISS_NOTIFICATION = 20 handled cases; `ACTION_OPEN_ENTRY` is the 21st constant and is unhandled). In total 22 `ACTION_` constants are declared (`NotificationReceiver.kt:381-410`).

### 1.9 Handler algorithms (OBSERVED)

**1.9.1 Argument extras.** Strings: `EXTRA_CHAPTER_URL` is `Array<String>`; `EXTRA_URI` is String (share image) or Parcelable (share backup). Long ids default to -1 when missing. Int notification ids default to -1; `EXTRA_GROUP_ID` defaults to 0.

**1.9.2 Dismiss-then-act pattern (MARK_AS_READ, MARK_AS_SEEN, DOWNLOAD_CHAPTER, DOWNLOAD_EPISODE)**:
1. `notificationId = intent.getIntExtra(EXTRA_NOTIFICATION_ID, -1)`; if `> -1` -> companion `dismissNotification(context, notificationId, intent.getIntExtra(EXTRA_GROUP_ID, 0))`.
2. `urls = intent.getStringArrayExtra(EXTRA_CHAPTER_URL) ?: return`.
3. `id = intent.getLongExtra(EXTRA_MANGA_ID, -1)`; if `> -1` -> run the action.

**1.9.3 Chapter/episode lookup contract.** The actions look up each URL with `getChapter.await(url, mangaId)` / `getEpisode.await(url, animeId)` (area:05-data-repos-entries-items). Missing rows are dropped silently (`mapNotNull`).

**1.9.4 Open chapter / episode (`openChapter`, `openEpisode`, NotificationReceiver.kt:223-254)**:
1. `manga = runBlocking { getManga.await(mangaId) }`; `chapter = runBlocking { getChapter.await(chapterId) }` (blocking DB reads on the main thread; OBSERVED).
2. If both non-null: start `ReaderActivity.newIntent(context, manga.id, chapter.id)` with flags `NEW_TASK | CLEAR_TOP`. Else `context.toast(context.stringResource(AYMR.strings.download_error))` (English "Error").
3. Episode path identical with `getAnime`, `getEpisode`, `PlayerActivity.newIntent`, and the same toast.

**1.9.5 Mark read / seen (`markAsRead`, `markAsSeen`, NotificationReceiver.kt:298-348)**:
Inside `launchIO { ... }` (Dispatchers.IO, fire-and-forget):
1. `downloadPreferences = Injekt.get<DownloadPreferences>()`, `sourceManager = Injekt.get<MangaSourceManager>()` (or `AnimeSourceManager`).
2. `toUpdate = chapterUrls.mapNotNull { getChapter.await(it, mangaId) }.map { chapter -> ... }`. For each original chapter `it`:
   a. `updated = it.copy(read = true)`.
   b. If `downloadPreferences.removeAfterMarkedAsRead().get()` is true: `manga = getManga.await(mangaId)` (re-read per chapter, OBSERVED); if non-null: `source = sourceManager.get(manga.source)`; if non-null: `mangaDownloadManager.deleteChapters(listOf(it), manga, source)`. A missing source skips deletion silently (the chapter is still marked read).
   c. Return `updated.toChapterUpdate()`.
3. `updateChapter.awaitAll(toUpdate)` (area:05-data-repos-entries-items). The interactor logs and swallows exceptions (`UpdateChapter.kt:20-26`), so failures are never reported to the user.
Anime path: `episode.copy(seen = true)`, `animeDownloadManager.deleteEpisodes(listOf(it), anime, source)`, `episode.toEpisodeUpdate()`, `updateEpisode.awaitAll(toUpdate)`.

OBSERVED consequence: `toChapterUpdate()` writes EVERY field of the snapshot (`ChapterUpdate.kt:19-34`: id, mangaId, read, bookmark, lastPageRead, dateFetch, sourceOrder, url, name, dateUpload, chapterNumber, scanlator, version). If the reader saved a different `lastPageRead` between the snapshot read and the write, the notification path overwrites it with the older value. The window is one DB round trip. PROPOSAL: write only `read` (and `seen`), using a partial update (T-35a-016, T-35a-017).

**1.9.6 Share image / share backup (NotificationReceiver.kt:202-214)**:
`Uri.toShareIntent(context, type)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt:13-34`) builds `ACTION_SEND` with `EXTRA_STREAM` for content URIs or `EXTRA_TEXT` for http(s), `clipData`, `FLAG_GRANT_READ_URI_PERMISSION`, wrapped in `Intent.createChooser(shareIntent, context.stringResource(MR.strings.action_share))` with `FLAG_ACTIVITY_NEW_TASK`. Image share uses type `image/*` (default); backup share uses `application/x-protobuf+gzip`.

**1.9.7 Download chapters / episodes (`downloadChapters`, `downloadEpisodes`, NotificationReceiver.kt:356-376)**:
1. `launchIO`: `manga = getManga.await(mangaId) ?: return`; `chapters = chapterUrls.mapNotNull { getChapter.await(it, mangaId) }`; `mangaDownloadManager.downloadChapters(manga, chapters)`.
2. Episode path: `anime = getAnime.await(animeId) ?: return`; `episodes = ...`; `animeDownloadManager.downloadEpisodes(anime, episodes)`.
No toast on failure; empty lists are passed through.

**1.9.8 Queue and job control (NotificationReceiver.kt:72-81, 261-290)**:
- Manga queue: resume -> `startDownloads()`, pause -> `pauseDownloads()`, clear -> `clearQueue()` on `MangaDownloadManager`. Anime equivalents on `AnimeDownloadManager`.
- Restore cancel -> `BackupRestoreJob.stop(context)`; library cancel -> `MangaLibraryUpdateJob.stop(context)`; anime library cancel -> `AnimeLibraryUpdateJob.stop(context)` (areas 22, 26).

**1.9.9 Dead code (OBSERVED)**: `ACTION_OPEN_ENTRY` (NotificationReceiver.kt:400) is never referenced. The entry-open behaviour exists through `MainActivity` shortcuts (section 1.7).

### 1.10 Notification surfaces owned by this scope (OBSERVED texts)

Text is the English MR/AYMR value; placeholders none.

| Surface | Id | Channel | Title | Text | Ongoing | Progress | Actions (label -> target) | Content intent |
|---|---|---|---|---|---|---|---|---|
| Update available (prompt) | 1 (`ID_APP_UPDATER`, default of `show()`) | `app_apk_update_channel` | "New version available!" (`update_check_notification_update_available`) | release `version` (raw tag, e.g. "v0.2.0") | no (not set) | none | "Download" (`action_download`) -> `ACTION_START_APP_UPDATE` (URL + version as title); "What's new" (`whats_new`) -> `ACTION_VIEW` release page | download broadcast (not the release page) |
| Download progress | 1 | same | the title passed to `onDownloadStarted(title)`; for a retry or a missing title it stays as the builder's current title (default app name) | "Downloading…" (`update_check_notification_download_in_progress`, U+2026) | yes | `setProgress(100, p, false)`, `setOnlyAlertOnce(true)` | "Cancel" (`action_cancel`) -> `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` | (none set) |
| Install prompt | 2 (`ID_APP_UPDATE_PROMPT`) | same | builder's title (the version if set by the download start) | "Tap to install update" (`update_check_notification_download_complete`) | yes (never cleared) | `setProgress(0, 0, false)` (no bar) | "Install" (`action_install`) -> APK `ACTION_VIEW`; "Cancel" -> dismiss id 2 | APK `ACTION_VIEW` |
| Download error | 3 (`ID_APP_UPDATE_ERROR`) | same | builder's title | "Download error" (`update_check_notification_download_error`) | yes (inherited, see 2.9) | none | "Retry" (`action_retry`) -> `ACTION_START_APP_UPDATE` (URL only, no title); "Cancel" -> dismiss id 3 | (inherited from the builder) |

Icons (Android drawable names, OBSERVED): prompt small icon `android.R.drawable.stat_sys_download_done`; progress `stat_sys_download`; install `stat_sys_download_done` and action icon `ic_system_update_alt_white_24dp`; error small icon `ic_warning_white_24dp`, action icons `ic_refresh_24dp`, `ic_close_24dp`, `ic_info_24dp`, `ic_close_24dp`. Kurobayomi uses bundled symbolic icons (T-35a-087).

### 1.11 Hide-notification-content (OBSERVED, cross-area)

`SecurityPreferences.hideNotificationContent()` (key `hide_notification_content`, default `false`; core/common SecurityPreferences.kt:19) is read by the download and library notifiers. When true they hide the item text. This scope's notifications (update prompt, install prompt, download progress, errors) do NOT read the preference (OBSERVED: no reference in AppUpdateNotifier.kt). PROPOSAL: the update text is not sensitive, so it stays visible; the shared Notifier applies the gate to any notice flagged `private_content = true` (T-35a-009).

### 1.12 Threading and dispatch (OBSERVED)

- `onReceive` on the main thread; it calls `runBlocking` for open actions (section 1.9.4): a blocking DB read on the UI thread.
- Mark / download actions use `launchIO` (`tachiyomi.core.common.util.lang.launchIO`, CoroutinesExtensions.kt:32).
- `AppUpdateDownloadJob` runs on WorkManager's `CoroutineWorker` dispatcher; the download body is wrapped in `withIOContext`.
- `AppUpdateChecker.checkForUpdate` wraps the interactor in `withIOContext`; `promptUpdate` runs on that IO thread (`AppUpdateChecker.kt:221-223`).
- Posting a notification from a worker thread is allowed on Android.

PROPOSAL (Rust): `Notifier::show` may be called from any thread; the GTK implementation marshals to the glib main context (`glib::MainContext::default().invoke`) because `gio::Application::send_notification` is documented for the main loop. The router (section 1.8) runs on the glib main context; DB and network work run on tokio.

### 1.13 Notification edge cases (OBSERVED unless marked)

1. `POST_NOTIFICATIONS` denied (API 33+): all `Context.notify` calls return silently; the download still runs (AppUpdateDownloadJob ignores the failure). PROPOSAL: no permission on Linux; when the desktop service is down, log once and continue.
2. Id 2 collision (section 1.4).
3. `cancelAll()` side effect (section 1.5(a)).
4. Group summary branch leaves the single notification (section 1.5b).
5. A download notification posted before `POST_NOTIFICATIONS` is granted is lost (no retry).
6. `AppUpdateNotifier` keeps one builder per instance; `AppUpdateDownloadJob` owns one instance so state carries from download start to error (ongoing flag, section 2.9).
7. Notification sound: only `backup_restore_complete_channel_v2` is silent. Other channels use the default sound.

---

## 2. App updater

### 2.1 Build gating (OBSERVED)

- `updaterEnabled` = `BuildConfig.UPDATER_ENABLED` (util/system/BuildConfig.kt:7-8). `UPDATER_ENABLED` = `Config.enableUpdater` = `project.hasProperty("enable-updater")` (buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt:12). Default: updater DISABLED unless the Gradle property is passed.
- `isPreviewBuildType` = `BuildConfig.BUILD_TYPE == "preview"` (util/system/BuildConfig.kt:13-14). Preview = `create("preview")` build type (app/build.gradle.kts:62-67), `applicationIdSuffix = ".debug"`.
- `GITHUB_REPO` (AppUpdateChecker.kt:233-239): preview -> `"aniyomiorg/aniyomi-preview"`; else `"aniyomiorg/aniyomi"`.
- `RELEASE_TAG` (AppUpdateChecker.kt:241-247): preview -> `"r${BuildConfig.COMMIT_COUNT}"`; else `"v${BuildConfig.VERSION_NAME}"`.
- `RELEASE_URL` (AppUpdateChecker.kt:249) = `"https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG"`.
- `BuildConfig.VERSION_NAME` in this source = "0.18.2.1" (app/build.gradle.kts:23); debug builds append `-<commit count>` (line 48); preview copies the debug suffix (line 67).
- `BuildConfig.COMMIT_COUNT` = `git rev-list --count HEAD` (buildSrc Commands.kt:11-13).

PROPOSAL (Kurobayomi): `updater` cargo feature, off by default (matches `enable-updater`). Repo owner/name come from config with NO default (the Kurobayomi checkout has no git remote, so no repo is assumed). Until configured, the check is disabled and About shows the row disabled with the note "Set update_repo to enable". (This note text is new; T-35a-046.)

### 2.2 Trigger points (OBSERVED)

1. Automatic: `MainActivity.CheckForUpdates()` (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:372-390`): `LaunchedEffect(Unit)` runs when the composable enters composition. If `updaterEnabled`: `AppUpdateChecker().checkForUpdate(context)` (forceCheck default false). If the result is `NewUpdate`, pushes `NewUpdateScreen(versionName, changelogInfo = release.info, releaseLink, downloadLink)`. Any `Exception` is logged with `logcat(LogPriority.ERROR, e)` and dropped.
2. Manual: About screen "Check for updates" row (`app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:98-134`), shown only when `updaterEnabled`. Click (if not already checking): `isCheckingUpdates = true`, `checkVersion(...)` (AboutScreen.kt:203-232) with `forceCheck = true`, `onFinish` resets the flag. Trailing spinner `CircularProgressIndicator(size 28.dp, strokeWidth 3.dp)` shown while checking.

### 2.3 Check algorithm `GetApplicationRelease.await(arguments)` (OBSERVED, domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt:19-46)

Input `Arguments(isPreview, commitCount, versionName, repository, forceCheck = false)`.

1. `now = Instant.now()`.
2. Throttle: if `!forceCheck` AND `now.isBefore(Instant.ofEpochMilli(lastChecked.get()).plus(3, DAYS))` -> return `NoNewUpdate` (no network call). `lastChecked` = `preferenceStore.getLong(Preference.appStateKey("last_app_check"), 0)`, key string `"__APP_STATE_last_app_check"` (`Preference.kt:43-45`, `APP_STATE_PREFIX = "__APP_STATE_"`). Default 0 means "never checked" -> proceed.
3. `release = service.latest(arguments) ?: return NoNewUpdate`. The timestamp is NOT updated on this path (OBSERVED; see 2.4 edge E4).
4. `lastChecked.set(now.toEpochMilli())`. The timestamp is set for every non-null release, new or not.
5. `isNewVersion(...)`: NewUpdate(release) or NoNewUpdate.

Exceptions from `service.latest` propagate and the timestamp is not set (so the next launch retries).

### 2.4 Version comparison `isNewVersion` (OBSERVED, GetApplicationRelease.kt:48-76)

```
newVersion = versionTag.replace("[^\\d.]".toRegex(), "")      // delete every char that is not a digit or dot
if isPreview:
    return newVersion.toInt() > commitCount
else:
    oldVersion = versionName.replace("[^\\d.]".toRegex(), "")
    newSemVer = newVersion.split(".").map { it.toInt() }
    oldSemVer = oldVersion.split(".").map { it.toInt() }
    oldSemVer.mapIndexed { index, i ->
        if (newSemVer[index] > i) return true                    // returns from isNewVersion
    }
    return false
```

Preview example: tag "r2000", commitCount 1000 -> "2000".toInt() = 2000 > 1000 -> true.

Release example A: tag "v2.0.0", versionName "v1.0.0" (test 2) -> true. Test 3: tag "v1.0.0", version "v2.0.0" -> false.

Defects (OBSERVED, all reproducible from the code):
- E1 Non-lexicographic: old "2.5.0", new "2.0.1": index 0: 2>2 no; index 1: 0>5 no; index 2: 1>0 YES -> true (update offered for an older tag).
- E2 Old "1.0.0", new "0.9.9": index 0: 0>1 no; index 1: 9>0 YES -> true (older tag offered).
- E3 Fewer parts in the new tag: old "1.2.3", new "1.2": index 2 -> `newSemVer[2]` IndexOutOfBoundsException.
- E4 Empty or non-numeric new tag: "latest" -> "" -> `"".split(".")` = [""] -> `"".toInt()` NumberFormatException. A tag like "release-1" -> "1" in preview; in release it yields [1] and fails on index 1 when old has 3 parts.
- E5 Debug build version with suffix: versionName "0.18.2.1-123" -> "0.18.2.1123" (the dash is removed and the digits are glued). The comparison is then against a huge last component; debug builds never see updates unless the tag's last component exceeds that number.
- E6 Pre-release tags: "v1.0.0-rc.1" -> "1.0.0.1" (4 parts). Compared only up to the old part count; behaves like 1.0.0.

PROPOSAL: parse both with `semver::Version` after stripping a leading `v`; compare with `Version` ordering; reject unparsable input with `UpdateError::BadVersion` (no panic). Test cases: T-35a-090.

Preview rule (PROPOSAL, only if a preview channel exists): integer compare of the trailing number. Not needed for Kurobayomi v1 (T-35a-061).

### 2.5 Release fetch and asset selection `ReleaseServiceImpl.latest` (OBSERVED, data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:18-43)

1. Request: `GET https://api.github.com/repos/${arguments.repository}/releases/latest` through `networkService.client.newCall(GET(url)).awaitSuccess()`; `awaitSuccess` throws `HttpException(code)` on non-2xx (404 when the repo has no release; the About toast then shows `e.message`).
2. Parse `GithubRelease` (`data/src/main/java/tachiyomi/data/release/GithubRelease.kt`): `tag_name` -> `version`; `body` -> `info`; `html_url` -> `releaseLink`; `assets[]` of `{name, browser_download_url -> downloadLink}`. Unknown JSON keys are ignored by the Json instance (area:12-network-stack).
3. `downloadLink = getDownloadLink(release) ?: return null`:
   a. `map = release.assets.associate { asset -> BUILD_TYPES.find { "-$it" in asset.name } to asset.downloadLink }` with `BUILD_TYPES = listOf("arm64-v8a", "armeabi-v7a", "x86_64", "x86")`. Key = matched ABI or `null`. `associate` keeps the LAST asset for a duplicated key.
   b. Return `map[Build.SUPPORTED_ABIS[0]] ?: map[null]`.
   So: the asset whose name contains `-<ABI of primary CPU ABI>`, else the last asset with no ABI token, else null (no update).
4. `Release(version = tag_name, info = body with @mentions rewritten, releaseLink = html_url, downloadLink)`.
5. Mention rewrite: regex `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` (IGNORE_CASE; GitHub username rules) replaced by `[@user](https://github.com/user)` (ReleaseServiceImpl.kt:29-31, 58-59).

PROPOSAL (Linux asset rule): candidates are assets whose name matches `kurobayomi-<version>-<arch>.<ext>` where `<arch>` is from `std::env::consts::ARCH` mapped `x86_64 -> x86_64`, `aarch64 -> aarch64`, and `<ext>` in {`AppImage`, `tar.zst`}. Fallback: an asset with no arch token (the `map[null]` rule). No match -> `NoNewUpdate` without touching the timestamp (T-35a-043).

### 2.6 Side effects of a NewUpdate (OBSERVED)

`AppUpdateChecker.checkForUpdate` (AppUpdateChecker.kt:210-229): when the result is `NewUpdate`, it calls `AppUpdateNotifier(context).promptUpdate(result.release)` BEFORE returning. So the same event produces:
1. the notification (section 1.10, row "Update available"), and
2. for the automatic path: `NewUpdateScreen` pushed by MainActivity (AppUpdateChecker returns the result; MainActivity pushes the screen), and for the manual path: `NewUpdateScreen` pushed by AboutScreen's `onAvailableUpdate`.

`promptUpdate` (AppUpdateNotifier.kt:194-230): `updateIntent = downloadAppUpdatePendingBroadcast(context, release.downloadLink, release.version)`; `releaseIntent` = `ACTION_VIEW` on `release.releaseLink` with `NEW_TASK | CLEAR_TOP` (request code `release.hashCode()`), sets title/text/small icon/content intent/actions as in section 1.10; posts id 1.

Gate effect: the throttle timestamp was set (2.3 step 4), so the automatic path shows the notification and the screen at most once per 3 days even if the user dismisses both.

### 2.7 Manual check result mapping (OBSERVED, AboutScreen.kt:203-232)

Inside `withUIContext`, `updateChecker.checkForUpdate(context, forceCheck = true)` (on IO):
- `NewUpdate` -> `onAvailableUpdate(result)` -> push `NewUpdateScreen`.
- `NoNewUpdate` -> toast `MR.strings.update_check_no_new_updates` ("No new updates available").
- `OsTooOld` -> toast `MR.strings.update_check_eol` ("This Android version is no longer supported"). Never produced (the check is commented out at AppUpdateChecker.kt:205-208). Dropped on Linux (T-35a-060).
- Exception -> `context.toast(e.message)` and log. `finally { onFinish() }`.

### 2.8 NewUpdateScreen (OBSERVED, app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt and presentation/more/NewUpdateScreen.kt)

- Changelog: `changelogInfo.replace("""---(\R|.)*Checksums(\R|.)*""".toRegex(), "")` (NewUpdateScreen.kt:24-26). Removes from the first `---` line that is followed later by the word "Checksums" through the end of the text. Note: `(\R|.)*` is greedy across newlines.
- Heading "New version available!" (`update_check_notification_update_available`), subtitle = version name, accept button "Download" (`update_check_confirm`), reject "Not now" (`action_not_now`); markdown body with links; text button "Open on GitHub" (`update_check_open`) with the open-in-new icon -> `openInBrowser(releaseLink)`.
- Accept: `AppUpdateDownloadJob.start(context, url = downloadLink, title = versionName)` then `navigator.pop()`.
- Reject: `navigator.pop()`.

### 2.9 Download job (OBSERVED, AppUpdateDownloadJob.kt:40-125)

`doWork()`:
1. `url = inputData.getString("DOWNLOAD_URL")`; `title = inputData.getString("DOWNLOAD_TITLE") ?: "Aniyomi"` (`MR.strings.app_name`).
2. `url` null or empty -> `Result.failure()` (no notification).
3. `setForeground(getForegroundInfo())` in try/catch `IllegalStateException` (logged, continues). `getForegroundInfo()` = `ForegroundInfo(ID_APP_UPDATER, notifier.onDownloadStarted().build(), type)` with type `FOREGROUND_SERVICE_TYPE_DATA_SYNC` on API 29+, else 0. `onDownloadStarted()` with no title posts id 1 (side effect).
4. `withIOContext { downloadApk(title, url) }`.
5. `Result.success()` is returned even when the download failed (the failure is shown as a notification). Observed; the work queue never retries.

`downloadApk(title, url)`:
1. `notifier.onDownloadStarted(title)` -> post id 1 (title, "Downloading…", ongoing, Cancel).
2. Progress listener (state `savedProgress = 0`, `lastTick = 0L`): on each `update(bytesRead, contentLength, done)`:
   `progress = (100 * (bytesRead.toFloat() / contentLength)).toInt()`; `now = currentTimeMillis()`; if `progress > savedProgress && now - 200 > lastTick` then `savedProgress = progress; lastTick = now; notifier.onProgressChange(progress)`.
   Rules: strictly increasing integer percent AND more than 200 ms since the last emitted update (strictly greater than 200). When `contentLength <= 0` the percent is <= 0 or NaN -> no updates (indeterminate, OBSERVED).
3. `response = network.client.newCachelessCallWithProgress(GET(url), progressListener).await()`:
   - `cache(null)` (no HTTP cache), `callTimeout(30, HOURS)`, network interceptor replacing the body with a progress-reporting body (OkHttpExtensions.kt:119-131). Other settings come from the shared client (area:12-network-stack).
4. `apkFile = File(context.externalCacheDir, "update.apk")`. OBSERVED: if `externalCacheDir` is null the path is the relative `update.apk`.
5. If `response.isSuccessful`: `response.body.source().saveTo(apkFile)` (`OkioExtensions.kt:14-27`: `mkdirs()` on the parent, copy, on exception delete the partial file and rethrow). Else `response.close()` and `throw Exception("Unsuccessful response")`.
6. Success: `notifier.cancel()` (withdraw id 1), then `notifier.promptInstall(apkFile.getUriCompat(context))` (FileProvider URI, `provider_paths.xml`).
7. Catch `Exception e`:
   - `shouldCancel = e is CancellationException || (e is StreamResetException && e.errorCode == ErrorCode.CANCEL)`.
   - If `shouldCancel`: `notifier.cancel()` (withdraw id 1). Else: `notifier.onDownloadError(url)` (post id 3). Id 1 is NOT withdrawn on this path (OBSERVED; the progress notification stays and is still `ongoing`). Section 2.10 lists the fix (T-35a-052).

Start / replace / stop (OBSERVED, AppUpdateDownloadJob.kt:133-154):
- `start(context, url, title?)`: `Constraints(requiredNetworkType = CONNECTED)`, one-time request with tag `"AppUpdateDownload"`, input `DOWNLOAD_URL` and `DOWNLOAD_TITLE` (`workDataOf` with a null title value), `enqueueUniqueWork("AppUpdateDownload", REPLACE, request)`.
- `stop(context)`: `cancelUniqueWork("AppUpdateDownload")`.
- Only one download at a time; a new start replaces the running one.

Retry from the error notification (NotificationReceiver.kt:283-286): `startDownloadAppUpdate` reads only `EXTRA_DOWNLOAD_URL` and calls `AppUpdateDownloadJob.start(context, url)`, so the title is lost (OBSERVED). The "Download" action on the prompt has the same loss: the progress notification then shows the default app name instead of the version (edge E-U3 in section 2.11).

### 2.10 Install step (OBSERVED, then Linux replacement)

Android: `promptInstall(uri)` posts the install prompt (section 1.10). "Install" opens the system package installer for the APK (`ACTION_VIEW`, APK MIME, read grant). The APK is never verified: no checksum, no signature check in this code path; the system installer verifies the signature.

Linux replacement (PROPOSAL, T-35a-053 and T-35a-055): the Linux build does not replace itself from an APK. The completed download is a file in `Paths::cache_dir()/updates/`. The prompt says "Downloaded <file>. Open folder" and the action `Opener::reveal_file(path)` runs. Package-managed installs (pacman/AUR) show "Update with your package manager" and no download button. AppImage replacement is out of scope (T-35a-056 Dropped).

### 2.11 Updater edge cases

- E-U1 Release with no matching asset: `latest` returns null -> `NoNewUpdate`; the timestamp is not set (the next launch re-queries GitHub). Manual check shows "No new updates available" (misleading; PROPOSAL: show "No downloadable build for this platform").
- E-U2 Rate limit: unauthenticated GitHub API requests are limited (60/hour per IP; GitHub's documented limit). A 403 surfaces as `HttpException(403)`; the About toast shows its message (`e.message`, which is the exception message only). PROPOSAL: show "GitHub rate limit reached, try later" for 403/429.
- E-U3 Title lost on Download/Retry (section 2.9).
- E-U4 Prompt "What's new" opens the release page; the prompt body (content intent) starts the download instead (`AppUpdateNotifier.kt:194-199` sets `setContentIntent(updateIntent)`).
- E-U5 Download on metered network: `CONNECTED` only (not `UNMETERED`); the download starts on mobile data too.
- E-U6 A failed prompt-start (no notification permission): the download still runs; the user only sees the screen if the check was manual or from MainActivity.
- E-U7 Interrupted download: partial `update.apk` is deleted by `saveTo` on exception (OkioExtensions.kt:21-23). A cancelled CancellationException during `saveTo` also deletes it.
- E-U8 Changelog with no "Checksums" heading: nothing removed.
- Live format of the Checksums section (checked read-only with `gh api repos/aniyomiorg/aniyomi/releases/latest` on 2026-10-08, release v0.18.2.1): the body ends with `---`, then `### Checksums`, then a table `| Variant | SHA-256 |` with rows `| Universal | <64 hex>`, `| arm64-v8a | <64 hex>`, `| armeabi-v7a | <64 hex>`, `| x86 | <64 hex>`, `| x86_64 | <64 hex> |`. The same release lists five assets: `aniyomi-arm64-v8a-v0.18.2.1.apk`, `aniyomi-armeabi-v7a-v0.18.2.1.apk`, `aniyomi-v0.18.2.1.apk` (Universal, no ABI token), `aniyomi-x86-v0.18.2.1.apk`, `aniyomi-x86_64-v0.18.2.1.apk`. The asset names confirm the ABI token rule of section 2.5.

- E-U9 `body` is null on GitHub (release without notes): Kotlin non-null `String` parsing fails -> exception -> logged; no prompt (PROPOSAL: treat missing body as empty).
- E-U10 Two checks at once (manual while automatic is in progress): both call the service; both may prompt (PROPOSAL: single-flight guard).

---

## 3. Library CSV export

### 3.1 UI flow (OBSERVED, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:346-498)

1. Settings > Data > "Export" group (`MR.strings.export` = "Export") with one text preference `MR.strings.library_list` = "Library List" (line 403-410).
2. Clicking it sets `showDialog = true`.
3. Dialog `ColumnSelectionDialog` (lines 414-498): `AlertDialog` with title `MR.strings.migration_dialog_what_to_include` = "Select data to include". Four checkboxes in this order:
   - Title (`MR.strings.title` = "Title"), enabled always. Unchecking Title also unchecks Type, Author and Artist and disables them.
   - Type (`AYMR.strings.type` = "Type"), enabled only while Title is checked.
   - Author (`MR.strings.author` = "Author"), enabled only while Title is checked.
   - Artist (`MR.strings.artist` = "Artist"), enabled only while Title is checked.
   Buttons: confirm `MR.strings.action_save` = "Save"; dismiss `MR.strings.action_cancel` = "Cancel".
4. Save: `exportOptions = ExportOptions(includeTitle, includeType, includeAuthor, includeArtist)` from the local states, `onDismissRequest()`, then `saveFileLauncher.launch("aniyomi_library.csv")` (line 397).
5. `saveFileLauncher = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/csv"))` (lines 372-390). On a non-null URI: `scope.launch { LibraryExporter.exportToCsv(context, uri, favorites, exportOptions) { scope.launch(Dispatchers.Main) { context.toast(MR.strings.library_exported) } } }`. Cancelled picker: nothing happens.
6. `favorites` is loaded once per composition: `LaunchedEffect(Unit) { favorites = getAnimeFavorites.await().map { it.toExportEntry() } + getMangaFavorites.await().map { it.toExportEntry() } }` (lines 366-370). Anime first, then manga. Changes made after the screen opened are not seen by the export (OBSERVED stale snapshot).
7. `exportOptions` defaults to all four true (lines 349-358). It is `remember`ed: it survives dialog reopening within the same screen but resets when the screen is left (OBSERVED; not persisted).
8. Toast on success: `MR.strings.library_exported` = "Library Exported" (even when nothing was written; see 3.5).

### 3.2 Data source (OBSERVED)

- `GetAnimeFavorites` / `GetMangaFavorites` call `AnimeRepository` / `MangaRepository.getFavorites()` -> `animesQueries.getFavorites(AnimeMapper::mapAnime)` / `mangasQueries.getFavorites(MangaMapper::mapManga)` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:53`, `data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:49`).
- SQL: `getFavorites: SELECT * FROM mangas WHERE favorite = 1;` (`data/src/main/sqldelight/data/mangas.sq:76-79`) and `SELECT * FROM animes WHERE favorite = 1;` (`data/src/main/sqldelightanime/dataanime/animes.sq:87-90`). No `ORDER BY`. Order = SQLite scan order (OBSERVED: undefined by contract).
- Index `library_favorite_index ON mangas(favorite) WHERE favorite = 1` exists (mangas.sq:33) and `animelib_favorite_index` (animes.sq:43), so the scan uses the partial index; order then follows index order (rowid within equal keys). PROPOSAL: `ORDER BY _id` (the primary key column is `_id` in both tables: mangas.sq:7, animes.sq) for a reproducible file.
- Export uses only `title`, `author`, `artist`, `favorite` and the entry type. No chapter/episode data is exported.

### 3.3 Options gating (OBSERVED only in the UI)

The UI forces Type, Author, Artist off when Title is off. The renderer does not enforce this: any combination is rendered (section 3.4 rule R4).

### 3.4 CSV rendering (OBSERVED, LibraryExporter.kt:65-95; exact algorithm)

Input: `favorites: List<ExportEntry>`, `options`.

1. `columnSize = count of true among [includeTitle, includeType, includeAuthor, includeArtist]`.
2. For each entry in order, build a row list of size `columnSize` (`buildList(columnSize)`), appending in this fixed order when enabled:
   a. `entry.title` if includeTitle;
   b. `entry.type.name.lowercase()` ("manga" or "anime") if includeType;
   c. `entry.author` (nullable) if includeAuthor;
   d. `entry.artist` (nullable) if includeArtist.
   Every row is added even when it is empty (`.let(::add)`).
3. For each row, each cell `column` is rendered:
   R1. If `column.isNullOrBlank()` (null, empty, or only whitespace per Kotlin's `isBlank`) -> empty string `""`.
   R2. Else if the cell contains any of `\r`, `\n`, `"`, `,` -> `"` + cell.replace("\"", "\"\"") + `"`.
   R3. Else the cell unchanged (leading/trailing spaces are kept and not quoted).
   Cells are joined with `,`.
4. Rows are joined with `\r\n` (no trailing separator).
5. The string is encoded with `String.toByteArray()` (UTF-8 on Android; no BOM).

Worked examples (options: all four on):
- Entry anime "Frieren", author "Kanehito Yamada", artist null -> `Frieren,anime,Kanehito Yamada,`
- Entry manga `He said "hi", then left` (author = `A<LF>B`, artist empty) -> `"He said ""hi"", then left",manga,"A<LF>B",`
- Two entries -> `row1\r\nrow2` (CRLF between rows, none at the end).

Zero enabled columns (all false): every row is `""` (empty). For N >= 1 rows the output is N-1 CRLFs. For zero favourites: the empty string (0 bytes).

### 3.5 Writing (OBSERVED, LibraryExporter.kt:47-61)

1. `withContext(Dispatchers.IO)`.
2. `context.contentResolver.openOutputStream(uri)?.use { it.write(csvData.toByteArray()) }`.
3. `onExportComplete()` is called in every case, including when `openOutputStream` returns null (no file written, success toast shown). OBSERVED defect: silent success.
4. Exceptions (for example `IOException` from a read-only target) propagate out of `exportToCsv`; the caller `scope.launch` has no handler, so the exception goes to the scope's uncaught handler. No toast (OBSERVED).

PROPOSAL (Kurobayomi): write to `<path>.tmp` in the same directory, `fsync`, then rename over the target; on error show the error toast (`MR.strings.cache_delete_error`-style key, new key `export_failed`, T-35a-074).

### 3.6 Export edge cases

- X1 Title with a leading `=` (CSV formula injection in spreadsheet apps): not escaped (OBSERVED). PROPOSAL: keep parity; mention in the docs.
- X2 Very large libraries: the whole CSV is built in memory as one String (OBSERVED). PROPOSAL: stream rows (`BufWriter`).
- X3 A name with `\r` alone: quoted (rule R2).
- X4 Non-ASCII text: UTF-8 bytes, no BOM (Excel will misread non-ASCII without a BOM; OBSERVED behaviour, keep for parity).
- X5 Favourites changed during the dialog: stale snapshot (3.1 step 6).

---

## 4. Android XML resources (OBSERVED, then the decision)

### 4.1 network_security_config.xml (`app/src/main/res/xml/network_security_config.xml`)

- `base-config cleartextTrafficPermitted="true"` (lines 4-6): HTTP is allowed for every host.
- `trust-anchors`: `certificates src="system"` (preinstalled CAs) and `certificates src="user"` (user-installed CAs), lines 7-15.
- Referenced by `AndroidManifest.xml:45` as `android:networkSecurityConfig="@xml/network_security_config"`.

Linux: reqwest (area:12-network-stack) allows `http://` URLs by default, so cleartext needs no setting. The system trust store is loaded when the `native-roots` (or `rustls-tls-native-roots`) feature is enabled. This reproduces "system + user CAs" (the user CAs on Linux live in the system store). PROPOSAL: leave the default; document that cleartext is allowed (T-35a-031).

### 4.2 provider_paths.xml (`app/src/main/res/xml/provider_paths.xml`)

- FileProvider (`AndroidManifest.xml:298-304`, authority `${applicationId}.provider`, `grantUriPermissions="true"`) exposes: `cache-path cache_files .`, `external-path ext .`, `external-cache-path ext_cache_files .`, `external-files-path ext_files .`, `root-path ext_storage /storage/`.
- Used by `File.getUriCompat(context)` (core/common FileExtensions.kt:16-18) for the APK install and for share/reveal.

Linux: no content URIs. Files are shared as paths (`Opener::reveal_file`) or through the desktop file chooser portal (`gtk::FileDialog`). Dropped (T-35a-030).

### 4.3 s_pen_actions.xml (`app/src/main/res/xml/s_pen_actions.xml`)

Samsung "Remote Action" set (`actionset_label="@string/app_name"`, `version="1.2"`). Referenced by ReaderActivity (`AndroidManifest.xml:194-205`, intent action `com.samsung.android.support.REMOTE_ACTION`) and PlayerActivity (`AndroidManifest.xml:206-221`).

| id | label key (English) | priority | repeatable | interval | trigger_key | gesture |
|---|---|---|---|---|---|---|
| `tachiyomi_next_page` | `spen_next_page` = "Next page" | 1 | true | short | `CTRL_LEFT+DPAD_RIGHT` | `click` |
| `tachiyomi_previous_page` | `spen_previous_page` = "Previous page" | 2 | true | short | `CTRL_LEFT+DPAD_LEFT` | `double_click` |
| `tachiyomi_back` | `action_webview_back` = "Back" | 3 | true | short | `BACK` | `circle_ccw` |
| `tachiyomi_next_chapter` | `action_next_chapter` = "Next chapter" | 4 | true | short | `N` | `swipe_right` |
| `tachiyomi_previous_chapter` | `action_previous_chapter` = "Previous chapter" | 5 | true | short | `P` | `swipe_left` |

Linux: Dropped as a protocol (Samsung hardware). The keys are kept as keyboard shortcuts: `N` next chapter/episode, `P` previous chapter/episode, `Back` maps to the window's back navigation (proposed keys Alt+Left and Backspace), and the page actions map to the reader's arrow keys (owned by area:44-reader-viewers and area:45-player-core). T-35a-027 (drop) and T-35a-028 (keyboard map).

### 4.4 searchable.xml (`app/src/main/res/xml/searchable.xml`)

`android:hint="@string/action_global_search_hint"` ("Global search…", U+2026) and `android:label="@string/app_name"`. Attached to `DeepLinkAnimeActivity` and `DeepLinkMangaActivity` (`AndroidManifest.xml:158-160` and `:190-192`) with search intents `android.intent.action.SEARCH`, `com.google.android.gms.actions.SEARCH_ACTION`, `eu.kanade.tachiyomi.SEARCH` / `eu.kanade.tachiyomi.ANIMESEARCH`.

Linux: Android's global search UI does not exist. The in-app search entry is owned by area:40-ui-browse-manga and area:41-ui-browse-anime. A GNOME/KDE search provider (`org.gnome.Shell.SearchProvider2` D-Bus) is a P3 extra (T-35a-029).

---

## 5. Behaviour summary table

| Feature | Trigger | Output | Persisted state | Failure behaviour |
|---|---|---|---|---|
| Channel setup | app start | channels/groups created, 9 channels deleted | none | logged |
| Update check (auto) | MainActivity composition, updater enabled | notification + NewUpdateScreen | `__APP_STATE_last_app_check` | logged, no UI |
| Update check (manual) | About row | screen or toast | same timestamp, bypassed with force | toast with message |
| Update download | "Download" on screen or notification | id 1 progress, then id 2 install or id 3 error | `externalCacheDir/update.apk` | id 3 error notification |
| Mark read / seen | notification action | chapter/episode rows updated; optional file delete | DB | silent |
| Library export | Settings > Export > Library List > Save | CSV file, toast | none (options in memory) | silent defect (3.5) |
