//! Anime folder and file names (port of the name builders in `AnimeDownloadProvider.kt`, behaviour 9.2).
//!
//! `fat_safe` is the manga sanitiser `DiskUtil.buildValidFilename`: the same rule set (dots and spaces trimmed at both
//! ends, forbidden characters to `_`, 240 UTF-16 units, `(invalid)` when empty, reserved device names get `_`).

use crate::sanitize::build_valid_filename;

/// Used for an episode whose name is blank.
pub const DEFAULT_EPISODE_NAME: &str = "Episode";

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn non_blank(text: Option<&str>) -> Option<&str> {
    text.filter(|t| !is_blank(t))
}

/// `fat_safe`: a single path component that is safe on FAT, NTFS and ext4.
pub fn fat_safe(name: &str) -> String {
    build_valid_filename(name)
}

/// `"<name> (<LANG>)"`, sanitised. The source folder name.
pub fn source_dir_name(name: &str, lang: &str) -> String {
    fat_safe(&format!("{name} ({})", lang.to_uppercase()))
}

/// The anime folder name.
pub fn anime_dir_name(title: &str) -> String {
    fat_safe(title)
}

/// `[<scanlator>_]<name>`, sanitised. A blank name becomes `Episode`, a blank scanlator is ignored.
pub fn episode_dir_name(name: &str, scanlator: Option<&str>) -> String {
    let name = if is_blank(name) { DEFAULT_EPISODE_NAME } else { name };
    let full = match scanlator.filter(|s| !is_blank(s)) {
        Some(scanlator) => format!("{scanlator}_{name}"),
        None => name.to_owned(),
    };
    fat_safe(&full)
}

/// The legacy folder name: `<scanlator>_<name>` whenever the scanlator is present, even when blank. Nothing else
/// changes, so a blank name stays blank (behaviour 9.2, legacy format).
pub fn legacy_episode_dir_name(name: &str, scanlator: Option<&str>) -> String {
    let full = match scanlator {
        Some(scanlator) => format!("{scanlator}_{name}"),
        None => name.to_owned(),
    };
    fat_safe(&full)
}

/// The names to look for, new name first, then the legacy name. The first existing one wins.
pub fn valid_episode_dir_names(name: &str, scanlator: Option<&str>) -> [String; 2] {
    [episode_dir_name(name, scanlator), legacy_episode_dir_name(name, scanlator)]
}

/// The longest file component in UTF-8 bytes (T-24a-072, E-F17).
pub const MAX_FILE_NAME_BYTES: usize = 200;

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

/// `<episode dir name> (<episode id>)`: the folder name of a new download whose plain name is taken by another folder
/// (T-24a-066, E-F03). The result stays within the 240-unit limit of [`fat_safe`].
pub fn unique_episode_dir_name(name: &str, scanlator: Option<&str>, episode_id: i64) -> String {
    let suffix = format!(" ({episode_id})");
    let base = episode_dir_name(name, scanlator);
    format!("{}{suffix}", truncate_utf8(&base, 240 - suffix.len()))
}

/// `true` when the folder name would change: the episode name differs, or the scanlator differs with blank and `None`
/// treated as equal.
pub fn is_episode_dir_name_changed(
    old_name: &str,
    old_scanlator: Option<&str>,
    new_name: &str,
    new_scanlator: Option<&str>,
) -> bool {
    old_name != new_name || non_blank(old_scanlator) != non_blank(new_scanlator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fat_safe_trims_maps_and_limits() {
        assert_eq!(fat_safe("  ..Title.. "), "Title");
        assert_eq!(fat_safe(" . "), "(invalid)");
        assert_eq!(fat_safe(""), "(invalid)");
        assert_eq!(fat_safe("a\"b*c/d:e<f>g?h\\i|j"), "a_b_c_d_e_f_g_h_i_j");
        assert_eq!(fat_safe("x\u{0}y\u{1f}z\u{7f}"), "x_y_z_");
        assert_eq!(fat_safe(&"a".repeat(241)).chars().count(), 240, "241 characters are cut to 240");
    }

    #[test]
    fn fat_safe_limit_counts_utf16_units() {
        let out = fat_safe(&"\u{1F600}".repeat(200));
        assert_eq!(out.encode_utf16().count(), 240);
    }

    #[test]
    fn source_and_anime_folder_names() {
        assert_eq!(source_dir_name("MangaDex", "en"), "MangaDex (EN)");
        assert_eq!(source_dir_name("Name/Slash", "pt-br"), "Name_Slash (PT-BR)");
        assert_eq!(anime_dir_name("Spy x Family: Part 2"), "Spy x Family_ Part 2");
    }

    #[test]
    fn episode_name_with_and_without_scanlator() {
        assert_eq!(episode_dir_name("Episode 1", None), "Episode 1");
        assert_eq!(episode_dir_name("Episode 1", Some("SubsPlease")), "SubsPlease_Episode 1");
        assert_eq!(episode_dir_name("Episode 1", Some("   ")), "Episode 1", "blank scanlator is ignored");
    }

    #[test]
    fn blank_episode_name_becomes_episode() {
        assert_eq!(episode_dir_name("   ", None), "Episode");
        assert_eq!(episode_dir_name("", Some("Scan")), "Scan_Episode");
    }

    #[test]
    fn invalid_characters_in_the_scanlator_are_mapped() {
        assert_eq!(episode_dir_name("Ep", Some("A/B")), "A_B_Ep");
    }

    #[test]
    fn legacy_name_has_no_blank_checks() {
        assert_eq!(legacy_episode_dir_name("Ep", None), "Ep");
        assert_eq!(legacy_episode_dir_name("Ep", Some("S")), "S_Ep");
        assert_eq!(legacy_episode_dir_name("", Some("S")), "S_", "a blank name is not replaced");
        assert_eq!(legacy_episode_dir_name("Ep", Some(" ")), "_Ep", "a blank scanlator still adds its prefix");
    }

    #[test]
    fn new_name_comes_first_then_the_legacy_name() {
        assert_eq!(valid_episode_dir_names("Ep", Some(" ")), ["Ep".to_owned(), "_Ep".to_owned()]);
        assert_eq!(valid_episode_dir_names("", None), ["Episode".to_owned(), "(invalid)".to_owned()]);
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

    #[test]
    fn unique_name_appends_the_episode_id() {
        assert_eq!(unique_episode_dir_name("Ep 1", None, 42), "Ep 1 (42)");
        assert_eq!(unique_episode_dir_name("Ep 1", Some("Scan"), 7), "Scan_Ep 1 (7)");
        let long = unique_episode_dir_name(&"a".repeat(300), None, 9);
        assert!(long.encode_utf16().count() <= 240 + " (9)".len());
        assert!(long.ends_with(" (9)"));
    }

    #[test]
    fn name_change_detection_treats_blank_and_none_alike() {
        assert!(is_episode_dir_name_changed("Ep 1", None, "Ep 2", None));
        assert!(!is_episode_dir_name_changed("Ep 1", None, "Ep 1", Some("  ")), "null and blank scanlator are equal");
        assert!(!is_episode_dir_name_changed("Ep 1", Some(""), "Ep 1", None));
        assert!(is_episode_dir_name_changed("Ep 1", Some("A"), "Ep 1", Some("B")));
        assert!(is_episode_dir_name_changed("Ep 1", None, "Ep 1", Some("B")));
    }
}
