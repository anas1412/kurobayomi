//! The `user-data/aniyomi/*` protocol (area 45, T-45a-020, T-45a-022, T-45a-023). Lua scripts set one
//! of these mpv properties; the app observes it, parses the value here and runs the matching
//! action. Port of `PlayerViewModel.handleLuaInvocation` (behaviour doc section 5). Pure: the caller
//! runs the returned command and then resets the property (`reset_write`).

use crate::custom_buttons::{CustomButton, primary};
use crate::enums::{Dialogs, Panels, Sheets};
use crate::episodes::begin_switch;
use crate::hosts::open_panel;
use crate::state::PlayerState;
use crate::transport::{Transport, parse_transport};

/// Prefix of every protocol property.
pub const PREFIX: &str = "user-data/aniyomi/";

/// `show`, `hide` or `toggle`, the value of the UI and button toggles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Show,
    Hide,
    Toggle,
}

/// Episode switch request: `p` for the previous episode, `n` for the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpisodeStep {
    Previous,
    Next,
}

/// `launch_int_picker` payload: `title|name_format|start|stop|step|property`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntPicker {
    pub title: String,
    pub name_format: String,
    pub start: i32,
    pub stop: i32,
    pub step: i32,
    /// The mpv property the picked value is written to.
    pub property: String,
}

impl IntPicker {
    /// The picker dialog for this request. `default_value` is the current value of `property`.
    pub fn dialog(&self, default_value: i32) -> Dialogs {
        Dialogs::IntegerPicker {
            default_value,
            min_value: self.start,
            max_value: self.stop,
            step: self.step,
            name_format: self.name_format.clone(),
            title: self.title.clone(),
        }
    }

    /// The mpv write for a value picked in the dialog (`onChange`).
    pub fn write(&self, value: i32) -> (String, i64) {
        (self.property.clone(), i64::from(value))
    }
}

/// One action requested by a protocol property.
#[derive(Clone, Debug, PartialEq)]
pub enum BridgeCommand {
    ShowText(String),
    ToggleUi(Toggle),
    ShowPanel(Panels),
    /// `software_keyboard`. Desktop has no soft keyboard, so the window ignores it.
    SoftwareKeyboard(Toggle),
    SetButtonTitle(String),
    ResetButtonTitle,
    ToggleButton(Toggle),
    SwitchEpisode(EpisodeStep),
    Transport(Transport),
    IntPicker(IntPicker),
}

/// The value with one pair of surrounding double quotes removed (`removePrefix`/`removeSuffix`).
fn unquote(value: &str) -> &str {
    let value = value.strip_prefix('"').unwrap_or(value);
    value.strip_suffix('"').unwrap_or(value)
}

/// Whether the caller must reset the property after handling this value. Every non-empty value is
/// reset, as in Kotlin, so a repeated identical command still fires.
pub fn reset_needed(value: &str) -> bool {
    !unquote(value).is_empty()
}

/// The write that clears a protocol property after its value is handled.
pub fn reset_write(property: &str) -> (String, &'static str) {
    (property.to_owned(), "")
}

fn toggle(data: &str) -> Option<Toggle> {
    match data {
        "show" => Some(Toggle::Show),
        "hide" => Some(Toggle::Hide),
        "toggle" => Some(Toggle::Toggle),
        _ => None,
    }
}

fn int_picker(data: &str) -> Option<IntPicker> {
    let parts: Vec<&str> = data.split('|').collect();
    let [title, name_format, start, stop, step, property, ..] = parts.as_slice() else {
        return None;
    };
    Some(IntPicker {
        title: (*title).to_owned(),
        name_format: (*name_format).to_owned(),
        start: start.parse().ok()?,
        stop: stop.parse().ok()?,
        step: step.parse().ok()?,
        property: (*property).to_owned(),
    })
}

/// Parses one observed protocol value into the action it asks for. `None` means nothing to do:
/// the property is outside the protocol, the value is empty, the value is not one the protocol
/// knows, or it is malformed. Malformed and unknown values are logged and dropped (E-53), never
/// a panic. The caller clears the property either way.
pub fn parse(property: &str, value: &str) -> Option<BridgeCommand> {
    let name = property.strip_prefix(PREFIX)?;
    let data = unquote(value);
    if data.is_empty() {
        return None;
    }
    let Some(command) = dispatch(name, property, data) else {
        tracing::warn!(property, value = data, "user-data payload ignored");
        return None;
    };
    if matches!(command, BridgeCommand::SoftwareKeyboard(_)) {
        tracing::info!("software keyboard is not supported on desktop; ignored");
    }
    Some(command)
}

fn dispatch(name: &str, property: &str, data: &str) -> Option<BridgeCommand> {
    Some(match name {
        "show_text" => BridgeCommand::ShowText(data.to_owned()),
        "toggle_ui" => BridgeCommand::ToggleUi(toggle(data)?),
        "show_panel" => BridgeCommand::ShowPanel(match data {
            "subtitle_settings" => Panels::SubtitleSettings,
            "subtitle_delay" => Panels::SubtitleDelay,
            "audio_delay" => Panels::AudioDelay,
            "video_filters" => Panels::VideoFilters,
            _ => return None,
        }),
        "software_keyboard" => BridgeCommand::SoftwareKeyboard(toggle(data)?),
        "set_button_title" => BridgeCommand::SetButtonTitle(data.to_owned()),
        "reset_button_title" => BridgeCommand::ResetButtonTitle,
        "toggle_button" => BridgeCommand::ToggleButton(toggle(data)?),
        "switch_episode" => BridgeCommand::SwitchEpisode(match data {
            "p" => EpisodeStep::Previous,
            "n" => EpisodeStep::Next,
            _ => return None,
        }),
        "launch_int_picker" => BridgeCommand::IntPicker(int_picker(data)?),
        "pause" | "seek_by" | "seek_to" | "seek_by_with_text" | "seek_to_with_text" => {
            BridgeCommand::Transport(parse_transport(property, data)?)
        }
        _ => return None,
    })
}

/// What the window still has to do after `apply` changed the state.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// The state change was all there was to do.
    None,
    /// Show this text in the pill.
    ShowText(String),
    /// Pause mpv, then load the previous or next episode (`changeEpisode`).
    SwitchEpisode { previous: bool },
    /// Open the picker dialog. The window reads the property for its default value.
    IntPicker(IntPicker),
    /// Run this transport action on mpv.
    Transport(Transport),
}

/// Applies one parsed command to the state (`handleLuaInvocation`, behaviour 12). `buttons` is the
/// loaded custom-button list, used for the favourite button.
pub fn apply(state: &mut PlayerState, command: BridgeCommand, buttons: &[CustomButton]) -> Effect {
    match command {
        BridgeCommand::ShowText(text) => return Effect::ShowText(text),
        BridgeCommand::ToggleUi(Toggle::Show) => state.controls_shown = true,
        BridgeCommand::ToggleUi(Toggle::Toggle) => state.controls_shown = !state.controls_shown,
        BridgeCommand::ToggleUi(Toggle::Hide) => {
            state.sheet = Sheets::None;
            state.panel = Panels::None;
            state.dialog = Dialogs::None;
            state.controls_shown = false;
        }
        BridgeCommand::ShowPanel(panel) => {
            if open_panel(state, panel) {
                state.controls_shown = false;
            }
        }
        BridgeCommand::SoftwareKeyboard(_) => {}
        BridgeCommand::SetButtonTitle(title) => state.primary_button_title = title,
        BridgeCommand::ResetButtonTitle => {
            if let Some(button) = primary(buttons) {
                state.primary_button_title = button.name.clone();
            }
        }
        BridgeCommand::ToggleButton(Toggle::Hide) => state.primary_button = None,
        BridgeCommand::ToggleButton(Toggle::Show) => show_button(state, buttons),
        BridgeCommand::ToggleButton(Toggle::Toggle) => {
            if state.primary_button.is_none() {
                show_button(state, buttons);
            } else {
                state.primary_button = None;
            }
        }
        BridgeCommand::SwitchEpisode(step) => {
            begin_switch(state);
            return Effect::SwitchEpisode {
                previous: step == EpisodeStep::Previous,
            };
        }
        BridgeCommand::Transport(t) => return Effect::Transport(t),
        BridgeCommand::IntPicker(picker) => return Effect::IntPicker(picker),
    }
    Effect::None
}

/// `showButton`: only fills the primary button when it is empty.
fn show_button(state: &mut PlayerState, buttons: &[CustomButton]) {
    if state.primary_button.is_none() {
        state.primary_button = primary(buttons).cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str, value: &str) -> Option<BridgeCommand> {
        parse(&format!("{PREFIX}{name}"), value)
    }

    #[test]
    fn show_text_keeps_the_text_and_drops_quotes() {
        assert_eq!(
            cmd("show_text", "\"Skipped intro\""),
            Some(BridgeCommand::ShowText("Skipped intro".to_owned()))
        );
    }

    #[test]
    fn ui_toggle_values() {
        assert_eq!(
            cmd("toggle_ui", "show"),
            Some(BridgeCommand::ToggleUi(Toggle::Show))
        );
        assert_eq!(
            cmd("toggle_ui", "hide"),
            Some(BridgeCommand::ToggleUi(Toggle::Hide))
        );
        assert_eq!(
            cmd("toggle_ui", "toggle"),
            Some(BridgeCommand::ToggleUi(Toggle::Toggle))
        );
        assert_eq!(cmd("toggle_ui", "blink"), None);
    }

    #[test]
    fn panels_map_to_the_four_names() {
        assert_eq!(
            cmd("show_panel", "video_filters"),
            Some(BridgeCommand::ShowPanel(Panels::VideoFilters))
        );
        assert_eq!(
            cmd("show_panel", "subtitle_delay"),
            Some(BridgeCommand::ShowPanel(Panels::SubtitleDelay))
        );
        assert_eq!(cmd("show_panel", "playlist"), None);
    }

    #[test]
    fn software_keyboard_is_accepted_for_the_window_to_ignore() {
        assert_eq!(
            cmd("software_keyboard", "show"),
            Some(BridgeCommand::SoftwareKeyboard(Toggle::Show))
        );
    }

    #[test]
    fn button_and_episode_commands() {
        assert_eq!(
            cmd("set_button_title", "Skip"),
            Some(BridgeCommand::SetButtonTitle("Skip".to_owned()))
        );
        assert_eq!(
            cmd("reset_button_title", "unused"),
            Some(BridgeCommand::ResetButtonTitle)
        );
        assert_eq!(
            cmd("toggle_button", "hide"),
            Some(BridgeCommand::ToggleButton(Toggle::Hide))
        );
        assert_eq!(
            cmd("switch_episode", "n"),
            Some(BridgeCommand::SwitchEpisode(EpisodeStep::Next))
        );
        assert_eq!(
            cmd("switch_episode", "p"),
            Some(BridgeCommand::SwitchEpisode(EpisodeStep::Previous))
        );
        assert_eq!(cmd("switch_episode", "x"), None);
    }

    #[test]
    fn transport_commands_reuse_the_transport_parser() {
        assert_eq!(
            cmd("pause", "pauseunpause"),
            Some(BridgeCommand::Transport(Transport::PauseUnpause))
        );
        assert_eq!(
            cmd("seek_by", "10"),
            Some(BridgeCommand::Transport(Transport::SeekBy(10.0)))
        );
        assert_eq!(
            cmd("seek_to_with_text", "90|Skip"),
            Some(BridgeCommand::Transport(Transport::SeekTo(90.0)))
        );
        assert_eq!(cmd("seek_by", "soon"), None);
    }

    #[test]
    fn int_picker_payload_becomes_a_dialog_and_a_write() {
        let Some(BridgeCommand::IntPicker(picker)) =
            cmd("launch_int_picker", "Speed|%d s|0|30|5|user-data/x")
        else {
            panic!("expected an int picker");
        };
        assert_eq!(picker.title, "Speed");
        assert_eq!(
            picker.dialog(10),
            Dialogs::IntegerPicker {
                default_value: 10,
                min_value: 0,
                max_value: 30,
                step: 5,
                name_format: "%d s".to_owned(),
                title: "Speed".to_owned(),
            }
        );
        assert_eq!(picker.write(15), ("user-data/x".to_owned(), 15));
    }

    #[test]
    fn malformed_int_picker_is_ignored() {
        assert_eq!(cmd("launch_int_picker", "Speed|%d|0|30|5"), None);
        assert_eq!(cmd("launch_int_picker", "Speed|%d|a|30|5|p"), None);
    }

    #[test]
    fn malformed_payloads_are_dropped_without_panicking() {
        assert_eq!(cmd("launch_int_picker", "a|b|c|d|e|f"), None);
        assert_eq!(cmd("launch_int_picker", "||||||"), None);
        assert_eq!(cmd("toggle_button", "blink"), None);
        assert_eq!(cmd("show_panel", "\"\""), None);
        assert_eq!(cmd("seek_by", "soon"), None);
    }

    #[test]
    fn empty_values_and_other_properties_do_nothing() {
        assert_eq!(cmd("show_text", ""), None);
        assert_eq!(cmd("show_text", "\"\""), None);
        assert_eq!(cmd("no_such_thing", "x"), None);
        assert_eq!(parse("speed", "2"), None);
    }

    #[test]
    fn every_non_empty_value_is_reset_and_empty_is_not() {
        assert!(reset_needed("\"hide\""));
        assert!(reset_needed("unused"));
        assert!(!reset_needed(""));
        assert_eq!(
            reset_write("user-data/aniyomi/pause"),
            ("user-data/aniyomi/pause".to_owned(), "")
        );
    }

    fn fav(id: i64, name: &str, favorite: bool) -> CustomButton {
        CustomButton {
            id,
            name: name.into(),
            is_favorite: favorite,
            sort_index: id,
            content: String::new(),
            long_press_content: String::new(),
            on_startup: String::new(),
        }
    }

    fn run(state: &mut PlayerState, name: &str, value: &str, buttons: &[CustomButton]) -> Effect {
        let command = cmd(name, value).expect("parses");
        apply(state, command, buttons)
    }

    #[test]
    fn show_text_becomes_a_pill_effect() {
        let mut s = PlayerState::default();
        assert_eq!(
            run(&mut s, "show_text", "Hi", &[]),
            Effect::ShowText("Hi".to_owned())
        );
    }

    #[test]
    fn toggle_ui_show_toggle_and_hide_rows() {
        let mut s = PlayerState::default();
        s.controls_shown = false;
        run(&mut s, "toggle_ui", "show", &[]);
        assert!(s.controls_shown);
        run(&mut s, "toggle_ui", "toggle", &[]);
        assert!(!s.controls_shown);
        run(&mut s, "toggle_ui", "toggle", &[]);
        assert!(s.controls_shown);
        s.sheet = Sheets::More;
        s.panel = Panels::AudioDelay;
        s.dialog = Dialogs::EpisodeList;
        run(&mut s, "toggle_ui", "hide", &[]);
        assert!(!s.controls_shown);
        assert_eq!(s.sheet, Sheets::None);
        assert_eq!(s.panel, Panels::None);
        assert_eq!(s.dialog, Dialogs::None);
    }

    #[test]
    fn show_panel_opens_it_and_hides_the_controls() {
        let mut s = PlayerState::default();
        s.sheet = Sheets::More;
        run(&mut s, "show_panel", "audio_delay", &[]);
        assert_eq!(s.panel, Panels::AudioDelay);
        assert_eq!(s.sheet, Sheets::None);
        assert!(!s.controls_shown);
    }

    #[test]
    fn button_title_set_and_reset_from_the_favourite() {
        let list = [fav(1, "Skip", false), fav(2, "Recap", true)];
        let mut s = PlayerState::default();
        run(&mut s, "set_button_title", "Custom", &list);
        assert_eq!(s.primary_button_title, "Custom");
        run(&mut s, "reset_button_title", "unused", &list);
        assert_eq!(s.primary_button_title, "Recap");
        run(&mut s, "set_button_title", "Custom", &[]);
        run(&mut s, "reset_button_title", "unused", &[]);
        assert_eq!(s.primary_button_title, "Custom");
    }

    #[test]
    fn toggle_button_show_hide_and_toggle_rows() {
        let list = [fav(2, "Recap", true)];
        let mut s = PlayerState::default();
        run(&mut s, "toggle_button", "show", &list);
        assert_eq!(s.primary_button.as_ref().map(|b| b.id), Some(2));
        run(&mut s, "toggle_button", "hide", &list);
        assert!(s.primary_button.is_none());
        run(&mut s, "toggle_button", "toggle", &list);
        assert!(s.primary_button.is_some());
        run(&mut s, "toggle_button", "toggle", &list);
        assert!(s.primary_button.is_none());
        run(&mut s, "toggle_button", "show", &[fav(1, "Skip", false)]);
        assert!(s.primary_button.is_none());
    }

    #[test]
    fn switch_episode_starts_the_switch_and_asks_for_the_load() {
        let mut s = PlayerState::default();
        s.sheet = Sheets::More;
        assert_eq!(
            run(&mut s, "switch_episode", "p", &[]),
            Effect::SwitchEpisode { previous: true }
        );
        assert!(s.is_loading_episode && s.is_loading_hosters);
        assert_eq!(s.sheet, Sheets::None);
        assert_eq!(
            run(&mut s, "switch_episode", "n", &[]),
            Effect::SwitchEpisode { previous: false }
        );
    }

    #[test]
    fn transport_and_picker_pass_through_and_keyboard_is_a_no_op() {
        let mut s = PlayerState::default();
        let before = s.clone();
        assert_eq!(
            run(&mut s, "seek_by", "10", &[]),
            Effect::Transport(Transport::SeekBy(10.0))
        );
        assert!(matches!(
            run(&mut s, "launch_int_picker", "T|%d|0|5|1|p", &[]),
            Effect::IntPicker(_)
        ));
        assert_eq!(run(&mut s, "software_keyboard", "show", &[]), Effect::None);
        assert_eq!(s, before);
    }
}
