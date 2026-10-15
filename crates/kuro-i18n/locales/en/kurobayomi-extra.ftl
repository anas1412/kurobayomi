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

# source: kurobayomi
reader_unknown_err = Unknown err

# source: kurobayomi
reader_chapter_not_downloaded = The chapter is not downloaded. Turn off downloaded-only mode or download it first.

# source: kurobayomi
reader_action_show_in_file_manager = Show in file manager

# source: kurobayomi
reader_custom_brightness_unavailable = Custom brightness is not available on this display

# source: kurobayomi
reader_menu_reader_settings = Reader settings…

# source: kurobayomi
reader_cutout_dropped_note = Show content in cutout area is not used on Linux: there is no display cutout.

# source: kurobayomi
reader_volume_keys_media_caveat = The desktop can take the volume keys before the reader gets them. Page Up, Page Down and Space always turn pages.

# source: kurobayomi
reader_pref_restore_after_crash = Restore the chapter and page after a crash

# source: kurobayomi
reader_requested_chapter_not_found = Requested chapter of id { NUMBER($arg1, useGrouping: "false") } not found in chapter list

# source: kurobayomi
browse_errors_offline = You're offline. Check your connection and try again.

# source: kurobayomi
browse_errors_generic = Something went wrong. Try again in a moment.

# source: kurobayomi
browse_errors_not_working_badge = Not working right now

# source: kurobayomi
browse_global_search_empty_hint = Search a source by title

# source: kurobayomi
browse_loading_sources = Loading sources…

# source: kurobayomi
entry_list_options_tooltip = List options

# source: kurobayomi
library_exclude_hint = Use -term to exclude

# source: kurobayomi
misc_interval_first_run_note = The first automatic update runs as soon as the conditions are met after you turn this on.

# source: kurobayomi
repos_adding_repo = Adding repository…

# source: kurobayomi
repos_could_not_add_repo = Could not add the repo

# source: kurobayomi
reader_still_loading = The page list is still loading. Try again in a moment.

# source: kurobayomi
crash_no_details = No crash details available

# source: kurobayomi
crash_subtitle = Kurobayomi ran into an unexpected error. Share the crash logs when you report the problem.

# source: kurobayomi
about_build_time_title = Build time

# source: kurobayomi
about_model_title = Device model

# source: kurobayomi
about_menu_title = Show in applications menu

# source: kurobayomi
about_startup_check_title = Check for updates at startup

# source: kurobayomi
about_startup_check_subtitle = A newer version is downloaded in the background and runs after a restart

# source: kurobayomi
settings_auth_unlock_button = Unlock

# source: kurobayomi
settings_auth_wrong_passphrase = Wrong passphrase

# source: kurobayomi
settings_browse_source_languages = Source languages

# source: kurobayomi
browse_nsfw_refused = Authentication is required to change this setting

# source: kurobayomi
settings_chooser_unavailable = No file chooser available

# source: kurobayomi
settings_library_csv_failed = Could not export library

# source: kurobayomi
settings_maintenance_copied_text = Crash logs copied to the clipboard

# source: kurobayomi
settings_maintenance_failed_text = Failed to get logs

# source: kurobayomi
settings_notifications_new_items_title = Notify about new chapters and episodes

# source: kurobayomi
player_default_volume = Default volume

# source: kurobayomi
settings_storage_info = Holds downloads (Anime, Manga), backups and local folders. A new location is used for new downloads after a restart; files already there stay where they are.

# source: kurobayomi
settings_tracking_not_ready = Tracker services are not available yet

# source: kurobayomi
settings_tracking_waiting = Waiting for the browser login…

# source: kurobayomi
settings_usage_unavailable = Storage size unavailable

# source: kurobayomi
shell_incognito_label = Incognito

# source: kurobayomi
storage_storage_scanning = Scanning downloads…

# source: kurobayomi
storage_storage_empty_category = No downloads in this category

# source: kurobayomi
storage_storage_source_missing = Source not installed; files not deleted

# source: kurobayomi
storage_delete_source_missing = Source not installed, downloads were not deleted

# source: kurobayomi
history_row_open_details = Open details

# source: kurobayomi
history_row_remove_from_history = Remove from history

# source: kurobayomi
library_update_could_not_start = Could not start library update

# source: kurobayomi
ui_use_for_all_titles = Use for all titles

# source: kurobayomi
ui_download_all = Download all

# source: kurobayomi
ui_connect_anilist = Connect AniList

# source: kurobayomi
ui_tracking_keeps_your_progress_on_anilist = Tracking keeps your progress on AniList.

# source: kurobayomi
ui_stop_tracking = Stop tracking

# source: kurobayomi
ui_from_a_local_folder = From a local folder

# source: kurobayomi
ui_checking_the_downloaded_files_again = Checking the downloaded files again

# source: kurobayomi
ui_compact = Compact

# source: kurobayomi
ui_restart = Restart

# source: kurobayomi
ui_links = Links

# source: kurobayomi
ui_failed_to_reindex_downloads = Failed to reindex downloads

# source: kurobayomi
ui_search_instead = Search instead

# source: kurobayomi
ui_restart_kurobayomi_to_use_the_new_version = Restart Kurobayomi to use the new version

# source: kurobayomi
ui_could_not_load_whats_new = Could not load what's new

# source: kurobayomi
ui_new = New

# source: kurobayomi
ui_update_waiting_open_about = An update is waiting: open About, What's new

# source: kurobayomi
settings_log_out_of_tracker = Log out of this tracker?

# source: kurobayomi
repos_page_repos_empty = No repositories yet. Add a link someone gave you to get sources.

# source: kurobayomi
repos_page_add_title = Add a repository

# source: kurobayomi
repos_page_add_intro = Paste the repository link.

# source: kurobayomi
repos_page_input_label = Repository link

# source: kurobayomi
library_load_failed = Could not load the library

# source: kurobayomi
browse_migrate_tracks_always = Tracking data is always transferred.

# source: kurobayomi
browse_open_website = Open website

# source: kurobayomi
browse_search_options = Search options

# source: kurobayomi
storage_showing_latest_500 = Showing latest 500

# source: kurobayomi
ui_quit = Quit

# source: kurobayomi
ui_ascending_descending = Ascending / descending
