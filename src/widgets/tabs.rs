//! Tab label with an optional count pill (T-49b-022). One line with an ellipsis; the pill shows when
//! the count is `Some`, including zero.

/// Alpha of the onBackground pill background: 12% in dark, 8% in light.
pub fn pill_alpha(dark: bool) -> f32 {
    if dark { 0.12 } else { 0.08 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pill_alpha_per_mode() {
        assert_eq!(pill_alpha(true), 0.12);
        assert_eq!(pill_alpha(false), 0.08);
    }

}
