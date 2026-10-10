//! Base activity and theming lifecycle (Kotlin `BaseActivity`, `ThemingDelegate` and `prepareTabletUiContext`, T-36a-061).
//!
//! Two rules: the theme is applied before the first widget is built, and the tablet context is applied before the layout
//! is measured. `Lifecycle` keeps the order and refuses a step that runs early.

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
}
