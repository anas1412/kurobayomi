//! `extension`, `stem` and `is_hidden` for file names (area:14 behavior section 4.1).

/// Kotlin `UniFile.extension`: `substringAfterLast('.')`. A name without a dot is its own extension.
/// Not lowercased: callers apply their own case rule.
pub fn extension(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(_, ext)| ext)
}

/// Kotlin `UniFile.nameWithoutExtension`: `substringBeforeLast('.')`. A name without a dot is returned whole.
pub fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// Dot names are hidden, and every local listing skips them (behavior 4.3).
pub fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Device names Windows reserves, with or without an extension.
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The NTFS-safe form of a generated name (E-IMG-8, behavior 27.4). Characters Windows refuses, and control
/// characters, become `_`. Trailing dots and spaces are dropped, and a reserved device name gets a `_` prefix. A name
/// without any of these is returned unchanged, so Linux output is the same for ordinary names.
pub fn sanitize_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let kept = out.trim_end_matches(['.', ' ']).len();
    out.truncate(kept);
    let device = out.split('.').next().unwrap_or_default().to_ascii_uppercase();
    if RESERVED_DEVICE_NAMES.contains(&device.as_str()) {
        out.insert(0, '_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_only_forbidden_characters() {
        assert_eq!(sanitize_name("Show - 01"), "Show - 01");
        assert_eq!(sanitize_name("Ep: 1?/x*"), "Ep_ 1__x_");
        assert_eq!(sanitize_name("a\tb"), "a_b");
        assert_eq!(sanitize_name("α β"), "α β");
    }

    #[test]
    fn extension_takes_the_text_after_the_last_dot() {
        assert_eq!(extension("cover"), "cover");
        assert_eq!(extension(".nomedia"), "nomedia");
        assert_eq!(extension("a.b.cbz"), "cbz");
        assert_eq!(extension("EP01.MKV"), "MKV");
        assert_eq!(extension("dir."), "");
    }

    #[test]
    fn stem_takes_the_text_before_the_last_dot() {
        assert_eq!(stem(".nomedia"), "");
        assert_eq!(stem("a.b.cbz"), "a.b");
        assert_eq!(stem("cover"), "cover");
        assert_eq!(stem("dir."), "dir");
    }

    #[test]
    fn only_a_leading_dot_hides() {
        assert!(is_hidden(".noxml"));
        assert!(!is_hidden("a.b"));
        assert!(!is_hidden(""));
    }
}
