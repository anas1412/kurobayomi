//! Pager direction mapping and move decisions (T-44a-022, T-44a-024, B§6.3, E-13, E-14, E-15).
//!
//! The pager speaks in physical directions (`Left`, `Right`) and in reading-order commands (`Next`, `Prev`). For
//! right-to-left the reading order runs against the list, so `Next` is a physical `Left`.

use crate::reader::viewers::ViewerKind;

/// A command from a key, tap or wheel: the reading-order step, or an absolute physical direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageCmd {
    Next,
    Prev,
    Left,
    Right,
    /// The first page of the list (Home). Not a step: `decide` jumps straight to it.
    First,
    /// The last page of the list (End). Not a step: `decide` jumps straight to it.
    Last,
}

/// A physical direction on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phys {
    Left,
    Right,
}

/// What a command does at the current index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveDecision {
    /// Pan the current wide page one viewport width before turning it.
    Pan(Phys),
    /// Move the strip to `index`; `smooth` selects the animation.
    Turn { index: usize, smooth: bool },
    /// The command has nowhere to go (no wrap, no chapter change).
    Nothing,
}

/// The physical direction a command moves in for `kind` (`moveToNext` is `moveLeft` for right-to-left).
pub fn physical(kind: ViewerKind, cmd: PageCmd) -> Phys {
    let rtl = kind == ViewerKind::RightToLeft;
    match cmd {
        PageCmd::Left => Phys::Left,
        PageCmd::Right => Phys::Right,
        PageCmd::Next if rtl => Phys::Left,
        PageCmd::Next => Phys::Right,
        PageCmd::Prev if rtl => Phys::Right,
        PageCmd::Prev => Phys::Left,
        // The absolute jumps have no step; `decide` handles them before this is asked.
        PageCmd::First => Phys::Left,
        PageCmd::Last => Phys::Right,
    }
}

/// The index a physical step lands on, or `None` at either end of the list.
pub fn step(dir: Phys, index: usize, len: usize) -> Option<usize> {
    match dir {
        Phys::Right if index + 1 < len => Some(index + 1),
        Phys::Left if index > 0 => Some(index - 1),
        _ => None,
    }
}

/// Decides a command (B§6.3). `can_pan` is the current page's pan capability in that direction; the page must
/// have more than one pixel left to pan (E-13), which the caller decides.
pub fn decide(kind: ViewerKind, cmd: PageCmd, index: usize, len: usize, navigate_pan: bool, can_pan: bool, smooth: bool) -> MoveDecision {
    let last = len.saturating_sub(1);
    match cmd {
        PageCmd::First if index > 0 => return MoveDecision::Turn { index: 0, smooth },
        PageCmd::Last if index < last => return MoveDecision::Turn { index: last, smooth },
        PageCmd::First | PageCmd::Last => return MoveDecision::Nothing,
        _ => {}
    }
    let dir = physical(kind, cmd);
    match step(dir, index, len) {
        None => MoveDecision::Nothing,
        Some(_) if navigate_pan && can_pan => MoveDecision::Pan(dir),
        Some(target) => MoveDecision::Turn { index: target, smooth },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ViewerKind::*;

    #[test]
    fn next_and_prev_follow_the_reading_direction() {
        assert_eq!(physical(LeftToRight, PageCmd::Next), Phys::Right);
        assert_eq!(physical(LeftToRight, PageCmd::Prev), Phys::Left);
        assert_eq!(physical(RightToLeft, PageCmd::Next), Phys::Left);
        assert_eq!(physical(RightToLeft, PageCmd::Prev), Phys::Right);
        assert_eq!(physical(Vertical, PageCmd::Next), Phys::Right);
    }

    #[test]
    fn physical_commands_ignore_the_reading_direction() {
        assert_eq!(physical(RightToLeft, PageCmd::Right), Phys::Right);
        assert_eq!(physical(RightToLeft, PageCmd::Left), Phys::Left);
    }

    #[test]
    fn no_move_at_either_end_and_no_wrap() {
        assert_eq!(step(Phys::Left, 0, 5), None);
        assert_eq!(step(Phys::Right, 4, 5), None);
        assert_eq!(step(Phys::Right, 3, 5), Some(4));
        assert_eq!(decide(LeftToRight, PageCmd::Prev, 0, 5, true, true, true), MoveDecision::Nothing);
        assert_eq!(decide(LeftToRight, PageCmd::Next, 4, 5, true, true, true), MoveDecision::Nothing);
    }

    #[test]
    fn right_to_left_next_steps_to_a_lower_index() {
        assert_eq!(decide(RightToLeft, PageCmd::Next, 3, 5, false, false, false), MoveDecision::Turn { index: 2, smooth: false });
        assert_eq!(decide(RightToLeft, PageCmd::Prev, 3, 5, false, false, true), MoveDecision::Turn { index: 4, smooth: true });
    }

    #[test]
    fn home_and_end_jump_to_the_first_and_last_page_and_do_nothing_when_already_there() {
        assert_eq!(decide(LeftToRight, PageCmd::First, 3, 5, true, true, true), MoveDecision::Turn { index: 0, smooth: true });
        assert_eq!(decide(RightToLeft, PageCmd::Last, 1, 5, false, false, false), MoveDecision::Turn { index: 4, smooth: false });
        assert_eq!(decide(LeftToRight, PageCmd::First, 0, 5, true, true, true), MoveDecision::Nothing);
        assert_eq!(decide(LeftToRight, PageCmd::Last, 4, 5, true, true, true), MoveDecision::Nothing);
        assert_eq!(decide(LeftToRight, PageCmd::Last, 0, 0, true, true, true), MoveDecision::Nothing, "an empty list has no last page");
    }

    #[test]
    fn a_pannable_page_pans_before_it_turns() {
        assert_eq!(decide(LeftToRight, PageCmd::Next, 1, 5, true, true, true), MoveDecision::Pan(Phys::Right));
        assert_eq!(decide(LeftToRight, PageCmd::Next, 1, 5, false, true, true), MoveDecision::Turn { index: 2, smooth: true });
        assert_eq!(decide(LeftToRight, PageCmd::Next, 1, 5, true, false, true), MoveDecision::Turn { index: 2, smooth: true });
    }
}
