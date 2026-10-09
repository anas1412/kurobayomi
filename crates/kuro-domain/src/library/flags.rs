//! Flag algebra of `Flag.kt`: the two operators on a masked flag word, written as functions.

use kuro_db::flags::set_flag;

/// Kotlin `Long + Flag` for a flag that carries a mask: `(value & !mask) | (flag & mask)`. Bits outside
/// `mask` are kept, bits inside `mask` take the value of `flag`.
pub const fn merge(value: i64, flag: i64, mask: i64) -> i64 {
    set_flag(value, flag, mask)
}

/// Kotlin `Long.contains(Flag)` for a masked flag: true when the masked bits of `value` equal `flag`.
pub const fn contains(value: i64, flag: i64, mask: i64) -> bool {
    flag == (value & mask)
}

/// Kotlin `Long + Flag` for a flag without a mask: plain OR.
pub const fn add_unmasked(value: i64, flag: i64) -> i64 {
    value | flag
}
