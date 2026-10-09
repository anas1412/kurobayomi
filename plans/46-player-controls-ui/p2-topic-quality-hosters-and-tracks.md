# Topic: track selection, hoster and video states, and how the sheets select by identity

Deep dive for `p2-behavior.md` §1-§6 and `p2-tasks.md` T-46b-007, T-46b-010 to T-46b-014. Repo root `/home/blackbox/aniyomi-src`.

## 1. Three identity schemes in use

The sheets identify things in three different ways. The port must keep the meaning of each.

| Thing | Kotlin identity | Where it comes from | Stability |
|---|---|---|---|
| Audio and subtitle track | mpv `track-list` id (`VideoTrack.id: Int`, `-1` = Off) | `PlayerViewModel.kt:473-477` | stable for one file load |
| Chapter | index in the **sorted** list, plus `Segment` data equality for the current chapter | `PlayerViewModel.kt:479-494` | stable for one file load; equality can collide |
| Hoster and video | `hosterIdx: Int` and `videoIdx: Int` (list positions), `Pair<Int, Int>` | `QualitySheet.kt:79-81, 189-191` | **unstable** if the hoster list changes while the sheet is open |

Only the third is a real hazard. The port uses hoster **name** as the key (`no_hoster_list` for flat mode) and video index inside that hoster. Behavior and tests: T-46b-014, edge E55, race R4.

## 2. Track rows: what each sheet shows

Track label rules are in `GenericTracksSheet.kt:108-129` (table in `p2-behavior.md` §1.4). Summary of selection display:

| Sheet | Selection model | Row control | Selected display |
|---|---|---|---|
| Audio (`AudioTracksSheet.kt`) | single id `selectedId: Int` | `RadioButton` | italic + ExtraBold text |
| Subtitles (`SubtitleTracksSheet.kt`) | list of ids, index is the slot (0 primary, 1 secondary) | `Checkbox` | italic + ExtraBold text and `#1` / `#2` suffix |
| Chapters (`ChaptersSheet.kt`) | single `Segment` equality | row click (no control) | italic + ExtraBold text |
| Qualities (`QualitySheet.kt`) | `Pair<hosterIdx, videoIdx>` | row click | color primary + italic + ExtraBold on the video text |

Track selection changes go to the view model (`onSelectSubtitle`, `onSelectAudio`), which owns the mpv writes. The sheets only show state and emit events. Keep that split in Rust: `TrackSheet` emits `TrackEvent::Select(id)`, area:45 applies it.

Subtitle slot rule: the sheet can show at most two `#N` labels because `selectedTracks` is produced by the view model with at most two entries (E43, E88). The port enforces two slots in its own model.

## 3. Hoster lifecycle as the sheet sees it

The sheet never loads anything. Loading is done by the player loader (area:45). The sheet renders the state it is given:

```
Idle --(user taps row)--> Loading --> Ready(videos, states)  or  Error
Ready(videos: 0) -> shown in the "empty" group, only if displayHosters.second
Error -> shown in the "failed" group, only if displayHosters.first
```

Each `Ready` hoster carries `videoState: List<Video.State>` parallel to `videoList`. Video states are `QUEUE`, `LOAD_VIDEO`, `READY`, `ERROR` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:180-185`). The sheet shows:

- `QUEUE`, `READY`: no icon.
- `LOAD_VIDEO`: spinner (2 px stroke) in a 28 px box (flat) or 20 px box (grouped).
- `ERROR`: red error icon filling the width of the box.

`getChangedAt(index, newVideo, newState)` (`QualitySheet.kt:62-72`) is the only mutation helper. It returns a copy with one slot replaced. The port keeps it pure.

## 4. Flat mode versus grouped mode

Condition (`QualitySheet.kt:137-140`):

```
hosterState.size == 1 && hosterState.first().name == Hoster.NO_HOSTER_LIST && hosterState.first() is HosterState.Ready
```

- True: flat list of videos, `noHoster = true` styling (text first, 28 px state icon, vertical padding 8).
- False: grouped hosters, `noHoster = false` styling (state icon first, 20 px icon, vertical padding 4). Any `Idle`, `Loading`, `Error` single-hoster state falls into grouped mode, even when the hoster name is `no_hoster_list`.

Edge: a source that returns exactly one hoster named `no_hoster_list` in `Loading` state uses grouped mode with a spinner row instead of the flat list. The port keeps this rule.

## 5. Grouping order and visibility

`hosterContent` is called up to three times inside one `LazyColumn` (`QualitySheet.kt:207-234`):

1. Valid hosters (Idle, Loading, Ready-with-videos), always.
2. Failed hosters (Error), only when `displayHosters.first` is true.
3. Empty hosters (Ready with zero videos), only when `displayHosters.second` is true.

Within each group the order is the original list order (`withIndex().filter`). The caller (`PlayerSheets`/area:45) decides `displayHosters`; the sheet does not compute it.

## 6. Expansion

- `expandedState: List<Boolean>` is parallel to `hosterState`. `expandedState.getOrNull(i) ?: false` (`QualitySheet.kt:246`).
- Expansion is visible only for `Ready` hosters (`visible = hoster is Ready && isExpanded`, line 257).
- The hoster row click calls `onClickHoster(hosterIdx)`; expansion state is owned by the caller.
- An expanded `Ready` hoster with zero videos renders an empty block (observed, parity).

Desktop proposal (not parity, flagged): a `Ready` hoster with videos auto-expands the first time it becomes Ready and the current selection is inside it. Not in Kotlin; decide with the parity owner.

## 7. Data flow in Rust

```rust
pub enum TrackEvent { Select(i64), AddExternal, OpenDelay }                // audio and subtitles
pub enum QualityEvent { ClickHoster(String), ClickVideo(String, usize) }   // hoster name key
pub struct QualityModel {
    pub is_loading: bool,
    pub hosters: Vec<HosterState>,
    pub expanded: std::collections::HashSet<String>,   // keyed by hoster name
    pub selected: Option<(String, usize)>,             // (hoster name, video index)
    pub show_failed: bool,
    pub show_empty: bool,
}
```

- `expanded` as a set of names replaces `List<Boolean>`. Renaming a hoster collapses it (acceptable; names are stable identifiers from the source).
- `selected` uses the name too (race R4).
- Flat mode is derived, not stored: `flat = hosters.len() == 1 && hosters[0].name() == NO_HOSTER_LIST && matches!(hosters[0], HosterState::Ready { .. })`.

Pure function tests: H1-H3 and the expansion rules in p2-edge-cases-and-tests.md.

## 8. Chapter ordering and identity

- `loadChapters()` sorts by `start` (stable sort). The ordinal is the position after sorting; the sheet shows `#<ordinal + 1>`.
- Clicking a chapter sends its **ordinal** to the seek path (`PlayerSheets.kt`, `onSeekToChapter(chapters.indexOf(it))`). Port: pass the ordinal stored with the row, so a list rebuilt during rendering cannot shift the target (race R9).
- The current chapter is identified by the player's current position (part 1), not by the sheet.

## 9. What is out of scope here

- How hosters are resolved and how videos are fetched: area:16 (source API) and area:45 (loader).
- How the track list is read from mpv (`track-list/N/*`): area:45 (`PlayerViewModel`).
- Who sets `displayHosters` and `expandedState` initially: area:45.

## 10. Open decisions (tracked)

- D1. Hoster click while Loading: ignore (proposal) or allow (Kotlin). Default: allow, parity.
- D2. Auto-expand on Ready: not in Kotlin; default off.
- D3. Empty result text: none in Kotlin; default none (parity). A new string would be needed for any text.
- D4. Subtitle slot promotion when the primary is deselected: the view model decides; the sheet shows whatever it receives.
