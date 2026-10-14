//! The release channel of this build. The nightly workflow sets `KB_CHANNEL=nightly` for the whole build; every other build
//! (local, package, stable) leaves it unset. The nightly app keeps its own folders, so it never touches stable data.

/// True in a nightly build.
pub const NIGHTLY: bool = is_nightly(option_env!("KB_CHANNEL"));

// a `const fn`: `==` and `match` on strings are not available in constants
const fn is_nightly(channel: Option<&str>) -> bool {
    let Some(channel) = channel else { return false };
    let (a, b) = (channel.as_bytes(), b"nightly");
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::is_nightly;

    #[test]
    fn only_the_nightly_channel_counts() {
        assert!(is_nightly(Some("nightly")));
        assert!(!is_nightly(Some("stable")));
        assert!(!is_nightly(Some("nightlyx")));
        assert!(!is_nightly(None));
    }
}
