# Default values written conceptually into config.toml and state.toml (generated from the Kotlin defaults, see p1-topic-key-registry.md for key strings)

Two files: `config.toml` holds normal keys, `state.toml` holds app-state keys. Private keys (`trackUsername`, `trackPassword`, `trackToken`) are not in either file: they live in the keyring. Per-tracker maps (`filter_tracked_*`, `track_auth_expired`) are TOML tables keyed by the tracker id as a string and are absent while at default. Every field below has `#[serde(default)]` in Rust.

## config.toml

```toml
schema = 1

[base]
extension_installer = "PACKAGEINSTALLER (LEGACY on MIUI)"
display_profile = ""
hardware_bitmap_threshold = 2048
always_decode_long_strip_with_ssiv = false

[ui]
theme_mode = "SYSTEM"
app_theme = "DEFAULT"
theme_dark_amoled = false
relative_time = true
date_format = ""
tablet_ui_mode = "AUTOMATIC"
start_screen = "ANIME"
nav_style = "MOVE_HISTORY_TO_MORE"

[sources]
source_display_mode = "COMPACT_GRID"
enabled_languages = ["all", "en"]  # plus the system language at first run
show_nsfw_source = true
migration_sorting_mode = "ALPHABETICAL"
migration_sorting_direction = "ASCENDING"
anime_extension_repos = []
manga_extension_repos = []
disabled_anime_sources = []
disabled_manga_sources = []
incognito_anime_extensions = []
incognito_manga_extensions = []
pinned_anime_sources = []
pinned_manga_sources = []
anime_extension_updates_count = 0
manga_extension_updates_count = 0
hide_in_anime_library_items = false
hide_in_manga_library_items = false
enable_source_blacklist = true
sources_tab_categories = []
sources_tab_categories_filter = false
sources_tab_sources_in_categories = []

[sources.data_saver]
data_saver = "NONE"
data_saver_ignore_jpeg = false
data_saver_ignore_gif = true
data_saver_image_quality = 80
data_saver_image_format_jpeg = false
data_saver_server = ""
data_saver_color_bw = false
data_saver_excluded_sources = []
data_saver_downloader = true

[tracking]
anilist_score_type = "POINT_10"
auto_update_track = true
track_on_adding_to_library = true
show_next_episode_airing_time = true
auto_update_track_on_mark_read = "ALWAYS"

[library]
display_mode = "COMPACT_GRID"
manga_sorting_mode = "ALPHABETICAL,ASCENDING"
anime_sorting_mode = "ALPHABETICAL,ASCENDING"
auto_update_interval = 0
auto_update_device_restrictions = ["wifi"]
auto_update_item_restrictions = ["manga_fully_read", "manga_ongoing", "manga_started", "manga_outside_release_period"]
auto_update_metadata = false
show_continue_viewing_button = false
category_tabs = true
category_number_of_items = false
categorized_display_settings = false
hide_hidden_categories_settings = false
filter_interval_custom = "DISABLED"
download_badge = false
unread_badge = true
local_badge = true
language_badge = false
new_show_updates_count = true
auto_clear_item_cache = false
random_anime_sort_seed = 0
random_manga_sort_seed = 0
anime_portrait_columns = 0
manga_portrait_columns = 0
anime_landscape_columns = 0
manga_landscape_columns = 0
filter_downloaded_anime = "DISABLED"
filter_downloaded_manga = "DISABLED"
filter_unseen = "DISABLED"
filter_unread = "DISABLED"
filter_started_anime = "DISABLED"
filter_started_manga = "DISABLED"
filter_bookmarked_anime = "DISABLED"
filter_bookmarked_manga = "DISABLED"
filter_completed_anime = "DISABLED"
filter_completed_manga = "DISABLED"
new_manga_updates_count = 0
new_anime_updates_count = 0
default_anime_category = -1
default_manga_category = -1
anime_update_categories = []
manga_update_categories = []
anime_update_categories_exclude = []
manga_update_categories_exclude = []
filter_episode_by_seen = 0
filter_chapter_by_read = 0
filter_episode_by_downloaded = 0
filter_chapter_by_downloaded = 0
filter_episode_by_bookmarked = 0
filter_chapter_by_bookmarked = 0
filter_episode_by_fillermarked = 0
sort_episode_by_source_or_number = 0
sort_chapter_by_source_or_number = 0
display_episode_by_name_or_number = 0
display_chapter_by_name_or_number = 0
sort_episode_by_ascending_or_descending = 0
sort_chapter_by_ascending_or_descending = 0
show_episode_thumbnail_previews = 0
show_episode_summaries = 0
filter_season_by_download = 0
filter_season_by_unseen = 0
filter_season_by_started = 0
filter_season_by_completed = 0
filter_season_by_bookmarked = 0
filter_season_by_fillermarked = 0
sort_season_by_source_or_number = 0
sort_season_by_ascending_or_descending = 0
season_display_grid_mode = 0
season_display_grid_size = 0
season_download_overlay = false
season_unseen_overlay = true
season_local_overlay = true
season_lang_overlay = false
season_continue_overlay = true
season_display_mode = 0
update_season_on_refresh = false
update_season_on_library_update = false
swipe_episode_start_action = "ToggleSeen"
swipe_episode_end_action = "ToggleBookmark"
swipe_chapter_start_action = "ToggleRead"
swipe_chapter_end_action = "ToggleBookmark"
mark_duplicate_read_chapter_as_read = []
mark_duplicate_seen_episode_as_seen = []

[downloads]
download_only_over_wifi = true
use_external_downloader = false
external_downloader_selection = ""
save_chapters_as_cbz = true
split_tall_images = true
auto_download_while_reading = 0
auto_download_while_watching = 0
remove_after_read_slots = -1
remove_after_marked_as_read = false
remove_bookmarked_chapters = false
download_fillermarked_items = false
remove_exclude_categories = []
remove_exclude_anime_categories = []
download_new_chapters = false
download_new_episodes = false
download_new_chapter_categories = []
download_new_episode_categories = []
download_new_chapter_categories_exclude = []
download_new_episode_categories_exclude = []
number_of_downloads = 3
download_speed_limit = 0
download_new_unread_chapters_only = false
download_new_unseen_episodes_only = false

[backup]
backup_interval = 12
backup_flags = ["1", "2", "4", "8"]

[security]
use_authenticator = false
lock_app_after = 0
secure_screen = "INCOGNITO"
hide_notification_content = false

[network]
verbose_logging = false  # true in debug builds
doh_provider = -1
default_user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"

[torrent]
torr_server_enable = false
torr_server_shown_notice = false
torr_server_port = "8090"
torr_server_trackers = """
http://nyaa.tracker.wf:7777/announce
http://anidex.moe:6969/announce
http://tracker.anirena.com:80/announce
udp://tracker.uw0.xyz:6969/announce
http://share.camoe.cn:8080/announce
http://t.nyaatracker.com:80/announce
udp://47.ip-51-68-199.eu:6969/announce
udp://9.rarbg.me:2940
udp://9.rarbg.to:2820
udp://exodus.desync.com:6969/announce
udp://explodie.org:6969/announce
udp://ipv4.tracker.harry.lu:80/announce
udp://open.stealth.si:80/announce
udp://opentor.org:2710/announce
udp://opentracker.i2p.rocks:6969/announce
udp://retracker.lanta-net.ru:2710/announce
udp://tracker.cyberia.is:6969/announce
udp://tracker.dler.org:6969/announce
udp://tracker.ds.is:6969/announce
udp://tracker.internetwarriors.net:1337
udp://tracker.openbittorrent.com:6969/announce
udp://tracker.opentrackr.org:1337/announce
udp://tracker.tiny-vps.com:6969/announce
udp://tracker.torrent.eu.org:451/announce
udp://valakas.rollo.dnsabr.com:2710/announce
udp://www.torrent.eu.org:451/announce"""  # leading newline is trimmed by TOML
torr_server_proxy_mode = "None"
torr_server_proxy_url = ""

[reader]
page_transitions = true
flash_on_page_change = false
flash_duration_millis = 100
flash_page_interval = 1
flash_color = "BLACK"
double_tap_anim_speed = 500
show_page_number = true
show_reading_mode = true
fullscreen = true
cutout_short = true
keep_screen_on = true
default_reading_mode = 2
default_orientation_type = 8
webtoon_double_tap_zoom_enabled = true
image_scale_type = 1
zoom_start = 1
reader_theme = 1
always_show_chapter_transition = true
preserve_reading_position = false
crop_borders = false
navigate_to_pan = true
landscape_zoom = true
crop_borders_webtoon = false
webtoon_side_padding = 0
reader_hide_threshold = "LOW"
folder_per_manga = false
skip_read = false
skip_filtered = true
skip_dupe = false
webtoon_disable_zoom_out = false
dual_page_split_paged = false
dual_page_invert_paged = false
dual_page_split_webtoon = false
dual_page_invert_webtoon = false
dual_page_rotate_to_fit = false
dual_page_rotate_to_fit_invert = false
dual_page_rotate_to_fit_webtoon = false
dual_page_rotate_to_fit_invert_webtoon = false
custom_brightness = false
custom_brightness_value = 0
color_filter = false
color_filter_value = 0
color_filter_mode = 0
grayscale = false
inverted_colors = false
read_with_long_tap = true
read_with_volume_keys = false
read_with_volume_keys_inverted = false
navigation_mode_pager = 0
navigation_mode_webtoon = 0
pager_nav_inverted = "NONE"
webtoon_nav_inverted = "NONE"
show_navigation_overlay_new_user = true
show_navigation_overlay_on_start = false

[player]
preserve_watching_position = false
progress_preference = 0.85
default_player_orientation_type = "SensorLandscape"
allow_gestures = false
show_loading_circle = true
show_current_chapter = true
remember_player_brightness = false
player_brightness_value = -1.0
remember_player_volume = false
player_volume_value = -1.0
show_failed_hosters = false
show_empty_hosters = false
player_fullscreen = true
hide_controls = false
display_vol_per = true
show_system_status_bar = false
reduce_motion = false
player_time_to_disappear = 4000
panel_opacity = 60
enable_skip_intro = true
auto_skip_intro = false
enable_netflix_style_intro_skip = false
waiting_time_intro_skip = 5
ani_skip_enabled = false
disable_ani_skip_on_chapters = true
enable_pip = true
pip_episode_toasts = true
pip_on_exit = false
pip_replace_with_previous = false
always_use_external_player = false
external_player_preference = ""
player_speed = 1.0
speed_presets = ["0.25", "0.5", "0.75", "1.0", "1.25", "1.5", "1.75", "2.0", "2.5", "3.0", "3.5", "4.0"]
invert_duration = false
aspect_state = "Fit"
autoplay_enabled = false

[player.gestures]
gesture_volume_brightness = true
swap_volume_brightness = false
gesture_horizontal_seek = true
show_seek_bar = false
default_intro_length = 85
skip_length_preference = 10
player_smooth_seek = false
left_double_tap_gesture = "Seek"
center_double_tap_gesture = "PlayPause"
right_double_tap_gesture = "Seek"
media_previous_gesture = "Switch"
media_play_pause_gesture = "PlayPause"
media_next_gesture = "Switch"

[player.decoder]
try_hw_decoding = true
gpu_next = false
video_debanding = "None"
use_yuv420p = true
brightness_filter = 0
saturation_filter = 0
contrast_filter = 0
gamma_filter = 0
hue_filter = 0

[player.audio]
preferred_audio_languages = ""
enable_pitch_correction = true
audio_channels = "AutoSafe"
volume_boost_cap = 30
audio_delay = 0

[player.subtitles]
preferred_sub_languages = ""
subtitle_whitelist = ""
subtitle_blacklist = ""
screenshot_subtitles = false
subtitle_font = "Sans Serif"
subtitle_font_size = 55
subtitle_font_scale = 1.0
subtitle_border_size = 3
bold_subtitles = false
italic_subtitles = false
text_color_subtitles = -1
border_color_subtitles = -16777216
border_style_subtitles = "OutlineAndShadow"
shadow_offset_subtitles = 0
background_color_subtitles = 0
subtitle_justification = "Auto"
subtitle_pos = 100
override_subs_ass = false
subtitles_delay = 0
subtitles_speed = 1.0
subtitles_secondary_delay = 0

[player.advanced]
mpv_user_files = false
mpv_conf = ""
mpv_input = ""
player_statistics_page = 0

```

## state.toml

```toml
schema = 1

[base]
downloaded_only = false
incognito_mode = false
shown_onboarding_flow = false

[sources]
trusted_extensions = []
global_search_filter_state = false
last_used_anime_source = -1
last_used_manga_source = -1

[library]
last_updated_timestamp = 0
last_used_anime_category = 0
last_used_manga_category = 0

[backup]
last_auto_backup_timestamp = 0

[storage]
base_storage_directory = ""  # (env) Paths::default_storage_dir()

[security]
last_app_closed = 0

```

Tables absent while at default: `[library.filter_tracked_anime]` and `[library.filter_tracked_manga]` (tracker id to TriState name, source keys `pref_filter_animelib_tracked_<id>_v2` / `pref_filter_library_tracked_<id>_v2`), `[tracking.auth_expired]` in state.toml (tracker id to bool, key `__PRIVATE_pref_tracker_auth_expired_<id>`).

Keyring-only fields: `tracking.track_username (pref_mangasync_username_${tracker.id})`, `tracking.track_password (pref_mangasync_password_${tracker.id})`, `tracking.track_auth_expired (pref_tracker_auth_expired_${tracker.id})`, `tracking.track_token (track_token_${tracker.id})`.
