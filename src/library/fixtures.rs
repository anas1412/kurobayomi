//! Row builders shared by the library tests. Test-only.

use kuro_domain::library::model::LibraryRow;
use kuro_domain::library::page::LibraryItem;

pub fn row(id: i64, category: i64) -> LibraryRow {
    LibraryRow {
        id,
        source: 7,
        title: format!("t{id}"),
        category,
        total: 10,
        done: 0,
        bookmarks: 0,
        fillermarks: 0,
        latest_upload: 0,
        item_fetched_at: 0,
        last_activity: 0,
        last_update: 0,
        date_added: 0,
        status: 0,
        fetch_interval: 0,
        viewer_flags: 0,
        always_update: true,
        next_update: 0,
    }
}

pub fn item(id: i64, category: i64) -> LibraryItem {
    LibraryItem { row: row(id, category), download_count: 0, unread_count: 0, is_local: false, source_language: String::new() }
}
