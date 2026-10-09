//! `SChapter`: one chapter as a source reports it (`SChapter.kt`, T-15a-004).

/// A chapter. `date_upload` is epoch milliseconds (0 when unknown); `chapter_number` is -1.0 when unknown.
#[derive(Debug, Clone, PartialEq)]
pub struct SChapter {
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    pub chapter_number: f32,
    pub scanlator: Option<String>,
}

impl SChapter {
    /// A chapter with the source defaults: no upload date, unknown number, no scanlator.
    pub fn new(url: impl Into<String>, name: impl Into<String>) -> Self {
        Self { url: url.into(), name: name.into(), date_upload: 0, chapter_number: -1.0, scanlator: None }
    }

    /// Copies `name`, `url`, `date_upload`, `chapter_number` and `scanlator` from `other`, and nothing else.
    pub fn copy_from(&mut self, other: &SChapter) {
        self.name.clone_from(&other.name);
        self.url.clone_from(&other.url);
        self.date_upload = other.date_upload;
        self.chapter_number = other.chapter_number;
        self.scanlator.clone_from(&other.scanlator);
    }
}

/// The chapter number as the domain stores it (f64). The f32 is widened through its shortest decimal text, so
/// `1.1f32` becomes `1.1` and not `1.100000023841858` (EC-CH-07).
pub fn domain_chapter_number(number: f32) -> f64 {
    number.to_string().parse().unwrap_or(f64::from(number))
}

/// A blank scanlator is `None`; otherwise the value is trimmed (EC-CH-10, EC-CH-11).
pub fn normalize_scanlator(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_yields_the_source_defaults() {
        let c = SChapter::new("/c/1", "Chapter 1");
        assert_eq!(c.date_upload, 0);
        assert_eq!(c.chapter_number, -1.0);
        assert_eq!(c.scanlator, None);
    }

    #[test]
    fn copy_from_copies_exactly_the_five_fields() {
        let mut mine = SChapter::new("/old", "Old");
        mine.date_upload = 1;
        let source = SChapter {
            url: "/new".into(),
            name: "New".into(),
            date_upload: 99,
            chapter_number: 4.5,
            scanlator: Some("Group".into()),
        };
        mine.copy_from(&source);
        assert_eq!(mine, source);
    }

    #[test]
    fn one_point_one_keeps_its_decimal_form_in_the_domain() {
        assert_eq!(domain_chapter_number(1.1), 1.1);
        assert_eq!(domain_chapter_number(-1.0), -1.0);
        assert_eq!(domain_chapter_number(12.5), 12.5);
    }

    #[test]
    fn blank_scanlator_is_none_and_others_are_trimmed() {
        assert_eq!(normalize_scanlator(Some("  ")), None);
        assert_eq!(normalize_scanlator(None), None);
        assert_eq!(normalize_scanlator(Some(" Group X ")), Some("Group X".to_owned()));
    }
}
