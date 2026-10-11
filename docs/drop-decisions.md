# Dropped Android build steps

Decisions recorded for area 54. Each item names the task, what was dropped, and what the user loses. Checked by
`scripts/check-android-leftovers.sh`.

| Task | Dropped | Replaced by | User impact |
|---|---|---|---|
| T-54a-014 | Upstream funding links file | nothing; the repository has no funding file | none for the app; the upstream maintainer's sponsor links are not shown here. A maintainer who wants links adds their own and notes it in CONTRIBUTING.md |
| T-54a-022 | Android integer version number | `[workspace.package] version` in `Cargo.toml` is the only version | none; Flatpak updates are ordered by the metainfo `<release>` list and the branch |
| T-54a-024 | Generated ahead-of-time compilation profile (37,790 lines) and its generator and readme | startup is measured by `scripts/bench-startup.sh` | none; a native binary has no JIT warm-up for a profile to replace |
| T-54a-025 | The macrobenchmark module | `scripts/bench-startup.sh`, `docs/benchmarking.md` | none |
| T-54a-026 | ProGuard and consumer rule files (14 files) | none; nothing in the Rust build reads a keep list | none |
| T-54a-030 | Variant filter block that referred to flavors that do not exist | none | none; the code never ran |
| T-54a-036 | Precompiled build plugins directory | workspace lints, `cargo xtask lint` | none |
| T-54a-043 | Glance home-screen widget | see below | the Android home-screen widget is gone |
| T-54a-060 | Android-only CI steps | see the table below | none |

## Extension keep list is obsolete

The extension API keep list (the source-api consumer rule file and lines 25 to 31 of the app rule file) existed so the
Android shrinker would not remove classes that extension APKs load by name. Extensions in Kurobayomi are not loaded
by class name (area 20, extension strategy redesign), and there is no shrinker. The list is obsolete. This note is the
record for it, because the area 20 decision log sits in the read-only plans.

## Home-screen widget

No `kb-widget` crate exists in v1. A desktop widget was added later as an optional extra: the two Plasma applets in
`dist/plasmoid/` show recent updates from the same data source (`kurobayomi --updates-json`; see `PACKAGING.md`).
A GNOME extension is still a possible later feature with the same data source.

## CI steps (p1-topic-ci-and-release.md, section 1)

No `setup-java` or `setup-gradle` action remains in any workflow.

| # | Step | Status | Replacement or note |
|---|---|---|---|
| 1 | Clone | kept | `actions/checkout` with `fetch-depth: 0` so the commit count is right |
| 2 | Dependency review | replaced | `cargo deny check` |
| 3 | Set up JDK 17 | dropped | Rust toolchain from `rust-toolchain.toml` |
| 4 | Set up Gradle | replaced | Rust build cache |
| 5 | Formatting check | replaced | `cargo xtask lint` |
| 6 | Assemble release | replaced | `cargo build --profile dist --locked` (`--features updater` for the tarball only) |
| 7 | Unit tests | replaced | `cargo test --workspace --locked` (`scripts/test-workspace.sh`) |
| 8 | Upload APK | replaced | upload the tarball and the Flatpak bundle |
| 9 | Upload R8 mapping | dropped | no R8 mapping file to share; the release carries split debug info (no user impact in practice) |
| 10 | Get tag name | kept | plus a check that the tag equals `v` and the `Cargo.toml` version |
| 11 | Sign APK (third-party signing action) | dropped | GPG-signed `SHA256SUMS` (T-54a-056) |
| 12 | Clean up artifacts and hash | replaced | `cargo xtask dist` writes `SHA256SUMS` |
| 13 | Create release | kept | same release action, draft release, body from the changelog |

## Android platform pieces not carried over (areas 26, 35 and 53)

Each row is checked by `src/dropped/mod.rs`: the test compares the row word for word and scans the code of `src/` for the dropped names.
The drop register of the legacy import (T-11a-029) is `src/dropped/register.rs`.
T-11a-029 is PARTIAL: the table, `lookup()`, `report()` and their tests exist, but nothing calls them yet, because the T-11a-010 normaliser and the import dialog do not exist in `src/` or `crates/`. When the normaliser lands it must skip keys through `dropped::register::lookup()` and the import dialog must list `report()`. The MIUI installer is covered only through the `extension_installer` row.

| Task | Dropped | Replaced by | User impact |
|---|---|---|---|
| T-26a-120 | foreground service declaration of the library update job | the resident process or the systemd timer | nothing beyond background running when the app is closed in mode A without the portal |
| T-26a-121 | notification channels and the deprecated channel cleanup | freedesktop urgency per kind and one notification switch (T-26a-036) | per-channel system settings |
| T-26a-122 | Wi-Fi pre-check for Android versions below 28 | the NetworkManager constraint (T-26a-027) in every case | none |
| T-26a-123 | FileProvider URIs for shared files | plain file:// URIs from gio (T-26a-096) | none |
| T-26a-124 | Injekt dependency lookup | explicit PassDeps construction at startup | none |
| T-26a-125 | WorkManager migrations | validation of the stored interval: only 0, 12, 24, 48, 72 and 168 hours, other values snap to the nearest choice | none |
| T-26a-126 | decoding the launcher icon bitmap for notifications | the freedesktop app icon name | none |
| T-35a-012 | channel creation and the nine deprecated channel ids | the notification switches of T-35a-004 | per-channel system settings |
| T-35a-025 | the unused ACTION_OPEN_ENTRY constant | entry opening of T-35a-015 | none |
| T-35a-026 | group summary dismissal of the notification receiver | none; desktop notices have no group summary | none visible; a single notice cannot be left behind |
| T-35a-027 | Samsung S Pen remote actions | the five actions as keyboard keys (T-35a-028) | S Pen remote gestures |
| T-35a-029 | searchable.xml, the Android global search hint and search provider | the in-app global search entry; the GNOME search provider is an optional extra of T-53a-066, not part of v1 | system-wide search for library titles on Android |
| T-35a-030 | FileProvider paths (provider_paths.xml) | plain file paths; no content URIs are created | none |
| T-35a-031 | network_security_config.xml (the Android cleartext and certificate-authority policy) | the reqwest client: rustls with the system certificate verifier, so the system and user-installed CAs are trusted, and http:// URLs are not refused | none |
| T-35a-056 | APK install and in-place AppImage self-replacement | the update notice offers Open folder and the user replaces the file | the app cannot replace itself |
| T-35a-059 | foreground service and the data sync service type | none; a desktop process is not suspended | none |
| T-35a-060 | the OsTooOld result of the update check | none; the check has no operating system version gate | none |
| T-35a-061 | the preview build channel with commit-count tags | one stable channel | preview builds through the in-app updater |
| T-35a-063 | the runtime notification permission check | the toast fallback of T-35a-011 and T-35a-002 when the notification service is missing | none |
| T-35a-064 | accent colour tint on notifications | the desktop theme tint | Android-style tinted notifications |
| T-35a-065 | Android sound override for the backup-complete notice | none; the category keeps high priority and the desktop picks the sound | the backup-complete notice may play the desktop sound |
| T-53a-036 | extension install services and activities | extension install of area 20 | installing APK extensions, and the vendor-skin timing workaround (defect E-29) is not carried |
| T-53a-038 | package install, delete and query permissions and the privileged installer provider; the extension_installer preference is not ported | none; no package query or install permission exists on Linux | installing extensions through the privileged helper app |
| T-53a-046 | the deep-link trampoline activities | route_request receives the search request directly | none; the forwarding was internal |
| T-53a-052 | the ten unreferenced player animations | none; the player has no transition, as in Aniyomi | none |
| T-53a-056 | the unused preference switch layout | gtk::Switch rows | none |
| T-53a-059 | the chapter download menu | the row menu of the download queue (T-53a-060) | none |
| T-53a-061 | the old bottom navigation menu | the HomeScreen tab set of area 36 | none |
| T-53a-062 | the md_* keep rule | none; no md_* layout exists | none |
| T-53a-064 | FileProvider and the path whitelist | files are opened through Opener | none visible |
| T-53a-065 | S Pen remote actions | keys Right, Left, Page Up, Page Down, N and P in the reader and player; Escape and Alt+Left go back | S Pen remote gestures |
| T-53a-067 | the empty module manifests and the macrobenchmark module | one Cargo crate per module, none needs a manifest | none |
| T-53a-068 | the home-screen widget receivers and provider files | the Updates tab (area 42) and desktop notices (area 35); the optional Plasma applets read the same data | the Android home-screen widget |

## Phone-only features removed (desktop)

A desktop window has no touch screen, no sensors, no task switcher and no battery-saver service, so these phone
features are removed, not hidden. The preference keys are listed in `DROPPED_KEYS` in `crates/kuro-prefs/src/registry.rs`
(the registry has no row for them), the text names in `DROPPED_NAMES` in `crates/kuro-i18n/src/import.rs` (the importer
leaves them out of every catalog). A key that an old `config.toml` or an Aniyomi backup still carries is ignored: the
config loader does not read a field it does not know, and the restore skips the key, counts it and reports no error.
Kept on purpose: page turning by swipe or drag in the reader viewers, the horizontal seek gesture and the double taps of
the player, the keyboard shortcuts, the volume slider and mouse-wheel volume, the remembered volume, the update and backup
scheduling constraints (unmetered network, power, battery level), notifications and the torrent server.

| Task | Dropped | Replaced by | User impact |
|---|---|---|---|
| T-desk-001 | screen orientation lock of the reader and the player: `pref_default_orientation_type_key`, `pref_default_player_orientation_type_key`, the per-series orientation bits of the reader flags, and the migration steps that renumbered the old values (rows 12, 13 and 42 of the migration table are DROP) | none; a window is resized by the window manager | none; the stored values are ignored |
| T-desk-002 | battery optimisation rows and the "don't kill my app" note (no preference existed; the texts `pref_disable_battery_optimization`, `battery_optimization_disabled` and their neighbours are gone) | none | none; the backup battery threshold and the update constraint `battery_not_low` stay |
| T-desk-003 | secure screen: `secure_screen_v2` and its older spelling `secure_screen`, with the conversion between them | none; a window cannot be hidden from capture or from the window list | none; screenshots of the app are not blocked, as before |
| T-desk-004 | Shizuku and the extension installer choice: `extension_installer` (already unregistered), the installer texts | the single kuroext install path | none |
| T-desk-005 | swipe actions of the chapter and episode rows: `pref_chapter_swipe_start_action`, `pref_chapter_swipe_end_action`, `pref_episode_swipe_start_action`, `pref_episode_swipe_end_action`, the enums `ChapterSwipeAction` and `EpisodeSwipeAction`, the swipe threshold and the swipe labels (quirk Q22, the crossed keys, goes with them) | the row context menu | row actions are one right click away |
| T-desk-006 | pull to refresh and haptics (no preference existed) | none | none |
| T-desk-007 | brightness and volume gestures of the player: `pref_gesture_volume_brightness`, `pref_swap_volume_and_brightness`, `pref_remember_brightness`, `player_brightness_value` (brightness had no other control) | keyboard shortcuts, the volume slider and mouse-wheel volume | no touch drags for volume or brightness; `pref_remember_volume` and `player_volume_value` stay |
| T-desk-008 | tablet layout mode: `tablet_ui_mode` | none | none; a stored value is ignored |
| T-desk-009 | the Monet app theme (dynamic colours of the phone wallpaper): the `MONET` value of `pref_app_theme` | the GTK theme default and the other palettes | a stored `MONET` reads as the default theme |
