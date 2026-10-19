# Source plug-in ABI

The contract between a WASM source plug-in (`.kuroext`, `source.wasm`) and the app. The WIT file is
`crates/kuro-source-wasm/wit/kuro-source.wit`; this page gives the JSON shapes. Field names are camelCase.

Versions: **1.5** is the current ABI (package `kuro:source@1.5.0`). **1.4** still loads (see Compatibility). A manifest
states its `abi`; `ABIS` in `crates/kuro-source-wasm/src/manifest.rs` lists the accepted values.

## Imports and exports

| Name | Kind | Since |
|---|---|---|
| `kuro:source/host@1.5.0`: `http`, `pref-get`, `pref-set`, `log`, `unix-seconds` | import | 1.5 |
| `kuro:source/host@1.4.0`: `http`, `pref-get`, `pref-set`, `log` | import | 1.4 |
| `invoke(op: string, input-json: string) -> result<string, string>` | export | 1.4 |

`unix-seconds: func() -> u64` is the host clock in whole Unix seconds. Use it for dates only (a chapter's upload date,
an episode's upload date). It is not monotonic.

## The envelope

Every op gets `input-json` = `{"sourceId": <i64>, "showAdult": <bool>, "args": {...}}` and returns JSON text, or an `Err`
string.

`showAdult` is the user's "Show adult (18+) titles" switch (Settings, NSFW group; off by default), read at each call. A
source that knows a title's rating leaves adult titles out of `popular`, `latest` and `search` while it is `false`. It is
additive to ABI 1.5: an older guest ignores it, and an older app sends no key, which a guest reads as `false`.

- An op the guest does not know returns `Err("unknown op: <op>")`.
- The host treats that exact text as **no change** for the ABI 1.5 additions below. Any other `Err` is a failure of that
  call.

## ABI 1.4 ops (unchanged)

Manga: `info`, `create-sources`, `popular`, `latest`, `search`, `details`, `chapters`, `pages`.
Anime: `popular`, `latest`, `search` (answer `{"animes", "hasNextPage"}`), `episodes`, `hosters`, `videos`, `subtitles`.
The shapes of these ops are in `crates/kuro-source-wasm/src/wire.rs` and `anime_wire.rs`.

## ABI 1.5 changes

### 1. Search filters (`search`)

`args` is `{"page": i32, "query": string, "filters": FilterList}`. `FilterList` is a JSON array, empty when the app holds
no filters. Each filter is the serde form of the app's `Filter` enum: an object with one key, the variant name.

```json
[{"CheckBox": {"name": "Flag", "state": true}},
 {"Select": {"name": "Sort", "values": ["A", "B"], "state": 0}},
 {"Text": {"name": "Author", "state": "x"}},
 {"TriState": {"name": "Genre", "state": 1}},
 {"Sort": {"name": "Order", "values": ["Name"], "state": {"index": 0, "ascending": true}}},
 {"Group": {"name": "More", "state": []}},
 {"Header": {"name": "Section"}}, {"Separator": {"name": "-"}}]
```

`TriState` is `0` ignore, `1` include, `2` exclude. `Sort.state` is `null` for no explicit sort. Today the app's
plug-ins declare no filters, so the array is `[]`. A guest ignores keys it does not read.

### 2. Request headers (`request-headers`)

`args` is `{}`. Output is `[[name, value], ...]`: the headers the app sends with each page image and video request of
this source. When the op answers, its list **replaces** the app's default set for that source (so a source that must not
send `Referer` or `Origin` simply leaves them out). `unknown op` means the default set applies.

```json
[["Accept", "image/avif,image/webp,image/*,*/*;q=0.8"]]
```

Invalid header names or values are dropped by the app.

### 3. Final URL (`http` reply)

The `http` reply gains `"url"`: the final URL after redirects, as text. Request and reply otherwise as in 1.4.

```json
{"status": 200, "headers": [["content-type", "text/html"]], "url": "https://example.org/final", "body": "...", "bodyEncoding": "base64"}
```

`bodyEncoding` is present only for a non-UTF-8 body. The host still checks the final host against the allow-list, and
drops a reply from a host outside it.

### 4. Request spacing (`info`, `http`)

The `info` object (and each `create-sources` item) may carry `"requestGapMs": <u64>`, the least gap in milliseconds
between two requests the host sends to **one host** for that source. Absent means no spacing.

The host enforces it inside `http`: it books the next send to the request's host at least the gap after the last booked
send, and sleeps until then. No guest sleep or clock is needed. The bookings are shared by every source in the process.

```json
{"id": 2131019126180322627, "name": "Weeb Central", "lang": "en", "supportsLatest": true, "requestGapMs": 2000}
```

### 5. Host clock (`unix-seconds`)

No JSON: `unix-seconds() -> u64`, seconds since 1970-01-01 UTC. It answers 0 only when the system clock is before 1970.

### 6. Page image fix-up (`unscramble`)

`args` is `{"body": "<base64 image bytes>", "headers": [[name, value], ...]}`, the response headers of the image fetch.
The app calls it after every 2xx image fetch, for each source.

Output: `{"body": "<base64>"}` shows these bytes. `{"body": null}` means the bytes are already right; show them as
fetched.

| Reply | The app |
|---|---|
| `{"body": "<base64>"}` | shows the decoded bytes |
| `{"body": null}` | shows the bytes as fetched |
| `Err("unknown op: unscramble")` | shows the bytes as fetched |
| `Err("Failed to decode image")` (any other `Err`) | fails that page with the text |

Example: the comix plug-in descrambles its grid images here (`docs/comix-image-hook.md` in the extensions repository).

## Compatibility

- A 1.4 package (`"abi": "1.4"`, imports `kuro:source/host@1.4.0`) loads unchanged. The host links the 1.4 import with
  the same four functions. A 1.4 guest does not know the 1.5 ops, so each one reads as **no change** (`unknown op`).
- A 1.5 package imports `host@1.5.0`. Its `unix-seconds` import is 1.5 only.
- Adding a 1.5 op never changes a 1.4 reply. A missing `requestGapMs` is "no spacing"; the `url` key is an addition that
  a 1.4 parser ignores.

## Where the code is

- WIT: `crates/kuro-source-wasm/wit/kuro-source.wit`
- Host imports and the 1.4 link: `crates/kuro-source-wasm/src/bridge.rs` (`Host` impl, `link_v14`, `book_send`)
- Ops and the envelope: `crates/kuro-source-wasm/src/source.rs` (`search_args`, `optional`, `request_headers`,
  `unscramble`)
- Descriptor: `crates/kuro-source-wasm/src/wire.rs` (`SourceDescriptor::request_gap_ms`)
- App side of the images: `src/reader/source_pages.rs` (`page_image_headers`, `fetch`, `shown`)
- Trait defaults the app calls: `crates/kuro-source/src/source.rs` (`MangaSource::request_headers`, `unscramble`)
- Tests: `crates/kuro-source-wasm/src/source.rs`, `bridge.rs`, `wire.rs` (WAT guests in `src/test_guest.rs`)
