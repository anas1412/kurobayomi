//! The browser step of the Simkl login (B-3.7): the authorise URL and the parsing of the redirect that carries the code.
//! Opening the URL and the dialog are GUI work. The parsing of a pasted redirect or code is here (TST-56).

use url::Url;

use super::api::{CLIENT_ID, REDIRECT_URL, SITE_URL};

/// The URL the browser opens. Parameters come in the source's order and encode the same way as `Uri.Builder`
/// (`redirect_uri=aniyomi%3A%2F%2Fsimkl-auth`).
pub fn authorize_url() -> String {
    let mut url = Url::parse(&format!("{SITE_URL}/oauth/authorize"))
        .expect("the Simkl site URL is a constant that parses");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", REDIRECT_URL);
    url.into()
}

/// The `code` parameter of a redirect such as `aniyomi://simkl-auth?code=...`. `None` when there is no non-empty code.
pub fn code_from_redirect(redirect: &str) -> Option<String> {
    let url = Url::parse(redirect).ok()?;
    url.query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.into_owned())
        .filter(|code| !code.is_empty())
}

/// The code from what the user pastes into the login dialog (TST-56). It takes the full redirect, which must be the
/// Simkl host, or the bare code: 20 to 200 characters of letters, digits, `.`, `_` and `-`. Any other link gives none,
/// so a page on another site never completes a login.
pub fn code_from_paste(input: &str) -> Option<String> {
    let input = input.trim();
    if input.contains("://") {
        return if is_simkl_redirect(input) {
            code_from_redirect(input)
        } else {
            None
        };
    }
    let plausible = (20..=200).contains(&input.len())
        && input
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    plausible.then(|| input.to_owned())
}

/// True for a launch argument that is the Simkl redirect (`aniyomi://simkl-auth...`), so the app can route it here
/// from startup or from a second instance's activation and leave other `aniyomi://` links alone.
pub fn is_simkl_redirect(uri: &str) -> bool {
    Url::parse(uri).is_ok_and(|u| u.scheme() == "aniyomi" && u.host_str() == Some("simkl-auth"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_32a_031_only_the_simkl_auth_host_is_a_simkl_redirect() {
        assert!(is_simkl_redirect("aniyomi://simkl-auth?code=x"));
        assert!(is_simkl_redirect("aniyomi://simkl-auth"));
        assert!(!is_simkl_redirect("aniyomi://add-repo?url=x"));
        assert!(!is_simkl_redirect("https://simkl-auth/?code=x"));
        assert!(!is_simkl_redirect("not a url"));
    }

    #[test]
    fn t_32a_031_the_authorise_url_is_byte_equal_to_the_source_form() {
        assert_eq!(
            authorize_url(),
            "https://simkl.com/oauth/authorize?response_type=code&client_id=aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth"
        );
    }

    #[test]
    fn t_32a_031_the_code_is_read_from_the_redirect() {
        assert_eq!(
            code_from_redirect("aniyomi://simkl-auth?code=abc.DEF-1").as_deref(),
            Some("abc.DEF-1")
        );
    }

    #[test]
    fn t_32a_048_a_pasted_redirect_or_bare_code_gives_the_code() {
        assert_eq!(
            code_from_paste("aniyomi://simkl-auth?code=ABC123").as_deref(),
            Some("ABC123")
        );
        let bare = "a1.b2_c3-d4e5f6g7h8i9j0k1";
        assert_eq!(
            code_from_paste(&format!("  {bare} ")).as_deref(),
            Some(bare)
        );
    }

    #[test]
    fn t_32a_048_other_hosts_and_implausible_codes_give_none() {
        assert_eq!(code_from_paste("https://example.com/?code=x"), None);
        assert_eq!(code_from_paste("aniyomi://add-repo?code=abc"), None);
        assert_eq!(code_from_paste("short"), None);
        assert_eq!(
            code_from_paste("has a space in it, and more than twenty"),
            None
        );
    }

    #[test]
    fn t_32a_031_a_redirect_without_a_code_or_with_an_empty_one_has_no_code() {
        assert_eq!(code_from_redirect("aniyomi://simkl-auth"), None);
        assert_eq!(code_from_redirect("aniyomi://simkl-auth?code="), None);
        assert_eq!(code_from_redirect("not a url"), None);
    }
}
