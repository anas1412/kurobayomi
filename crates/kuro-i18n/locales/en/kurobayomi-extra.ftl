# source: kurobayomi
restore_checking_backup = Checking backup file…

# source: kurobayomi
restore_close_keep_running = Keep running

# source: kurobayomi
restore_close_cancel_and_close = Cancel restore and close

# source: kurobayomi
restore_progress_anime_categories = Anime categories

# source: kurobayomi
restore_progress_manga_categories = Manga categories

# source: kurobayomi
backup_extensions_info = Extension packages are saved as Kurobayomi extension files. Android APK extensions are replaced by them and are not included.

# source: kurobayomi
ext_not_runnable_on_linux = Not runnable on Linux

# source: kurobayomi
ext_unsigned_package = Unsigned package

# source: kurobayomi
ext_store_not_runnable_hidden =
    { $count ->
        [one] 1 entry not runnable on Linux
       *[other] { NUMBER($count, useGrouping: "false") } entries not runnable on Linux
    }

# source: kurobayomi
ext_load_problems = Load problems

# source: kurobayomi
settings_save_failed = Could not save settings. Your changes are kept and saved again on the next change.

# source: kurobayomi
pref_download_only_over_wifi = Unmetered connection only

# source: kurobayomi
db_error_too_new = This library was created by a newer version of Kurobayomi (database version { NUMBER($arg1, useGrouping: "false") }, this version supports up to { NUMBER($arg2, useGrouping: "false") }). Update Kurobayomi to open it.

# source: kurobayomi
db_error_legacy_file = This is an Aniyomi library database. Import it to keep your library.

# source: kurobayomi
db_error_corrupt = The library database is damaged. Restore it from the newest backup, or start with an empty library.

# source: kurobayomi
db_restore_from_backup = Restore from backup

# source: kurobayomi
db_import_report = Imported { NUMBER($arg1, useGrouping: "false") } entries. { NUMBER($arg2, useGrouping: "false") } could not be imported.

# source: kurobayomi
db_pre_migration_backup = A backup of your library was saved before the upgrade at { $arg1 }.

# source: kurobayomi
cloudflare_check_title = Cloudflare check

# source: kurobayomi
cloudflare_check_hint = Complete the check in this window; it closes by itself when it succeeds.

# source: kurobayomi
cookie_import_title = Import Cloudflare cookie

# source: kurobayomi
cookie_import_hint = Paste the value of the cf_clearance cookie from your browser and the browser's user agent.

# source: kurobayomi
pref_library_summary = Categories, global update

# source: kurobayomi
pref_player_internal_summary = Progress, controls

# source: kurobayomi
pref_security_summary = App lock

# source: kurobayomi
pref_advanced_summary = Dump crash logs

# source: kurobayomi
pref_reset_viewer_flags_summary = Resets reading mode of all series

# source: kurobayomi
add_source_repository = Add a source repository

# source: kurobayomi
restore_from_backup = Restore from a backup

# source: kurobayomi
library_empty_search_hint = Your library is empty. Search for something in Browse to add it.

# source: kurobayomi
welcome_title = Welcome to Kurobayomi

# source: kurobayomi
welcome_intro = Add a source repository to start watching anime and reading manga.

# source: kurobayomi
welcome_guide = Getting started guide

# source: kurobayomi
action_select = Select

# source: kurobayomi
episode_time_left = { $arg1 } left

# source: kurobayomi
action_delete_download = Delete download

# source: kurobayomi
action_mark_previous_as_seen = Mark previous as watched

# source: kurobayomi
label_extension_repos = Repositories

# source: kurobayomi
left_to_right_viewer = Paged, left to right

# source: kurobayomi
right_to_left_viewer = Paged, right to left

# source: kurobayomi
vertical_viewer = Paged, vertical

# source: kurobayomi
source_status_active = Working

# source: kurobayomi
source_status_blocked = Temporarily blocked by the site

# source: kurobayomi
source_status_broken = Broken: waiting for a fix

# source: kurobayomi
browse_check_sources = Check for new and updated sources

# source: kurobayomi
downloads_check_files = Check the downloaded files again
