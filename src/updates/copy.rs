//! English copy of area 42 that has no key in the `kuro-i18n` table yet (T-42a-062 adds them). Each constant names the
//! key it will move to; keys that already exist in `locales/en` are read through `tr` by the widgets instead.

/// Statistics section titles and captions (p1-ui-parity 4.2). Keys: `label_overview_section` and friends exist for
/// some of these; the rest are additions.
pub const STATS_OVERVIEW: &str = "Overview";
pub const STATS_ENTRIES: &str = "Entries";
pub const STATS_CHAPTERS: &str = "Chapters";
pub const STATS_EPISODES: &str = "Episodes";
pub const STATS_TRACKERS: &str = "Trackers";
pub const STATS_IN_LIBRARY: &str = "In library";
pub const STATS_READ_DURATION: &str = "Read duration";
pub const STATS_WATCHED_DURATION: &str = "Watched duration";
pub const STATS_COMPLETED: &str = "Completed entries";
pub const STATS_IN_GLOBAL_UPDATE: &str = "In global update";
pub const STATS_STARTED: &str = "Started";
pub const STATS_LOCAL: &str = "Local";
pub const STATS_TOTAL: &str = "Total";
pub const STATS_READ: &str = "Read";
pub const STATS_WATCHED: &str = "Watched";
pub const STATS_DOWNLOADED: &str = "Downloaded";
pub const STATS_TRACKED: &str = "Tracked entries";
pub const STATS_MEAN_SCORE: &str = "Mean score";
pub const STATS_USED: &str = "Used";
pub const NONE: &str = "None";
pub const NOT_AVAILABLE: &str = "N/A";

/// Storage copy (p1-ui-parity 5 and EC-42-T).
pub const STORAGE_CATEGORY: &str = "Category";
pub const STORAGE_ALL: &str = "All";
pub const STORAGE_DEFAULT: &str = "Default";
pub const STORAGE_SCANNING: &str = "Scanning downloads…";
pub const STORAGE_EMPTY_CATEGORY: &str = "No downloads in this category";
pub const STORAGE_SOURCE_MISSING: &str = "Source not installed; files not deleted";
pub const DELETE_TITLE_CHAPTERS: &str = "Delete downloaded chapters?";
pub const DELETE_TITLE_EPISODES: &str = "Delete downloaded episodes?";

/// Updates copy (T-42a-032, T-42a-038, T-42b-003, T-42b-016).
pub const SHOWING_LATEST_500: &str = "Showing latest 500";
pub const LIBRARY_UPDATE_RUNNING: &str = "Library update already running";
pub const CONFIRM_DELETE_CHAPTERS: &str = "Are you sure you want to delete the selected chapters?";
pub const CONFIRM_DELETE_EPISODES: &str = "Are you sure you want to delete the selected episodes?";
pub const INTERNAL_ERROR: &str = "InternalError: Check crash logs for further information";
pub const LAST_UPDATED_PREFIX: &str = "Library last updated: ";
pub const JUST_NOW: &str = "Just now";
pub const CONFIRM_OK: &str = "OK";
