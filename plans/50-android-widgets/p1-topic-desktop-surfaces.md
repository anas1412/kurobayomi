# 50-android-widgets — topic: desktop surfaces that replace home-screen widgets

Question answered here: the Android widgets put "recent unread covers" on the launcher and the lock screen. What does Kurobayomi on CachyOS/KDE (and other desktops) offer instead, what is dropped, and what does the user lose?

## 1. Surfaces considered

| Surface | Exists on Linux desktops | Chosen | Reason |
|---|---|---|---|
| KDE Plasma 6 applet (QML, `Plasma/Applet` package) | yes (KDE Plasma, the user's desktop) | yes, two applets | Native widget host on the user's desktop; can run a CLI and render images. |
| GNOME Shell extension | yes, but no widget host API; needs a JS extension per GNOME version | no | Maintenance cost per GNOME release; not the user's desktop. Dropped on GNOME. |
| Desktop Entry actions (`.desktop` `Actions=`) | yes, every freedesktop launcher (KDE menu, GNOME, dock) | yes | Cheap, works everywhere; gives "Updates" in the launcher's right-click menu. |
| Tray icon (StatusNotifierItem) with a menu of recent updates | yes on KDE; GNOME needs the AppIndicator extension | no (for now) | Needs a background process; the user did not ask for a tray. Open as a P3 idea, not in the task list. |
| Lock-screen widget | no (KDE lock screen has no plugin API; GNOME none) | no | Dropped; see section 4. |
| Notification with cover | yes (`org.freedesktop.Notifications`) | out of scope | Owned by area:35-data-misc-updater-notification-export (notifications); this area does not add notifications. |
| D-Bus service with push signals | yes (`zbus`) | no (poll chosen) | A daemon would need a second long-running process. Poll keeps the applet independent of the app window. Recorded as Q-2. |

## 2. The KDE applet

### 2.1 Package layout (PROPOSAL)

```
dist/plasmoid/org.kurobayomi.updates.manga/
  metadata.json
  contents/ui/main.qml
  contents/ui/CoverTile.qml
  contents/ui/Placeholder.qml   (optional split; keep in main.qml if short)
dist/plasmoid/org.kurobayomi.updates.anime/
  metadata.json
  contents/ui/main.qml          (same file, copied at build time, or symlinked in the package step)
```

`metadata.json` (manga; anime differs in `Id` and `Name`):

```json
{
  "KPlugin": {
    "Id": "org.kurobayomi.updates.manga",
    "Name": "Updates",
    "Description": "See your recently updated library entries",
    "Icon": "io.github.blackbox.Kurobayomi",
    "Category": "Utilities"
  },
  "KPackageStructure": "Plasma/Applet",
  "X-Plasma-API-Minimum-Version": "6.0"
}
```

Verify keys with `kpackagetool6 --type Plasma/Applet --install <dir>` and `plasmoidviewer -a org.kurobayomi.updates.manga`. The `Name` and `Description` values come from `label_recent_updates` ("Updates") and `appwidget_updates_description` ("See your recently updated library entries"); the anime applet uses `label_recent_anime_updates` ("Anime Updates").

Why two applets and not one with a setting: Android has two providers per kind (home and lock) and no per-instance configuration. Two Plasma applet ids reproduce "pick the kind when you add it" without writing a configuration dialog (which Android also never had).

### 2.2 Data path

1. The applet's `Timer` (60 s, `triggeredOnStart: true`) and the resize debounce call `executable.connectSource(cmd)`.
2. `cmd` = `kurobayomi --updates-json --kind <manga|anime> --rows <R> --cols <C>`.
3. The `executable` engine returns `stdout`, `stderr`, and `exit code`. `onNewData` parses `stdout` as JSON (schema 1) and sets `payload`.
4. `disconnectSource(source)` is called in the handler so the engine does not keep the finished command.
5. Covers are `Image` elements with `source` = `"file://" + item.cover_path` (only when `cover_path` is non-null).

### 2.3 Engine keys (executable engine, Plasma 6)

Keys used: `stdout`, `stderr`, `exit code`. These are the keys of the Plasma 5-compat executable DataSource (`org.kde.plasma.plasma5support`). Verify on the installed Plasma; they have been stable across Plasma 5 and 6 but this plan does not pin a Plasma patch version.

### 2.4 Click

`executable.connectSource("kurobayomi --open " + kind + ":" + entryId)`. The `--open` argument is parsed by the running app instance through GApplication's command-line handling (area:36-ui-shell-navigation-security-webview). If the app is not running, the same command starts it and then applies the target.

### 2.5 Sizing

- The grid is computed from the applet's own `width` and `height` with the Android cell pitch (64 x 95 logical px). Min 1 x 1 cell, max 10 x 10, default 4 x 2 (256 x 190 px). See `p1-ui-parity.md` section 2.7.
- The applet does not set `Layout.minimumWidth` from Android's `minResizeWidth`. The cell grid implies the minimum.

### 2.6 Colours

Plasma applets inherit the active colour scheme. Use `Kirigami.Theme.textColor`, `Kirigami.Theme.backgroundColor`, `Kirigami.Theme.highlightColor`. Do not hard-code colours except the placeholder fill `#1F888888`, which is kept from Android (`cover_placeholder`).

### 2.7 What the applet cannot do (and the workaround)

- No push refresh: 60 s latency. Workaround: header click in error or empty state; a manual refresh button is not added (YAGNI, and it would be the only control in an otherwise passive widget).
- No cover-screen or keyguard placement: Plasma's lock screen does not run applets.
- No per-cover context menu: Android had none either.

## 3. Desktop Entry action

`dist/kurobayomi.desktop` (the `Actions=` line and the action group are owned by this area; the rest of the file belongs to area:01-foundation-app-bootstrap):

```ini
Actions=Updates;

[Desktop Action Updates]
Name=Updates
Exec=kurobayomi --open updates
```

Works in KDE's application menu (right-click on the launcher), GNOME's dock actions, and any freedesktop launcher that reads `Actions=`. The action name `Updates` matches Android's `show_recently_updated` shortcut (`app/shortcuts.xml`, label `label_recent_updates`).

## 4. Dropped surfaces and what the user loses

| Android surface | Dropped because | What the user loses | Replacement |
|---|---|---|---|
| Keyguard (lock screen) cover grid (`updates_grid_lockscreen_widget_info.xml`, `widgetCategory="keyguard"`) | no lock-screen widget API on KDE or GNOME | seeing recent covers without unlocking | none; the in-app Updates tab after unlock |
| Samsung cover screen (`samsung-appwidget-provider`, `display="sub_screen"`, `privacyWidget="true"`) | Samsung-only SDK | seeing covers on the sub-screen | none |
| Home-screen widget provider (`android.appwidget.provider`, `updates_grid_homescreen_widget_info.xml`) | replaced by the Plasma applet on KDE | the widget on non-KDE desktops | `.desktop` action on any desktop; the Updates tab |
| Widget-picker previews (`updates_grid_widget_preview.webp`, `updates_grid_coverscreen_widget_preview.webp`) | picker is Android-only | previews in the picker | applet screenshot in the store/metadata (later; not in this plan) |
| Push refresh (`MangaWidgetManager`, `AnimeWidgetManager`) | no push surface; poll chosen | immediate refresh after reading a chapter | 60 s poll; the Updates tab updates immediately |
| Material You colours | Android 12+ wallpaper-derived palette | the wallpaper-tinted card | Plasma colour scheme (the user's theme) |

No feature in this scope is dropped silently. Each row above has a task id in `p1-tasks.md`.

## 5. The lock rule on every surface

Every desktop surface that shows titles or covers must obey the same rule as the Android widget:

- If the lock flag `use_biometric_lock` is true, the surface shows only the locked message.
- The CLI decides, not the applet, so the rule cannot be bypassed by a different applet build.
- The applet never caches a previous unlocked payload across a lock change (it replaces `items` with `[]` when `locked` is true).

The app-level unlock prompt is owned by area:36-ui-shell-navigation-security-webview and is not re-implemented here.

## 6. Packaging and install

- The applet packages are installed by a build step: `kpackagetool6 --type Plasma/Applet --install dist/plasmoid/org.kurobayomi.updates.manga` (and the anime twin). Uninstall with `--remove`.
- The binary `kurobayomi` must be on `PATH` for the applet's command. Arch: `/usr/bin/kurobayomi`.
- The applet and the `.desktop` file are optional packages (no runtime dependency for the core app). The app runs without them.
- Flatpak: the applet runs on the host, so the Flatpak build cannot host it; the Flatpak build ships the `.desktop` action only, and the applet is a host package. Document this in the packaging task.

## 7. Testing the surfaces

- `plasmoidviewer -a org.kurobayomi.updates.manga` for the visual states (loading, content, empty, locked, error).
- `env -u DISPLAY -u WAYLAND_DISPLAY kurobayomi --updates-json --kind manga --rows 2 --cols 4` must print JSON and exit 0 (the CLI path has no display dependency).
- `desktop-file-validate dist/kurobayomi.desktop` must pass.
- Lock check: set `use_biometric_lock` to true, run the CLI, expect `"locked": true` and an empty `items`.

## 8. Open items

- Tray icon with a recent-updates menu: a P3 idea, not in the task list (no request, adds a background process).
- GNOME Shell extension: out of scope.
- Push refresh over D-Bus: Q-2 in the Rust design.
