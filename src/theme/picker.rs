//! Theme picker (T-49a-062, T-49b-014) and the pure-black switch (T-49b-015): the mode row, one card per
//! theme, and the AMOLED switch. A choice writes the stored settings and re-renders the palette at once.
//!
//! Each card renders its own scheme through `resolve_with_seed`, not the live scheme. The card CSS is a
//! pure function, so the rules are tested without a display.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;
use kuro_prefs::ui::ThemeMode;

use super::app_theme::AppTheme;
use super::apply;
use super::icons::Icon;
use super::prefs;
use super::tokens::{Role, Tokens};
use super::{SchemeSel, ThemeSettings, monet, resolve_with_seed};
use crate::widgets::{observe, surface};

/// Card width in px.
pub const CARD_WIDTH: i32 = 114;
/// Preview height in px: a 9:16 preview of the card width (`114 * 16 / 9` rounded).
pub const CARD_PREVIEW_HEIGHT: i32 = 203;
/// Border width in px; primary when the card is selected.
pub const CARD_BORDER: i32 = 4;
/// Outer corner radius in px.
pub const CARD_RADIUS_OUTER: i32 = 17;
/// Inner corner radius in px.
pub const CARD_RADIUS_INNER: i32 = 13;
/// Label opacity.
pub const LABEL_ALPHA: f64 = 0.78;
/// Label height in lines (reserved even for one-line titles).
pub const LABEL_LINES: i32 = 2;
/// Height of the preview bar that carries the check icon, in px.
const CARD_BAR_HEIGHT: i32 = 24;

/// The mode row, in order.
pub const MODES: [ThemeMode; 3] = [ThemeMode::SYSTEM, ThemeMode::LIGHT, ThemeMode::DARK];

/// The CSS class of a card. It comes from the stable stored name, so reordering the list does not
/// reset the selection.
pub fn card_class(app: AppTheme) -> String {
    format!("kb-card-{}", app.stored_name().to_ascii_lowercase().replace('_', "-"))
}

/// The tokens a card previews: its own scheme in the chosen mode, not the live one.
pub fn preview_tokens(app: AppTheme, mode: ThemeMode, amoled: bool, system_dark: bool) -> Tokens {
    let settings = ThemeSettings {
        scheme: SchemeSel::from(app),
        amoled,
        mode,
    };
    resolve_with_seed(&settings, system_dark, monet::seed(), monet::contrast()).tokens
}

/// The CSS for one card: its surface, its primary bar, a primary border only when selected.
pub fn card_css(class: &str, tokens: &Tokens, selected: bool) -> String {
    let surface = tokens.get(Role::Surface).to_css();
    let on_surface = tokens.get(Role::OnSurface).to_css();
    let primary = tokens.get(Role::Primary).to_css();
    let border = if selected { primary.as_str() } else { "transparent" };
    format!(
        ".{class} {{ background-color: {surface}; color: {on_surface}; border: {CARD_BORDER}px solid {border}; border-radius: {CARD_RADIUS_OUTER}px; }}\n\
         .{class} .kb-card-bar {{ background-color: {primary}; border-radius: {CARD_RADIUS_INNER}px {CARD_RADIUS_INNER}px 0 0; }}\n"
    )
}

/// The stylesheet for every card, with `selected` marked.
pub fn cards_css(entries: &[(AppTheme, Tokens)], selected: AppTheme) -> String {
    entries
        .iter()
        .map(|(app, tokens)| card_css(&card_class(*app), tokens, *app == selected))
        .collect()
}

/// The CSS class of a card's focusable slot. Its focus ring is `CARD_FOCUS_CSS`.
pub const CARD_SLOT_CLASS: &str = "kb-card-slot";

/// The focus ring of a card, shown for keyboard focus only (T-49a-065).
pub const CARD_FOCUS_CSS: &str = ".kb-card-slot:focus-visible { outline: 2px solid @accent_color; outline-offset: 2px; }\n";

/// The card index that an arrow key moves to. The ends stop; the focus does not wrap.
pub fn next_card(current: usize, len: usize, step: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as isize + step).clamp(0, len as isize - 1) as usize
}

/// Keys on a card: Left and Right move focus to the neighbour, Enter and Space select the card.
fn card_keys(index: usize, slots: Rc<RefCell<Vec<gtk::Box>>>, select: Rc<dyn Fn()>) -> gtk::EventControllerKey {
    use gtk::gdk::Key;
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        let step = match key {
            Key::Left | Key::KP_Left => -1,
            Key::Right | Key::KP_Right => 1,
            Key::Return | Key::KP_Enter | Key::space => {
                select();
                return gtk::glib::Propagation::Stop;
            }
            _ => return gtk::glib::Propagation::Proceed,
        };
        let slots = slots.borrow();
        slots[next_card(index, slots.len(), step)].grab_focus();
        gtk::glib::Propagation::Stop
    });
    keys
}

/// The pure-black switch is insensitive in Light mode. Its stored value is kept there.
pub fn amoled_sensitive(mode: ThemeMode) -> bool {
    mode != ThemeMode::LIGHT
}

/// The label of a mode in the mode row.
pub fn mode_label(mode: ThemeMode) -> String {
    match mode {
        ThemeMode::LIGHT => tr(MsgKey::theme_light),
        ThemeMode::DARK => tr(MsgKey::theme_dark),
        _ => tr(MsgKey::theme_system),
    }
}

fn store() -> PrefStore {
    crate::data::app().base.store().clone()
}

/// Writes a change to the stored theme inputs and re-renders the palette at once (no restart).
pub fn change_settings(change: impl FnOnce(&mut ThemeSettings)) {
    let store = store();
    let mut settings = prefs::read_settings(&store);
    change(&mut settings);
    prefs::write_scheme(&store, settings.scheme);
    prefs::write_mode(&store, settings.mode);
    prefs::write_amoled(&store, settings.amoled);
    if let Some(display) = gtk::gdk::Display::default() {
        apply::set_settings(&display, settings);
        apply::sync_gtk_preference(settings.mode, apply::system_is_dark());
    }
}

/// The mode row: System, Light and Dark as a linked segmented control.
pub fn mode_row(current: ThemeMode) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("linked");
    let mut group: Option<gtk::ToggleButton> = None;
    for mode in MODES {
        let button = gtk::ToggleButton::with_label(&mode_label(mode));
        if let Some(first) = &group {
            button.set_group(Some(first));
        }
        button.set_active(mode == current);
        // connected after set_active, so building the row never writes a setting
        button.connect_toggled(move |b| {
            if b.is_active() {
                change_settings(|s| s.mode = mode);
            }
        });
        if group.is_none() {
            group = Some(button.clone());
        }
        row.append(&button);
    }
    row
}

/// Keeps `widget` insensitive while the stored mode is Light and sensitive otherwise. It follows
/// every later change of the mode, whoever writes it. `mode` is the stored mode preference of `store`.
/// The widget is held weakly: the binding does not keep it alive.
pub fn bind_amoled_sensitivity(widget: &impl IsA<gtk::Widget>, store: &PrefStore, mode: &Pref<ThemeMode>) {
    let target = widget.as_ref().downgrade();
    observe::bind(widget, store, mode, move |m| {
        if let Some(target) = target.upgrade() {
            target.set_sensitive(amoled_sensitive(m));
        }
    });
}

/// The pure-black switch. Insensitive in Light mode, now and after a mode change; toggling applies at once.
pub fn amoled_switch(amoled: bool, mode: ThemeMode) -> gtk::Switch {
    let switch = gtk::Switch::new();
    switch.set_active(amoled);
    switch.set_sensitive(amoled_sensitive(mode));
    switch.connect_state_set(|_, on| {
        change_settings(|s| s.amoled = on);
        gtk::glib::Propagation::Proceed
    });
    let store = store();
    bind_amoled_sensitivity(&switch, &store, &Pref::new(store.clone(), prefs::KEY_THEME_MODE, ThemeMode::SYSTEM));
    switch
}

/// The row of theme cards: `GtkTheme` first, the static schemes in order, Dynamic only with a seed.
/// Selecting a card applies it at once. Previews are built when the picker opens.
pub fn theme_cards(stored: AppTheme, mode: ThemeMode, amoled: bool) -> gtk::ScrolledWindow {
    let system_dark = apply::system_is_dark();
    let selected = AppTheme::selected_in_picker(stored, monet::available());
    let entries: Rc<Vec<(AppTheme, Tokens)>> = Rc::new(
        AppTheme::picker(monet::available())
            .into_iter()
            .map(|app| (app, preview_tokens(app, mode, amoled, system_dark)))
            .collect(),
    );
    let checks: Rc<RefCell<Vec<(AppTheme, gtk::Image)>>> = Rc::new(RefCell::new(Vec::new()));

    let provider = gtk::CssProvider::new();
    provider.load_from_string(&cards_css(&entries, selected));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let slots: Rc<RefCell<Vec<gtk::Box>>> = Rc::new(RefCell::new(Vec::new()));
    for (app, _) in entries.iter() {
        let app = *app;
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let preview = gtk::Box::new(gtk::Orientation::Vertical, 0);
        preview.add_css_class(&card_class(app));
        preview.set_size_request(CARD_WIDTH, CARD_PREVIEW_HEIGHT);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("kb-card-bar");
        bar.set_size_request(-1, CARD_BAR_HEIGHT);
        bar.set_halign(gtk::Align::End);
        let check = gtk::Image::from_icon_name(Icon::Done.name());
        check.set_visible(app == selected);
        bar.append(&check);
        checks.borrow_mut().push((app, check));
        preview.append(&bar);
        card.append(&preview);

        let label = gtk::Label::new(Some(&app.title().unwrap_or_default()));
        label.set_wrap(true);
        label.set_lines(LABEL_LINES);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_justify(gtk::Justification::Center);
        label.set_opacity(LABEL_ALPHA);
        label.set_max_width_chars(12);
        card.append(&label);

        let (entries, provider, checks) = (entries.clone(), provider.clone(), checks.clone());
        let on_select: Rc<dyn Fn()> = Rc::new(move || {
            change_settings(|s| s.scheme = SchemeSel::from(app));
            provider.load_from_string(&cards_css(&entries, app));
            for (card_app, check) in checks.borrow().iter() {
                check.set_visible(*card_app == app);
            }
        });
        let index = slots.borrow().len();
        let slot = surface::clickable(&card, true, { let on_select = on_select.clone(); move || on_select() }, None);
        slot.add_css_class(CARD_SLOT_CLASS);
        slot.set_focusable(true);
        slots.borrow_mut().push(slot.clone());
        slot.add_controller(card_keys(index, slots.clone(), on_select));
        row.append(&slot);
    }
    if let Some(display) = gtk::gdk::Display::default() {
        let focus = gtk::CssProvider::new();
        focus.load_from_string(CARD_FOCUS_CSS);
        gtk::style_context_add_provider_for_display(&display, &focus, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&row)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::ui::AppTheme as Scheme;

    #[test]
    fn card_geometry() {
        assert_eq!(CARD_WIDTH, 114);
        assert_eq!(CARD_PREVIEW_HEIGHT, (114.0_f64 * 16.0 / 9.0).round() as i32);
        assert_eq!((CARD_BORDER, CARD_RADIUS_OUTER, CARD_RADIUS_INNER), (4, 17, 13));
        assert_eq!(LABEL_LINES, 2);
    }

    #[test]
    fn card_class_is_stable_per_theme() {
        assert_eq!(card_class(AppTheme::Cloudflare), card_class(AppTheme::Cloudflare));
        assert_ne!(card_class(AppTheme::Cloudflare), card_class(AppTheme::Doom));
        assert!(card_class(AppTheme::GtkTheme).chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn each_card_previews_its_own_scheme() {
        let cloudflare = preview_tokens(AppTheme::Cloudflare, ThemeMode::LIGHT, false, false);
        let doom = preview_tokens(AppTheme::Doom, ThemeMode::LIGHT, false, false);
        assert_ne!(cloudflare.get(Role::Primary), doom.get(Role::Primary));
        assert_eq!(
            cloudflare.get(Role::Primary),
            super::super::resolve(
                &ThemeSettings {
                    scheme: SchemeSel::Scheme(Scheme::CLOUDFLARE),
                    amoled: false,
                    mode: ThemeMode::LIGHT,
                },
                false,
            )
            .tokens
            .get(Role::Primary)
        );
    }

    #[test]
    fn only_the_selected_card_has_a_primary_border() {
        let tokens = preview_tokens(AppTheme::Cloudflare, ThemeMode::LIGHT, false, false);
        let primary = tokens.get(Role::Primary).to_css();
        let on = card_css("kb-card-x", &tokens, true);
        let off = card_css("kb-card-x", &tokens, false);
        assert!(on.contains(&format!("border: 4px solid {primary}")));
        assert!(off.contains("border: 4px solid transparent"));
    }

    #[test]
    fn selection_moves_between_cards_in_the_stylesheet() {
        let entries = vec![
            (AppTheme::Cloudflare, preview_tokens(AppTheme::Cloudflare, ThemeMode::LIGHT, false, false)),
            (AppTheme::Doom, preview_tokens(AppTheme::Doom, ThemeMode::LIGHT, false, false)),
        ];
        let css = cards_css(&entries, AppTheme::Doom);
        let cloudflare_line = css.lines().find(|l| l.starts_with(".kb-card-cloudflare {")).unwrap();
        let doom_line = css.lines().find(|l| l.starts_with(".kb-card-doom {")).unwrap();
        assert!(cloudflare_line.contains("transparent"));
        assert!(!doom_line.contains("transparent"));
    }

    #[test]
    fn amoled_is_insensitive_only_in_light_mode() {
        assert!(!amoled_sensitive(ThemeMode::LIGHT));
        assert!(amoled_sensitive(ThemeMode::DARK));
        assert!(amoled_sensitive(ThemeMode::SYSTEM));
    }

    // T-49b-015: the switch follows mode changes made anywhere, with a fake store.
    #[test]
    fn the_switch_follows_the_stored_mode() {
        crate::testing::with_display(|| {
            let store = PrefStore::in_memory();
            let mode = Pref::new(store.clone(), prefs::KEY_THEME_MODE, ThemeMode::SYSTEM);
            let switch = gtk::Switch::new();
            bind_amoled_sensitivity(&switch, &store, &mode);
            assert!(switch.is_sensitive());
            mode.set(ThemeMode::LIGHT).unwrap();
            let ctx = gtk::glib::MainContext::default();
            let start = std::time::Instant::now();
            while switch.is_sensitive() && start.elapsed() < std::time::Duration::from_secs(2) {
                ctx.iteration(false);
            }
            assert!(!switch.is_sensitive());
            mode.set(ThemeMode::DARK).unwrap();
            while !switch.is_sensitive() && start.elapsed() < std::time::Duration::from_secs(4) {
                ctx.iteration(false);
            }
            assert!(switch.is_sensitive());
        });
    }

    // T-49a-065: arrows stop at the ends of the row.
    #[test]
    fn arrow_keys_move_one_card_and_stop_at_the_ends() {
        assert_eq!(next_card(0, 3, 1), 1);
        assert_eq!(next_card(2, 3, 1), 2);
        assert_eq!(next_card(0, 3, -1), 0);
        assert_eq!(next_card(1, 3, -1), 0);
        assert_eq!(next_card(0, 0, 1), 0);
    }

    #[test]
    fn cards_show_a_focus_ring_for_keyboard_focus() {
        assert!(CARD_FOCUS_CSS.contains(".kb-card-slot:focus-visible"));
        assert!(CARD_FOCUS_CSS.contains("outline"));
    }

    #[test]
    fn mode_row_order_is_system_light_dark() {
        assert_eq!(MODES, [ThemeMode::SYSTEM, ThemeMode::LIGHT, ThemeMode::DARK]);
    }
}
