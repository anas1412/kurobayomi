# 19-extensions-anime: UI parity (extension stores page and dialogs)

This file covers the only UI in scope: the extension stores screen and its dialogs. The Browse tab that lists extensions, the install buttons on it, and the untrusted dialog are outside this scope (areas 40 and 41). The untrusted dialog's behaviour is recorded in `p1-behavior.md` section 6.4.

Sources (repo-relative to `/home/blackbox/aniyomi-src`):
- Screen entry and dialogs routing: `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt`
- Screen model and dialog state: `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt`
- Scaffold, app bar, FAB, empty state: `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresScreen.kt`
- List and row: `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt`
- Dialogs: `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt`
- FAB: `app/src/main/java/eu/kanade/presentation/category/components/CategoryFloatingActionButton.kt:16-28`
- FAB expansion rule: `presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt:10-19`
- Clipboard: `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:39-55`
- Open in browser: `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:71-86`

Kurobayomi target: GTK4 (gtk4-rs 0.11, GTK 4.16+), no libadwaita, the user's GTK theme. The mockup is `src/settings.rs`, `src/ui.rs`, and `spec/settings.json`.

---

## 1. Screen states

| State | Trigger | What is shown | Source |
|---|---|---|---|
| Loading | Screen model initial state `Loading` before the store flow emits | Full-screen loading indicator (`LoadingScreen()`), no app bar | SCREEN:36-39; SMODEL:24, 37-51 |
| Empty | `stores` is empty | App bar, FAB, and `EmptyScreen` with text `You haven't added an extension store yet` (`extensionStoresScreen.emptyLabel`), padded by the scaffold padding | components/AnimeExtensionStoresScreen.kt:60-66 |
| List | `stores` is non-empty | App bar, FAB, list of store cards | components/AnimeExtensionStoresScreen.kt:68-78 |
| Dialog open | `dialog != null` | Overlay dialog on top of the screen (3 variants, section 4) | SCREEN:54-83 |

Transitions: the store flow (`GetAnimeExtensionStores.subscribe()`) updates the list in place. The first emission moves `Loading` to `Success`. Later emissions copy the new list and keep the open dialog (SMODEL:41-48). Adding or deleting a store is reflected by the flow, so the list does not need a manual reload.

Error state: there is no full-screen error state. Errors appear inside the Create or Confirm dialog as supporting text (section 4).

---

## 2. Scaffold and app bar

### 2.1 App bar (components/AnimeExtensionStoresScreen.kt:37-52)
- `Scaffold` with `topBar` that receives `scrollBehavior` and renders `AppBar(navigateUp, title, scrollBehavior, actions)`.
- Title: `stringResource(MR.strings.extensionStores)` = `Extension stores`.
- Navigation: `navigateUp = navigator::pop` (SCREEN:51). A back arrow is shown. This is the Voyager back stack. In Kurobayomi the page is pushed on the GTK nav stack, so the back button returns to the caller (Settings or Browse).
- Actions: one `IconButton` with `Icons.Outlined.Refresh`, `contentDescription = stringResource(MR.strings.action_webview_refresh)` = `Refresh`. Action: `onClickRefresh` → `refreshRepos()` (SMODEL:97-105).

### 2.2 Floating action button (components/AnimeExtensionStoresScreen.kt:53-58)
- `CategoryFloatingActionButton(lazyListState, onCreate = onClickCreate)`.
- It is an `ExtendedFloatingActionButton`. Text `Add` (`action_add`), icon `Icons.Outlined.Add`, icon `contentDescription = ""` (empty, so the label is the accessible name). CategoryFloatingActionButton.kt:21-27.
- Expansion: `expanded = lazyListState.shouldExpandFAB()` (CategoryFloatingActionButton.kt:25). The FAB shows the text when the list is at the top (`firstVisibleItemIndex == 0 && firstVisibleItemScrollOffset == 0`), or after a backward scroll (`lastScrolledBackward`), or when the list cannot scroll forward (`!canScrollForward`). When the user scrolls down, the FAB collapses to the icon only (LazyListState.kt:13-15).
- Empty state: the FAB is still shown, and it is expanded because `canScrollForward` is false.

### 2.3 Content padding (components/AnimeExtensionStoresScreen.kt:71-73)
Content padding = scaffold padding + `topSmallPaddingValues` + horizontal `MaterialTheme.padding.medium`. The horizontal inset is applied to the list, not to each card.

---

## 3. List and row

### 3.1 List (components/AnimeExtensionStoresContent.kt:41-59)
- `LazyColumn` with the list state from the scaffold (so the FAB can read scroll state).
- `contentPadding = paddingValues`.
- `verticalArrangement = Arrangement.spacedBy(MaterialTheme.padding.small)`.
- Each store is one `item` with `Modifier.animateItem()`. Adds, removes, and moves animate placement.
- Order: the order of the `stores` list from the store flow (rowid order, SQL without ORDER BY, behavior 7.9).

### 3.2 Row card (components/AnimeExtensionStoresContent.kt:71-126)
Structure, top to bottom:

1. `ElevatedCard` (default Material 3 elevated container).
2. Header `Row` (fill width). Padding: start `medium`, top `medium`, end `medium`. Vertical alignment centred.
   - Icon `Icons.AutoMirrored.Outlined.Label` (a tag icon), `contentDescription = null` (decorative).
   - `Text(store.name)` with `style = titleMedium`, leading padding `medium`.
3. Action `Row` (fill width), arrangement `End` (actions aligned to the right):
   - Open website: `IconButton` with `Icons.Outlined.Public`, `contentDescription = "Open in browser"`. Action: `openInBrowser(contact.website)`.
   - Discord: only when `contact.discord != null`. `IconButton` with `CustomIcons.Discord`, `contentDescription = null`. Action: `openInBrowser(discord)`.
   - Copy: `IconButton` with `Icons.Outlined.ContentCopy`, `contentDescription = "Copy to clipboard"`. Action: copy `indexUrl`.
   - Delete: `IconButton` with `Icons.Outlined.Delete`, `contentDescription = "Delete"`. Action: open the delete dialog.

**Observed:** the row does not show the index URL, the badge label, or the signing key. Those are visible only through the copy action, or in the delete dialog (name and URL).

**Observed:** the discord button is hidden when there is no discord link. The website button is always shown, even when `website` is empty. The Kotlin code does not check it, so an empty website opens nothing and shows the error from `openInBrowser` (toast of `e.message`, ContextExtensions.kt:84-86).

### 3.3 Copy behaviour (ContextExtensions.kt:39-55)
- `copyToClipboard(label = indexUrl, content = indexUrl)`. Returns early when `content.isBlank()`.
- Writes a plain text clip with `label`.
- On Android 12L and earlier (`SDK_INT <= S_V2`), shows a toast `Copied to clipboard:\n<content truncated to 50 chars with "..." in the middle>` (`copied_to_clipboard`). Android 13 and later show the system confirmation instead.
- On failure, a toast `Failed to copy to clipboard` (`clipboard_copy_error`).

Linux: write the text to the GDK clipboard. Show an in-app banner `Copied to clipboard:` followed by the URL truncated to 50 characters with `...` in the middle (`truncateCenter(50)`, StringExtensions.kt:24-32). Always show the banner, since there is no system confirmation on Linux.

### 3.4 Open in browser (ContextExtensions.kt:71-86)
- `Intent(ACTION_VIEW, uri)`. `forceDefaultBrowser` is false for this screen.
- Exception → toast with `e.message`.

Linux: `gtk::UriLauncher::new(uri).launch(...)` with the window as parent. On error, show a banner with the error text.

---

## 4. Dialogs

All three dialogs are `AlertDialog`s with a title, body, and two buttons (confirm and dismiss). Each is shown from `dialog` state (SCREEN:54-83).

### 4.1 Create dialog (`AnimeExtensionStoreCreateDialog`, AnimeExtensionStoresDialogs.kt:28-101)

Opened by the FAB (`showDialog(Create())`).

| Element | Value | Source |
|---|---|---|
| Title | `Add extension store` (`extensionStoresScreen.addStore.title`) | :49 |
| Text field | `OutlinedTextField`, `fillMaxWidth`, `focusRequester` | :52-55 |
| Field label | `Store index URL` (`extensionStoresScreen.addStoreInput.inputLabel`) | :58 |
| Keyboard | `KeyboardType.Uri` | :69 |
| Lines | `TextFieldLineLimits.SingleLine` | :70 |
| Supporting text | see the rule below | :60-67 |
| Error styling | `isError = errorMessage != null || storeAlreadyExists` | :68 |
| Confirm button text | `Processing…` while `processing`, else `Add` | :79-86 |
| Confirm enabled | `!processing && text.isNotEmpty() && !storeAlreadyExists` | :76 |
| Dismiss button | `Cancel` | :89-93 |
| Initial focus | the field, after a 100 ms delay | :96-100 |

Supporting text rule (:60-67): when `storeAlreadyExists`, show `This extension store already exists`. Otherwise show `*required` (`information_required_plain`). When `errorMessage != null`, it replaces the supporting text entirely (:66, `errorMessage ?: stringResource(msgRes)`).

Duplicate check (:37-42): `derivedStateOf { storeIndexUrls.contains(state.text.toString()) }`. The set is the stored index URLs (SCREEN:60). Exact string match, no trimming, no normalisation.

Confirm action (:75): `onCreate(state.text.toString())`. The screen model then calls `createRepo(url)` (SMODEL:58-92).

Error after confirm: the model sets `errorMessage = throwable.message ?: "unknown error"` and `processing = false` (SMODEL:78-81). The dialog stays open.

**Observed:** the error is the raw exception message (for example `Provided legacy store url is not valid`, or a JSON decoding message). Kurobayomi shows fixed strings (section 6, decision K-3).

### 4.2 Delete dialog (`AnimeExtensionStoreDeleteDialog`, AnimeExtensionStoresDialogs.kt:104-132)

Opened by the row delete button (`showDialog(Delete(store))`).

| Element | Value | Source |
|---|---|---|
| Title | `Remove extension store` (`extensionStoresScreen.deleteStore.title`) | :113 |
| Body | `Do you wish to remove the extension store "%1$s" (%2$s)?` with the store name and index URL | :116 |
| Confirm button | `OK` (`action_ok`). Calls `onDelete()` then `onDismissRequest()` | :118-123 |
| Dismiss button | `Cancel` | :126-130 |

Confirm action: `deleteRepo(indexUrl)` removes the row, then calls `findAvailableExtensions()` (SMODEL:110-115). No progress state. The dialog closes at once; the list updates from the store flow.

### 4.3 Confirm dialog for deep links (`AnimeExtensionStoreConfirmDialog`, AnimeExtensionStoresDialogs.kt:135-192)

Opened by `addFromDeeplink(url)` (SMODEL:117-126), after `aniyomi://add-repo?url=https%3A%2F%2Fexample.org%2Frepo.json` or `aniyomi://extension-store?url=https%3A%2F%2Fexample.org%2Frepo.json`.

| Element | Value | Source |
|---|---|---|
| Title | `Add extension store` | :147 |
| Body text | `Do you wish to add the extension store below?` (`extensionStoresScreen.addStoreDeeplink.bodyText`) | :151 |
| Text field | read-only, prefilled with the URL (`rememberTextFieldState(initialText = storeIndexUrl)`) | :143, :152-156 |
| Supporting text | `storeAlreadyExists` → `This extension store already exists`; else `errorMessage` if set; else none | :156-168 |
| Error styling | `isError = errorMessage != null || storeAlreadyExists` | :169 |
| Confirm button text | `Processing…` while processing, else `Add` | :176-183 |
| Confirm enabled | `!storeAlreadyExists && !processing` | :174 |
| Dismiss | `Cancel` | :187-191 |

Confirm action: `createRepo(dialog.url)` (SCREEN:76). Same error path as the create dialog.

Layout: the body text and the read-only field sit in one `Column` with `spacedBy(4.dp)` (:150).

### 4.4 Dialog routing (SCREEN:54-83)
Only one dialog is shown at a time. The `when` on `successState.dialog` covers `null`, `Create`, `Delete`, `Confirm`. `Confirm` passes `dialog.alreadyExists` from state (computed in `addFromDeeplink` against the stored list at that moment).

---

## 5. Deep link and entry points

- `AnimeExtensionStoresScreen(url: String?)` (SCREEN:20-22). When `url` is non-null, `LaunchedEffect(url) { addFromDeeplink(url) }` runs once per composition key (SCREEN:32-34).
- Entry from the main activity (`MainActivity.kt:553-560`): `navigator.popUntilRoot()` then push the screen with the URL. The URL is `getQueryParameter("url")`, so it is URL-decoded.
- Entry without a URL: Settings (`SettingsBrowseScreen.kt:66`), Browse tab (`AnimeExtensionsScreen.kt:116`, `AnimeExtensionsTab.kt:53`).
- Scheme and hosts (`app/src/main/AndroidManifest.xml:78-94`): `aniyomi://add-repo` (legacy) and `aniyomi://extension-store`. The manga scheme `tachiyomi://add-repo` is a different screen and is not in this scope.

---

## 6. Kurobayomi decisions

Each decision is numbered so that the tasks can cite it.

| ID | Decision | Reason |
|---|---|---|
| K-1 | Replace the FAB with a header button `Add` plus a keyboard shortcut `Ctrl+N` (Proposal). Keep the scrolling collapse behaviour out. | GTK apps have a header bar, not a FAB. The collapse rule is mobile-only. |
| K-2 | Keep all three dialogs with the same fields, labels, enable rules, and body text. Use `gtk::Window` (modal, transient for main window), `gtk::Entry` with `input-purpose` URL. | Parity with the user's existing mental model. |
| K-3 | Map errors to fixed localised strings. Keep `Provided legacy store url is not valid` and `This extension store already exists` as they are. Other errors show `Failed to add extension store`. The raw text goes to the log. | Raw Kotlin exception text is not user-facing copy. |
| K-4 | Clipboard success shows a banner on every platform (see 3.3). | No system confirmation on Linux. |
| K-5 | Refresh button runs the full refresh: store documents and extension lists (behavior 7.5), not only the store rows as Aniyomi does (behavior 12.2). | Aniyomi's refresh does not update the Browse list. Users expect the refresh to be visible there. |
| K-6 | Show per-store failures after a refresh as one banner per failed store: `Could not update store "<name>"`. Aniyomi shows nothing (behavior 7.8). | Silent failure hides broken stores. |
| K-7 | Discord button shown only when a discord value exists (same as Aniyomi). Website button hidden when the website is empty (Aniyomi shows it and fails on click). | Avoids a dead button. |
| K-8 | Store rows show the badge label and a secondary line with the index URL. Aniyomi shows only the name. | Gives the user a way to tell two stores with the same name apart. Proposal. |
| K-9 | Deep-link confirm dialog is opened for an unknown URL only after validation of the URL shape (`http` or `https` scheme). Other shapes show a banner `This link is not an extension store address`. | Aniyomi accepts any string. Proposal. |
| K-11 | Trim whitespace from the typed URL before the duplicate check and before the fetch. Aniyomi does neither (edge case E-UI-02). | Prevents false duplicates and fetch failures from pasted text. Proposal. |
| K-10 | Animation: the Aniyomi `animateItem()` placement animation is kept (GTK `gtk::ListBox` has no equivalent; no animation). The FAB expand animation is dropped with K-1. | GTK has no item placement animation in `ListBox`. |

---

## 7. Keyboard and mouse on desktop (Proposal)

| Action | Key or pointer | Notes |
|---|---|---|
| Open the Add dialog | `Ctrl+N` or the header `Add` button | K-1 |
| Refresh stores | `F5` or `Ctrl+R`, or the header refresh button | `F5` is the usual GTK refresh key |
| Submit Add or Confirm | `Enter` in the URL field | Only when the confirm button is enabled (same rule as the button) |
| Cancel any dialog | `Escape` | Same as Dismiss |
| Delete a store | Row delete button (mouse), or `Delete` key when the row is focused, then confirm dialog | The key does not delete without the dialog |
| Open website | Row website button; `Enter` on the focused button | |
| Copy URL | Row copy button; `Ctrl+C` on the focused row copies the index URL | Row focus rule: `gtk::ListBoxRow` activation |
| Move between rows | `Up` and `Down` arrow keys | Provided by `gtk::ListBox` |

Tooltips (`set_tooltip_text`) are shown for every icon-only button with the same text as the Kotlin `contentDescription`: `Open in browser`, `Copy to clipboard`, `Delete`, `Refresh`. The discord button tooltip is `Discord` (Proposal; Aniyomi uses `null`, which gives no accessible name).

---

## 8. Responsive behaviour

- Desktop: the page content is a `gtk::ScrolledWindow` with a `gtk::ListBox` of `boxed-list` style, `max-width` 720 px, centred (Proposal).
- Narrow window (the mockup switches to a bottom tab bar below 760 px, `src/main.rs`): the list stays full width with a 16 px side margin. No horizontal scrolling.
- Long URLs: the subtitle is ellipsised at the end (`ellipsize = END`), with the full URL in the tooltip.
- Dialog width: `default-width` 420 px, which fits the Aniyomi `AlertDialog` default width on a desktop screen.

---

## 9. Mockup gap list (what `/home/blackbox/kurobayomi` lacks)

Current mockup: `src/settings.rs:13` (`"settings_anime_extension_stores" => cards(id, &["Store A", "Store B"], false)`), `src/settings.rs:134-160` (`cards`), `src/settings.rs:154` (`dialog_of(&i, "edit_button")` placeholder for non-star rows, not used for stores).

| Missing | Current mockup | Needed |
|---|---|---|
| Real store list | Two hard-coded rows, `Store A` and `Store B` | Rows from the store repository, ordered as in 3.1 |
| Store name and URL | Name only | Name, and the index URL as subtitle (K-8) |
| Badge label | None | Shown in the row or the subtitle |
| Website button | `web-browser-symbolic` with tooltip `Open website` (settings.rs:157) | Same icon. Tooltip `Open in browser` (Aniyomi text). Hidden when website is empty (K-7) |
| Discord button | None | Shown when discord exists |
| Copy button | `edit-copy-symbolic` with tooltip `Copy to clipboard` (settings.rs:158) | Copies the index URL. Banner on success (K-4) |
| Delete button | None | `user-trash-symbolic`, tooltip `Delete`, confirm dialog |
| Add button | `icon_button` FAB in `cards` (`Add FAB` per settings.rs:133 comment) | Header `Add` button (K-1) |
| Refresh button | None | Header refresh button (K-5) |
| Empty state | None | `You haven't added an extension store yet` |
| Create dialog | None | Section 4.1, with duplicate check and error text |
| Confirm dialog for deep link | None | Section 4.3 |
| Delete dialog | None | Section 4.2 |
| Deep link `aniyomi://add-repo` and `extension-store` | None | Section 5 and `deeplink.rs` |
| Per-store refresh failures | None | Banners (K-6) |
| Loading state | None | Spinner while the first store list loads |
| Install progress and states on the Browse tab | Out of this scope (area:41-ui-browse-anime) | Not in this file |

---

## 10. Strings

Every string on this page is in `p1-data-and-config.md` section 12, with the Kotlin key and the English value. The new strings of this file (K-3, K-4, K-6, K-9, the tooltips in section 7) are not in Aniyomi and are listed here:

| Key (Kurobayomi, proposal) | English text |
|---|---|
| `anime_stores.error.generic` | `Failed to add extension store` |
| `anime_stores.error.legacy_url` | `Provided legacy store url is not valid` (same as Aniyomi) |
| `anime_stores.error.redirect_loop` | `This extension store points to itself` |
| `anime_stores.error.too_large` | `The extension store index is too large` |
| `anime_stores.error.decode` | `This extension store could not be read` |
| `anime_stores.refresh.failed` | `Could not update store "%s"` |
| `anime_stores.copied` | `Copied to clipboard: %s` |
| `anime_stores.invalid_link` | `This link is not an extension store address` |
| `anime_stores.discord.tooltip` | `Discord` |
