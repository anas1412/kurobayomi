//! The desktop notification boundary (CRATE-MAP C-04). The app implements [`Notifier`] over `gio::Notification`; library
//! code only calls it, so it never needs GTK.

/// Shows and withdraws desktop notices by id. A notice shown with an id that is already shown replaces it.
pub trait Notifier: Send + Sync {
    /// `body` is `None` when the notice must not show its content.
    fn show(&self, id: &str, title: &str, body: Option<&str>);
    /// Removes the notice with this id. A missing id is not an error.
    fn withdraw(&self, id: &str);
}
