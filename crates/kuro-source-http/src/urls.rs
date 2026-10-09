//! URL helpers of the HTTP source base: `setUrlWithoutDomain` and the base-plus-path join (`HttpSource.kt`;
//! T-15a-024, EC-URL-01..07).

/// Keeps the path, query and fragment of an absolute URL, decoded as `java.net.URI` returns them (`getUrlWithoutDomain`,
/// T-16a-044). Spaces are written as `%20` before parsing, so a space comes back as a space. A rejected input and an
/// opaque URL such as `mailto:` come back unchanged. The rules are the anime rules, in `kuro_source::anime`.
pub fn url_without_domain(raw: &str) -> String {
    kuro_source::anime::url_without_domain_manga(raw)
}

/// `baseUrl + path` by plain concatenation (behaviour 2.2). A base that ends with `/` next to a path that starts with
/// `/` would give `//`, so one slash is removed from the base with a warning (EC-URL-07).
pub fn join_base(base: &str, path: &str) -> String {
    if base.ends_with('/') && path.starts_with('/') {
        tracing::warn!(base, "base URL ends with a slash; removing one before the path");
        format!("{}{}", &base[..base.len() - 1], path)
    } else {
        format!("{base}{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_query_and_fragment_are_kept_in_decoded_form() {
        assert_eq!(url_without_domain("https://a.test/x/y?q=1#f"), "/x/y?q=1#f");
        assert_eq!(url_without_domain("https://a.test/a%26b?q=a%26b#x%20y"), "/a&b?q=a&b#x y");
    }

    #[test]
    fn spaces_are_kept_as_spaces() {
        assert_eq!(url_without_domain("https://a.test/a b/c"), "/a b/c");
    }

    #[test]
    fn an_encoded_slash_is_decoded_as_java_decodes_it() {
        assert_eq!(url_without_domain("https://a.test/a%2Fb"), "/a/b");
    }

    #[test]
    fn opaque_and_unparseable_inputs_come_back_unchanged() {
        assert_eq!(url_without_domain("mailto:x@y.test"), "mailto:x@y.test");
        assert_eq!(url_without_domain("https://a.test/with%zz"), "https://a.test/with%zz");
    }

    #[test]
    fn the_join_keeps_one_slash_between_base_and_path() {
        assert_eq!(join_base("https://a.test", "/x"), "https://a.test/x");
    }

    #[test]
    fn a_base_with_a_trailing_slash_loses_one_slash_before_a_slash_path() {
        assert_eq!(join_base("https://a.test/", "/x"), "https://a.test/x");
        assert_eq!(join_base("https://a.test/", "x"), "https://a.test/x");
    }
}
