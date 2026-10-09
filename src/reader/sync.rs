//! Settings change propagation to the open reader (T-043a-080). A preference change is mapped to the reader parts
//! it affects, and the reader applies those parts while it stays open. The download-ahead amount and the
//! remove-after-read slots are read at open and are not listed here.

use std::rc::Weak;
use std::time::Duration;

use gtk::glib;
use kuro_prefs::store::PrefStore;

/// The reader parts a preference change can touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Theme,
    PageNumber,
    Fullscreen,
    KeepAwake,
    Flash,
    Brightness,
    ColorFilter,
}

/// The effects of a changed preference key. Keys that touch no open reader part give nothing.
pub fn effects_for(key: &str) -> Vec<Effect> {
    match key {
        "pref_reader_theme_key" => vec![Effect::Theme],
        "pref_show_page_number_key" => vec![Effect::PageNumber],
        "fullscreen" => vec![Effect::Fullscreen],
        "pref_keep_screen_on_key" => vec![Effect::KeepAwake],
        "pref_reader_flash" | "pref_reader_flash_duration" | "pref_reader_flash_interval" | "pref_reader_flash_mode" => {
            vec![Effect::Flash]
        }
        "pref_custom_brightness_key" | "custom_brightness_value" => vec![Effect::Brightness],
        "pref_color_filter_key" | "color_filter_value" | "color_filter_mode" | "pref_grayscale" | "pref_inverted_colors" => {
            vec![Effect::ColorFilter]
        }
        _ => Vec::new(),
    }
}

/// Polls the store's change feed on the GTK thread and calls `apply` for each effect while the reader lives. The
/// poll ends when `owner` is gone, which is when the reader closes.
pub fn follow<T: 'static>(store: &PrefStore, owner: Weak<T>, apply: impl Fn(&T, Effect) + 'static) {
    let changes = store.subscribe();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let Some(owner) = owner.upgrade() else { return glib::ControlFlow::Break };
        while let Ok(change) = changes.try_recv() {
            for effect in change.key.as_deref().map(effects_for).unwrap_or_default() {
                apply(&owner, effect);
            }
        }
        glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_live_settings_map_to_their_reader_parts() {
        assert_eq!(effects_for("pref_reader_theme_key"), [Effect::Theme]);
        assert_eq!(effects_for("pref_show_page_number_key"), [Effect::PageNumber]);
        assert_eq!(effects_for("fullscreen"), [Effect::Fullscreen]);
        assert_eq!(effects_for("pref_keep_screen_on_key"), [Effect::KeepAwake]);
        assert_eq!(effects_for("pref_reader_flash_duration"), [Effect::Flash]);
        assert_eq!(effects_for("custom_brightness_value"), [Effect::Brightness]);
        assert_eq!(effects_for("pref_grayscale"), [Effect::ColorFilter]);
    }

    #[test]
    fn the_read_at_open_settings_and_unrelated_keys_give_no_effect() {
        assert!(effects_for("pref_download_ahead_amount").is_empty());
        assert!(effects_for("reader_long_tap").is_empty());
    }
}
