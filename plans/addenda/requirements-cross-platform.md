# Standing requirement: keep Kurobayomi portable to Windows (and macOS) later

Status: user requirement, recorded 2026-10-08. Linux is the only shipping target for now. Windows is a later port, but the code must be structured now so that port is a platform-layer job and not a rewrite.

## Why this is cheap now and expensive later

GTK4 (gtk4-rs), tokio, rusqlite, reqwest, serde, the `image` crates and libmpv all run on Windows. The parts of Aniyomi that touch the operating system (secrets, notifications, media keys, web view, storage folders, file associations, background scheduling) are exactly where a Linux-only implementation would leak into the rest of the code. If those calls are scattered through the app, a Windows port means touching every area. If they sit behind one small crate, it means writing one new implementation of that crate.

## Rule set (applies to every area's Rust design)

1. All OS-specific code lives in one workspace crate named `kuro-platform`. No other crate may use `#[cfg(target_os = ...)]`, call `libsecret`, `zbus`, `xdg` or `std::env::var("XDG_*")`, or shell out to `xdg-open` directly.
2. `kuro-platform` exposes traits, with a Linux implementation first (module `linux`) and room for `windows` and `macos` modules selected by Cargo features `platform-linux`, `platform-windows`, `platform-macos`.
3. Application crates receive these traits through the app context (the same place the Injekt/DI replacement from area:01-foundation-app-bootstrap lives), never through globals.
4. File-system paths are never built from string literals. Every directory comes from the `Paths` trait (data, config, cache, downloads default, backups default, covers, logs, temp, extension/plugin dir).
5. Path handling uses `std::path::Path`/`PathBuf` and never assumes `/` separators; file names derived from titles go through one sanitising function that is valid for NTFS as well (reserved names CON/PRN/AUX/NUL/COM1-9/LPT1-9, forbidden characters `<>:"/\|?*`, trailing dots and spaces, 255 UTF-16 unit limit) because the same download folder layout must be readable on both systems.
6. Nothing in the UI crates may assume a Linux theme or icon set: icons are looked up through one function that falls back to bundled symbolic SVGs, and the app ships its own Tokyonight CSS so the look does not depend on the host GTK theme.
7. CI compiles the workspace for `x86_64-pc-windows-gnu` (`cargo check`) from the first milestone on so Windows drift is caught immediately, even though no Windows binary is shipped yet.

## Platform traits to define (and their Linux / Windows implementations)

| Trait | Responsibility | Linux implementation | Windows implementation (later) |
|---|---|---|---|
| `Paths` | data/config/cache/log/temp/default download+backup dirs | XDG base dirs via the `directories` crate | Known Folders (`%APPDATA%`, `%LOCALAPPDATA%`) via the same crate |
| `SecretStore` | tracker OAuth tokens, source credentials, API keys | Secret Service (libsecret) via the `keyring` crate (or `oo7`) | Windows Credential Manager via `keyring` |
| `Notifier` | download/update/backup notifications with actions and progress | `gio::Notification` or `notify-rust` | Windows toast notifications via `notify-rust`/WinRT |
| `MediaControls` | media keys, now-playing metadata, transport controls | MPRIS over D-Bus via the `souvlaki` crate | System Media Transport Controls via `souvlaki` |
| `WebViewHost` | Cloudflare challenge solving, tracker login pages, source WebView | WebKitGTK (`webkit6` crate) | WebView2 (`webview2-com`); fallback is "open system browser + loopback redirect + cookie import" |
| `Opener` | open URL / reveal file / launch external player | `xdg-open`/`gio::AppInfo` via the `open` crate | `ShellExecute` via the `open` crate |
| `IdleInhibitor` | keep the screen awake while playing | `org.freedesktop.ScreenSaver`/portal Inhibit | `SetThreadExecutionState` |
| `SingleInstance` | one running app, forward CLI args/URLs (deep links, .tachibk files) | `gio::Application` unique id | named mutex + named pipe |
| `SchedulerBackend` | optional wake-up of the app for library updates/auto-backup when it is not running | systemd user timer or in-process only | Task Scheduler or in-process only |
| `FileAssociations` | register `.tachibk`, `.cbz`, `.epub`, custom URL scheme handlers | `.desktop` file + shared-mime-info XML | registry entries written by the installer |
| `Autostart` | optional start at login | XDG autostart entry | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| `ExternalPlayer` | launch mpv/vlc with a stream URL and headers | spawn process / D-Bus activation | spawn process |
| `Biometrics/AppLock` | app lock replacement for Aniyomi's biometric unlock | PAM/polkit prompt or passphrase | Windows Hello via WinRT or passphrase |

## Known Windows risks to design around now

- **Embedding mpv:** use the libmpv render API drawing into a `GtkGLArea` on every platform (never an X11/Wayland window id embedding), so the same player widget works on Windows.
- **Web view:** WebView2 cannot be embedded inside a GTK4 widget tree as simply as WebKitGTK. Cloudflare and login flows must therefore work through the `WebViewHost` trait with the browser-fallback path implemented from day one on Linux too.
- **Packaging:** GTK4 on Windows is distributed with the GTK runtime bundled (gvsbuild/MSYS2 builds, then an installer or MSIX). This is a packaging task for the Windows milestone and has no impact on the Rust code if rules 1-7 are followed.
- **Case-insensitive file systems:** local-source scanning, download folder names and the download cache index must treat paths case-insensitively when the platform requires it.

## Tasks

See `requirements-cross-platform-tasks.md` in this folder. Every area's Rust design must reference the traits above instead of inventing its own OS access.
