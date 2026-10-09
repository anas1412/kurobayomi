//! JPEG XL stills through `jxl-oxide`'s `image` integration.

use std::io::Cursor;

use image::ImageDecoder;
use jxl_oxide::integration::JxlDecoder;

pub(super) fn open(bytes: &[u8]) -> Result<Box<dyn ImageDecoder + '_>, String> {
    JxlDecoder::new(Cursor::new(bytes))
        .map(|d| Box::new(d) as Box<dyn ImageDecoder + '_>)
        .map_err(|e| e.to_string())
}
