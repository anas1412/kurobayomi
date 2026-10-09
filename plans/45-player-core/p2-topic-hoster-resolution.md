# Topic: hoster and video resolution (45-player-core part 2)

Deep dive for the chain that turns an episode into a playable URL. Full algorithms: `p2-behavior.md` §3 and §4. Edge cases: E-32 to E-40. Tasks: T-45b-007 to T-45b-015, T-45b-063, T-45b-064, T-45b-066.

## 1. The model in one picture

```
Episode
  └─ EpisodeLoader.getHosters
       ├─ downloaded?  -> [Hoster{ videoList = [buildVideo] }]         (no network)
       ├─ AnimeHttpSource -> getHosterList (sorted)  | legacy getVideoList -> toHosterList
       └─ LocalAnimeSource -> [Hoster{ videoList = [file uri] }]
  Hoster{ lazy?, videoList? }
       ├─ lazy (not loaded)      -> HosterState.Idle   (loaded only on user click)
       ├─ videoList == null      -> HosterState.Loading -> getVideos(source, hoster)
       └─ videoList present      -> HosterState.Ready(videos, all QUEUE)
  Video{ videoUrl == "null" }    -> resolved on use by source.getVideoUrl(video)
  Video{ initialized == false }  -> resolved on use by source.resolveVideo(video)
  Selected Video -> HosterLoader.getResolvedVideo -> Video{initialized = true, videoUrl != ""} -> mpv
```

Two independent lazy steps exist and must not be confused:
1. Hoster laziness (`Hoster.lazy`): whether the hoster's video list is fetched at all.
2. Video laziness (`videoUrl == "null"` or `initialized == false`): whether the playable URL needs a second call.

## 2. State machine per hoster slot (`HosterState`)

| From | Event | To | Source |
|---|---|---|---|
| (start) | hoster.lazy | Idle | PlayerViewModel.kt:1378 |
| (start) | videoList == null | Loading | :1380 |
| (start) | videoList present | Ready(QUEUE…) | :1383-1388 |
| Loading | loadHosterVideos ok | Ready | EpisodeLoader.kt:194-197 |
| Loading | loadHosterVideos throws (non-cancel) | Error | :200-202 |
| Idle | user click (force=true) | Loading → Ready/Error | PlayerViewModel.kt:1563-1574 |
| Ready | user click | accordion toggle only | :1560-1562 |
| any | cancel job | Idle (all hosters reset) | :1450-1452 |

Per video slot (`Video.State`):

| From | Event | To |
|---|---|---|
| QUEUE | selected for loading | LOAD_VIDEO |
| LOAD_VIDEO | resolved non-empty URL | READY (video replaced with resolved copy) |
| LOAD_VIDEO | resolve null or empty URL | ERROR |
| ERROR | user click | ignored (PlayerViewModel.kt:1542) |

## 3. Selection rules in one table

| Question | Rule | Where |
|---|---|---|
| Which video to try first after load? | Saved (hosterIndex, videoIndex) if present; else first `preferred` video in the first hoster that has one; else `selectBestVideo` | PlayerViewModel.kt:1401-1446 |
| What counts as selectable? | video state READY or QUEUE; preferred flag, or non-empty `videoUrl` (`"null"` counts as non-empty) | HosterLoader.kt:32-50 |
| What if a preferred video is in ERROR? | skipped, next preferred or first valid | E-35 |
| When does loading "give up"? | when `selectBestVideo` returns (-1,-1) and no hoster is still Loading → throw "No available videos" | PlayerViewModel.kt:1488-1496 |
| When does loading "wait"? | when no selectable video but a hoster is still Loading → selection reset, return false, no error | :1490-1492 |

## 4. Concurrency (desktop plan)

- One `load_hosters` task per call, with its own `CancellationToken`; per-hoster child tasks spawned on tokio.
- The "first preferred wins" flag is an `AtomicBool` compare-and-set, the same as the Kotlin `AtomicBoolean`.
- A session-level `Mutex` serialises `load_video` (E-38). User selection cancels the in-flight load token before starting a new one (E-40).
- `get_best_video` (external-player path) uses a `JoinSet`. The first resolved non-empty preferred video wins; siblings are aborted. Fallback is sequential over `select_best_video`.

## 5. Edge cases that matter in practice

- A source that returns the literal string `"null"` as a real URL is treated as lazy and resolved again (E-32). The Rust type uses `Option<String>` internally so the sentinel is not overloaded.
- Downloaded episodes never call the network; a deleted download file falls through to "Hoster list is empty" because `buildVideo` errors are swallowed (E-34 analogue, behavior §3.4).
- `initialized` is set on the returned copy even for non-HTTP sources (behavior §3.10). The port keeps this so that the "resolved once" invariant holds everywhere.
- The mpv handle is paused before resolution (`pause()` in `loadVideo`, behavior §4.2 step 5). Resolution failure leaves playback paused, which is the Kotlin behaviour.

## 6. Tests (see E doc §4.1, §4.12)

- T1-T5 `select_best_video` table.
- T63-T68 `load_hosters` and `load_video` with fakes: saved selection, saved selection failing, no videos, loading defers, cancel resets, single in flight.
- T46/T47 are not part of this topic (volume).

## 7. Open decisions

- Whether ERROR hosters become retryable (E-08). Default in the port: retry allowed behind flag `hoster_retry`, off, so the Kotlin behaviour remains the default.
- Whether an "Off" entry exists for subtitles (E-31). Unrelated to resolution but decided in the same settings pass.
