# Topic: anime store index protocol (fetch, detect, decode, map)

Deep dive for area:19-extensions-anime. Covers the three wire shapes an Aniyomi anime store can serve, the detection order, the redirect rules, and the exact mapping. Repo-relative sources are under `/home/blackbox/aniyomi-src`. Full field tables are in `p1-data-and-config.md` section 3. Rust types are in `p1-rust-design.md` section 3.

## 1. The four documents a store can serve

| Document | Served at | First byte | Decoder | Kotlin |
|---|---|---|---|---|
| Store, JSON | the store URL (usually `https://example.org/index.min.json` or a custom path) | `{` | `NetworkAnimeExtensionStore` (JSON) | SSVC:45-49, 89 |
| Store, protobuf | the store URL | not `[` or `{` | `NetworkAnimeExtensionStore` (protobuf) | SSVC:50, 90 |
| Extension list, JSON or protobuf | `extensionListUrl` | `{` or other | `NetworkAnimeExtensionStore.ExtensionList` | SSVC:73-82 |
| Legacy repo | `https://example.org/repo.json` | `{` | `NetworkLegacyAnimeExtensionRepo` | SSVC:40-46, LEGREPO:72-98 |
| Legacy list | `https://example.org/index.min.json` | `[` | `List<NetworkLegacyAnimeExtension>` | SSVC:35-42, 97-99, LEG:10-61 |

A "store" is identified by its index URL. An "extension list" is a sub-document that a store may point to. A "legacy" store is one that predates the current format.

## 2. Fetch sequence (adding a store)

Entry point: `AddAnimeExtensionStore(indexUrl)` → `AnimeExtensionStoreRepositoryImpl.insert` → `AnimeExtensionStoreService.fetch` (SREPO:19-21, SSVC:28-68).

```
fetch(url, depth=0):
  body = GET url           (non-2xx -> error)
  if body starts with 1f 8b: body = gunzip(body)
  match first byte:
    '['  -> require url endswith "/index.min.json" else error("Provided legacy store url is not valid")
            repo = GET url.replace("/index.min.json", "/repo.json") (no gunzip)
            parse repo as LegacyRepo; store.index_url = that repo url
    '{'  -> try LegacyRepo(body); on failure parse Store(body)
    else -> parse Store(body) as protobuf
  if result is LegacyRepo and index_v2 present:
      return fetch(index_v2, depth+1)            # Aniyomi: no limit (E-STO-04). Kurobayomi: at most 4 redirects; a redirect from depth 4 -> RedirectLoop
  return StoreRow(from result)
```

Points that matter for implementation:
- The stored `index_url` is the **final** URL after the legacy rewrite or the redirect. Duplicate detection compares against the stored URL, not the typed one (behavior 12.4, UI E-UI-04).
- `fetch` does not read the extension list. Adding a store succeeds even when its list is broken. The list is read at the first `findAvailableExtensions`.
- The store `name` and `badgeLabel` come from the document. For legacy, `badgeLabel = shortName ?: name` (LEGREPO:89).

## 3. Extension list fetch (refresh and browse)

Entry point: `AnimeExtensionStoreService.getExtensions(store)` (SSVC:70-109). Three branches, in this order:

1. `store.extensionListUrl != null` → GET that URL, gunzip, decode as `ExtensionList` (JSON if `{`, else protobuf).
2. `!store.isLegacy` → GET `store.indexUrl`, gunzip, decode as `NetworkAnimeExtensionStore` (JSON or protobuf), then `extensionList!!`.
3. legacy → GET `base + "/index.min.json"` where `base = store.indexUrl.removeSuffix("/repo.json")`, decode as a JSON array of legacy entries. No gunzip.

Branch 3 uses the stored row's `isLegacy` flag. A row whose `isLegacy` is wrong fails. Two known ways to get a wrong flag:
- Migration row (`insertFromPreference`, SREPO:23-36): `isLegacy = false` but the URL is a `repo.json`. Branch 2 fails. (E-STO-21.)
- Backup without `isLegacy` (BRESTORE:68): default `true`. A non-legacy store is then read through branch 3. (E-STO-22.)

## 4. Detection rules, exact

| Context | Check | Result |
|---|---|---|
| fetch | gzip magic `1f 8b` on the first two bytes | gunzip (`GzDecoder` in Rust) |
| fetch | first byte `[` (`0x5B`) | legacy list path (needs `/index.min.json`) |
| fetch | first byte `{` (`0x7B`) | try legacy repo, then store JSON |
| fetch | other | store protobuf |
| getExtensions | gzip magic | gunzip |
| getExtensions | first byte `{` | JSON |
| getExtensions | other | protobuf |

The gzip check uses `peek().readShort().toInt() == 0x1f8b` (SSVC:112-118). A body shorter than two bytes is treated as not gzip. A body that is empty fails at `peek().readByte()`.

**Rust rule:** check the magic bytes with a slice match, never with a parser. A short body is an error, not a panic.

## 5. Mapping rules (wire to domain)

Summary (full table in `p1-rust-design.md` section 3.3):

| Field | Store entry | Legacy entry |
|---|---|---|
| name | `name` (no prefix strip) | `name` with `Aniyomi: ` stripped |
| pkg | `packageName` | `pkg` |
| versionName | `versionName` | `version` |
| versionCode | `versionCode` | `code` |
| aniyomi lib | `extensionLib` parsed as f64, failure fails the store | `version` prefix before last `.` parsed as f64 |
| lang | one distinct language, else `all` | `lang` |
| nsfw | `contentWarning >= MIXED` | `nsfw == 1` |
| torrent | `isTorrent` | false |
| sources | `id`, `name`, `language`, `homeUrl` | `id`, `lang`, `name`, `baseUrl`; or one synthetic id 0 |
| apk url | `resources.apkUrl` | `base + "/apk/" + apk` |
| icon url | `resources.iconUrl` | `base + "/icon/" + pkg + ".png"` |
| kind | Wasm when `resources.kuroext` is valid, else ApkOnly | ApkOnly |

Two rules are easy to get wrong:
- `lang` for **zero** sources is `"all"` in the store path (WIRE:113) but `""` in the loader (LDR:350). The store value is what the Browse list shows.
- `isNsfw` in the store path is `>= MIXED`. The loader path (APK metadata `aniyomix.contentWarning > 0`) is looser. Kurobayomi uses `>= MIXED` in both paths (data section 2.1). This is a deliberate change from Aniyomi.

## 6. Errors and what the user sees

| Failure | Where | User-visible |
|---|---|---|
| Non-2xx on store | add or refresh | add: `Failed to add extension store`. refresh: per-store banner (K-6) |
| Legacy URL check | add | `Provided legacy store url is not valid` (kept verbatim, K-3) |
| Redirect loop | add or refresh | `This extension store points to itself` |
| Body too large | add or refresh | `The extension store index is too large` |
| Decode error | add or refresh | `This extension store could not be read` |
| `extensionLib` not numeric | browse fetch | per-store banner (the whole store fails) |
| `contentWarning` unknown | browse fetch | per-store banner |
| Cancelled | any | no message (previous data kept) |

Aniyomi shows the raw exception text in the add dialog (behavior 12.1) and shows nothing for browse failures (behavior 7.8). Kurobayomi replaces both with the fixed strings above.

## 7. Limits (Proposal)

- 16 MiB after gunzip for any index or list body.
- 64 MiB cap on gunzip output (protects against bombs).
- Redirect depth: at most 4 redirects. A fifth redirect is `RedirectLoop`.
- At most 4 concurrent store fetches during refresh.
- Per-request timeout 30 seconds. Aniyomi uses the OkHttp defaults.
- Default cache control on GET: `maxAge(10, MINUTES)` (`core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:16`). Whether a disk cache serves it is area:12-network-stack's decision. Refresh must bypass a cached response (Proposal: `Cache-Control: no-cache` on refresh requests only).

## 8. Test anchors

TST-19-010 to TST-19-030, TST-19-049 to TST-19-055 (see `p1-edge-cases-and-tests.md` section 4.1). Fixtures in section 5 of that file.

## 9. Open questions

None blocking. The `kuroext` addition (data section 8.2) is a proposal that Aniyomi ignores. Publishers who want Kurobayomi packages must publish both the Aniyomi entry and the `kuroext` block. Whether a store without an Aniyomi APK can be published by a third party is out of scope.
