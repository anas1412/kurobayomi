# Area 36 — Implementation tasks (app shell, navigation, security/unlock, WebView, base activities, onboarding)

Task IDs use the prefix T-36a-. Sources are repo-relative to `/home/blackbox/aniyomi-src` (Kotlin reference) unless the task names a Kurobayomi path (`src/...`, `spec/...`). Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint p1-tasks.md`.

Priority: P0 needed for daily use; P1 parity; P2 long tail; P3 Android-only / obsolete (drop task states the replacement or drop).

Dependency tokens: T-ids inside this file or other areas, or `area:<id>`.

---

## Shell and startup

### T-36a-001 — Process bootstrap, single instance, migration gate before first window
- Priority: P0
- Effort: M
- Depends: area:01-foundation-app-bootstrap, area:11-app-migrations, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/mihon/core/migration/Migrator.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/activity/BaseActivity.kt, src/main.rs
- Acceptance:
  - `gtk::Application` id `io.github.anas1412.Kurobayomi` registers on D-Bus; a second launch forwards its arguments to the first instance and exits with status 0.
  - The first window is not presented until the migration future resolves (no `block_on` on the GTK thread).
  - The tokio runtime is created once in `main` and reachable from `shell::Shell`.
  - A debug run with `KUROBAYOMI_SMOKE` set still uses `ApplicationFlags::NON_UNIQUE` (src/main.rs:15-16).

### T-36a-002 — Root navigator and screen transition
- Priority: P0
- Effort: M
- Depends: T-36a-001, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/presentation/util/Navigator.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `shell::nav::Navigator` keeps `stack[0] == Route::Home` for its whole life; `pop` returns `None` on length 1.
  - `push`, `pop`, `replace`, `pop_until_root` produce the same stack as the Kotlin sequence in behaviour §4.2 (unit test).
  - Forward transitions use `gtk::Stack` `SlideLeftRight` 250 ms; a pop uses the reverse direction.

### T-36a-003 — Navigator disposal and nested sheet navigators
- Priority: P1
- Effort: S
- Depends: T-36a-002
- Sources: app/src/main/java/eu/kanade/presentation/components/AdaptiveSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Popped pages drop their widget tree (`disposeSteps = true`); the widget count after pop returns to the pre-push count (checked in a smoke test).
  - When a sheet closes, all its inner pages are released in reverse order (`disposeNestedNavigators = false` path).

### T-36a-004 — Home tab host and start screen
- Priority: P0
- Effort: M
- Depends: T-36a-002, area:02-preferences-system, area:37-ui-library, area:40-ui-browse-manga, area:41-ui-browse-anime, area:42-ui-updates-history-stats-storage-upcoming
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt, app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, src/main.rs
- Acceptance:
  - The start tab is chosen from `start_screen` (ANIME default; MANGA, UPDATES, HISTORY, BROWSE map to the tab kinds in behaviour §2.5).
  - Tab state key `HomeTabs` is preserved across window re-layout (wide/narrow switch keeps the selected tab).
  - Changing `start_screen` in Settings applies on the next Home build (proposal in behaviour §2.1; test that the Kotlin static capture is not copied).

### T-36a-005 — Adaptive tab chrome (bottom switcher vs sidebar)
- Priority: P0
- Effort: M
- Depends: T-36a-004, T-36a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt, app/src/main/java/eu/kanade/presentation/util/DisplayExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt, src/main.rs
- Acceptance:
  - `layout::chrome_for(mode, width, height, maximized)` returns Sidebar when width >= 760 or maximised in AUTOMATIC mode (table in behaviour §8.2).
  - The bottom bar hides when the current tab equals the More-moved tab (`bar_visible`), with a 200 ms expand/shrink.
  - Reader and player pages hide both chrome elements (src/main.rs immersive rule kept).

### T-36a-006 — NavStyle and bar tab composition
- Priority: P0
- Effort: S
- Depends: T-36a-004, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - `NavStyle::bar_tabs()` returns the six-minus-one list for each of the four styles in the order given in behaviour §2.4 (unit test covering all four).
  - The stored names `MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE` (default), `MOVE_BROWSE_TO_MORE` are read from preference `bottom_rail_nav_style`; an unknown value falls back to MOVE_HISTORY_TO_MORE.
  - Spec change: `spec/app.json` `nav` lists `anime_library` and `manga_library` as separate entries (design §6.2).

### T-36a-007 — Tab badges (updates and extensions)
- Priority: P1
- Effort: S
- Depends: T-36a-005, area:08-domain-library-category-history-updates, area:09-domain-source-track-download-release-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Updates badge = anime + manga new updates when `library_show_updates_count` is true, otherwise 0; hidden at 0.
  - Browse badge = manga extension updates + anime extension updates; hidden at 0.
  - Hard-coded mockup badges `"8"` and `"1"` (src/main.rs:175) are removed; values come from `watch` receivers.
  - Accessibility name uses the plural keys `notification_chapters_generic` and `update_check_notification_ext_updates`.

### T-36a-008 — Tab reselect and back-to-start rule
- Priority: P0
- Effort: S
- Depends: T-36a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt
- Acceptance:
  - `back_enabled(current, default, more)` equals the Kotlin expression `(A || B) && (C || D)` for all 2^4 combinations (unit test, behaviour §2.6).
  - Clicking the selected bar item runs `on_reselect`; for More it pushes Settings.
  - Escape on a non-start tab selects `start_target(default, more)`.

### T-36a-009 — Tab routing events (openTab, search, bottom bar visibility)
- Priority: P1
- Effort: M
- Depends: T-36a-004, T-36a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt
- Acceptance:
  - `NavAction::SelectTab` mirrors `openTabEvent` for every `Tab` variant (AnimeLib, Library, Updates, History, Browse with extensions flags, More).
  - `NavAction::SearchLibrary(q)` first selects the start tab, then calls the library search only when the start tab is AnimeLibrary or MangaLibrary.
  - Channel is bounded (16) with drop-oldest; no deadlock when called from a tokio task.

### T-36a-010 — Intent: notification dismiss
- Priority: P1
- Effort: S
- Depends: T-36a-001, area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `LaunchRequest::Notification { id, group_id }` withdraws the notification with that id; group id defaults to 0 when absent.
  - `route_request` returns no NavAction for a notification-only request, and `ready = true`.

### T-36a-011 — Intent: shortcut actions
- Priority: P1
- Effort: S
- Depends: T-36a-001, T-36a-004, area:53-platform-manifest-intents-workers
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `ShortcutAction` covers the 11 Constants actions (SHOW_ANIMELIB, SHOW_LIBRARY, SHOW_ANIME, SHOW_MANGA, SHOW_RECENTLY_UPDATED, SHOW_RECENTLY_READ, SHOW_CATALOGUES, ANIMEEXTENSIONS, EXTENSIONS, SHOW_DOWNLOADS, SHOW_ANIME_DOWNLOADS).
  - `.desktop` `[Desktop Action]` entries exist for each and launch `kurobayomi --action=<name>`.
  - SHOW_ANIME / SHOW_MANGA with a missing id return `RouteError::MissingId` and still set `ready`.

### T-36a-012 — Intent: search (ACTION_SEARCH / ACTION_SEND equivalents)
- Priority: P1
- Effort: M
- Depends: T-36a-010, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/DeepLinkScreenType.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - `--search <text> [--type anime|manga]` and the Browse drop target both produce `Search { query, kind }`; empty query is a no-op.
  - Missing kind defaults to Anime; an unknown kind string returns `RouteError` and a toast, never a panic.
  - Result stack: `PopToRoot`, `Push(GlobalSearch)`, `Push(DeepLink)` in that order (behaviour §4.2).

### T-36a-013 — Intent: extension-driven search
- Priority: P1
- Effort: S
- Depends: T-36a-012, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `ExtSearch { kind, query, filter }` pushes `GlobalSearch` with the filter; empty query is a no-op.
  - Manga kind uses the manga global search; anime kind uses the anime one.

### T-36a-014 — Intent: file and URI open (.tachibk, add-repo)
- Priority: P1
- Effort: M
- Depends: T-36a-002, area:22-backup-restore, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - `OpenFile(path)` ending in `.tachibk` pushes `RestoreBackup { uri }` after PopToRoot; other file types are ignored.
  - `tachiyomi://add-repo?url=` pushes manga repos; `aniyomi://add-repo?url=` and `aniyomi://extension-store?url=` push anime stores (behaviour §4.4); a missing `url` is a no-op.
  - `.desktop` declares `MimeType=application/x-tachibk;` and `x-scheme-handler/tachiyomi;x-scheme-handler/aniyomi;`.

### T-36a-015 — onNewIntent equivalent (GApplication open and activate while running)
- Priority: P1
- Effort: S
- Depends: T-36a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Requests arriving while the window exists are applied on the GTK thread in arrival order.
  - Requests arriving before `activate` are queued and applied after the window is built.

### T-36a-016 — Auto-clear chapter cache at launch
- Priority: P1
- Effort: S
- Depends: T-36a-001, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - When `auto_clear_chapter_cache` is true, the chapter cache is cleared once per launch on a background task; default false.
  - The clear runs off the GTK thread; the window is not delayed by it.

### T-36a-017 — Incognito reset at launch and source-screen pop on incognito off
- Priority: P1
- Effort: S
- Depends: T-36a-002, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `__APP_STATE_incognito_mode` is set false at every launch (MainActivity.kt:216).
  - Turning incognito off pops to root when the top page is a Browse source page or an Entry opened from source (`from_source = true`); other pages are untouched.

### T-36a-018 — App state banners (indexing, downloaded-only, incognito)
- Priority: P0
- Effort: M
- Depends: T-36a-005, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/components/Banners.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Banners appear in the order indexing, downloaded-only, incognito, each with expand/shrink animation.
  - Texts use `download_notifier_cache_renewal`, `label_downloaded_only`, `pref_incognito_mode`; colours are secondary, tertiary, primary.
  - Turning on downloaded-only shows its banner within one frame of the preference change.

### T-36a-019 — System bar and edge-to-edge styling (drop)
- Priority: P3
- Effort: S
- Depends: T-36a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt
- Acceptance:
  - Drop decision recorded: GTK draws the title bar; the status-bar luminance rule (0.5) and transparent system bars have no Linux equivalent.
  - The header tint accent (behaviour §1.2) is kept as an optional CSS class, not a system call.

### T-36a-020 — Startup gate (splash equivalent)
- Priority: P2
- Effort: S
- Depends: T-36a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `StartupGate::keep_hidden(now)` is true while elapsed <= 500 ms, or while not ready and elapsed <= 5000 ms (unit test at 0, 499, 501, 4999, 5001 ms).
  - Ready is set after the first route is applied, including the MissingId failure path (behaviour §1.6).

### T-36a-021 — Changelog dialog after migration
- Priority: P1
- Effort: S
- Depends: T-36a-001, area:11-app-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Shown only when migration reported a change and the build is not debug; not shown on the next launch.
  - Title uses `updated_version` with the version; buttons `whats_new` (opens the release URL) and `action_ok`.

### T-36a-022 — Startup update checks (app release and extensions)
- Priority: P1
- Effort: M
- Depends: T-36a-001, area:35-data-misc-updater-notification-export, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt
- Acceptance:
  - Release check runs only when the updater is enabled; a `NewUpdate` result routes to the update prompt.
  - Extension checks run after the app check; a failure in one is logged and does not stop the other unless the Kotlin ordering requires it (behaviour §7.1 single try).
  - Release URL is `https://github.com/aniyomiorg/aniyomi/releases/tag/v<version>` (preview builds use `aniyomi-preview` and `r<commit count>`).

### T-36a-023 — New update prompt page
- Priority: P1
- Effort: M
- Depends: T-36a-022, area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt, app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt
- Acceptance:
  - Changelog text has everything from the first `---` line through `Checksums` removed (regex `---(\R|.)*Checksums(\R|.)*`), tested with a fixture that has a checksum block.
  - Accept starts the download job with the download URL and version title, then pops; Reject pops; Open in browser opens the release link.
  - Page title is `New version available!` (spec `new_update`).

---

## Onboarding

### T-36a-024 — Onboarding gate and four-step wizard
- Priority: P0
- Effort: M
- Depends: T-36a-002, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/more/onboarding/OnboardingScreen.kt, app/src/main/java/eu/kanade/presentation/more/onboarding/OnboardingStep.kt, app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Shown when `__APP_STATE_onboarding_complete` is false; Next is disabled while the current step's `is_complete()` is false.
  - Button label is Finish on step 4 and Next otherwise; Back is blocked on step 1 and steps back otherwise.
  - Finish sets the flag to true and pops; closing the window before Finish leaves the flag false.

### T-36a-025 — Onboarding step 1: theme
- Priority: P0
- Effort: S
- Depends: T-36a-024, T-36a-029, T-36a-030
- Sources: app/src/main/java/eu/kanade/presentation/more/onboarding/ThemeStep.kt
- Acceptance:
  - Changing theme mode applies the GTK colour scheme immediately and persists `pref_theme_mode_key`.
  - Changing app theme persists `pref_app_theme`; AMOLED toggle persists `pref_theme_dark_amoled_key`.

### T-36a-026 — Onboarding step 2: storage folder
- Priority: P0
- Effort: M
- Depends: T-36a-024, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/presentation/more/onboarding/StorageStep.kt
- Acceptance:
  - "Select folder" opens the portal folder chooser (ashpd `FileChooser`, directory mode) with a fallback to `gtk::FileDialog` when the portal is absent.
  - Selected path is stored under `__APP_STATE_storage_dir`; Next enables only when it is set.
  - Cancelling the chooser leaves the step incomplete and shows no error.
  - Help button opens `https://aniyomi.org/docs/faq/storage` in the default browser.

### T-36a-027 — Onboarding step 3: desktop status (replaces permissions)
- Priority: P2
- Effort: S
- Depends: T-36a-024
- Sources: app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt
- Acceptance:
  - Shows notifications status (available on desktop) and autostart status as informational rows with no grant buttons.
  - Install-apps and battery-optimisation rows are absent (decision recorded in behaviour §10).

### T-36a-028 — Onboarding step 4: guides and restore entry
- Priority: P0
- Effort: S
- Depends: T-36a-024, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/onboarding/GuidesStep.kt, app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt
- Acceptance:
  - "Getting started guide" opens `https://aniyomi.org/docs/guides/getting-started`.
  - "Restore backup" finishes onboarding and opens Settings > Data and storage with the restore row highlighted (see T-36a-062).

---

## Theme and layout

### T-36a-029 — App theme application (name map, AMOLED, Linux fallback)
- Priority: P1
- Effort: M
- Depends: T-36a-001, area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt, app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt
- Acceptance:
  - `AppTheme::parse` accepts all 22 stored names (DEFAULT, 18 themed, 3 deprecated) and maps unknown names to Default (unit test).
  - AMOLED on + dark applies CSS class `kuro-amoled`; off applies nothing.
  - MONET (dynamic colour) resolves to Default via `effective()`.

### T-36a-030 — Theme mode to GTK colour scheme
- Priority: P0
- Effort: S
- Depends: T-36a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - LIGHT and DARK set `gtk-application-prefer-dark-theme` accordingly; SYSTEM follows the freedesktop colour-scheme portal (`PreferDark` -> dark, `PreferLight` -> light, `NoPreference` -> GTK default).
  - Stored names `LIGHT`, `DARK`, `SYSTEM` (default) are read from `pref_theme_mode_key`.

### T-36a-031 — Tablet UI mode and breakpoints
- Priority: P0
- Effort: S
- Depends: T-36a-005, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/util/DisplayExtensions.kt, app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt
- Acceptance:
  - `TabletUiMode` stored names AUTOMATIC (default), ALWAYS, LANDSCAPE, NEVER read from `tablet_ui_mode`.
  - Mode table in behaviour §8.2 is implemented; unit test for each mode at widths 500, 760, 900 and orientations.

---

## Security and unlock

### T-36a-032 — App lock state machine
- Priority: P1
- Effort: L
- Depends: T-36a-001, area:02-preferences-system, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - `on_app_stopped` and `on_app_started` in `shell::lock` match the Kotlin for all combinations of enabled, authenticating, require_unlock and lock_after in {-1, 0, 5, 30} (table test, behaviour §11).
  - `__APP_STATE_last_app_closed` is written only when lock_after > 0 and deleted on every start when lock is enabled.
  - `require_unlock` starts true on every process start.
  - Setting lock_after = -1 never locks after a clean restart.

### T-36a-033 — App lock authenticator (Linux replacement for BiometricPrompt)
- Priority: P1
- Effort: L
- Depends: T-36a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt
- Acceptance:
  - Passphrase stored as Argon2id PHC (m=19456 KiB, t=2, p=1) in `$XDG_CONFIG_HOME/kurobayomi/lock.json`, mode 0600; check is constant-time.
  - fprintd verification via zbus (`net.reactivated.Fprint`) when the device is available; passphrase fallback after 5 failed fingerprint attempts.
  - No plaintext passphrase is logged; secrets use `secrecy::SecretString`.
  - Enabling the lock without a passphrase or fingerprint device shows a setup dialog, not a silent disable.

### T-36a-034 — Unlock window
- Priority: P1
- Effort: M
- Depends: T-36a-033, T-36a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt
- Acceptance:
  - Lock overlay covers the window with a blank page before showing the entry; main content is not visible while locked (screenshot test).
  - Success calls `unlock()` and hides the overlay; cancel closes the overlay and keeps the app running (intentional change from Kotlin, which exits).
  - Lock state re-checked on window re-activation; no duplicate overlays (Kotlin resume loop, behaviour §11.5).

### T-36a-035 — Secure screen flag (drop)
- Priority: P3
- Effort: S
- Depends: T-36a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt
- Acceptance:
  - Drop decision recorded: no Linux API blocks screen capture for normal clients.
  - `secure_screen_v2` preference is still readable for backup compatibility; the Settings screen (area 47) shows a note that it has no effect on Linux.

### T-36a-036 — Incognito state
- Priority: P1
- Effort: S
- Depends: T-36a-017, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/widget/TachiyomiTextInputEditText.kt
- Acceptance:
  - `__APP_STATE_incognito_mode` drives the incognito banner (T-36a-018) and the secure-screen rule (dropped, T-36a-035).
  - The IME hint has no GTK equivalent; the drop is recorded in the Settings note.

---

## App bar, search and shared components

### T-36a-037 — App bar: title, subtitle, count pill, navigation icon
- Priority: P0
- Effort: M
- Depends: T-36a-001
- Sources: app/src/main/java/eu/kanade/presentation/components/AppBar.kt
- Acceptance:
  - Title one line with ellipsis; subtitle one line with ellipsis and full-text tooltip (marquee dropped).
  - Count pill shows `count` at 14 pt when `count > 0`; colour onBackground at 8% light / 12% dark.
  - Navigation icon: up arrow by default, custom icon when given; tooltip and accessible name `action_bar_up_description`.

### T-36a-038 — App bar: action mode and actions
- Priority: P0
- Effort: M
- Depends: T-36a-037
- Sources: app/src/main/java/eu/kanade/presentation/components/AppBar.kt
- Acceptance:
  - Action mode shows the selected count as title, a close button named `action_cancel`, and the mode actions; Escape also cancels (proposal).
  - Action buttons show tooltip = title and respect `enabled`; overflow button appears only when an overflow action exists, named `action_menu_overflow_description`.

### T-36a-039 — Search toolbar
- Priority: P0
- Effort: M
- Depends: T-36a-037
- Sources: app/src/main/java/eu/kanade/presentation/components/AppBar.kt
- Acceptance:
  - Search icon opens the entry; Enter runs `onSearch` only for non-blank queries, then clears focus.
  - Reset button (named `action_reset`) appears only for non-empty queries and refocuses the entry.
  - Keyboard opens with the entry only when the query was empty at open time (gtk entry focus test).
  - `searchEnabled = false` hides the search icon; placeholder defaults to `action_search_hint`.
  - Consumers debounce with 250 ms (`SEARCH_DEBOUNCE_MILLIS`).

### T-36a-040 — Tabbed screen (per-page search and actions)
- Priority: P0
- Effort: M
- Depends: T-36a-039, T-36a-037
- Sources: app/src/main/java/eu/kanade/presentation/components/TabbedScreen.kt
- Acceptance:
  - Tab row fixed or scrollable (13 px edge padding when scrollable); tabs show badges.
  - Each `TabContent` binds its own search query (no index parity rule), the anime and manga pages keep their existing queries after switching.
  - Search disabled on a page hides the search entry for that page only.

### T-36a-041 — Tabbed dialog (settings sheet)
- Priority: P1
- Effort: M
- Depends: T-36a-042
- Sources: app/src/main/java/eu/kanade/presentation/components/TabbedDialog.kt
- Acceptance:
  - Tab row on surface-container-high; overflow button on the row end when a callback is given.
  - Body padding 24 px horizontal, 8 px vertical (`TabbedDialogPaddings`).
  - Pager swipe or click switches tabs; height animates to content.

### T-36a-042 — Adaptive sheet (phone bottom sheet, tablet dialog)
- Priority: P1
- Effort: M
- Depends: T-36a-002
- Sources: app/src/main/java/eu/kanade/presentation/components/AdaptiveSheet.kt
- Acceptance:
  - Narrow window: sheet anchored to the bottom; wide: centred; content max width 460 px.
  - Escape and scrim click dismiss; nested stack pops the inner page first; fade 220 ms (delay 90 ms) in, 90 ms out.
  - Swipe dismiss is not implemented (documented).

### T-36a-043 — Dropdown menus (width, radio, nested)
- Priority: P1
- Effort: S
- Depends: T-36a-037
- Sources: app/src/main/java/eu/kanade/presentation/components/DropdownMenu.kt
- Acceptance:
  - Popover content fixed at 196 px width.
  - Radio row shows checked/unchecked icons with accessible names `selected` / `not_selected`.
  - Nested row opens a sub-popover on click or Right arrow; Left arrow or Escape closes it.

### T-36a-044 — Entry download menu
- Priority: P1
- Effort: S
- Depends: T-36a-043, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/components/EntryDownloadDropdownMenu.kt
- Acceptance:
  - Options in order: Next 1, 5, 10, 25 (plural keys) and Unread/Unseen (`download_unread` / `download_unseen`).
  - Selecting an option dispatches the `DownloadAction` then closes the menu; the anime and manga plural keys differ as in the Kotlin.

### T-36a-045 — Floating add button with collapse-on-scroll
- Priority: P1
- Effort: S
- Depends: T-36a-037
- Sources: app/src/main/java/eu/kanade/presentation/components/FloatingActionAddButton.kt, presentation-core LazyListState.kt (shouldExpandFAB)
- Acceptance:
  - Expanded (icon + "Add") at top, after backward scroll, or at list end; collapsed (icon only) while scrolling forward.
  - Transition 200 ms.

### T-36a-046 — Download indicator
- Priority: P1
- Effort: S
- Depends: T-36a-001
- Sources: app/src/main/java/eu/kanade/presentation/components/ItemDownloadIndicator.kt
- Acceptance:
  - Indicator 26 px, padding 2 px, arrow 19 px; long press fires the long-click action.
  - Haptic feedback is not implemented (dropped).

### T-36a-047 — Relative date text
- Priority: P0
- Effort: S
- Depends: T-36a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/components/DateText.kt
- Acceptance:
  - Timestamp <= 0 renders `not_applicable`.
  - `relative_time_v2` default true; `app_date_format` empty uses the locale SHORT date style; a non-empty pattern is used as chrono format; invalid pattern falls back to the SHORT style and logs.
  - Date-time variant uses the local time zone.

### T-36a-048 — Empty screen component
- Priority: P0
- Effort: S
- Depends: T-36a-037
- Sources: app/src/main/java/eu/kanade/presentation/components/EmptyScreen.kt
- Acceptance:
  - Component renders an icon, text, and up to two action buttons; list areas use it (behaviour §14.7).
  - The Kotlin preview helpers are not ported (drop recorded).

---

## WebView

### T-36a-049 — WebView screen: header, navigation actions, progress
- Priority: P0
- Effort: M
- Depends: T-36a-037, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt
- Acceptance:
  - Header: close icon, title (page title or initial title), subtitle (current URL).
  - Back and Forward buttons enabled only when `can_go_back` / `can_go_forward`; overflow has Refresh, Share, Open in browser, Clear cookies in that order.
  - Progress bar indeterminate while initialising, determinate while loading (missing progress = 1.0), hidden when idle.

### T-36a-050 — WebView Cloudflare help banner
- Priority: P1
- Effort: S
- Depends: T-36a-049
- Sources: app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt
- Acceptance:
  - Banner shown when the page HTML contains `window._cf_chl_opt` or `Ray ID is`; re-evaluated on each page finish.
  - Click opens `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare`.

### T-36a-051 — WebView navigation policy and custom headers
- Priority: P1
- Effort: M
- Depends: T-36a-049, T-36a-052
- Sources: app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt
- Acceptance:
  - `blob:http` URIs are allowed; `intent://` URIs are blocked; other requests get the source headers (lowercase keys; `user-agent` applied as the UA string).
  - Headers are applied once per request, not by re-issuing the load (proposal in behaviour §12.3; test that a link produces one request).

### T-36a-052 — WebKitGTK engine, settings and cookie bridge
- Priority: P1
- Effort: L
- Depends: T-36a-001, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - `webkit6::WebView` with JavaScript on, developer extras only in debug, smooth scrolling on, zoom controls via Ctrl+plus/minus/0.
  - Cookie store at `$XDG_DATA_HOME/kurobayomi/webview/`; `clear_cookies_for(host)` removes cookies for the host and returns the count.
  - Cookies are shared with the HTTP client jar (area 12) through one sync function; a test shows a cookie set in the WebView is sent by the HTTP client.
  - Minimum engine check: missing WebKitGTK 6.0 shows the `information_webview_required` toast and the window closes.

### T-36a-053 — WebView activity, screen and model
- Priority: P1
- Effort: M
- Depends: T-36a-052, area:16-source-api-anime, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewScreenModel.kt
- Acceptance:
  - Source headers come from the manga or anime source matching the id; the anime lookup overrides the manga lookup when both match (Kotlin order).
  - Missing URL shows nothing and does not crash; the assist/current URL tracks navigations.
  - The unused stats state type of the Kotlin model is not ported.
  - Clear cookies uses a safe URL parse; an invalid URL is a toast, not a crash (defect fix from behaviour §12.1).

### T-36a-054 — Share and open in browser
- Priority: P1
- Effort: S
- Depends: T-36a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Share copies the URL to the clipboard and shows a toast (proposal, behaviour §10).
  - Open in browser uses `gio::AppInfo::launch_default_for_uri` and does not hand the URL back to Kurobayomi for `tachiyomi://` or `aniyomi://` schemes (filtered first).

---

## Deep links

### T-36a-055 — Deep-link kinds and routing
- Priority: P1
- Effort: M
- Depends: T-36a-012, T-36a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/DeepLinkScreenType.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaActivity.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - `Kind` enum with ANIME and MANGA; unknown kind string is an error, not a panic.
  - Trampoline behaviour replaced: a shared text or URL with kind chosen by the picker routes to `Search { kind }` then `DeepLink { kind, query }`.
  - Trampoline activities are not ported (Replace-with-crate row in inventory).

### T-36a-056 — Anime deep-link resolve page and model
- Priority: P1
- Effort: M
- Depends: T-36a-055, area:16-source-api-anime, area:24-downloads-anime, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreenModel.kt
- Acceptance:
  - Source chosen as the first resolvable anime source whose `get_uri_type(query)` is not Unknown; no match gives NoResults.
  - Anime with no episode id opens the anime entry page with `from_source = true`; with an episode id it opens the player for that episode (routed through the player router, T-36a-058).
  - Episode lookup: local first, otherwise an episodes update with fetch; not found gives the entry page.
  - Any resolver error shows an error row with Retry (Kotlin stays on loading; defect fix).

### T-36a-057 — Manga deep-link resolve page and model
- Priority: P1
- Effort: M
- Depends: T-36a-055, area:15-source-api-manga, area:07-domain-entries-items, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt
- Acceptance:
  - Same source selection rule as anime, using catalogue sources.
  - Chapter URL: local chapter first; if missing, fetch the chapter list, sync with the source, then find by URL.
  - Chapter result opens the reader at that chapter (`ReaderActivity` equivalent, area 43); entry result opens the entry page with `from_source = true`.
  - Error row with Retry on failure (defect fix).

---

## Player routing and external players

### T-36a-058 — Player router (internal player vs external player)
- Priority: P2
- Effort: M
- Depends: T-36a-055, area:45-player-core, area:46-player-controls-ui
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - `start_player(anime_id, episode_id, ext_player: bool, source_id: Option<i64>, video, hoster_index: -1, video_index: -1, hoster_list)` opens the internal player when `ext_player` is false.
  - External player: the local HTTP server starts with a 5 s timeout; failure shows `http_server_start_failure` and returns.
  - Source id defaults to the anime's source, or -1 when the anime is missing.

### T-36a-059 — External player result handling
- Priority: P2
- Effort: S
- Depends: T-36a-058
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - When the external player process exits with success, the progress callback (`ExternalIntents::on_activity_result` equivalent) runs with the saved anime and episode ids.
  - Non-success exit does nothing.

### T-36a-060 — Entry open routing from tabs
- Priority: P0
- Effort: S
- Depends: T-36a-009, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `Tab::AnimeLib { anime_id: Some(id) }` selects the anime library and pushes `Entry { Anime, id, from_source: false }`.
  - Manga equivalent pushes a manga entry; downloads with `toDownloads` push the Downloads page.

---

## Base activity and lifecycle

### T-36a-061 — Base activity and theming lifecycle
- Priority: P0
- Effort: S
- Depends: T-36a-001, T-36a-029
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/base/activity/BaseActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt
- Acceptance:
  - `shell::Shell::build` applies the theme before the first widget is created (the theme class is set on the window before show).
  - Tablet mode context override (behaviour §8.1) is applied before layout is measured.

### T-36a-062 — Restore backup highlight from onboarding
- Priority: P1
- Effort: S
- Depends: T-36a-024, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt
- Acceptance:
  - Restore action opens Settings at Data and storage and highlights the restore row (`highlightKey` = `label_backup`) once.
  - The highlight clears after the row is shown.

---

## Drops and widgets

### T-36a-063 — Android-only widgets (drop with replacement)
- Priority: P3
- Effort: S
- Depends: T-36a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/widget/MinMaxNumberPicker.kt, app/src/main/java/eu/kanade/tachiyomi/widget/RevealAnimationView.kt, app/src/main/java/eu/kanade/tachiyomi/widget/TachiyomiTextInputEditText.kt, app/src/main/java/eu/kanade/tachiyomi/widget/ViewPagerAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/widget/listener/IgnoreFirstSpinnerListener.kt, app/src/main/java/eu/kanade/tachiyomi/widget/listener/SimpleSeekBarListener.kt
- Acceptance:
  - Drop decisions recorded for NumberPicker (replacement: `gtk::SpinButton` with min and max in shared widgets), reveal animation, IME flag, ViewPager, spinner listener, seek bar listener.
  - No Kurobayomi code imports these concepts; grep for `ViewPager`, `NumberPicker`, `SeekBar` in `src/` returns nothing for this area.

### T-36a-064 — Assist content (drop)
- Priority: P3
- Effort: S
- Depends: T-36a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt, app/src/main/java/eu/kanade/presentation/util/Navigator.kt
- Acceptance:
  - Drop decision recorded: Android assistant context has no Linux equivalent.
  - `AssistContentScreen` is replaced by the window's current URL in the WebView page header only.

### T-36a-065 — Keyboard and mouse map for the shell
- Priority: P1
- Effort: S
- Depends: T-36a-002, T-36a-005
- Sources: app/src/main/java/eu/kanade/presentation/util/Navigator.kt, app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt
- Acceptance:
  - Escape implements back; Alt+1..5 select bar tabs; Ctrl+F opens search; Ctrl+, opens Settings; Ctrl+L locks; Ctrl+Q quits.
  - Bindings registered with `set_accels_for_action` so they show in menus.

### T-36a-066 — Accessibility labels and focus order for the shell
- Priority: P1
- Effort: S
- Depends: T-36a-037, T-36a-005
- Sources: app/src/main/java/eu/kanade/presentation/components/AppBar.kt, app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt
- Acceptance:
  - Every icon-only button has an accessible name from the same string keys the Kotlin uses.
  - Tab focus order: header buttons then page content; banners are not focusable.

### T-36a-067 — Shell test suite
- Priority: P1
- Effort: M
- Depends: T-36a-006, T-36a-008, T-36a-020, T-36a-032, T-36a-014, T-36a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt
- Acceptance:
  - Unit tests exist for: NavStyle lists, back rule truth table, start target, route_request cases (§4 table), lock state table, startup gate times, `AppTheme::parse`, `chrome_for`.
  - `cargo test` passes; no GTK needed for these tests.

### T-36a-068 — Android permission and intent rows (drop)
- Priority: P3
- Effort: S
- Depends: T-36a-027
- Sources: app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Drop decisions recorded for install-unknown-apps permission, runtime notification permission, and battery-optimisation exemption, each with the reason and what the user loses (none on Linux).
  - Desktop notification status row is the only replacement (T-36a-027).

### T-36a-069 — App state banner widget (WarningBanner)
- Priority: P0
- Effort: S
- Depends: T-36a-018
- Sources: app/src/main/java/eu/kanade/presentation/components/Banners.kt
- Acceptance:
  - Warning banner uses the error colour, 16 px padding, centred text in `bodyMedium`, and is used by the WebView Cloudflare banner.
  - Text width wraps at the window width; no horizontal scroll.
