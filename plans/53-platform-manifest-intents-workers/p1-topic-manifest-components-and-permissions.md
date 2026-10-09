# Topic: manifest components, permissions and resources — what carries over, what is dropped, and what the user loses

Scope: `app/src/main/AndroidManifest.xml` (324 LOC), `app/shortcuts.xml`, the ten empty module manifests, `presentation-widget/src/main/AndroidManifest.xml`, and the `res` directories declared in scope (layouts, menus, animations, raw, xml). This topic is the consolidated decision record for the manifest: every declaration, its replacement or drop, and the user-visible consequence. Component details are in `p1-behavior.md` section 1 and the tables in `p1-data-and-config.md` sections 1 and 2.

## 1. Decision ledger (one line per declaration)

| Declaration | Decision | Replacement | User loss | Task |
|---|---|---|---|---|
| MainActivity launcher | Port | GTK application window | none | T-53a-001 |
| MainActivity link filters (`tachiyomi`, `aniyomi`) | Port | `parse_uri` and `.desktop` MimeType | none (aliases kept) | T-53a-004 |
| MainActivity `.tachibk` filter | Port | MIME association and `connect_open` | none | T-53a-003, T-53a-010 |
| MainActivity shortcuts meta-data | Port | desktop actions | none; history action is no longer duplicated | T-53a-011 |
| CrashActivity | Owned by area:01 | crash handling | per area:01 | T-53a-069 |
| DeepLinkAnimeActivity, DeepLinkMangaActivity (trampolines) | Drop | direct route | none | T-53a-046 |
| Search filters (`SEARCH`, `SEARCH_ACTION`, `ANIMESEARCH`, `SEND`) | Port | `--search`, text route, `ExtSearch` | Google Assistant voice search is not available | T-53a-005 |
| `searchable.xml` | Rewrite (P2) | GNOME SearchProvider2, KRunner | system-wide search results appear only if the desktop provides them | T-53a-066 |
| ReaderActivity (singleTask, S Pen) | Port reader; S Pen drop | reader window; keyboard keys | Samsung S Pen gestures | T-53a-015, T-53a-065 |
| PlayerActivity (singleTask, PiP, resizable) | Port player; PiP drop | player window; compact mode optional | picture-in-picture floating window | T-53a-015, T-53a-033 |
| UnlockActivity | Owned by area:36 | app lock | biometric unlock becomes passphrase or PAM/polkit | T-53a-070 |
| WebViewActivity | Owned by area:36 | `WebViewHost` | none | T-53a-071 |
| MangaExtensionInstallActivity, AnimeExtensionInstallActivity | Drop | area:20 extension mechanism | installing APK extensions | T-53a-036 |
| TrackLoginActivity | Port | OAuth return through `parse_uri` | none (see launch topic section 6) | T-53a-009 |
| NotificationReceiver | Rewrite | GAction dispatcher | none for visible actions; dead actions removed | T-53a-017 to T-53a-019 |
| MangaExtensionInstallService, AnimeExtensionInstallService | Drop | area:20 | as above | T-53a-036 |
| AppLocalesMetadataHolderService | Drop | area:51 language setting | per-app locale is not a Linux concept | T-53a-031 |
| SystemForegroundService (WorkManager) | Drop | `BackgroundHold` and job runner | none visible | T-53a-029, T-53a-037 |
| TorrentServerService | Port | background service with notice | none; the notice has a Stop button | T-53a-035 |
| HttpServerService | Port | background service with notice | none | T-53a-035 |
| FileProvider and path whitelist | Drop | `Opener` with paths | share chooser replaced by "Show in folder" and "Copy link" | T-53a-064, T-53a-040 |
| ShizukuProvider | Drop | none | Shizuku-based extension install | T-53a-038 |
| WebView metadata (safe browsing, metrics) | Drop | none | none | T-53a-032 |
| 14 permissions | Replace or drop (table in data section 2) | network monitor, portal, notifications check | battery optimisation prompt, all-files access | T-53a-024 to T-53a-030 |
| application attributes | Drop or replace | desktop entry, reqwest policy | none | T-53a-032 |
| network_security_config | Rewrite | reqwest TLS policy | none | T-53a-063 |
| provider_paths | Drop | none | none | T-53a-064 |
| s_pen_actions | Drop | keyboard map | Samsung S Pen | T-53a-065 |
| shortcuts.xml | Rewrite | desktop actions | none | T-53a-011, T-53a-012 |
| 10 module manifests | Drop | Cargo crates | none | T-53a-067 |
| presentation-widget receivers (six) | Drop | Updates tab, notices | home-screen and lock-screen cards of recent updates | T-53a-068 |

## 2. Resources (layouts, menus, animations, raw, xml) — what is used and what is dead

| Group | Used and ported | Dead and dropped |
|---|---|---|
| `layout` (7 files) | `download_header`, `download_item`, `download_list` (download screens); `player_layout`; `reader_activity`; `reader_error` | `pref_widget_switch_material` |
| `menu` (3 files) | `download_single` (row overflow menu, six items) | `chapter_download`, `main_nav` |
| `anim` (14 files) | `shared_axis_x_*` (four files, reader and web view, pre-API-33 copies) | `player_*` (ten files) |
| `anim-v33` (4 files) | `shared_axis_x_*` (API 33 and above copies; the same timings with absolute distances) | none |
| `raw` (1 file) | none | `keep.xml` |
| `xml` (4 files) | `network_security_config` (behaviour), `searchable` (P2) | `provider_paths`, `s_pen_actions` |

Dead resources were verified with two searches: a grep for `R.<type>.<name>`, `@<type>/<name>` and ViewBinding class names over `app/src/main/java`, and a check that no manifest references them. The ViewBinding class for each used layout is in `p1-data-and-config.md` section 10.1.

## 3. What the user loses (consolidated)

Ordered by how much a daily user would notice:

1. **Home-screen and lock-screen widgets** of recent manga and anime updates (six receivers). Replacement: the Updates tab and desktop notices.
2. **Installing APK extensions** from the app (installers and Shizuku). Replacement: the extension mechanism of area:20.
3. **Picture-in-picture floating player.** Replacement: the player window; a compact mode is optional (P3).
4. **Share chooser** for images and backups. Replacement: "Show in folder" and "Copy link".
5. **Biometric app lock.** Replacement: passphrase or PAM/polkit prompt (area:36 decides).
6. **Samsung S Pen gestures.** Replacement: keyboard keys N, P, arrows and Escape (`p1-ui-parity.md` section 6).
7. **Battery-optimisation and all-files access prompts.** Not needed on the desktop.
8. **Google Assistant voice search.** Replacement: the desktop's own search (P2 provider) and `--search`.

Nothing that changes saved data is lost. The Android backup flag (`allowBackup=false`) means Android never stored app data in cloud backup, so no migration of OS-level state is needed. Kurobayomi reads Aniyomi backups (`.tachibk`) through the restore flow of area:22 only.

## 4. Permissions model on Linux (summary)

- No runtime permission prompts exist. Access to the network, to notifications and to folders is granted by the desktop or by the sandbox portal when used (folder chooser, notifications, OpenURI).
- The packaging audit (T-53a-013) limits sandbox permissions to network, the notification and file-chooser portals, and no `filesystem=host`.
- Network state (`ACCESS_NETWORK_STATE`, `ACCESS_WIFI_STATE`) is read from GIO and NetworkManager (T-53a-024 to T-53a-026). Unknown state blocks Wi-Fi-only downloads (defect E-15).

## 5. Test coverage for this topic

- Route and argv tests (TC-53-01 to TC-53-03, TC-53-14, TC-53-15) cover the intent-filter replacements.
- Desktop file and MIME golden tests (TC-53-36, TC-53-37) cover the manifest-to-desktop mapping.
- Dead-code guard (TC-53-24) and string-key coverage (TC-53-23) cover the drops and the strings.
- The packaging permission audit (T-53a-013) is a review step; its result is recorded in the task's acceptance check.
