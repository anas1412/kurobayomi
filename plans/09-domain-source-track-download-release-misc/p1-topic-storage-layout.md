# Topic: storage layout and `StorageManager`

Scope files: `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt`, `domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt`. Read: `core/common/src/main/java/tachiyomi/core/common/storage/AndroidStorageFolderProvider.kt`, `app/src/main/java/eu/kanade/presentation/more/onboarding/StorageStep.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (storage location row), `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt:101,122,354-358`. Stats and storage screens (`storage` mockup screen) belong to area:42; the folder-picker plumbing belongs to area:13-core-util-storage-archive-torrent.

## 1. Directory tree (exact names)

| Accessor | Relative path | Created by | Used by |
|---|---|---|---|
| base | `storage_dir` | user / first run | everything |
| `getAutomaticBackupsDirectory` | `autobackup` | accessor + on change | automatic backup writer (area:21) |
| `getDownloadsDirectory` | `downloads` | accessor + on change | download provider/cache (areas 23-25) |
| `getLocalMangaSourceDirectory` | `local` | accessor + on change | local manga source (area:14) |
| `getLocalAnimeSourceDirectory` | `localanime` | accessor + on change | local anime source (area:14) |
| `getMPVConfigDirectory` | `mpv-config` | accessor + on change | MPV file editor, player start (area:45) |
| `getFontsDirectory` | `mpv-config/fonts` | accessor + on change | subtitle font picker, `sub-fonts-dir` |
| `getScriptsDirectory` | `mpv-config/scripts` | accessor + on change | copied into the session config dir when user files are enabled |
| `getScriptOptsDirectory` | `mpv-config/script-opts` | accessor + on change | same |
| `getShadersDirectory` | `mpv-config/shaders` | accessor + on change | same |

Within `downloads/`: `<source display name>/<entry folder>/<item folder or file>` (area:23/24).

## 2. Defaults on Linux

| Quantity | Android | Kurobayomi default |
|---|---|---|
| base | `<external storage>/<app name>` | `$XDG_DATA_HOME/kurobayomi` (`~/.local/share/kurobayomi`) |
| Not stored here (area:01 `Paths`) | databases, caches, logs | `$XDG_DATA_HOME/kurobayomi/db`, `$XDG_CACHE_HOME/kurobayomi`, `$XDG_STATE_HOME/kurobayomi/logs`, `$XDG_CONFIG_HOME/kurobayomi/{settings.toml,state.toml}` |

Why `~/.local/share` and not `~/Kurobayomi`: large downloaded media in the XDG data dir keeps the home folder clean; the Settings > Data and storage row "Storage location" lets the user move the whole tree to any folder (for example a second disk). The databases and settings files are NOT inside the user-visible base folder (on Android the databases are in app-private storage and only backups/downloads/local sources are in the shared folder); the layout above preserves that split.

## 3. `StorageManager` semantics (Rust)

```rust
impl StorageManager {
    pub fn ensure_layout(base: &Path) -> io::Result<()> {
        for d in ["autobackup", "downloads", "local", "localanime",
                  "mpv-config", "mpv-config/fonts", "mpv-config/scripts",
                  "mpv-config/script-opts", "mpv-config/shaders"] {
            std::fs::create_dir_all(base.join(d))?;
        }
        for marker in [".nomedia", ".noindex"] {
            let f = base.join("downloads").join(marker);
            if !f.exists() { std::fs::write(f, b"")?; }
        }
        Ok(())
    }
}
```
1. Construction: read `state.storage_dir` (or default); if it does not exist try `create_dir_all`; if that fails return `StorageError::Unavailable { path, source }` (the app shows the dialog of behavior 4.1). Always run `ensure_layout` (upstream only did so on change, section 4 of behavior doc quirks).
2. `set_base(new)`: must be absolute; must exist or be creatable; probe writability by creating and deleting `.kuro-write-probe`; on success store, `ensure_layout`, persist `state.storage_dir` atomically, then `changes.send(())`. Existing files are NOT moved (Aniyomi does not move either); the dialog text says so (`storage_move_notice`: "Existing downloads and backups stay in the old folder. Move them yourself, then reindex downloads."). Consumers rebuild the download cache index on the broadcast event (area:25 listens, like `MangaDownloadCache.kt:122`).
3. Accessors never fail: they return the joined path and `create_dir_all` it lazily (cheap when present). If the directory vanished at runtime they recreate it; if creation fails the caller's own I/O error surfaces (downloads show "Storage unavailable").
4. `changes` is a `tokio::sync::broadcast` with capacity 1 (consumers only need "something changed"); a lagging receiver is fine (`RecvError::Lagged` is treated as a change).
5. Case-insensitive file systems (FAT/exFAT/NTFS mounts): all path comparisons of source and entry folder names use the folder-name sanitizer of area:13 (`DiskUtil.buildValidFilename` port) and are compared with `eq_ignore_ascii_case` when the mount is case-insensitive (area:25 detects it once).

## 4. Drops

| Android item | Decision | Loss |
|---|---|---|
| Storage Access Framework URIs, persisted URI permissions, `UniFile` | Drop; plain paths | none; a Flatpak build needs `--filesystem=` for custom folders (portal-selected folders are exposed under `/run/user/$UID/doc/...`, which works but changes the path on every reselect; document in packaging notes) |
| `.nomedia` file in `downloads/` (`DiskUtil.createNoMediaFile`) | Keep, extended | Android used it to hide downloaded images from the gallery. On Linux, GNOME Tracker's file miner skips directories that contain `.nomedia` (default `ignored-directories-with-content` includes `.nomedia`, `.trackerignore`, `.git`, `.hg`) and KDE Baloo skips directories that contain `.noindex`. `ensure_layout` therefore creates two empty marker files, `downloads/.nomedia` and `downloads/.noindex`, so downloaded pages are not indexed by the desktop search. Nothing is lost |
| `isExternalStorageManager()` gating of mpv.conf editing | Drop | none (files are always writable) |

## 5. Backup interplay

`storage_dir` is an app-state key: never written into backups (area:21 excludes `__APP_STATE_*`). A restored backup therefore keeps the current machine's storage folder.

## 6. Tasks

T-09a-019, T-09a-020, T-09a-021 in `p1-tasks.md`.
