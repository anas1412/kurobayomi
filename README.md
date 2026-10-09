# Kurobayomi

A manga reader and anime player for the Linux desktop, written in Rust with GTK4 (no libadwaita, so your own GTK theme applies).
It is a rewrite of [Aniyomi](https://github.com/aniyomiorg/aniyomi): same library, reader, player, sources and tracking model, built as a native app instead of an Android one.

The name is *kurobako* (黒箱, "black box") plus *-yomi* ("reading"), like Tachiyomi and Aniyomi.

![Kurobayomi mockup](docs/screenshot.png)

## Status: early work in progress

The window you see is a **mockup with placeholder data**. The real features are being built underneath it, crate by crate.
The plan has about 3,800 tasks; roughly 11% are reported done. Treat everything as unfinished.

What exists today (library crates with tests, not yet wired to the screens):
- preference store, databases (manga and anime SQLite schemas), repositories and domain rules
- source API and an HTTP source layer, extension manager, local source
- image pipeline and cover handling, archive (CBZ) reading
- download queue, mpv-based player core
- 67 mockup screens, sheets and dialogs that match Aniyomi's layout

What does not work yet: reading and playing from a real source, tracking, backups, downloads in the app.

## Sources

Like Aniyomi, the app ships with no online sources. You add a source repository by link and install sources from it.
The local source (files on your computer) is built in.

## Build and run

Needs a recent stable Rust, GTK 4.16 or newer, and libmpv.

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
