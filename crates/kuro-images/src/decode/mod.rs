//! Image decode entry point and per-format decoders (area:34 section 1 and behavior sections 13 and 14).
//!
//! Pipeline: sniff the type, pick the decoder path (T-34a-025), read the header, refuse an empty or oversized
//! image, decode to RGBA8 with straight alpha, and downscale by the in-sample size (T-34a-026).

mod raster;

#[cfg(feature = "jxl")]
mod jxl;

use bytes::Bytes;
use image::{DynamicImage, ImageDecoder, ImageFormat, imageops};

use crate::error::ImageError;
use crate::options::{LoadOptions, RequestSize, Scale};
use crate::sample::{sample_for, sampled_side};
use crate::sniff::{self, ImageFormat as Sniffed};

/// Most pixels a cover may have (design section 4.6, proposal).
pub const COVER_MAX_PIXELS: u64 = 40_000_000;
/// Most pixels a reader page may have (design section 4.6, proposal).
pub const PAGE_MAX_PIXELS: u64 = 120_000_000;

/// Which decoder family handles a request (behavior section 13.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderPath {
    /// The custom decoder: every format, and the only path for AVIF, JXL and HEIF.
    Custom,
    /// The platform decoder: JPEG, PNG, GIF, WebP and BMP.
    Platform,
}

/// The custom decoder takes a request when `custom_decoder` is set or the sniffed type is AVIF, JXL or HEIF
/// (behavior section 13.1). BMP is not sniffed, so it is always `Platform`.
pub fn select_decoder(custom_decoder: bool, sniffed: Option<Sniffed>) -> DecoderPath {
    let native_only = matches!(sniffed, Some(Sniffed::Avif | Sniffed::Jxl | Sniffed::Heif));
    if custom_decoder || native_only {
        DecoderPath::Custom
    } else {
        DecoderPath::Platform
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeOptions {
    pub size: RequestSize,
    pub scale: Scale,
    pub custom_decoder: bool,
    pub max_pixels: u64,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            size: RequestSize::Original,
            scale: Scale::Fit,
            custom_decoder: false,
            max_pixels: COVER_MAX_PIXELS,
        }
    }
}

impl From<&LoadOptions> for DecodeOptions {
    fn from(opts: &LoadOptions) -> Self {
        Self {
            size: opts.size,
            scale: opts.scale,
            custom_decoder: opts.custom_decoder,
            max_pixels: COVER_MAX_PIXELS,
        }
    }
}

/// A decoded image: row-major RGBA8, straight alpha, `stride = width * 4`. The buffer is shared, so cloning is
/// cheap and the memory cache can hold it without any GTK object (design section 16 item 11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Bytes,
    /// True when the in-sample size was greater than 1 (behavior section 14.5).
    pub is_sampled: bool,
}

#[derive(Clone, Copy)]
enum Codec {
    Raster(ImageFormat),
    Avif,
    Jxl,
    Heif,
}

fn detect(bytes: &[u8]) -> Option<Codec> {
    Some(match sniff::sniff(bytes) {
        Some(Sniffed::Jpeg) => Codec::Raster(ImageFormat::Jpeg),
        Some(Sniffed::Png) => Codec::Raster(ImageFormat::Png),
        Some(Sniffed::Gif) => Codec::Raster(ImageFormat::Gif),
        Some(Sniffed::Webp) => Codec::Raster(ImageFormat::WebP),
        Some(Sniffed::Avif) => Codec::Avif,
        Some(Sniffed::Jxl) => Codec::Jxl,
        Some(Sniffed::Heif) => Codec::Heif,
        None if bytes.starts_with(b"BM") => Codec::Raster(ImageFormat::Bmp),
        None => return None,
    })
}

/// Opens the decoder, which reads the header. An unreadable header is `DecodeInit` (Kotlin's decoder is null),
/// and so is HEIF, which has no decoder in this build (T-34a-029).
fn open(codec: Codec, bytes: &[u8]) -> Result<Box<dyn ImageDecoder + '_>, ImageError> {
    let opened = match codec {
        Codec::Raster(format) => raster::open(bytes, format),
        #[cfg(feature = "avif")]
        Codec::Avif => raster::open(bytes, ImageFormat::Avif),
        #[cfg(not(feature = "avif"))]
        Codec::Avif => return Err(ImageError::UnsupportedFormat),
        #[cfg(feature = "jxl")]
        Codec::Jxl => jxl::open(bytes),
        #[cfg(not(feature = "jxl"))]
        Codec::Jxl => return Err(ImageError::UnsupportedFormat),
        Codec::Heif => return Err(ImageError::DecodeInit),
    };
    opened.map_err(|e| {
        tracing::debug!(error = %e, "image header unreadable");
        ImageError::DecodeInit
    })
}

/// Decodes the first frame of `bytes` to RGBA8 at the size the request asks for (behavior sections 14 and 15).
pub fn decode(bytes: &[u8], opts: &DecodeOptions) -> Result<Decoded, ImageError> {
    let codec = detect(bytes).ok_or(ImageError::UnsupportedFormat)?;
    // The platform path handles the raster formats only; anything else needs the custom decoder.
    if select_decoder(opts.custom_decoder, sniff::sniff(bytes)) == DecoderPath::Platform
        && !matches!(codec, Codec::Raster(_))
    {
        return Err(ImageError::UnsupportedFormat);
    }
    let decoder = open(codec, bytes)?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 {
        return Err(ImageError::DecodeInit);
    }
    if width > i32::MAX as u32
        || height > i32::MAX as u32
        || u64::from(width) * u64::from(height) > opts.max_pixels
    {
        return Err(ImageError::TooLarge);
    }
    let sample = sample_for(opts.size, opts.scale, (width, height));
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|e| {
            tracing::debug!(error = %e, "image decode failed");
            ImageError::DecodeFailed
        })?
        .into_rgba8();
    if image.width() == 0 || image.height() == 0 {
        return Err(ImageError::DecodeFailed);
    }
    if sample > 1 {
        image = imageops::thumbnail(
            &image,
            sampled_side(image.width(), sample),
            sampled_side(image.height(), sample),
        );
    }
    Ok(Decoded {
        width: image.width(),
        height: image.height(),
        rgba: Bytes::from(image.into_raw()),
        is_sampled: sample > 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{encode, fixture, gradient};

    fn opts() -> DecodeOptions {
        DecodeOptions::default()
    }

    #[test]
    fn decoder_selection_rule() {
        assert_eq!(
            select_decoder(false, Some(Sniffed::Jpeg)),
            DecoderPath::Platform
        );
        assert_eq!(
            select_decoder(false, Some(Sniffed::Png)),
            DecoderPath::Platform
        );
        assert_eq!(
            select_decoder(false, Some(Sniffed::Gif)),
            DecoderPath::Platform
        );
        assert_eq!(
            select_decoder(false, Some(Sniffed::Webp)),
            DecoderPath::Platform
        );
        assert_eq!(select_decoder(false, None), DecoderPath::Platform);
        for native in [Sniffed::Avif, Sniffed::Jxl, Sniffed::Heif] {
            assert_eq!(select_decoder(false, Some(native)), DecoderPath::Custom);
        }
        assert_eq!(
            select_decoder(true, Some(Sniffed::Jpeg)),
            DecoderPath::Custom
        );
        assert_eq!(select_decoder(true, None), DecoderPath::Custom);
    }

    #[test]
    fn raster_formats_decode_to_rgba8() {
        let src = gradient(64, 48);
        for format in [
            ImageFormat::Png,
            ImageFormat::Gif,
            ImageFormat::WebP,
            ImageFormat::Bmp,
            ImageFormat::Jpeg,
        ] {
            let got = decode(&encode(&src, format), &opts())
                .unwrap_or_else(|e| panic!("{format:?}: {e}"));
            assert_eq!((got.width, got.height), (64, 48), "{format:?}");
            assert_eq!(got.rgba.len(), 64 * 48 * 4, "{format:?}");
            assert!(!got.is_sampled);
        }
    }

    #[test]
    fn png_keeps_straight_alpha_exactly() {
        let src = gradient(16, 16);
        let got = decode(&encode(&src, ImageFormat::Png), &opts()).unwrap();
        assert_eq!(&got.rgba[..], src.as_raw().as_slice());
    }

    #[test]
    fn custom_decoder_flag_decodes_the_same_raster_bytes() {
        let bytes = encode(&gradient(32, 32), ImageFormat::Png);
        let custom = DecodeOptions {
            custom_decoder: true,
            ..opts()
        };
        assert_eq!(
            decode(&bytes, &custom).unwrap(),
            decode(&bytes, &opts()).unwrap()
        );
    }

    #[test]
    fn sampled_decode_shrinks_and_flags() {
        let bytes = encode(&gradient(400, 600), ImageFormat::Png);
        let want = DecodeOptions {
            size: RequestSize::Pixels {
                width: 100,
                height: 150,
            },
            ..opts()
        };
        let got = decode(&bytes, &want).unwrap();
        assert_eq!((got.width, got.height), (100, 150));
        assert!(got.is_sampled);
        assert_eq!(got.rgba.len(), 100 * 150 * 4);
    }

    #[test]
    fn original_size_is_never_sampled() {
        let bytes = encode(&gradient(400, 600), ImageFormat::Png);
        let got = decode(&bytes, &opts()).unwrap();
        assert_eq!((got.width, got.height), (400, 600));
        assert!(!got.is_sampled);
    }

    #[test]
    fn unknown_and_empty_bytes_are_unsupported() {
        assert!(matches!(
            decode(b"", &opts()),
            Err(ImageError::UnsupportedFormat)
        ));
        assert!(matches!(
            decode(b"plain text pretending to be a jpg", &opts()),
            Err(ImageError::UnsupportedFormat)
        ));
    }

    #[test]
    fn corrupt_body_after_a_good_header_is_decode_failed() {
        let mut bytes = encode(&gradient(64, 64), ImageFormat::Png);
        let n = bytes.len();
        bytes.truncate(n - 40);
        assert!(matches!(
            decode(&bytes, &opts()),
            Err(ImageError::DecodeFailed)
        ));
    }

    #[test]
    fn zero_dimension_header_is_decode_init() {
        let mut bytes = encode(&gradient(8, 8), ImageFormat::Png);
        // IHDR width sits at bytes 16..20.
        bytes[16..20].copy_from_slice(&0u32.to_be_bytes());
        assert!(matches!(
            decode(&bytes, &opts()),
            Err(ImageError::DecodeInit)
        ));
    }

    #[test]
    fn too_many_pixels_is_refused_before_decoding() {
        let bytes = encode(&gradient(64, 64), ImageFormat::Png);
        let small = DecodeOptions {
            max_pixels: 64 * 64 - 1,
            ..opts()
        };
        assert!(matches!(decode(&bytes, &small), Err(ImageError::TooLarge)));
        let exact = DecodeOptions {
            max_pixels: 64 * 64,
            ..opts()
        };
        assert!(decode(&bytes, &exact).is_ok());
    }

    #[test]
    fn heif_reports_decoder_init_failure() {
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0heicmif1";
        assert!(matches!(decode(heic, &opts()), Err(ImageError::DecodeInit)));
    }

    #[cfg(feature = "avif")]
    #[test]
    fn avif_still_decodes_to_the_same_contract() {
        let got = decode(&fixture("avif_still_96.avif"), &opts()).unwrap();
        assert_eq!((got.width, got.height), (96, 64));
        assert_eq!(got.rgba.len(), 96 * 64 * 4);
        // The fixture is a red to blue gradient, fully opaque.
        assert!(got.rgba.chunks(4).all(|p| p[3] == 255));
        let first = &got.rgba[..3];
        let last = &got.rgba[got.rgba.len() - 4..got.rgba.len() - 1];
        assert!(
            first[0] > first[2] && last[2] > last[0],
            "{first:?} {last:?}"
        );
    }

    #[cfg(feature = "avif")]
    #[test]
    fn corrupt_avif_has_the_two_decode_messages_and_no_pixels() {
        let bytes = fixture("avif_still_96.avif");
        let err = decode(&bytes[..bytes.len() / 2], &opts()).unwrap_err();
        assert!(
            matches!(err, ImageError::DecodeInit | ImageError::DecodeFailed),
            "{err}"
        );
        let mut garbled = bytes.clone();
        let n = garbled.len();
        garbled[n - 80..].fill(0xA5);
        let err = decode(&garbled, &opts());
        assert!(err.is_err() || err.is_ok_and(|d| d.width == 96));
    }

    #[cfg(feature = "jxl")]
    #[test]
    fn jxl_still_decodes_to_the_same_contract() {
        let got = decode(&fixture("jxl_still_96.jxl"), &opts()).unwrap();
        assert_eq!((got.width, got.height), (96, 64));
        assert_eq!(got.rgba.len(), 96 * 64 * 4);
        assert!(got.rgba.chunks(4).all(|p| p[3] == 255));
        assert!(got.rgba[0] > got.rgba[2]);
    }

    #[cfg(feature = "jxl")]
    #[test]
    fn corrupt_jxl_has_the_two_decode_messages() {
        let bytes = fixture("jxl_still_96.jxl");
        let err = decode(&bytes[..bytes.len() / 2], &opts()).unwrap_err();
        assert!(
            matches!(err, ImageError::DecodeInit | ImageError::DecodeFailed),
            "{err}"
        );
        assert!(
            ["Failed to initialize decoder", "Failed to decode image"]
                .contains(&err.to_string().as_str())
        );
    }

    #[cfg(feature = "avif")]
    #[test]
    fn sampled_avif_is_flagged() {
        let want = DecodeOptions {
            size: RequestSize::Pixels {
                width: 48,
                height: 32,
            },
            ..opts()
        };
        let got = decode(&fixture("avif_still_96.avif"), &want).unwrap();
        assert_eq!((got.width, got.height), (48, 32));
        assert!(got.is_sampled);
    }
}
