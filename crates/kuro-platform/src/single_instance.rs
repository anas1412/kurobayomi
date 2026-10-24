//! Single instance: GApplication forwards a second launch (its command line and files) to the running app, on every
//! platform. This module only decides whether the app asks to be unique.

/// Dev environment variables that force a non-unique instance (several windows can run side by side).
pub const NON_UNIQUE_ENV_VARS: [&str; 6] = [
    "KUROBAYOMI_SMOKE",
    "KUROBAYOMI_SHOT",
    "KUROBAYOMI_SHEET",
    "KUROBAYOMI_SIZE",
    "KUROBAYOMI_RAIL",
    "KUROBAYOMI_OPEN",
];

/// True when no dev variable forces a second instance. `lookup` reads one variable (`std::env::var_os` in the app).
pub fn is_unique(lookup: impl Fn(&str) -> bool) -> bool {
    !NON_UNIQUE_ENV_VARS.iter().any(|name| lookup(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_variables_disable_uniqueness() {
        assert!(is_unique(|_| false));
        assert!(!is_unique(|name| name == "KUROBAYOMI_SHOT"));
    }
}
