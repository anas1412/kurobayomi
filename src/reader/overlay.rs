//! The page-area overlays (T-043a-064, T-043a-066, T-043a-063): the brightness layer, the colour filter layer with
//! its blend mode, grayscale, invert and the page-change flash. They sit over the pages only, never over the chrome.
//!
//! The values and the mapping are pure functions, tested without a display. [`Tint`] holds the three
//! `DrawingArea` layers and the CSS classes that apply them.
//!
//! The brightness layer is the black overlay of negative values. A positive value would drive the backlight; the
//! reader has no backlight access, so a positive value leaves the page unchanged (E-BRIGHT-2, recorded decision).

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use kuro_prefs::reader::{FlashColor, ReaderPrefs};

use super::blend_box::BlendBox;

use super::pref_rules;
use super::rules;

/// The custom brightness slider range (`ColorFilterPage.kt:41`).
pub const BRIGHTNESS_RANGE: (i32, i32) = (-75, 100);

/// One byte of the packed ARGB colour filter value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Alpha,
    Red,
    Green,
    Blue,
}

impl Channel {
    pub const fn shift(self) -> u32 {
        match self {
            Self::Alpha => 24,
            Self::Red => 16,
            Self::Green => 8,
            Self::Blue => 0,
        }
    }

    pub const fn mask(self) -> u32 {
        0xFF << self.shift()
    }
}

/// One channel (`0..=255`) of the packed colour.
pub fn channel_get(packed: i32, channel: Channel) -> i32 {
    ((packed as u32 & channel.mask()) >> channel.shift()) as i32
}

/// The packed colour with one channel replaced and the other three bytes kept (`getColorValue`:
/// `(value shl shift) or (current and mask.inv())`).
pub fn channel_set(packed: i32, channel: Channel, value: i32) -> i32 {
    let byte = value.clamp(0, 255) as u32;
    ((byte << channel.shift()) | (packed as u32 & !channel.mask())) as i32
}

/// The alpha of the black brightness layer: `|v| / 100` for a negative value while the feature is on, else none.
pub fn brightness_alpha(enabled: bool, value: i32) -> f64 {
    if enabled && value < 0 { f64::from(value.clamp(BRIGHTNESS_RANGE.0, -1).abs()) / 100.0 } else { 0.0 }
}

/// The colour of the filter layer as red, green, blue and alpha in `0..=1`, when the filter is on.
pub fn filter_rgba(enabled: bool, packed: i32) -> Option<(f64, f64, f64, f64)> {
    enabled.then(|| {
        let unit = |channel| f64::from(channel_get(packed, channel)) / 255.0;
        (unit(Channel::Red), unit(Channel::Green), unit(Channel::Blue), unit(Channel::Alpha))
    })
}

/// The CSS class for grayscale and invert. Grayscale applies first, then invert (T-043a-066).
pub fn filter_class(grayscale: bool, inverted: bool) -> Option<&'static str> {
    match (grayscale, inverted) {
        (false, false) => None,
        (true, false) => Some("reader-gray"),
        (false, true) => Some("reader-invert"),
        (true, true) => Some("reader-gray-invert"),
    }
}

const FILTER_CLASSES: [&str; 3] = ["reader-gray", "reader-invert", "reader-gray-invert"];

type Rgba = (f64, f64, f64, f64);

/// The overlay layers over the page area.
pub struct Tint {
    brightness: gtk::DrawingArea,
    /// Holds the pages and blends the colour filter over them.
    blend: BlendBox,
    flash: gtk::DrawingArea,
    brightness_colour: Rc<Cell<Rgba>>,
    flash_colour: Rc<Cell<Rgba>>,
    /// Counts flashes, so a timer only ends the flash it was started for.
    flash_generation: Rc<Cell<u64>>,
    /// Page changes seen while the flash is on (the interval counter).
    flash_trigger: Cell<u32>,
}

/// A hidden, click-through layer that paints `colour` over its whole area.
fn layer(colour: &Rc<Cell<Rgba>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_can_target(false);
    area.set_focusable(false);
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_visible(false);
    let colour = Rc::clone(colour);
    area.set_draw_func(move |_, cr, width, height| {
        let (r, g, b, a) = colour.get();
        cr.set_source_rgba(r, g, b, a);
        cr.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
        let _ = cr.fill();
    });
    area
}

impl Tint {
    /// Wraps `page` in the colour filter layer, makes that the child of `overlay`, and adds the brightness and flash
    /// layers above it.
    pub fn new(overlay: &gtk::Overlay, page: &impl IsA<gtk::Widget>) -> Self {
        let brightness_colour = Rc::new(Cell::new((0.0, 0.0, 0.0, 0.0)));
        let flash_colour = Rc::new(Cell::new((0.0, 0.0, 0.0, 1.0)));
        let blend = BlendBox::new(page);
        overlay.set_child(Some(&blend));
        let brightness = layer(&brightness_colour);
        let flash = layer(&flash_colour);
        overlay.add_overlay(&brightness);
        overlay.add_overlay(&flash);
        Self {
            brightness,
            blend,
            flash,
            brightness_colour,
            flash_colour,
            flash_generation: Rc::new(Cell::new(0)),
            flash_trigger: Cell::new(0),
        }
    }

    /// Applies the brightness, colour filter, grayscale and invert preferences. `page` is the page area widget.
    pub fn apply(&self, prefs: &ReaderPrefs, page: &impl IsA<gtk::Widget>) {
        let alpha = brightness_alpha(prefs.custom_brightness.get(), prefs.custom_brightness_value.get());
        self.brightness_colour.set((0.0, 0.0, 0.0, alpha));
        self.brightness.set_visible(alpha > 0.0);
        self.brightness.queue_draw();

        self.blend.set_filter(filter_rgba(prefs.color_filter.get(), prefs.color_filter_value.get()), prefs.color_filter_mode.get());

        for class in FILTER_CLASSES {
            page.remove_css_class(class);
        }
        if let Some(class) = filter_class(prefs.grayscale.get(), prefs.inverted_colors.get()) {
            page.add_css_class(class);
        }
    }

    /// A page changed. Counts it toward the flash interval and starts a flash when it is due. Called only while the
    /// flash feature is on (E-FLASH-1), so a later enable does not flash at once.
    pub fn on_page_changed(&self, prefs: &ReaderPrefs) {
        let (due, next) = rules::flash_due(self.flash_trigger.get(), pref_rules::flash_interval(prefs.flash_page_interval.get()));
        self.flash_trigger.set(next);
        if due {
            self.start_flash(prefs.flash_color.get(), pref_rules::flash_duration_ms(prefs.flash_duration_millis.get()));
        }
    }

    /// Clears the interval counter and any flash on screen, for when the feature is turned off.
    pub fn reset_flash(&self) {
        self.flash_trigger.set(0);
        self.flash_generation.set(self.flash_generation.get() + 1);
        self.flash.set_visible(false);
    }

    fn start_flash(&self, colour: FlashColor, duration_ms: i32) {
        let generation = self.flash_generation.get() + 1;
        self.flash_generation.set(generation);
        let duration = Duration::from_millis(duration_ms.max(1) as u64);
        let phases = rules::flash_phases(colour, duration);
        let paint = |cell: &Rc<Cell<Rgba>>, rgb: (f64, f64, f64)| cell.set((rgb.0, rgb.1, rgb.2, 1.0));
        paint(&self.flash_colour, phases[0].rgb);
        self.flash.set_visible(true);
        self.flash.queue_draw();
        let current = Rc::clone(&self.flash_generation);
        if let Some(second) = phases.get(1).copied() {
            let (area, cell, current) = (self.flash.clone(), Rc::clone(&self.flash_colour), Rc::clone(&current));
            glib::timeout_add_local_once(second.start, move || {
                if current.get() == generation {
                    cell.set((second.rgb.0, second.rgb.1, second.rgb.2, 1.0));
                    area.queue_draw();
                }
            });
        }
        let area = self.flash.clone();
        glib::timeout_add_local_once(duration, move || {
            if current.get() == generation {
                area.set_visible(false);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_channel_mask_write() {
        let packed = 0x1122_3344_u32 as i32;
        assert_eq!(channel_set(packed, Channel::Red, 0xAA) as u32, 0x11AA_3344);
        assert_eq!(channel_set(packed, Channel::Alpha, 0xFF) as u32, 0xFF22_3344);
        assert_eq!(channel_set(packed, Channel::Green, 0) as u32, 0x1122_0044);
        assert_eq!(channel_set(packed, Channel::Blue, 300) as u32, 0x1122_33FF, "a value is clamped to a byte");
        assert_eq!(channel_get(packed, Channel::Green), 0x33);
        assert_eq!(channel_get(0xFF00_0000_u32 as i32, Channel::Alpha), 255);
    }

    #[test]
    fn brightness_only_darkens_for_negative_values_while_on() {
        assert_eq!(brightness_alpha(true, -50), 0.5);
        assert_eq!(brightness_alpha(true, -75), 0.75);
        assert_eq!(brightness_alpha(true, -200), 0.75, "the value is held to the slider range");
        assert_eq!(brightness_alpha(true, 0), 0.0);
        assert_eq!(brightness_alpha(true, 40), 0.0);
        assert_eq!(brightness_alpha(false, -50), 0.0, "off hides the layer whatever the value");
    }

    #[test]
    fn the_filter_colour_is_unpacked_and_hidden_when_off() {
        assert_eq!(filter_rgba(false, 0x80FF_0000_u32 as i32), None);
        let (r, g, b, a) = filter_rgba(true, 0x80FF_0000_u32 as i32).unwrap();
        assert_eq!((r, g, b), (1.0, 0.0, 0.0));
        assert!((a - 128.0 / 255.0).abs() < 1e-9);
    }

    #[test]
    fn grayscale_and_invert_are_off_by_default_and_combine_in_order() {
        assert_eq!(filter_class(false, false), None);
        assert_eq!(filter_class(true, false), Some("reader-gray"));
        assert_eq!(filter_class(false, true), Some("reader-invert"));
        assert_eq!(filter_class(true, true), Some("reader-gray-invert"));
    }
}
