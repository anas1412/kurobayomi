# Topic: subtitle style, subtitle delay, and their persistence

Deep dive for `p2-behavior.md` §11-§15, §17 and `p2-data-and-config.md` §3-§4. Repo root `/home/blackbox/aniyomi-src`.

## 1. Model: three layers that must agree

Every subtitle control in this part writes three things:

1. The **preference** (persistent, Android SharedPreferences through area:02).
2. The **mpv property** (live, the value the renderer uses now).
3. The **Compose state** (the value the slider or toggle shows).

The Kotlin code writes (1) and (2) in the same callback and keeps (3) as a local `remember` value. The read path differs per control:

| Control group | Initial read (3) comes from | Reset re-reads from |
|---|---|---|
| Delay (`sub-delay`, `secondary-sub-delay`) | mpv (`SubtitleDelayPanel.kt:89-93`) | reset sets state to 0, and the effect writes mpv |
| Speed (`sub-speed`) | mpv (line 93) | reset sets state to 1f |
| Typography | mpv for every control (`SubtitleSettingsTypographyCard.kt:134-161`) | mpv after `resetTypography` (lines 218-227) |
| Colors | mpv via `getCurrentMPVColor` (`SubtitleSettingsColorsCard.kt:92`) | mpv after `resetColors` |
| Misc (ASS override, scale, position) | mpv (lines 74-76, 89, 92) | mpv after reset (lines 134-145) |

So mpv is the **display source** for every control, and the preference is the **restart source**. This is consistent except where a write skips one of the two (section 4).

## 2. Startup order

`AniyomiMPVView.setupSubtitlesOptions()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:261-288`) runs once per file load and uses `setOptionString` (not `setPropertyString`). Order, with the key it reads:

1. `sub-delay` ← `pref_subtitles_delay` / 1000.0 (line 262)
2. `sub-speed` ← `pref_subtitles_speed` (263)
3. `secondary-sub-delay` ← `pref_subtitles_secondary_delay` / 1000.0 (264-267)
4. `sub-font` ← `pref_subtitle_font` (269)
5. if `pref_override_subtitles_ass`: `sub-ass-override=force` and `sub-ass-justify=yes` (270-272)
6. `sub-font-size` (274), `sub-bold` yes/no (275), `sub-italic` yes/no (276), `sub-justify` (277)
7. `sub-color` (278), `sub-back-color` (279-282), `sub-border-color` (283)
8. `sub-border-size` (284), `sub-border-style` (285), `sub-shadow-offset` (286)
9. `sub-pos` (287), `sub-scale` (288)

Observed consequences:
- `sub-ass-override` is **not** set to `scale` when the preference is false. mpv's own default is `scale`, so the result is the same unless the user changed `mpv.conf`.
- `sub-ass-justify` is set only with ASS override. Justification chosen in the typography card therefore does not affect ASS subtitles after a restart (E27).
- `sub-bold` and `sub-italic` are written as `yes` or `no` at startup, but the panel writes booleans through `setPropertyBoolean`. Both forms are valid for mpv.

## 3. Write matrix (every control, every target)

| Control (file, lines) | Preference written | mpv property written | Value written |
|---|---|---|---|
| Delay primary (SubtitleDelayPanel 100) | `pref_subtitles_delay` only on "Set as default" | `sub-delay` | `delay / 1000.0` |
| Delay secondary (101) | **none** | `secondary-sub-delay` | `secondaryDelay / 1000.0` |
| Delay both (103-104) | `pref_subtitles_delay` on default | both | `delay / 1000.0` |
| Speed (95) | `pref_subtitles_speed` on default if 0.1..10 (132-133) | `sub-speed` only if 0.1..1 | double |
| Bold (174) | `pref_bold_subtitles` | `sub-bold` bool | |
| Italic (188) | `pref_italic_subtitles` | `sub-italic` bool | |
| Justify toggle, checked (203-205) | `pref_sub_justify` = justification | `sub-ass-justify` = true; `sub-justify` = value | |
| Justify toggle, unchecked (206-209) | `pref_sub_justify` = `auto` | `sub-ass-justify` = false; `sub-justify` = `auto` | |
| Font (254-255) | `pref_subtitle_font` | `sub-font` | |
| Font size (268-269) | `pref_subtitles_font_size` | `sub-font-size` int | |
| Border style (310-311) | `pref_border_style_subtitles` (enum name) | `sub-border-style` | value string |
| Border size (332-333) | `pref_sub_border_size` | `sub-border-size` int | |
| Shadow (344-345) | `sub_shadow_offset` | `sub-shadow-offset` int | |
| Color channel (`SubtitleSettingsColorsCard.kt:138-142`, `onColorChange`) | per-target color pref (`pref_text_color_subtitles`, `pref_border_color_subtitles`, `pref_background_color_subtitles`) | `sub-color` / `sub-border-color` / `sub-back-color` as `#AARRGGBB` | |
| ASS override (81-82) | `pref_override_subtitles_ass` | `sub-ass-override` `force`/`scale` | |
| Scale (100-101) | `pref_sub_scale` (float) | `sub-scale` double | |
| Position (117-118) | `pref_sub_pos` | `sub-pos` int | |

## 4. Known gaps (decisions in the task list)

G1. **Secondary delay is never saved** (T-46b-038). `subtitlesSecondaryDelay` is read at startup (`AniyomiMPVView.kt:264-267`) and written nowhere in the app's UI scope. Users lose it on restart.

G2. **Live speed range is 0.1..1 only** (T-46b-038). `LaunchedEffect(speed)` (`SubtitleDelayPanel.kt:94-96`) skips values above 1.0, while saving accepts 0.1..10 (line 132). Result: a speed of 2.0 is saved, applied at startup, but not applied while the panel is open.

G3. **Justification and ASS-justify are decoupled** (T-46b-033, T-46b-041). The toggles set `sub-ass-justify` from their own state; reset sets it from `pref_override_subtitles_ass`; startup sets it only with ASS override.

G4. **Reset writes the wrong ASS flag** (T-46b-039). `resetTypography` line 359 writes `sub-ass-justify` from `overrideSubsASS().deleteAndGet()`.

G5. **Rounding is inconsistent** (E7). Initial delay read uses `roundToInt()`, the affected-subtitle effect uses `toInt()` truncation. Fix: `round` everywhere.

G6. **Unchecking a justify toggle leaves it checked** in the UI (E25, `SubtitleSettingsTypographyCard.kt:201`), because `justify` is set to the toggle's own value even when unchecked.

G7. **Stray debug output** (E19, line 75) on every composition.

## 5. Delay sign semantics (for the port's test oracle)

The delay in milliseconds is `delay`; mpv gets `delay / 1000.0` seconds. A positive delay makes the subtitle appear later.

Tap-sync (`DelayCard`, `SubtitleDelayPanel.kt:204-318`) produces a signed delta of `±elapsed` milliseconds. The sign is set by the button that starts the run (behavior §11.5):

- Subtitle: "Voice heard" starts a Negative run (delay decreases); "Text seen" starts a Positive run (delay increases). A run is ended by pressing the other button.
- Audio (for the shared card): "Sound heard" starts Positive; "Sound spotted" starts Negative.

Intuition for the subtitle case: if the user hears the voice and then sees the text some seconds later, the subtitle is late, so the delay should go negative. The port keeps the exact Kotlin mapping; the naming was chosen by the original author and is not reversed here.

## 6. Test oracle (copy of the expected writes)

For `reset_typography` with an empty preference store, the expected recorded writes in order are (from `resetTypography`, lines 357-365, with the G4 fix applied):

```
sub-bold = false
sub-italic = false
sub-ass-justify = false          (fixed: Kotlin writes overrideSubsASS value, default false)
sub-justify = "auto"
sub-font = "Sans Serif"
sub-font-size = 55
sub-border-size = 3
sub-shadow-offset = 0
sub-border-style = "outline-and-shadow"
```

For `reset_misc`: `sub-pos = 100`, `sub-scale = 1.0`, `sub-ass-override = "scale"`.

For `reset_colors(Text)`: `sub-color = "#FFFFFFFF"`.

These are the assertions in p2-edge-cases-and-tests.md SV2-SV4.

## 7. Rust mapping

- `SubtitlePrefs` (typed, in `src/player/settings_view.rs`) owns all keys; methods `reset_typography`, `reset_misc`, `reset_colors` perform the exact write list above through `MpvProps` and `Store`.
- The panel model is a plain struct updated by widget callbacks; each callback calls exactly one function that performs (1) store write and (2) mpv write, so the two cannot drift (the design rule from p2-rust-design §13.1).
- Startup application is a single function `apply_subtitle_prefs(&SubtitlePrefs, &dyn MpvProps)` called from area:45's file load path, with the G3 decision implemented there (T-46b-041).
