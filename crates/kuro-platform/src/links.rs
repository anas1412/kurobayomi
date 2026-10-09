//! Help links (`Constants.kt`). They open through [`crate::opener::Opener`].

/// The troubleshooting guide, opened from the Help rows and the browse, search and migration help buttons.
pub const URL_HELP: &str = "https://aniyomi.org/docs/guides/troubleshooting/";

/// The FAQ entry the upcoming screens link to.
pub const URL_HELP_UPCOMING: &str = "https://aniyomi.org/docs/faq/updates/upcoming";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_urls_are_the_aniyomi_pages() {
        assert_eq!(URL_HELP, "https://aniyomi.org/docs/guides/troubleshooting/");
        assert_eq!(URL_HELP_UPCOMING, "https://aniyomi.org/docs/faq/updates/upcoming");
    }
}
