# Topic: the Kavita tracker (auth, chapter counting, progress)

Source root `/home/blackbox/aniyomi-src`. Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/` (`Kavita.kt`, `KavitaApi.kt`, `KavitaInterceptor.kt`, `KavitaModels.kt`). Full detail in `p1-behavior.md` section 2. This note covers the three things that are easy to get wrong.

## 1. Three JWT slots, filled from another plugin's settings

The tracker never asks for a Kavita URL or key. It reads them from the **Kavita source's** preferences, for up to three Kavita sources (`Kavita.kt:118-148`). The slot's source id is `MD5("kavita_<n>/all/1")` (see `p1-data-and-config.md` section 4). It reads `APIURL` and `APIKEY` from that source's preference file.

Lifecycle of the in-memory table:

```
authentications == null ─┐
                         │ first request on any Kavita URL
                         ▼
          loadOAuth(): for n in 1..3
              prefs(kavita_n) -> APIURL, APIKEY
              both set? -> POST {APIURL}/Plugin/authenticate?apiKey=…&pluginName=Tachiyomi-Kavita
                  200 -> jwt stored for APIURL
                  401/500 -> IOException (aborts the whole loop, D-05)
                  timeout -> skipped
                  other -> skipped
          authentications = table
                         │
                         ▼
          every request: Authorization: Bearer <jwt of the slot whose APIURL == api base>
          no refresh, no reset (D-07)
```

Rust (`p1-rust-design.md` section 3.5): one `KavitaAuth` with a `RwLock` table, a `Mutex` single-flight loader, per-slot error isolation, and invalidate-and-retry-once on 401.

Mapping an entry URL to a slot: `api base = url.split("/api/").first() + "/api"` (`KavitaApi.kt:30-32`). The slot matches on exact string equality of its `APIURL`. So the user's `APIURL` must have exactly the same form as the series URL's host and path. The authenticate path is appended directly (`KavitaApi.kt:43`), so `APIURL` must already end in `/api`. This is inferred from the code. The Kavita extension is outside this repo.

## 2. Counting chapters from volumes

`total_chapters` is not a field the server returns. The tracker computes it from the volume list (`KavitaApi.kt:101-124`). Read the walk in `p1-behavior.md` section 2.6. In short:

- A volume whose largest chapter number is exactly `0` counts as one chapter (a single-file volume). The count is `volumeNumber`.
- Otherwise the largest chapter number, truncated to a whole number, is `maxChapterNumber`.
- The result is the larger of the two.

Example: volumes `[0]`, `[0]`, `[1, 2, 10.5]`. The count is `max(10, 2) = 10`. Volumes `[0]` × 4 and `[1, 2]` give `max(2, 4) = 4`.

This is a heuristic. Two cases produce a wrong number:

- A series that is only volumes with chapter numbers above zero (for example a volume numbered 3 whose chapters are 3.1 to 3.9): the largest chapter number is taken, not the volume count.
- Kavita's own encoding (upstream `Kavita.Services/TachiyomiService.cs`): loose-leaf volumes are encoded in chapter numbers as `volume * 10000`. The Kotlin does not decode it. The count can then be far too large (`E-113`).

Proposal: keep the Kotlin heuristic (parity), and add an open item to decide whether to decode the `* 10000` form (`p1-edge-cases-and-tests.md` E-113).

## 3. Latest chapter read: what the number means

`GET {api}/Tachiyomi/latest-chapter?seriesId=…` (`KavitaApi.kt:126-148`) returns the highest fully read chapter. The response is:

- `200` with a `ChapterDto` whose `number` is a string, read after replacing `,` with `.` (`:133`).
- `204` when the user has no progress, read as `0.0` (`:137-139`).
- Any other status: silently `0.0` (`:146`).

Upstream (`Kavita.Services/TachiyomiService.cs:38-94`): for a series with progress, the endpoint returns the highest read chapter, or the highest read volume for volume-only series (`volume * 10000`). The client takes whatever number comes back.

Defect D-22: a `"-1"` number is stored as `-1.0`. Rust clamps to `0.0`.

## 4. Marking progress

`POST {api}/Tachiyomi/mark-chapter-until-as-read?seriesId=…&chapterNumber=<Double>` with body `{}` (`KavitaApi.kt:178-193`). The server marks every non-special chapter whose max number is `<= chapterNumber`. For `chapterNumber < 1.0` it treats the value as a volume (upstream `TachiyomiService.cs:108-150`).

Two details the port must keep:

- The number is sent as Kotlin renders a `Double`: `12.0`, not `12`. The server parses it as a float, so both work, but a test should pin the string (`E-32`).
- The response body is a bool and is ignored. Success is any `2xx`.

After the POST the tracker calls `getTrackSearch` again (`KavitaApi.kt:192`). So one progress write costs four requests. The status it returns comes from the server.

## 5. Status

Server-driven (`KavitaApi.kt:162-166`). The Kotlin branch order is `pagesRead == pages` first, then `pagesRead == 0`, then READING. This makes `0/0` COMPLETED. The Rust port checks `pagesRead == 0` first (D-24). `pagesRead > pages` is READING in both.

The local status in `Kavita.update` (`Kavita.kt:61-72`) is dead code (`p1-topic-enhanced-tracker-framework.md` section 3).

## 6. Cover

`thumbnail_url` is not in the upstream `SeriesDto` (`Kavita.Models/DTOs/SeriesDto.cs`, read-only check). The Kotlin field therefore stays `""`. The cover image is not shown from the tracker. Proposal: derive it from the series id if the cover endpoint is confirmed.

## 7. Search

Not implemented. `searchManga` is `TODO("Not yet implemented: search")` (`Kavita.kt:78-80`). That throws `NotImplementedError`, an `Error`, not an `Exception`. The tracker's `match` cannot catch it because `match` catches `Exception` (`Kavita.kt:101-106`). In practice `match` does not call `searchManga`, so the TODO is only reached from a user-facing search path. Rust returns `SearchUnsupported`.

## 8. What to test first

1. The auth table with one slot returning 401 (E-02).
2. The volume walk with `[0]`, `[1,2,10.5]`, and an empty volume (E-21 to E-25).
3. The `latest-chapter` number formats (E-28 to E-31).
4. `mark-chapter-until-as-read` string formatting (E-32, E-33).
