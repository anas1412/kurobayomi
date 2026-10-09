//! RestoreOptions: seven independent flags, defaults, labels and the bit array (area 22, T-22a-002, B-02 of 22).

/// The seven restore toggles. Unlike backup creation, no option depends on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreOptions {
    pub library_entries: bool,
    pub categories: bool,
    pub app_settings: bool,
    pub extension_stores: bool,
    pub custom_buttons: bool,
    pub source_settings: bool,
    pub extensions: bool,
}

/// Number of toggles in the bit array.
pub const OPTION_COUNT: usize = 7;

impl RestoreOptions {
    /// English labels in bit index order.
    pub const LABELS: [&'static str; OPTION_COUNT] = {
        let t = crate::text::RESTORE_OPTION_LABELS;
        [t[0].english, t[1].english, t[2].english, t[3].english, t[4].english, t[5].english, t[6].english]
    };

    /// Index order of the bit array: library, categories, app, stores, buttons, source, extensions.
    pub fn to_bits(self) -> [bool; OPTION_COUNT] {
        [
            self.library_entries,
            self.categories,
            self.app_settings,
            self.extension_stores,
            self.custom_buttons,
            self.source_settings,
            self.extensions,
        ]
    }

    pub fn from_bits(b: [bool; OPTION_COUNT]) -> Self {
        Self {
            library_entries: b[0],
            categories: b[1],
            app_settings: b[2],
            extension_stores: b[3],
            custom_buttons: b[4],
            source_settings: b[5],
            extensions: b[6],
        }
    }

    /// Reads the first seven entries. A shorter slice is `None` (the Kotlin version throws here).
    pub fn from_slice(b: &[bool]) -> Option<Self> {
        let arr: [bool; OPTION_COUNT] = b.get(..OPTION_COUNT)?.try_into().ok()?;
        Some(Self::from_bits(arr))
    }

    /// The Restore action is enabled when any flag is set.
    pub fn can_restore(self) -> bool {
        self.to_bits().iter().any(|on| *on)
    }

    /// Checkbox handler. Returns `false` when `index` is outside the seven toggles.
    pub fn set(&mut self, index: usize, value: bool) -> bool {
        let mut bits = self.to_bits();
        match bits.get_mut(index) {
            Some(slot) => {
                *slot = value;
                *self = Self::from_bits(bits);
                true
            }
            None => false,
        }
    }
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            library_entries: true,
            categories: true,
            app_settings: true,
            extension_stores: true,
            custom_buttons: true,
            source_settings: true,
            extensions: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_six_on_extensions_off() {
        let d = RestoreOptions::default();
        assert_eq!(d.to_bits(), [true, true, true, true, true, true, false]);
        assert!(d.can_restore());
    }

    #[test]
    fn bits_round_trip_all_128_combinations() {
        for n in 0u32..(1 << OPTION_COUNT) {
            let bits: [bool; OPTION_COUNT] = std::array::from_fn(|i| n & (1 << i) != 0);
            assert_eq!(RestoreOptions::from_bits(bits).to_bits(), bits);
        }
    }

    #[test]
    fn can_restore_false_only_when_all_seven_off() {
        for n in 0u32..(1 << OPTION_COUNT) {
            let bits: [bool; OPTION_COUNT] = std::array::from_fn(|i| n & (1 << i) != 0);
            assert_eq!(RestoreOptions::from_bits(bits).can_restore(), n != 0);
        }
    }

    #[test]
    fn labels_are_in_order() {
        assert_eq!(
            RestoreOptions::LABELS,
            ["Library", "Categories", "App settings", "Extension stores", "Custom buttons", "Source settings", "Extensions"]
        );
    }

    #[test]
    fn set_by_index_and_reject_out_of_range() {
        let mut o = RestoreOptions::default();
        assert!(o.set(6, true));
        assert!(o.extensions);
        assert!(o.set(0, false));
        assert!(!o.library_entries);
        assert!(!o.set(7, true));
    }

    #[test]
    fn short_slice_is_none() {
        assert_eq!(RestoreOptions::from_slice(&[true; 6]), None);
        assert!(RestoreOptions::from_slice(&[false; 7]).is_some());
    }
}
