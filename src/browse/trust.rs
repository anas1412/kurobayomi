//! The untrusted-extension dialog and what its answers do (T-41a-012). Pure: the page runs the effects. Trusting or
//! uninstalling an entry that already vanished does nothing (T28, race R13).

use kuro_i18n::{MsgKey, tr};

/// The dialog text: title "Untrusted extension", the risk body, and the buttons Trust and Uninstall.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustDialog {
    pub title: String,
    pub body: String,
    pub trust: String,
    pub uninstall: String,
}

pub fn trust_dialog() -> TrustDialog {
    TrustDialog {
        title: tr(MsgKey::untrusted_extension),
        body: tr(MsgKey::untrusted_extension_message),
        trust: tr(MsgKey::ext_trust),
        uninstall: tr(MsgKey::ext_uninstall),
    }
}

/// How the dialog was answered. Dismissing it without a button press is `Dismiss`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustAnswer {
    Trust,
    Uninstall,
    Dismiss,
}

/// What the page does after an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustEffect {
    Trust,
    Uninstall,
    Nothing,
}

/// `still_untrusted` is whether the entry is still an untrusted extension when the answer arrives.
pub fn answer(answer: TrustAnswer, still_untrusted: bool) -> TrustEffect {
    match (answer, still_untrusted) {
        (TrustAnswer::Dismiss, _) | (_, false) => TrustEffect::Nothing,
        (TrustAnswer::Trust, true) => TrustEffect::Trust,
        (TrustAnswer::Uninstall, true) => TrustEffect::Uninstall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_carries_the_untrusted_text_and_both_buttons() {
        let dialog = trust_dialog();
        assert_eq!(dialog.title, tr(MsgKey::untrusted_extension));
        assert_eq!(dialog.body, tr(MsgKey::untrusted_extension_message));
        assert_eq!(dialog.trust, tr(MsgKey::ext_trust));
        assert_eq!(dialog.uninstall, tr(MsgKey::ext_uninstall));
    }

    #[test]
    fn trust_and_uninstall_act_only_while_the_entry_is_still_untrusted() {
        assert_eq!(answer(TrustAnswer::Trust, true), TrustEffect::Trust);
        assert_eq!(answer(TrustAnswer::Uninstall, true), TrustEffect::Uninstall);
        assert_eq!(answer(TrustAnswer::Trust, false), TrustEffect::Nothing, "a vanished entry is a no-op");
        assert_eq!(answer(TrustAnswer::Uninstall, false), TrustEffect::Nothing);
    }

    #[test]
    fn dismissing_changes_nothing() {
        assert_eq!(answer(TrustAnswer::Dismiss, true), TrustEffect::Nothing);
    }
}
