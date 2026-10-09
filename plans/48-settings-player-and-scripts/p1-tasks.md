# P1 tasks: 48-settings-player-and-scripts

Task IDs use the 48a prefix (001 to 058). Sources are repo-relative paths in `/home/blackbox/aniyomi-src` (the behaviour reference). Target Rust modules are named in each acceptance list. Priorities follow the plan rules: P0 needed for daily reading and watching; P1 parity features; P2 long tail; P3 Android-only with a replacement or drop decision.

Coverage: every file in `p1-inventory.md` is referenced by at least one task (or is covered by a P3 drop task). The open decisions are listed at the end of this file.

---

## A. Preference model and global UI settings

### T-48a-001 — Typed preference store (SQLite kv, cache, enum-by-name)
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system, area:04-db-anime-schema-migrations
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - `src/prefs/mod.rs` implements `Pref<T>` with `get` returning the default for a missing or undecodable value, and never an error.
  - Enums are stored by variant name; an unknown name returns the default (unit test table for `PlayerOrientation`, `SingleAction`, `Debanding`, `AudioChannels`, `ProxyMode`, `ThemeMode`).
  - `f32` values such as `0.85` round-trip exactly.
  - The `prefs(key TEXT PRIMARY KEY, value TEXT NOT NULL)` table is loaded once into an in-memory cache; a write updates the cache first, then persists through the DB worker.

### T-48a-002 — UI preference model (theme, nav, start screen, date format)
- Priority: P0
- Effort: S
- Depends: T-48a-001
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - `src/prefs/ui.rs` exposes the eight keys from `p1-data-and-config.md` section 1.1 with the listed defaults (`pref_theme_mode_key` SYSTEM, `pref_app_theme` DEFAULT on desktop, `pref_theme_dark_amoled_key` false, `relative_time_v2` true, `app_date_format` "", `tablet_ui_mode` AUTOMATIC, `start_screen` ANIME, `bottom_rail_nav_style` MOVE_HISTORY_TO_MORE).
  - Stored names of every enum equal the Kotlin constant names.

### T-48a-003 — Theme mode and AMOLED switch
- Priority: P1
- Effort: M
- Depends: T-48a-002, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - LIGHT forces the light GTK theme, DARK forces dark (`gtk::Settings` `gtk-application-prefer-dark-theme`), SYSTEM follows the xdg-desktop-portal `color-scheme` (via `ashpd`, fallback to GTK default when the portal is absent).
  - Changing the setting applies without restart.
  - AMOLED switch is stored and read; it has no visual effect unless the theme module (area:36) implements it.

### T-48a-004 — App colour themes (19 live palettes, dynamic replaced)
- Priority: P2
- Effort: L
- Depends: T-48a-002, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt
- Acceptance:
  - Each of the 19 live `AppTheme` values maps to a CSS token set in `src/ui/theme.rs`; the palette values are copied from the Kotlin theme definitions (area:36 owns the values).
  - `MONET` ("Dynamic") maps to "follow the system (GTK) colours" with no custom palette.
  - Stored `DARK_BLUE`, `HOT_PINK`, `BLUE` map to `DEFAULT` at read time and are not rewritten.

### T-48a-005 — Start screen
- Priority: P0
- Effort: S
- Depends: T-48a-002, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt
- Acceptance:
  - Five values ANIME, MANGA, UPDATES, HISTORY, BROWSE open the matching tab at startup, with titles "Anime", "Manga", "Updates", "History", "Browse".
  - Default ANIME on first run.

### T-48a-006 — Navigation style (move one tab into More)
- Priority: P1
- Effort: M
- Depends: T-48a-002, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt
- Acceptance:
  - Four styles (`MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE`) produce exactly five navigation tabs with More always present.
  - The moved tab appears under More with the icon listed in `p1-behavior.md` section 15.

### T-48a-007 — Tablet UI mode: drop, use responsive layout
- Priority: P3
- Effort: S
- Depends: T-48a-002
- Sources: app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt
- Acceptance:
  - Decision recorded: the four `TabletUiMode` values are not offered on desktop; the stored `tablet_ui_mode` is ignored.
  - The shell switches between single pane and two pane by window width (threshold 720 px, shared with T-48a-010).

### T-48a-008 — Date format setting
- Priority: P1
- Effort: S
- Depends: T-48a-002
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/presentation/components/DateText.kt
- Acceptance:
  - Empty value gives the localised short date.
  - A non-empty pattern is validated with chrono; an invalid pattern falls back to the short date and shows "Invalid format" under the entry.

### T-48a-009 — Relative time toggle
- Priority: P1
- Effort: S
- Depends: T-48a-002
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - `relative_time_v2` defaults to true; when true, date labels in lists use relative forms ("2 minutes ago"), when false, absolute dates.

---

## B. Player settings pages

### T-48a-010 — Player settings root (nine rows, sidebar and narrow layouts)
- Priority: P0
- Effort: M
- Depends: T-48a-012, area:47b-settings-widgets-search-more
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsMainScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/PlayerSettingsScreen.kt
- Acceptance:
  - `src/settings/player/mod.rs` shows the nine rows in the order and with the title/subtitle keys of `p1-behavior.md` section 1.
  - Width >= 720 px: sidebar plus page; below 720 px: list page with back button.
  - Title is "Player" when opened from main settings and "Player settings" otherwise.
  - Mockup spec entries `settings_player_custom_buttons`, `settings_player_editor`, `settings_player_code_editor` are routed to Rust builders (T-48a-045, T-48a-037, T-48a-038); placeholder data is removed.

### T-48a-011 — Search action in player settings
- Priority: P1
- Effort: S
- Depends: T-48a-010, area:47b-settings-widgets-search-more
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsMainScreen.kt
- Acceptance:
  - The header search button opens the cross-cutting settings search scoped to player settings (`SettingsSearchScreen(true)` equivalent).
  - Selecting a result opens the page and highlights the row for about 1 s (highlight key semantics, `p1-behavior.md` section 2).

### T-48a-012 — Preference row kit (switch, list, slider, text, edit, multi-line, info)
- Priority: P0
- Effort: L
- Depends: T-48a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/EditTextPreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt
- Acceptance:
  - `src/settings/player/rows.rs` builds each row kind from the `Row` enum in `p1-rust-design.md` section 9.1.
  - Edit dialog OK is enabled iff `text != stored && (non-blank || can_be_blank) && validate(text)`; invalid input shows an error icon and the validation message.
  - Disabled rows are greyed and insensitive (not hidden), as recorded in `p1-ui-parity.md` section 2.2.
  - List dialog applies on selection and closes; Cancel changes nothing.

### T-48a-013 — Controls and Hosters groups
- Priority: P0
- Effort: M
- Depends: T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - Five controls with defaults `pref_allow_gestures_in_panels` false, `pref_show_loading` true, `pref_show_current_chapter` true, `pref_remember_brightness` false; the volume-memory switch is not shown (T-48a-052).
  - Hosters: `pref_show_failed_hosters` false and `pref_show_empty_hosters` false, each stored and applied by the player controls (area:46).

### T-48a-014 — Display group
- Priority: P0
- Effort: M
- Depends: T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt
- Acceptance:
  - Rows: fullscreen (default true, always enabled on desktop), hide controls on open (false), volume as percentage (true), reduce motion (false), hide-controls time list 500..5000 ms step 500 (default 4000, label "N ms"), panel opacity slider 0..100 (default 60, label as percent).
  - The "system status bar" row is not shown (T-48a-020).

### T-48a-015 — Progress threshold and preserve position
- Priority: P0
- Effort: S
- Depends: T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - Progress list values 1.00, 0.95, 0.90, 0.85, 0.80, 0.75, 0.70 (labels "100%" … "70%"), default 0.85; a stored value outside the list is shown as "Custom (N%)" and kept (edge E-PS-06).
  - `pref_preserve_watching_position` default false.

### T-48a-016 — Intro skip group and AniSkip settings
- Priority: P1
- Effort: M
- Depends: T-48a-012, T-48a-010, area:45-player-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - Rows and defaults as in `p1-data-and-config.md` section 1.2 (intro skip on, auto skip off, Netflix style off, button timeout 5 with values 5..10 s, AniSkip off, disable on chapters on).
  - Children greyed when intro skip is off; "disable on chapters" greyed unless AniSkip is on; info text shown.

### T-48a-017 — PiP group as always-on-top mini player
- Priority: P2
- Effort: M
- Depends: T-48a-012, T-48a-010, area:45-player-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt
- Acceptance:
  - Group shown only when a compositor probe reports support (edge E-PS-05); otherwise hidden and the stored `pref_enable_pip` is left unchanged.
  - When enabled and supported, leaving the fullscreen player switches to a small always-on-top window (mpv `ontop` on X11; on Wayland the window is shown but not forced on top, documented).
  - Toast-on-episode-switch and replace-with-previous options keep their defaults (true, false).

### T-48a-018 — External player group (MIME-based)
- Priority: P1
- Effort: M
- Depends: T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - `pref_always_use_external_player` (false) and `external_player_preference` (default "") present.
  - Dropdown lists "None" plus apps from `gio::AppInfo::all_for_type("video/*")`; a stored app that is not installed shows "(unavailable)" (E-PS-11).
  - Launch with empty preference opens the default `video/*` handler or the chooser (E-PS-12).

### T-48a-019 — Orientation setting: drop
- Priority: P3
- Effort: S
- Depends: T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - Decision recorded: no orientation row on desktop; the stored `pref_default_player_orientation_type_key` is ignored and never deleted (E-PS-09).
  - User loss stated in the settings help: "Screen rotation is not used on desktop."

### T-48a-020 — System status bar setting: drop
- Priority: P3
- Effort: S
- Depends: T-48a-014
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt
- Acceptance:
  - Decision recorded: the row is not shown; the stored `pref_show_system_status_bar` is ignored.
  - No Android status bar exists on desktop; the window header bar is always visible in fullscreen mode unless the user hides it (area:36).

### T-48a-021 — Gesture sliders group (volume and brightness)
- Priority: P1
- Effort: M
- Depends: T-48a-022, T-48a-012
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsGesturesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt
- Acceptance:
  - `pref_gesture_volume_brightness` (true) and `pref_swap_volume_and_brightness` (false) present with their labels.
  - Brightness is applied as an mpv video filter (`brightness`, -100..100) instead of screen backlight; the change is documented in the settings help.
  - Vertical drag on the left half changes brightness and on the right half volume unless swapped (zones per area:46).

### T-48a-022 — Seeking group (horizontal seek, seek bar, intro length, skip length, precise seek)
- Priority: P0
- Effort: M
- Depends: T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsGesturesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt
- Acceptance:
  - Defaults: horizontal seek true, seek bar false, intro length 85 s, skip length 10 s (entries 30, 20, 10, 5, 3, 0 with 0 "Disable"), precise seek false.
  - Intro length dialog: spin 0..255 with "s" suffix; the initial value is the stored value; OK without change keeps it (E-GS-02); Cancel and Escape close without change.
  - Value 0 disables the intro button (E-GS-01).

### T-48a-023 — Double-tap action settings
- Priority: P0
- Effort: M
- Depends: T-48a-025, T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsGesturesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - Left default Seek (options None, Seek, PlayPause, Switch, Custom); centre default PlayPause (None, PlayPause, Custom); right default Seek (same as left).
  - Dropdown labels: None, Seek, Play/Pause, Change episode, Custom.
  - Info text lists the key names for `Custom` (desktop names, `p1-topic-custom-buttons.md` section 9).

### T-48a-024 — Media controls: MPRIS and keyboard media keys
- Priority: P1
- Effort: M
- Depends: T-48a-025, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsGesturesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `pref_media_previous` (Switch), `pref_media_playpause` (PlayPause), `pref_media_next` (Switch) apply to MPRIS (`mpris-server` crate) and to keyboard media keys.
  - Allowed options per slot match `p1-behavior.md` section 4.

### T-48a-025 — Single action gesture dispatch and custom key names
- Priority: P0
- Effort: M
- Depends: T-48a-001, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `src/player/gestures.rs` maps each `SingleAction` to: none, seek by skip length (0 = no seek), play/pause, change episode (previous for left/previous, next for right/next), or send the mpv key name for `Custom`.
  - Key names: `kurobayomi-double-tap-left`, `kurobayomi-double-tap-center`, `kurobayomi-double-tap-right`, `kurobayomi-media-previous`, `kurobayomi-media-play-pause`, `kurobayomi-media-next`.
  - Unit test: each (slot, action) pair dispatches exactly one command; a disallowed pair is not offered in the UI.

### T-48a-026 — Decoder: hardware decoding switch
- Priority: P1
- Effort: S
- Depends: T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsDecoderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - `pref_try_hwdec` default true; on start it sets mpv `hwdec` to `auto` (true) or `no` (false).
  - Android `mediacodec*` values are not offered.
  - Change applies at next player start; the page shows the note from `p1-ui-parity.md` section 2.4.

### T-48a-027 — Decoder: gpu-next switch
- Priority: P1
- Effort: S
- Depends: T-48a-026
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsDecoderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - `pref_gpu_next` default false selects mpv `vo=gpu-next`, otherwise `vo=gpu`.
  - If mpv rejects `gpu-next`, the player falls back to `gpu` and shows a toast (E-DC-03).

### T-48a-028 — Debanding and YUV420P filter chain
- Priority: P1
- Effort: S
- Depends: T-48a-026, T-48a-012
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsDecoderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - `src/player/vf.rs::build_vf_chain` returns `gradfun=radius=12` for CPU, `format=yuv420p` for YUV420P, joined by `,` when both are set (E-DC-01).
  - GPU debanding sets mpv `deband=yes` and does not change `vf` (E-DC-02).
  - Labels "None", "CPU", "GPU" as in Kotlin (raw names); default None; YUV420P default true.

### T-48a-029 — Subtitle page: preferred languages, whitelist, blacklist
- Priority: P1
- Effort: M
- Depends: T-48a-030, T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsSubtitleScreen.kt
- Acceptance:
  - Preferred languages validated by `lang::validate_list`; the error reads `Invalid code: "<code>"`.
  - Whitelist and blacklist: free text, no validation, empty allowed; each has its dialog subtitle text from `p1-behavior.md` section 6.
  - Values are used by the track selector in area:45.

### T-48a-030 — Language list validator
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsAudioScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsSubtitleScreen.kt, app/src/main/java/mihon/core/migration/migrations/PrefLangMigration.kt
- Acceptance:
  - `src/lang.rs::validate_list` splits on `,`, drops empty elements, trims, then validates each element (E-LG-01..07).
  - Accepts ISO 639-1 and ISO 639-3 codes from a static table (decision D-03).
  - Unit test table covers every E-LG case.

### T-48a-031 — Audio page
- Priority: P1
- Effort: M
- Depends: T-48a-030, T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsAudioScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - Preferred languages (validated), pitch correction (default true, mpv `audio-pitch-correction`), audio channels (dropdown Auto, Auto-safe, Mono, Stereo, Reverse stereo; default Auto-safe), volume boost cap slider 0..200 (default 30, mpv `volume-max` = cap + 100).
  - Channel mapping follows `p1-rust-design.md` section 3 (Auto→`auto`, Auto-safe→`auto-safe`; deliberate fix, E-AU-01).
  - ReverseStereo sets `af=pan=[stereo|c0=c1|c1=c0]` and clears it when another channel mode is chosen (E-AU-04).

### T-48a-032 — TorrServer settings page
- Priority: P2
- Effort: M
- Depends: T-48a-012, T-48a-010, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsTorrentScreen.kt, core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt
- Acceptance:
  - Rows and defaults as in `p1-topic-torrent-settings.md` section 1 (enable false, port "8090", trackers the 26-line default, proxy None, proxy URL "").
  - Legal notice shown once on the first enable (E-TS-01).
  - Trackers reset row enabled only when the value differs from the default (E-TS-09).

### T-48a-033 — TorrServer process lifecycle (replace JNI with child process)
- Priority: P2
- Effort: L
- Depends: T-48a-032, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt, core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt
- Acceptance:
  - `src/torrent/server.rs` starts a configured `torrserver` binary with port, data directory `$XDG_DATA_HOME/kurobayomi/torrserver`, and trackers; it reports the bound port or a readable error.
  - Enabling starts it, disabling stops it, a port or tracker change restarts it (E-TS-06, E-TS-07).
  - The child process is killed on app exit; no orphan process remains after a crash test.
  - Missing binary shows "Failed to start: torrserver not found" (E-TS-10).

### T-48a-034 — Torrent port, proxy URL and proxy mode validation
- Priority: P2
- Effort: S
- Depends: T-48a-032
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsTorrentScreen.kt
- Acceptance:
  - Port: `0..=65535`, empty invalid, error "Port must be a number between 0 and 65535" (E-TS-02).
  - Proxy URL: empty allowed only when mode is None; otherwise host and scheme in {http, https, socks4, socks4a, socks5, socks5h} (E-TS-03); messages "Invalid uri" and `Unsupported protocol: "<scheme>"`.
  - Mode None sends no proxy arguments (E-TS-04); credentials never appear in logs (E-TS-08).

### T-48a-035 — Advanced page: user files switch, mpv.conf and input.conf editors
- Priority: P1
- Effort: M
- Depends: T-48a-012, T-48a-010, T-48a-036
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AdvancedPlayerPreferences.kt
- Acceptance:
  - `mpv_scripts` switch default false; `pref_mpv_conf` and `pref_mpv_input` multi-line editors, default empty, OK enabled only when changed.
  - Editors store the text in the preference; no mirror file is written (E-AV-01).
  - "Open mpv-config folder" action opens the directory with `gio::AppInfo::launch_default_for_uri` (desktop addition).

### T-48a-036 — mpv runtime directory and player-start sync
- Priority: P0
- Effort: M
- Depends: T-48a-051, T-48a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - `src/player/mpv_dir.rs` writes `mpv.conf`, `input.conf`, `scripts/aniyomi.lua` and `scripts/custombuttons.lua` into the runtime directory at each player start.
  - User scripts, script-opts and shaders are copied (or referenced) only when `mpv_scripts` is on (E-AV-03); deleted user scripts disappear on next start (E-SY-01).
  - Fonts are available before the first frame (E-SY-02); `sub-fonts-dir` and `osd-fonts-dir` are set.
  - Bundled `subfont.ttf` and `cacert.pem` are replaced when their SHA-256 differs (E-SY-04).
  - Unwritable runtime directory falls back to the cache directory (E-SY-06).

### T-48a-037 — Script editor: file list, type switch, create, rename, delete
- Priority: P1
- Effort: M
- Depends: T-48a-051, T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/PlayerSettingsEditorScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/PlayerSettingsEditorScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/components/EditorScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/components/EditorDialogs.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/components/EditorTypeDropdown.kt
- Acceptance:
  - `src/editor/files.rs` lists regular files of `mpv-config/scripts` (Scripts) or `mpv-config/script-opts` (Script options), sorted case-insensitively (E-ED-01), with size (decimal units) and modified date labels (E-ED-11).
  - Create defaults to `file.lua` or `file.conf` with the base name selected; validation messages "File already exists" and "Invalid filename" (E-ED-02, E-ED-05..07).
  - Rename keeps a case-only change allowed (E-ED-03); delete asks for confirmation.
  - Empty state text and error toasts from `p1-ui-parity.md` section 3.

### T-48a-038 — Code editor: load, edit, save, unsaved guard
- Priority: P1
- Effort: M
- Depends: T-48a-037, T-48a-039
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/codeeditor/CodeEditScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/codeeditor/CodeEditScreenModel.kt
- Acceptance:
  - Opens the file by path; error state shows the message (or "Unknown exception").
  - Save is enabled only when modified; saving writes through a temp file and rename (E-CE-05, atomic); success toast "File saved"; failure toast with the error.
  - Back or Alt+Left with unsaved changes shows "Warning" with "You have unsaved changes. Do you still wish to go back?".
  - Non-UTF-8 file shows the banner and requires confirmation before the first save (E-CE-03); CRLF is preserved (E-CE-04).
  - Ctrl+S saves; double save is ignored while saving (E-CE-06).

### T-48a-039 — Syntax highlighter (Lua and conf rule sets)
- Priority: P1
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/codeeditor/Highlight.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/codeeditor/SyntaxHighlight.kt
- Acceptance:
  - `src/editor/highlight.rs` uses `fancy-regex` with the ten Lua rules and six conf rules verbatim from `p1-topic-mpv-editor-and-highlight.md` sections 4.1 and 4.2, in the same order, with the same colour per rule.
  - Java `\w`, `\d`, `\s` are replaced with explicit ASCII classes (E-HL-07, E-HL-10).
  - Highlighting is skipped above 15000 characters (E-CE-02, E-CE-11).
  - All E-HL cases pass as golden tests.

### T-48a-040 — mpv Lua bridge (user-data/aniyomi protocol)
- Priority: P0
- Effort: L
- Depends: T-48a-042, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/assets/aniyomi.lua
- Acceptance:
  - `src/player/lua_bridge.rs::parse_lua_call` covers the 14 properties and every row of `p1-topic-custom-buttons.md` section 8, including quote stripping, pipes in text, and 6-part `launch_int_picker`.
  - Every observed property is cleared to `""` after handling, including ignored values (E-LB-01, E-LB-02).
  - Bad integers are logged and ignored, not crashed (E-CB-08); unknown property ignored (E-LB-06).
  - Calls are applied on the GTK thread in arrival order (E-LB-07).
  - `software_keyboard` is a logged no-op on desktop (E-LB-08).

### T-48a-041 — custom_buttons table, migration and seed row
- Priority: P0
- Effort: S
- Depends: area:04-db-anime-schema-migrations
- Sources: data/src/main/sqldelightanime/dataanime/custom_buttons.sq, data/src/main/sqldelightanime/migrations/129.sqm
- Acceptance:
  - `src/db/custom_buttons.rs` creates the table with the seven columns of `p1-data-and-config.md` section 2 and no extra constraints.
  - The seed row (`_id` 1, `+85 s`, favourite, sortIndex 0, the three Lua bodies copied exactly) is inserted with `INSERT OR IGNORE` at migration time only (E-CB-18).
  - Migration is idempotent under `PRAGMA user_version`.

### T-48a-042 — Custom button repository and worker (create, update, delete, reorder, favourite)
- Priority: P0
- Effort: M
- Depends: T-48a-041, T-48a-001
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/interactor/CreateCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/DeleteCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ReorderCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ToggleFavoriteCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/UpdateCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/GetCustomButtons.kt, data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/custombuttons/exception/SaveCustomButtonException.kt
- Acceptance:
  - All writes run in the single DB worker, one command at a time; no separate mutex.
  - Create uses max(sortIndex)+1 or 0, read inside the same job (E-CB-04: 100 concurrent creates give 0..99).
  - Update never writes sortIndex (E-CB-05); favourite toggle reads fresh state (E-CB-06); reorder to same index is `Unchanged` (E-CB-12); out-of-range index gives an internal error toast (E-CB-13).
  - Delete renumbers in the same transaction (E-CB-14); deleting the favourite leaves none (E-CB-11).
  - Every error is logged (update included).

### T-48a-043 — custombuttons.lua generation and placeholder substitution
- Priority: P0
- Effort: M
- Depends: T-48a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButtonUpdate.kt
- Acceptance:
  - `src/player/custom_buttons/lua.rs` produces the header and per-button blocks of `p1-topic-custom-buttons.md` section 7 with the names `call_button_<id>` and `call_button_<id>_long`.
  - `$id` and `$isPrimary` are replaced textually in the same order as Kotlin (E-CB-03, E-CB-15).
  - Each body is wrapped so that one syntax error does not disable the other buttons (E-CB-01); unit test loads the output in an embedded Lua 5.1 interpreter.
  - Written to the runtime directory and loaded with `load-script`.

### T-48a-044 — Primary (favourite) button runtime state
- Priority: P0
- Effort: S
- Depends: T-48a-042, T-48a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomRightPlayerControls.kt
- Acceptance:
  - On player start the favourite is the primary button; its title is the favourite's name unless Lua set one first.
  - `set_button_title`, `reset_button_title`, `toggle_button show|hide|toggle` behave as in `p1-topic-custom-buttons.md` section 6.
  - Intro-skip button takes precedence over the custom button in the bottom-right controls (area:46 renders it).
  - Tap calls `call_button_<id>`; long press calls `call_button_<id>_long`.

### T-48a-045 — Custom buttons list screen
- Priority: P1
- Effort: M
- Depends: T-48a-042, T-48a-012, T-48a-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/components/CustomButtonScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/components/CustomButtonListItem.kt
- Acceptance:
  - `src/settings/player/custom_buttons.rs` lists buttons by sortIndex with the card layout of `p1-ui-parity.md` section 2.9 (name, star, edit, delete, three-line Lua preview).
  - Empty state text "You have no custom buttons. Tap the plus button to create a custom button."
  - Drag reorder writes once on drop; the list is not replaced from the DB while a drag is active (R-02).
  - Star shows the single favourite; toggling it on turns the previous favourite off.
  - Error state with Retry when the database cannot be read (desktop addition).

### T-48a-046 — Create and edit dialog with validation
- Priority: P1
- Effort: S
- Depends: T-48a-045, T-48a-039
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/components/CustomButtonDialogs.kt
- Acceptance:
  - Four fields in order: Title, Lua code, Lua code (on long press), On startup; labels, supporting texts and line counts as in `p1-ui-parity.md` section 2.9.
  - Confirm enabled iff title and Lua code are non-empty and the title is not used by another button; "Button already exists!" shown in error style.
  - Edit shows "(ID: n)"; edit never changes favourite state.
  - Confirm closes the dialog; a failed write shows the internal-error toast afterwards (Kotlin behaviour). Decision D-07 may change this to keep the dialog open until success.
  - Dialog state survives a list refresh (E-CB-07).

### T-48a-047 — Delete confirmation and delete semantics
- Priority: P1
- Effort: S
- Depends: T-48a-045, T-48a-042
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/components/CustomButtonDialogs.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/DeleteCustomButton.kt
- Acceptance:
  - Confirmation text `Do you wish to delete the button "<name>"?`, destructive OK, Cancel.
  - After delete the remaining buttons are renumbered; deleting the favourite leaves no favourite (E-CB-11).
  - Desktop choice (decision D-06) is recorded: keep no-promotion behaviour.

### T-48a-048 — Backup and restore of custom buttons (interaction)
- Priority: P1
- Effort: S
- Depends: T-48a-042, area:21-backup-format-and-create, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupCustomButtons.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/CustomButtonRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/CustomButtonBackupCreator.kt
- Acceptance:
  - Backup fields 1..6 (name, isFavorite, sortIndex, content, longPressContent, onStartup) and the container numbers 109 (LegacyBackup) and 506 (Backup) are honoured by area:21 and area:22.
  - Restore skips buttons whose name already exists (by name, as in Kotlin).
  - Restore produces at most one favourite (E-CB-09 fixed).
  - After a restore that adds buttons, a notice lists the names and states that they run Lua (security note in design section 14).

### T-48a-049 — Custom buttons help link
- Priority: P2
- Effort: S
- Depends: T-48a-045
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreen.kt
- Acceptance:
  - Help button (tooltip "Custom buttons guide") opens `https://aniyomi.org/docs/guides/player-settings/custom-buttons` with `gtk::UriLauncher`.
  - If launching fails, toast "Could not open link".

### T-48a-050 — Fonts and shaders directories
- Priority: P1
- Effort: S
- Depends: T-48a-051, T-48a-036
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `mpv-config/fonts` and `mpv-config/shaders` are created on demand (lazy, as in `StorageManager`).
  - Fonts placed there appear as subtitle fonts after the next player start.
  - Missing base folder shows the banner from E-SY-05 and the player still starts.

### T-48a-051 — Storage root and mpv-config layout
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - Base directory defaults to `$XDG_DATA_HOME/kurobayomi` and is changeable in settings (folder chooser via `gtk::FileDialog`).
  - Creates `mpv-config/` with `fonts`, `scripts`, `script-opts`, `shaders` on first use; creates nothing else in scope.
  - Base folder unavailable is reported by a banner (E-SY-05), not a crash.

### T-48a-052 — Remember volume and brightness: decision
- Priority: P2
- Effort: S
- Depends: T-48a-013, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt
- Acceptance:
  - Decision D-05 recorded: the volume switch (`pref_remember_volume`, no reader in Kotlin, E-PS-01) is not shown on desktop.
  - Brightness memory (`pref_remember_brightness`) is kept and applied on start when `player_brightness_value` is set (E-PS-02).

### T-48a-053 — External player package allowlist: replace with MIME lookup
- Priority: P3
- Effort: S
- Depends: T-48a-018
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Decision recorded: the 12 Android package names (`externalPlayers`) are not used on desktop; the list is derived from installed `video/*` handlers.
  - User loss stated: Android-only players (MX Player, Just Player, X Player, Web Video Caster) are not offered unless they register a desktop handler.

### T-48a-054 — Android device gates (PiP support, fullscreen SDK gate): drop
- Priority: P3
- Effort: S
- Depends: T-48a-017, T-48a-014
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt
- Acceptance:
  - `deviceHasPip()` and `Build.VERSION.SDK_INT` gates are replaced by runtime capability probes (T-48a-017 for PiP; fullscreen always enabled).
  - No code path reads `Build` on desktop.

### T-48a-055 — English string table for this area
- Priority: P1
- Effort: S
- Depends: T-48a-010
- Sources: i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, i18n/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - Every key used by this area has an English value in one Rust table (`src/i18n_player.rs`), copied from the base XML files; no key is invented.
  - Format placeholders (`%s`, `%d`, `%1$s`) are filled by Rust formatting; `\"` escapes are unescaped.
  - A test fails if a key used in code is missing from the table.

### T-48a-056 — Unit and integration tests for this area
- Priority: P0
- Effort: M
- Depends: T-48a-001, T-48a-030, T-48a-040, T-48a-042, T-48a-043, T-48a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/codeeditor/SyntaxHighlight.kt
- Acceptance:
  - Tests from `p1-edge-cases-and-tests.md` section 4 items 1-14 are implemented and pass in CI (`cargo test`).
  - Fixtures from section 4 exist under `tests/fixtures/`.
  - Manual checklist items 15-22 are recorded in the release checklist.

### T-48a-057 — Settings search index entries for player pages
- Priority: P1
- Effort: S
- Depends: T-48a-010, area:47b-settings-widgets-search-more
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsMainScreen.kt
- Acceptance:
  - Every row title on the nine player pages is indexed with its page path (for example "Player > Gestures > Double tap (left)").
  - Searching "subtitle" returns the Subtitles page rows; searching "mpv" returns the Advanced page rows.

### T-48a-058 — Dynamic (Monet) theme replacement
- Priority: P3
- Effort: S
- Depends: T-48a-004
- Sources: app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - Decision recorded: Android wallpaper-based dynamic colours are not reproduced. The "Dynamic" option is shown on desktop as "System colours" and follows the GTK theme (portal accent colour if available).
  - Stored `MONET` values are read as "System colours" (E-UP-04).

---

## Open decisions (recorded here, resolved by the product owner before implementation)

- D-01: Show disabled settings greyed (proposed) or hidden like Android. Default proposed: greyed.
- D-02: Auto/Auto-safe mapping fix (E-AU-01). Proposed: fix, mention in release notes.
- D-03: Language codes accepted: ISO 639-1 only, or ISO 639-1 and ISO 639-3. Proposed: both.
- D-04: Custom button Lua isolation with per-button pcall (E-CB-01). Proposed: yes.
- D-05: Remember volume switch (E-PS-01). Proposed: remove (T-48a-052).
- D-06: Deleting the favourite button promotes nothing (current behaviour). Proposed: keep; optional promote option later.
- D-07: Custom button create dialog closes before the write result is known (Kotlin). Proposed: keep the dialog open until success and show the error inline.
- D-08: Script rename keeps the extension (E-ED-08). Proposed: keep extension.
