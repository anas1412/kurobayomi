# Area 18 — Manga extensions: UI parity

Only parts of the scope render UI: `MangaExtensionInstallActivity.kt` (an Activity that shows the system install prompt), `MangaExtensionInstallService.kt` (a foreground notification), `ExtensionUpdateNotifier.kt` (a notification). The Compose extension screens are outside this scope (area:17-source-app-layer). This file records the visible surfaces that this area drives, so the Linux UI shows the same states. Source text for the screens is quoted from `spec/app.json` (Browse tab) and `spec/entries.json` (extension details, extension repos), which were extracted from the Aniyomi UI.

Paths are repo-relative to `/home/blackbox/aniyomi-src` unless marked `kurobayomi:`.

---

## 1. Surfaces driven by this area

| # | Surface | Kotlin source | Linux surface | Notes |
|---|---|---|---|---|
| S1 | Browse > Manga Extensions tab, extension rows | area:17-source-app-layer screen, state from `GetMangaExtensionsByType` (this area) | kurobayomi `src/entry.rs` `extensions()` | Section 2 |
| S2 | Browse tab badge on the Manga Extensions tab | `spec/app.json` Browse notes: "extension tabs show a numeric badge = pending updates" | `kurobayomi:src/main.rs:153` sidebar badge | Count = `ext_updates_count` |
| S3 | Sidebar Browse icon badge | `spec/app.json` navigation notes: "Browse icon shows a numeric Badge = pending extension updates (anime + manga)" | `kurobayomi:src/main.rs` navigation | Sum of anime and manga counts; this area supplies the manga part |
| S4 | Update notification | `ExtensionUpdateNotifier.kt:19-44` | `gio::Notification` | Section 4 |
| S5 | Per-row install progress ring | `spec/app.json` "circular progress ring around it while pending/downloading/installing" | GTK spinner or `gtk::ProgressBar` (circular: `gtk::Spinner` plus icon) | Section 3 |
| S6 | Extension details screen (`extension_details`) | `spec/entries.json` `extension_details` notes | area:17-source-app-layer | Uninstall button and install-state text come from this area |
| S7 | Untrusted extension dialog | `spec/entries.json:1807-1812` (`extension_trust`) | area:17-source-app-layer dialog; the Trust button calls `Command::Trust` | Section 5 |
| S8 | Uninstall confirmation | `spec/entries.json:1817-1822` (`remove_private_extension`) | area:17-source-app-layer dialog | Text: `Do you really want to remove "%s" extension?` |
| S9 | Install-service notification | `MangaExtensionInstallService.kt:26-36` | dropped (section 6) | — |
| S10 | System install prompt | `MangaExtensionInstallActivity.kt` | dropped | — |
| S11 | Orphan warning banner in details | `spec/entries.json` `extension_details` notes | area:17-source-app-layer banner (text from spec) | Driven by `is_obsolete` (this area) |
| S12 | Permission banner ("Permissions are needed to install extensions. Tap here to grant.") | `spec/app.json` Browse notes | dropped (Linux needs no install permission) | Section 6 |

---

## 2. Extension rows (S1)

### 2.1 Row anatomy (from `spec/app.json` Browse notes, matched to the rules in this area)
- Leading: 40 dp extension icon. While pending, downloading or installing, a circular progress ring surrounds it and the icon shrinks with 8 dp padding.
- Title: extension name (`bodyMedium`).
- Subtitle (`FlowRow`, `bodySmall`, secondary colour):
  - language (installed rows only; the spec says "language (installed only)");
  - version name;
  - red uppercase tags: `UNTRUSTED`, `ORPHANED`, `18+` (NSFW);
  - ` • Pending`, ` • Downloading`, or ` • Installing` while the row has an install step.
- Trailing buttons:
  - In progress: `Cancel` (X icon).
  - Error: `Retry` (refresh icon).
  - Installed, no update: `Settings` (gear) → details.
  - Installed, update available: `Settings` plus `Update` (get-app icon).
  - Untrusted: `Trust` (verified-user icon) → trust dialog.
  - Available: `Open in WebView` (globe icon) plus `Install` (get-app icon).
- Tap: available → install; installed → details; untrusted → trust dialog.
- Long-press: available → install; installed → uninstall (S8).

### 2.2 Sections (`spec/app.json` Browse notes, in order)
1. `Updates pending` header with a filled `Update all` button at the end (only when updates exist; rows are `updates`).
2. `Installed` header: installed rows (obsolete first, from `by_type`) and untrusted rows, in that order.
3. Available rows grouped under a language header per language from `languages()`.

The Kotlin list is a `LazyColumn`. Empty state: `Well, this is awkward` (no data) or `No results found` (search active), with action `Extension repos`.

### 2.3 Search
Toolbar search icon turns the title into a search field. The field filters rows by name (case-insensitive, substring). Area:17 owns the field; this area's `by_type` is run unfiltered and filtered in the view.

### 2.4 Overflow menu
`Filter` (opens the language filter, area:17-source-app-layer, backed by `languages()`) and `Extension repos` (manga). Source: `spec/app.json` Browse notes.

### 2.5 Pull to refresh
Runs `Command::FindAvailable { force: true }`. Spinner shown for at least 1 second (`spec/app.json` and behaviour 12.1). On Linux: a `gtk::ScrolledWindow` with a refresh row or `Ctrl+R` (keyboard equivalent, Proposal).

### 2.6 Warning banner
Not shown on Linux (section 6, S12).

---

## 3. Install states (S5)

| Step (`InstallStep`) | Row text suffix | Ring | Trailing button |
|---|---|---|---|
| `Idle` | none | none | per 2.1 |
| `Pending` | ` • Pending` | indeterminate | `Cancel` |
| `Downloading` | ` • Downloading` | determinate (byte progress, Proposal) | `Cancel` |
| `Installing` | ` • Installing` | indeterminate | `Cancel` (disabled for the active package, see behaviour 7.12 Shizuku note; Linux: enabled) |
| `Installed` | none (row becomes installed) | none | per 2.1 |
| `Error` | none | none | `Retry` |

Rules:
- The state map is keyed by the **package name** (Proposal; behaviour 11.1), so a split row and its sibling show the same state.
- `Installed` removes the key after the row re-renders (behaviour 12.1 completion).
- Determinate progress is Proposal: the Kotlin code has no byte progress (it polls DownloadManager status). Kurobayomi emits byte counts in the download task (T-18a-034).

---

## 4. Update notification (S4)

Exact copy (`ExtensionUpdateNotifier.kt:24-35`, `base/plurals.xml:89-92`):
- Title, one update: `Extension update available`.
- Title, several: `%d extension updates available` with `%d` = count.
- Body (only when `hide_notification_content` is false): names joined with `", "`, shown with the big-text style.
- Body when the setting is true: no body; the title alone.
- Click action: open the Manga Extensions tab (`NotificationReceiver.openExtensionsPendingActivity`). Linux: `app.activate-action("open-browse-extensions")` (Proposal; action name owned by area:17-source-app-layer's navigation).
- Dismiss on click: yes (`setAutoCancel(true)`).
- Dismissed automatically when the count reaches 0 (behaviour 2.12).

Linux notification details (Proposal): `gio::Notification::new(title)`, `set_body` (names, or none when hidden), `set_default_action("app.open-browse-extensions")`, id `extension-updates`, priority `NotificationPriority::Normal`. No channels on Linux.

---

## 5. Trust dialog (S7)

Copy from `spec/entries.json:1807-1812`:
- Title: `Untrusted extension`.
- Body: `Malicious extensions can read any stored login credentials or execute arbitrary code.` then a blank line then `By trusting this extension, you accept these risks.`
- Buttons: area:17-source-app-layer owns labels (`Trust` / cancel). On confirm → `Command::Trust { pkg }`.

Behaviour: confirm trusts the current version only (behaviour 10.2). Dismiss leaves the row untrusted.

Kurobayomi note: the untrusted state for a Linux package is decided by the repo fingerprint (behaviour 16.4). The dialog is the only way to trust a single version from an unknown repo.

---

## 6. Dropped surfaces (what the user loses)

| Surface | Why dropped | User loss |
|---|---|---|
| S9 foreground install notification `Installing extension…` | No Android foreground service on Linux | System-tray progress for installs. In-app row progress remains. |
| S10 system install prompt | No Android package installer | None; the Linux install needs no confirmation (the user already chose Install). |
| S12 permission banner | Linux needs no "install unknown apps" permission | None. |
| `Open in WebView` (globe) | Android WebView | Opens the extension's `website` in the default browser instead (`gio::AppInfo::launch_default_for_uri`). Same destination, different window. |
| `App info` button in details (spec: "App info (only for shared/system-installed extensions)") | No shared packages | Not present. |
| Enable and disable per source and `Clear cookies` in details | Source toggles stay (behaviour 11.3). `Clear cookies` belongs to the network area (area:12-network-stack) and is out of this area. | Nothing in this area. |

---

## 7. Keyboard and mouse equivalents (Proposal)

| Android gesture | Linux equivalent |
|---|---|
| Tap row | Click row or `Enter` on focused row |
| Long-press row | Right-click (context menu with Install or Uninstall) or `Shift+F10` / `Menu` key |
| Pull to refresh | `Ctrl+R` in the Browse tab, or a refresh button in the header bar |
| Search icon | `Ctrl+F` focuses the search field |
| Cancel (X) in row | `Escape` while the row is focused, plus the button |

These are Proposal; they follow GNOME HIG for list rows (`GtkListView` with `activate` and `GtkGestureClick`). Area:17 implements them.

---

## 8. Responsive behaviour (Proposal)

- Rows use the full window width. At narrow widths (under 360 px) the trailing buttons collapse into a single `…` menu button, containing the same actions.
- The language headers are sticky (`GtkListView` section headers); the Kotlin screen keeps them sticky too (`spec/app.json` notes: "Section header text").
- Tablet and desktop: two columns of rows are not used; the Browse list is one column (matches the spec).

---

## 9. Mockup gap (kurobayomi)

Current mockup `src/entry.rs:311-357` (`extensions(anime: bool)`):

| Gap | Current mockup | Required |
|---|---|---|
| Data | Hard-coded rows (`src/entry.rs:322-327`: "Extension A".."Extension F") | Rows from `ExtensionStore` via `by_type` and `languages` (section 10 of the Rust design) |
| Actions | Local label swap `Install`/`Update` → `Installing` (`src/entry.rs:332-345`) | Real `Command`s and `InstallStep` (section 3) |
| States | None | `Pending`, `Downloading`, `Installing`, `Installed`, `Error`, `Idle` per 3 |
| Obsolete tag | None | `ORPHANED` tag and details banner (S11) |
| NSFW tag | None | `18+` tag (red) |
| Language headers | Four fixed headers | Headers from `languages()` |
| Empty state | None | `Well, this is awkward` / `No results found` plus `Extension repos` action |
| Search | Search entry present, no filter | Filter by name |
| Refresh | None | Pull to refresh, spinner at least 1 s |
| Badge | None | Count from `updates_count` (`src/main.rs:153`) |
| Update all | Button without action | `Command::Update` for each update row |
| Trust | Opens `dialog_of("browse", "extension_trust")` (`src/entry.rs:337`) | Keep the dialog; add the Trust command |
| Open in browser | None | `gio::AppInfo::launch_default_for_uri(website)` |
| Notification | None | Section 4 |
| Permission banner | Not present | Not needed (section 6) |

The mockup's anime branch (`anime == true`) is kept until the anime area ports its loader. This area provides the manga data only.
