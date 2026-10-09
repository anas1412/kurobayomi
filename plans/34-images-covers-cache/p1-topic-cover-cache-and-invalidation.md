# Topic: cover lifecycle, custom covers and invalidation

Deep dive for area 34. Facts are in `p1-behavior.md` §10, §12, §18, §19, §24; this file gives the state machine and the transition table used by tests C1-C12 (`p1-edge-cases-and-tests.md` §3). Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`.

## 1. The state of one cover

A cover of one entry is in exactly one of these states at any time. The state is derived from two facts: whether a thumbnail file exists for the current `thumbnail_url`, and whether a custom file exists for the entry id. The DB field `cover_last_modified` changes on every transition that replaces or removes an image.

| State | Thumbnail file | Custom file | Shown | Cache key |
|---|---|---|---|---|
| S-A `remote-uncached` | absent | absent | network image (or none) | `manga;<url>;<lm>` |
| S-B `remote-cached` | present (favourites only) | absent | cover file | `manga;<url>;<lm>` |
| S-C `custom` | any | present | custom file | `manga;<id>;<lm>` |
| S-D `local` | local folder image (area:14-source-local-and-metadata) | n/a | local file | `manga;<url>;<lm>` (url is `file://` or path) |
| S-E `none` | absent | absent | error icon | not cached (`NoCoverSpecified`, E-IMG-20) |

Non-favourite remote entries never reach S-B: their thumbnails live only in the Coil disk cache (`p1-behavior.md` §6.1), so the state is S-A with a disk entry instead of a file.

## 2. Transitions

| From | Event | To | Side effects (exact) | Source |
|---|---|---|---|---|
| S-A or S-B | source refresh with a different URL, not custom | S-A (new URL) | delete thumbnail file; `cover_last_modified = now` | `MangaExtensions.kt` `prepUpdateCover` else-branch |
| S-B or S-A | source refresh with the same URL, `refreshSameUrl = true` | S-A | delete thumbnail; `cover_last_modified = now` | same, the same-URL guard is skipped |
| S-C | source refresh with a different URL | S-C | delete thumbnail only; timestamp unchanged | `hasCustomCover` branch |
| any remote | source refresh with null or empty URL | unchanged | none | early returns |
| S-D | source refresh with different URL | S-D | timestamp only | `isLocal()` branch first |
| S-A, S-B, S-C | user picks a file (favourite entry) | S-C | write custom file; `cover_last_modified = now` | `editCover` remote branch |
| S-A (non-favourite anime) | user picks a file | S-A | nothing written; UI says success (E-IMG-14), Rust reports failure (U-1) | `AnimeExtensions.kt` `editCover` |
| S-C | user chooses Delete | S-A or S-B | delete custom; `cover_last_modified = now` | `MangaCoverScreenModel.deleteCustomCover` |
| S-A, S-B, S-C | entry removed from library | S-E or S-A (non-favourite) | delete thumbnail and custom if any; bump timestamp only when a file was deleted | `removeCovers` (`MangaExtensions.kt`) and `toggleFavorite` (`MangaScreenModel.kt`) |
| S-D | entry removed from library | S-D | nothing (`isLocal` early return) | `removeCovers` |
| any | cache clear (chapter cache) | unchanged | covers are not in the chapter cache | `ChapterCache.clear` only touches its own folder |

## 3. Why the timestamp matters

The keyer output includes `cover_last_modified` (`p1-behavior.md` §10). A changed timestamp is the only signal that makes Coil (and the Rust memory cache) drop the old decoded image. Therefore every transition that changes the visible image must bump the timestamp, and every transition that does not change the visible image must not bump it (it would cause needless re-decodes).

Consequences checked by the tests:
- C6: a custom cover survives a source URL change, so no bump (the visible image did not change).
- C4: a same-URL refresh deletes the file and bumps, so the next load fetches the new bytes.
- C8/C9: `removeCovers` bumps only when there was something to delete (no spurious change for entries without covers).

## 4. Disk cache versus cover file

| Property | Cover file (`covers/`, `animecovers/`, `animebackgrounds/`) | Disk image cache (`<cache>/images`) |
|---|---|---|
| Scope | favourites (`isLibraryManga`/`isLibraryAnime`) | everything else, and favourites' first load |
| Survives cache clear | yes | no |
| Keyed by | `md5(thumbnail_url)` | `md5(cache key string)` |
| Deleted by | removal from library, URL change, custom delete | LRU eviction only |
| Written by | fetcher, `writeResponseToCoverCache` | fetcher, `writeToDiskCache` |
| Promoted from disk to file | `moveSnapshotToCoverCache` on first favourite load | n/a |

Promotion (behavior §6.3) happens once per favourite cover: after the first successful load the cover moves into the cover file and the disk entry is removed. The Rust version does the same (`cover_file::promote_from_disk`).

## 5. Custom cover precedence rules

1. Custom file exists and `use_custom_cover` is true → custom file is the image (behavior §5.1 step 2).
2. Background requests use the background custom file only when `use_background` is true (behavior §4.2).
3. The custom file's existence is checked on every key computation and every fetch (behavior §19.6). A file deleted by hand takes effect on the next load because the key changes only after the timestamp changes; until then the memory cache may still show the old decoded image (edge E-IMG-19).

## 6. Migration copy (behavior §24.5)

When an entry is migrated to a new source, the custom cover (and anime background) is copied from the old entry id to the new one. The copy uses `setCustomCoverToCache` on the new entry, so the new entry starts in S-C. The timestamp of the new entry is bumped by the migration code path (area:40-ui-browse-manga/area:41-ui-browse-anime); this area does not bump it.

## 7. Rust state machine (proposal)

```rust
pub enum CoverState { RemoteUncached, RemoteCached, Custom, Local, NoCover }

pub fn state_of(art: &ArtRef, thumb_file: bool, custom_file: bool, is_local: bool) -> CoverState {
    if custom_file { return CoverState::Custom; }
    if is_local { return CoverState::Local; }
    match (art.url.as_deref(), thumb_file) {
        (None, _) => CoverState::NoCover,
        (Some(""), _) => CoverState::NoCover,
        (Some(_), true) => CoverState::RemoteCached,
        (Some(_), false) => CoverState::RemoteUncached,
    }
}
```
Note the precedence: a custom file wins over locality. Local entries cannot have a custom file in practice (their covers are local files), so the order does not change behavior; the test covers it anyway. `Local` takes precedence over the URL checks because local URLs are paths, which the fetcher handles.

## 8. Invalidation function (proposal, mirrors Kotlin exactly)

```rust
pub fn prep_update_cover(entry: &mut Entry, remote_url: Option<&str>, refresh_same_url: bool, now_ms: i64, cache: &CoverPaths) -> Result<bool, io::Error> {
    let Some(new_url) = remote_url else { return Ok(false) };      // C1
    if new_url.is_empty() { return Ok(false); }                     // C2
    if !refresh_same_url && entry.url.as_deref() == Some(new_url) { return Ok(false); } // C3
    if entry.is_local() { entry.cover_last_modified = now_ms; return Ok(true); }        // C5
    if cache.custom_cover_exists(entry.id) {                        // C6
        cache.delete_thumbnail(entry.url.as_deref())?;
        return Ok(false);
    }
    cache.delete_thumbnail(entry.url.as_deref())?;                  // C4, C7
    entry.cover_last_modified = now_ms;
    Ok(true)
}
```
Return value: `true` means the entry changed and must be written (the Kotlin code returns a copy; the bool here is the "changed" flag). In C6 the Kotlin returns `this` (no change), so the Rust returns `false` even though a file was deleted. Writing this down avoids a double bump.

The `delete_thumbnail` errors are returned, while Kotlin `deleteFromCache` swallows the `delete()` result (returns a count). Deviation: Rust reports the I/O error to the caller, which logs it and keeps the entry unchanged (no timestamp bump when deletion failed, so the old file is still the image; this avoids showing a stale image under a fresh key).
