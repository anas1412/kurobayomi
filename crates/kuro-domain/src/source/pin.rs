//! Pin flags of a source (`Pin.kt`). Two bits: `Pinned` and `Actual`. The Kotlin `contains` is a plain mask test,
//! so `UNPINNED.contains(Unpinned)` is true (Q-PIN-1); [`Pins::is_pinned`] is the check callers want.

/// One pin bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Pin {
    Unpinned = 0b00,
    Pinned = 0b01,
    Actual = 0b10,
}

/// The pin set of a source, two bits. The plan calls it `PinFlags` (T-17a-002); both names are this type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pins(u8);

pub type PinFlags = Pins;

impl Pins {
    /// The `Pinned` bit (`Pin.Pinned`).
    pub const PINNED_BIT: u8 = 0b01;
    /// The `Actual` bit (`Pin.Actual`).
    pub const ACTUAL_BIT: u8 = 0b10;
    pub const UNPINNED: Pins = Pins(0b00);
    /// `Pinned | Actual`.
    pub const PINNED: Pins = Pins(0b11);

    pub fn bits(self) -> u8 {
        self.0
    }

    /// Keeps the two low bits only (E-PIN-01).
    pub fn from_bits(bits: u8) -> Self {
        Pins(bits & 0b11)
    }

    /// `(flag & self) == flag`, as upstream.
    pub fn contains(self, flag: Pin) -> bool {
        (flag as u8) & self.0 == flag as u8
    }

    pub fn with(self, flag: Pin) -> Pins {
        Pins(self.0 | flag as u8)
    }

    /// Clears one bit (AND-NOT). Upstream uses XOR here (Q-PIN-2), which this port fixes.
    pub fn without(self, flag: Pin) -> Pins {
        Pins(self.0 & !(flag as u8))
    }

    /// Kotlin `Pins.minus`, which is an exclusive or: `UNPINNED.minus(Actual)` gives `Actual` (EC-34, a quirk that the
    /// enabled list reproduces). Use [`Pins::without`] for the clearing meaning.
    pub fn minus(self, flag: Pin) -> Pins {
        Pins(self.0 ^ flag as u8)
    }

    /// True only for the full pinned set.
    pub fn is_pinned(self) -> bool {
        self == Pins::PINNED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_a_mask_test_and_is_pinned_is_exact() {
        assert!(Pins::UNPINNED.contains(Pin::Unpinned), "Q-PIN-1");
        assert!(!Pins::UNPINNED.contains(Pin::Pinned));
        assert!(Pins::PINNED.contains(Pin::Pinned));
        assert!(Pins::PINNED.contains(Pin::Actual));
        assert!(Pins::PINNED.is_pinned());
        assert!(!Pins::UNPINNED.is_pinned());
        assert!(Pins::UNPINNED.with(Pin::Pinned).with(Pin::Actual).is_pinned());
    }

    #[test]
    fn without_clears_only_the_named_bit() {
        assert_eq!(Pins::PINNED.without(Pin::Actual).bits(), 0b01, "AND-NOT, not XOR");
        assert_eq!(Pins::PINNED.without(Pin::Pinned).bits(), 0b10);
        assert_eq!(Pins::UNPINNED.without(Pin::Pinned), Pins::UNPINNED);
    }

    #[test]
    fn from_bits_masks_to_two_bits() {
        assert_eq!(Pins::from_bits(0b1111_1101).bits(), 0b01);
        assert_eq!(Pins::default(), Pins::UNPINNED);
    }

    #[test]
    fn pin_codes_and_the_pinned_set_match_the_kotlin_constants() {
        assert_eq!((Pin::Unpinned as u8, Pin::Pinned as u8, Pin::Actual as u8), (0b00, 0b01, 0b10));
        assert_eq!(Pins::PINNED.bits(), 0b11);
        assert_eq!(Pins::UNPINNED.bits(), 0b00);
    }

    #[test]
    fn the_flag_constants_and_the_zero_bit_test_match_the_plan() {
        assert_eq!((PinFlags::PINNED.bits(), PinFlags::UNPINNED.bits()), (0b11, 0b00));
        assert_eq!((Pins::PINNED_BIT, Pins::ACTUAL_BIT), (0b01, 0b10));
        assert!(PinFlags::UNPINNED.contains(Pin::Unpinned), "contains(0) is true for every value");
        assert!(Pins::from_bits(0).contains(Pin::Unpinned));
        assert_eq!(PinFlags::PINNED.without(Pin::Actual).bits(), 0b01);
        assert_eq!(PinFlags::UNPINNED.without(Pin::Actual).bits(), 0b00);
    }

    #[test]
    fn tc_pin_01_to_06() {
        // TC-PIN-01
        assert!(Pins::PINNED.contains(Pin::Pinned) && Pins::PINNED.contains(Pin::Actual));
        // TC-PIN-02
        assert!(!Pins::UNPINNED.contains(Pin::Pinned));
        // TC-PIN-03
        assert_eq!(Pins::UNPINNED.with(Pin::Pinned).with(Pin::Actual), Pins::PINNED);
        // TC-PIN-04 (E-SRC-07): pin then unpin returns to exactly UNPINNED
        assert_eq!(Pins::PINNED.without(Pin::Pinned).without(Pin::Actual), Pins::UNPINNED);
        // TC-PIN-05 (Q-PIN-1): the mask test is true for the zero flag
        assert!(Pins::UNPINNED.contains(Pin::Unpinned));
        // TC-PIN-06: from_bits masks to two bits
        assert_eq!(Pins::from_bits(0xFF), Pins::PINNED);
        assert_eq!(Pins::from_bits(0b100), Pins::UNPINNED, "E-PIN-01");
    }
}
