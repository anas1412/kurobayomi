//! Double-tap feedback geometry and timing (area 46 `OvalBox.kt` and `DoubleTapSeekTriangles.kt`,
//! T-46a-017). Pure: the GTK layer turns `PathCmd`s into a cairo path and draws the triangles with
//! the alphas and rotation from here.

/// One step of a half-oval outline, in surface pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    /// Cubic bezier: first control point, second control point, end point.
    CubicTo((f32, f32), (f32, f32), (f32, f32)),
    Close,
}

/// Width of the oval box: 40% of the surface width.
pub fn oval_width(surface_width: f32) -> f32 {
    surface_width * 0.4
}

/// Left edge of the oval box. Forward seeks use the right side, backward seeks the left side.
pub fn oval_x(surface_width: f32, forward: bool) -> f32 {
    if forward {
        surface_width - oval_width(surface_width)
    } else {
        0.0
    }
}

/// Outline of the right half-oval (`RightSideOvalShape`, OvalBox.kt:41-53). The second control
/// point's x is the absolute value -30, as in the Kotlin.
pub fn right_oval(w: f32, h: f32) -> Vec<PathCmd> {
    let inset = w / 10.0;
    vec![
        PathCmd::MoveTo(w, h),
        PathCmd::LineTo(w, 0.0),
        PathCmd::LineTo(inset, 0.0),
        PathCmd::CubicTo((inset, 0.0), (-30.0, h / 2.0), (inset, h)),
        PathCmd::Close,
    ]
}

/// Outline of the left half-oval (`LeftSideOvalShape`, OvalBox.kt:65-77). Its control point sits
/// at `w` (scaled), as the Kotlin does, so it is not an exact mirror of `right_oval`.
pub fn left_oval(w: f32, h: f32) -> Vec<PathCmd> {
    let inset = w - w / 10.0;
    vec![
        PathCmd::MoveTo(0.0, 0.0),
        PathCmd::LineTo(0.0, h),
        PathCmd::LineTo(inset, h),
        PathCmd::CubicTo((inset, h), (w, h / 2.0), (inset, 0.0)),
        PathCmd::Close,
    ]
}

/// Fill alpha of the oval: 0.2 while the amount is not zero, 0 otherwise. The caller animates the
/// change between the two values.
pub fn oval_alpha(amount: i64) -> f32 {
    if amount == 0 { 0.0 } else { 0.2 }
}

/// Number of 150 ms steps in one triangle cycle.
pub const TRIANGLE_STEPS: u64 = 6;
/// Duration of one triangle step in milliseconds.
pub const TRIANGLE_STEP_MS: u64 = 150;

/// Alphas of the three triangles at `elapsed_ms` into the animation. The cycle runs six steps of
/// 150 ms (900 ms): each triangle fades in, then the three fade out in the same order. The phase is
/// discrete, one state per step.
pub fn triangle_alphas(elapsed_ms: u64) -> [f32; 3] {
    let phase = (elapsed_ms / TRIANGLE_STEP_MS) % TRIANGLE_STEPS;
    match phase {
        0 => [0.0, 0.0, 0.0],
        1 => [1.0, 0.0, 0.0],
        2 => [1.0, 1.0, 0.0],
        3 => [1.0, 1.0, 1.0],
        4 => [0.0, 1.0, 1.0],
        _ => [0.0, 0.0, 1.0],
    }
}

/// Rotation of the triangle row in degrees: 0 for forward, 180 for backward.
pub fn triangle_rotation(forward: bool) -> f32 {
    if forward { 0.0 } else { 180.0 }
}

/// The label under the oval: the given override text, or `N second(s)` with the signed amount. The
/// plural follows the absolute value, so -1 is `-1 second`.
pub fn feedback_text(amount: i64, override_text: Option<&str>) -> String {
    match override_text {
        Some(text) => text.to_owned(),
        None if amount.unsigned_abs() == 1 => format!("{amount} second"),
        None => format!("{amount} seconds"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oval_is_forty_percent_on_the_chosen_side() {
        assert!((oval_width(1000.0) - 400.0).abs() < 1e-4);
        assert!((oval_x(1000.0, true) - 600.0).abs() < 1e-4);
        assert_eq!(oval_x(1000.0, false), 0.0);
    }

    #[test]
    fn right_oval_follows_the_kotlin_path() {
        let path = right_oval(100.0, 50.0);
        assert_eq!(path[0], PathCmd::MoveTo(100.0, 50.0));
        assert_eq!(path[1], PathCmd::LineTo(100.0, 0.0));
        assert_eq!(path[2], PathCmd::LineTo(10.0, 0.0));
        assert_eq!(
            path[3],
            PathCmd::CubicTo((10.0, 0.0), (-30.0, 25.0), (10.0, 50.0))
        );
    }

    #[test]
    fn left_oval_is_the_mirror_with_the_scaled_control_point() {
        let path = left_oval(100.0, 50.0);
        assert_eq!(path[0], PathCmd::MoveTo(0.0, 0.0));
        assert_eq!(path[2], PathCmd::LineTo(90.0, 50.0));
        assert_eq!(
            path[3],
            PathCmd::CubicTo((90.0, 50.0), (100.0, 25.0), (90.0, 0.0))
        );
    }

    #[test]
    fn oval_fill_is_point_two_while_active() {
        assert_eq!(oval_alpha(0), 0.0);
        assert_eq!(oval_alpha(10), 0.2);
        assert_eq!(oval_alpha(-10), 0.2);
    }

    #[test]
    fn triangles_fade_in_then_out_over_nine_hundred_ms() {
        assert_eq!(triangle_alphas(0), [0.0, 0.0, 0.0]);
        assert_eq!(triangle_alphas(150), [1.0, 0.0, 0.0]);
        assert_eq!(triangle_alphas(300), [1.0, 1.0, 0.0]);
        assert_eq!(triangle_alphas(450), [1.0, 1.0, 1.0]);
        assert_eq!(triangle_alphas(600), [0.0, 1.0, 1.0]);
        assert_eq!(triangle_alphas(750), [0.0, 0.0, 1.0]);
        assert_eq!(triangle_alphas(900), triangle_alphas(0));
        assert_eq!(triangle_alphas(1050), triangle_alphas(150));
    }

    #[test]
    fn backward_triangles_are_rotated() {
        assert_eq!(triangle_rotation(true), 0.0);
        assert_eq!(triangle_rotation(false), 180.0);
    }

    #[test]
    fn feedback_text_uses_the_signed_plural() {
        assert_eq!(feedback_text(10, None), "10 seconds");
        assert_eq!(feedback_text(-20, None), "-20 seconds");
        assert_eq!(feedback_text(-1, None), "-1 second");
        assert_eq!(feedback_text(0, Some("Skipped")), "Skipped");
    }
}
