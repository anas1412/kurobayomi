# Area 12-network-stack, part 1: UI parity

The scope files render no Compose screen. Three files produce visible UI: `WebViewInterceptor.kt` and `CloudflareInterceptor.kt` (toasts, and a hidden WebView that the user does not see), `util/system/ToastExtensions.kt` (toast), and `WebViewUtil.kt` (only version text and the WebView availability check). The Networking settings rows that call this area's code are in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:220-307`, owned by `area:47-settings-screens-general`. Their behaviour is specified in `p1-behavior.md` section 8.1 and implemented in this area. Their layout is summarised here because they are the only user-facing controls of this area.

## 1. Toasts produced by this area

| Trigger | Text key | Text (English) | Length | Desktop surface |
|---|---|---|---|---|
| Cloudflare detected, WebKitGTK not available | `information_webview_required` (Kurobayomi: `information_webkit_missing`) | "WebView is required for the app to function" (Kurobayomi: "WebKitGTK is required to solve Cloudflare checks. Install webkitgtk-6.0.") | Long, 3500 ms | `Toaster` overlay (`p1-rust-design.md` 3.4) |
| Solve failed and WebKit is older than 2.42 | `information_webview_outdated` (Kurobayomi: `information_webview_outdated_linux`) | "Please update the WebView app for better compatibility" (Kurobayomi: "Please update WebKitGTK for better compatibility") | Long | `Toaster` |
| Clear cookies row | `cookies_cleared` | "Cookies cleared" | Short, 2000 ms | `Toaster` |
| Clear WebView data success | `webview_data_deleted` | "WebView data cleared" | Short | `Toaster` |
| Clear WebView data failure | `cache_delete_error` | "Error occurred while clearing" | Short | `Toaster` |
| DoH or verbose logging changed | `requires_app_restart` | "Requires app restart to take effect" | Short | `Toaster` |
| UA rejected by OkHttp rules | `error_user_agent_string_invalid` | "Invalid user agent string" | Short | `Toaster` |
| Reset UA | `requires_app_restart` | as above | Short | `Toaster` |

Rules for every toast: shown from any thread (hop to the GTK thread inside `Toaster`), empty text is ignored, at most 5 queued (Kurobayomi divergence), never stacked (one visible at a time).

## 2. Error surface in the browse UI (owned by area 40, consumes `NetError::user_message`)

| Error | Visible text key | Text | Actions |
|---|---|---|---|
| `HttpException(code)` | `exception_http` | "HTTP %d, check website in WebView" | "Open in WebView" (opens area 36 page) |
| `UnknownHost`, offline | `exception_offline` | "No Internet connection" | "Retry" |
| `UnknownHost`, online | `exception_unknown_host` | "Couldn't reach %s" | "Retry" |
| `CloudflareBypassFailed` | `information_cloudflare_bypass_failure` | "Failed to bypass Cloudflare" | "Open in WebView", and the help link `information_cloudflare_help` ("Tap here for help with Cloudflare") |
| Other | raw text | `Display` of `NetError` | "Retry" |

## 3. Cloudflare challenge window (Kurobayomi only, planned P1/P2)

Android has no visible challenge UI: the WebView is created, loaded and destroyed without ever being attached to a window (`WebViewInterceptor.kt:85-91`, `CloudflareInterceptor.kt:71-107`). Linux needs a window for compositor throttling reasons (`p1-topic-cloudflare-and-webview.md` section 3 item 8). Parity rules:

* Default (hidden) mode: a `gtk::Window` of 1x1 px, undecorated, not focusable, not shown in the taskbar (`set_skip_taskbar_hint`), presented and kept behind. The user sees nothing; the request simply takes up to 30 s.
* Visible mode (T-12a-032, used when the hidden solve fails with `Timeout` and the user opts in, or immediately when the spike in T-12a-017 fails): a 960 x 720 window titled `cloudflare_check_title` ("Cloudflare check") with a header label `cloudflare_check_hint` ("Complete the check in this window; it closes by itself when it succeeds."), a `gtk::WebView` filling the content, and a Cancel button (`action_cancel`). Closes on success (`Cleared`), on Cancel (`Failed(Cancelled)`), on the 30 s deadline extended to 120 s in visible mode (divergence: a human needs longer than a script). Keyboard: Escape = Cancel. Mouse: window close button = Cancel.
* Visible mode is modal to nothing: other windows stay usable; a second Cloudflare request for the same domain waits on the same solve (single flight, topic section 2.3) and shows the same window.
* The window uses the app's Tokyonight CSS (addendum rule 6); no theme-dependent colours.

## 4. Settings > Advanced > Networking (owner area 47; behaviour here)

Group header: "Networking" (`label_network`). Rows in order, with Kurobayomi control types:

| # | Row title (key) | Control | Current value / subtitle | Behaviour |
|---|---|---|---|---|
| 1 | "Clear cookies" (`pref_clear_cookies`) | action row | none | Click: `CookieJar::remove_all`, toast "Cookies cleared". No confirmation. |
| 2 | "Clear WebView data" (`pref_clear_webview_data`) | action row | none | Click: `WebViewHost::clear_data` (T-12a-034). Toast success or error. No confirmation (Kotlin has none). |
| 3 | "DNS over HTTPS (DoH)" (`pref_dns_over_https`) | single-choice dropdown | selected label; "Disabled" when -1 | 14 entries: Disabled (-1), Cloudflare (1), Google (2), AdGuard (3), Quad9 (4), AliDNS (5), DNSPod (6), 360 (7), Quad 101 (8), Mullvad (9), Control D (10), Njalla (11), Shecan (12), LibreDNS (13). On change: store, toast "Requires app restart to take effect". Always accepts the selection. Kurobayomi adds a helper line under the control: "Local names such as localhost are not resolved while DoH is on." (Kotlin does not show it; a Rust-side addition to explain the rule in `p1-topic-doh.md` section 2.) |
| 4 | "Default user agent string" (`pref_user_agent_string`) | text entry (dialog) | the value, shown as subtitle in quotes | See section 4.1. |
| 5 | "Reset default user agent string" (`pref_reset_user_agent_string`) | action row | none | Enabled only while the value differs from the default. Click: delete the preference, toast "Requires app restart to take effect". |

Verbose logging (`pref_verbose_logging`, summary `pref_verbose_logging_summary`) is a switch row in the Advanced group owned by area 47; it changes `verbose_logging` and shows the restart toast. It is listed here because it controls this area's logging.

### 4.1 User agent dialog

* Opens with the current value in a single-line entry, selected all.
* OK enabled only when the text is non-blank AND differs from the stored value (`EditTextPreferenceWidget` rules, `p1-behavior.md` 8.1 item 4). Trailing icon: error icon while blank, clear button otherwise.
* Validation on OK: `HeaderValue::from_str` (equivalent of `Headers.Builder().add`). Failure shows the toast "Invalid user agent string" and keeps the dialog open with the entry text. Success stores the value and shows the restart toast.
* Enter key in the entry = OK. Escape = Cancel. Keyboard focus is in the entry when the dialog opens.
* Kurobayomi control: `gtk::Entry` in an `adw`-free `gtk::Window` modal dialog (no libadwaita, per the project scope). Trailing icon via `set_icon_from_icon_name(Secondary)` with `dialog-error-symbolic` (blank) and `edit-clear-symbolic` (non-blank), both resolved through the bundled symbolic set (addendum rule 6).

## 5. States of each control

| Control | Loading | Empty | Error | Disabled |
|---|---|---|---|---|
| Clear cookies | n/a (synchronous store clear; disk write is debounced) | n/a | none (the store cannot fail to clear; a disk failure is logged) | never |
| Clear WebView data | row shows a spinner while the directory delete runs (can take seconds) | n/a | toast `cache_delete_error` | while another clear is running |
| DoH dropdown | n/a | never empty | n/a | never |
| UA entry | n/a | blank: OK disabled, error icon | invalid header chars: toast and dialog stays | never |
| Reset UA | n/a | n/a | n/a | value equals the default |

## 6. Keyboard and mouse equivalents

* Every action row is reachable with Tab and activates with Space or Enter.
* Dropdown: arrow keys change selection while focused; typing a letter jumps to the first matching label.
* Dialogs: Escape cancels, Enter confirms (default button).
* Mouse: single click activates rows; no hover-only controls in this area.

## 7. Responsive behaviour

The Networking rows are a single column in the settings page; at window widths below 600 px the subtitle wraps under the title (no truncation of the UA string beyond two lines, with the full value in the dialog). The challenge window (visible mode) has a minimum size of 480 x 360 and scales its WebView.

## 8. Animations

None in this area. The toast fades out over 200 ms (Revealer transition) and the challenge window has no transition. Android uses the platform toast animation; the 200 ms fade is the Kurobayomi equivalent.

## 9. What the mockup lacks

* `spec/settings.json` Networking rows exist (lines around 2805 to 2843) but there is no handler, no DoH dropdown control, and no UA dialog in `src/settings.rs`.
* No `Toaster` exists; `src/player.rs:39` has a player-specific flash helper that is not reusable for app-wide toasts.
* No challenge window and no `WebViewHost` implementation.
* The DoH helper line (section 4 row 3) and the cookie-import dialog (T-12a-033, strings `cookie_import_title` and `cookie_import_hint`) are new and have no spec entry. The spec must be extended by area 47 (settings) or this area's tasks T-12a-031 and T-12a-033 carry the content.
* No error surface wiring from `NetError::user_message` into browse screens (area 40).
