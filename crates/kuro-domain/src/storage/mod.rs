//! Storage domain (area 09 section 1, T-09a-047 to T-09a-051): the base directory preference, the folder tree under
//! it, and the change notification. Folder names are kept from Aniyomi so a copied install works unchanged (Q-STO-1).
//!
//! The base directory is a local path. Android content URIs and remote locations are refused with
//! `StorageUnavailable`; the directory picker returns local paths only.

mod manager;

pub use manager::{STORAGE_DIR_KEY, StorageManager};

/// Folder of automatic backups, under the base.
pub const AUTOMATIC_BACKUPS_PATH: &str = "autobackup";
/// Folder of downloaded chapters and episodes.
pub const DOWNLOADS_PATH: &str = "downloads";
/// Folder of local manga source folders (`LOCAL_SOURCE_PATH`; the anime one is `localanime`).
pub const LOCAL_SOURCE_PATH: &str = "local";
/// Folder of local anime source folders.
pub const LOCAL_ANIMESOURCE_PATH: &str = "localanime";
/// Folder of the player configuration.
pub const MPV_CONFIG_PATH: &str = "mpv-config";
/// Player fonts, under `mpv-config`.
pub const FONTS_PATH: &str = "fonts";
/// Player scripts, under `mpv-config`.
pub const SCRIPTS_PATH: &str = "scripts";
/// Player script options, under `mpv-config`.
pub const SCRIPT_OPTS_PATH: &str = "script-opts";
/// Player shaders, under `mpv-config`.
pub const SHADERS_PATH: &str = "shaders";

/// The folders `ensure_tree` and `set_base` create under the base, as path segments (TC-STO-09).
pub const TREE: &[&[&str]] = &[
    &[AUTOMATIC_BACKUPS_PATH],
    &[DOWNLOADS_PATH],
    &[LOCAL_SOURCE_PATH],
    &[LOCAL_ANIMESOURCE_PATH],
    &[MPV_CONFIG_PATH],
    &[MPV_CONFIG_PATH, FONTS_PATH],
    &[MPV_CONFIG_PATH, SCRIPTS_PATH],
    &[MPV_CONFIG_PATH, SCRIPT_OPTS_PATH],
    &[MPV_CONFIG_PATH, SHADERS_PATH],
];
