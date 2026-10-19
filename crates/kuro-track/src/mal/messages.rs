//! English texts of the MyAnimeList tracker, in one table (T-29a-034, T-29a-051). Errors and toasts read these
//! constants, so a translation later changes this file only. The GUI shows the same texts through
//! `messages::english`.

pub const NOT_AUTHENTICATED: &str = "MAL: User is not authenticated";
pub const REFRESH_FAILED: &str = "MAL: Failed to refresh account token";
pub const LOGIN_EXPIRED: &str = "MAL: Login has expired";
/// Followed by a space and the status code (`HttpException.kt:13`).
pub const HTTP_ERROR: &str = "HTTP error";
pub const NO_CLIENT_ID: &str = "Set a MyAnimeList client id to log in";
pub const NO_PENDING_LOGIN: &str = "MAL: No login is in progress";
pub const UNKNOWN_STATUS: &str = "MAL: Unknown status code";
pub const UNKNOWN_SCORE: &str = "MAL: Unknown score";

/// Toast when the redirect did not arrive within five minutes (E-LOGIN-5).
pub const LOGIN_TIMED_OUT: &str = "Login timed out. Start again in Kurobayomi.";
/// Toast when the loopback redirect port is taken by another program, so the browser login cannot come back.
pub const PORT_BUSY: &str = "MyAnimeList login needs port 8765, which another program is using";
/// Toast after the user refused consent, or the redirect carried no code (D-04).
pub const LOGIN_NOT_COMPLETED: &str = "Login was not completed";
/// Error text of the paste field when the input is neither a redirect URL nor a code (E-LOGIN-9).
pub const PASTE_INVALID: &str = "Paste the full redirect URL or the code";
/// Settings row subtitle while the loopback login waits (T-29a-047).
pub const WAITING_FOR_LOGIN: &str = "Waiting for MyAnimeList...";
/// Settings row subtitle when the stored login expired (E-UI-2, T-29a-047).
pub const LOG_IN_AGAIN: &str = "Log in again";
/// Toast when a refresh found the title missing and added it again (D-10).
pub const RE_ADDED: &str = "Re-added to MyAnimeList";
