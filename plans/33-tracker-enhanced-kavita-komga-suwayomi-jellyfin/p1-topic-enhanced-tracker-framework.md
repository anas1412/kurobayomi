# Topic: how enhanced trackers live and die

Scope: the four enhanced trackers (Kavita, Komga, Suwayomi, Jellyfin). Source root `/home/blackbox/aniyomi-src`. Each claim cites the Kotlin file and line. Defect ids refer to `p1-behavior.md` section 6.

## 1. What makes a tracker "enhanced"

An enhanced tracker never shows a search dialog to the user. It binds automatically to entries that come from one specific source, and it talks to the same server that the source already talks to. Two interfaces mark it:

- `EnhancedMangaTracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt:12`) with `accept`, `getAcceptedSources`, `loginNoop`, `match`, `isTrackFrom`, `migrateTrack`.
- `EnhancedAnimeTracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt:12`), the same for anime.

Kavita, Komga and Suwayomi implement the manga one. Jellyfin implements the anime one (`jellyfin/Jellyfin.kt:20`).

The accepted-source check compares the source's **fully qualified class name** with a hard-coded list (`EnhancedMangaTracker.kt:16-18`). The lists are:

| Tracker | Accepted class |
|---|---|
| Komga | `eu.kanade.tachiyomi.extension.all.komga.Komga` |
| Kavita | `eu.kanade.tachiyomi.extension.all.kavita.Kavita` |
| Suwayomi | `eu.kanade.tachiyomi.extension.all.tachidesk.Tachidesk` |
| Jellyfin | `eu.kanade.tachiyomi.animeextension.all.jellyfin.Jellyfin` |

These are extension class names. The Rust port maps them to source identity keys (`p1-rust-design.md` section 4).

## 2. Lifecycle

```
user logs in via Settings row ("Enhanced trackers")        SettingsTrackingScreen.kt:231 -> loginNoop()
    -> saves dummy credentials "user"/"pass"              Kavita.kt:95-97 etc.
    -> isLoggedIn becomes true                            BaseTracker.kt:32-34

user adds an entry from a browse screen (or opens one)
    -> bindEnhancedTrackers(entry, source)                AddMangaTracks.kt:81
       for each logged-in enhanced tracker accepting source:
           match(entry)  -> GET entry.url                 (one or more requests)
           bind(track)   -> no-op                         Kavita.kt:74-76
           insertTrack(track)                             AddMangaTracks.kt:91
           syncChapterProgressWithTrack(...)              AddMangaTracks.kt:93-97

user reads a chapter / episode
    -> TrackChapter (manga) or TrackEpisode (anime)       TrackChapter.kt:25-59
       for each track whose lastChapterRead < chapterNumber and service logged in:
           refresh(track)  -> GET (2 to 3 requests)
           update(track, true) -> POST/PUT + refresh
           insertTrack(updated)

user changes progress in the tracking sheet
    -> setRemoteLastChapterRead / setRemoteMangaStatus    MangaTracker.kt:66-89
       -> updateRemote -> update(track) with didReadChapter=false   MangaTracker.kt:111-121
```

Two sync paths exist. The bind path uses `SyncChapterProgressWithTrack` (`SyncChapterProgressWithTrack.kt:21-50`). The read path uses `TrackChapter` (`TrackChapter.kt:25-59`). Both push and pull progress. The pull side marks local chapters read, and the push side writes the remote value.

## 3. Where the status goes (and does not go)

Three of the four trackers compute a local status inside `update()` when `didReadChapter` is true (Kavita `Kavita.kt:61-72`, Komga `Komga.kt:58-70`, Suwayomi `Suwayomi.kt:51-63`). Trace the value:

1. `TrackChapter` passes `updatedTrack.toDbTrack()` (`TrackChapter.kt:43`). `toDbTrack()` creates a new object.
2. `update()` mutates that new object.
3. The return value of `update()` is not assigned anywhere in `TrackChapter`.
4. `insertTrack.await(updatedTrack)` stores the **domain** object, which the mutation never touched (`TrackChapter.kt:44`).

`SyncChapterProgressWithTrack` does the same (`SyncChapterProgressWithTrack.kt:44-46`). So the local status written by `update()` is lost on every real call path. The stored status is the one from `refresh()`, which takes the server's value. That makes the status branch dead code (defect D-02). Trackers that ignore the status branch (Jellyfin, `Jellyfin.kt:60-62`) behave the same way.

Proposal (in `p1-rust-design.md` section 3.5): keep status server-driven. `update` returns the refreshed track, and the caller persists its status.

## 4. Caching and DNS policy

### 4.1 DNS

All four trackers replace the shared DoH resolver with the system resolver: `dns(Dns.SYSTEM)` (`Jellyfin.kt:31`, `Komga.kt:30`, `KavitaApi.kt:26`, `SuwayomiApi.kt:32`). The comment says DoH "breaks IP addressing". Kavita's `getNewToken` is the exception: it uses the base client with DoH (`KavitaApi.kt:40-44`, `Kavita.client` = `BaseTracker.client`, `BaseTracker.kt:19-20`). That is defect D-04.

The DoH choice is applied once when the shared client is built (`core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:42-57`). Suwayomi's client is excluded from DoH by design (see the area:12-network-stack catalogue row for `doh_provider`).

Proposal: system resolver for all four, always (`p1-rust-design.md` section 3.10).

### 4.2 OkHttp HTTP cache (risk)

`NetworkHelper.kt:26-31` installs an OkHttp `Cache` of 5 MiB under `network_cache`. Tracker clients are derived with `newBuilder()` (`Jellyfin.kt:28-33`, `Komga.kt:28-31`, `KavitaApi.kt:26`, `SuwayomiApi.kt:30-33`) and keep it. Every `GET` the trackers send is therefore eligible for a cached response, if the server sends cache headers.

Effect: a progress `GET` can return a stale value, and the refresh that follows an update can show the old state. Kotlin has no code to bypass this. The Rust port uses reqwest with no HTTP cache (`p1-rust-design.md` pitfall P-5). This is a behaviour difference that makes the port more correct. Document it when shipping.

### 4.3 Timeouts

Connect 30 s, read 30 s, call 2 min (`NetworkHelper.kt:23-25`). Kavita's authenticate uses the same client, so a slow Kavita server blocks the first request for up to 2 min per slot (3 slots, so up to 6 min in the worst case if each slot times out on the call timeout). Proposal: 10 s connect and 20 s call timeout for authenticate only. Not a Kotlin behaviour; mark as Proposal.

## 5. Migration semantics

`MigrateMangaDialog.kt:269-272` and `MigrateAnimeDialog.kt:316-319` do this for each tracker:
1. `firstOrNull { it.isTrackFrom(updatedTrack, oldEntry, oldSource) }` finds the track of the old entry.
2. `service.migrateTrack(updatedTrack, newEntry, newSource)` returns the migrated track or `null`.

For the four trackers `migrateTrack` is "copy the entry URL if the new source is accepted, else null" (`Kavita.kt:111-116`, `Komga.kt:109-114`, `Suwayomi.kt:105-110`, `Jellyfin.kt:98-105`).

`isTrackFrom` compares the stored `remoteUrl` with the entry URL and the source acceptance. Suwayomi skips the URL comparison (`Suwayomi.kt:99-103`, D-09). In the migration dialog that means any Suwayomi track matches any Suwayomi-sourced entry. Fix in Rust: compare the URL.

## 6. Settings group

`SettingsTrackingScreen.kt:116-135` partitions the trackers by whether any installed catalogue source is accepted. The "available" ones become rows. The rest are named in the info text. The row action is `loginNoop()`, and the logout action is `logout()` (`SettingsTrackingScreen.kt:229-241`). The mockup's `spec/settings.json` has the four rows hard-coded (see `p1-rust-design.md` section 7.1).

## 7. Credentials

The four trackers keep no credentials of their own. Everything comes from the source. Kavita and Jellyfin read API keys from source preferences. Suwayomi reads server and Basic auth from its source preferences. Komga has no credentials at all and relies on cookies (D-19). The tracker's own "username/password" pair is a dummy (`"user"`, `"pass"`), used only to make `isLoggedIn` true. Proposal: replace the dummy with a flag (`p1-data-and-config.md` section 5.2).

## 8. Error surfaces

- Exceptions from `update()` in `updateRemote` become a toast with `e.message` (`MangaTracker.kt:117-119`). That is how the English text of `IOException("Please log in through the extension")` reaches the user.
- Exceptions in `SyncChapterProgressWithTrack` and `SyncEpisodeProgressWithTrack` are logged at WARN and dropped (`SyncChapterProgressWithTrack.kt:47-49`).
- Exceptions in `TrackChapter` are queued (`delayedTrackingStore`, `TrackChapter.kt:46-50`) and retried by a worker (area:27-tracking-core).
- `match` swallows every exception (D-29). The bind flow then sees `null` and does nothing, with no message.

## 9. Concurrency

`bindEnhancedTrackers` runs inside `withNonCancellableContext` (`AddMangaTracks.kt:81-82`). A page that closes does not stop a bind in progress. `TrackChapter` launches one `async` per track (`TrackChapter.kt:37-54`) and awaits all. The Rust port follows area:27-tracking-core (`p1-rust-design.md` section 5).

## 10. Checklist for a new enhanced tracker

1. Pick the interface (manga or anime).
2. List the accepted source class names and map them to source identities.
3. Decide the login behaviour (dummy flag, or a real secret in `SecretStore`).
4. Implement `match` from the source URL, with errors returned, not swallowed.
5. Implement `update` as write-then-refresh, and let the caller persist the server's status.
6. Implement `refresh` copying only server-owned fields.
7. Implement `isTrackFrom` comparing the URL and source acceptance.
8. Implement `migrateTrack` copying the URL when the new source is accepted.
9. Use the system resolver and no HTTP cache.
10. Add the tracker to the settings group partition.
