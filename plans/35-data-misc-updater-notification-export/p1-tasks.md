# Area 35 - Notifications, app updater, library export, misc data services: implementation tasks (part 1)

Task ids are `T-35a-NNN`. Blocks: 001-031 notification platform, routing and actions (P0 TLS item 031); 040-062 app updater; 070-077 library CSV export; 080-087 notification categories and id registry for other areas; 090-098 tests; 063-065 and 012, 025-030 drops with reasons.

Conventions: Priority P0 = needed for daily use; P1 = parity most users rely on; P2 = long tail; P3 = Android-only with a drop or replacement decision stated. Effort S <= 1 day, M <= 3 days, L <= 1.5 weeks.

Rust module and file names are the ones in p1-rust-design.md. Behaviour and data references are in p1-behavior.md and p1-data-and-config.md. Every task lists its repo-relative Kotlin sources; new Rust files are written under `kurobayomi` or `crates/kuro-platform`.

---

### T-35a-001 — kuro-platform Notifier trait, Notice model and Category enum
- Priority: P1
- Effort: M
- Depends: T-req-platform-004, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt
- Acceptance:
  - `crates/kuro-platform/src/notifier.rs` defines `Category` (14 variants as in p1-data-and-config.md section 3.2), `Priority`, `NoticeId`, `NoticeAction`, `Notice`, `NotifyError` and the `Notifier` trait exactly as p1-rust-design.md section 2.1.
  - The crate compiles with `--no-default-features` and with `platform-linux` enabled; no `cfg(target_os)` appears outside the crate (addenda rule 1).
  - Unit test: `Notice` is `Send` (compile-time assertion).

### T-35a-002 — GioNotifier: post and withdraw through gtk::Application
- Priority: P1
- Effort: M
- Depends: T-35a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - `crates/kuro-platform/src/linux/notifier.rs` implements `Notifier` with `gio::Notification`, `send_notification(Some(id), ...)` and `withdraw_notification(id)` on the main context.
  - A notice with a default action and two buttons is sent; `gdbus monitor --session` shows the `Notify` call on KDE Plasma (manual check recorded in the PR).
  - Calls from a tokio thread are marshalled (`glib::MainContext::default().invoke`); no GTK object crosses threads (code review check).
  - When `app` is gone (`WeakRef` upgrade fails), `show` returns `NotifyError::Unavailable`.

### T-35a-003 — Notice validation: button limit and content rules
- Priority: P1
- Effort: S
- Depends: T-35a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt
- Acceptance:
  - `Notice` with 4 buttons is rejected with `NotifyError::TooManyButtons` before any call to the service (unit test).
  - Title and body are escaped for `&`, `<`, `>` before sending when the server supports markup (unit test with `a & <b>`).
  - Body longer than 200 characters is truncated with an ellipsis (unit test; PROPOSAL rule).

### T-35a-004 — Category registry and per-category enable preferences
- Priority: P1
- Effort: M
- Depends: T-35a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - `src/notify/mute.rs` reads `notify.enabled.<category>` (bool, default true) for every category except `AppUpdate`, which is governed by `update.auto_check` and the updater feature.
  - `Notifier::show` returns `Ok(false)` without a service call when the category is disabled (fake-notifier test).
  - The settings screen has one switch per category (labels from p1-ui-parity.md section 8); the change applies without restart.
  - The 13 Android channels map to the 14 categories as listed in p1-data-and-config.md section 3.2 (table test).

### T-35a-005 — Notice id registry with the Android id collision fixed
- Priority: P1
- Effort: S
- Depends: T-35a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt
- Acceptance:
  - `src/notify/registry.rs` exposes the id functions and `ALL_IDS` with the 22 ids of p1-rust-design.md section 3.2 plus `save-image`.
  - Test: `ALL_IDS` has no duplicates and contains `save-image` and `app-update.install` as different values (fixes the Android id 2 collision, behaviour 1.4).
  - Every producer area uses these constants; grep check finds no integer notification ids in `src/`.

### T-35a-006 — AppAction codec and the app.notice GAction router
- Priority: P1
- Effort: M
- Depends: T-35a-003, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `src/notify/actions.rs` defines `AppAction` (every variant in p1-rust-design.md section 3.1) with serde JSON, `to_target`, `from_target` and `MAX_PAYLOAD_BYTES` = 1 MiB.
  - `src/notify/router.rs` registers `gio::SimpleAction` named `notice` with parameter type `s` on the `gtk::Application` before the window is shown (src/main.rs build_ui).
  - A GVariant string activation with valid JSON dispatches; invalid JSON logs a warning and does nothing (no panic).
  - Shortcut mapping table from behaviour spec 1.7 is covered by a test (each Constants value maps to one variant or tab).

### T-35a-007 — Withdraw-only dismissal and the cancel-all guard
- Priority: P1
- Effort: S
- Depends: T-35a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `AppAction::Dismiss { notice }` calls `Notifier::withdraw(&notice)` only (unit test with fake notifier records one withdraw and no other events).
  - `cancelAll()` / `withdraw_all` does not occur in `src/` or `crates/` (CI grep check, fails the build).
  - Actions with `notice: Some(id)` withdraw that id before the work starts (order test).

### T-35a-008 — ProgressThrottle with the Android 200 ms and strictly-increasing rule
- Priority: P1
- Effort: S
- Depends: T-35a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - `src/notify/throttle.rs` implements `ProgressThrottle::offer` as in p1-rust-design.md section 3.4.
  - Unit tests in p1-edge-cases-and-tests.md section 5.3 pass (sequence 10, 20 at 100 ms, 20 at 200 ms, 20 at 201 ms, 20 at 300 ms, 21 at 450 ms).
  - `total` None or 0 never emits.

### T-35a-009 — Hide-content gate for private notices
- Priority: P1
- Effort: S
- Depends: T-35a-003, area:02-preferences-system
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt
- Acceptance:
  - Preference key `notify.hide_content` (bool, default false) is read by `GioNotifier` through an injected closure.
  - With the setting on, a notice with `private_content = true` returns `Ok(false)` and no service call happens (unit test).
  - Update notices (`private_content = false`) still show with the setting on (test).

### T-35a-010 — GioOpener: open URL, open file and reveal in file manager
- Priority: P1
- Effort: S
- Depends: T-req-platform-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `crates/kuro-platform/src/linux/opener.rs`: `open_url` accepts only `http`/`https`; `reveal_file` calls `org.freedesktop.FileManager1.ShowItems` through zbus (blocking) and falls back to `open_file(parent)` on error.
  - Manual check on KDE Plasma: reveal selects the file in Dolphin; with no file manager running the parent folder opens.
  - Unit test: `open_url("file:///etc/passwd")` returns `OpenError::NoHandler`.

### T-35a-011 — Application id, desktop file and notification service check at startup
- Priority: P1
- Effort: S
- Depends: T-35a-002, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - `gtk::Application` is built with an application id (PROPOSAL `dev.kurobayomi.Kurobayomi`) and a desktop file ships with `Name=Kurobayomi` and the same id.
  - At startup the app checks that `org.freedesktop.Notifications` exists on the session bus (zbus NameHasOwner in kuro-platform) and logs once if not.
  - The `setupNotificationChannels` equivalent is replaced by this startup check (no channel creation); the function logs but never fails startup.

### T-35a-012 — Drop: channel creation and deprecated-channel cleanup
- Priority: P3
- Effort: S
- Depends: T-35a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - Decision recorded in p1-behavior.md section 1.1 and this task: no OS channels exist on freedesktop; the nine deprecated channel ids have no Linux meaning.
  - No code calls `createNotificationChannel`-equivalent; the user loses per-channel OS settings (replaced by T-35a-004 switches).
  - Startup logs nothing about channels.

### T-35a-013 — Open downloads actions (manga and anime)
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:47b-settings-widgets-search-more
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `OpenMangaDownloads` and `OpenAnimeDownloads` switch to the More tab with the downloads view (matching `HomeScreen.Tab.More(toDownloads = true)` in area:36-ui-shell-navigation-security-webview).
  - Both pop any sub-screen before switching (test on the navigation model).

### T-35a-014 — Open chapter and episode actions with the missing-row toast
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:43-reader-core, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `OpenChapter { manga_id, chapter_id }` loads both rows on tokio and opens the reader; no `block_on` on the GTK thread (grep check).
  - Either row missing => toast with `notify.error.item_missing` ("Error") and no navigation (test N9).
  - `OpenEpisode` does the same with the player.

### T-35a-015 — Open manga and anime entry actions with notice dismissal
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `OpenManga` and `OpenAnime` navigate to the entry (library tab with id, pop to root first) and withdraw `notice` when present.
  - Id <= 0 is rejected (no navigation, log warning); the Android id-0 quirk (behaviour 1.7 note) is not reproduced (test).

### T-35a-016 — Mark chapters read action
- Priority: P1
- Effort: M
- Depends: T-35a-006, area:08-domain-library-category-history-updates, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt
- Acceptance:
  - `MarkChaptersRead` withdraws `notice`, looks up each URL with (url, manga_id), and sets `read = true` with a partial update (no other columns written; test N10 keeps `last_page_read`).
  - When the download-after-read preference is on and the source exists, the chapter download is deleted; when the source is missing the chapter is still marked read (test N11).
  - Lookup misses are skipped; empty URL list is a no-op after dismissal (test N12).

### T-35a-017 — Mark episodes seen action
- Priority: P1
- Effort: M
- Depends: T-35a-006, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt, domain/src/main/java/tachiyomi/domain/items/episode/model/EpisodeUpdate.kt
- Acceptance:
  - `MarkEpisodesSeen` mirrors T-35a-016 with `seen = true` and the episode table; `last_second_seen` is untouched (test).
  - Anime id is taken from the `anime_id` field of the payload (not from a field named for manga) (test).

### T-35a-018 — Download chapters action
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `DownloadChapters` withdraws `notice`, loads the manga (return if missing) and the chapters by URL, and enqueues them with the manga download manager (test with fake manager).
  - No toast on any failure (parity).

### T-35a-019 — Download episodes action
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `DownloadEpisodes` mirrors T-35a-018 with the anime download manager.
  - Missing anime => no enqueue (test).

### T-35a-020 — Queue control actions (resume, pause, clear) for manga and anime
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Six actions call `start`, `pause` and `clear` on the matching manager (manga or anime) (test with fake managers).
  - The download notices offering these buttons come from area:23-downloads-manga and area:24-downloads-anime and use these exact `AppAction` variants.

### T-35a-021 — Cancel library update actions (manga and anime)
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `CancelLibraryUpdate` and `CancelAnimeLibraryUpdate` stop the matching job (test with fake job handle).
  - The notice is withdrawn when the action carries a notice id.

### T-35a-022 — Cancel restore action
- Priority: P1
- Effort: S
- Depends: T-35a-006, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `CancelRestore` calls the restore job's stop (test) and withdraws the restore progress notice id.

### T-35a-023 — Share image becomes reveal in file manager
- Priority: P2
- Effort: S
- Depends: T-35a-010, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - The save-image notice (id `save-image`) offers "Open folder" => `RevealFile { path }` instead of Android share.
  - The reveal path is validated against allowed roots (design 6.1) (test: path outside downloads root rejected).

### T-35a-024 — Backup share becomes reveal in file manager
- Priority: P2
- Effort: S
- Depends: T-35a-010, area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
  - The backup-complete notice offers "Open folder" => `RevealFile` of the backup file (path inside the configured backup folder).
  - The Android MIME `application/x-protobuf+gzip` is not used (no share chooser) (grep check).

### T-35a-025 — Drop: the unused ACTION_OPEN_ENTRY
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - Decision recorded: the constant is declared at NotificationReceiver.kt:400 but never referenced; no `AppAction` variant is created for it.
  - Entry opening is covered by T-35a-015 (no user loss).

### T-35a-026 — Drop: group-summary dismissal logic
- Priority: P3
- Effort: S
- Depends: T-35a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - Decision recorded: desktop notices have no group summary in GIO; the Android branch (NotificationReceiver.kt:573-582) that cancels only the summary when two notifications are active is not ported.
  - User loss stated: none visible; the defect (single notice left behind) cannot occur.

### T-35a-027 — Drop: Samsung S Pen remote actions
- Priority: P3
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/res/xml/s_pen_actions.xml, app/src/main/AndroidManifest.xml
- Acceptance:
  - Decision recorded: the `remote-actions` protocol is Samsung hardware only and is not implemented.
  - User loss stated: S Pen remote gestures are gone; the five actions remain available as keyboard keys (T-35a-028).

### T-35a-028 — Keyboard equivalents for the five S Pen actions in reader and player
- Priority: P2
- Effort: S
- Depends: area:44-reader-viewers, area:45-player-core
- Sources: app/src/main/res/xml/s_pen_actions.xml, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `N` = next chapter/episode, `P` = previous chapter/episode in both reader and player (gtk::ShortcutController test).
  - Back = window back navigation (Alt+Left and Backspace in the reader).
  - Page-next and page-previous use the reader's existing arrow keys (no new binding).

### T-35a-029 — Drop or replace searchable.xml (global search hint and search provider)
- Priority: P3
- Effort: S
- Depends: area:40-ui-browse-manga, area:41-ui-browse-anime
- Sources: app/src/main/res/xml/searchable.xml, app/src/main/AndroidManifest.xml
- Acceptance:
  - Decision recorded: Android system search is not available; the in-app search entry with placeholder "Global search…" (`action_global_search_hint`) is provided by areas 40/41.
  - A GNOME/KDE search provider (`org.gnome.Shell.SearchProvider2`) is an optional extra, not part of v1; the decision is stated here.

### T-35a-030 — Drop: FileProvider paths (provider_paths.xml)
- Priority: P3
- Effort: S
- Depends: T-35a-010
- Sources: app/src/main/res/xml/provider_paths.xml, app/src/main/AndroidManifest.xml
- Acceptance:
  - Decision recorded: Linux shares plain file paths; no content URIs are created.
  - Grep test: no `FileProvider` or `content://` string in `src/`.

### T-35a-031 — TLS and cleartext configuration for the HTTP client (replaces network_security_config.xml)
- Priority: P0
- Effort: S
- Depends: area:12-network-stack
- Sources: app/src/main/res/xml/network_security_config.xml, app/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt
- Acceptance:
  - reqwest client uses `rustls-tls-native-roots` so system CAs are trusted; http:// URLs are not refused by the client (test with local http server).
  - Decision recorded: the Android XML is Dropped; its effect (cleartext allowed, system and user CAs trusted) is reproduced by reqwest defaults plus native roots.

### T-35a-040 — Release and GitHub DTO models
- Priority: P1
- Effort: S
- Depends: area:12-network-stack
- Sources: data/src/main/java/tachiyomi/data/release/GithubRelease.kt, domain/src/main/java/tachiyomi/domain/release/model/Release.kt, data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - `src/update/model.rs` defines `Release`, `UpdateCheck`, `UpdateError` (p1-rust-design.md section 8.1) and the DTOs `GithubRelease`, `GithubAsset` with serde names `tag_name`, `body`, `html_url`, `assets`, `browser_download_url`.
  - Deserialising the captured fixture (p1-edge-cases-and-tests.md section 6) succeeds; a null `body` yields an empty string.

### T-35a-041 — Version parsing and is_newer (strict semver)
- Priority: P1
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - `src/update/version.rs` `parse_tag` strips one leading `v` and parses semver; `is_newer` compares strictly.
  - Regression tests E1, E2, E3, E4, E6 from p1-edge-cases-and-tests.md section 5.1 pass.
  - Non-semver input returns `BadVersion` and never panics.

### T-35a-042 — fetch_latest: GitHub releases/latest with status mapping
- Priority: P1
- Effort: M
- Depends: T-35a-040, area:12-network-stack
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - `src/update/check.rs` `fetch_latest` requests `https://api.github.com/repos/{owner}/{repo}/releases/latest` with the headers in p1-data-and-config.md section 5.
  - Status mapping: 404 => `NoReleases`; 403 and 429 => `RateLimited`; other non-2xx => `Http(code)`; transport error => `Network`.
  - `repo` must match `[A-Za-z0-9._-]+/[A-Za-z0-9._-]+`; otherwise `Disabled` (test).

### T-35a-043 — pick_asset: architecture tiers and extension preference
- Priority: P1
- Effort: S
- Depends: T-35a-040
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - `src/update/asset.rs` implements the two tiers of p1-rust-design.md section 8.4 with `ARCH_TOKENS` containing `x86_64`, `aarch64`, `x86`, `armeabi-v7a`, `arm64-v8a`.
  - Tests in p1-edge-cases-and-tests.md section 5.1 pass, including the captured Aniyomi asset list (x86 asset is not universal).
  - No match => `Ok(None)` and no timestamp change (the caller test).

### T-35a-044 — Release notes: mention rewrite, checksum strip, sha256 extraction
- Priority: P2
- Effort: S
- Depends: T-35a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt, data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - `src/update/notes.rs` `rewrite_mentions`, `strip_checksums_section` and `sha256_for` behave as in p1-rust-design.md section 8.5 and p1-edge-cases-and-tests.md section 5.2.
  - `sha256_for` parses the live table format (`| Universal | <hex>`) and the `sha256sum` format.

### T-35a-045 — Update throttle state (3 days, app state key)
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - `src/update/check.rs` `may_check` implements `now >= last + 3 days || force`; boundary test at exactly 3 days passes.
  - `last_update_check_ms` is stored under the app-state namespace (excluded from backup exports), default 0.
  - The timestamp is written only when a release was fetched (test U4 leaves it unchanged).

### T-35a-046 — Updater cargo feature and update.repo setting
- Priority: P1
- Effort: S
- Depends: area:54-build-release-distribution-docs, area:02-preferences-system
- Sources: buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt
- Acceptance:
  - Cargo feature `updater` is off by default; without it no network request is made by the update code (test: build without feature has no `update` module symbols).
  - Setting `update.repo` (string, default empty) and `update.auto_check` (bool, default true) exist and are read at startup.
  - Empty repo => `UpdateError::Disabled` and the About row shows the hint (T-35a-058).

### T-35a-047 — UpdateService::check orchestration
- Priority: P1
- Effort: M
- Depends: T-35a-041, T-35a-042, T-35a-043, T-35a-045, T-35a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt, domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - `src/update/mod.rs` `UpdateService::check(force)` follows behaviour spec 2.3 and 2.6 step by step, with a single-flight mutex (test U10).
  - Posts the update prompt notice (T-35a-048) exactly once per NewUpdate result.
  - Returns `UpdateCheck::NoNewUpdate` for an older or equal tag.

### T-35a-048 — Update available notice
- Priority: P1
- Effort: S
- Depends: T-35a-001, T-35a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt
- Acceptance:
  - Notice id `app-update.progress`, category `AppUpdate`, title "New version available!", body = version tag.
  - Default action `OpenUrl { release_url }`; buttons "Download" (`StartAppUpdate { url, version }`) and "What's new" (`OpenUrl`). Decision E-U4 recorded: Android's body-tap download is changed to open the release page.
  - Fake-notifier test checks the exact payload.

### T-35a-049 — Update screen page
- Priority: P1
- Effort: M
- Depends: area:36-ui-shell-navigation-security-webview, T-35a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt, app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt
- Acceptance:
  - Page shows heading "New version available!", subtitle version, scrollable notes without the Checksums section, buttons "Open on GitHub", "Not now", "Download" as in p1-ui-parity.md section 2.
  - "Download" starts the download task (T-35a-050) and closes the page; "Not now" closes it and changes no state.
  - Empty notes show "No release notes." (`update.no_notes`).

### T-35a-050 — Downloader: single-slot download task with stale part cleanup
- Priority: P1
- Effort: M
- Depends: T-req-platform-001, T-35a-001, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - `src/update/download.rs` `Downloader::start` cancels a running task, awaits it, then starts the new one (test: two starts leave one final file).
  - Writes `<asset>.part` under `Paths::update_dir()` and renames on success; on startup `.part` files older than 24 h are deleted (test).
  - URL must pass the host check in design 6.1 before any request (test U13).

### T-35a-051 — Download progress notice
- Priority: P1
- Effort: S
- Depends: T-35a-008, T-35a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt
- Acceptance:
  - Notice `app-update.progress`, category `AppUpdate`, priority Low; body "Downloading…" then "Downloading 42%"; button "Cancel" => `CancelAppUpdate`.
  - Title is the version when started from the prompt or the Download button; kept when started from Retry (defect E-U3 fixed by carrying `version` in `StartAppUpdate`; test N6).
  - Updates follow `ProgressThrottle` (test N7); unknown length shows no percentage (test N8).

### T-35a-052 — Error notice with ordering: withdraw progress first
- Priority: P1
- Effort: S
- Depends: T-35a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - On a failed download the fake notifier records `Withdraw(app-update.progress)` before `Show(app-update.error)` (test N5).
  - The error notice has "Retry" (`StartAppUpdate` with the same url and version) and "Cancel" (`Dismiss app-update.error`).
  - Cancel path: progress withdrawn, no error notice (test U17).

### T-35a-053 — Completed download notice and reveal action
- Priority: P1
- Effort: S
- Depends: T-35a-010, T-35a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - After rename: progress withdrawn, notice `app-update.install` with body "Downloaded <file name>" (plus " (not verified)" when no checksum), default action `RevealFile { final path }`, buttons "Open folder" and "Dismiss".
  - Replaces the Android APK install intent; the file is never executed (test: no `open_file` call on the install path).

### T-35a-054 — SHA-256 verification before reveal
- Priority: P2
- Effort: S
- Depends: T-35a-044, T-35a-050
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt, app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt
- Acceptance:
  - When `sha256` is present the downloaded file is hashed (sha2) before rename; a mismatch deletes `.part` and shows the error notice (test U16).
  - When absent the install notice says "(not verified)".

### T-35a-055 — Install kind detection (package-managed vs AppImage vs other)
- Priority: P2
- Effort: M
- Depends: T-req-platform-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt
- Acceptance:
  - `kuro-platform::install_kind()` returns `PackageManaged`, `AppImage`, or `Other` (test with fixture paths).
  - `PackageManaged` hides the download button in the update screen and the About row subtitle reads "Update with your package manager".

### T-35a-056 — Drop: APK install and in-place AppImage self-replacement
- Priority: P3
- Effort: S
- Depends: T-35a-053
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt
- Acceptance:
  - Decision recorded: APK installation has no Linux equivalent; AppImage in-place replacement needs permission and signature handling that this plan does not design.
  - User loss stated: the user replaces the file manually after "Open folder".

### T-35a-057 — Download waits for network with NetworkStatus
- Priority: P2
- Effort: S
- Depends: T-req-platform-001, T-35a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - When `is_online()` is false the progress notice says "Waiting for network" and no request is sent until `online_events()` yields true (test U19).
  - Metered connections are allowed (parity; test U20 documents it).

### T-35a-058 — About "Check for updates" row
- Priority: P1
- Effort: S
- Depends: T-35a-047, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt
- Acceptance:
  - Row "Check for updates" with a 16 px spinner while checking; the row is insensitive during a check.
  - Results: new version => update page; none => toast "No new updates available"; no asset => toast "No build for this platform in the latest release"; 404 => "No releases published yet"; rate limit => its toast; other errors => toast with the error text.
  - Disabled with hint when `update.repo` is empty (T-35a-046).

### T-35a-059 — Drop: foreground service and DATA_SYNC type
- Priority: P3
- Effort: S
- Depends: T-35a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - Decision recorded: a desktop process is not suspended by Android background limits; no foreground type is needed.
  - User loss: none.

### T-35a-060 — Drop: OsTooOld result
- Priority: P3
- Effort: S
- Depends: T-35a-041
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt
- Acceptance:
  - `UpdateCheck` has no `OsTooOld` variant; the Android check is commented out in the source (AppUpdateChecker.kt:205-208) and is not ported.
  - The toast key `update_check_eol` is not created.

### T-35a-061 — Drop: preview build channel (commit-count tags)
- Priority: P3
- Effort: S
- Depends: T-35a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt, domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - Decision recorded: Kurobayomi has one stable channel; the Aniyomi preview repository (`aniyomiorg/aniyomi-preview`, tags `r<commit count>`) is not used.
  - User loss: no preview builds through the in-app updater.

### T-35a-062 — Release notes renderer (Markdown subset)
- Priority: P2
- Effort: M
- Depends: T-35a-044, area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt
- Acceptance:
  - Renders headings, paragraphs, bullet lists, emphasis, inline code, and links (links open through `Opener::open_url`) into a `gtk::TextView` with tags styled from the app CSS.
  - Test with the captured Aniyomi body: no panic, links count equals the count in the source.

### T-35a-063 — Drop: runtime notification permission
- Priority: P3
- Effort: S
- Depends: T-35a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt
- Acceptance:
  - Decision recorded: Linux has no runtime notification permission; the POST_NOTIFICATIONS check (NotificationExtensions.kt:29-50) is not ported.
  - When the service is unavailable the fallback in T-35a-011 and T-35a-002 applies.

### T-35a-064 — Drop: accent colour tint on notifications
- Priority: P3
- Effort: S
- Depends: T-35a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt
- Acceptance:
  - Decision recorded: GIO notifications take the desktop theme; `R.color.accent_blue` (NotificationExtensions.kt:71) is not applied.
  - User loss: Android-style tinted notifications; the desktop tint is used instead.

### T-35a-065 — Drop: Android notification sound override for backup-complete
- Priority: P3
- Effort: S
- Depends: T-35a-082
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Decision recorded: GIO cannot request silence portably. The category keeps High priority; sound is the desktop's choice.
  - User loss stated: the backup-complete notice may play the desktop sound.

### T-35a-070 — ExportEntry model and favourites query
- Priority: P2
- Effort: S
- Depends: area:05-data-repos-entries-items, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - `src/export/library_csv.rs` defines `EntryKind`, `ExportEntry`, `ExportOptions` (default all true).
  - Favourites query returns anime rows first then manga rows, each with `ORDER BY _id` (PROPOSAL; test with a fixture DB).
  - Only `title`, `author`, `artist` and the kind are read.

### T-35a-071 — CSV renderer, byte-exact
- Priority: P2
- Effort: S
- Depends: T-35a-070
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt
- Acceptance:
  - `render` implements the cell rule, row join and file rule of p1-topic-library-csv-export.md sections 4-5.
  - Golden test: the worked example in section 6 matches byte for byte; zero rows gives `""`; zero columns gives `\r\n` for two rows.

### T-35a-072 — Export columns dialog
- Priority: P2
- Effort: S
- Depends: area:47-settings-screens-general, T-35a-071
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt
- Acceptance:
  - Dialog with four check buttons (Title, Type, Author, Artist), title "Select data to include", buttons Cancel and Save.
  - Unchecking Title unchecks and disables the other three; re-checking Title keeps them unchecked (test of the sensitivity rule).
  - Enter activates Save; Escape activates Cancel.

### T-35a-073 — Save-file dialog for the CSV
- Priority: P2
- Effort: S
- Depends: T-35a-072, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - `gtk::FileDialog::save` with initial name `kurobayomi_library.csv` and a CSV filter; dismissal is silent (no toast, no error).
  - Chosen path is passed to the writer; the dialog is not blocking the GTK thread (callback form).

### T-35a-074 — Atomic CSV write with error reporting
- Priority: P2
- Effort: S
- Depends: T-35a-071, area:52-util-app
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt
- Acceptance:
  - `write_csv` writes `<name>.kuro-tmp`, syncs, renames; on error it removes the temp file and returns `ExportError::Write` (test with read-only directory: no temp left).
  - Runs on `spawn_blocking`; errors show toast "Export failed: <error>" (`export.failed`).
  - Defect 3.5 (silent success on null stream) cannot occur: there is no null stream path.

### T-35a-075 — Completion toast
- Priority: P2
- Effort: S
- Depends: T-35a-074
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - On success the toast "Library Exported" (`export.done`) is shown; with zero rows the text is "Library Exported (0 entries)" (`export.done_empty`).
  - The toast is shown only after the rename succeeded (test).

### T-35a-076 — Snapshot favourites at Save time
- Priority: P2
- Effort: S
- Depends: T-35a-070, T-35a-073
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - The favourites query runs when Save is confirmed, not when the page opens (test: change a favourite between open and save; the file reflects the change).
  - Defect 3.1 step 6 (load-once) is not reproduced.

### T-35a-077 — Export options kept for the settings session only
- Priority: P2
- Effort: S
- Depends: T-35a-072
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - The last-used options are kept while the settings page is open and reset to all checked on leaving (test).
  - Nothing is written to preferences for the export options (grep check: no `export.` key).

### T-35a-080 — Library notification categories and ids (for area 26)
- Priority: P1
- Effort: S
- Depends: T-35a-004, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Ids `library.progress`, `library.error`, `library.size-warning`, `new.chapters`, `new.episodes` and categories `LibraryProgress`, `LibraryErrors`, `NewChapters` exist in `registry.rs` and are used by area:26-library-update-jobs (grep check).
  - Group names `eu.kanade.tachiyomi.NEW_CHAPTERS` and `eu.kanade.tachiyomi.NEW_EPISODES` are kept as desktop grouping hints only (no behaviour depends on them).

### T-35a-081 — Download notification categories and ids (for areas 23 and 24)
- Priority: P1
- Effort: S
- Depends: T-35a-004, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt
- Acceptance:
  - Ids `download.manga.progress`, `download.manga.error`, `download.anime.progress`, `download.anime.error` and categories `DownloadProgress`, `DownloadErrors` exist; areas 23/24 use them (grep check).
  - Progress text follows the throttle rule (T-35a-008).

### T-35a-082 — Backup and restore categories and ids (for areas 21 and 22)
- Priority: P1
- Effort: S
- Depends: T-35a-004, area:21-backup-format-and-create, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
  - Ids `backup.progress`, `backup.complete`, `restore.progress`, `restore.complete` and categories `BackupProgress`, `BackupComplete` exist and are used by areas 21/22.
  - `BackupComplete` has priority High (parity with the HIGH channel, Notifications.kt:170-175).

### T-35a-083 — Incognito mode notice category
- Priority: P2
- Effort: S
- Depends: T-35a-004, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Id `incognito` and category `Incognito` exist; area:01-foundation-app-bootstrap posts it while incognito is on and withdraws it when off (test).

### T-35a-084 — Server status categories (torrent and HTTP servers)
- Priority: P2
- Effort: S
- Depends: T-35a-004, area:13-core-util-storage-archive-torrent, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt, app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Ids `server.torrent`, `server.http` and category `ServerStatus` exist; running servers are shown as a status line in the app window, not as a persistent notice (decision recorded; user loss: no notice while the app is minimised).

### T-35a-085 — Extension update categories (for areas 18 and 19)
- Priority: P2
- Effort: S
- Depends: T-35a-004, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/ExtensionUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Ids `extensions.updates`, `extensions.installing` and category `ExtensionUpdates` exist; areas 18/19 use them.

### T-35a-086 — Save-image notice gets its own id
- Priority: P2
- Effort: S
- Depends: T-35a-005, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Save-image notices use id `save-image` and category `SaveImage`; showing an update prompt no longer replaces a save-image notice (test N2).

### T-35a-087 — Bundled symbolic icons for notices
- Priority: P2
- Effort: S
- Depends: T-req-platform-009
- Sources: app/src/main/res/xml/searchable.xml, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt
- Acceptance:
  - `kuro-platform::icons` maps each notice icon name (`software-update-available`, `folder-download-symbolic`, `dialog-warning-symbolic`, `network-server-symbolic`, `emblem-system-symbolic`) to a bundled SVG; no theme-dependent name is used without a bundled fallback (test: each name resolves).
  - Notification icons do not change when the GTK icon theme changes (manual check).

### T-35a-090 — Version and regression unit tests
- Priority: P1
- Effort: S
- Depends: T-35a-041
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - Tests listed in p1-edge-cases-and-tests.md section 5.1 exist and pass in CI (`cargo test`).

### T-35a-091 — Asset selection unit tests
- Priority: P1
- Effort: S
- Depends: T-35a-043
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - Tests in p1-edge-cases-and-tests.md section 5.1 (tiers, extension preference, first match, x86 not universal) pass with the captured fixture and the synthetic Kurobayomi fixture.

### T-35a-092 — Action codec and validation tests
- Priority: P1
- Effort: S
- Depends: T-35a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - Round trip for every `AppAction` variant; payload > 1 MiB rejected; host and path validation tests of design section 6.1 pass.

### T-35a-093 — ProgressThrottle unit tests
- Priority: P1
- Effort: S
- Depends: T-35a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - Sequence and boundary tests of p1-edge-cases-and-tests.md section 5.3 pass.

### T-35a-094 — Port GetApplicationReleaseTest (four cases)
- Priority: P1
- Effort: S
- Depends: T-35a-047
- Sources: domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt
- Acceptance:
  - The four upstream cases of p1-edge-cases-and-tests.md section 4 exist under the names listed there and pass.

### T-35a-095 — CSV golden and edge tests
- Priority: P2
- Effort: S
- Depends: T-35a-071
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt
- Acceptance:
  - Golden test of section 6 and the edge list of p1-edge-cases-and-tests.md section 5.6 pass; `write_csv` atomic-replace test passes.

### T-35a-096 — Release fixtures and fetch test against a local server
- Priority: P1
- Effort: S
- Depends: T-35a-042
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - `fetch_latest` tested against a local TCP server that replays `github_latest_aniyomi.json` (status 200), a 404 and a 403 response; mapping of each status is asserted.

### T-35a-097 — Downloader integration tests
- Priority: P1
- Effort: M
- Depends: T-35a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt
- Acceptance:
  - The six download scenarios of p1-edge-cases-and-tests.md section 5.7 pass against a local server (success, 404, cancel, checksum mismatch, replace, offline).

### T-35a-098 — Router tests with a fake notifier
- Priority: P1
- Effort: S
- Depends: T-35a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - Missing chapter => toast "Error" and no navigation; `MarkChaptersRead` withdraws before the DB call; `Dismiss` withdraws only its id (order assertions on the fake event log).
