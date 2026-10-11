//! Base activity and theming lifecycle (Kotlin `BaseActivity` and `ThemingDelegate`, T-36a-061).
//!
//! One rule: the theme is applied before the first widget is built. `Lifecycle` keeps the order and refuses a step
//! that runs early.

/// The step that ran too early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    /// A widget was built before the theme was applied.
    ThemeNotApplied,
}

/// The order of the window's first steps. The flag starts false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lifecycle {
    theme: bool,
}

impl Lifecycle {
    pub fn new() -> Self {
        Self::default()
    }

    /// The theme is applied (`applyAppTheme`). Runs before any widget.
    pub fn apply_theme(&mut self) {
        self.theme = true;
    }

    /// A widget may be built only after the theme is applied.
    pub fn build_widget(&self) -> Result<(), LifecycleError> {
        if self.theme { Ok(()) } else { Err(LifecycleError::ThemeNotApplied) }
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
}
