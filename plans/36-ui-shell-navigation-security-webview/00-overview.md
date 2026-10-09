# Area 36 — App shell, navigation, security/unlock, WebView, base activities, onboarding: overview

Area lead overview. Kotlin reference tree: `/home/blackbox/aniyomi-src` (all repo-relative paths below are relative to it unless prefixed `src/`, `spec/` or `plans/`, which are Kurobayomi paths under `/home/blackbox/kurobayomi`).

Scope: 42 Kotlin files, 4326 LOC (`python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 36-ui-shell-navigation-security-webview 1`). Tasks: 69 (T-36a-001 to T-36a-069) in [p1-tasks.md](p1-tasks.md).

---

## 1. Purpose and scope

### 1.1 What the Kotlin does in this area (OBSERVED)

- Process entry and startup gate: `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt` (667 LOC). Waits for the migration, decides the splash window (500 ms minimum, 5000 ms maximum), dispatches launch intents, runs the changelog and update checks, and gates onboarding.
- Home shell: `app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt`. Five bar entries plus More, chosen by `NavStyle`, with start tab, back-to-start rule, tab reselect, and update/extension badges.
- Navigator: a Voyager-style push/pop/replace stack (`app/src/main/java/eu/kanade/presentation/util/Navigator.kt`, owned in its inventory by area 34; this area consumes it).
- Intent and URI routing: shortcut actions (`eu.kanade.tachiyomi.SHOW_*`), search intents (`SEARCH`, `ANIMESEARCH`, `SEARCH_ACTION`, `SEND text/plain`), `.tachibk` files, `tachiyomi://add-repo` and `aniyomi://add-repo|extension-store`. Two trampoline activities (`ui/deeplink/anime/DeepLinkAnimeActivity.kt`, `ui/deeplink/manga/DeepLinkMangaActivity.kt`) forward searches.
- Deep-link resolve pages: a query is resolved to a source entry and optionally to an episode or chapter (`ui/deeplink/{anime,manga}/`).
- App lock: `ui/base/delegate/SecureActivityDelegate.kt`, `ui/security/UnlockActivity.kt` and `util/system/AuthenticatorUtil.kt` implement a timed lock (`lock_app_after`) with a BiometricPrompt-based unlock. FLAG_SECURE is handled through `util/view/WindowExtensions.kt` (owned by area 52 in its inventory).
- WebView: `ui/webview/WebViewActivity.kt`, `WebViewScreen.kt`, `WebViewScreenModel.kt` and `presentation/webview/WebViewScreenContent.kt`. Used for source login and Cloudflare challenges, with source headers, share, open-in-browser and cookie clearing.
- Base activity and theming: `ui/base/activity/BaseActivity.kt`, `ui/base/delegate/ThemingDelegate.kt`, tablet-mode display rules in `util/system/DisplayExtensions.kt`.
- Onboarding: a four-step wizard (`presentation/more/onboarding/`, host in `ui/more/OnboardingScreen.kt`). Update prompt: `ui/more/NewUpdateScreen.kt`.
- Shared chrome used by every list and detail area: `presentation/components/` (AppBar, TabbedScreen, TabbedDialog, AdaptiveSheet, DropdownMenu, FAB, download indicator, banners, date text, empty-screen previews).

### 1.2 Why it matters for a usable Kurobayomi

Without this area there is no window that routes anywhere. Concretely:

- No start tab, no navigation between Anime, Manga, Updates, History, Browse and More, and no way to open an entry page.
- No state banners, so a user cannot see that downloaded-only or incognito mode is active.
- No Cloudflare or login path. Many sources need a WebView session before their pages load; the WebView screen (T-36a-049) is P0, but its engine and navigation policy (T-36a-051, T-36a-052) are not. See section 4.4.
- No first-run onboarding and no storage folder choice.
- No app lock if the user wants one.
- No way to open `.tachibk` backups or repo links from the desktop.

### 1.3 Out of scope (owned elsewhere)

Tab bodies and entry screens (areas 37 to 42), reader (43, 44), player (45, 46), settings screens (47, 48), palette and theme resources (49), migrations (11), updater and notifications (35), backup format and restore (21, 22), HTTP client and cookie jar (12), preferences store (02).

---

## 2. Module map

### 2.1 Kotlin packages to responsibilities

LOC is the sum of the inventory rows (`p1-inventory.md`).

| Kotlin package / file group | Files | LOC | Responsibility |
|---|---|---|---|
| `app/src/main/java/eu/kanade/presentation/components/` | 11 | 1375 | Shared chrome: `AppBar.kt` (456), `TabbedScreen.kt` (158), `TabbedDialog.kt` (118), `DropdownMenu.kt` (110), `AdaptiveSheet.kt` (95), `Banners.kt` (187), `DateText.kt` (79), `ItemDownloadIndicator.kt` (48), `EntryDownloadDropdownMenu.kt` (47), `FloatingActionAddButton.kt` (28), `EmptyScreen.kt` (49, previews only) |
| `app/src/main/java/eu/kanade/presentation/more/onboarding/` | 6 | 489 | Four wizard steps (`ThemeStep`, `StorageStep`, `PermissionStep`, `GuidesStep`), host (`OnboardingScreen`), contract (`OnboardingStep`) |
| `app/src/main/java/eu/kanade/presentation/webview/` | 1 | 230 | `WebViewScreenContent.kt`: toolbar, progress, Cloudflare banner, navigation policy |
| `app/src/main/java/eu/kanade/tachiyomi/ui/main/` | 1 | 667 | `MainActivity.kt`: startup, intent dispatch, changelog, update check, onboarding gate, player routing |
| `app/src/main/java/eu/kanade/tachiyomi/ui/home/` | 1 | 343 | `HomeScreen.kt`: tabs, NavStyle, back rule, badges, tab events |
| `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/` | 7 | 373 | `DeepLinkScreenType`, two trampoline activities, two resolve screens and models |
| `app/src/main/java/eu/kanade/tachiyomi/ui/webview/` | 3 | 255 | `WebViewActivity.kt` (151), `WebViewScreenModel.kt` (65), `WebViewScreen.kt` (39) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/base/` | 3 | 200 | `BaseActivity.kt` (25), `SecureActivityDelegate.kt` (122), `ThemingDelegate.kt` (53) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/security/` | 1 | 47 | `UnlockActivity.kt`: biometric prompt host |
| `app/src/main/java/eu/kanade/tachiyomi/ui/more/` | 2 | 95 | `NewUpdateScreen.kt` (43), `OnboardingScreen.kt` (52) wrapper |
| `app/src/main/java/eu/kanade/tachiyomi/widget/` | 6 | 252 | Android-only widgets, all dropped (see section 4.5) |
| **Total** | **42** | **4326** | |

Files read for exact behaviour but owned by other areas: `app/src/main/java/eu/kanade/presentation/util/Navigator.kt` (area 34 inventory), `app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt` (area 52 inventory), `core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt` (area 12), `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt` (area 02), `app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt` (area 02 and 49), `presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt` (`shouldExpandFAB`, area 49-theme-and-design-system inventory).

### 2.2 Proposed Rust module map

Source of truth: [p1-rust-design.md](p1-rust-design.md) section 1. Crate `kurobayomi`, edition 2024, `gtk4` 0.11 with feature `v4_16`, vanilla GTK 4 (no libadwaita).

| Rust module | Replaces (Kotlin) | Responsibility |
|---|---|---|
| `src/shell/mod.rs` | `BaseActivity`, `MainActivity` window | `Shell::build`: window, navigator, tab host, lock gate, toast sink |
| `src/shell/route.rs` | `HomeScreen.Tab`, `Constants` actions | `Route`, `Tab`, `TabKind`, `Kind`, `NavAction` |
| `src/shell/nav.rs` | `presentation/util/Navigator.kt` | `Navigator { stack: Vec<Route> }`, `stack[0] == Route::Home` invariant |
| `src/shell/tabs.rs` | `HomeScreen.kt`, `NavStyle`, `StartScreen` | Bar tab lists, start target, back rule, badge rule |
| `src/shell/startup.rs` | splash logic in `MainActivity.kt` | `StartupGate` (500 ms / 5000 ms), changelog rule |
| `src/shell/intents.rs` | `MainActivity.handleIntentAction` | `LaunchRequest` and pure `route_request` |
| `src/shell/banners.rs` | `presentation/components/Banners.kt` | Banner state and order |
| `src/shell/theme.rs` | `ThemingDelegate`, `ThemeMode`, `AppTheme` | Name parsing, `effective()`, CSS classes, colour-scheme portal |
| `src/shell/layout.rs` | `DisplayExtensions.kt` | `TabletUiMode`, `chrome_for` |
| `src/shell/lock.rs` | `SecureActivityDelegate.kt` | `LockConfig`, `LockState`, `on_app_stopped`, `on_app_started`, `unlock` |
| `src/shell/auth.rs` | `AuthenticatorUtil.kt` | `Authenticator` trait, `PassphraseAuth` (argon2id), `FprintdAuth` (zbus) |
| `src/shell/unlock_window.rs` | `UnlockActivity.kt` | Full-window lock overlay |
| `src/shell/onboarding.rs` | `presentation/more/onboarding/*`, `ui/more/OnboardingScreen.kt` | `OnboardingStep` trait, four steps, host |
| `src/shell/update_prompt.rs` | `ui/more/NewUpdateScreen.kt` | Changelog cleanup, accept, reject, open |
| `src/ui/appbar.rs`, `src/ui/search.rs` | `AppBar.kt` | `gtk::HeaderBar` builder, `gtk::SearchEntry` wrapper |
| `src/ui/tabbed.rs` | `TabbedScreen.kt`, `TabbedDialog.kt` | Tab row and stack, per-tab search and actions |
| `src/ui/sheet.rs` | `AdaptiveSheet.kt` | Modal window, max width 460 |
| `src/ui/menu.rs` | `DropdownMenu.kt`, `EntryDownloadDropdownMenu.kt` | Fixed-width popover menus |
| `src/ui/fab.rs` | `FloatingActionAddButton.kt` | Add button with collapse on scroll |
| `src/ui/download_indicator.rs` | `ItemDownloadIndicator.kt` | 26 px ring, 2 px stroke, long press |
| `src/ui/empty.rs` | `EmptyScreen` (other area) | Icon, text, up to two actions |
| `src/util/date.rs` | `DateText.kt` | Relative date text (chrono) |
| `src/web/mod.rs`, `src/web/window.rs`, `src/web/screen.rs`, `src/web/policy.rs`, `src/web/model.rs` | `ui/webview/*`, `presentation/webview/*` | WebKitGTK profile, page, navigation policy, model |
| `src/deeplink/mod.rs`, `src/deeplink/resolve.rs`, `src/ui/deeplink.rs` | `ui/deeplink/*` | Kind, resolve (tokio), loading/result page |

Existing mockup files that the shell calls rather than replaces: `src/main.rs`, `src/ui.rs` (`nav_init`, `page`, `screen`, `decoration`), `src/screens.rs` (`body(id)`), `src/entry.rs` (`webview()` at line 572, a static mockup to replace), `spec/app.json`. Mockup gaps are listed in [p1-ui-parity.md](p1-ui-parity.md) section 17 and [p1-rust-design.md](p1-rust-design.md) section 6.7.

---

## 3. Reading guide

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Table of all 42 scope files with LOC, declarations, port decision, target module and task IDs |
| [p1-behavior.md](p1-behavior.md) | Full OBSERVED behaviour with path:line citations for startup, navigation, intents, toolbars, deep links, updates, tablet layout, theming, lock, WebView, onboarding, concurrency and error keys |
| [p1-rust-design.md](p1-rust-design.md) | Module layout, Rust types and signatures, threading model, crate choices, Android-to-Linux mapping, mockup changes, pitfalls, keyboard map |
| [p1-data-and-config.md](p1-data-and-config.md) | Enum storage names, Tab and NavStyle models, every preference key with default and owner, constants table, intent extras, files on disk, external URLs |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E1 to E55, failure-mode handling, race table R1 to R12, unit, GTK smoke and property tests, fixtures, accessibility checks, recorded gaps |
| [p1-ui-parity.md](p1-ui-parity.md) | Visible layout, banners, badges, app bar, search, tabbed screen, sheets, menus, FAB, WebView screen, onboarding, update prompt, lock screen, toasts, keyboard, breakpoints, timings, mockup gap list |
| [p1-tasks.md](p1-tasks.md) | The 69 implementation tasks (T-36a-001 to T-36a-069) with priority, effort, dependencies, sources and acceptance criteria |
| [p1-topic-startup-lock-security.md](p1-topic-startup-lock-security.md) | Deep dive: startup order, lock timing state machine tables, authenticator decision, FLAG_SECURE decision, incognito interplay, data at rest, lock test checklist |
| [p1-topic-deeplinks-intents.md](p1-topic-deeplinks-intents.md) | Deep dive: inbound intent surface, route table, deep-link resolution algorithm, desktop mapping, security notes for external inputs |
| [00-overview.md](00-overview.md) | This document |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the area docs (with the reasoning kept)

| # | Decision | Kotlin behaviour | Linux choice | Reference |
|---|---|---|---|---|
| D1 | One window, one `Navigator` stack instead of Activities | Activities plus Voyager `Navigator` | `shell::nav::Navigator` over `gtk::Stack`; `stack[0] == Route::Home` | [p1-rust-design.md](p1-rust-design.md) 2.3 |
| D2 | Trampoline activities removed | `DeepLinkAnimeActivity` and `DeepLinkMangaActivity` forward `SEARCH`/`SEND` to `MainActivity` with `type` | Pure `route_request(LaunchRequest)`; GApplication `open` and `activate`; `.desktop` MIME and scheme handlers | [p1-topic-deeplinks-intents.md](p1-topic-deeplinks-intents.md) 4 |
| D3 | Lock backend is a passphrase (argon2id PHC), with fprintd as an optional second factor | BiometricPrompt, `BIOMETRIC_WEAK or DEVICE_CREDENTIAL` | `PassphraseAuth` in `$XDG_CONFIG_HOME/kurobayomi/lock.json` (mode 0600); `FprintdAuth` via zbus when enrolled | [p1-behavior.md](p1-behavior.md) 11.8 |
| D4 | Lock timing rules unchanged | `onApplicationStopped` / `onApplicationStart` table | Same rules, pure functions in `src/shell/lock.rs`; the one timestamp key `__APP_STATE_last_app_closed` kept | [p1-topic-startup-lock-security.md](p1-topic-startup-lock-security.md) 2 |
| D5 | FLAG_SECURE dropped | `secure_screen_v2` ALWAYS, INCOGNITO (default), NEVER | No Linux API; the lock overlay blanks the content before it appears | [p1-topic-startup-lock-security.md](p1-topic-startup-lock-security.md) 4 |
| D6 | Chrome is a sidebar at width >= 760 px or when maximised, otherwise a bottom switcher | `isTabletUi()` at smallest width 720 dp, with `tablet_ui_mode` overrides | `layout::chrome_for(mode, width, height, maximized)`; key `tablet_ui_mode` kept for backup compatibility | [p1-behavior.md](p1-behavior.md) 8.2 |
| D7 | Bar composition follows `NavStyle` with four variants, default `MOVE_HISTORY_TO_MORE` | `tabs` = six tabs minus `moreTab` | Same four variants; `More` always last | [p1-behavior.md](p1-behavior.md) 2.4 |
| D8 | Theme is the user's GTK theme; the stored `AppTheme` name has no visual effect | 18 Android style resources, Monet dynamic colour | `AppTheme::effective()` maps `MONET` to `DEFAULT`; `AMOLED` becomes CSS class `kuro-amoled` in dark mode only | [p1-behavior.md](p1-behavior.md) 9.4 |
| D9 | WebView engine is WebKitGTK 6 through `webkit6`, one `WebContext` per profile | Android WebView via Accompanist | `web::window`; `decide-policy` for navigation; `CookieManager` bridged to the reqwest jar (area 12) | [p1-behavior.md](p1-behavior.md) 12.5 |
| D10 | Share becomes "copy URL to clipboard and toast" | `Intent.ACTION_SEND` share sheet | Clipboard; no portal share | [p1-behavior.md](p1-behavior.md) 10 |
| D11 | Onboarding keeps four steps and order; battery and install rows dropped | `ThemeStep`, `StorageStep`, `PermissionStep`, `GuidesStep` | `DesktopStep` replaces `PermissionStep` (status only); storage uses ashpd `FileChooser` with `gtk::FileDialog` fallback | [p1-behavior.md](p1-behavior.md) 13.6 |
| D12 | Start screen applied on the next Home build, not only at process start | Read once when `HomeScreen` object initialises (behaviour 2.1) | Read on each `Home` build (proposal; changes a Kotlin defect E20) | [p1-behavior.md](p1-behavior.md) 2.1 |
| D13 | Update checks run at startup and then every 24 h | Run once per launch from `LaunchedEffect(Unit)` | Proposal: 24 h cadence, preference key `last_update_check_ms` (see open question Q3 before shipping) | [p1-behavior.md](p1-behavior.md) 7.4 |

Kotlin defects deliberately not copied (each has an edge-case ID in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md)): unknown `type` extra throws and crashes (E5); deep-link model stays in Loading forever on exception (E35); navigation and `startActivity` run in composition body (E37); lock prompt reopens on every resume (E11); anime deep links bypass the external-player setting (E-gap in [p1-behavior.md](p1-behavior.md) 6.2); cancel on unlock quits the app (E10, intentional change, see Q1); `clearCookies` throws on bad URL in one path (E29); library tab search depends on `page % 2` (E25).

### 4.2 Open questions that need the user's call

Each has a recommendation. Nothing below blocks writing the other tasks.

| ID | Question | Why it is open | Recommendation |
|---|---|---|---|
| Q1 | What does Cancel on the lock overlay do? | The docs disagree. [p1-topic-startup-lock-security.md](p1-topic-startup-lock-security.md) 2.3 says "overlay stays, app stays". [p1-behavior.md](p1-behavior.md) 11.8 says "closes the window and returns to the lock screen". [p1-ui-parity.md](p1-ui-parity.md) section 12 says "closes the overlay and returns to the lock screen". | Overlay stays, content stays blank, the app keeps running, with "Try again" and "Use passphrase" actions. Keep one wording in all three documents. Ctrl+Q still quits. |
| Q2 | Is the app lock in v1 at all? | Costs: T-36a-032 (L), T-36a-033 (L), T-36a-034 (M), plus a passphrase with no recovery path. | Ship lock as opt-in, default off, passphrase only, in P1. Fprintd is a later add-on. Forgotten passphrase means reinstalling or deleting `lock.json`; state this in the settings help. |
| Q3 | Should Kurobayomi check for updates against Aniyomi's GitHub releases? | [p1-tasks.md](p1-tasks.md) T-36a-022 and T-36a-023 hard-code `https://github.com/aniyomiorg/aniyomi/releases/tag/v<version>`. Following it would prompt users to install Aniyomi builds. | Disable the app-release check until Kurobayomi has its own release repository, and point `GITHUB_REPO` at that repository then. Extension update checks (areas 18, 19) stay if extension repos are kept. Change the acceptance text of T-36a-022 when this is decided. |
| Q4 | Keep the aniyomi.org help links? | Four links: `https://aniyomi.org/docs/guides/getting-started` (GuidesStep), `https://aniyomi.org/docs/faq/storage` (StorageStep help), `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare` (Cloudflare banner), plus `Constants.URL_HELP` in area 53. | Keep only the Cloudflare troubleshooting link (it describes the same challenge). Replace the other two with the Kurobayomi README anchor or hide them. Decide before T-36a-026, T-36a-028, T-36a-050 are implemented. |
| Q5 | Start screen: live or restart-only? | Kotlin reads it once at object init (behaviour 2.1). | Live (applied on next Home build). Confirm, since it changes the Kotlin behaviour. |
| Q6 | Keep the four NavStyle variants, or fix the bar to one layout? | Backup files carry `bottom_rail_nav_style`; area 21 and 22 restore it. | Keep all four variants with the Kotlin default, because the names are part of backups. |
| Q7 | Show `AppTheme` and AMOLED settings on Linux when they do nothing? | Settings area 47 would otherwise show dead options. | Keep stored, show a note in Settings that the GTK theme sets the colours (area 47 to implement the note). |
| Q8 | Register `tachiyomi://` and `aniyomi://` schemes in the `.desktop` file? | Only needed for add-repo and extension-store links from other apps. | Register both; the handlers are small (T-36a-014). Remove if repo links are not used. |
| Q9 | Who owns `Navigator.kt`, `AuthenticatorUtil.kt`, `DisplayExtensions.kt`, `WindowExtensions.kt`? | These files are read for exact behaviour here. Their inventory rows are in areas 34 and 52, not in this area's 42 rows. | Area 36 owns the behaviour, areas 34 and 52 keep the file rows. The lead should confirm so nobody ports them twice. |

### 4.3 Priority corrections needed in the task list

These are P0 tasks that depend on P1 tasks, or a P0 need that sits at P1. Each needs a decision before the P0 critical path can be built.

- T-36a-025 (P0, onboarding theme step) depends on T-36a-029 (P1, app theme application). Promote T-36a-029 to P0, effort M, or demote T-36a-025.
- T-36a-061 (P0, base activity and theming lifecycle) depends on T-36a-029 (P1). Same fix as above.
- T-36a-060 (P0, entry open routing from tabs) depends on T-36a-009 (P1, tab routing events), which depends on T-36a-010 (P1, notification dismiss, area 35). Promote T-36a-009 to P0. T-36a-009 also lists T-36a-010 (P1, notification dismiss) as a dependency, so either promote T-36a-010 too (S) or remove that dependency if the routing events do not need dismiss handling.
- T-36a-049 (P0, WebView screen) has no dependency on the engine task T-36a-052 (P1, L, WebKitGTK engine, settings, cookie bridge). A WebView screen without an engine cannot pass a Cloudflare check. Promote T-36a-052 to P0, and add T-36a-052 as a dependency of T-36a-049.
- T-36a-058 (P2, player router) decides whether a tapped episode opens the internal player or the external player. Anime playback from an entry page is a P0 daily-use path. Promote T-36a-058 to P0 (M), since T-36a-060 routes into it.
- T-36a-020 (P2, startup gate) is cosmetic (splash equivalent). Keep at P2.

### 4.4 Risks that shape the decisions

- WebKit and Cloudflare: WebKitGTK may not pass every Cloudflare challenge even with the right cookies. The mitigation is the Cloudflare banner plus the open-in-browser action (T-36a-054). Do not promise a bypass in the UI copy.
- Cookie store split: WebKit cookies and the reqwest cookie jar (area 12) can diverge. [p1-rust-design.md](p1-rust-design.md) pitfall 5 requires one source of truth per host or sync on navigation finish. Area 12 must confirm which.
- GTK main thread: the Kotlin `runBlocking` calls (`Migrator.awaitAndRelease`, external player result) must become awaited futures. A `block_on` in a GTK callback freezes the window (pitfall 2).
- Window snapshots: compositors may keep the last window image for the task switcher. The lock must blank the content widgets before presenting the overlay (T-36a-034, edge E54). This is weaker than FLAG_SECURE and is documented as a loss.
- Passphrase loss: no recovery. Q2 covers it.
- Open before activate: GApplication `open` can arrive before the window exists. Requests are queued and applied in arrival order (edge E53, race R2).
- Live preference watches: badges, incognito, downloaded-only and start tab depend on area 02 providing change streams (`tokio::sync::watch` or equivalent). If area 02 gives only synchronous reads, every banner and badge becomes a polling loop. Confirm with area 02.
- Panic on unknown inputs: every parse of `type`, `url`, `tachibk` path and scheme must return `RouteError` and show a toast, never `unwrap` (pitfall 3).

### 4.5 Android-only features dropped (what the user loses)

| Feature | Kotlin location | User loses |
|---|---|---|
| FLAG_SECURE (`secure_screen_v2`) | `util/view/WindowExtensions.kt`, `SecureActivityDelegate.kt` | Protection from other apps capturing the window and from the Android recents thumbnail (partly mitigated by the blank overlay). |
| Biometric prompt | `AuthenticatorUtil.kt`, `UnlockActivity.kt` | Fingerprint and device-credential reuse. Passphrase or fprintd only. |
| Splash screen art and exit animation | `MainActivity.kt` (lines 162, 318 to 322, 424 to 458) | Splash art. A spinner window stays for up to 500 ms, at most 5000 ms. |
| Edge-to-edge, navigation scrim | `MainActivity.kt` 190 to 198; `DisplayExtensions.kt` 70 to 73 | None; GTK draws the title bar. |
| Assist content | `MainActivity.kt` 348 to 355; `WebViewActivity.kt` 93 to 96 | Assistant page context. |
| Battery optimisation and unknown app sources rows | `PermissionStep.kt` 80 to 82, 100 to 111 | None; no Linux concept. |
| Notification permission row | `PermissionStep.kt` 85 to 98 | Replaced by a status row. |
| TV-box folder creation | `StorageStep.kt` 65 to 79 | TV-box support. |
| Share sheet | `WebViewScreenModel.kt` 47 to 53; `WebViewActivity.kt` 112 to 118 | Share targets. Replaced by clipboard copy. |
| Marquee subtitle | `AppBar.kt` 166 to 211 | Scrolling long URLs. Replaced by ellipsis plus tooltip. |
| Haptics on long press | `ItemDownloadIndicator.kt`; `commonClickable` | Vibration only. |
| Monet dynamic colour | `ThemingDelegate.kt`; `UiPreferences` default | Wallpaper-based palette. |
| Swipe-dismiss on sheets | `AdaptiveSheet.kt` | Swipe gesture. Escape closes sheets. |
| Circular reveal, NumberPicker, SeekBar, Spinner, ViewPager widgets | `widget/*.kt` | Decorative animation only. |
| IME "no personalised learning" in incognito | `TachiyomiTextInputEditText.kt` | Keyboard learning suppression. Incognito still shown in banner. |
| Deep-link trampoline activities | `DeepLinkAnimeActivity.kt`, `DeepLinkMangaActivity.kt` | Nothing user-visible; routed through `route_request`. |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Used by tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | `gtk::Application` id, tokio runtime creation, logging setup, `KUROBAYOMI_SMOKE` env handling | T-36a-001 |
| area:02-preferences-system | Typed getters and setters and change streams for `start_screen`, `bottom_rail_nav_style`, `tablet_ui_mode`, `pref_theme_mode_key`, `pref_app_theme`, `pref_theme_dark_amoled_key`, `use_biometric_lock`, `lock_app_after`, `secure_screen_v2`, `relative_time_v2`, `app_date_format`, `library_show_updates_count`, the `__APP_STATE_*` KV store (lock timestamp, incognito, downloaded-only, onboarding flag, storage dir) | T-36a-004, 006, 018, 024, 030, 031, 032, 036, 047 |
| area:11-app-migrations | Migration future and `didMigration` result | T-36a-001, 021 |
| area:12-network-stack | Cookie jar sync, user agent, source HTTP headers, `WebViewUtil` equivalent (`MINIMUM_WEBVIEW_VERSION` 118 is Android-only and does not apply) | T-36a-049, 052, 053 |
| area:13-core-util-storage-archive-torrent | Default storage directory, folder chooser semantics | T-36a-026 |
| area:07-domain-entries-items, area:05-data-repos-entries-items | `get_by_url_and_source`, `get_by_url_and_parent`, `NetworkToLocal*`, `SyncChaptersWithSource`, `UpdateAnimeFromRemote.awaitEpisodesUpdate` | T-36a-056, 057 |
| area:15-source-api-manga, area:16-source-api-anime | `ResolvableSource`, `getUriType`, `getAnime`, `getEpisode`, `getChapterList`, `HttpSource.headers` | T-36a-053, 056, 057 |
| area:18-extensions-manga, area:19-extensions-anime | Extension update checks and counts (`ext_updates_count`, `animeext_updates_count`) | T-36a-007, 022 |
| area:22-backup-restore, area:21-backup-format-and-create | `RestoreBackupScreen(uri)`, `.tachibk` extension rule, backup of the preference keys listed in [p1-data-and-config.md](p1-data-and-config.md) section 3 | T-36a-014, 062 |
| area:23-downloads-manga, area:24-downloads-anime | Downloader indexing state (banner), download action enum, entry download menu | T-36a-018, 044, 046 |
| area:34-images-covers-cache | `ChapterCache.clear` for auto-clear on launch | T-36a-016 |
| area:35-data-misc-updater-notification-export | App-update check and download job, notification dismiss, `gio::Notification` plumbing | T-36a-010, 022, 023 |
| area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime, area:40-ui-browse-manga, area:41-ui-browse-anime, area:42-ui-updates-history-stats-storage-upcoming | Tab bodies, `AnimeScreen`, `MangaScreen`, global search screens, `UpdatesTab`, `HistoriesTab`, `StatsScreen`, storage screen | T-36a-002, 004, 040, 060 |
| area:45-player-core, area:46-player-controls-ui | `PlayerActivity.newIntent` equivalent, `ExternalIntents`, `HttpServerService` | T-36a-058, 059 |
| area:43-reader-core, area:44-reader-viewers | Reader open at chapter id | T-36a-057, 060 |
| area:47-settings-screens-general, area:48-settings-player-and-scripts | `SettingsScreen` destinations (More reselect, onboarding restore), `SearchableSettings` highlight key, lock and theme settings widgets | T-36a-028, 032, 062 |
| area:49-theme-and-design-system | CSS tokens and named GTK colours, `AppTheme` look-up | T-36a-029, 061 |
| area:51-i18n | Message keys listed in [p1-behavior.md](p1-behavior.md) section 16, plus two new keys: `copied_to_clipboard` (confirm exists) and `open_in_browser_failed` (proposal) | All UI tasks |
| area:53-platform-manifest-intents-workers | Shortcut and action names, `.desktop` file, scheme and MIME handlers | T-36a-011, 014, 055 |
| area:54-build-release-distribution-docs | `.desktop` install, `update-desktop-database` in packaging hook | T-36a-014 |

### 5.2 What others need from this area

| Consumer | What it needs | Provided by |
|---|---|---|
| Areas 37 to 42 (every tab and detail screen) | `Navigator` push, pop, replace, `pop_until_root`; `SelectTab`; `SearchLibrary(query)`; `AppBar`, `SearchToolbar`, `TabbedScreen`, `AdaptiveSheet`, `DropdownMenu`, `FloatingActionAddButton`, `DownloadIndicator`, `EmptyScreen`, relative date text | T-36a-002, 009, 037, 038, 039, 040, 041, 042, 043, 044, 045, 046, 047, 048 |
| Area 45, 46 (player) | `route_player(entry, child)` through one router (external player or internal) | T-36a-058, 059 |
| Area 43, 44 (reader) | Route to reader from a deep link or a tab entry | T-36a-057, 060 |
| Area 35 (updates and notifications) | `LaunchRequest::Notification` and `route_request` for dismiss; update prompt page | T-36a-010, 023 |
| Area 47 (settings) | Lock lifecycle (`on_app_stopped`, `on_app_started`, `unlock`), `LockConfig`, `AppTheme::parse`, `TabletUiMode`, `NavStyle` enum names, `StartScreen` enum names, toast sink | T-36a-030, 031, 032, 033, 036 |
| Area 12 (network stack) and the source areas | WebView page with headers and cookie bridge for login and Cloudflare challenges | T-36a-049, 051, 052, 053 |
| Area 22 (backup restore) | Route `RestoreBackup { uri }` | T-36a-014 |
| All areas | Toast sink `show(text, duration)` (ui-parity section 13), `StartupGate` readiness, `Badges` receivers | T-36a-007, 020, 037 |

---

## 6. Task summary

### 6.1 Counts (from `tasks.py export`, rows whose file starts with `36-ui-shell-navigation-security-webview/` in `plans/TASKS.csv`)

Total: 69 tasks.

| Priority | Count | Effort S | Effort M | Effort L | Effort XL |
|---|---|---|---|---|---|
| P0 | 23 | 11 | 12 | 0 | 0 |
| P1 | 37 | 19 | 15 | 3 | 0 |
| P2 | 4 | 3 | 1 | 0 | 0 |
| P3 | 5 | 5 | 0 | 0 | 0 |
| **Total** | **69** | **38** | **28** | **3** | **0** |

The lint of `36-ui-shell-navigation-security-webview/p1-tasks.md` reports `ok`, and every `area:` token in the file resolves to a real folder id. Task IDs are unchanged. The export run reports four errors in `40-ui-browse-manga/p1-tasks.md` (`area:41-ui-browse-manga-part-2` is not a real area id). Those are in area 40 and were not changed here.

### 6.2 P0 critical path (in order, after the priority corrections in section 4.3)

Dependency order. Tasks on the same line can run in parallel.

1. T-36a-001 process bootstrap and migration gate (M). Needs areas 01, 11, 02.
2. T-36a-002 root navigator (M). Needs T-36a-001 and area 37 for the page stub.
3. T-36a-004 home tab host and start screen (M). Needs T-36a-002.
4. T-36a-006 NavStyle and bar tab composition (S). Needs T-36a-004.
5. T-36a-005 adaptive chrome (M). Needs T-36a-004 and T-36a-006.
6. T-36a-031 tablet UI mode and breakpoints (S). Needs T-36a-005.
7. T-36a-008 tab reselect and back-to-start (S). Needs T-36a-005.
8. T-36a-009 tab routing events (promote to P0, M). Needs T-36a-004, and T-36a-010 (promote with it, or drop the dependency; see section 4.3).
9. T-36a-037 app bar (M). Needs T-36a-001. Parallel with steps 2 to 8.
10. T-36a-038 action mode (M), T-36a-039 search toolbar (M), then T-36a-040 tabbed screen (M). Need T-36a-037.
11. T-36a-047 relative date text (S) and T-36a-048 empty screen (S). Parallel with step 10.
12. T-36a-018 state banners (M) after T-36a-005, then T-36a-069 banner widget (S).
13. T-36a-030 theme mode to GTK colour scheme (S). Needs T-36a-001. Parallel.
14. T-36a-029 app theme application (promote to P0, M), then T-36a-061 base activity lifecycle (S).
15. T-36a-024 onboarding gate and wizard (M). Needs T-36a-002.
16. T-36a-025 theme step (S), T-36a-026 storage step (M), T-36a-028 guides step (S). Need T-36a-024. T-36a-027 is P2 and not on the path.
17. T-36a-052 WebKitGTK engine (promote to P0, L). Needs T-36a-001 and area 12. Then T-36a-049 WebView screen (M), which needs T-36a-037.
18. T-36a-060 entry open routing (S). Needs T-36a-009 and areas 38, 39.

### 6.3 Risks

| Risk | Impact | Mitigation | Task |
|---|---|---|---|
| P0 tasks depend on P1 tasks (section 4.3) | Critical path is wider than the P0 list suggests | Promote the four dependencies (T-36a-009, 029, 052, 058) | Priority review with the lead |
| WebKit does not pass a Cloudflare challenge | Some sources unusable from inside the app | Banner with browser fallback; no bypass claim | T-36a-050, 054 |
| Cookie store divergence (WebKit versus reqwest jar) | Login appears to work in WebView but source requests fail | One source of truth or sync on navigation finish | T-36a-052 with area 12 |
| Lock passphrase lost | User locked out of the app | Q2 decision; settings help text; lock is opt-in | T-36a-033 |
| Lock overlay shown too late | Content flashes before the lock | Build the overlay before the content page is shown (topic doc, pitfall 1) | T-36a-034, 035 |
| Update check points at Aniyomi | Users pushed to the wrong product | Q3 decision | T-36a-022, 023 |
| External links to aniyomi.org | Broken or foreign help pages | Q4 decision | T-36a-026, 028, 050 |
| Live preference streams missing in area 02 | Badges, banners and start tab need polling | Confirm with area 02 before T-36a-007 and T-36a-018 | Area 02 |
| Blocking calls on the GTK thread | Frozen window | Futures only; review in code review | All shell tasks |
| GApplication `open` before `activate` | Lost routing request | Queue until the window exists | T-36a-015 |
| Notification and portal absence on minimal desktops | Missing dialogs or notifications | Degrade silently; GTK toast fallback | T-36a-010, 026 |
| Unknown inputs from the desktop (`--type`, schemes, file names) | Crash on malformed input | `RouteError`, toast, no `unwrap` | T-36a-011 to 014 |

---

## 7. Definition of done for this area

Each item is an observable behaviour on a Linux desktop (Wayland or X11), checked by the T-36a-067 test suite or by hand.

1. Launching the app shows no window until the migration finishes. A second launch with a `.tachibk` path hands the file to the running instance and exits with status 0.
2. The tab bar shows exactly five entries for every `bottom_rail_nav_style` value, with More last. The entry named by the style is moved into More, and the bar hides when that moved tab is selected.
3. `start_screen` picks the first tab. Changing it in Settings applies on the next Home build.
4. Escape and mouse back: on a non-start tab they go to the start tab; on the start tab they do nothing; on a deeper page stack they pop one page. The Home root is never popped.
5. Clicking the already-selected More tab opens Settings.
6. The Updates tab badge shows the sum of anime and manga new updates (3 and 4 gives 7). It is hidden when `library_show_updates_count` is off or the count is 0. The Browse badge shows the extension update count. No badge is capped.
7. Window width of 760 px or more, or a maximised window, shows the sidebar. Narrower shows the bottom switcher. Reader and player pages hide the sidebar.
8. The banner stack shows Indexing, Downloaded only, Incognito, in that order. Incognito turns off on every launch.
9. `kurobayomi --search "x" --type manga` opens global manga search and the deep-link page. `--type both` shows an error toast and does not crash. An empty search does nothing.
10. `tachiyomi://add-repo?url=U` opens the manga extension repo page with U. A missing `url` does nothing.
11. Lock with `lock_app_after = 0`: after relaunch, the overlay appears and no content is visible. With 5 minutes: relaunch within 5 minutes does not lock; after 5 minutes it does. A wrong passphrase shows "Incorrect passphrase" and keeps the overlay. The correct passphrase unlocks. `lock.json` has mode 0600. Cancel behaves as decided in Q1.
12. Onboarding appears on a fresh config directory. Next is disabled on the storage step until a folder is chosen. Back is blocked until Finish. Finish records completion, and the next launch goes straight to the app.
13. The changelog appears once after a migration, in release builds only.
14. The WebView opens a source URL with the source's headers. The Cloudflare banner appears when the page contains `window._cf_chl_opt` or `Ray ID is`. `intent://` links do nothing. Clear cookies removes the host's cookies. Share copies the URL and shows a toast. Open in browser opens the default browser and never reopens Kurobayomi for its own schemes.
15. The deep-link page shows a spinner, then either goes to global search (no result), the entry page, or the player or reader at the child item. A failure shows an error with Retry.
16. The update page reads "New version available!" and shows the changelog with the checksum block removed. Accept starts the download job. This item is conditional on Q3.
17. Every shell page that is popped releases its widgets. A smoke test checks that the widget count returns to its pre-push value.
18. Dropped features show no broken UI: no FLAG_SECURE control, no battery or install-source rows, no share sheet in the WebView overflow menu.
19. The keyboard map in [p1-ui-parity.md](p1-ui-parity.md) section 14 works: Escape, Ctrl+F, Alt+1 to Alt+5 (proposal), Ctrl+, (proposal), Ctrl+L (proposal), Ctrl+Q.
20. The T-36a-067 shell test suite passes, and `cargo test` covers the pure functions named in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5.
