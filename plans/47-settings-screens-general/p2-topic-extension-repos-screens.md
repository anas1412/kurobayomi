# Topic: extension stores and repos screens (state machines)

Scope files: `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/` (both screens, both models, all component files). Paths repo-relative to `/home/blackbox/aniyomi-src`. Full behaviour: `p2-behavior.md` sections 3 and 4. Tasks: T-47b-009 to T-47b-026, T-47b-062, T-47b-063, T-47b-066 to T-47b-069.

## 1. Why two screens exist
Aniyomi keeps two separate stores:
- anime: `extension_store` (index URL is the key; stores contain an extension list or a legacy list).
- manga: `extension_repos` (base URL is the key; signing-key fingerprint is UNIQUE).

They share UI shape (list of cards, Add, Refresh, Delete) but differ in data and dialogs. The Rust port keeps both pages, sharing dialog widgets and the card layout.

## 2. Anime store page

```
            +-----------+
            |  Loading  |  -- store subscription emits --> Ready{stores, dialog: None}
            +-----------+
                                 |
          +----------------------+------------------------+
          |                      |                        |
   add (Create)           delete (Delete)          deep link (Confirm)
          |                      |                        |
  Create{processing}     Delete{store}          Confirm{url, alreadyExists}
          |                      |                        |
   OK -> processing=true   OK -> remove row       OK -> processing=true
          |                  + reload extensions          |
   fetch+upsert                                    fetch+upsert
   |          |                                    |          |
 success    failure                            success    failure
   |          |                                    |          |
 reload +   processing=false,                  reload +   processing=false,
 dismiss    error=text                         dismiss    error=text
```

Rules:
- Dialog slot is single: a new dialog replaces the old one (Kotlin). Rust keeps this.
- Deep link while Loading: queue, apply on Ready (Kotlin drops it: RACE-01).
- `alreadyExists` is exact-match on `index_url` (Kotlin). Rust normalises first (T-47b-062).
- Refresh (`refreshAll`) runs only in Ready; it upserts each stored index. A row deleted during refresh must not come back (RACE-03, T-47b-063).

## 3. Manga repo page

```
  Loading --(subscribeAll emits)--> Ready{repos, dialog: None}

  Create dialog OK -> createRepo(url) (dialog already closed)
     -> InvalidUrl           -> toast "Invalid repo URL"
     -> RepoAlreadyExists    -> toast "This repo already exists!"
     -> DuplicateFingerprint -> dialog Conflict{old,new}
     -> Success              -> (list updates via DB emission)
     -> Error                -> toast "Could not add the repo" (Kotlin: silent)

  Conflict OK (Replace) -> replaceRepo(new), dialog closes
  Delete OK             -> deleteRepo(baseUrl), dialog closes
  Confirm OK (deep link)-> createRepo(url), dialog closes
```

Rules:
- Create closes the dialog before the result arrives (Kotlin behaviour). Errors therefore come only as toasts or as the Conflict dialog.
- Every DB emission currently rebuilds `Ready` and clears the dialog (Kotlin, RACE-02). Rust keeps the dialog unless its target row disappeared.
- Deep-link Confirm uses the index URL in its text; the stored value is the base URL.

## 4. Shared dialog set (Rust)

| Dialog | Widget | Buttons | Enabled rule |
|---|---|---|---|
| Add store (anime) | `gtk::Window` with entry | Cancel, Add | Add: non-empty, not duplicate, not processing |
| Remove store (anime) | `gtk::AlertDialog` destructive | Cancel, OK | always |
| Add store deep link | `gtk::Window` read-only entry | Cancel, Add | Add: not duplicate, not processing |
| Add repo (manga) | `gtk::Window` with entry | Cancel, Add | Add: non-empty, not flagged duplicate |
| Delete repo | `gtk::AlertDialog` destructive | Cancel, OK | always |
| Fingerprint conflict | `gtk::AlertDialog` | Cancel, Replace | always |
| Add repo deep link | `gtk::AlertDialog` | Cancel, Add | always |

Labels, titles and body strings: `p2-behavior.md` section 15.1.

## 5. Card contract (both pages)

- Name row: tag icon + name (ellipsize).
- Actions: website (globe or external link), [Discord, anime only, when set], copy (anime copies `index_url`, manga copies `<base>/index.min.json`), delete.
- Icon buttons have tooltips equal to the Kotlin content descriptions.

## 6. Error catalogue

| Source | User sees | Kotlin text source |
|---|---|---|
| Anime fetch failure | raw exception message, or "unknown error" | `throwable.message ?: "unknown error"` (AnimeExtensionStoresScreenModel.kt:80,84) |
| Anime legacy URL not ending in index.min.json | "Provided legacy store url is not valid" | AnimeExtensionStoreService.kt (area:19) |
| Manga invalid or unreachable | "Invalid repo URL" | `invalid_repo_name` |
| Manga duplicate URL | "This repo already exists!" | `error_repo_exists` |
| Manga duplicate fingerprint | Conflict dialog text | `action_replace_repo_message` |
| Manga DB error | (silent in Kotlin) "Could not add the repo" in Rust | new key |

Rust keeps the anime raw text for parity, but wraps network and HTTP errors (T-47b-067). This is the only behaviour change proposed for error text in this page family.

## 7. Race and concurrency summary

- All state changes happen in the reducer on the GTK thread.
- IO results come through an unbounded `async_channel`.
- Deep links are queued until the first Ready state.
- Add buttons are disabled synchronously on first click.
- Page drop cancels the subscription task; late results are ignored.

## 8. Test hooks
- Reducer tests T-UT-10 to T-UT-15 (manga), T-UT-12/13/14 (anime).
- Integration: T-IT-01 (anime fetch fixtures), T-IT-02 (manga fixtures incl. conflict and empty fingerprint), T-IT-05 (refresh vs delete).
- GTK smoke GS-02 and GS-03.
