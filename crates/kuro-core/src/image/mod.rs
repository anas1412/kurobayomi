//! Image helpers: magic-byte sniffing, MIME and extension mapping, decoding,
//! header-only dimensions, JPEG re-encoding, and the animated and wide-page
//! rules. Port of `core/common/.../util/system/ImageUtil.kt`.
//!
//! # Format decisions (OQ-13a-5, OQ-13a-6)
//!
//! - JPEG, PNG, GIF and WebP decode through the `image` crate; JXL through `jxl-oxide`.
//! - AVIF: the AV1 decoder is dav1d, pulled in by the `image` crate's `avif-native` feature. It is
//!   reachable only when the `heif` cargo feature is on; otherwise `Decode("feature disabled")`.
//! - HEIF: no decoder. `libheif` is not installed on the build host and `libheif-rs` is not a
//!   dependency. Even with the `heif` feature, HEIF stays an explicit `Decode("heif decoder not
//!   built")` error, and sniffing still names the kind so file-name checks keep working. Wiring a
//!   decoder means replacing that one arm of `format_of` and adding the binding behind `heif`.
//! - `heif` is off by default (area 13 overview, OQ-13a-5): HEIF and AVIF are rare in manga and
//!   anime sources. Turn it on if users read such files.
//! - libarchive binding (OQ-13a-6): the planned default stands, `libarchive3-sys` behind one safe
//!   wrapper under the `archive` feature; the spike T-13a-031 has not run, so no binding exists yet.

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbImage, RgbaImage};
use jxl_oxide::integration::JxlDecoder;

/// Size of the head buffer the Kotlin decoder reads (`ImageUtil.getImageType`).
const HEAD_LEN: usize = 32;
const JXL_CONTAINER: &[u8] = b"\0\0\0\x0cJXL \r\n\x87\n";

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unknown image format")]
    UnknownFormat,
    #[error("decode failed: {0}")]
    Decode(String),
    #[error("encode failed: {0}")]
    Encode(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Avif,
    Gif,
    /// Deliberately unsupported: no HEIF decoder (libheif is not installed). Decoding and
    /// `dimensions` return an explicit `Decode` error for HEIF.
    Heif,
    Jpeg,
    Jxl,
    Png,
    Webp,
}

impl ImageKind {
    pub const ALL: [ImageKind; 7] = [
        ImageKind::Avif,
        ImageKind::Gif,
        ImageKind::Heif,
        ImageKind::Jpeg,
        ImageKind::Jxl,
        ImageKind::Png,
        ImageKind::Webp,
    ];

    pub fn mime(self) -> &'static str {
        match self {
            ImageKind::Avif => "image/avif",
            ImageKind::Gif => "image/gif",
            ImageKind::Heif => "image/heif",
            ImageKind::Jpeg => "image/jpeg",
            ImageKind::Jxl => "image/jxl",
            ImageKind::Png => "image/png",
            ImageKind::Webp => "image/webp",
        }
    }

    /// JPEG is `jpg`, not `jpeg` (ImageUtil.ImageType).
    pub fn extension(self) -> &'static str {
        match self {
            ImageKind::Avif => "avif",
            ImageKind::Gif => "gif",
            ImageKind::Heif => "heif",
            ImageKind::Jpeg => "jpg",
            ImageKind::Jxl => "jxl",
            ImageKind::Png => "png",
            ImageKind::Webp => "webp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sniff {
    pub kind: ImageKind,
    pub animated: bool,
}

/// Magic-byte detection on the first 32 bytes, in Kotlin order. Shorter input is
/// zero-padded like the Kotlin buffer.
pub fn sniff(head: &[u8]) -> Option<Sniff> {
    let mut b = [0u8; HEAD_LEN];
    let n = head.len().min(HEAD_LEN);
    b[..n].copy_from_slice(&head[..n]);

    let still = |kind| Some(Sniff { kind, animated: false });
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return still(ImageKind::Jpeg);
    }
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        return still(ImageKind::Png);
    }
    // The table only checks RIFF; requiring WEBP also keeps WAV and AVI out.
    if b.starts_with(b"RIFF") && &b[8..12] == b"WEBP" {
        return Some(Sniff { kind: ImageKind::Webp, animated: webp_animated(&b) });
    }
    if b.starts_with(b"GIF8") {
        return Some(Sniff { kind: ImageKind::Gif, animated: true });
    }
    if b.starts_with(JXL_CONTAINER) || b.starts_with(&[0xFF, 0x0A]) {
        return still(ImageKind::Jxl);
    }
    if &b[4..8] == b"ftyp" {
        return ftyp_kind(&b).map(|kind| Sniff { kind, animated: false });
    }
    None
}

/// VP8X chunk at offset 12; its flags byte is at offset 20, animation bit 0x02.
fn webp_animated(b: &[u8; HEAD_LEN]) -> bool {
    &b[12..16] == b"VP8X" && b[20] & 0x02 != 0
}

/// ISO-BMFF brand search. Brands start at offset 8, step 4, and must lie within
/// the box (`headerSize`) and the 32-byte head.
fn ftyp_kind(b: &[u8; HEAD_LEN]) -> Option<ImageKind> {
    let header_size = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as i64;
    let max_offset = header_size.min(HEAD_LEN as i64) - 4;
    let mut offset = 8usize;
    while offset as i64 <= max_offset {
        // The table's `hei?` pattern does not match its own `heic` vector, so accept the
        // `hei` and `hev` prefixes (heic, heix, hevc, ...).
        let brand = &b[offset..offset + 4];
        if brand.starts_with(b"hei") || brand.starts_with(b"hev") {
            return Some(ImageKind::Heif);
        }
        if brand.starts_with(b"avi") {
            return Some(ImageKind::Avif);
        }
        offset += 4;
    }
    None
}

/// Exact, case-sensitive MIME match, no parameters.
pub fn kind_from_mime(mime: &str) -> Option<ImageKind> {
    ImageKind::ALL.into_iter().find(|k| k.mime() == mime)
}

/// ImageUtil.isImage: the extension is case-sensitive, and a name without a dot
/// is its own extension. Otherwise the head bytes must sniff as an image.
pub fn is_image_name(name: &str, head: impl FnOnce() -> Option<Vec<u8>>) -> bool {
    let ext = name.rsplit_once('.').map_or(name, |(_, e)| e);
    ImageKind::ALL.iter().any(|k| k.extension() == ext)
        || head().is_some_and(|h| sniff(&h).is_some())
}

/// ImageUtil.getExtensionFromMimeType: exact MIME first, then the sniffed
/// format, then `jpg`.
pub fn extension_for(mime: Option<&str>, head: impl FnOnce() -> Option<Vec<u8>>) -> &'static str {
    let kind = mime
        .and_then(kind_from_mime)
        .or_else(|| head().and_then(|h| sniff(&h)).map(|s| s.kind));
    kind.map_or("jpg", ImageKind::extension)
}

/// The `image` crate format for `kind`. `None` for JXL, which decodes through jxl-oxide.
/// AVIF and HEIF need the `heif` feature; HEIF has no decoder yet.
fn format_of(kind: ImageKind) -> Result<Option<ImageFormat>, ImageError> {
    Ok(Some(match kind {
        ImageKind::Avif | ImageKind::Heif if !cfg!(feature = "heif") => {
            return Err(ImageError::Decode("feature disabled".into()));
        }
        ImageKind::Avif => ImageFormat::Avif,
        ImageKind::Heif => return Err(ImageError::Decode("heif decoder not built".into())),
        ImageKind::Gif => ImageFormat::Gif,
        ImageKind::Jpeg => ImageFormat::Jpeg,
        ImageKind::Png => ImageFormat::Png,
        ImageKind::Webp => ImageFormat::WebP,
        ImageKind::Jxl => return Ok(None),
    }))
}

fn decode_err(e: image::ImageError) -> ImageError {
    ImageError::Decode(e.to_string())
}

fn jxl_decoder(bytes: &[u8]) -> Result<JxlDecoder<Cursor<&[u8]>>, ImageError> {
    JxlDecoder::new(Cursor::new(bytes)).map_err(decode_err)
}

/// Decodes the first frame to RGBA8.
pub fn decode_rgba(bytes: &[u8]) -> Result<RgbaImage, ImageError> {
    let kind = sniff(bytes).ok_or(ImageError::UnknownFormat)?.kind;
    let img = match format_of(kind)? {
        Some(format) => image::load_from_memory_with_format(bytes, format).map_err(decode_err)?,
        None => DynamicImage::from_decoder(jxl_decoder(bytes)?).map_err(decode_err)?,
    };
    Ok(img.into_rgba8())
}

/// Reads the header only; no pixel decode.
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32), ImageError> {
    let kind = sniff(bytes).ok_or(ImageError::UnknownFormat)?.kind;
    match format_of(kind)? {
        Some(format) => ImageReader::with_format(Cursor::new(bytes), format)
            .into_dimensions()
            .map_err(decode_err),
        None => Ok(jxl_decoder(bytes)?.dimensions()),
    }
}

/// JPEG has no alpha, so the alpha channel is dropped. Quality 100 is the
/// Kotlin re-encode setting.
pub fn encode_jpeg_q100(img: &RgbaImage) -> Result<Vec<u8>, ImageError> {
    let rgb: Vec<u8> = img.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect();
    let rgb = RgbImage::from_raw(img.width(), img.height(), rgb)
        .ok_or_else(|| ImageError::Encode("pixel buffer size mismatch".into()))?;
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 100)
        .encode_image(&rgb)
        .map_err(|e| ImageError::Encode(e.to_string()))?;
    Ok(out)
}

/// GIF is always animated-supported. WebP only when the VP8X animation bit is
/// set. HEIF and AVIF are never animated-supported (documented capability change).
pub fn is_animated_supported(sniff: Option<Sniff>) -> bool {
    match sniff {
        Some(Sniff { kind: ImageKind::Gif, .. }) => true,
        Some(Sniff { kind: ImageKind::Webp, animated }) => animated,
        _ => false,
    }
}

/// Double-page rule: strictly wider than tall.
pub fn is_wide(width: u32, height: u32) -> bool {
    width > height
}

/// Wide check from header dimensions only.
pub fn is_wide_image(bytes: &[u8]) -> Result<bool, ImageError> {
    dimensions(bytes).map(|(w, h)| is_wide(w, h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba};

    fn padded(prefix: &[u8]) -> Vec<u8> {
        let mut v = prefix.to_vec();
        v.resize(HEAD_LEN, 0);
        v
    }

    fn webp_head(flags: u8) -> Vec<u8> {
        let mut v = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0".to_vec();
        v.push(flags);
        padded(&v)
    }

    fn encode(img: DynamicImage, fmt: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, fmt).unwrap();
        out.into_inner()
    }

    fn png_with_alpha(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, y| Rgba([x as u8 * 40, y as u8 * 60, 128, 255]));
        encode(DynamicImage::ImageRgba8(img), ImageFormat::Png)
    }

    /// Header-only JPEG: the bytes end right after the SOS segment, so the scan data is missing.
    fn jpeg_headers_only(w: u32, h: u32) -> Vec<u8> {
        let full = encode(DynamicImage::ImageRgb8(RgbImage::new(w, h)), ImageFormat::Jpeg);
        let sos = full.windows(2).position(|p| p == [0xFF, 0xDA]).unwrap();
        let len = u16::from_be_bytes([full[sos + 2], full[sos + 3]]) as usize;
        full[..sos + 2 + len].to_vec()
    }

    fn still(kind: ImageKind) -> Option<Sniff> {
        Some(Sniff { kind, animated: false })
    }

    #[test]
    fn sniff_jpeg_png_gif() {
        assert_eq!(sniff(&padded(&[0xFF, 0xD8, 0xFF, 0xE0])), still(ImageKind::Jpeg));
        assert_eq!(sniff(&padded(&[0xFF, 0xD8, 0xFF])[..31]), still(ImageKind::Jpeg));
        assert_eq!(sniff(&padded(&[0xFF, 0xD8, 0x00])), None);
        assert_eq!(sniff(&padded(b"\x89PNG\r\n\x1a\n")), still(ImageKind::Png));
        assert_eq!(
            sniff(b"GIF89a"),
            Some(Sniff { kind: ImageKind::Gif, animated: true })
        );
    }

    #[test]
    fn sniff_webp_animation_bit() {
        assert_eq!(sniff(&webp_head(0x00)), still(ImageKind::Webp));
        assert_eq!(
            sniff(&webp_head(0x02)),
            Some(Sniff { kind: ImageKind::Webp, animated: true })
        );
        assert_eq!(sniff(&webp_head(0x10)), still(ImageKind::Webp));
        assert_eq!(sniff(&padded(b"RIFF\0\0\0\0WAVE")), None);
    }

    #[test]
    fn sniff_jxl_container_and_codestream() {
        assert_eq!(sniff(&padded(JXL_CONTAINER)), still(ImageKind::Jxl));
        assert_eq!(sniff(&padded(&[0xFF, 0x0A])), still(ImageKind::Jxl));
    }

    #[test]
    fn sniff_ftyp_brands() {
        assert_eq!(sniff(&padded(b"\0\0\0\x18ftypheic")), still(ImageKind::Heif));
        assert_eq!(sniff(&padded(b"\0\0\0\x1cftypavif")), still(ImageKind::Avif));
        assert_eq!(sniff(&padded(b"\0\0\0\x0cftyp")), None);
        // headerSize 8 gives maxOffset 4, so the brand at offset 8 is out of range.
        assert_eq!(sniff(&padded(b"\0\0\0\x08ftypheic")), None);
    }

    #[test]
    fn sniff_empty_and_unknown() {
        assert_eq!(sniff(&[]), None);
        assert_eq!(sniff(&padded(b"hello")), None);
    }

    #[test]
    fn mime_and_extension_mapping() {
        assert_eq!(kind_from_mime("image/jpeg"), Some(ImageKind::Jpeg));
        assert_eq!(kind_from_mime("image/jpg"), None);
        assert_eq!(kind_from_mime("IMAGE/JPEG"), None);
        assert_eq!(ImageKind::Jpeg.extension(), "jpg");
        assert_eq!(ImageKind::Jxl.mime(), "image/jxl");
        assert_eq!(ImageKind::ALL.len(), 7);
    }

    #[test]
    fn is_image_name_rules() {
        let no_sniff = || None;
        let jpeg_head = || Some(padded(&[0xFF, 0xD8, 0xFF]));
        assert!(!is_image_name("x.JPG", no_sniff));
        assert!(is_image_name("x.JPG", jpeg_head));
        assert!(is_image_name("x.jpg", no_sniff));
        assert!(!is_image_name("x.jpeg", no_sniff));
        assert!(is_image_name("x.jpeg", jpeg_head));
        assert!(is_image_name("x", jpeg_head));
    }

    #[test]
    fn extension_for_rules() {
        let no_head = || None;
        let png_head = || Some(padded(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(extension_for(Some("image/jpeg"), no_head), "jpg");
        assert_eq!(extension_for(Some("image/webp"), no_head), "webp");
        assert_eq!(extension_for(None, no_head), "jpg");
        assert_eq!(extension_for(None, png_head), "png");
        // Unknown MIME falls back to the sniff.
        assert_eq!(extension_for(Some("image/jpg"), png_head), "png");
        assert_eq!(extension_for(Some("image/jpg"), no_head), "jpg");
    }

    #[test]
    fn decode_png_roundtrip_keeps_pixels() {
        let img = decode_rgba(&png_with_alpha(4, 3)).unwrap();
        assert_eq!((img.width(), img.height()), (4, 3));
        assert_eq!(img.get_pixel(1, 2), &Rgba([40, 120, 128, 255]));
    }

    #[test]
    fn decode_jpeg_gif_webp_fixtures() {
        let rgb = DynamicImage::ImageRgb8(RgbImage::new(5, 2));
        let rgba = DynamicImage::ImageRgba8(RgbaImage::new(5, 2));
        for bytes in [
            encode(rgb, ImageFormat::Jpeg),
            encode(rgba.clone(), ImageFormat::Gif),
            encode(rgba, ImageFormat::WebP),
        ] {
            let img = decode_rgba(&bytes).unwrap();
            assert_eq!((img.width(), img.height()), (5, 2));
        }
    }

    #[test]
    fn decode_jxl_codestream_and_container_fixtures() {
        for bytes in [
            &include_bytes!("../../tests/fixtures/small.jxl")[..],
            &include_bytes!("../../tests/fixtures/small_container.jxl")[..],
        ] {
            assert_eq!(dimensions(bytes).unwrap(), (6, 4));
            let img = decode_rgba(bytes).unwrap();
            assert_eq!((img.width(), img.height()), (6, 4));
        }
    }

    #[test]
    fn truncated_jxl_is_a_decode_error_not_a_panic() {
        let full = include_bytes!("../../tests/fixtures/small_container.jxl");
        assert!(matches!(decode_rgba(&full[..40]), Err(ImageError::Decode(_))));
    }

    #[cfg(not(feature = "heif"))]
    #[test]
    fn avif_and_heif_report_feature_disabled_by_default() {
        for head in [&b"\0\0\0\x1cftypavif"[..], &b"\0\0\0\x18ftypheic"[..]] {
            match decode_rgba(&padded(head)) {
                Err(ImageError::Decode(msg)) => assert_eq!(msg, "feature disabled"),
                other => panic!("expected Decode, got {other:?}"),
            }
        }
    }

    #[test]
    fn heif_is_explicitly_unsupported() {
        let head = padded(b"\0\0\0\x18ftypheic");
        let want = if cfg!(feature = "heif") { "heif decoder not built" } else { "feature disabled" };
        assert!(matches!(decode_rgba(&head), Err(ImageError::Decode(m)) if m == want));
        assert!(matches!(dimensions(&head), Err(ImageError::Decode(m)) if m == want));
    }

    #[test]
    fn decode_errors() {
        assert!(matches!(decode_rgba(b"hello"), Err(ImageError::UnknownFormat)));
        let full = encode(DynamicImage::ImageRgb8(RgbImage::new(64, 64)), ImageFormat::Jpeg);
        assert!(matches!(decode_rgba(&full[..full.len() / 2]), Err(ImageError::Decode(_))));
    }

    #[test]
    fn dimensions_read_headers() {
        assert_eq!(dimensions(&png_with_alpha(4, 3)).unwrap(), (4, 3));
        assert!(matches!(dimensions(b"hello"), Err(ImageError::UnknownFormat)));
    }

    #[test]
    fn wide_detection_uses_headers_only() {
        let truncated = jpeg_headers_only(1200, 800);
        assert_eq!(dimensions(&truncated).unwrap(), (1200, 800));
        assert!(is_wide_image(&truncated).unwrap());
    }

    #[test]
    fn encode_jpeg_drops_alpha_and_keeps_size() {
        let src = decode_rgba(&png_with_alpha(4, 3)).unwrap();
        let jpg = encode_jpeg_q100(&src).unwrap();
        assert_eq!(sniff(&jpg).map(|s| s.kind), Some(ImageKind::Jpeg));
        let back = decode_rgba(&jpg).unwrap();
        assert_eq!((back.width(), back.height()), (4, 3));
    }

    #[test]
    fn animated_support_rules() {
        assert!(is_animated_supported(Some(Sniff { kind: ImageKind::Gif, animated: true })));
        assert!(is_animated_supported(Some(Sniff { kind: ImageKind::Webp, animated: true })));
        assert!(!is_animated_supported(Some(Sniff { kind: ImageKind::Webp, animated: false })));
        assert!(!is_animated_supported(Some(Sniff { kind: ImageKind::Heif, animated: true })));
        assert!(!is_animated_supported(Some(Sniff { kind: ImageKind::Avif, animated: true })));
        assert!(!is_animated_supported(Some(Sniff { kind: ImageKind::Png, animated: false })));
        assert!(!is_animated_supported(None));
    }

    #[test]
    fn wide_is_strictly_wider_than_tall() {
        assert!(is_wide(1200, 800));
        assert!(!is_wide(800, 800));
        assert!(!is_wide(800, 1200));
        // The edge-case table lists 1000 x 800 as false. That contradicts width > height and is not applied.
        assert!(is_wide(1000, 800));
    }
}
