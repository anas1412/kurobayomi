# Topic — Intents, URIs and deep-link resolution

Deep dive for area 36. Covers the Android intent surface (MainActivity, the two trampoline activities, deep-link screens and models) and its Linux replacement. Full behaviour: `p1-behavior.md` §4, §6. Manifest facts: `app/src/main/AndroidManifest.xml`.

## 1. Inbound surface on Android (observed)

| Entry point | Manifest filter | Handled by | Result |
|---|---|---|---|
| Launcher | MAIN + LAUNCHER | MainActivity | Home, default tab |
| Shortcut actions | `android.app.shortcuts` (`@xml/shortcuts`) | MainActivity | Constants.SHORTCUT_* routes |
| Backup file | VIEW, schemes file/content, pathPattern `.*\.tachibk` (up to 7 segments) | MainActivity | RestoreBackupScreen |
| Add manga repo | VIEW, scheme `tachiyomi`, host `add-repo` | MainActivity | MangaExtensionReposScreen |
| Legacy add anime store | VIEW, scheme `aniyomi`, host `add-repo` | MainActivity | AnimeExtensionStoresScreen |
| Extension store | VIEW, scheme `aniyomi`, host `extension-store` | MainActivity | AnimeExtensionStoresScreen |
| Anime search (trampoline) | SEARCH, SEARCH_ACTION, `eu.kanade.tachiyomi.ANIMESEARCH`, SEND text/plain | DeepLinkAnimeActivity -> MainActivity | GlobalAnimeSearch + DeepLinkAnime |
| Manga search (trampoline) | SEARCH, SEARCH_ACTION, `eu.kanade.tachiyomi.SEARCH`, SEND text/plain | DeepLinkMangaActivity -> MainActivity | GlobalMangaSearch + DeepLinkManga |
| Extension search | `eu.kanade.tachiyomi.SEARCH` / `ANIMESEARCH` (custom) | MainActivity | GlobalSearch with filter |

The trampolines add `type = ANIME|MANGA`, set `FLAG_ACTIVITY_CLEAR_TOP | NEW_TASK` and forward the same intent. They are `Theme.NoDisplay` and `singleTask`.

Key observation: the trampolines are only reached through SEARCH / SEND, never through VIEW. Shared URLs arrive as SEND text/plain and are placed in `EXTRA_TEXT`, which MainActivity reads as the query (`query = QUERY ?: EXTRA_TEXT`).

## 2. Route table (Linux)

| Source | Parsed to | Pure function output |
|---|---|---|
| `--action=show-library` (SHOW_LIBRARY) | `Shortcut { Library, None }` | `[SelectTab(Library)]`, ready |
| `--action=show-anime --id N` | `Shortcut { AnimeEntry, Some(N) }` | `[PopToRoot, SelectTab(AnimeLib{Some(N)})]` (applying a `SelectTab` with a payload pushes the entry, as HomeScreen does) |
| `--action=show-anime` (no id) | `Shortcut { AnimeEntry, None }` | `Err(MissingId)`, ready |
| `--search "x" --type manga` | `Search { "x", Some(Manga) }` | `[PopToRoot, Push(GlobalSearch{Manga,"x"}), Push(DeepLink{Manga,"x"})]` |
| `--search ""` | `Search { "", _ }` | `[]`, ready |
| file `x.tachibk` | `OpenFile` | `[PopToRoot, Push(RestoreBackup{x})]` |
| `tachiyomi://add-repo?url=U` | `OpenUri` | `[PopToRoot, Push(ExtensionStores{Manga,U})]` |
| `aniyomi://add-repo?url=U` | `OpenUri` | `[PopToRoot, Push(ExtensionStores{Anime,U})]` |
| `aniyomi://extension-store?url=U` | `OpenUri` | `[PopToRoot, Push(ExtensionStores{Anime,U})]` |
| notification id I group G | `Notification { I, G }` | `[]` plus withdraw, ready |

Note: `ExtensionStores` with `Kind::Manga` maps to `MangaExtensionReposScreen`, and with `Kind::Anime` maps to `AnimeExtensionStoresScreen` (behaviour §4.4).

## 3. Deep-link resolution (both kinds)

Shared algorithm (anime shown; manga differs in step 4 only):

1. Start `Loading`.
2. `source = first(resolvable sources where get_uri_type(query) != Unknown)`. None -> `NoResults`.
3. `entry = source.get_anime(query)` -> local by `(url, source_id)`, else insert from network (`NetworkToLocalAnime`). None -> `NoResults`.
4. If `get_uri_type(query) == Episode`: local episode by `(url, anime_id)`; else update from remote with fetch, then find by URL. Manga: local chapter; else `get_chapter_list` -> `SyncChaptersWithSource` -> find by URL.
5. Result: `Result(entry, child_id?)`.
6. UI: `NoResults` -> replace with GlobalSearch(query). `Result` without child -> replace with the entry page (`from_source = true`). `Result` with child -> pop and open the player or reader at the child.

Rust (proposal): `resolve_anime(query, ctx) -> Result<Resolved, ResolveError>`, run on tokio, result sent to the GTK thread with `async_channel`. Errors are a state, not a silent stop (Kotlin defect, E35).

Gaps to avoid copying:
- The Result branch performs navigation in composition (E37). Use a one-shot effect.
- Anime child opens `PlayerActivity` directly, bypassing the external-player setting. Rust routes through the player router (T-36a-058).

## 4. Desktop mapping

| Android | Linux |
|---|---|
| Manifest intent filter | `.desktop` `MimeType`, `Exec=kurobayomi %U`, `[Desktop Action]` |
| Trampoline activity | not needed: `route_request` receives the request directly |
| `Intent.extras` | `LaunchRequest` enum |
| `FLAG_ACTIVITY_CLEAR_TOP` | `PopToRoot` action |
| `singleTask` | GApplication single instance |
| `Theme.NoDisplay` | not needed (no window for routing) |

Install-time registration: the `.desktop` file must list `MimeType=application/x-tachibk;x-scheme-handler/tachiyomi;x-scheme-handler/aniyomi;` and `x-scheme-handler/aniyomi` covers both `add-repo` and `extension-store` hosts. `update-desktop-database` is run by the package post-install hook (packaging, area 54).

## 5. Tests (map to T-36a-010..014, 055..057)
- Table test of `route_request` for every row in §2.
- Resolve tests with fake sources: source selection, NoResults, Entry with and without child, error path.
- Smoke test: `kurobayomi --search "https://example.test/anime/1" --type anime` opens the search page and the resolve page.
- Negative: `--type both` produces an error toast and no crash.

## 6. Security notes
- Query strings from external apps are untrusted; they are only used as source-resolution input and as a search string, never as a file path or a shell argument. Passing `--search` values to `Command` must use argv arrays, not a shell.
- `.tachibk` files are parsed by the backup restore screen (area 22) only after the user confirms; the route does not read the file.
- Open-in-browser must not reopen Kurobayomi for its own schemes (behaviour §12.1 forced default browser); filter `tachiyomi://` and `aniyomi://` before launching.
