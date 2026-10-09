# Topic: the Jellyfin progress model (tracking URLs, played flags, marking)

Source root `/home/blackbox/aniyomi-src`. Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/` (`Jellyfin.kt`, `JellyfinApi.kt`, `JellyfinInterceptor.kt`, `dto/JFItem.kt`). Full detail in `p1-behavior.md` section 5. This note explains the model; the behaviour file lists the algorithms.

## 1. The tracking URL carries the whole model

Jellyfin has no "tracking entry" of its own. A tracking URL is a Jellyfin item URL plus a fragment that says what kind of thing it is (`JellyfinApi.kt:33`, `:126`, `:41`):

```
https://{server}/Users/{userId}/Items/{itemId}#movie…            -> the item is a movie
https://{server}/Users/{userId}/Items/{seasonId}#seriesId,{seriesId}   -> the item is a season of a series
```

- `userId` is `pathSegments[1]`. It also selects the API key (`JellyfinInterceptor.kt:32`).
- For a season, `seasonId` is the last path segment, and `seriesId` is the last comma part of the fragment (`JellyfinApi.kt:72-73`).
- A missing fragment throws (`fragment!!`), which is defect D-28.

The Jellyfin extension (outside this repo) writes these URLs. The producer's exact form is inferred from the consumer.

## 2. Watched state comes from `UserData.Played`

`JFItem.userData.played` (`dto/JFItem.kt:16`) is the only progress signal. Each episode has one. A season has one too, but the code never uses it for series progress.

The tracker reports:
- Movie: `last_episode_seen` is `1.0` when played, `0.0` otherwise. Status COMPLETED or UNSEEN (`JellyfinApi.kt:55-67`).
- Season: the continuous rule over the episode list (below).

## 3. The continuous rule (season progress)

Given the episode list in server order, with `firstUnwatched` = index of the first episode whose `Played` is false (`JellyfinApi.kt:96`):

| Case | `last_episode_seen` | Status |
|---|---|---|
| `firstUnwatched == 0` (none watched at the start) | `0.0` | UNSEEN |
| `firstUnwatched == -1` (all watched) | `total` (the last element's `IndexNumber`) | COMPLETED |
| otherwise | `IndexNumber` of the episode **before** `firstUnwatched` | WATCHING |

The value is the **IndexNumber** of the item at position `firstUnwatched - 1`, not its position (`JellyfinApi.kt:117`). So the rule depends on the list order. The request sends no `SortBy` (`JellyfinApi.kt:88`, `getEpisodesUrl` at `:69-84`), so the server default order is used (defect D-14).

Proposal (`p1-rust-design.md` section 3.9): request `SortBy=IndexNumber&SortOrder=Ascending`, and compute the total from the maximum `IndexNumber`, not from the last element.

Reading order for the table above: `[T, T, F, F]` gives `episodes[1].IndexNumber`, which is `2` when indexes are `1..4`. `[F, F]` gives `0.0`. `[T, T]` gives the total.

## 4. Marking progress

Target: `last_episode_seen`, a Double such as `3.0`.

Kotlin (`JellyfinApi.kt:123-159`):
1. Fetch the episode list.
2. Find **one** episode whose `IndexNumber` equals the target within `0.001` (`:131-133`, `equalsTo` at `:161-163`).
3. If found, `POST /Users/{u}/PlayedItems/{episodeId}?DatePlayed=…` (`:145-155`).
4. If not found, do nothing (D-12).

So the Kotlin marks **one** episode per sync (D-11). If the local side has marked episodes 1 to 3 read, only episode 3 is marked played on the server. Episodes 1 and 2 stay unplayed there. The next refresh then reports `firstUnwatched == 0`, which resets local progress to `0.0` (UNSEEN). This is the defect that makes Jellyfin progress unreliable in the Kotlin version.

Proposal (`p1-rust-design.md` section 3.9): mark every unplayed episode whose index is `<= target` (continuous range, `JELLYFIN_MARK_RANGE = true`). Strict parity is one const.

The movie path (`JellyfinApi.kt:126-127`) posts played for any target, including `0.0` (D-32). Proposal: post only when the target is at least `1.0`.

## 5. The `DatePlayed` value

`DATE_FORMATTER` is `yyyy-MM-dd'T'HH:mm:ss.SSS'Z'` in the device time zone, with a literal `Z` (`JellyfinApi.kt:166`). On a non-UTC device the server gets local time labelled as UTC (D-10). The Rust port formats UTC.

## 6. Path rewriting

`postUrl` is built from the tracking URL by removing the last two path segments and appending `PlayedItems/{id}` (`JellyfinApi.kt:145-151`). Removal is by index (`removePathSegment(3)`, then `(2)`). That only works for a URL whose path is exactly `/Users/{u}/Items/{i}`. A server behind a path prefix (`https://host/jellyfin/Users/…`) gets a wrong URL (D-13). The Rust port finds the `Users` and `Items` segments by name.

Likewise `getEpisodesUrl` resets the path to `/` (`encodedPath("/")`, `JellyfinApi.kt:72`), so a prefix is lost on the episode request too.

## 7. Authentication

API key per user, from the Jellyfin source preferences, slots 1 to 10 (`JellyfinInterceptor.kt:51-67`). The key goes into every URL as `api_key` (`:36-38`). Proposal: send the key as the `X-Emby-Token` header instead, so it stays out of URLs and logs (`p1-rust-design.md` pitfall P-8).

Missing key → `IOException("Please log in through the extension")` (`:33-34`). That message is shown to the user as toast text (`MangaTracker.kt:119`).

## 8. Known limits

- Episode list is requested without paging parameters (`JellyfinApi.kt:69-84`). Whether the server returns every episode of a long season was not verified (E-88).
- `equalsTo` uses `< 0.001`, so `3.0004` matches episode 3 (`JellyfinApi.kt:162`). Parity kept.
- `Jellyfin.update` ignores `didWatchEpisode` (`Jellyfin.kt:60-62`, D-31).
- `refresh` copies `last_episode_seen` from the server, so a local value that is ahead of the server is overwritten on refresh until the next successful sync.

## 9. Test first

1. Continuous rule on `[T,T,F,F,F]`, `[F,F]`, `[T,T]` (E-70, E-71, E-72).
2. Unsorted list `[3,1,2]` (E-74).
3. Range marking for target `3.0` with one already played episode (E-76, E-79).
4. Movie with target `0.0` (E-68).
5. UTC date under a non-UTC time zone (E-80).
6. Reverse-proxy prefix (E-81).
