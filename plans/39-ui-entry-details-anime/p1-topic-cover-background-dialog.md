# Topic: cover and background images dialog, and AnimeImageScreenModel

Deep dive on `app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt` (247 LOC) and `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt` (207 LOC). Paths are repo-relative to `/home/blackbox/aniyomi-src`.

## 1. Entry points

- Cover tap in the info box (`AnimeInfoBox` cover `onCoverClick`) sets `AnimeScreenModel.Dialog.FullImages`. The host in `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:328-361` does: `rememberScreenModel { AnimeImageScreenModel(successState.anime.id) }`; collects `sm.state`; shows `LoadingScreen` until the anime is non-null; then calls `AnimeImagesDialog(anime, snackbarHostState = sm.snackbarHostState, pagerState = sm.pagerState, isCustomCover = anime.hasCustomCover(), isCustomBackground = anime.hasCustomBackground(), onShareClick, onSaveClick, onEditClick, onDismissRequest)`.
- The dialog is a `Dialog` with `usePlatformDefaultWidth = false` and `decorFitsSystemWindows = false` (AnimeImagesDialog.kt:98-104). Full-window on Linux too.

## 2. Model state

`class AnimeImageScreenModel(animeId, getAnime = Injekt, imageSaver = Injekt, coverCache = Injekt, backgroundCache = Injekt, updateAnime = Injekt, snackbarHostState = SnackbarHostState(), pagerState = PagerState(pageCount = { 2 })) : StateScreenModel<Anime?>(null)`.

- `state` (`Anime?`): kept in sync by `getAnime.subscribe(animeId)` in `init` (launchIO, `collect` updates the state). So after a cover edit the dialog shows the new `lastModified` value without a manual refresh.
- `isCover` (private getter): `pagerState.currentPage != 1`. Page 0 is the cover; any other page value is treated as cover too (only pages 0 and 1 exist).
- `pagerState.pageCount = 2`.

## 3. Pages and controls (AnimeImagesDialog.kt:65-236)

Derived values:
- `isCover = pagerState.currentPage != 1`.
- Arrow icon: `KeyboardArrowRight` on the cover page, `KeyboardArrowLeft` on the background page.
- `(editStringResource, alternateStringResource)`:
  - cover page: (`action_edit_cover` "Edit cover", `action_edit_background` "Edit background").
  - background page: (`action_edit_background` "Edit background", `action_edit_cover` "Edit cover").
- Switch click: `scope.launchUI { pagerState.animateScrollToPage(1 - pagerState.currentPage) }`. Its content description is `alternateStringResource`.

Bottom bar (`Scaffold` with `containerColor = Transparent`, `navigationBarsPadding`, 4 dp padding):

| Pill | Button | Icon | Description | Action |
|---|---|---|---|---|
| left | Close | `Close` | `action_close` "Close" | `onDismissRequest` |
| left | Switch | arrow (above) | `alternateStringResource` | switch page |
| right | Share | `Share` | `action_share` "Share" | `onShareClick` |
| right | Save | `Save` | `action_save` "Save" | `onSaveClick` |
| right | Edit | `Edit` | `editStringResource` | see 3.1 |

Each pill: `clip(MaterialTheme.shapes.extraLarge)`, background `colorScheme.background` at 0.95 alpha.

### 3.1 Edit button

- If the current page has a custom image (`(isCover && isCustomCover) || (!isCover && isCustomBackground)`), tapping opens a `DropdownMenu` (offset 8 dp, 0 dp) with:
  - `MR.strings.action_edit` "Edit" → `onEditClick(EditCoverAction.EDIT)`.
  - `MR.strings.action_delete` "Delete" → `onEditClick(EditCoverAction.DELETE)`.
- Otherwise tapping calls `onEditClick(EditCoverAction.EDIT)` directly, which opens the picker described in section 1 (`AnimeScreen.kt:343-350`).
- `onEditClick == null` hides the Edit button entirely.

`EditCoverAction` (`app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt`): `EDIT`, `DELETE`.

Observed: Delete has no confirmation step.

### 3.2 Content

- `Dialog` → `Scaffold` (transparent) with `SnackbarHost(snackbarHostState)`.
- Body: full-size `Box` with `clickableNoIndication(onDismissRequest)`; inside, `HorizontalPager(pagerState)` with one page per index.
- Each page is an `AndroidView` (`ReaderPageImageView`, the Android zoomable image view) with `onViewClicked = onDismissRequest`, `clipToPadding = false`, `clipChildren = false`.
- Image loading (`update` block):
  - `ImageRequest.Builder(context).data(anime).useBackground(page == 1).size(Size.ORIGINAL).memoryCachePolicy(CachePolicy.DISABLED)`.
  - On result: `image.asDrawable(resources)`; if it is a `BitmapDrawable`, copy the bitmap to `Bitmap.Config.HARDWARE` (comment: SSIV must read the image fully and memory-cache bitmaps cannot be used); `view.setImage(copy, Config(zoomDuration = 500))`.
  - `view.updatePadding(top = statusBarPx, bottom = bottomPx)` so the image is not hidden under the bars.
- Visible state while loading: the view is blank (no placeholder).

## 4. Save, share, edit, delete (AnimeImageScreenModel.kt)

### 4.1 Save (`saveImage(context)`, lines 59-85)
- Message keys: cover → `cover_saved` "Cover saved" / `error_saving_cover` "Error saving cover"; background → `background_saved` "Background saved" / `error_saving_background` "Error saving background".
- `screenModelScope.launch { try { saveImageInternal(context, temp = false); snackbar(saved, withDismissAction = true) } catch (e) { logcat(ERROR, e); snackbar(error, withDismissAction = true) } }`.
- Defect D-2: `saveImageInternal` returns `null` for a missing bitmap without throwing, so the success snackbar is shown.

### 4.2 Share (`shareImage(context)`, lines 87-107)
- Error keys: `error_sharing_cover` "Error sharing cover" / `error_sharing_background` "Error sharing background".
- `uri = saveImageInternal(context, temp = true) ?: return@launch`; `withUIContext { context.startActivity(uri.toShareIntent(context)) }`.
- No success message. Defect D-15: a null result returns silently.

### 4.3 `saveImageInternal(context, temp)` (lines 115-135)
```
anime = state.value ?: return null
req = ImageRequest.Builder(context).data(anime).size(Size.ORIGINAL).build()      # D-3: no useBackground
return withIOContext {
   drawable = context.imageLoader.execute(req).image?.asDrawable(resources)
   bitmap = drawable?.getBitmapOrNull() ?: return@withIOContext null
   imageSaver.save(Image.Cover(bitmap, name = if isCover "cover" else "background",
                               location = if temp Location.Cache else Location.Pictures(anime.title)))
}
```
- The source comment `Handle animated image` (`AnimeImageScreenModel.kt:125`) marks animated images as unfinished. Rust: static frames only; GIF saves the first frame (documented deviation).
- `imageSaver.save` returns a `Uri`; on desktop it returns a file path.

### 4.4 Edit (`editImage(context, data: Uri)`, lines 143-159)
```
anime = state.value ?: return
launchIO {
  context.contentResolver.openInputStream(data)?.use {           # D-13: open is outside try
     try {
        if isCover: anime.editCover(Injekt.get(), it, updateAnime, coverCache)
        else:       anime.editBackground(Injekt.get(), it, updateAnime, backgroundCache)
        notifyImageUpdated(context)
     } catch (e: Exception) { notifyFailedImageUpdate(context, e) }
  }
}
```
- `editCover` and `editBackground` (`app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt:36-...`) write the custom image file and update `cover_last_modified` or `background_last_modified` through `UpdateAnime`.
- A null stream (`openInputStream` returns null) does nothing at all (no message).

### 4.5 Delete custom (`deleteCustomImage(context)`, lines 161-177)
```
animeId = state.value?.id ?: return
launchIO {
  try {
    if isCover: coverCache.deleteCustomCover(animeId); updateAnime.awaitUpdateCoverLastModified(animeId)
    else:       backgroundCache.deleteCustomBackground(animeId); updateAnime.awaitUpdateBackgroundLastModified(animeId)
    notifyImageUpdated(context)
  } catch (e) { notifyFailedImageUpdate(context, e) }
}
```
Note: the page is read at call time (`isCover`). See R-8.

### 4.6 Notifications
- `notifyImageUpdated`: `cover_updated` "Cover updated" or `background_updated` "Background updated". Snackbar with dismiss action.
- `notifyFailedImageUpdate(context, e)`: `notification_cover_update_failed` "Failed to update cover" or `notification_background_update_failed` "Failed to update background". Snackbar first, then `logcat(ERROR, e)` after the snackbar returns (D-14).

All notifications use `screenModelScope.launch` so they run on the main dispatcher.

## 5. Files on disk

| Kind | Location | Name |
|---|---|---|
| Custom cover | `animecovers/custom/` in the app files or external files dir | `md5(animeId)` |
| Custom background | `animebackgrounds/custom/` | `md5(animeId)` |
| Share temp | `Location.Cache` → `context.cacheImageDir` | `cover.<ext>` or `background.<ext>` |
| Save | `Location.Pictures(anime.title)` → public Pictures / `<app name>` / title | `cover.<ext>` or `background.<ext>` |

Extension: from `ImageUtil.findImageType(data)` (magic bytes). Unknown data throws `IllegalArgumentException("Not an image")` and is reported as a save error.

Naming function: `DiskUtil.buildValidFilename("${name}.${ext}")` (file name only). Directory is not sanitised (D-4).

See p1-data-and-config.md section 5 for the full table and Linux paths.

## 6. Rust design

Module: `src/anime_details/images/`.

```rust
pub enum ImageKind { Cover, Background }

pub struct ImageModel {
    anime_id: i64,
    title: String,                    // for the save directory
    cover_dir: PathBuf,               // .../animecovers
    background_dir: PathBuf,          // .../animebackgrounds
    db: DbHandle,                     // UpdateAnime equivalent (area 04)
    http: reqwest::Client,            // remote image fetch (area 12)
    saved_dir: PathBuf,               // $XDG_PICTURES_DIR/Kurobayomi
    tx: async_channel::Sender<UiEvent>,
}

impl ImageModel {
    pub async fn load(&self, kind: ImageKind, size: LoadSize) -> Result<gdk::Texture, ImageError>;
    pub async fn save(&self, kind: ImageKind) -> Result<PathBuf, ImageError>;          // Location.Pictures
    pub async fn share_temp(&self, kind: ImageKind) -> Result<PathBuf, ImageError>;    // Location.Cache
    pub async fn edit(&self, kind: ImageKind, src: &Path) -> Result<(), ImageError>;
    pub async fn delete_custom(&self, kind: ImageKind) -> Result<(), ImageError>;
}
```

Rules:
- `load` for `Background` uses `backgroundUrl`; `Cover` uses `thumbnailUrl` (fixes D-3 by construction: the caller cannot forget the kind).
- `save` and `share_temp` take the kind at call time (R-8).
- `save` returns `Err(ImageError::NoImage)` instead of success when decoding fails (D-2).
- `edit` opens the file inside the `Result` and maps every failure to `ImageError::UpdateFailed(kind)` (D-13).
- After `edit` or `delete_custom` succeeds, the model sends `UiEvent::ImageUpdated(kind)` and the view reloads the picture with the new `last_modified` key (R-7).
- Messages: `UiEvent::Snackbar(...)` with the keys from p1-ui-parity.md section 10.
- Logging: `tracing::error!` before sending the snackbar (D-14).

Image type detection: `infer::get(&bytes)` returning MIME, mapped to extension (`image/jpeg` → `jpg`, `image/png` → `png`, `image/webp` → `webp`, `image/gif` → `gif`); anything else is `ImageError::NotAnImage`. Equivalent to `ImageUtil.findImageType`.

Decoding: `gdk::Texture::from_bytes(&glib::Bytes)` on the GTK thread; decoding the full image on tokio is avoided by passing bytes (pitfall 5 in p1-rust-design.md).

Zoom: `gtk::Picture` in `gtk::ScrolledWindow`; `gtk::GestureZoom` for pinch; Ctrl+scroll adjusts a zoom factor; double click resets zoom to fit. The Kotlin zoom duration is 500 ms (`Config(zoomDuration = 500)`); Rust uses 250 ms CSS-like animation for the reset (Proposal).

Window: `gtk::Window` modal, transient for the main window, sized to the monitor work area, `gtk::Stack` with two pages (cover, background), bottom bar as an overlay with the two pills described in section 3. Escape or click on empty area closes. The page switch animates with `gtk::StackTransitionType::SlideLeftRight`, 250 ms.

## 7. Behaviour checklist for tests

- T-I1..T-I14 (p1-edge-cases-and-tests.md section 5.8) cover save, share, edit, delete, labels, menus, cache bump, kind capture, temp cleanup.
- Manual: switching pages changes the arrow, the switch label and the Edit label (T-I10).
- Manual: a custom cover edit updates the info box cover without reopening the page (live state via `getAnime.subscribe`; Rust: model event to the info box).

## 8. Open items

- OD-3: confirm before delete on Linux (proposal: yes).
- OD-8: animated GIF covers: the Kotlin save path decodes one bitmap and drops animation (source comment `Handle animated image`, `AnimeImageScreenModel.kt:125`, marks it as unfinished). Proposal: save the original bytes for GIF, so the saved file keeps its animation; the display path stays static.
