//! The empty JSON object (port of `JsonObject.kt`: `JsonObjectEmpty` and `JsonObjectEmptyBytes`).

/// The two ASCII bytes of `{}` (`0x7B 0x7D`).
pub const EMPTY_OBJECT_BYTES: &[u8] = b"{}";

/// The empty JSON object as text, `{}`.
pub fn empty_object() -> &'static str {
    "{}"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_object_serialises_to_braces() {
        assert_eq!(empty_object(), "{}");
        assert_eq!(serde_json::to_string(&serde_json::Map::new()).unwrap(), empty_object());
        assert_eq!(serde_json::to_vec(&serde_json::Map::new()).unwrap(), EMPTY_OBJECT_BYTES);
    }

    #[test]
    fn byte_constant_is_the_two_ascii_braces() {
        assert_eq!(EMPTY_OBJECT_BYTES, b"{}");
        assert_eq!(EMPTY_OBJECT_BYTES, [0x7B, 0x7D]);
        assert_eq!(empty_object().as_bytes(), EMPTY_OBJECT_BYTES);
    }
}
