//! Colour scheme through the XDG desktop portal: the `org.freedesktop.appearance` `color-scheme`
//! setting, read once and then followed through the `SettingChanged` signal.
//!
//! Callers keep GTK's own value when [`read_color_scheme`] returns `None` (no portal, or no
//! setting). The GTK layer applies the result; this module does not touch GTK.

use futures_util::{Stream, StreamExt};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, Message, Proxy};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Settings";
const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";

/// The colour scheme the desktop asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScheme {
    /// Portal value `0`, or any value the portal does not define.
    NoPreference,
    /// Portal value `1`.
    PreferDark,
    /// Portal value `2`.
    PreferLight,
}

impl ColorScheme {
    /// Decodes a portal setting value. Nested variants are unwrapped. A value that is not a
    /// `u32` gives `None`, so the caller keeps its own value.
    pub fn from_portal(value: &Value<'_>) -> Option<Self> {
        let mut inner = value;
        while let Value::Value(boxed) = inner {
            inner = boxed;
        }
        match inner {
            Value::U32(1) => Some(Self::PreferDark),
            Value::U32(2) => Some(Self::PreferLight),
            Value::U32(_) => Some(Self::NoPreference),
            _ => None,
        }
    }
}

/// Reads the current colour scheme. `None` when the portal or the setting is unavailable.
pub async fn read_color_scheme(conn: &Connection) -> Option<ColorScheme> {
    let proxy = settings_proxy(conn).await.ok()?;
    let value: OwnedValue = proxy.call("Read", &(NAMESPACE, KEY)).await.ok()?;
    ColorScheme::from_portal(&value)
}

/// Follows `SettingChanged` for the colour scheme. Each item is the new scheme. Signals for other
/// settings are skipped.
pub async fn color_scheme_changes(
    conn: &Connection,
) -> zbus::Result<impl Stream<Item = ColorScheme>> {
    let proxy = settings_proxy(conn).await?;
    let signals = proxy.receive_signal("SettingChanged").await?;
    Ok(signals.filter_map(|msg| std::future::ready(changed_scheme(&msg))))
}

fn changed_scheme(msg: &Message) -> Option<ColorScheme> {
    let (namespace, key, value) = msg
        .body()
        .deserialize::<(String, String, OwnedValue)>()
        .ok()?;
    if namespace != NAMESPACE || key != KEY {
        return None;
    }
    ColorScheme::from_portal(&value)
}

async fn settings_proxy(conn: &Connection) -> zbus::Result<Proxy<'static>> {
    Proxy::new(conn, DESTINATION, PATH, INTERFACE).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_values_map_to_schemes() {
        assert_eq!(ColorScheme::from_portal(&Value::U32(0)), Some(ColorScheme::NoPreference));
        assert_eq!(ColorScheme::from_portal(&Value::U32(1)), Some(ColorScheme::PreferDark));
        assert_eq!(ColorScheme::from_portal(&Value::U32(2)), Some(ColorScheme::PreferLight));
    }

    #[test]
    fn an_undefined_portal_value_is_no_preference() {
        assert_eq!(ColorScheme::from_portal(&Value::U32(9)), Some(ColorScheme::NoPreference));
    }

    #[test]
    fn nested_variants_are_unwrapped() {
        let nested = Value::Value(Box::new(Value::U32(1)));
        assert_eq!(ColorScheme::from_portal(&nested), Some(ColorScheme::PreferDark));
    }

    #[test]
    fn a_non_integer_value_is_unreadable() {
        assert_eq!(ColorScheme::from_portal(&Value::from("dark")), None);
    }

    #[test]
    fn a_setting_changed_signal_decodes_only_the_colour_scheme() {
        let build = |ns: &str, key: &str, value: u32| {
            Message::signal("/org/freedesktop/portal/desktop", INTERFACE, "SettingChanged")
                .expect("signal builder")
                .build(&(ns, key, Value::U32(value)))
                .expect("signal body")
        };
        assert_eq!(
            changed_scheme(&build(NAMESPACE, KEY, 1)),
            Some(ColorScheme::PreferDark)
        );
        assert_eq!(changed_scheme(&build("org.gnome.desktop", KEY, 1)), None);
        assert_eq!(changed_scheme(&build(NAMESPACE, "contrast", 1)), None);
    }
}
