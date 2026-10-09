//! Custom buttons in the More sheet and the primary button (area 46 `MoreSheet.kt`, area 45
//! `PlayerViewModel.kt` init block and `CustomButton.execute`; T-46b-020, T-45b-036). Pure: the
//! fetch state, the primary button and its title, and the script messages mpv receives. The button
//! model and the message names come from `kuro_domain::custom_buttons`.

pub use kuro_domain::custom_buttons::CustomButton;
use kuro_domain::custom_buttons::message_name;

/// Chips wrap in a flow box this wide.
pub const FLOW_WIDTH: i32 = 420;

/// Message shown when the fetch fails without a message of its own (`?: "Unable to fetch buttons"`).
pub const FETCH_FAILED_TEXT: &str = "Unable to fetch buttons";

/// The fetch state of the button list (`CustomButtonFetchState`).
#[derive(Clone, Debug, PartialEq)]
pub enum FetchState {
    Loading,
    Success(Vec<CustomButton>),
    Error(String),
}

impl FetchState {
    /// The buttons to show: the list on success, nothing while loading or after an error.
    pub fn buttons(&self) -> &[CustomButton] {
        match self {
            Self::Success(buttons) => buttons,
            Self::Loading | Self::Error(_) => &[],
        }
    }

    /// The error state for a failed fetch. A missing message gets the default text.
    pub fn failed(message: Option<&str>) -> Self {
        Self::Error(message.unwrap_or(FETCH_FAILED_TEXT).to_owned())
    }
}

/// The section is shown only when at least one button exists.
pub fn section_visible(buttons: &[CustomButton]) -> bool {
    !buttons.is_empty()
}

/// The primary button: the first favourite in the list.
pub fn primary(buttons: &[CustomButton]) -> Option<&CustomButton> {
    buttons.iter().find(|b| b.is_favorite)
}

/// The title to store when the primary title is still empty: the primary button's name. A title
/// that a Lua script already set (non-empty) is kept, so this returns `None`.
pub fn initial_title(current_title: &str, buttons: &[CustomButton]) -> Option<String> {
    if !current_title.is_empty() {
        return None;
    }
    primary(buttons).map(|b| b.name.clone())
}

/// Message for a tap: `call_button_<id>`.
pub fn tap_message(id: i64) -> String {
    message_name(id, false)
}

/// Message for a long press (touch) or a secondary click (mouse): `call_button_<id>_long`.
pub fn long_message(id: i64) -> String {
    message_name(id, true)
}

/// The mpv command for a script message.
pub fn script_message(message: &str) -> Vec<String> {
    vec!["script-message".to_owned(), message.to_owned()]
}

/// The command a press of `button` sends: a tap, or a long press when `long_press` is set.
pub fn execute(button: &CustomButton, long_press: bool) -> Vec<String> {
    script_message(&message_name(button.id, long_press))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(id: i64, name: &str, favorite: bool) -> CustomButton {
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

    #[test]
    fn section_hides_without_buttons() {
        assert!(!section_visible(&[]));
        assert!(section_visible(&[button(3, "Skip", false)]));
    }

    #[test]
    fn tap_and_long_press_send_their_messages() {
        assert_eq!(tap_message(7), "call_button_7");
        assert_eq!(long_message(7), "call_button_7_long");
        assert_eq!(
            script_message(&tap_message(7)),
            vec!["script-message", "call_button_7"]
        );
        let skip = button(7, "Skip", false);
        assert_eq!(
            execute(&skip, false),
            vec!["script-message", "call_button_7"]
        );
        assert_eq!(
            execute(&skip, true),
            vec!["script-message", "call_button_7_long"]
        );
    }

    #[test]
    fn fetch_state_shows_only_a_success_list() {
        let list = vec![button(1, "Skip", true)];
        assert!(FetchState::Loading.buttons().is_empty());
        assert_eq!(FetchState::Success(list.clone()).buttons(), list.as_slice());
        assert!(FetchState::failed(Some("db down")).buttons().is_empty());
        assert_eq!(
            FetchState::failed(None),
            FetchState::Error("Unable to fetch buttons".into())
        );
        assert_eq!(
            FetchState::failed(Some("db down")),
            FetchState::Error("db down".into())
        );
    }

    #[test]
    fn primary_is_the_first_favourite() {
        let list = [
            button(1, "Skip", false),
            button(2, "Recap", true),
            button(3, "Next", true),
        ];
        assert_eq!(primary(&list).map(|b| b.id), Some(2));
        assert!(primary(&[button(1, "Skip", false)]).is_none());
    }

    #[test]
    fn primary_title_defaults_to_the_name_and_keeps_a_lua_title() {
        let list = [button(2, "Recap", true)];
        assert_eq!(initial_title("", &list), Some("Recap".to_owned()));
        assert_eq!(initial_title("Lua title", &list), None);
        assert_eq!(initial_title("", &[button(1, "Skip", false)]), None);
    }
}
