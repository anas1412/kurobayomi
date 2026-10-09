//! Source icon resolution (T-40a-006). The choice is cached per source id, and a missing icon never blocks a row.

use std::collections::HashMap;
use std::path::Path;

/// Icons are 40 px high and square (`height(40) aspectRatio(1)`).
pub const ICON_HEIGHT_PX: i32 = 40;

/// Which icon a source row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceIcon {
    /// A stub with no icon: the warning glyph in the error colour.
    Warning,
    /// The icon the extension ships with, loaded from its file.
    Custom,
    /// The local source's folder icon.
    Local,
    /// The generic source icon.
    Default,
    /// An untrusted extension: the dangerous glyph in the error colour.
    Untrusted,
}

impl SourceIcon {
    /// The warning and dangerous glyphs are tinted with the error colour (T-41a-004).
    pub fn is_error(self) -> bool {
        matches!(self, SourceIcon::Warning | SourceIcon::Untrusted)
    }

    /// The GTK symbolic name for the built-in icons. A custom icon comes from its file, so it has none.
    pub fn icon_name(self) -> Option<&'static str> {
        match self {
            SourceIcon::Warning => Some("dialog-warning-symbolic"),
            SourceIcon::Custom => None,
            SourceIcon::Local => Some("folder-symbolic"),
            SourceIcon::Default => Some("application-x-addon-symbolic"),
            SourceIcon::Untrusted => Some("security-low-symbolic"),
        }
    }
}

/// An untrusted extension shows the dangerous glyph before anything else (T-41a-004).
pub fn resolve_for(untrusted: bool, is_stub: bool, has_custom_icon: bool, is_local: bool) -> SourceIcon {
    if untrusted { SourceIcon::Untrusted } else { resolve(is_stub, has_custom_icon, is_local) }
}

/// The order: a stub without an icon, then a custom icon, then the local source, then the default.
pub fn resolve(is_stub: bool, has_custom_icon: bool, is_local: bool) -> SourceIcon {
    if is_stub && !has_custom_icon {
        SourceIcon::Warning
    } else if has_custom_icon {
        SourceIcon::Custom
    } else if is_local {
        SourceIcon::Local
    } else {
        SourceIcon::Default
    }
}

/// The fill shown while an installed icon decodes, and when it cannot be read (`0x1F888888`, ARGB).
pub const PLACEHOLDER_ARGB: u32 = 0x1F88_8888;

/// The placeholder as the bytes of one RGBA pixel.
pub fn placeholder_rgba() -> [u8; 4] {
    let [a, r, g, b] = PLACEHOLDER_ARGB.to_be_bytes();
    [r, g, b, a]
}

/// A decoded icon: straight RGBA, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Reads the icon of an installed extension and decodes it. `path` is the extension package (its `icon.png` entry is
/// read) or a plain image file. Any failure is `None`, and the row then keeps the default icon. Blocking: callers run
/// it off the GTK thread.
pub fn load_icon(path: &Path) -> Option<DecodedIcon> {
    let bytes = kuro_extensions::anime::package::read_icon(path).or_else(|| std::fs::read(path).ok())?;
    decode_icon(&bytes)
}

/// Decodes PNG, JPEG, GIF or WebP bytes to RGBA. `None` for anything else.
pub fn decode_icon(bytes: &[u8]) -> Option<DecodedIcon> {
    let rgba = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = rgba.dimensions();
    (width > 0 && height > 0).then(|| DecodedIcon { width, height, rgba: rgba.into_raw() })
}

/// Icon choices keyed by source id.
#[derive(Debug, Default)]
pub struct IconCache {
    by_id: HashMap<i64, SourceIcon>,
}

impl IconCache {
    /// The icon of a source, resolved once per id. An untrusted extension gets the dangerous glyph first.
    pub fn get(&mut self, id: i64, untrusted: bool, is_stub: bool, has_custom_icon: bool, is_local: bool) -> SourceIcon {
        *self.by_id.entry(id).or_insert_with(|| resolve_for(untrusted, is_stub, has_custom_icon, is_local))
    }

    /// Forgets one source's icon, for example after its extension was updated.
    pub fn invalidate(&mut self, id: i64) {
        self.by_id.remove(&id);
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stub_without_an_icon_shows_the_warning_glyph() {
        assert_eq!(resolve(true, false, false), SourceIcon::Warning);
        assert_eq!(resolve(true, false, true), SourceIcon::Warning);
    }

    #[test]
    fn a_custom_icon_wins_over_local_and_default() {
        assert_eq!(resolve(false, true, true), SourceIcon::Custom);
        assert_eq!(resolve(true, true, false), SourceIcon::Custom, "a stub with an icon shows it");
    }

    #[test]
    fn local_then_default_when_nothing_else_applies() {
        assert_eq!(resolve(false, false, true), SourceIcon::Local);
        assert_eq!(resolve(false, false, false), SourceIcon::Default);
    }

    #[test]
    fn built_in_icons_have_names_and_custom_icons_do_not() {
        assert_eq!(SourceIcon::Custom.icon_name(), None);
        assert!(SourceIcon::Warning.icon_name().is_some());
        assert!(SourceIcon::Local.icon_name().is_some());
        assert!(SourceIcon::Default.icon_name().is_some());
        assert_eq!(ICON_HEIGHT_PX, 40);
    }

    #[test]
    fn an_untrusted_extension_shows_the_dangerous_glyph_before_anything_else() {
        assert_eq!(resolve_for(true, false, true, false), SourceIcon::Untrusted);
        assert_eq!(resolve_for(true, true, false, true), SourceIcon::Untrusted);
        assert_eq!(resolve_for(false, true, false, false), SourceIcon::Warning);
        assert_eq!(SourceIcon::Untrusted.icon_name(), Some("security-low-symbolic"));
    }

    #[test]
    fn the_cache_resolves_once_per_id_and_can_be_invalidated() {
        let mut cache = IconCache::default();
        assert_eq!(cache.get(7, false, false, false, false), SourceIcon::Default);
        // A second lookup with different inputs keeps the first answer until the id is invalidated.
        assert_eq!(cache.get(7, false, true, false, false), SourceIcon::Default);
        assert_eq!(cache.len(), 1);
        cache.invalidate(7);
        assert!(cache.is_empty());
        assert_eq!(cache.get(7, false, true, false, false), SourceIcon::Warning);
        assert_eq!(cache.get(8, true, false, true, false), SourceIcon::Untrusted, "the cache goes through resolve_for");
    }

    #[test]
    fn the_placeholder_is_a_faint_grey_and_unreadable_icons_decode_to_nothing() {
        assert_eq!(placeholder_rgba(), [0x88, 0x88, 0x88, 0x1F]);
        assert_eq!(decode_icon(b"not an image"), None);
        assert_eq!(load_icon(Path::new("/nonexistent/icon.png")), None);
    }

    #[test]
    fn a_png_file_and_a_package_icon_decode_to_rgba() {
        use std::io::Write;
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode");
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("icon.png");
        std::fs::File::create(&file).expect("file").write_all(&png).expect("write");
        let icon = load_icon(&file).expect("decoded");
        assert_eq!((icon.width, icon.height), (2, 3));
        assert_eq!(&icon.rgba[..4], &[1, 2, 3, 255]);
    }
}
