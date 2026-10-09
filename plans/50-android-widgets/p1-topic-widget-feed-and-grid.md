# 50-android-widgets — topic: the feed rule and the grid, step by step

This topic is the algorithm the widgets run, written so it can be implemented and checked by hand. Every number below comes from the Android source (cited in `p1-behavior.md`). Worked examples use the same numbers so an engineer can verify the port.

## 1. Inputs

| Input | Source | Type |
|---|---|---|
| `now` | system clock, local zone | `DateTime<Local>` |
| `kind` | applet id or `--kind` | `manga` or `anime` |
| `rows`, `cols` | applet size (Android: largest instance) | `u32`, each 1..=10 |
| `lock_flag` | `use_biometric_lock` | `bool` (default `false`) |
| unread rows | `updatesView` / `animeupdatesView` | rows (section 3) |
| covers | cache directory | file or absent (section 5) |

## 2. The algorithm

```
function build_feed(now, kind, rows, cols, lock_flag):
    after_ms = window_start_ms(now)            # section 2.1
    if lock_flag:
        return { locked: true, items: [] }      # no query at all (section 2.3)
    raw = query_unread(kind, after_ms, limit=500)   # section 3
    items = dedupe_and_cap(raw, cap = rows * cols)  # section 2.2
    for item in items:
        item.cover_path = cached_thumbnail(item)    # section 5
    return { locked: false, items, grid: {rows, cols, cap}, window_after_ms: after_ms }
```

### 2.1 Window

```
window_start_ms(now) = (now minus 3 calendar months, same local time, month-end clamped).timestamp_millis()
```

Android: `ZonedDateTime.now().minusMonths(3).toInstant().toEpochMilli()`. Clamping: if the target day does not exist, java.time uses the last day of the target month.

Examples (local zone, time of day kept):

| now | window start |
|---|---|
| 2026-10-08 12:00 | 2026-07-08 12:00 |
| 2026-05-31 00:00 | 2026-02-28 00:00 (2026 is not a leap year) |
| 2028-05-31 00:00 | 2028-02-29 00:00 (2028 is a leap year) |
| 2026-11-30 23:59 | 2026-08-30 23:59 |

The row test is `date_upload > window_start_ms` (strict).

### 2.2 Dedupe and cap

```
seen = {}
out = []
for row in raw (in SQL order, newest fetch first):
    if len(out) == cap: break
    if row.entry_id in seen: continue
    seen.add(row.entry_id)
    out.append(row)
```

Properties:

- Keeps the first row per entry, so the newest unread update wins (cover and click target).
- Stops at `cap` items. `cap = rows * cols`, max 100.
- Rows without a cover URL are kept (placeholder), as on Android.

Worked example (cap 4, rows in SQL order):

| # | entry_id | update_id | kept? |
|---|---|---|---|
| 1 | 10 | 501 | yes (out = [10]) |
| 2 | 10 | 500 | no (seen) |
| 3 | 22 | 499 | yes (out = [10, 22]) |
| 4 | 31 | 498 | yes (out = [10, 22, 31]) |
| 5 | 10 | 497 | no (seen) |
| 6 | 47 | 496 | yes (out = [10, 22, 31, 47]) |
| 7 | 53 | 495 | not reached (cap) |

### 2.3 Lock

- `lock_flag` true: return `locked: true` and an empty list. The query is not run, so titles and covers are not read into memory.
- `lock_flag` false: the feed is built normally.
- The flag is the only input. The app's unlock state and `lock_app_after` are not consulted (Android behaviour, E-8).

### 2.4 Grid

```
rows = clamp(floor((height_px - pad_top - pad_bottom) / 95), 1, 10)
cols = clamp(floor(width_px / 64), 1, 10)
```

Android uses dp and `Float.toInt()` (truncation). Logical px at scale 1 are the same numbers. Worked examples:

| width | height | pads | rows | cols | Note |
|---|---|---|---|---|---|
| 64 | 95 | 0, 0 | 1 | 1 | exact minimum |
| 127 | 189 | 0, 0 | 1 | 1 | just below a step |
| 128 | 190 | 0, 0 | 2 | 2 | two steps exactly |
| 256 | 190 | 0, 0 | 2 | 4 | default 4 x 2 |
| 256 | 190 | 24, 24 | 1 | 4 | 190 - 48 = 142, 142 / 95 = 1.49 -> 1 |
| 640 | 950 | 0, 0 | 10 | 10 | max |
| 2000 | 2000 | 0, 0 | 10 | 10 | clamped |
| 10 | 10 | 0, 0 | 1 | 1 | clamped up |

The cell pitch includes the paddings: a cell is 58 px wide plus 3 px on each side (64), and a row is 87 px high plus 4 px above and below (95).

### 2.5 Layout of the grid

Rows are filled top to bottom. Within a row, cells are filled left to right. The block of rows is centred vertically and each row is centred horizontally. Partial rows are centred too.

Example: 7 items, rows 2, cols 4 → row 1 holds items 1-4, row 2 holds items 5-7 centred under them.

## 3. The query

Manga, as SQLite-compatible SQL (proposal for the Kurobayomi schema; the Android view is in `p1-behavior.md` section 13):

```sql
SELECT mangaId     AS entry_id,
       chapterId   AS update_id,
       mangaTitle  AS title,
       sourceId    AS source_id,
       thumbnailUrl AS thumbnail_url,
       coverLastModified AS cover_last_modified,
       dateFetch   AS date_fetch_ms
FROM updatesView
WHERE read = 0
  AND dateUpload > :after_ms
ORDER BY dateFetch DESC, chapterId DESC
LIMIT :limit;
```

- `:limit` = 500 (Android parity). Applied before dedupe.
- Explicit ORDER BY (Android relies on the view's order; see E-4). The tie-break on id makes the order deterministic.
- Anime: `animeupdatesView`, `seen = 0`, `episodeId AS update_id`, `ORDER BY dateFetch DESC, episodeId DESC`.

## 4. Thumbnail key and state

The Android side keys covers with `MangaCoverKeyer`: `"manga;<id>;<lastModified>"` when a custom cover exists, otherwise `"manga;<url>;<lastModified>"` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt:22-31`). The desktop thumbnail key is owned by area:34-images-covers-cache; this topic only requires that the key changes when `cover_last_modified` changes (so a new cover replaces the old thumbnail).

Thumbnail geometry:

- Source: the library cover file (area:34-images-covers-cache cache).
- Target box: 58 x 87 logical px x `device_scale` (default 2) → 116 x 174 px.
- Fit: scale the source so it covers the box, then centre-crop (equivalent to Android `Scale.FILL` followed by `ContentScale.Crop`).
- Corners: rounded-rect alpha mask, radius 12 x `device_scale` px.
- Format: PNG, written to a temporary file then renamed.

## 5. Cover lookup inside the CLI

```
cover_path(entry):
    p = cached_thumbnail(cache_dir, entry.thumbnail_url, entry.cover_last_modified, device_scale)
    return p as string if p exists else null
```

- No network, no decode, no write.
- A missing `thumbnail_url` gives `null` immediately.
- A path that exists but is not a readable PNG is still reported (the QML `Image` then shows its own error state). This is deliberate: checking the PNG header costs a read per cover; the renderer is the only writer, and it writes atomically.

## 6. Output contract (schema 1)

```json
{
  "schema": 1,
  "generated_at_ms": 1791417600000,
  "kind": "manga",
  "locked": false,
  "grid": { "rows": 2, "cols": 4, "cap": 8 },
  "window_after_ms": 1783478400000,
  "items": [
    { "entry_id": 10, "kind": "manga", "title": "…", "unread_update_id": 501, "cover_path": "/home/USER/.cache/kurobayomi/widget-covers/ab12.png" }
  ]
}
```

- `locked: true` → `items` is `[]`, and `title` and `cover_path` never appear.
- `items` order = the feed order (section 2.2).
- `cover_path` null = placeholder.
- Timestamps in epoch milliseconds (UTC).

## 7. Refresh timing

| Trigger | Android | Desktop |
|---|---|---|
| Process start | manager subscribes, first widget render | applet first run (`triggeredOnStart`) |
| New chapter fetched | manager refresh (id set changes) | next poll, up to 60 s |
| Chapter read | manager refresh (id leaves set) | next poll, up to 60 s |
| Lock flag changed | manager refresh | next poll, up to 60 s |
| Widget resized | Glance re-renders | applet debounce 300 ms, then CLI |
| Periodic | none (`updatePeriodMillis` absent) | every 60 s |

## 8. Worked end-to-end example

Setup: now = 2026-10-08 12:00 local. Lock off. Applet size 256 x 190 px, no paddings.

1. rows = floor(190 / 95) = 2; cols = floor(256 / 64) = 4; cap = 8.
2. after_ms = 2026-07-08 12:00 local in ms.
3. Query returns 9 rows, newest first: entries 10, 10, 22, 31, 10, 47, 53, 53, 60.
4. Dedupe (cap 8): keep 10 (row 1), 22, 31, 47, 53 (row 7), 60 (row 9). Six items; the cap is not reached.
5. Thumbnails: entries 10, 22 and 31 have PNGs; 47 has none; 53 and 60 have PNGs. `cover_path` = path, path, path, null, path, path.
6. Layout: row 1 = items 1-4 (10, 22, 31, 47), row 2 = items 5-6 (53, 60) centred.
7. Output: `"items"` with six entries in that order.

Lock on for the same inputs: `{"locked": true, "items": []}`; no query, no thumbnails touched.

## 9. Invariants to test

- `items.len() <= rows * cols` and `items.len() <= 100`.
- No two items share `entry_id`.
- Order of `items` equals the order of first appearance in the SQL result.
- `window_after_ms` = `window_start_ms(now)`.
- With lock on, the query count is zero.
- With lock off and an empty result, `items` is empty and `locked` is false (the empty state is driven by `items.len() == 0`).
