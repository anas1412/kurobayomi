//! Text for messages that start on a background thread (plan 51-i18n T-51a-032).
//!
//! Background code (tokio tasks) builds a [`Notice`]: a key and typed values, never text. The GTK
//! main thread calls [`Notice::render`], so the text uses the language active at display time.

use crate::{Arg, MsgKey, tr, tr_plural, trf};

/// An argument that owns its data, so it can cross threads.
#[derive(Clone, Debug, PartialEq)]
pub enum OwnedArg {
    Str(String),
    Int(i64),
    Dec(f64),
}

impl OwnedArg {
    fn as_arg(&self) -> Arg<'_> {
        match self {
            OwnedArg::Str(s) => Arg::Str(s),
            OwnedArg::Int(i) => Arg::Int(*i),
            OwnedArg::Dec(d) => Arg::Dec(*d),
        }
    }
}

/// A message to render on the GTK thread. `args` are named `arg1`, `arg2`, and so on.
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// A `<string>` message.
    Text {
        key: MsgKey,
        args: Vec<(&'static str, OwnedArg)>,
    },
    /// A `<plurals>` message; `count` selects the form and is also `$count`.
    Plural {
        key: MsgKey,
        count: i64,
        args: Vec<(&'static str, OwnedArg)>,
    },
}

impl Notice {
    /// The text in the active language. Call on the GTK main thread.
    pub fn render(&self) -> String {
        match self {
            Notice::Text { key, args } => trf(*key, &borrow(args)),
            Notice::Plural { key, count, args } => tr_plural(*key, *count, &borrow(args)),
        }
    }
}

fn borrow<'a>(args: &'a [(&'static str, OwnedArg)]) -> Vec<(&'static str, Arg<'a>)> {
    args.iter()
        .map(|(name, arg)| (*name, arg.as_arg()))
        .collect()
}

/// Renders a message with no arguments (kept for symmetry with [`Notice`] callers).
pub fn plain(key: MsgKey) -> String {
    tr(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send + 'static>() {}

    #[test]
    fn notice_is_sendable_and_renders_the_plural_form() {
        // T-51a-032: a background thread builds the notice, the main thread renders it.
        assert_send::<Notice>();
        let notice = Notice::Plural {
            key: MsgKey::notification_chapters_generic,
            count: 3,
            args: Vec::new(),
        };
        let handle = std::thread::spawn(move || notice.render());
        assert_eq!(handle.join().expect("thread"), "3 new chapters");
    }

    #[test]
    fn text_notice_passes_named_arguments() {
        let notice = Notice::Text {
            key: MsgKey::restore_duration,
            args: vec![("arg1", OwnedArg::Int(3)), ("arg2", OwnedArg::Int(7))],
        };
        assert_eq!(notice.render(), "03 min, 07 sec");
    }

    #[test]
    fn single_chapter_uses_the_one_form() {
        let notice = Notice::Plural {
            key: MsgKey::notification_chapters_generic,
            count: 1,
            args: Vec::new(),
        };
        assert_eq!(notice.render(), "1 new chapter");
    }
}
