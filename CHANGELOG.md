# Changelog

All notable changes to Kurobayomi. The format follows [Keep a Changelog](https://keepachangelog.com/), and the versions
follow [Semantic Versioning](https://semver.org/). A release is cut from the section of its version (`scripts/release-notes.sh`
reads it), so the section must exist and have notes before its tag is pushed (see `docs/RELEASING.md`).

## [Unreleased]

## [0.1.1] - 2026-10-19

### Added

- MyAnimeList tracking next to AniList: log in from Settings, then track any anime or manga from its Tracking page
  (status, progress, score, start and finish dates). Progress after reading or watching goes to every tracker an entry
  is bound to.

### Fixed

- The library update at launch, the refresh buttons and the automatic update waited for nothing, so an update that
  started before the sources had loaded failed every entry with "Source not installed". They now wait for the sources.
- Library updates gave sources only the entry's address, without its title, so AnimePahe entries whose link had
  expired failed with "AnimePahe no longer lists". The stored title is now passed along.

## [0.1.0] - 2026-10-18

### Added

- Manga and anime in one desktop app (Rust and GTK 4): one library with categories, filters and sorting, an updates feed, a
  history, and a download queue.
- Sources from repositories: recipe sources and WASM plug-ins; browsing, search, filters and a global search.
- A manga reader and an anime player (mpv) that keep your progress.
- Tracking with AniList, Kitsu, MangaUpdates and Simkl.
- Local folders as sources, and restoring Aniyomi and Tachiyomi backups (`.tachibk`).
- An AppImage for Linux that carries its own libraries; it needs glibc 2.39 or newer (Ubuntu 24.04, Fedora 40, Debian 13, Arch).
- The AppImage updates itself: at start it downloads a newer release in the background, checks it, and runs it after a
  restart.
