# 46 Player controls UI: overview

Area lead overview for `46-player-controls-ui`. Repo root for every path: `/home/blackbox/aniyomi-src` (repo-relative). Target repo: `/home/blackbox/kurobayomi`. Task ids: part 1 uses `T-46a-NNN`, part 2 uses `T-46b-NNN`.

Status of this folder: 2 task files lint `ok` (`p1-tasks.md`, `p2-tasks.md`). Two dependency cycles and several priority inversions remain (section 8). They are reported here, not silently changed.

---

## 1. Purpose and scope

Aniyomi's anime player screen is a set of Compose overlays on top of an mpv surface. This area covers everything the viewer touches on that screen except the mpv engine, the view model and the reader/player data layer (owned by area:45-player-core):

- Root overlay with show/hide, auto-hide and lock (`PlayerControls.kt`).
- Pointer and touch gestures on the video surface: tap, double-tap zones with seek feedback, long-press speed boost, horizontal seek drag, vertical volume/brightness drag (`GestureHandler.kt`).
- Transport and corner controls: play/pause, previous/next, top-left back and titles, top-right auto-play, subtitles, audio, quality, more; bottom-left lock, rotate, speed, chapter pill; bottom-right skip-intro or custom primary button, PiP, aspect.
- Seekbar with chapter ticks and read-ahead, timers, and the floating thumbnail preview.
- Volume and brightness pills, the brightness veil, and the player update toast.
- Bottom sheets (tracks, chapters, quality/hosters, playback speed, more, screenshot), right-edge panels (subtitle settings, subtitle delay, audio delay, video filters) and dialogs (episode list, integer picker, shell).

Why it matters for a usable Kurobayomi: without this area the player is a bare mpv window. Watching an episode needs the transport, seek, speed, track selection, quality/hoster choice and the sleep timer. The gesture layer is the main input model on a touch laptop and is also what most Aniyomi users expect.

Size: 46 Kotlin files, 7781 LOC in scope (part 1: 32 files, 4717 LOC; part 2: 14 files, 3064 LOC; LOC from `_tools/areas.py files`). Part 1 is in `p1-inventory.md`; part 2 is in `p2-inventory.md`.

Desktop-specific scope additions (new, not in Aniyomi): keyboard shortcut map (T-46a-043), mouse wheel and right-click (T-46a-044), Linux system volume and brightness backends (T-46a-049), mini-player window replacing PiP (T-46a-046).

---

## 2. Module map

### 2.1 Kotlin packages to responsibilities

All paths are under `app/src/main/java/eu/kanade/`.

| Package / folder | Files in scope | Responsibility |
|---|---|---|
| `presentation/player/components/` | `ExpandableCard.kt`, `ExposedTextDropDownMenu.kt`, `OutlinedNumericChooser.kt`, `OvalBox.kt`, `PlayerSheet.kt`, `RepeatingIconButton.kt`, `SliderItem.kt`, `SwitchPreference.kt`, `TintedSliderItem.kt` | Shared Compose widgets: sheet container, sliders, numeric chooser, switch row, dropdown, expander, repeat button, oval clip shapes. |
| `tachiyomi/ui/player/controls/` | `BottomLeftPlayerControls.kt`, `BottomRightPlayerControls.kt`, `GestureHandler.kt`, `MiddlePlayerControls.kt`, `PlayerControls.kt`, `PlayerDialogs.kt`, `PlayerPanels.kt`, `PlayerSheets.kt`, `TopLeftPlayerControls.kt`, `TopRightPlayerControls.kt` | Root overlay and its children; gesture surface; routing of sheets, panels and dialogs. |
| `tachiyomi/ui/player/controls/components/` | `AutoPlaySwitch.kt`, `BrightnessOverlay.kt`, `ControlsButton.kt`, `CurrentChapter.kt`, `DoubleTapSeekTriangles.kt`, `PlayerUpdates.kt`, `SeekBar.kt`, `ThumbnailPreview.kt`, `VerticalSliders.kt` | Leaf widgets: buttons, seekbar, thumbnail, gauges, toast, veil, chapter pill, auto-play toggle. |
| `tachiyomi/ui/player/controls/components/dialogs/` | `EpisodeListDialog.kt`, `IntegerPickerDialog.kt`, `PlayerDialog.kt` | Modal dialogs and their shell. |
| `tachiyomi/ui/player/controls/components/panels/` | `AudioDelayPanel.kt`, `SubtitleDelayPanel.kt`, `SubtitleSettingsColorsCard.kt`, `SubtitleSettingsMiscellaneousCard.kt`, `SubtitleSettingsPanel.kt`, `SubtitleSettingsTypographyCard.kt`, `VideoFiltersPanel.kt` | Right-edge panels; `SubtitleDelayPanel.kt` owns `DelayCard` and `DelayType`, reused by audio delay. |
| `tachiyomi/ui/player/controls/components/sheets/` | `AudioTracksSheet.kt`, `ChaptersSheet.kt`, `GenericTracksSheet.kt`, `MoreSheet.kt`, `PlaybackSpeedSheet.kt`, `QualitySheet.kt`, `ScreenshotSheet.kt`, `SubtitleTracksSheet.kt` | Bottom sheets. `GenericTracksSheet.kt` is the shared skeleton for audio, subtitle and chapter lists. |
| `tachiyomi/ui/player/` (owned elsewhere) | `PlayerViewModel.kt`, `PlayerEnums.kt`, `PlayerActivity.kt`, `utils/ChapterUtils.kt`, `settings/*.kt` | State, mpv bridge, enums, preferences. Owners: area:45-player-core, area:02-preferences-system. |
| `presentation-core/` (owned elsewhere) | `components/ActionButton.kt`, `components/material/Constants.kt`, `components/SettingsItems.kt` | Shared button, alpha constants (`DISABLED_ALPHA = .38f`, `SECONDARY_ALPHA = .78f`), settings row helpers. Owner: area:36-ui-shell-navigation-security-webview. |

### 2.2 Rust module map

Two part-level designs exist and disagree (section 4.2). The map below is the lead's merged proposal. Every module is new under `src/player/`; the mockup file is `src/player.rs` (337 lines, placeholder surface).

Proposal: one facade, one pure folder, one GTK folder.

```
src/player.rs                       facade: pub fn player(title, episode) -> gtk::Box; re-exports
src/player/backend.rs               MpvCmd, MpvProps trait (typed get/set f64/i64/bool/str + command), event types   [area:45 wrapper]
src/player/mpvprops.rs              typed helpers over MpvProps (sub-delay, sub-speed, audio-delay, ...)
src/player/settings.rs              PlayerSettings with serde defaults; typed reads/writes over area:02 store
src/player/logic/                   PURE, no GTK, unit-tested
  logic/mod.rs
  model.rs                          ControlsModel, SheetKind, PanelKind, DialogKind, PlayerUpdate, host rules
  math.rs                           gesture math, zone(), speed and aspect cycles, volume/brightness icon and text
  time.rs                           pretty_time(), pretty_time_signed(), format_episode_number()
  segments.rs                       build_seek_segments(), current_chapter(), chapter_label()
  repeat.rs                         RepeatSchedule (750 ms start, decay 0.25, floor 5 ms)
  track_label.rs                    getTrackTitle decision table
  delay_measure.rs                  tap-to-sync state machine
  hoster_groups.rs                  hoster partition
  hoster_state.rs                   HosterState enum + get_changed_at
  number.rs                         to_fixed, round_half_even_3
  color.rs                          copy_argb, color_hex (#AARRGGBB)
  speed_presets.rs                  preset parse/format/sort/add/remove
  audio_channels.rs                 AudioChannels label <-> mpv value table
  stats_page.rs                     statistics page toggle truth table
  fonts.rs                          TTF family scan (ttf-parser), filename test
  decoder.rs                        Decoder enum <-> mpv value, unknown-value fallback
src/player/controls/                overlay and its leaves (GTK)
  controls/mod.rs                   assembly: overlay, visibility wiring, host routing
  overlay.rs                        root gtk::Overlay, veil, placement, animation timings
  gestures.rs                       GestureClick, GestureDrag, GestureLongPress wiring
  seekbar.rs                        SeekBar GObject subclass (snapshot-drawn)
  transport.rs                      centre previous/play-pause/next and gesture seek text
  top.rs                            top-left (back, titles) and top-right actions (per p1-rust-design section 1)
  bottom.rs                         bottom-left and bottom-right rows
  buttons.rs                        ControlsButton and FilledControlsButton equivalents, CSS classes
  gauge.rs                          VerticalGauge widget with overflow segment
  volume.rs                         VolumePill and SystemVolume trait
  brightness.rs                     BrightnessPill and SystemBrightness trait
  veil.rs                           BrightnessVeil (software dim for negative brightness)
  oval.rs                           half-oval cairo paths, DoubleTapTriangles animation
  toast.rs                          PlayerUpdate pill
  thumbnail.rs                      ThumbnailPreview popup, tile LRU
  numeric.rs                        NumericChooser (integer and float), RepeatingButton
  auto_play.rs                      auto-play toggle
  chapter_pill.rs                   current-chapter pill
  widgets.rs                        SwitchRow, SliderRow, TintedSlider, ExpandableCard, DropDownRow
src/player/ui/                      sheets, panels, dialogs (GTK)
  ui/host.rs                        SheetHost: single owner of open/close, scrim, Escape, mutual exclusion
  ui/sheets/*.rs                    track_list, tracks (audio + subtitle), chapters, quality, speed, more, screenshot, sleep_timer
  ui/panels/*.rs                    subtitle_settings, subtitle_typography, subtitle_colors, subtitle_misc, subtitle_delay, delay_card, video_filters
  ui/dialogs/*.rs                   shell, episode_list, integer_picker
```

Notes on the map:

- `p1-rust-design.md` section 1 is the canonical file list for `controls/`. `p1-inventory.md` column "Target Rust module" uses different names (`top_left.rs`, `top_right.rs`, `bottom_left.rs`, `bottom_right.rs`, `system_overlay.rs`, `panels.rs`, `host.rs`). Use the design doc's names. The inventory must be corrected to match.
- `src/player/mod.rs` (p1) and `src/player.rs` facade (p2) are the same thing. Proposal: `src/player.rs` plus submodules, no `mod.rs`, to match the existing mockup layout.
- Pure modules must not import `gtk`, `glib` or `libmpv2`. This is the rule that makes the tests in section 7 possible.

---

## 3. Reading guide

All files are in this folder. Links are relative.

- [p1-inventory.md](p1-inventory.md): the 32 part-1 Kotlin files, LOC, declared names, port decision and task ids per file, plus out-of-scope references.
- [p1-behavior.md](p1-behavior.md): OBSERVED behaviour for part 1: overlay visibility, gestures, double-tap feedback, transport, corner controls, seekbar, thumbnail, volume/brightness, toast, dialogs, panels host, with `file:line` citations.
- [p1-data-and-config.md](p1-data-and-config.md): part-1 enums with exact values, domain models, preference keys and defaults, mpv properties and commands, Android APIs and Linux replacements, on-disk paths, layout constants, Lua message contract, input validity rules.
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases, failure modes, races, Linux-specific input handling and the unit, manual and fixture test plan for part 1.
- [p1-rust-design.md](p1-rust-design.md): Rust module layout, pure model and state machine, math, backend trait, GTK widget tree, gesture and seekbar implementation, threading, error handling, pitfalls, test seams.
- [p1-tasks.md](p1-tasks.md): part-1 task list (60 tasks) in the lint format.
- [p1-topic-system-volume-brightness.md](p1-topic-system-volume-brightness.md): deep dive on Linux device volume (`wpctl`), brightness (sysfs and logind), software veil and boost.
- [p1-ui-parity.md](p1-ui-parity.md): visual and state parity with the Kotlin UI, mockup gap list, deviations D-1 to D-20, Linux input equivalents, verification checklist.
- [p2-inventory.md](p2-inventory.md): the 14 part-2 Kotlin files (panels and sheets), LOC, declared names, port decisions, task ids, and the symbols owned by other files.
- [p2-behavior.md](p2-behavior.md): OBSERVED behaviour for part 2: shared building blocks, track sheets, chapters, quality/hosters, playback speed, more sheet, sleep timer, screenshot sheet, subtitle settings, delay, filters.
- [p2-data-and-config.md](p2-data-and-config.md): part-2 enums with ordinals, data classes, preferences, mpv properties and startup writes, files, message keys, constants.
- [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md): per-file edge cases E1 to E89 with decision tags (parity, fix, decision needed), failure modes, races, parity contract, Rust test list.
- [p2-rust-design.md](p2-rust-design.md): crate and module layout for part 2, `SheetHost`, shared widgets, tap-to-sync state machine, settings view, font scan, pure functions, data flow, concurrency.
- [p2-tasks.md](p2-tasks.md): part-2 task list (36 tasks) in the lint format.
- [p2-topic-quality-hosters-and-tracks.md](p2-topic-quality-hosters-and-tracks.md): deep dive on track lists, the hoster/video model and quality sheet state machine.
- [p2-topic-sheets-panels-host.md](p2-topic-sheets-panels-host.md): deep dive on the sheet and panel host, router and open/close rules.
- [p2-topic-subtitle-style-and-delay.md](p2-topic-subtitle-style-and-delay.md): deep dive on subtitle typography, colours, delay and the tap-to-sync measurement.
- [p2-ui-parity.md](p2-ui-parity.md): visual parity for every part-2 sheet and panel, desktop differences, the mockup gap list and proposed new strings.

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the docs (lead agrees)

| Decision | Choice | Where |
|---|---|---|
| Overlay root | `gtk::Overlay` with a visibility state machine; auto-hide after `playerTimeToDisappear` default 4000 ms, only while playing and not seeking | p1-behavior 1.1 to 1.3, p1-rust-design 2.2 |
| Gestures | `GestureClick` (n_press 1 and 2), `GestureLongPress`, `GestureDrag` on a child of the overlay | p1-inventory row 12, p1-rust-design 5 |
| Seekbar | Custom GObject `SeekBar`, snapshot-drawn, replaces the `dev.vivvvek.seeker` 1.2.2 library | p1-inventory external list, T-46a-010 |
| Device volume | PipeWire via `wpctl set-volume @DEFAULT_AUDIO_SINK@`, 15 steps, behind a `SystemVolume` trait | p1-topic-system-volume-brightness 2.1, p1-rust-design 11 |
| Brightness | Hardware where writable (sysfs or logind), software veil for negative values and as fallback | p1-behavior 8.5, D-20 |
| Boost volume | mpv `volume` property above 100, cap `pref_audio_volume_boost_cap`, `volume-max` = cap + 100 | p1-behavior 8.5 |
| Hoster/quality | Rust enum `HosterState` (Idle, Loading, Error, Ready); composable `VideoTrack` renamed `VideoRow` to avoid clash with `PlayerViewModel.VideoTrack` | p2-inventory notes, p2-ui-parity 2.3 |
| Integer picker | List instead of a wheel; commits the displayed item | D-6, D-11 |
| Play/pause icon | Cross-fade, not morph | D-7 |
| Toasts | In-window pills, not OS notifications | D-8 |
| Ripple | CSS hover and active states | D-18 |
| Rotate button | Removed on Linux | D-15 |
| Haptics | Removed | D-17 |
| Clock dial in sleep timer | Dropped for v1; two `gtk::SpinButton` (hours 0..99, minutes 0..59) | p2-ui-parity 6.2 |
| Sheet drag-to-dismiss | Dropped; dismiss by scrim, Escape, close button, back | p2-rust-design 601 |

### 4.2 Conflicts between the docs (must be resolved before implementation)

1. Sheet container. `p1-ui-parity.md` D-19 says sheets and dialogs are modal windows with the scrim in the player window (T-46a-029 "Sheet container"). `p2-rust-design.md` section 3 and `T-46b-001` say an in-window `SheetHost` overlay with `gtk::Revealer` and scrim. Both are P0, both L, both own the same responsibility. Recommendation: in-window overlay (`SheetHost` in `ui/host.rs`) as the single owner. Reasons: one surface, follows fullscreen, avoids transient-window stacking problems on Wayland and X11. Fold T-46a-029 into T-46b-001, or make T-46a-029 a dependency of T-46b-001 with a narrower scope.
2. Module tree. Part 1 uses `src/player/controls/` for sheets, panels and dialogs (`sheet.rs`, `panels.rs`, `dialogs/`). Part 2 uses `src/player/ui/` for the same things and `src/player/logic/` for pure code. Recommendation: section 2.2 above (overlay in `controls/`, sheets/panels/dialogs in `ui/`, all pure code in `logic/`).
3. Settings module. Part 1 has `settings.rs`; part 2 has `settings_view.rs`. Recommendation: one `settings.rs`.
4. Top and bottom rows. `p1-inventory.md` names four files (`top_left.rs`, `top_right.rs`, `bottom_left.rs`, `bottom_right.rs`); `p1-rust-design.md` names two (`top.rs`, `bottom.rs`). Recommendation: the two-file version.
5. Subtitle delay `DelayCard`. Part 1 places the shared `DelayCard` in `controls/panels.rs`; part 2 places it in `ui/panels/delay_card.rs`. Recommendation: `ui/panels/delay_card.rs`, owned by part 2, imported by `AudioDelayPanel` (part 1).
6. Task overlap. `T-46a-027` (sheet and dialog host routing) and `T-46b-046` (wire the mockup player screen to the real sheet host) both touch the same router. Recommendation: `T-46a-027` covers only the dialog dispatch (`PlayerDialogs.kt`); sheet and panel routing belongs to `T-46b-001` and `T-46b-046`.

### 4.3 Open questions for the user

Each has a recommendation. The docs already tag these as "decision needed" in `p2-edge-cases-and-tests.md`.

1. Persist the secondary subtitle delay. Kotlin `onApply` saves only `delay` and `speed` (E5). Recommendation: also persist `pref_subtitles_secondary_delay` on "Set as default" when target is Secondary or Both.
2. Tap-to-sync run interrupted by closing the panel (E8). Recommendation: apply the current value on close. Kotlin discards it.
3. Editing the delay field during a tap-to-sync run (E11). Recommendation: disable the field while a run is active.
4. Subtitle override switch source of truth (E22). Recommendation: mpv is the source for the switch; the preference is written and read on next start.
5. Subtitle justification without ASS override (E27). Recommendation: set `sub-ass-justify yes` whenever justify is not Auto.
6. Clicks on a hoster while it is `Loading` (E54). Recommendation: ignore them.
7. Statistics page toggle desync when the overlay is toggled from the keyboard (E72). Recommendation: read the preference on every change; P2 task.
8. Set-as-art confirm text is the same for every art type (E86). Recommendation: keep parity text in v1; new per-type keys are a later i18n task.
9. Removing the primary track from a two-slot selection shifts slot labels (E89). Recommendation: removing the primary promotes the secondary to primary, as the view model does.
10. Keyboard map (T-46a-043) and mouse wheel and right-click (T-46a-044) are new Linux behaviour (D-14). Recommendation: approve the map in `p1-ui-parity.md` section 10.2 before implementation; it is not in Aniyomi.
11. PiP replacement (D-16, T-46a-046). Recommendation: a mini-player window that keeps playing, not a system PiP. The user should confirm they want it at all, since it is the least-used control.
12. Brightness hardware writes (T-46a-049). Writing `/sys/class/backlight` needs permission the user may not have. Recommendation: try logind `SetBrightness` first, then sysfs, then fall back to the veil. The logind signature was read with `gdbus introspect`, but a successful write was not tested on the target machine. Confirm the user accepts this order before implementation.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Where it is used |
|---|---|---|
| area:45-player-core | `PlayerViewModel` state flows (paused, position, duration, chapters, hosters, `VideoTrack` list, speed, sleep timer), mpv bridge, `takeScreenshot`, `setAsArt`, `updateDecoder`, custom button execution, skip-intro chapter logic, PiP params, episode list data, thumbnail tile fetch | Most control tasks. The token appears 17 times across the two task files. |
| area:02-preferences-system | `PreferenceStore` getters, defaults, `getEnum` for every key in `p1-data-and-config.md` section 3 and `p2-data-and-config.md` section 3 | T-46a-042, T-46b-005 |
| area:16-source-api-anime | `Video`, `Video.State`, `Hoster`, `Hoster.NO_HOSTER_LIST`, `ChapterType`, `TimeStamp`, `AnimeHttpSource.getImageTile` | T-46a-010, T-46b-007, T-46b-014 |
| area:04-db-anime-schema-migrations | Bookmark and fillermark storage used by episode list | T-46a-025 |
| area:13-core-util-storage-archive-torrent | `StorageManager.getFontsDirectory`, `UniFile`, relative date helper | T-46b-008 (font scan) |
| area:34-images-covers-cache | Cover, background and thumbnail edit used by `setAsArt` | T-46b-021 |
| area:43-reader-core | Image save notification and file naming shared with manga | T-46b-021 |
| area:14-source-local-and-metadata | `isLocalSource` gating of the thumbnail-art option | T-46b-021 |
| area:36-ui-shell-navigation-security-webview | Theme colour roles (`surface`, `surfaceDim`, `tertiary`, `errorContainer`, `inversePrimary`), alpha constants, ripple configuration | Every visual task |
| area:39-ui-entry-details-anime | Episode number formatting and date conventions | T-46a-024 |
| area:07-domain-entries-items | `Episode` fields (`seen`, `bookmark`, `fillermark`, `dateUpload`, `episodeNumber`, `scanlator`) and `Anime.EPISODE_DISPLAY_NUMBER` | T-46a-024 |

### 5.2 What others need from this area

- area:45-player-core: the GTK player surface that the overlay and sheets sit on. The overlay must not own mpv; `backend.rs` is the boundary.
- area:48-settings-player-and-scripts and area:47-settings-screens-general: the player preference screens reuse `SwitchRow`, `SliderRow`, `NumericChooser` and `DropDownRow` from `controls/widgets.rs` and `controls/numeric.rs`. Those widgets are defined here.
- area:49-theme-and-design-system: the CSS classes (`circular`, `pill`, `osd`) used by player buttons and toast.
- area:36-ui-shell-navigation-security-webview: the keyboard shortcut layer (T-46a-043) must be registered with the window shell so that global keys do not conflict.

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, filtered to `file` starting with `46-player-controls-ui/`. The export reports 3801 tasks overall and 0 errors.

| Scope | Tasks | P0 | P1 | P2 | P3 | S | M | L | XL |
|---|---|---|---|---|---|---|---|---|---|
| part 1 (`p1-tasks.md`) | 60 | 22 | 23 | 11 | 4 | 33 | 23 | 4 | 0 |
| part 2 (`p2-tasks.md`) | 36 | 17 | 14 | 4 | 1 | 20 | 13 | 3 | 0 |
| **area total** | **96** | **39** | **37** | **15** | **5** | **53** | **36** | **7** | **0** |

Dependency tokens: every `area:<id>` token in the two task files resolves to a real folder under `plans/`. Two invalid tokens were fixed in this pass (`area:45` in `p2-tasks.md`, in prose, now `area:45-player-core`). No `T-` dependency points outside this area or to a missing id.

### 6.1 P0 critical path

Longest weighted chains (weights S=1, M=2, L=4) computed from `TASKS.csv`:

1. `T-46b-001`(L) to `T-46b-002`(M) to `T-46b-010`(S) to `T-46b-014`(L). Weight 11. Quality sheet, after the sheet host and shared controls.
2. `T-46a-042`(S) to `T-46a-041`(S) to `T-46a-029`(L) to `T-46a-027`(M) to `T-46a-004`(M). Weight 10. Top-right actions, after prefs, animation timings and sheet container.
3. `T-46a-040`(S) to `T-46a-010`(L) to `T-46a-018`(M). Weight 7. Seekbar then horizontal drag seek.

Recommended P0 build order (leaves first, each step verifiable with the pure tests):

1. Leaves with no dependency: `T-46a-039` gesture math, `T-46a-040` time formatting, `T-46a-058` control button primitives, `T-46b-003` pure formatting and colour helpers, `T-46b-004` mpv property boundary (after area:45 wrapper), `T-46b-007` hoster model (after area:16).
2. Preferences: `T-46a-042` (area:02), `T-46b-005` (area:02).
3. Overlay: `T-46a-001` visibility state machine and auto-hide, then `T-46a-002` lock, `T-46a-014` single tap, `T-46a-003` top-left, `T-46a-006` transport, `T-46a-007` seek indicator text.
4. Seekbar: `T-46a-010` (L), then `T-46a-011` time labels, `T-46a-018` horizontal drag seek.
5. Double tap: `T-46a-015` zones, `T-46a-016` accumulator and timing.
6. Sheet host: `T-46b-001` (L), `T-46b-002`, then `T-46a-029` (or its merged replacement, section 4.2), `T-46a-059` dismissal, `T-46a-027` dialog dispatch.
7. Sheets: `T-46b-010`, then `T-46b-011` audio, `T-46b-012` subtitles, `T-46b-013` chapters, `T-46b-014` quality, `T-46b-015` speed, `T-46b-016` and `T-46b-017` more-sheet chips.
8. Corners and bottom row: `T-46a-004`, `T-46a-008`, `T-46a-009`, and the wiring in `T-46b-046`.
9. Tests: `T-46a-056` and `T-46b-045` run before the final integration.

### 6.2 Risks

1. Dependency cycles (blocking, must fix before the critical path is trusted):
   - `T-46a-006` depends on `T-46a-053`, and `T-46a-053` depends on `T-46a-006`.
   - `T-46a-009` depends on `T-46a-050`, and `T-46a-050` depends on `T-46a-009`.
   Recommendation: drop the parent-to-child edges (`T-46a-006` to `T-46a-053`, `T-46a-009` to `T-46a-050`), keeping the child-to-parent direction.
2. Priority inversions. A P0 task depends on a lower-priority task. Either promote the dependency or demote the dependent.
   - P0 depends on P1: `T-46a-004` to `T-46a-005`; `T-46a-006` to `T-46a-041`; `T-46a-009` to `T-46a-050`; `T-46a-016` to `T-46a-017`; `T-46a-029` to `T-46a-041`; `T-46b-045` to `T-46b-008`.
   - P0 depends on P2: `T-46a-006` to `T-46a-053`; `T-46a-008` to `T-46a-052`; `T-46a-009` to `T-46a-051`; `T-46a-009` to `T-46a-046`.
   - P1 depends on P2: `T-46a-030` to `T-46a-031`.
3. Duplicate host ownership (section 4.2 items 1 and 6). Two P0 L tasks own the same container.
4. External blocker. Every control that touches mpv depends on area:45-player-core. The gesture and seekbar code can be built and tested against the fake backend, but the player screen is not usable until area:45 has a real libmpv binding.
5. Hardware brightness needs permission on the target machine. The docs say `brightnessctl` is absent and `/sys/class/backlight/intel_backlight` exists. The logind `SetBrightness` signature was read from `gdbus introspect` (p1-topic-system-volume-brightness). Verify before relying on it.
6. Unverified `wpctl` behaviour: whether `set-volume` clears mute. The doc says "verify on the machine" and proposes calling `wpctl set-mute @DEFAULT_AUDIO_SINK@ 0` first. Test on the target before shipping.
7. Scale. 96 tasks, 7781 LOC of Kotlin, 39 P0 tasks. The lead recommends a vertical slice first: overlay, seekbar, transport, single-sheet (audio tracks), tap and double-tap seek, using the fake backend. Then the remaining sheets.

---

## 7. Definition of done

Each item is an observable behaviour on the target machine (CachyOS, KDE Plasma, Wayland and X11), with the libmpv backend from area:45-player-core. Each item maps to acceptance criteria in the tasks files.

1. Tapping the video toggles the controls. Controls hide after 4000 ms while playing and not seeking, and stay visible when paused. The lock button hides all controls except the lock, and a tap on the lock shows it.
2. Double-tapping the left or right third seeks by the amount in preferences. The seek indicator shows the accumulated amount. 800 ms after the last tap the accumulated amount resets to 0 and the seek text is cleared; the bar is hidden 100 ms after that (p1-behavior 3.1).
3. Horizontal drag on the video seeks, and the seek text shows the target time. Releasing the drag seeks to the target. Vertical drag on the left half changes brightness, and on the right half changes volume. The device volume range is 15 steps (`max_steps`), so one step is about 67 px of drag (p1-topic-system-volume-brightness 2.1).
4. Long-press on the video starts the speed boost and releases it on pointer-up. Releasing does not restore speed (D-4).
5. The seekbar shows chapter ticks, a read-ahead region, elapsed time on the left, and duration (or remaining, on toggle) on the right. While seeking, the thumbnail preview appears above the seekbar with the chapter label when the source provides a tile (p1-behavior 7).
6. Play/pause, previous and next work. Play/pause shows a spinner while loading and the icon cross-fades when the state changes.
7. The top-left back button closes the player. The title block shows the anime title and the episode name, and it opens the episode list dialog.
8. The top-right row opens the subtitle sheet, the audio sheet, the quality sheet (online sources only) and the more sheet. Long-press on the subtitle button opens the subtitle settings panel, and long-press on the audio button opens the audio delay panel (mockup mapping, p2-rust-design section 14). The auto-play toggle persists its value (D-13).
9. Opening any sheet closes the other sheet. Escape, scrim click and the close button close the sheet. No sheet or panel survives a player close.
10. The bottom-left row cycles playback speed on tap and opens the speed sheet on long-press, and shows the chapter pill when the episode has chapters.
11. The bottom-right row shows Skip intro when the intro chapter is current and the source defines one, and the custom primary button otherwise when configured. The aspect button cycles the video aspect.
12. The volume pill shows the device volume in steps and the boost value above the device maximum. The brightness pill shows the current brightness. Negative brightness dims with a black veil and never reaches hardware.
13. The sleep timer in the more sheet takes hours and minutes (seconds = hour*3600 + minute*60), shows the remaining time as `MM:SS` or `H:MM:SS`, and offers cancel only while a timer runs. Setting 00:00 equals cancel (p2-behavior 8.4). The action when the timer fires is defined in area:45-player-core.
14. The subtitle settings panel writes the mpv subtitle properties live. Changing the typography, colours or delay changes the rendered subtitles immediately. The tap-to-sync measurement produces a delay value and applies it on stop.
15. The episode list dialog highlights the current episode and toggles bookmark and fillermark per episode.
16. All strings come from the i18n keys listed in `p1-data-and-config.md` section 11 and `p2-data-and-config.md` section 6, or from the proposed keys in `p2-ui-parity.md` section 17 after they are added.
17. Every pure module has unit tests that run without a display (`cargo test`), and the test list in section 5 of `p2-edge-cases-and-tests.md` and section 11.1 of `p1-edge-cases-and-tests.md` is green.
18. Every deviation D-1 to D-20 is either implemented as listed or changed in the doc with the user's approval.

---

## 8. Open items in this pass

- Two dependency cycles (section 6.2) were reported, not changed. Changing a dependency edge changes the critical path, so the author should choose which edge to drop.
- The priority inversions (section 6.2) were reported, not changed, for the same reason.
- The module-tree conflicts (section 4.2) were resolved in this overview only. The part docs still carry the old names until their authors reconcile them.
- The `area:45` prose tokens in `p2-tasks.md` were fixed to `area:45-player-core`. No task id was changed.
