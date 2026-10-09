//! Typed mpv property boundary for the sheets and panels (area 46, T-46b-004). `MpvProps` is the
//! seam: `MpvHandle` implements it, and tests use a recording fake. A failed write is logged and
//! never aborts the UI update (E-46-088). A failed read is `None`, and the caller falls back to the
//! stored preference.

use crate::mpv::{MpvError, MpvHandle};

pub trait MpvProps {
    fn get_f64(&self, name: &str) -> Option<f64>;
    fn get_i64(&self, name: &str) -> Option<i64>;
    fn get_bool(&self, name: &str) -> Option<bool>;
    fn get_str(&self, name: &str) -> Option<String>;

    fn set_f64(&self, name: &str, value: f64) -> Result<(), MpvError>;
    fn set_i64(&self, name: &str, value: i64) -> Result<(), MpvError>;
    fn set_bool(&self, name: &str, value: bool) -> Result<(), MpvError>;
    fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError>;

    fn command(&self, args: &[&str]) -> Result<(), MpvError>;
}

impl MpvProps for MpvHandle {
    fn get_f64(&self, name: &str) -> Option<f64> {
        MpvHandle::get_f64(self, name)
    }

    fn get_i64(&self, name: &str) -> Option<i64> {
        MpvHandle::get_i64(self, name)
    }

    fn get_bool(&self, name: &str) -> Option<bool> {
        MpvHandle::get_flag(self, name)
    }

    fn get_str(&self, name: &str) -> Option<String> {
        MpvHandle::get_string(self, name)
    }

    fn set_f64(&self, name: &str, value: f64) -> Result<(), MpvError> {
        self.set_property_f64(name, value)
    }

    fn set_i64(&self, name: &str, value: i64) -> Result<(), MpvError> {
        self.set_property_i64(name, value)
    }

    fn set_bool(&self, name: &str, value: bool) -> Result<(), MpvError> {
        self.set_property_flag(name, value)
    }

    fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError> {
        self.set_property_str(name, value)
    }

    fn command(&self, args: &[&str]) -> Result<(), MpvError> {
        MpvHandle::command(self, args)
    }
}

/// Logs a failed write with `tracing::warn!` and drops the error (E-46-088).
pub fn warn_on_error(result: Result<(), MpvError>, name: &str) {
    if let Err(err) = result {
        tracing::warn!(property = name, error = %err, "mpv write failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Records every write and answers reads from a map.
    #[derive(Default)]
    struct Fake {
        values: HashMap<&'static str, &'static str>,
        writes: RefCell<Vec<(String, String)>>,
    }

    impl Fake {
        fn record(&self, name: &str, value: String) -> Result<(), MpvError> {
            self.writes.borrow_mut().push((name.to_owned(), value));
            Ok(())
        }
    }

    impl MpvProps for Fake {
        fn get_f64(&self, name: &str) -> Option<f64> {
            self.values.get(name)?.parse().ok()
        }

        fn get_i64(&self, name: &str) -> Option<i64> {
            self.values.get(name)?.parse().ok()
        }

        fn get_bool(&self, name: &str) -> Option<bool> {
            self.values.get(name)?.parse().ok()
        }

        fn get_str(&self, name: &str) -> Option<String> {
            self.values.get(name).map(|v| (*v).to_owned())
        }

        fn set_f64(&self, name: &str, value: f64) -> Result<(), MpvError> {
            self.record(name, value.to_string())
        }

        fn set_i64(&self, name: &str, value: i64) -> Result<(), MpvError> {
            self.record(name, value.to_string())
        }

        fn set_bool(&self, name: &str, value: bool) -> Result<(), MpvError> {
            self.record(name, value.to_string())
        }

        fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError> {
            self.record(name, value.to_owned())
        }

        fn command(&self, args: &[&str]) -> Result<(), MpvError> {
            if args.is_empty() {
                return Err(MpvError::EmptyCommand);
            }
            self.record("command", args.join(" "))
        }
    }

    #[test]
    fn typed_writes_record_the_exact_value_strings() {
        let fake = Fake::default();
        fake.set_bool("sub-bold", false).unwrap();
        fake.set_str("sub-ass-override", "force").unwrap();
        fake.set_i64("sub-font-size", 55).unwrap();
        fake.set_f64("speed", 1.5).unwrap();
        assert_eq!(
            *fake.writes.borrow(),
            vec![
                ("sub-bold".to_owned(), "false".to_owned()),
                ("sub-ass-override".to_owned(), "force".to_owned()),
                ("sub-font-size".to_owned(), "55".to_owned()),
                ("speed".to_owned(), "1.5".to_owned()),
            ]
        );
    }

    #[test]
    fn a_missing_read_is_none_so_the_caller_can_fall_back() {
        let fake = Fake::default();
        assert_eq!(fake.get_f64("sub-scale"), None);
        let pref = 1.0;
        assert_eq!(fake.get_f64("sub-scale").unwrap_or(pref), 1.0);
    }

    #[test]
    fn commands_are_joined_in_order() {
        let fake = Fake::default();
        fake.command(&["script-binding", "stats/display-stats-toggle"])
            .unwrap();
        assert_eq!(
            fake.writes.borrow()[0].1,
            "script-binding stats/display-stats-toggle"
        );
        assert!(fake.command(&[]).is_err());
    }

    #[test]
    fn a_failed_write_is_logged_and_does_not_panic() {
        warn_on_error(Err(MpvError::EmptyCommand), "sub-pos");
        warn_on_error(Ok(()), "sub-pos");
    }
}
