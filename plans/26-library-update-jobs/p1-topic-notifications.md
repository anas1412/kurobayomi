# Topic: library update notifications on Linux (area 26)

Text, ids and action semantics are fixed in `p1-behavior.md` section 8 and `p1-ui-parity.md` section 1. This file covers the freedesktop mapping and its limits.

## 1. Mapping

| Aniyomi element | freedesktop Notifications implementation |
|---|---|
| Channel / importance | no channels; urgency hint `low` (progress, errors, size warning), `normal` (summary, per-entry) |
| Progress (ID -101) | one notification, `replaces_id` updated in place, `transient` hint true, `category=transfer`, action "Cancel" (key `cancel`) |
| Size warning (ID -103) | `expire-timeout` 30000 ms; body text from `notification_size_warning`; default action opens FAQ URL via `gio::AppInfo::launch_default_for_uri` |
| Summary (ID -301 / -1301) | one notification per kind, replaced in place on each pass; body text per behaviour 8.5; default action opens the Updates screen |
| Per-entry | one notification per entry, generic category `network`, actions `read`, `view`, `download` (download only under threshold) |
| Error (ID -102) | `urgency=low`, body "N update(s) failed"; action `open-log` |
| Cancel the progress | `CloseNotification(id)` |
| Group | not available; emulate by keeping the summary notification as the group head and per-entry notifications independent. Dismissing the summary does not dismiss children (differs from Android group summary behaviour). |

## 2. Action callbacks

`notify-rust` action handlers run on a background thread. They must not touch GTK. Each handler sends a message on the `async-channel` used by the GTK side:

```rust
pub enum NotifyAction {
    MarkRead { kind: EntryKind, entry_id: i64, item_ids: Vec<i64> },
    OpenEntry { kind: EntryKind, entry_id: i64 },
    OpenItem { kind: EntryKind, entry_id: i64, item_id: i64 },   // default action of per-entry notification
    Download { kind: EntryKind, entry_id: i64, item_ids: Vec<i64> },
    CancelPass { kind: EntryKind },
    OpenErrorLog { path: std::path::PathBuf },
    OpenFaq,
}
```

Handling:
- `MarkRead`: writes `read = true` (manga) or `seen = true` (anime) for the item ids; if `removeAfterMarkedAsRead` is on, deletes the downloads for those items (area:23-downloads-manga/24). Then close the notification (same as Kotlin's `dismissNotification` with group id).
- `Download`: enqueue with autostart true (Kotlin `downloadChapters` default `autoStart = true`).
- `OpenItem`: opens the reader (manga) or the player (anime) at that item. If the item or entry no longer exists, show the toast "Download error" (AYMR `download_error`, Kotlin behaviour in `NotificationReceiver.openChapter`).

## 3. Hidden content

`hideNotificationContent` (area:02-preferences-system) suppresses titles and per-entry notifications. The summary keeps the generic count text. The error and warning notifications are not suppressed (matches Kotlin).

## 4. Images

- Notification large icon: cover image scaled to 192 px, circle crop, stored at `$XDG_CACHE_HOME/kurobayomi/covers/notif/<entry-hash>.png`. Freedesktop servers accept `image-path` hint with a file path.
- Failure to produce the image: omit the hint.

## 5. Desktop-specific limits

- Some servers ignore `expire-timeout` for `critical` urgency; we never use critical.
- Action buttons are not shown by every server (GNOME shells ignore them for some apps). The default action is therefore the primary action: per-entry default = `OpenItem` (replaces Android's content intent).
- KDE Plasma shows action buttons and supports `replaces_id`. Tested target: Plasma 6 with the Kurobayomi desktop entry installed (`org.kurobayomi.Kurobayomi.desktop`, proposal id).

## 6. Strings

Use the English strings listed in `p1-ui-parity.md` section 7. Do not translate in this area; localisation is a separate task owned by the i18n area.

## 7. Tasks

Covered by T-26a-030..T-26a-037 in `p1-tasks.md`.
