# Topic: airing schedule lookup (AniList, MAL via AniList, Simkl)

Source: `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt` (165 lines). Caller: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1536-1543`. Storage: `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:79-83, 310-312` and `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt`.

Behaviour details (step lists, defaults, error paths) are in `p1-behavior.md` section 7. This file gives the decision flow, the data shapes, the bugs, and the test vectors.

## 1. Decision flow

```
loadAiringTime(anime, trackItems, manual)
 │
 ├─ stored = (anime.nextEpisodeToAir, anime.nextEpisodeAiringAt)        decoded from viewerFlags
 ├─ status == COMPLETED(2) and not manual ?  ── yes ──► return stored    (no network)
 │
 ├─ track = first trackItem where tracker ∈ {AniList, MAL, Simkl} and track != null
 │     none ──► return (1, 0)                                            (caller overwrites stored!)
 │
 ├─ AniList  ──► anilist_airing(remoteId)
 ├─ MAL      ──► anilist_airing(mal_to_anilist(remoteId))               mal_to_anilist returns 0 when unknown
 └─ Simkl    ──► simkl_airing(remoteId)
                    for calendar in [anime, tv, movie_release]:
                       fetch (failure ──► return (1, 0))
                       if target.mal != null ──► return anilist_airing(mal_to_anilist(target.mal))
                       episode, date = target entry in calendar
                       if date is in the future ──► return (episode, date)
                    return (last episode seen or 1, 0)
```

Caller (after the function returns): `setAnimeViewerFlags.awaitSetNextEpisodeAiring(id, pair)` writes both fields, then the UI state is updated.

## 2. Wire formats

### 2.1 AniList GraphQL
Request: `POST https://graphql.anilist.co`, `Content-Type: application/json; charset=utf-8`, body `{"query":"..."}`.

Queries (whitespace normalised):
- `query { Media(idMal:<MAL id>,type: ANIME) { id } }`
- `query { Media(id:<AniList id>) { nextAiringEpisode { episode airingAt } } }`

Response examples (fixtures):
- `{"data":{"Media":{"id":1535}}}` -> AniList id `1535`
- `{"data":{"Media":null}}` -> not found (returns `0` upstream)
- `{"data":{"Media":{"nextAiringEpisode":{"episode":12,"airingAt":1765000000}}}}` -> `(12, 1765000000)`
- `{"data":{"Media":{"nextAiringEpisode":null}}}` -> `(1, 0)`

### 2.2 Simkl calendar
`GET https://data.simkl.in/calendar/<anime|tv|movie_release>.json`. JSON array. Entry (observed 2026-10-08, first entry of `anime.json`, values trimmed):

```json
{"title":"A Will Eternal Final Season",
 "date":"2026-10-07T00:00:00+09:00",
 "release_date":"2026-07-22",
 "ids":{"simkl_id":3194924,"slug":"yi-nian-yong-heng-wanjie-ji","tmdb":null,"mal":"62248"},
 "episode":{"episode":14,"url":"https://simkl.com/anime/3194924/yi-nian-yong-heng-wanjie-ji/episode-14/"},
 "anime_type":"ona"}
```

Key facts: `ids.mal` is a string or `null`; `date` has an offset; `episode.episode` is an integer; the entry key order is `title, poster, date, release_date, rank, ratings, url, ids, episode, anime_type`.

## 3. Upstream parsing (string search) and its bugs

Upstream never parses Simkl as JSON. It searches the body for the text `"simkl_id":<id>,` and slices around it. Three defects follow from that.

### Bug A: `removeAiredSimkl` slices by segment index
```kotlin
val index = body.split("\"date\":\"").drop(1).indexOfFirst { ... }   // index in segment list
return if (index >= 0) body.substring(index) else ""                  // used as a character offset
```
Effect: the slice starts at character `index`, not at the k-th upcoming entry. The "skip aired entries" intent is lost. Since the search then finds the first `simkl_id` match, an already-aired entry can be returned, giving an airing time in the past.

Fix (Rust): filter on `date > now` after parsing; no slicing.

### Bug B: MAL lookup crosses entry boundaries
`substringAfter("\"mal\":\"")` on the text after the target's `simkl_id` finds the next `"mal":"` in the file when the target's `mal` is `null` (JSON null has no opening quote). The result is the MAL id of a later entry, and the AniList lookup is then for the wrong show.

Fix (Rust): `ids.mal: Option<String>`; `None` means "no MAL path".

### Bug C: episode number taken from the rest of the file
`substringBefore("\"}}").substringAfterLast("\"episode\":")`. If `"}}` does not occur after the target, `substringBefore` returns the whole remainder and `substringAfterLast` takes the LAST `"episode":` in the file.

Fix (Rust): `episode.episode` of the same entry.

Also: `toUnixTimestamp` throws on a date without an offset; the exception escapes (no catch). Rust skips that entry.

## 4. Rust algorithm (Proposal)

```rust
pub fn next_simkl_airing(entries: &[SimklEntry], simkl_id: i64, now_s: i64)
    -> Option<(i32, i64, Option<i64>)>
{
    entries.iter()
        .filter(|e| e.ids.simkl_id == simkl_id)
        .filter_map(|e| {
            let t = DateTime::parse_from_rfc3339(&e.date).ok()?.timestamp();
            (t > now_s).then_some((e.episode.episode, t, e.ids.mal.as_deref().and_then(|m| m.parse::<i64>().ok())))
        })
        .min_by_key(|(_, t, _)| *t)
}
```

Then `load_airing` (in `kuro-net`):
1. Status and track checks as upstream.
2. Simkl: for each calendar type in order: parse with `serde_json` (`Vec<SimklEntry>`), call `next_simkl_airing`. If `Some((ep, t, Some(mal)))` and `mal != 0`: return AniList path with the MAL id (the upstream MAL branch takes precedence over the Simkl date). If `Some((ep, t, None))`: return `(ep, t)`. If `None`: next type.
3. After the loop: `(1, 0)`.

Deviations to record in the tasks: (a) "no future entry" returns `(1, 0)` (upstream may return a carried episode number with time 0); (b) per-entry parse errors are skipped.

Ordering note: the upstream MAL branch returns the AniList result for the MAL id even when the Simkl date is present. The Rust port keeps this, because the AniList value is authoritative for airing.

## 5. Stored result encoding (exact)

| Field | Mask (hex) | Bit range | Encoding | Decoding |
|---|---|---|---|---|
| episode | `0x000000000FFFF00` | 8..23 | `episode << 8` | `(flags & mask) >> 8` |
| airing time (s) | `0x0FFFFFFFF000000` | 24..55 | `seconds << 24` | `(flags & mask) >> 24` |

Write rule: `flags = (flags & !mask) | (value & mask)`. Each field is a separate write upstream (two `updateAnime` calls); Rust writes once.

Test vectors (TC-16):
- `(12, 1765000000)` round trip.
- episode 70000 -> stored as `70000 & 0xFFFF = 4464`.
- time `2^32` -> stored as `0`.

## 6. Edge-case table

| Case | Upstream | Rust |
|---|---|---|
| Completed anime, auto refresh | no network | no network |
| Completed anime, manual refresh | network | network |
| No tracker linked | `(1, 0)`, overwrites stored | `(1, 0)`, overwrites stored (kept, flagged as a decision in tasks T-52a-013) |
| AniList down | `(1, 0)` | `(1, 0)` |
| MAL id unknown | queries `Media(id:0)` | short-circuit to `(1, 0)`, no request |
| Simkl calendar 404 | returns `(1, 0)` | next calendar type (decision: better than upstream, no UI change for the target because time is 0 in both cases; flagged) |
| Simkl > 16 MiB | reads all | `(1, 0)` with a warning |
| Two loads for one anime | both run | serialised by `JobRegistry` tag `airing:<id>` |

## 7. Tests

See `p1-edge-cases-and-tests.md` sections 1.7 and 5.6 (TC-13 to TC-16) and fixtures `anilist_airing_ok.json`, `anilist_airing_null.json`, `anilist_mal_ok.json`, `anilist_mal_null.json`, `simkl_s3_mal_null_then_mal.json`, `simkl_s1_head.json`.
