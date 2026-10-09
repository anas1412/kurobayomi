# Topic: launch surface on Linux (intents, deep links, file opens, OAuth return) — Aniyomi observed, Kurobayomi decided

Scope: everything that brings the user from outside the window to a page: the MainActivity intent router, the search and share trampolines, the `aniyomi://` and `tachiyomi://` links, the `.tachibk` open, the launcher shortcuts, the tracker OAuth return. Route table and `LaunchRequest` belong to area:36 (`p1-topic-deeplinks-intents.md`); this topic gives the Linux entry points that feed that table, the precedence rules and the open questions.

## 1. Inbound surface on Android (observed, one table)

| Inbound | Android declaration | Handled in | Result |
|---|---|---|---|
| Launcher tap | MAIN + LAUNCHER filter, MainActivity | `MainActivity.onCreate` → `handleIntentAction` (no action) | library tab |
| Launcher long-press shortcut (5) | `app/shortcuts.xml` | `handleIntentAction` `SHOW_*` branches (`MainActivity.kt:471` to `:496`) | tab |
| Notification content intent | `PendingIntent` to MainActivity with `SHOW_MANGA` or `SHOW_ANIME` and `notificationId` | `handleIntentAction` → dismiss → entry tab | entry page |
| Notification button | `PendingIntent.getBroadcast` to NotificationReceiver | `onReceive` | action (section 4 of behaviour) |
| Global search from Google or system | `SEARCH`, `SEARCH_ACTION` on the two trampolines | trampoline → `type` extra → `handleIntentAction` `ACTION_SEARCH` branch | global search + deep-link page |
| Shared text | `SEND` `text/plain` on the two trampolines | same as search, query = whole text | global search + deep-link page |
| Extension search from a source | `eu.kanade.tachiyomi.SEARCH`, `eu.kanade.tachiyomi.ANIMESEARCH` | `INTENT_SEARCH`, `INTENT_ANIMESEARCH` branches | global search with filter |
| Backup file | VIEW with `file` or `content` scheme and `.tachibk` path | `ACTION_VIEW` branch, string suffix test | restore backup screen |
| Add repo | VIEW `tachiyomi://add-repo?url=` and `aniyomi://add-repo?url=` | `ACTION_VIEW` branch | extension repos page |
| Extension store | VIEW `aniyomi://extension-store?url=` | `ACTION_VIEW` branch | anime extension stores |
| Tracker OAuth return | VIEW `aniyomi://<provider>-auth` (exported TrackLoginActivity) | `TrackLoginActivity.handleResult` | token saved, Settings shown |
| Samsung S Pen remote | `REMOTE_ACTION` on reader and player | none (no Kotlin consumer) | none |
| App lock start | none (in-app) | `UnlockActivity` | prompt |

## 2. Linux entry points (Proposal)

| Entry point | Mechanism | Feeds |
|---|---|---|
| Launcher and dock | `.desktop` file, `DBusActivatable=true`, `Exec=kurobayomi %U` | `connect_activate` (no request) |
| Desktop actions | `[Desktop Action]` groups `show-*` | `parse_argv` `--action` |
| Command line | `kurobayomi --action=… --id=… --search=… --type=…`, positional `.tachibk` path and URL | `connect_command_line` → `parse_argv` |
| File open | `application/x-tachibk` MIME, `connect_open` | `OpenFile` |
| URL open | `x-scheme-handler/kurobayomi`, `aniyomi`, `tachiyomi` in `MimeType=`, `connect_open` with URI files | `parse_uri` |
| Notification click | GAction `app.notice` with JSON `AppAction` as target | dispatcher (`src/notice.rs`) |
| Second process | GApplication forwards the command line and open calls to the primary instance | the same queue as the first process |
| Desktop search | GNOME SearchProvider2 and KRunner (P2) | `Search` route |

Precedence (Proposal): launch requests are applied in arrival order. A later request does not cancel an earlier one that is still loading; each request's result is applied in order, and no request is dropped silently. Every failure produces a toast (edge E-16).

Argument grammar (Proposal, fixed):

```
kurobayomi [--action=<name>] [--id=<integer>] [--search=<text>] [--type=manga|anime] [<path.tachibk>] [<url>]...
```

- `--action` names: `show-library`, `show-animelib`, `show-updates`, `show-history`, `show-browse`, `show-extensions`, `show-anime-extensions`, `show-downloads`, `show-anime-downloads`, `show-manga`, `show-anime`. `show-manga` and `show-anime` require `--id`.
- `--type` default `anime`. Any other value is `RouteError::UnknownKind` (never a crash, defect E-14).
- A positional argument is a file when it ends in `.tachibk` after path decoding; an `http://` or `https://` argument is a text search (behaviour section 2.8); a URL with a supported scheme is an OAuth or add-repo route; anything else is `LaunchError::Unrecognised`.

## 3. Scheme and host table (Proposal)

| Scheme | Host | Route | Notes |
|---|---|---|---|
| `kurobayomi` | `add-repo` | manga extension repos (area:20) | primary; `url` query parameter required |
| `kurobayomi` | `extension-store` | anime extension stores | primary; the Aniyomi name is the legacy alias |
| `kurobayomi` | `anilist-auth`, `bangumi-auth`, `myanimelist-auth`, `shikimori-auth`, `simkl-auth` | tracker OAuth (area:27) | see section 6 (open question) |
| `aniyomi` | `add-repo` | anime extension stores | legacy alias (`MainActivity.kt:549` to `:554`) |
| `aniyomi` | `extension-store` | anime extension stores | legacy alias |
| `aniyomi` | `*-auth` | tracker OAuth | legacy alias, same handler |
| `tachiyomi` | `add-repo` | manga extension repos | legacy alias (`MainActivity.kt:556` to `:560`) |

Scheme names are compared case-sensitively after `url::Url` parsing (the parser lower-cases the scheme; host names are lower-case in the table).

## 4. Shortcut to route mapping (Proposal, matches area:36)

The five desktop actions and the notice-only actions map to `ShortcutAction` (area:36 section 2): `show-library` → Library; `show-animelib` → AnimeLib; `show-updates` → Updates; `show-history` → History; `show-browse` → Catalogues; `show-extensions` → Extensions; `show-anime-extensions` → AnimeExtensions; `show-downloads` → Downloads; `show-anime-downloads` → AnimeDownloads; `show-manga` `--id` → MangaEntry; `show-anime` `--id` → AnimeEntry. The duplicate `show_recently_updated` id of the Android shortcut file is not carried (defect E-24).

## 5. Deep-link resolution after routing (summary of behaviour section 3)

- Anime and manga resolution run on the tokio runtime; the result returns to the GTK thread as a one-shot navigation effect.
- The loading page shows "Search…" in the title (`action_search_hint`). A resolve error shows Retry and "Search instead" (Proposal, new string `deeplink_search_instead`).
- The episode or chapter result opens the player or the reader through their routers (area:45 and area:43), so the external-player setting applies (defect E-09).

## 6. Open questions (not decided here; shared with area:27)

These questions block a final decision on the OAuth return. Implementation of the parser does not depend on them.

1. **Redirect URI registration.** Each tracker provider accepts only the redirect URIs registered for its client id. The Aniyomi client ids are registered for `aniyomi://<provider>-auth`. Kurobayomi can use `kurobayomi://<provider>-auth` only after new client registrations exist at AniList, MyAnimeList, Bangumi, Shikimori and Simkl. Until then, the `aniyomi://` alias is the only working redirect and the registrations are a manual step for the user. The decision belongs to area:27 (its topic file `p1-topic-tracker-oauth.md` proposes the custom scheme; the loopback alternative is rejected there).
2. **Scheme ownership.** Two applications can register the same scheme on the desktop. If Aniyomi (Android-derived builds on a desktop, or another client) registers `aniyomi:`, the desktop chooses one handler. Proposal: Kurobayomi registers all three schemes. The settings page (area:36) offers a button that makes Kurobayomi the default handler through `xdg-mime default` after the user confirms; the app never changes the default by itself.
3. **Legacy alias removal.** Keeping `aniyomi://` and `tachiyomi://` costs three lines of parsing. The question is whether to keep them after the first release. Proposal: keep them until the tracker registrations are moved, then drop them in a later release with a note.
4. **Open-in-browser loops.** A link opened from the app in the system browser must not return to the app for its own schemes (area:36 section 6). Proposal: filter `kurobayomi://`, `aniyomi://` and `tachiyomi://` before calling `Opener::open_url`.
5. **Share text as URL.** Behaviour section 2.8 proposes that a shared `http(s)` URL resolves directly. Aniyomi does not do this. Keep as a P2 improvement; no user-visible change for text search.

## 7. Security notes

- Every URL and file path from outside the process is untrusted (any local process can send a GApplication command line; any web page can offer a link). Payload validation of the notice actions (T-53a-018) and of launch arguments applies.
- A `.tachibk` file is never executed or parsed by the route; the restore screen parses it only after the user confirms (area:22).
- OAuth fragments are parsed as form data; the token is not logged (verbose logging prints the route name only).
- `kurobayomi://` links cannot change settings by themselves; only the `*-auth` routes store a token, and only after the provider's own redirect.
