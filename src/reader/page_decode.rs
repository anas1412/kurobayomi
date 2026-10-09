//! Page decode (T-43b-014, decode half). The decode itself is pure (the `image` crate, no GTK) and runs on the blocking
//! pool; the page view turns the RGBA bytes into a `gdk::Texture` on the GTK thread.
//!
//! `needs_tiling` is the decision the tiling work (T-43b-021, P1) will use. P0 always uploads one texture (topic
//! section 6).

/// The hardware bitmap threshold default (`GLUtil.SAFE_TEXTURE_LIMIT`).
pub const DEFAULT_TILE_THRESHOLD: u32 = 2048;

/// Why a page could not be decoded (E-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The bytes start like an image but do not decode, or are empty.
    Corrupt,
    /// The bytes are not a format this reader knows.
    UnsupportedFormat,
}

/// Where a page is in its load. Loading hides the page and shows the indicator; Error emits `image-load-error`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoadState {
    #[default]
    Empty,
    Loading,
    Ready,
    Error,
}

/// A decoded page: straight (not premultiplied) RGBA8, rows of `width * 4` bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Whether a page this large must be tiled rather than uploaded whole (E-06): the longer edge exceeds the threshold.
pub fn needs_tiling(width: u32, height: u32, threshold: u32) -> bool {
    width.max(height) > threshold
}

/// Decodes page bytes to RGBA8. An empty input is `Corrupt`. Bytes with no known image signature are
/// `UnsupportedFormat`. Bytes with a known signature that fail to decode, including a truncated file, are `Corrupt`.
pub fn decode_rgba(bytes: &[u8]) -> Result<DecodedPage, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError::Corrupt);
    }
    if image::guess_format(bytes).is_err() {
        return Err(DecodeError::UnsupportedFormat);
    }
    let rgba = image::load_from_memory(bytes).map_err(|_| DecodeError::Corrupt)?.to_rgba8();
    Ok(DecodedPage { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn encoded(format: image::ImageFormat, width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, format).expect("encode");
        out.into_inner()
    }

    #[test]
    fn e06_tiling_boundary_is_strictly_above_the_threshold() {
        assert!(!needs_tiling(2048, 100, DEFAULT_TILE_THRESHOLD));
        assert!(needs_tiling(2049, 100, DEFAULT_TILE_THRESHOLD));
        assert!(needs_tiling(100, 2049, DEFAULT_TILE_THRESHOLD));
    }

    #[test]
    fn a_png_decodes_to_rgba_with_its_size() {
        let page = decode_rgba(&encoded(image::ImageFormat::Png, 3, 2)).expect("png");
        assert_eq!((page.width, page.height), (3, 2));
        assert_eq!(page.rgba.len(), 3 * 2 * 4);
        assert_eq!(&page.rgba[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn e07_a_truncated_jpeg_is_corrupt_not_a_panic() {
        let jpeg = encoded(image::ImageFormat::Jpeg, 16, 16);
        // Keep only the signature and a few header bytes.
        assert_eq!(decode_rgba(&jpeg[..4]), Err(DecodeError::Corrupt));
    }

    #[test]
    fn e07_unknown_bytes_are_unsupported_and_empty_input_is_corrupt() {
        assert_eq!(decode_rgba(b"this is plain text, not an image"), Err(DecodeError::UnsupportedFormat));
        assert_eq!(decode_rgba(&[]), Err(DecodeError::Corrupt));
    }

    #[test]
    fn the_default_load_state_is_empty() {
        assert_eq!(LoadState::default(), LoadState::Empty);
    }
}
