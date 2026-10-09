# Android / third-party API mapping, chunk 5 of 8

Source of the item list: `python3 plans/_tools/inventory.py items imports 5 8` (54 items). First item: `LifecycleCoroutineScope`. Last item: `Observable`. All paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`).

Each item is a class name imported by Aniyomi (`import <fully.qualified.Name>`). Counts are `files/occ.`: number of Kotlin/Java files that import that exact fully qualified name, and whole-word occurrences of the simple name inside those files (the import line counts, so 2 = one real use). `rg -c` style counts were produced by a script over every `.kt`/`.java` file in the clone. "Representative use" is the most informative `path:line`. Replacement choices assume: Rust stable, `gtk4` 0.11 (GTK 4.16+, no libadwaita), `tokio`, `reqwest`, `rusqlite`, `serde`, `libmpv`; target Linux (CachyOS/KDE, Wayland+X11).

Naming caveats. `LocalContext` and `LocalSize` here are the **Glance** (home-screen widget) ones (`androidx.glance.*`), not Compose's `LocalContext`. `LocalLifecycleOwner` is `androidx.lifecycle.compose.LocalLifecycleOwner`. `Network` is `android.net.Network` (not an Aniyomi class). `Notification`/`NotificationManager` are `android.app.*`. `Observable` is RxJava 1 `rx.Observable`. `Markdown` is `com.halilibo.richtext.markdown.Markdown`. `LoadState` is Paging 3 (`androidx.paging.LoadState`); `LoadingState` is the Accompanist-WebView one (`com.kevinnzou.web.LoadingState`).

Cross-references: `area:01-foundation-app-bootstrap`, `area:12-network-stack`, `area:22-backup-restore`, `area:23-downloads-manga`, `area:24-downloads-anime`, `area:26-library-update-jobs`, `area:27-tracking-core`, `area:35-data-misc-updater-notification-export`, `area:36-ui-shell-navigation-security-webview`, `area:43-reader-core`, `area:44-reader-viewers`, `area:45-player-core`, `area:49-theme-and-design-system`, `area:50-android-widgets`, `area:51-i18n`, `area:53-platform-manifest-intents-workers`.

## 1. Index

| # | Item | Fully qualified | Files/occ. | Replacement (one line) | Difficulty |
|---|---|---|---|---|---|
| 1 | LifecycleCoroutineScope | androidx.lifecycle.LifecycleCoroutineScope | 2/4 | dropped with the widget (else `glib::MainContext::spawn_local`) | trivial |
| 2 | LifecycleOwner | androidx.lifecycle.LifecycleOwner | 4/10 | `gtk::Window` `is-active` / `map` / `unmap` signals | moderate |
| 3 | LinearLayout | android.widget.LinearLayout | 3/12 | `gtk::Box` | trivial |
| 4 | LinearLayoutManager | androidx.recyclerview.widget.LinearLayoutManager | 3/6 | `gtk::ListView` / `gtk::ScrolledWindow` + adjustment | moderate |
| 5 | LinearOutSlowInInterpolator | androidx.interpolator.view.animation.LinearOutSlowInInterpolator | 1/2 | dropped (splash exit, API < 31 only) | trivial |
| 6 | LoadState | androidx.paging.LoadState | 8/34 | `enum PageState { Idle, Loading, Error(e) }` + manual pager | moderate |
| 7 | LoadingState | com.kevinnzou.web.LoadingState | 1/4 | `webkit6::WebView` `estimated-load-progress` + `gtk::ProgressBar` | trivial |
| 8 | LocalBroadcastManager | androidx.localbroadcastmanager.content.LocalBroadcastManager | 2/8 | `tokio::sync::mpsc` / `broadcast` channel | trivial |
| 9 | LocalContext | androidx.glance.LocalContext | 4/8 | dropped (Glance widget) | trivial |
| 10 | LocalLifecycleOwner | androidx.lifecycle.compose.LocalLifecycleOwner | 4/8 | the owning `gtk::Window` handle | trivial |
| 11 | LocalSize | androidx.glance.LocalSize | 2/4 | dropped (Glance widget); formula kept for an optional grid | trivial |
| 12 | LocaleListCompat | androidx.core.os.LocaleListCompat | 3/8 | `sys-locale` + `unic-langid`, `gettext` domain/`LANGUAGE` env | moderate |
| 13 | Log | android.util.Log | 1/2 | `tracing::error!` | trivial |
| 14 | LogCallback | com.arthenica.ffmpegkit.LogCallback | 1/2 | parse `ffmpeg` child-process stderr | moderate |
| 15 | LogPriority | logcat.LogPriority | 154/457 | `tracing::Level` | trivial |
| 16 | LogRedirectionStrategy | com.arthenica.ffmpegkit.LogRedirectionStrategy | 1/2 | pipe child stderr to `tracing` | trivial |
| 17 | LogcatLogger | logcat.LogcatLogger | 1/3 | `tracing-subscriber` with reloadable `EnvFilter` | trivial |
| 18 | Looper | android.os.Looper | 2/4 | `glib::MainContext::default()` / `glib::timeout_add_local` | trivial |
| 19 | MATCH_PARENT | android.view.ViewGroup.LayoutParams.MATCH_PARENT | 5/15 | `set_hexpand(true)` / `set_vexpand(true)` | trivial |
| 20 | MacrobenchmarkRule | androidx.benchmark.macro.junit4.MacrobenchmarkRule | 1/2 | dropped; optional `criterion` startup bench | trivial |
| 21 | ManagedActivityResultLauncher | androidx.activity.compose.ManagedActivityResultLauncher | 1/2 | `gtk::FileDialog::select_folder` future | trivial |
| 22 | Manifest | android.Manifest | 2/6 | no runtime permissions; check notification daemon | trivial |
| 23 | Markdown | com.halilibo.richtext.markdown.Markdown | 1/2 | `pulldown-cmark` -> `gtk::TextView` tags (or `webkit6`) | moderate |
| 24 | MaterialButton | com.google.android.material.button.MaterialButton | 1/2 | `gtk::Button` | trivial |
| 25 | MaterialContainerTransform | com.google.android.material.transition.platform.MaterialContainerTransform | 1/2 | dropped (shared-element transition) | trivial |
| 26 | MaterialDynamicColors | com.google.android.material.color.utilities.MaterialDynamicColors | 1/2 | `material-colors` crate (HCT) or custom port; or follow GTK theme | hard |
| 27 | MaterialTextView | com.google.android.material.textview.MaterialTextView | 1/2 | `gtk::Label` with Pango markup | trivial |
| 28 | Matrix | android.graphics.Matrix | 1/2 | `image::imageops::rotate90/270` | trivial |
| 29 | MediaScannerConnection | android.media.MediaScannerConnection | 1/2 | dropped (no media indexer needed) | trivial |
| 30 | MediaSession | android.media.session.MediaSession | 1/4 | MPRIS2 D-Bus service (`mpris-server` or `zbus`) | moderate |
| 31 | MediaStore | android.provider.MediaStore | 1/16 | `$XDG_PICTURES_DIR/<AppName>/...` + `std::fs` | trivial |
| 32 | MediaType | okhttp3.MediaType | 1/2 | `reqwest` `content-type` header / `mime` crate | trivial |
| 33 | Menu | android.view.Menu | 1/2 | `gio::Menu` | trivial |
| 34 | MenuItem | android.view.MenuItem | 5/10 | `gio::MenuItem` + `gio::SimpleAction` | trivial |
| 35 | MimeTypeMap | android.webkit.MimeTypeMap | 1/2 | `mime_guess` / `gio::content_type_*` | trivial |
| 36 | MotionEvent | android.view.MotionEvent | 14/60 | `gtk::GestureClick`, `GestureDrag`, `GestureZoom`, `GestureLongPress`, `EventControllerScroll` | hard |
| 37 | MultipartBody | okhttp3.MultipartBody | 1/3 | `reqwest::multipart::Form` | trivial |
| 38 | NO_POSITION | androidx.recyclerview.widget.RecyclerView.NO_POSITION | 1/2 | `Option<usize>` / `gtk::INVALID_LIST_POSITION` | trivial |
| 39 | Network | android.net.Network | 1/3 | `gio::NetworkMonitor` | moderate |
| 40 | NetworkCallback | android.net.ConnectivityManager.NetworkCallback | 1/2 | `NetworkMonitor::connect_network_changed` | trivial |
| 41 | NetworkCapabilities | android.net.NetworkCapabilities | 4/16 | `NetworkMonitor` connectivity/metered + NetworkManager D-Bus | moderate |
| 42 | NetworkRequest | android.net.NetworkRequest | 2/4 | `struct NetworkConstraints` evaluated against `NetState` | trivial |
| 43 | NetworkType | androidx.work.NetworkType | 5/12 | same `NetworkConstraints` | trivial |
| 44 | Notification | android.app.Notification | 3/6 | `gio::Notification` | moderate |
| 45 | NotificationChannelCompat | androidx.core.app.NotificationChannelCompat | 1/4 | static `Channel` table + per-channel setting | moderate |
| 46 | NotificationChannelGroupCompat | androidx.core.app.NotificationChannelGroupCompat | 1/4 | group heading in settings only | trivial |
| 47 | NotificationCompat | androidx.core.app.NotificationCompat | 9/41 | `gio::Notification` / `notify-rust` builder | hard |
| 48 | NotificationManager | android.app.NotificationManager | 1/2 | `gio::Application::{send,withdraw}_notification` | trivial |
| 49 | NotificationManagerCompat | androidx.core.app.NotificationManagerCompat | 4/14 | same | moderate |
| 50 | NotificationWithIdAndTag | androidx.core.app.NotificationManagerCompat.NotificationWithIdAndTag | 1/2 | `(String id, Notification)` batch | trivial |
| 51 | NumberPicker | android.widget.NumberPicker | 1/2 | `gtk::SpinButton` (class is dead code) | trivial |
| 52 | ORIENTATION_LANDSCAPE | android.content.res.Configuration.ORIENTATION_LANDSCAPE | 2/4 | `width > height` of the window | trivial |
| 53 | ORIENTATION_PORTRAIT | android.content.res.Configuration.ORIENTATION_PORTRAIT | 1/2 | `width <= height` of the window | trivial |
| 54 | Observable | rx.Observable | 11/46 | `async fn` + `Result`; Rx surface dropped | moderate |

Crates named in this chunk: `gtk4`, `gio`, `glib`, `webkit6`, `tracing`, `tracing-subscriber`, `tracing-appender`, `reqwest` (features `multipart`, `stream`), `tokio`, `futures`, `image`, `mime`, `mime_guess`, `pulldown-cmark`, `sys-locale`, `unic-langid`, `notify-rust` (optional), `zbus`, `mpris-server` (optional), `material-colors` (optional), `thiserror`.

---

## 2. Lifecycle, threading, scopes

### 2.1 LifecycleCoroutineScope
* OBSERVED: only the two Glance widget managers take it: `presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt:22` (`fun Context.init(scope: LifecycleCoroutineScope)`) and the anime twin (`.../entries/anime/AnimeWidgetManager.kt:22`). 2 files / 4 occ. `App.kt:150-157` passes `ProcessLifecycleOwner.get().lifecycleScope`, so the collection lives as long as the process. The manager combines `getUpdates.subscribe(read = false, after = DateLimit.toEpochMilli())` with `securityPreferences.useAuthenticator().changes()`, de-duplicates by `(second, set of chapterId)` and calls `MangaUpdatesGridGlanceWidget().updateAll(this)` and `MangaUpdatesGridCoverScreenGlanceWidget().updateAll(this)`, catching any `Exception` with `logcat(LogPriority.ERROR, e) { "Failed to update widget" }`.
* Replacement: the Android home-screen widget is DROPPED (see `Alignment`/`Box` in `catalogs/android-api-mapping-01.md` and `area:50-android-widgets`). User loses: the "recent updates" cover grid on the launcher. Proposal: if a desktop equivalent is wanted later, a KDE Plasma widget is out of scope; the Updates tab already shows the same data.
* If a task needs a "lifetime = process" scope in Rust, use a `tokio` runtime owned by `AppContext` (`area:01-foundation-app-bootstrap`) and `glib::MainContext::default().spawn_local` for UI-thread work.
* Difficulty trivial. Risk: none.

### 2.2 LifecycleOwner
* OBSERVED (4 files / 10 occ.): (a) `app/src/main/java/eu/kanade/tachiyomi/App.kt:214,218` implements `DefaultLifecycleObserver`: `onStart` -> `SecureActivityDelegate.onApplicationStart()`, `onStop` -> `SecureActivityDelegate.onApplicationStopped()` (process foreground/background); (b) `app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt` (`onCreate` -> secure-screen flow, `onResume` -> app lock); (c) `app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt:58` and `app/src/main/java/eu/kanade/presentation/util/Permissions.kt:23`: `onResume` re-reads permission state after the user returns from system settings.
* App-lock algorithm that must be preserved (`SecureActivityDelegate.kt`, companion): `requireUnlock = true` at process start. `onApplicationStopped`: if `useAuthenticator` is false return; if not currently authenticating: if `requireUnlock` return; if `lockAppAfter > 0` store `lastAppClosed = System.currentTimeMillis()`. `onApplicationStart`: if not authenticating and not `requireUnlock`: `requireUnlock = when (lockAppAfter) { -1 -> false /*Never*/; 0 -> true /*Always*/; else -> lastAppClosed + lockAppAfter * 60_000 <= now }`; then `lastAppClosed.delete()`. `unlock()` sets `requireUnlock = false`. `setAppLock` on resume: if `useAuthenticator` and the device supports it and `requireUnlock`, start `UnlockActivity`; if auth is not supported, `useAuthenticator` is forcibly set to false.
* Replacement: app-foreground = "any Kurobayomi window `is-active`". Track with `gtk::Window::connect_is_active_notify` on every top-level; on the all-windows-inactive edge call `on_application_stopped`, on the first-window-active edge call `on_application_start`. Resume-style rechecks = the same `is-active` rising edge. Authenticator (BiometricPrompt) is dropped (see catalog 01); Proposal: when `useAuthenticator` is on, show an in-app lock screen that asks for the user's login password through PAM (`pam-client` crate) or `org.freedesktop.portal.Secret`-free fallback: a locally salted `argon2` PIN stored in settings. Keep the three `lockAppAfter` values (-1, 0, N minutes) verbatim for backup/restore parity.
* Difficulty moderate. Risks: on Wayland/X11 a window can lose focus to a dialog of the same app (file chooser); do not treat transient focus loss to a child dialog as "app stopped" (check `gtk::Window::list_toplevels` for any active window of the app). Android `lockAppAfter` store key `lastAppClosed` is app-state (`Preference.appStateKey`, not backed up).

### 2.3 LocalLifecycleOwner
* OBSERVED: `LocalLifecycleOwner.current` is read in `PermissionStep.kt:52`, `Permissions.kt:17`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:100` and `.../entries/manga/MangaScreen.kt:91`; in the entry screens it is passed as `MangaScreenModel(context, lifecycleOwner.lifecycle, mangaId, fromSource)` so the model can observe `Lifecycle.Event`s (refresh on resume, stop downloads UI on destroy).
* Replacement: each screen controller receives a `glib::WeakRef<gtk::Window>`/a `gtk::Widget` and uses `connect_map` / `connect_unmap` / `connect_destroy` (all `Widget` signals) as ON_START/ON_STOP/ON_DESTROY. Unsubscribe flows by dropping the `glib::SignalHandlerId` and the `tokio` `JoinHandle`.
* Difficulty trivial. Risk: leaking closures that capture `Rc<Screen>`; use `glib::clone!(#[weak] ...)`.

### 2.4 Looper
* OBSERVED (2 files / 4 occ.): (a) `app/src/main/java/eu/kanade/tachiyomi/App.kt:225` inside `getPackageName()` override: reads `Looper.getMainLooper().thread.stackTrace` and, if the stack contains `org.chromium.base.BuildInfo` with method `getAll`/`getPackageName`/`<init>`, returns `WebViewUtil.spoofedPackageName(...)`, so Chromium WebView sends a spoofed `X-Requested-With` (used to get past Cloudflare/anti-bot checks that blocklist the app id). (b) `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt:20`: `Handler(Looper.getMainLooper())` posts the long-tap runnable with `postDelayed(longTapFn, longTapTime)`.
* Replacement: (b) `glib::timeout_add_local_once(Duration::from_millis(500), ...)` (see `MotionEvent`, 4.1 for the exact timing); cancel with the returned `glib::SourceId::remove()`. (a) is Android-specific and DROPPED: WebKitGTK sends no `X-Requested-With`; the WebView screen (`area:36-ui-shell-navigation-security-webview`) sets the user agent from the same `defaultUserAgentProvider` the HTTP client uses so Cloudflare cookies line up (see `area:12-network-stack`). User loses nothing on Linux.
* Difficulty trivial.

---

## 3. Views, layouts and list plumbing

### 3.1 LinearLayout
* OBSERVED (3 files / 12 occ.): reader chapter-transition views. `ui/reader/viewer/pager/PagerTransitionHolder.kt:34` extends `LinearLayout(readerThemedContext)` with `orientation = VERTICAL`, `gravity = Gravity.CENTER`, `setPadding(64.dpToPx, 0, 64.dpToPx, 0)`, and an inner `pagesContainer` `LinearLayout` with `LayoutParams(MATCH_PARENT, WRAP_CONTENT)`; `ui/reader/viewer/webtoon/WebtoonAdapter.kt:123` creates a bare `LinearLayout` for `TRANSITION_VIEW`; `webtoon/WebtoonTransitionHolder.kt:27` holds it as `val layout: LinearLayout`.
* Replacement: `gtk::Box::new(Orientation::Vertical, 0)` with `set_halign(Center)`/`set_valign(Center)` and 64 px margins (`set_margin_start(64)`, `set_margin_end(64)`; GTK logical pixels = Android dp at 1x). The transition view shows the previous/next chapter titles and a status (`Loading`, `Error` + retry, `Downloaded`/`Not downloaded` hint); see `area:44-reader-viewers`.
* Difficulty trivial.

### 3.2 LinearLayoutManager
* OBSERVED (3 files / 6 occ.): (a) download queue: `ui/download/manga/MangaDownloadQueueScreen.kt:62` and the anime twin set `layoutManager = LinearLayoutManager(context)` on a FlexibleAdapter `RecyclerView` with `isHandleDragEnabled = true` (drag-reorder) plus per-item popup menu `R.id.move_to_top`, `move_to_bottom`, `move_to_top_series`, `move_to_bottom_series` (`MangaDownloadQueueScreenModel.kt:68-100`); (b) `ui/reader/viewer/webtoon/WebtoonRecyclerView.kt:74-79` reads `findLastVisibleItemPosition()` / `findFirstVisibleItemPosition()` in `onScrolled`, and `onScrollStateChanged` sets `atLastPosition = visibleItemCount > 0 && lastVisibleItemPosition == totalItemCount - 1` and `atFirstPosition = firstVisibleItemPosition == 0`.
* Replacement: (a) `gtk::ListView` over a `gio::ListStore` of `DownloadItem` objects grouped by series (`gtk::TreeListModel` or a flat list with a header row); drag-and-drop reorder via `gtk::DragSource` + `gtk::DropTarget` with a `glib::Value` of the item id; the four move actions are `gio::SimpleAction`s (`download.move-top`, `download.move-bottom`, `download.move-series-top`, `download.move-series-bottom`) on a `gtk::PopoverMenu`. Reorder semantics to keep: series moves use `partition { item.download.manga.id == it.manga.id }` then `selected + others` (top) or `others + selected` (bottom); item moves operate inside the item's header group only. (b) The webtoon strip is a `gtk::ScrolledWindow` containing a vertical `gtk::Box` of page widgets (not a `ListView`: variable-height lazy images need exact control); first/last visible = iterate children and compare `child.compute_bounds(&viewport)` against `0..viewport_height`. "Last end-visible" (see `NO_POSITION`) = the last child whose bottom edge is `<= viewport_bottom` or whose top is `< viewport_top`.
* Difficulty moderate. Risks: GTK `ListView` recycles rows, so per-row subscriptions must be bound in `bind` and dropped in `unbind`.

### 3.3 NO_POSITION
* OBSERVED: `androidx.recyclerview.widget.WebtoonLayoutManager.kt:51` `return NO_POSITION` (`-1`) from `findLastEndVisibleItemPosition()` when no child qualifies (loop `for (i in childCount - 1 downTo 0)` with condition `childEnd <= end || childStart < start`). The caller `WebtoonViewer.refreshAdapter` (`WebtoonViewer.kt:354-358`) then does `adapter.notifyItemRangeChanged(max(0, position - 3), min(position + 3, adapter.itemCount - 1))`.
* Also relevant: `WebtoonLayoutManager` disables item prefetch (`isItemPrefetchEnabled = false`) and returns `extraLayoutSpace` = `heightPixels * 3 / 4` (`WebtoonViewer.kt:53`) so images load roughly three quarters of a screen early; RecyclerView cache size `RECYCLER_VIEW_CACHE_SIZE = 4` (double the default).
* Replacement: `Option<usize>`; preload window = `viewport_height * 3 / 4` above and below the viewport; keep up to 4 off-screen page widgets alive before recycling.
* Difficulty trivial. Risk: refresh window math must clamp (`position + 3` capped at `len - 1`); with `None` skip the refresh.

### 3.4 MATCH_PARENT
* OBSERVED (5 files / 15 occ.): `ui/reader/viewer/ReaderPageImageView.kt:273,388` (`addView(pageView, MATCH_PARENT, MATCH_PARENT)`), `PagerTransitionHolder.kt:50`, `webtoon/WebtoonPageHolder.kt:99` (`FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT)` with `bottomMargin = 15.dpToPx` when the viewer is not continuous, and `marginStart = marginEnd = screenWidth * (config.sidePadding / 100f)`), `webtoon/WebtoonViewer.kt`.
* Replacement: `widget.set_hexpand(true); widget.set_vexpand(true)`; WRAP_CONTENT = default GTK natural size. Webtoon side padding: `margin = window_width * side_padding / 100` applied as `set_margin_start/end` on the page frame; 15 px bottom margin for paged-not-continuous mode.
* Difficulty trivial.

### 3.5 NumberPicker
* OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/widget/MinMaxNumberPicker.kt:14` subclasses `NumberPicker`. It is declared with `R.styleable.MinMaxNumberPicker` (`app/src/main/res/values/attrs.xml:4`) but grep over `*.kt` and `res` finds no instantiation or layout use: it is DEAD CODE. Its logic: when `setDisplayedValues` contains a value whose first char is not a digit it sets `descendantFocusability = FOCUS_BLOCK_DESCENDANTS` (no keyboard entry); after layout it forces the inner `EditText` to `InputType.TYPE_CLASS_NUMBER`.
* Replacement: drop. If a numeric stepper is needed elsewhere use `gtk::SpinButton::with_range(min, max, 1.0)` with `set_numeric(true)`.
* Difficulty trivial.

### 3.6 MaterialButton
* OBSERVED: `ui/reader/viewer/ReaderButton.kt:18` `class ReaderButton ... : MaterialButton`. `onTouchEvent`: `viewer?.pager?.setGestureDetectorEnabled(false)` on every touch, re-enable on `ACTION_UP`, so tap-zone navigation of the pager does not fire while a button inside a transition page (retry/open-in-browser) is pressed.
* Replacement: `gtk::Button`. In GTK the child `Button` claims the click gesture first (`gtk::PropagationPhase::Target` default) so the page-level `GestureClick` on the container does not fire when set up in the bubble phase; no custom class needed. Test: pressing "Retry" on the transition page must not flip the page.
* Difficulty trivial.

### 3.7 MaterialTextView
* OBSERVED: `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/OpenSourceLibraryLicenseScreen.kt:74` wraps `MaterialTextView(it)` in `AndroidView` and sets `it.text = HtmlCompat.fromHtml(html, HtmlCompat.FROM_HTML_MODE_COMPACT)` for license text inside a `verticalScroll` with 16.dp padding.
* Replacement: `gtk::Label` (`set_wrap(true)`, `set_selectable(true)`, `set_xalign(0.0)`) in a `gtk::ScrolledWindow`. License HTML must be converted to Pango markup (`<b>`,`<i>`,`<u>`, `<a href>` are valid Pango; `<p>`/`<br>` -> `\n\n`/`\n`). Proposal: generate the license list at build time from `cargo about` (Rust deps) plus a hand-written list for libmpv/GTK, replacing the Android `aboutlibraries` JSON.
* Difficulty trivial. Risk: Pango markup is strict; always `glib::markup_escape_text` unknown text.

### 3.8 Menu
* OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt:67` `View.popupMenu(@MenuRes menuRes, initMenu: (Menu.() -> Unit)? = null, onMenuItemClick: MenuItem.() -> Unit)` builds a `PopupMenu(context, this, Gravity.NO_GRAVITY, R.attr.actionOverflowMenuStyle, 0)`, inflates an XML menu, lets the caller hide/enable items, and always consumes the click.
* Replacement: `gio::Menu` (model) shown through `gtk::PopoverMenu::from_model(Some(&menu))` parented with `set_parent(&widget)`; `initMenu` = build the model conditionally in Rust. Menu XML resources are replaced by Rust builders (`menu.append(Some(label), Some("win.action-name"))`).
* Difficulty trivial.

### 3.9 MenuItem
* OBSERVED (5 files / 10 occ.): the download-queue `onMenuItemClick(position, menuItem)` (`ui/download/manga/MangaDownloadAdapter.kt:25`, `MangaDownloadQueueScreenModel.kt:68`, anime twins) and `ViewExtensions.kt:67`. Item ids used: `R.id.move_to_top`, `R.id.move_to_bottom`, `R.id.move_to_top_series`, `R.id.move_to_bottom_series`.
* Replacement: each id becomes a `gio::SimpleAction` with a string/`u32` parameter (the download's id) added to the queue view's `gio::SimpleActionGroup`; menu entries are `gio::MenuItem::new(Some(label), Some("queue.move-top::<id>"))` via `set_action_and_target_value`.
* Difficulty trivial.

---

## 4. Input and gestures

### 4.1 MotionEvent
* OBSERVED (14 files / 60 occ.). Uses, with the exact behavior to port:
  1. `GestureDetectorWithLongTap` (`ui/reader/viewer/GestureDetectorWithLongTap.kt`): custom long-tap that does not conflict with quick-scale. `slop = ViewConfiguration.scaledTouchSlop`, `longTapTime = getLongPressTimeout()` (Android default 500 ms), `doubleTapTime = getDoubleTapTimeout()` (default 300 ms). `ACTION_DOWN`: remember the event; only if `ev.downTime - lastUp > doubleTapTime` store `downX/downY` and `postDelayed(longTapFn, longTapTime)`. `ACTION_MOVE`: cancel if `abs(dx) > slop || abs(dy) > slop`. `ACTION_UP`: `lastUp = eventTime`, cancel. `ACTION_CANCEL` and `ACTION_POINTER_DOWN`: cancel.
  2. `ui/reader/viewer/pager/Pager.kt:23-90`: `tapListener` (from `onSingleTapConfirmed`), `longTapListener` (on confirmed long tap, if listener returns true perform `HapticFeedbackConstants.LONG_PRESS`), `dispatchTouchEvent` feeds the detector only when `isGestureDetectorEnabled`; `onInterceptTouchEvent` swallows `IllegalArgumentException` (ViewPager multi-touch crash).
  3. `ui/reader/viewer/webtoon/WebtoonFrame.kt:51-110`: `ScaleGestureDetector` + `GestureDetector` (fling) feeding `WebtoonRecyclerView`; touch coordinates are clamped into the recycler's hit rect inset by 1 px (so gestures outside the strip still scroll it); `doubleTapZoom` toggles `scaleDetector.isQuickScaleEnabled`; `onFling` -> `recycler.zoomFling(vx, vy)`.
  4. `ui/reader/viewer/ReaderPageImageView.kt:364-380`: `PhotoView` with `setScaleLevels(1F, 2F, MAX_ZOOM_SCALE)`, `MAX_ZOOM_SCALE = 5F`, double tap toggles: `if (scale > 1F) setScale(1F, e.x, e.y, true) else setScale(2F, e.x, e.y, true)`; single-tap confirmed -> `onViewClicked()`. Subsampling path: `setDoubleTapZoomStyle(ZOOM_FOCUS_CENTER)`, `maxScale = scale * MAX_ZOOM_SCALE` (line 278).
  5. `ui/reader/viewer/webtoon/WebtoonSubsamplingImageView.kt:17` returns `false` from `onTouchEvent` (all gestures handled by the strip).
  6. Mouse wheel: `ReaderActivity.kt:331` `dispatchGenericMotionEvent` -> `viewer.handleGenericMotionEvent`; `pager/PagerViewer.kt:424-436`: if `event.source and InputDevice.SOURCE_CLASS_POINTER != 0` and `ACTION_SCROLL`: `getAxisValue(AXIS_VSCROLL) < 0f` -> `moveDown()` else `moveUp()`, return true; webtoon returns `false` (lets the RecyclerView scroll natively).
  7. `ui/reader/ReaderNavigationOverlayView.kt:109`: any touch hides the overlay (fades with `FADE_DURATION = 1000L`).
  8. `app/src/main/java/eu/kanade/presentation/player/components/RepeatingIconButton.kt:53`: `pointerInteropFilter` sets `pressed = (action == ACTION_DOWN)`; while pressed, calls `onClick()` repeatedly with delay starting at `maxDelayMillis = 750`, each step `delay = max(minDelayMillis = 5, delay - delay * 0.25)` (`delayDecayFactor = .25f`); release (any non-DOWN action) stops it.
* Replacement (GTK4 controllers on the page container, `gtk::PropagationPhase::Bubble` unless noted):
  * tap = `gtk::GestureClick` (`set_button(1)`); single-tap-confirmed = emit after the `gtk-double-click-time` setting (default 400 ms) elapses without a second press; Proposal: keep the "ignore down if within doubleTapTime (300 ms) of the last up" rule using `glib::monotonic_time()`; long tap = own `glib::timeout_add_local_once(500 ms)` started on press (to keep Aniyomi's constant and avoid `GestureLongPress` interfering with zoom) and cancelled by movement beyond the `gtk-dnd-drag-threshold` setting (default 8 px, close to Android's 8 dp slop); pinch zoom = `gtk::GestureZoom` (touch/touchpad), ctrl+wheel for mouse; double-tap zoom = `GestureClick` with `n_press == 2` -> toggle `1.0 <-> 2.0` anchored at pointer, max zoom 5.0; fling = `gtk::GestureSwipe` ; wheel = `gtk::EventControllerScroll` with `ScrollFlags::VERTICAL` (negative dy = down) for the pager, kinetic for webtoon; the repeat button = `gtk::GestureClick` `pressed` starts a `glib::timeout_add_local` chain with the same 750 -> 5 ms x0.75 decay, `released`/`stopped`/`cancel` stops it; ReaderButton conflict = child widgets claim events (see MaterialButton).
  * Haptic feedback: dropped (no hardware). Optional: none.
* Difficulty hard (reader feel is the product). Risks: GTK delivers touchpad pinch via `GestureZoom` but mouse users need explicit shortcuts; kinetic scrolling of `ScrolledWindow` conflicts with custom fling; set `set_kinetic_scrolling(true)` and let GTK do it, apply zoom only via `GestureZoom`/ctrl-wheel. Wayland touchpad "tap" vs "click" differ; test both.

---

## 5. Animation, theming, rich text, orientation

### 5.1 LinearOutSlowInInterpolator
* OBSERVED: `ui/main/MainActivity.kt:435` inside the pre-Android-12 splash exit (`SDK_INT < S && splashScreen != null`): `ValueAnimator.ofFloat(1F, 0F)` with `LinearOutSlowInInterpolator`, `duration = SPLASH_EXIT_ANIM_DURATION`, translating the content root by `value * 16.dpToPx` while the splash icon fades (with `FastOutSlowInInterpolator`).
* Replacement: dropped; Android splash screens do not exist. If a startup fade is wanted use `adw`-free CSS `transition: opacity 200ms`. User loses a 16 dp slide-in on app start.
* Difficulty trivial.

### 5.2 MaterialContainerTransform
* OBSERVED: `ui/reader/ReaderActivity.kt:537`: `if (window.sharedElementEnterTransition is MaterialContainerTransform) window.sharedElementEnterTransition.doOnEnd { setOrientation(...) } else setOrientation(...)`; the reason is an API 26 crash when the orientation is locked during the transition.
* Replacement: dropped; the shared-element cover-to-reader animation has no GTK equivalent. Apply the reader orientation lock (not applicable on desktop) immediately. User loses a decorative cover zoom animation.
* Difficulty trivial.

### 5.3 MaterialDynamicColors
* OBSERVED: `app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt:84` `val dynamicColors = MaterialDynamicColors()`; `generateColorSchemeFromSeed(context, seed, dark)` builds `SchemeContent(Hct.fromInt(seed), dark, contrast)` where `contrast = UiModeManager.contrast` on API 34+ else `0.0`, then maps the Material 3 roles (`primary`, `onPrimary`, `primaryContainer`, `onPrimaryContainer`, `inversePrimary`, `secondary*`, `tertiary*`, `background`, `onBackground`, `surface`, `onSurface`, `surfaceVariant`, `onSurfaceVariant`, `surfaceTint`, `inverseSurface`, `inverseOnSurface`, `error*`, `outline`, `outlineVariant`, `surfaceBright`, `surfaceDim`, `surfaceContainer`, `...High`, `...Highest`, `...Low`, `...Lowest`) to Compose colors via `dynamicColors.<role>().getArgb(scheme)`; `scrim = Color.Black` is fixed (lines 114). Used for the "Monet" wallpaper theme and custom-seed themes. NOTE a quirk: `tertiaryContainer` is assigned `dynamicColors.tertiary()` (`MonetColorScheme.kt:97`), not `tertiaryContainer()`; this looks like an upstream bug; do not copy it.
* Replacement: Kurobayomi does NOT theme itself: vanilla GTK follows the user's GTK/KDE theme (project rule; the user runs Tokyo Night everywhere). Therefore the Material 3 palette generator is DROPPED for the app chrome. Proposal (optional, P3): a "use accent color" is read from `gtk::Settings` / `org.freedesktop.appearance accent-color` via the XDG settings portal (`ashpd::desktop::settings`), and only the reader background colors (black/white/gray/automatic) remain app-controlled. If a custom palette is ever wanted, the `material-colors` crate implements HCT/`SchemeContent`.
* User loses: Monet/custom-seed themes and the 20-odd preset app themes. Replaced by: the system GTK theme (plus dark/light follow via `gtk-application-prefer-dark-theme`).
* Difficulty hard only if palettes are kept; as planned trivial. See `area:49-theme-and-design-system`.

### 5.4 Markdown
* OBSERVED: `app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt:55` renders GitHub release notes (`changelogInfo`) inside `RichText(style = RichTextStyle(stringStyle = RichTextStringStyle(linkStyle = SpanStyle(color = primary))))`, followed by a "Open in browser" `TextButton` (`MR.strings.update_check_open`); dialog buttons `update_check_confirm` / `action_not_now`.
* Replacement: `pulldown-cmark` parsing to `gtk::TextBuffer` with `gtk::TextTag`s (headings -> scale/weight, `**` -> bold, code -> monospace, links -> underlined tag + `gtk::GestureClick` that calls `gio::AppInfo::launch_default_for_uri`). Lists/quotes via paragraph indent tags. Tables and images are not rendered (show raw text). Proposal: because the Linux build updates through the package manager, this dialog is shown only for the "new version available" notice (`area:35-data-misc-updater-notification-export`).
* Difficulty moderate. Risk: GitHub release bodies contain HTML (`<details>`); strip unknown HTML events from `pulldown-cmark` instead of rendering them.

### 5.5 ORIENTATION_LANDSCAPE
* OBSERVED (2 files / 4 occ.): `presentation-core/src/main/java/tachiyomi/presentation/core/components/AdaptiveSheet.kt:64`: `maxWidth = if (orientation == ORIENTATION_LANDSCAPE) 600.dp else 460.dp`; `app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt:87`: `if (landscape) 720.dp else 420.dp` and `maxHeight = screenHeightDp * .95f`. Also indirectly `BrowseMangaSourceScreenModel.getColumnsPreference(orientation)` (`BrowseMangaSourceScreenModel.kt:127-134`): landscape uses `mangaLandscapeColumns` else `mangaPortraitColumns`; `columns == 0` means `GridCells.Adaptive(128.dp)` else `Fixed(columns)`.
* Replacement: `is_landscape = window.width() > window.height()` re-evaluated in `connect_default_width_notify`/`gtk::Widget::connect_map` plus the surface `layout` signal; sheet = a `gtk::Dialog`-free overlay (`gtk::Overlay` + `gtk::Box` with `set_size_request`/max width via `gtk::Widget::set_width_request` or `gtk::Frame` with CSS `max-width`; GTK CSS lacks `max-width` so clamp inside a custom `gtk::LayoutManager`/`gtk::Widget::measure` override). Keep the constants: 600/460 (settings sheet), 720/420 (player sheet, height 95% of window). Library grid: portrait vs landscape column prefs map to window aspect; 0 = auto with a minimum column width of 128 px.
* Difficulty trivial.

### 5.6 ORIENTATION_PORTRAIT
* OBSERVED: `ui/player/controls/components/panels/SubtitleSettingsPanel.kt:78`: portrait lays the three subtitle cards (typography, colors, miscellaneous; `rememberPagerState { 3 }`) as a column with a top bar titled `AYMR.strings.player_sheets_subtitles_settings_title`, 32.dp top offset; landscape uses a different two-pane constraint layout.
* Replacement: `gtk::Stack` with the three cards plus a `gtk::StackSwitcher`; "portrait" branch = narrow window (width <= height) shows the stack as tabs; wide window shows the cards side by side. See `area:46-player-controls-ui`.
* Difficulty trivial.

---

## 6. Paging, WebView state, messaging, permissions

### 6.1 LoadState
* OBSERVED (8 files / 34 occ.): browse screens `app/src/main/java/eu/kanade/presentation/browse/manga/BrowseMangaSourceScreen.kt:57-130` (+ anime twin, and `components/Browse{Manga,Anime}Source{List,ComfortableGrid,CompactGrid}.kt`). Logic: `errorState = list.loadState.refresh.takeIf { it is Error } ?: list.loadState.append.takeIf { it is Error }`; if `itemCount > 0 && error`: Snackbar with `formattedMessage`, action `MR.strings.action_retry`, `SnackbarDuration.Indefinite`, Retry -> `list.retry()`; if `itemCount <= 0 && error`: `EmptyScreen` with actions: local source -> `local_source_help_guide`; others -> `action_retry` (`refresh()`), `action_open_in_web_view`, `label_help`; if `itemCount == 0 && refresh is Loading` full-screen `LoadingScreen`; grids/lists show a bottom spinner item when `prepend`, `refresh` or `append` is `Loading`.
* Paging source (`data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt`): `PagingConfig(pageSize = 25)` (`BrowseMangaSourceScreenModel.kt:112`); `load(params)`: `page = params.key ?: 1`; calls `requestNextPage(page)` (popular `getPopularManga`, latest `getLatestUpdates`, search `getSearchManga(page, query, filters)`) on IO; an EMPTY page throws `NoChaptersException` (shown as `MR.strings.no_results_found`); `nextKey = if (hasNextPage) page + 1 else null`, `prevKey = null`; any exception becomes `LoadResult.Error(e)`. `hideInLibraryItems` filters `favorite` items; each fetched `SManga` is upserted (`networkToLocalManga.await`) and re-observed from the DB.
* Replacement: a small `BrowsePager` struct, no Paging crate: `{ source, kind: Popular|Latest|Search(query, filters), next_page: Option<u32> = Some(1), items: Vec<EntryId>, state: PageState }` with `PageState::{Idle, Loading{refresh: bool}, Error{error, append: bool}}`. `load_next()` runs when the grid scroll adjustment reaches `upper - page_size_px` (Proposal: trigger when fewer than 25/2 items remain below the viewport), appends results, sets `next_page = has_next.then(|| page + 1)`, errors on empty page (`NoResults`). Error UI mirrors the three cases above. `retry()` re-requests the failed page; `refresh()` clears and restarts at 1.
* Difficulty moderate. Risk: do not trigger a second load while `Loading`; Aniyomi's Paging dedups by key, Rust must guard with the state enum.

### 6.2 LoadingState
* OBSERVED: `app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt:191-200`: `LoadingState.Initializing` -> indeterminate `LinearProgressIndicator` at the bottom of the app bar; `LoadingState.Loading` -> determinate with `progress` (0..1); `Finished` -> nothing. Same screen shows a Cloudflare help link `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare`.
* Replacement: `webkit6::WebView::estimated_load_progress()` (f64 0..1) through `connect_estimated_load_progress_notify` plus `connect_load_changed` (`LoadEvent::Started` = initializing/pulse, `Finished` = hide) driving a `gtk::ProgressBar` (`pulse()` while progress == 0). Replace the help link with Kurobayomi's own docs URL (decision in `area:54-build-release-distribution-docs`).
* Difficulty trivial.

### 6.3 LocalBroadcastManager
* OBSERVED (2 files / 8 occ.): `app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/InstallerManga.kt:115,152,167` (+ anime twin) sends/receives intent action `"Installer.action.CANCEL_QUEUE"` with extra `"Installer.extra.DOWNLOAD_ID"` (Long) to cancel a queued extension install. `cancelQueue(downloadId)`: finds `queue.find { it.downloadId == id } ?: waitingInstall`, if `cancelEntry` succeeds removes it, continues the queue (`checkQueue`) and sets `InstallStep.Idle`. `checkQueue`: if not ready return; if queue empty `service.stopSelf()`; else `waitingInstall.compareAndSet(null, nextEntry)` then process.
* Replacement: the extension model changes (see `area:20-extension-strategy-redesign`); the install queue is an in-process `tokio::sync::mpsc::UnboundedSender<InstallCmd>` with `enum InstallCmd { Enqueue(Entry), Cancel(u64) }` consumed by one task that owns `queue: VecDeque<Entry>` and `waiting: Option<Entry>`; no broadcast needed. Preserve: single active install at a time; cancel of the active one proceeds with the next; state per id `Idle/Pending/Downloading/Installing/Installed/Error`.
* Difficulty trivial.

### 6.4 Manifest
* OBSERVED: `android.Manifest.permission.POST_NOTIFICATIONS` (API 33+) checked in `util/system/NotificationExtensions.kt:33,46` (silently returns if not granted) and requested in `PermissionStep.kt:60,96`. The onboarding Permission step also shows "install unknown apps" (`canRequestPackageInstalls`) and "ignore battery optimizations" (`PowerManager.isIgnoringBatteryOptimizations`) rows; `isComplete = true` always (the step never blocks onboarding).
* Replacement: Linux has no runtime permission model for a native app. Notifications: probe `org.freedesktop.Notifications` (`GetServerInformation`) at startup and when sending; if absent, skip silently (matches the Android early return) and show a single in-app hint once. The "install apps" and "battery" rows are dropped; if Kurobayomi ships as Flatpak, file-system and notification access come from the portal at first use (`ashpd`).
* Difficulty trivial.

---

## 7. Locale and logging

### 7.1 LocaleListCompat
* OBSERVED (3 files / 8 occ.): (a) `presentation/more/settings/screen/appearance/AppLanguageScreen.kt:52-60`: current = `AppCompatDelegate.getApplicationLocales().get(0)?.toLanguageTag() ?: ""` (empty = follow system); choosing sets `AppCompatDelegate.setApplicationLocales(if empty getEmptyLocaleList() else forLanguageTags(tag))`; the list is parsed from `R.xml.locales_config` (`<locale android:name="...">`, generated at build; not present in the clone) and each shows `LocaleHelper.getLocalizedDisplayName(tag)` (endonym, first letter upper-cased) and `getDisplayName(tag)` (device-language name). (b) `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:40-80`: normalizes `zh-CN` -> `zh-Hans`, `zh-TW` -> `zh-Hant`; `""` -> `LocaleListCompat.getAdjustedDefault()[0]`; default enabled source languages = `setOf("all", "en", Locale.getDefault().language)`; `getSimpleLocaleDisplayName()` = `LocaleListCompat.getDefault()[0].displayLanguage`. (c) `ui/player/utils/TrackSelect.kt:36`: if the preferred-audio/subtitle language list is empty falls back to `listOf(LocaleListCompat.getDefault()[0]!!)`.
* TrackSelect algorithm to port exactly (`TrackSelect.kt:20-75`): preferred langs = comma split, drop empty, trim; whitelist/blacklist (subtitles only) likewise; `locales = langs.map(::Locale).ifEmpty { [system locale] }`; `chosenLocale = first locale for which any track matches`; if none, return null; `filtered = tracks (with index) minus those whose name contains any blacklist entry (ignore case), keeping tracks matching chosenLocale`; result = first filtered whose name contains a whitelist entry (ignore case), else `filtered[0]`. `containsLang(track, locale)`: `localName = locale.getDisplayName(locale)`, `englishName = locale.getDisplayName(ENGLISH).substringBefore(" (")`, regex `\b<iso3>|<lang>\b` (ignore case) on `track.language`; true if the track name contains localName or englishName (ignore case) or the language matches.
* Replacement: system locale from `sys-locale::get_locales()` (reads `LC_ALL`/`LC_MESSAGES`/`LANG`/`LANGUAGE`); tags as `unic_langid::LanguageIdentifier`; language display names: `isolang` for English names + ISO 639-3 codes (needed for `containsLang`), endonyms from a static table compiled from the shipped locales (`area:51-i18n`). App language override = set `LANGUAGE` env and call `setlocale(LC_MESSAGES, ...)` before any widget is built, which requires a restart or rebuilding the UI (user is told "restart to apply"). Keep the two `zh` aliases and the stored value format (`""` = system).
* Difficulty moderate. Risk: Java's `Locale.getDisplayName` has no direct Rust equal; track names from mpv include "English", "eng", "en" so keep both English-name and ISO-3 matching.

### 7.2 Log
* OBSERVED: only `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt:123`: `Log.e("PreferenceRestorer", "Failed to restore preference <$key>", e)` inside a per-key try/catch; a failing key is logged and the loop continues.
* Replacement: `tracing::error!(target: "preference_restorer", key, error = %e, "Failed to restore preference")` and `continue`. See `area:22-backup-restore`.
* Difficulty trivial.

### 7.3 LogPriority
* OBSERVED (154 files / 457 occ., the most used item of the chunk): `logcat.LogPriority` enum `VERBOSE, DEBUG, INFO, WARN, ERROR, ASSERT`; used through the wrapper `core/common/src/main/java/tachiyomi/core/common/util/system/LogcatExtensions.kt`: `inline fun Any.logcat(priority = DEBUG, throwable: Throwable? = null, message: () -> String = { "" })`, which appends `throwable.asLog()` (stack trace) on a new line after a non-blank message. Representative: `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt:45` `logcat(LogPriority.ERROR, e)`; `.../track/anime/interactor/SyncEpisodeProgressWithTrack.kt:48` `WARN`; `TrackEpisode.kt:~52` `INFO` for tracker errors; installer/downloader errors use `ERROR`.
* Replacement: `tracing` macros. Mapping: VERBOSE -> `trace!`, DEBUG -> `debug!`, INFO -> `info!`, WARN -> `warn!`, ERROR -> `error!`, ASSERT -> `error!` with `target: "assert"`. Provide one helper `fn log_err(level, err: &dyn std::error::Error, msg)` that logs `msg` then the `source()` chain (replacement for `asLog()`); `anyhow`'s `{:#}` alternate format gives the chain. The tag (Android derives it from the class name) = `module_path!()` target automatically.
* Difficulty trivial. Risk: porting 457 call sites mechanically; the rule "message is a lambda" maps to the lazy evaluation `tracing` already does.

### 7.4 LogcatLogger
* OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/App.kt:160-161`: `if (!LogcatLogger.isInstalled && networkPreferences.verboseLogging().get()) LogcatLogger.install(AndroidLogcatLogger(LogPriority.VERBOSE))`. Default for `verboseLogging` = `isDebugBuildType` (`di/PreferenceModule.kt:39`); when disabled no logger is installed (all `logcat` calls are no-ops). Same preference also enables Coil `DebugLogger` (`App.kt:205`) and torrent server logs (`TorrentServerService.kt:66`). Setting UI: `SettingsAdvancedScreen.kt:112`.
* Replacement: `tracing_subscriber::registry()` with `EnvFilter` held in a `tracing_subscriber::reload::Handle`. Default filter `kurobayomi=warn` (always log WARN+ to a rotating file `~/.local/state/kurobayomi/kurobayomi.log` via `tracing-appender`, 5 files daily), verbose preference true -> reload to `kurobayomi=trace,reqwest=debug`, also stderr. Honor `RUST_LOG` when set. Debug builds default verbose = `cfg!(debug_assertions)`.
* Difficulty trivial. Risk: Aniyomi itself only writes to logcat; the file sink is an addition (Proposal), needed because Linux users expect logs for bug reports.

### 7.5 LogCallback
* OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt:594`: `LogCallback { log -> if (log.level <= Level.AV_LOG_WARNING) log.message?.let { logcat(LogPriority.ERROR) { it } } }`: FFmpegKit log lines of level warning or worse (lower number = more severe) are logged as ERROR. A `StatisticsCallback` converts `s.time / 1000.0` (ms to s) to progress `100 * outTime / duration` (only if `duration != 0 && outTime > 0`), where `duration` comes from `ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "<url>"` (with the same `-headers` option), parsed with `toFloatOrNull()`.
* Replacement: FFmpegKit is Android-only. Spawn the system `ffmpeg`/`ffprobe` with `tokio::process::Command` (`area:24-downloads-anime`): arguments built as a `Vec<String>` (NO shell string parsing; Aniyomi builds one string and `FFmpegKitConfig.parseArguments` re-splits it, which is quote-fragile). Progress: add `-progress pipe:1 -nostats -loglevel warning`; read stdout lines `out_time_ms=<micro-seconds>` (note: despite the name the value is microseconds) and `progress=end`; stderr lines (warning and above because of `-loglevel warning`) -> `tracing::error!`. Duration via `ffprobe ... -of default=noprint_wrappers=1:nokey=1`. Cancel = `child.kill().await`. Missing binary -> user-visible error and optional fallback to libmpv `--stream-record`.
* Difficulty moderate. Risk: external dependency on `ffmpeg`; declare it as a package dependency.

### 7.6 LogRedirectionStrategy
* OBSERVED: `AnimeDownloader.kt:583` `FFmpegKitConfig.setLogRedirectionStrategy(LogRedirectionStrategy.ALWAYS_PRINT_LOGS)`: ffmpeg's own console output also goes to logcat.
* Replacement: forward the child's stderr into `tracing` at `debug` level (always), and keep warnings at `error` as in 7.5. Nothing else to port.
* Difficulty trivial.

---

## 8. Benchmarks, file pickers, graphics helpers, media

### 8.1 MacrobenchmarkRule
* OBSERVED: `macrobenchmark/src/main/java/tachiyomi/macrobenchmark/StartupBenchmark.kt:56` `val benchmarkRule = MacrobenchmarkRule()` in `AbstractStartupBenchmark(startupMode)` with `@Test` cases `startupNoCompilation`, `startupBaselineProfileDisabled` (`CompilationMode.Partial(baselineProfileMode = Disable, warmupIterations = 1)`), `startupBaselineProfile` (`Require`), `startupFullCompilation`; subclasses `ColdStartupBenchmark`, `WarmStartupBenchmark` (`StartupMode.WARM`), `HotStartupBenchmark` (`StartupMode.HOT`) run with `@RunWith(AndroidJUnit4ClassRunner::class)`.
* Replacement: dropped (ART baseline-profile tuning has no meaning for native code). Optional Proposal: one `cargo bench` / script measuring time-to-first-frame of the main window. User loses nothing.
* Difficulty trivial.

### 8.2 ManagedActivityResultLauncher
* OBSERVED: `presentation/more/settings/screen/SettingsDataScreen.kt:124` `storageLocationPicker(storageDirPref): ManagedActivityResultLauncher<Uri?, Uri?>` using `ActivityResultContracts.OpenDocumentTree()`. On result: `takePersistableUriPermission(uri, READ|WRITE)` in a try/catch that logs `SecurityException` and toasts `MR.strings.file_picker_uri_permission_unsupported` (InkBook/Samsung quirk), then `UniFile.fromUri(context, uri)?.let { storageDirPref.set(it.uri.toString()) }`.
* Replacement: `gtk::FileDialog::new().select_folder(Some(&window), gio::Cancellable::NONE, |res| ...)` (the portal-backed dialog under Flatpak/KDE with `GTK_USE_PORTAL=1`); store the absolute path string (`gio::File::path()`); if `path()` is `None` (non-local gvfs) reject with a toast. Default storage = `$XDG_DATA_HOME/kurobayomi` (`area:13-core-util-storage-archive-torrent`). No persistable-permission step exists, so the try/catch disappears.
* Difficulty trivial.

### 8.3 Matrix
* OBSERVED: `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:150-165`: `rotateImage(imageSource, degrees)` decodes with `BitmapFactory.decodeStream`, `Matrix().apply { postRotate(degrees) }`, `Bitmap.createBitmap(..., matrix, true)`, re-encodes as `JPEG` quality `100`. Callers (`pager/PagerPageHolder.kt:211-212`, `webtoon/WebtoonPageHolder.kt:235`) only ever pass `90f` or `-90f` (dual-page "rotate to fit", `dualPageRotateToFitInvert` flips the sign), and only when `ImageUtil.isWideImage(imageSource)`.
* Replacement: `image::DynamicImage::rotate90()` (clockwise 90) for +90 and `rotate270()` for -90; no need for arbitrary angles. Avoid the lossy JPEG re-encode: keep the rotated pixels as a `gdk::MemoryTexture` and rotate at render time with `gtk::Snapshot::rotate(90.0)` (lossless, faster). Preserve: only wide images (`width > height`) are rotated.
* Difficulty trivial.

### 8.4 MediaScannerConnection
* OBSERVED: `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:101` `MediaScannerConnection.scanFile(context, arrayOf(uri.path), null, null)` (called from `ImageSaver.kt:59,111` after saving a page/cover, and when creating `.nomedia` at `DiskUtil.kt:92`). `NOMEDIA_FILE = ".nomedia"` is created inside download directories so gallery apps skip chapters.
* Replacement: dropped; Linux has no global media scanner contract (Tracker/Baloo indexes by itself). `.nomedia` has no Linux meaning; Proposal: do not create it. User loses nothing visible.
* Difficulty trivial.

### 8.5 MediaSession
* OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:124,891-980`: `MediaSession(this, "PlayerActivity")` with a callback implementing `onPlay`, `onPause`, `onSkipToPrevious`, `onSkipToNext`, `onStop`; actions advertised: `ACTION_PLAY or ACTION_PAUSE or ACTION_STOP or ACTION_SKIP_TO_PREVIOUS or ACTION_SKIP_TO_NEXT`; `isActive = true`. Behavior is configurable through `gesturePreferences`: `mediaPlayPauseGesture` / `mediaPreviousGesture` / `mediaNextGesture`, each a `SingleActionGesture` in `{None, Seek, PlayPause, Custom, Switch}`: for Play/Pause: `PlayPause` = `viewModel.unpause()`/`pause()` and toggles `FLAG_KEEP_SCREEN_ON`; `Custom` = `MPVLib.command(["keypress", CustomKeyCodes.MediaPlay.keyCode])`; `None/Seek/Switch` do nothing. Previous/Next: `Seek` = `leftSeek()`/`rightSeek()`, `PlayPause` = `pauseUnpause()`, `Custom` = keypress `MediaPrevious`/`MediaNext`, `Switch` = `changeEpisode(true/false)` (previous = true). `onStop` sets `isActive = false` and calls the activity's `onStop()`. Also `noisyReceiver` (`ACTION_AUDIO_BECOMING_NOISY`) pauses and clears keep-screen-on (headphones unplugged).
* Replacement: an MPRIS2 service on the session bus `org.mpris.MediaPlayer2.kurobayomi` (crate `mpris-server`, or `zbus` by hand) exposing `Play`, `Pause`, `PlayPause`, `Stop`, `Next`, `Previous`, `CanPlay/CanPause/CanGoNext/CanGoPrevious = true`, `PlaybackStatus`, `Metadata` (`mpris:trackid`, `xesam:title` = episode name, `xesam:artist` = anime title, `mpris:artUrl` = cached cover file URI, `mpris:length`), `Position`/`Seek`. Route each MPRIS call through the same three `SingleActionGesture` preferences (keep the enum and default values; `Custom` -> `mpv` `keypress` command via libmpv `command`). Keep-screen-on = `org.freedesktop.ScreenSaver.Inhibit` (via `zbus`) or `gtk::Application::inhibit(window, ApplicationInhibitFlags::IDLE, reason)` while playing, `uninhibit` on pause. "Audio becoming noisy" has no portable Linux signal: Proposal: listen to PipeWire/PulseAudio default sink removal (`pipewire`/`libpulse-binding` `Subscription` on sink events) to auto-pause on headphone unplug; mark P2.
* Difficulty moderate. Risk: MPRIS media keys on KDE go to the most recent active player; set `CanRaise`/`DesktopEntry` so Plasma shows the right icon. Reader mode must not register MPRIS.

### 8.6 MediaStore
* OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt:20-200` (16 occ.). `save(image)`: `type = ImageUtil.findImageType(data)` or `IllegalArgumentException("Not an image")`; `filename = DiskUtil.buildValidFilename("${image.name}.${type.extension}")`; API < 29 or `Location.Cache`: write into `Location.directory(context)` (`Cache` = `cacheImageDir`; `Pictures` = `<Pictures>/<app_name>/<relativePath>`) with `mkdirs()`; API 29+ for `Location.Pictures`: `MediaStore.Images.Media` (mime supported by `MimeTypeMap.hasMimeType`) else `MediaStore.Files`, `RELATIVE_PATH = "<Pictures|Documents>/<app_name>/<relativePath>"`, `DISPLAY_NAME`, `MIME_TYPE`, `DATE_MODIFIED = now seconds`; it first looks up an existing row with the same `RELATIVE_PATH` (normalised with a trailing `/`) and `DISPLAY_NAME` (`findUriOrDefault`) and overwrites it (`openOutputStream(uri, "w")`), failing with `IOException(MR.strings.error_saving_picture)`. Covers are stored as JPEG quality 100.
* `buildValidFilename` (`DiskUtil.kt:~106-130`): `name = origName.trim('.', ' ')`; empty -> `"(invalid)"`; replace chars in `0x00..0x1f`, `"`, `*`, `/`, `:`, `<`, `>`, `?`, `\`, `|`, `0x7f` with `_`; truncate to `240` chars (255 minus 15 reserved).
* Replacement: `std::fs` into `glib::user_special_dir(glib::UserDirectory::Pictures)/<AppName>/<relative>` (fallback `~/Pictures`), same sanitising function and 240-char cap (on ext4 the limit is 255 bytes, so truncate by bytes on a char boundary, not chars). Overwrite an existing file of the same name (matches the "find then overwrite" behavior). Mime/extension from the `image` crate's format guess (`image::guess_format`): AVIF, GIF, HEIF, JPEG, JPEG_XL, PNG, WEBP (the `ImageType` enum at `ImageUtil.kt:106`). Show a notification with Share/Open actions (see Notification items). Share = `gtk::FileLauncher::open_containing_folder`/`gtk::UriLauncher`, or the `org.freedesktop.portal.OpenURI`.
* Difficulty trivial. Risk: Windows-style forbidden characters are harmless on ext4 but keep them replaced so the folder can sit on an NTFS/exFAT disk.

### 8.7 MimeTypeMap
* OBSERVED: `ImageSaver.kt:71` `MimeTypeMap.getSingleton().hasMimeType(type.mime)` decides Pictures vs Documents collection; AVIF/HEIF/JXL may be unknown on older Android.
* Replacement: not needed (no MediaStore collections). If a mime lookup is needed: `mime_guess::from_ext` or `gio::content_type_guess(Some(filename), &data)`.
* Difficulty trivial.

---

## 9. Networking helpers

### 9.1 MediaType
* OBSERVED: `core/common/src/main/java/eu/kanade/tachiyomi/network/ProgressResponseBody.kt:21` `override fun contentType(): MediaType? = responseBody.contentType()`: `ProgressResponseBody(responseBody, progressListener)` is a pass-through wrapper whose `ForwardingSource.read` accumulates `totalBytesRead += bytesRead (if != -1)` and calls `progressListener.update(totalBytesRead, responseBody.contentLength(), bytesRead == -1L)` on every read. Installed by `OkHttpClient.newCachelessCallWithProgress(request, listener)` (`OkHttpExtensions.kt:~117-131`): clone of the client with `cache(null)`, `callTimeout(30, HOURS)`, and a network interceptor wrapping the body. Used for the app updater APK download.
* Replacement: `reqwest::Response::content_length()` and `headers().get(CONTENT_TYPE)` (parse with the `mime` crate when needed); stream with `bytes_stream()` (`futures::StreamExt`), accumulate and call `on_progress(read, total: Option<u64>, done)` (note `contentLength()` returns `-1` when unknown: model as `Option<u64>`). Use a separate `reqwest::Client` without the HTTP cache layer and with `timeout(Duration::from_secs(30 * 3600))`. Since the Linux build does not self-update (package manager), the main user of this helper becomes extension/app downloads and torrent files; the downloader (`area:23-downloads-manga`) has its own progress model.
* Difficulty trivial.

### 9.2 MultipartBody
* OBSERVED: `core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerApi.kt:80-95` `uploadTorrent(file: InputStream, title, save = false)`: reads all bytes, `fileRequestBody = bytes.toRequestBody("application/x-bittorrent")`, `MultipartBody.Builder().setType(MultipartBody.FORM)` with parts `file` (filename = `title`), `save` (`"true"/"false"`), `title`; `POST "$hostUrl/torrent/upload"`, `awaitSuccess()`, decodes `Torrent` JSON from the response. The sibling `addTorrent` posts JSON to `"$hostUrl/torrents"` with `application/json`.
* Replacement: `reqwest::multipart::Form::new().part("file", Part::bytes(bytes).file_name(title).mime_str("application/x-bittorrent")?).text("save", save.to_string()).text("title", title)`; `client.post(format!("{host}/torrent/upload")).multipart(form).send().await?.error_for_status()?.json::<Torrent>().await?`. Enable reqwest feature `multipart`. The embedded TorrServer is an Android Go binary: Linux alternative = run the stock `torrserver` binary as a child process or use `librqbit` (see `area:13-core-util-storage-archive-torrent`).
* Difficulty trivial.

### 9.3 Network
* OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt:30,33` callback parameters `onCapabilitiesChanged(network: Network, caps)`, `onLost(network: Network)`; handles are ignored, the code re-reads `activeNetworkState()`.
* Replacement: `gio::NetworkMonitor::default()`; no handle type needed. Define in `kurobayomi-platform`:

```rust
pub struct NetState { pub is_connected: bool, pub is_validated: bool, pub is_wifi: bool }
impl NetState { pub fn is_online(&self) -> bool { self.is_connected && self.is_validated } }
```
  with `is_connected = monitor.is_network_available()` (connectivity != Local), `is_validated = monitor.connectivity() == NetworkConnectivity::Full` (Limited/Portal = not validated), `is_wifi` from NetworkManager D-Bus (`org.freedesktop.NetworkManager` property `PrimaryConnection` -> `Type == "802-11-wireless"`) via `zbus`; when NetworkManager is absent set `is_wifi = !is_network_metered` (documented guess).
* Difficulty moderate. Risk: `NetworkMonitor` needs the `glib-networking` GIO module; Arch ships it with GTK; fall back to "always online" if the monitor reports `Local` only because the module is missing (check `NetworkMonitor::get_network_available` and the extension point name).

### 9.4 NetworkCallback
* OBSERVED: `NetworkStateTracker.kt:27-45` `networkStateFlow() = callbackFlow { registerDefaultNetworkCallback(callback); awaitClose { unregister } }` emitting `activeNetworkState()` on every `onCapabilitiesChanged` and `onLost`. Consumers: `data/download/manga/MangaDownloadJob.kt:71` (and anime) combine it with the `downloadOnlyOverWifi` preference.
* Replacement: `monitor.connect_network_changed(move |_, _| tx.send(NetState::read()))` into a `tokio::sync::watch::channel<NetState>` (the `watch` receiver gives "latest value" semantics like `StateFlow`). Also react to `notify::connectivity` and `notify::network-metered`. Debounce 500 ms (NetworkManager fires several signals per switch).
* Downloader rule to keep (`MangaDownloadJob.kt:88-100`): if `state.is_online()`: `no_wifi = require_wifi && !state.is_wifi`; if `no_wifi` stop the downloader with message key `download_notifier_text_only_wifi`; return `!no_wifi`; else (offline) stop with `download_notifier_no_network`, return false. The job loop ends when `!isRunning || !networkCheck`.
* Difficulty trivial.

### 9.5 NetworkCapabilities
* OBSERVED (4 files / 16 occ.): (a) `util/system/NetworkExtensions.kt:16-26` `isOnline()`: active network exists, capabilities exist, and `hasTransport` for any transport in the integer range `TRANSPORT_CELLULAR (0) .. TRANSPORT_LOWPAN (6)` (API 27+; else `..TRANSPORT_WIFI_AWARE (5)`) i.e. cellular, wifi, bluetooth, ethernet, VPN, wifi-aware, lowpan; (b) `isConnectedToWifi()`: `wifiManager.isWifiEnabled` and (API 29+) active network has `TRANSPORT_WIFI` and `NET_CAPABILITY_INTERNET` (older: `wifiManager.connectionInfo.bssid != null`); (c) `NetworkStateTracker.kt:20-25` `NET_CAPABILITY_VALIDATED` (internet really reachable) and wifi flag as above; (d) `data/library/manga/MangaLibraryUpdateJob.kt:454-457` (+anime `:467-470`): `addTransportType(TRANSPORT_WIFI)` and `addCapability(NET_CAPABILITY_NOT_METERED)`.
* Users of `isOnline()`: `domain/track/manga/interactor/TrackChapter.kt` / `TrackEpisode.kt:38` (if online push the tracker update, else store in `delayedTrackingStore` and schedule the delayed job), `presentation/util/ExceptionFormatter.kt:22` (UnknownHostException: offline -> `MR.strings.exception_offline` else `MR.strings.exception_unknown_host` with the message), `ui/player/ExternalIntents.kt:545`.
* Replacement: `NetState` from 9.3. `is_online()` for these callers = `monitor.is_network_available()` (do not require Full; a captive portal still counts as "network present" for `isOnline`, matching the transport-range check). `metered = monitor.is_network_metered()`; NetworkManager `Metered` property values `1 yes`, `3 guess-yes` -> metered; `2 no`, `4 guess-no` -> unmetered; `0 unknown` -> treat as unmetered.
* Difficulty moderate. Risk: VPN-only or Tailscale setups report no wifi; "only on Wi-Fi" must also accept ethernet (Proposal: rename semantics to "unmetered or wifi/ethernet"; keep the stored preference value `wifi`).

### 9.6 NetworkRequest
* OBSERVED: `MangaLibraryUpdateJob.kt:452` / `AnimeLibraryUpdateJob.kt:465` build a `NetworkRequest` only to feed `Constraints.Builder().setRequiredNetworkRequest(request, networkType)` (WorkManager 2.9+, applies on Android 9+; older uses `networkType`).
* Replacement: represent the schedule constraints as a struct (see 9.7) instead of a request object.
* Difficulty trivial.

### 9.7 NetworkType
* OBSERVED (5 files / 12 occ.): `NetworkType.CONNECTED` in `domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt:60`, anime twin, `data/updater/AppUpdateDownloadJob.kt:135` (one-time job, `ExistingWorkPolicy.REPLACE`); `UNMETERED` vs `CONNECTED` in the library update jobs. Library auto-update scheduling (`MangaLibraryUpdateJob.setupTask`, `MangaLibraryUpdateJob.kt:440-480`): if `autoUpdateInterval > 0` (hours): restrictions set `autoUpdateDeviceRestrictions` with string values `DEVICE_ONLY_ON_WIFI = "wifi"`, `DEVICE_NETWORK_NOT_METERED = "network_not_metered"`, `DEVICE_CHARGING = "ac"` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:399-401`); network type `UNMETERED` if not-metered else `CONNECTED`; `setRequiresCharging("ac" in restrictions)`; `setRequiresBatteryNotLow(true)` always; periodic work every `interval` hours with flex `10` minutes, linear backoff `10` minutes, unique name `WORK_NAME_AUTO`, policy `ExistingPeriodicWorkPolicy.UPDATE`; `interval == 0` cancels it. At run time on API < 28 the job additionally returns `Result.retry()` if `"wifi"` is required and `!isConnectedToWifi()`, and returns retry if the manual job (`WORK_NAME_MANUAL`) is running. Delayed tracking: one-time, `BackoffPolicy.EXPONENTIAL` base 5 minutes (WorkManager caps backoff at 5 hours), `ExistingWorkPolicy.REPLACE`, retries (`Result.retry()`) while the store is non-empty.
* Replacement: `struct JobConstraints { network: Network::{Any, Connected, Unmetered}, wifi_only: bool, charging: bool, battery_not_low: bool }`, evaluated against `NetState` + UPower (`org.freedesktop.UPower`: `OnBattery` false = charging/AC; `BatteryNotLow` = `OnBattery == false || percentage > 15`; the 15 % is WorkManager's documented threshold, not in the Aniyomi source). Scheduler: in-process `tokio` task (`loop { sleep_until(next_due); if constraints ok { run } else { re-check on NetState/UPower change or after 10 min } }`) with jitter `rand(0..10 min)` replacing the flex window, `next_due = last_run + interval_hours`, last_run persisted. Optional background mode: install a `systemd --user` timer `OnUnitActiveSec=<interval>h`, `RandomizedDelaySec=10min`, `Persistent=true` that launches `kurobayomi --update-library` headless (decide in `area:26-library-update-jobs`). Delayed tracking retries: delay `5min * 2^(attempt-1)` capped at 5 h, plus an immediate retry on `NetState` change to online.
* Difficulty trivial. Risk: unmetered detection when NetworkManager is absent defaults to allowed; document it in settings help text.

---

## 10. Notifications

All notifier classes (`BackupNotifier`, `AnimeDownloadNotifier`, `MangaDownloadNotifier`, `AnimeLibraryUpdateNotifier`, `MangaLibraryUpdateNotifier`, `AppUpdateNotifier`, `SaveImageNotifier`, `HttpServerService`, `TorrentServerService`, `App.kt` incognito) build Android notifications. Unified Rust design (Proposal):

```rust
pub struct Notice { pub id: &'static str /*or String*/, pub channel: Channel, pub title: String, pub body: Option<String>,
  pub progress: Option<Progress>, pub ongoing: bool, pub auto_cancel: bool, pub timeout_ms: Option<u32>,
  pub group: Option<&'static str>, pub actions: Vec<NoticeAction>, pub default_action: Option<NoticeAction>, pub icon: Option<PathBuf> }
pub enum Progress { Indeterminate, Value { current: u32, max: u32 } }
```
Back end `gio::Application::send_notification(Some(id), &gio::Notification)` for event notifications (supports title, body, icon, priority, default action, up to buttons, `withdraw_notification(id)`), and an in-app progress strip (download queue, library update, backup/restore progress bars shown in the window footer) because `gio::Notification` has no progress/timeout. Optional second back end `notify-rust` (`Notification::hint(Hint::CustomInt("value", pct))`, `.timeout(Timeout::Milliseconds(ms))`, `.urgency(...)`): honored by Plasma and `dunst` for the `value` hint (verify on the target machine). The application needs an installed `.desktop` file whose name equals the `gio::Application` id, otherwise notifications are not shown or grouped.

### 10.1 Notification
* OBSERVED (3 files / 6 occ.): return type of `createNewChaptersNotification(manga, chapters): Notification` (`data/library/manga/MangaLibraryUpdateNotifier.kt:229`) and `createNewEpisodesNotification` (`AnimeLibraryUpdateNotifier.kt:247`), and parameter of `Context.notify(id, notification)` (`util/system/NotificationExtensions.kt:29`) which drops the notification when API >= 33 and `POST_NOTIFICATIONS` is not granted.
* Replacement: `gio::Notification`; `notify()` = no-op when no notification daemon (see `Manifest`).
* Difficulty moderate.

### 10.2 NotificationChannelCompat
* OBSERVED: `util/system/NotificationExtensions.kt:104-110` `buildNotificationChannel(channelId, importance, block)`; instances in `data/notification/Notifications.kt:130-205` (13 channels). Table (id -> importance, group, flags):
  `common_channel` LOW; `library_progress_channel` LOW, group `group_library`, no badge; `library_errors_channel` LOW, `group_library`, no badge; `new_chapters_episodes_channel` DEFAULT; `downloader_progress_channel` LOW, `group_downloader`, no badge; `downloader_error_channel` LOW, `group_downloader`, no badge; `backup_restore_progress_channel` LOW, `group_backup_restore`, no badge; `backup_restore_complete_channel_v2` HIGH, `group_backup_restore`, no badge, `setSound(null, null)` (silent); `incognito_mode_channel` LOW; `torrent_server_channel` LOW, no badge; `http_server_channel` LOW, no badge; `app_apk_update_channel` DEFAULT, `group_apk_updates`; `ext_apk_update_channel` DEFAULT, `group_apk_updates`.
  Deprecated channels deleted at start: `downloader_channel`, `downloader_complete_channel`, `backup_restore_complete_channel`, `library_channel`, `library_progress_channel`, `updates_ext_channel`, `downloader_cache_renewal`, `crash_logs_channel`, `library_skipped_channel` (note `library_progress_channel` is both deleted and re-created: the deletion list runs first, so it is recreated every launch).
* Replacement: `enum Channel { Common, LibraryProgress, LibraryError, NewItems, DownloaderProgress, DownloaderError, BackupProgress, BackupComplete, Incognito, TorrentServer, HttpServer, AppUpdate, ExtensionUpdate }` with a static `ChannelInfo { id, name_key, importance: Low|Default|High, group, silent }`. Mapping: LOW -> `gio::NotificationPriority::Low`, DEFAULT -> `Normal`, HIGH -> `High` (`Urgent` unused). Per-channel on/off toggles live in Kurobayomi settings (Android gives them in system settings; freedesktop has none per channel). Defaults: all on. Deprecated channel list is not needed.
* Difficulty moderate. Risk: `Silent` (no sound) is not expressible with `gio::Notification`; with `notify-rust` use hint `suppress-sound=true`.

### 10.3 NotificationChannelGroupCompat
* OBSERVED: `NotificationExtensions.kt:86-92` builder; groups created in `Notifications.createChannels`: `group_backup_restore` (name `MR.strings.label_backup`), `group_downloader` (`download_notifier_downloader_title`), `group_library` (`label_library`), `group_apk_updates` (`label_recent_updates`).
* Replacement: group headings in the notification settings page only.
* Difficulty trivial.

### 10.4 NotificationCompat
* OBSERVED (9 files / 41 occ.). Features used and their exact semantics:
  * `Builder(context, channelId).setColor(accent_blue)` for every notification (`notificationBuilder`).
  * Progress: `setProgress(0, 0, true)` (indeterminate; backup start `BackupNotifier.kt:52`), `setProgress(max, progress, false)` (restore `:107`; library update `setProgress(total, current, false)`; manga download `setProgress(pages.size, downloadedImages, false)`; anime download `setProgress(100, download.progress, indeterminate = progress==0)`), clear with `setProgress(0, 0, false)`.
  * `setOngoing(true)` for progress notifications (library update, download progress, backup, incognito, servers); `setOnlyAlertOnce(true)`; `setAutoCancel(false)` for progress, `setAutoCancel(true)` for results/new-chapter/error.
  * `setTimeoutAfter(ms)`: warnings `WARNING_NOTIF_TIMEOUT_MS = 30_000L` (`MangaDownloader.kt:767`); download error with optional `timeout`.
  * Styles: `BigTextStyle().bigText(...)` (updating text, errors, new-chapters description), `BigPictureStyle().bigPicture(bitmap)` (saved image, `SaveImageNotifier.kt:~66`).
  * Groups: `setGroup(GROUP_NEW_CHAPTERS = "eu.kanade.tachiyomi.NEW_CHAPTERS" / GROUP_NEW_EPISODES = "eu.kanade.tachiyomi.NEW_EPISODES")`, `setGroupAlertBehavior(GROUP_ALERT_SUMMARY)`, `setGroupSummary(true)` on the parent, `priority = PRIORITY_HIGH`.
  * Actions: `addAction(icon, label, pendingIntent)`; `clearActions()` before re-adding.
  * Content text rules: new-chapter parent (`MangaLibraryUpdateNotifier.kt:166-205`): title `MR.strings.notification_new_chapters`; if exactly one updated manga and `!hideNotificationContent`: text = `title.chop(45)`; else text = plural `notification_new_chapters_summary` (count) and, if `!hideNotificationContent`, BigText = titles joined by `\n`, each `chop(NOTIF_TITLE_MAX_LEN = 45)`; per-manga notifications posted only if `!hideNotificationContent`, id = `manga.id.hashCode()`, tag-less, large icon = cover circle-cropped at `NOTIF_ICON_SIZE = 192`. Per-manga text (`getNewChaptersDescription`, `:~290-345`): from chapters with `isRecognizedNumber`, sorted by `chapterNumber`, formatted with `formatChapterNumber`, de-duplicated (`toSet`): 0 numbers -> plural `notification_chapters_generic(count)`; 1 number -> if no others `notification_chapters_single(n)` ("Chapter 2.5") else `notification_chapters_single_and_more(n, remaining = chapters.size - 1)`; >1 numbers -> if more than `NOTIF_MAX_CHAPTERS = 5`: plural `notification_chapters_multiple_and_more(first5 joined by ", ", remaining = size - 5)` else `notification_chapters_multiple(all joined by ", ")`. Actions per manga: Mark as read (`MR.strings.action_mark_as_read`), View chapters (`action_view_chapters`), Download (`action_download`, only if `chapters.size <= CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 15`); default tap opens the first chapter. Parent tap opens the app on Updates (`Constants.SHORTCUT_UPDATES`). Large-update warning (`:130-141`): channel `library_progress_channel`, id `ID_LIBRARY_SIZE_WARNING = -103`, title `label_warning`, BigText `notification_size_warning`, timeout 30 s, tap opens `HELP_WARNING_URL = "https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads"` (shown only when the largest per-source update count exceeds `MANGA_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 60`, `MangaLibraryUpdateNotifier.kt:126,400`). Error log: title `notification_update_error` ("%1$d update(s) failed", argument `failed`), text `action_show_errors`, opens the error log file; nothing is shown if `failed == 0`. Backup: progress title `creating_backup`; complete `backup_created` with text = file display path, one action Share (`action_share`); errors `creating_backup_error`/`restoring_backup_error` with the message; restore progress title `restoring_backup` or `syncing_library` (if sync), text only if `!hideNotificationContent`, one action Cancel (`action_cancel`). Download progress title: when `hideNotificationContent` -> the progress text only; else `"<manga.title.chop(15)> - <chapter name with leading '<title>[ -]*' removed (case-insensitive)>"` chopped to 30. Incognito: ongoing, title `pref_incognito_mode`, text `notification_incognito_text`, tap broadcasts `tachi.action.DISABLE_INCOGNITO_MODE` which sets `incognitoMode = false`.
  * `chop(n)` = truncate to `n` chars and append an ellipsis when longer (helper in `tachiyomi.core.common.util.lang`); keep the same lengths (15, 30, 45).
* Replacement: build every notification through the `Notice` struct above. New-chapter grouping: send the summary with `Priority::High`, and per-title notices as `Priority::Normal` with a shared `group` (Plasma stacks notifications per app; no explicit group API in `gio`). Actions = `gio::Notification::add_button_with_target_value("Mark as read", "app.mark-read", &(entry_id, chapter_ids).to_variant())` with matching `gio::SimpleAction`s registered on the `gio::Application`; default action `set_default_action_and_target_value("app.open-chapter", ...)`. Large icon = `gio::Notification::set_icon` with a `gio::FileIcon` of the cached cover scaled to 192 px (circle crop done with `gdk-pixbuf`/cairo). `setColor` is dropped; `setOnlyAlertOnce` = update the same id (replace) instead of re-posting (gio replaces same id without re-alerting on most daemons). Progress notifications -> in-app progress strip + optional `value` hint back end. Keep every string key (`area:51-i18n`), thresholds (45/15/30, 5, 15, 60, 30 000 ms, 192 px), and the `hideNotificationContent` privacy setting (security preference).
* Difficulty hard (many call sites, behaviors scattered). Risks: freedesktop daemons differ (actions may be shown only in the expanded view; KDE supports them); notification ids must be strings; a `gio::Application` is mandatory, so headless background runs need `--gapplication-service` or the `notify-rust` back end.

### 10.5 NotificationManager
* OBSERVED: `util/system/NotificationExtensions.kt:17` `val Context.notificationManager: NotificationManager get() = getSystemService()!!`. Direct class use is limited to this accessor.
* Replacement: the `gio::Application` handle held in `AppContext`.
* Difficulty trivial.

### 10.6 NotificationManagerCompat
* OBSERVED (4 files / 14 occ.): `Notifications.createChannels` (`NotificationManagerCompat.from(context)`, `deleteNotificationChannel`, `createNotificationChannelGroupsCompat`, `createNotificationChannelsCompat`), `notify(id, notification)`, `notify(List<NotificationWithIdAndTag>)`, `cancel(id)` (`cancelNotification`), and importance constants `IMPORTANCE_LOW/DEFAULT/HIGH` (`Notifications.kt:6-8`). Notification ids (all `const val` in `Notifications.kt`): `ID_DOWNLOAD_IMAGE = 2`, `ID_LIBRARY_PROGRESS = -101`, `ID_LIBRARY_ERROR = -102`, `ID_LIBRARY_SIZE_WARNING = -103`, `ID_DOWNLOAD_CHAPTER_PROGRESS = -201`, `ID_DOWNLOAD_CHAPTER_ERROR = -202`, `ID_DOWNLOAD_EPISODE_PROGRESS = -203`, `ID_DOWNLOAD_EPISODE_ERROR = -204`, `ID_NEW_CHAPTERS = -301`, `ID_NEW_EPISODES = -1301`, `ID_UPDATES_TO_EXTS = -401`, `ID_EXTENSION_INSTALLER = -402`, `ID_BACKUP_PROGRESS = -501`, `ID_BACKUP_COMPLETE = -502`, `ID_RESTORE_PROGRESS = -503`, `ID_RESTORE_COMPLETE = -504`, `ID_INCOGNITO_MODE = -701`, `ID_TORRENT_SERVER = -801`, `ID_HTTP_SERVER = -901`, `ID_APP_UPDATER = 1`, `ID_APP_UPDATE_PROMPT = 2`, `ID_APP_UPDATE_ERROR = 3`. (`ID_DOWNLOAD_IMAGE` and `ID_APP_UPDATE_PROMPT` are both `2` on different channels.)
* Replacement: string ids `"library-progress"`, `"library-error"`, `"library-size-warning"`, `"download-chapter-progress"`, `"download-chapter-error"`, `"download-episode-progress"`, `"download-episode-error"`, `"new-chapters"`, `"new-episodes"`, `"updates-to-exts"`, `"extension-installer"`, `"backup-progress"`, `"backup-complete"`, `"restore-progress"`, `"restore-complete"`, `"incognito"`, `"torrent-server"`, `"http-server"`, `"app-updater"`, `"app-update-prompt"`, `"app-update-error"`, `"save-image"`; per-entry ids `"entry-<db id>"` (replaces `manga.id.hashCode()`). `cancel(id)` = `withdraw_notification(id)`.
* Difficulty moderate.

### 10.7 NotificationWithIdAndTag
* OBSERVED: `NotificationExtensions.kt:42` `fun Context.notify(list: List<NotificationWithIdAndTag>)` used to post all per-manga notifications in one go (`MangaLibraryUpdateNotifier.kt:219` `NotificationWithIdAndTag(manga.id.hashCode(), createNewChaptersNotification(...))`, anime `AnimeLibraryUpdateNotifier.kt:237`); same permission early return.
* Replacement: `fn notify_all(items: Vec<(String, gio::Notification)>)` looping `send_notification`, in a `glib::idle_add_local_once` per item batch of 10 to avoid blocking the UI with hundreds of covers; cap Proposal: if more than 20 updated entries, only the summary is posted.
* Difficulty trivial.

---

## 11. Rx

### 11.1 Observable
* OBSERVED (11 files / 46 occ.): RxJava 1 (`rx.Observable`) survives only as deprecated extension-compat surface in the source API: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt:68,75,82` (`fetchMangaDetails`, `fetchChapterList`, `fetchPageList`), `source/CatalogueSource.kt:64,71,78` (`fetchPopularManga`, `fetchSearchManga`, `fetchLatestUpdates`), the anime equivalents in `animesource/AnimeSource.kt:160,178,184` and `animesource/AnimeCatalogueSource.kt:73,79,85`, and the default implementations in `source/online/HttpSource.kt:112-114` (`client.newCall(popularMangaRequest(page)).asObservableSuccess().map { popularMangaParse(it) }`) and `animesource/online/AnimeHttpSource.kt` (every suspend `getX` default calls `fetchX(...).awaitSingle()`, lines ~251, ~298). Bridges: `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:25-71` (`Call.asObservable()`: clone the call per subscriber, execute on `request(n)`, `unsubscribe()` cancels; `asObservableSuccess()`: if `!response.isSuccessful` close the response and throw `HttpException(code)`), `core/common/src/main/java/tachiyomi/core/common/util/lang/RxCoroutineBridge.kt` (`awaitSingle()` = `single().awaitOne()`: resumes with the first element; completion without element -> `IllegalStateException("Should have invoked onNext")`; cancellation unsubscribes) and `app/src/main/java/eu/kanade/core/util/RxJavaExtensions.kt` (`Observable<T>.asFlow()` via `callbackFlow`).
* Replacement: Rx is dropped entirely. The Rust source trait only has `async fn` methods (`popular(page) -> Result<EntriesPage>`, `search`, `latest`, `details`, `chapters`, `pages`, ...) returning `Result<_, SourceError>`; `HttpException(code)` becomes `SourceError::Http(StatusCode)` produced by `response.error_for_status()` semantics (`!is_success()` -> error; 3xx are followed by reqwest). One-shot "subscribe" = `.await`; cancellation = dropping the future or `tokio::select!` with a `CancellationToken`. For streams (`asFlow`) use `tokio_stream`/`futures::Stream` or `tokio::sync::watch/broadcast`. Compatibility with legacy Kotlin extensions that still override the `fetch*` methods is a concern of the extension strategy (`area:20-extension-strategy-redesign`): native Rust sources never need it; if a bridge to Kotlin/JVM extensions is ever built, the `fetchX(...)` -> `getX(...)` adaptation above is the whole contract.
* Preserve: error taxonomy (HTTP status errors, network errors, `NoChaptersException` for empty lists, parse errors), `awaitSingle` semantics (exactly one result, else error).
* Difficulty moderate (design decision, little code). Risk: none for native sources.

---

## 12. Cross-cutting decisions recorded by this chunk

1. App foreground/background = any-window `is-active` edges (2.2); drives app lock, tracker flush and "resume" refreshes.
2. One `NetState` watch channel (`gio::NetworkMonitor` + NetworkManager + UPower) feeds downloader, library updater, tracker delayed queue, error messages (9.3-9.7).
3. Notifications: `gio::Notification` events + in-app progress strip; every Aniyomi channel, id, threshold and string key is kept (section 10).
4. MPRIS2 replaces `MediaSession`; keep the `SingleActionGesture` preference set (`None`, `Seek`, `PlayPause`, `Custom`, `Switch`) (8.5).
5. External tools: `ffmpeg`/`ffprobe` binaries replace FFmpegKit with `-progress pipe:1` parsing (7.5).
6. Dropped with reason: Glance widget (2.1), splash/shared-element animations (5.1, 5.2), Monet/Material dynamic color generation (5.3, theme follows GTK), `MediaScannerConnection`/`.nomedia` (8.4), Macrobenchmark (8.1), Chromium `X-Requested-With` spoof (2.4), `MinMaxNumberPicker` (dead code, 3.5), haptic feedback (4.1).
