# Built-in sources

Three sources ship inside the app as Rust crates. Nothing is installed: they are listed in Browse from the first start.

| Source | Kind | Language | Id | Crate |
|---|---|---|---|---|
| Weeb Central | manga | en | `2131019126180322627` | `crates/kuro-src-weebcentral` |
| Comix | manga | en | `generate_id("Comix", "en", 1)` | `crates/kuro-src-comix` |
| KickAssAnime | anime | en | `2081771513481539599` | `crates/kuro-src-kickassanime` |

The ids are the ones the Kotlin extensions give, so entries restored from an Aniyomi or Mihon backup match them.

## Where they are wired

- **Start.** `ext_host::start` calls `browse::builtin::start` right after the recipe sources. It builds the three sources
  over one app network client and installs the image service (`entry_core::cover_view::install_images`), so remote
  covers draw through the image cache: in the Browse grids, the library and the entry pages.
- **Manga Browse.** `browse::directory::current()` returns the built-in directory in front of the installed one (the
  recipe sources). The Sources list, a source's page (Popular, Latest, search text and the source's own filter sheet),
  global search and the migration picker all read it. Each listed card is a library row of the source
  (`network_to_local_manga`), inserted as a non-favourite when new.
- **Anime Browse.** The anime directory, the anime Sources list (`ext_host::anime_sources`) and the anime entry page
  (`live_anime_sources`) add the built-in anime sources after the recipe ones.
- **Manga entry page.** `entry_core::backend::SourceBackend` fetches the details (written with the library refresh rule,
  `refresh_update`) and the chapter list (the domain's chapter sync) for any source the directory hands out through
  `SourceDirectory::manga_source`. A source it does not know reads as before (not installed). Add to library writes the
  favourite flag on the same row.
- **Anime entry page.** `RemoteAnimeBackend` over the live anime sources, as for the recipe sources.
- **Errors.** Every source call runs on the runtime, never on the GTK thread. The listing calls and the manga details
  and chapter calls run under `kuro_source::isolate`, so a panicking source is an error there. A failed listing shows
  the listing's error state with Retry; a failed manga details or chapter fetch shows a banner with Retry; a failed
  anime refresh shows a snack with Retry. The sources keep their own rate limits (one request at a time, 1 to 2 seconds
  apart), so a global search never floods a site.

## For the reader and the player

- A manga chapter opens `reader::launch::open_chapter(manga_id, chapter_id)` (by ids: a browsed manga is not a
  favourite, so the old title lookup would not find it). The reader's page loader can take the source with
  `browse::directory::current()?.manga_source(manga.source)` and the image headers with
  `browse::builtin::current()?.image_headers(source_id)` (Weeb Central: `Referer` and `Accept`; Comix: none).
- Comix page images also need `kuro_src_comix::descramble::unscramble` and the alt-host retries (crate README).
- An anime episode still goes to `screens::open_media(true, title, name)`. The player needs the episode's videos from
  the source (`AnimeSource` hosters and videos) and each `Video`'s `headers`.

## Not done

- Deep links (`ResolvableSource` of Weeb Central) are not offered to global search.
- The Comix key refresh (`capture.js` in a web view) has no host call.
- The source settings screens: none of the three declares settings yet.
- The recipe manga sources still use the stub entry backend: their directory does not hand out its sources
  (`manga_source`), which is a three-line method in `src/recipe_sources.rs`.
