//! Storage info rows of the Data settings screen (T-22a-035, X-205, X-206). One row per storage root: the path, a usage
//! bar and the caption `Available: X / Total: Y`. The GTK page draws the bar and the heading; the numbers live here.

use std::path::{Path, PathBuf};

/// One row. `used` is `None` when the bar is hidden (total unknown or zero).
#[derive(Debug, Clone, PartialEq)]
pub struct StorageRow {
    pub path: PathBuf,
    /// `1 - available / total`, clamped to 0.0..=1.0.
    pub used: Option<f64>,
    pub caption: String,
}

/// The row for one mount point.
pub fn storage_row(path: &Path, total: u64, available: u64) -> StorageRow {
    let used = (total > 0).then(|| (1.0 - available as f64 / total as f64).clamp(0.0, 1.0));
    StorageRow {
        path: path.to_path_buf(),
        used,
        caption: format!("Available: {} / Total: {}", format_size(available), format_size(total)),
    }
}

/// Decimal sizes as the GLib formatter shows them: plain bytes below 1000, then one decimal in kB, MB, GB, TB.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64 / 1000.0;
    for unit in ["kB", "MB", "GB"] {
        if value < 1000.0 {
            return format!("{value:.1} {unit}");
        }
        value /= 1000.0;
    }
    format!("{value:.1} TB")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_is_one_minus_available_over_total_and_clamped() {
        let row = storage_row(Path::new("/data"), 1000, 250);
        assert_eq!(row.used, Some(0.75));
        assert_eq!(storage_row(Path::new("/d"), 100, 200).used, Some(0.0), "more available than total clamps to empty");
        assert_eq!(storage_row(Path::new("/d"), 100, 0).used, Some(1.0));
    }

    #[test]
    fn a_zero_or_unknown_total_hides_the_bar() {
        assert_eq!(storage_row(Path::new("/d"), 0, 0).used, None);
    }

    #[test]
    fn caption_uses_the_decimal_sizes() {
        let row = storage_row(Path::new("/data"), 2_500_000_000, 1_200_000);
        assert_eq!(row.caption, "Available: 1.2 MB / Total: 2.5 GB");
        assert_eq!(row.path, PathBuf::from("/data"));
    }

    #[test]
    fn small_sizes_are_plain_bytes() {
        assert_eq!(format_size(999), "999 bytes");
        assert_eq!(format_size(1000), "1.0 kB");
        assert_eq!(format_size(3_000_000_000_000), "3.0 TB");
    }
}
