# Topic: chapters, AniSkip and the skip-intro state machine (45-player-core part 2)

Deep dive for how chapters are assembled and how the skip-intro button, countdown and auto-skip behave. Full algorithms: `p2-behavior.md` §5.6, §6. Edge cases: E-03, E-04, E-05, E-09, E-10, E-19, E-41 to E-45. Tasks: T-45b-018 to T-45b-022, T-45b-069, T-45b-076.

## 1. Where chapters come from

Two sources feed `PlayerViewModel.chapters`:

1. mpv's own chapter list, read on every `chapter-list` event (`loadChapters`, PlayerViewModel.kt:479-494). Each entry is `IndexedSegment(name = title, start = time, index = 0)`.
2. External timestamps (AniSkip, or extension-provided `Video.timestamps`), merged through `ChapterUtils.mergeChapters` (ChapterUtils.kt:138-220).

The merge runs once after the AniSkip lookup during activity setup (PlayerActivity.kt:1296-1305), and the result replaces the chapter list. A later `chapter-list` event overwrites it again (race 4 in the E doc).

Index conventions (keep them, the UI depends on them):
| index | Meaning |
|---|---|
| `>= 0` | mpv chapter number |
| `-2` | external chapter start (carries `ChapterType` and colour) |
| `-1` | external end marker, or synthetic chapter at 0 with empty name |

## 2. AniSkip, end to end

```
init                       -> resolve tracks for anime (getTracks)
  first track only (E-03)  -> MAL id (MAL track) | AniList lookup (non-zero) | none
  duration (whole s)       -> GET skip-times (types ed, mixed-ed, mixed-op, op, recap)
  response found=true      -> TimeStamp list (name "Opening" | "Ending" | "Recap" | "Mixed-op")
  merge with mpv chapters  -> chapters list
  setChapter(pos)          -> current chapter and skip-button state
```

Gating (PlayerActivity.kt:1287-1297): AniSkip runs only when `introSkipEnabled` (default true), `pref_enable_ani_skip` (default false), and not (`pref_disabled_ani_skip_chapters` (default true) AND mpv already has chapters). So by default AniSkip is off and, when enabled, it is skipped for files that already have chapters.

Rust design: `skip::aniskip::resolve_timestamps(deps, anime, episode_number, duration) -> Option<Vec<TimeStamp>>` in `src/player/skip/aniskip.rs`, using the shared `reqwest::Client`. Decode via serde with unknown `skipType` tolerated (E-04). Episode number is truncated to an integer (12.5 → 12, behavior §6.5).

## 3. Merge algorithm, plainly

1. Sort stamps by start.
2. For each stamp: start snaps to 0.0 when it is the first and starts before 1.0 s. Emit a start segment (index -2). Emit an end marker (index -1, name "") at `end` unless the end is within 1 s of the file duration or within 1 s of the next stamp's start.
3. Drop mpv chapters that start within 1 s of an external start.
4. Drop external segments that start within 1 s of a kept mpv chapter (whose index is not -2).
5. Add a synthetic chapter at 0 (index -1, name "") when there are chapters but none starts at 0 (and none of the kept lists has one at 0).
6. Merge, sort by start (stable), and remove an empty-named segment when the segment before it (in the pre-filter list) is also empty-named.

Unit tests T22-T28 pin each rule; T26 and T27 are the subtle ones (E-43, E-45).

## 4. The skip-intro state machine

State: `waiting` (i32, starts at `pref_waiting_time_aniskip` = 5), `default_wait` (5), flags `enabled` (`enable_skip_intro`, default true), `auto_skip` (default false), `netflix` (default false).

Per whole-second tick with the current chapter `c` (behavior §6.8):

```
if !enabled                       -> nothing
if c.kind == Other                -> label = none; waiting = default_wait
else if netflix:
    if waiting == default_wait    -> pill "Skip Intro: Skipping <name> in <waiting> seconds"
    show(c, next_start, waiting)  ; waiting -= 1
else if auto_skip                 -> seek(next_start, text "<name> skipped")
else                              -> label = "Skip <TypeName>"
show(c, next_start, w):
    w > 0   -> label = "don't skip"
    w == 0  -> seek(next_start, text "Skip <name>")
    w == -1 -> label = "Skip <TypeName>"       (user cancelled)
on_skip (user taps):
    if waiting > 0 && netflix     -> waiting = -1 (cancel), return
    else                          -> seek(next_start, "Skip <name>")
```

Walk-through with the defaults, netflix on, one intro chapter starting at t=0 of that chapter:

| tick | value read | label shown | effect |
|---|---|---|---|
| 0 | 5 | "don't skip" (+ toast) | waiting → 4 |
| 1 | 4 | "don't skip" | waiting → 3 |
| 2 | 3 | "don't skip" | waiting → 2 |
| 3 | 2 | "don't skip" | waiting → 1 |
| 4 | 1 | "don't skip" | waiting → 0 |
| 5 | 0 | (seek) | seek to chapter end, waiting → -1 |

A tap during ticks 0-4 sets waiting to -1; the label then reverts to the plain skip label and the countdown never seeks.

Two defects to fix in the port (both in E doc):
- E-09: the counter only resets on an `Other` chapter, so a second intro chapter starts without countdown. Fix: reset when the chapter index changes.
- E-10: the counter also moves on the non-tick `setChapter` calls (PlayerActivity.kt:1304, 1383). Fix: a separate `sync()` entry point that does not decrement.

The Netflix toast text prefix "Skip Intro: " is hard-coded English (PlayerViewModel.kt:2083). Fix: localised template (T-45b-061).

## 5. Chapter selection and current chapter

- `getCurrentChapter(pos)` = last chapter with `start <= pos` (ties go to the first maximal element). Used by `setChapter` and `onSkipIntro`.
- `next_start` = start of the following chapter in the list, or the current position when this is the last chapter (PlayerViewModel.kt:2077, 2141). With the last chapter, a skip seeks to the current position, which is a no-op. Keep it, but the button should be hidden in that case (proposal, UI task T-45b-076).
- `selectChapter(index)` seeks with the exact `absolute` flag regardless of the smooth-seek pref (E-25).

## 6. Labels and strings

| Chapter type | Label key (base text) |
|---|---|
| Opening | `player_chapter_type_opening` ("Opening") |
| Ending | `player_chapter_type_ending` ("Ending") |
| Recap | `player_chapter_type_recap` ("Recap") |
| MixedOp | `player_chapter_type_mixedop` ("Mixed opening") |
| Other | none |

Skip button text: `player_skip_action` with the type label; countdown text: `player_aniskip_dontskip`; seek text after skip: `player_aniskip_skip` (chapter name), `player_intro_skipped` (auto skip). AniSkip's own `getString()` names ("Opening", "Ending", "Recap", "Mixed-op") are used as chapter names, not as UI labels.

## 7. Tests (E doc §4.5-4.7)

- T22-T28 merge rules.
- T29-T35 state machine including the walk-through above, cancel, per-second vs per-call (T31), reset on chapter change (T32), manual label (T33), auto-skip (T34), other-chapter clear (T35).
- T36-T40 AniSkip decode, not-found, unknown skip type, AniList zero, first-tracker selection.

## 8. Open decisions

- Keep AniSkip off by default (Kotlin default) or on when a MAL/AniList tracker is linked. Default in the port: off, unchanged.
- Whether `pip_replace_with_previous` and similar behaviour should follow the chapter model. Not decided; not part of this topic.
