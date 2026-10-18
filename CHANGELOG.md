# Changelog

All notable changes to Kurobayomi. The format follows [Keep a Changelog](https://keepachangelog.com/), and the versions
follow [Semantic Versioning](https://semver.org/). A release is cut from the section of its version (`scripts/release-notes.sh`
reads it), so the section must exist and have notes before its tag is pushed (see `docs/RELEASING.md`).

## [Unreleased]

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
