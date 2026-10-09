# 46-player-controls-ui (part 2 of 2): implementation tasks

Task IDs use the `T-46b-NNN` form. Every scope file in `p2-inventory.md` is covered by at least one task (inventory column "Task ids"). Behavior references are to `p2-behavior.md`, data to `p2-data-and-config.md`, design to `p2-rust-design.md`, UI to `p2-ui-parity.md`, tests to `p2-edge-cases-and-tests.md`.

Priority scale: P0 = needed for daily reading or watching (track selection, chapters, quality, speed, decoder, the host that holds them). P1 = parity features most users rely on (sleep timer, screenshot, subtitle style and delay). P2 = long tail. P3 = dropped or Android-only, with the replacement stated.

---

### T-46b-001 — In-player sheet and panel host (overlay container)
- Priority: P0
- Effort: L
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerPanels.kt
- Acceptance:
  - `SheetHost::open(kind)` shows exactly one sheet; opening another replaces it; `SheetKind` values are exactly None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot (behavior §0.3).
  - Scrim click and Escape each call `close()`; `on_close` runs exactly once per close, even when both fire in the same frame.
  - Sheet width is 420 px, or 720 px when the window is at least 900 px wide; max height is 95% of the player height; the content scrolls inside (p2-ui-parity §0, §15).
  - Panels (`PanelKind`) are right-aligned, 420 px wide, placed at 80% of available height (behavior §0.2).
  - Open and close use a 350 ms reveal; focus moves to the first control on open and returns to the opener on close.
  - Clicks inside the card do not reach the scrim (GestureClick claims the sequence).

### T-46b-002 — Shared controls (slider, numeric chooser, switch, expander, dropdown, chips)
- Priority: P0
- Effort: M
- Depends: T-46b-001
- Sources: app/src/main/java/eu/kanade/presentation/player/components/SliderItem.kt, app/src/main/java/eu/kanade/presentation/player/components/OutlinedNumericChooser.kt, app/src/main/java/eu/kanade/presentation/player/components/SwitchPreference.kt, app/src/main/java/eu/kanade/presentation/player/components/ExpandableCard.kt, app/src/main/java/eu/kanade/presentation/player/components/ExposedTextDropDownMenu.kt, app/src/main/java/eu/kanade/presentation/player/components/TintedSliderItem.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/ActionButton.kt
- Acceptance:
  - Integer slider writes only when the integer changes, and truncates the float position toward zero (behavior §0.4).
  - Float slider forwards raw values (no rounding) and supports `steps = 0` for continuous.
  - Numeric chooser does not clamp: values outside [min, max] stay as set and show "Value too big" or "Value too small" (behavior §0.5).
  - Typing a blank value writes 0; typing "-" writes 0; unparseable text writes nothing.
  - Integer step arithmetic saturates at `i64::MIN` / `i64::MAX` (edge E1).
  - Switch row toggles on a click anywhere on the row.

### T-46b-003 — Pure formatting and color helpers (number, color, track label)
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/PlaybackSpeedSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/GenericTracksSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsColorsCard.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/ChaptersSheet.kt
- Acceptance:
  - `to_fixed`, `preset_string`, `round3_half_even`, `format_elapsed`, `pretty_time`, `copy_argb`, `color_hex`, `parse_mpv_color` and `track_label` match behavior §1.4, §6.3, §13.9, §14.1 and the tests N1-N7, C1-C8, T1-T6 in p2-edge-cases-and-tests.md.
  - `color_hex(-1) == "#FFFFFFFF"`, `color_hex(0) == "#00000000"`.
  - `parse_mpv_color("red")` returns None (no panic).
  - `track_label(-1, "Off", None) == "Off"`.

### T-46b-004 — mpv property boundary for this part
- Priority: P0
- Effort: S
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsColorsCard.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsMiscellaneousCard.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/PlaybackSpeedSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt
- Acceptance:
  - `MpvProps` trait with typed get/set for f64, i64, bool, str plus `command`; implemented for the area:45-player-core wrapper and for a recording fake.
  - Every `get_*` returning None falls back to the stored preference, never to a panic (edge F1, behavior §13.2, §14.1).
  - Writes use the exact property names and value strings of p2-data-and-config.md §4 (for example `sub-color` written as `#AARRGGBB`, `sub-bold` as a bool).
  - `sub-ass-override` is written as a string (`force` / `scale`), not as a bool.
  - Errors are logged with `tracing::warn!` and never abort the UI update.

### T-46b-005 — Typed settings for keys used by this part
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AdvancedPlayerPreferences.kt
- Acceptance:
  - Every key and default in p2-data-and-config.md §3 round-trips through the `Store` trait (for example `sub_shadow_offset` has no `pref_` prefix, `pref_subtitles_font_size` is plural).
  - Enum preferences are stored by variant name (`AutoSafe`, `OutlineAndShadow`, `Auto`, …) and an unknown stored name returns the default.
  - `speedPresets` default set is the 12 values in behavior §6.1.
  - `reset_typography`, `reset_misc`, `reset_colors` produce exactly the writes in p2-edge-cases-and-tests.md SV2-SV4, with the `sub-ass-justify` fix (E26).

### T-46b-006 — Font family scan (TTF/OTF in mpv-config/fonts)
- Priority: P1
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt, domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - Only files whose lowercase name ends in `.ttf` or `.otf` are read (regex `.*\.[ot]tf$`); `.ttc` is skipped.
  - Family name comes from the name table (family id 1), Unicode record preferred; unreadable files are skipped silently.
  - Results are de-duplicated keeping first occurrence, and `Sans Serif` is not repeated (E30).
  - The scan runs on a worker thread; a result arriving after the panel closed is dropped (weak reference, race R5).
  - The fonts directory is created once at startup, not on every render (E29).
  - Test F1-F6 in p2-edge-cases-and-tests.md pass with the fixture `tests/fixtures/fonts/Sample-Regular.ttf`.

### T-46b-007 — Hoster model and grouping (QualitySheet logic)
- Priority: P0
- Effort: S
- Depends: area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - `HosterState` has four variants (Idle, Loading, Error, Ready) with the fields in p2-data-and-config.md §1.13.
  - `group_hosters` returns valid = Idle | Loading | Ready-with-videos; failed = Error; empty = Ready-without-videos, in declaration order (H1-H3).
  - The flat mode is used only when there is exactly one hoster named `no_hoster_list` and it is Ready.
  - `get_changed_at` keeps all other entries unchanged and never panics on an out-of-range index.
  - A video-state lookup for a missing index returns `Video.State::Queue` instead of panicking (edge E52).

### T-46b-008 — Tap-to-sync delay state machine
- Priority: P1
- Effort: M
- Depends: T-46b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt
- Acceptance:
  - `DelayMeasure::press` implements the rule "a button is enabled iff the running sign differs from its own start sign" (behavior §11.5, design §5.1).
  - Start signs: Audio A = Positive, Audio B = Negative, Subtitle A = Negative, Subtitle B = Positive.
  - Tests D1-D7 in p2-edge-cases-and-tests.md pass, including `Instant`-based elapsed with saturating subtraction.
  - The value at stop is `start ± elapsed_ms` (integer milliseconds), and the port writes it once through the single `on_change` path.
  - Closing the panel during a run applies the current value (E8 decision, flagged in the release notes).

### T-46b-009 — Audio channel, decoder and statistics-page logic
- Priority: P0
- Effort: S
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - `AudioChannels::mpv_write` maps Auto → (`audio-channels`, `auto-safe`), AutoSafe → (`audio-channels`, `auto`), Mono, Stereo, ReverseStereo → (`af`, `pan=[stereo|c0=c1|c1=c0]`) (tests A1-A6).
  - Labels are "Auto" for Auto and "Auto-safe" for AutoSafe (the swapped-name check).
  - `from_hwdec_current` returns None for unknown strings instead of failing (DC4); `"no"` → Sw; `"vaapi-copy"` → Hw.
  - `stats_actions` implements the xor truth table (tests S1-S5).
  - Decoder chips are AutoCopy, Sw, Hw, HwPlus; `Auto` is never shown (DC5).

### T-46b-010 — Generic track list sheet skeleton
- Priority: P0
- Effort: S
- Depends: T-46b-001, T-46b-002, T-46b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/GenericTracksSheet.kt
- Acceptance:
  - Layout is header, scrollable list, and a footer slot that is always present as the last list item (behavior §1.1).
  - Title row uses padding start 16, end 16, top 8, bottom 4 px; actions are right-aligned.
  - Add row is 48 px high; the whole row is clickable.
  - The modifier is applied once, not twice (E44).

### T-46b-011 — Audio tracks sheet
- Priority: P0
- Effort: M
- Depends: T-46b-010, T-46b-001, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/AudioTracksSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt
- Acceptance:
  - Rows are a single-select radio group; the selected row is italic and extra-bold; `Off` (id -1) is selectable.
  - Row labels use `track_label` (T-46b-003).
  - The header action "Delay" opens the audio delay panel (`PanelKind::AudioDelay`).
  - "Add external audio tracks" opens `gtk::FileDialog` with no filter; cancel does nothing (E45).
  - Selecting a row changes the playing audio track through the player model (area:45-player-core).

### T-46b-012 — Subtitle tracks sheet (two selection slots)
- Priority: P0
- Effort: M
- Depends: T-46b-010, T-46b-001, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/SubtitleTracksSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt
- Acceptance:
  - Rows show a checkbox; the first selected row shows `#1`, the second `#2`; a third selection is impossible (E43, E88).
  - Header actions "Palette" (opens `PanelKind::SubtitleSettings`) and "Delay" (opens `PanelKind::SubtitleDelay`).
  - Add row "Add external subtitles" opens a file dialog; cancel does nothing.
  - Footer text is exactly "Secondary subtitles will have no ASS/SSA Styling.".
  - Clicking a checked row deselects it; slot labels update in the same frame.

### T-46b-013 — Chapters sheet
- Priority: P0
- Effort: M
- Depends: T-46b-010, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/ChaptersSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Labels are `#N: <name>` where N is the stored ordinal (computed once at load, not by `indexOf`) (behavior §4.4, E49).
  - A null chapter name renders as an empty name, never the text "null" (E48).
  - Timestamps use `pretty_time` of the integer part of the start (E50).
  - The current chapter row is italic and extra-bold; clicking a row seeks to that chapter.
  - The sheet is not shown when there is no current chapter (E47).
  - For more than 200 chapters the list is a virtualized `gtk::ListView`.

### T-46b-014 — Quality sheet (hosters, expansion, per-video state)
- Priority: P0
- Effort: L
- Depends: T-46b-007, T-46b-010, T-46b-001, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt
- Acceptance:
  - Spinner shown while hosters load; content cross-fades in (p2-ui-parity §2.5).
  - Flat list when the only hoster is `no_hoster_list` and Ready; video rows show text then state icon.
  - Grouped mode shows hoster rows (32 px) with the four state presentations from p2-ui-parity §2.2, expansion arrows, and expandable video blocks.
  - Failed and empty groups are shown only when their flags are set (E58).
  - Selection and expansion are keyed by hoster name, not by list index (E55, R4); video selection uses (hoster name, video index).
  - Video state icons: spinner for LoadVideo, red error icon for Error, nothing for Queue and Ready.

### T-46b-015 — Playback speed sheet and speed presets
- Priority: P0
- Effort: M
- Depends: T-46b-002, T-46b-003, T-46b-005, T-46b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/PlaybackSpeedSheet.kt
- Acceptance:
  - Speed slider 0.01 to 6.00, continuous, label "Speed", value text "%.2fx".
  - Preset chips sorted numerically; invalid stored strings are skipped (E61); duplicate float values are merged (E63); removing a chip matches by float value (E62).
  - "Add" stores `preset_string(speed)`; the reset-presets button restores the 12 defaults; "Make default speed" stores the exact float; reset speed sets 1.0 and deletes the stored default.
  - Pitch correction switch writes `audio-pitch-correction` and the preference immediately.
  - Desktop: keyboard steps 0.05 with Shift for 0.01 (p2-ui-parity §3.2).

### T-46b-016 — More sheet: decoder chips
- Priority: P0
- Effort: S
- Depends: T-46b-009, T-46b-004, T-46b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Chips AutoCopy, Sw, Hw, HwPlus with titles "Auto", "SW", "HW", "HW+" (literal, not localized).
  - Selecting a chip sets `hwdec` for the session only; the preference is not written (behavior §7.4, E70).
  - Linux values come from `linux_value(mode, gpu)` in the design table; Android values are never written on Linux.
  - Selected chip is derived from the current `hwdec-current`; unknown values select nothing and do not throw (E71).

### T-46b-017 — More sheet: audio channel chips
- Priority: P0
- Effort: S
- Depends: T-46b-009, T-46b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt
- Acceptance:
  - Chips Auto, Auto-safe, Mono, Stereo, Reverse stereo, labelled exactly, in enum order.
  - Choosing Reverse stereo sets `audio-channels=auto` and adds the pan filter string to `af`.
  - Choosing any other value removes only the pan filter from `af`; the Kotlin `af=""` (which clears all filters) is not reproduced (E77).
  - The stored preference is the enum name; the default is AutoSafe.

### T-46b-018 — Sleep timer dialog
- Priority: P1
- Effort: M
- Depends: T-46b-001, T-46b-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt
- Acceptance:
  - Modal window transient for the player with title "Enter duration"; two spin buttons (hours 0..99, minutes 0..59 with wrap) replace the clock dial (dropped, §P3 task T-46b-040).
  - "Cancel current timer" appears only when a timer is running; "OK" sends `hour*3600 + minute*60` seconds (behavior §8.3).
  - 00:00 with OK sends 0, which cancels (E80).
  - The More-sheet button label is "Sleep Timer" when idle and "<remaining> remaining" with `format_elapsed` when running.
  - Hours above 23 are accepted (E79).

### T-46b-019 — More sheet: statistics page chips
- Priority: P2
- Effort: S
- Depends: T-46b-009, T-46b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt
- Acceptance:
  - Six chips: "Off" for page 0, "Page N" for 1..5.
  - Clicking sends the commands from `stats_actions` (toggle and/or `display-page-N`) and then stores the page in `pref_player_statistics_page`.
  - Selected chip is the stored page (desync with keyboard toggling is tracked as T-46b-042).

### T-46b-020 — More sheet: custom buttons
- Priority: P1
- Effort: S
- Depends: T-46b-001, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Section is hidden when there are no custom buttons.
  - Tap sends `script-message call_button_<id>`; long press (touch) or secondary click (mouse) sends `call_button_<id>_long` (p2-ui-parity §4.2).
  - Chips wrap in a flow box at 420 px width.
  - Button label is the `name` field; no other text is shown.

### T-46b-021 — Screenshot sheet (set as cover, background, thumbnail; share; save)
- Priority: P1
- Effort: M
- Depends: T-46b-001, area:34-images-covers-cache, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/ScreenshotSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt
- Acceptance:
  - Five tiles: Set as cover, Set as background, Set as thumbnail (only for a local source), Share, Save.
  - "Include Subtitles" switch shown only when the video has subtitle tracks; it changes the `subtitles` / `video` flag of the next capture.
  - Each capture writes a unique temp file (`<epoch-ms>_shot.png`), then renames to the final name; Share and Save never race on one file (E83, R6).
  - A failed capture (mpv wrote no file) shows the new message `player_screenshot_failed` instead of crashing (E82).
  - Set-as-art confirm dialog uses the text "Use this image as cover art?" for all three types (parity, E86), with OK and Cancel.
  - Share uses the XDG portal; Save uses `gtk::FileDialog::save` with default name `<title>_<seconds>.png`.

### T-46b-030 — Subtitle delay panel
- Priority: P1
- Effort: M
- Depends: T-46b-008, T-46b-031, T-46b-004, T-46b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt
- Acceptance:
  - Target dropdown Primary / Secondary / Both; "Both" writes the primary value to both mpv properties (behavior §11.2).
  - Delay field step 50 ms, unclamped; Speed field (0.10 to 10.00, step 0.01) shown only for Primary.
  - Live speed applies every value in 0.1..10 (E3, fix).
  - Rounding uses `round` everywhere (E7).
  - "Set as default" saves delay and, when in range, speed (E4); the target dropdown is disabled while a tap-sync run is active (E10).
  - Speed is rounded to three decimals with round-half-even on change (E13).

### T-46b-031 — Delay card widget (shared with the audio delay panel)
- Priority: P1
- Effort: M
- Depends: T-46b-008, T-46b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/AudioDelayPanel.kt
- Acceptance:
  - Two buttons with the labels in behavior §11.4: Audio "Sound heard" / "Sound spotted"; Subtitle "Voice heard" / "Text seen".
  - While a run is active, the button that started it is disabled and the other button is enabled and stops the run; from idle both buttons are enabled (truth table in behavior §11.5).
  - "Set as default" and "Reset" are disabled while a run is active.
  - The 20 ms tick updates the state machine; the final value is applied once on stop.
  - The same widget is used by the audio delay panel with `DelayKind::Audio` (part 1 owns that panel).

### T-46b-032 — Subtitle settings container (three cards)
- Priority: P1
- Effort: S
- Depends: T-46b-001, T-46b-033, T-46b-034, T-46b-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsPanel.kt
- Acceptance:
  - One layout on desktop: a header with title "Subtitles settings" and a close button, then a stack switcher with pages Typography, Colors, Miscellaneous (p2-ui-parity §7.2).
  - Escape and the close button both close the panel.
  - No orientation check; the Android pager is not reproduced.

### T-46b-033 — Typography card
- Priority: P1
- Effort: L
- Depends: T-46b-006, T-46b-004, T-46b-005, T-46b-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt
- Acceptance:
  - Bold and italic toggles write preference and `sub-bold` / `sub-italic`.
  - Three justification toggles (Left, Center, Right); a checked one is unchecked by a second click and writes `auto` (E25).
  - Font dropdown is the list from T-46b-006 with "Sans Serif" first; the spinner shows while scanning.
  - Font size 1..100; border size 0..100; shadow offset 0..100 (defaults 55, 3, 0 from behavior §13.9).
  - Border style menu lists Outline and shadow, Opaque box, Background box with a check mark on the current style.
  - Reset writes the defaults listed in p2-edge-cases-and-tests.md SV2 and re-reads all controls from mpv.

### T-46b-034 — Colors card
- Priority: P1
- Effort: M
- Depends: T-46b-003, T-46b-004, T-46b-005, T-46b-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsColorsCard.kt
- Acceptance:
  - Target toggles Text, Border, Background; four sliders Red, Green, Blue, Alpha in 0..255.
  - Values are read from the preference and mpv with fallback (E14); an unparseable mpv color falls back to the preference (E15).
  - Writes are coalesced to at most one every 50 ms while dragging (E17, proposal).
  - Reset restores the default for the selected target (white, black, or transparent).
  - Optional extra: a `gtk::ColorDialogButton` that updates the four sliders; the sliders stay the source of truth.

### T-46b-035 — Miscellaneous card
- Priority: P1
- Effort: S
- Depends: T-46b-004, T-46b-005, T-46b-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsMiscellaneousCard.kt
- Acceptance:
  - Override ASS/SSA switch writes the preference and `sub-ass-override` (`force` or `scale`); the switch state reads mpv (E22).
  - Scale slider 0.00..5.00 with two-decimal text; Position slider 0..150.
  - Reset restores position 100, scale 1, override false, and writes `sub-ass-override=scale`.
  - The debug print of the mpv value is removed (E19).
  - Scale 0 shows the hint "Subtitles are hidden at scale 0" (p2-ui-parity §10.2, new string).

### T-46b-036 — Video filters panel
- Priority: P2
- Effort: S
- Depends: T-46b-002, T-46b-004, T-46b-005, T-46b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/VideoFiltersPanel.kt
- Acceptance:
  - Five sliders in order Brightness, Saturation, Contrast, Gamma, Hue, range -100..100, integer values.
  - Reset sets all five to 0 in mpv and deletes the stored values.
  - The warning "Some filters may not work on your current video driver" is shown only when the video output is not `gpu-next`; the value is observed, not read once (E39).

### T-46b-038 — Persist secondary subtitle delay and fix live speed range
- Priority: P1
- Effort: S
- Depends: T-46b-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt
- Acceptance:
  - "Set as default" stores the secondary delay under `pref_subtitles_secondary_delay` when the target is Secondary or Both (E5, decision: proposal adopted).
  - Startup applies the stored secondary delay (existing behavior of `AniyomiMPVView.kt:264-267`), and a test confirms the round-trip.
  - Live speed and saved speed use the same range 0.1..10 (E3, E4).

### T-46b-039 — Typography correctness fixes
- Priority: P1
- Effort: S
- Depends: T-46b-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt
- Acceptance:
  - Unknown mpv values for justification or border style fall back to Auto or OutlineAndShadow instead of throwing (E23, E24).
  - Typography reset writes `sub-ass-justify=false`, not the ASS-override preference (E26).
  - Font list de-duplicates against "Sans Serif" (E30).
  - Startup writes `sub-ass-justify yes` whenever justification is not Auto, decided with T-46b-041 (E27).

### T-46b-040 — Android-only behavior dropped (documented)
- Priority: P3
- Effort: S
- Depends: T-46b-001, T-46b-018
- Sources: app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt, app/src/main/java/eu/kanade/presentation/player/components/SliderItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt
- Acceptance:
  - Drag-to-dismiss with anchored draggable physics is dropped; replacement is scrim click and Escape. Users lose the flick-to-close gesture (noted in the release notes).
  - Haptic feedback on slider changes is dropped; no desktop equivalent is provided.
  - Clock-dial `TimePicker` is dropped; replacement is two spin buttons (T-46b-018). Users lose the dial input mode; no function is lost.
  - Android Back is replaced by Escape; no separate Back gesture exists.

### T-46b-041 — Startup application parity audit for subtitle and audio prefs
- Priority: P1
- Effort: S
- Depends: T-46b-005, T-46b-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - A table test lists each key from p2-data-and-config.md §3 and the startup mpv write it produces (behavior §17).
  - Justification set without ASS override applies `sub-ass-justify yes` at startup (E27 decision adopted).
  - `audio-channels` is applied at file load (`PlayerActivity.kt:586`) with the enum-name mapping of T-46b-009.
  - `hwdec` at startup follows `pref_try_hwdec` (`auto` or `no`), unchanged.

### T-46b-042 — Statistics page desync when toggled by keyboard
- Priority: P2
- Effort: S
- Depends: T-46b-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - Decision recorded: either mpv is the source of truth (query the stats overlay state when the sheet opens) or the preference stays the truth and the desync is documented (E72).
  - If mpv is queried, the chip selection matches the overlay state after keyboard toggling.

### T-46b-043 — Remember decoder choice across files (opt-in)
- Priority: P2
- Effort: S
- Depends: T-46b-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Default remains session-only (parity, E70).
  - An opt-in setting, default off, stores the chosen decoder under a new key and applies it at startup instead of `pref_try_hwdec`.
  - Without the opt-in, behavior is identical to T-46b-016.

### T-46b-045 — Unit and fixture test suite for this part
- Priority: P0
- Effort: M
- Depends: T-46b-003, T-46b-005, T-46b-007, T-46b-008, T-46b-009, T-46b-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/PlaybackSpeedSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt
- Acceptance:
  - All tests in p2-edge-cases-and-tests.md §5 are implemented (T1-T6, N1-N7, C1-C8, D1-D7, S1-S5, A1-A6, P1-P5, H1-H3, DC1-DC6, F1-F6, X1, SV1-SV4, M1-M5).
  - The fixture `tests/fixtures/fonts/Sample-Regular.ttf` and the corrupt fixture `tests/fixtures/fonts/Broken.ttf` are committed.
  - `cargo test` passes without a display server (no GTK initialization in these tests).

### T-46b-046 — Wire the mockup player screen to the real sheet host
- Priority: P0
- Effort: S
- Depends: T-46b-001, area:45-player-core
- Sources: src/player.rs, src/ui.rs, spec/reader_player.json
- Acceptance:
  - Top-bar buttons Subtitles, Audio, Qualities, More open `SheetHost` sheets (not modal windows) for the player; long-press behavior is kept as documented in p2-ui-parity §16.3.
  - Chapters and speed open their sheets; surface long-press opens the screenshot sheet.
  - The reader screen still uses `show_sheet` and `show_dialog` unchanged.
  - The spec entries for the player sheets are kept as documentation and marked as superseded in a comment.
