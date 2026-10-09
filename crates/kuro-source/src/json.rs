//! Encoding and decoding of source JSON payloads (`JsonExtensions.kt`, `defaultJson()`, T-15a-037, T-16a-047).
//!
//! The Kotlin options are `ignoreUnknownKeys = true` and `explicitNulls = false`. The serde rules that give the same
//! behaviour:
//! - read: unknown keys are skipped (serde default). A `null` on a non-`Option` field reads as that field's default
//!   ([`null_as_default`]), so it is the same as a missing key. A missing `Option` field reads as `None`.
//! - write: a `None` field is not written. Every optional field of a model carries `skip_serializing_if =
//!   "Option::is_none"`, and [`encode_source_json`] writes what serde gives it.
//!
//! A body that is not JSON, or that is empty, is a `Parse` error.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::SourceError;

/// Decodes a source payload. See the module comment for the rules.
pub fn decode_source_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, SourceError> {
    serde_json::from_slice(body).map_err(|err| SourceError::Parse(err.to_string()))
}

/// Encodes a value the way a source payload is written. See the module comment for the rules.
pub fn encode_source_json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(value)
}

/// Reads `null` as the default of a non-`Option` field, so a `null` is the same as a missing key.
pub fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Sample {
        title: String,
        #[serde(default)]
        note: Option<String>,
        count: Option<i32>,
    }

    #[test]
    fn an_unknown_key_is_ignored() {
        let body = br#"{"title":"A","count":2,"extra":{"deep":true}}"#;
        let parsed: Sample = decode_source_json(body).unwrap();
        assert_eq!(parsed, Sample { title: "A".into(), note: None, count: Some(2) });
    }

    #[test]
    fn an_absent_optional_key_is_none() {
        let parsed: Sample = decode_source_json(br#"{"title":"A"}"#).unwrap();
        assert_eq!(parsed.count, None);
        assert_eq!(parsed.note, None);
    }

    #[test]
    fn an_empty_or_broken_body_is_a_parse_error() {
        assert!(matches!(decode_source_json::<Sample>(b""), Err(SourceError::Parse(_))));
        assert!(matches!(decode_source_json::<Sample>(b"{"), Err(SourceError::Parse(_))));
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct Shape {
        #[serde(deserialize_with = "null_as_default")]
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    }

    #[test]
    fn a_null_on_a_non_optional_field_reads_as_the_default() {
        let parsed: Shape = decode_source_json(br#"{"label":null,"hint":null}"#).unwrap();
        assert_eq!(parsed, Shape { label: String::new(), hint: None });
    }

    #[test]
    fn a_none_field_is_not_written_and_a_set_one_is() {
        let none = String::from_utf8(encode_source_json(&Shape { label: "a".into(), hint: None }).unwrap()).unwrap();
        assert_eq!(none, r#"{"label":"a"}"#);
        let set = String::from_utf8(encode_source_json(&Shape { label: "a".into(), hint: Some("b".into()) }).unwrap())
            .unwrap();
        assert_eq!(set, r#"{"label":"a","hint":"b"}"#);
    }
}
