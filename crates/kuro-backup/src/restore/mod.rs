//! Restore side (area 22): the seven restore options, the file-pick route, progress amounts and the ordered section
//! run, the error log, the end state of a run and the restore screen model.
//!
//! Decision record for the behaviour-changing defects (T-22a-048). Each row names the test that encodes it, or says
//! that no test does yet.
//!
//! | Item | Decision | Encoded by |
//! |---|---|---|
//! | D5 metadata direction | keep the Kotlin rule: the newer side owns metadata, the row keeps title and flags | `manga::newer_backup_owns_metadata_and_keeps_the_row_title` |
//! | D8 position regression | keep for parity: a smaller backup position still wins when both sides are read | `merge::read_flag_never_reverts_and_position_follows_the_table` |
//! | D10 and E-17 custom buttons | fixed: one favourite, the first in backup order, and only when the database has none; one row per name | `extensions::only_the_first_favourite_of_the_backup_becomes_favourite`, `extensions::duplicate_names_inside_the_backup_make_one_row` |
//! | D16 chapter metadata | keep: existing chapters keep the database name, number and date | `manga::chapters_insert_new_and_merge_progress_on_existing` |
//! | D22 favourite default | keep the decoder default (a missing field is a favourite); decoder detail of area 21 | `tests/restore_integration.rs::x110_a_manga_without_the_favorite_field_is_restored_as_a_favourite` |
//! | D23 categorized display | keep: the flag is written only when the backup computes it | `categories::categorized_display_is_true_only_with_two_distinct_flags`, `prefs::categorized_display_is_written_only_when_computed` |
//! | E-25 `is_syncing` | gone: the unified tables do not store it, so a restored row can never be left marked as syncing | no test needed |
//! | E-26 `lastModifiedAt` and `favoriteModifiedAt` | keep the Kotlin write set; the sort reads the backup value | `manga::backup_modified_times_are_not_written_on_an_existing_row`, `manga::sort_puts_new_rows_first_then_newest_modified` |
//! | E-16 duplicate category names | fixed: one row per name, later duplicates map to it | `categories::duplicate_backup_names_create_one_row_and_map_to_it` |
//! | E-04 zero-content backup | refused before the restore starts, with the text "Backup does not contain any library entries" | `validate::empty_backup_is_refused_with_key_text` |
//! | D4 and F-01 cancel mid-entry | fixed: the entry is rolled back and no error line is written | `driver::cancel_between_entry_writes_rolls_the_entry_back`, `driver::cancel_stops_at_the_exact_entry_through_the_driver` |
//! | D17 and F-03 cancel and log | fixed: the error log is written on cancel too | `job::cancel_writes_the_errors_so_far_and_reports_canceled` |

pub mod anime;
pub mod categories;
pub mod driver;
pub mod entry;
pub mod error_log;
pub mod extensions;
pub mod job;
pub mod manga;
pub mod merge;
pub mod options;
pub mod prefs;
pub mod progress;
pub mod runner;
pub mod screen;
pub mod sections;
pub mod write;

/// Why a restore stopped before its plan finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RestoreError {
    /// The user asked to stop; checked before each section and each item (F-02).
    #[error("restore canceled")]
    Cancelled,
}
