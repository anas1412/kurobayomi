# 50-android-widgets — Part 1 UI parity

This file covers the visible UI of the Android Glance widgets (the four providers, their body composables, the lock placeholder and the cover tile) and the desktop equivalent. The Android widgets are launcher and lock-screen surfaces; the desktop equivalent is a Plasma applet plus a `.desktop` action. The in-app Updates tab is area:42-ui-updates-history-stats-storage-upcoming and is not repeated here, except where the mockup must change for this area.

Android facts are from `presentation-widget/src/main/java/tachiyomi/presentation/widget/` (abbreviated `W/`). Layout values are in dp. Desktop values are logical px at scale 1.

## 1. Visible elements (Android, OBSERVED)

### 1.1 Home-screen card (manga and anime, `W/entries/*/Base*GlanceWidget.kt:68-110`)

- Container: fills the widget (`fillMaxSize`), `background(background)` (drawable `appwidget_background`: rectangle, fill `appwidget_secondary_container`), corner radius 16 dp (system radius on Android 12+), top and bottom padding 0.
- Text colour of spinner and empty message: `appwidget_on_secondary_container` (manga and anime home).
- No title bar, no header, no footer, no count, no timestamp, no button. The whole widget is the cover grid or one message.

### 1.2 States of the home card

| State | Trigger | What is drawn | Exact text / values |
|---|---|---|---|
| Initial (before Glance renders) | Android renders `layout/appwidget_loading.xml` | Frame with `appwidget_background`, centred TextView, colour `?android:attr/textColorPrimary` | "Loading…" (`@string/loading`, `i18n/src/commonMain/moko-resources/base/strings.xml:170`) |
| Loading | `provideContent` runs, data `null` | `CircularProgressIndicator(color = contentColor)` centred in the box | no text |
| Empty | list is empty | `Text(stringResource(MR.strings.information_no_recent), style = TextStyle(color = contentColor))` centred | "No recent updates" |
| Locked | `use_biometric_lock` is true | `LockedXWidget`: box with padding 8 dp, centred text, 12 sp, centre-aligned, colour `foreground` | "Widget not available when app lock is enabled" |
| Content | list non-empty | grid of covers (section 1.3) | none |
| Cover missing | bitmap `null` | placeholder cover (section 1.4) | none |

No error state is shown. Coil and DB errors leave the spinner or the last drawn data (behaviour file section 15).

### 1.3 Cover grid (OBSERVED, `W/components/*/Updates*Widget.kt:49-96`)

- Layout: a `Column` (`fillMaxHeight`, vertically and horizontally centred) containing `rowCount` rows.
- Row: `Row(padding(vertical = 4.dp).fillMaxWidth(), horizontalAlignment = Center, verticalAlignment = Center)`; rows with no items are not drawn.
- Cell: `Box(padding(horizontal = 3.dp), contentAlignment = Center)`.
- Cover: 58 x 87 dp, radius 12 dp, `ContentScale.Crop`.
- Pitch: 64 dp per column (58 + 6), 95 dp per row (87 + 8).
- Ordering: row-major, feed order (newest unread first).
- Partial last row: centred, not left-aligned.
- Column count and row count come from the instance's own size with the same formula (behaviour file section 5), capped at 10 each.
- Click target: the cover itself (`clickable(actionStartActivity(intent))`).

### 1.4 Placeholder cover (OBSERVED, `drawable/appwidget_cover_error.xml`)

- Full-cell rounded rectangle, radius 12 dp, fill `#1F888888` (`cover_placeholder`).
- Over it, a 24x24 vector icon (a picture-frame glyph) inset 24 dp on every side. The inset bounds are 58 - 48 = 10 dp wide and 87 - 48 = 39 dp high. VectorDrawable keeps its aspect ratio, so the glyph renders about 10 x 10 dp, centred. (Derived from the layer-list inset and VectorDrawable's default aspect handling; verify on device.)
- `contentDescription = null`.

### 1.5 Cover screen and keyguard variant (OBSERVED)

- `background` = `appwidget_coverscreen_background` (`#00000000`, fully transparent).
- Foreground = `Color.White` (not themed).
- Bottom padding 24 dp (reserves space for the device's bottom area).
- Otherwise identical to the home card. Samsung sub-screen metadata is in the manifest (`privacyWidget = true`).

### 1.6 Click behaviour (OBSERVED)

- Cover click: `MainActivity` with action `SHOW_MANGA`/`SHOW_ANIME` and the entry id, `FLAG_ACTIVITY_NEW_TASK | FLAG_ACTIVITY_CLEAR_TOP`. Opens the entry in the library (behaviour file section 11).
- Locked placeholder click: opens `MainActivity` with no action (default screen).
- No long-press, no swipe, no menu, no keyboard handling, no drag. Glance does not support focus rings or hover.

### 1.7 Animations

None. Glance has no transitions. The spinner is the platform indeterminate spinner.

### 1.8 Responsive behaviour (OBSERVED)

- Size mode `Exact` (`W/entries/*/Base*GlanceWidget.kt:61`): Glance recomposes for each reported size.
- Grid recomputes with size; excess covers are not drawn (`take(rows * cols)`).
- Minimum size: 1 row, 1 column (`coerceIn(1, 10)`). Information XML minimums (`minWidth` 240 dp, `minHeight` 80 dp, `minResizeWidth` 80 dp, `minResizeHeight` 110 dp) are enforced by the launcher.
- Maximum: 10 x 10 cells. `maxResizeWidth` and `maxResizeHeight` are 600 dp.
- Target size on placement: 4 x 2 cells (`targetCellWidth` 4, `targetCellHeight` 2).

### 1.9 Strings shown (OBSERVED)

| Key | English | Where |
|---|---|---|
| `information_no_recent` | No recent updates | empty state (both kinds) |
| `appwidget_unavailable_locked` | Widget not available when app lock is enabled | locked state |
| `loading` | Loading… | initial RemoteViews |
| `label_recent_updates` | Updates | picker label, manga provider |
| `label_recent_anime_updates` | Anime Updates | picker label, anime provider |
| `appwidget_updates_description` | See your recently updated library entries | picker description |

## 2. Desktop equivalent (PROPOSAL)

The applet (`org.kurobayomi.updates.manga` and `org.kurobayomi.updates.anime`) reproduces sections 1.1 to 1.6 with Plasma components. Differences are marked.

### 2.1 Card

- Background: Plasma's default applet background (`Plasmoid.backgroundHints: PlasmaCore.Types.DefaultBackground`). The Android secondary-container fill is dropped (DIFFERENCE: follows the user's Plasma theme; no fixed card colour).
- Radius: the applet does not draw a card. Covers keep their 12 px radius (baked into the PNG).
- Padding: none (0), so the grid uses the full applet.

### 2.2 States

| State | Desktop element | Text (new keys marked NEW) |
|---|---|---|
| Loading (first run, before first result) | `PlasmaComponents.BusyIndicator` centred, `running: true` | none |
| Empty | `PlasmaExtras.PlaceholderMessage { iconName: "view-refresh"; text: i18nd("kurobayomi", "No recent updates") }` | "No recent updates" (same key) |
| Locked | `PlasmaExtras.PlaceholderMessage { iconName: "lock"; text: i18nd("kurobayomi", "Widget not available when app lock is enabled") }`. Whole area clickable (runs `kurobayomi`, no args, which raises the window) | same key |
| Error (CLI exit != 0) | `PlasmaExtras.PlaceholderMessage { iconName: "dialog-error"; text: i18nd("kurobayomi", "Updates unavailable") }` with a tooltip holding the stderr line. Click retries now | NEW `widget_error_title` = "Updates unavailable" |
| Content | cover grid (2.3) | none |
| Cover missing | placeholder tile (2.4) | none |

The error state is new: Android shows nothing for DB or Coil errors. The desktop shows a visible error so a broken CLI is not mistaken for an empty library (this is a deliberate difference).

### 2.3 Cover grid

- `Grid`-like layout with `rows` and `cols` from `Math.floor(height / 95)` and `Math.floor(width / 64)`, each clamped to 1..10 (same formula as Android), centred horizontally and vertically.
- Cell: 58 x 87 logical px (`Layout.preferredWidth` and `preferredHeight`), 4 px vertical gap inside each row and 3 px horizontal gap per cell, so each cell is 64 x 95 with its margins.
- Image: `Image { fillMode: Image.PreserveAspectCrop; asynchronous: true; sourceSize: Qt.size(58 * dpr, 87 * dpr) }` with `source` = `"file://" + cover_path`. `dpr` is the applet's device pixel ratio.
- Short last row: centred (same as Android).
- Pointer: `cursorShape: Qt.PointingHandCursor` over a cover (DIFFERENCE: Android has no cursor).
- Keyboard: each cover is focusable with Tab (`activeFocusOnTab: true`); Enter and Space activate, same as a click (DIFFERENCE: Android has none).
- Tooltip (DIFFERENCE): hovering a cover after 500 ms shows the entry title (`PlasmaCore.ToolTipArea`, `mainText: title`). Android has no title anywhere on the widget.
- Accessible name (DIFFERENCE): `Accessible.name = title` on each cover (Android `contentDescription = null`).

### 2.4 Placeholder tile

- Rounded rectangle, radius 12 px, fill `#1F888888` (kept from `cover_placeholder`, so the look matches Android).
- Icon: `Kirigami.Icon { source: "image-x-generic"; width: 10; height: 10; anchors.centerIn: parent }` (ten logical px, centred; matches the derived Android size in 1.4). Colour: Plasma text colour at 0.6 opacity.

### 2.5 Click and keyboard

- Cover activate: `executable.connectSource("kurobayomi --open " + kind + ":" + entryId)`.
- Locked or error activate: `kurobayomi` (no args) raises the window. Android opens the app for the locked placeholder; same effect.
- Refresh: clicking the applet header area (a 24 px header only shown in error or empty state) runs an immediate refresh. Normal state has no header (Android has none).

### 2.6 Animations

None (matches Android). Covers do not fade in. Only the `BusyIndicator` animates.

### 2.7 Resize behaviour

- The applet recomputes `rows` and `cols` on every size change. Changes are debounced: the CLI runs once 300 ms after the last change (proposal). Between runs the old grid is shown, clipped to the new size.
- Minimum size: one cell (64 x 95 px). Default size: 4 x 2 cells (256 x 190 px).
- Maximum: 10 x 10 cells (640 x 950 px).
- The Android `minResizeWidth` 80 dp / `minResizeHeight` 110 dp are not reproduced; the cell grid implies the minimum.

### 2.8 Locked state specifics

- The applet draws the locked placeholder from the JSON `locked: true` field. It never draws a cover, title, or number when `locked` is true, and it does not cache the previous feed across a lock change: when `locked` flips to true, `items` is cleared before the next paint.
- The applet shows the Android text (same key), so the user sees the same message on both platforms.

### 2.9 Applet configuration UI

None. Android has no configuration activity (no `android:configure` in the provider XML), so the applet has none. Kind is fixed per applet id.

## 3. Mockup gaps (OBSERVED in `/home/blackbox/kurobayomi`)

| Gap | Evidence | What is needed |
|---|---|---|
| No cover grid anywhere | `src/screens.rs:277-300` `fn updates()` builds day-grouped rows (`heading(day)`, `boxed_list()`, `list_row(...)`) with `thumb(40, 40)` placeholders | The Updates screen (area:42-ui-updates-history-stats-storage-upcoming) needs a cover grid option or stays a list; the applet needs its own grid (new code). |
| Hard-coded data | `src/screens.rs:281` `"Library last updated: 5 minutes ago"`, `title_n(i)` titles, `format!("Episode {}", 13 - i)` | Replace with `feed::query::unread_updates` output (area:42-ui-updates-history-stats-storage-upcoming wiring). |
| No loading, empty, error or locked state for the updates feed | `updates()` always renders rows | Add the four states (section 1.2) to the Updates screen and to the applet. |
| Sidebar badge is a literal | `src/ui.rs:175` `"updates" => "8"` | Compute the unread count (`feed::query` count, section 15 of the Rust design). |
| No `--open` routing | `src/main.rs:12-20` has no argument handling | Early CLI branch and GApplication open targets (area:36-ui-shell-navigation-security-webview). |
| No CLI, no applet, no `.desktop` action | no files under `dist/` | Create per the Rust design. |
| No lock placeholder | no lock check anywhere in the updates UI | Add the locked state; it depends on the lock flag (area:02-preferences-system/36). |
| No strings catalogue | literal English in `screens.rs` | New keys `widget_error_title` and the Android keys above, through the i18n area (area:51-i18n). |

## 4. Responsive and input summary

| Input | Android widget | Desktop applet |
|---|---|---|
| Mouse click on cover | opens entry | opens entry |
| Mouse hover | none | tooltip with title, pointer cursor |
| Keyboard | none | Tab focus, Enter/Space activate |
| Touch | tap | tap (Plasma on touch devices) |
| Long press, swipe, drag | none | none (not added: no Android equivalent) |
| Right-click menu | none | none (not added) |
| Resize | Glance exact size, grid recomputed | live resize, 300 ms debounce |
| Dark / light | Material You (Android 12+) or Tachiyomi tokens | Plasma colour scheme |
| RTL | not handled explicitly | Plasma RTL handled by Qt layout mirroring for the grid (cell order mirrors) |

## 5. Dialogs, snackbars, toasts, menus

Android widgets have none. Desktop applet: none (errors use the placeholder, section 2.2). Desktop app: no dialogs are added by this area.
