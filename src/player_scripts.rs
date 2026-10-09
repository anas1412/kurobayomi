//! The custom-button scripts at player start (T-09a-041): the rendered Lua of every button is written to the mpv
//! scripts directory, the `load-script` commands for mpv are produced, and a script that does not parse is reported
//! with the button's name while the button stays in the list. Sending the commands needs an mpv handle, which the
//! player screen does not hold yet, so [`start`] hands them to the caller.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_data::custom_button::CustomButtonRepo;
use kuro_db::handle::Db;
use kuro_domain::custom_buttons::service::CustomButtonService;
use kuro_domain::custom_buttons::{CustomButton, load_script_commands, script_problems, write_player_scripts};

/// What a player start produced: the mpv commands that load the scripts, and one user message per problem.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Prepared {
    pub load: Vec<Vec<String>>,
    pub problems: Vec<String>,
}

/// The directory mpv reads the scripts from, under the platform data directory.
pub fn scripts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("mpv").join("scripts")
}

/// Writes the scripts of `buttons` into `dir` and lists the load commands. A button whose Lua fails to parse is named
/// in `problems` and still written and loaded (its own file fails alone, C10). A write error is a problem too.
pub fn prepare(buttons: &[CustomButton], dir: &Path) -> Prepared {
    let mut problems: Vec<String> =
        script_problems(buttons, &dir.to_string_lossy()).into_iter().map(|p| format!("{}: {}", p.button, p.message)).collect();
    let load = match write_player_scripts(buttons, dir) {
        Ok(files) => load_script_commands(&files, dir),
        Err(e) => {
            problems.push(format!("Custom button scripts could not be written: {e}"));
            Vec::new()
        }
    };
    Prepared { load, problems }
}

/// The player start: reads the buttons from the anime database, prepares the scripts on a worker thread, shows each
/// problem as a toast, and calls `loaded` with the commands that load the scripts into mpv.
pub fn start(anime_db: Arc<Db>, data_dir: PathBuf, loaded: impl FnOnce(Vec<Vec<String>>) + Send + 'static) {
    std::thread::spawn(move || {
        let buttons = match CustomButtonService::new(CustomButtonRepo::new(anime_db)).list() {
            Ok(buttons) => buttons,
            Err(e) => {
                tracing::warn!(error = %e, "could not read the custom buttons");
                return;
            }
        };
        let prepared = prepare(&buttons, &scripts_dir(&data_dir));
        for problem in &prepared.problems {
            crate::toast::notify(problem.clone());
        }
        loaded(prepared.load);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(id: i64, name: &str, content: &str) -> CustomButton {
        CustomButton {
            id,
            name: name.into(),
            is_favorite: false,
            sort_index: id,
            content: content.into(),
            long_press_content: String::new(),
            on_startup: String::new(),
        }
    }

    #[test]
    fn every_button_gets_a_file_and_a_load_command() {
        let dir = tempfile::tempdir().unwrap();
        let prepared = prepare(&[button(1, "Skip", "mp.command('seek 85')"), button(2, "Shader", "")], dir.path());
        assert!(prepared.problems.is_empty());
        assert_eq!(prepared.load.len(), 2);
        assert_eq!(prepared.load[0][0], "load-script");
        assert!(dir.path().join("custombutton_1.lua").is_file());
        assert!(dir.path().join("aniyomi.lua").is_file());
    }

    #[test]
    fn a_script_that_does_not_parse_is_named_and_the_button_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let prepared = prepare(&[button(1, "Skip", "mp.command('seek 85')"), button(2, "Broken", "if then")], dir.path());
        assert_eq!(prepared.problems.len(), 1);
        assert!(prepared.problems[0].starts_with("Broken: "), "{:?}", prepared.problems);
        assert_eq!(prepared.load.len(), 2, "the broken button stays in the list");
    }

    #[test]
    fn scripts_of_deleted_buttons_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        prepare(&[button(1, "Skip", ""), button(2, "Old", "")], dir.path());
        prepare(&[button(1, "Skip", "")], dir.path());
        assert!(!dir.path().join("custombutton_2.lua").exists());
    }

    #[test]
    fn the_scripts_live_under_the_data_directory() {
        assert_eq!(scripts_dir(Path::new("/d")), Path::new("/d/mpv/scripts"));
    }
}
