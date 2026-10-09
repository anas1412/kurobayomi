# Topic: how player preferences become mpv options (and the Linux libmpv calls)

OBSERVED sources: `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt` (`initOptions` lines 118-170, `postInitOptions`, `setupAudioOptions`, `setupSubtitlesOptions`), `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` (`setupPlayerMPV` 424-447, `copyUserFiles` 449-ff, `setupPlayerAudio` 583-), `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt` (`VideoFilters`). The player itself is area:45-player-core; this file fixes only the preference-to-mpv contract so area:45-player-core and area:48-settings-player-and-scripts do not re-derive it. Linux uses `libmpv` through the render API into a `GtkGLArea` (requirement addendum), options set with `mpv_set_option_string` before `mpv_initialize` (Kotlin `setOptionString`) and properties with `mpv_set_property_string` after (Kotlin `setPropertyString`).

## 1. Initialisation order (OBSERVED, `PlayerActivity.setupPlayerMPV` then `AniyomiMPVView.initOptions`)

1. Log level for the library: `"info"` when `verbose_logging` else `"warn"` (passed to `player.initialize(logLvl)`); mpv `msg-level` option `all=v` when verbose else `all=warn`.
2. `mpv.conf` and `input.conf` are rewritten from the preferences `pref_mpv_conf` and `pref_mpv_input` on every player start (file replaced with the preference text, even when empty) in the mpv config dir.
3. `copyUserFiles`: the `scripts`, `script-opts` and `shaders` subfolders of the mpv config dir are deleted on every start; if `mpv_scripts` is true the user's folders (storage manager scripts/script-opts/shaders directories) are copied in file by file. Then bundled assets and fonts are copied (`sub-fonts-dir` and `osd-fonts-dir` properties point at the fonts folder).
4. `sub-ass-force-margins=yes`, `sub-use-margins=yes` options.
5. `initOptions` (below) runs before `mpv_initialize`; `observeProperties` registers the observed property set; `postInitOptions` toggles the statistics page.

Proposal (Linux): the mpv config dir is `$XDG_CONFIG_HOME/kurobayomi/mpv` (via `Paths`). Keep rule 2 and 3 exactly (preferences are the source of truth for `mpv.conf` and `input.conf`; user script folders are copied only when `mpv_scripts` is on; if the user prefers editing files directly they leave `pref_mpv_conf` empty and put files in the `user/` folder which is never wiped: new folder `$XDG_CONFIG_HOME/kurobayomi/mpv-user/{scripts,script-opts,shaders,fonts}` is the copy source, replacing Android's storage-manager folders).

## 2. Preference to option table

| Preference (key) | Kotlin application | mpv name and value | Linux notes |
|---|---|---|---|
| `pref_gpu_next` (false) | `setVo(if (gpuNext) "gpu-next" else "gpu")` | `vo` = `gpu-next` or `gpu` | With the render API the VO is chosen by libmpv (`MPV_RENDER_PARAM_API_TYPE = opengl`); area:45-player-core decides whether `gpu-next` can be honoured and otherwise ignores the toggle (value kept). |
| `pref_try_hwdec` (true) | `setOptionString("hwdec", if (it) "auto" else "no")` | `hwdec` = `auto` / `no` | Proposal: `auto-safe` instead of `auto` on Linux (mpv documents it as the recommended safe list: vaapi, nvdec, ...). Property `hwdec-current` is observed to show the active decoder. |
| `pref_video_debanding` (None) | CPU: `vf=gradfun=radius=12`; GPU: `deband=yes` | `vf` / `deband` | Combine with YUV420P as in quirk Q9. |
| `use_yuv420p` (true) | `setOptionString("vf", "format=yuv420p")` | `vf` | Needed on Android for MediaCodec copy paths; on Linux with vaapi/gpu rendering forcing yuv420p costs a conversion. Proposal: keep the toggle and the default (true) for parity but have area:45-player-core apply it only when `hwdec` ends with `-copy`. |
| `pref_player_filter_brightness`, `_saturation`, `_contrast`, `_gamma`, `_hue` (0) | `VideoFilters.entries.forEach { setOptionString(it.mpvProperty, pref.get().toString()) }` | `brightness`, `saturation`, `contrast`, `gamma`, `hue`, each -100..100 | Same names on Linux. Live changes use `set_property`. |
| `pref_player_speed` (1.0) | `setOptionString("speed", ...)` | `speed` | persisted last speed; reset by user on new file only if the player UI says so (area:46-player-controls-ui). |
| `pref_audio_lang` ("") | `setOptionString("alang", value)` (raw string) and also `TrackSelect(subtitle = false)` for the audio track list | `alang` = comma list e.g. `ja,en` | A migration (`mihon/core/migration/migrations/PrefLangMigration.kt`, version 130) removed entries that are not valid language codes. |
| `pref_audio_delay` (0, ms) | `audio-delay` = ms / 1000.0 | `audio-delay` seconds as decimal string | Division is floating: `(ms / 1000.0).toString()` gives `0.0`, `-0.25` etc. |
| `pref_audio_pitch_correction` (true) | `audio-pitch-correction` | `yes` / `no` (Kotlin passes `true`/`false` strings; mpv accepts both) | |
| `pref_audio_volume_boost_cap` (30) | `volume-max` = cap + 100 | `volume-max` | cap 30 gives 130 |
| `pref_audio_config` (AutoSafe) | `setPropertyString(it.property, it.value)` after init (`PlayerActivity.kt:586`) | Auto -> `audio-channels=auto-safe`; AutoSafe -> `audio-channels=auto`; Mono -> `audio-channels=mono`; Stereo -> `audio-channels=stereo`; ReverseStereo -> `af=pan=[stereo|c0=c1|c1=c0]` | When switching away from ReverseStereo the `af` property must be cleared (`af=` empty) by the UI; Kotlin relies on the player being re-created. |
| `pref_subtitle_lang` ("") | not given to mpv; consumed by `TrackSelect.getPreferredTrackIndex(tracks, subtitle = true)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/TrackSelect.kt`), algorithm in section 4 | none | comma separated language codes |
| `pref_subtitle_whitelist`, `pref_subtitle_blacklist` ("") | same `TrackSelect` algorithm | none | comma separated substrings matched case-insensitively against the subtitle track name |
| `pref_subtitles_delay` (0 ms) | `sub-delay` = ms / 1000.0 | `sub-delay` | |
| `pref_subtitles_secondary_delay` (0 ms) | `secondary-sub-delay` = ms / 1000.0 | `secondary-sub-delay` | |
| `pref_subtitles_speed` (1.0) | `sub-speed` | `sub-speed` | |
| `pref_subtitle_font` ("Sans Serif") | `sub-font` | `sub-font` | On Linux fontconfig resolves the family; default "Sans Serif" maps to the fontconfig `sans-serif` alias. |
| `pref_override_subtitles_ass` (false) | when true: `sub-ass-override=force`, `sub-ass-justify=yes` | same | no option set when false (mpv defaults) |
| `pref_subtitles_font_size` (55) | `sub-font-size` | `sub-font-size` (scaled pixels at 720p) | |
| `pref_bold_subtitles` / `pref_italic_subtitles` (false) | `sub-bold`, `sub-italic` = `yes`/`no` | same | |
| `pref_sub_justify` (Auto) | `sub-justify` = `left`/`center`/`right`/`auto` | same | |
| `pref_text_color_subtitles` (-1 = 0xFFFFFFFF) | `sub-color` = `"#" + hex8.uppercase()` | `sub-color` `#AARRGGBB` | mpv accepts `#AARRGGBB`; Int stored as signed 32-bit: format `format!("#{:08X}", v as u32)` |
| `pref_background_color_subtitles` (0 = transparent) | `sub-back-color` | `sub-back-color` `#AARRGGBB` | |
| `pref_border_color_subtitles` (-16777216 = 0xFF000000) | `sub-border-color` | `sub-border-color` | |
| `pref_sub_border_size` (3) | `sub-border-size` | `sub-border-size` | |
| `pref_border_style_subtitles` (OutlineAndShadow) | `sub-border-style` | `outline-and-shadow` / `opaque-box` / `background-box` | |
| `sub_shadow_offset` (0) | `sub-shadow-offset` | `sub-shadow-offset` | |
| `pref_sub_pos` (100) | `sub-pos` | `sub-pos` | |
| `pref_sub_scale` (1.0) | `sub-scale` | `sub-scale` | |
| `pref_player_statistics_page` (0) | after init: if != 0: `script-binding stats/display-stats-toggle`, then `script-binding stats/display-page-N` | commands | |
| `pref_gesture_*`, `pref_*_double_tap`, `pref_media_*` | handled in Kotlin UI; `SingleActionGesture.Custom` sends mpv key names `0x10001`..`0x10006` via `keypress`/`keydown` (see `CustomKeyCodes`) | | Desktop: double-click zones and media keys call the same handler (area:46-player-controls-ui). |
| `pref_progress_preference` (0.85) | seen when `lastSecondSeen >= totalSeconds * progress` (`ExternalIntents.kt:467-469`, `PlayerViewModel.kt:1672`) | none | Strictly: `>=`, only when `totalSeconds > 0`, only if the episode is not already seen (a seen episode stays seen), and nothing is written when `incognito_mode` is on. |
| `pref_preserve_watching_position` (false) | start position when opening an episode that is already seen: `0` unless this pref is true (`PlayerActivity.kt:1074-1082`); otherwise `episode.last_second_seen`; command `set start <seconds as float>` (`resumePosition / 1000F` since the stored value is milliseconds there) | `start` | |

Fixed options set in `initOptions` that are not preferences (listed so area:45-player-core does not forget them): `profile=fast`, `pause` property true, `keep-open` true, `input-default-bindings` true, `ytdl=no`, `tls-verify=yes`, `tls-ca-file=<files dir>/mpv/cacert.pem` (Linux: use the system trust store via `tls-ca-file=/etc/ssl/certs/ca-certificates.crt`, found with `openssl-probe`-style lookup), `demuxer-max-bytes` and `demuxer-max-back-bytes` = 64 MiB (32 MiB on Android before 8.1), `screenshot-directory` = public Pictures dir (Linux: `xdg-user-dir PICTURES` through `Paths`), `vd-lavc-film-grain=cpu`.

## 3. What reads each preference live

`changes()` collectors in the player UI re-apply on change (the settings sheets call `set_property` directly); `AniyomiMPVView.initOptions` and `PlayerActivity` read once at start. So for `pref_try_hwdec`, `pref_gpu_next`, `use_yuv420p`, `pref_video_debanding` a change takes effect at the next episode open; subtitle and audio style prefs are applied live by the sheets. The Rust player must follow the same split: option-class prefs (those in the `initOptions` table above that use `setOptionString`) apply at next player creation; property-class prefs (sheet-driven) apply immediately.

## 4. Track selection algorithm (OBSERVED, `TrackSelect.kt:14-56`)

Input: the list of tracks (each with `name` and optional `language`), a flag `subtitle`.

1. `prefLangs` = (`pref_subtitle_lang` if subtitle else `pref_audio_lang`) split on `,`, empty strings dropped, each trimmed. `whitelist` and `blacklist` = the subtitle whitelist/blacklist split the same way; both are empty for audio.
2. `locales` = `prefLangs` mapped to `Locale(code)`; if there are none, the single system default locale.
3. `chosenLocale` = the first locale for which some track matches `containsLang(track, locale)`; if no locale matches any track, the result is `null` (no automatic selection).
4. `filtered` = tracks (in original order) that do NOT match any blacklist entry (`track.name.contains(entry, ignoreCase = true)`) AND match `containsLang(track, chosenLocale)`.
5. Result = the first track in `filtered` whose name contains any whitelist entry (case-insensitive); if none, the first track in `filtered`; if `filtered` is empty, `null`.
6. `containsLang(track, locale)`: `localName = locale.getDisplayName(locale)`; `englishName = locale.getDisplayName(ENGLISH).substringBefore(" (")`; regex `\b<iso3>|<iso2>\b` (case-insensitive; because of alternation precedence this is `(\b<iso3>)|(<iso2>\b)`); true if the track name contains `localName` or `englishName` (case-insensitive) or the track's `language` field matches the regex.

Proposal: implement in `kuro-player` (area:45-player-core) with the `isolang` crate for ISO 639-1/639-3 names and display names (English names only; native names are optional), keep the regex precedence quirk (so "ja" matches "ja", "Japanese", "jpn" at word start). The default-locale fallback uses `sys-locale`. Unit tests: T-02a-015.
