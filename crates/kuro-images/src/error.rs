//! `ImageError` and its user-facing message mapping (area:34 section 12).

use std::io;

use kuro_net::NetError;

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("Invalid image")]
    InvalidImage,
    #[error("No cover specified")]
    NoCoverSpecified,
    /// The message is the HTTP reason phrase, as Kotlin's `IOException(response.message)` (behavior section 7.3).
    #[error("{reason}")]
    Http { code: u16, reason: String },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Net(NetError),
    /// A timestamp write failed in the caller's store (behavior section 19.5).
    #[error("{0}")]
    Store(String),
    #[error("Canceled")]
    Cancelled,
    /// The header could not be read, or it reports a zero width or height (behavior section 14.2).
    #[error("Failed to initialize decoder")]
    DecodeInit,
    /// The decoder returned no pixels (behavior section 14.4). Also the second decode of a buffer (E-IMG-12).
    #[error("Failed to decode image")]
    DecodeFailed,
    /// No decoder for the sniffed type: unknown bytes, or a decoder feature that is not built (design section 4.3).
    #[error("Unsupported image format")]
    UnsupportedFormat,
    /// More pixels than the request allows, or a side that does not fit a texture (design section 4.6).
    #[error("Image too large")]
    TooLarge,
    /// The bytes to save are not an image by sniffing (behavior section 21.4).
    #[error("Not an image")]
    NotAnImage,
    /// The file could not be written under the save folder (behavior section 21.6, key `error_saving_picture`).
    #[error("Error saving picture")]
    SaveFailed,
}

impl From<NetError> for ImageError {
    fn from(err: NetError) -> Self {
        match err {
            NetError::Canceled => Self::Cancelled,
            other => Self::Net(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_match_kotlin_texts() {
        assert_eq!(ImageError::InvalidImage.to_string(), "Invalid image");
        assert_eq!(
            ImageError::NoCoverSpecified.to_string(),
            "No cover specified"
        );
        let http = ImageError::Http {
            code: 404,
            reason: "Not Found".into(),
        };
        assert_eq!(http.to_string(), "Not Found");
    }

    #[test]
    fn decode_messages_match_behavior_section_14() {
        assert_eq!(
            ImageError::DecodeInit.to_string(),
            "Failed to initialize decoder"
        );
        assert_eq!(
            ImageError::DecodeFailed.to_string(),
            "Failed to decode image"
        );
        assert_eq!(
            ImageError::UnsupportedFormat.to_string(),
            "Unsupported image format"
        );
        assert_eq!(ImageError::TooLarge.to_string(), "Image too large");
    }

    #[test]
    fn canceled_net_error_maps_to_cancelled() {
        assert!(matches!(
            ImageError::from(NetError::Canceled),
            ImageError::Cancelled
        ));
        assert!(matches!(
            ImageError::from(NetError::Timeout),
            ImageError::Net(NetError::Timeout)
        ));
    }
}
