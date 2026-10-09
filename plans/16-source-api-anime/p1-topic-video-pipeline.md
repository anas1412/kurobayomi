# Topic: from episode to playable video (hoster, video, resolve, pick)

This is the one path every anime playback and every anime download goes through. It crosses the source API (this area) and the player loader (`app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/`). Read with p1-behavior.md sections 3 to 8 and p1-rust-design.md sections 3.4 and 5.

---

## 1. Data shapes

```
Episode (app) ──toSEpisode()──> SEpisode
                                  │
          ┌───────────────────────┴───────────────────────┐
          │ provides_hosters() == true                    │ false (legacy)
          ▼                                               ▼
  hosters(episode) -> [Hoster]                    videos_for_episode(episode) -> [Video]
          │                                               │ sort_videos()
          │ sort_hosters()                                ▼
          ▼                                        [Hoster::synthetic(videos)]
  [Hoster { video_list: None | Some(..), lazy }]           │
          │                                               │
          └───────────────────────┬───────────────────────┘
                                  ▼
                 load_hoster(hoster)  (lazy? -> Idle)
                                  │  video_list Some -> use it
                                  │  video_list None -> videos_for_hoster(hoster)
                                  ▼
                       sort_videos()  -> HosterState::Ready { videos: [VideoEntry{QUEUE}] }
                                  │
                                  ▼
             best_video(): phase A (parallel, preferred) -> phase B (select_best loop)
                                  │
                                  ▼
             resolve_once(video): resolve_video() unless initialized
                                  │
                                  ▼
                          Video { video_url non-empty, initialized = true }
```

Kotlin names: `EpisodeLoader.getHosters`, `getHostersOnHttp`, `checkHasHosters`, `loadHosterVideos`, `getVideos`, `HosterLoader.getBestVideo`, `selectBestVideo`, `getResolvedVideo`. Source: `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt`, `HosterLoader.kt`.

---

## 2. Decision table: which discovery path runs

| Condition | Path | Hoster result | Source calls |
|---|---|---|---|
| Episode is downloaded | downloaded | one hoster from `DownloadManager.buildVideo` (any Throwable → empty) | none |
| Source is local (`LocalAnimeSource`) | local | one hoster with `Video(uri, "Local source: <episode.url>")`; any exception → empty | none |
| Source is `AnimeHttpSource` and `checkHasHosters` true | http-hosters | `getHosterList` result, `sortHosters` | `getHosterList(episode)` |
| Source is `AnimeHttpSource` and `checkHasHosters` false | http-legacy | `getVideoList(episode)`, `sortVideos`, wrapped | `getVideoList(episode)` |
| Source is neither | error | `error("source not supported")` | none |

`checkHasHosters` (Kotlin, OBSERVED): true only if a class below `AnimeHttpSource`/`ParsedAnimeHttpSource` in the hierarchy declares `getHosterList`, `hosterListRequest` or `hosterListParse`. Rust: explicit `provides_hosters()` (E-01).

---

## 3. Hoster load (`loadHosterVideos`)

Input: `(source, hoster, force = false)`.

1. `if !force && hoster.lazy` → `HosterState.Idle(name)`. Stop.
2. Otherwise compute the video list:
   - `hoster.videoList != null` and source is HTTP → `parseVideoUrls` (legacy `"null"` fix-up, dropped in Rust) → sort.
   - `hoster.videoList != null` and source is not HTTP → as is, no sort.
   - `videoList == null` and HTTP → `getVideoList(hoster)` → `parseVideoUrls` → sort.
   - otherwise → `error("source not supported")` (thrown; caught below).
3. Success → `Ready(name, videos, QUEUE × n)`.
4. Failure (non-cancellation) → `Error(name)`. Cancellation rethrown.

Timing: none in this code. Requests use the default client timeouts (area:12).

Rust (`load_hoster`): step 2 always sorts (default sort is identity, so the result matches for sources that do not override it). Errors map to `Error` except `Cancelled`.

---

## 4. Selection (`selectBestVideo`)

Input: list of `HosterState`. Only `Ready` hosters are candidates. Pairs `(video, state)`.

Tier 1 (preferred): `video.preferred && state ∈ {READY, QUEUE}`. First hoster with any such pair; first such video in it.

Tier 2 (URL): `video.videoUrl != "" && state ∈ {READY, QUEUE}`. First hoster with any such pair; first such video.

Else none.

Worked example (ordering matters):
| hoster | videos (url, preferred, state) |
|---|---|
| 0 "A" (Ready) | (`""`, false, QUEUE), (`https://a/1.m3u8`, false, QUEUE) |
| 1 "B" (Ready) | (`https://b/1.m3u8`, true, ERROR), (`https://b/2.m3u8`, false, READY) |
| 2 "C" (Error) | — |

Tier 1: video `B.0` is preferred but ERROR → not eligible. No tier-1 match.
Tier 2: hoster 0 video 1 (`https://a/1.m3u8`, QUEUE) is the first valid. Result `(0, 1)`.

If `B.0` were QUEUE instead, tier 1 returns `(1, 0)`.

---

## 5. Best video (`getBestVideo`)

Phase A (parallel, preferred only):
- For each hoster concurrently: load (step 3), then if `Ready` and there is a video with `preferred && !initialized` at index `p`: mark it `LOAD_VIDEO`, resolve it (section 6). On a non-empty URL, cancel all siblings and return it. On failure mark `ERROR` and continue.
- Only index `p` (the first such video) is probed per hoster.

Phase B (sequential):
- Loop: `select_best(states)`; if none → return null. Mark that video `LOAD_VIDEO`, resolve; success → return; failure → mark `ERROR`, repeat.

Important ordering rule: phase A can win with a preferred video from a later hoster even if an earlier hoster has a non-preferred valid URL. Phase B only runs when phase A finds nothing. This is the "preferred beats order" rule.

Cancellation: any `CancellationException` aborts the whole call (rethrown by `getResolvedVideo`).

---

## 6. Resolution (`getResolvedVideo`)

```
if source is HTTP and !video.initialized:
    try   r = source.resolveVideo(video)        // default: identity
    catch CancellationException -> rethrow
    catch Exception             -> r = null
else:
    r = video
return r?.copy(initialized = true)
```

Consequences:
- A source that returns the same video from `resolveVideo` still gets `initialized = true`.
- A non-HTTP source's videos are never resolved and are still marked initialized in the returned copy.
- Resolution happens once per video object. A video returned from phase A that already has `initialized = true` is not re-resolved.

Rust (`resolve_once`): same rules; the empty-URL filter is applied after resolution (E-35).

---

## 7. Sorting rule summary

| Level | Default | Override effect |
|---|---|---|
| `sortHosters` | identity | source-defined order = priority (tier 2 picks first hoster) |
| `sortVideos` | identity (`sort()` returns `this`) | source-defined order = priority within a hoster |

No quality sort anywhere in the app for anime. A source that wants 1080p first must sort in `sort_videos`.

---

## 8. Worked end-to-end (Kotlin behavior, legacy source)

A legacy extension (`hosterListParse` not overridden) returns for episode E:
`getVideoList(E) = [Video(url="page1", quality="720p", videoUrl=null), Video(url="page2", quality="1080p", videoUrl="https://cdn/x.m3u8")]`

1. `checkHasHosters` → false → `videoList = [...]`, `sortVideos` → identity → `toHosterList()` → `[Hoster(name="no_hoster_list", videoList=[...])]`.
2. `loadHosterVideos`: `videoList != null` and HTTP → `parseVideoUrls`: video 0 has `videoUrl == "null"` → `getVideoUrl(video0)` → `videoUrlRequest(video0)` = `GET("page1")` → `videoUrlParse(response)` → URL string → `copy(videoUrl = <resolved>)`. Video 1 untouched.
3. `Ready` with two QUEUE videos. Neither is preferred.
4. Phase B: tier 2 → video 0 (now has a URL) → `resolve_video` (default identity) → marked `initialized`.

Rust equivalent: the legacy step 2 is not ported (no APK sources). Built-in sources implement `videos_for_episode` and return final URLs, so step 2 is skipped and the result is video 0 `https://cdn/x.m3u8` only if that source emits it. This is the intentional behavior change (drop T-16a-015).

---

## 9. Timing and concurrency summary

| Activity | Concurrency | Bound |
|---|---|---|
| Hoster loads in phase A | one task per hoster | none in code; rate limiter (area:12) |
| Phase B | sequential | — |
| Season fetch (`UnmeteredSource`) | parallel | none for unmetered |
| Season fetch (others) | sequential | `ensureActive()` before each |
| Image tile preload | sequential, first 2 tiles | 2 |

---

## 10. Test cases (must pass)

1. `selects_preferred_over_order` — section 4 example with `B.0` QUEUE → `(1, 0)`.
2. `selects_first_valid_when_no_preferred` — section 4 example → `(0, 1)`.
3. `preferred_error_skipped` — `B.0` ERROR → not chosen in tier 1.
4. `phase_a_first_wins_cancels_rest` — section 5 with two hosters; the second hoster's `videos_for_hoster` sleeps 2 s; result returns in under 1 s.
5. `phase_b_runs_after_phase_a_none` — no preferred anywhere; first valid resolved in phase B.
6. `resolve_empty_is_failure` — `resolve_video` returns `""` → next candidate.
7. `resolve_cancel_aborts` — cancellation inside `resolve_video` aborts the call.
8. `initialized_not_resolved_again` — video with `initialized = true` → resolver call count 0.
9. `lazy_hoster_idle` — `lazy = true`, `force = false` → `Idle`; `force = true` → loads.
10. `legacy_routing_synthetic_hoster` — `provides_hosters() == false` → one hoster named `no_hoster_list` containing all videos in source order.
11. `hoster_error_is_state_not_failure` — `videos_for_hoster` error → `HosterState::Error`, other hosters still selectable.
12. `downloaded_empty_on_error` — download lookup error → empty list (no propagation).
