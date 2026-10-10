//! Derived colours of the tinted slider (T-46a-036; `TintedSliderItem.kt:141-167`). Channels are `0.0..=1.0`.
//! Lightening is clamped to `1.0`: the source clamps to 255, which does nothing on a `0.0..=1.0` channel.

/// An RGB colour with alpha, channels `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// The alpha of every disabled variant.
pub const DISABLED_ALPHA: f32 = 0.5;

impl Rgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Scales each channel by `factor`, never below zero. The alpha is kept.
    pub fn darken(self, factor: f32) -> Self {
        Self {
            r: (self.r * factor).max(0.0),
            g: (self.g * factor).max(0.0),
            b: (self.b * factor).max(0.0),
            a: self.a,
        }
    }

    /// Moves each channel `factor` of the way to white, never above one. The alpha is kept.
    pub fn lighten(self, factor: f32) -> Self {
        Self {
            r: (self.r + (1.0 - self.r) * factor).min(1.0),
            g: (self.g + (1.0 - self.g) * factor).min(1.0),
            b: (self.b + (1.0 - self.b) * factor).min(1.0),
            a: self.a,
        }
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
}

/// The colours of one tinted slider, from one base tint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderTints {
    pub thumb: Rgba,
    pub active_track: Rgba,
    pub active_tick: Rgba,
    pub inactive_track: Rgba,
    pub inactive_tick: Rgba,
    pub disabled_thumb: Rgba,
    pub disabled_active_track: Rgba,
    pub disabled_active_tick: Rgba,
    pub disabled_inactive_track: Rgba,
    pub disabled_inactive_tick: Rgba,
}

/// Derives every slider colour from the base tint: the thumb is the tint itself, the active track lightens by 0.2 and
/// the active tick by 0.4, the inactive track darkens by 0.2 and the inactive tick by 0.4. Disabled variants use
/// `DISABLED_ALPHA`.
pub fn slider_tints(base: Rgba) -> SliderTints {
    let active_track = base.lighten(0.2);
    let active_tick = base.lighten(0.4);
    let inactive_track = base.darken(0.2);
    let inactive_tick = base.darken(0.4);
    let disabled = |c: Rgba| c.with_alpha(DISABLED_ALPHA);
    SliderTints {
        thumb: base,
        active_track,
        active_tick,
        inactive_track,
        inactive_tick,
        disabled_thumb: disabled(base),
        disabled_active_track: disabled(active_track),
        disabled_active_tick: disabled(active_tick),
        disabled_inactive_track: disabled(inactive_track),
        disabled_inactive_tick: disabled(inactive_tick),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Rgba, b: Rgba) -> bool {
        let near = |x: f32, y: f32| (x - y).abs() < 1e-5;
        near(a.r, b.r) && near(a.g, b.g) && near(a.b, b.b) && near(a.a, b.a)
    }

    #[test]
    fn a_red_tint_derives_the_expected_colours() {
        let t = slider_tints(Rgba::new(1.0, 0.0, 0.0, 1.0));
        assert!(close(t.thumb, Rgba::new(1.0, 0.0, 0.0, 1.0)));
        assert!(close(t.active_track, Rgba::new(1.0, 0.2, 0.2, 1.0)));
        assert!(close(t.active_tick, Rgba::new(1.0, 0.4, 0.4, 1.0)));
        assert!(close(t.inactive_track, Rgba::new(0.2, 0.0, 0.0, 1.0)));
        assert!(close(t.inactive_tick, Rgba::new(0.4, 0.0, 0.0, 1.0)));
    }

    #[test]
    fn a_mid_grey_tint_moves_each_step_from_half() {
        let t = slider_tints(Rgba::new(0.5, 0.5, 0.5, 1.0));
        assert!(close(t.active_track, Rgba::new(0.6, 0.6, 0.6, 1.0)));
        assert!(close(t.active_tick, Rgba::new(0.7, 0.7, 0.7, 1.0)));
        assert!(close(t.inactive_track, Rgba::new(0.1, 0.1, 0.1, 1.0)));
        assert!(close(t.inactive_tick, Rgba::new(0.2, 0.2, 0.2, 1.0)));
    }

    #[test]
    fn a_white_tint_lightens_to_one_and_darkens_to_grey() {
        let t = slider_tints(Rgba::new(1.0, 1.0, 1.0, 1.0));
        assert!(
            close(t.active_track, Rgba::new(1.0, 1.0, 1.0, 1.0)),
            "clamped at one"
        );
        assert!(close(t.inactive_track, Rgba::new(0.2, 0.2, 0.2, 1.0)));
    }

    #[test]
    fn disabled_variants_use_half_alpha_and_keep_the_colour() {
        let t = slider_tints(Rgba::new(1.0, 0.0, 0.0, 1.0));
        assert!(close(t.disabled_thumb, Rgba::new(1.0, 0.0, 0.0, 0.5)));
        assert!(close(
            t.disabled_active_track,
            Rgba::new(1.0, 0.2, 0.2, 0.5)
        ));
        assert!(close(
            t.disabled_inactive_tick,
            Rgba::new(0.4, 0.0, 0.0, 0.5)
        ));
    }
}
