# Topic: page cache lifecycle and the save/share pipeline

Deep dive for area 34. Facts: `p1-behavior.md` §20 (chapter cache), §21-§23 (saving and sharing), §22 (names); rules: `p1-rust-design.md` §6.3, §8; tests: `p1-edge-cases-and-tests.md` §10-§11. Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`.

## 1. Page cache lifecycle (one chapter)

```
open chapter ─► getPages()
                 ├─ page list cached? ─ yes ─► use cached list (no network)
                 └─ no ─► source.getPageList() ─► ReaderPage list (indexes re-based)
loadPage(page i)
                 ├─ status READY and image not cached ─► status QUEUE (re-download)
                 ├─ status ERROR ─► status QUEUE
                 └─ queue page i (priority 1) and next pages (priority 0)
internalLoadPage
                 ├─ imageUrl empty ─► source.getImageUrl()
                 ├─ image not cached ─► status DOWNLOAD_IMAGE ─► source.getImage() ─► putImageToCache()
                 ├─ page.stream = read cached file
                 └─ status READY (or ERROR on any Throwable)
leave chapter (recycle)
                 └─ launchIO putPageListToCache(chapter, pages)   (only online chapters with a list)
```

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt` (behavior §20.7). The page-image entry is shared by the downloader (behavior §20.8): a download of a chapter that was read first copies the cached file and needs no network.

## 2. Size control

- Limit: 100 MiB (`PARAMETER_CACHE_SIZE`, `ChapterCache.kt:209`), LRU.
- Both page lists and images count toward the limit in Rust (design §6.3). In Kotlin the page list is in the same DiskLruCache, so it also counts.
- Auto-clear on launch (`auto_clear_chapter_cache`, default off) empties the cache at start; it does not touch covers (topic cover file §4).
- Manual clear reports the number of files removed and the readable size before and after (ui §7.2).

## 3. Failure paths (reader)

| Failure | Result | User sees |
|---|---|---|
| page list JSON corrupt | treated as miss; network list used | nothing |
| image write fails | page status ERROR; retry on tap | error page placeholder (reader area) |
| source getImage throws | ERROR | error page |
| cancelled while downloading | status is set to ERROR first, then the CancellationException is rethrown (`HttpPageLoader.kt`, `internalLoadPage`); the next subscription re-queues ERROR pages (behavior §20.7) | page returns to queue on next view |

## 4. Save pipeline (reader page)

```
user taps Save in the reader page menu
  └─ ReaderViewModel.saveImage()
       ├─ status must be READY, else return (no feedback)
       ├─ notifier.onClear()                           (cancel previous save notification)
       ├─ filename = generateFilename(manga, page)     (behavior §22.3)
       ├─ relativePath = folder-per-manga ? buildValidFilename(title) : ""
       └─ launchNonCancellable:
            imageSaver.save(Image.Page(stream, name, Location.Pictures(relativePath)))
              ├─ sniff type (else IllegalArgumentException "Not an image")
              ├─ buildValidFilename("<name>.<ext>")
              ├─ mkdirs; write file; media scan (dropped on Linux)
              └─ return URI
            ├─ success: notifier.onComplete(uri) ─► notification with preview, open and share actions
            │           Event.SavedImage(Success(uri))
            └─ failure: notifier.onError(message) ─► "Error" notification
                        Event.SavedImage(Error(e))
```
Rust (design §8.1): same steps; the notification goes through `Notifier::picture_saved` / `picture_error`; the event goes to the reader view model.

## 5. Share pipeline (reader page)

```
Share
  └─ destDir = cacheImageDir (cache/shared_image)
     launch: destDir.deleteRecursively()           (only the last share is kept)
             imageSaver.save(Image.Page(..., Location.Cache))
             Event.ShareImage(uri, page)           (reader opens share intent)
Copy
  └─ same staging, Event.CopyImage(uri)            (clipboard copy)
```
Rust (design §8.3, §10): staging delete, save, then `Sharer::share_file` (open in default viewer, copy URI) or `Sharer::copy_image` (clipboard texture). Deviation D-14: cover shares also clear the staging folder first.

## 6. Covers and backgrounds (save and share)

- Cover save (`MangaCoverScreenModel.saveCover`): decode at Original (`Size.ORIGINAL`, memory cache on), re-encode JPEG quality 100, name = manga title, location = Pictures (no folder). Snackbar `Cover saved` / `Error saving cover`.
- Cover share: same decode, location Cache, then `startActivity(share intent)`; no staging delete in Kotlin (E-IMG-15).
- Anime image save: name `cover` or `background`, folder = anime title; snackbar `Cover saved` or `Background saved`.
- Anime share: `shareImage(context)` → `saveImageInternal(temp = true)` → share intent. Errors: `Error sharing cover` / `Error sharing background`.

## 7. Names

| Caller | Name passed to `save` | Final file name (example) |
|---|---|---|
| Reader page | `<title> - <chapter> - <page>` (byte-limited) | `One Piece - Chapter 1 - 3.jpg` |
| Player frame | `<title> - <episode> - <HH:MM:SS>` | `Show - Episode 2 - 00:12:34.jpg` |
| Manga cover | manga title | `Title.jpg` |
| Anime cover | `cover` | `cover.jpg` |
| Anime background | `background` | `background.jpg` |

The extension comes from the sniffed type, so a `.webp` page saves as `.webp` and a re-encoded cover is always `.jpg` (`ImageSaver.kt:38-39`, behavior §21.4).

## 8. Folder rules

| Caller | Folder under Pictures | Preference |
|---|---|---|
| Reader page | `Aniyomi/<title>/` if `create_folder_per_manga` is on, else `Aniyomi/` | `create_folder_per_manga` (default off) |
| Player frame | `Aniyomi/<anime title>/` always | none |
| Manga cover | `Aniyomi/` | none |
| Anime image | `Aniyomi/<anime title>/` | none |

Linux: `Kurobayomi/` instead of `Aniyomi/` (data §4.2).

## 9. Notification identity

One notification id for all picture saves (`ID_DOWNLOAD_IMAGE`). Each save cancels the previous one first (`onClear`). Two quick saves therefore show one notification with the latest result (behavior §23.2). Rust keeps this (Notifier uses one id, design §10).

## 10. Test anchors

- Chapter cache tests: `p1-edge-cases-and-tests.md` §10.
- Save and share tests: §11.
- Manual script: §14 steps 6 and 7.
