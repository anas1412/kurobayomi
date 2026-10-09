# Area 14 — Local source and metadata: edge cases, failure modes, tests

Identifiers: `E-<AREA>-<n>` are referenced from `p1-behavior.md`, `p1-data-and-config.md` and `p1-rust-design.md`. Each entry states the Kotlin behavior (OBSERVED, with path:line), the Rust behavior (PROPOSAL), and the test that pins it.

Test naming: `t_<area>_<topic>_<case>`; tests live in `kuro-source-local/tests/` (integration, temp-dir trees) and `kuro-source-local/src/**` (unit). Fixture files are listed in §4.

---

## 1. Edge cases and failure modes

### 1.1 Browse and listing

**E-BROWSE-1 — missing storage base.** OBSERVED: `getBaseDirectory()` returns null, `getFilesInBaseDirectory()` returns `emptyList` (`LocalMangaSourceFileSystem.kt:14-16`); browse returns an empty page, no error. PROPOSAL: same (`browse` returns `ItemsPage { items: [], has_next_page: false }`). TEST `t_14_browse_missing_root_empty`.

**E-BROWSE-2 — hidden folders and files.** OBSERVED: dot-prefixed directories are excluded from browse (`MANGA_SRC:92`); dot-prefixed files are never directories, so they are excluded too. A file named `Manga` (no dir) is excluded. TEST `t_14_browse_hidden_and_files_excluded`.

**E-BROWSE-3 — case-insensitive contains.** OBSERVED: `name.contains(query, ignoreCase = true)` (`MANGA_SRC:98`). Kotlin's `ignoreCase` compares characters with uppercase and lowercase variants. PROPOSAL: `to_lowercase()` on both sides (differs only for characters whose lowercase has a different length or special folding, e.g. `İ`). TEST `t_14_browse_query_case_insensitive` with ASCII and Latin-1 cases; the exotic case is excluded from the fixture set and documented.

**E-BROWSE-4 — `CASE_INSENSITIVE_ORDER` ordering.** OBSERVED: browse title sort uses `String.CASE_INSENSITIVE_ORDER` (not natural order), so `"10" < "9"` (`MANGA_SRC:108`). PROPOSAL: per-char lowercase comparison, not natural. TEST `t_14_sort_title_is_not_natural`: folders `9`, `10`, `a` sort as `10`, `9`, `a` in ascending order (`'1' < '9'`, and digits before letters).

**E-BROWSE-5 — duplicate names.** A directory and a file cannot share a name, so `distinctBy { name }` only matters on case-insensitive filesystems (Windows) where two names differ only in case are the same entry; Linux never collides. TEST `t_14_browse_distinct_names`.

**E-BROWSE-6 — latest ignores query.** OBSERVED: when `filters === LatestFilters` the query branch is skipped (`MANGA_SRC:84-101`). A search string passed through the latest path is ignored. TEST `t_14_latest_ignores_query`.

**E-BROWSE-7 — latest window boundary.** OBSERVED: `lastModified() >= now - 604800000` (inclusive). An entry exactly 7 days old is included. TEST `t_14_latest_window_inclusive` using a fixed clock and `filetime` to set mtimes exactly at the boundary (`now - 604_800_000` included, `now - 604_800_001` excluded).

**E-BROWSE-8 — `page` unused.** OBSERVED: the page argument is ignored; `hasNextPage` is always false (`MANGA_SRC:142`). Calling with page 2 returns the same set as page 1. TEST `t_14_browse_page_ignored`.

**E-BROWSE-9 — cover found in browse.** OBSERVED: `coverManager.find(name)` runs for every folder during browse (`MANGA_SRC:134`). A folder whose cover file is not an image (e.g. `cover.txt` with text) has no thumbnail. TEST `t_14_browse_cover_requires_image`.

**E-BROWSE-10 — directory listing errors.** OBSERVED: `listFiles()` returns null on IO failure, mapped to empty (`orEmpty`). A permission-denied base folder yields an empty browse, silently. PROPOSAL: return `Err(Io)` for permission errors so the UI can show a message; empty for missing. This is a deliberate deviation. TEST `t_14_browse_permission_denied_error` (unix, skipped on root).

### 1.2 Manga details and ComicInfo

**E-MANGA-1 — manga folder vanished.** OBSERVED: `getMangaDetails` throws `IllegalStateException("<url> is not a valid directory")` inside the guarded block and logs it (`MANGA_SRC:153`); the manga is returned unchanged (cover step ran before). PROPOSAL: same result; `details` returns `Ok(())`. TEST `t_14_details_missing_folder_logs_and_returns`.

**E-MANGA-2 — ComicInfo.xml malformed.** OBSERVED: exception logged, manga keeps its prior fields, file stays; it is re-parsed and fails again on every details fetch, because branch (a) is taken each time. PROPOSAL: same, plus a `warn!` naming the file. TEST `t_14_comicinfo_malformed_keeps_file_and_fields`.

**E-MANGA-3 — EPUB chapter breaks chapter listing.** OBSERVED: `epub.fillMetadata` is not guarded in `getChapterList` (`MANGA_SRC:273-278`); a corrupt EPUB (missing container, missing OPF, not a zip) propagates and the **whole** chapter list fails. PROPOSAL (deviation): the EPUB chapter keeps its filename-derived name and `date_upload = modified_ms`, the error is logged with `warn!`, and the other chapters are still listed. Reason: one bad file must not hide the rest of the folder. TEST `t_14_chapters_bad_epub_keeps_others`.

**E-MANGA-DETAILS-2 — directory named like an archive.** OBSERVED: `mangaDirFiles.filter(ArchiveManga::isSupported)` does not check `isDirectory` (`MANGA_SRC:195`); a directory `Vol.cbz/` is opened as an archive and throws, which aborts branch (c) and is logged. PROPOSAL: skip directories (`!is_dir`). TEST `t_14_details_dir_named_cbz_skipped`.

**E-MANGA-4 — no archives and no ComicInfo.** OBSERVED: `.noxml` is created (empty file) so the next call skips the scan (`MANGA_SRC:202`). TEST `t_14_details_noxml_marker_created_once`.

**E-MANGA-5 — `.noxml` plus later ComicInfo.** OBSERVED: a top-level `ComicInfo.xml` added after `.noxml` is applied and `.noxml` is deleted on the next details fetch (`MANGA_SRC:166`). TEST `t_14_details_comicinfo_removes_noxml`.

**E-MANGA-6 — `details.json` write fails.** OBSERVED: the JSON is deleted only inside `use` after the XML write; a failed write leaves the JSON and the manga fields already assigned (`MANGA_SRC:181-190`). PROPOSAL: keep (write first, delete after). TEST `t_14_details_legacy_json_kept_on_write_failure` (make the folder read-only).

**E-MANGA-7 — archive ComicInfo copy order.** OBSERVED: first archive in `listFiles` order containing root `ComicInfo.xml` wins (`MANGA_SRC:216-225`). PROPOSAL: natural-CI name order (deterministic). TEST `t_14_archive_comicinfo_first_by_name`.

**E-MANGA-8 — ComicInfo in subdirectory of archive.** OBSERVED: only the exact entry `ComicInfo.xml` at archive root matches (`getInputStream(entryName)` compares full entry name). `Vol1/ComicInfo.xml` is ignored. TEST `t_14_archive_comicinfo_root_only`.

**E-CI-1 — empty Series.** OBSERVED: `<Series/>` yields `""`; `title = ""` is assigned (`COMIC:34`). PROPOSAL: keep (assign empty). TEST `t_14_comicinfo_empty_series_assigns_empty`.

**E-CI-2 — status always overwritten.** OBSERVED: `status = toSMangaValue(null)` = 0 when `PublishingStatusTachiyomi` is absent (`COMIC:61`). PROPOSAL: keep. TEST `t_14_comicinfo_absent_status_resets_to_unknown`.

**E-CI-3 — genre joined without split, artist with split.** OBSERVED: genre `"Action, Drama"` + tags `"Drama"` -> `"Action, Drama, Drama"` (`COMIC:38-46`); artist `"A, B"` + penciller `"A"` -> `"A, B"` (split then distinct, `COMIC:48-59`). TEST `t_14_comicinfo_genre_merge_no_split` and `t_14_comicinfo_artist_merge_split`.

**E-CI-4 — duplicate element.** OBSERVED: xmlutil with `ignoreUnknownChildren` on duplicate known elements: behavior not established by this source (it may keep the first or throw). PROPOSAL: last occurrence wins. TEST `t_14_comicinfo_duplicate_last_wins`. Verify once against the upstream library if the choice matters.

**E-CI-5 — unknown elements.** OBSERVED: `ignoreUnknownChildren()` (`AppModule.kt:181`). PROPOSAL: skip. TEST `t_14_comicinfo_unknown_children_ignored`.

**E-CI-6 — namespace binding.** OBSERVED: `PublishingStatusTachiyomi` is bound to the XML Schema namespace with prefix `ty`; xmlutil matches the element by namespace URI plus local name. PROPOSAL: match by local name only (a file with an unbound `ty:` prefix or no prefix is then honored). Difference: a file with an unbound `<PublishingStatusTachiyomi>` is `Unknown` in Kotlin and `Ongoing` in Rust. TEST `t_14_comicinfo_status_local_name_match` documents the deviation.

**E-CI-7 — root name mismatch.** OBSERVED: decode of a document whose root is not `ComicInfo` fails (xmlutil `XmlSerialName` on the root). PROPOSAL: same, `Root` error. TEST `t_14_comicinfo_wrong_root_error`.

**E-CI-8 — empty file.** OBSERVED: decode of empty input throws (EOF). PROPOSAL: `Xml` error. TEST `t_14_comicinfo_empty_file_error`.

**E-CI-9 — XML entities in text.** OBSERVED: xmlutil unescapes standard entities and numeric character references. PROPOSAL: `quick_xml::Reader` `unescape()` with the same five named entities plus numeric references. TEST `t_14_comicinfo_entities_roundtrip` (`&amp;`, `&lt;`, `&#x3B1;`).

**E-CI-10 — writing special characters.** OBSERVED: text escaping is performed by xmlutil; the exact set of escaped characters is not established by this source (see data §5.2). PROPOSAL: escape `& < > " '` (quick-xml default). Golden test asserts the upstream output once generated. TEST `t_14_comicinfo_write_escapes` and the golden test `t_14_comicinfo_golden_upstream`.

**E-CI-11 — very large number strings.** `Number` values are text and never parsed in the local source. TEST none needed.

**E-CI-12 — BOM and encodings.** OBSERVED: the file is read with `AndroidXmlReader(stream, "UTF-8")` (`MANGA_SRC:236`). A UTF-8 BOM is consumed by the reader. PROPOSAL: strip a leading UTF-8 BOM before parsing. A UTF-16 file is not supported by Kotlin either (decoding as UTF-8 fails). TEST `t_14_comicinfo_utf8_bom_accepted`, `t_14_comicinfo_utf16_rejected`.

### 1.3 Chapter and episode listing

**E-CHAPTERS-1 — chapters.json failure is silent.** OBSERVED: whole decode in `runCatching` (`MANGA_SRC:249-251`); a single element missing `chapter_number` disables the file. PROPOSAL: same (silent `None`), with a `warn!`. TEST `t_14_chapters_json_missing_number_disables_file`.

**E-CHAPTERS-2 — duplicate chapter numbers in chapters.json.** OBSERVED: `firstOrNull` (first in file order wins) (`MANGA_SRC:282`). TEST `t_14_chapters_json_first_match_wins`.

**E-CHAPTERS-3 — chapter number tolerance.** OBSERVED: `abs(a - b) < 0.0001` (`MANGA_SRC:310`). Values 4.0 and 4.00005 match; 4.0 and 4.001 do not. TEST `t_14_chapters_json_tolerance`.

**E-CHAPTERS-4 — unconditional scanlator clear.** OBSERVED: `scanlator = data.scanlator` even when null (`MANGA_SRC:285`), so a chapters.json entry without scanlator clears an EPUB-provided scanlator. TEST `t_14_chapters_json_clears_scanlator`.

**E-CHAPTERS-5 — chapter name source.** OBSERVED: name = directory name for directories, file stem for files; for EPUB, the `dc:title` replaces the name after the number is computed (`MANGA_SRC:263-278`). TEST `t_14_chapters_epub_title_replaces_name_not_number`.

**E-CHAPTERS-6 — ordering with equal numbers.** OBSERVED: equal numbers sort by name descending, naturally (`MANGA_SRC:290-293`). Example: `Ch 1a`, `Ch 1b`, `Ch 1` -> number 1.1, 1.2, 1.0 -> order `1.2`, `1.1`, `1.0`. TEST `t_14_chapters_sort_number_desc_then_name_desc`.

**E-CHAPTERS-7 — unrecognised numbers.** OBSERVED: `-1` for all unrecognised entries (`ChapterRecognition.kt:53`). They sort at the bottom, by name descending. TEST `t_14_chapters_unrecognised_bottom`.

**E-CHAPTERS-8 — dot files and loose images.** OBSERVED: `.cover.jpg` ignored (dot), `page1.jpg` in the manga folder ignored (not dir/archive/epub). TEST `t_14_chapters_ignore_dot_and_loose_images`.

**E-CHAPTERS-9 — uppercase extensions.** OBSERVED: `Vol 1.CBZ` and `Vol 1.EPUB` accepted (case-insensitive); `Ep 1.MKV` rejected for anime (case-sensitive). TEST `t_14_chapters_uppercase_archive_accepted`, `t_14_episodes_uppercase_rejected`.

**E-CHAPTERS-10 — corrupt archive chapter.** OBSERVED: archives are not opened during chapter listing (only for the cover fallback of the lowest chapter, §1.4). A corrupt archive is listed; it fails on read. PROPOSAL: same. TEST `t_14_chapters_corrupt_archive_listed`.

**E-CHAPTERS-11 — cover fallback from corrupt archive.** OBSERVED: `updateCover` catches everything and returns null (`MANGA_SRC:371-374`). TEST `t_14_cover_fallback_corrupt_archive_swallowed`.

**E-CHAPTERS-12 — cover fallback only when blank.** OBSERVED: runs only when `manga.thumbnail_url.isNullOrBlank()` (`MANGA_SRC:296`). A non-blank URL is never replaced by this path. TEST `t_14_cover_fallback_only_when_blank`.

**E-DATE-1 — lenient date parsing.** OBSERVED: `SimpleDateFormat` is lenient, rolls `2024-02-31` to March 2 and ignores trailing text (`MANGA_SRC:305-307`). PROPOSAL: strict parsing; invalid date = "no override" (no abort). Deviation: documented. TEST `t_14_dates_strict_invalid_no_override`; and `t_14_dates_trailing_text_rejected` (documents the difference).

**E-DATE-2 — invalid date aborts the whole chapter list.** OBSERVED: `parseDate` throws `ParseException` (no catch) inside the mapping (`MANGA_SRC:284,305-307`), failing `getChapterList`. PROPOSAL (deviation): treat as absent. TEST `t_14_chapters_json_bad_date_keeps_file_date`.

**E-DATE-3 — EPUB date with bare Z.** OBSERVED: pattern `ssZ` with `SimpleDateFormat`; whether a literal `Z` parses is not established by this source. Failure is swallowed (`EpubReaderExtensions.kt:44-46`). PROPOSAL: accept `Z` as UTC and `+hhmm`/`-hhmm`. TEST `t_14_epub_date_formats` for `2024-01-02T03:04:05+0100`, `-0800`, `Z`.

**E-DATE-4 — date zone.** OBSERVED: device default time zone (`SimpleDateFormat` default). PROPOSAL: captured local offset at startup (design §19). TEST `t_14_dates_use_local_offset` with an injected offset.

**E-DATE-5 — mtime zero.** A file with `modified_ms == 0` (unknown) keeps 0 as `date_upload` (Kotlin `lastModified()` returns 0 when unknown). TEST `t_14_chapters_zero_mtime_kept`.

**E-ANIME-1 — episode number from title.** OBSERVED: `EpisodeRecognition.parseEpisodeNumber(anime.title, name, -1.0)` (`ANIME_SRC:255`); title is the anime folder name (or ComicInfo-free anime `details.json` title, §11.4). TEST `t_14_episodes_number_uses_anime_title`.

**E-ANIME-2 — details.json failure propagates.** OBSERVED: `getOldAnimeDetails` has no try (`ANIME_SRC:199`); the error reaches `getAnimeSeasonUpdate`/`getAnimeEpisodeUpdate` (`supervisorScope`, async await rethrows). The caller's season or episode update fails. PROPOSAL: same. TEST `t_14_anime_details_bad_json_fails_update`.

**E-ANIME-3 — episodes.json failure propagates.** OBSERVED: no runCatching (`ANIME_SRC:242`). PROPOSAL: same. TEST `t_14_episodes_json_bad_fails_listing`. (Contrast E-CHAPTERS-1.)

**E-ANIME-4 — missing episode_number.** OBSERVED: `EpisodeDetails.episode_number` has no default (`EpisodeDetails.kt:7`); a missing key is a `MissingFieldException` -> E-ANIME-3. TEST `t_14_episodes_json_missing_number_fails`.

**E-ANIME-5 — preview_url and fillermark ignored.** OBSERVED: not copied (`ANIME_SRC:262-270` copies name, date, scanlator, summary only). TEST `t_14_episodes_json_preview_and_filler_ignored`.

**E-ANIME-6 — thumbnail regeneration on every listing.** OBSERVED: for each episode the thumbnail step runs whenever `preview_url == null`, which is always true for a freshly created `SEpisode` (`ANIME_SRC:272-281`). Every listing re-runs ffprobe+ffmpeg for every episode and overwrites `<name>-thumbnail.jpg`. PROPOSAL (deviation): skip extraction when the thumbnail file already exists and is non-empty, with an explicit force flag. TEST `t_14_thumbnail_skip_existing` (fake tool counts invocations).

**E-ANIME-7 — temp file leak.** OBSERVED: `File.createTempFile("tmp_", suffix)` is never deleted (`ANIME_SRC:338`). PROPOSAL: delete after the image is written or on failure. TEST `t_14_thumbnail_temp_removed`.

**E-ANIME-8 — hidden directory makes Seasons.** OBSERVED: `files.any { it.isDirectory }` counts hidden directories (`LocalAnimeFetchTypeManager.kt:17`). PROPOSAL: keep (documented quirk, parity). Alternative would change the fetch type of existing libraries. TEST `t_14_fetch_type_hidden_dir_counts`.

**E-ANIME-9 — directory named like an episode.** OBSERVED: the episode filter is `!startsWith('.') && ArchiveAnime.isSupported(it)` (`ANIME_SRC:246-248`) and does not check `isDirectory`, so a directory `Dir.mkv/` is listed as an episode and fails at playback. PROPOSAL (deviation, obvious bug): exclude directories from episodes. TEST `t_14_episodes_dir_named_mkv_excluded`.

**E-ANIME-10 — season directory vs episodes in same folder.** OBSERVED: a folder with both an episode file and a sub-folder is `Episodes`; sub-folders are ignored in the episode list. TEST `t_14_fetch_type_mixed_is_episodes`.

**E-ANIME-11 — season nested path.** OBSERVED: season url `"show/season1"`; episode url `"show/season1/ep.mkv"`; resolution through nested `findFile` (raw backend). PROPOSAL: `safe_join`. TEST `t_14_seasons_nested_urls_resolve`.

**E-ANIME-12 — season with zero episodes.** A season directory with no video files is `Episodes` with an empty list (no error). TEST `t_14_seasons_empty_season_episode_list_empty`.

**E-ANIME-13 — anime covers from first episode.** OBSERVED: cover fallback picks `episodes.lastOrNull()` (lowest number) (`ANIME_SRC:292`). TEST `t_14_anime_cover_from_lowest_episode`.

**E-ANIME-14 — fallback condition uses object state.** OBSERVED: `anime.thumbnail_url == null || find == null` (`ANIME_SRC:290`); when the caller's object has a URL but the file is missing, the fallback runs. TEST `t_14_anime_cover_fallback_when_file_missing`.

### 1.4 Playback and reading

**E-PLAY-1 — missing episode file.** OBSERVED: `videoFile!!` NPE caught -> `emptyList()` (`EpisodeLoader.kt:124-146`); the UI shows no hosters, no error text. PROPOSAL: same empty list plus a `warn!`. TEST `t_14_playback_missing_file_empty`.

**E-PLAY-2 — episode URL without slash.** OBSERVED: destructuring throws, caught, empty list. PROPOSAL: empty list. TEST `t_14_playback_bad_url_empty`.

**E-PLAY-3 — video title.** OBSERVED: `"Local source: ${episode.url}"`. TEST `t_14_playback_title_format`.

**E-PLAY-4 — external player file path.** OBSERVED: local anime hands a `FileProvider` URI to external apps (`ExternalIntents.kt:146-160`). PROPOSAL: plain path. TEST `t_14_playback_external_uses_path` (unit on the URL builder).

**E-READ-1 — chapter url without slash.** OBSERVED: `getFormat` rethrows raw destructuring error (`MANGA_SRC:323`). PROPOSAL: `ChapterNotFound`. TEST `t_14_format_bad_url_chapter_not_found`.

**E-READ-2 — chapter folder missing.** OBSERVED: `Exception(chapter_not_found)` (`MANGA_SRC:328`). TEST `t_14_format_missing_chapter`.

**E-READ-3 — chapter entry of unknown type.** OBSERVED: `local_invalid_format` (`MANGA_SRC:329-330`). Example: a `.rar` renamed `.txt` cannot be a chapter (not listed), but a hard-linked `.txt` passed by URL is "Invalid chapter format". TEST `t_14_format_unknown_type_invalid`.

**E-READ-4 — EPUB open failure.** OBSERVED: exception propagates to the reader (not caught in `getFormat`). PROPOSAL: `Archive`/`Epub` error to the reader's error state. TEST `t_14_format_epub_open_error_propagates`.

### 1.5 Covers, backgrounds, thumbnails

**E-IMG-1 — overwrite keeps name.** OBSERVED: `find(...) ?: createFile("cover.jpg")` (`MANGA_SRC` `LocalMangaCoverManager.kt:37`): an existing `cover.png` receives JPEG bytes and keeps the `.png` name. PROPOSAL: same. TEST `t_14_images_overwrite_keeps_name`.

**E-IMG-2 — no validation.** OBSERVED: bytes are copied without decoding. PROPOSAL: same (the UI decodes lazily). TEST `t_14_images_no_validation`.

**E-IMG-3 — extension test case sensitivity.** OBSERVED: `ImageType.extension == extension` is case-sensitive; `COVER.JPG` is accepted only via sniffing. TEST `t_14_images_uppercase_ext_via_sniff`.

**E-IMG-4 — `jpeg` spelling.** OBSERVED: extension test fails for `.jpeg` and sniffing accepts it. TEST `t_14_images_jpeg_extension_sniffed`.

**E-IMG-5 — first match in directory order.** OBSERVED: undefined (filesystem order). PROPOSAL: name-sorted. TEST `t_14_images_find_first_sorted`.

**E-IMG-6 — `.nomedia` after writes.** OBSERVED: created only when absent; Android also scans. PROPOSAL: create only when absent. TEST `t_14_images_nomedia_once`.

**E-IMG-7 — thumbnail find quirk.** OBSERVED: `find` compares the stem to the full file name with `.jpg`, so the update path always creates (`LocalEpisodeThumbnailManager.kt:23,35-36`). PROPOSAL: exact name lookup, same observable outcome. TEST `t_14_thumbnail_write_overwrites_same_name`.

**E-IMG-8 — episode name with forbidden characters.** OBSERVED: Linux accepts `:`; Windows does not. PROPOSAL: sanitize per NTFS rules, Linux output identical when no forbidden character appears. TEST `t_14_thumbnail_name_sanitized`.

**E-IMG-9 — background stem.** `background` stem matching is case-insensitive (`LocalAnimeBackgroundManager.kt:22`). TEST `t_14_images_background_case_insensitive`.

**E-IMG-10 — cover of wrong type in archive.** OBSERVED: fallback chooses first `isFile && isImage` in natural-CI order; a `ComicInfo.xml` or `.txt` is skipped. TEST `t_14_cover_archive_first_image_natural`.

### 1.6 Paths and security

**E-PATH-1 — traversal in URL.** OBSERVED: no validation; `findFile("../x")` could address outside the root. PROPOSAL: `safe_join` rejects `..`, `.`, empty, absolute, NUL, backslash. TEST `t_14_path_rejects_traversal`. (Kotlin: exposed only to data already stored by this app; a malicious backup could introduce such a URL, hence the rule.)

**E-PATH-2 — symlink escape.** OBSERVED: `isDirectory` follows symlinks. A symlink inside the root pointing outside is listed. PROPOSAL: keep (user-managed folders often use symlinks); document. TEST `t_14_path_symlink_inside_root_listed`.

**E-PATH-3 — backslashes on Linux.** A file name containing `\` is valid on Linux and is rejected in URLs by `safe_join`. The browse listing still shows it. TEST `t_14_path_backslash_name_listed_not_addressable`. Documented limitation.

**E-PATH-4 — names with `/` in folder name.** Impossible on Linux; Windows-created folders cannot contain `/`. Not testable.

**E-PATH-5 — non-UTF-8 names.** OBSERVED: UniFile names are Java Strings (lossy). PROPOSAL: keep `OsString` for paths; display names use `to_string_lossy()`. TEST `t_14_path_non_utf8_listed_lossy`.

**E-PATH-6 — very long names.** Linux limit 255 bytes per component. A chapter file name over 255 bytes cannot exist; no test needed.

### 1.7 Media tools

**E-MEDIA-1 — tools missing.** OBSERVED: FFmpegKit is bundled, always available on Android. PROPOSAL: if `ffprobe`/`ffmpeg` are not on PATH, skip thumbnails and covers from video, warn once per process, and continue listing. TEST `t_14_media_missing_tool_skips` (set PATH to empty).

**E-MEDIA-2 — probe output not a number.** OBSERVED: `toFloat()` throws; caller logs. PROPOSAL: `MediaError::BadDuration`. TEST `t_14_media_bad_duration`.

**E-MEDIA-3 — duration zero or N/A.** `N/A` -> E-MEDIA-2. `0` -> `second = 0` -> frame at 0 s. TEST `t_14_media_zero_duration_frame_zero`.

**E-MEDIA-4 — extraction produced empty file.** OBSERVED: `length() > 0` gate, no error (`ANIME_SRC:359`). PROPOSAL: same, no error, no image. TEST `t_14_media_empty_output_no_image`.

**E-MEDIA-5 — ffmpeg hang.** OBSERVED: no timeout. PROPOSAL: timeouts 20 s / 120 s, kill child. TEST `t_14_media_timeout_kills_child` (fake tool that sleeps).

**E-MEDIA-6 — path with quotes.** OBSERVED: FFmpegKit string requires `"` escaping. PROPOSAL: argument vector, no escaping. TEST `t_14_media_path_with_quotes_and_spaces` (fake tool echoes args).

**E-MEDIA-7 — frame position integer division.** `duration = 5.9` -> `toInt() = 5` -> `second = 2`. TEST `t_14_media_frame_position_truncates`.

**E-MEDIA-8 — concurrent extraction.** OBSERVED: sequential. PROPOSAL: semaphore(1). TEST `t_14_media_serialized`.

### 1.8 JSON and metadata side files

**E-JSON-1 — unknown keys.** OBSERVED: ignored (`ignoreUnknownKeys = true`). TEST `t_14_json_unknown_keys_ignored`.

**E-JSON-2 — null vs absent.** OBSERVED: null and absent both mean "no override" for nullable fields. TEST `t_14_json_null_equals_absent`.

**E-JSON-3 — wrong type.** `"status": "1"` (string) fails decode (no lenient mode). Manga legacy: propagates to branch (b) error (logged); anime: propagates. TEST `t_14_json_wrong_type_fails`.

**E-JSON-4 — genre as string.** `"genre": "Action"` (string instead of array) fails decode. TEST `t_14_json_genre_string_fails`.

**E-JSON-5 — numbers.** `chapter_number` integer `4` decodes to `4.0`. `chapter_number` `"4"` fails. TEST `t_14_json_number_forms`.

**E-JSON-6 — BOM.** A UTF-8 BOM in JSON: Kotlin `Json` rejects it (decodeFromStream expects `{`). PROPOSAL: strip BOM before parsing (deviation; accept more). TEST `t_14_json_bom_accepted`.

**E-JSON-7 — empty file.** `details.json` empty: Kotlin throws (EOF); PROPOSAL: error. TEST `t_14_json_empty_file_error`.

### 1.9 Sync, library and update rules

**E-SYNC-1 — empty local list is valid.** OBSERVED: `!source.isLocal()` gate (`SyncChaptersWithSource.kt:56`). TEST `t_14_sync_local_empty_ok` (in the app/domain test area; listed here because the rule belongs to local sources).

**E-SYNC-2 — local badge toggle.** OBSERVED: badge only when `display_local_badge` (`MangaLibraryScreenModel.kt:382`). TEST `t_14_badge_pref_respected`.

**E-SYNC-3 — downloaded filter always true for local.** OBSERVED (`ChapterFilter.kt:17-30`). TEST `t_14_filter_downloaded_true_for_local`.

**E-SYNC-4 — local count excludes EPUB.** OBSERVED: `MangaDownloadManager.getDownloadCount` counts directories and manga archives only (`:224-227`), not EPUB. PROPOSAL: same (documented; parity), or include epub (deviation). Decision: keep parity; the count is shown in the UI as "downloaded". TEST `t_14_download_count_local_excludes_epub`.

**E-SYNC-5 — size of local folder.** OBSERVED: `getMangaDirectory(url)?.size() ?: 0L`. PROPOSAL: recursive byte sum. TEST `t_14_download_size_local_recursive`.

---

## 2. Race conditions and concurrency

**RC-1 — scan during write.** A cover write while browse lists the folder may yield a partially written file in the browse thumbnail. Kotlin has the same window. PROPOSAL: write to `cover.jpg.tmp` then rename within the folder (same filesystem; atomic), then `.nomedia`. TEST `t_14_images_write_atomic_rename` (checks no `.tmp` left and content replaced).

**RC-2 — `ComicInfo.xml` read while downloader writes.** Not applicable: the local source never writes into chapter folders; the downloader writes into downloads, a different root.

**RC-3 — two details fetches at once.** Both may write `ComicInfo.xml` (branch b) or both create `.noxml`. OBSERVED: no lock. PROPOSAL: `details` is serialised per manga URL with a `tokio::sync::Mutex` map keyed by URL (cheap). TEST `t_14_details_concurrent_same_manga_single_write`.

**RC-4 — legacy JSON deleted while read.** Single-threaded per manga (RC-3 lock) prevents this. TEST covered by RC-3 test.

**RC-5 — folder renamed during listing.** `list_dir` returns `Err(NotFound)` for the root only; a removed child between listing and `metadata` gives `None` and the entry is skipped (`filter_map`). TEST `t_14_browse_entry_removed_mid_scan_skipped` (simulated by deleting after list in a hook).

**RC-6 — episode file removed after listing.** Playback returns empty (E-PLAY-1). TEST covered by E-PLAY-1.

**RC-7 — ffmpeg writing when the app exits.** `kill_on_drop` kills the child; the temp file is removed on drop (`tempfile`). TEST `t_14_media_drop_cleans_temp`.

---

## 3. Upstream tests to port

### 3.1 `ChapterRecognitionTest` (domain module; logic owned by area:07-domain-entries-items, cases needed here)
Source: `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` (34 `@Test` functions). Each is `assertChapter(mangaTitle, name, expected)` = `ChapterRecognition.parseChapterNumber(mangaTitle, name) shouldBe expected`. Port as `kuro_domain::recognition` tests (area:07-domain-entries-items) and call them from `kuro-source-local` tests through the public function (one smoke test here). The complete list (Kotlin test names -> expected):

| Test | Title | Name | Expected |
|---|---|---|---|
| Basic Ch prefix | Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4: Misrepresentation | 4.0 |
| Basic Ch prefix with space after period | Mokushiroku Alice | Mokushiroku Alice Vol. 1 Ch. 4: Misrepresentation | 4.0 |
| Basic Ch prefix with decimal | Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4.1: Misrepresentation; Mokushiroku Alice Vol.1 Ch.4.4: Misrepresentation | 4.1; 4.4 |
| Basic Ch prefix with alpha postfix | Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4.a: Misrepresentation; Mokushiroku Alice Vol.1 Ch.4.b: Misrepresentation; Mokushiroku Alice Vol.1 Ch.4.extra: Misrepresentation | 4.1; 4.2; 4.99 |
| Name containing one number | Bleach | Bleach 567 Down With Snowwhite | 567.0 |
| Name containing one number and decimal | Bleach | Bleach 567.1 Down With Snowwhite; Bleach 567.4 Down With Snowwhite | 567.1; 567.4 |
| Name containing one number and alpha | Bleach | 567.a; 567.b; 567.extra | 567.1; 567.2; 567.99 |
| Chapter containing manga title and number | Solanin | Solanin 028 Vol. 2 | 28.0 |
| Chapter containing manga title and number decimal | Solanin | Solanin 028.1 Vol. 2; 028.4 | 28.1; 28.4 |
| Chapter containing manga title and number alpha | Solanin | 028.a; 028.b; 028.extra | 28.1; 28.2; 28.99 |
| Extreme case | Onepunch-Man | Onepunch-Man Punch Ver002 028 | 28.0 |
| Extreme case with decimal | Onepunch-Man | Onepunch-Man Punch Ver002 028.1; Onepunch-Man Punch Ver002 028.4 | 28.1; 28.4 |
| Extreme case with alpha | Onepunch-Man | Onepunch-Man Punch Ver002 028.a; Onepunch-Man Punch Ver002 028.b; Onepunch-Man Punch Ver002 028.extra | 28.1; 28.2; 28.99 |
| Chapter containing dot v2 | random | Vol.1 Ch.5v.2: Alones | 5.0 |
| Number in manga title | Ayame 14 | Ayame 14 1 - The summer of 14 | 1.0 |
| Space between ch x | Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch. 4: Misrepresentation | 4.0 |
| Chapter title with ch substring | Ayame 14 | Vol.1 Ch.1: March 25 (First Day Cohabiting) | 1.0 |
| Chapter containing multiple zeros | random | Vol.001 Ch.003: Kaguya Doesn't Know Much | 3.0 |
| Chapter with version before number | Onepunch-Man | Onepunch-Man Punch Ver002 086 : Creeping Darkness [3] | 86.0 |
| Version attached to chapter number | Ansatsu Kyoushitsu | Ansatsu Kyoushitsu 011v002: Assembly Time | 11.0 |
| Number after manga title with chapter in chapter title case | Tokyo ESP | Tokyo ESP 027: Part 002: Chapter 001 | 27.0 |
| Number after unwanted tag | One-punch Man | Mag Version 195.5 | 195.5 |
| Unparseable chapter | random | Foo | -1.0 |
| Chapter with time in title | random | Fairy Tail 404: 00:00 | 404.0 |
| Chapter with alpha without dot | random | Asu No Yoichi 19a | 19.1 |
| Chapter title containing extra and vol | Fairy Tail | Fairy Tail 404.extravol002; Fairy Tail 404 extravol002 | 404.99; 404.99 |
| Chapter title containing omake (japanese extra) and vol | Fairy Tail | 404.omakevol002; 404 omakevol002 | 404.98; 404.98 |
| Chapter title containing special and vol | Fairy Tail | 404.specialvol002; 404 specialvol002 | 404.97; 404.97 |
| Chapter title containing commas | One Piece | One Piece 300,a; One Piece Ch,123,extra; One Piece the sunny, goes swimming 024,005 | 300.1; 123.99; 24.005 |
| Chapter title containing hyphens | Solo Leveling | ch 122-a; Solo Leveling Ch.123-extra; Solo Leveling, 024-005; Ch.191-200 Read Online | 122.1; 123.99; 24.005; 191.200 |
| Chapters containing season | D.I.C.E | D.I.C.E[Season 001] Ep. 007 | 7.0 |
| Chapters in format sx - chapter xx | The Gamer | S3 - Chapter 20 | 20.0 |
| Chapters ending with s | One Outs | One Outs 001 | 1.0 |
| Chapters containing ordinals | The Sister of the Woods with a Thousand Young | The 1st Night; The 2nd Night; The 3rd Night; The 4th Night | 1.0; 2.0; 3.0; 4.0 |

Count check: the table lists 34 Kotlin tests (the list above covers each `@Test` in the file; the grep count was 34).

### 3.2 Natural comparator (`net.greypanther.natsort`)
Source (read for this plan): `https://github.com/gpanther/java-nat-sort` — `src/test/java/net/greypanther/natsort/AbstractSimpleNaturalComparatorTest.java` (default cases) and `CaseInsensitiveSimpleNaturalComparatorTest.java` (adds `"a" EQUALS "A"`), plus `InputOutputTest.java` with `src/test/resources/alphanum_test_1_input.txt`/`_output.txt` and `alphanum_test_2_*`. Port the default cases exactly:

| string1 | relation | string2 |
|---|---|---|
| z2.doc | < | z10.doc |
| z010.doc | = | z10.doc |
| z10 | < | z10a |
| z10a | > | z10 |
| a | < | z |
| 10 | < | z |
| z | > | 10 |
| 10 | < | 20 |
| 10 | > | 5 |
| 10 | = | 10 |
| 9223372036854775806 | < | 9223372036854775807 |
| 18446744073709551614 | < | 18446744073709551615 |
| a | = | A (case-insensitive only) |

The alphanum resource files (input/output lists) are copied into `kuro-source-local/tests/fixtures/natsort/` (see §4) and checked with `InputOutputTest` semantics: sorting the input with `natural_cmp` (case-sensitive variant) must equal the output file.

### 3.3 Upstream tests for EPUB, ComicInfo, archives
None exist in `aniyomi-src` for this scope (`find source-local core-metadata -name "*Test*"` is empty). New tests are required (§5).

---

## 4. Fixtures

All fixtures are generated by test code into `tempfile::TempDir` unless they are binary files checked in under `kuro-source-local/tests/fixtures/`.

| Fixture | How created | Used by |
|---|---|---|
| `manga_tree/` | test code: folders `Foo/`, `Foo/Ch 01/` (3 jpg), `Foo/Ch 02.cbz` (zip with 2 jpg), `Foo/Ch 03.epub`, `.hidden/`, `Bar/` (only cover.png) | browse, chapters, cover |
| `anime_tree/` | test code: `Show/Ep 01.mkv` (0-byte placeholder is enough for listing), `Show/Ep 02.MKV` (uppercase), `Show/cover.jpg`, `Show/Season 1/Ep 01.mp4`, `Other/Season A/x.mp4`, `Dir.mkv/` (dir) | anime listing, fetch type |
| `comicinfo/` | test code writes ComicInfo.xml variants: `full.xml`, `empty_series.xml`, `no_status.xml`, `malformed.xml`, `bom.xml`, `utf16.xml`, `entities.xml`, `dup_series.xml`, `unbound_status.xml` | ComicInfo read tests |
| `comicinfo/golden_upstream.xml` | generated once by running upstream `ComicInfo` serializer (`XML` config in `AppModule.kt:179-189`) on the sample in §4.1 of this file; committed | golden write test |
| `epub/minimal.epub` | built by test code (zip: `mimetype`, `META-INF/container.xml` -> `OEBPS/content.opf`, `OEBPS/content.opf` with `dc:title`, `dc:creator`, `dc:publisher`, `dc:description`, `dc:date`, manifest and spine, `OEBPS/p1.xhtml` with two `img`) | epub metadata, cover, page list |
| `epub/backslash.epub` | same with `META-INF\container.xml` entry name (separator detection) | separator test |
| `epub/corrupt.epub` | random bytes | E-MANGA-3, E-READ-4 |
| `media/tiny.mp4` | generated with `ffmpeg -f lavfi -i testsrc=duration=4:size=64x64:rate=5 -y tiny.mp4` (documented command; committed as a 5 KB binary) | ffprobe / frame tests (gated by tool presence) |
| `media/fake-ffprobe.sh`, `media/fake-ffmpeg.sh` | shell scripts that print a fixed duration and write a 1-byte JPEG, or sleep for timeouts, or echo args | media tests without ffmpeg |
| `json/chapters_valid.json`, `json/chapters_missing_number.json`, `json/chapters_bad_date.json`, `json/episodes_valid.json`, `json/details_manga.json`, `json/details_anime.json` | hand-written | JSON tests |
| `natsort/alphanum_test_1_input.txt`, `alphanum_test_1_output.txt` (and `_2_`) | copied from `gpanther/java-nat-sort` `src/test/resources` (Apache-2.0 / upstream license noted in the fixture header) | natsort tests |

### 4.1 Golden ComicInfo sample
Input: `ComicInfo { series: "Foo", summary: "A & B <c>", writer: "W", penciller: "P", genre: "Action, Drama", publishing_status: "Ongoing", categories: "Read", source: "Source A", number: "12", title: "Chapter 12", web: "https://a.example/1", translator: "Team" }`. The golden file is what the upstream serializer emits for the same object (generated once, §4 table). Until generated, the test asserts the structure described in data §5.4 with a regex and is marked `#[ignore = "golden pending"]`. (This is the only ignored test allowed in the area; removing the ignore is the acceptance criterion of T-14a-030.)

---

## 5. New tests to write (behavioral)

### 5.1 Filesystem and naming
- `t_14_names_extension_no_dot`: `extension("cover") == "cover"`, `stem("cover") == "cover"`, `extension(".nomedia") == "nomedia"`, `stem("a.b.cbz") == "a.b"`, `extension("a.b.cbz") == "cbz"`.
- `t_14_format_manga_archive_case_insensitive`: `Vol.CBZ`, `vol.cbz`, `Vol.Cb7` true; `Vol.mkv` false.
- `t_14_format_anime_case_sensitive`: `Ep.mkv` true, `Ep.MKV` false, `Ep.torrent` true.
- `t_14_format_epub_case_insensitive`: `Book.EPUB` -> Epub.
- `t_14_format_dir_wins`: a directory named `Book.epub` -> Directory.
- `t_14_format_order`: `archive.epub` (file) -> Epub, not Archive.
- `t_14_fs_safe_join_accepts_nested`: `"Show/Season 1/ep.mkv"` resolves under root.
- `t_14_path_rejects_traversal` (E-PATH-1), `t_14_path_rejects_absolute`, `t_14_path_rejects_empty_component`, `t_14_path_rejects_nul`.

### 5.2 Images
- `t_14_images_sniff_table`: 1x1 PNG, JPEG (FFD8FF), GIF87a/GIF89a, WEBP RIFF header with `WEBP` at 8, AVIF with `ftypavif`, HEIF `ftypheic`, JXL codestream FF0A and container header -> expected type; random bytes -> None.
- `t_14_images_is_image_named_ext_first`: `cover.jpg` true without reading bytes (the closure must not be called).
- `t_14_images_find_first_sorted` (E-IMG-5), `t_14_images_overwrite_keeps_name` (E-IMG-1), `t_14_images_nomedia_once` (E-IMG-6), `t_14_images_write_atomic_rename` (RC-1).

### 5.3 Natural comparator and sorting
- All 13 cases of §3.2 as `#[test]`s.
- `t_14_sort_title_is_not_natural` (E-BROWSE-4).
- `t_14_chapters_sort_number_desc_then_name_desc` (E-CHAPTERS-6).

### 5.4 Browse
- `t_14_browse_popular_title_asc_default` (default filter = Title ascending).
- `t_14_browse_latest_date_desc_window` (E-BROWSE-6, E-BROWSE-7).
- `t_14_browse_query_contains_ignore_case` (E-BROWSE-3).
- `t_14_browse_whitespace_query_is_blank` (query `"   "` returns everything).
- `t_14_browse_search_sort_none_keeps_name_order` (no sort filter -> name-sorted list, Rust determinism).

### 5.5 Chapters and covers
- `t_14_chapters_dirs_archives_epubs_only` (E-CHAPTERS-8, E-CHAPTERS-9).
- `t_14_chapters_url_format` (`"Foo/Ch 02.cbz"`, directory name without extension).
- `t_14_chapters_json_override_name_date_scanlator` (E-CHAPTERS-4).
- `t_14_chapters_json_first_match_wins` (E-CHAPTERS-2), `t_14_chapters_json_tolerance` (E-CHAPTERS-3), `t_14_chapters_json_missing_number_disables_file` (E-CHAPTERS-1).
- `t_14_chapters_json_bad_date_keeps_file_date` (E-DATE-2 deviation).
- `t_14_cover_fallback_only_when_blank`, `t_14_cover_fallback_corrupt_archive_swallowed`, `t_14_cover_archive_first_image_natural` (E-CHAPTERS-11/12, E-IMG-10).

### 5.6 Details and ComicInfo
- `t_14_details_branch_a_comicinfo_wins_over_json` (precedence).
- `t_14_details_branch_b_legacy_json_migrates_and_deletes` (E-MANGA-6 success path: json removed, ComicInfo written, fields copied, status mapped).
- `t_14_details_branch_c_archive_copy` (ComicInfo in archive copied to folder root and applied).
- `t_14_details_noxml_marker_created_once` (E-MANGA-4).
- `t_14_details_comicinfo_removes_noxml` (E-MANGA-5).
- `t_14_details_dir_named_cbz_skipped` (E-MANGA-DETAILS-2).
- `t_14_details_concurrent_same_manga_single_write` (RC-3).
- `t_14_comicinfo_golden_upstream` (write golden, §4.1; ignored until generated).
- `t_14_comicinfo_roundtrip_all_fields` (write then read yields the same struct for every field).
- `t_14_comicinfo_genre_merge_no_split` (E-CI-3), `t_14_comicinfo_artist_merge_split` (E-CI-3).
- `t_14_comicinfo_absent_status_resets_to_unknown` (E-CI-2), `t_14_comicinfo_empty_series_assigns_empty` (E-CI-1).
- `t_14_comicinfo_status_text_table`: all 7 status pairs in both directions; `"ongoing"` lowercase -> 0.
- `t_14_comicinfo_status_upcoming_writes_unknown`: `to_comic_info_value(7) == "Unknown"`.
- `t_14_comicinfo_write_escapes` (E-CI-10), `t_14_comicinfo_entities_roundtrip` (E-CI-9).
- `t_14_comicinfo_duplicate_last_wins` (E-CI-4), `t_14_comicinfo_unknown_children_ignored` (E-CI-5), `t_14_comicinfo_status_local_name_match` (E-CI-6), `t_14_comicinfo_wrong_root_error` (E-CI-7), `t_14_comicinfo_empty_file_error` (E-CI-8), `t_14_comicinfo_utf8_bom_accepted` and `t_14_comicinfo_utf16_rejected` (E-CI-12).
- `t_14_manga_to_comic_info_fields` (bridge: series always, summary/writer/penciller/genre optional, status always).

### 5.7 EPUB
- `t_14_epub_fill_metadata_fields` (creator -> author, description -> description, title -> chapter name, publisher -> scanlator, date parsed).
- `t_14_epub_scanlator_falls_back_to_creator` (publisher absent).
- `t_14_epub_date_formats` (E-DATE-3).
- `t_14_epub_separator_backslash` (container entry with `\`).
- `t_14_epub_first_image_spine_order`.
- `t_14_epub_image_xlink_href` (`<image xlink:href>` in SVG cover page).
- `t_14_epub_corrupt_keeps_listing` (E-MANGA-3).

### 5.8 Anime
- `t_14_anime_browse_fetch_type_and_images` (cover and background found; fetch type Episodes for `Show`).
- `t_14_fetch_type_episode_wins_over_dir` (`Show/` with episode and dir -> Episodes).
- `t_14_fetch_type_dir_only_is_seasons`, `t_14_fetch_type_hidden_dir_counts` (E-ANIME-8), `t_14_fetch_type_mixed_is_episodes` (E-ANIME-10).
- `t_14_seasons_list_names_and_urls` (`Show/Season 1` url `Show/Season 1`, title `Season 1`).
- `t_14_seasons_nested_urls_resolve` (E-ANIME-11), `t_14_seasons_empty_season_episode_list_empty` (E-ANIME-12).
- `t_14_episodes_number_uses_anime_title` (E-ANIME-1), `t_14_episodes_uppercase_rejected` (E-CHAPTERS-9), `t_14_episodes_dir_named_mkv_excluded` (E-ANIME-9).
- `t_14_episodes_json_bad_fails_listing` (E-ANIME-3), `t_14_episodes_json_missing_number_fails` (E-ANIME-4), `t_14_episodes_json_preview_and_filler_ignored` (E-ANIME-5), `t_14_episodes_json_overrides_summary_scanlator` (unconditional).
- `t_14_anime_details_bad_json_fails_update` (E-ANIME-2).
- `t_14_anime_cover_from_lowest_episode` (E-ANIME-13), `t_14_anime_cover_fallback_when_file_missing` (E-ANIME-14).
- `t_14_anime_season_update_flags_false_keeps_input`.
- `t_14_anime_season_update_error_from_one_side_still_returns_error`.

### 5.9 Playback and reader format
- `t_14_playback_hoster_single_video_title` (E-PLAY-3), `t_14_playback_missing_file_empty` (E-PLAY-1), `t_14_playback_bad_url_empty` (E-PLAY-2), `t_14_playback_external_uses_path` (E-PLAY-4).
- `t_14_format_missing_chapter` (E-READ-2), `t_14_format_bad_url_chapter_not_found` (E-READ-1), `t_14_format_unknown_type_invalid` (E-READ-3), `t_14_format_epub_open_error_propagates` (E-READ-4).

### 5.10 Media (ffmpeg)
- `t_14_media_frame_position_truncates` (E-MEDIA-7), `t_14_media_missing_tool_skips` (E-MEDIA-1), `t_14_media_bad_duration` (E-MEDIA-2), `t_14_media_zero_duration_frame_zero` (E-MEDIA-3), `t_14_media_empty_output_no_image` (E-MEDIA-4), `t_14_media_timeout_kills_child` (E-MEDIA-5), `t_14_media_path_with_quotes_and_spaces` (E-MEDIA-6), `t_14_media_serialized` (E-MEDIA-8), `t_14_media_drop_cleans_temp` (RC-7), `t_14_thumbnail_skip_existing` (E-ANIME-6), `t_14_thumbnail_temp_removed` (E-ANIME-7).
- Real-tool test `t_14_media_real_ffmpeg_frame` (gated: skips with message when `ffmpeg` is not on PATH): probe `tiny.mp4` -> duration 4.0, frame at 2 s, output JPEG magic.

### 5.11 Thumbnails and covers
- `t_14_thumbnail_write_overwrites_same_name` (E-IMG-7), `t_14_thumbnail_name_sanitized` (E-IMG-8), `t_14_images_background_case_insensitive` (E-IMG-9), `t_14_images_uppercase_ext_via_sniff` (E-IMG-3), `t_14_images_jpeg_extension_sniffed` (E-IMG-4), `t_14_images_no_validation` (E-IMG-2).
- `t_14_set_cover_success_returns_file_url`.

### 5.12 Library and sync rules (checked through the domain functions when area:07-domain-entries-items, area:08-domain-library-category-history-updates and area:10-app-domain-layer land; listed for ownership)
- `t_14_badge_pref_respected` (E-SYNC-2), `t_14_filter_downloaded_true_for_local` (E-SYNC-3), `t_14_download_count_local_excludes_epub` (E-SYNC-4), `t_14_download_size_local_recursive` (E-SYNC-5), `t_14_sync_local_empty_ok` (E-SYNC-1).

### 5.13 Path and platform
- `t_14_path_symlink_inside_root_listed` (E-PATH-2), `t_14_path_backslash_name_listed_not_addressable` (E-PATH-3), `t_14_path_non_utf8_listed_lossy` (E-PATH-5), `t_14_browse_permission_denied_error` (E-BROWSE-10, unix only, skipped under root).

### 5.14 Time
- `t_14_dates_strict_invalid_no_override` (E-DATE-1), `t_14_dates_trailing_text_rejected` (E-DATE-1), `t_14_dates_use_local_offset` (E-DATE-4), `t_14_chapters_zero_mtime_kept` (E-DATE-5).

### 5.15 JSON
- `t_14_json_unknown_keys_ignored` (E-JSON-1), `t_14_json_null_equals_absent` (E-JSON-2), `t_14_json_wrong_type_fails` (E-JSON-3), `t_14_json_genre_string_fails` (E-JSON-4), `t_14_json_number_forms` (E-JSON-5), `t_14_json_bom_accepted` (E-JSON-6), `t_14_json_empty_file_error` (E-JSON-7).

---

## 6. Test infrastructure
- `kuro-testkit` provides a temp-dir `Paths` (area:01-foundation-app-bootstrap) and an in-memory preference store (area:02-preferences-system).
- Clock injection: `browse_inner(root, mode, now_ms)`; tests pass `now_ms` explicitly.
- Modification time setting: `filetime` crate (dev-dependency) for `t_14_browse_latest_*`, `t_14_chapters_zero_mtime_kept` (set 0 is not possible through `filetime` for all platforms; use `filetime::set_file_mtime` with `FileTime::from_unix_time(0, 0)` which is valid on Linux).
- Fake tools: scripts in `tests/fixtures/media/` are executable (set mode 0755 in the test on unix; skip on windows).
- Parallelism: tests are independent (each uses its own temp dir); no global state except the local-offset `OnceLock` (tests that need an offset set it through the test-only setter before any read).

---

## 7. Coverage map (behavior -> test)

| Behavior section | Tests |
|---|---|
| §1 identity | `t_14_source_identity`, `t_14_is_local_predicates` |
| §2 roots | `t_14_browse_missing_root_empty`, `t_14_roots_created_lazily` |
| §3 URLs | `t_14_path_rejects_traversal`, `t_14_fs_safe_join_accepts_nested`, `t_14_seasons_nested_urls_resolve` |
| §4.1-4.2 names/formats | §5.1 |
| §4.4 images | §5.2 |
| §4.5 natsort | §5.3 |
| §5 recognition | §3.1 smoke + `t_14_chapters_url_format` |
| §6 manga browse | §5.4 |
| §7 details | §5.6 |
| §8 chapters | §5.5 |
| §9 covers | §5.5, §5.11 |
| §10 reader | §5.9 |
| §11 anime | §5.8 |
| §12 seasons | §5.8 |
| §13 managers | §5.11 |
| §14 ffmpeg | §5.10 |
| §15-16 set-as-art | §5.11 |
| §17 library | §5.12 |
| §18 sync | §5.12 |
| §19 ComicInfo | §5.6 |
| §20 JSON | §5.15 |
| §21 EPUB | §5.7 |
| §22 defaults | `t_14_entry_defaults` (each default from data §1 asserted) |
| §23 messages | `t_14_messages_exact_text` ("Chapter not found", "Invalid chapter format", "Local manga source", "Local anime source", "Order by", "Title", "Date") |
| §24 concurrency | RC-3, RC-7 |
| §25 EPUB contract | §5.7 |
| §26 prefs | `t_14_pref_badge_key_default_true` (key `display_local_badge`) |
| §27 platform | §5.13 |
| §28-29 interactions/help | `t_14_help_urls_exact` |

---

## 8. Acceptance thresholds for the area
- All `t_14_*` tests pass on Linux without optional tools; media tests that need `ffmpeg` are skipped with a printed reason.
- The natsort port passes all cases in §3.2.
- The ComicInfo golden test is un-ignored and passes after the golden file is generated from the upstream serializer.
- No test depends on filesystem enumeration order.
