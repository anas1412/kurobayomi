# Topic: the nine animated vector drawables and their GTK equivalents

Source: `app/src/main/res/drawable/anim_*.xml` (nine files, 2470-line scope total is in the inventory). Driver code: Compose `rememberAnimatedVectorPainter(image, atEnd)` at the call sites listed below. Timelines are exact values from the XML; the behaviour spec is `p1-behavior.md` section 3.

## 1. Driver semantics (Compose, observed)

- `rememberAnimatedVectorPainter(image, atEnd)`: `atEnd == false` shows the base `<vector>`; a change of `atEnd` plays every `<target>` objectAnimator forward (false to true) or backward (true to false). The end state is each objectAnimator's `valueTo`.
- Each objectAnimator runs for its own `android:duration` with its own `android:interpolator`. `<set>` children start together unless `android:ordering="sequentially"`; the default is `together`. `android:startOffset` delays a child inside its set.
- Interpolators used by the scope: `@android:interpolator/fast_out_slow_in` (cubic-bezier 0.4, 0, 0.2, 1), `@android:anim/decelerate_interpolator`, and a `<pathInterpolator android:pathData="M 0.0,0.0 c0.5,0 0.5,1 1.0,1.0"/>` (ease-in-out, used only by `anim_more_enter`, which is dropped).

## 2. Call sites and driver expressions (observed)

| Drawable | Call site (repo-relative) | `atEnd` expression |
|---|---|---|
| anim_animelibrary_leave | app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt:78-83 | `isSelected` |
| anim_animelibrary_enter | (no direct call; used by `ic_animelibrary_selector_24dp.xml` transition from checked to normal) | state of the selector |
| anim_library_enter | app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt:77, 82 | `isSelected` (`current.key == key`) |
| anim_browse_enter | app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt:43, 47 | `isSelected` (`current is BrowseTab`) |
| anim_history_enter | app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt:35, 44; app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt:84, 88 | `isSelected` |
| anim_more_enter | app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt:49, 53 | `isSelected` |
| anim_updates_enter | app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt:30, 40; app/src/main/java/eu/kanade/tachiyomi/ui/category/CategoriesTab.kt:38, 42; app/src/main/java/eu/kanade/tachiyomi/ui/stats/StatsTab.kt:28, 32; app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt:28, 32 | `isSelected` |
| anim_caret_down | app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt:637-639; app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt:605 | `!expanded` |
| anim_play_to_pause | app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt:86, 113 | `!paused` |

## 3. Per-drawable timelines and GTK replacements

### 3.1 anim_animelibrary_enter / anim_animelibrary_leave (39 and 37 lines)
- Target `playbutt`: one objectAnimator, `propertyName="pathData"`, `valueType="pathType"`, duration 300 ms, `fast_out_slow_in`.
- Leave is the same animator with `valueFrom` and `valueTo` swapped (verified: enter `valueFrom` equals leave `valueTo`'s start).
- GTK: no morph. Two SVGs (`kb-animelibrary-outline-symbolic` and `kb-animelibrary-filled-symbolic`) in a `gtk::Stack` with `Crossfade`, `transition-duration` 300. The selector state is the toggle's checked state.

### 3.2 anim_library_enter (31 lines)
- Viewport 1080. Target `bookmark`: pathData 300 ms, `fast_out_slow_in`. Start path is the bookmark stack; end path is a shifted bookmark with a notch.
- GTK: crossfade between `kb-library-symbolic` (static) and nothing (the end state is decorative). Equivalent: a single static icon that scales 1.0 to 1.0 (no animation). Keep the static icon; the morph is decorative and is not reproduced (P2, T-49ba-022).

### 3.3 anim_browse_enter (104 lines)
- Groups: `main_parent` (pivot 12,12) contains `group` (pivot 12,12, holds `dot`), `dot_mask` (clip-path), `compass`, `ring_parent` (pivot 12,12, scale 0.833, holds `ring`), `group_1` (empty).
- Target `compass`: pathData 300 ms, `fast_out_slow_in` (compass to ring-with-dot).
- Target `main_parent`: `rotation` 0 to 180, 300 ms, `fast_out_slow_in`.
- Target `group` (set, together): `scaleX` 0 to 0 over 66 ms, then 0 to 1 over 100 ms (offset 66, `decelerate_interpolator`); the same for `scaleY`.
- GTK: the Browse icon `kb-compass-symbolic`; selected state `Rotation::Half` (class `kb-rot-180`, transition 300 ms `cubic-bezier(0.4, 0, 0.2, 1)`). The compass-to-ring morph and the dot pulse are dropped (decorative).

### 3.4 anim_history_enter (36 lines)
- Groups `arrow_head` (pivot 13,12) with path `arrow`; `clock` path outside the group.
- Target `arrow_head`: `rotation` 360 to 0, 500 ms, `fast_out_slow_in`.
- GTK: `document-open-recent-symbolic` with `Rotation::FullBackSlow` (class `kb-rot-0-slow`, 500 ms). The clock hand is part of the system icon.

### 3.5 anim_updates_enter (27 lines)
- Target `updates_filled`: pathData 300 ms, `fast_out_slow_in`. Start is a four-point shape; end is the star burst.
- GTK: crossfade between `software-update-available-symbolic` (for the tabs) and `kb-updates-outline-symbolic` (for the More entry). Decorative morph dropped.

### 3.6 anim_more_enter (201 lines) — DROPPED (P3)
- Groups `_R_G`, `_R_G_L_2_G`, `_R_G_L_1_G`, `_R_G_L_0_G` hold three circle paths (leftmost centre x about 270, middle about 540, rightmost about 810).
- Per-dot objectAnimator schedules (ordering together, pathInterpolator ease-in-out):
  - leftmost (`_R_G_L_2_G_D_0_P_0`): 83 ms @0, 100 ms @83, 133 ms @183, 67 ms @317. Total 384 ms (1 ms gap at 316-317).
  - middle (`_R_G_L_1_G_D_0_P_0`): 150 ms @0, 100 ms @150, 133 ms @250, 67 ms @383. Total 450 ms.
  - rightmost (`_R_G_L_0_G_D_0_P_0`): 217 ms @0, 100 ms @217, 133 ms @317, 67 ms @450. Total 517 ms.
- Target `time_group` (empty group): translateX 0 to 1 over 833 ms. No children, so no visible effect.
- GTK: static `view-more-horizontal-symbolic`. Reason for dropping: decorative bounce; the only non-decorative part is nothing.

### 3.7 anim_play_to_pause (44 lines)
- Group `group` (pivot 12,12), path `path`.
- Target `path`: pathData 300 ms, `fast_out_slow_in`. Start: `M 8 5 L 8 12 L 19 12 L 19 12 L 8 5 M 8 12 L 8 19 L 19 12 L 19 12 L 8 12` (triangle). End: `M 5 6 L 5 10 L 19 10 L 19 6 L 5 6 M 5 14 L 5 18 L 19 18 L 19 14 L 5 14` (two bars).
- Target `group`: `rotation` 0 to 90, 300 ms, `fast_out_slow_in`.
- GTK: two `gtk::Image`s in a `gtk::Stack` (`Icon::Play` and `Icon::Pause`), `Crossfade`, 300 ms (`anim::crossfade_to`). Rotation dropped.

### 3.8 anim_caret_down (85 lines)
- Base vector `caret_up` with group `caret01` (rotation 90, translateX 12, translateY 15), arms `caret_l` and `caret_r` (rotation +/-45 pivoted by `caret_*_pivot` with translateY +/-4 and rectangles `caret_*_rect`).
- Target `caret01`: `pathData="M 12.0,9.0 c 0.0,0.66667 0.0,5.0 0.0,6.0"` with `propertyXName=translateX`, `propertyYName=translateY`, 300 ms, `fast_out_slow_in` (moves the group along one cubic from (12,9) to (12,15)).
- Target `caret_l`: rotation -45 to 45, 300 ms. Target `caret_r`: rotation 45 to -45, 300 ms.
- GTK: `pan-down-symbolic` / `pan-up-symbolic` swapped on state change (instant), with optional rotation 180 over 300 ms via `kb-icon-rot` and a rotate class (P2, T-49ba-021). The path translate is dropped.

## 4. CSS for the GTK equivalents

Add to `data/style.css` (the transition timing is the Android `fast_out_slow_in`):

```css
.kb-icon-rot {
  transition: -gtk-icon-transform 300ms cubic-bezier(0.4, 0, 0.2, 1);
}
.kb-icon-rot.kb-rot-180 {
  -gtk-icon-transform: rotate(180deg);
}
.kb-icon-rot.kb-rot-0-slow {
  transition: -gtk-icon-transform 500ms cubic-bezier(0.4, 0, 0.2, 1);
  -gtk-icon-transform: rotate(0deg);
}
```

Verification (T-49ba-022): on GTK 4.16 `-gtk-icon-transform` must apply to `gtk::Image` and animate. If it does not animate, replace the class toggle in `anim::rotate_icon` with a `gtk::DrawingArea` that rotates with cairo and a frame-clock tick over 300 ms (or 500 ms for History). The timing values stay the same.

`decelerate_interpolator` (Browse group scale, dropped with the dot pulse) maps to CSS `ease-out` if it is ever reintroduced.

## 5. Interpolator mapping

| Android | Value | CSS / GTK | Compose |
|---|---|---|---|
| `@android:interpolator/fast_out_slow_in` | cubic-bezier (0.4, 0, 0.2, 1) | `cubic-bezier(0.4, 0, 0.2, 1)` | `FastOutSlowInEasing` (also the default easing of `tween`) |
| `@android:anim/decelerate_interpolator` | 1 - (1 - t)^2 (quadratic ease-out) | CSS `ease-out` = `cubic-bezier(0, 0, 0.58, 1)` (close, not identical) | no exact Compose equivalent; use `CubicBezierEasing(0f, 0f, 0.58f, 1f)` |
| `<pathInterpolator M 0,0 c0.5,0 0.5,1 1,1>` | cubic-bezier (0.5, 0, 0.5, 1) | `cubic-bezier(0.5, 0, 0.5, 1)` | `CubicBezierEasing(0.5f, 0f, 0.5f, 1f)` |
| (no interpolator, `tween` default) | `FastOutSlowInEasing` | `cubic-bezier(0.4, 0, 0.2, 1)` | same |

The double-tap triangles use the Compose default easing (`tween` with no argument), i.e. FastOutSlowIn; `src/anim.rs` `cubic_bezier(0.4, 0.0, 0.2, 1.0, t)` reproduces it.

## 6. Timing summary (exact)

| Animation | Durations (ms) | Interpolator | End state trigger |
|---|---|---|---|
| anim_animelibrary_enter / leave | 300 | fast_out_slow_in | selector state |
| anim_library_enter | 300 | fast_out_slow_in | isSelected |
| anim_browse_enter (compass) | 300 | fast_out_slow_in | isSelected |
| anim_browse_enter (rotation) | 300 | fast_out_slow_in | isSelected |
| anim_browse_enter (group scale) | 66 then 100 (offset 66) | fast_out_slow_in, decelerate | isSelected (dropped) |
| anim_history_enter | 500 | fast_out_slow_in | isSelected |
| anim_updates_enter | 300 | fast_out_slow_in | isSelected |
| anim_more_enter | 83/100/133/67, 150/100/133/67, 217/100/133/67, time_group 833 | pathInterpolator | isSelected (dropped) |
| anim_play_to_pause | 300 (both targets) | fast_out_slow_in | !paused |
| anim_caret_down | 300 (all three targets) | fast_out_slow_in | !expanded |
| DoubleTapSeekTriangles (not an AVD) | 6 steps x 150 | FastOutSlowIn (default tween) | fixed loop while seeking |
| splash exit (not an AVD) | 400 | LinearOutSlowIn (slide), FastOutSlowIn (fade) | splash removal |

## 7. Checks

- T10 (`src/anim.rs`): `seek_alpha` schedule and easing.
- Visual: play/pause crossfade at 30 fps: the two icons overlap for about 300 ms with no black frame.
- Visual: Browse selection rotates the compass 180 degrees in one 300 ms ease; History rotates 360 to 0 over 500 ms.
- Interaction: toggling a caret twice within 300 ms ends in the second target state with no stuck intermediate (edge case A1).
- Reduced motion (T-49ba-046): with `gtk-enable-animations` false, all durations are 0 and the states still switch.
