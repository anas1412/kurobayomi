//! EPUB contract (area:14 behavior 25, T-14a-042): package document, spine-ordered page images, and separator
//! detection for backslash archives. Zip access is `kuro-archive`'s; these functions take entry names and a reader
//! callback, so they run without a zip file.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use kuro_archive::ArchiveReader;
use kuro_source::model::{SChapter, SManga};

use crate::archive::read_entry;
use crate::dates;

/// The media type of page documents (`manifest > item`, Kotlin `getPagesFromDocument`).
const XHTML: &str = "application/xhtml+xml";

/// Reads one archive entry by name; `None` when it does not exist.
pub type EntryRead<'a> = dyn FnMut(&str) -> io::Result<Option<Vec<u8>>> + 'a;

/// `\` when the archive holds `META-INF\container.xml` (Kotlin `getPathSeparator`), otherwise `/`.
pub fn separator_of(names: &[String]) -> char {
    if names.iter().any(|name| name == "META-INF\\container.xml") {
        '\\'
    } else {
        '/'
    }
}

/// The container entry name with the archive's separator.
pub fn container_name(sep: char) -> String {
    format!("META-INF{sep}container.xml")
}

/// The package document path: the first `rootfile[full-path]` of the container, else `OEBPS/content.opf`. A container
/// that is missing, not UTF-8 or malformed falls back the same way. (Kotlin's Jsoup returns "" for a missing attribute
/// and then fails; this port falls back instead.)
///
/// libarchive reports every entry name with `/` even when the zip stores `\` (kuro-archive epub.rs), so a `\` in the
/// container's `full-path` is a separator when the names use `/`.
pub fn package_href(container: Option<&[u8]>, sep: char) -> String {
    container
        .and_then(|xml| rootfile_path(xml).ok().flatten())
        .map(|href| {
            if sep == '/' {
                href.replace('\\', "/")
            } else {
                href
            }
        })
        .unwrap_or_else(|| resolve("OEBPS", "content.opf", sep))
}

/// Every page image of the EPUB in spine order, as archive entry names (Kotlin `getImagesFromPages`).
pub fn image_entries(names: &[String], read: &mut EntryRead<'_>) -> io::Result<Vec<String>> {
    let sep = separator_of(names);
    let container = read(&container_name(sep))?;
    let package = package_href(container.as_deref(), sep);
    let opf = read(&package)?.ok_or_else(|| missing(&package))?;
    let pages = spine_pages(text_of(&opf)?)?;
    let base = parent_dir(&package, sep);
    let mut images = Vec::new();
    for page in pages {
        let entry = resolve(base, &page, sep);
        let xhtml = read(&entry)?.ok_or_else(|| missing(&entry))?;
        images.extend(page_images(&entry, &String::from_utf8_lossy(&xhtml), sep));
    }
    Ok(images)
}

/// The XHTML page hrefs in spine order. Spine entries with no XHTML manifest item are dropped. A duplicate manifest id
/// keeps its last item (Kotlin `associateBy`).
pub fn spine_pages(opf: &str) -> io::Result<Vec<String>> {
    let mut reader = Reader::from_str(opf);
    let mut stack: Vec<String> = Vec::new();
    let mut manifest: HashMap<String, String> = HashMap::new();
    let mut spine: Vec<String> = Vec::new();
    loop {
        let (element, opened) = match reader.read_event().map_err(invalid)? {
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            Event::End(_) => {
                stack.pop();
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let local = element.local_name().into_inner().to_owned();
        let parent = stack.last().map(String::as_str);
        if parent == Some("manifest") && local == "item" {
            if attr(&element, "media-type")?.as_deref() == Some(XHTML) {
                if let (Some(id), Some(href)) = (attr(&element, "id")?, attr(&element, "href")?) {
                    manifest.insert(id, href);
                }
            }
        } else if parent == Some("spine") && local == "itemref" {
            if let Some(idref) = attr(&element, "idref")? {
                spine.push(idref);
            }
        }
        if opened {
            stack.push(local);
        }
    }
    Ok(spine
        .into_iter()
        .filter_map(|idref| manifest.get(&idref).cloned())
        .collect())
}

/// The images of one XHTML page as entry names, in document order: `img/@src` and `image/@xlink:href`, resolved
/// against the page's folder (Kotlin `getImagesFromPages`). An empty reference is skipped. A parse error ends the scan
/// with what was found, as Jsoup is lenient.
pub fn page_images(page_entry: &str, xhtml: &str, sep: char) -> Vec<String> {
    let base = parent_dir(page_entry, sep);
    let mut reader = Reader::from_str(xhtml);
    let mut found = Vec::new();
    loop {
        let element: BytesStart<'_> = match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => e,
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => continue,
        };
        let reference = match element.local_name().into_inner() {
            "img" => attr(&element, "src").ok().flatten(),
            "image" => attr(&element, "xlink:href").ok().flatten(),
            _ => None,
        };
        if let Some(reference) = reference.filter(|r| !r.is_empty()) {
            found.push(resolve(base, &reference, sep));
        }
    }
    found
}

/// The first page image of an EPUB file, its bytes (behavior 9.3, the EPUB branch of the cover fallback).
pub fn first_image_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let archive = kuro_archive::ArchiveReader::open(path).map_err(io::Error::other)?;
    let names: Vec<String> = archive
        .entries()
        .map_err(io::Error::other)?
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    let images = image_entries(&names, &mut |name: &str| read_entry(&archive, name))?;
    match images.first() {
        Some(first) => read_entry(&archive, first),
        None => Ok(None),
    }
}

/// The Dublin Core fields the chapter fill reads (Kotlin `fillMetadata`). `None` when the element is absent. An element
/// with no text is `Some("")`, as Jsoup's `text()` gives it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EpubMetadata {
    pub title: Option<String>,
    pub creator: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    /// `dc:date`, or `dcterms:modified` when there is no `dc:date` element.
    pub date: Option<String>,
}

/// The Dublin Core fields of a package document. The first element of each name wins (Jsoup `first()`), and the text is
/// whitespace-collapsed like Jsoup's `text()`.
pub fn metadata(opf: &str) -> io::Result<EpubMetadata> {
    let mut reader = Reader::from_str(opf);
    let mut found: HashMap<&'static str, String> = HashMap::new();
    // The element whose text is being read: its end-tag name, its slot, and the text so far.
    let mut capture: Option<(String, &'static str, String)> = None;
    loop {
        match reader.read_event().map_err(invalid)? {
            Event::Start(e) => {
                if let (None, Some(slot)) = (&capture, metadata_slot(&e)?) {
                    capture = Some((e.name().into_inner().to_owned(), slot, String::new()));
                }
            }
            Event::Empty(e) => {
                if let Some(slot) = metadata_slot(&e)? {
                    found.entry(slot).or_default();
                }
            }
            Event::Text(text) => {
                if let Some((_, _, buffer)) = capture.as_mut() {
                    let raw = text.xml_content(XmlVersion::Implicit1_0);
                    buffer.push_str(&unescape(&raw).map_err(invalid)?);
                }
            }
            Event::End(e) => {
                if capture
                    .as_ref()
                    .is_some_and(|(end, _, _)| end == e.name().into_inner())
                {
                    if let Some((_, slot, text)) = capture.take() {
                        found.entry(slot).or_insert_with(|| {
                            text.split_whitespace().collect::<Vec<_>>().join(" ")
                        });
                    }
                }
            }
            Event::GeneralRef(reference) => {
                if let Some((_, _, buffer)) = capture.as_mut() {
                    buffer.push_str(
                        &crate::comicinfo::resolve_ref(&reference).map_err(io::Error::other)?,
                    );
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(EpubMetadata {
        title: found.get("title").cloned(),
        creator: found.get("creator").cloned(),
        publisher: found.get("publisher").cloned(),
        description: found.get("description").cloned(),
        date: found.get("date").or_else(|| found.get("modified")).cloned(),
    })
}

/// The metadata slot an element fills: the `dc:*` elements, and `meta[property=dcterms:modified]`.
fn metadata_slot(element: &BytesStart<'_>) -> io::Result<Option<&'static str>> {
    Ok(match element.name().into_inner() {
        "dc:title" => Some("title"),
        "dc:creator" => Some("creator"),
        "dc:publisher" => Some("publisher"),
        "dc:description" => Some("description"),
        "dc:date" => Some("date"),
        "meta" if attr(element, "property")?.as_deref() == Some("dcterms:modified") => {
            Some("modified")
        }
        _ => None,
    })
}

/// Kotlin `fillMetadata`: the manga author and description, the chapter name, the scanlator (publisher, else creator)
/// and the date. A date that does not parse leaves the chapter's date alone (E-DATE-3).
pub fn fill_metadata(manga: &mut SManga, chapter: &mut SChapter, meta: &EpubMetadata) {
    if let Some(creator) = &meta.creator {
        manga.author = Some(creator.clone());
    }
    if let Some(description) = &meta.description {
        manga.description = Some(description.clone());
    }
    if let Some(title) = &meta.title {
        chapter.name = title.clone();
    }
    if let Some(scanlator) = meta.publisher.as_ref().or(meta.creator.as_ref()) {
        chapter.scanlator = Some(scanlator.clone());
    }
    if let Some(millis) = meta.date.as_deref().and_then(dates::parse_epub_date) {
        chapter.date_upload = millis;
    }
}

/// The Dublin Core fields of the EPUB file at `path`, read through its package document.
pub fn read_metadata(path: &Path) -> io::Result<EpubMetadata> {
    let archive = ArchiveReader::open(path).map_err(io::Error::other)?;
    let names: Vec<String> = archive
        .entries()
        .map_err(io::Error::other)?
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    let sep = separator_of(&names);
    let container = read_entry(&archive, &container_name(sep))?;
    let package = package_href(container.as_deref(), sep);
    let opf = read_entry(&archive, &package)?.ok_or_else(|| missing(&package))?;
    metadata(text_of(&opf)?)
}

/// `rootfile[full-path]` of a container, the first one that has the attribute.
fn rootfile_path(xml: &[u8]) -> io::Result<Option<String>> {
    let mut reader = Reader::from_str(text_of(xml)?);
    loop {
        match reader.read_event().map_err(invalid)? {
            Event::Start(e) | Event::Empty(e) if e.local_name().into_inner() == "rootfile" => {
                if let Some(path) = attr(&e, "full-path")? {
                    return Ok(Some(path));
                }
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
    }
}

/// `text_of` for the bytes of a document: a UTF-8 BOM is skipped.
fn text_of(bytes: &[u8]) -> io::Result<&str> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(invalid)
}

/// An attribute's unescaped value, `None` when absent. `key` is the raw name, prefix included.
fn attr(element: &BytesStart<'_>, key: &str) -> io::Result<Option<String>> {
    match element.try_get_attribute(key).map_err(invalid)? {
        Some(found) => Ok(Some(
            found
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(invalid)?
                .into_owned(),
        )),
        None => Ok(None),
    }
}

/// The parent folder of an entry name: the text before the last separator, `""` at the top.
fn parent_dir(path: &str, sep: char) -> &str {
    path.rfind(sep).map_or("", |index| &path[..index])
}

/// Kotlin `resolveZipPath`: `rel` joined onto `base`, with `.` and `..` folded and the separator normalised. A `rel`
/// that starts with the separator is returned as it is.
pub fn resolve(base: &str, rel: &str, sep: char) -> String {
    if rel.starts_with(sep) {
        return rel.to_owned();
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split(['/', sep]).chain(rel.split(['/', sep])) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join(&sep.to_string())
}

fn missing(entry: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("EPUB entry is missing: {entry}"),
    )
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTAINER: &str = r#"<?xml version="1.0"?>
<container><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

    const OPF: &str = r#"<package><manifest>
  <item id="p2" href="text/p2.xhtml" media-type="application/xhtml+xml"/>
  <item id="p1" href="text/p1.xhtml" media-type="application/xhtml+xml"/>
  <item id="c" href="style.css" media-type="text/css"/>
</manifest><spine><itemref idref="p1"/><itemref idref="p2"/><itemref idref="c"/><itemref idref="gone"/></spine></package>"#;

    const P1: &str = r#"<html><body><img src="../img/a.jpg"/><p>x</p><svg><image xlink:href="../img/b.png"/></svg></body></html>"#;
    const P2: &str = r#"<html><body><img src="../img/c.jpg"/></body></html>"#;

    /// An in-memory archive: entry name to bytes.
    fn archive(entries: &[(&str, &str)]) -> (Vec<String>, HashMap<String, Vec<u8>>) {
        let names = entries.iter().map(|(name, _)| (*name).to_owned()).collect();
        let map = entries
            .iter()
            .map(|(name, body)| ((*name).to_owned(), body.as_bytes().to_vec()))
            .collect();
        (names, map)
    }

    fn images_of(names: &[String], map: &HashMap<String, Vec<u8>>) -> io::Result<Vec<String>> {
        let mut read = |name: &str| -> io::Result<Option<Vec<u8>>> { Ok(map.get(name).cloned()) };
        image_entries(names, &mut read)
    }

    #[test]
    fn the_package_href_comes_from_the_container_with_a_fallback() {
        assert_eq!(
            package_href(Some(CONTAINER.as_bytes()), '/'),
            "OEBPS/content.opf"
        );
        assert_eq!(package_href(None, '/'), "OEBPS/content.opf");
        assert_eq!(package_href(None, '\\'), "OEBPS\\content.opf");
        assert_eq!(
            package_href(Some(b"<container/>"), '/'),
            "OEBPS/content.opf"
        );
    }

    #[test]
    fn a_backslash_full_path_in_a_slash_archive_is_a_separator() {
        let container = CONTAINER.replace("OEBPS/content.opf", "OEBPS\\content.opf");
        assert_eq!(
            package_href(Some(container.as_bytes()), '/'),
            "OEBPS/content.opf"
        );
        assert_eq!(
            package_href(Some(container.as_bytes()), '\\'),
            "OEBPS\\content.opf"
        );
    }

    #[test]
    fn the_separator_is_backslash_only_when_the_backslash_container_exists() {
        assert_eq!(separator_of(&["META-INF\\container.xml".to_owned()]), '\\');
        assert_eq!(separator_of(&["META-INF/container.xml".to_owned()]), '/');
    }

    #[test]
    fn t_14_epub_first_image_spine_order() -> io::Result<()> {
        let (names, map) = archive(&[
            ("META-INF/container.xml", CONTAINER),
            ("OEBPS/content.opf", OPF),
            ("OEBPS/text/p1.xhtml", P1),
            ("OEBPS/text/p2.xhtml", P2),
        ]);
        let images = images_of(&names, &map)?;
        assert_eq!(
            images,
            ["OEBPS/img/a.jpg", "OEBPS/img/b.png", "OEBPS/img/c.jpg"]
        );
        Ok(())
    }

    #[test]
    fn t_14_epub_separator_backslash() -> io::Result<()> {
        let container = CONTAINER.replace("OEBPS/content.opf", "OEBPS\\content.opf");
        let (names, map) = archive(&[
            ("META-INF\\container.xml", &container),
            ("OEBPS\\content.opf", OPF),
            ("OEBPS\\text\\p1.xhtml", P1),
            ("OEBPS\\text\\p2.xhtml", P2),
        ]);
        let images = images_of(&names, &map)?;
        assert_eq!(
            images,
            [
                "OEBPS\\img\\a.jpg",
                "OEBPS\\img\\b.png",
                "OEBPS\\img\\c.jpg"
            ]
        );
        Ok(())
    }

    #[test]
    fn a_missing_package_document_is_an_error() {
        let (names, map) = archive(&[("META-INF/container.xml", CONTAINER)]);
        assert!(images_of(&names, &map).is_err());
    }

    #[test]
    fn spine_pages_keep_spine_order_and_drop_non_xhtml_and_unknown_ids() -> io::Result<()> {
        let pages = spine_pages(OPF)?;
        assert_eq!(pages, ["text/p1.xhtml", "text/p2.xhtml"]);
        Ok(())
    }

    #[test]
    fn page_images_resolve_against_the_page_folder_in_document_order() {
        let images = page_images("OEBPS/text/p1.xhtml", P1, '/');
        assert_eq!(images, ["OEBPS/img/a.jpg", "OEBPS/img/b.png"]);
        assert!(page_images("p.xhtml", r#"<html><img src=""/></html>"#, '/').is_empty());
    }

    #[test]
    fn resolve_folds_dot_segments_and_keeps_absolute_paths() {
        assert_eq!(
            resolve("OEBPS/text", "../img/a.jpg", '/'),
            "OEBPS/img/a.jpg"
        );
        assert_eq!(resolve("", "./a/./b.jpg", '/'), "a/b.jpg");
        assert_eq!(resolve("a", "/abs.jpg", '/'), "/abs.jpg");
        assert_eq!(
            resolve("OEBPS\\text", "..\\img\\a.jpg", '\\'),
            "OEBPS\\img\\a.jpg"
        );
    }

    #[test]
    fn a_non_utf8_container_falls_back_to_the_default_package() {
        assert_eq!(
            package_href(Some(&[0xFF, 0xFE, 0x00]), '/'),
            "OEBPS/content.opf"
        );
    }

    #[test]
    fn t_14_epub_cover_from_minimal_epub() -> io::Result<()> {
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/minimal.epub"
        ));
        assert_eq!(first_image_bytes(path)?, Some(b"epub-image".to_vec()));
        Ok(())
    }

    #[test]
    fn t_14_epub_separator_backslash_fixture() -> io::Result<()> {
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/backslash.epub"
        ));
        assert_eq!(first_image_bytes(path)?, Some(b"epub-image".to_vec()));
        Ok(())
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    use kuro_source::model::{SChapter, SManga};

    const OPF: &str = r#"<?xml version="1.0"?>
<package xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata>
<dc:title>  Chapter   One </dc:title><dc:creator>Ann</dc:creator><dc:publisher>Team</dc:publisher>
<dc:description>Line &amp; more</dc:description><dc:date>2024-01-02T03:04:05+0100</dc:date>
<meta property="dcterms:modified">2020-01-01T00:00:00Z</meta></metadata></package>"#;

    #[test]
    fn the_dublin_core_fields_are_read_with_jsoup_text_rules() -> io::Result<()> {
        let meta = metadata(OPF)?;
        assert_eq!(meta.title.as_deref(), Some("Chapter One"));
        assert_eq!(meta.description.as_deref(), Some("Line & more"));
        assert_eq!(meta.date.as_deref(), Some("2024-01-02T03:04:05+0100"));
        Ok(())
    }

    #[test]
    fn dcterms_modified_is_the_date_when_dc_date_is_absent() -> io::Result<()> {
        let opf = r#"<package><metadata><meta property="dcterms:modified">2024-01-02T03:04:05Z</meta></metadata></package>"#;
        assert_eq!(metadata(opf)?.date.as_deref(), Some("2024-01-02T03:04:05Z"));
        Ok(())
    }

    #[test]
    fn fill_takes_the_publisher_then_the_creator_and_parses_the_date() -> io::Result<()> {
        let mut manga = SManga::new("Show", "Show");
        let mut chapter = SChapter::new("Show/x.epub", "x");
        fill_metadata(&mut manga, &mut chapter, &metadata(OPF)?);
        assert_eq!(manga.author.as_deref(), Some("Ann"));
        assert_eq!(manga.description.as_deref(), Some("Line & more"));
        assert_eq!(chapter.name, "Chapter One");
        assert_eq!(chapter.scanlator.as_deref(), Some("Team"));
        assert_eq!(
            dates::parse_epub_date("2024-01-02T02:04:05Z"),
            Some(chapter.date_upload)
        );
        Ok(())
    }

    #[test]
    fn creator_is_the_scanlator_without_a_publisher_and_a_bad_date_keeps_the_file_date()
    -> io::Result<()> {
        let opf = r#"<package><metadata><dc:creator>Ann</dc:creator><dc:date>not a date</dc:date></metadata></package>"#;
        let mut manga = SManga::new("Show", "Show");
        let mut chapter = SChapter::new("Show/x.epub", "x");
        chapter.date_upload = 5;
        fill_metadata(&mut manga, &mut chapter, &metadata(opf)?);
        assert_eq!(chapter.scanlator.as_deref(), Some("Ann"));
        assert_eq!(chapter.date_upload, 5);
        assert_eq!(chapter.name, "x");
        Ok(())
    }
}
