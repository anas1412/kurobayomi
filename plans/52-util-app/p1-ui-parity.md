# 52-util-app — UI parity (only the scope functions that render or trigger UI)

Scope files that render or trigger UI: `ViewExtensions.kt`, `ContextExtensions.kt` (toasts, clipboard, browser, reader theme), `AuthenticatorUtil.kt` (lock prompt), `NotificationExtensions.kt`, `CheckboxState.kt`, `PreferenceMutableState.kt`, `DisplayExtensions.kt` (layout), `IntentExtensions.kt` (share), `CrashLogUtil.kt` (crash dump), `AnimeSourceUtil.kt` / `MangaSourceUtil.kt` (loading gate), `DrawableExtensions.kt` (image), `CollectionUtils.kt` (list separators). Other scope files are pure logic (see `p1-inventory.md`).

Desktop equivalents assume KDE Plasma with GTK 4.16 and the user's GTK theme. Tokyo Night colours come from the user's theme; no colour is hard-coded here.

---

## 1. Toasts (`Context.toast`, `Activity.toast`, `withUIContext { toast }`)

Upstream: Android `Toast` (short or long), bottom centre, no action, no dismiss.

Desktop replacement: `ui::toast::Toaster`, an in-window pill anchored bottom-centre of the main window overlay (`gtk::Overlay`), same look as `pill()` in `src/player.rs:39`.

| Property | Value | Source |
|---|---|---|
| Position | bottom-centre, 24 px above bottom edge (Proposal) | Android default |
| Duration short | 2000 ms | Android `LENGTH_SHORT` |
| Duration long | 3500 ms | Android `LENGTH_LONG` |
| Stacking | one at a time; a new message replaces the current one | Android replaces the toast |
| Max width | 80% of window width; wraps | Android |
| Dismiss | on timeout or click | Android: no click (Proposal adds click-to-dismiss, no loss) |
| Accessibility | `accessible-role: alert`; text announced | Proposal |

Messages that go through toast (exact keys):
- `copied_to_clipboard`: `Copied to clipboard:\n%1$s` (`%1$s` = 50-char centre-truncated content; behaviour 9.6)
- `clipboard_copy_error`: `Failed to copy to clipboard`
- `Failed to get logs` (LITERAL, `CrashLogUtil.kt:40`)
- raw `e.message` from `openInBrowser` failure (`ContextExtensions.kt:85`, unlocalised)
- raw `errString` from a failed lock prompt (`AuthenticatorUtil.kt:77`)

Empty or null message: nothing shown (`e.message` can be null; upstream then shows an empty toast; Rust skips it).

---

## 2. Clipboard copy

Entry points: any "Copy" action (`copyToClipboard(label, content)`).

States
- Idle: action button enabled.
- Blank content: no action and no toast (`ContextExtensions.kt:40`).
- Success: toast `Copied to clipboard:` plus truncated content, short duration.
- Failure: toast `Failed to copy to clipboard`, short duration.

Keyboard and mouse: `Ctrl+C` on a focused selectable label copies the selection (GTK default for `gtk::Label` with `selectable`); the explicit copy button is an icon button with tooltip `Copy to clipboard` (mockup `src/settings.rs:158` already uses this icon `edit-copy-symbolic`).

---

## 3. Open in browser

Entry: links in entry details (MAL/AniList/Simkl pages), source web view, settings "about" links.

States
- Launching: no spinner (portal call is fast). Button stays enabled.
- Success: the desktop browser opens; the app does nothing else.
- Failure: toast with the error's display text (raw).

Keyboard: `Enter` activates a link row; `Ctrl+Click` opens in new tab is not applicable (desktop default browser).

`forceDefaultBrowser` is removed: the desktop default browser is always used (no UI option). Settings that offered "open in default browser" lose that toggle; no user-visible difference on KDE.

---

## 4. Share and save

Upstream: Android share sheet (`toShareIntent`) for crash logs, covers, backups, text.

Desktop:
- "Share" button becomes "Save…" with a native file dialog (portal) defaulting to `~/Downloads` and the suggested file name.
- Dialog states: idle, open (modal to main window), saved (toast `Saved` is NOT upstream; no toast on success; the dialog closing is the feedback), cancelled (nothing happens).
- Save errors: toast with `error.to_string()`.
- Link sharing (http/https): "Copy link" (clipboard, section 2).

Keyboard: `Ctrl+S` on the cover page triggers save (Proposal). `Escape` cancels the dialog.

---

## 5. App lock prompt (`AuthenticatorUtil.authenticate`)

Upstream: system biometric or device-credential sheet (`BiometricPrompt`), title passed by caller, subtitle default `Authenticate to confirm change` (`confirm_lock_change`).

Desktop: modal `gtk::Window` (transient for main window, deletable = false):
- Title: caller-provided title (e.g. the setting name).
- Subtitle: `Authenticate to confirm change` by default.
- Content: `gtk::PasswordEntry` labelled with the user's name (`glib::user_name()`), a primary button `Confirm` (LITERAL, Proposal; upstream button is system-provided), secondary `Cancel`.
- States:
  - Idle: entry focused.
  - Checking: entry and buttons disabled, spinner visible (PAM call on blocking thread).
  - Wrong password: entry cleared, error line below entry shows PAM text (raw, as upstream toasts raw text); focus returns to entry.
  - Success: window closes; the caller resumes (`resume(true)` in upstream).
  - Cancel / close: resumes with `false`.
  - Unsupported (no PAM service file): the lock setting cannot be enabled; settings row shows a subtitle `Authentication is not available on this system` (Proposal wording; upstream has no such case, since it simply turns the setting off at `SecureActivityDelegate.kt:117-119`).
- Keyboard: `Enter` confirms, `Escape` cancels. Mouse: standard.
- Guard: `AuthenticatorUtil.isAuthenticating` is set while the window is open; the lock screen never opens twice.

---

## 6. Lock screen (`UnlockActivity`, area 36) and the authenticator guard

Not owned here; the util contract is in section 5. The lock screen itself is a full-window page shown on app resume when `requireUnlock` is set (behaviour 24.1, area 36). Desktop: the page replaces the content stack; the sidebar is hidden; `Escape` does nothing (no bypass).

---

## 7. Notifications (`NotificationExtensions.kt`)

Upstream surfaces: download progress, library update result, backup result, app update. Their content is owned by areas 23-26, 35.

Desktop rules
- A notification is shown only when the window is not focused OR the notification is an error (Proposal; upstream shows all notifications subject to the OS. The GNOME/KDE portal shows them regardless, so this rule is a Proposal to reduce noise; the user can disable it in settings (area 47)).
- Default action (click): focus the main window and open the relevant page (`app.open-page` action).
- Priority: `Normal` default, `Low` for progress, `High` for errors and "download failed".
- Cancel: when a job finishes, its progress notification is withdrawn (`cancelNotification(id)`).
- No accent colour (dropped).
- Batch variant posts several items in one call (Android `notify(List)`); the portal coalesces them.

Permission: no prompt. If the desktop refuses (notifications disabled in KDE settings), the app shows an info row in settings: `Notifications are disabled in system settings` (Proposal wording).

---

## 8. Checkbox tri-state (`CheckboxState.kt`)

Widget: `gtk::CheckButton` with `inconsistent`.

| State (`CheckboxState.TriState`) | `inconsistent` | `active` | Visual |
|---|---|---|---|
| `None` | false | false | empty box |
| `Include` | false | true | checked box |
| `Exclude` | true | false | dash box |

Keyboard: `Space` toggles (GTK default). Click cycle is defined by the filter dialog (area 37/38, see behaviour section 2).

Accessibility: label text and state announced (`accessible-role: checkbox`, state `mixed` for `Exclude`).

---

## 9. Preference binding (`PreferenceMutableState.kt`)

No visible UI. Behaviour: a widget bound to a preference updates when the preference changes from anywhere (other window, settings import, migration). The UI reflects the value after the watch echo (behaviour section 1).

Edge: two windows bound to the same preference update on each other's writes.

---

## 10. Loading gate (`ifMangaSourcesLoaded`, `ifAnimeSourcesLoaded`)

States of the Browse and Extensions pages while sources load:
- Loading: centred `gtk::Spinner` (spinning) with caption `Loading…` (Proposal; upstream shows the Compose progress indicator without text).
- Loaded: list rendered once.
- Failed to load: the source manager does not report failure upstream; the Rust port shows the empty state of section 12 after 30 s without a value (Proposal, timeout is new).

---

## 11. Layout (`DisplayExtensions.kt`)

Upstream: tablet UI (navigation rail) vs phone UI (bottom nav). Mode from `tablet_ui_mode`: Auto, Always, Landscape, Never.

Desktop: `Layout::Wide` (sidebar with labels, always visible) vs `Layout::Compact` (icon-only sidebar of 56 px width). Breakpoints (logical px, behaviour 18.3): AUTO uses 720 px as the wide threshold (the forced-tablet minimum); LANDSCAPE mode uses `width > height`; ALWAYS = wide; NEVER = compact.

Responsive behaviour
- Window resize across the threshold re-lays the page within 100 ms (no animation: `gtk::Stack` transition `none` on layout change).
- Minimum window size: 360 x 480 (Proposal). Below that the content scrolls horizontally is NOT allowed; the sidebar collapses to compact first.
- Maximised windows use the current size.

---

## 12. Empty and error states touched by this area

| Situation | Upstream | Desktop |
|---|---|---|
| No airing info for an anime | the "Next episode" line is hidden (caller behaviour) | same; no empty message |
| Airing lookup failed | silent `(1, 0)` (behaviour 7.9) | silent; no toast (upstream has none) |
| Crash log dump failed | toast `Failed to get logs` | same |
| No tracker linked for airing | next-episode line hidden | same |
| Simkl entry absent | `(episode 1, time 0)` | same |

---

## 13. Relative date display (`DateText.kt`, release dates)

Visible strings come from section 10.1 of behaviour. Examples on a Friday (en):
- Released 2 h ago: `2 hours ago`
- Released 30 min ago: `30 minutes ago`
- Released 30 s ago or 30 s ahead: `Now`
- Released yesterday: `Yesterday`
- Released 3 days ago: `3 days ago`
- Released 9 days ago: absolute short date (e.g. `10/1/26`)
- Releases tomorrow: `Tomorrow`; in 5 days: `In 5 days`; in 10 days: absolute date

Tooltip on the relative text (Proposal): the absolute date-time (`short_datetime`). Upstream shows no tooltip.

---

## 14. Crash dump (`CrashLogUtil.dumpLogs`)

Triggers: "Dump crash logs" action in advanced settings; crash screen (area 36).

States
- Idle: action row.
- Working: row subtitle `Collecting logs…` (Proposal; upstream has no progress state; the call is synchronous with a non-cancellable context).
- Done: save dialog opens (section 4). No toast.
- Failed: toast `Failed to get logs`.

The panic hook (behaviour 8.4) writes a file silently and shows the crash screen on next start (area 36).

---

## 15. Storage size values (`toSize`, `dir_size`)

Shown in storage settings (area 47 / 42). Format per behaviour 9.2 (`1.50 MB`, `999 B`). A size that is still computing shows `…` (Proposal). A failed size shows `—` (em dash, Proposal).

---

## 16. Reader background (`createReaderThemeContext`)

Visible effect: reader page background. Values from reader background chips (behaviour 9.8): White (0), Black (1, default), Gray (2), Auto (3).
- Dark background: text and UI in the reader use the dark token set.
- Light background: light token set.
- Auto follows `themeMode`: System follows the desktop colour scheme (portal), Dark / Light force.

Transition: changing the chip repaints the reader immediately (no animation).

---

## 17. Animations (`animatorDurationScale`)

Visible: crossfade of page images in the reader, cover fades.
- Scale 1.0: crossfade 300 ms.
- Scale 0.0 (GTK animations disabled): crossfade 0 ms (instant), page animation minimum 1 ms.
- Scale other: `(300 * scale).toInt()` ms.

Keyboard/mouse: none.

---

## 18. Tap-zone inversion (`RectF.invert`)

Visible in reader settings "Invert tapping" chips: None, Horizontal, Vertical, Both. Labels from `tapping_inverted_*` (`None`, `Horizontal`, `Vertical`, `Both`). Desktop: mouse click zones; keyboard arrows are not affected by inversion (upstream: arrows are not affected either, verify in area 43).

---

## 19. Visibility check (`isVisibleOnScreen`)

Not visible; used to pause work for off-screen pages (area 44). Desktop: uses `compute_bounds` against the window.

---

## 20. Popup menu (`View.popupMenu`)

Used in download holder rows (`AnimeDownloadHolder.kt`, `MangaDownloadHolder.kt`) and the chapter/episode overflow.

Desktop: `gtk::PopoverMenu` attached to a `gtk::MenuButton` (icon `view-more-symbolic`, tooltip `More`, mockup `src/ui.rs:216`).
- Opens on click or `F10` / `Menu` key (Proposal; GTK menu button activates on `Space`/`Enter`).
- Items are actions; the menu closes after activation.
- Right-click on a row opens the same menu (Proposal; upstream long-press).
- Keyboard navigation: `Up`/`Down` move, `Enter` activates, `Escape` closes (GTK default).

---

## 21. Mockup gaps (what the existing mockup lacks)

| Item | Mockup status | Needed |
|---|---|---|
| Toaster (section 1) | `pill()` in `src/player.rs:39` exists only for the player | `ui::toast::Toaster` shared by all pages, queue of one, timeouts |
| Native save dialog (section 4) | none | `kuro_platform::open::save_dialog` |
| Copy action wired to clipboard | icon buttons exist (`src/settings.rs:158`, `src/entry.rs:150`) but are no-ops | connect to `gdk` clipboard + toast |
| Tri-state filter widget | `Item::Tristate` in spec (`src/spec.rs:8`) rendered? (verify in `src/ui.rs:221-353`) | `ui::checkbox::tri_state` |
| Notification surfaces | none | `kuro_platform::notify::Notifier` + default action `app.open-page` |
| App lock prompt | none | modal window (section 5) |
| Layout breakpoints (section 11) | `connect_default_width_notify` only triggers a re-render (`src/main.rs:68`) | `ui::layout::apply` with `Layout` |
| Loading gate (section 10) | none | spinner state per page |
| Popup menu | `ui::menu_button` (`src/ui.rs:202`) builds a Popover with plain buttons, no PopoverMenu model | `ui::popover::from_menu(gio::Menu)` |
| Reader background | none | `reader_background()` mapping + CSS classes |
| Crash dump working state | none | row subtitle state (section 14) |
| Relative date tooltip | none | absolute tooltip (section 13) |
| Storage size placeholders | `storage()` at `src/screens.rs:468` is static | computed sizes with `…`/`—` states |
| Minimum window size | not set | `set_size_request(360, 480)` on the main window |

---

## 22. Keyboard and mouse summary

| Action | Keyboard | Mouse |
|---|---|---|
| Copy | `Ctrl+C` on selection; copy button focusable with `Tab` | click copy icon |
| Open link | `Enter` on link row | click |
| Save / share | `Ctrl+S` on cover page (Proposal) | click Save |
| Lock prompt | `Enter` confirm, `Escape` cancel | click buttons |
| Notification action | activate the notification (portal) | click notification |
| Popup menu | `F10`, `Menu` key, `Space`/`Enter` on button | click or right-click row (Proposal) |
| Tri-state toggle | `Space` | click |
| Layout toggle | window resize; no shortcut | drag window edge |
