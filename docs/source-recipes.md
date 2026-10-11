# Source recipes

A recipe is one TOML file that turns a website into a manga or an anime source. It holds URL templates and CSS
selectors and nothing else: no code runs. The engine is the `kuro-source-scrape` crate.

Put recipes in the scrapers folder of the app (`scrapers_dir()` in `kuro-platform`, the `scrapers` folder inside the
platform data directory), one file per source, named `<id>.toml`. At start the app reads every `*.toml` there. A manga
recipe shows up in Browse as a manga source, an anime recipe as an anime source. A file that is not a valid recipe is
skipped and logged with the reason, and so is a second recipe with an id already used.

A repository link is the way recipes reach this folder: the index lists each recipe with its sha256, and the file is
fetched on first use and checked against it (`kuro-extensions` `repo` module). The app-side link flow is not built yet,
so until it is, copy a recipe into the folder by hand.

## Rules

- **Unknown keys are errors.** So are selectors and regexes that do not compile, and a missing operation.
- **`base_urls` are the only hosts the recipe may request.** The first one is the site root: stored links are joined to
  it. A request to any other host is refused before it is sent, and a redirect to another host is an error. (The files
  the page points to, covers, images and videos, are loaded by the app, not by the recipe, so their hosts need not be
  listed.) List each host exactly; `example.org` does not cover `www.example.org`.
- **An empty value counts as no match.** A required value (`title`, `url`, `name`, `image`) with no match fails the
  whole page, so a wrong selector is loud, and the message names the item. Optional values are simply left out.
- **Links are resolved against the page** (or its `<base href>`), then stored without the host (`/series/blue-lake`).
  Covers, page images and videos stay full addresses.
- **Listings need a way to find the next page.** Give `next_page` (a selector that matches when there is a next page) or
  `last_page` (a value holding the last page number). Give neither and the listing has one page.
- **The engine runs requests one at a time**, at least `rate_limit_ms` apart (default 0).

## Top level

| Key | |
|---|---|
| `id` | letters, digits, `-`, `_`; also the file name |
| `name`, `lang` | shown in Browse; with `version` they make the source id, so do not rename a published recipe |
| `kind` | `"manga"` or `"anime"` |
| `version` | optional, default 1; raise it to make a new source id |
| `base_urls` | the allowed hosts, see above |
| `headers` | optional table sent with every request (a `User-Agent` here replaces the app's) and handed to the player for videos. `Host` is not allowed. Covers and page images are loaded with them too (except the `User-Agent`), so a `Referer` check on the image host passes |
| `rate_limit_ms` | optional, at most 60000 |
| `[urls]` | `popular` (required), `latest`, `search`: a path starting with `/`, or a full address on a listed host. `{page}` (starts at 1) and `{query}` (search text, percent-encoded) are the only placeholders |
| `[tests]` | optional, for the checker: a fixture file per operation, `query` (the text a live search uses, default `a`) and `item_url` (a regex the first chapter or episode address must match, else the check fails) |

`latest` and `urls.latest` go together, and so do `search` and `urls.search`. Without them the source has no Latest or
no search.

## Values

Wherever a value is read it is a table (`title = { select = "h3 a" }`) with these optional keys:

| Key | |
|---|---|
| `select` | CSS selector, searched below the current element. Left out, the element itself is used (so `url = { attr = "href" }` reads the item's own `href`) |
| `nth` | which match to take, 0 is the first (the default). This is Jsoup's `a:eq(1)`: `{ select = "dd", nth = 1 }` |
| `all` | take every match and join them with `join` (default `", "`). Not together with `nth` |
| `attr` | read this attribute. Left out, the text is read (whitespace collapsed) |
| `regex` | applied to the text or the attribute: the first group, or the whole match when there is no group. No match, no value. Rust `regex` syntax (no look-around) |
| `format` | applied last: `{}` is replaced by the value, so a bare id becomes a link: `{ attr = "value", format = "/index.php?nro={}" }` |

## Operations

Every operation is a table named after it. `items` selects one element per result.

| Table | Keys |
|---|---|
| `popular`, `latest`, `search` | `items`; `title`, `url` (required); `cover`; `next_page` (selector) or `last_page` (a value, read as an integer) |
| `details` | all optional: `title`, `author`, `artist`, `description`, `genre` (use `all = true`), `cover`, `status`. `status` needs `[details.status_map]`: a status name (`unknown`, `ongoing`, `completed`, `licensed`, `publishing_finished`, `cancelled`, `on_hiatus`) with the words that mean it, matched anywhere in the lower-cased text; when several names match, the first in the order just listed wins (`ongoing` before `completed`) |
| `chapters` (manga) or `episodes` (anime) | `items`; `name`, `url` (required); `number`, `scanlator`; `date` with `date_format` (a chrono format such as `%Y-%m-%d`, or `unix` or `unix_ms`; a date that does not fit is left out); `reverse = true` when the site lists the oldest first (the app wants the newest first) |
| `pages` (manga only) | `items` (one element per page); `image` |
| `videos` (anime only) | `url`; `items` (one video per element; without it `url` is read from the whole page, and `all = true` gives several); `title`; `default_title` (default `Video`). The address is returned as the page gives it, a direct file or an HLS playlist. There are no extractors: a page that hides its stream behind a player script is not covered |

A manga recipe needs `chapters` and `pages` and may not have `episodes` or `videos`; an anime recipe is the other way
round. `details` is needed in both. Details, chapters and pages are read from the stored link of the entry (joined to the
first base), so the details and the chapter list come from the same page.

To read a stream address out of a script, pick the whole page with a regex:
`url = { select = "script", all = true, regex = "file: \"(https?://[^\"]+\\.m3u8)\"" }`.

## Manga example

```toml manga
id = "example-manga"
name = "Example Manga"
lang = "en"
kind = "manga"
base_urls = ["https://manga.example.org"]
rate_limit_ms = 1000

[headers]
Referer = "https://manga.example.org/"

[urls]
popular = "/popular?page={page}"
latest = "/latest?page={page}"
search = "/search?q={query}&page={page}"

[popular]
items = "div.series-card"
title = { select = "h3 a" }
url = { select = "h3 a", attr = "href" }
cover = { select = "img", attr = "data-src" }
next_page = "a.next"

[latest]
items = "ul.updates li"
title = { select = "a.name" }
url = { select = "a.name", attr = "href" }
cover = { select = "img", attr = "src" }
last_page = { select = "span.pages", regex = "of (\\d+)" }

[search]
items = "div.series-card"
title = { select = "h3 a" }
url = { select = "h3 a", attr = "href" }
cover = { select = "img", attr = "data-src" }
next_page = "a.next"

[details]
title = { select = "h1.title" }
author = { select = "dl dd", nth = 0 }
artist = { select = "dl dd", nth = 1 }
description = { select = "div.summary p", all = true, join = "\n\n" }
genre = { select = "ul.genres li", all = true }
cover = { select = "div.poster img", attr = "src" }
status = { select = "span.status" }

[details.status_map]
ongoing = ["ongoing"]
completed = ["completed", "finished"]

[chapters]
items = "ol.chapters li"
name = { select = "a" }
url = { select = "a", attr = "href" }
number = { select = "a", regex = "Chapter ([0-9.]+)" }
date = { select = "time", attr = "datetime" }
date_format = "%Y-%m-%d"
reverse = true

[pages]
items = "div.reader img"
image = { attr = "data-src" }

[tests]
popular = "popular.html"
latest = "latest.html"
search = "search.html"
details = "series.html"
chapters = "series.html"
pages = "chapter.html"
query = "one piece"
```

## Anime example

```toml anime
id = "example-anime"
name = "Example Anime"
lang = "en"
kind = "anime"
base_urls = ["https://anime.example.org"]
rate_limit_ms = 1000

[headers]
Referer = "https://anime.example.org/"

[urls]
popular = "/anime?sort=popular&page={page}"
search = "/search?keyword={query}&page={page}"

[popular]
items = "div.film"
title = { select = "a.film-name" }
url = { select = "a.film-name", attr = "href" }
cover = { select = "img", attr = "data-src" }
next_page = "li.next a"

[search]
items = "div.film"
title = { select = "a.film-name" }
url = { select = "a.film-name", attr = "href" }
cover = { select = "img", attr = "data-src" }
last_page = { select = "div.pager", regex = "/ (\\d+)" }

[details]
title = { select = "h1" }
description = { select = "p.plot" }
genre = { select = "a.genre", all = true }
cover = { select = "img.poster", attr = "src" }
status = { select = "span.airing" }

[details.status_map]
ongoing = ["airing", "ongoing"]
completed = ["finished"]

[episodes]
items = "ul.episodes li a"
name = {}
url = { attr = "href" }
number = { regex = "Episode (\\d+)" }

[videos]
items = "video source"
url = { attr = "src" }
title = { attr = "label" }
default_title = "Video"

[tests]
popular = "popular.html"
search = "search.html"
details = "show.html"
episodes = "show.html"
videos = "episode.html"
query = "frieren"
```

## Checking a recipe

Save the pages the recipe reads (one per operation, as a browser shows them) and name them in `[tests]`. Then run

```
cargo run -p kuro-source-scrape --example check_recipe -- recipe.toml fixtures/
```

It prints what every operation returned from its fixture: the entries, the details it found, the chapters or episodes,
the pages or videos. It exits with an error when an operation fails, returns nothing, or has no fixture.

```
cargo run -p kuro-source-scrape --example check_recipe -- recipe.toml fixtures/ --live
```

runs the operations against the real site once, one request at a time with a second between them and a plain user
agent: popular, latest, search, the first entry's details and chapters or episodes, and the first chapter's pages or the
first episode's videos.

## Not in this version

Requests with a body (POST), pages loaded by script, a Cloudflare
challenge, filters, relative dates such as "3 days ago", extractors for video hosts, and seasons.
