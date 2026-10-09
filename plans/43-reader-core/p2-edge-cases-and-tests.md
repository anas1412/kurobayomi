# 43-reader-core (part 2 of 2) — Edge cases, failure modes, tests

Repo root: `/home/blackbox/aniyomi-src`. Behavior references point to `p2-behavior.md` (§n) and `p2-topic-*.md`.

Contents
1. Upstream tests to port (what exists, what they assert)
2. Test IDs and what each proves (E-01 to E-43, including the b/c/d/e sub-IDs)
3. Edge cases by file
4. Failure modes and recovery
5. Race conditions and ordering
6. Decisions required (user or owner)
7. Fixtures
8. Test layout in Kurobayomi

---

## 1. Upstream tests to port

Upstream unit tests exist for the chapter-gap domain function this scope wraps. No `*Test.kt` file exists for the in-scope viewer, preference or navigation classes (grep over `app/src/test` and `domain/src/test` found none that reference `ReaderPreferences`, `ViewerNavigation`, `ReadingMode`, `GestureDetectorWithLongTap` or `ReaderPageImageView`).

### 1.1 `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt`
Relevant test cases (JUnit 5 + kotest matchers, concurrent execution):
| Test name (verbatim) | Assertions | Port to |
|---|---|---|
| `calculateChapterGap returns difference` | `calculateChapterGap(chapter(10.0), chapter(9.0)) == 0f`; `(10.0, 8.0) == 1f`; `(10.0, 8.5) == 1f`; `(10.0, 1.1) == 8f`; same for raw Doubles `(10.0,9.0)=0`, `(10.0,8.0)=1`, `(10.0,8.5)=1`, `(10.0,1.1)=8` | E-28 |
| `calculateChapterGap returns 0 if either are not valid chapter numbers` | `(chapter(-1.0), chapter(10.0)) == 0`; `(chapter(99.0), chapter(-1.0)) == 0`; raw `(-1.0, 10.0) == 0`; `(99.0, -1.0) == 0` | E-28 |

Note: the upstream suite does not test the case where both numbers share the same integer part (e.g. 5.0 and 5.5) and therefore does not pin the -1 result (E-29 adds it).

### 1.2 Tests in the same file for episodes (`calculateEpisodeGap`) are the anime twin; they belong to area 16/07 and are not ported here.

## 2. Test IDs

Each test is one row. "Unit" = pure Rust test (no GTK). "Widget" = GTK test under `gtk::init()` with a headless display (Xvfb or `GDK_BACKEND=broadway` or wayland headless) and marked `#[ignore]` by default.

### 2.1 Page zoom model (unit, `src/reader/page_zoom.rs`)
| ID | Test | Given | Expect |
|---|---|---|---|
| E-01 | min_scale per type | view 800×600, src 1000×2000: FitScreen, Stretch, FitWidth, FitHeight, OriginalSize, SmartFit | FitScreen = min(0.8, 0.3) = 0.3; Stretch = max = 0.8; FitWidth = 0.8; FitHeight = 0.3; Original = 1.0; SmartFit: src_h > src_w → fit width = 0.8 |
| E-01b | SmartFit landscape branch | view 800×600, src 2000×1000 | src_h > src_w is false → fit height = 0.6 |
| E-02 | clamp_scale bounds | min 0.3; inputs 0.1, 0.3, 1.0, 1.6, 9.0 | 0.3, 0.3, 1.0, 1.5, 1.5 (max = 5 × 0.3) |
| E-03 | zoom start anchors | ZoomStart Left/Right/Center at ready, src 1000×2000, zoom 0.3 | Left center (0,0); Right (1000,0); Center (500,1000) |
| E-04 | landscape_zoom_applies: five terms | cfg true/false; kind FitScreen vs FitWidth; src landscape vs portrait; zoom == min vs zoom > min | true only when all five hold |
| E-04b | landscape delay race | precondition true at T0, user zooms at T0+200 ms | Kotlin still animates at T0+500 (no re-check). Rust must match (documented), or add a guard — decision D-3 |
| E-05 | RectN.contains half-open | rect [0.0,0.5)×[0,1); points x=0.0 (in), x=0.5 (out), x=0.4999 (in), empty rect (out) | as stated |
| E-06 | needs_tiling boundary | threshold 2048; (2048,100) false; (2049,100) true; (100,2049) true | as stated |
| E-07 | DecodeError paths | truncated JPEG, unknown magic, zero-byte input | `Err(Corrupt)`, `Err(UnsupportedFormat)`, `Err(Corrupt)`; widget → LoadState::Error |
| E-12 | pan clamp | src 1000×2000, zoom 0.3 (fits), view 800×600 | center forced to src center on both axes (image smaller than view); at zoom 2.0 center clamped inside edges |
| E-13 | can_pan thresholds | remaining 1.0 px | false (strict > 1); 1.5 px true |
| E-14 | double-tap toggle | non-animated at fit → double tap → 2×min; again → min. Animated levels: 1 → 2 → 1 | as stated |
| E-16 | system animation factor | gtk-enable-animations false, zoom_duration 500 | duration 1 ms (min), state set instantly |
| E-17 | Stretch = crop-to-fill | stored 2, fit with src 1000×2000 in 800×600 | min scale = max(0.8, 0.3) = 0.8 (crop), not non-uniform stretch (decision D-1) |
| E-18 | invalid scale type int | stored 0, 7, 99 | Kotlin throws IllegalArgumentException (for 0/99; 7 is CUSTOM and valid). Rust: FitScreen for 0 and 99, CUSTOM treated as FitScreen (decision D-2) |

### 2.2 Prefs and enums (unit, `src/reader/prefs.rs`, `enums.rs`, `mode.rs`)
| ID | Test | Given | Expect |
|---|---|---|---|
| E-19 | enum unknown name → default | stored "WHITEBLACK" for flash mode | BLACK (default), no panic |
| E-19b | enum round-trip names | FlashColor, TappingInvertMode, ReaderHideThreshold | encode(name) == Kotlin enum name, decode(encode) == v |
| E-19c | int invalid → default + row deleted | stored "abc" under an int key | get() == default, row removed (mirrors AndroidPreference ClassCastException path) |
| E-19d | bool encoding | "true", "false", "1", "" | true, false, default, default |
| E-20 | ReadingMode from_stored | None, 0, 1..5, 6, 7, -1 | Default for None/0/6/7/-1; others the matching variant |
| E-20b | with_reading_mode keeps orientation bits | viewer_flags = 0x38 | 0x3A for RightToLeft (bits 3..5 preserved, mode bits replaced) |
| E-21 | resolve never returns Default | Default with global RTL | RightToLeft; Default with global Default (corrupt pref) → RightToLeft (fallback to constant) — decision D-4 |
| E-21b | factory on Default | viewer_kind for Default | None; factory returns Err |
| E-22 | ViewerConfig usePageTransitions initial | fresh ViewerConfig with pref true | first snapshot false; after first emission true |
| E-23 | distinct suppression | pref set to same value twice | onChanged called once |
| E-24 | overlay one-shot | first ViewerConfig with pref true; second | first: force true & pref false; second: force false |
| E-25 | invert horizontal | LEFT rect [0,0.33)×[0,1); invert H | rect moves to [0.67,1); tap x=0.1 → MENU; x=0.9 → LEFT |
| E-26 | DisabledNavigation | empty region list | MENU for every point (matches `DisabledNavigation.kt`) |
| E-27 | overlap first-match | two overlapping regions, list order A then B | A wins where both contain the point |
| E-28 | chapter gap upstream cases | see §1.1 | 0, 1, 1, 8, 0, 0 as upstream |
| E-29 | chapter gap same integer part | (5.0, 5.5), (5.9, 5.1) | -1 and -1; caller hides ≤ 0 |
| E-29b | chapter gap null and negative | None either side; chapter number -1 | 0 |

### 2.3 Gestures (unit, `src/reader/gesture.rs`; E-08..E-11 cover the Kotlin state machine)
| ID | Test | Given | Expect |
|---|---|---|---|
| E-08 | press arms | press at t=1000, prior release none | Arm { after_ms = long_press } |
| E-08b | press inside double-tap window does not arm | release at 1000, press at 1200 with double-tap 300 | None (down - lastUp = 200 ≤ 300) |
| E-09 | move beyond slop disarms | press (100,100), move to (109,100) slop 8 | Disarm; move to (107,100) keeps armed |
| E-10 | release disarms; stale timer ignored | press, release, then `fire()` | fire returns false |
| E-11 | pointer-down / cancel disarm | press, then second pointer down | Disarm |

### 2.4 Viewer and page widget (widget, `src/reader/page_view.rs`)
| ID | Test | Given | Expect |
|---|---|---|---|
| E-15 | recycle then set_image | set page, recycle, set another | texture replaced; visible toggles false then true; no leak of previous texture (weak ref count) |
| E-40 | stale decode after close | decode finishes after reader closed (session token mismatch) | no widget touched; no panic |
| E-42 | animated frames in webtoon | is_webtoon true, GIF | no zoom gestures; frames play |
| E-41 | animated double-tap absolute | fit 0.3, animated: double tap | scale 2.0 (absolute), not 0.6 |
| E-43 | webtoon gestures disabled | is_webtoon true | gesture controllers inactive |

### 2.5 Flash, progress, transition, button (unit for logic, widget for drawing)
| ID | Test | Given | Expect |
|---|---|---|---|
| E-29c | flash counter pattern | interval 3, 7 page changes | flashes at calls 1, 4, 7 (1-based) |
| E-30 | interval change resets | calls 1,2 then set_interval(2), call | flashes on the next call (counter reset to 0) |
| E-30b | rapid calls during active flash | active, then flash() again | no restart: one active flag, no second timer |
| E-31 | progress clamp | set_progress(150), set_progress(-5) | 100 and 0 (Rust clamps; Kotlin annotation only; decision D-5) |
| E-32 | transition bind | manga None → cleared; manga local → goingToDownloaded true and no DL lookup called | as stated (mock DownloadIndex counts calls) |
| E-33 | webtoon hide threshold | threshold 31; dy = 31 (no hide), dy = 32 (hide), dy = -32 (hide) | strict `>` comparison |
| E-37 | webtoon side padding display | 0, 10, 25 | "0%", "10%", "25%" with NumberFormat percent rules (Rust: whole-percent formatting) |
| E-36 | brightness range | -76, -75, 0, 100, 101 | clamped to -75..100 in Rust (Kotlin: KDoc range, enforced by slider) |
| E-28b | button retry | error state | retry invokes load; no pager coupling |

### 2.6 Settings screen logic (unit + widget)
| ID | Test | Given | Expect |
|---|---|---|---|
| E-35 | cutout item visibility | hasDisplayCutout false/true, fullscreen on/off | Kotlin rule: shown only when both; Linux: never shown (decision D-6) |
| E-34 | pref changes propagate to viewer | set landscape_zoom false | viewer config updated; open page's landscape auto-zoom not started on next page |
| E-38 | keep-screen-on lifecycle | pref true, open reader, close reader, pref false | inhibit added on open, removed on close; pref false → no inhibit |
| E-39 | volume keys | pref on, XF86AudioRaiseVolume | previous page (non-inverted), next page (inverted) |

## 3. Edge cases by file

### 3.1 ReaderPreferences.kt
- Keys with the same accessor name and different keys: none; every key string is unique (verified by grep). Test: all keys unique (E-19e, a unit test that iterates the catalog).
- `getEnum` silent fallback hides typos in stored names (E-19).
- `webtoonSidePadding` range 0..25 is enforced only by the slider; a stored 99 renders a 99% padding. Rust clamps on read to 0..=25 (decision D-7).
- `customBrightnessValue` range -75..100 same.
- `colorFilterMode` index beyond the list (e.g. 7 on API 28+) falls through the `when`/index lookup in the color filter code (part 1/area 44). Rust: clamp to 0..=2 for the always-available entries and treat 3..=5 as valid (all Linux-available).
- `readerTheme` 0..3 only; 4+ unknown → treated by ReaderActivity (part 1) as default black. Rust: unknown → Black.

### 3.2 ReaderSettingsScreenModel.kt
- `Lazily` sharing: the flows are not stopped; memory is held until the screen model is destroyed (Voyager). Rust: no equivalent state, fine.
- `distinctUntilChanged` on `viewer` and `manga`: a new manga object with identical content still re-renders (reference equality on data class? `Manga` is a data class: equal content → no update). Rust must compare by id and flags, not object identity.

### 3.3 ReadingMode.kt
- `toViewer(DEFAULT)` throws. Caller must resolve first (E-21).
- `CONTINUOUS_VERTICAL` vs `WEBTOON` differ only in `isContinuous` (area 44).
- `MASK` covers 0..7 but only 0..5 are defined.

### 3.4 GestureDetectorWithLongTap.kt
- `lastDownEvent!!` inside the runnable: safe because the runnable is removed on UP/CANCEL; a stale runnable after recycle would throw. Rust: capture values, not the event object.
- Long press fires while the pager is mid-scroll if the finger is still (not moving beyond slop): the consumer must decide (area 44). The tracker itself does not know about scrolling.
- Built-in `onLongPress` is not overridden here; the GTK port must not add a second long-press path.

### 3.5 ReaderPageImageView.kt
- `landscapeZoom` `!!` on `animateScaleAndCenter` and `pan` `!!` on `animateCenter`: both return null when the view is not ready. Rust: return early.
- `result as BitmapImage` (Coil) — cast failure throws. Rust: match and emit error.
- Non-bitmap drawable throws. Rust: decode path returns `DecodeError::UnsupportedFormat`.
- `prepareNonAnimatedImageView` reuses an existing SSIV, so per-load settings must be re-applied on every load (they are: lines 293-296). Rust: apply all per-load settings in `set_image`.
- `setImage` before the previous load finished: Kotlin keeps the previous SSIV bitmap until the new one is ready (SSIV replaces on `setImage`), and the old listener is replaced. Rust: cancel the previous decode job by session generation.
- `recycle()` keeps the child; a `setImage` after recycle works. Rust: same.
- `onPageSelected` when the page never becomes ready: no error callback, listener stays. Rust: keep the same.

### 3.6 ViewerConfig.kt
- `usePageTransitions` default false until the first emission (E-22).
- `forceNavigationOverlay` is written to the preference the first time any viewer is constructed, even if the reader is never shown; the overlay is then never shown again (quirk, E-24).
- Subscriptions are bound to the viewer scope; a destroyed viewer must cancel them (Rust: drop `Subscription`).

### 3.7 ViewerNavigation.kt
- Dead `constantMenuRegion` (E-26 covers the observable behavior).
- `invertMode` set before `regionList` is ready: `getRegions` applies inversion lazily, so the order does not matter.

### 3.8 MissingChapters.kt
- Negative gap (E-29) is possible; `ChapterTransition` hides it, the details screen (area 38) may show it. Rust: one function, callers check > 0; decision D-8 whether details should also hide.

### 3.9 ReaderTransitionView.kt
- `bind` is one-shot; download events do not update it (topic §3).
- Both `isChapterDownloaded` and `isLocal` are cheap but the filesystem check runs on the GTK thread in Rust. Decision: do the check on a blocking task and update the widget when it returns (T-43b-032).

### 3.10 ReaderProgressIndicator.kt
- `setProgress` accepts 0..100 by annotation only; values above 100 render beyond full ring in the Kotlin composable. Rust clamps (E-31).

### 3.11 ReaderButton.kt
- `ACTION_CANCEL` leaves the pager detector disabled (Kotlin defect). Rust does not use the coupling (E-28b).

### 3.12 Viewer.kt
- `destroy()` default is empty: a viewer that holds timers must override; Rust: `Drop` on the widget tree cancels timers via the `Source` handles.

## 4. Failure modes and recovery

| Failure | Kotlin behavior | Rust behavior |
|---|---|---|
| Decode error (corrupt file) | `onImageLoadError` → viewer error UI | `DecodeError::Corrupt`; LoadState::Error; retry button |
| Unsupported image format | Coil/SSIV error → error UI | `UnsupportedFormat`; error UI |
| Out of memory on huge page | SSIV tiles; Coil may OOM | cap texture size (decision D-9); downscale above cap |
| Preference DB write fails | (SharedPreferences apply is async, no error) | keep in-memory value, log, retry on next set |
| Preference invalid type | key deleted, default used | same (E-19c) |
| Display or GL context lost | SSIV re-inits | GTK recreates textures; `PageView` re-uploads from retained bytes (keep `glib::Bytes` for current page) |
| Long press during zoom animation | long press fires (no check) | tracker does not know about zoom; consumer decides; default: ignore long press while a zoom animation runs (decision D-10) |
| Flash while reader closed | host is removed with the activity | `FlashHost` timers removed on widget unmap |

## 5. Race conditions and ordering

1. Decode completion vs page change: the viewer may request page N+1 while N is still decoding. Use a generation counter per `PageView`; drop results whose generation is stale.
2. Landscape zoom 500 ms timer vs user gesture: the timer must be cancelled on any user input that changes zoom or center (Rust), or accepted as Kotlin does (E-04b, decision D-3).
3. Pan animation vs gesture: pan is interruptible; a new gesture cancels the pan animation (SSIV `withInterruptible(true)`).
4. Flash timers vs page change: the active flag prevents restart (E-30b). A timer firing after the widget is unmapped must not touch it (weak reference).
5. Preference change during a load: the page reads config at `set_image` time. Changing `imageScaleType` during a load affects the next load only (Kotlin also: `setMinimumScaleType` is applied at load). Rust: same; on pref change, re-apply to the current page without reloading the texture (improvement; decision D-11).
6. Settings sheet writes and viewer reads happen on the GTK main thread; no lock needed. Persistence is async and never read back by the reader.
7. Chapter gap value and transition bind use snapshots of chapter numbers; a chapter list refresh during transition leaves the old gap until the next bind (acceptable; matches Kotlin).

## 6. Decisions required

| ID | Question | Recommended default |
|---|---|---|
| D-1 | "Stretch" (stored 2) is crop-to-fill in Aniyomi. Keep the behavior (fill with crop) and relabel, or implement a real stretch? | Keep crop-to-fill for compatibility with Aniyomi backups; label it "Fill (crop)" in Kurobayomi and keep the stored value 2 |
| D-2 | Invalid scale-type stored values: throw (Kotlin) or fall back? | Fall back to Fit screen (never crash) |
| D-3 | Landscape auto-zoom after user already zoomed within 500 ms | Cancel the pending auto-zoom on any user zoom/pan gesture (improvement) |
| D-4 | `resolve` when the global default is itself Default (corrupt pref) | Use RIGHT_TO_LEFT (the Kotlin constant default) |
| D-5 | Progress above 100 | Clamp to 100 |
| D-6 | Cutout setting on Linux | Hidden (P3 drop) |
| D-7 | Stored webtoon side padding outside 0..25 | Clamp on read |
| D-8 | Negative chapter gaps on the details screen | Hide (match the reader) |
| D-9 | Maximum texture dimension before downscale | 16384 px on either side (conservative GPU limit; verify at runtime with `gdk::GLContext` where available) |
| D-10 | Long press during an animation | Ignore while an animation runs |
| D-11 | Live re-apply of scale type to the current page | Yes |

## 7. Fixtures

- `fixtures/reader/page_1x1.png`, `page_800x600.jpg`, `page_1000x2000.png` (portrait), `page_2000x1000.png` (landscape), `page_5000x7000.png` (tiling and cap), `anim_3frames.gif` (frame delays 100, 200, 300 ms), `corrupt.jpg` (first 100 bytes of a JPEG), `empty.bin` (0 bytes). Created by the test harness at build time from a seed (no binary files in git: generate with the `image` crate in a test helper).
- `fixtures/prefs/legacy_kv.sql`: rows for each §2 key in the Aniyomi string format, including one invalid int and one invalid enum name.
- `fixtures/chapters/gap_cases.json`: pairs `(higher, lower, expected)` from §1.1 plus (5.0,5.5,-1), (5.9,5.1,-1), (3.0,2.5,0), (-1,4,0).

## 8. Test layout in Kurobayomi

```
src/reader/
  page_zoom.rs        #[cfg(test)] mod tests { E-01..E-04, E-12..E-14, E-16..E-18 }
  nav.rs              #[cfg(test)] mod tests { E-05, E-25..E-27 }
  gesture.rs          #[cfg(test)] mod tests { E-08..E-11 }
  chapter_gap.rs      #[cfg(test)] mod tests { E-28, E-29, E-29b }
  enums.rs / mode.rs / prefs.rs  #[cfg(test)] mod tests { E-19..E-21b, E-22..E-24 }
  flash.rs            #[cfg(test)] mod tests { E-29c, E-30, E-30b }
  page_decode.rs      #[cfg(test)] mod tests { E-06, E-07 }
tests/reader_widgets.rs   #[ignore] widget tests E-15, E-31, E-32, E-33, E-41..E-43 (run with a headless display)
```

Run: `cargo test reader::` for unit tests; `cargo test -- --ignored` with a display for widget tests. No new test framework (plain `#[test]`; `assert_eq!` only).
