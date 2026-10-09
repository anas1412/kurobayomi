//! The two writes a player control makes: the preference store and mpv (area 46 `p2-topic-subtitle-style-and-delay`
//! section 1, T-46b-004, T-46b-005). Every control returns the list of writes it wants, so the
//! preference and the live property cannot drift apart: the caller applies the whole list. Pure: the
//! store side is the caller's job, mpv writes go through `PropertySink`.

use crate::mpv::PropertySink;

/// One write requested by a control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
    /// Stores a preference. The value is the text the store parses back (`true`, `-1`, `1.5`, a
    /// variant name).
    Pref { key: &'static str, value: String },
    /// Stores a string set (`getStringSet(...).set(...)`, for example the speed presets).
    PrefSet {
        key: &'static str,
        values: Vec<String>,
    },
    /// `deleteAndGet`: removes the stored value so the default applies again.
    PrefDelete { key: &'static str },
    /// Sets an mpv property to the text mpv parses (`yes`, `#AARRGGBB`, `55`).
    Mpv { name: &'static str, value: String },
}

impl Write {
    pub fn pref(key: &'static str, value: impl Into<String>) -> Self {
        Self::Pref {
            key,
            value: value.into(),
        }
    }

    pub fn mpv(name: &'static str, value: impl Into<String>) -> Self {
        Self::Mpv {
            name,
            value: value.into(),
        }
    }
}

/// Sends every mpv write in order. A failed write is logged and the rest still go out, so one
/// refused property never leaves a control half-applied on the UI side.
pub fn apply_mpv(sink: &impl PropertySink, writes: &[Write]) {
    for write in writes {
        if let Write::Mpv { name, value } = write
            && let Err(error) = sink.set_str(name, value)
        {
            tracing::warn!(property = name, %error, "mpv refused a subtitle property write");
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use crate::mpv::{MpvError, PropertySink, PropertySource};

    /// Records mpv writes and answers reads from the same map, so a test can apply writes and
    /// read the properties back the way the panel does after a reset.
    #[derive(Default)]
    pub struct FakeMpv {
        pub values: RefCell<HashMap<String, String>>,
        pub writes: RefCell<Vec<(String, String)>>,
    }

    impl FakeMpv {
        pub fn with(pairs: &[(&str, &str)]) -> Self {
            let fake = Self::default();
            for (k, v) in pairs {
                fake.values
                    .borrow_mut()
                    .insert((*k).to_owned(), (*v).to_owned());
            }
            fake
        }
    }

    impl PropertySink for FakeMpv {
        fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError> {
            self.writes
                .borrow_mut()
                .push((name.to_owned(), value.to_owned()));
            self.values
                .borrow_mut()
                .insert(name.to_owned(), value.to_owned());
            Ok(())
        }
    }

    impl PropertySource for FakeMpv {
        fn get_str(&self, name: &str) -> Option<String> {
            self.values.borrow().get(name).cloned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::FakeMpv;
    use super::*;

    #[test]
    fn only_mpv_writes_reach_the_sink_in_order() {
        let fake = FakeMpv::default();
        apply_mpv(
            &fake,
            &[
                Write::pref("pref_bold_subtitles", "true"),
                Write::mpv("sub-bold", "yes"),
                Write::PrefDelete {
                    key: "pref_bold_subtitles",
                },
                Write::mpv("sub-italic", "no"),
            ],
        );
        assert_eq!(
            *fake.writes.borrow(),
            vec![
                ("sub-bold".to_owned(), "yes".to_owned()),
                ("sub-italic".to_owned(), "no".to_owned()),
            ]
        );
    }
}
