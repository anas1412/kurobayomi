//! Player custom buttons (`p1-topic-custom-buttons.md` sections 3 and 4): the Lua text of each button, the placeholder
//! expansion, and the per-button script files that libmpv loads. The table and its repository live in `kuro-data`.

pub mod service;

pub use kuro_data::custom_button::CustomButtonUpdate;

use std::io;
use std::path::Path;

/// The `aniyomi` Lua module (`app/src/main/assets/aniyomi.lua`, ported unchanged). Button scripts `require 'aniyomi'`, so
/// [`write_player_scripts`] writes it next to them as [`MODULE_FILE`], a directory their preamble puts on `package.path`.
pub const ANIYOMI_LUA: &str = include_str!("aniyomi.lua");

/// File name of the `aniyomi` module inside the scripts directory.
pub const MODULE_FILE: &str = "aniyomi.lua";

/// File-name prefix of every generated button script. Files with this prefix that no longer belong to a button are
/// retired when the scripts are rebuilt (C11).
pub const SCRIPT_PREFIX: &str = "custombutton_";

/// One user button as the player reads it (`CustomButton.kt`).
#[derive(Clone, Debug, PartialEq)]
pub struct CustomButton {
    pub id: i64,
    pub name: String,
    pub is_favorite: bool,
    pub sort_index: i64,
    pub content: String,
    pub long_press_content: String,
    pub on_startup: String,
}

/// One generated Lua file: its name inside the scripts directory and its text.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptFile {
    pub name: String,
    pub text: String,
}

/// The id of the favourite button, which the `$isPrimary` placeholder refers to. `0` when no button is a favourite,
/// so no button matches (`primaryButton.value?.id ?: 0L`).
pub fn primary_id(buttons: &[CustomButton]) -> i64 {
    buttons.iter().find(|b| b.is_favorite).map_or(0, |b| b.id)
}

/// The button the player draws as primary when none is a favourite (E-CB-05): the one with the lowest `sortIndex`,
/// ties by id. `$isPrimary` still follows [`primary_id`], so a script never sees a fallback as primary. `None` when
/// the list is empty.
pub fn fallback_primary(buttons: &[CustomButton]) -> Option<i64> {
    buttons.iter().min_by_key(|b| (b.sort_index, b.id)).map(|b| b.id)
}

/// Replaces `$id` first, then `$isPrimary` (C7, C8). The order is kept on purpose: user scripts may rely on `$id`
/// being expanded inside identifiers such as `button_$id`.
pub fn expand_placeholders(text: &str, id: i64, primary_id: i64) -> String {
    let is_primary = if primary_id == id { "true" } else { "false" };
    text.replace("$id", &id.to_string()).replace("$isPrimary", is_primary)
}

/// The message a button press sends through libmpv: `call_button_<id>`, or `call_button_<id>_long` for a long press.
pub fn message_name(id: i64, long_press: bool) -> String {
    if long_press { format!("call_button_{id}_long") } else { format!("call_button_{id}") }
}

/// The name of the script file of one button.
pub fn script_name(id: i64) -> String {
    format!("{SCRIPT_PREFIX}{id}.lua")
}

/// The Lua preamble every button file starts with: the package path extension and `require 'aniyomi'`.
pub fn preamble(scripts_dir: &str) -> String {
    format!(
        "local lua_modules = mp.find_config_file('scripts')\n\
         if lua_modules then package.path = package.path .. ';' .. lua_modules .. '/?.lua;' .. lua_modules .. '/?/init.lua;' .. '{scripts_dir}' .. '/?.lua' end\n\
         local aniyomi = require 'aniyomi'"
    )
}

/// One file per button (C10: a syntax error in one button leaves the others working). Each file holds the preamble,
/// the expanded `onStartup` text, the press function, the long-press function and both message registrations. The
/// blocks are joined with newlines. Buttons are written in the order given, which is `sortIndex` order.
pub fn build_player_scripts(buttons: &[CustomButton], scripts_dir: &str) -> Vec<ScriptFile> {
    let primary = primary_id(buttons);
    buttons
        .iter()
        .map(|b| {
            let ex = |text: &str| expand_placeholders(text, b.id, primary);
            let id = b.id;
            let blocks = [
                preamble(scripts_dir),
                ex(&b.on_startup),
                format!("function button{id}()"),
                ex(&b.content),
                "end".to_owned(),
                format!("mp.register_script_message('call_button_{id}', button{id})"),
                format!("function button{id}long()"),
                ex(&b.long_press_content),
                "end".to_owned(),
                format!("mp.register_script_message('call_button_{id}_long', button{id}long)"),
            ];
            ScriptFile { name: script_name(id), text: blocks.join("\n") }
        })
        .collect()
}

/// The script files left over from an earlier build: names with the button prefix that `current` no longer
/// contains. With no buttons at all, every old file is retired (C11).
pub fn retired_scripts(existing: &[String], current: &[ScriptFile]) -> Vec<String> {
    existing
        .iter()
        .filter(|name| name.starts_with(SCRIPT_PREFIX) && !current.iter().any(|f| &f.name == *name))
        .cloned()
        .collect()
}

/// The libmpv commands that load every script of `files` from `scripts_dir` (`load-script <path>`), in order.
pub fn load_script_commands(files: &[ScriptFile], scripts_dir: &Path) -> Vec<Vec<String>> {
    files
        .iter()
        .map(|f| vec!["load-script".to_owned(), scripts_dir.join(&f.name).to_string_lossy().into_owned()])
        .collect()
}

/// The libmpv command of a button press: `script-message call_button_<id>`, or the `_long` form (T-06a-062).
pub fn press_command(id: i64, long_press: bool) -> Vec<String> {
    vec!["script-message".to_owned(), message_name(id, long_press)]
}

/// A button whose generated Lua does not parse: its name and the parser's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptProblem {
    pub button: String,
    pub message: String,
}

/// Parses the script of each button without running it. A failing button is reported by name; it stays in the list
/// and in its own file (C10), so the caller shows the message and keeps the button.
pub fn script_problems(buttons: &[CustomButton], scripts_dir: &str) -> Vec<ScriptProblem> {
    let lua = mlua::Lua::new();
    buttons
        .iter()
        .zip(build_player_scripts(buttons, scripts_dir))
        .filter_map(|(b, file)| {
            let err = lua.load(&file.text).set_name(&file.name).into_function().err()?;
            Some(ScriptProblem { button: b.name.clone(), message: err.to_string() })
        })
        .collect()
}

/// Writes the module and one file per button into `scripts_dir` (created when missing), removes the button files that no
/// longer belong to a button (C11, also when there are none), and returns the files written, for [`load_script_commands`].
/// One unwritable button file does not stop the others (C10); the first such error is returned after the rest are done.
pub fn write_player_scripts(buttons: &[CustomButton], scripts_dir: &Path) -> io::Result<Vec<ScriptFile>> {
    std::fs::create_dir_all(scripts_dir)?;
    std::fs::write(scripts_dir.join(MODULE_FILE), ANIYOMI_LUA)?;
    let files = build_player_scripts(buttons, &scripts_dir.to_string_lossy());
    let mut first_error = None;
    for file in &files {
        if let Err(e) = std::fs::write(scripts_dir.join(&file.name), &file.text) {
            first_error.get_or_insert(e);
        }
    }
    let existing: Vec<String> = std::fs::read_dir(scripts_dir)?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    for old in retired_scripts(&existing, &files) {
        if let Err(e) = std::fs::remove_file(scripts_dir.join(old)) {
            first_error.get_or_insert(e);
        }
    }
    first_error.map_or(Ok(files), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(id: i64, favorite: bool, content: &str) -> CustomButton {
        CustomButton {
            id,
            name: format!("b{id}"),
            is_favorite: favorite,
            sort_index: id,
            content: content.to_owned(),
            long_press_content: format!("-- long {id}"),
            on_startup: String::new(),
        }
    }

    #[test]
    fn c7_id_is_replaced_before_is_primary_so_identifiers_are_mangled_as_documented() {
        assert_eq!(expand_placeholders("$identity $isPrimary", 7, 7), "7entity true");
    }

    #[test]
    fn c8_id_inside_text_is_replaced_and_is_primary_is_false_for_other_buttons() {
        assert_eq!(expand_placeholders("x$idy", 12, 99), "x12y");
        assert_eq!(expand_placeholders("$isPrimary", 1, 2), "false");
    }

    #[test]
    fn primary_is_the_first_favourite_or_zero_when_none_is_favourite() {
        let buttons = [button(4, false, ""), button(5, true, ""), button(6, true, "")];
        assert_eq!(primary_id(&buttons), 5);
        assert_eq!(primary_id(&[button(4, false, "")]), 0);
    }

    #[test]
    fn a_built_file_has_the_preamble_then_the_blocks_in_order() {
        let files = build_player_scripts(&[button(3, true, "mp.pause()")], "/cfg/scripts");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "custombutton_3.lua");
        let text = &files[0].text;
        assert!(text.starts_with("local lua_modules = mp.find_config_file('scripts')"));
        assert!(text.contains("'/cfg/scripts' .. '/?.lua' end\nlocal aniyomi = require 'aniyomi'\n"));
        let tail = [
            "function button3()",
            "mp.pause()",
            "end",
            "mp.register_script_message('call_button_3', button3)",
            "function button3long()",
            "-- long 3",
            "end",
            "mp.register_script_message('call_button_3_long', button3long)",
        ]
        .join("\n");
        assert!(text.ends_with(&tail), "{text}");
    }

    /// C10: one broken button stays in its own file, so the other files are unaffected.
    #[test]
    fn a_broken_button_stays_in_its_own_file() {
        let files = build_player_scripts(&[button(1, false, "end end ("), button(2, false, "mp.pause()")], "/s");
        assert_eq!(files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["custombutton_1.lua", "custombutton_2.lua"]);
        assert!(files[0].text.contains("end end ("));
        assert!(!files[1].text.contains("end end ("));
        assert!(files[1].text.contains("mp.pause()"));
    }

    /// C11: with no buttons nothing is written, and every old button file is retired while other files are kept.
    #[test]
    fn zero_buttons_write_nothing_and_retire_every_old_button_file() {
        let files = build_player_scripts(&[], "/s");
        assert!(files.is_empty());
        let existing = vec!["custombutton_1.lua".to_owned(), "custombutton_9.lua".to_owned(), "aniyomi.lua".to_owned()];
        assert_eq!(retired_scripts(&existing, &files), ["custombutton_1.lua", "custombutton_9.lua"]);
        let kept = build_player_scripts(&[button(9, false, "")], "/s");
        assert_eq!(retired_scripts(&existing, &kept), ["custombutton_1.lua"]);
    }

    #[test]
    fn the_fallback_primary_is_the_lowest_index_and_never_changes_is_primary() {
        let mut buttons = [button(4, false, "x"), button(5, false, "y")];
        buttons[0].sort_index = 3;
        buttons[1].sort_index = 1;
        assert_eq!(fallback_primary(&buttons), Some(5));
        assert_eq!(primary_id(&buttons), 0, "no favourite: $isPrimary is false everywhere");
        assert_eq!(fallback_primary(&[]), None);
    }

    /// TC-CB-03: a snippet that fails to parse is reported with the button name; valid buttons are not.
    #[test]
    fn a_snippet_that_does_not_parse_is_reported_by_button_name() {
        let buttons = [button(1, false, "mp.pause()"), button(2, false, "end end (")];
        let problems = script_problems(&buttons, "/s");
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].button, "b2");
        assert!(!problems[0].message.is_empty());
        assert!(script_problems(&buttons[..1], "/s").is_empty());
    }

    #[test]
    fn press_messages_match_the_script_registrations() {
        assert_eq!(message_name(4, false), "call_button_4");
        assert_eq!(message_name(4, true), "call_button_4_long");
    }

    /// Every function of the topic's section 4 is in the shipped module.
    #[test]
    fn the_shipped_module_implements_every_function_of_the_topic() {
        let names = [
            "show_text", "hide_ui", "show_ui", "toggle_ui", "show_subtitle_settings", "show_subtitle_delay",
            "show_audio_delay", "show_video_filters", "show_software_keyboard", "hide_software_keyboard",
            "toggle_software_keyboard", "set_button_title", "reset_button_title", "hide_button", "show_button",
            "toggle_button", "previous_episode", "next_episode", "pause", "unpause", "pauseunpause", "seek_by",
            "seek_to", "seek_by_with_text", "seek_to_with_text", "int_picker", "left_seek_by", "right_seek_by",
        ];
        for name in names {
            assert!(ANIYOMI_LUA.contains(&format!("function aniyomi.{name}(")), "missing aniyomi.{name}");
        }
        assert!(ANIYOMI_LUA.trim_end().ends_with("return aniyomi"));
    }

    #[test]
    fn the_writer_writes_the_module_and_one_file_per_button_and_removes_retired_ones() {
        let dir = tempfile::tempdir().expect("temp dir");
        let scripts = dir.path().join("mpv/scripts");
        std::fs::create_dir_all(&scripts).expect("dir");
        std::fs::write(scripts.join("custombutton_99.lua"), "old").expect("old");
        std::fs::write(scripts.join("user.lua"), "mine").expect("user");
        let files = write_player_scripts(&[button(1, true, "mp.pause()"), button(2, false, "end end (")], &scripts).expect("written");
        assert_eq!(files.len(), 2);
        assert_eq!(std::fs::read_to_string(scripts.join(MODULE_FILE)).expect("module"), ANIYOMI_LUA);
        assert_eq!(std::fs::read_to_string(scripts.join("custombutton_1.lua")).expect("one"), files[0].text);
        assert!(std::fs::read_to_string(scripts.join("custombutton_2.lua")).expect("two").contains("end end ("));
        assert!(!scripts.join("custombutton_99.lua").exists(), "a retired button file is removed");
        assert!(scripts.join("user.lua").exists(), "files that are not button scripts stay");
    }

    /// C11 on disk: with zero buttons every old button file goes, and the module stays.
    #[test]
    fn zero_buttons_remove_the_old_files_from_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_player_scripts(&[button(1, false, "mp.pause()")], dir.path()).expect("first");
        assert!(dir.path().join("custombutton_1.lua").exists());
        let files = write_player_scripts(&[], dir.path()).expect("second");
        assert!(files.is_empty());
        assert!(!dir.path().join("custombutton_1.lua").exists());
        assert!(dir.path().join(MODULE_FILE).exists());
    }

    #[test]
    fn the_player_commands_load_each_script_and_send_the_press_messages() {
        let files = build_player_scripts(&[button(3, false, "")], "/s");
        assert_eq!(load_script_commands(&files, Path::new("/s")), vec![vec!["load-script".to_owned(), "/s/custombutton_3.lua".to_owned()]]);
        assert_eq!(press_command(3, false), ["script-message", "call_button_3"]);
        assert_eq!(press_command(3, true), ["script-message", "call_button_3_long"]);
    }
}
