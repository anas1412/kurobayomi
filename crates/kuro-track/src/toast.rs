//! Toast surface of the track windows (behaviour U section 10, T-27b-039). One message shows at a time, and a new one
//! replaces the visible one. A message hides after [`TOAST_HIDE_MS`], or at once when dismissed by a click. The GUI
//! draws the overlay at the bottom of the active window and resolves the [`MsgKey`] to text.

use kuro_platform::toast::{ToastLength, Toaster};

use crate::messages::{MsgKey, english};

/// How long a toast stays up, in milliseconds.
pub const TOAST_HIDE_MS: i64 = 3_500;

/// The toast of one window. Empty when nothing is shown.
#[derive(Debug, Default)]
pub struct ToastSurface {
    current: Option<(MsgKey, i64)>,
}

impl ToastSurface {
    /// Shows `message` and starts its timer at `now_ms`. Replaces any visible toast.
    pub fn show(&mut self, message: MsgKey, now_ms: i64) {
        self.current = Some((message, now_ms + TOAST_HIDE_MS));
    }

    /// The message still visible at `now_ms`. `None` once it has hidden.
    pub fn visible(&self, now_ms: i64) -> Option<&MsgKey> {
        self.current
            .as_ref()
            .filter(|(_, hide_at)| now_ms < *hide_at)
            .map(|(message, _)| message)
    }

    /// Click dismiss: hides the toast at once.
    pub fn dismiss(&mut self) {
        self.current = None;
    }
}

/// Which window a routed toast went to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastTarget {
    /// The open track dialog, which has its own overlay.
    Dialog,
    /// The main window. Toasts raised after a dialog closed land here.
    Main,
}

/// Sends the track toasts to the presenter of the root crate (`kuro_platform::toast`), whose overlay shows one
/// message at a time at the bottom of its window. A toast goes to the open dialog's overlay, and to the main
/// window's once the dialog has closed (T-27b-039).
#[derive(Debug, Clone)]
pub struct ToastRouter {
    main: Toaster,
    dialog: Option<Toaster>,
}

impl ToastRouter {
    pub fn new(main: Toaster) -> Self {
        Self { main, dialog: None }
    }

    /// A dialog opened with its own overlay sender.
    pub fn dialog_opened(&mut self, dialog: Toaster) {
        self.dialog = Some(dialog);
    }

    /// The dialog closed. Later toasts, such as the result of a detached write, go to the main window.
    pub fn dialog_closed(&mut self) {
        self.dialog = None;
    }

    /// Shows `message` in the active window and says which one that was. [`ToastLength::Long`] lasts
    /// [`TOAST_HIDE_MS`].
    pub fn show(&self, message: &MsgKey) -> ToastTarget {
        let (toaster, target) = match &self.dialog {
            Some(dialog) => (dialog, ToastTarget::Dialog),
            None => (&self.main, ToastTarget::Main),
        };
        toaster.show(english(message), ToastLength::Long);
        target
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::toast::toast_channel;

    use super::*;

    #[test]
    fn tr_27b_039_long_toasts_last_as_long_as_the_surface() {
        assert_eq!(
            ToastLength::Long.duration().as_millis() as i64,
            TOAST_HIDE_MS
        );
    }

    #[tokio::test]
    async fn tr_27b_039_a_toast_goes_to_the_open_dialog_then_to_the_main_window_after_it_closes() {
        let (main, main_feed) = toast_channel();
        let (dialog, dialog_feed) = toast_channel();
        let mut router = ToastRouter::new(main);
        router.dialog_opened(dialog);
        assert_eq!(router.show(&MsgKey::LoginSuccess), ToastTarget::Dialog);
        assert_eq!(
            dialog_feed.next().await.unwrap().text,
            english(&MsgKey::LoginSuccess)
        );
        router.dialog_closed();
        assert_eq!(router.show(&MsgKey::UnknownError), ToastTarget::Main);
        let shown = main_feed.next().await.unwrap();
        assert_eq!(shown.text, english(&MsgKey::UnknownError));
        assert_eq!(shown.length, ToastLength::Long);
    }

    #[test]
    fn tr_27b_039_a_toast_hides_after_3500_ms() {
        let mut t = ToastSurface::default();
        t.show(MsgKey::LoginSuccess, 1_000);
        assert_eq!(
            t.visible(1_000 + TOAST_HIDE_MS - 1),
            Some(&MsgKey::LoginSuccess)
        );
        assert_eq!(t.visible(1_000 + TOAST_HIDE_MS), None);
        assert_eq!(TOAST_HIDE_MS, 3_500);
    }

    #[test]
    fn tr_27b_039_a_new_toast_replaces_the_visible_one_and_restarts_the_timer() {
        let mut t = ToastSurface::default();
        t.show(MsgKey::LoginSuccess, 0);
        t.show(MsgKey::UnknownError, 3_000);
        assert_eq!(t.visible(3_100), Some(&MsgKey::UnknownError));
        assert_eq!(t.visible(3_000 + TOAST_HIDE_MS), None);
    }

    #[test]
    fn tr_27b_039_a_click_dismisses_at_once() {
        let mut t = ToastSurface::default();
        t.show(MsgKey::NoResultsFound, 0);
        t.dismiss();
        assert_eq!(t.visible(1), None);
    }
}
