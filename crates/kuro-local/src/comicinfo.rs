//! ComicInfo.xml model, writer and reader (area:14 behavior 19, data 3 and 5, topic-comicinfo).
//!
//! The 17 element names, their order and the namespace prefixes follow data 3.1 and 5.3. The writer and the reader
//! list the elements by hand: that list is the contract (topic-comicinfo 9).

use quick_xml::escape::unescape;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

use crate::error::LocalSourceError;

/// `COMIC_INFO_FILE`: the file name at the top of a manga folder.
pub const COMIC_INFO_FILE: &str = "ComicInfo.xml";

/// `xmlns:xsd` and the prefix `ty` / `ay` URI (behavior 19.2: both prefixes bind the XML Schema URI).
const NS_XSD: &str = "http://www.w3.org/2001/XMLSchema";
const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The 17 element names in write order (data 5.3).
const ELEMENTS: [&str; 17] = [
    "Title",
    "Series",
    "Number",
    "Summary",
    "Writer",
    "Penciller",
    "Inker",
    "Colorist",
    "Letterer",
    "CoverArtist",
    "Translator",
    "Genre",
    "Tags",
    "Web",
    "PublishingStatusTachiyomi",
    "Categories",
    "SourceAniyomi",
];

/// A ComicInfo document. `None` is an absent element; `Some("")` is an empty element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComicInfo {
    pub title: Option<String>,
    pub series: Option<String>,
    pub number: Option<String>,
    pub summary: Option<String>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub inker: Option<String>,
    pub colorist: Option<String>,
    pub letterer: Option<String>,
    pub cover_artist: Option<String>,
    pub translator: Option<String>,
    pub genre: Option<String>,
    pub tags: Option<String>,
    pub web: Option<String>,
    /// Element `PublishingStatusTachiyomi` (prefix `ty`).
    pub publishing_status: Option<String>,
    /// Element `Categories` (prefix `ty`).
    pub categories: Option<String>,
    /// Element `SourceAniyomi` (prefix `ay`).
    pub source: Option<String>,
}

impl ComicInfo {
    /// The document text: declaration, root with the two namespace declarations, present elements in
    /// data 5.3 order, two-space indentation, no trailing newline. Byte-identical to the upstream xmlutil output
    /// (golden run, O-1 to O-4).
    pub fn to_xml(&self) -> String {
        let mut w = Writer::new_with_indent(Vec::new(), b' ', 2);
        let decl = BytesDecl::new("1.0", Some("UTF-8"), None);
        let mut root = BytesStart::new("ComicInfo");
        root.push_attribute(("xmlns:xsd", NS_XSD));
        root.push_attribute(("xmlns:xsi", NS_XSI));
        // Writing into a Vec cannot fail.
        w.write_event(Event::Decl(decl)).expect("write to Vec");
        w.write_event(Event::Start(root)).expect("write to Vec");
        let values: [Option<&str>; 17] = [
            self.title.as_deref(),
            self.series.as_deref(),
            self.number.as_deref(),
            self.summary.as_deref(),
            self.writer.as_deref(),
            self.penciller.as_deref(),
            self.inker.as_deref(),
            self.colorist.as_deref(),
            self.letterer.as_deref(),
            self.cover_artist.as_deref(),
            self.translator.as_deref(),
            self.genre.as_deref(),
            self.tags.as_deref(),
            self.web.as_deref(),
            self.publishing_status.as_deref(),
            self.categories.as_deref(),
            self.source.as_deref(),
        ];
        for (index, (name, value)) in ELEMENTS.iter().zip(values).enumerate() {
            let prefix = match index {
                14 | 15 => Some("ty"),
                16 => Some("ay"),
                _ => None,
            };
            write_text(&mut w, name, value, prefix);
        }
        w.write_event(Event::End(BytesEnd::new("ComicInfo")))
            .expect("write to Vec");
        String::from_utf8(w.into_inner()).expect("quick-xml writes UTF-8")
    }

    /// Parses a document. A UTF-8 BOM is skipped, entities and numeric references are unescaped, unknown children
    /// are ignored, a duplicate element keeps its last value, and the namespace prefix is ignored (E-CI-4 to E-CI-6).
    pub fn from_xml(bytes: &[u8]) -> Result<Self, LocalSourceError> {
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        let text = std::str::from_utf8(bytes).map_err(|e| xml_error(e.to_string()))?;
        let mut reader = Reader::from_str(text);
        let mut info = ComicInfo::default();
        let mut depth = 0usize;
        let mut root_seen = false;
        // The known element being read at depth 1, with the text collected so far.
        let mut pending: Option<(&'static str, String)> = None;
        loop {
            match reader.read_event().map_err(|e| xml_error(e.to_string()))? {
                Event::Start(e) => {
                    if depth == 0 {
                        check_root(e.local_name().into_inner())?;
                        root_seen = true;
                    } else if depth == 1 {
                        pending = known_element(e.local_name().into_inner())
                            .map(|name| (name, String::new()));
                    }
                    depth += 1;
                }
                Event::Empty(e) => {
                    if depth == 0 {
                        check_root(e.local_name().into_inner())?;
                        root_seen = true;
                    } else if depth == 1 {
                        if let Some(name) = known_element(e.local_name().into_inner()) {
                            info.set(name, String::new());
                        }
                    }
                }
                Event::Text(t) => {
                    if let Some((_, buf)) = pending.as_mut() {
                        buf.push_str(&unescape(&t).map_err(|e| xml_error(e.to_string()))?);
                    }
                }
                Event::CData(t) => {
                    if let Some((_, buf)) = pending.as_mut() {
                        buf.push_str(&t);
                    }
                }
                Event::GeneralRef(r) => {
                    if let Some((_, buf)) = pending.as_mut() {
                        buf.push_str(&resolve_ref(&r)?);
                    }
                }
                Event::End(_) => {
                    depth = depth.saturating_sub(1);
                    if depth == 1 {
                        if let Some((name, buf)) = pending.take() {
                            info.set(name, buf);
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        if !root_seen {
            return Err(xml_error("document has no root element".into()));
        }
        if depth != 0 {
            return Err(xml_error("unclosed element".into()));
        }
        Ok(info)
    }

    /// Stores a value under its element name (the 17 names of `ELEMENTS`).
    fn set(&mut self, name: &str, value: String) {
        let slot = match name {
            "Title" => &mut self.title,
            "Series" => &mut self.series,
            "Number" => &mut self.number,
            "Summary" => &mut self.summary,
            "Writer" => &mut self.writer,
            "Penciller" => &mut self.penciller,
            "Inker" => &mut self.inker,
            "Colorist" => &mut self.colorist,
            "Letterer" => &mut self.letterer,
            "CoverArtist" => &mut self.cover_artist,
            "Translator" => &mut self.translator,
            "Genre" => &mut self.genre,
            "Tags" => &mut self.tags,
            "Web" => &mut self.web,
            "PublishingStatusTachiyomi" => &mut self.publishing_status,
            "Categories" => &mut self.categories,
            "SourceAniyomi" => &mut self.source,
            _ => return,
        };
        *slot = Some(value);
    }
}

/// `ComicInfoPublishingStatus` (data 3.2, behavior 19.8). Lookups are exact and case-sensitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComicInfoPublishingStatus {
    Ongoing,
    Completed,
    Licensed,
    PublishingFinished,
    Cancelled,
    OnHiatus,
    Unknown,
}

/// Kotlin declaration order: the first entry wins in both lookups.
const STATUS_TABLE: [(ComicInfoPublishingStatus, &str, i32); 7] = [
    (ComicInfoPublishingStatus::Ongoing, "Ongoing", 1),
    (ComicInfoPublishingStatus::Completed, "Completed", 2),
    (ComicInfoPublishingStatus::Licensed, "Licensed", 3),
    (
        ComicInfoPublishingStatus::PublishingFinished,
        "Publishing finished",
        4,
    ),
    (ComicInfoPublishingStatus::Cancelled, "Cancelled", 5),
    (ComicInfoPublishingStatus::OnHiatus, "On hiatus", 6),
    (ComicInfoPublishingStatus::Unknown, "Unknown", 0),
];

impl ComicInfoPublishingStatus {
    /// The text written to the file (`comicInfoValue`).
    pub fn comic_info_value(self) -> &'static str {
        STATUS_TABLE
            .iter()
            .find(|row| row.0 == self)
            .map_or("Unknown", |row| row.1)
    }

    /// The SManga status number (`sMangaModelValue`).
    pub fn s_manga_value(self) -> i32 {
        STATUS_TABLE
            .iter()
            .find(|row| row.0 == self)
            .map_or(0, |row| row.2)
    }

    /// Kotlin `toComicInfoValue(Long)`: the text for an SManga status; any other number, `7` included, is `Unknown`.
    pub fn to_comic_info_value(value: i64) -> &'static str {
        let value = i32::try_from(value).unwrap_or(i32::MIN);
        STATUS_TABLE
            .iter()
            .find(|row| row.2 == value)
            .map_or("Unknown", |row| row.1)
    }

    /// Kotlin `toSMangaValue(String?)`: the SManga status for a text; absent or unknown text is `0`.
    pub fn to_s_manga_value(text: Option<&str>) -> i32 {
        text.and_then(|t| STATUS_TABLE.iter().find(|row| row.1 == t))
            .map_or(0, |row| row.2)
    }
}

fn xml_error(message: String) -> LocalSourceError {
    LocalSourceError::Xml(message)
}

fn check_root(local: &str) -> Result<(), LocalSourceError> {
    if local == "ComicInfo" {
        Ok(())
    } else {
        Err(xml_error("root element is not ComicInfo".into()))
    }
}

/// The canonical name of a known element, `None` for an unknown child (skipped, `ignoreUnknownChildren`).
fn known_element(local: &str) -> Option<&'static str> {
    ELEMENTS.iter().copied().find(|name| *name == local)
}

/// One `&...;` reference inside text: a character reference, one of the five predefined entities, or the raw text
/// when the name is unknown.
pub(crate) fn resolve_ref(
    reference: &quick_xml::events::BytesRef<'_>,
) -> Result<String, LocalSourceError> {
    if reference.is_char_ref() {
        let resolved = reference
            .resolve_char_ref()
            .map_err(|e| xml_error(e.to_string()))?;
        return Ok(resolved.map(String::from).unwrap_or_default());
    }
    let raw = format!("&{};", &**reference);
    Ok(unescape(&raw)
        .map_err(|e| xml_error(e.to_string()))?
        .into_owned())
}

/// Writes `<name>value</name>`, or `<prefix:name xmlns:prefix="uri">value</prefix:name>`. Absent values write nothing.
fn write_text(w: &mut Writer<Vec<u8>>, name: &str, value: Option<&str>, prefix: Option<&str>) {
    let Some(value) = value else { return };
    let qname = match prefix {
        Some(prefix) => format!("{prefix}:{name}"),
        None => name.to_owned(),
    };
    let mut start = BytesStart::new(qname.clone());
    if let Some(prefix) = prefix {
        start.push_attribute((format!("xmlns:{prefix}").as_str(), NS_XSD));
    }
    w.write_event(Event::Start(start)).expect("write to Vec");
    // Only `& < >` are escaped; quotes stay literal, as in the upstream output (O-3).
    let escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    w.write_event(Event::Text(quick_xml::events::BytesText::from_escaped(
        escaped,
    )))
    .expect("write to Vec");
    w.write_event(Event::End(BytesEnd::new(qname)))
        .expect("write to Vec");
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: &str = include_str!("../tests/fixtures/comicinfo/golden_upstream.xml");
    const GOLDEN_ESCAPES: &str =
        include_str!("../tests/fixtures/comicinfo/golden_upstream_escapes.xml");

    fn golden_input() -> ComicInfo {
        ComicInfo {
            title: Some("Chapter 12".into()),
            series: Some("Foo".into()),
            number: Some("12".into()),
            summary: Some("A & B <c>".into()),
            writer: Some("W".into()),
            penciller: Some("P".into()),
            translator: Some("Team".into()),
            genre: Some("Action, Drama".into()),
            web: Some("https://a.example/1".into()),
            publishing_status: Some("Ongoing".into()),
            categories: Some("Read".into()),
            source: Some("Source A".into()),
            ..ComicInfo::default()
        }
    }

    #[test]
    fn the_elements_are_the_seventeen_in_data_order() {
        assert_eq!(ELEMENTS.len(), 17);
        assert_eq!(ELEMENTS[14], "PublishingStatusTachiyomi");
        assert_eq!(ELEMENTS[16], "SourceAniyomi");
        assert_eq!(COMIC_INFO_FILE, "ComicInfo.xml");
    }

    #[test]
    fn t_14_comicinfo_golden_upstream() {
        // Fixture generated by the upstream serializer: xmlutil 0.90.3 with the AppModule.kt XML configuration over
        // the real ComicInfo.kt classes (JDK 17, Kotlin 2.4.0). Choices it settled: O-1 the `ty`/`ay` namespace is
        // declared on each element; O-2 no trailing newline; O-3 quotes are not escaped in text; O-4 an empty
        // string is written `<X></X>`. Compared byte for byte.
        assert_eq!(golden_input().to_xml(), GOLDEN);
    }

    #[test]
    fn t_14_comicinfo_golden_upstream_escapes_and_empty() {
        let info = ComicInfo {
            series: Some(String::new()),
            summary: Some("a < b & \"c\" 'd' > e".into()),
            ..ComicInfo::default()
        };
        assert_eq!(info.to_xml(), GOLDEN_ESCAPES);
    }

    #[test]
    fn the_writer_omits_nulls_and_keeps_empty_elements() {
        let info = ComicInfo {
            series: Some(String::new()),
            ..ComicInfo::default()
        };
        let xml = info.to_xml();
        assert!(xml.contains("<Series></Series>"), "{xml}");
        assert!(!xml.contains("<Title"), "{xml}");
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    }

    #[test]
    fn the_writer_escapes_text() {
        let info = ComicInfo {
            summary: Some("a < b & \"c\"".into()),
            ..ComicInfo::default()
        };
        let xml = info.to_xml();
        assert!(xml.contains("a &lt; b &amp; \"c\""), "{xml}");
    }

    #[test]
    fn the_round_trip_keeps_every_field() -> Result<(), LocalSourceError> {
        let info = golden_input();
        let back = ComicInfo::from_xml(info.to_xml().as_bytes())?;
        assert_eq!(back, info);
        Ok(())
    }

    #[test]
    fn entities_and_numeric_references_unescape() -> Result<(), LocalSourceError> {
        let xml = "<ComicInfo><Summary>x &amp; &lt;y&gt; &#x3B1;</Summary></ComicInfo>";
        let info = ComicInfo::from_xml(xml.as_bytes())?;
        assert_eq!(info.summary.as_deref(), Some("x & <y> α"));
        Ok(())
    }

    #[test]
    fn a_self_closing_element_is_an_empty_value() -> Result<(), LocalSourceError> {
        let info = ComicInfo::from_xml(b"<ComicInfo><Series/><Writer></Writer></ComicInfo>")?;
        assert_eq!(info.series.as_deref(), Some(""));
        assert_eq!(info.writer.as_deref(), Some(""));
        assert_eq!(info.summary, None);
        Ok(())
    }

    #[test]
    fn a_utf8_bom_is_accepted_and_utf16_is_rejected() {
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(b"<ComicInfo><Series>S</Series></ComicInfo>");
        let info = ComicInfo::from_xml(&bom);
        assert_eq!(info.map(|i| i.series).ok().flatten().as_deref(), Some("S"));
        let utf16: Vec<u8> = [0xFF_u8, 0xFE]
            .into_iter()
            .chain("<ComicInfo/>".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert!(matches!(
            ComicInfo::from_xml(&utf16),
            Err(LocalSourceError::Xml(_))
        ));
    }

    #[test]
    fn duplicates_keep_the_last_value() -> Result<(), LocalSourceError> {
        let info =
            ComicInfo::from_xml(b"<ComicInfo><Series>A</Series><Series>B</Series></ComicInfo>")?;
        assert_eq!(info.series.as_deref(), Some("B"));
        Ok(())
    }

    #[test]
    fn unknown_children_are_skipped() -> Result<(), LocalSourceError> {
        let xml = "<ComicInfo><Foo><Series>no</Series></Foo><Series>yes</Series></ComicInfo>";
        let info = ComicInfo::from_xml(xml.as_bytes())?;
        assert_eq!(info.series.as_deref(), Some("yes"));
        Ok(())
    }

    #[test]
    fn a_prefixed_or_bare_status_element_is_read_by_local_name() -> Result<(), LocalSourceError> {
        let prefixed = ComicInfo::from_xml(
            b"<ComicInfo xmlns:ty=\"x\"><ty:PublishingStatusTachiyomi>Completed</ty:PublishingStatusTachiyomi></ComicInfo>",
        )?;
        let bare = ComicInfo::from_xml(
            b"<ComicInfo><PublishingStatusTachiyomi>Completed</PublishingStatusTachiyomi></ComicInfo>",
        )?;
        assert_eq!(prefixed.publishing_status.as_deref(), Some("Completed"));
        assert_eq!(bare.publishing_status.as_deref(), Some("Completed"));
        Ok(())
    }

    #[test]
    fn a_wrong_root_or_empty_input_is_an_error() {
        assert!(matches!(
            ComicInfo::from_xml(b"<Book><Series>x</Series></Book>"),
            Err(LocalSourceError::Xml(_))
        ));
        assert!(matches!(
            ComicInfo::from_xml(b""),
            Err(LocalSourceError::Xml(_))
        ));
        assert!(matches!(
            ComicInfo::from_xml(b"<ComicInfo><Series>x"),
            Err(LocalSourceError::Xml(_))
        ));
    }

    #[test]
    fn t_14_comicinfo_status_text_table() {
        let rows = [
            ("Ongoing", 1),
            ("Completed", 2),
            ("Licensed", 3),
            ("Publishing finished", 4),
            ("Cancelled", 5),
            ("On hiatus", 6),
            ("Unknown", 0),
        ];
        for (text, value) in rows {
            assert_eq!(
                ComicInfoPublishingStatus::to_s_manga_value(Some(text)),
                value,
                "{text}"
            );
            assert_eq!(
                ComicInfoPublishingStatus::to_comic_info_value(i64::from(value)),
                text
            );
        }
        assert_eq!(ComicInfoPublishingStatus::to_s_manga_value(None), 0);
        assert_eq!(
            ComicInfoPublishingStatus::to_s_manga_value(Some("ongoing")),
            0
        );
        assert_eq!(ComicInfoPublishingStatus::to_comic_info_value(7), "Unknown");
        assert_eq!(
            ComicInfoPublishingStatus::PublishingFinished.comic_info_value(),
            "Publishing finished"
        );
        assert_eq!(ComicInfoPublishingStatus::OnHiatus.s_manga_value(), 6);
    }
}
