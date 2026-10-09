# Area 01 — UI parity

Scope files that render UI: `app/src/main/java/eu/kanade/presentation/crash/CrashScreen.kt` (Compose screen), `app/src/main/java/eu/kanade/tachiyomi/crash/CrashActivity.kt` (its host), and the one `App.kt` surface that is visible to the user (incognito notification). `CheckboxState.kt` (tri-state checkbox mapping) and the `ifXSourcesLoaded` gates affect widgets but draw nothing themselves; they are listed in section 4.

## 1. What Aniyomi shows (OBSERVED)

### 1.1 Crash screen = `InfoScreen` (`presentation-core/src/main/java/tachiyomi/presentation/core/screens/InfoScreen.kt`) with crash content
Structure (Material 3 `Scaffold`):
* Bottom bar (`bottomBar`): a column with the app background colour, a 1-pixel-hairline line drawn at its top edge in `colorScheme.outline` (`Dp.Hairline`), window insets padding for the navigation bar, padding 16 dp horizontal (`MaterialTheme.padding.medium`) and 8 dp vertical (`padding.small`). Inside: a full-width filled `Button` (text = accept text; enabled = `canAccept`, default true) and, when a reject text and handler exist, a full-width `OutlinedButton` below it. There is no spacing modifier between the two buttons.
* Status-bar scrim: a `Box` with `zIndex(2f)`, `secondaryItemAlpha()` (alpha 0.78, constant `SECONDARY_ALPHA = .78f` in `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:10`), background = `colorScheme.background`, full width, height = the Scaffold top padding (status bar height).
* Body: `Column` with `verticalScroll(rememberScrollState())`, full width, scaffold padding, 48 dp top padding, 16 dp horizontal padding. Children in order: (1) `Icon` 48 dp, tint `colorScheme.primary`, bottom padding 8 dp; (2) heading `Text` with `MaterialTheme.typography.headlineLarge`; (3) subtitle `Text` with `titleSmall`, `secondaryItemAlpha()` (alpha 0.78), vertical padding 8 dp; (4) the screen's `content()`.
Paddings from `tachiyomi.presentation.core.components.material.Padding`: extraLarge 32, large 24, medium 16, mediumSmall 12, small 8, extraSmall 4 (dp).

`CrashScreen` fills it with: icon `Icons.Outlined.BugReport`; heading "Whoops!"; subtitle "Aniyomi ran into an unexpected error. We suggest you share the crash logs in our support channel on Discord."; accept button "Share crash logs"; reject button "Restart the application"; content = `Box` with vertical padding 8 dp, clipped to `MaterialTheme.shapes.small` (Material 3 default small shape = 8 dp rounded corners), `fillMaxSize()`, background `surfaceVariant`, containing `Text(exception.toString())` with colour `onSurfaceVariant`, padding 8 dp, default body text style (`LocalTextStyle`, Material 3 bodyLarge on `Text` default, 16 sp). Text is not selectable and not monospace; long stack traces wrap at the box width; the whole column scrolls vertically.

States: there is only one visible state. While "Share crash logs" runs nothing changes visually (no progress indicator; the button stays enabled); on success the system share sheet opens; on failure a toast "Failed to get logs" appears (hard-coded string, `Toast` short duration, about 2 s). "Restart the application" closes all activities and opens `MainActivity` (cold start screen).

Edge-to-edge: `WindowCompat.setDecorFitsSystemWindows(window, false)`; theme from `BaseActivity` (same colour scheme as the main app).

Gestures / navigation: system back closes the crash task (no custom handler). No animations.

### 1.2 Incognito notification (App.kt)
Android notification, channel "Incognito mode" (low importance), small icon glasses, title "Incognito mode", text "Disable incognito mode", ongoing (cannot be swiped away), tap = disable incognito and remove the notification. Posted whenever the preference is true at startup or becomes true; removed when it becomes false.

## 2. Kurobayomi crash window (Proposal)

Window: standard `gtk::ApplicationWindow` created by `crash_window::run`; title "Kurobayomi" (no header-bar custom content; use the window manager's title bar, like the main window which sets an empty box as its titlebar in the mockup only because every page draws its own header; the crash window has no pages, so it keeps a plain header with the app name); default size 640 x 720; minimum 420 x 360; resizable; centered on screen; closes with Escape and the title-bar close button (both quit the reporter).

Layout (top to bottom), matching section 1.1:

| Element | Widget and style | Notes |
|---|---|---|
| Scroll area | `gtk::ScrolledWindow`, vertical policy automatic, horizontal never, `vexpand` | Contains the body column |
| Body column | `gtk::Box` vertical, margins: top 48, start/end 16, bottom 16 | `column()` helper in `src/ui.rs` uses margin 18; the crash window uses its own margins to match 48/16 |
| Icon | `gtk::Image` icon name `bug-symbolic` with fallback `dialog-error-symbolic`, `pixel_size(48)`, `halign start`, CSS class `accent` (primary tint) | bottom margin 8. The mockup uses `dialog-error-symbolic`; the real screen should use the bug glyph when the theme has it |
| Heading | `gtk::Label` "Whoops!", class `title-1` (32 sp equivalent of headlineLarge), `xalign 0` | If the user's GTK theme lacks `title-1`, fall back to Pango scale 2.0 (`<span size="xx-large">`) through `attributes` |
| Subtitle | `gtk::Label`, wrap, `xalign 0`, class `kuro-secondary` (CSS `opacity: 0.78`), `title-4` size, vertical margins 8 | text from `crash_screen_description` formatted with the app name `Kurobayomi` |
| Crash box | `gtk::Box` (class `kuro-crash-box`: `border-radius: 8px; background: alpha(currentColor, 0.08);`) with 8 px vertical margin, containing a `gtk::Label` with `selectable(true)`, `wrap(true)`, `wrap_mode(WrapMode::Char)` so long frames wrap, `xalign 0`, class `monospace`, margins 8 | Text = crash text (header lines + backtrace). The box expands (`vexpand`) so it fills the remaining height; the whole body scrolls as in Aniyomi |
| Status line | `gtk::Label` between scroll area and bottom bar, class `kuro-secondary`, hidden by default | Shows `crash_logs_saved` for 4 s or `crash_logs_failed` for 2 s |
| Bottom bar | `gtk::Box` vertical, spacing 8, margins 16 horizontal and 8 vertical, with a `gtk::Separator` (horizontal, hairline) above it | Contains the two buttons |
| Primary button | `gtk::Button` label "Share crash logs", CSS class `suggested-action`, full width, is the default widget | disabled while the dump runs |
| Secondary button | `gtk::Button` label "Restart the application", full width | |

Dark/light and accent colours come from the user's GTK theme (no libadwaita; the mockup already relies on theme classes `title-1`, `heading`, `dim-label`, `caption`, `boxed-list`, `suggested-action`). The two custom classes (`kuro-secondary`, `kuro-crash-box`) are defined in the app CSS provider created in `kurobayomi::theme_mode` (area:49 owns the global stylesheet; the reporter installs the same two rules itself because it does not load the global provider).

States:
| State | Visual |
|---|---|
| Showing report | as above |
| Report unreadable | crash box text `No crash details available` (string key `crash_report_unavailable`) |
| Sharing | primary button insensitive, its label unchanged, a 16 px `gtk::Spinner` shown to the left of the status line |
| Shared | status line `Crash logs copied to the clipboard`, hides after 4 s; button re-enabled |
| Share failed | status line `Failed to get logs`, hides after 2 s; button re-enabled |
| Restart failed | status line `Could not restart the application` for 4 s |

Keyboard and mouse:
* Enter: activates the default widget (Share crash logs). Tab order: crash text (selectable), Share, Restart.
* Escape or Alt+F4: quit. Ctrl+C: copy selected text; Ctrl+A in the crash text selects all (default `GtkLabel` behaviour); a right-click menu offers Copy and Select All (default `GtkLabel` context menu).
* Mouse wheel scrolls the body; the scrollbar is the theme's overlay scrollbar.
* No animations (Aniyomi has none).

Responsive behaviour: single column at any width; below 420 px the window cannot shrink further (minimum size), long lines wrap; at wide sizes the content column is capped at 900 px and centered (`adw::Clamp` is libadwaita-only, so the existing `src/clamp.rs` helper from the mockup is used).

Accessibility: icon has accessible label `Bug`; heading marked as heading via `Accessible` role; buttons are labelled by text.

## 3. Incognito indicator (Proposal; owner of the window chrome is area:36)

| Element | Behaviour |
|---|---|
| Window banner | A `gtk::Revealer` (slide-down 200 ms) above the page stack in the main window containing a strip: glasses symbolic icon (`night-light-symbolic` fallback if no glasses glyph), text "Incognito mode", and a flat button "Disable incognito mode". Visible exactly while `incognito_mode` is true. Colour: CSS class `incognito-banner` (token from area:49; Aniyomi uses `IncognitoModeBannerBackgroundColor` for the status bar). |
| Precedence | When several banners apply, only one shows, with the same precedence as `MainActivity`: indexing banner, then downloaded-only banner, then incognito banner. |
| Desktop notification | One notification, id `incognito_mode_channel:-701`, summary "Incognito mode", body "Disable incognito mode", default action `app.incognito-off`; withdrawn when incognito turns off. Never shown if the user's `Notifier` implementation reports notifications unavailable. |
| More tab row | The "Incognito mode" switch of the More screen (already in `spec/app.json` and drawn by the mockup's generic spec renderer in `src/ui.rs`) is bound to the same preference (`PrefBinding<bool>`), so toggling it updates banner and notification. |

## 4. Widgets introduced by files of this scope

| Widget | Source in Aniyomi | Kurobayomi |
|---|---|---|
| Tri-state check button | `CheckboxState.kt` mapping used by filter sheets (library, updates, source filters) | `widgets::tri_check` (`p1-rust-design.md` 3.3). Cycle none → include → exclude → none. The mockup already implements this inline in `src/ui.rs` (`Item::Tristate`: `set_inconsistent(true)` plus `set_active(true)` for exclude, guard against re-entrant `connect_toggled`); the task extracts that code into the shared widget so filter sheets of areas 37, 40, 41 use one implementation |
| Loading gate for source screens | `ifMangaSourcesLoaded`/`ifAnimeSourcesLoaded` then `LoadingScreen()` (centered circular progress) | `when_sources_ready`: a `gtk::Stack` with a centered `gtk::Spinner` (size 32) page and the real content page; switch with crossfade 150 ms when the gate flips to true |
| Separators in lists | `insertSeparators` (date headers) | list model items of type `Header(String)` rendered as `heading` label rows (areas 37, 42) |
| Preference-bound controls | `PreferenceMutableState` | `PrefBinding` + `bind()`; a `gtk::Switch` bound to a boolean preference writes on `state-set` and updates on notification |

## 5. What the existing mockup lacks (precise)

1. `src/screens.rs::info_screen("crash")` is an in-app page, not a window: heading comes from `spec/app.json` (`"Whoops!"`), the subtitle is hard-coded and still says Discord, the exception text is a fixed string `java.lang.RuntimeException: something went wrong`, the box is a bordered `gtk::Frame` (no filled rounded surface, not selectable, not monospace), the buttons sit in the scrolling column instead of a bottom bar with a separator, "Share crash logs" has no handler, "Restart the application" just pops the page.
2. There is no separate reporter process, no panic hook, no report file, no log tail dump.
3. No incognito banner, no desktop notification, and the More-tab "Incognito mode" row is not bound to any preference (it renders from spec defaults only).
4. No process-lifecycle events, no app lock, no theme-mode application (the mockup follows the GTK default only).
5. Sidebar badge counts are literals (`"updates" => "8"`, `"browse" => "1"` in `sidebar()` of `src/main.rs`).
6. No `AppContext`: screens build from `spec/*.json` through `thread_local! SPEC` and hold no services.
7. No source-ready gate or loading placeholder for browse/search screens (they render immediately from spec).
8. No single-instance handling beyond `gtk::Application` defaults and the three dev environment variables; no `--open <deep link>` argument.
9. No `--version` output and no About-screen build info source.
