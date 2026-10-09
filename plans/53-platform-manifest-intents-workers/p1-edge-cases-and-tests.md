# Area 53 — edge cases, Kotlin defects, race conditions and tests

Upstream tests: `find` over the Aniyomi tree finds no `*Test.kt` or `*Tests.kt` that references `NotificationReceiver`, `NotificationHandler`, `Notifications`, the deep-link models, `MainActivity` intent routing, `TrackLoginActivity`, the shortcuts or the animation resources. The scope therefore contributes no upstream tests to port. Section 5 lists the tests this area writes. Section 4 gives the fixtures they use.

Defect and edge IDs are `E-nn`. "Kotlin" means the behaviour observed in the file named; "Linux" is the required behaviour in Kurobayomi. Each entry names the test that proves the Linux behaviour (section 5, IDs `TC-53-nn`) and the task that implements it.

## 1. Defects observed in the Kotlin (not to be reproduced)

| ID | Where (path:line) | What the Kotlin does | Why it is wrong | Linux behaviour | Test | Task |
|---|---|---|---|---|---|---|
| E-01 | `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt:191` to `:194` (private `dismissNotification`), used by `ACTION_DISMISS_NOTIFICATION` at `:66` to `:70` | `cancelNotification(id)` then `notificationManager.cancelAll()` | Removes every notice of the app, including progress notices of running downloads, library updates and servers, when the user dismisses one app-update notice | withdraw exactly one id; no bulk withdraw exists in the `Notifier` trait | TC-53-07 | T-53a-019 |
| E-02 | `Notifications.kt:123` (`CHANNEL_LIBRARY_PROGRESS = "library_progress_channel"`) and `:195` (deprecated list) | the channel id is deleted and recreated at every process start | discards any user change to that channel's importance and sound on every launch | no per-start deletion; categories are stored as preferences | TC-53-17 | T-53a-021 |
| E-03 | `NotificationReceiver.kt:224` to `:225` and `:244` to `:245` (`runBlocking { getManga.await(...) }`) on the broadcast thread (comment `:48` to `:52` says UI thread) | blocks the main thread on database reads | can freeze the window; a slow database blocks input | async handler; no blocking call on the GTK thread | TC-53-08 | T-53a-017 |
| E-04 | `NotificationReceiver.kt:86` (`intent.getStringExtra(EXTRA_URI)!!`) and `:92` (`getParcelableExtraCompat(EXTRA_URI)!!`) | a missing extra throws `NullPointerException` | a malformed or stale notice crashes the app | payload validation returns a typed error and a log line | TC-53-05 | T-53a-018 |
| E-05 | `NotificationReceiver.kt` companion builders with request code `0`, for example `:425` to `:435`, `:872` to `:888`, `:595` to `:606`; `NotificationHandler.kt:21` to `:32` | `PendingIntent` identity ignores extras; `FLAG_UPDATE_CURRENT` overwrites the extras of an earlier PendingIntent with the same action | the second notice's button acts on the first notice's payload (for example an update URL or a share URI) | payload is inside the GAction target; each button is independent | TC-53-09 | T-53a-018 |
| E-06 | `NotificationReceiver.kt:578` to `:581` | with exactly two notices in the group, the summary is cancelled and the single child is not (verified only by reading; device behaviour unverified) | the child remains visible while the code intends to clear the group | no group summary exists on Linux; a single id is withdrawn | TC-53-10 (device check) | T-53a-020 |
| E-07 | `DeepLinkAnimeScreenModel.kt:33` to `:60` and `DeepLinkMangaScreenModel.kt:34` to `:61` (`launchIO` without try) | an exception (network, parse) escapes; `mutableState` is never updated | the page shows the loading indicator forever | `ResolveError` becomes a visible error state with Retry | TC-53-20 | T-53a-048, T-53a-050 |
| E-08 | `TorrentServerService.kt:96` to `:103` (`setDeleteIntent(startAgainIntent)`, line 120) | dismissing the notice starts the server again | a dismiss restarts a stopped server | the notice has no delete action; stopping is explicit | TC-53-12 | T-53a-035 |
| E-09 | `DeepLinkAnimeScreen.kt:60` to `:66` and `DeepLinkAnimeScreenModel` (episode path) | the episode opens `PlayerActivity.newIntent` directly | the external-player setting (area:45) is bypassed | route through the player router, which applies the setting | TC-53-21 | T-53a-048 |
| E-10 | `DeepLinkAnimeScreen.kt:44` to `:68` and `DeepLinkMangaScreen.kt:44` to `:68` | `navigator.replace(...)` and `navigator.pop()` called from inside `when (state)` in the composable body | navigation runs on every recomposition while the state holds; repeated pushes are possible | one-shot navigation effect, executed once per state change | TC-53-22 | T-53a-047, T-53a-049 |
| E-11 | `MainActivity.kt:545` (`intent.data.toString().endsWith(".tachibk")`) | the test is a string suffix on the full URI | a `content://` URI whose encoded name does not end in the suffix is ignored; a query string ending in `.tachibk` is accepted | parse the path component after URL decoding | TC-53-14 | T-53a-010 |
| E-12 | `NotificationExtensions.kt:30` to `:37` (`POST_NOTIFICATIONS` check, silent return) | on API 33 and above a missing permission drops the notice with no message | the user never learns that notices are off | desktop: if no notification server, show an in-app toast with the same text | TC-53-06 | T-53a-023 |
| E-13 | `AppUpdateDownloadJob.kt:106` (`File(context.externalCacheDir, "update.apk")`) | `externalCacheDir` can be null; `File(null, name)` is then relative to the working directory | the APK is written to an unpredictable location | `Paths::cache()` is always absolute; fail with a visible error if it is not | TC-53-18 | T-53a-034 |
| E-14 | `MainActivity.kt:508` to `:510` (`DeepLinkScreenType.valueOf`) | any non-blank `type` other than `ANIME` or `MANGA` throws `IllegalArgumentException` | a crafted intent crashes the window | `RouteError::UnknownKind` and a toast | TC-53-03 | T-53a-045 |
| E-15 | `NetworkStateTracker.kt:24` (`isWifi = wifiManager.isWifiEnabled && capabilities.hasTransport(WIFI)`) | Wi-Fi is required to be the transport; with no NetworkManager the Linux equivalent is unknown | downloads could start on metered connections when the state is unknown | unknown Wi-Fi state blocks "only on Wi-Fi" downloads (conservative) | TC-53-16 | T-53a-026 |
| E-16 | `MainActivity.kt:564` (`else -> return false`, unhandled actions leave `ready` false without message); `DeepLink*ScreenModel` silent stop | errors are silent | the user sees a blank or stuck screen | every route error and resolve error produces a toast or an error panel | TC-53-04 | T-53a-045 |
| E-17 | `DeepLinkMangaActivity.kt:16` sets `type=MANGA` for `eu.kanade.tachiyomi.ANIMESEARCH` (manifest, manga activity filters); `MainActivity.kt:534` routes that action to the anime search | the kind parameter is ignored for the action, the forwarded `type` is misleading | a manga-side link opens anime search | `ExtSearch` carries its own kind; no type is consulted | TC-53-02 | T-53a-005 |
| E-18 | `MainActivity.kt:475` and `:480` (`?: return false`) | `SHOW_MANGA` or `SHOW_ANIME` without the id returns false, the window is not ready and no message is shown | a broken shortcut shows nothing | `RouteError::MissingId`, toast, and the default tab is shown | TC-53-04 | T-53a-045 |
| E-19 | `Notifications.kt:117` (`ID_DOWNLOAD_IMAGE = 2`) and `:184` (`ID_APP_UPDATE_PROMPT = 2`) | two different notices share id 2 | the save-image notice replaces the update prompt or the reverse | stable distinct string ids (`notice:save-image`, `notice:app-update-prompt`) | TC-53-11 | T-53a-020 |
| E-20 | `TorrentServerService.kt:124` (`"Stop"` literal) | an English literal in the UI | cannot be translated (area:51) | string resource key with English value "Stop" | TC-53-23 | T-53a-035 |
| E-21 | `AppUpdateDownloadJob.kt:90` (`100 * (bytesRead.toFloat() / contentLength)`) | `contentLength` 0 or -1 (chunked response) gives Infinity or a negative percent; `toInt()` of Infinity is `Int.MAX_VALUE` | the notice progress jumps to an invalid value | `ProgressThrottle::offer` returns None when total is 0; with total -1 (unknown) show indeterminate progress | TC-53-13 | T-53a-044 |
| E-22 | `TrackLoginActivity.kt:20` (regex `(?:access_token=)(.*?)(?:&)`) | the token must be followed by `&`; a fragment with the token last fails and the user is logged out | a valid AniList login is treated as logout | parse the fragment as form data; the token is the value of `access_token` | TC-53-15 | T-53a-009 |
| E-23 | `TrackLoginActivity.kt:9` to `:17` (`when (data?.host)` with no `else`) | an unknown host leaves the loading screen forever | stuck screen | `RouteError::UnknownHost` toast and return to Settings | TC-53-03 | T-53a-009 |
| E-24 | `shortcuts.xml` (two shortcuts with `android:shortcutId="show_recently_updated"`) | duplicate id | the history shortcut is not reliably listed | distinct ids; history shortcut `show-history` | TC-53-19 | T-53a-011 |
| E-25 | `AppUpdateDownloadJob.kt:42` (title default `MR.strings.app_name`) and `NotificationReceiver.kt:880` (title extra `DOWNLOAD_TITLE` set by the builder but the receiver branch at `:283` to `:286` does not read it) | title passed but ignored | the notice always shows the default title | the title is carried in `StartAppUpdate { title }` and used | TC-53-18 | T-53a-034 |
| E-26 | `HttpServerService.kt:108` (`NAME = "HttpServerSerivce"`, misspelled) | the misspelling is part of persisted intent action and extra names | the identifiers cannot be corrected in Android without breaking running services | Linux uses plain names (`http-server`) | none (no persisted state) | T-53a-035 |
| E-27 | `NotificationReceiver.kt:104` to `:118` (`ACTION_OPEN_CHAPTER`, `ACTION_OPEN_EPISODE`) and `:400` (`ACTION_OPEN_ENTRY`) | dead branches; no builder sends these actions | confusing code; `openEpisode` reads the manga key for the episode (`:113` to `:116`) | not ported | TC-53-24 (grep test) | T-53a-017 |
| E-28 | `NotificationReceiver.kt:95` (`ACTION_CANCEL_RESTORE`) with `EXTRA_NOTIFICATION_ID` set by `cancelRestorePendingBroadcast` (`:986` to `:997`) but not read | the notice itself is not withdrawn by this action | the restore notice stays after cancel until the job posts its own | the cancel action withdraws the restore notice id | TC-53-25 | T-53a-017 |
| E-29 | `MangaExtensionInstallActivity.kt:34` to `:37` (MIUI 1 s ignore window) | timing-based heuristic | fragile across devices | not applicable (APK install dropped) | none | T-53a-036 |
| E-30 | `NotificationReceiver.kt:306` to `:311` (`removeAfterMarkedAsRead`): for each chapter the manga is fetched again (`getManga.await(mangaId)` inside `map`) | N extra reads for N chapters | slow for large batches | fetch manga once per batch | TC-53-26 | T-53a-043 |
| E-31 | `MangaLibraryUpdateNotifier` and `AnimeLibraryUpdateNotifier` use `entry.id.hashCode()` as the notification id (`NotificationReceiver.kt:619`, `:640`, `:684`, `:705`, `:754`, `:781`, `:802`, `:821`) for both kinds | a manga with id 7 and an anime with id 7 produce the same notice id in the same tray | one notice replaces the other | ids include the kind: `notice:entry:<kind>:<id>` | TC-53-11 | T-53a-020 |
| E-32 | `NotificationReceiver.kt:662` and `:727` (`EXTRA_NOTIFICATION_ID = anime.id.hashCode()`) and `dismissNotification(context, notificationId, groupId)` (`:554` to `:585`) | the group id is a constant (`ID_NEW_*`), the entry id is per entry; both are matched through `activeNotifications` | dismiss relies on the platform's active list, not on ids the app owns | the app keeps its own id registry; withdraw is by id | TC-53-07 | T-53a-019 |
| E-33 | `AppUpdateDownloadJob.kt:117` to `:123` (classification of errors: `CancellationException` or `StreamResetException` with CANCEL → cancel the notice; otherwise error notice) | a timeout is shown as "Download error" with Retry | correct, but a cancel by the user from the notice goes through the job cancel path, and a cancel by REPLACE also withdraws the notice of the new download | the REPLACE path does not withdraw the new download's notice | TC-53-27 | T-53a-034 |

## 2. Race conditions and concurrency edges

| ID | Scenario | Risk | Linux rule | Test |
|---|---|---|---|---|
| R-01 | Two notice buttons pressed quickly (mark read, then download) for the same entry | both run; the second sees chapters already read | the actions are idempotent: marking read twice writes the same value; download of an already queued chapter is a no-op (area:23) | TC-53-28 |
| R-02 | App-update "Download" pressed twice | two downloads for the same URL | `run_unique(Replace)` cancels the first and starts the second (Kotlin REPLACE) | TC-53-29 |
| R-03 | Window closed while a torrent server runs | the server stops with the process (Android service survives) | `BackgroundHold` keeps the process alive while the server runs; Stop releases it | TC-53-30 |
| R-04 | Notice posted and withdrawn in the same tick | the withdraw happens before the post is accepted | the notifier serialises post and withdraw per id (a map of pending ids on the main thread) | TC-53-31 |
| R-05 | Command-line request arrives before the window exists | request lost | queued in `connect_command_line` and drained after `connect_activate` | TC-53-32 |
| R-06 | Second process starts while the first is loading its library | two windows | GApplication forwards the request to the primary instance; no second window | TC-53-33 |
| R-07 | Resolve running while the user navigates away | the page pops, then the result pushes a page | the resolve result is dropped when the loading page is no longer on the stack (`Weak` reference check) | TC-53-34 |
| R-08 | Progress notices faster than 200 ms | desktop floods | `ProgressThrottle` (`p1-rust-design.md` section 2.5) | TC-53-13 |
| R-09 | App-update download finishes while the window is closed | notice has no window to open | the notice's "Show in folder" action works through the portal; `hold()` was active during download | TC-53-30 |
| R-10 | Incognito toggled on and off rapidly | the withdraw of the incognito notice races the post | last state wins: the notice reflects the stored preference after each change event, and withdraws when false | TC-53-35 |

## 3. Edge cases by input

### 3.1 Launch arguments and URIs

| ID | Input | Expected |
|---|---|---|
| EC-01 | `kurobayomi` (no args) | activate; library tab |
| EC-02 | `--action=show-anime` (no id) | `RouteError::MissingId`, toast "Missing id for this shortcut" (Proposal string `route_missing_id`) |
| EC-03 | `--action=show-anime --id=42` | anime entry 42 after `PopToRoot` |
| EC-04 | `--action=unknown` | `LaunchError::Unrecognised`, message on stderr and toast |
| EC-05 | `--search=""` | no request (area:36 `Search { "" }` → `[]`) |
| EC-06 | `--search=naruto --type=manga` | global manga search for "naruto" then the manga deep-link page |
| EC-07 | `--search=naruto --type=comic` | `RouteError::UnknownKind("comic")`, toast; the window opens on the default tab |
| EC-08 | `--search=naruto` (no type) | anime (default, area:36 and behaviour 2.2) |
| EC-09 | `/home/u/Downloads/backup.tachibk` | `OpenFile`, restore backup screen (area:22) |
| EC-10 | `/home/u/Downloads/backup.tachibk.part` | `Unrecognised` (not a backup; no crash) |
| EC-11 | `https://example.test/anime/1` as argument | `Search` with the URL as text; resolver runs (section 2.8 of behaviour) |
| EC-12 | `kurobayomi://add-repo?url=https%3A%2F%2Frepo.example.test%2Findex.min.json` | manga extension repos page with the decoded URL |
| EC-13 | `tachiyomi://add-repo?url=...` | manga extension repos (legacy alias) |
| EC-14 | `aniyomi://extension-store?url=...` | anime extension stores |
| EC-15 | `aniyomi://add-repo?url=...` | anime extension stores (legacy) |
| EC-16 | `kurobayomi://add-repo` (no url) | `route_request` returns no navigation; toast "Missing repository address" (Proposal) |
| EC-17 | `ftp://x/y` | `UnsupportedScheme`, toast |
| EC-18 | `kurobayomi://unknown-host` | `UnknownHost`, toast |
| EC-19 | `kurobayomi://anilist-auth#access_token=abc&token_type=Bearer` | tracker OAuth route with token `abc` |
| EC-20 | `kurobayomi://anilist-auth#access_token=abc` (token last, no `&`) | token `abc` (fixes E-22) |
| EC-21 | `kurobayomi://bangumi-auth?code=xyz` | tracker OAuth, code `xyz` |
| EC-22 | `kurobayomi://simkl-auth` (no code) | logout branch (Kotlin behaviour, area:27) |
| EC-23 | `kurobayomi://myanimelist-auth?code=%20x` | code is decoded with form decoding; a space in the code is `" x"` (not trimmed) |
| EC-24 | argument with spaces and non-ASCII (`/home/u/Descargas/Copia de seguridad.tachibk`) | `OpenFile` with the exact path (no re-encoding) |

### 3.2 Notices

| ID | Input | Expected |
|---|---|---|
| EN-01 | `Dismiss { notice: "notice:app-update-prompt" }` | only that id is withdrawn |
| EN-02 | payload with 501 URLs | `PayloadError::TooManyUrls(501)`, log, no database write |
| EN-03 | payload with a 2049-byte URL | `PayloadError::TooLarge` for the url (`MAX_URL_BYTES`) |
| EN-04 | `StartAppUpdate { url: "http://..." }` | `BadScheme` (https only) |
| EN-05 | `RevealFile { path: "/etc/passwd" }` | `OutsideRoots` |
| EN-06 | `RevealFile { path: "relative/x" }` | `NotAbsolute` |
| EN-07 | malformed JSON in the GAction target | logged, no action; no crash |
| EN-08 | `MarkChaptersRead` with a URL that matches no chapter | chapters not found are skipped (Kotlin `mapNotNull`); the notice is withdrawn; no error toast |
| EN-09 | `MarkChaptersRead` with `removeAfterMarkedAsRead = true` | downloads of those chapters are deleted once each (area:23) |
| EN-10 | desktop notification server absent | `server_available() == false`; the same text is shown as a toast |
| EN-11 | category switch off | notice not posted; toast not shown either (the switch means "do not disturb") |
| EN-12 | three buttons | all three shown; a fourth button is an error `PayloadError` at construction (Proposal cap) |

### 3.3 Downloads and progress

| ID | Input | Expected |
|---|---|---|
| ED-01 | progress 1, 1, 2 at 0 ms, 100 ms and 300 ms | notice 1 at 0 ms (first update is always due); the second 1 is not greater than saved; notice 2 at 300 ms (more than 200 ms after the last notice) |
| ED-02 | progress 50 then 49 (decreasing) | 49 never posted |
| ED-03 | total 0 | None (no division by zero, E-21) |
| ED-04 | total -1 (unknown length) | indeterminate notice text, no percent |
| ED-05 | progress 100 | posted once, then the download completes and the notice is withdrawn |
| ED-06 | cancel during progress | the cancel is a REPLACE or user cancel; the notice is withdrawn (E-33) |

### 3.4 Desktop integration files

| ID | Check | Expected |
|---|---|---|
| EF-01 | `.desktop` file validity | `desktop-file-validate` passes (no errors) |
| EF-02 | `.desktop` actions | five `Desktop Action` groups, each with `Name` and `Exec` |
| EF-03 | `MimeType` | contains `application/x-tachibk`, `x-scheme-handler/kurobayomi`, `x-scheme-handler/aniyomi`, `x-scheme-handler/tachiyomi` |
| EF-04 | `DBusActivatable=true` | present |
| EF-05 | MIME XML | `xmllint --noout` passes; glob `*.tachibk` |
| EF-06 | notification ids | all ids in `NoticeId` constants satisfy the invariant (ASCII, length, charset) |
| EF-07 | SearchProvider `.ini` | `Version=2`, `BusName`, `ObjectPath` and `DesktopId` present (P2) |

## 4. Fixtures

| Fixture file (kuro-testkit) | Content |
|---|---|
| `fixtures/argv/*.txt` | one argument list per line, used by TC-53-01 |
| `fixtures/uri/*.txt` | one URI per line with the expected route, used by TC-53-02, TC-53-03, TC-53-14 |
| `fixtures/notice/*.json` | serialised `AppAction` payloads (valid and invalid), used by TC-53-04, TC-53-05 |
| `fixtures/progress/*.csv` | `(bytes, total, millis)` sequences with the expected notices, used by TC-53-13 |
| `fixtures/desktop/io.github.blackbox.Kurobayomi.desktop` | golden copy of the desktop entry (`p1-rust-design.md` section 2.6) |
| `fixtures/mime/application-x-tachibk.xml` | golden copy of the MIME XML |
| `fixtures/resolve/anime/*.json` and `fixtures/resolve/manga/*.json` | fake source responses: `getUriType`, `getAnime`, `getEpisode`, episode lists (kind, url, id) |
| `fixtures/network/*.json` | `NetworkState` samples: connected+wifi, connected+ethernet, metered, no NM, offline |
| `fixtures/shortcuts/*.txt` | the five shortcut ids and labels, used by TC-53-19 |

## 5. Tests to write

Each test ID is referenced from sections 1 to 3. "Type" is unit (kuro-core), integration (kuro-platform with the system bus), golden (file comparison), or manual (device or desktop check).

| ID | Type | What it proves | Input | Expected |
|---|---|---|---|---|
| TC-53-01 | unit | argv parsing for every flag and shortcut | fixtures/argv | matching `LaunchRequest` list or `LaunchError` |
| TC-53-02 | unit | route table of area:36 reproduced (including E-17 fix) | fixtures/uri and argv | `route_request` outputs per table in `p1-data-and-config.md` 4.2 |
| TC-53-03 | unit | unknown kind, unknown scheme, unknown host are errors (E-14, E-18, E-23) | EC-07, EC-17, EC-18 | `RouteError` or `LaunchError`, no panic |
| TC-53-04 | unit | every error produces a user message (E-16) | all error variants | message key exists in the strings table |
| TC-53-05 | unit | payload missing field returns error, never panics (E-04) | fixtures/notice invalid | `PayloadError` or serde error |
| TC-53-06 | integration | no notification server → `server_available() == false` and toast path (E-12) | session bus without notification service (test bus) | `false` |
| TC-53-07 | unit | Dismiss withdraws only its id (E-01, E-32) | `RecordingNotifier` | exactly one withdraw call, no bulk call |
| TC-53-08 | unit | dispatch never blocks the main thread (E-03) | slow fake repository (sleep 200 ms) | dispatch returns in under 5 ms |
| TC-53-09 | unit | payload identity: two notices with different URLs keep their own payloads (E-05) | two `DownloadChapters` payloads | each button's action equals its own payload |
| TC-53-10 | manual | group summary not used; single id dismissed (E-06) | desktop session with two notices | one notice withdrawn, the other stays |
| TC-53-11 | unit | notice ids are distinct across kinds and features (E-19, E-31) | `NoticeId::entry(Manga, 7)` vs `entry(Anime, 7)` vs `SAVE_IMAGE` vs `APP_UPDATE_PROMPT` | all distinct |
| TC-53-12 | unit | torrent notice has no delete action (E-08) | notice built by the torrent service | `default_action` and buttons contain only Stop |
| TC-53-13 | unit | progress throttle sequences (E-21, ED-01 to ED-04) | fixtures/progress | expected Some/None sequence |
| TC-53-14 | unit | `.tachibk` detection after decoding (E-11, EC-09, EC-10, EC-24) | URIs and paths | `OpenFile` only for `Path::extension == tachibk` |
| TC-53-15 | unit | OAuth fragment parse (E-22, EC-19, EC-20) | fragments | token extracted in both cases |
| TC-53-16 | unit | Wi-Fi unknown blocks "only on Wi-Fi" (E-15) | `FixedNetwork` with NM absent | `is_wifi == false`, download gated |
| TC-53-17 | unit | categories persist; no channel deletion at start (E-02) | preferences store | category switches unchanged after restart |
| TC-53-18 | unit | update path is absolute; title is used (E-13, E-25) | `Paths` fake returning relative path | error, no write; title in notice |
| TC-53-19 | golden | shortcut ids are unique (E-24) | fixtures/shortcuts | five distinct ids |
| TC-53-20 | unit | resolver error becomes `Failed` state, not hang (E-07) | fake source that errors | state `Failed { reason }` |
| TC-53-21 | unit | episode resolve goes through the player router (E-09) | fake anime source with episode | router called, not `PlayerActivity` directly |
| TC-53-22 | unit | navigation effect runs once per state (E-10) | state sequence Loading, Result, Result | one push |
| TC-53-23 | unit | torrent "Stop" comes from a string key (E-20) | key lookup | key present in the string table |
| TC-53-24 | grep | no dead action constants in Rust (E-27) | `rg "OpenChapter\|OpenEpisode\|OpenEntry" src kuro-core` | matches only the enum definitions used by builders |
| TC-53-25 | unit | cancel restore withdraws the restore notice id (E-28) | `CancelRestore` | withdraw `RESTORE_PROGRESS` id |
| TC-53-26 | unit | batch mark-read loads manga once (E-30) | 50 urls | one manga fetch |
| TC-53-27 | unit | REPLACE does not withdraw the new download's notice (E-33) | start A, start B | notice of B remains |
| TC-53-28 | unit | double mark-read is idempotent (R-01) | same payload twice | same database state |
| TC-53-29 | unit | REPLACE cancels the first job (R-02) | `run_unique` twice | first token cancelled, one job remains |
| TC-53-30 | integration | hold keeps the process alive while a job runs (R-03, R-09) | fake job 1 s | `active()` contains the label until done |
| TC-53-31 | unit | post and withdraw in one tick are ordered (R-04) | sequence | final state matches the last call |
| TC-53-32 | unit | command-line before activate is queued (R-05) | queue drain | request applied once after window creation |
| TC-53-33 | manual | second launch is forwarded, no second window (R-06) | `kurobayomi --action=show-updates` while running | one window, Updates tab selected |
| TC-53-34 | unit | resolve result after pop is dropped (R-07) | cancel after pop | no push |
| TC-53-35 | unit | incognito last-state-wins (R-10) | toggles on, off, on | notice present exactly when value is true |
| TC-53-36 | golden | `.desktop` file matches the golden copy and validates (EF-01 to EF-04) | fixtures/desktop | byte equality and `desktop-file-validate` exit 0 |
| TC-53-37 | golden | MIME XML matches golden and validates (EF-05) | fixtures/mime | byte equality and `xmllint --noout` exit 0 |
| TC-53-38 | unit | resolve anime: local hit, network insert, episode fetch (behaviour 3.1) | fixtures/resolve/anime | expected Resolved values |
| TC-53-39 | unit | resolve manga: chapter list sync with the `false` flag (behaviour 3.2) | fixtures/resolve/manga | `sync` called with `false` |
| TC-53-40 | manual | notification click starts a closed app (DBusActivatable, pitfall 1) | close app, click notice | window opens on the target |
| TC-53-41 | manual | `Show in folder` opens the file manager at the file (FileManager1) | RevealFile on a real file | file manager highlights the file |
| TC-53-42 | manual | Stack transition duration and disabled animations (section 3 of UI parity) | toggle `gtk-enable-animations` | immediate switch when disabled |
| TC-53-43 | integration | NetworkManager properties read; unknown property treated as unknown (pitfall 7) | fake D-Bus service | `is_wifi` true with wireless primary; unknown property yields false |

## 6. Failure modes that must be visible

| Failure | Where it shows (Linux) | Retry |
|---|---|---|
| Route error (argv or URI) | toast on the window | none (user fixes the input) |
| Resolve error | error panel on the loading page | Retry button |
| Payload rejected | log warning and toast "This action is no longer valid" (Proposal string `notice_invalid`) | none |
| Notification server absent | toast with the notice text | none |
| App update download error | notice with Retry (area:35) | Retry |
| Torrent or HTTP server failed to start | toast "Failed to start http server" (AYMR `http_server_start_failure`) | user retries from settings |
| Folder chooser cancelled | no change | none |
| Portal refused open | toast "Could not open the file" (Proposal string `open_failed`) | none |

## 7. Coverage checklist for implementers

- Every row of `p1-behavior.md` section 4.2 has a matching `AppAction` variant or a recorded drop (section 5 of data file).
- Every notice id in `p1-data-and-config.md` section 3.3 has a `NoticeId` constant or is recorded as dropped (channels) or as a fixed per-entry id.
- Every route row of `p1-data-and-config.md` section 4.2 has a fixture in `fixtures/uri` or `fixtures/argv`.
- Every E-nn in section 1 has a test ID in the Test column and a task ID.
