//! Page decode (T-43b-014, T-44a-050, decode half). The decode itself is pure (the `image` crate, no GTK) and runs on the
//! blocking pool. The page view cuts the RGBA into a tile pyramid (`tiles`) and turns the visible tiles into
//! `gdk::Texture`s on the GTK thread.
//!
//! `needs_tiling` is the predicate for a page too large for one texture; the pyramid itself is built for every page
//! and has one level when the page fits.

/// The hardware bitmap threshold default (`GLUtil.SAFE_TEXTURE_LIMIT`).
pub const DEFAULT_TILE_THRESHOLD: u32 = 2048;

/// The largest page edge that is kept as decoded (D-9, T-43b-021). Pages up to this size upload without a downscale
/// (as tiles of at most [`DEFAULT_TILE_THRESHOLD`]); a longer edge is shrunk to this by [`limit_edge`].
pub const MAX_TEXTURE_EDGE: u32 = 16384;

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

/// Shrinks a page to `width` pixels wide, keeping its aspect ratio, when it is wider than that. A narrower page is never
/// enlarged (B§8.4: webtoon pages decode at the view width). A page that cannot be rebuilt comes back empty, which the
/// page view shows as a load error.
pub fn fit_to_width(page: DecodedPage, width: u32) -> DecodedPage {
    if width == 0 || page.width <= width {
        return page;
    }
    let height = (u64::from(page.height) * u64::from(width) / u64::from(page.width)).max(1) as u32;
    let DecodedPage { width: src_w, height: src_h, rgba } = page;
    match image::RgbaImage::from_raw(src_w, src_h, rgba) {
        Some(img) => {
            let out = image::imageops::resize(&img, width, height, image::imageops::FilterType::Triangle);
            DecodedPage { width: out.width(), height: out.height(), rgba: out.into_raw() }
        }
        None => DecodedPage { width: 0, height: 0, rgba: Vec::new() },
    }
}

/// Shrinks a page whose longer edge is above `max_edge` so that edge is `max_edge`, keeping the aspect ratio (D-9). A page
/// within the limit is returned as it is. A page that cannot be rebuilt comes back empty, which the page view shows as a
/// load error.
pub fn limit_edge(page: DecodedPage, max_edge: u32) -> DecodedPage {
    let longest = page.width.max(page.height);
    if max_edge == 0 || longest <= max_edge {
        return page;
    }
    let shrink = |edge: u32| (u64::from(edge) * u64::from(max_edge) / u64::from(longest)).max(1) as u32;
    let (width, height) = (shrink(page.width), shrink(page.height));
    let DecodedPage { width: src_w, height: src_h, rgba } = page;
    match image::RgbaImage::from_raw(src_w, src_h, rgba) {
        Some(img) => {
            let out = image::imageops::resize(&img, width, height, image::imageops::FilterType::Triangle);
            DecodedPage { width: out.width(), height: out.height(), rgba: out.into_raw() }
        }
        None => DecodedPage { width: 0, height: 0, rgba: Vec::new() },
    }
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
    fn a_wide_page_is_shrunk_to_the_view_width_and_a_narrow_one_is_kept() {
        let wide = DecodedPage { width: 400, height: 200, rgba: vec![7; 400 * 200 * 4] };
        let shrunk = fit_to_width(wide, 200);
        assert_eq!((shrunk.width, shrunk.height), (200, 100));
        assert_eq!(shrunk.rgba.len(), 200 * 100 * 4);
        let narrow = DecodedPage { width: 100, height: 50, rgba: vec![7; 100 * 50 * 4] };
        let kept = fit_to_width(narrow, 800);
        assert_eq!((kept.width, kept.height), (100, 50), "never enlarged");
        let zero_view = DecodedPage { width: 10, height: 10, rgba: vec![0; 400] };
        assert_eq!(fit_to_width(zero_view, 0).width, 10, "no view width, no change");
    }

    #[test]
    fn a_page_up_to_the_texture_limit_is_kept_and_a_longer_one_is_shrunk_to_it() {
        let at_limit = DecodedPage { width: MAX_TEXTURE_EDGE, height: 2, rgba: vec![9; MAX_TEXTURE_EDGE as usize * 2 * 4] };
        let kept = limit_edge(at_limit, MAX_TEXTURE_EDGE);
        assert_eq!((kept.width, kept.height), (MAX_TEXTURE_EDGE, 2), "16384 px uploads as it is");
        let over = DecodedPage { width: 4, height: 40, rgba: vec![9; 4 * 40 * 4] };
        let shrunk = limit_edge(over, 20);
        assert_eq!((shrunk.width, shrunk.height), (2, 20), "the long edge is the limit and the ratio stays");
        assert_eq!(shrunk.rgba.len(), 2 * 20 * 4);
        let thin = DecodedPage { width: 1, height: 100, rgba: vec![9; 400] };
        assert_eq!(limit_edge(thin, 10).width, 1, "a side never shrinks to nothing");
        let wide = DecodedPage { width: 40, height: 4, rgba: vec![9; 40 * 4 * 4] };
        assert_eq!(limit_edge(wide, 0).width, 40, "no limit, no change");
    }

    #[test]
    fn the_texture_limit_is_the_d9_value_above_the_tile_threshold() {
        assert_eq!(MAX_TEXTURE_EDGE, 16384);
        assert!(needs_tiling(MAX_TEXTURE_EDGE, 10, DEFAULT_TILE_THRESHOLD), "a page at the limit is tiled, not downscaled");
    }

    #[test]
    fn the_default_load_state_is_empty() {
        assert_eq!(LoadState::default(), LoadState::Empty);
    }
}
