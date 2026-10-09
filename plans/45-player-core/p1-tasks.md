# Area 45 (player core, part 1): implementation tasks

Task IDs use the prefix `T-45a-`. Every scope file in `p1-inventory.md` maps to at least one task below. Dependency tokens `area:45-player-core` mean the part-2 document (PlayerViewModel, loaders, settings screens, AniSkip, ChapterUtils, TrackSelect, player controls) owned by the same area.

Global conventions for every task:
- Rust target crate: `kurobayomi` (single crate, see `p1-rust-design.md`).
- All mpv calls go through the `MpvHandle` wrapper described in `p1-rust-design.md`. No task calls libmpv directly from UI code.
- Every "Acceptance" line is checkable by an automated test or a manual scripted check listed in `p1-edge-cases-and-tests.md`.

### T-45a-001 — Module skeleton and player UI state model
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `src/player/mod.rs`, `src/player/state.rs`, `src/player/enums.rs` exist and compile with `cargo check`.
  - `enums.rs` defines `SetAsArt{Success,AddToLibraryFirst,Error}`, `ArtType{Cover,Background,Thumbnail}`, `VideoAspect{Crop,Fit,Stretch}`, `SingleActionGesture{None,Seek,PlayPause,Switch,Custom}`, `Sheets{None,PlaybackSpeed,SubtitleTracks,AudioTracks,QualityTracks,Chapters,More,Screenshot}`, `Panels{None,SubtitleSettings,SubtitleDelay,AudioDelay,VideoFilters}`, and `Dialogs{None,EpisodeList,IntegerPicker{...}}` with the variants named exactly as in PlayerEnums.kt.
  - A unit test asserts `Sheets` and `Panels` have the variant counts 8 and 5.

### T-45a-002 — libmpv handle and GL render-context embedding
- Priority: P0
- Effort: L
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Opening a local MP4 in the player window renders video frames inside a `gtk::GLArea` using the libmpv render API (`mpv_render_context`, OpenGL backend).
  - Destroying the window calls `mpv_render_context_free` before `mpv_destroy`; no crash on 20 consecutive open/close cycles.
  - Playback starts paused (`pause=yes` set before first `loadfile`), matching `AniyomiMPVView.kt:116`.

### T-45a-003 — mpv startup option table (initOptions)
- Priority: P0
- Effort: M
- Depends: T-45a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - Before handle initialisation the Rust code sets, in this order: `vo` (`gpu-next` if the `pref_gpu_next` preference is true, else `gpu`), `pause=yes`, `profile=fast`, `hwdec`, debanding, `format=yuv420p` when `use_yuv420p` is true, `msg-level`, `keep-open=yes`, `input-default-bindings=yes`, `ytdl=no`, `tls-verify=yes`, `tls-ca-file`, cache limits, `screenshot-directory`, filters, `speed`, `vd-lavc-film-grain=cpu`, subtitle options, audio options (see T-45a-004, T-45a-005, T-45a-030, T-45a-031, T-45a-033).
  - A test asserts the generated option list for default preferences equals the golden list in `p1-data-and-config.md` section 4.

### T-45a-004 — Decoder selection, debanding and video-output options
- Priority: P0
- Effort: M
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - `Decoder` enum (`AutoCopy`/`Auto`/`SW`/`HW`/`HWPlus` with values `auto-copy`, `auto`, `no`, `mediacodec-copy`, `mediacodec`) and `getDecoderFromValue` are ported; on Linux the hwdec value is chosen from `hwdec` candidates `vaapi-copy`, `nvdec-copy`, `auto-copy` (see `p1-rust-design.md` section 6); a mismatch falls back to `auto`.
  - `Debanding::CPU` writes `vf=gradfun=radius=12`; `Debanding::GPU` writes `deband=yes`; `None` writes nothing.
  - `use_yuv420p=true` writes `vf=format=yuv420p`. Note that Kotlin overwrites `vf` when both debanding CPU and yuv420p are set (see behavior doc, section 2.2); the Rust port must join both filters with a comma instead.

### T-45a-005 — Demuxer cache, TLS, log level and fixed mpv flags
- Priority: P0
- Effort: S
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `demuxer-max-bytes` and `demuxer-max-back-bytes` are both `67108864` (64 MiB). The Android 32 MiB branch is not used on Linux.
  - `msg-level` is `all=v` when `verbose_logging` is true, else `all=warn`.
  - `tls-ca-file` points at `<data_dir>/mpv/cacert.pem`.

### T-45a-006 — Property observation table and typed event dispatch
- Priority: P0
- Effort: M
- Depends: T-45a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerObserver.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - All 34 entries of `observedProps` (AniyomiMPVView.kt:210-252) are observed with the same format class (INT64, NONE, STRING, DOUBLE, FLAG).
  - Each observed value arrives as one of five typed events (`Property(name)`, `PropertyInt(name, i64)`, `PropertyFlag(name, bool)`, `PropertyStr(name, String)`, `PropertyDouble(name, f64)`) mirroring `PlayerObserver.eventProperty` overloads.
  - A unit test with a fake `MpvHandle` asserts that observing `user-data/aniyomi/pause` with value `"pause"` routes to the bridge handler (T-45a-020).

### T-45a-007 — Event marshalling to the GTK main loop
- Priority: P0
- Effort: M
- Depends: T-45a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerObserver.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - mpv events are pushed through a `glib::MainContext::channel` (or `async_channel`) and handled on the GTK main thread; no widget is touched from the mpv event thread (verified by a debug assertion `gtk::is_initialized_main_thread()`).
  - Events are not dropped while the player is exiting; they are ignored after `exiting=true` (see T-45a-039).

### T-45a-008 — mpv log observer and level mapping
- Priority: P1
- Effort: S
- Depends: T-45a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerObserver.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - mpv `FATAL` and `ERROR` log levels map to the `error` log level, `WARN` to `warn`, `INFO` to `info`, everything else to `debug`, in the Rust logger with the target `mpv/<prefix>`.
  - Any message containing the substring `HTTP error` is stored as the "last HTTP error" for T-45a-009.

### T-45a-009 — Playback error reporting (efEvent)
- Priority: P0
- Effort: S
- Depends: T-45a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerObserver.kt
- Acceptance:
  - An mpv end-file error with no message produces the text `Error: File ended`.
  - If a stored HTTP error exists it is appended as `: <text>` and cleared; the toast is shown with the long duration (`Toast.LENGTH_LONG`; Proposal: 5 s on GTK).

### T-45a-010 — Local, content, data and protocol URI resolution
- Priority: P0
- Effort: S
- Depends: T-45a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerUtils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `file://` URIs resolve to their path; `data:` URIs become `data://<schemeSpecificPart>`; any scheme in the mpv protocol list (`Utils.PROTOCOLS`, see data doc) passes through as the full string; other schemes log `unknown scheme: <scheme>` and resolve to null.
  - The `content:` scheme has no Linux equivalent; it is rejected with the error text `unknown scheme: content` (see T-45a-068 for the on-disk replacement).
  - A `parseVideoUrl(null)` call returns null without panic.

### T-45a-011 — loadfile with start position and per-video options
- Priority: P0
- Effort: M
- Depends: T-45a-010, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Playing a video issues `loadfile <resolved> replace 0 <options>` where options is `key="value"` pairs joined by `,` from `Video.mpvArgs` (exact quoting in the behavior doc, section 3.4).
  - Loading a second video replaces the first without leaving a second playlist entry.
  - A null `Video` argument is a no-op (no command sent).

### T-45a-012 — Resume start position rule
- Priority: P0
- Effort: S
- Depends: T-45a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - When loading a new episode (`isLoadingEpisode == true`) and no explicit position is passed: `seen && !preserve_watching_position` gives start 0; otherwise `last_second_seen` (milliseconds) is used. The mpv command is `set start <ms/1000 as f32>`.
  - When the same episode is reloaded, the current `time-pos` is used as start.
  - Unit tests cover the four combinations of seen and preserve flags.

### T-45a-013 — Per-video mpv arguments from file metadata
- Priority: P1
- Effort: S
- Depends: T-45a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - For a local (`file`, `content`, `data`) video, after `FILE_LOADED` the Rust code reads the mpv `metadata` property, parses it as a JSON string-to-string map, and applies `ANIYOMI_MPV_ARGS` entries (`;`-separated `option=value`, value may contain `=`).
  - Parsing errors are logged at error level with the message `Failed to read video metadata` and do not abort playback.

### T-45a-014 — HTTP header passthrough for online playback
- Priority: P0
- Effort: S
- Depends: T-45a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - For online episodes the `http-header-fields` mpv option is set to `Key: value` pairs joined by `,`, with every `,` inside a value escaped as `\,`.
  - Headers come from the video's own headers if present, else from the source headers; duplicate keys keep the first value.
  - Offline and local episodes never set this option.

### T-45a-015 — Track selection (aid, sid, secondary-sid)
- Priority: P0
- Effort: M
- Depends: T-45a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Reading a track property returns `-1` when the value is `no`, not an integer, or missing; writing `-1` sets the property to `no`; writing any other integer sets it as an int.
  - A change event whose value is `auto` is ignored (no selection update), as in `trackId` returning null in `AniyomiMPVView`/`PlayerActivity.kt:723-729`.
  - Changing the audio track in the UI issues `aid <id>` and the track change event updates the selected id.

### T-45a-016 — External audio and subtitle track injection
- Priority: P1
- Effort: S
- Depends: T-45a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - For each `Video.audioTracks` entry the command `audio-add <url> auto <lang>` is issued; for each `subtitleTracks` entry `sub-add <url> auto <lang>`.
  - When both lists are empty, `onFinishLoadingTracks` is invoked directly (see T-45a-084).

### T-45a-017 — Chapters and skip-timestamp injection
- Priority: P1
- Effort: M
- Depends: T-45a-011, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - A `TimeStamp` with an empty `name` and a `type` other than `Other` gets the localised name of its `ChapterType` (`Opening`, `Ending`, `Recap`, `MixedOp`); otherwise the name is kept.
  - The merged list is handed to the chapter model (part 2) and `setChapter(pos)` is called afterwards.
  - An empty timestamp list leaves the chapter list untouched.

### T-45a-018 — Media title and episode-title properties
- Priority: P0
- Effort: S
- Depends: T-45a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `user-data/current-anime/episode-title` is set to the episode name.
  - `force-media-title` is `"<anime title> - E<NN> - <episode name>"` where NN is the episode number left-padded to 2 characters and printed without `.0` when integral (e.g. `1.5` stays `1.5`, `12.0` prints `12`).
  - `user-data/current-anime/episode-number` is set to the number as a double (see part 2 usage).

### T-45a-019 — Transport commands and seek/pause events
- Priority: P0
- Effort: M
- Depends: T-45a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/assets/aniyomi.lua
- Acceptance:
  - The bridge forwards `pause`, `unpause` and `pauseunpause` to the part-2 view-model methods `pause()`, `unpause()`, `pauseUnpause()`; the `pause` flag event updates the UI and the keep-awake state (T-45a-041).
  - `MPV_EVENT_SEEK` sets the loading flag true; `paused-for-cache` and `seeking` flags mirror into the loading flag.
  - `seek_by` and `seek_to` forward their numeric value to the part-2 seek handlers; the mpv seek command itself is issued there.

### T-45a-020 — user-data/aniyomi protocol bridge in Rust
- Priority: P1
- Effort: M
- Depends: T-45a-019
- Sources: app/src/main/assets/aniyomi.lua, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Every `user-data/aniyomi/*` property in the protocol table (behavior doc, section 5) is handled by a Rust match arm that performs the same action as the Kotlin view-model method it names.
  - Writing a value to a `user-data/aniyomi/*` property and reading it back does not leave the property set (the handler resets it to an empty string after dispatch, matching repeated use of the same value).
  - An unknown property under this prefix is logged at debug level and ignored.
  - Proposal: the reset-to-empty step replaces the Kotlin behaviour of leaving the last value in place, which makes repeated identical commands invisible to observers.

### T-45a-021 — Custom buttons compiled to mpv Lua
- Priority: P2
- Effort: M
- Depends: T-45a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt
- Acceptance:
  - The generated `custombuttons.lua` contains, for each button, `function button<id>()`, `function button<id>long()`, and `mp.register_script_message('call_button_<id>', ...)` / `..._long` registrations, with `$id` and `$isPrimary` substituted.
  - The file is written under `<data_dir>/mpv/scripts/` and loaded with `load-script <path>`.
  - A button with empty content produces an empty function body and no runtime error.

### T-45a-022 — Integer picker protocol
- Priority: P1
- Effort: S
- Depends: T-45a-020
- Sources: app/src/main/assets/aniyomi.lua, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - The Lua call `aniyomi.int_picker(title, name_format, start, stop, step, property)` produces a `Dialogs::IntegerPicker` with those values; `name_format` is passed through unchanged to the picker (its format semantics are defined in the part-2 dialog code).
  - Changing the picker value writes the integer to the named mpv `property`.

### T-45a-023 — OSD text, UI toggles and panel protocol
- Priority: P1
- Effort: S
- Depends: T-45a-020
- Sources: app/src/main/assets/aniyomi.lua
- Acceptance:
  - `show_text` shows the given text as an OSD message (display duration is decided by the part-2 view model; this file does not set one); `toggle_ui` accepts `show|hide|toggle`; `show_panel` accepts `subtitle_settings|subtitle_delay|audio_delay|video_filters`; `software_keyboard` is accepted and ignored on desktop (logged as unsupported).

### T-45a-024 — Keyboard input mapped to mpv key names
- Priority: P0
- Effort: M
- Depends: T-45a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - GTK key presses are translated to mpv key names (table in `p1-rust-design.md` section 8) with modifiers prefixed in the order `shift+`, `ctrl+`, `alt+`, `meta+` (Super maps to `meta`).
  - Auto-repeat presses (repeat count > 0) are swallowed without sending `keydown`.
  - Printing keys without a table entry send the Unicode character itself; dead keys send nothing.
  - Key press sends `keydown <name>` and key release sends `keyup <name>` as `mpv command` strings.

### T-45a-025 — Window-level key bindings and media-key gesture codes
- Priority: P0
- Effort: S
- Depends: T-45a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - Up and Down arrow keys change volume by +1 / -1 and show the volume slider (Android volume keys have no direct equivalent: Up and Down are bound here, PgUp/PgDn are not bound in the Android code).
  - Left/Right arrow (D-pad) run the left/right double-tap actions; Space toggles pause; Media Stop closes the window; Media Rewind and Fast-Forward run the left and right double-tap actions.
  - `CustomKeyCodes` values `0x10001`..`0x10006` are sent via `keypress` for Custom gestures.
  - Every other key is forwarded to T-45a-024 and the event is consumed.

### T-45a-026 — mpv.conf and input.conf generation
- Priority: P0
- Effort: S
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Every time a player window opens, `<data_dir>/mpv/mpv.conf` and `<data_dir>/mpv/input.conf` are rewritten from the `pref_mpv_conf` and `pref_mpv_input` preferences (empty string produces an empty file).
  - Writes are truncating (`create` then write); no append.

### T-45a-027 — User scripts, script-opts and shaders import
- Priority: P2
- Effort: S
- Depends: T-45a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - When `mpv_scripts` is true, every file in the user scripts, script-opts and shaders directories is copied into `<data_dir>/mpv/scripts`, `script-opts`, `shaders` after those three directories are deleted.
  - When `mpv_scripts` is false the three directories are still emptied (deleted) and nothing is copied.

### T-45a-028 — Bundled subfont.ttf and cacert.pem
- Priority: P0
- Effort: S
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `subfont.ttf` and `cacert.pem` are installed into `<data_dir>/mpv/` at startup.
  - An existing file of the same length is not rewritten; a copy error logs `Failed to copy asset file: <name>` and continues.

### T-45a-029 — Fonts directory sync
- Priority: P1
- Effort: S
- Depends: T-45a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Every file in the user fonts directory is copied into `<data_dir>/mpv/fonts/`, then `sub-fonts-dir` and `osd-fonts-dir` are set to that directory.
  - The copy runs on a background thread and the properties are set only after the copy completes.
  - Linux improvement: the fonts directory may be used directly (no copy) because mpv reads it from disk; the copy is then removed from the plan.

### T-45a-030 — Subtitle option mapping and styling
- Priority: P0
- Effort: M
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - The 22 subtitle options in `setupSubtitlesOptions` are set with the exact option names and value formats in `p1-data-and-config.md` section 3.3.
  - Colors are written as `#AARRGGBB`-style hex through the same conversion as `toColorHexString` (see data doc).
  - `sub-ass-force-margins=yes` and `sub-use-margins=yes` are set (PlayerActivity.kt:438-439).

### T-45a-031 — Audio option mapping
- Priority: P0
- Effort: S
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - `alang`, `audio-delay` (ms/1000), `audio-pitch-correction`, `volume-max` (boost cap + 100) are set.
  - Channel layout selection writes the property and value from `AudioChannels` (`audio-channels=auto|mono|stereo` or `af=pan=[stereo|c0=c1|c1=c0]`).

### T-45a-032 — Video filters
- Priority: P1
- Effort: S
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - `VideoFilters` entries `brightness`, `saturation`, `contrast`, `gamma`, `hue` are set at startup from the stored integer preferences and can be changed at runtime with the same property names.
  - Proposal: clamp values to -100..100 before writing. The Kotlin does not clamp in this file; out-of-range stored values are passed through as-is.

### T-45a-033 — Playback speed and speed presets
- Priority: P0
- Effort: S
- Depends: T-45a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - The initial `speed` option is the stored `pref_player_speed` value (default 1.0).
  - Speed change events update the displayed speed; a `speed` double event sets the playback-speed state.

### T-45a-034 — Aspect modes and video-out geometry helpers
- Priority: P0
- Effort: S
- Depends: T-45a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - The `VideoAspect` value is applied by `changeVideoAspect` in the part-2 view model (area:45-player-core); this file only exposes the property write hooks. Exact property values are in the part-2 document, not restated here.
  - `getVideoOutAspect()` returns 0 when `video-params/aspect` is below 0.001, and inverts the aspect when `video-params/rotate % 180 == 90`.

### T-45a-035 — Android orientation policy replaced by video-rotate
- Priority: P3
- Effort: S
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Decision: drop the Android `PlayerOrientation` enum on desktop (window orientation does not exist). Replacement: a `video-rotate` control (0, 90, 180, 270) in the Rust settings; the stored value `pref_default_player_orientation_type_key` is ignored.
  - User loses: automatic portrait/landscape lock on phones. Documented in `p1-ui-parity.md`.
  - Note for the record: the Kotlin maps `ReverseLandscape` to `SCREEN_ORIENTATION_REVERSE_PORTRAIT` (PlayerActivity.kt:849), which is a bug; it is not carried over.

### T-45a-036 — Episode switching and autoplay on EOF
- Priority: P0
- Effort: M
- Depends: T-45a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Switching episodes pauses, clears the open sheet and panel, resets state, sets loading, then loads the episode; prev/next flags are recalculated from the playlist index.
  - `eof-reached=true` with `pref_auto_play_enabled=true` starts the next episode with `autoPlay=true`; with the flag false nothing happens.

### T-45a-037 — "Next episode not found" and empty-hoster messages
- Priority: P0
- Effort: S
- Depends: T-45a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - When no next episode exists and autoplay is false, the toast `Next Episode not found!` (key `no_next_episode`) is shown; when autoplay is true it is suppressed.
  - An empty hoster list during switching shows `Hoster list is empty` (key `no_hosters`) and closes the window through the initial-error path (T-45a-052).

### T-45a-038 — Single-instance player window and newIntent handling
- Priority: P0
- Effort: M
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Only one player window exists at a time; opening an episode while the window is open reuses it (equivalent of `launchMode=singleTask` plus `onNewIntent`).
  - Missing `animeId` or `episodeId` (value -1) closes the window.
  - The previous episode's progress is saved before loading the new one.

### T-45a-039 — Lifecycle, isExiting gate and shutdown order
- Priority: P0
- Effort: M
- Depends: T-45a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - `isExiting` is set true in `onPause` unless the window is entering picture-in-picture, and cleared in `onResume` (PLAYBACK_RESTART cannot clear it because the event gate returns first, see behavior doc 6.4); while true, observer events and MPV commands from `executeMPVCommand` are dropped.
  - Shutdown order: stop HTTP server, release audio focus (T-45a-042), release media session (T-45a-043), remove observers, destroy mpv render context then handle.

### T-45a-040 — Progress persistence on pause and exit
- Priority: P0
- Effort: M
- Depends: T-45a-039, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Leaving the window (close) calls `deletePendingEpisodes` and sends `stop` to mpv; a plain pause calls `pause` on the view model and saves watching progress first.
  - Entering picture-in-picture does not stop playback or delete pending episodes.

### T-45a-041 — Keep-awake while playing
- Priority: P1
- Effort: S
- Depends: T-45a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - While `pause=false` the window holds an idle inhibition (`gtk::Application::inhibit` with `IDLE` flags, reason `Playing video`); on pause or close it is released.
  - The inhibit call is made on the main thread and its cookie is kept in the window state.

### T-45a-042 — Audio focus policy
- Priority: P3
- Effort: S
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Decision: drop Android AudioFocus on desktop. Linux has no global focus API; PipeWire/PulseAudio do not ask players to duck.
  - User loses: automatic pause when a phone call or another app takes focus. Documented in the parity doc.
  - The volume-duck behaviour (`multiply volume 0.5`, restore `2`) is not carried over.

### T-45a-043 — MPRIS media controls (replacement for MediaSession)
- Priority: P1
- Effort: M
- Depends: T-45a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - An MPRIS `org.mpris.MediaPlayer2.Player` service on the session bus exposes Play, Pause, Next, Previous with the same action mapping as `setupMediaSession` (`SingleActionGesture` per button: None, Seek, PlayPause, Switch, Custom).
  - `Seek` for Previous calls the left seek and for Next the right seek of the part-2 view model (amount from `pref_skip_length_preference`, default 10); `Switch` changes episode; `Custom` sends the keycode `0x10004` (previous), `0x10005` (play) or `0x10006` (next) through the bridge.
  - The playback status is `Playing` or `Paused`, and `Metadata` carries the anime title and episode title.

### T-45a-044 — Fullscreen and immersive mode
- Priority: P0
- Effort: S
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Opening the player window makes it fullscreen when `player_fullscreen` is true (default); the header bar and sidebar are hidden while playing.
  - Escape and the on-screen button leave fullscreen without closing the window.

### T-45a-045 — Brightness memory and control (dropped on desktop)
- Priority: P3
- Effort: S
- Depends: T-45a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Decision: drop screen brightness control. Desktop monitors have no standard API for this; `brightnessctl` or DDC/CI are out of scope.
  - Replacement: the video-filter `brightness` (T-45a-032) is the only brightness control.
  - User loses: panel backlight control from the player. Documented in the parity doc.

### T-45a-046 — Volume memory and volume keys
- Priority: P1
- Effort: S
- Depends: T-45a-025
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Volume is mpv's own `volume` property (0 to `volume-max`); up and down change it by 1 step.
  - On resume after the window was hidden (`isExiting` was true), the Kotlin sets mpv volume to 100 when the system music-stream volume is below its maximum (`PlayerActivity.kt:634-648`). `pref_remember_volume` / `player_volume_value` are read by part 2; this task keeps the resume rule only.

### T-45a-047 — Picture-in-picture policy (dropped as Android PiP)
- Priority: P3
- Effort: S
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt
- Acceptance:
  - Decision: drop Android picture-in-picture (`enterPictureInPictureMode`, `PictureInPictureParams`, `setAutoEnterEnabled`) because GTK4 has no PiP API and Wayland has no always-on-top protocol.
  - Replacement: T-45a-048 mini-player window. Settings `pref_enable_pip`, `pref_pip_on_exit`, `pref_pip_episode_toasts`, `pip_replace_with_previous` are kept with a "mini player" label.

### T-45a-048 — Mini-player window with transport controls
- Priority: P2
- Effort: M
- Depends: T-45a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - A second, small `gtk::Window` with the same `GLArea` renders the video; it has four buttons in this order: previous (or skip -10 s when `pip_replace_with_previous` is false), play/pause (icon swaps), next (disabled at last episode), and close.
  - Previous is disabled when the index is 0; Next is disabled when the index equals the count minus 1.
  - A `pip_control` action with the same codes 1 (pause), 2 (play), 3 (previous), 4 (next), 5 (skip) is dispatched through the same Rust channel as MPRIS.

### T-45a-049 — Screenshot result messages
- Priority: P1
- Effort: S
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - A successful screenshot shows the toast `Picture saved` (key `picture_saved`).
  - A failed screenshot logs the error at error level and shows no toast.

### T-45a-050 — Share screenshot
- Priority: P2
- Effort: S
- Depends: T-45a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Sharing opens the desktop share path (`gtk::FileLauncher` or the XDG Desktop Portal `OpenURI`/`Email` share) with the message `"<anime title>: <episode name>, <seconds>"` (key `share_screenshot_info`).
  - The chooser title is `Share` (key `action_share`).

### T-45a-051 — Set-as-art result messages
- Priority: P2
- Effort: S
- Depends: T-45a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - Success messages: `Cover updated` (Cover), `Background updated` (Background), `Thumbnail updated` (Thumbnail).
  - Failure messages: `Please add the entry to your library before doing this` (AddToLibraryFirst) and `Failed to update cover` (Error).

### T-45a-052 — Initial episode load error path
- Priority: P0
- Effort: S
- Depends: T-45a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - An init failure with a `ExceptionWithStringResource` shows its localised message; any other error shows its raw message (or `Unknown error` when null), then the window closes.
  - The error is logged at error level before closing.

### T-45a-053 — Stats overlay page on startup
- Priority: P2
- Effort: S
- Depends: T-45a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - When `pref_player_statistics_page` is not 0, the `stats/display-stats-toggle` and `stats/display-page-<n>` script bindings run after init.

### T-45a-054 — Uncaught exception handler (dropped)
- Priority: P3
- Effort: S
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - Decision: do not install a process-wide uncaught-exception handler that closes the window (Android `Thread.setDefaultUncaughtExceptionHandler`).
  - Replacement: GTK error dialog for handled errors; panics in worker threads are logged through the global logger.

### T-45a-055 — External player command builder (Linux)
- Priority: P1
- Effort: M
- Depends: T-45a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - For an empty player preference the Rust code opens the file with the desktop default (`gio open` / `xdg-open` equivalent via `gio::AppInfo::launch_default_for_uri`).
  - For a named preset the program and argv come from T-45a-056; the command is spawned detached (`std::process::Command::spawn`, stdio to null, process group new).
  - If the binary is not on `PATH` the toast `Video URL is empty.` is not used; instead the error `Player not found: <program>` is shown.

### T-45a-056 — External player presets (Linux)
- Priority: P1
- Effort: S
- Depends: T-45a-055
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Presets (Linux): `mpv` (binary `mpv`), `vlc` (binary `vlc`), `celluloid` (binary `celluloid`), `custom` (user command template with `{url}`, `{title}`, `{position}` placeholders).
  - Android package constants (`is.xyz.mpv`, `com.mxtech.videoplayer`, `org.videolan.vlc`, `live.mehiz.mpvkt`, `com.brouken.player`, `dev.anilbeesetti.nextplayer`, `video.player.videoplayer`) are kept only as a mapping table in the data doc, not used for launching.

### T-45a-057 — External player extras: title, position, headers, subtitles
- Priority: P1
- Effort: M
- Depends: T-45a-055
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - The title passed to the external player is `"<anime.title> - <episode.name>"`.
  - The start position is the value from T-45a-062 in milliseconds (mpv `--start=<seconds>` form when the preset is mpv).
  - Headers are passed as `--http-header-fields=` joined with `,` for mpv and VLC; the value is `Key: value` pairs.

### T-45a-058 — Subtitle selection by locale for external players
- Priority: P1
- Effort: S
- Depends: T-45a-057
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - The subtitle whose language contains the system's simple locale display name (case-insensitive) is selected; if none matches, the first subtitle track is used; if there are no subtitles nothing is passed.
  - The selected subtitle URL is passed as `--sub-file=` for mpv and as a separate argument for VLC.

### T-45a-059 — MIME type guess for external players
- Priority: P2
- Effort: S
- Depends: T-45a-055
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Extension mapping: `mp4` gives `video/mp4`, `mkv` gives `video/x-matroska`, `m3u8` gives `application/x-mpegURL`, anything else `video/any`. Used only when the desktop opener needs a content type.

### T-45a-060 — Web Video Caster intent (dropped)
- Priority: P3
- Effort: S
- Depends: T-45a-055
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Decision: drop the Web Video Caster package path (`com.instantbits.cast.webvideo`) and its `subtitle` and `HTTP_HEADERS` extras on Linux.
  - User loses: casting from the Android Cast flow. No Linux equivalent is planned.

### T-45a-061 — External player return and progress feedback
- Priority: P1
- Effort: M
- Depends: T-45a-055, T-45a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Desktop external players do not return results. For mpv the Rust code starts mpv with `--input-ipc-server=<socket>` and polls `time-pos` and `duration` every 5 seconds; when the process exits the last values are saved through T-45a-063 with cause `playback_completion` when the position is within 1 second of the duration.
  - For other players progress is not recorded (documented loss).

### T-45a-062 — External start position rule
- Priority: P1
- Effort: S
- Depends: T-45a-061
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Start is 1 ms when the episode is seen and (preserve is off, or the last position equals the total); otherwise `lastSecondSeen`.
  - Unit tests cover the truth table with the preserve flag on and off.

### T-45a-063 — History and progress writes with incognito gate
- Priority: P0
- Effort: M
- Depends: area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - When `incognito_mode` is true no history row and no episode progress is written.
  - Otherwise `UpsertAnimeHistory` gets the current time (`Date()`), and `UpdateEpisode` gets `seen`, `bookmark`, `fillermark`, `lastSecondSeen`, `totalSeconds`.
  - `totalSeconds <= 0` skips progress writes entirely.

### T-45a-064 — Completion threshold for marking seen
- Priority: P0
- Effort: S
- Depends: T-45a-063
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - An unseen episode becomes seen when `lastSecondSeen >= totalSeconds * pref_progress_preference` (default 0.85).
  - An already-seen episode stays seen (the write sets `seen=true`).

### T-45a-065 — Delete seen episodes after watching
- Priority: P1
- Effort: M
- Depends: T-45a-063, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - When `remove_after_read_slots` is not -1, the episode at index `current_index - slots` in the list sorted by `anime.sorting` is enqueued for deletion, only if it is seen and exists. With slots = 0 this is the current episode itself (Kotlin behaviour, kept and documented).
  - Unknown sorting values throw `NotImplementedError` in Kotlin; the Rust port returns an error and skips deletion.

### T-45a-066 — Tracker update on seen
- Priority: P1
- Effort: M
- Depends: T-45a-063, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - When `pref_auto_update_manga_sync_key` is true and the episode is seen, every logged-in anime tracker whose `lastEpisodeSeen` is lower than the episode number gets an update request.
  - Failures are logged at info level and never block playback (the Kotlin runs each update in `runCatching` inside `async` and logs `exceptionOrNull`).

### T-45a-067 — Offline tracker delay queue
- Priority: P1
- Effort: S
- Depends: T-45a-066
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - When offline the update is stored in the delayed-tracking store (`addAnime(animeId, lastEpisodeSeen)`) and the delayed-update job is scheduled; the job replays it when online.

### T-45a-068 — Local file and on-device detection
- Priority: P0
- Effort: S
- Depends: T-45a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - A video whose source is the local source (`LocalAnimeSource.ID`) or whose episode is downloaded (`isEpisodeDownloaded` with `skipCache=true`) is played from its file path directly.
  - The Android `FileProvider` URI step is dropped; the path is passed as is.

### T-45a-069 — Local HTTP server lifecycle
- Priority: P1
- Effort: M
- Depends: area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt
- Acceptance:
  - A `LocalServer` owned by the Rust app binds `127.0.0.1` on an ephemeral port (0), exposes `url()` as `http://localhost:<port>`, and stops when dropped.
  - Starting a new server stops the previous one first (same as `HttpServerService.onStartCommand`).
  - Source lookup failing (no `AnimeHttpSource`) returns an error and does not start a server.

### T-45a-070 — Local HTTP server start from playback and external launch
- Priority: P1
- Effort: S
- Depends: T-45a-069
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - A video that `usesHttpServer()` (URL matches `http://localhost:1` with no digit after) is rewritten to the bound port before loading.
  - Failure shows the toast `Failed to start http server` (key `http_server_start_failure`) and aborts the load.

### T-45a-071 — Local HTTP server status notification
- Priority: P2
- Effort: S
- Depends: T-45a-069
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - While the server runs, a desktop notification `Http server is running` (key `http_server_is_running`) with a `Close` action (key `action_close`) is shown; the action stops the server.
  - Notification id and channel names are not reused (Android constants `-901` and `http_server_channel` are dropped).

### T-45a-072 — Torrent server lifecycle
- Priority: P2
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt
- Acceptance:
  - Start: call `echo`; if empty, start the TorrServer library on the configured port (`pref_torrserver_port`, default `8090`) with the data directory, proxy mode and proxy URL, then wait up to 10 s for `echo` to answer, then push trackers.
  - Stop: stop the server, clear the notification, end the service.

### T-45a-073 — Torrent server API client
- Priority: P2
- Effort: M
- Depends: T-45a-072
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerApi.kt
- Acceptance:
  - `GET /echo` returns the body text or an empty string on any error.
  - `POST /torrents` with JSON `{"action":"add", ...}` returns a `Torrent`; `POST /torrent/upload` sends multipart fields `file`, `save`, `title` with `application/x-bittorrent`.

### T-45a-074 — Torrent upload and magnet index parsing
- Priority: P2
- Effort: M
- Depends: T-45a-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - A `content://` source is uploaded (file read on the Rust side from the local path) and played at index 0.
  - A `magnet:` URL with `index=<n>` parses `n` as the file index; a non-integer falls back to 0.

### T-45a-075 — Torrent play links
- Priority: P2
- Effort: S
- Depends: T-45a-073
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt
- Acceptance:
  - The link is `http://127.0.0.1:<port>/stream/<urlencoded name>?link=<hash>&index=<index>&play`, where name is the file's basename (or the torrent title when the file is not found), URL-encoded with UTF-8.

### T-45a-076 — Torrent trackers default list
- Priority: P2
- Effort: S
- Depends: T-45a-072
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt, core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt
- Acceptance:
  - The default list of 26 tracker URLs (data doc section 5) is stored under `pref_torrserver_tackers` (spelling kept).
  - At startup the stored list is split on newline and joined with `,\n` before being pushed.

### T-45a-077 — Torrent proxy modes
- Priority: P2
- Effort: S
- Depends: T-45a-072
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt
- Acceptance:
  - `ProxyMode` values `None`=`tracker`, `Tracker`=`tracker`, `Peers`=`peers`, `Full`=`full` are passed to the server; the proxy URL is sent only when mode is not `None`.

### T-45a-078 — Torrent playback routing in setVideo
- Priority: P2
- Effort: S
- Depends: T-45a-072, T-45a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - A video is routed to the torrent path when torrent support is enabled and its URL starts with the torrent host, starts with `magnet`, or ends with `torrent`.
  - Otherwise it is loaded normally.

### T-45a-079 — Torrent server status notification
- Priority: P2
- Effort: S
- Depends: T-45a-072
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt
- Acceptance:
  - While running, the notification `Torrent Server is running` (key `torrentserver_is_running`) with a `Stop` action is shown; closing it stops the server.
  - Its title is the app name.

### T-45a-080 — Service lifetime model (dropped Android foreground services)
- Priority: P3
- Effort: S
- Depends: T-45a-069, T-45a-072
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt, app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt
- Acceptance:
  - Decision: drop `Service`, `START_STICKY`, `startForeground` and `ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC`.
  - Replacement: servers are owned by the Rust app state and stop on app exit; a server shutdown on app exit is covered by T-45a-039.
  - User loses: servers surviving app exit. Documented in the parity doc.

### T-45a-081 — Android-only platform pieces (dropped)
- Priority: P3
- Effort: S
- Depends: T-45a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerUtils.kt
- Acceptance:
  - Dropped: `FileProvider` URIs, `UniFile`, `ParcelFileDescriptor` fd-to-path (`fd://`), the Samsung S-Pen `REMOTE_ACTION` manifest entry, `Environment.getExternalStoragePublicDirectory` screenshot dir (replaced by `XDG_PICTURES_DIR`), `Settings.System` brightness, and `ContentResolver` display names.
  - `getFileName` for `content:` URIs is replaced by the file's basename on disk.

### T-45a-082 — Audio becoming noisy: pause on output change
- Priority: P2
- Effort: S
- Depends: T-45a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - When the default PipeWire/PulseAudio sink disappears (unplugged headphones) mpv is paused and keep-awake is released.
  - Replacement for `ACTION_AUDIO_BECOMING_NOISY`, which is a phone-only broadcast.

### T-45a-083 — Volume and brightness restore on resume (folded into T-45a-046)
- Priority: P1
- Effort: S
- Depends: T-45a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - After the window is re-shown from a hidden state the volume is restored to the remembered value; no brightness step (T-45a-045).

### T-45a-084 — fileLoaded orchestration order
- Priority: P0
- Effort: M
- Depends: T-45a-011, T-45a-013, T-45a-015, T-45a-016, T-45a-017, T-45a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - On `FILE_LOADED` the steps run in this order: mpv options from metadata, media title, orientation (dropped, T-45a-035), chapters, tracks, then the skip-intro lookup (part 2).
  - Every step is skipped when `isExiting` is true.
  - A test asserts the step order with a recording fake.
