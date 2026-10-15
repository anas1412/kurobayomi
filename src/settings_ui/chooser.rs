//! The message for a file chooser that could not open (task T-47b-055). A dismissed chooser is not an error; any other
//! failure of `gtk::FileDialog` means no chooser could be shown.
use kuro_i18n::{MsgKey, tr};
use gtk::glib;

pub fn unavailable() -> String {
    tr(MsgKey::settings_chooser_unavailable)
}

/// The toast for a chooser error: none when the user dismissed the chooser, otherwise the unavailable message.
pub fn failure_toast(err: &glib::Error) -> Option<String> {
    (!err.matches(gtk::DialogError::Dismissed)).then(unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dismissed_chooser_shows_no_toast() {
        let err = glib::Error::new(gtk::DialogError::Dismissed, "dismissed");
        assert_eq!(failure_toast(&err), None);
    }

    #[test]
    fn any_other_chooser_failure_shows_the_unavailable_message() {
        let err = glib::Error::new(gtk::DialogError::Failed, "no portal");
        assert_eq!(failure_toast(&err), Some(unavailable()));
    }
}
