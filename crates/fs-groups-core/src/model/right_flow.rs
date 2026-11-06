//! The setting of a group that says whether and where its rights are passed on (ADR 0055 point 6).
//!
//! Pattern: Value object — only the setting and its stored text. What the setting *means* is decided
//! in one place, `crate::right_flow` (provisional).

/// Whether and in which direction a group passes its rights on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RightFlow {
    /// Not at all: a right on the group counts for the group only. The default.
    #[default]
    None,
    /// To the children, onward through descendants that also pass down.
    Down,
    /// To the parents, onward through ancestors that also pass up.
    Up,
}

impl RightFlow {
    /// Stable text used in the AS2 content (`fs:rightFlow`).
    pub fn as_str(self) -> &'static str {
        match self {
            RightFlow::None => "none",
            RightFlow::Down => "down",
            RightFlow::Up => "up",
        }
    }

    /// Inverse of [`RightFlow::as_str`].
    pub fn parse(s: &str) -> Option<RightFlow> {
        [RightFlow::None, RightFlow::Down, RightFlow::Up]
            .into_iter()
            .find(|f| f.as_str() == s)
    }
}
