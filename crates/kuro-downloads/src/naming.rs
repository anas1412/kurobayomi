//! Folder and file names for both kinds (merges `layout.rs` names, `anime/naming.rs` and the scan rules of both
//! indexes). Pure functions, no I/O. What differs between manga and anime is a `match kind` here and nowhere else.
//!
//! Tree: `<root>/<source dir>/<entry dir>/<unit>`. A manga unit is a folder or `<name>.cbz`; an anime unit is a folder
//! (one `.mkv` or `.mp4` inside) or a bare `.mkv`/`.mp4` file.
//!
//! The units are passed as [`UnitName`] and sources as their folder name (see [`source_dir_name`]) so this file does not
//! depend on the shared unit types of lane A1; the caller builds a `UnitName` from `UnitRef.name` and `.scanlator`.

use kuro_platform::kind::Kind;

use crate::sanitize::build_valid_filename;

/// Suffix of an in-progress unit folder (`TMP_DIR_SUFFIX`).
pub const TMP_DIR_SUFFIX: &str = "_tmp";
/// Extension of a manga archive.
pub const CBZ_EXTENSION: &str = "cbz";
/// Suffix of a CBZ archive that is still being written (`<name>.cbz.part`).
pub const CBZ_PART_SUFFIX: &str = ".cbz.part";
/// Used for a manga chapter whose name is blank.
pub const DEFAULT_CHAPTER_NAME: &str = "Chapter";
/// Used for an anime episode whose name is blank.
pub const DEFAULT_EPISODE_NAME: &str = "Episode";
/// The longest file component in UTF-8 bytes (T-24a-072, E-F17).
pub const MAX_FILE_NAME_BYTES: usize = 200;

/// The name and scanlator of a unit: all the naming rules need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitName<'a> {
    pub name: &'a str,
    pub scanlator: Option<&'a str>,
}

impl<'a> UnitName<'a> {
    pub fn new(name: &'a str, scanlator: Option<&'a str>) -> Self {
        Self { name, scanlator }
    }
}

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// `fat_safe`: a single path component that is safe on FAT, NTFS and ext4 (the shared sanitiser).
pub fn fat_safe(name: &str) -> String {
    build_valid_filename(name)
}

/// `"<name> (<LANG>)"`, sanitised; `"<name>"` when the language is empty. The source folder name, the same for the app
/// and for the orphan-folder scan.
pub fn source_dir_name(name: &str, lang: &str) -> String {
    if is_blank(lang) {
        fat_safe(name)
    } else {
        fat_safe(&format!("{name} ({})", lang.to_uppercase()))
    }
}

/// The entry (manga or anime) folder name: the sanitised title.
pub fn entry_dir_name(title: &str) -> String {
    fat_safe(title)
}

/// `[<scanlator>_]<name>`, sanitised. A blank name becomes `Chapter` or `Episode`, a blank scanlator is ignored.
pub fn unit_dir_name(kind: Kind, unit: UnitName<'_>) -> String {
    let default = match kind {
        Kind::Manga => DEFAULT_CHAPTER_NAME,
        Kind::Anime => DEFAULT_EPISODE_NAME,
    };
    let name = if is_blank(unit.name) { default } else { unit.name };
    match unit.scanlator.filter(|s| !is_blank(s)) {
        Some(scanlator) => fat_safe(&format!("{scanlator}_{name}")),
        None => fat_safe(name),
    }
}

/// The legacy anime folder name: `<scanlator>_<name>` whenever the scanlator is present, even blank, and a blank name
/// stays blank (behaviour 9.2).
fn legacy_anime_dir_name(unit: UnitName<'_>) -> String {
    match unit.scanlator {
        Some(scanlator) => fat_safe(&format!("{scanlator}_{}", unit.name)),
        None => fat_safe(unit.name),
    }
}

/// The names a unit may have on disk, in lookup order. Manga: the folder, then `<folder>.cbz`. Anime: the current
/// folder name, then the legacy one.
pub fn unit_dir_names(kind: Kind, unit: UnitName<'_>) -> [String; 2] {
    match kind {
        Kind::Manga => {
            let dir = unit_dir_name(kind, unit);
            let cbz = format!("{dir}.{CBZ_EXTENSION}");
            [dir, cbz]
        }
        Kind::Anime => [unit_dir_name(kind, unit), legacy_anime_dir_name(unit)],
    }
}

/// The bare names the index keeps for a unit (no extension), in lookup order and without repeats. Manga has one,
/// anime the current and the legacy name.
pub fn unit_dir_stems(kind: Kind, unit: UnitName<'_>) -> Vec<String> {
    let mut stems = Vec::new();
    match kind {
        Kind::Manga => stems.push(unit_dir_name(kind, unit)),
        Kind::Anime => {
            for name in unit_dir_names(kind, unit) {
                if !stems.contains(&name) {
                    stems.push(name);
                }
            }
        }
    }
    stems
}

/// The staging folder names of a unit: `<stem>_tmp` for each stem.
pub fn staging_dir_names(kind: Kind, unit: UnitName<'_>) -> Vec<String> {
    unit_dir_stems(kind, unit).into_iter().map(|stem| format!("{stem}{TMP_DIR_SUFFIX}")).collect()
}

/// The file extensions (without the dot) of a finished unit that is a single file.
pub fn unit_extensions(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Manga => &[CBZ_EXTENSION],
        Kind::Anime => &["mkv", "mp4"],
    }
}

/// The name without the extension of a unit file, or the name itself. The index never stores an extension.
pub fn bare_unit_name(kind: Kind, name: &str) -> &str {
    for ext in unit_extensions(kind) {
        if let Some(stem) = name.strip_suffix(&format!(".{ext}")) {
            return stem;
        }
    }
    name
}

/// The scan rule of both indexes: the index name of a directory entry of an entry folder, `None` when it is not a
/// finished unit. A folder counts unless it is blank or ends in `_tmp`; a file counts by its stem when its extension is
/// one of [`unit_extensions`].
pub fn is_unit_entry(kind: Kind, name: &str, is_dir: bool) -> Option<String> {
    if is_blank(name) {
        return None;
    }
    if is_dir {
        return (!name.ends_with(TMP_DIR_SUFFIX)).then(|| name.to_owned());
    }
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && unit_extensions(kind).contains(&ext)).then(|| stem.to_owned())
}

/// Cuts `text` to at most `max` UTF-8 bytes, on a character boundary.
pub fn truncate_utf8(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// `<stem><ext>` with at most [`MAX_FILE_NAME_BYTES`] bytes. Only the stem is cut, so the extension stays.
pub fn file_component(stem: &str, ext: &str) -> String {
    let stem = truncate_utf8(stem, MAX_FILE_NAME_BYTES.saturating_sub(ext.len()));
    format!("{stem}{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn un<'a>(name: &'a str, scanlator: Option<&'a str>) -> UnitName<'a> {
        UnitName::new(name, scanlator)
    }

    // Ported from `sanitize.rs` (TST-34): the shared sanitiser, reached through `fat_safe`.
    #[test]
    fn edge_case_table_ec_71_to_ec_79() {
        assert_eq!(fat_safe("  ..Title.. "), "Title");
        assert_eq!(fat_safe("   "), "(invalid)");
        assert_eq!(fat_safe("a/b:c*d?e\"f<g>h|i\\j"), "a_b_c_d_e_f_g_h_i_j");
        assert_eq!(fat_safe("\u{7}"), "_");
        let emoji = "\u{1F600}".repeat(200);
        let out = fat_safe(&emoji);
        assert_eq!(out.encode_utf16().count(), 240);
        assert_eq!(out, "\u{1F600}".repeat(120));
        assert_eq!(fat_safe("CON"), "CON_");
        assert_eq!(fat_safe("Name (EN)"), "Name (EN)");
        assert_eq!(fat_safe("Mangá/Ż"), "Mangá_Ż");
        assert_eq!(fat_safe("a:b"), fat_safe("a_b"));
    }

    #[test]
    fn reserved_names_match_with_any_case_and_extension() {
        assert_eq!(fat_safe("nul"), "nul_");
        assert_eq!(fat_safe("Com9"), "Com9_");
        assert_eq!(fat_safe("LPT1.txt"), "LPT1_.txt");
        assert_eq!(fat_safe("CONSOLE"), "CONSOLE");
        assert_eq!(fat_safe("COM10"), "COM10");
    }

    #[test]
    fn names_without_reserved_words_match_the_kotlin_rule() {
        assert_eq!(fat_safe("One Piece: Chapter 1?"), "One Piece_ Chapter 1_");
        assert_eq!(fat_safe("...  "), "(invalid)");
        assert_eq!(fat_safe("v1.2 final."), "v1.2 final");
    }

    #[test]
    fn fat_safe_limit_counts_utf16_units() {
        assert_eq!(fat_safe(&"a".repeat(241)).chars().count(), 240, "241 characters are cut to 240");
        assert_eq!(fat_safe("x\u{0}y\u{1f}z\u{7f}"), "x_y_z_");
    }

    #[test]
    fn source_and_entry_folder_names() {
        assert_eq!(source_dir_name("MangaDex", "en"), "MangaDex (EN)");
        assert_eq!(source_dir_name("Name/Slash", "pt-br"), "Name_Slash (PT-BR)");
        assert_eq!(source_dir_name("Name", ""), "Name", "no language, no suffix");
        assert_eq!(entry_dir_name("Spy x Family: Part 2"), "Spy x Family_ Part 2");
        assert_eq!(entry_dir_name("Mangá/Ż"), "Mangá_Ż");
    }

    // TST-35, for both kinds
    #[test]
    fn unit_dir_name_with_and_without_scanlator() {
        for kind in Kind::ALL {
            assert_eq!(unit_dir_name(kind, un("Ch 1", Some("Scan"))), "Scan_Ch 1");
            assert_eq!(unit_dir_name(kind, un("Ch 1", Some("   "))), "Ch 1", "blank scanlator is ignored");
            assert_eq!(unit_dir_name(kind, un("Ch 1", None)), "Ch 1");
            assert_eq!(unit_dir_name(kind, un("a/b", Some("S"))), "S_a_b");
            assert_eq!(unit_dir_name(kind, un("Ep", Some("A/B"))), "A_B_Ep");
        }
    }

    #[test]
    fn a_blank_unit_name_becomes_chapter_or_episode() {
        assert_eq!(unit_dir_name(Kind::Manga, un("  ", None)), "Chapter");
        assert_eq!(unit_dir_name(Kind::Anime, un("   ", None)), "Episode");
        assert_eq!(unit_dir_name(Kind::Anime, un("", Some("Scan"))), "Scan_Episode");
    }

    // TST-36
    #[test]
    fn manga_names_include_cbz_after_the_folder() {
        assert_eq!(unit_dir_names(Kind::Manga, un("Ch 1", None)), ["Ch 1".to_owned(), "Ch 1.cbz".to_owned()]);
        assert_eq!(unit_dir_stems(Kind::Manga, un("Ch 1", None)), vec!["Ch 1".to_owned()]);
    }

    #[test]
    fn anime_names_come_new_first_then_legacy() {
        assert_eq!(unit_dir_names(Kind::Anime, un("Ep", Some(" "))), ["Ep".to_owned(), "_Ep".to_owned()]);
        assert_eq!(unit_dir_names(Kind::Anime, un("", None)), ["Episode".to_owned(), "(invalid)".to_owned()]);
        assert_eq!(unit_dir_stems(Kind::Anime, un("Ep", Some(" "))), vec!["Ep".to_owned(), "_Ep".to_owned()]);
        assert_eq!(unit_dir_stems(Kind::Anime, un("Ep", Some("S"))), vec!["S_Ep".to_owned()], "same names count once");
    }

    #[test]
    fn the_legacy_anime_name_has_no_blank_checks() {
        assert_eq!(legacy_anime_dir_name(un("Ep", None)), "Ep");
        assert_eq!(legacy_anime_dir_name(un("Ep", Some("S"))), "S_Ep");
        assert_eq!(legacy_anime_dir_name(un("", Some("S"))), "S_", "a blank name is not replaced");
        assert_eq!(legacy_anime_dir_name(un("Ep", Some(" "))), "_Ep", "a blank scanlator still adds its prefix");
    }

    #[test]
    fn staging_names_follow_the_stems() {
        assert_eq!(staging_dir_names(Kind::Manga, un("Ch 1", Some("S"))), vec!["S_Ch 1_tmp".to_owned()]);
        assert_eq!(staging_dir_names(Kind::Anime, un("Ep", Some(" "))), vec!["Ep_tmp".to_owned(), "_Ep_tmp".to_owned()]);
    }

    #[test]
    fn the_scan_rule_per_kind() {
        // folders count for both kinds, staging folders and blank names never
        for kind in Kind::ALL {
            assert_eq!(is_unit_entry(kind, "Ch 1", true), Some("Ch 1".to_owned()));
            assert_eq!(is_unit_entry(kind, "Ch 2_tmp", true), None);
            assert_eq!(is_unit_entry(kind, "  ", true), None);
            assert_eq!(is_unit_entry(kind, "notes.txt", false), None);
            assert_eq!(is_unit_entry(kind, "Ch 4.zip", false), None);
            assert_eq!(is_unit_entry(kind, "Ch 5.cbz.part", false), None);
        }
        assert_eq!(is_unit_entry(Kind::Manga, "Ch 3.cbz", false), Some("Ch 3".to_owned()));
        assert_eq!(is_unit_entry(Kind::Manga, ".cbz", false), None, "an empty stem is no chapter");
        assert_eq!(is_unit_entry(Kind::Manga, "Ep 1.mkv", false), None);
        assert_eq!(is_unit_entry(Kind::Anime, "Ep 1.mkv", false), Some("Ep 1".to_owned()));
        assert_eq!(is_unit_entry(Kind::Anime, "Ep 2.mp4", false), Some("Ep 2".to_owned()));
        assert_eq!(is_unit_entry(Kind::Anime, "Ep 4.webm", false), None);
        assert_eq!(is_unit_entry(Kind::Anime, "Ch 3.cbz", false), None);
    }

    #[test]
    fn the_bare_name_drops_the_kinds_extension_only() {
        assert_eq!(bare_unit_name(Kind::Manga, "Ch 5.cbz"), "Ch 5");
        assert_eq!(bare_unit_name(Kind::Manga, "Ch 5"), "Ch 5");
        assert_eq!(bare_unit_name(Kind::Anime, "Ep 1.mkv"), "Ep 1");
        assert_eq!(bare_unit_name(Kind::Anime, "Ep 1.cbz"), "Ep 1.cbz");
    }

    #[test]
    fn file_components_are_cut_to_200_bytes_on_a_char_boundary() {
        let stem = "\u{e9}".repeat(150); // 2 bytes per character, 300 bytes
        let name = file_component(&stem, ".mkv");
        assert!(name.len() <= 200, "got {} bytes", name.len());
        assert!(name.ends_with(".mkv"), "the extension is kept");
        assert!(name.is_char_boundary(name.len()));
        assert_eq!(file_component("short", ".mkv"), "short.mkv");
        assert_eq!(truncate_utf8("a\u{1F600}", 2), "a", "a 4-byte character is not split");
    }
}
