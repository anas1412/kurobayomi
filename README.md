# Kurobayomi

A manga reader and anime player for the Linux desktop, written in Rust with GTK4 (no libadwaita, so your own GTK theme applies).
It is a rewrite of [Aniyomi](https://github.com/aniyomiorg/aniyomi): same library, reader, player, sources and tracking model, built as a native app instead of an Android one.

The name is *kurobako* (黒箱, "black box") plus *-yomi* ("reading"), like Tachiyomi and Aniyomi.

![Kurobayomi mockup](docs/screenshot.png)

## Status: early work in progress

The window you see is a **mockup with placeholder data**. The real features are being built underneath it, crate by crate.
The plan has about 3,800 tasks. Treat everything as unfinished.

What exists today (library crates with tests, not yet wired to the screens):
- preference store, databases (manga and anime SQLite schemas), repositories and domain rules
- source API and an HTTP source layer, extension manager, local source
- image pipeline and cover handling, archive (CBZ) reading
- download queue, mpv-based player core
- 67 mockup screens, sheets and dialogs that match Aniyomi's layout

What does not work yet: reading and playing from a real source, tracking, backups, downloads in the app.

### Progress (audited 2026-10-11)

These states come from reading the code, not from running the app. The old percentages were estimates and are gone.
The full list of what is left, in order of user impact, is in `docs/audit/LEFTOVERS.md`.

| Feature | What it means for you | State |
|---|---|---|
| Settings | Your choices are saved; eight pages | works; cut keys still registered; the Player page still shows Custom buttons and Script editor |
| Databases and library | Library, chapters, episodes and history are stored on disk | works in code; an empty database at start is not tested |
| Manga reader | Open a chapter, page through it, slider, per-manga direction | works in code; not checked in a window |
| Video player | Play, audio, subtitle and quality menus, mpv config files | works in code; not run |
| Manga downloads | Save chapters for offline reading | partial: queue page and notifications exist, the details page buttons do nothing |
| Anime downloads | Save episodes for offline watching | partial; not run |
| Backup and restore | Export and import, including Aniyomi backups | works in code; not run |
| Palettes | Theme choice | partial: the UI offers System, Light and Dark, but the palette code is still in the build |
| Data saver and detach | Removed | removed on 2026-10-11 |
| Sources, browse, network, covers, local files, trackers | Not audited in this pass | see the older audit files in `docs/audit/` |

## Sources

Like Aniyomi, the app ships with no online sources. You add a source repository by link and install sources from it.
The local source (files on your computer) is built in.

## Build and run

Needs a recent stable Rust, GTK 4.14 or newer, and libmpv.

```bash
cargo run                      # the mockup window
cargo run -- --version         # prints the version and exits
cargo test --workspace         # all crate tests
```

Developer switches for the mockup: `KUROBAYOMI_OPEN=<screen id>`, `KUROBAYOMI_SIZE=WxH`, `KUROBAYOMI_SMOKE=1` (opens every screen once and exits).

## Layout

| Path | What it is |
|---|---|
| `src/` | the GTK4 window (mockup) |
| `crates/kuro-core` | shared helpers, logging, crash reports, image and storage utilities |
| `crates/kuro-platform` | all OS-specific code (paths, secrets), so a Windows port stays possible |
| `crates/kuro-prefs`, `kuro-db`, `kuro-data`, `kuro-domain` | preferences, databases, repositories, domain logic |
| `crates/kuro-source*`, `kuro-extensions`, `kuro-net` | sources, extension manager, HTTP |
| `crates/kuro-local`, `kuro-archive`, `kuro-images`, `kuro-downloads`, `kuro-player` | local files, CBZ, images, downloads, mpv player |
| `plans/` | the full port plan: per-area docs, catalogs, roadmap and the task list |
| `spec/` | screen definitions extracted from Aniyomi's UI |

Start with `plans/00-INDEX.md` for the plan and `plans/ROADMAP.md` for the order of work.

## Licence

Apache License 2.0, see `LICENSE`. Kurobayomi is derived from Aniyomi, Mihon and Tachiyomi, which use the same licence; see `NOTICE`.
