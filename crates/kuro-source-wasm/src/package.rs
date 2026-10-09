//! Reading and re-sealing a `.kuroext` zip (data file section 7.3). Entries are found by exact name only, so a name
//! such as `../x` is never opened (edge case I13), and every read is bounded (I14, I15).

use std::io::{Cursor, Read, Write};

use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::limits::MAX_WASM_BYTES;
use crate::manifest::{Manifest, ManifestError};
use crate::verify::Verified;

pub const MANIFEST_ENTRY: &str = "manifest.json";
pub const WASM_ENTRY: &str = "source.wasm";
/// Largest `manifest.json` the host reads.
const MAX_MANIFEST_BYTES: u64 = 1 << 20;

/// The parts of a package the host runs.
#[derive(Debug)]
pub struct Package {
    pub manifest: Manifest,
    pub wasm: Vec<u8>,
}

fn manifest_error(reason: impl std::fmt::Display) -> ManifestError {
    ManifestError::Json(reason.to_string())
}

/// Reads at most `limit` bytes of one entry. An entry that holds more is an error, whatever size it declares.
fn read_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, ManifestError> {
    let entry = archive.by_name(name).map_err(|e| manifest_error(format!("{name}: {e}")))?;
    let mut bytes = Vec::new();
    entry.take(limit + 1).read_to_end(&mut bytes).map_err(|e| manifest_error(format!("{name}: {e}")))?;
    if bytes.len() as u64 > limit {
        return Err(ManifestError::Field {
            field: if name == WASM_ENTRY { "source.wasm" } else { "manifest.json" },
            reason: "too large".to_owned(),
        });
    }
    Ok(bytes)
}

/// Opens the archive and validates `manifest.json`. `source.wasm` is not read.
pub fn read_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| manifest_error(format!("not a zip: {e}")))?;
    Manifest::parse(&read_entry(&mut archive, MANIFEST_ENTRY, MAX_MANIFEST_BYTES)?)
}

/// Opens the archive, validates `manifest.json` and returns it with `source.wasm`.
pub fn read_package(bytes: &[u8]) -> Result<Package, ManifestError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| manifest_error(format!("not a zip: {e}")))?;
    let manifest = Manifest::parse(&read_entry(&mut archive, MANIFEST_ENTRY, MAX_MANIFEST_BYTES)?)?;
    let wasm = read_entry(&mut archive, WASM_ENTRY, MAX_WASM_BYTES as u64)?;
    Ok(Package { manifest, wasm })
}

/// Rebuilds the archive with `signerPub` set to the key that [`crate::verify::verify_package`] accepted. Taking a
/// [`Verified`] makes this the only way to write the field, and it is called only by the installer (design 9.4).
pub fn with_signer(bytes: &[u8], signer: &Verified) -> Result<Vec<u8>, ManifestError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| manifest_error(format!("not a zip: {e}")))?;
    let mut manifest: Value = serde_json::from_slice(&read_entry(&mut archive, MANIFEST_ENTRY, MAX_MANIFEST_BYTES)?)
        .map_err(manifest_error)?;
    {
        use base64::Engine as _;
        let object = manifest.as_object_mut().ok_or_else(|| manifest_error("manifest.json is not an object"))?;
        let key = base64::engine::general_purpose::STANDARD.encode(signer.public_key());
        object.insert("signerPub".to_owned(), Value::String(key));
    }
    let mut out = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(manifest_error)?;
        if entry.name() != MANIFEST_ENTRY {
            out.raw_copy_file(entry).map_err(manifest_error)?;
        }
    }
    out.start_file(MANIFEST_ENTRY, SimpleFileOptions::default().compression_method(CompressionMethod::Stored))
        .map_err(manifest_error)?;
    out.write_all(manifest.to_string().as_bytes()).map_err(manifest_error)?;
    Ok(out.finish().map_err(manifest_error)?.into_inner())
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    /// A `.kuroext` archive with the given entries, stored without compression.
    pub fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            out.start_file(*name, SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
            out.write_all(data).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    /// The manifest of `pkg` at `version_code`, in the form of a valid package.
    pub fn manifest_json(pkg: &str, version_code: i64) -> String {
        serde_json::json!({
            "schema": 1, "pkg": pkg, "name": "Example", "versionName": "1.4.1", "versionCode": version_code,
            "abi": "1.4", "lang": "en", "hosts": [], "sources": []
        })
        .to_string()
    }

    pub fn package(pkg: &str, version_code: i64) -> Vec<u8> {
        archive(&[(MANIFEST_ENTRY, manifest_json(pkg, version_code).as_bytes()), (WASM_ENTRY, b"wasm-bytes")])
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn a_package_reads_back_its_manifest_and_wasm() {
        let package = read_package(&package("eu.example.one", 5)).expect("reads");
        assert_eq!(package.manifest.version_code, 5);
        assert_eq!(package.wasm, b"wasm-bytes");
    }

    #[test]
    fn missing_entries_and_non_zips_are_errors() {
        assert!(read_package(b"not a zip").is_err());
        let no_wasm = archive(&[(MANIFEST_ENTRY, manifest_json("eu.example.one", 1).as_bytes())]);
        assert!(read_package(&no_wasm).is_err());
        let no_manifest = archive(&[(WASM_ENTRY, b"x")]);
        assert!(read_package(&no_manifest).is_err());
    }

    #[test]
    fn a_traversal_entry_is_never_opened() {
        let tricky = archive(&[
            ("../manifest.json", manifest_json("eu.example.one", 1).as_bytes()),
            (WASM_ENTRY, b"x"),
        ]);
        assert!(read_package(&tricky).is_err(), "only the exact name manifest.json counts");
    }

    #[test]
    fn a_wasm_over_the_limit_is_refused() {
        let big = vec![0u8; MAX_WASM_BYTES + 1];
        let manifest = manifest_json("eu.example.one", 1);
        let archive = archive(&[(MANIFEST_ENTRY, manifest.as_bytes()), (WASM_ENTRY, &big)]);
        assert!(matches!(read_package(&archive), Err(ManifestError::Field { field: "source.wasm", .. })));
        let at_limit = vec![0u8; MAX_WASM_BYTES];
        let archive = super::fixtures::archive(&[(MANIFEST_ENTRY, manifest.as_bytes()), (WASM_ENTRY, &at_limit)]);
        assert!(read_package(&archive).is_ok());
    }
}
