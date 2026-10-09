//! One-shot player messages and the result events of the screenshot actions (area 45
//! `PlayerViewModel.playerUpdate`, `Event.SavedImage`, `Event.ShareImage`, `Event.SetAsArt`;
//! behaviour 1.1, E-23; T-45b-060). A single pill slot holds the visible text: a newer message
//! replaces it and restarts its timer. Result events travel on an unbounded channel, so a
//! producer never blocks on the window.

use std::io;
use std::path::PathBuf;

use crate::enums::SetAsArt;
use crate::errors::PlayerMessage;

/// How long a message pill stays up after it was shown (2.5 s, UI parity section 1.1).
pub const MESSAGE_PILL_MS: u64 = 2500;

/// What the message pill says: a fixed message or text from a script (`show_text`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PillText {
    Message(PlayerMessage),
    Text(String),
}

/// The single pill slot. `now_ms` is any monotonic clock in milliseconds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessagePill {
    shown: Option<(PillText, u64)>,
}

impl MessagePill {
    /// Shows `text` and restarts the timer, replacing whatever was up.
    pub fn show(&mut self, text: PillText, now_ms: u64) {
        self.shown = Some((text, now_ms));
    }

    /// The text to draw at `now_ms`, or `None` once the timer has run out.
    pub fn visible(&self, now_ms: u64) -> Option<&PillText> {
        let (text, shown_at) = self.shown.as_ref()?;
        (now_ms.saturating_sub(*shown_at) < MESSAGE_PILL_MS).then_some(text)
    }
}

/// A result of a screenshot action, sent to the window.
pub enum PlayerEvent {
    SavedImage(io::Result<PathBuf>),
    ShareImage(io::Result<PathBuf>),
    SetAsArt(SetAsArt),
}

/// The channel for result events. Unbounded: `send` never waits for the receiver (E-23).
pub fn event_channel() -> (
    async_channel::Sender<PlayerEvent>,
    async_channel::Receiver<PlayerEvent>,
) {
    async_channel::unbounded()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_message_replaces_the_visible_one_and_restarts_the_timer() {
        let mut pill = MessagePill::default();
        pill.show(PillText::Message(PlayerMessage::AutoPlayOn), 0);
        pill.show(PillText::Text("Skipped".into()), 2000);
        assert_eq!(pill.visible(2000), Some(&PillText::Text("Skipped".into())));
        assert_eq!(
            pill.visible(2000 + MESSAGE_PILL_MS - 1),
            Some(&PillText::Text("Skipped".into()))
        );
        assert_eq!(pill.visible(2000 + MESSAGE_PILL_MS), None);
    }

    #[test]
    fn an_empty_slot_shows_nothing() {
        assert_eq!(MessagePill::default().visible(0), None);
    }

    #[test]
    fn results_are_queued_without_a_waiting_receiver() {
        let (tx, rx) = event_channel();
        for _ in 0..1000 {
            tx.try_send(PlayerEvent::SetAsArt(SetAsArt::Success))
                .expect("unbounded send never fails");
        }
        tx.try_send(PlayerEvent::SavedImage(Ok(PathBuf::from("/a.png"))))
            .expect("unbounded send never fails");
        let mut count = 0;
        while let Ok(event) = rx.try_recv() {
            count += 1;
            if count == 1001 {
                assert!(matches!(event, PlayerEvent::SavedImage(Ok(_))));
            }
        }
        assert_eq!(count, 1001);
    }
}
