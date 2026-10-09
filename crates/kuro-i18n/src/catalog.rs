//! Fluent bundles for one locale chain, with lookup down the chain (plan 51-i18n section 3).

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentError, FluentResource, FluentValue};
use tracing::warn;
use unic_langid::LanguageIdentifier;

use crate::locale::LocaleInfo;

/// Bundles for a locale chain such as `["pt-BR", "pt", "en"]`. Lookup walks the chain in order.
/// An empty bundle with what every catalogue needs: no isolation marks (no U+2068 or U+2069),
/// `NUMBER()`, and zero-padded integers for `minimumIntegerDigits`.
pub(crate) fn new_bundle(langid: LanguageIdentifier) -> FluentBundle<FluentResource> {
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    if let Err(error) = bundle.add_builtins() {
        warn!("[i18n] NUMBER() unavailable: {error}");
    }
    bundle.set_formatter(Some(integer_digits));
    bundle
}

/// fluent-bundle 0.16 drops `minimumIntegerDigits` in its default formatter, so `%02d` would
/// print `3`. This prints the padded integer. Other values use the default formatter.
fn integer_digits<M>(value: &FluentValue, _memoizer: &M) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    let width = number.options.minimum_integer_digits?;
    if number.value.fract() != 0.0 || number.options.minimum_fraction_digits.is_some() {
        return None;
    }
    Some(format!("{:0width$}", number.value as i64))
}

pub struct Catalog {
    info: LocaleInfo,
    bundles: Vec<FluentBundle<FluentResource>>,
}

impl Catalog {
    /// Builds one bundle per chain entry that has a source. `en` is always appended, so the
    /// English text is the last fallback. Isolation marks are off (no U+2068 or U+2069).
    pub fn build(chain: &[&'static str], sources: &[(&str, &str)]) -> Catalog {
        let mut tags: Vec<&str> = chain.to_vec();
        if !tags.contains(&"en") {
            tags.push("en");
        }
        let info = LocaleInfo::new(tags[0]);
        let mut bundles = Vec::with_capacity(tags.len());
        for tag in tags {
            let Some((_, text)) = sources.iter().find(|(t, _)| *t == tag) else {
                warn!("[i18n] no catalogue for {tag}");
                continue;
            };
            let Ok(langid) = tag.parse::<LanguageIdentifier>() else {
                warn!("[i18n] bad locale tag {tag}");
                continue;
            };
            let resource = match FluentResource::try_new(text.to_string()) {
                Ok(resource) => resource,
                Err((resource, errors)) => {
                    warn!(
                        "[i18n] {tag}: {} parse error(s), valid entries kept",
                        errors.len()
                    );
                    resource
                }
            };
            let mut bundle = new_bundle(langid);
            if let Err(errors) = bundle.add_resource(resource) {
                warn!(
                    "[i18n] {tag}: {} duplicate message(s) ignored",
                    errors.len()
                );
            }
            bundles.push(bundle);
        }
        Catalog { info, bundles }
    }

    pub fn info(&self) -> LocaleInfo {
        self.info
    }

    /// Formats `id` with the first bundle in the chain that has a value for it. Returns the text
    /// and any Fluent errors (missing or extra arguments). `None` when no bundle has the id.
    pub fn format(
        &self,
        id: &str,
        args: Option<&FluentArgs>,
    ) -> Option<(String, Vec<FluentError>)> {
        for bundle in &self.bundles {
            let Some(message) = bundle.get_message(id) else {
                continue;
            };
            let Some(pattern) = message.value() else {
                continue;
            };
            let mut errors = Vec::new();
            let text = bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned();
            return Some((text, errors));
        }
        None
    }
}
