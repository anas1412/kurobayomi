//! The `manifest.toml` of one extension package (`p1-data-and-config.md` D8). Unknown keys are ignored, so a newer
//! manifest still loads on this host.

use serde::Deserialize;

use crate::error::LoadError;

/// The fields the loader reads. `api` is the WIT world version as `major.minor`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    pub pkg: String,
    pub name: String,
    pub version: String,
    pub version_code: i64,
    pub api: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub nsfw: bool,
}

impl Manifest {
    /// Parses the TOML text and checks the api version.
    pub fn parse(text: &str) -> Result<Self, LoadError> {
        let manifest: Manifest = toml_edit::de::from_str(text).map_err(|err| LoadError::Manifest(err.to_string()))?;
        manifest.check_api()?;
        Ok(manifest)
    }

    /// The major version of `api` must be 1 (T-20a-001: an `api` outside `1.x` is rejected).
    fn check_api(&self) -> Result<(), LoadError> {
        match self.api.split('.').next().and_then(|major| major.parse::<u32>().ok()) {
            Some(1) => Ok(()),
            _ => Err(LoadError::UnsupportedApi(self.api.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
pkg = "app.kurobayomi.ext.en.example"
name = "Example"
version = "1.4.2"
version_code = 42
api = "1.4"
lang = "en"
nsfw = false
future_key = "ignored"
"#;

    #[test]
    fn a_valid_manifest_parses_and_unknown_keys_are_ignored() {
        let manifest = Manifest::parse(VALID).expect("valid");
        assert_eq!(manifest.pkg, "app.kurobayomi.ext.en.example");
        assert_eq!(manifest.version_code, 42);
        assert_eq!(manifest.lang, "en");
        assert!(!manifest.nsfw);
    }

    #[test]
    fn any_1_x_api_is_accepted_and_other_majors_are_rejected() {
        for api in ["1", "1.0", "1.5"] {
            let text = VALID.replace("api = \"1.4\"", &format!("api = \"{api}\""));
            assert!(Manifest::parse(&text).is_ok(), "api {api} is 1.x");
        }
        for api in ["2.0", "0.9", "x", ""] {
            let text = VALID.replace("api = \"1.4\"", &format!("api = \"{api}\""));
            assert_eq!(Manifest::parse(&text), Err(LoadError::UnsupportedApi(api.into())), "api {api:?}");
        }
    }

    #[test]
    fn a_manifest_without_the_required_fields_is_a_manifest_error() {
        let missing_pkg = VALID.replace("pkg = \"app.kurobayomi.ext.en.example\"\n", "");
        assert!(matches!(Manifest::parse(&missing_pkg), Err(LoadError::Manifest(_))));
        assert!(matches!(Manifest::parse("pkg = "), Err(LoadError::Manifest(_))));
    }
}
