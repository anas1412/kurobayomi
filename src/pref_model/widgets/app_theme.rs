//! App theme strip (Kotlin `AppThemePreferenceWidget`): one card per theme, horizontal scroll, no wrapping.
//! Cards paint the theme's own palette through the `.theme-<name>` classes that the theme CSS provides.
//! Clicking a card stores the theme and calls `apply` (live CSS swap, no restart).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{pango, Align, AspectFrame, Box as GBox, Button, Image, Label, Orientation, PolicyType, ScrolledWindow};
use kuro_prefs::pref::Pref;
use kuro_prefs::ui::AppTheme;

use crate::theme::resolve::{scheme_for, tokens_for};
use crate::theme::tokens::Role;

use super::{SECONDARY_ALPHA, THEME_CARD_WIDTH};

/// Cover width inside the card: half of the 98 px content width (Kotlin `fillMaxWidth(0.5f)`), at 2:3.
const COVER_WIDTH: i32 = 49;

/// Themes shown on Linux, in declaration order: all names minus the deprecated three and MONET (T.2).
pub fn strip_themes() -> Vec<AppTheme> {
    AppTheme::ALL
        .iter()
        .copied()
        .filter(|theme| theme.title_key().is_some() && *theme != AppTheme::MONET)
        .collect()
}

/// A stored theme that the strip does not offer reads as DEFAULT (E-48, E-49). The caller writes it back.
pub fn normalise(stored: AppTheme) -> AppTheme {
    match stored {
        AppTheme::DARK_BLUE | AppTheme::HOT_PINK | AppTheme::BLUE | AppTheme::MONET => AppTheme::DEFAULT,
        other => other,
    }
}

/// Amoled only applies in dark mode (E-52).
pub fn effective_amoled(dark: bool, amoled: bool) -> bool {
    dark && amoled
}

/// Card title from the theme's string key.
pub fn card_title(theme: AppTheme) -> String {
    use kuro_i18n::{MsgKey, tr};
    tr(match theme {
        AppTheme::DEFAULT | AppTheme::MONET | AppTheme::DARK_BLUE | AppTheme::HOT_PINK | AppTheme::BLUE => MsgKey::label_default,
        AppTheme::CLOUDFLARE => MsgKey::theme_cloudflare,
        AppTheme::COTTONCANDY => MsgKey::theme_cottoncandy,
        AppTheme::DOOM => MsgKey::theme_doom,
        AppTheme::GREEN_APPLE => MsgKey::theme_greenapple,
        AppTheme::LAVENDER => MsgKey::theme_lavender,
        AppTheme::MATRIX => MsgKey::theme_matrix,
        AppTheme::MIDNIGHT_DUSK => MsgKey::theme_midnightdusk,
        AppTheme::MOCHA => MsgKey::theme_mocha,
        AppTheme::SAPPHIRE => MsgKey::theme_sapphire,
        AppTheme::NORD => MsgKey::theme_nord,
        AppTheme::STRAWBERRY_DAIQUIRI => MsgKey::theme_strawberrydaiquiri,
        AppTheme::TAKO => MsgKey::theme_tako,
        AppTheme::TEALTURQUOISE => MsgKey::theme_tealturquoise,
        AppTheme::TIDAL_WAVE => MsgKey::theme_tidalwave,
        AppTheme::YINYANG => MsgKey::theme_yinyang,
        AppTheme::YOTSUBA => MsgKey::theme_yotsuba,
        AppTheme::MONOCHROME => MsgKey::theme_monochrome,
    })
}

/// CSS class of a card's palette, e.g. `theme-green-apple`.
pub fn palette_class(theme: AppTheme) -> String {
    format!("theme-{}", theme.name().to_ascii_lowercase().replace('_', "-"))
}

/// CSS for one card's preview, painted from the theme's own palette (Kotlin `AppThemePreviewItem`).
/// Every rule is scoped by the card's `palette_class`, so all 18 cards share one provider.
pub fn preview_css(theme: AppTheme, dark: bool, amoled: bool) -> String {
    let tokens = tokens_for(scheme_for(theme), dark, effective_amoled(dark, amoled));
    let c = |role: Role| tokens.get(role).to_css();
    let class = palette_class(theme);
    format!(
        ".{class}.theme-card {{ background-color: {bg}; border-color: {divider}; }}\n\
         .{class}.theme-card.selected {{ border-color: {primary}; }}\n\
         .{class} .tp-bar {{ background-color: {on_surface}; border-radius: 4px; }}\n\
         .{class} .tp-check {{ color: {primary}; }}\n\
         .{class} .tp-cover {{ background-color: {divider}; border-radius: 4px; }}\n\
         .{class} .tp-chip-a {{ background-color: {tertiary}; }}\n\
         .{class} .tp-chip-b {{ background-color: {secondary}; }}\n\
         .{class} .tp-bottom {{ background-color: {container}; }}\n\
         .{class} .tp-dot {{ background-color: {primary}; border-radius: 999px; }}\n\
         .{class} .tp-pill {{ background-color: {on_surface}; border-radius: 4px; opacity: 0.6; }}\n",
        bg = c(Role::Background),
        divider = c(Role::OutlineVariant),
        primary = c(Role::Primary),
        on_surface = c(Role::OnSurface),
        tertiary = c(Role::Tertiary),
        secondary = c(Role::Secondary),
        container = c(Role::SurfaceContainer),
    )
}

thread_local! {
    static PALETTES: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Loads the preview palettes of every strip theme into one display-wide provider (created once, reloaded after).
fn install_palettes(dark: bool, amoled: bool) {
    let css: String = strip_themes().into_iter().map(|t| preview_css(t, dark, amoled)).collect();
    PALETTES.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let Some(display) = gtk::gdk::Display::default() else {
                return;
            };
            let provider = gtk::CssProvider::new();
            gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            *slot = Some(provider);
        }
        if let Some(provider) = slot.as_ref() {
            provider.load_from_string(&css);
        }
    });
}

fn chip(class: &str) -> GBox {
    let chip = GBox::new(Orientation::Vertical, 0);
    chip.add_css_class(class);
    chip.set_size_request(12, 16);
    chip
}

/// The mini preview: app bar, 2:3 cover with two colour chips, bottom bar. Returns the frame and its check icon.
fn preview(theme: AppTheme) -> (GBox, Image) {
    let frame = GBox::new(Orientation::Vertical, 0);
    frame.add_css_class("theme-card");
    frame.add_css_class(&palette_class(theme));
    frame.set_vexpand(true);

    let bar_row = GBox::new(Orientation::Horizontal, 4);
    bar_row.set_size_request(-1, 40);
    bar_row.set_margin_start(8);
    bar_row.set_margin_end(8);
    let bar = GBox::new(Orientation::Vertical, 0);
    bar.add_css_class("tp-bar");
    bar.set_hexpand(true);
    bar.set_valign(Align::Center);
    bar.set_size_request(-1, 24);
    let check = Image::from_icon_name("emblem-ok-symbolic");
    check.add_css_class("tp-check");
    check.set_pixel_size(16);
    bar_row.append(&bar);
    bar_row.append(&check);
    frame.append(&bar_row);

    let cover = GBox::new(Orientation::Vertical, 0);
    cover.add_css_class("tp-cover");
    cover.set_halign(Align::Start);
    cover.set_margin_start(8);
    cover.set_margin_top(2);
    cover.set_size_request(COVER_WIDTH, COVER_WIDTH * 3 / 2);
    let chips = GBox::new(Orientation::Horizontal, 0);
    chips.set_halign(Align::Start);
    chips.set_valign(Align::Start);
    chips.set_margin_start(4);
    chips.set_margin_top(4);
    chips.set_overflow(gtk::Overflow::Hidden);
    chips.append(&chip("tp-chip-a"));
    chips.append(&chip("tp-chip-b"));
    cover.append(&chips);
    frame.append(&cover);

    let spacer = GBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    frame.append(&spacer);

    let bottom = GBox::new(Orientation::Horizontal, 8);
    bottom.add_css_class("tp-bottom");
    bottom.set_size_request(-1, 32);
    bottom.set_valign(Align::End);
    let dot = GBox::new(Orientation::Vertical, 0);
    dot.add_css_class("tp-dot");
    dot.set_size_request(17, 17);
    dot.set_valign(Align::Center);
    dot.set_margin_start(8);
    let pill = GBox::new(Orientation::Vertical, 0);
    pill.add_css_class("tp-pill");
    pill.set_size_request(-1, 17);
    pill.set_hexpand(true);
    pill.set_valign(Align::Center);
    pill.set_margin_end(8);
    bottom.append(&dot);
    bottom.append(&pill);
    frame.append(&bottom);
    frame.set_overflow(gtk::Overflow::Hidden);
    (frame, check)
}

/// `dark` says whether the window is dark right now; `amoled` is `pref_theme_dark_amoled_key`.
/// `apply` gets the chosen theme and the amoled flag that is effective for it (dark mode only, E-52).
pub fn strip(
    pref: Pref<AppTheme>,
    amoled: Pref<bool>,
    dark: Rc<dyn Fn() -> bool>,
    apply: Rc<dyn Fn(AppTheme, bool)>,
) -> ScrolledWindow {
    let stored = pref.get();
    let selected = normalise(stored);
    if selected != stored {
        let _ = pref.set(selected);
    }
    install_palettes(dark(), amoled.get());

    let cards = GBox::new(Orientation::Horizontal, 8);
    cards.set_margin_start(16);
    cards.set_margin_end(16);
    cards.set_margin_top(8);
    cards.set_margin_bottom(8);
    let mut handles: Vec<(AppTheme, GBox, Image)> = Vec::new();
    let mut buttons: Vec<(AppTheme, Button)> = Vec::new();
    for theme in strip_themes() {
        let (frame, check) = preview(theme);
        mark(&frame, &check, theme == selected);
        let aspect = AspectFrame::new(0.5, 0.0, 9.0 / 16.0, false);
        aspect.set_child(Some(&frame));
        let button = Button::new();
        button.add_css_class("flat");
        button.add_css_class("theme-button");
        button.set_child(Some(&aspect));
        button.set_tooltip_text(Some(&card_title(theme)));
        let label = Label::new(Some(&card_title(theme)));
        label.set_wrap(true);
        label.set_lines(2);
        label.set_ellipsize(pango::EllipsizeMode::End);
        label.set_justify(gtk::Justification::Center);
        label.set_size_request(-1, 40);
        label.set_valign(Align::Start);
        label.set_opacity(SECONDARY_ALPHA);
        let column = GBox::new(Orientation::Vertical, 8);
        column.set_size_request(THEME_CARD_WIDTH, -1);
        column.append(&button);
        column.append(&label);
        cards.append(&column);
        handles.push((theme, frame, check));
        buttons.push((theme, button));
    }

    let handles = Rc::new(handles);
    for (theme, button) in buttons {
        let (pref, amoled, dark, apply, handles) = (pref.clone(), amoled.clone(), dark.clone(), apply.clone(), handles.clone());
        button.connect_clicked(move |_| {
            if pref.get() == theme || pref.set(theme).is_err() {
                return;
            }
            for (other, frame, check) in handles.iter() {
                mark(frame, check, *other == theme);
            }
            let is_dark = dark();
            install_palettes(is_dark, amoled.get());
            apply(theme, effective_amoled(is_dark, amoled.get()));
        });
    }

    let scroll = ScrolledWindow::new();
    scroll.set_hscrollbar_policy(PolicyType::Automatic);
    scroll.set_vscrollbar_policy(PolicyType::Never);
    scroll.set_child(Some(&cards));
    scroll
}

/// Selected card: primary border and the check icon; every other card has neither.
fn mark(frame: &GBox, check: &Image, selected: bool) {
    if selected {
        frame.add_css_class("selected");
    } else {
        frame.remove_css_class("selected");
    }
    check.set_visible(selected);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_shows_eighteen_themes_without_monet_or_deprecated_names() {
        let themes = strip_themes();
        assert_eq!(themes.len(), 18);
        assert!(!themes.contains(&AppTheme::MONET));
        assert!(!themes.contains(&AppTheme::DARK_BLUE));
        assert!(!themes.contains(&AppTheme::HOT_PINK));
        assert!(!themes.contains(&AppTheme::BLUE));
    }

    #[test]
    fn deprecated_and_monet_names_read_as_default() {
        for stored in [AppTheme::DARK_BLUE, AppTheme::HOT_PINK, AppTheme::BLUE, AppTheme::MONET] {
            assert_eq!(normalise(stored), AppTheme::DEFAULT);
        }
        assert_eq!(normalise(AppTheme::NORD), AppTheme::NORD);
    }

    #[test]
    fn amoled_only_in_dark_mode() {
        assert!(effective_amoled(true, true));
        assert!(!effective_amoled(false, true));
        assert!(!effective_amoled(true, false));
    }

    #[test]
    fn preview_paints_the_themes_own_palette() {
        let tokens = tokens_for(scheme_for(AppTheme::NORD), true, false);
        let css = preview_css(AppTheme::NORD, true, false);
        assert!(css.contains(&tokens.get(Role::Primary).to_css()));
        assert!(css.contains(".theme-nord.theme-card.selected"));
        assert_ne!(css, preview_css(AppTheme::DOOM, true, false));
    }

    #[test]
    fn amoled_changes_the_dark_preview_only() {
        let light = preview_css(AppTheme::DEFAULT, false, false);
        assert_eq!(light, preview_css(AppTheme::DEFAULT, false, true));
        assert_ne!(preview_css(AppTheme::DEFAULT, true, false), preview_css(AppTheme::DEFAULT, true, true));
    }

    #[test]
    fn palette_class_is_kebab_case() {
        assert_eq!(palette_class(AppTheme::GREEN_APPLE), "theme-green-apple");
        assert_eq!(palette_class(AppTheme::DEFAULT), "theme-default");
    }
}
