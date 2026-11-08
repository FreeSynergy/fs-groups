//! The words of the program (ADR 0057): all English text of this frontend in one place.
//!
//! Pattern: Value objects — plain texts; translation (ADR 0053) will replace this file's content,
//! not its users.

use fs_components_workbenches::people::PeopleLabels;

/// The title of the window.
pub const TITLE: &str = "FreeSynergy Groups";

/// The words of the workbench (it is the list-and-page workbench of people, used for groups).
pub fn labels() -> PeopleLabels {
    let mut labels = PeopleLabels::default();
    labels.search = "Search groups".into();
    labels.columns = vec!["Name".into(), "Kind".into()];
    labels
}

/// Headings and labels of what is shown about a group.
pub(crate) mod field {
    pub const NAME: &str = "Name";
    pub const KIND: &str = "Kind";
    pub const DETAILS: &str = "Details";
    pub const PARENTS: &str = "Part of";
    pub const MEMBERS: &str = "Members";
    pub const PLAIN_MEMBER: &str = "Member";
    pub const GROUP_OF: &str = "Group";
    pub const NO_NAME: &str = "A group needs a name.";
}
