//! Stored values and the typed codec. A stored value keeps its kind, as in `SharedPreferences`:
//! an `Int` and a `Long` are different kinds even when they hold the same number.

use std::collections::BTreeSet;

/// A stored preference value.
#[derive(Clone, Debug, PartialEq)]
pub enum PrefValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Str(String),
    Bool(bool),
    Set(BTreeSet<String>),
}

/// Result of decoding a stored value into a typed preference.
#[derive(Debug, PartialEq)]
pub enum Decoded<T> {
    Ok(T),
    /// Stored as another kind (Kotlin's `ClassCastException`). The reader deletes the key.
    WrongType,
    /// Right kind, but not a valid value, such as an unknown enum name. The reader gets the
    /// default and the stored value stays.
    Invalid,
}

/// A type that can be stored as a [`PrefValue`].
pub trait PrefType: Clone + 'static {
    fn encode(&self) -> PrefValue;
    fn decode(v: &PrefValue) -> Decoded<Self>;
}

macro_rules! primitive {
    ($ty:ty, $variant:ident) => {
        impl PrefType for $ty {
            fn encode(&self) -> PrefValue {
                PrefValue::$variant(self.clone())
            }

            fn decode(v: &PrefValue) -> Decoded<Self> {
                match v {
                    PrefValue::$variant(x) => Decoded::Ok(x.clone()),
                    _ => Decoded::WrongType,
                }
            }
        }
    };
}

primitive!(bool, Bool);
primitive!(i32, Int);
primitive!(i64, Long);
primitive!(f32, Float);
primitive!(String, Str);
primitive!(BTreeSet<String>, Set);

/// Declares an enum stored by its variant name, the same as Kotlin's `Enum.name`.
///
/// Variant names must be the Kotlin constant names, so they are not renamed. An unknown name
/// decodes as [`values::Decoded::Invalid`].
#[macro_export]
macro_rules! pref_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The stored name.
            pub fn name(self) -> &'static str {
                match self {
                    $($name::$variant => stringify!($variant)),+
                }
            }

            /// The variant stored under `s`.
            pub fn from_name(s: &str) -> Option<$name> {
                match s {
                    $(stringify!($variant) => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl $crate::values::PrefType for $name {
            fn encode(&self) -> $crate::values::PrefValue {
                $crate::values::PrefValue::Str(self.name().to_owned())
            }

            fn decode(v: &$crate::values::PrefValue) -> $crate::values::Decoded<Self> {
                match v {
                    $crate::values::PrefValue::Str(s) => match Self::from_name(s) {
                        Some(x) => $crate::values::Decoded::Ok(x),
                        None => $crate::values::Decoded::Invalid,
                    },
                    _ => $crate::values::Decoded::WrongType,
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    pref_enum! {
        pub enum ProxyMode { None, Peers }
    }

    #[test]
    fn primitives_round_trip_and_reject_other_kinds() {
        assert_eq!(i32::decode(&5i32.encode()), Decoded::Ok(5));
        assert_eq!(i64::decode(&5i64.encode()), Decoded::Ok(5));
        assert_eq!(f32::decode(&0.85f32.encode()), Decoded::Ok(0.85));
        assert_eq!(bool::decode(&true.encode()), Decoded::Ok(true));
        assert_eq!(
            String::decode(&"abc".to_owned().encode()),
            Decoded::Ok("abc".to_owned())
        );
        assert_eq!(i32::decode(&PrefValue::Long(5)), Decoded::WrongType);
        assert_eq!(i64::decode(&PrefValue::Int(5)), Decoded::WrongType);
        assert_eq!(
            bool::decode(&PrefValue::Str("true".into())),
            Decoded::WrongType
        );
    }

    #[test]
    fn enum_stores_the_constant_name() {
        assert_eq!(ProxyMode::Peers.encode(), PrefValue::Str("Peers".into()));
        assert_eq!(
            ProxyMode::decode(&PrefValue::Str("Peers".into())),
            Decoded::Ok(ProxyMode::Peers)
        );
        assert_eq!(ProxyMode::ALL, &[ProxyMode::None, ProxyMode::Peers]);
    }

    #[test]
    fn enum_unknown_name_is_invalid_and_wrong_kind_is_wrong_type() {
        assert_eq!(
            ProxyMode::decode(&PrefValue::Str("Nope".into())),
            Decoded::Invalid
        );
        assert_eq!(ProxyMode::decode(&PrefValue::Int(1)), Decoded::WrongType);
    }
}
