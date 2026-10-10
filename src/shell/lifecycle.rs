//! Base activity and theming lifecycle (Kotlin `BaseActivity`, `ThemingDelegate` and `prepareTabletUiContext`, T-36a-061).
//!
//! Two rules: the theme is applied before the first widget is built, and the tablet context is applied before the layout
//! is measured. `Lifecycle` keeps the order and refuses a step that runs early. The tablet rule (behaviour section 8.1) is
//! `override_smallest_width`.

use kuro_prefs::ui::TabletUiMode;

/// `TABLET_UI_REQUIRED_SCREEN_WIDTH_DP`: a smallest width from this value is a tablet.
pub const TABLET_MIN_DP: i32 = 720;
/// `TABLET_UI_MIN_SCREEN_WIDTH_PORTRAIT_DP`, the Automatic mode threshold in portrait.
pub const AUTOMATIC_PORTRAIT_DP: i32 = 700;
/// `TABLET_UI_MIN_SCREEN_WIDTH_LANDSCAPE_DP`, the Automatic mode threshold in landscape.
pub const AUTOMATIC_LANDSCAPE_DP: i32 = 600;

/// The step that ran too early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    /// A widget was built before the theme was applied.
    ThemeNotApplied,
    /// The layout was measured before the tablet context was applied.
    TabletContextNotApplied,
}

/// The order of the window's first steps. Both flags start false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lifecycle {
    theme: bool,
    tablet: bool,
}

impl Lifecycle {
    pub fn new() -> Self {
        Self::default()
    }

    /// The theme is applied (`applyAppTheme`). Runs before any widget.
    pub fn apply_theme(&mut self) {
        self.theme = true;
    }

    /// The tablet context is applied (`prepareTabletUiContext`). Runs before the layout is measured.
    pub fn apply_tablet_context(&mut self) {
        self.tablet = true;
    }

    /// A widget may be built only after the theme is applied.
    pub fn build_widget(&self) -> Result<(), LifecycleError> {
        if self.theme { Ok(()) } else { Err(LifecycleError::ThemeNotApplied) }
    }

    /// The layout may be measured only after the theme and the tablet context are applied.
    pub fn measure(&self) -> Result<(), LifecycleError> {
        self.build_widget()?;
        if self.tablet { Ok(()) } else { Err(LifecycleError::TabletContextNotApplied) }
    }
}

/// Whether the screen counts as a tablet UI at this smallest width.
pub fn is_tablet_ui(smallest_width_dp: i32) -> bool {
    smallest_width_dp >= TABLET_MIN_DP
}

/// Whether the tablet UI is expected for the user's mode (`prepareTabletUiContext`, `expected`).
pub fn expected_tablet_ui(mode: TabletUiMode, smallest_width_dp: i32, landscape: bool) -> bool {
    match mode {
        TabletUiMode::AUTOMATIC => {
            let threshold = if landscape { AUTOMATIC_LANDSCAPE_DP } else { AUTOMATIC_PORTRAIT_DP };
            smallest_width_dp >= threshold
        }
        TabletUiMode::ALWAYS => true,
        TabletUiMode::LANDSCAPE => landscape,
        TabletUiMode::NEVER => false,
    }
}

/// The smallest width the layout measures with: unchanged when it already matches the mode, otherwise raised to 720 for
/// a tablet or lowered to 719 for a phone.
pub fn override_smallest_width(mode: TabletUiMode, smallest_width_dp: i32, landscape: bool) -> i32 {
    let expected = expected_tablet_ui(mode, smallest_width_dp, landscape);
    if is_tablet_ui(smallest_width_dp) == expected {
        smallest_width_dp
    } else if expected {
        smallest_width_dp.max(TABLET_MIN_DP)
    } else {
        smallest_width_dp.min(TABLET_MIN_DP - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_widget_is_refused_until_the_theme_is_applied() {
        let mut life = Lifecycle::new();
        assert_eq!(life.build_widget(), Err(LifecycleError::ThemeNotApplied));
        life.apply_theme();
        assert_eq!(life.build_widget(), Ok(()));
    }

    #[test]
    fn measuring_is_refused_until_the_tablet_context_is_applied() {
        let mut life = Lifecycle::new();
        life.apply_theme();
        assert_eq!(life.measure(), Err(LifecycleError::TabletContextNotApplied));
        life.apply_tablet_context();
        assert_eq!(life.measure(), Ok(()));
    }

    #[test]
    fn automatic_uses_the_portrait_and_landscape_thresholds() {
        assert!(expected_tablet_ui(TabletUiMode::AUTOMATIC, 650, true));
        assert!(!expected_tablet_ui(TabletUiMode::AUTOMATIC, 650, false));
        assert!(expected_tablet_ui(TabletUiMode::AUTOMATIC, 700, false));
    }

    #[test]
    fn fixed_modes_ignore_the_width() {
        assert!(expected_tablet_ui(TabletUiMode::ALWAYS, 300, false));
        assert!(!expected_tablet_ui(TabletUiMode::NEVER, 900, true));
        assert!(expected_tablet_ui(TabletUiMode::LANDSCAPE, 300, true));
        assert!(!expected_tablet_ui(TabletUiMode::LANDSCAPE, 900, false));
    }

    #[test]
    fn a_phone_width_is_raised_to_the_tablet_minimum_when_the_tablet_is_expected() {
        assert_eq!(override_smallest_width(TabletUiMode::ALWAYS, 400, false), 720);
        assert_eq!(override_smallest_width(TabletUiMode::ALWAYS, 800, false), 800);
    }

    #[test]
    fn a_tablet_width_is_lowered_to_719_when_the_phone_is_expected() {
        assert_eq!(override_smallest_width(TabletUiMode::NEVER, 900, false), 719);
        assert_eq!(override_smallest_width(TabletUiMode::NEVER, 400, false), 400);
    }
}
