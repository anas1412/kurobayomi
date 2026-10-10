# Enhanced trackers: known differences in this release

Kurobayomi keeps the same tracker features as Aniyomi, with the differences below. Some of them fix bugs in the
original app, and some are limits of this first release.

## Kavita

- Covers are not shown yet. The Kavita cover address could not be checked against a live server, so the app
  leaves the cover empty for now.
- Status is always taken from the server when you refresh. Changing the status in the app does not change it on
  Kavita.
- A series with no pages and no pages read is no longer marked as completed. It counts as unread.

## Komga and Suwayomi

- Changing the status in the app does not change it on the server. The status always comes from the server when
  you refresh.
- Suwayomi checks the entry address when it decides whether a track belongs to a manga, the same way as the other
  trackers do.

## Jellyfin

- Marking progress marks every unwatched episode up to your last seen episode as played. Before, only one episode
  was marked.
- Episodes are read in order from the server, so the progress count is correct for a season.
- Seasons with more than 200 episodes are read in full, one page at a time.
- "Played" dates are written in UTC, so they are right in every timezone.
- Jellyfin servers behind a reverse proxy with a path prefix are supported.

## Manga trackers in general

- Refreshing an entry keeps your own score, dates and private setting. Only the status, the chapters read and the
  totals come from the server.
