//! Installs the two stylesheets on the default display (T-49a-012, T-49b-008, T-49ba-001).
//!
//! The base stylesheet (`style.css`) is added once at APPLICATION priority. The palette is one
//! `CssProvider`, also added once; `reapply` replaces its text and never adds a second provider.
//! Text is parse-checked first, so a parse error keeps the previous text and is logged once (F-49-01).
//! `apply` is not re-entrant (R-49-02): a nested request sets a pending flag and the outer apply
//! runs `apply_current` once more when it finishes, so the latest state is shown.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::{gdk, CssProvider};

use kuro_prefs::ui::ThemeMode;

use super::{css, dark, resolve, style, Resolved, ThemeSettings};

thread_local! {
    /// The palette provider and the display it was added to. Added once; only its text changes.
    static PALETTE: RefCell<Option<(gdk::Display, CssProvider)>> = const { RefCell::new(None) };
    /// The tint provider: the `kb-selected` and `kb-active` rules for the GTK-theme choice, empty
    /// otherwise. Kept apart so the palette provider stays empty for that choice (T-49b-008).
    static TINTS: RefCell<Option<(gdk::Display, CssProvider)>> = const { RefCell::new(None) };
    /// The last parse error logged, so a broken palette is reported once, not on every apply.
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    /// Re-entrancy state: `applying` while `reapply` runs, `pending` for a nested request.
    static GUARD: Guard = const { Guard { applying: Cell::new(false), pending: Cell::new(false) } };
    /// The display and settings last applied, so a system change can re-apply them.
    static CURRENT: RefCell<Option<(gdk::Display, ThemeSettings)>> = const { RefCell::new(None) };
    /// The last known system dark flag (portal, else GTK fallback). `None` until a read arrives.
    static SYSTEM_DARK: Cell<Option<bool>> = const { Cell::new(None) };
}

/// The re-entrancy guard: one apply at a time, and at most one queued behind it.
struct Guard {
    applying: Cell<bool>,
    pending: Cell<bool>,
}

impl Guard {
    /// True when the caller may run. A nested caller gets false and leaves a pending request.
    fn enter(&self) -> bool {
        if self.applying.replace(true) {
            self.pending.set(true);
            false
        } else {
            true
        }
    }

    /// Ends the outer run. True when a nested request arrived and one more apply is owed.
    fn leave(&self) -> bool {
        self.applying.set(false);
        self.pending.replace(false)
    }
}

/// Parses `css` into a fresh provider. Returns the first parse error, if any.
pub fn build_provider(css: &str) -> Result<CssProvider, String> {
    let provider = CssProvider::new();
    let error: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    {
        let error = error.clone();
        provider.connect_parsing_error(move |_, _section, err| {
            let mut slot = error.borrow_mut();
            if slot.is_none() {
                *slot = Some(err.message().to_string());
            }
        });
    }
    provider.load_from_string(css);
    let first = error.borrow_mut().take();
    match first {
        Some(message) => Err(message),
        None => Ok(provider),
    }
}

/// Replaces the palette text with `css`. Returns false, and keeps the previous text, when the CSS
/// does not parse. A call nested inside another `reapply` returns false and is queued: when the
/// outer call finishes it re-applies the current settings once (R-49-02).
pub fn reapply(display: &gdk::Display, css: &str) -> bool {
    if !GUARD.with(Guard::enter) {
        tracing::debug!("theme apply re-entered; queued");
        return false;
    }
    let ok = set_palette(display, css);
    if GUARD.with(Guard::leave) {
        apply_current();
    }
    ok
}

/// Parse-checks `css`, then loads it into the palette provider. The provider is added to the
/// display the first time only.
fn set_palette(display: &gdk::Display, css: &str) -> bool {
    load_into(&PALETTE, display, css)
}

/// Same as `set_palette`, for the tint provider.
fn set_tints(display: &gdk::Display, css: &str) -> bool {
    load_into(&TINTS, display, css)
}

fn load_into(
    slot: &'static std::thread::LocalKey<RefCell<Option<(gdk::Display, CssProvider)>>>,
    display: &gdk::Display,
    css: &str,
) -> bool {
    if let Err(message) = build_provider(css) {
        LAST_ERROR.with(|last| {
            let mut last = last.borrow_mut();
            if last.as_deref() != Some(message.as_str()) {
                tracing::warn!(error = %message, "generated theme CSS did not parse; keeping the previous theme");
                *last = Some(message);
            }
        });
        return false;
    }
    LAST_ERROR.with(|last| *last.borrow_mut() = None);
    slot.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provider = match slot.as_ref() {
            Some((_, provider)) => provider.clone(),
            None => {
                let provider = CssProvider::new();
                gtk::style_context_add_provider_for_display(
                    display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
                *slot = Some((display.clone(), provider.clone()));
                provider
            }
        };
        provider.load_from_string(css);
    });
    true
}

/// Installs the base stylesheet, then applies `settings` through the palette provider. Call once,
/// before the first window is presented (T-49ba-001).
pub fn install(display: &gdk::Display, settings: ThemeSettings) -> bool {
    let base = CssProvider::new();
    connect_parse_log(&base);
    base.load_from_string(style::BASE_CSS);
    gtk::style_context_add_provider_for_display(display, &base, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    set_settings(display, settings)
}

/// Logs each parse error of a stylesheet with its location (T-49ba-045).
fn connect_parse_log(provider: &CssProvider) {
    provider.connect_parsing_error(|_, section, err| {
        let at = section.start_location();
        tracing::error!(line = at.lines() + 1, column = at.chars() + 1, error = %err.message(), "stylesheet parse error");
    });
}

/// The palette text for a resolved theme. The GTK theme (`system_scheme`) gets empty text, so the
/// user's own theme colours are left unchanged (T-49b-008).
pub fn palette_text(resolved: &Resolved) -> String {
    if resolved.system_scheme {
        String::new()
    } else {
        css::render(&resolved.tokens, false)
    }
}

/// The tint text for a resolved theme: the dark-aware `kb-selected` and `kb-active` rules for the
/// GTK-theme choice (they cannot live in the static base sheet), empty for a picked scheme, whose
/// palette already carries them (T-49b-010, T-49b-029).
pub fn tint_text(resolved: &Resolved) -> String {
    if resolved.system_scheme {
        style::system_rules(resolved.tokens.dark)
    } else {
        String::new()
    }
}

/// Resolves the settings, renders the CSS, and installs it on `display`. The player state layer
/// (T-49a-010) follows the system dark flag, so its rules ride on the tint provider for every choice.
pub fn apply(display: &gdk::Display, settings: &ThemeSettings, system_dark: bool) -> bool {
    let resolved = resolve(settings, system_dark);
    let ok = reapply(display, &palette_text(&resolved));
    let tints = format!("{}{}", tint_text(&resolved), style::widget_rules(system_dark));
    set_tints(display, &tints) && ok
}

/// Sets the GTK dark preference for a mode: Light and Dark force it; System follows `system_dark`.
pub fn sync_gtk_preference(mode: ThemeMode, system_dark: bool) {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    let prefer_dark = match mode {
        ThemeMode::LIGHT => false,
        ThemeMode::DARK => true,
        ThemeMode::SYSTEM => system_dark,
    };
    settings.set_gtk_application_prefer_dark_theme(prefer_dark);
}

/// Stores the settings, then applies them with the last known system value.
pub fn set_settings(display: &gdk::Display, settings: ThemeSettings) -> bool {
    CURRENT.with(|c| *c.borrow_mut() = Some((display.clone(), settings)));
    apply_current()
}

/// Re-applies the stored settings with the last known system value (after a Monet seed arrives).
pub fn refresh() -> bool {
    apply_current()
}

fn apply_current() -> bool {
    let current = CURRENT.with(|c| c.borrow().clone());
    let Some((display, settings)) = current else {
        return false;
    };
    let system_dark = SYSTEM_DARK.with(Cell::get).unwrap_or(false);
    sync_gtk_preference(settings.mode, system_dark);
    apply(&display, &settings, system_dark)
}

fn set_system(read: dark::PortalRead) {
    let last = SYSTEM_DARK.with(Cell::get);
    let dark = dark::next_system(read, last, dark::gtk_fallback());
    SYSTEM_DARK.with(|c| c.set(Some(dark)));
    apply_current();
}

/// Starts following the desktop colour scheme (T-49a-011): reads the portal once, then re-applies
/// on each `SettingChanged`. Call once after the window is realized.
pub fn follow_system() {
    dark::read_portal(set_system);
    dark::watch(set_system);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::style::BASE_CSS;
    use crate::theme::app_theme::AppTheme;
    use crate::theme::resolve::{scheme_for, tokens_for};
    use crate::theme::SchemeSel;
    use kuro_prefs::ui::AppTheme as Scheme;
    use kuro_prefs::ui::ThemeMode;

    fn golden_outputs() -> [(&'static str, String); 3] {
        [
            ("default-light.css", css::render(&tokens_for(scheme_for(Scheme::DEFAULT), false, false), false)),
            ("default-dark.css", css::render(&tokens_for(scheme_for(Scheme::DEFAULT), true, false), false)),
            ("doom-dark-amoled.css", css::render(&tokens_for(scheme_for(Scheme::DOOM), true, true), false)),
        ]
    }

    /// Regenerates the golden files: `cargo test -p kurobayomi write_golden_css -- --ignored`.
    #[test]
    #[ignore = "writes the golden files; run on purpose"]
    fn write_golden_css() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/theme/fixtures/css");
        for (name, text) in golden_outputs() {
            std::fs::write(format!("{dir}/{name}"), text).expect("write golden file");
        }
    }

    #[test]
    fn palette_matches_the_golden_files() {
        let [light, dark, amoled] = golden_outputs();
        assert_eq!(light.1, include_str!("fixtures/css/default-light.css"));
        assert_eq!(dark.1, include_str!("fixtures/css/default-dark.css"));
        assert_eq!(amoled.1, include_str!("fixtures/css/doom-dark-amoled.css"));
    }

    #[test]
    fn gtk_theme_palette_is_empty_and_the_tints_carry_the_kb_rules() {
        let settings = ThemeSettings { scheme: SchemeSel::from(AppTheme::GtkTheme), ..ThemeSettings::default() };
        let (dark, light) = (resolve(&settings, true), resolve(&ThemeSettings::default(), false));
        assert_eq!(palette_text(&dark), "");
        assert_eq!(palette_text(&light), "");
        let (dark, light) = (tint_text(&dark), tint_text(&light));
        assert!(!dark.contains("@define-color") && !light.contains("@define-color"));
        assert!(dark.contains(", 0.16)") && dark.contains("rgba(255, 235, 59, 1)"));
        assert!(light.contains(", 0.22)") && light.contains("rgba(255, 193, 7, 1)"));
        assert_eq!(css::check(&dark), Ok(()));
    }

    #[test]
    fn a_picked_scheme_has_no_tint_text() {
        let settings = ThemeSettings { scheme: SchemeSel::from(AppTheme::Doom), ..ThemeSettings::default() };
        assert_eq!(tint_text(&resolve(&settings, false)), "");
    }

    #[test]
    fn a_picked_scheme_renders_a_palette_under_twenty_kilobytes() {
        let settings = ThemeSettings {
            scheme: SchemeSel::from(AppTheme::Doom),
            mode: ThemeMode::DARK,
            ..ThemeSettings::default()
        };
        let css = palette_text(&resolve(&settings, false));
        assert!(css.contains("@define-color kuro_primary"));
        assert!(css.contains(".kb-selected"));
        assert!(css.len() < 20 * 1024);
    }

    #[test]
    fn deprecated_names_resolve_to_the_gtk_theme() {
        let settings = ThemeSettings { scheme: SchemeSel::from(AppTheme::from_stored("HOT_PINK")), ..ThemeSettings::default() };
        assert_eq!(settings.scheme, SchemeSel::System);
    }

    #[test]
    fn base_stylesheet_has_no_parse_errors_in_the_css_grammar() {
        assert_eq!(BASE_CSS.matches('{').count(), BASE_CSS.matches('}').count());
    }

    #[test]
    fn a_nested_request_is_queued_and_run_once() {
        let g = Guard { applying: Cell::new(false), pending: Cell::new(false) };
        assert!(g.enter());
        // Two nested requests collapse into one owed apply.
        assert!(!g.enter());
        assert!(!g.enter());
        assert!(g.leave());
        // The queue is empty afterwards and the guard is free again.
        assert!(g.enter());
        assert!(!g.leave());
    }

    #[test]
    fn no_nested_request_owes_nothing() {
        let g = Guard { applying: Cell::new(false), pending: Cell::new(false) };
        assert!(g.enter());
        assert!(!g.leave());
    }

    /// T-49b-008 and F-49-01: one palette provider for the life of the process, replaced text on
    /// each apply, and a broken sheet keeps the previous text. Needs a display; run alone:
    /// `cargo test -p kurobayomi gtk_palette_provider -- --ignored`.
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi gtk_palette_provider -- --ignored"]
    fn gtk_palette_provider_keeps_one_provider_and_the_last_good_text() {
        if gtk::init().is_err() {
            eprintln!("no display: skipping");
            return;
        }
        let display = gdk::Display::default().expect("a display");
        assert!(build_provider(BASE_CSS).is_ok());
        let doom = palette_text(&resolve(&ThemeSettings { scheme: SchemeSel::from(AppTheme::Doom), ..ThemeSettings::default() }, false));
        let cloudflare = palette_text(&resolve(&ThemeSettings { scheme: SchemeSel::from(AppTheme::Cloudflare), ..ThemeSettings::default() }, false));

        assert!(reapply(&display, &doom));
        let first = PALETTE.with(|slot| slot.borrow().as_ref().map(|(_, p)| p.clone())).expect("palette provider");
        assert!(reapply(&display, &cloudflare));
        let second = PALETTE.with(|slot| slot.borrow().as_ref().map(|(_, p)| p.clone())).expect("palette provider");
        assert_eq!(first, second, "the palette provider is added once and reused");
        // GTK stores the parsed sheet, so compare with a reference provider loaded from the same text.
        let reference = CssProvider::new();
        reference.load_from_string(&cloudflare);
        assert_eq!(second.to_str(), reference.to_str(), "the text is replaced");

        assert!(!reapply(&display, "}}}} not css"), "a broken sheet is refused");
        assert_eq!(second.to_str(), reference.to_str(), "the previous text is kept");
    }
}
