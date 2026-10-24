# Cross-platform readiness tasks

### T-req-platform-001 — Create the kuro-platform crate and the Paths trait
- Priority: P0
- Effort: M
- Depends: area:01-foundation-app-bootstrap
- Sources: core/common/src/main/java/tachiyomi/core/common/storage/FolderProvider.kt, core/common/src/main/java/tachiyomi/core/common/storage/AndroidStorageFolderProvider.kt
- Acceptance:
  - a workspace crate `kuro-platform` exists with features `platform-linux`, `platform-windows`, `platform-macos`
  - the `Paths` trait returns data, config, cache, log, temp, default downloads, default backups, covers and plugin directories; the Linux implementation follows XDG base directories
  - no other crate builds a directory path from a string literal (checked by a grep test in CI)

### T-req-platform-002 — Forbid OS-specific code outside kuro-platform
- Priority: P0
- Effort: S
- Depends: T-req-platform-001
- Sources: app/src/main/AndroidManifest.xml
- Acceptance:
  - a CI check fails when `cfg(target_os`, `xdg-open`, `libsecret`, `zbus` or `XDG_` appears outside the `kuro-platform` crate
  - the rule is written in the contributor docs

### T-req-platform-003 — SecretStore trait with Linux implementation
- Priority: P0
- Effort: M
- Depends: T-req-platform-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt
- Acceptance:
  - tracker tokens and source credentials are stored and read only through `SecretStore`
  - the Linux implementation uses the Secret Service; when no keyring daemon is running the app falls back to an encrypted file in the config directory and tells the user once

### T-req-platform-004 — Notifier trait with Linux implementation
- Priority: P1
- Effort: M
- Depends: T-req-platform-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - every Aniyomi notification channel maps to a `Notifier` category the user can mute
  - notifications support a title, text, progress, and action buttons routed back into the app

### T-req-platform-005 — MediaControls trait for the player
- Priority: P1
- Effort: M
- Depends: T-req-platform-001, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - play/pause/next/previous/seek from media keys and the desktop media widget control the player through `MediaControls`
  - now-playing title and artwork are published

### T-req-platform-006 — WebViewHost trait with browser-fallback path
- Priority: P1
- Effort: L
- Depends: T-req-platform-001, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewScreenModel.kt
- Acceptance:
  - Cloudflare solving and tracker login work through `WebViewHost`
  - when no embedded web view exists the host opens the system browser, receives the OAuth redirect on a loopback port, and lets the user import cookies for a source

### T-req-platform-007 — Opener, IdleInhibitor, SingleInstance and ExternalPlayer traits
- Priority: P1
- Effort: M
- Depends: T-req-platform-001
- Sources: app/src/main/AndroidManifest.xml
- Acceptance:
  - opening URLs, revealing files, launching an external player, preventing screen idle during playback and forwarding command-line URLs/files to the running instance all go through these traits

### T-req-platform-008 — Portable file-name sanitiser
- Priority: P0
- Effort: S
- Depends: T-req-platform-001
- Sources: core/common/src/main/java/tachiyomi/core/common/storage/UniFileExtensions.kt
- Acceptance:
  - one function builds download and local-source folder/file names, valid on ext4 and NTFS (reserved names, forbidden characters, trailing dots/spaces, 255-unit limit)
  - the function has unit tests with the reserved names and a round-trip test against Aniyomi's folder naming

### T-req-platform-009 — Bundled icons and CSS so the look does not depend on the host theme
- Priority: P1
- Effort: M
- Depends: area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt
- Acceptance:
  - every icon used by the UI resolves through one function with bundled symbolic SVG fallbacks
  - the Tokyonight CSS ships with the app and is applied identically on every platform

### T-req-platform-010 — CI cross-compile check for Windows
- Priority: P1
- Effort: S
- Depends: T-req-platform-002
- Sources: build.gradle.kts
- Acceptance:
  - CI runs `cargo check --target x86_64-pc-windows-gnu` for every non-UI crate from the first milestone on
  - a failure blocks the merge

### T-req-platform-011 — Windows port spike (later milestone)
- Priority: P3
- Effort: XL
- Depends: T-req-platform-003, T-req-platform-004, T-req-platform-005, T-req-platform-006, T-req-platform-007
- Sources: app/src/main/AndroidManifest.xml
- Acceptance:
  - a Windows implementation of every `kuro-platform` trait exists
  - the app starts, plays a local video through the libmpv render API in a GtkGLArea, reads a local manga, and passes the smoke run
  - a GTK-bundled Inno Setup installer is produced (the only Windows package)
