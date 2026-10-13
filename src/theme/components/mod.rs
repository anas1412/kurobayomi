//! GTK components built on the theme classes. Pure helpers are unit-tested; the builders are thin.

/// CSS class names. Rules for each live in `css::component_rules`.
pub mod classes {
    #[cfg(test)]
    pub const PILL: &str = "kuro-pill";
    #[cfg(test)]
    pub const GROUP_HEADER: &str = "kuro-group-header";
}


#[cfg(test)]
mod tests {
    use super::classes;

    #[test]
    fn class_names_are_kuro_prefixed() {
        for name in [classes::PILL, classes::GROUP_HEADER] {
            assert!(name.starts_with("kuro-"), "{name}");
        }
    }
}
