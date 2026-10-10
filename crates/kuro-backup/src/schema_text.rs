//! The `Backup` message as text, for the debug screen's backup schema page (T-21a-035, `BackupSchemaScreen.kt`). The
//! text lists the thirteen fields of the current container with their numbers. Entry messages are not listed here.

/// The current `Backup` container in the proto-style text the debug screen shows.
pub const TEXT: &str = "message Backup {
  repeated BackupManga backupManga = 1;
  repeated BackupCategory backupCategories = 2;
  repeated BackupSource backupSources = 101;
  repeated BackupPreference backupPreferences = 104;
  repeated BackupSourcePreferences backupSourcePreferences = 105;
  repeated BackupExtensionRepos backupMangaExtensionRepo = 106;
  bool isLegacy = 500;
  repeated BackupAnime backupAnime = 501;
  repeated BackupCategory backupAnimeCategories = 502;
  repeated BackupAnimeSource backupAnimeSources = 503;
  repeated BackupExtension backupExtensions = 504;
  repeated BackupExtensionStore backupAnimeExtensionStores = 505;
  repeated BackupCustomButtons backupCustomButton = 506;
}";

#[cfg(test)]
mod tests {
    use super::TEXT;

    /// Field number and name of each `Backup` field, from `plans/21-backup-format-and-create/p1-data-and-config.md`
    /// section 1.1. Field 3 and field 103 are not part of the current container.
    const EXPECTED: [(u32, &str); 13] = [
        (1, "backupManga"),
        (2, "backupCategories"),
        (101, "backupSources"),
        (104, "backupPreferences"),
        (105, "backupSourcePreferences"),
        (106, "backupMangaExtensionRepo"),
        (500, "isLegacy"),
        (501, "backupAnime"),
        (502, "backupAnimeCategories"),
        (503, "backupAnimeSources"),
        (504, "backupExtensions"),
        (505, "backupAnimeExtensionStores"),
        (506, "backupCustomButton"),
    ];

    /// The `(number, name)` pairs of the fields between the braces, in the order they appear.
    fn fields(text: &str) -> Vec<(u32, String)> {
        let body = text.split('{').nth(1).and_then(|rest| rest.split('}').next()).unwrap_or_default();
        body.lines()
            .filter_map(|line| {
                let stmt = line.trim().strip_suffix(';')?;
                let (decl, number) = stmt.rsplit_once(" = ")?;
                let name = decl.split_whitespace().last()?;
                Some((number.parse().ok()?, name.to_owned()))
            })
            .collect()
    }

    #[test]
    fn the_text_lists_all_thirteen_fields_with_the_plan_numbers() {
        let got = fields(TEXT);
        let want: Vec<(u32, String)> = EXPECTED.iter().map(|(n, name)| (*n, (*name).to_owned())).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn the_text_names_the_message_backup() {
        assert!(TEXT.starts_with("message Backup {"));
    }
}
