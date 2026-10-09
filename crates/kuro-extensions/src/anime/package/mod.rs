//! The `.kuroext` container (T-19a-004): a zip with `manifest.json`, `source.wasm` and an optional icon. Entry names
//! are checked before anything is read, and size caps apply to the declared and the read sizes.

pub mod manifest;
pub mod verify;

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use zip::ZipArchive;

use crate::anime::model::LoadError;
pub use manifest::Manifest;

pub const MANIFEST_ENTRY: &str = "manifest.json";
pub const WASM_ENTRY: &str = "source.wasm";
pub const ICON_ENTRY: &str = "icon.png";
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const MAX_WASM_BYTES: u64 = 16 * 1024 * 1024;
/// An icon above this size is dropped, not fatal.
pub const MAX_ICON_BYTES: u64 = 256 * 1024;

/// A package whose container and manifest passed. The component is not read; it runs only in the WASM host.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub manifest: Manifest,
    /// The icon entry, when the archive holds one within [`MAX_ICON_BYTES`].
    pub icon_entry: Option<String>,
}

/// Opens and checks the package at `path`.
pub fn open(path: &Path) -> Result<Package, LoadError> {
    let file = File::open(path).map_err(|err| LoadError::Io(format!("{}: {err}", path.display())))?;
    open_reader(file)
}

/// Opens and checks a package from any seekable reader.
pub fn open_reader<R: Read + Seek>(reader: R) -> Result<Package, LoadError> {
    let mut archive =
        ZipArchive::new(reader).map_err(|err| LoadError::Container(format!("not a zip archive: {err}")))?;
    let names: BTreeSet<String> = archive.file_names().map(str::to_owned).collect();
    if let Some(bad) = names.iter().find(|name| unsafe_name(name)) {
        return Err(LoadError::Container(format!("unsafe entry name {bad:?}")));
    }
    if !names.contains(MANIFEST_ENTRY) {
        return Err(LoadError::Manifest("manifest.json is missing".into()));
    }
    if !names.contains(WASM_ENTRY) {
        return Err(LoadError::Container("source.wasm is missing".into()));
    }
    let wasm_size = archive.by_name(WASM_ENTRY).map_err(|err| LoadError::Container(err.to_string()))?.size();
    if wasm_size > MAX_WASM_BYTES {
        return Err(LoadError::Container("source.wasm is larger than 16 MiB".into()));
    }
    let manifest_bytes = read_capped(&mut archive, MANIFEST_ENTRY, MAX_MANIFEST_BYTES)?
        .ok_or_else(|| LoadError::Manifest("manifest.json is larger than 64 KiB".into()))?;
    let manifest = Manifest::parse(&manifest_bytes)?;
    manifest.validate(&|name| names.contains(name))?;
    let icon_name = manifest.icon.clone().unwrap_or_else(|| ICON_ENTRY.to_owned());
    let icon_entry = if names.contains(&icon_name) {
        let size = archive.by_name(&icon_name).map_err(|err| LoadError::Container(err.to_string()))?.size();
        (size <= MAX_ICON_BYTES).then_some(icon_name)
    } else {
        None
    };
    Ok(Package { manifest, icon_entry })
}

/// The icon bytes of the package at `path`, or `None` when it has no icon, the icon is over [`MAX_ICON_BYTES`], or the
/// file cannot be read. Never an error: a package without an icon shows the generic one.
pub fn read_icon(path: &Path) -> Option<Vec<u8>> {
    let name = open(path).ok()?.icon_entry?;
    let mut archive = ZipArchive::new(File::open(path).ok()?).ok()?;
    read_capped(&mut archive, &name, MAX_ICON_BYTES).ok().flatten()
}

/// `..` components, absolute names and backslashes are refused before any entry is read.
fn unsafe_name(name: &str) -> bool {
    name.starts_with('/') || name.contains('\\') || name.split('/').any(|part| part == "..")
}

/// Reads one entry, or `None` when it is larger than `limit` (the declared size is not trusted alone).
fn read_capped<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, LoadError> {
    let entry = archive.by_name(name).map_err(|err| LoadError::Container(format!("{name}: {err}")))?;
    let mut bytes = Vec::new();
    entry.take(limit + 1).read_to_end(&mut bytes).map_err(|err| LoadError::Container(format!("{name}: {err}")))?;
    Ok((bytes.len() as u64 <= limit).then_some(bytes))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::anime::testkit::{MANIFEST, build_zip};

    #[test]
    fn a_well_formed_package_opens_and_names_its_manifest() {
        let bytes = build_zip(&[(MANIFEST_ENTRY, MANIFEST.as_bytes()), (WASM_ENTRY, b"\0asm")]);
        let package = open_reader(Cursor::new(bytes)).expect("opens");
        assert_eq!(package.manifest.pkg, "eu.example.animeext.en.demo");
        assert_eq!(package.icon_entry, None);
    }

    #[test]
    fn a_parent_directory_entry_is_refused_before_anything_is_read() {
        let bytes = build_zip(&[(MANIFEST_ENTRY, MANIFEST.as_bytes()), (WASM_ENTRY, b"\0asm"), ("../x", b"escape")]);
        assert!(matches!(open_reader(Cursor::new(bytes)), Err(LoadError::Container(text)) if text.contains("unsafe")));
    }

    #[test]
    fn absolute_and_backslash_names_are_unsafe() {
        for name in ["/etc/passwd", "a\\b", "x/../../y"] {
            assert!(unsafe_name(name), "{name}");
        }
        assert!(!unsafe_name("icon.png"));
    }

    #[test]
    fn a_manifest_over_64_kib_is_refused() {
        let padding = "x".repeat(70 * 1024);
        let big = format!("{{\"pad\": \"{padding}\"}}");
        let bytes = build_zip(&[(MANIFEST_ENTRY, big.as_bytes()), (WASM_ENTRY, b"\0asm")]);
        assert!(matches!(open_reader(Cursor::new(bytes)), Err(LoadError::Manifest(text)) if text.contains("64 KiB")));
    }

    #[test]
    fn a_wasm_entry_over_16_mib_is_refused_by_its_declared_size() {
        let wasm = vec![0u8; 17 * 1024 * 1024];
        let bytes = build_zip(&[(MANIFEST_ENTRY, MANIFEST.as_bytes()), (WASM_ENTRY, &wasm)]);
        assert!(matches!(open_reader(Cursor::new(bytes)), Err(LoadError::Container(text)) if text.contains("16 MiB")));
    }

    #[test]
    fn an_icon_over_256_kib_is_dropped_and_the_package_still_loads() {
        let icon = vec![1u8; 300 * 1024];
        let manifest = MANIFEST.replace("\"nsfw\": false,", "\"nsfw\": false, \"icon\": \"icon.png\",");
        let bytes = build_zip(&[(MANIFEST_ENTRY, manifest.as_bytes()), (WASM_ENTRY, b"\0asm"), (ICON_ENTRY, &icon)]);
        let package = open_reader(Cursor::new(bytes)).expect("still loads");
        assert_eq!(package.icon_entry, None);
    }

    #[test]
    fn a_missing_manifest_or_component_is_refused() {
        let no_manifest = build_zip(&[(WASM_ENTRY, b"\0asm")]);
        assert!(matches!(open_reader(Cursor::new(no_manifest)), Err(LoadError::Manifest(_))));
        let no_wasm = build_zip(&[(MANIFEST_ENTRY, MANIFEST.as_bytes())]);
        assert!(matches!(open_reader(Cursor::new(no_wasm)), Err(LoadError::Container(_))));
    }

    #[test]
    fn bytes_that_are_not_a_zip_are_a_container_error() {
        assert!(matches!(open_reader(Cursor::new(b"not a zip".to_vec())), Err(LoadError::Container(_))));
    }
}
