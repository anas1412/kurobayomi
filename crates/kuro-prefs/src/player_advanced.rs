//! Advanced player preferences (T-02a-016): `mpv.conf`, `input.conf`, user script folders and the statistics page.
//! The files are rewritten on every player start, before mpv initialises.

use std::fs;
use std::io;
use std::path::Path;

use crate::registry::define_prefs;

define_prefs! {
    /// `[player.advanced]` section: the four keys of Aniyomi `AdvancedPlayerPreferences`.
    pub struct AdvancedPlayerPrefs in "player.advanced" {
        mpv_user_files: bool = "mpv_scripts" => false;
        mpv_conf: String = "pref_mpv_conf" => String::new();
        mpv_input: String = "pref_mpv_input" => String::new();
        player_statistics_page: i32 = "pref_player_statistics_page" => 0;
    }
}

/// The mpv folders that user files replace on every start.
pub const USER_FOLDERS: [&str; 3] = ["scripts", "script-opts", "shaders"];

/// Rewrites `mpv.conf` and `input.conf` in `config_dir` with the preference text (an empty text gives an empty
/// file). The `scripts`, `script-opts` and `shaders` folders are emptied, then, when `copy_user_files` is set,
/// refilled file by file from the same folders under `user_dir`.
pub fn write_mpv_files(config_dir: &Path, user_dir: &Path, conf: &str, input: &str, copy_user_files: bool) -> io::Result<()> {
    fs::create_dir_all(config_dir)?;
    fs::write(config_dir.join("mpv.conf"), conf)?;
    fs::write(config_dir.join("input.conf"), input)?;
    for folder in USER_FOLDERS {
        let target = config_dir.join(folder);
        match fs::remove_dir_all(&target) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        fs::create_dir_all(&target)?;
        if copy_user_files {
            copy_files(&user_dir.join(folder), &target)?;
        }
    }
    Ok(())
}

/// Copies the regular files of `from` into `to`. A missing source folder copies nothing.
fn copy_files(from: &Path, to: &Path) -> io::Result<()> {
    let entries = match fs::read_dir(from) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// The mpv commands that run after init for a statistics page: nothing for 0, otherwise the toggle and the page.
pub fn statistics_commands(page: i32) -> Vec<String> {
    if page == 0 {
        Vec::new()
    } else {
        vec![
            "script-binding stats/display-stats-toggle".to_owned(),
            format!("script-binding stats/display-page-{page}"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PrefStore;
    use crate::testutil::assert_section_registered;

    #[test]
    fn keys_and_defaults_are_registered() {
        let prefs = AdvancedPlayerPrefs::new(PrefStore::in_memory());
        assert_eq!(AdvancedPlayerPrefs::TABLE, "player.advanced");
        assert_section_registered(AdvancedPlayerPrefs::TABLE, AdvancedPlayerPrefs::FIELDS, &prefs.defaults());
        assert!(!prefs.mpv_user_files.get());
        assert_eq!(prefs.mpv_conf.get(), "");
        assert_eq!(prefs.mpv_input.get(), "");
        assert_eq!(prefs.player_statistics_page.get(), 0);
        assert_eq!(prefs.mpv_user_files.key(), "mpv_scripts");
    }

    #[test]
    fn conf_and_input_are_written_even_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("mpv");
        let user = dir.path().join("mpv-user");
        write_mpv_files(&config, &user, "hwdec=auto\n", "", false).unwrap();
        assert_eq!(fs::read_to_string(config.join("mpv.conf")).unwrap(), "hwdec=auto\n");
        assert_eq!(fs::read_to_string(config.join("input.conf")).unwrap(), "");
    }

    #[test]
    fn user_folders_are_emptied_and_refilled_only_when_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("mpv");
        let user = dir.path().join("mpv-user");
        fs::create_dir_all(user.join("scripts")).unwrap();
        fs::write(user.join("scripts").join("skip.lua"), b"lua").unwrap();
        fs::create_dir_all(config.join("scripts")).unwrap();
        fs::write(config.join("scripts").join("stale.lua"), b"old").unwrap();

        write_mpv_files(&config, &user, "", "", false).unwrap();
        assert!(!config.join("scripts").join("stale.lua").exists(), "stale script is removed");
        assert!(!config.join("scripts").join("skip.lua").exists(), "not copied when disabled");

        write_mpv_files(&config, &user, "", "", true).unwrap();
        assert!(config.join("scripts").join("skip.lua").exists(), "copied when enabled");
    }

    #[test]
    fn statistics_page_zero_does_nothing_and_n_toggles_then_selects_the_page() {
        assert!(statistics_commands(0).is_empty());
        assert_eq!(
            statistics_commands(2),
            ["script-binding stats/display-stats-toggle", "script-binding stats/display-page-2"]
        );
    }
}
