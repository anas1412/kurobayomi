//! Backup options model (area 21, T-21a-010): defaults, the 12-slot bool array, `can_create`, disable and
//! reset rules. Options are never persisted (B-03, p1-data-and-config section 6).

/// The twelve toggles in `asBooleanArray()` order (B-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupOptions {
    pub library_entries: bool,
    pub categories: bool,
    pub chapters: bool,
    pub tracking: bool,
    pub history: bool,
    pub read_entries: bool,
    pub app_settings: bool,
    pub extension_stores: bool,
    pub custom_button: bool,
    pub source_settings: bool,
    pub private_settings: bool,
    pub extensions: bool,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            library_entries: true,
            categories: true,
            chapters: true,
            tracking: true,
            history: true,
            read_entries: true,
            app_settings: true,
            extension_stores: true,
            custom_button: true,
            source_settings: true,
            private_settings: false,
            extensions: false,
        }
    }
}

/// Number of toggles in the bool array.
pub const OPTION_COUNT: usize = 12;

impl BackupOptions {
    /// Index order of B-02: library entries 0 .. extensions 11.
    pub fn as_bool_array(&self) -> [bool; OPTION_COUNT] {
        [
            self.library_entries,
            self.categories,
            self.chapters,
            self.tracking,
            self.history,
            self.read_entries,
            self.app_settings,
            self.extension_stores,
            self.custom_button,
            self.source_settings,
            self.private_settings,
            self.extensions,
        ]
    }

    /// Reads the first twelve entries. A shorter slice returns `None` (B-02 would throw).
    pub fn from_bool_array(a: &[bool]) -> Option<Self> {
        let a: [bool; OPTION_COUNT] = a.get(..OPTION_COUNT)?.try_into().ok()?;
        Some(Self {
            library_entries: a[0],
            categories: a[1],
            chapters: a[2],
            tracking: a[3],
            history: a[4],
            read_entries: a[5],
            app_settings: a[6],
            extension_stores: a[7],
            custom_button: a[8],
            source_settings: a[9],
            private_settings: a[10],
            extensions: a[11],
        })
    }

    /// The Create button rule (B-02, E-06). Chapters, tracking, history, read entries, sensitive settings and
    /// extensions alone do not enable it.
    pub fn can_create(&self) -> bool {
        self.library_entries || self.categories || self.app_settings || self.extension_stores || self.custom_button || self.source_settings
    }

    /// "Include sensitive settings" applies only while App or Source settings is on (E-27).
    pub fn private_enabled(&self) -> bool {
        self.app_settings || self.source_settings
    }

    /// Private keys are written only when the option is on and a settings parent is on.
    pub fn effective_private(&self) -> bool {
        self.private_settings && self.private_enabled()
    }

    /// Chapters, tracking, history and read entries are disabled while library entries is off (B-03).
    pub fn entry_children_enabled(&self) -> bool {
        self.library_entries
    }

    pub fn set_library_entries(&mut self, on: bool) {
        self.library_entries = on;
    }

    pub fn set_app_settings(&mut self, on: bool) {
        self.app_settings = on;
        self.reset_private_if_disabled();
    }

    pub fn set_source_settings(&mut self, on: bool) {
        self.source_settings = on;
        self.reset_private_if_disabled();
    }

    fn reset_private_if_disabled(&mut self) {
        if !self.private_enabled() {
            self.private_settings = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_kotlin() {
        let d = BackupOptions::default();
        let a = d.as_bool_array();
        assert_eq!(a.iter().filter(|b| **b).count(), 10);
        assert!(!d.private_settings);
        assert!(!d.extensions);
    }

    #[test]
    fn as_bool_array_order_matches_b02() {
        let mut o = BackupOptions { library_entries: false, categories: false, chapters: false, tracking: false, history: false, read_entries: false, app_settings: false, extension_stores: false, custom_button: false, source_settings: false, private_settings: false, extensions: false };
        o.extensions = true;
        assert!(o.as_bool_array()[11]);
        o.extensions = false;
        o.read_entries = true;
        assert!(o.as_bool_array()[5]);
        o.read_entries = false;
        o.private_settings = true;
        assert!(o.as_bool_array()[10]);
    }

    #[test]
    fn from_bool_array_roundtrip_all_combinations() {
        for bits in 0u32..(1 << OPTION_COUNT) {
            let arr: Vec<bool> = (0..OPTION_COUNT).map(|i| bits & (1 << i) != 0).collect();
            let o = BackupOptions::from_bool_array(&arr).unwrap();
            assert_eq!(o.as_bool_array().to_vec(), arr);
        }
    }

    #[test]
    fn from_short_array_is_none() {
        assert_eq!(BackupOptions::from_bool_array(&[true; 11]), None);
    }

    #[test]
    fn can_create_matrix_over_six_fields() {
        // Six fields that decide canCreate(); the other six are fixed at false here and must not matter.
        for bits in 0u32..64 {
            let mut o = BackupOptions { library_entries: false, categories: false, chapters: false, tracking: false, history: false, read_entries: false, app_settings: false, extension_stores: false, custom_button: false, source_settings: false, private_settings: false, extensions: false };
            o.library_entries = bits & 1 != 0;
            o.categories = bits & 2 != 0;
            o.app_settings = bits & 4 != 0;
            o.extension_stores = bits & 8 != 0;
            o.custom_button = bits & 16 != 0;
            o.source_settings = bits & 32 != 0;
            assert_eq!(o.can_create(), bits != 0);
        }
        let only_extensions = BackupOptions { library_entries: false, categories: false, chapters: false, tracking: false, history: false, read_entries: false, app_settings: false, extension_stores: false, custom_button: false, source_settings: false, private_settings: false, extensions: true };
        assert!(!only_extensions.can_create());
        let only_history = BackupOptions { history: true, ..only_extensions };
        assert!(!only_history.can_create());
    }

    #[test]
    fn disable_rules() {
        let mut o = BackupOptions::default();
        assert!(o.entry_children_enabled());
        o.library_entries = false;
        assert!(!o.entry_children_enabled());
        assert!(o.private_enabled());
        o.app_settings = false;
        o.source_settings = false;
        assert!(!o.private_enabled());
    }

    #[test]
    fn toggle_keeps_children() {
        let mut o = BackupOptions { chapters: false, history: false, ..Default::default() };
        o.set_library_entries(false);
        assert!(!o.chapters && !o.history && o.tracking && o.read_entries);
        o.set_library_entries(true);
        assert!(!o.chapters && !o.history && o.tracking && o.read_entries);
    }

    #[test]
    fn private_resets_when_both_parents_off() {
        let mut o = BackupOptions { private_settings: true, ..Default::default() };
        o.set_app_settings(false);
        assert!(o.private_settings, "source settings still on");
        o.set_source_settings(false);
        assert!(!o.private_settings, "E-27: both parents off resets the option");
        assert!(!o.effective_private());
    }
}
