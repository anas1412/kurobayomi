//! XML helper for flat documents such as `ComicInfo.xml` (T-01a-016): a root with attributes and one level of text
//! children. The writer emits a declaration naming the encoding and two-space indentation; the reader returns only
//! the child elements the caller knows and skips every other element, whatever it contains.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

/// Reading failed: malformed XML, or a root element other than the expected one.
#[derive(Debug, thiserror::Error)]
#[error("invalid XML: {0}")]
pub struct XmlError(String);

/// Write `<root attrs>` with `children` as `(name, text)` pairs, in the given order. Text is escaped.
pub fn write(root: &str, attrs: &[(&str, &str)], children: &[(&str, &str)]) -> String {
    let mut w = Writer::new_with_indent(Vec::new(), b' ', 2);
    let ok = "write to Vec";
    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None))).expect(ok);
    let mut start = BytesStart::new(root);
    for (k, v) in attrs {
        start.push_attribute((*k, *v));
    }
    w.write_event(Event::Start(start)).expect(ok);
    for (name, text) in children {
        w.write_event(Event::Start(BytesStart::new(*name))).expect(ok);
        w.write_event(Event::Text(BytesText::new(text))).expect(ok);
        w.write_event(Event::End(BytesEnd::new(*name))).expect(ok);
    }
    w.write_event(Event::End(BytesEnd::new(root))).expect(ok);
    String::from_utf8(w.into_inner()).expect("quick-xml writes UTF-8")
}

/// Read the direct children of `root` named in `known` as `(name, text)` pairs in document order. Unknown children
/// (and their subtrees) are ignored.
pub fn read(text: &str, root: &str, known: &[&str]) -> Result<Vec<(String, String)>, XmlError> {
    let err = |e: &dyn std::fmt::Display| XmlError(e.to_string());
    let mut r = Reader::from_str(text);
    let mut out = Vec::new();
    let mut in_root = false;
    loop {
        match r.read_event().map_err(|e| err(&e))? {
            Event::Start(e) if !in_root => {
                if e.name().as_ref() != root {
                    return Err(XmlError(format!("root element is not <{root}>")));
                }
                in_root = true;
            }
            Event::Start(e) => {
                let name = e.name().as_ref().to_owned();
                if known.contains(&name.as_str()) {
                    let body = r.read_text(e.name()).map_err(|e| err(&e))?;
                    let body = quick_xml::escape::unescape(&body).map_err(|e| err(&e))?;
                    out.push((name, body.into_owned()));
                } else {
                    r.read_to_end(e.name()).map_err(|e| err(&e))?;
                }
            }
            Event::End(_) if in_root => return Ok(out),
            Event::Eof => return Err(XmlError("no root element".into())),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN: [&str; 4] = ["Series", "Summary", "Writer", "Genre"];

    fn sample() -> Vec<(&'static str, &'static str)> {
        vec![("Series", "Blue & Gold"), ("Summary", "a <b> story"), ("Writer", "Anon"), ("Genre", "Action, Drama")]
    }

    #[test]
    fn writes_declaration_and_two_space_indent() {
        let xml = write("ComicInfo", &[("xmlns:xsd", "http://www.w3.org/2001/XMLSchema")], &sample());
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(xml.contains("\n  <Series>Blue &amp; Gold</Series>\n  <Summary>a &lt;b&gt; story</Summary>"));
        assert!(xml.ends_with("</ComicInfo>"));
    }

    #[test]
    fn comicinfo_round_trips() {
        let xml = write("ComicInfo", &[], &sample());
        let back = read(&xml, "ComicInfo", &KNOWN).unwrap();
        let want: Vec<(String, String)> = sample().into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        assert_eq!(back, want);
    }

    #[test]
    fn unknown_children_are_skipped_with_their_subtrees() {
        let xml = r#"<ComicInfo><Pages><Page Image="0"><Series>nested</Series></Page></Pages><Series>S</Series><Extra/></ComicInfo>"#;
        let back = read(xml, "ComicInfo", &KNOWN).unwrap();
        assert_eq!(back, vec![("Series".to_string(), "S".to_string())]);
    }

    #[test]
    fn wrong_root_and_malformed_input_are_errors() {
        assert!(read("<Other><Series>x</Series></Other>", "ComicInfo", &KNOWN).is_err());
        assert!(read("", "ComicInfo", &KNOWN).is_err());
        assert!(read("<ComicInfo><Series>x</ComicInfo>", "ComicInfo", &KNOWN).is_err());
    }
}
