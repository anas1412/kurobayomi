//! The storage usage rows of Data and storage (T-22a-035): one row per mounted storage root with its path, a usage bar
//! and the caption "Available: X / Total: Y". The mounts come from the kernel's mount table, the sizes from GIO, and the
//! sizes are formatted by `glib::format_size`, since `kuro-backup` has no GTK.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use kuro_backup::storage_info::{StorageRow, storage_row_with};

/// The mount table the kernel keeps for this process.
const MOUNT_TABLE: &str = "/proc/self/mounts";

/// The mount points of the storage devices in a mount table: lines whose source is a block device, loop devices and
/// squashfs images left out, one mount point per device (the shortest path when a device is mounted twice, as with
/// subvolumes). Paths come back sorted.
pub fn storage_roots(table: &str) -> Vec<PathBuf> {
    let mut by_device: BTreeMap<String, PathBuf> = BTreeMap::new();
    for line in table.lines() {
        let mut fields = line.split_whitespace();
        let (Some(device), Some(mount), Some(fs_type)) = (fields.next(), fields.next(), fields.next()) else { continue };
        if !device.starts_with("/dev/") || device.starts_with("/dev/loop") || fs_type == "squashfs" {
            continue;
        }
        let mount = PathBuf::from(unescape(mount));
        match by_device.get(device) {
            Some(kept) if kept.as_os_str().len() <= mount.as_os_str().len() => {}
            _ => {
                by_device.insert(device.to_owned(), mount);
            }
        }
    }
    let mut roots: Vec<PathBuf> = by_device.into_values().collect();
    roots.sort();
    roots
}

/// The mount table writes a space as `\040` and a tab as `\011`: three octal digits after a backslash.
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = (bytes[i] == b'\\' && i + 3 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[i + 1..i + 4]).ok().and_then(|d| u8::from_str_radix(d, 8).ok()))
            .flatten();
        match octal {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `(total, available)` bytes of the file system at `path`, or `None` when GIO cannot tell.
fn sizes(path: &Path) -> Option<(u64, u64)> {
    let info = gio::File::for_path(path)
        .query_filesystem_info("filesystem::size,filesystem::free", None::<&gio::Cancellable>)
        .ok()?;
    Some((info.attribute_uint64("filesystem::size"), info.attribute_uint64("filesystem::free")))
}

/// One row per mounted storage root.
pub fn rows() -> Vec<StorageRow> {
    let table = std::fs::read_to_string(MOUNT_TABLE).unwrap_or_default();
    storage_roots(&table)
        .into_iter()
        .map(|root| {
            let (total, available) = sizes(&root).unwrap_or((0, 0));
            storage_row_with(&root, total, available, &|bytes| glib::format_size(bytes).to_string())
        })
        .collect()
}

fn row_widget(row: &StorageRow) -> gtk::ListBoxRow {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    crate::ui::margins(&column, 12);
    column.append(&gtk::Label::builder().label(row.path.display().to_string()).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::Middle).css_classes(["heading"]).build());
    if let Some(used) = row.used {
        column.append(&gtk::ProgressBar::builder().fraction(used).build());
    }
    column.append(&crate::ui::dim(&row.caption));
    gtk::ListBoxRow::builder().child(&column).activatable(false).selectable(false).build()
}

fn find_label(widget: &gtk::Widget, starts_with: &str) -> Option<gtk::Label> {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        if label.text().starts_with(starts_with) {
            return Some(label.clone());
        }
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_label(&c, starts_with) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// Puts the real rows where the screen's one-line "Available: … / Total: …" row stands. Returns false when the page has
/// no such row, which leaves it as it is.
pub fn replace_usage_row(page: &gtk::Widget, rows: &[StorageRow]) -> bool {
    let Some(label) = find_label(page, "Available:") else { return false };
    let (Some(row), Some(list)) = (label.ancestor(gtk::ListBoxRow::static_type()), label.ancestor(gtk::ListBox::static_type())) else {
        return false;
    };
    let (Some(row), Some(list)) = (row.downcast::<gtk::ListBoxRow>().ok(), list.downcast::<gtk::ListBox>().ok()) else { return false };
    let at = row.index();
    for (offset, storage) in rows.iter().enumerate() {
        list.insert(&row_widget(storage), at + 1 + i32::try_from(offset).unwrap_or(0));
    }
    list.remove(&row);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
proc /proc proc rw,nosuid 0 0
tmpfs /run tmpfs rw 0 0
/dev/nvme0n1p2 / ext4 rw 0 0
/dev/nvme0n1p3 /home btrfs rw,subvol=/@home 0 0
/dev/nvme0n1p3 /home/snap/.snapshots btrfs rw,subvol=/@snap 0 0
/dev/sdb1 /run/media/me/My\\040Disk exfat rw 0 0
/dev/loop3 /snap/core squashfs ro 0 0
/dev/sdc1 /mnt/img squashfs ro 0 0
";

    #[test]
    fn roots_are_the_block_device_mounts_once_per_device_and_sorted() {
        let roots = storage_roots(TABLE);
        let shown: Vec<_> = roots.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(shown, ["/", "/home", "/run/media/me/My Disk"]);
    }

    #[test]
    fn an_empty_table_has_no_roots() {
        assert!(storage_roots("").is_empty());
    }

    #[test]
    fn octal_escapes_are_decoded() {
        assert_eq!(unescape("a\\040b\\011c"), "a b\tc");
        assert_eq!(unescape("plain"), "plain");
        assert_eq!(unescape("tail\\04"), "tail\\04", "a short escape stays as written");
    }
}
