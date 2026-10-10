//! The preference key registry: one row per preference declared by the Kotlin preference classes.
//!
//! Rows follow `plans/02-preferences-system/p1-topic-key-registry.md` in order; a test checks each
//! row. A key containing `{id}` is a family (per tracker or per source); the id replaces `{id}`.
//! `table` is the dotted TOML table and `field` the TOML key inside it. Family rows keep the id as
//! the TOML key, so `field` is empty. Private keys with a string value never reach a file.
//! The two keys shared by a manga and an anime Kotlin function appear twice on purpose.

use std::collections::BTreeSet;

use crate::keys::Namespace;
use crate::values::PrefValue;

/// A registry default, written as a literal so the table is `const`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lit {
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f32),
    Str(&'static str),
    Set(&'static [&'static str]),
}

impl Lit {
    /// The default as a stored value.
    pub fn value(self) -> PrefValue {
        match self {
            Lit::Bool(b) => PrefValue::Bool(b),
            Lit::Int(i) => PrefValue::Int(i),
            Lit::Long(l) => PrefValue::Long(l),
            Lit::Float(f) => PrefValue::Float(f),
            Lit::Str(s) => PrefValue::Str(s.to_owned()),
            Lit::Set(items) => PrefValue::Set(
                items
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect::<BTreeSet<_>>(),
            ),
        }
    }
}

/// The stored kind of a value. Each variant has a [`Lit`] variant of the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int,
    Long,
    Float,
    Str,
    Set,
}

/// One preference key.
#[derive(Debug)]
pub struct PrefMeta {
    pub key: &'static str,
    pub ns: Namespace,
    pub kind: Kind,
    pub default: Lit,
    pub table: &'static str,
    pub field: &'static str,
}

impl PrefMeta {
    /// Whether the key is a family, with `{id}` in place of a tracker or source id.
    pub fn is_family(&self) -> bool {
        self.key.contains("{id}")
    }
}

/// Declares preference keys in one of two forms.
///
/// Registry form: rows `key, namespace, kind, default, table, field;` expand to
/// `pub const REGISTRY: &[PrefMeta]`, in row order. The kind token builds both the kind and the
/// default literal, so the two cannot disagree.
///
/// Section form: one settings struct for one TOML table, with a typed handle per key:
///
/// ```text
/// define_prefs! {
///     /// `[network]`: Aniyomi `NetworkPreferences`.
///     pub struct NetworkPrefs in "network" {
///         verbose_logging: bool = "verbose_logging" => cfg!(debug_assertions);
///         doh_provider: i32 = "doh_provider" => -1;
///     }
/// }
/// ```
///
/// The struct gets a `pub` `Pref<T>` per field, built with the declared default, and these
/// associated items: `TABLE` (the TOML table), `FIELDS` (Rust field name and key pairs),
/// `defaults()` (key and default value per field), `new(store)` and `store()`. Section tests
/// compare `FIELDS` and `defaults()` with `REGISTRY`, so a key cannot drift from its table, its
/// TOML field or its default.
macro_rules! define_prefs {
    ($($key:literal, $ns:ident, $kind:ident, $default:expr, $table:literal, $field:literal;)*) => {
        pub const REGISTRY: &[PrefMeta] = &[
            $(PrefMeta {
                key: $key,
                ns: Namespace::$ns,
                kind: Kind::$kind,
                default: Lit::$kind($default),
                table: $table,
                field: $field,
            },)*
        ];
    };
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident in $table:literal {
            $( $(#[$fmeta:meta])* $field:ident : $ty:ty = $key:literal => $default:expr; )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone)]
        $vis struct $name {
            store: $crate::store::PrefStore,
            $( $(#[$fmeta])* pub $field: $crate::pref::Pref<$ty>, )*
        }

        impl $name {
            /// The TOML table this section is stored in.
            pub const TABLE: &'static str = $table;

            /// The Rust field name and registry key of every field, in declaration order.
            pub const FIELDS: &'static [(&'static str, &'static str)] =
                &[$((stringify!($field), $key)),*];

            /// A handle for every key, each with its declared default.
            pub fn new(store: $crate::store::PrefStore) -> Self {
                Self {
                    $($field: $crate::pref::Pref::new(store.clone(), $key, $default),)*
                    store,
                }
            }

            /// The store the handles read and write.
            pub fn store(&self) -> &$crate::store::PrefStore {
                &self.store
            }

            /// The registry key and default value of every field, in declaration order.
            pub fn defaults(&self) -> Vec<(&'static str, $crate::values::PrefValue)> {
                vec![$(($key, $crate::values::PrefType::encode(&self.$field.default_value())),)*]
            }

            /// Startup repair of every field: invalid stored values go back to their defaults.
            pub fn repair(&self) -> Result<(), $crate::PrefError> {
                $(self.$field.repair()?;)*
                Ok(())
            }
        }
    };
}

pub(crate) use define_prefs;

/// Finds the registry row for a stored key. For a family key the id is returned too.
pub fn lookup(key: &str) -> Option<(&'static PrefMeta, Option<&str>)> {
    REGISTRY
        .iter()
        .find_map(|meta| match meta.key.split_once("{id}") {
            None => (meta.key == key).then_some((meta, None)),
            Some((prefix, suffix)) => {
                let id = key.strip_prefix(prefix)?.strip_suffix(suffix)?;
                (!id.is_empty()).then_some((meta, Some(id)))
            }
        })
}

/// Hidden search terms of a preference: its TOML table and its Rust field name. The settings search matches them
/// too, so "tapping" finds `reader.pager_nav_inverted`. `None` for a key the registry does not know.
pub fn search_terms(key: &str) -> Option<[&'static str; 2]> {
    let (meta, _) = lookup(key)?;
    Some([meta.table, meta.field])
}

/// Finds the registry row for a `table.field` path (`library.display_mode`,
/// `player.gestures.gesture_horizontal_seek`). The field is the TOML name after the last dot.
pub fn find_path(path: &str) -> Option<&'static PrefMeta> {
    let (table, field) = path.rsplit_once('.')?;
    REGISTRY.iter().find(|m| m.table == table && m.field == field)
}

/// Plan `pref_torrserver_tackers` default: 26 announce URLs, spaces removed, newline-joined,
/// no trailing newline (p1-data-and-config.md section 6.1).
pub const TORRSERVER_TRACKERS: &str = concat!(
    "http://nyaa.tracker.wf:7777/announce\n",
    "http://anidex.moe:6969/announce\n",
    "http://tracker.anirena.com:80/announce\n",
    "udp://tracker.uw0.xyz:6969/announce\n",
    "http://share.camoe.cn:8080/announce\n",
    "http://t.nyaatracker.com:80/announce\n",
    "udp://47.ip-51-68-199.eu:6969/announce\n",
    "udp://9.rarbg.me:2940\n",
    "udp://9.rarbg.to:2820\n",
    "udp://exodus.desync.com:6969/announce\n",
    "udp://explodie.org:6969/announce\n",
    "udp://ipv4.tracker.harry.lu:80/announce\n",
    "udp://open.stealth.si:80/announce\n",
    "udp://opentor.org:2710/announce\n",
    "udp://opentracker.i2p.rocks:6969/announce\n",
    "udp://retracker.lanta-net.ru:2710/announce\n",
    "udp://tracker.cyberia.is:6969/announce\n",
    "udp://tracker.dler.org:6969/announce\n",
    "udp://tracker.ds.is:6969/announce\n",
    "udp://tracker.internetwarriors.net:1337\n",
    "udp://tracker.openbittorrent.com:6969/announce\n",
    "udp://tracker.opentrackr.org:1337/announce\n",
    "udp://tracker.tiny-vps.com:6969/announce\n",
    "udp://tracker.torrent.eu.org:451/announce\n",
    "udp://valakas.rollo.dnsabr.com:2710/announce\n",
    "udp://www.torrent.eu.org:451/announce",
);

#[rustfmt::skip]
define_prefs! {
    "__APP_STATE_pref_downloaded_only", AppState, Bool, false, "base", "downloaded_only";
    "__APP_STATE_incognito_mode", AppState, Bool, false, "base", "incognito_mode";
    "__APP_STATE_onboarding_complete", AppState, Bool, false, "base", "shown_onboarding_flow";
    "pref_display_profile_key", Normal, Str, "", "base", "display_profile";
    "pref_always_decode_long_strip_with_ssiv", Normal, Bool, false, "base", "always_decode_long_strip_with_ssiv";
    "pref_theme_mode_key", Normal, Str, "SYSTEM", "ui", "theme_mode";
    "pref_app_theme", Normal, Str, "DEFAULT", "ui", "app_theme";
    "pref_theme_dark_amoled_key", Normal, Bool, false, "ui", "theme_dark_amoled";
    "relative_time_v2", Normal, Bool, true, "ui", "relative_time";
    "app_date_format", Normal, Str, "", "ui", "date_format";
    "tablet_ui_mode", Normal, Str, "AUTOMATIC", "ui", "tablet_ui_mode";
    "start_screen", Normal, Str, "ANIME", "ui", "start_screen";
    "bottom_rail_nav_style", Normal, Str, "MOVE_HISTORY_TO_MORE", "ui", "nav_style";
    "pref_display_mode_catalogue", Normal, Str, "COMPACT_GRID", "sources", "source_display_mode";
    "source_languages", Normal, Set, &["all", "en"], "sources", "enabled_languages";
    "show_nsfw_source", Normal, Bool, true, "sources", "show_nsfw_source";
    "pref_migration_sorting", Normal, Str, "ALPHABETICAL", "sources", "migration_sorting_mode";
    "pref_migration_direction", Normal, Str, "ASCENDING", "sources", "migration_sorting_direction";
    "anime_extension_repos", Normal, Set, &[], "sources", "anime_extension_repos";
    "extension_repos", Normal, Set, &[], "sources", "manga_extension_repos";
    "__APP_STATE_trusted_extensions", AppState, Set, &[], "sources", "trusted_extensions";
    "__APP_STATE_has_filters_toggle_state", AppState, Bool, false, "sources", "global_search_filter_state";
    "hidden_anime_catalogues", Normal, Set, &[], "sources", "disabled_anime_sources";
    "hidden_catalogues", Normal, Set, &[], "sources", "disabled_manga_sources";
    "incognito_anime_extensions", Normal, Set, &[], "sources", "incognito_anime_extensions";
    "incognito_manga_extensions", Normal, Set, &[], "sources", "incognito_manga_extensions";
    "pinned_anime_catalogues", Normal, Set, &[], "sources", "pinned_anime_sources";
    "pinned_catalogues", Normal, Set, &[], "sources", "pinned_manga_sources";
    "__APP_STATE_last_anime_catalogue_source", AppState, Long, -1, "sources", "last_used_anime_source";
    "__APP_STATE_last_catalogue_source", AppState, Long, -1, "sources", "last_used_manga_source";
    "ext_updates_count", Normal, Int, 0, "sources", "manga_extension_updates_count";
    "browse_hide_in_anime_library_items", Normal, Bool, false, "sources", "hide_in_anime_library_items";
    "browse_hide_in_library_items", Normal, Bool, false, "sources", "hide_in_manga_library_items";
    "data_saver", Normal, Str, "NONE", "sources", "data_saver";
    "ignore_jpeg", Normal, Bool, false, "sources", "data_saver_ignore_jpeg";
    "ignore_gif", Normal, Bool, true, "sources", "data_saver_ignore_gif";
    "data_saver_image_quality", Normal, Int, 80, "sources", "data_saver_image_quality";
    "data_saver_image_format_jpeg", Normal, Bool, false, "sources", "data_saver_image_format_jpeg";
    "data_saver_server", Normal, Str, "", "sources", "data_saver_server";
    "data_saver_color_bw", Normal, Bool, false, "sources", "data_saver_color_bw";
    "data_saver_excluded", Normal, Set, &[], "sources", "data_saver_excluded_sources";
    "data_saver_downloader", Normal, Bool, true, "sources", "data_saver_downloader";
    "__PRIVATE_pref_mangasync_username_{id}", Private, Str, "", "secrets", "track_username";
    "__PRIVATE_pref_mangasync_password_{id}", Private, Str, "", "secrets", "track_password";
    "__PRIVATE_pref_tracker_auth_expired_{id}", Private, Bool, false, "tracking.auth_expired", "";
    "__PRIVATE_track_token_{id}", Private, Str, "", "secrets", "track_token";
    "anilist_score_type", Normal, Str, "POINT_10", "tracking", "anilist_score_type";
    "pref_auto_update_manga_sync_key", Normal, Bool, true, "tracking", "auto_update_track";
    "track_on_adding_to_library", Normal, Bool, true, "tracking", "track_on_adding_to_library";
    "show_next_episode_airing_time", Normal, Bool, true, "tracking", "show_next_episode_airing_time";
    "pref_auto_update_manga_on_mark_read", Normal, Str, "ALWAYS", "tracking", "auto_update_track_on_mark_read";
    "pref_display_mode_library", Normal, Str, "COMPACT_GRID", "library", "display_mode";
    "library_sorting_mode", Normal, Str, "ALPHABETICAL,ASCENDING", "library", "manga_sorting_mode";
    "animelib_sorting_mode", Normal, Str, "ALPHABETICAL,ASCENDING", "library", "anime_sorting_mode";
    "__APP_STATE_library_update_last_timestamp", AppState, Long, 0, "library", "last_updated_timestamp";
    "pref_library_update_interval_key", Normal, Int, 0, "library", "auto_update_interval";
    "library_update_restriction", Normal, Set, &["wifi"], "library", "auto_update_device_restrictions";
    "library_update_manga_restriction", Normal, Set, &["manga_fully_read", "manga_ongoing", "manga_started", "manga_outside_release_period"], "library", "auto_update_item_restrictions";
    "auto_update_metadata", Normal, Bool, false, "library", "auto_update_metadata";
    "display_continue_reading_button", Normal, Bool, false, "library", "show_continue_viewing_button";
    "display_category_tabs", Normal, Bool, true, "library", "category_tabs";
    "display_number_of_items", Normal, Bool, false, "library", "category_number_of_items";
    "categorized_display", Normal, Bool, false, "library", "categorized_display_settings";
    "hidden_categories", Normal, Bool, false, "library", "hide_hidden_categories_settings";
    "pref_filter_library_interval_custom", Normal, Str, "DISABLED", "library", "filter_interval_custom";
    "display_download_badge", Normal, Bool, false, "library", "download_badge";
    "display_unread_badge", Normal, Bool, true, "library", "unread_badge";
    "display_local_badge", Normal, Bool, true, "library", "local_badge";
    "display_language_badge", Normal, Bool, false, "library", "language_badge";
    "library_show_updates_count", Normal, Bool, true, "library", "new_show_updates_count";
    "auto_clear_chapter_cache", Normal, Bool, false, "library", "auto_clear_item_cache";
    "library_random_anime_sort_seed", Normal, Int, 0, "library", "random_anime_sort_seed";
    "library_random_manga_sort_seed", Normal, Int, 0, "library", "random_manga_sort_seed";
    "pref_animelib_columns_portrait_key", Normal, Int, 0, "library", "anime_portrait_columns";
    "pref_library_columns_portrait_key", Normal, Int, 0, "library", "manga_portrait_columns";
    "pref_animelib_columns_landscape_key", Normal, Int, 0, "library", "anime_landscape_columns";
    "pref_library_columns_landscape_key", Normal, Int, 0, "library", "manga_landscape_columns";
    "pref_filter_animelib_downloaded_v2", Normal, Str, "DISABLED", "library", "filter_downloaded_anime";
    "pref_filter_library_downloaded_v2", Normal, Str, "DISABLED", "library", "filter_downloaded_manga";
    "pref_filter_animelib_unread_v2", Normal, Str, "DISABLED", "library", "filter_unseen";
    "pref_filter_library_unread_v2", Normal, Str, "DISABLED", "library", "filter_unread";
    "pref_filter_animelib_started_v2", Normal, Str, "DISABLED", "library", "filter_started_anime";
    "pref_filter_library_started_v2", Normal, Str, "DISABLED", "library", "filter_started_manga";
    "pref_filter_animelib_bookmarked_v2", Normal, Str, "DISABLED", "library", "filter_bookmarked_anime";
    "pref_filter_library_bookmarked_v2", Normal, Str, "DISABLED", "library", "filter_bookmarked_manga";
    "pref_filter_animelib_completed_v2", Normal, Str, "DISABLED", "library", "filter_completed_anime";
    "pref_filter_library_completed_v2", Normal, Str, "DISABLED", "library", "filter_completed_manga";
    "pref_filter_animelib_tracked_{id}_v2", Normal, Str, "DISABLED", "library.filter_tracked_anime", "";
    "pref_filter_library_tracked_{id}_v2", Normal, Str, "DISABLED", "library.filter_tracked_manga", "";
    "library_unread_updates_count", Normal, Int, 0, "library", "new_manga_updates_count";
    "library_unseen_updates_count", Normal, Int, 0, "library", "new_anime_updates_count";
    "default_anime_category", Normal, Int, -1, "library", "default_anime_category";
    "default_category", Normal, Int, -1, "library", "default_manga_category";
    "__APP_STATE_last_used_anime_category", AppState, Int, 0, "library", "last_used_anime_category";
    "__APP_STATE_last_used_category", AppState, Int, 0, "library", "last_used_manga_category";
    "animelib_update_categories", Normal, Set, &[], "library", "anime_update_categories";
    "library_update_categories", Normal, Set, &[], "library", "manga_update_categories";
    "animelib_update_categories_exclude", Normal, Set, &[], "library", "anime_update_categories_exclude";
    "library_update_categories_exclude", Normal, Set, &[], "library", "manga_update_categories_exclude";
    "default_episode_filter_by_seen", Normal, Long, 0, "library", "filter_episode_by_seen";
    "default_chapter_filter_by_read", Normal, Long, 0, "library", "filter_chapter_by_read";
    "default_episode_filter_by_downloaded", Normal, Long, 0, "library", "filter_episode_by_downloaded";
    "default_chapter_filter_by_downloaded", Normal, Long, 0, "library", "filter_chapter_by_downloaded";
    "default_episode_filter_by_bookmarked", Normal, Long, 0, "library", "filter_episode_by_bookmarked";
    "default_chapter_filter_by_bookmarked", Normal, Long, 0, "library", "filter_chapter_by_bookmarked";
    "default_episode_filter_by_fillermarked", Normal, Long, 0, "library", "filter_episode_by_fillermarked";
    "default_episode_sort_by_source_or_number", Normal, Long, 0, "library", "sort_episode_by_source_or_number";
    "default_chapter_sort_by_source_or_number", Normal, Long, 0, "library", "sort_chapter_by_source_or_number";
    "default_chapter_display_by_name_or_number", Normal, Long, 0, "library", "display_episode_by_name_or_number";
    "default_chapter_display_by_name_or_number", Normal, Long, 0, "library", "display_chapter_by_name_or_number";
    "default_chapter_sort_by_ascending_or_descending", Normal, Long, 0, "library", "sort_episode_by_ascending_or_descending";
    "default_chapter_sort_by_ascending_or_descending", Normal, Long, 0, "library", "sort_chapter_by_ascending_or_descending";
    "default_episode_show_thumbnail_previews", Normal, Long, 0, "library", "show_episode_thumbnail_previews";
    "default_episode_show_summaries", Normal, Long, 0, "library", "show_episode_summaries";
    "default_season_filter_by_downloaded", Normal, Long, 0, "library", "filter_season_by_download";
    "default_season_filter_by_unseen", Normal, Long, 0, "library", "filter_season_by_unseen";
    "default_season_filter_by_started", Normal, Long, 0, "library", "filter_season_by_started";
    "default_season_filter_by_completed", Normal, Long, 0, "library", "filter_season_by_completed";
    "default_season_filter_by_bookmarked", Normal, Long, 0, "library", "filter_season_by_bookmarked";
    "default_season_filter_by_fillermarked", Normal, Long, 0, "library", "filter_season_by_fillermarked";
    "default_season_sort_by_source_or_number", Normal, Long, 0, "library", "sort_season_by_source_or_number";
    "default_season_sort_by_ascending_or_descending", Normal, Long, 0, "library", "sort_season_by_ascending_or_descending";
    "default_season_grid_display_mode", Normal, Long, 0, "library", "season_display_grid_mode";
    "default_season_grid_display_size", Normal, Int, 0, "library", "season_display_grid_size";
    "default_season_download_overlay", Normal, Bool, false, "library", "season_download_overlay";
    "default_season_unseen_overlay", Normal, Bool, true, "library", "season_unseen_overlay";
    "default_season_local_overlay", Normal, Bool, true, "library", "season_local_overlay";
    "default_season_lang_overlay", Normal, Bool, false, "library", "season_lang_overlay";
    "default_season_continue_overlay", Normal, Bool, true, "library", "season_continue_overlay";
    "default_season_display_mode", Normal, Long, 0, "library", "season_display_mode";
    "pref_update_season_on_refresh", Normal, Bool, false, "library", "update_season_on_refresh";
    "pref_update_season_on_library_update", Normal, Bool, false, "library", "update_season_on_library_update";
    "pref_episode_swipe_end_action", Normal, Str, "ToggleSeen", "library", "swipe_left_episode";
    "pref_episode_swipe_start_action", Normal, Str, "ToggleBookmark", "library", "swipe_right_episode";
    "pref_chapter_swipe_end_action", Normal, Str, "ToggleRead", "library", "swipe_left_chapter";
    "pref_chapter_swipe_start_action", Normal, Str, "ToggleBookmark", "library", "swipe_right_chapter";
    "mark_duplicate_read_chapter_read", Normal, Set, &[], "library", "mark_duplicate_read_chapter_as_read";
    "mark_duplicate_seen_episode_seen", Normal, Set, &[], "library", "mark_duplicate_seen_episode_as_seen";
    "pref_download_only_over_wifi_key", Normal, Bool, true, "downloads", "download_only_over_wifi";
    "use_external_downloader", Normal, Bool, false, "downloads", "use_external_downloader";
    "external_downloader_selection", Normal, Str, "", "downloads", "external_downloader_selection";
    "save_chapter_as_cbz", Normal, Bool, true, "downloads", "save_chapters_as_cbz";
    "split_tall_images", Normal, Bool, true, "downloads", "split_tall_images";
    "auto_download_while_reading", Normal, Int, 0, "downloads", "auto_download_while_reading";
    "auto_download_while_watching", Normal, Int, 0, "downloads", "auto_download_while_watching";
    "remove_after_read_slots", Normal, Int, -1, "downloads", "remove_after_read_slots";
    "pref_remove_after_marked_as_read_key", Normal, Bool, false, "downloads", "remove_after_marked_as_read";
    "pref_remove_bookmarked", Normal, Bool, false, "downloads", "remove_bookmarked_chapters";
    "pref_download_fillermarked", Normal, Bool, false, "downloads", "download_fillermarked_items";
    "remove_exclude_categories", Normal, Set, &[], "downloads", "remove_exclude_categories";
    "remove_exclude_anime_categories", Normal, Set, &[], "downloads", "remove_exclude_anime_categories";
    "download_new", Normal, Bool, false, "downloads", "download_new_chapters";
    "download_new_episode", Normal, Bool, false, "downloads", "download_new_episodes";
    "download_new_categories", Normal, Set, &[], "downloads", "download_new_chapter_categories";
    "download_new_anime_categories", Normal, Set, &[], "downloads", "download_new_episode_categories";
    "download_new_categories_exclude", Normal, Set, &[], "downloads", "download_new_chapter_categories_exclude";
    "download_new_anime_categories_exclude", Normal, Set, &[], "downloads", "download_new_episode_categories_exclude";
    "download_slots", Normal, Int, 1, "downloads", "number_of_downloads";
    "download_speed_limit", Normal, Int, 0, "downloads", "download_speed_limit";
    "download_new_unread_chapters_only", Normal, Bool, false, "downloads", "download_new_unread_chapters_only";
    "download_new_unread_episodes_only", Normal, Bool, false, "downloads", "download_new_unseen_episodes_only";
    "backup_interval", Normal, Int, 12, "backup", "backup_interval";
    "__APP_STATE_last_auto_backup_timestamp", AppState, Long, 0, "backup", "last_auto_backup_timestamp";
    "backup_flags", Normal, Set, &["1", "2", "4", "8"], "backup", "backup_flags";
    "__APP_STATE_storage_dir", AppState, Str, "", "storage", "base_storage_directory";
    "use_biometric_lock", Normal, Bool, false, "security", "use_authenticator";
    "lock_app_after", Normal, Int, 0, "security", "lock_app_after";
    "secure_screen_v2", Normal, Str, "INCOGNITO", "security", "secure_screen";
    "hide_notification_content", Normal, Bool, false, "security", "hide_notification_content";
    "__APP_STATE_last_app_closed", AppState, Long, 0, "security", "last_app_closed";
    "verbose_logging", Normal, Bool, false, "network", "verbose_logging";
    "doh_provider", Normal, Int, -1, "network", "doh_provider";
    "default_user_agent", Normal, Str, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0", "network", "default_user_agent";
    "pref_torrserver_enable", Normal, Bool, false, "torrent", "torr_server_enable";
    "pref_torrserver_shownotice", Normal, Bool, false, "torrent", "torr_server_shown_notice";
    "pref_torrserver_port", Normal, Str, "8090", "torrent", "torr_server_port";
    "pref_torrserver_tackers", Normal, Str, TORRSERVER_TRACKERS, "torrent", "torr_server_trackers";
    "pref_torrserver_proxymode", Normal, Str, "None", "torrent", "torr_server_proxy_mode";
    "pref_torrserver_proxyurl", Normal, Str, "", "torrent", "torr_server_proxy_url";
    "pref_enable_transitions_key", Normal, Bool, true, "reader", "page_transitions";
    "pref_reader_flash", Normal, Bool, false, "reader", "flash_on_page_change";
    "pref_reader_flash_duration", Normal, Int, 100, "reader", "flash_duration_millis";
    "pref_reader_flash_interval", Normal, Int, 1, "reader", "flash_page_interval";
    "pref_reader_flash_mode", Normal, Str, "BLACK", "reader", "flash_color";
    "pref_double_tap_anim_speed", Normal, Int, 500, "reader", "double_tap_anim_speed";
    "pref_show_page_number_key", Normal, Bool, true, "reader", "show_page_number";
    "pref_show_reading_mode", Normal, Bool, true, "reader", "show_reading_mode";
    "fullscreen", Normal, Bool, true, "reader", "fullscreen";
    "cutout_short", Normal, Bool, true, "reader", "cutout_short";
    "pref_keep_screen_on_key", Normal, Bool, true, "reader", "keep_screen_on";
    "pref_default_reading_mode_key", Normal, Int, 2, "reader", "default_reading_mode";
    "pref_default_orientation_type_key", Normal, Int, 8, "reader", "default_orientation_type";
    "pref_enable_double_tap_zoom_webtoon", Normal, Bool, true, "reader", "webtoon_double_tap_zoom_enabled";
    "pref_image_scale_type_key", Normal, Int, 1, "reader", "image_scale_type";
    "pref_zoom_start_key", Normal, Int, 1, "reader", "zoom_start";
    "pref_reader_theme_key", Normal, Int, 1, "reader", "reader_theme";
    "always_show_chapter_transition", Normal, Bool, true, "reader", "always_show_chapter_transition";
    "pref_preserve_reading_position", Normal, Bool, false, "reader", "preserve_reading_position";
    "crop_borders", Normal, Bool, false, "reader", "crop_borders";
    "navigate_pan", Normal, Bool, true, "reader", "navigate_to_pan";
    "landscape_zoom", Normal, Bool, true, "reader", "landscape_zoom";
    "crop_borders_webtoon", Normal, Bool, false, "reader", "crop_borders_webtoon";
    "webtoon_side_padding", Normal, Int, 0, "reader", "webtoon_side_padding";
    "reader_hide_threshold", Normal, Str, "LOW", "reader", "reader_hide_threshold";
    "create_folder_per_manga", Normal, Bool, false, "reader", "folder_per_manga";
    "skip_read", Normal, Bool, false, "reader", "skip_read";
    "skip_filtered", Normal, Bool, true, "reader", "skip_filtered";
    "skip_dupe", Normal, Bool, false, "reader", "skip_dupe";
    "webtoon_disable_zoom_out", Normal, Bool, false, "reader", "webtoon_disable_zoom_out";
    "pref_dual_page_split", Normal, Bool, false, "reader", "dual_page_split_paged";
    "pref_dual_page_invert", Normal, Bool, false, "reader", "dual_page_invert_paged";
    "pref_dual_page_split_webtoon", Normal, Bool, false, "reader", "dual_page_split_webtoon";
    "pref_dual_page_invert_webtoon", Normal, Bool, false, "reader", "dual_page_invert_webtoon";
    "pref_dual_page_rotate", Normal, Bool, false, "reader", "dual_page_rotate_to_fit";
    "pref_dual_page_rotate_invert", Normal, Bool, false, "reader", "dual_page_rotate_to_fit_invert";
    "pref_dual_page_rotate_webtoon", Normal, Bool, false, "reader", "dual_page_rotate_to_fit_webtoon";
    "pref_dual_page_rotate_invert_webtoon", Normal, Bool, false, "reader", "dual_page_rotate_to_fit_invert_webtoon";
    "pref_custom_brightness_key", Normal, Bool, false, "reader", "custom_brightness";
    "custom_brightness_value", Normal, Int, 0, "reader", "custom_brightness_value";
    "pref_color_filter_key", Normal, Bool, false, "reader", "color_filter";
    "color_filter_value", Normal, Int, 0, "reader", "color_filter_value";
    "color_filter_mode", Normal, Int, 0, "reader", "color_filter_mode";
    "pref_grayscale", Normal, Bool, false, "reader", "grayscale";
    "pref_inverted_colors", Normal, Bool, false, "reader", "inverted_colors";
    "reader_long_tap", Normal, Bool, true, "reader", "read_with_long_tap";
    "reader_volume_keys", Normal, Bool, false, "reader", "read_with_volume_keys";
    "reader_volume_keys_inverted", Normal, Bool, false, "reader", "read_with_volume_keys_inverted";
    "reader_navigation_mode_pager", Normal, Int, 0, "reader", "navigation_mode_pager";
    "reader_navigation_mode_webtoon", Normal, Int, 0, "reader", "navigation_mode_webtoon";
    "reader_tapping_inverted", Normal, Str, "NONE", "reader", "pager_nav_inverted";
    "reader_tapping_inverted_webtoon", Normal, Str, "NONE", "reader", "webtoon_nav_inverted";
    "reader_navigation_overlay_new_user", Normal, Bool, true, "reader", "show_navigation_overlay_new_user";
    "reader_navigation_overlay_on_start", Normal, Bool, false, "reader", "show_navigation_overlay_on_start";
    "pref_preserve_watching_position", Normal, Bool, false, "player", "preserve_watching_position";
    "pref_progress_preference", Normal, Float, 0.85f32, "player", "progress_preference";
    "pref_default_player_orientation_type_key", Normal, Str, "SensorLandscape", "player", "default_player_orientation_type";
    "pref_allow_gestures_in_panels", Normal, Bool, false, "player", "allow_gestures";
    "pref_show_loading", Normal, Bool, true, "player", "show_loading_circle";
    "pref_show_current_chapter", Normal, Bool, true, "player", "show_current_chapter";
    "pref_remember_brightness", Normal, Bool, false, "player", "remember_player_brightness";
    "player_brightness_value", Normal, Float, -1.0f32, "player", "player_brightness_value";
    "pref_remember_volume", Normal, Bool, false, "player", "remember_player_volume";
    "player_volume_value", Normal, Float, -1.0f32, "player", "player_volume_value";
    "pref_show_failed_hosters", Normal, Bool, false, "player", "show_failed_hosters";
    "pref_show_empty_hosters", Normal, Bool, false, "player", "show_empty_hosters";
    "player_fullscreen", Normal, Bool, true, "player", "player_fullscreen";
    "player_hide_controls", Normal, Bool, false, "player", "hide_controls";
    "pref_display_vol_as_per", Normal, Bool, true, "player", "display_vol_per";
    "pref_show_system_status_bar", Normal, Bool, false, "player", "show_system_status_bar";
    "pref_reduce_motion", Normal, Bool, false, "player", "reduce_motion";
    "pref_player_time_to_disappear", Normal, Int, 4000, "player", "player_time_to_disappear";
    "pref_panel_opacity", Normal, Int, 60, "player", "panel_opacity";
    "pref_enable_skip_intro", Normal, Bool, true, "player", "enable_skip_intro";
    "pref_enable_auto_skip_ani_skip", Normal, Bool, false, "player", "auto_skip_intro";
    "pref_enable_netflixStyle_aniskip", Normal, Bool, false, "player", "enable_netflix_style_intro_skip";
    "pref_waiting_time_aniskip", Normal, Int, 5, "player", "waiting_time_intro_skip";
    "pref_enable_ani_skip", Normal, Bool, false, "player", "ani_skip_enabled";
    "pref_disabled_ani_skip_chapters", Normal, Bool, true, "player", "disable_ani_skip_on_chapters";
    "pref_enable_pip", Normal, Bool, true, "player", "enable_pip";
    "pref_pip_episode_toasts", Normal, Bool, true, "player", "pip_episode_toasts";
    "pref_pip_on_exit", Normal, Bool, false, "player", "pip_on_exit";
    "pip_replace_with_previous", Normal, Bool, false, "player", "pip_replace_with_previous";
    "pref_always_use_external_player", Normal, Bool, false, "player", "always_use_external_player";
    "external_player_preference", Normal, Str, "", "player", "external_player_preference";
    "pref_player_speed", Normal, Float, 1.0f32, "player", "player_speed";
    "default_speed_presets", Normal, Set, &["0.25", "0.5", "0.75", "1.0", "1.25", "1.5", "1.75", "2.0", "2.5", "3.0", "3.5", "4.0"], "player", "speed_presets";
    "invert_duration", Normal, Bool, false, "player", "invert_duration";
    "pref_player_aspect_state", Normal, Str, "Fit", "player", "aspect_state";
    "pref_auto_play_enabled", Normal, Bool, false, "player", "autoplay_enabled";
    "pref_gesture_volume_brightness", Normal, Bool, true, "player.gestures", "gesture_volume_brightness";
    "pref_swap_volume_and_brightness", Normal, Bool, false, "player.gestures", "swap_volume_brightness";
    "pref_gesture_horizontal_seek", Normal, Bool, true, "player.gestures", "gesture_horizontal_seek";
    "pref_show_seekbar", Normal, Bool, false, "player.gestures", "show_seek_bar";
    "pref_default_intro_length", Normal, Int, 85, "player.gestures", "default_intro_length";
    "pref_skip_length_preference", Normal, Int, 10, "player.gestures", "skip_length_preference";
    "pref_player_smooth_seek", Normal, Bool, false, "player.gestures", "player_smooth_seek";
    "pref_left_double_tap", Normal, Str, "Seek", "player.gestures", "left_double_tap_gesture";
    "pref_center_double_tap", Normal, Str, "PlayPause", "player.gestures", "center_double_tap_gesture";
    "pref_right_double_tap", Normal, Str, "Seek", "player.gestures", "right_double_tap_gesture";
    "pref_media_previous", Normal, Str, "Switch", "player.gestures", "media_previous_gesture";
    "pref_media_playpause", Normal, Str, "PlayPause", "player.gestures", "media_play_pause_gesture";
    "pref_media_next", Normal, Str, "Switch", "player.gestures", "media_next_gesture";
    "pref_try_hwdec", Normal, Bool, true, "player.decoder", "try_hw_decoding";
    "pref_gpu_next", Normal, Bool, false, "player.decoder", "gpu_next";
    "pref_video_debanding", Normal, Str, "None", "player.decoder", "video_debanding";
    "use_yuv420p", Normal, Bool, true, "player.decoder", "use_yuv420p";
    "pref_player_filter_brightness", Normal, Int, 0, "player.decoder", "brightness_filter";
    "pref_player_filter_saturation", Normal, Int, 0, "player.decoder", "saturation_filter";
    "pref_player_filter_contrast", Normal, Int, 0, "player.decoder", "contrast_filter";
    "pref_player_filter_gamma", Normal, Int, 0, "player.decoder", "gamma_filter";
    "pref_player_filter_hue", Normal, Int, 0, "player.decoder", "hue_filter";
    "pref_audio_lang", Normal, Str, "", "player.audio", "preferred_audio_languages";
    "pref_audio_pitch_correction", Normal, Bool, true, "player.audio", "enable_pitch_correction";
    "pref_audio_config", Normal, Str, "AutoSafe", "player.audio", "audio_channels";
    "pref_audio_volume_boost_cap", Normal, Int, 30, "player.audio", "volume_boost_cap";
    "pref_audio_delay", Normal, Int, 0, "player.audio", "audio_delay";
    "pref_subtitle_lang", Normal, Str, "", "player.subtitles", "preferred_sub_languages";
    "pref_subtitle_whitelist", Normal, Str, "", "player.subtitles", "subtitle_whitelist";
    "pref_subtitle_blacklist", Normal, Str, "", "player.subtitles", "subtitle_blacklist";
    "pref_screenshot_subtitles", Normal, Bool, false, "player.subtitles", "screenshot_subtitles";
    "pref_subtitle_font", Normal, Str, "Sans Serif", "player.subtitles", "subtitle_font";
    "pref_subtitles_font_size", Normal, Int, 55, "player.subtitles", "subtitle_font_size";
    "pref_sub_scale", Normal, Float, 1.0f32, "player.subtitles", "subtitle_font_scale";
    "pref_sub_border_size", Normal, Int, 3, "player.subtitles", "subtitle_border_size";
    "pref_bold_subtitles", Normal, Bool, false, "player.subtitles", "bold_subtitles";
    "pref_italic_subtitles", Normal, Bool, false, "player.subtitles", "italic_subtitles";
    "pref_text_color_subtitles", Normal, Int, -1, "player.subtitles", "text_color_subtitles";
    "pref_border_color_subtitles", Normal, Int, -16777216, "player.subtitles", "border_color_subtitles";
    "pref_border_style_subtitles", Normal, Str, "OutlineAndShadow", "player.subtitles", "border_style_subtitles";
    "sub_shadow_offset", Normal, Int, 0, "player.subtitles", "shadow_offset_subtitles";
    "pref_background_color_subtitles", Normal, Int, 0, "player.subtitles", "background_color_subtitles";
    "pref_sub_justify", Normal, Str, "Auto", "player.subtitles", "subtitle_justification";
    "pref_sub_pos", Normal, Int, 100, "player.subtitles", "subtitle_pos";
    "pref_override_subtitles_ass", Normal, Bool, false, "player.subtitles", "override_subs_ass";
    "pref_subtitles_delay", Normal, Int, 0, "player.subtitles", "subtitles_delay";
    "pref_subtitles_speed", Normal, Float, 1.0f32, "player.subtitles", "subtitles_speed";
    "pref_subtitles_secondary_delay", Normal, Int, 0, "player.subtitles", "subtitles_secondary_delay";
    "mpv_scripts", Normal, Bool, false, "player.advanced", "mpv_user_files";
    "pref_mpv_conf", Normal, Str, "", "player.advanced", "mpv_conf";
    "pref_mpv_input", Normal, Str, "", "player.advanced", "mpv_input";
    "pref_player_statistics_page", Normal, Int, 0, "player.advanced", "player_statistics_page";
    "show_not_runnable", Normal, Bool, false, "sources", "show_not_runnable";
    "anime_ext_last_check", Normal, Long, 0, "sources", "anime_ext_last_check";
    "check_app_updates", Normal, Bool, false, "base", "check_app_updates";
    "__APP_STATE_last_app_check", AppState, Long, 0, "base", "last_app_check";
    "muted_notification_channels", Normal, Set, &[], "base", "muted_notification_channels";
    "pref_remember_decoder", Normal, Bool, false, "player.decoder", "remember_decoder";
    "player_decoder", Normal, Str, "", "player.decoder", "remembered_decoder";
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKDOWN: &str =
        include_str!("../../../plans/02-preferences-system/p1-topic-key-registry.md");

    /// Keys that two Kotlin functions share on purpose (behaviour section 4). Both rows are kept.
    const SHARED_KEYS: [&str; 2] = [
        "default_chapter_display_by_name_or_number",
        "default_chapter_sort_by_ascending_or_descending",
    ];

    /// Plan rows that are not ported: the Android installer choice (T-19a-037), the four commented-out SY keys
    /// (T-17a-039) and the hardware bitmap threshold (T-13a-030). The registry must not hold them.
    const DROPPED_KEYS: [&str; 6] = [
        "extension_installer",
        "pref_hardware_bitmap_threshold",
        "eh_enable_source_blacklist",
        "sources_tab_categories",
        "sources_tab_categories_filter",
        "sources_tab_source_categories",
    ];

    /// Rows of the plan's registry tables as (key, store, kind, default).
    fn plan_rows() -> Vec<(String, String, String, String)> {
        MARKDOWN
            .lines()
            .filter_map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                if cells.len() < 8 || cells[1].parse::<u32>().is_err() {
                    return None;
                }
                let key = cells[3]
                    .trim_matches('`')
                    .replace("${tracker.id}", "{id}")
                    .replace("${id}", "{id}");
                // T-19a-064: the anime update badge count is derived in memory, never registered
                (key != "animeext_updates_count" && !DROPPED_KEYS.contains(&key.as_str())).then(|| (
                    key,
                    cells[4].to_owned(),
                    cells[5].to_owned(),
                    cells[6].to_owned(),
                ))
            })
            .collect()
    }

    fn kind_matches(kind: Kind, plan_kind: &str) -> bool {
        match kind {
            Kind::Bool => plan_kind == "Boolean",
            Kind::Int => plan_kind == "Int",
            Kind::Long => plan_kind == "Long",
            Kind::Float => plan_kind == "Float",
            Kind::Set => plan_kind == "Set<String>",
            Kind::Str => {
                plan_kind == "String"
                    || plan_kind.starts_with("Enum ")
                    || plan_kind.starts_with("Object ")
            }
        }
    }

    /// `Some(matches)` when the plan's default is a literal; `None` for prose (checked by hand).
    fn default_matches(lit: Lit, plan: &str) -> Option<bool> {
        if let Some(quoted) = plan.strip_prefix('"') {
            let Lit::Str(s) = lit else { return Some(false) };
            return Some(quoted.strip_suffix('"')? == s);
        }
        if plan.starts_with('{') {
            let Lit::Set(items) = lit else {
                return Some(false);
            };
            if plan.contains('<') {
                return None;
            }
            let body = plan.split('}').next()?.trim_start_matches('{');
            let mut plan_items: Vec<&str> = body
                .split(',')
                .map(|s| s.trim().trim_matches('"'))
                .filter(|s| !s.is_empty())
                .collect();
            let mut want: Vec<&str> = items.to_vec();
            plan_items.sort_unstable();
            want.sort_unstable();
            return Some(plan_items == want);
        }
        let first = plan.split_whitespace().next()?;
        let rest = &plan[first.len()..];
        match lit {
            Lit::Bool(b) => Some(first == b.to_string()),
            Lit::Int(i) => Some(first.parse::<i64>().ok() == Some(i64::from(i))),
            Lit::Long(l) => Some(first.trim_end_matches('L').parse::<i64>().ok() == Some(l)),
            Lit::Float(f) => Some((first.parse::<f64>().ok()? - f64::from(f)).abs() < 1e-6),
            Lit::Str(s) => (rest.is_empty() || rest.starts_with(" (")).then(|| first == s),
            Lit::Set(_) => None,
        }
    }

    /// Defaults the plan gives as prose, pinned to the value the registry must hold on Linux.
    const PROSE_DEFAULTS: [(&str, Lit); 5] = [
        ("pref_app_theme", Lit::Str("DEFAULT")),
        ("source_languages", Lit::Set(&["all", "en"])),
        ("library_update_restriction", Lit::Set(&["wifi"])),
        ("__APP_STATE_storage_dir", Lit::Str("")),
        ("pref_torrserver_tackers", Lit::Str(TORRSERVER_TRACKERS)),
    ];

    #[test]
    fn registry_has_the_321_plan_rows_in_order() {
        let rows = plan_rows();
        assert_eq!(rows.len(), 321, "the plan's 328 rows minus the derived animeext_updates_count and the six dropped keys");
        // five Kurobayomi rows after the plan's rows: `show_not_runnable` (T-19a-066), `anime_ext_last_check` (T-19a-064),
        // `check_app_updates` and `last_app_check` (T-07a-036), and `muted_notification_channels` (T-req-platform-004)
        assert_eq!(REGISTRY.len(), rows.len() + 7);
        assert_eq!(REGISTRY[rows.len()].key, "show_not_runnable");
        assert_eq!(REGISTRY[rows.len() + 1].key, "anime_ext_last_check");
        assert_eq!(REGISTRY[rows.len() + 2].key, "check_app_updates");
        assert_eq!(REGISTRY[rows.len() + 3].key, "__APP_STATE_last_app_check");
        assert_eq!(REGISTRY[rows.len() + 4].key, "muted_notification_channels");
        // T-46b-043: the opt-in and the remembered decoder
        assert_eq!(REGISTRY[rows.len() + 5].key, "pref_remember_decoder");
        assert_eq!(REGISTRY[rows.len() + 6].key, "player_decoder");
        let mut pinned = 0;
        for (meta, (key, store, kind, default)) in REGISTRY.iter().zip(&rows) {
            assert_eq!(meta.key, key);
            let ns = match store.as_str() {
                "normal" => Namespace::Normal,
                "app-state" => Namespace::AppState,
                "private" => Namespace::Private,
                other => panic!("unknown store {other} for {key}"),
            };
            assert_eq!(meta.ns, ns, "{key}");
            assert_eq!(Namespace::of(meta.key), ns, "{key}");
            assert!(kind_matches(meta.kind, kind), "{key}: kind {kind}");
            match default_matches(meta.default, default) {
                Some(matches) => assert!(matches, "{key}: default {default} vs {:?}", meta.default),
                None => {
                    let (_, want) = PROSE_DEFAULTS
                        .iter()
                        .find(|(k, _)| *k == meta.key)
                        .unwrap_or_else(|| panic!("{key}: prose default is not pinned"));
                    assert_eq!(meta.default, *want, "{key}");
                    pinned += 1;
                }
            }
        }
        assert_eq!(pinned, PROSE_DEFAULTS.len());
    }

    #[test]
    fn dropped_keys_are_not_registered() {
        for key in DROPPED_KEYS {
            assert!(REGISTRY.iter().all(|meta| meta.key != key), "{key} is dropped");
        }
    }

    #[test]
    fn search_terms_name_the_table_and_the_field_of_a_key() {
        assert_eq!(search_terms("reader_tapping_inverted"), Some(["reader", "pager_nav_inverted"]));
        assert_eq!(search_terms("no_such_key"), None);
    }

    #[test]
    fn torrserver_trackers_are_the_plan_list_joined_by_newlines() {
        const PLAN: &str = include_str!("../../../plans/02-preferences-system/p1-data-and-config.md");
        let block = PLAN.split("### 6.1").nth(1).unwrap().split("```").nth(1).unwrap();
        let urls: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        assert_eq!(urls.len(), 26);
        assert_eq!(urls.join("\n"), TORRSERVER_TRACKERS);
    }

    #[test]
    fn keys_are_unique_except_the_shared_pairs() {
        let mut seen = std::collections::HashMap::new();
        for meta in REGISTRY {
            *seen.entry(meta.key).or_insert(0) += 1;
        }
        for (key, count) in seen {
            let expected = if SHARED_KEYS.contains(&key) { 2 } else { 1 };
            assert_eq!(count, expected, "{key}");
        }
    }

    #[test]
    fn lookup_finds_exact_and_family_keys() {
        let (meta, id) = lookup("display_unread_badge").unwrap();
        assert_eq!(meta.key, "display_unread_badge");
        assert_eq!(id, None);
        let (meta, id) = lookup("pref_filter_library_tracked_7_v2").unwrap();
        assert_eq!(meta.key, "pref_filter_library_tracked_{id}_v2");
        assert_eq!(id, Some("7"));
        let (meta, id) = lookup("__PRIVATE_pref_tracker_auth_expired_3").unwrap();
        assert_eq!(meta.table, "tracking.auth_expired");
        assert_eq!(id, Some("3"));
    }

    #[test]
    fn lookup_rejects_unknown_and_empty_family_ids() {
        assert!(lookup("no_such_key").is_none());
        assert!(lookup("pref_filter_library_tracked__v2").is_none());
        assert!(lookup("pref_filter_library_tracked_1_v2_extra").is_none());
    }

    #[test]
    fn literal_helpers_cover_each_kind() {
        assert_eq!(Lit::Long(5).value(), PrefValue::Long(5));
        assert_eq!(
            Lit::Set(&["wifi"]).value(),
            PrefValue::Set(BTreeSet::from(["wifi".to_owned()]))
        );
        assert_eq!(
            default_matches(Lit::Str("DISABLED"), "DISABLED"),
            Some(true)
        );
        assert_eq!(
            default_matches(
                Lit::Str("PACKAGEINSTALLER"),
                "PACKAGEINSTALLER (LEGACY on MIUI)"
            ),
            Some(true)
        );
        assert_eq!(
            default_matches(
                Lit::Str("x"),
                "MONET if dynamic colour available else DEFAULT"
            ),
            None
        );
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn a_table_dot_field_path_finds_its_row() {
        assert_eq!(
            find_path("library.display_mode").map(|m| m.key),
            Some("pref_display_mode_library")
        );
        assert_eq!(
            find_path("player.gestures.gesture_horizontal_seek").map(|m| m.key),
            Some("pref_gesture_horizontal_seek")
        );
        assert!(find_path("library.no_such_field").is_none());
        assert!(find_path("no_dot").is_none());
    }
}
