//! App theme strip (Kotlin `AppThemePreferenceWidget`): one card per theme, horizontal scroll, no wrapping.
//! Cards paint the theme's own palette through the `.theme-<name>` classes that this module installs.
//! Clicking a card stores the theme and applies it through `theme::apply` (live CSS swap, no restart).
//!
//! The strip offers the Linux picker list (`theme::AppTheme::PICKER`): the desktop's own GTK theme first, the 18
//! Aniyomi schemes and Tokyo Night. The onboarding page uses the typed `strip_themes`, `card_title` and `normalise`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{pango, Align, AspectFrame, Box as GBox, Button, Image, Label, Orientation, PolicyType, ScrolledWindow};
use kuro_prefs::store::PrefStore;
use kuro_prefs::ui::{AppTheme, ThemeMode};

use crate::theme::tokens::Role;
use crate::theme::{apply, is_dark, picker, prefs, SchemeSel};
use crate::theme::AppTheme as Choice;

use super::{SECONDARY_ALPHA, THEME_CARD_WIDTH};

/// Cover width inside the card: half of the 98 px content width (Kotlin `fillMaxWidth(0.5f)`), at 2:3.
const COVER_WIDTH: i32 = 49;

/// The 18 Aniyomi schemes the onboarding list offers, in picker order (no GTK theme and no Tokyo Night entry: the
/// onboarding stores a typed `pref_app_theme`).
pub fn strip_themes() -> Vec<AppTheme> {
    Choice::PICKER.iter().filter_map(|choice| choice.scheme()).collect()
}

/// A stored theme that the onboarding list does not offer (the deprecated names and the dynamic one) reads as
/// DEFAULT (E-48, E-49). The caller writes it back.
pub fn normalise(stored: AppTheme) -> AppTheme {
    if strip_themes().contains(&stored) { stored } else { AppTheme::DEFAULT }
}

/// Amoled only applies in dark mode (E-52).
pub fn effective_amoled(dark: bool, amoled: bool) -> bool {
    dark && amoled
}

/// Card title of an onboarding theme.
pub fn card_title(theme: AppTheme) -> String {
    Choice::PICKER
        .iter()
        .find(|choice| choice.scheme() == Some(theme))
        .and_then(|choice| choice.title())
        .unwrap_or_default()
}

/// The choices the strip offers, in order.
pub fn strip_choices() -> Vec<Choice> {
    Choice::PICKER.to_vec()
}

/// The choice a stored scheme selects. A scheme with no card shows the GTK theme.
pub fn selected_choice(scheme: SchemeSel) -> Choice {
    match scheme {
        SchemeSel::System => Choice::GtkTheme,
        SchemeSel::TokyoNight => Choice::TokyoNight,
        SchemeSel::Scheme(scheme) => Choice::ALL.into_iter().find(|c| c.scheme() == Some(scheme)).unwrap_or(Choice::GtkTheme),
    }
}

/// CSS class of a card's palette, e.g. `theme-green-apple`.
pub fn palette_class(choice: Choice) -> String {
    format!("theme-{}", choice.stored_name().to_ascii_lowercase().replace('_', "-"))
}

/// CSS for one card's preview, painted from the theme's own palette (Kotlin `AppThemePreviewItem`).
/// Every rule is scoped by the card's `palette_class`, so all cards share one provider. The GTK theme card shows
/// the Default palette: the real colours are the desktop's.
pub fn preview_css(choice: Choice, dark: bool, amoled: bool) -> String {
    let mode = if dark { ThemeMode::DARK } else { ThemeMode::LIGHT };
    let tokens = picker::preview_tokens(choice, mode, effective_amoled(dark, amoled), dark);
    let c = |role: Role| tokens.get(role).to_css();
    let class = palette_class(choice);
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

/// Loads the preview palettes of every strip choice into one display-wide provider (created once, reloaded after).
fn install_palettes(dark: bool, amoled: bool) {
    let css: String = strip_choices().into_iter().map(|c| preview_css(c, dark, amoled)).collect();
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
fn preview(choice: Choice) -> (GBox, Image) {
    let frame = GBox::new(Orientation::Vertical, 0);
    frame.add_css_class("theme-card");
    frame.add_css_class(&palette_class(choice));
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

/// The App theme strip over `store`. The selected card follows the stored scheme; a click stores the new scheme and
/// applies it at once (`picker::change_settings_in`), with no restart. Previews are painted for the mode in effect
/// when the strip is built.
pub fn picker_strip(store: &PrefStore) -> ScrolledWindow {
    super::super::style::install();
    let settings = prefs::read_settings(store);
    let dark = is_dark(settings.mode, apply::system_is_dark());
    install_palettes(dark, settings.amoled);
    let selected = Rc::new(Cell::new(selected_choice(settings.scheme)));

    let cards = GBox::new(Orientation::Horizontal, 8);
    cards.set_margin_start(16);
    cards.set_margin_end(16);
    cards.set_margin_top(8);
    cards.set_margin_bottom(8);
    let mut handles: Vec<(Choice, GBox, Image)> = Vec::new();
    let mut buttons: Vec<(Choice, Button)> = Vec::new();
    for choice in strip_choices() {
        let title = choice.title().unwrap_or_default();
        let (frame, check) = preview(choice);
        mark(&frame, &check, choice == selected.get());
        let aspect = AspectFrame::new(0.5, 0.0, 9.0 / 16.0, false);
        aspect.set_child(Some(&frame));
        let button = Button::new();
        button.add_css_class("flat");
        button.add_css_class("theme-button");
        button.set_child(Some(&aspect));
        button.set_tooltip_text(Some(&title));
        button.update_property(&[gtk::accessible::Property::Label(&title)]);
        let label = Label::new(Some(&title));
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
        handles.push((choice, frame, check));
        buttons.push((choice, button));
    }

    let handles = Rc::new(handles);
    for (choice, button) in buttons {
        let (store, selected, handles) = (store.clone(), selected.clone(), handles.clone());
        button.connect_clicked(move |_| {
            if selected.get() == choice {
                return;
            }
            selected.set(choice);
            picker::change_settings_in(&store, |s| s.scheme = SchemeSel::from(choice));
            for (other, frame, check) in handles.iter() {
                mark(frame, check, *other == choice);
            }
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

    // The onboarding list only. The App theme strip on the Appearance page is not these 18 cards: see
    // `the_built_strip_has_the_picker_cards_in_order`.
    #[test]
    fn strip_shows_eighteen_themes_without_deprecated_names() {
        let themes = strip_themes();
        assert_eq!(themes.len(), 18);
        assert_eq!(themes.first(), Some(&AppTheme::DEFAULT));
        assert!(!themes.contains(&AppTheme::DARK_BLUE));
        assert!(!themes.contains(&AppTheme::HOT_PINK));
        assert!(!themes.contains(&AppTheme::BLUE));
    }

    #[test]
    fn deprecated_names_read_as_default() {
        for stored in [AppTheme::DARK_BLUE, AppTheme::HOT_PINK, AppTheme::BLUE] {
            assert_eq!(normalise(stored), AppTheme::DEFAULT);
        }
        assert_eq!(normalise(AppTheme::NORD), AppTheme::NORD);
        assert_eq!(card_title(AppTheme::DEFAULT), "Default");
        assert_eq!(card_title(AppTheme::NORD), Choice::Nord.title().expect("a title"));
    }

    #[test]
    fn amoled_only_in_dark_mode() {
        assert!(effective_amoled(true, true));
        assert!(!effective_amoled(false, true));
        assert!(!effective_amoled(true, false));
    }

    #[test]
    fn preview_paints_the_themes_own_palette() {
        let tokens = crate::theme::resolve::tokens_for(crate::theme::resolve::scheme_for(AppTheme::NORD), true, false);
        let css = preview_css(Choice::Nord, true, false);
        assert!(css.contains(&tokens.get(Role::Primary).to_css()));
        assert!(css.contains(".theme-nord.theme-card.selected"));
        assert_ne!(css, preview_css(Choice::Doom, true, false));
        assert_ne!(preview_css(Choice::TokyoNight, true, false), preview_css(Choice::Doom, true, false));
    }

    #[test]
    fn amoled_changes_the_dark_preview_only() {
        let light = preview_css(Choice::Default, false, false);
        assert_eq!(light, preview_css(Choice::Default, false, true));
        assert_ne!(preview_css(Choice::Default, true, false), preview_css(Choice::Default, true, true));
    }

    #[test]
    fn palette_class_is_kebab_case() {
        assert_eq!(palette_class(Choice::GreenApple), "theme-green-apple");
        assert_eq!(palette_class(Choice::Default), "theme-default");
        assert_eq!(palette_class(Choice::GtkTheme), "theme-system");
    }

    // T-47ba-041: the strip is the desktop theme, the 18 Aniyomi schemes and Tokyo Night; no deprecated name.
    #[test]
    fn the_strip_offers_the_picker_list_without_deprecated_names() {
        let choices = strip_choices();
        assert_eq!(choices.len(), 20);
        assert_eq!(choices[0], Choice::GtkTheme);
        assert_eq!(choices.iter().filter(|c| c.scheme().is_some()).count(), 18, "the 18 Aniyomi schemes");
        assert!(choices.iter().all(|c| !c.is_deprecated() && c.title().is_some()));
        assert_eq!(*choices.last().expect("a last choice"), Choice::TokyoNight);
    }

    // T-47ba-041: the strip that `picker_strip` builds is not the 18 cards of the plan. It is the desktop's own GTK
    // theme, the 18 Aniyomi schemes and Tokyo Night: 20 cards. Count, order and titles are pinned on the built widgets.
    #[test]
    fn the_built_strip_has_the_picker_cards_in_order() {
        crate::testing::with_display(|| {
            fn buttons(widget: &gtk::Widget, found: &mut Vec<Button>) {
                if let Some(button) = widget.downcast_ref::<Button>().filter(|b| b.has_css_class("theme-button")) {
                    found.push(button.clone());
                }
                let mut child = widget.first_child();
                while let Some(c) = child {
                    buttons(&c, found);
                    child = c.next_sibling();
                }
            }
            let strip = picker_strip(&PrefStore::in_memory());
            let mut cards = Vec::new();
            buttons(strip.upcast_ref(), &mut cards);
            let shown: Vec<String> = cards.iter().filter_map(|b| b.tooltip_text().map(|t| t.to_string())).collect();
            let expected: Vec<String> = strip_choices().iter().map(|c| c.title().expect("a card title")).collect();
            assert_eq!(shown, expected, "one card per picker choice, in picker order");
            assert_eq!(cards.len(), 20);
            assert_eq!(strip_choices().first(), Some(&Choice::GtkTheme), "the GTK theme card comes first");
            assert_eq!(strip_choices().last(), Some(&Choice::TokyoNight), "Tokyo Night comes last");
            assert_eq!(strip_choices().iter().filter(|c| c.scheme().is_some()).count(), 18);
        });
    }

    // T-47ba-042: every stored scheme selects one card.
    #[test]
    fn a_stored_scheme_selects_its_card() {
        for choice in strip_choices() {
            assert_eq!(selected_choice(SchemeSel::from(choice)), choice, "{choice:?}");
        }
    }

    // T-47ba-042: a click applies the scheme live and stores it, and the selected card moves.
    #[test]
    fn clicking_a_card_stores_the_scheme_and_moves_the_selection() {
        crate::testing::with_display(|| {
            let store = PrefStore::in_memory();
            let strip = picker_strip(&store);
            let mut cards = Vec::new();
            fn collect(widget: &gtk::Widget, found: &mut Vec<Button>) {
                if let Some(button) = widget.downcast_ref::<Button>().filter(|b| b.has_css_class("theme-button")) {
                    found.push(button.clone());
                }
                let mut child = widget.first_child();
                while let Some(c) = child {
                    collect(&c, found);
                    child = c.next_sibling();
                }
            }
            collect(strip.upcast_ref(), &mut cards);
            assert_eq!(cards.len(), strip_choices().len());
            let doom = strip_choices().iter().position(|c| *c == Choice::Doom).expect("Doom is offered");
            cards[doom].emit_clicked();
            assert_eq!(prefs::read_settings(&store).scheme, SchemeSel::from(Choice::Doom));
            let selected = |button: &Button| {
                let mut found = false;
                let mut stack = vec![button.clone().upcast::<gtk::Widget>()];
                while let Some(w) = stack.pop() {
                    found |= w.has_css_class("selected");
                    let mut child = w.first_child();
                    while let Some(c) = child {
                        stack.push(c.clone());
                        child = c.next_sibling();
                    }
                }
                found
            };
            assert!(selected(&cards[doom]));
            assert_eq!(cards.iter().filter(|b| selected(b)).count(), 1);
        });
    }
}
