# Topic: the three Kitsu ids and the `remote_id` lifecycle

Deep dive for `p1-behavior.md` sections 6, 7, 9, 12, 13. Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt`, `Kitsu.kt`, `dto/KitsuListSearch.kt`, `dto/KitsuSearch.kt`, `dto/KitsuAddEntry.kt`, `dto/KitsuUser.kt`.

Kitsu uses three different numeric namespaces. Aniyomi stores all of them in one column, `remote_id` (`manga_sync.remote_id` / `anime_sync.remote_id`, `INTEGER NOT NULL`), and the meaning changes over the track's life.

## 1. The three ids

| Id | Meaning | Source of value | Used in requests as |
|---|---|---|---|
| Media id | Kitsu manga or anime record | Algolia hit `id`; list-entry `included[].id` | `media.data.id` (POST add); `filter[manga_id]` / `filter[anime_id]` (find); tracking URL `https://kitsu.app/manga/<id>` |
| Library-entry id | this user's list row | POST add response `data.id`; list-entry `data[].id` | `PATCH|DELETE library-entries/<id>`; `filter[id]` (get); `data.id` (PATCH body) |
| User id | the logged-in Kitsu user | `GET users?filter[self]=true` `data[0].id` (string) | `relationships.user.data.id` (POST); `filter[user_id]` (find) |

The user id is stored in the password slot of the credentials (Kitsu.kt:238-240). See `p1-data-and-config.md` section 5.

## 2. Lifecycle of `remote_id` for one track

```
search hit          remote_id = MEDIA id          (KitsuSearch.kt:42 / :60)
  |
  | bind -> find (filter[manga_id]=MEDIA)  -> remote found: remote_id = LIBRARY id (KitsuApi / KitsuListSearch.kt:25-ish)
  |                                          -> PATCH library-entries/LIBRARY
  | bind -> find -> nothing: add (POST) -> remote_id = LIBRARY id from response (KitsuApi.kt:83,128)
  |
bound track         remote_id = LIBRARY id        (from here on)
  |
  | refresh  -> get (filter[id]=LIBRARY)          remote_id unchanged
  | update   -> PATCH library-entries/LIBRARY     remote_id unchanged
  | delete   -> DELETE library-entries/LIBRARY    remote_id unchanged
```

Invariant (Aniyomi, relied upon but not enforced): `bind` is the only place that moves `remote_id` from media id to library id. Any call that uses `remote_id` as a library id before `bind` sends a request to `library-entries/<media id>`, which is a different entry or a 404.

## 3. Hazards in the Kotlin code

1. `firstToMangaTrack()` and `firstToAnimeTrack()` set `remote_id = data[0].id` (library id) and then compute `tracking_url = mangaUrl(remote_id)`. The URL is built from the library id, not the media id (KitsuListSearch.kt:25-30, 55-64). The bogus URL is not persisted because `bind` keeps the search track's URL and `refresh` does not copy URL. Latent: any future caller that persists a list-derived track gets the wrong link. Task T-30a-029.
2. `bind` and `refresh` each return a track built from a list response. Only personal fields are copied, so `remote_id` of the caller's track is set explicitly in `bind` (Kitsu.kt:172) and not in `refresh`. Correct today, fragile.
3. `delete` uses `track.remoteId` (library id). A track that was never bound (media id) sends `DELETE library-entries/<media id>`. The UI only offers delete for tracks in the DB, which are always bound or were added (so this does not happen in practice). Guard in Rust: `NotFound` for `remote_id` that was never bound (flag `bound: bool` on `TrackRecord`, Proposal, T-30a-011).
4. JSON:API type inconsistency in bodies: the library id is sent as a JSON number in PATCH (`data.id`), the media id as a number in POST (`relationships.media.data.id`), the user id as a string. Keep as-is; test in T-30a-033 step 4.

## 4. Rust representation (Proposal)

Keep one `remote_id: i64` in `TrackRecord` for compatibility with the DB column, and add `bound: bool` (true after `bind` succeeds, or when loaded from a row whose entry was added through Kurobayomi). Methods that need a library id return `KitsuError::NotBound` when `bound == false`. Methods that need a media id require `bound == false`.

Loading from the DB cannot tell a media id from a library id by value alone. Resolution: store `bound` as a new column in the Rust schema (area:03/04 proposal). Migration of Aniyomi backups (T-30a-032): rows with a tracking URL and remote_id set after a bind are bound; a backup row is treated as bound (Aniyomi only persists tracks after bind or add).

## 5. Search pipeline (summary)

1. Key: `GET https://kitsu.app/api/edge/algolia-keys/media/` with bearer → `media.key`.
2. Query: `POST https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/` with `X-Algolia-Application-Id`, `X-Algolia-API-Key`, body `{"params":"query=<enc>" + <verbatim ALGOLIA_FILTER constant for the kind>}`.
3. Filter `subtype != "novel"`.
4. Map each hit to a `SearchHit` where `remote_id = hit.id` (MEDIA id). The UI then calls bind with a track whose `remote_id` is that media id.

Cross-check with the bind algorithm: step 4 is the only way a media id enters a track, which is why search results must keep the media id until bind completes. The UI must not persist a `SearchHit` directly.

## 6. Test hooks

- KT-42 (bind found) asserts `remote_id` changes from media to library id.
- KT-40/41 (add) assert `remote_id` becomes the response id.
- KT-66 asserts `tracking_url` is not changed by bind (media id URL kept).
- KT-57/58 (delete) assert the DELETE path uses the library id and refuses an unbound track (Proposal).
